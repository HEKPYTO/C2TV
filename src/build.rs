use crate::diag::{self, Diagnostic, Level};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct Built {
    pub c_o0: PathBuf,
    pub c_o2: PathBuf,
    pub c_ubsan: PathBuf,
    pub rust_o0: PathBuf,
    pub rust_o2: PathBuf,
    pub clang_version: String,
    pub rustc_version: String,
}

pub fn version(program: &str) -> String {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

pub fn probe(program: &str) -> Option<String> {
    let program = program.to_string();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let version = Command::new(program)
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .map(|line| line.trim().to_string())
            });
        let _ = sender.send(version);
    });
    receiver.recv_timeout(Duration::from_secs(2)).ok().flatten()
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
    let mut command = Command::new(program);
    command.args(args);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    match command.output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => errors.push(Diagnostic::new(
            diag::NONZERO_EXIT,
            fixture,
            None,
            level,
            format!("{program} exited {}: {}", output.status, String::from_utf8_lossy(&output.stderr).trim()),
        )),
        Err(error) => errors.push(Diagnostic::new(
            diag::SPAWN,
            fixture,
            None,
            level,
            format!("cannot run {program}: {error}"),
        )),
    }
}

fn c_step(
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
        ("CARGO_TARGET_DIR", target_dir.to_string_lossy().into_owned()),
        (
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
            match level {
                Level::O0 => "0".to_string(),
                Level::O2 => "2".to_string(),
            },
        ),
    ];
    step(fixture, Some(level), "cargo", &args, Some(crate_dir), &environment, errors);
}


fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
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
    let source = source_root.join("c").join("main.c").to_string_lossy().into_owned();
    let c_o0 = work.join("c_o0").join("harness");
    let c_o2 = work.join("c_o2").join("harness");
    let c_ubsan = work.join("c_ubsan").join("harness");
    let rust_o0 = work.join("rust_o0").join("release").join("harness");
    let rust_o2 = work.join("rust_o2").join("release").join("harness");

    c_step(fixture, Level::O0, &source, &c_o0, false, &mut errors);
    c_step(fixture, Level::O2, &source, &c_o2, false, &mut errors);
    c_step(fixture, Level::O0, &source, &c_ubsan, true, &mut errors);

    let crate_dir = source_root.join("rust");
    rust_step(fixture, Level::O0, &crate_dir, &work.join("rust_o0"), &mut errors);
    rust_step(fixture, Level::O2, &crate_dir, &work.join("rust_o2"), &mut errors);

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
        let args = vec!["-c".to_string(), format!("printf '%s' '{long}' >&2; exit 1")];
        let mut errors = Vec::new();
        step("f", Some(Level::O0), "sh", &args, None, &[], &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "TV0007");
        assert!(errors[0].message.contains(&long));
    }
}
