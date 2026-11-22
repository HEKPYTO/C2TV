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
#[command(
    name = "c2tv",
    version,
    about = "Deterministic C to Rust translation-validation gate"
)]
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

fn fixture_dirs(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> Vec<PathBuf> {
    let fixture = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "c2tv".to_string());
    if matches!(fs::symlink_metadata(path), Ok(metadata) if metadata.file_type().is_symlink()) {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            &fixture,
            None,
            None,
            format!("symlinked fixture path not allowed: {}", path.display()),
        ));
        return Vec::new();
    }
    if path.join("port.toml").is_file() {
        return vec![path.to_path_buf()];
    }
    if path.join("corpus.hex").exists() || path.join("c").is_dir() || path.join("rust").is_dir() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            &fixture,
            None,
            None,
            "fixture directory missing port.toml",
        ));
        return Vec::new();
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::UNKNOWN_KEY,
                &fixture,
                None,
                None,
                format!("cannot list fixture directory: {error}"),
            ));
            return Vec::new();
        }
    };
    let mut dirs = Vec::new();
    let mut invalid = false;
    for entry in entries {
        match entry {
            Ok(entry) => {
                let candidate = entry.path();
                match entry.file_type() {
                    Ok(kind) if kind.is_symlink() => {
                        let name = candidate
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        diagnostics.push(Diagnostic::new(
                            diag::MANIFEST_VALUE,
                            &name,
                            None,
                            None,
                            format!(
                                "symlinked fixture entry not allowed: {}",
                                candidate.display()
                            ),
                        ));
                        invalid = true;
                    }
                    Ok(kind) if kind.is_dir() => {
                        dirs.extend(fixture_dirs(&candidate, diagnostics));
                    }
                    Ok(_) => {}
                    Err(error) => diagnostics.push(Diagnostic::new(
                        diag::UNKNOWN_KEY,
                        &fixture,
                        None,
                        None,
                        format!("cannot inspect fixture directory entry: {error}"),
                    )),
                }
            }
            Err(error) => diagnostics.push(Diagnostic::new(
                diag::UNKNOWN_KEY,
                &fixture,
                None,
                None,
                format!("cannot read fixture directory entry: {error}"),
            )),
        }
    }
    if invalid {
        return Vec::new();
    }
    dirs.sort_by(|left, right| {
        left.file_name()
            .cmp(&right.file_name())
            .then_with(|| left.cmp(right))
    });
    let mut duplicate = false;
    for pair in dirs.windows(2) {
        if pair[0].file_name() == pair[1].file_name() {
            let id = pair[0]
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            diagnostics.push(Diagnostic::new(
                diag::MANIFEST_VALUE,
                "c2tv",
                None,
                None,
                format!(
                    "duplicate fixture id {id}: {} and {}",
                    pair[0].display(),
                    pair[1].display()
                ),
            ));
            duplicate = true;
        }
    }
    if duplicate {
        return Vec::new();
    }
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
struct Prepared {
    id: String,
    manifest: manifest::Manifest,
    cases: Vec<Vec<u8>>,
    digests: digest::Digests,
    built: build::Built,
}

fn prepare_fixture(
    directory: &Path,
    work_root: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Prepared> {
    let id = directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let manifest_text = read_fixture_input(
        &directory.join("port.toml"),
        &id,
        diag::MANIFEST_PARSE,
        diagnostics,
    )?;
    let manifest = match manifest::parse_manifest(&manifest_text, &id, &id) {
        Ok(manifest) => manifest,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            return None;
        }
    };
    let corpus_text = read_fixture_input(
        &directory.join("corpus.hex"),
        &id,
        diag::CORPUS_HEX,
        diagnostics,
    )?;
    let cases = match manifest::parse_corpus(&corpus_text, &id) {
        Ok(cases) => cases,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            return None;
        }
    };
    let digests = digest::fixture_digests(directory, &id, diagnostics);
    let built = match build::build(directory, &work_root.join(&id), &id) {
        Ok(built) => built,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            return None;
        }
    };
    Some(Prepared {
        id,
        manifest,
        cases,
        digests,
        built,
    })
}

fn normalize_work_root(diagnostics: &mut [Diagnostic], work_root: &Path) {
    let needle = work_root.to_string_lossy();
    for diagnostic in diagnostics {
        if diagnostic.message.contains(needle.as_ref()) {
            diagnostic.message = diagnostic.message.replace(needle.as_ref(), "<work_root>");
        }
    }
}

fn run_pipeline(directories: &[PathBuf], slot: &str) -> Pipeline {
    let mut clang_version = build::version("clang");
    let mut rustc_version = build::version("rustc");
    let c2rust_version = build::probe_c2rust();
    let mut diagnostics = capability_diagnostics(c2rust_version.clone());
    let work_root = std::env::temp_dir().join(format!("c2tv-work-{}-{slot}", std::process::id()));
    if !prepare_work_root(&work_root, &mut diagnostics) {
        normalize_work_root(&mut diagnostics, &work_root);
        return Pipeline {
            clang_version,
            rustc_version,
            c2rust_version,
            fixtures: Vec::new(),
            diagnostics,
        };
    }
    let mut fixtures = Vec::new();

    let width = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4));
    let mut prepared = Vec::with_capacity(directories.len());
    for chunk in directories.chunks(width) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|directory| {
                    scope.spawn(|| {
                        let mut errors = Vec::new();
                        let prepared = prepare_fixture(directory, &work_root, &mut errors);
                        (prepared, errors)
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok((item, errors)) => {
                        diagnostics.extend(errors);
                        if let Some(item) = item {
                            prepared.push(item);
                        }
                    }
                    Err(_) => diagnostics.push(Diagnostic::new(
                        diag::SPAWN,
                        "c2tv",
                        None,
                        None,
                        "fixture preparation thread panicked",
                    )),
                }
            }
        });
    }

    for Prepared {
        id,
        manifest,
        cases,
        digests,
        built,
    } in prepared
    {
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
    normalize_work_root(&mut diagnostics, &work_root);
    Pipeline {
        clang_version,
        rustc_version,
        c2rust_version,
        fixtures,
        diagnostics,
    }
}

fn check(fixture: PathBuf, json: bool) -> i32 {
    let mut diagnostics = Vec::new();
    let directories = fixture_dirs(&fixture, &mut diagnostics);
    if directories.is_empty() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "no fixtures found",
        ));
        return fail_with(diagnostics, json);
    }
    let mut pipeline = run_pipeline(&directories, "check");
    diagnostics.append(&mut pipeline.diagnostics);
    pipeline.diagnostics = diagnostics;
    let code = diag::exit_code(&pipeline.diagnostics);
    emit(&pipeline, json);
    code
}

fn self_test(fixtures: PathBuf, json: bool) -> i32 {
    let mut diagnostics = Vec::new();
    let directories = fixture_dirs(&fixtures, &mut diagnostics);
    if directories.is_empty() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "no fixtures found",
        ));
        return fail_with(diagnostics, json);
    }
    let lock = match digest::read_lock(&fixtures) {
        Ok(lock) => lock,
        Err(diagnostic) => return fail_with(vec![diagnostic], json),
    };
    let (mut first, second) = std::thread::scope(|scope| {
        let first_thread = scope.spawn(|| run_pipeline(&directories, "first"));
        let second_thread = scope.spawn(|| run_pipeline(&directories, "second"));
        (
            first_thread.join().expect("first pipeline panicked"),
            second_thread.join().expect("second pipeline panicked"),
        )
    });
    let first_bytes = report::canonical(&first.report());
    let second_bytes = report::canonical(&second.report());
    diagnostics.append(&mut first.diagnostics);
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
    let mut diagnostics = Vec::new();
    let directories = fixture_dirs(&fixtures, &mut diagnostics);
    if directories.is_empty() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "no fixtures found",
        ));
    }
    let mut all = digest::AllDigests::new();
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
        eprintln!(
            "{}",
            report::human(
                &Pipeline {
                    clang_version: build::version("clang"),
                    rustc_version: build::version("rustc"),
                    c2rust_version: build::probe_c2rust(),
                    fixtures: Vec::new(),
                    diagnostics,
                }
                .report()
            )
        );
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

    fn temp_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("c2tv-fixture-dirs-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn discovers_fixtures_in_nested_directories() {
        let root = temp_root("nested");
        fs::create_dir_all(root.join("good").join("nested")).unwrap();
        fs::create_dir_all(root.join("bad")).unwrap();
        fs::write(root.join("good").join("nested").join("port.toml"), "").unwrap();
        fs::write(root.join("bad").join("port.toml"), "").unwrap();
        let mut diagnostics = Vec::new();
        let dirs = fixture_dirs(&root, &mut diagnostics);
        assert!(diagnostics.is_empty());
        assert_eq!(
            dirs,
            vec![root.join("bad"), root.join("good").join("nested")]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn empty_fixture_root_has_no_diagnostics() {
        let root = temp_root("empty");
        fs::create_dir_all(&root).unwrap();
        let mut diagnostics = Vec::new();
        assert!(fixture_dirs(&root, &mut diagnostics).is_empty());
        assert!(diagnostics.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn malformed_fixture_leaf_is_diagnosed() {
        let root = temp_root("malformed");
        fs::create_dir_all(root.join("good").join("leaf")).unwrap();
        fs::write(root.join("good").join("leaf").join("corpus.hex"), "-\n").unwrap();
        let mut diagnostics = Vec::new();
        assert!(fixture_dirs(&root, &mut diagnostics).is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TV0003");
        assert_eq!(diagnostics[0].fixture, "leaf");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn duplicate_fixture_ids_fail_closed() {
        let root = temp_root("duplicate");
        fs::create_dir_all(root.join("good").join("dup")).unwrap();
        fs::create_dir_all(root.join("bad").join("dup")).unwrap();
        fs::write(root.join("good").join("dup").join("port.toml"), "").unwrap();
        fs::write(root.join("bad").join("dup").join("port.toml"), "").unwrap();
        let mut diagnostics = Vec::new();
        assert!(fixture_dirs(&root, &mut diagnostics).is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TV0003");
        assert!(diagnostics[0]
            .message
            .contains(&root.join("good").join("dup").display().to_string()));
        assert!(diagnostics[0]
            .message
            .contains(&root.join("bad").join("dup").display().to_string()));
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_loop_is_diagnosed_and_fails_closed() {
        use std::os::unix::fs::symlink;
        let root = temp_root("symlink");
        fs::create_dir_all(root.join("good").join("leaf")).unwrap();
        fs::write(root.join("good").join("leaf").join("port.toml"), "").unwrap();
        symlink(&root, root.join("loop")).unwrap();
        let mut diagnostics = Vec::new();
        assert!(fixture_dirs(&root, &mut diagnostics).is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TV0003");
        assert_eq!(diagnostics[0].fixture, "loop");
        assert!(diagnostics[0]
            .message
            .contains("symlinked fixture entry not allowed"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn failing_build_diagnostics_are_path_independent() {
        let root = temp_root("buildfail");
        let fixture = root.join("broken");
        fs::create_dir_all(fixture.join("c")).unwrap();
        fs::create_dir_all(fixture.join("rust").join("src")).unwrap();
        fs::write(fixture.join("c").join("main.c"), "#error broken build\n").unwrap();
        fs::write(
            fixture.join("rust").join("Cargo.toml"),
            "[package]\nname = \"harness\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[[bin]]\nname = \"harness\"\npath = \"src/main.rs\"\n[workspace]\n",
        )
        .unwrap();
        fs::write(
            fixture.join("rust").join("src").join("main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        fs::write(
            fixture.join("port.toml"),
            "schema = 1\nid = \"broken\"\nkind = \"good\"\nclasses = [\"ret\"]\nexpected = \"pass\"\nnote = \"build failure\"\n",
        )
        .unwrap();
        fs::write(fixture.join("corpus.hex"), "-\n").unwrap();
        let dirs = vec![fixture];
        let first = run_pipeline(&dirs, "test-a");
        let second = run_pipeline(&dirs, "test-b");
        assert_eq!(
            report::canonical(&first.report()),
            report::canonical(&second.report())
        );
        assert!(first.diagnostics.iter().any(|d| d.code == "TV0007"));
        assert!(first
            .diagnostics
            .iter()
            .any(|d| d.message.contains("<work_root>")));
        fs::remove_dir_all(&root).unwrap();
    }
}
