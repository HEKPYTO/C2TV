use crate::abi;
use crate::compare::Observation;
use crate::diag::{self, Diagnostic, Level};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

struct Raw {
    output: Vec<u8>,
    errors: Vec<u8>,
    success: bool,
}

fn drain(mut reader: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = reader.read_to_end(&mut bytes);
    bytes
}

fn reap(
    child: &mut std::process::Child,
    output_thread: thread::JoinHandle<Vec<u8>>,
    error_thread: thread::JoinHandle<Vec<u8>>,
) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = output_thread.join();
    let _ = error_thread.join();
}

fn execute(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
    level: Level,
) -> Result<Raw, Diagnostic> {
    let request = abi::encode_request(payload).ok_or_else(|| {
        Diagnostic::new(
            diag::CORPUS_HEX,
            fixture,
            Some(case),
            Some(level),
            "payload exceeds u32 length",
        )
    })?;
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            Diagnostic::new(
                diag::SPAWN,
                fixture,
                Some(case),
                Some(level),
                format!("cannot start harness: {error}"),
            )
        })?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let output_thread = thread::spawn(move || drain(stdout));
    let error_thread = thread::spawn(move || drain(stderr));

    let mut stdin = child.stdin.take().unwrap();
    if let Err(error) = stdin.write_all(&request) {
        drop(stdin);
        let exited = matches!(child.try_wait(), Ok(Some(_)));
        reap(&mut child, output_thread, error_thread);
        let code = if exited {
            diag::NONZERO_EXIT
        } else {
            diag::SPAWN
        };
        let message = if exited {
            format!("harness exited before accepting request: {error}")
        } else {
            format!("cannot write request: {error}")
        };
        return Err(Diagnostic::new(code, fixture, Some(case), Some(level), message));
    }
    drop(stdin);

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                reap(&mut child, output_thread, error_thread);
                return Err(Diagnostic::new(
                    diag::SPAWN,
                    fixture,
                    Some(case),
                    Some(level),
                    format!("cannot wait for harness: {error}"),
                ));
            }
        }
        if Instant::now() >= deadline {
            reap(&mut child, output_thread, error_thread);
            return Err(Diagnostic::new(
                diag::TIMEOUT,
                fixture,
                Some(case),
                Some(level),
                "harness exceeded 10000ms timeout",
            ));
        }
        thread::sleep(Duration::from_millis(2));
    };

    let output = match output_thread.join() {
        Ok(output) => output,
        Err(_) => {
            return Err(Diagnostic::new(
                diag::SPAWN,
                fixture,
                Some(case),
                Some(level),
                "stdout drain thread failed",
            ));
        }
    };
    let errors = match error_thread.join() {
        Ok(errors) => errors,
        Err(_) => {
            return Err(Diagnostic::new(
                diag::SPAWN,
                fixture,
                Some(case),
                Some(level),
                "stderr drain thread failed",
            ));
        }
    };
    Ok(Raw {
        output,
        errors,
        success: status.success(),
    })
}

pub fn run(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
    level: Level,
) -> Result<(Observation, String), Diagnostic> {
    let raw = execute(binary, payload, fixture, case, level)?;
    if !raw.success {
        return Err(Diagnostic::new(
            diag::NONZERO_EXIT,
            fixture,
            Some(case),
            Some(level),
            "harness exited non-zero",
        ));
    }
    let response =
        abi::parse_response(&raw.output).map_err(|error| error.diagnostic(fixture, case, level))?;
    Ok((response, String::from_utf8_lossy(&raw.errors).into_owned()))
}

pub fn run_ubsan(
    binary: &Path,
    payload: &[u8],
    fixture: &str,
    case: usize,
) -> (Option<String>, Vec<Diagnostic>) {
    let raw = match execute(binary, payload, fixture, case, Level::O0) {
        Ok(raw) => raw,
        Err(diagnostic) => return (None, vec![diagnostic]),
    };
    let stderr = String::from_utf8_lossy(&raw.errors).into_owned();
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
                "ubsan harness exited non-zero",
            )],
        );
    }
    match abi::parse_response(&raw.output) {
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

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn script(text: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("c2tv-script-{}-{id}.sh", std::process::id()));
        let _ = fs::write(&path, format!("#!/bin/sh\n{text}\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o700));
        }
        path
    }

    #[test]
    fn spawn_failure_is_typed() {
        let error = run(
            Path::new("/nonexistent/c2tv-harness"),
            &[],
            "f",
            0,
            Level::O0,
        )
        .unwrap_err();
        assert_eq!(error.code, "TV0005");
    }

    #[test]
    fn short_read_trailing_and_bad_status_are_distinct() {
        let short = script("printf 'abc'");
        let error = run(&short, &[], "f", 1, Level::O0).unwrap_err();
        assert_eq!(error.code, "TV0010");

        let trailing = script("dd if=/dev/zero bs=1 count=21 2>/dev/null");
        let error = run(&trailing, &[], "f", 2, Level::O0).unwrap_err();
        assert_eq!(error.code, "TV0012");

        let bad_status = script("perl -e 'print pack(\"Q<\",2),pack(\"Q<\",0),pack(\"V\",0)'");
        let error = run(&bad_status, &[], "f", 3, Level::O0).unwrap_err();
        assert_eq!(error.code, "TV0015");
    }

    #[test]
    fn nonzero_exit_is_typed() {
        let path = script("exit 7");
        let error = run(&path, &[], "f", 0, Level::O0).unwrap_err();
        assert_eq!(error.code, "TV0007");
    }

    #[test]
    fn write_failure_to_exited_child_is_typed() {
        let path = script("exit 0");
        let payload = vec![0u8; 2_000_000];
        let error = run(&path, &payload, "f", 0, Level::O0).unwrap_err();
        assert!(error.code == "TV0005" || error.code == "TV0007");
    }
}
