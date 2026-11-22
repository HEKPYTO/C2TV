use crate::build;
use crate::compare::FirstDiff;
use crate::compare::Observation;
use crate::diag::{self, Diagnostic, Level};
use crate::digest::sha256_hex;
use crate::manifest;
use crate::process;
use crate::report::{self, TranspileReport};
use crate::run;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
const EXPECTED_VERSION: &str = "transpile 0.22.1";
const SFILEX_STRUCT: &str = "#[repr(C)]\npub struct __sFILEX {\n    _private: [u8; 0],\n}\n";
const GENERATED_FILES: [&str; 4] = ["Cargo.toml", "build.rs", "lib.rs", "src/main.rs"];

pub struct LaneResult {
    pub version: Option<String>,
    pub sha256: Option<String>,
    pub digest: Option<String>,
    pub report: TranspileReport,
    pub diagnostics: Vec<Diagnostic>,
}

fn transpile_error(fixture: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(diag::TRANSPILE_FAILED, fixture, None, None, message)
}

pub fn check_version(version: Option<String>) -> Result<String, Diagnostic> {
    match version {
        Some(value) if value == EXPECTED_VERSION => Ok(value),
        Some(value) => Err(Diagnostic::new(
            diag::TRANSPILER_VERSION,
            "c2tv",
            None,
            None,
            format!("expected {EXPECTED_VERSION}, got {value}"),
        )),
        None => Err(Diagnostic::new(
            diag::TRANSPILER_VERSION,
            "c2tv",
            None,
            None,
            format!("expected {EXPECTED_VERSION}, transpiler unavailable"),
        )),
    }
}

pub fn binary_sha256(program: &str) -> Result<String, Diagnostic> {
    let path = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
    });
    let bytes = match path {
        Some(path) => fs::read(path).map_err(|error| {
            Diagnostic::new(
                diag::TRANSPILER_VERSION,
                "c2tv",
                None,
                None,
                format!("cannot read transpiler binary: {error}"),
            )
        })?,
        None => {
            return Err(Diagnostic::new(
                diag::TRANSPILER_VERSION,
                "c2tv",
                None,
                None,
                "transpiler binary not found on PATH",
            ))
        }
    };
    Ok(sha256_hex(&bytes))
}

fn read_corpus(
    directory: &Path,
    fixture: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Vec<u8>>> {
    let text = match fs::read_to_string(directory.join("corpus.hex")) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::CORPUS_HEX,
                fixture,
                None,
                None,
                format!("cannot read corpus: {error}"),
            ));
            return None;
        }
    };
    match manifest::parse_corpus(&text, fixture) {
        Ok(cases) => Some(cases),
        Err(errors) => {
            diagnostics.extend(errors);
            None
        }
    }
}

fn strip_features(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| !line.trim_start().starts_with("#![feature("))
        .collect()
}

fn replace_sfilex(text: &str, fixture: &str) -> Result<String, Diagnostic> {
    let mut lines: Vec<String> = Vec::new();
    let mut replacements = 0;
    for line in text.split_inclusive('\n') {
        if line.trim() == "pub type __sFILEX;" {
            replacements += 1;
            continue;
        }
        lines.push(line.to_string());
    }
    if replacements != 1 {
        return Err(transpile_error(
            fixture,
            format!("expected one __sFILEX extern type, found {replacements}"),
        ));
    }
    let insertion = lines
        .iter()
        .position(|line| line.trim() == "extern \"C\" {")
        .ok_or_else(|| transpile_error(fixture, "no extern block for __sFILEX replacement"))?;
    lines.splice(
        insertion..insertion,
        SFILEX_STRUCT.lines().map(|line| format!("{line}\n")),
    );
    Ok(lines.concat())
}

fn validate_layout(out: &Path, fixture: &str) -> Result<(), Diagnostic> {
    let mut names: Vec<String> = fs::read_dir(out)
        .map_err(|error| transpile_error(fixture, format!("cannot read generated crate: {error}")))?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|error| {
                    transpile_error(fixture, format!("cannot read generated file: {error}"))
                })
        })
        .collect::<Result<_, _>>()?;
    names.sort();
    let expected = [
        "Cargo.toml",
        "build.rs",
        "lib.rs",
        "rust-toolchain.toml",
        "src",
    ];
    if names
        .iter()
        .map(String::as_str)
        .ne(expected.iter().copied())
    {
        return Err(transpile_error(
            fixture,
            format!("unexpected generated layout: {}", names.join(",")),
        ));
    }
    let mut source_names: Vec<String> = fs::read_dir(out.join("src"))
        .map_err(|error| transpile_error(fixture, format!("cannot read generated src: {error}")))?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|error| {
                    transpile_error(fixture, format!("cannot read generated file: {error}"))
                })
        })
        .collect::<Result<_, _>>()?;
    source_names.sort();
    if source_names.len() != 1 || source_names[0] != "main.rs" {
        return Err(transpile_error(
            fixture,
            format!(
                "unexpected generated src layout: {}",
                source_names.join(",")
            ),
        ));
    }
    Ok(())
}

fn generated_digest(out: &Path, fixture: &str) -> Result<String, Diagnostic> {
    let mut canonical = String::new();
    for name in GENERATED_FILES {
        let bytes = fs::read(out.join(name))
            .map_err(|error| transpile_error(fixture, format!("cannot read {name}: {error}")))?;
        let _ = writeln!(canonical, "{name}\t{}", sha256_hex(&bytes));
    }
    Ok(sha256_hex(canonical.as_bytes()))
}

fn postprocess(out: &Path, fixture: &str) -> Result<String, Diagnostic> {
    validate_layout(out, fixture)?;
    let lib = fs::read_to_string(out.join("lib.rs"))
        .map_err(|error| transpile_error(fixture, format!("cannot read lib.rs: {error}")))?;
    let main = fs::read_to_string(out.join("src/main.rs"))
        .map_err(|error| transpile_error(fixture, format!("cannot read src/main.rs: {error}")))?;
    let main = strip_features(&replace_sfilex(&main, fixture)?);
    fs::write(out.join("lib.rs"), strip_features(&lib))
        .map_err(|error| transpile_error(fixture, format!("cannot rewrite lib.rs: {error}")))?;
    fs::write(out.join("src/main.rs"), main).map_err(|error| {
        transpile_error(fixture, format!("cannot rewrite src/main.rs: {error}"))
    })?;
    fs::remove_file(out.join("rust-toolchain.toml")).map_err(|error| {
        transpile_error(fixture, format!("cannot remove toolchain pin: {error}"))
    })?;
    generated_digest(out, fixture)
}

fn transpile_once(
    source: &Path,
    out: &Path,
    fixture: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let source = match source.canonicalize() {
        Ok(source) => source,
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                fixture,
                None,
                None,
                format!("cannot resolve C source: {error}"),
            ));
            return None;
        }
    };
    let out = match out.canonicalize() {
        Ok(out) => out,
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                fixture,
                None,
                None,
                format!("cannot resolve transpile output: {error}"),
            ));
            return None;
        }
    };
    let source_arg = source.to_string_lossy().into_owned();
    let out_arg = out.to_string_lossy().into_owned();
    let args = ["transpile", "-b", "main", "-o", &out_arg, &source_arg];
    let mut command = process::sanitized("c2rust");
    command.args(args);
    let captured = match process::run(&mut command, None, process::BUILD_TIMEOUT) {
        Ok(captured) => captured,
        Err(error) => {
            diagnostics.push(transpile_error(
                fixture,
                format!("cannot run c2rust transpile: {error}"),
            ));
            return None;
        }
    };
    if captured.truncated {
        diagnostics.push(diag::truncation(
            fixture,
            None,
            None,
            format!(
                "c2rust transpile output exceeded {} byte budget",
                process::OUTPUT_CAP
            ),
        ));
    }
    if captured.timed_out {
        diagnostics.push(Diagnostic::new(
            diag::TIMEOUT,
            fixture,
            None,
            None,
            format!(
                "c2rust transpile exceeded {}ms timeout",
                process::BUILD_TIMEOUT.as_millis()
            ),
        ));
        return None;
    }
    if !captured.success {
        diagnostics.push(transpile_error(
            fixture,
            format!(
                "c2rust transpile exited {}: {}",
                captured.status,
                String::from_utf8_lossy(&captured.stderr).trim()
            ),
        ));
        return None;
    }
    match postprocess(&out, fixture) {
        Ok(digest) => Some(digest),
        Err(error) => {
            diagnostics.push(error);
            None
        }
    }
}

fn build_transpiled(
    fixture: &str,
    crate_dir: &Path,
    target_dir: &Path,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let mut command = process::sanitized("cargo");
    command
        .args(["build", "--offline", "--release"])
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "2");
    let captured = process::run(&mut command, None, process::BUILD_TIMEOUT).map_err(|error| {
        vec![Diagnostic::new(
            diag::TRANSPILED_BUILD_FAILED,
            fixture,
            None,
            None,
            format!("cannot run cargo build: {error}"),
        )]
    })?;
    let mut errors = Vec::new();
    if captured.truncated {
        errors.push(diag::truncation(
            fixture,
            None,
            None,
            format!(
                "cargo build output exceeded {} byte budget",
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
                "cargo build exceeded {}ms timeout",
                process::BUILD_TIMEOUT.as_millis()
            ),
        ));
        return Err(errors);
    }
    if !captured.success {
        errors.push(Diagnostic::new(
            diag::TRANSPILED_BUILD_FAILED,
            fixture,
            None,
            None,
            format!(
                "cargo build exited {}: {}",
                captured.status,
                String::from_utf8_lossy(&captured.stderr).trim()
            ),
        ));
        return Err(errors);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let binary = target_dir.join("release").join("main");
    if !binary.is_file() {
        return Err(vec![Diagnostic::new(
            diag::TRANSPILED_BUILD_FAILED,
            fixture,
            None,
            None,
            "cargo build produced no main binary",
        )]);
    }
    Ok(binary)
}

fn exact_frame(observation: &Observation) -> Vec<u8> {
    let mut frame = Vec::with_capacity(20 + observation.out.len());
    frame.extend_from_slice(&observation.status.to_le_bytes());
    frame.extend_from_slice(&observation.ret.to_le_bytes());
    frame.extend_from_slice(&(observation.out.len() as u32).to_le_bytes());
    frame.extend_from_slice(&observation.out);
    frame
}

fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    let common = left.len().min(right.len());
    left[..common]
        .iter()
        .zip(right[..common].iter())
        .position(|(a, b)| a != b)
        .or_else(|| (left.len() != right.len()).then_some(common))
}

pub fn combined_digest(entries: &[TranspileReport]) -> Option<String> {
    let mut pairs = Vec::new();
    for entry in entries {
        pairs.push((entry.id.as_str(), entry.generated_digest.as_deref()?));
    }
    pairs.sort_unstable();
    let mut canonical = String::new();
    for (id, digest) in pairs {
        let _ = writeln!(canonical, "{id}\t{digest}");
    }
    (!canonical.is_empty()).then(|| sha256_hex(canonical.as_bytes()))
}

fn prepare_root(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> bool {
    if fs::symlink_metadata(root).is_ok() {
        diagnostics.push(Diagnostic::new(
            diag::SPAWN,
            "c2tv",
            None,
            None,
            format!("transpile work root already exists: {}", root.display()),
        ));
        return false;
    }
    if let Err(error) = fs::create_dir(root) {
        diagnostics.push(Diagnostic::new(
            diag::SPAWN,
            "c2tv",
            None,
            None,
            format!("cannot create transpile work root: {error}"),
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
                "transpile work root became a symlink",
            ));
            false
        }
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                "c2tv",
                None,
                None,
                format!("cannot verify transpile work root: {error}"),
            ));
            false
        }
    }
}

pub fn transpiled_rustc_version() -> String {
    build::version("rustc")
}

fn error_result(
    fixture: String,
    version: Option<String>,
    sha256: Option<String>,
    diagnostics: Vec<Diagnostic>,
) -> LaneResult {
    LaneResult {
        version,
        sha256,
        digest: None,
        report: TranspileReport {
            id: fixture,
            verdict: "error",
            generated_digest: None,
            first_diff: None,
            rustc_version: None,
        },
        diagnostics,
    }
}

pub fn run_lane(directory: &Path, slot: &str) -> LaneResult {
    let fixture = directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut diagnostics = Vec::new();
    let version = match check_version(build::probe_transpile()) {
        Ok(version) => version,
        Err(error) => {
            diagnostics.push(error);
            return error_result(fixture, None, None, diagnostics);
        }
    };
    let sha256 = match binary_sha256("c2rust") {
        Ok(sha256) => sha256,
        Err(error) => {
            diagnostics.push(error);
            return error_result(fixture, Some(version), None, diagnostics);
        }
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let roots = ["a", "b"].map(|side| {
        env::temp_dir().join(format!(
            "c2tv-transpile-{fixture}-{}-{slot}-{nonce}-{side}",
            std::process::id()
        ))
    });
    let mut digests = Vec::new();
    for root in &roots {
        if !prepare_root(root, &mut diagnostics) {
            break;
        }
        let out = root.join("out");
        if let Err(error) = fs::create_dir_all(&out) {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                &fixture,
                None,
                None,
                format!("cannot create transpile output: {error}"),
            ));
            break;
        }
        match transpile_once(
            &directory.join("c").join("main.c"),
            &out,
            &fixture,
            &mut diagnostics,
        ) {
            Some(digest) => digests.push(digest),
            None => break,
        }
    }

    let digest = if digests.len() == 2 {
        if digests[0] == digests[1] {
            Some(digests[0].clone())
        } else {
            diagnostics.push(Diagnostic::new(
                diag::TRANSPILED_NONDETERMINISM,
                &fixture,
                None,
                None,
                "generated crate digests differ between transpiles",
            ));
            None
        }
    } else {
        None
    };

    let mut verdict = "error";
    let mut first_diff = None;
    if let Some(digest) = &digest {
        let _ = digest;
        let c_directory = roots[0].join("c_o2");
        let c_binary = c_directory.join("harness");
        if let Err(error) = fs::create_dir_all(&c_directory) {
            diagnostics.push(Diagnostic::new(
                diag::SPAWN,
                &fixture,
                None,
                None,
                format!("cannot create C build directory: {error}"),
            ));
        } else {
            let source = directory
                .join("c")
                .join("main.c")
                .to_string_lossy()
                .into_owned();
            build::c_step(
                &fixture,
                Level::O2,
                &source,
                &c_binary,
                false,
                &mut diagnostics,
            );
            let rust_binary =
                build_transpiled(&fixture, &roots[0].join("out"), &roots[0].join("target"));
            match (
                read_corpus(directory, &fixture, &mut diagnostics),
                rust_binary,
            ) {
                (Some(cases), Ok(rust_binary)) => {
                    let mut errored = false;
                    let mut diverged = false;
                    for (index, payload) in cases.iter().enumerate() {
                        let c_result = run::run(&c_binary, payload, &fixture, index, Level::O2);
                        let r_result = run::run(&rust_binary, payload, &fixture, index, Level::O2);
                        let (c_response, r_response) = match (c_result, r_result) {
                            (Ok((c_response, _)), Ok((r_response, _))) => (c_response, r_response),
                            (c_result, r_result) => {
                                for result in [c_result, r_result] {
                                    if let Err(errors) = result {
                                        diagnostics.extend(errors);
                                    }
                                }
                                errored = true;
                                continue;
                            }
                        };
                        for observation in [&c_response, &r_response] {
                            if observation.out.len() > report::OUT_HEX_CAP {
                                diagnostics.push(diag::truncation(
                                    &fixture,
                                    Some(index),
                                    Some(Level::O2),
                                    format!(
                                        "response out_hex truncated to {} bytes from {}",
                                        report::OUT_HEX_CAP,
                                        observation.out.len()
                                    ),
                                ));
                            }
                        }
                        let left = exact_frame(&c_response);
                        let right = exact_frame(&r_response);
                        if let Some(offset) = first_difference(&left, &right) {
                            let difference = FirstDiff {
                                field: "frame",
                                offset,
                                c: left.get(offset).copied().unwrap_or(0),
                                r: right.get(offset).copied().unwrap_or(0),
                            };
                            diagnostics.push(Diagnostic::new(
                                diag::TRANSPILED_DIVERGENCE,
                                &fixture,
                                Some(index),
                                Some(Level::O2),
                                difference.message(),
                            ));
                            if first_diff.is_none() {
                                first_diff = Some(difference);
                            }
                            diverged = true;
                        }
                    }
                    verdict = if errored {
                        "error"
                    } else if diverged {
                        "diverge"
                    } else {
                        "pass"
                    };
                }
                (_, Err(errors)) => diagnostics.extend(errors),
                (None, _) => {}
            }
        }
    }

    for root in &roots {
        let needle = root.to_string_lossy();
        for diagnostic in &mut diagnostics {
            diagnostic.message = diagnostic.message.replace(needle.as_ref(), "<work_root>");
        }
        let _ = fs::remove_dir_all(root);
    }
    let needle = directory.to_string_lossy();
    for diagnostic in &mut diagnostics {
        diagnostic.message = diagnostic.message.replace(needle.as_ref(), "<fixture>");
    }

    LaneResult {
        version: Some(version),
        sha256: Some(sha256),
        digest: digest.clone(),
        report: TranspileReport {
            id: fixture,
            verdict,
            generated_digest: digest,
            first_diff,
            rustc_version: Some(transpiled_rustc_version()),
        },
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_must_match_exactly() {
        assert_eq!(
            check_version(Some(EXPECTED_VERSION.into())).unwrap(),
            EXPECTED_VERSION
        );
        for version in [
            None,
            Some("C2Rust 0.22.1".into()),
            Some("transpile 0.22.0".into()),
        ] {
            let error = check_version(version).unwrap_err();
            assert_eq!(error.code, "TV0016");
            assert_eq!(error.exit_class, diag::ExitClass::ToolError);
            assert_eq!(diag::exit_code(&[error]), 2);
        }
    }

    #[test]
    fn missing_transpiler_fails_closed() {
        let version = build::probe_args(
            "c2tv-missing-c2rust-transpiler",
            &["transpile", "--version"],
        );
        assert!(version.is_none());
        let error = check_version(version).unwrap_err();
        assert_eq!(diag::exit_code(&[error]), 2);
    }

    #[test]
    fn strips_feature_lines_only() {
        let text = "#![allow(dead_code)]\n  #![feature(extern_types, raw_ref_op)]\n#![feature(extern_types)]\nfn main() {}\n";
        assert_eq!(strip_features(text), "#![allow(dead_code)]\nfn main() {}\n");
    }

    #[test]
    fn replaces_sfilex_extern_type() {
        let text = "extern \"C\" {\n    pub type __sFILEX;\n    static mut X: i32;\n}\n";
        let result = replace_sfilex(text, "f").unwrap();
        assert!(result.starts_with(SFILEX_STRUCT));
        assert!(result.contains("extern \"C\" {\n    static mut X: i32;\n}\n"));
        assert!(!result.contains("pub type __sFILEX;"));
    }

    #[test]
    fn unexpected_layout_is_typed() {
        let root = env::temp_dir().join(format!("c2tv-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("unexpected"), "x").unwrap();
        let error = postprocess(&root, "f").unwrap_err();
        assert_eq!(error.code, "TV0017");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn build_failure_maps_to_tv0018() {
        let root = env::temp_dir().join(format!("c2tv-buildfail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"broken\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n[[bin]]\nname = \"main\"\npath = \"src/main.rs\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() { undefined(); }\n",
        )
        .unwrap();
        let errors = build_transpiled("f", &root, &root.join("target")).unwrap_err();
        assert_eq!(errors[0].code, "TV0018");
        assert!(errors[0].message.contains("cannot find function"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn combined_digest_is_order_independent_over_sorted_entries() {
        let entry = |id: &str, digest: &str| TranspileReport {
            id: id.to_string(),
            verdict: "pass",
            generated_digest: Some(digest.to_string()),
            first_diff: None,
            rustc_version: None,
        };
        let left = combined_digest(&[entry("a", "aa"), entry("b", "bb")]).unwrap();
        let right = combined_digest(&[entry("b", "bb"), entry("a", "aa")]).unwrap();
        assert_eq!(left, right);
    }
    #[test]
    fn transpiled_rustc_version_is_recorded() {
        let version = transpiled_rustc_version();
        assert!(version.starts_with("rustc "), "version={version}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_work_root_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = env::temp_dir().join(format!("c2tv-transpile-root-{}", std::process::id()));
        let target = env::temp_dir().join(format!("c2tv-transpile-target-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&target);
        fs::create_dir_all(&target).unwrap();
        symlink(&target, &root).unwrap();
        let mut diagnostics = Vec::new();
        assert!(!prepare_root(&root, &mut diagnostics));
        assert_eq!(diagnostics[0].code, "TV0005");
        fs::remove_dir_all(&target).unwrap();
        fs::remove_file(&root).unwrap();
    }
}
