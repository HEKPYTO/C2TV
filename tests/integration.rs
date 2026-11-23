use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const C_SOURCE: &str = r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    unsigned char header[4];
    if (fread(header, 1, 4, stdin) != 4) return 3;
    uint32_t len = (uint32_t)header[0] | ((uint32_t)header[1] << 8) |
                   ((uint32_t)header[2] << 16) | ((uint32_t)header[3] << 24);
    unsigned char *data = (unsigned char *)malloc(len ? len : 1);
    if (!data) return 4;
    if (len && fread(data, 1, len, stdin) != len) return 5;
    uint64_t sum = 0;
    for (uint32_t i = 0; i < len; i++) sum += data[i];
    unsigned char out[20] = {0};
    for (int i = 0; i < 8; i++) out[8 + i] = (unsigned char)((sum >> (8 * i)) & 0xff);
    if (fwrite(out, 1, 20, stdout) != 20) return 6;
    free(data);
    return 0;
}
"#;

fn rust_source(delta: bool) -> String {
    let adjustment = if delta { "    sum += 1;\n" } else { "" };
    format!(
        r#"use std::io::{{Read, Write}};

fn main() {{
    let mut header = [0u8; 4];
    if std::io::stdin().read_exact(&mut header).is_err() {{
        std::process::exit(3);
    }}
    let len = u32::from_le_bytes(header) as usize;
    let mut data = vec![0u8; len];
    if std::io::stdin().read_exact(&mut data).is_err() {{
        std::process::exit(5);
    }}
    let mut sum: u64 = data.iter().map(|byte| *byte as u64).sum();
{adjustment}    let mut out = Vec::with_capacity(20);
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&sum.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    std::io::stdout().write_all(&out).unwrap();
}}
"#
    )
}

const RUST_MANIFEST: &str = r#"[package]
name = "harness"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "harness"
path = "src/main.rs"

[workspace]
"#;

const CORPUS: &str = "-\n01\n0102\nff00\n";

fn manifest(id: &str, bad: bool) -> String {
    let kind = if bad { "bad" } else { "good" };
    let expected = if bad { "diverge" } else { "pass" };
    let bug = if bad {
        "bug = \"off-by-one\"\nexpect_case = 0\nexpect_class = \"ret\"\n"
    } else {
        ""
    };
    format!(
        "schema = 1\nid = \"{id}\"\nkind = \"{kind}\"\nclasses = [\"ret\"]\nexpected = \"{expected}\"\n{bug}note = \"integration fixture\"\n"
    )
}

fn write_fixture(parent: &Path, id: &str, delta: bool) -> PathBuf {
    let directory = parent.join(id);
    fs::create_dir_all(directory.join("c")).unwrap();
    fs::create_dir_all(directory.join("rust").join("src")).unwrap();
    fs::write(directory.join("c").join("main.c"), C_SOURCE).unwrap();
    fs::write(directory.join("rust").join("Cargo.toml"), RUST_MANIFEST).unwrap();
    fs::write(
        directory.join("rust").join("src").join("main.rs"),
        rust_source(delta),
    )
    .unwrap();
    fs::write(directory.join("port.toml"), manifest(id, delta)).unwrap();
    fs::write(directory.join("corpus.hex"), CORPUS).unwrap();
    directory
}

fn sandbox(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("integration-{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn c2tv(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_c2tv"))
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn version_flag_reports_package_version() {
    let (code, stdout, stderr) = c2tv(&["--version"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    assert_eq!(stdout.trim(), format!("c2tv {}", env!("CARGO_PKG_VERSION")));
    assert!(stderr.is_empty(), "stderr={stderr}");
}

fn probe_c2rust(program: &str) -> Result<bool, String> {
    match Command::new(program)
        .args(["transpile", "--version"])
        .output()
    {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if output.status.success() && version == "transpile 0.22.1" {
                Ok(true)
            } else {
                Err(format!("expected transpile 0.22.1, got {version}"))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot start {program}: {error}")),
    }
}

fn require_c2rust(test: &str) -> bool {
    match probe_c2rust("c2rust") {
        Ok(true) => true,
        Ok(false) => {
            println!("SKIP {test}: c2rust transpile unavailable");
            false
        }
        Err(message) => panic!("{message}"),
    }
}

#[cfg(unix)]
fn path_without_c2rust() -> PathBuf {
    let root = sandbox("no-c2rust-path");
    let mut bins = Vec::new();
    let path = std::env::var_os("PATH").unwrap_or_default();
    for (index, directory) in std::env::split_paths(&path).enumerate() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        let bin = root.join(format!("bin{index}"));
        fs::create_dir_all(&bin).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_str() == Some("c2rust") || !entry.path().is_file() {
                continue;
            }
            std::os::unix::fs::symlink(entry.path(), bin.join(name)).unwrap();
        }
        bins.push(bin);
    }
    std::env::join_paths(bins).unwrap().into()
}

#[cfg(not(unix))]
fn path_without_c2rust() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let bins: Vec<PathBuf> = std::env::split_paths(&path)
        .filter(|directory| {
            !directory.join("c2rust").is_file() && !directory.join("c2rust.exe").is_file()
        })
        .collect();
    std::env::join_paths(bins).unwrap_or_default().into()
}

#[cfg(unix)]
#[test]
fn unusable_transpiler_fails_instead_of_skipping() {
    let root = sandbox("unusable-c2rust");
    let program = root.join("c2rust");
    fs::write(&program, b"not executable").unwrap();
    assert_eq!(
        probe_c2rust(root.join("absent-c2rust").to_str().unwrap()),
        Ok(false)
    );
    assert!(probe_c2rust(program.to_str().unwrap()).is_err());
}
fn lock_fixtures(root: &Path) {
    let (code, _, stderr) = c2tv(&["lock", "--fixtures", root.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr={stderr}");
}

#[test]
fn passing_fixture_exits_zero() {
    let root = sandbox("pass");
    let fixture = write_fixture(&root, "good", false);
    lock_fixtures(&root);
    let (code, stdout, stderr) = c2tv(&["check", "--fixture", fixture.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    assert!(stdout.contains("\"observed\":\"pass\""));
    assert!(stdout.contains("\"clang_version\""));
    assert!(stdout.contains("\"rustc_version\""));
    assert!(!stdout.contains("TV0008"));
}

#[test]
fn divergent_fixture_exits_one() {
    let root = sandbox("diverge");
    let fixture = write_fixture(&root, "bad", true);
    lock_fixtures(&root);
    let (code, stdout, stderr) = c2tv(&["check", "--fixture", fixture.to_str().unwrap(), "--json"]);
    assert_eq!(code, 1, "stderr={stderr}\nstdout={stdout}");
    assert!(stdout.contains("\"observed\":\"diverge\""));
    assert!(stdout.contains("TV0008"));
    assert!(stdout.contains("first differing byte"));
}

#[test]
fn self_test_accepts_truth_table_and_detects_digest_drift() {
    let root = sandbox("selftest");
    write_fixture(&root, "good", false);
    write_fixture(&root, "bad", true);
    lock_fixtures(&root);
    let root_name = root.to_str().unwrap();

    let (code, stdout, stderr) = c2tv(&["self-test", "--fixtures", root_name, "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    assert_eq!(stdout.matches("\"id\":\"good\"").count(), 1);
    assert_eq!(stdout.matches("\"id\":\"bad\"").count(), 1);

    let (code, _, stderr) = c2tv(&["lock", "--fixtures", root_name]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(root.join("digests.toml").is_file());

    let (code, stdout, _) = c2tv(&["self-test", "--fixtures", root_name, "--json"]);
    assert_eq!(code, 0, "stdout={stdout}");

    fs::write(root.join("bad").join("corpus.hex"), "-\n01\n").unwrap();
    let (code, stdout, _) = c2tv(&["self-test", "--fixtures", root_name, "--json"]);
    assert_eq!(code, 2, "stdout={stdout}");
    assert!(stdout.contains("TV0011"));
}

fn write_exit_fixture(parent: &Path) -> PathBuf {
    let directory = parent.join("broken");
    fs::create_dir_all(directory.join("c")).unwrap();
    fs::create_dir_all(directory.join("rust").join("src")).unwrap();
    fs::write(
        directory.join("c").join("main.c"),
        "#include <stdio.h>\nint main(void) { while (getchar() != EOF) {} return 7; }\n",
    )
    .unwrap();
    fs::write(directory.join("rust").join("Cargo.toml"), RUST_MANIFEST).unwrap();
    fs::write(
        directory.join("rust").join("src").join("main.rs"),
        "use std::io::Read;\nfn main() { let mut sink = Vec::new(); let _ = std::io::stdin().read_to_end(&mut sink); std::process::exit(7); }\n",
    )
    .unwrap();
    fs::write(directory.join("port.toml"), manifest("broken", false)).unwrap();
    fs::write(directory.join("corpus.hex"), "-\n").unwrap();
    directory
}

#[test]
fn both_side_failures_are_reported() {
    let root = sandbox("bothfail");
    let fixture = write_exit_fixture(&root);
    lock_fixtures(&root);
    let (code, stdout, _) = c2tv(&["check", "--fixture", fixture.to_str().unwrap(), "--json"]);
    assert_eq!(code, 2, "stdout={stdout}");
    assert!(stdout.contains("\"observed\":\"error\""));
    assert_eq!(stdout.matches("TV0007").count(), 5);
}

#[test]
fn empty_fixture_set_is_tool_error() {
    let root = sandbox("empty");
    let (code, stdout, _) = c2tv(&["self-test", "--fixtures", root.to_str().unwrap(), "--json"]);
    assert_eq!(code, 2, "stdout={stdout}");
    assert!(stdout.contains("TV0003"));
    assert!(stdout.contains("no fixtures found"));
}

#[test]
fn corrupt_lock_is_tool_error() {
    let root = sandbox("corruptlock");
    write_fixture(&root, "good", false);
    fs::write(root.join("digests.toml"), "not toml = [").unwrap();
    let (code, stdout, _) = c2tv(&["self-test", "--fixtures", root.to_str().unwrap(), "--json"]);
    assert_eq!(code, 2, "stdout={stdout}");
    assert!(stdout.contains("TV0011"));
}

#[test]
fn duplicate_ids_across_kinds_fail_closed() {
    let root = sandbox("duplicates");
    write_fixture(&root.join("a"), "dup", false);
    write_fixture(&root.join("m"), "mid", false);
    write_fixture(&root.join("z"), "dup", false);
    let (code, stdout, stderr) =
        c2tv(&["self-test", "--fixtures", root.to_str().unwrap(), "--json"]);
    assert_eq!(code, 2, "stderr={stderr}\nstdout={stdout}");
    assert!(stdout.contains("TV0003"), "stdout={stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["fixtures"].as_array().unwrap().len(), 0);
    assert!(!stdout.contains("TV0008"), "stdout={stdout}");
}

#[test]
fn transpile_crc32_matches_c_and_is_deterministic() {
    if !require_c2rust("transpile_crc32_matches_c_and_is_deterministic") {
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("good")
        .join("crc32");
    let argument = fixture.to_str().unwrap();
    let (code, stdout, stderr) = c2tv(&["transpile", "--fixture", argument, "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    let first: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(first["c2rust_version"], "transpile 0.22.1");
    assert_eq!(first["c2rust_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(first["transpile"][0]["verdict"], "pass");
    assert_eq!(
        first["transpile"][0]["binary_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );

    let (code, stdout, stderr) = c2tv(&["transpile", "--fixture", argument, "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    let second: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(first["generated_digest"], second["generated_digest"]);
    assert_eq!(
        first["transpile"][0]["generated_digest"],
        second["transpile"][0]["generated_digest"]
    );
    assert_eq!(
        first["transpile"][0]["binary_sha256"],
        second["transpile"][0]["binary_sha256"]
    );
}

#[test]
fn self_test_transpile_runs_flagged_fixtures() {
    if !require_c2rust("self_test_transpile_runs_flagged_fixtures") {
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .to_str()
        .unwrap()
        .to_string();
    let (code, stdout, stderr) = c2tv(&[
        "self-test",
        "--transpile",
        "--fixtures",
        &fixtures,
        "--json",
    ]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entries = report["transpile"].as_array().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let expected: BTreeSet<String> = ["bad", "good", "real"]
        .iter()
        .flat_map(|kind| fs::read_dir(fixture_root.join(kind)).unwrap())
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            let manifest = fs::read_to_string(path.join("port.toml")).unwrap();
            manifest
                .contains("transpile = true")
                .then(|| path.file_name().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    let actual: BTreeSet<String> = entries
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(entries.len(), 5);
    assert_eq!(actual, expected);
    assert_eq!(report["fixtures"].as_array().unwrap().len(), 26);
    assert!(entries.iter().all(|entry| entry["verdict"] == "pass"));
}

#[test]
fn missing_transpiler_fails_transpile_self_test_closed() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .to_str()
        .unwrap()
        .to_string();
    let path = path_without_c2rust();
    let output = Command::new(env!("CARGO_BIN_EXE_c2tv"))
        .args([
            "self-test",
            "--transpile",
            "--fixtures",
            &fixtures,
            "--json",
        ])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TV0016"), "stdout={stdout}");
    assert!(stdout.contains("TV0014"), "stdout={stdout}");
}
#[test]
fn check_one_fixture_with_full_lock_passes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    assert!(fixtures.join("digests.toml").is_file());
    let fixture = fixtures.join("good").join("crc32");
    let (code, stdout, stderr) = c2tv(&["check", "--fixture", fixture.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    assert!(stdout.contains("\"observed\":\"pass\""));
    assert!(!stdout.contains("TV0011"), "stdout={stdout}");
}

#[test]
fn bench_publishes_rates_for_real_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let (code, stdout, stderr) =
        c2tv(&["bench", "--fixtures", fixtures.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "stderr={stderr}\nstdout={stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let counts = &value["counts"];
    assert_eq!(counts["total"].as_u64(), Some(26));
    assert_eq!(counts["good"].as_u64(), Some(20));
    assert_eq!(counts["bad"].as_u64(), Some(3));
    assert_eq!(counts["real"].as_u64(), Some(3));
    assert_eq!(counts["true_positive"].as_u64(), Some(3));
    assert_eq!(counts["false_negative"].as_u64(), Some(0));
    assert_eq!(counts["true_negative"].as_u64(), Some(23));
    assert_eq!(counts["false_positive"].as_u64(), Some(0));
    assert_eq!(counts["tool_errors"].as_u64(), Some(0));
    let rates = &value["rates"];
    assert_eq!(rates["false_positive"]["numerator"].as_u64(), Some(0));
    assert_eq!(rates["false_positive"]["denominator"].as_u64(), Some(23));
    assert_eq!(rates["false_positive"]["rate"].as_f64(), Some(0.0));
    assert_eq!(rates["false_negative"]["numerator"].as_u64(), Some(0));
    assert_eq!(rates["false_negative"]["denominator"].as_u64(), Some(3));
    assert_eq!(rates["false_negative"]["rate"].as_f64(), Some(0.0));
}

#[test]
fn doctor_strict_json_reports_toolchain() {
    let root = sandbox("doctor");
    let output = Command::new(env!("CARGO_BIN_EXE_c2tv"))
        .args(["doctor", "--json", "--strict"])
        .current_dir(&root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={stderr}\nstdout={stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["schema"], 1);
    assert_eq!(value["tool"], "c2tv");
    assert_eq!(value["c2tv_version"], env!("CARGO_PKG_VERSION"));
    assert!(value["rustc"]["present"].as_bool().unwrap());
    assert!(value["cargo"]["version"].as_str().is_some());
    assert!(value["clang"]["present"].as_bool().unwrap());
    assert!(value["c2rust"].is_object());
    assert!(value["diagnostics"].is_array());
    assert!(value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|diagnostic| diagnostic["severity"] != "error"));
    assert!(!stdout.contains(&root.display().to_string()));
    fs::remove_dir_all(&root).unwrap();
}
