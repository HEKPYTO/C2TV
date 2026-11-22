use crate::diag::{self, Diagnostic, Level};
use crate::process::{self, Captured};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub struct Built {
    pub c_o0: PathBuf,
    pub c_o2: PathBuf,
    pub c_ubsan: PathBuf,
    pub rust_o0: PathBuf,
    pub rust_o2: PathBuf,
    pub clang_version: String,
    pub rustc_version: String,
}

fn first_line(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn version(program: &str) -> String {
    let mut command = process::sanitized(program);
    command.arg("--version");
    process::run(&mut command, None, process::PROBE_TIMEOUT)
        .map(|captured| first_line(&captured))
        .unwrap_or_default()
}
pub fn probe_args(program: &str, args: &[&str]) -> Option<String> {
    let mut command = process::sanitized(program);
    command.args(args);
    process::run(&mut command, None, process::PROBE_TIMEOUT)
        .ok()
        .filter(|captured| captured.success)
        .map(|captured| first_line(&captured))
        .filter(|text| !text.is_empty())
}

pub fn probe(program: &str) -> Option<String> {
    probe_args(program, &["--version"])
}

pub fn probe_transpile() -> Option<String> {
    probe_args("c2rust", &["transpile", "--version"])
}

pub fn probe_c2rust() -> Option<String> {
    probe("c2rust")
}

fn step(
    fixture: &str,
    level: Option<Level>,
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    environment: &[(&str, String)],
    errors: &mut Vec<Diagnostic>,
) {
    let mut command = process::sanitized(program);
    command.args(args);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    match process::run(&mut command, None, process::BUILD_TIMEOUT) {
        Ok(captured) => {
            if captured.truncated {
                errors.push(diag::truncation(
                    fixture,
                    None,
                    level,
                    format!(
                        "{program} output exceeded {} byte budget",
                        process::OUTPUT_CAP
                    ),
                ));
            }
            if captured.timed_out {
                errors.push(Diagnostic::new(
                    diag::TIMEOUT,
                    fixture,
                    None,
                    level,
                    format!(
                        "{program} exceeded {}ms timeout",
                        process::BUILD_TIMEOUT.as_millis()
                    ),
                ));
            } else if !captured.success {
                errors.push(Diagnostic::new(
                    diag::NONZERO_EXIT,
                    fixture,
                    None,
                    level,
                    format!(
                        "{program} exited {}: {}",
                        captured.status,
                        String::from_utf8_lossy(&captured.stderr).trim()
                    ),
                ));
            }
        }
        Err(error) => errors.push(Diagnostic::new(
            diag::SPAWN,
            fixture,
            None,
            level,
            format!("cannot run {program}: {error}"),
        )),
    }
}

pub(crate) fn test_step(
    fixture: &str,
    crate_dir: &Path,
    target_dir: &Path,
    errors: &mut Vec<Diagnostic>,
) {
    let mut command = process::sanitized("cargo");
    command
        .args(["test", "--offline"])
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_NET_OFFLINE", "true");
    match process::run(&mut command, None, process::BUILD_TIMEOUT) {
        Ok(captured) => {
            if captured.truncated {
                errors.push(diag::truncation(
                    fixture,
                    None,
                    None,
                    format!(
                        "cargo test output exceeded {} byte budget",
                        process::OUTPUT_CAP
                    ),
                ));
            }
            if captured.timed_out {
                errors.push(Diagnostic::new(
                    diag::TIMEOUT,
                    fixture,
                    None,
                    None,
                    format!(
                        "cargo test exceeded {}ms timeout",
                        process::BUILD_TIMEOUT.as_millis()
                    ),
                ));
            } else if !captured.success {
                errors.push(Diagnostic::new(
                    diag::CRATE_TEST_FAILED,
                    fixture,
                    None,
                    None,
                    format!(
                        "cargo test exited {}: stderr: {} stdout: {}",
                        captured.status,
                        String::from_utf8_lossy(&captured.stderr).trim(),
                        String::from_utf8_lossy(&captured.stdout).trim()
                    ),
                ));
            }
        }
        Err(error) => errors.push(Diagnostic::new(
            diag::SPAWN,
            fixture,
            None,
            None,
            format!("cannot run cargo test: {error}"),
        )),
    }
}

pub(crate) fn c_step(
    fixture: &str,
    level: Level,
    source: &str,
    output: &Path,
    sanitize: bool,
    errors: &mut Vec<Diagnostic>,
) {
    let mut args = vec!["-std=c11".to_string()];
    args.push(match level {
        Level::O0 => "-O0".to_string(),
        Level::O2 => "-O2".to_string(),
    });
    if sanitize {
        args.push("-fsanitize=undefined".to_string());
        args.push("-fno-sanitize-recover=all".to_string());
    }
    args.push(source.to_string());
    args.push("-o".to_string());
    args.push(output.to_string_lossy().into_owned());
    step(fixture, Some(level), "clang", &args, None, &[], errors);
}

fn rust_step(
    fixture: &str,
    level: Level,
    crate_dir: &Path,
    target_dir: &Path,
    errors: &mut Vec<Diagnostic>,
) {
    let args = vec![
        "build".to_string(),
        "--offline".to_string(),
        "--release".to_string(),
        "--bin".to_string(),
        "harness".to_string(),
    ];
    let environment = [
        (
            "CARGO_TARGET_DIR",
            target_dir.to_string_lossy().into_owned(),
        ),
        (
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
            match level {
                Level::O0 => "0".to_string(),
                Level::O2 => "2".to_string(),
            },
        ),
        ("CARGO_NET_OFFLINE", "true".to_string()),
    ];
    step(
        fixture,
        Some(level),
        "cargo",
        &args,
        Some(crate_dir),
        &environment,
        errors,
    );
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if kind.is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("symlink not allowed: {}", source.display()),
            ));
        } else if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_file() {
            fs::copy(&source, target)?;
        } else {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("unsupported fixture entry: {}", source.display()),
            ));
        }
    }
    Ok(())
}
pub fn build(dir: &Path, work: &Path, fixture: &str) -> Result<Built, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let subdirectories = ["c_o0", "c_o2", "c_ubsan", "rust_o0", "rust_o2"];
    for name in subdirectories {
        if let Err(error) = fs::create_dir_all(work.join(name)) {
            errors.push(Diagnostic::new(
                diag::SPAWN,
                fixture,
                None,
                None,
                format!("cannot create build directory: {error}"),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let source_root = work.join("source");
    for name in ["c", "rust"] {
        if let Err(error) = copy_tree(&dir.join(name), &source_root.join(name)) {
            errors.push(Diagnostic::new(
                diag::SPAWN,
                fixture,
                None,
                None,
                format!("cannot copy fixture sources: {error}"),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let source = source_root
        .join("c")
        .join("main.c")
        .to_string_lossy()
        .into_owned();
    let c_o0 = work.join("c_o0").join("harness");
    let c_o2 = work.join("c_o2").join("harness");
    let c_ubsan = work.join("c_ubsan").join("harness");
    let rust_o0 = work.join("rust_o0").join("release").join("harness");
    let rust_o2 = work.join("rust_o2").join("release").join("harness");

    c_step(fixture, Level::O0, &source, &c_o0, false, &mut errors);
    c_step(fixture, Level::O2, &source, &c_o2, false, &mut errors);
    c_step(fixture, Level::O0, &source, &c_ubsan, true, &mut errors);

    let crate_dir = source_root.join("rust");
    rust_step(
        fixture,
        Level::O0,
        &crate_dir,
        &work.join("rust_o0"),
        &mut errors,
    );
    rust_step(
        fixture,
        Level::O2,
        &crate_dir,
        &work.join("rust_o2"),
        &mut errors,
    );

    if errors.is_empty() {
        Ok(Built {
            c_o0,
            c_o2,
            c_ubsan,
            rust_o0,
            rust_o2,
            clang_version: version("clang"),
            rustc_version: version("rustc"),
        })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_full_stderr_without_truncation() {
        let long = "x".repeat(300);
        let args = vec![
            "-c".to_string(),
            format!("printf '%s' '{long}' >&2; exit 1"),
        ];
        let mut errors = Vec::new();
        step("f", Some(Level::O0), "sh", &args, None, &[], &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "TV0007");
        assert!(errors[0].message.contains(&long));
    }
    #[cfg(unix)]
    #[test]
    fn copy_tree_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("c2tv-copy-{}-{}", std::process::id(), 1));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("from")).unwrap();
        fs::write(root.join("from").join("file.txt"), "x").unwrap();
        symlink(
            root.join("from").join("file.txt"),
            root.join("from").join("link"),
        )
        .unwrap();
        let error = copy_tree(&root.join("from"), &root.join("to")).unwrap_err();
        assert!(error.to_string().contains("symlink not allowed"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn crate_test_failure_is_typed_and_keeps_test_output() {
        let root = std::env::temp_dir().join(format!("c2tv-test-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("crate").join("src")).unwrap();
        fs::write(
            root.join("crate").join("Cargo.toml"),
            "[package]\nname = \"failing\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
        )
        .unwrap();
        fs::write(
            root.join("crate").join("src").join("lib.rs"),
            "#[test]\nfn deliberate_failure() { panic!(\"TV0021 proof\"); }\n",
        )
        .unwrap();
        let mut errors = Vec::new();
        test_step("f", &root.join("crate"), &root.join("target"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "TV0021");
        assert_eq!(errors[0].exit_class, diag::ExitClass::ToolError);
        assert!(errors[0].message.contains("TV0021 proof"));
        fs::remove_dir_all(&root).unwrap();
    }
}
