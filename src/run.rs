use crate::abi;
use crate::compare::Observation;
use crate::diag::{self, Diagnostic, Level};
use crate::process::{self, Captured};
use std::path::Path;

const STDERR_TAIL: usize = 2048;

fn stderr_tail(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(STDERR_TAIL);
    String::from_utf8_lossy(&bytes[start..]).trim().to_string()
}

fn execute(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
    level: Level,
) -> Result<Captured, Vec<Diagnostic>> {
    let request = abi::encode_request(payload)
        .map_err(|error| vec![error.diagnostic(fixture, case, level)])?;
    let mut command = process::sanitized(binary);
    let captured =
        process::run(&mut command, Some(&request), process::HARNESS_TIMEOUT).map_err(|error| {
            vec![Diagnostic::new(
                diag::SPAWN,
                fixture,
                Some(case),
                Some(level),
                format!("cannot start harness: {error}"),
            )]
        })?;
    let mut errors = Vec::new();
    if captured.truncated {
        errors.push(diag::truncation(
            fixture,
            Some(case),
            Some(level),
            format!(
                "harness output exceeded {} byte budget",
                process::OUTPUT_CAP
            ),
        ));
    }
    if captured.timed_out {
        errors.push(Diagnostic::new(
            diag::TIMEOUT,
            fixture,
            Some(case),
            Some(level),
            format!(
                "harness exceeded {}ms timeout",
                process::HARNESS_TIMEOUT.as_millis()
            ),
        ));
    }
    if let Some(error) = &captured.write_error {
        let (code, message) = if captured.success {
            (
                diag::NONZERO_EXIT,
                "harness exited before accepting request",
            )
        } else {
            (diag::SPAWN, "cannot write request")
        };
        errors.push(Diagnostic::new(
            code,
            fixture,
            Some(case),
            Some(level),
            format!("{message}: {error}"),
        ));
    }
    if errors.is_empty() {
        Ok(captured)
    } else {
        Err(errors)
    }
}

pub fn run(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
    level: Level,
) -> Result<(Observation, String), Vec<Diagnostic>> {
    let raw = execute(binary, payload, fixture, case, level)?;
    if !raw.success {
        return Err(vec![Diagnostic::new(
            diag::NONZERO_EXIT,
            fixture,
            Some(case),
            Some(level),
            format!(
                "harness exited non-zero; stderr tail: {}",
                stderr_tail(&raw.stderr)
            ),
        )]);
    }
    let response = abi::parse_response(&raw.stdout)
        .map_err(|error| vec![error.diagnostic(fixture, case, level)])?;
    Ok((response, String::from_utf8_lossy(&raw.stderr).into_owned()))
}

pub fn run_ubsan(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
) -> (Option<String>, Vec<Diagnostic>) {
    let raw = match execute(binary, payload, fixture, case, Level::O0) {
        Ok(raw) => raw,
        Err(errors) => return (None, errors),
    };
    let stderr = String::from_utf8_lossy(&raw.stderr).into_owned();
    if let Some(event) = first_ub(&stderr) {
        return (Some(event), Vec::new());
    }
    if !raw.success {
        return (
            None,
            vec![Diagnostic::new(
                diag::NONZERO_EXIT,
                fixture,
                Some(case),
                Some(Level::O0),
                format!(
                    "ubsan harness exited non-zero; stderr tail: {}",
                    stderr_tail(&raw.stderr)
                ),
            )],
        );
    }
    match abi::parse_response(&raw.stdout) {
        Ok(_) => (None, Vec::new()),
        Err(error) => (None, vec![error.diagnostic(fixture, case, Level::O0)]),
    }
}

pub fn first_ub(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .find(|line| line.contains("runtime error:"))
        .map(|line| {
            let start = line.find("runtime error:").unwrap();
            line[start..].trim().to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempScript(PathBuf);

    impl Drop for TempScript {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    impl std::ops::Deref for TempScript {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn script(text: &str) -> TempScript {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("c2tv-script-{}-{id}.sh", std::process::id()));
        let staged = path.with_extension("tmp");
        fs::write(&staged, format!("#!/bin/sh\ncat >/dev/null\n{text}\n")).unwrap();
        fs::rename(&staged, &path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        TempScript(path)
    }

    #[test]
    fn spawn_failure_is_typed() {
        let errors = run(
            Path::new("/nonexistent/c2tv-harness"),
            &[],
            "f",
            0,
            Level::O0,
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "TV0005");
    }

    #[test]
    fn short_read_trailing_and_bad_status_are_distinct() {
        let short = script("printf 'abc'");
        let errors = run(&short, &[], "f", 1, Level::O0).unwrap_err();
        assert_eq!(errors[0].code, "TV0010");

        let trailing = script("head -c 21 /dev/zero");
        let errors = run(&trailing, &[], "f", 2, Level::O0).unwrap_err();
        assert_eq!(errors[0].code, "TV0012");

        let bad_status = script("printf '\\002\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000\\000'");
        let errors = run(&bad_status, &[], "f", 3, Level::O0).unwrap_err();
        assert_eq!(errors[0].code, "TV0015");
    }

    #[test]
    fn nonzero_exit_is_typed_with_stderr_tail() {
        let path = script("printf 'triage marker' >&2; exit 7");
        let errors = run(&path, &[], "f", 0, Level::O0).unwrap_err();
        assert_eq!(errors[0].code, "TV0007");
        assert!(errors[0].message.contains("triage marker"));
    }

    #[test]
    fn oversized_payload_is_rejected_before_spawn() {
        let path = script("printf 'spawned' >&2; exit 0");
        let payload = vec![0u8; abi::REQUEST_CAP + 1];
        let errors = run(&path, &payload, "f", 0, Level::O0).unwrap_err();
        assert_eq!(errors[0].code, "TV0004");
        assert!(!errors[0].message.contains("spawned"));
    }

    #[test]
    fn oversized_harness_output_is_flagged() {
        let path = script("head -c 2000000 /dev/zero");
        let errors = run(&path, &[], "f", 0, Level::O0).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "TV0022"));
    }
}
