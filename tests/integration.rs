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
        "bug = \"off-by-one\"\n"
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
    fs::write(directory.join("rust").join("src").join("main.rs"), rust_source(delta)).unwrap();
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
fn passing_fixture_exits_zero() {
    let root = sandbox("pass");
    let fixture = write_fixture(&root, "good", false);
    let (code, stdout, stderr) = c2tv(&[
        "check",
        "--fixture",
        fixture.to_str().unwrap(),
        "--json",
    ]);
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
    let (code, stdout, stderr) = c2tv(&[
        "check",
        "--fixture",
        fixture.to_str().unwrap(),
        "--json",
    ]);
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
    fs::write(directory.join("c").join("main.c"), "int main(void) { return 7; }\n").unwrap();
    fs::write(directory.join("rust").join("Cargo.toml"), RUST_MANIFEST).unwrap();
    fs::write(
        directory.join("rust").join("src").join("main.rs"),
        "fn main() { std::process::exit(7); }\n",
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
