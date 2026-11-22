mod abi;
mod build;
mod compare;
mod diag;
mod digest;
mod manifest;
mod report;
mod run;

use clap::{Parser, Subcommand};
use diag::{Diagnostic, Level};
use report::{CaseReport, FixtureReport, LevelReport, Pipeline, ResponseReport};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "c2tv", version, about = "Deterministic C to Rust translation-validation gate")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        json: bool,
    },
    SelfTest {
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Lock {
        #[arg(long)]
        fixtures: PathBuf,
    },
}

fn fixture_dirs(path: &Path) -> Vec<PathBuf> {
    if path.join("port.toml").is_file() {
        return vec![path.to_path_buf()];
    }
    let mut dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate.is_dir() && candidate.join("port.toml").is_file() {
                dirs.push(candidate);
            }
        }
    }
    dirs.sort();
    dirs
}

fn read_fixture_input(
    path: &Path,
    fixture: &str,
    code: diag::CodeInfo,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                code,
                fixture,
                None,
                None,
                format!("cannot read fixture input: {error}"),
            ));
            None
        }
    }
}

fn prepare_work_root(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> bool {
    match fs::remove_dir_all(root) {
        Ok(()) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => true,
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                "c2tv",
                None,
                None,
                format!("cannot clean build directory: {error}"),
            ));
            false
        }
    }
}

fn emit(pipeline: &Pipeline, json: bool) {
    let report = pipeline.report();
    if json {
        print!("{}", report::canonical(&report));
    } else {
        print!("{}", report::human(&report));
    }
}

fn fail_with(diagnostics: Vec<Diagnostic>, json: bool) -> i32 {
    let pipeline = Pipeline {
        clang_version: build::version("clang"),
        rustc_version: build::version("rustc"),
        c2rust_version: build::probe_c2rust(),
        fixtures: Vec::new(),
        diagnostics: diagnostics.clone(),
    };
    emit(&pipeline, json);
    diag::exit_code(&diagnostics)
}


fn capability_diagnostics(version: Option<String>) -> Vec<Diagnostic> {
    match version {
        Some(_) => Vec::new(),
        None => vec![Diagnostic::new(
            diag::CAPABILITY,
            "c2tv",
            None,
            None,
            "c2rust not available on PATH",
        )],
    }
}
fn run_pipeline(directories: &[PathBuf]) -> Pipeline {
    let mut clang_version = build::version("clang");
    let mut rustc_version = build::version("rustc");
    let c2rust_version = build::probe_c2rust();
    let mut diagnostics = capability_diagnostics(c2rust_version.clone());
    let work_root = std::env::temp_dir().join(format!("c2tv-work-{}", std::process::id()));
    if !prepare_work_root(&work_root, &mut diagnostics) {
        return Pipeline {
            clang_version,
            rustc_version,
            c2rust_version,
            fixtures: Vec::new(),
            diagnostics,
        };
    }
    let mut fixtures = Vec::new();

    for directory in directories {
        let id = directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let manifest_text = match read_fixture_input(
            &directory.join("port.toml"),
            &id,
            diag::MANIFEST_PARSE,
            &mut diagnostics,
        ) {
            Some(text) => text,
            None => continue,
        };
        let manifest = match manifest::parse_manifest(&manifest_text, &id, &id) {
            Ok(manifest) => manifest,
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                continue;
            }
        };
        let corpus_text = match read_fixture_input(
            &directory.join("corpus.hex"),
            &id,
            diag::CORPUS_HEX,
            &mut diagnostics,
        ) {
            Some(text) => text,
            None => continue,
        };
        let cases = match manifest::parse_corpus(&corpus_text, &id) {
            Ok(cases) => cases,
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                continue;
            }
        };
        let digests = digest::fixture_digests(directory, &id, &mut diagnostics);
        let built = match build::build(directory, &work_root.join(&id), &id) {
            Ok(built) => built,
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                continue;
            }
        };
        clang_version = built.clang_version;
        rustc_version = built.rustc_version;

        let mut case_reports = Vec::new();
        let mut errored = false;
        let mut diverged = false;
        let mut first_diff = None;
        for (index, payload) in cases.iter().enumerate() {
            let mut levels = Vec::new();
            for level in Level::all() {
                let (c_binary, rust_binary) = match level {
                    Level::O0 => (&built.c_o0, &built.rust_o0),
                    Level::O2 => (&built.c_o2, &built.rust_o2),
                };
                let c_result = run::run(c_binary, payload, &id, index, level);
                let rust_result = run::run(rust_binary, payload, &id, index, level);
                let mut verdict = "ok";
                if let Err(error) = &c_result {
                    diagnostics.push(error.clone());
                }
                if let Err(error) = &rust_result {
                    diagnostics.push(error.clone());
                }
                let (c_response, rust_response) = match (c_result, rust_result) {
                    (Ok((c_response, _)), Ok((rust_response, _))) => {
                        let (difference, mut errors) = compare::compare(
                            &id,
                            index,
                            level,
                            &c_response,
                            &rust_response,
                            &manifest.classes,
                        );
                        diagnostics.append(&mut errors);
                        if difference.is_some() {
                            verdict = "diverge";
                            diverged = true;
                            if first_diff.is_none() {
                                first_diff = difference;
                            }
                        }
                        (
                            Some(ResponseReport::from(&c_response)),
                            Some(ResponseReport::from(&rust_response)),
                        )
                    }
                    _ => {
                        verdict = "error";
                        errored = true;
                        (None, None)
                    }
                };
                levels.push(LevelReport {
                    level: level.label(),
                    c: c_response,
                    rust: rust_response,
                    verdict,
                });

                if level == Level::O0 {
                    let (event, mut errors) = run::run_ubsan(&built.c_ubsan, payload, &id, index);
                    diagnostics.append(&mut errors);
                    if let Some(event) = event {
                        diagnostics.push(Diagnostic::new(
                            diag::UB,
                            &id,
                            Some(index),
                            Some(Level::O0),
                            event,
                        ));
                    }
                }
            }
            case_reports.push(CaseReport { index, levels });
        }

        let observed = if errored {
            "error"
        } else if diverged {
            "diverge"
        } else {
            "pass"
        };
        fixtures.push(FixtureReport {
            id,
            kind: manifest.kind.label(),
            expected: manifest.expected.label(),
            observed,
            classes: manifest.classes.iter().map(|class| class.label()).collect(),
            not_compared: compare::not_compared(&manifest.classes),
            digests,
            cases: case_reports,
            first_diff,
        });
    }

    let _ = fs::remove_dir_all(&work_root);
    Pipeline {
        clang_version,
        rustc_version,
        c2rust_version,
        fixtures,
        diagnostics,
    }
}

fn check(fixture: PathBuf, json: bool) -> i32 {
    let directories = fixture_dirs(&fixture);
    if directories.is_empty() {
        return fail_with(
            vec![Diagnostic::new(
                diag::MANIFEST_VALUE,
                "c2tv",
                None,
                None,
                "no fixtures found",
            )],
            json,
        );
    }
    let pipeline = run_pipeline(&directories);
    let code = diag::exit_code(&pipeline.diagnostics);
    emit(&pipeline, json);
    code
}

fn self_test(fixtures: PathBuf, json: bool) -> i32 {
    let directories = fixture_dirs(&fixtures);
    if directories.is_empty() {
        return fail_with(
            vec![Diagnostic::new(
                diag::MANIFEST_VALUE,
                "c2tv",
                None,
                None,
                "no fixtures found",
            )],
            json,
        );
    }
    let lock = match digest::read_lock(&fixtures) {
        Ok(lock) => lock,
        Err(diagnostic) => return fail_with(vec![diagnostic], json),
    };
    let first = run_pipeline(&directories);
    let first_bytes = report::canonical(&first.report());
    let second = run_pipeline(&directories);
    let second_bytes = report::canonical(&second.report());
    let mut diagnostics = first.diagnostics;
    let expected: BTreeMap<&str, &str> = first
        .fixtures
        .iter()
        .map(|fixture| (fixture.id.as_str(), fixture.expected))
        .collect();
    for diagnostic in &mut diagnostics {
        if diagnostic.code == "TV0008"
            && expected.get(diagnostic.fixture.as_str()) == Some(&"diverge")
        {
            diagnostic.exit_class = diag::ExitClass::Ok;
        }
    }
    if first_bytes != second_bytes {
        diagnostics.push(Diagnostic::new(
            diag::DETERMINISM,
            "c2tv",
            None,
            None,
            "canonical report bytes differ between consecutive runs",
        ));
    }
    if let Some(lock) = lock {
        let all: digest::AllDigests = first
            .fixtures
            .iter()
            .map(|fixture| (fixture.id.clone(), fixture.digests.clone()))
            .collect::<BTreeMap<_, _>>();
        diagnostics.extend(digest::verify(&all, &lock));
    }
    for fixture in &first.fixtures {
        if fixture.expected != fixture.observed {
            diagnostics.push(Diagnostic::new(
                diag::DIVERGENCE,
                &fixture.id,
                None,
                None,
                format!(
                    "expected {}, observed {}",
                    fixture.expected, fixture.observed
                ),
            ));
        }
    }
    let pipeline = Pipeline {
        clang_version: first.clang_version,
        rustc_version: first.rustc_version,
        c2rust_version: first.c2rust_version,
        fixtures: first.fixtures,
        diagnostics: diagnostics.clone(),
    };
    let code = diag::exit_code(&diagnostics);
    emit(&pipeline, json);
    code
}

fn lock(fixtures: PathBuf) -> i32 {
    let directories = fixture_dirs(&fixtures);
    let mut all = digest::AllDigests::new();
    let mut diagnostics = Vec::new();
    for directory in &directories {
        let id = directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let digests = digest::fixture_digests(directory, &id, &mut diagnostics);
        all.insert(id, digests);
    }
    if !diagnostics.is_empty() {
        let code = diag::exit_code(&diagnostics);
        eprintln!("{}", report::human(&Pipeline {
            clang_version: build::version("clang"),
            rustc_version: build::version("rustc"),
            c2rust_version: build::probe_c2rust(),
            fixtures: Vec::new(),
            diagnostics,
        }
        .report()));
        return code;
    }
    match digest::write_lock(&fixtures, &all) {
        Ok(()) => {
            println!("wrote digests for {} fixtures", all.len());
            0
        }
        Err(diagnostic) => {
            eprintln!("{} {}", diagnostic.code, diagnostic.message);
            2
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check { fixture, json } => check(fixture, json),
        Command::SelfTest { fixtures, json } => self_test(fixtures, json),
        Command::Lock { fixtures } => lock(fixtures),
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_root_cleanup_fails_closed() {
        let path = std::env::temp_dir().join(format!("c2tv-not-a-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::write(&path, "x").unwrap();
        let mut diagnostics = Vec::new();
        assert!(!prepare_work_root(&path, &mut diagnostics));
        assert_eq!(diagnostics[0].code, "TV0005");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_c2rust_is_info_ok_and_exit_zero() {
        let version = build::probe("c2tv-missing-c2rust-binary");
        assert!(version.is_none());
        let diagnostics = capability_diagnostics(version);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TV0014");
        assert_eq!(diagnostics[0].severity, diag::Severity::Info);
        assert_eq!(diagnostics[0].exit_class, diag::ExitClass::Ok);
        assert_eq!(diag::exit_code(&diagnostics), 0);
    }
}
