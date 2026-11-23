mod abi;
mod bench;
mod build;
mod compare;
mod diag;
mod digest;
mod manifest;
mod process;
mod report;
mod run;
mod transpile;

use clap::{Parser, Subcommand};
use diag::{Diagnostic, Level};
use report::{CaseReport, FixtureReport, LevelReport, Pipeline, ResponseReport, TranspileReport};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    Bench {
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long)]
        json: bool,
    },
    SelfTest {
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        transpile: bool,
    },
    Transpile {
        #[arg(long)]
        fixture: PathBuf,
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

fn work_root(slot: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("c2tv-work-{}-{slot}-{nonce}", std::process::id()))
}

fn prepare_work_root(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> bool {
    if fs::symlink_metadata(root).is_ok() {
        diagnostics.push(Diagnostic::new(
            diag::SPAWN,
            "c2tv",
            None,
            None,
            format!("work root already exists: {}", root.display()),
        ));
        return false;
    }
    if let Err(error) = fs::create_dir(root) {
        diagnostics.push(Diagnostic::new(
            diag::SPAWN,
            "c2tv",
            None,
            None,
            format!("cannot create build directory: {error}"),
        ));
        return false;
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) if !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                "c2tv",
                None,
                None,
                "work root became a symlink",
            ));
            false
        }
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                "c2tv",
                None,
                None,
                format!("cannot verify build directory: {error}"),
            ));
            false
        }
    }
}

fn normalize_paths(diagnostics: &mut [Diagnostic], root: &Path) {
    let needle = root.to_string_lossy();
    for diagnostic in diagnostics {
        if diagnostic.message.contains(needle.as_ref()) {
            diagnostic.message = diagnostic.message.replace(needle.as_ref(), "<fixtures>");
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

fn failed_pipeline(mut diagnostics: Vec<Diagnostic>, root: Option<&Path>) -> Pipeline {
    if let Some(root) = root {
        normalize_paths(&mut diagnostics, root);
    }
    Pipeline {
        clang_version: build::version("clang"),
        rustc_version: build::version("rustc"),
        c2rust_version: build::probe_c2rust(),
        c2rust_sha256: None,
        generated_digest: None,
        transpile: None,
        fixtures: Vec::new(),
        diagnostics,
    }
}

fn fail_with(diagnostics: Vec<Diagnostic>, json: bool, root: Option<&Path>) -> i32 {
    let pipeline = failed_pipeline(diagnostics, root);
    let code = diag::exit_code(&pipeline.diagnostics);
    emit(&pipeline, json);
    code
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
    crate_dir: PathBuf,
    test_target_dir: PathBuf,
}

fn timing(label: &str, start: Instant) {
    if std::env::var_os("C2TV_TIMINGS").is_some() {
        eprintln!("{label}: {}ms", start.elapsed().as_millis());
    }
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
    let work = work_root.join(&id);
    let built = match build::build(directory, &work, &id) {
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
        crate_dir: work.join("source").join("rust"),
        test_target_dir: work.join("test_target"),
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
    let work_root = work_root(slot);
    if !prepare_work_root(&work_root, &mut diagnostics) {
        normalize_work_root(&mut diagnostics, &work_root);
        return Pipeline {
            clang_version,
            rustc_version,
            c2rust_version,
            c2rust_sha256: None,
            generated_digest: None,
            transpile: None,
            fixtures: Vec::new(),
            diagnostics,
        };
    }
    let mut fixtures = Vec::new();

    let width = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4));
    let mut prepared = Vec::with_capacity(directories.len());
    let prep_start = Instant::now();
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
    timing("prep", prep_start);
    let tests_start = Instant::now();
    for chunk in prepared.chunks(width) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|item| {
                    scope.spawn(|| {
                        let mut errors = Vec::new();
                        build::test_step(
                            &item.id,
                            &item.crate_dir,
                            &item.test_target_dir,
                            &mut errors,
                        );
                        errors
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok(errors) => diagnostics.extend(errors),
                    Err(_) => diagnostics.push(Diagnostic::new(
                        diag::SPAWN,
                        "c2tv",
                        None,
                        None,
                        "crate test thread panicked",
                    )),
                }
            }
        });
    }
    timing("crate_tests", tests_start);

    let run_start = Instant::now();
    for Prepared {
        id,
        manifest,
        cases,
        digests,
        built,
        ..
    } in prepared
    {
        clang_version = built.clang_version;
        rustc_version = built.rustc_version;

        let mut case_reports = Vec::new();
        let mut errored = false;
        let mut diverged = false;
        let mut first_diff = None;
        let mut first_diff_case = None;
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
                if let Err(errors) = &c_result {
                    diagnostics.extend(errors.clone());
                }
                if let Err(errors) = &rust_result {
                    diagnostics.extend(errors.clone());
                }
                let (c_response, rust_response) = match (c_result, rust_result) {
                    (Ok((c_response, _)), Ok((rust_response, _))) => {
                        for observation in [&c_response, &rust_response] {
                            if observation.out.len() > report::OUT_HEX_CAP {
                                diagnostics.push(diag::truncation(
                                    &id,
                                    Some(index),
                                    Some(level),
                                    format!(
                                        "response out_hex truncated to {} bytes from {}",
                                        report::OUT_HEX_CAP,
                                        observation.out.len()
                                    ),
                                ));
                            }
                        }
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
                                first_diff_case = Some(index);
                            }
                        }
                        (
                            Some(ResponseReport::from(&c_response)),
                            Some(ResponseReport::from(&rust_response)),
                        )
                    }
                    (c_result, rust_result) => {
                        verdict = "error";
                        errored = true;
                        (
                            c_result
                                .ok()
                                .map(|(response, _)| ResponseReport::from(&response)),
                            rust_result
                                .ok()
                                .map(|(response, _)| ResponseReport::from(&response)),
                        )
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
                    if !errors.is_empty() {
                        errored = true;
                    }
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
            first_diff_case,
            expect_case: manifest.expect_case,
            expect_class: manifest.expect_class.map(|class| class.label()),
        });
    }

    timing("run_compare", run_start);
    let _ = fs::remove_dir_all(&work_root);
    normalize_work_root(&mut diagnostics, &work_root);
    Pipeline {
        clang_version,
        rustc_version,
        c2rust_version,
        c2rust_sha256: None,
        generated_digest: None,
        transpile: None,
        fixtures,
        diagnostics,
    }
}

fn require_lock(root: &Path) -> Result<digest::DigestFile, Diagnostic> {
    let mut candidate = Some(root);
    while let Some(directory) = candidate {
        if let Some(lock) = digest::read_lock(directory)? {
            return Ok(lock);
        }
        candidate = directory.parent();
    }
    Err(Diagnostic::new(
        diag::DIGEST,
        "digests.toml",
        None,
        None,
        "missing digests.toml; run c2tv lock --fixtures <dir>",
    ))
}

pub(crate) fn expectation_diagnostics(fixtures: &[report::FixtureReport]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for fixture in fixtures {
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
            continue;
        }
        let (Some(expected_case), Some(expected_class)) =
            (fixture.expect_case, fixture.expect_class)
        else {
            continue;
        };
        if fixture.observed != "diverge" {
            continue;
        }
        let expected_field = if expected_class == "err" {
            "status"
        } else {
            expected_class
        };
        let observed_field = fixture.first_diff.map(|difference| difference.field);
        if fixture.first_diff_case != Some(expected_case) || observed_field != Some(expected_field)
        {
            diagnostics.push(Diagnostic::new(
                diag::DIVERGENCE,
                &fixture.id,
                Some(expected_case),
                None,
                format!(
                    "expected divergence case {expected_case} class {expected_class}, observed case {} class {}",
                    fixture
                        .first_diff_case
                        .map(|case| case.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    observed_field.unwrap_or("none")
                ),
            ));
        }
    }
    diagnostics
}

fn downgrade_expected(diagnostics: &mut [Diagnostic], fixtures: &[report::FixtureReport]) {
    let expected: BTreeMap<&str, &str> = fixtures
        .iter()
        .map(|fixture| (fixture.id.as_str(), fixture.expected))
        .collect();
    for diagnostic in diagnostics {
        if diagnostic.code == "TV0008"
            && expected.get(diagnostic.fixture.as_str()) == Some(&"diverge")
        {
            diagnostic.exit_class = diag::ExitClass::Ok;
        }
    }
}

fn lock_digests(pipeline: &Pipeline) -> digest::AllDigests {
    pipeline
        .fixtures
        .iter()
        .map(|fixture| (fixture.id.clone(), fixture.digests.clone()))
        .collect()
}

fn fixture_roots(directories: &[PathBuf]) -> BTreeMap<String, PathBuf> {
    directories
        .iter()
        .map(|directory| {
            (
                directory
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                directory.clone(),
            )
        })
        .collect()
}

fn subset_lock(lock: &digest::DigestFile, all: &digest::AllDigests) -> digest::DigestFile {
    digest::DigestFile {
        fixtures: lock
            .fixtures
            .iter()
            .filter(|(id, _)| all.contains_key(*id))
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect(),
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
        return fail_with(diagnostics, json, Some(&fixture));
    }
    let lock = match require_lock(&fixture) {
        Ok(lock) => lock,
        Err(error) => return fail_with(vec![error], json, Some(&fixture)),
    };
    let mut pipeline = run_pipeline(&directories, "check");
    diagnostics.append(&mut pipeline.diagnostics);
    let all = lock_digests(&pipeline);
    diagnostics.extend(digest::verify(
        &fixture_roots(&directories),
        &all,
        &subset_lock(&lock, &all),
    ));
    diagnostics.extend(expectation_diagnostics(&pipeline.fixtures));
    pipeline.diagnostics = diagnostics;
    let code = diag::exit_code(&pipeline.diagnostics);
    emit(&pipeline, json);
    code
}

fn transpile_cli(fixture: PathBuf, json: bool) -> i32 {
    let mut diagnostics = Vec::new();
    let directories = fixture_dirs(&fixture, &mut diagnostics);
    if directories.len() != 1 {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "expected exactly one fixture directory",
        ));
        return fail_with(diagnostics, json, Some(&fixture));
    }
    let directory = &directories[0];
    let id = directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(text) = read_fixture_input(
        &directory.join("port.toml"),
        &id,
        diag::MANIFEST_PARSE,
        &mut diagnostics,
    ) else {
        return fail_with(diagnostics, json, Some(&fixture));
    };
    match manifest::parse_manifest(&text, &id, &id) {
        Ok(manifest) if manifest.transpile => {}
        Ok(_) => {
            diagnostics.push(Diagnostic::new(
                diag::MANIFEST_VALUE,
                &id,
                None,
                None,
                "transpile = true is required for c2tv transpile",
            ));
            return fail_with(diagnostics, json, Some(&fixture));
        }
        Err(errors) => {
            diagnostics.extend(errors);
            return fail_with(diagnostics, json, Some(&fixture));
        }
    }
    let lane = transpile::run_lane(directory, "cli");
    diagnostics.extend(lane.diagnostics);
    let pipeline = Pipeline {
        clang_version: build::version("clang"),
        rustc_version: build::version("rustc"),
        c2rust_version: lane.version,
        c2rust_sha256: lane.sha256,
        generated_digest: lane.digest,
        transpile: Some(vec![lane.report]),
        fixtures: Vec::new(),
        diagnostics: diagnostics.clone(),
    };
    let code = diag::exit_code(&diagnostics);
    emit(&pipeline, json);
    code
}
fn transpile_self_test(
    directories: &[PathBuf],
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    Vec<TranspileReport>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    if let Err(error) = transpile::check_version(build::probe_transpile()) {
        diagnostics.push(error);
        return (Vec::new(), None, None, None);
    }
    let sha256 = match transpile::binary_sha256("c2rust") {
        Ok(sha256) => Some(sha256),
        Err(error) => {
            diagnostics.push(error);
            return (Vec::new(), None, None, None);
        }
    };
    let mut selected = Vec::new();
    for directory in directories {
        let id = directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(text) = read_fixture_input(
            &directory.join("port.toml"),
            &id,
            diag::MANIFEST_PARSE,
            diagnostics,
        ) else {
            continue;
        };
        match manifest::parse_manifest(&text, &id, &id) {
            Ok(manifest) if manifest.transpile => selected.push(directory.clone()),
            Ok(_) => {}
            Err(errors) => diagnostics.extend(errors),
        }
    }
    if selected.is_empty() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "no transpile fixtures found",
        ));
        return (Vec::new(), None, sha256, None);
    }
    let mut reports = Vec::new();
    let mut version = None;
    for (index, directory) in selected.iter().enumerate() {
        let lane = transpile::run_lane(directory, &format!("selftest-{index}"));
        version = lane.version.or(version);
        diagnostics.extend(lane.diagnostics);
        reports.push(lane.report);
    }
    let digest = transpile::combined_digest(&reports);
    (reports, version, sha256, digest)
}

fn self_test_pipeline(fixtures: &Path, transpile_enabled: bool) -> (Pipeline, i32) {
    let mut diagnostics = Vec::new();
    let start = Instant::now();
    let directories = fixture_dirs(fixtures, &mut diagnostics);
    timing("discovery", start);
    if directories.is_empty() {
        diagnostics.push(Diagnostic::new(
            diag::MANIFEST_VALUE,
            "c2tv",
            None,
            None,
            "no fixtures found",
        ));
        let pipeline = failed_pipeline(diagnostics, Some(fixtures));
        let code = diag::exit_code(&pipeline.diagnostics);
        return (pipeline, code);
    }
    let start = Instant::now();
    let lock = match require_lock(fixtures) {
        Ok(lock) => lock,
        Err(error) => {
            let pipeline = failed_pipeline(vec![error], Some(fixtures));
            let code = diag::exit_code(&pipeline.diagnostics);
            return (pipeline, code);
        }
    };
    timing("lock", start);
    let start = Instant::now();
    let (mut first, mut second) = std::thread::scope(|scope| {
        let first_thread = scope.spawn(|| run_pipeline(&directories, "first"));
        let second_thread = scope.spawn(|| run_pipeline(&directories, "second"));
        (
            first_thread.join().expect("first pipeline panicked"),
            second_thread.join().expect("second pipeline panicked"),
        )
    });
    timing("pipelines", start);
    let first_bytes = report::canonical(&first.report());
    let second_bytes = report::canonical(&second.report());
    diagnostics.append(&mut first.diagnostics);
    if first_bytes != second_bytes {
        diagnostics.append(&mut second.diagnostics);
        diagnostics.push(Diagnostic::new(
            diag::DETERMINISM,
            "c2tv",
            None,
            None,
            "canonical report bytes differ between consecutive runs",
        ));
    }
    let start = Instant::now();
    downgrade_expected(&mut diagnostics, &first.fixtures);
    diagnostics.extend(digest::verify(
        &fixture_roots(&directories),
        &lock_digests(&first),
        &lock,
    ));
    timing("verification", start);
    diagnostics.extend(expectation_diagnostics(&first.fixtures));
    let mut c2rust_version = first.c2rust_version.clone();
    let mut c2rust_sha256 = None;
    let mut generated_digest = None;
    let mut transpile = None;
    if transpile_enabled {
        let start = Instant::now();
        let (reports, version, sha256, digest) =
            transpile_self_test(&directories, &mut diagnostics);
        timing("transpile_lane", start);
        transpile = Some(reports);
        c2rust_version = version.or(c2rust_version);
        c2rust_sha256 = sha256;
        generated_digest = digest;
    }
    let pipeline = Pipeline {
        clang_version: first.clang_version,
        rustc_version: first.rustc_version,
        c2rust_version,
        c2rust_sha256,
        generated_digest,
        transpile,
        fixtures: first.fixtures,
        diagnostics: diagnostics.clone(),
    };
    let code = diag::exit_code(&pipeline.diagnostics);
    (pipeline, code)
}

fn self_test(fixtures: PathBuf, json: bool, transpile_enabled: bool) -> i32 {
    let (pipeline, code) = self_test_pipeline(&fixtures, transpile_enabled);
    emit(&pipeline, json);
    code
}

fn bench_command(fixtures: PathBuf, json: bool) -> i32 {
    let start = Instant::now();
    let (pipeline, code) = self_test_pipeline(&fixtures, false);
    let elapsed = start.elapsed();
    let counts = bench::measure(&pipeline.fixtures, &pipeline.diagnostics);
    let exit = if counts.tool_errors > 0 {
        2
    } else if counts.false_positive > 0 || counts.false_negative > 0 {
        1
    } else {
        code
    };
    let rates = bench::rates(&counts);
    let report = bench::Report {
        schema: 1,
        clang_version: pipeline.clang_version,
        rustc_version: pipeline.rustc_version,
        c2rust_version: pipeline.c2rust_version,
        counts,
        rates,
    };
    if json {
        print!("{}", bench::canonical(&report));
    } else {
        print!("{}", bench::human(&report, elapsed));
    }
    exit
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
        return fail_with(diagnostics, false, Some(&fixtures));
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
    normalize_paths(&mut diagnostics, &fixtures);
    if !diagnostics.is_empty() {
        let code = diag::exit_code(&diagnostics);
        eprintln!(
            "{}",
            report::human(
                &Pipeline {
                    clang_version: build::version("clang"),
                    rustc_version: build::version("rustc"),
                    c2rust_version: build::probe_c2rust(),
                    c2rust_sha256: None,
                    generated_digest: None,
                    transpile: None,
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
        Command::Bench { fixtures, json } => bench_command(fixtures, json),
        Command::SelfTest {
            fixtures,
            json,
            transpile,
        } => self_test(fixtures, json, transpile),
        Command::Transpile { fixture, json } => transpile_cli(fixture, json),
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
