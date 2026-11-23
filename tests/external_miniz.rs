use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const VENDOR: &str = "external/miniz";
const C_SHA256: &str = "7487d4c8cd761b951a99d182672bb3badfa72a5ad5760b70b392fafa95223657";
const H_SHA256: &str = "13a6940a8f33b3a76d4b64fb6d482b246954bae3bf5dc713f7f054d2096d0aba";
const LICENSE_SHA256: &str = "0115478d567121238cf6cc1c0c361926cf07a49d9e4c9e66da97fac6a01646b3";
const EVIDENCE: &str = "independent production implementations, not affiliated with this project; behavioural evidence, not proof";
const C_CLI: &str = r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "miniz.h"

static int read_all(FILE *stream, unsigned char **out, size_t *out_len) {
    size_t cap = 4096;
    size_t len = 0;
    unsigned char *buf = (unsigned char *)malloc(cap);
    if (!buf) return 0;
    for (;;) {
        if (len == cap) {
            if (cap > (size_t)-1 / 2) { free(buf); return 0; }
            cap *= 2;
            unsigned char *next = (unsigned char *)realloc(buf, cap);
            if (!next) { free(buf); return 0; }
            buf = next;
        }
        size_t got = fread(buf + len, 1, cap - len, stream);
        len += got;
        if (got == 0) {
            if (ferror(stream)) { free(buf); return 0; }
            break;
        }
    }
    *out = buf;
    *out_len = len;
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    int do_compress;
    if (strcmp(argv[1], "c") == 0) do_compress = 1;
    else if (strcmp(argv[1], "d") == 0) do_compress = 0;
    else return 2;
    unsigned char *src = NULL;
    size_t src_len = 0;
    if (!read_all(stdin, &src, &src_len)) return 3;
    mz_ulong cap = do_compress ? mz_compressBound((mz_ulong)src_len) : (mz_ulong)(1u << 20);
    if (cap < 1024u) cap = 1024u;
    unsigned char *dst = (unsigned char *)malloc((size_t)cap);
    if (!dst) { free(src); return 4; }
    mz_ulong dst_len = cap;
    int status = do_compress ? mz_compress(dst, &dst_len, src, (mz_ulong)src_len) : mz_uncompress(dst, &dst_len, src, (mz_ulong)src_len);
    if (status != MZ_OK) { free(dst); free(src); return 5; }
    if (fwrite(dst, 1, (size_t)dst_len, stdout) != (size_t)dst_len) { free(dst); free(src); return 6; }
    free(dst);
    free(src);
    return fflush(stdout) == 0 ? 0 : 7;
}
"#;

fn sha256(path: &Path) -> String {
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn corpus() -> Vec<(&'static str, Vec<u8>)> {
    let mut state = 0x1234_5678_u32;
    let pseudo_random: Vec<u8> = (0..4096)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    let all_bytes: Vec<u8> = (0..1024).map(|index| (index % 256) as u8).collect();
    vec![
        ("empty", Vec::new()),
        ("short text", b"short text".to_vec()),
        ("64 KiB repeated", vec![b'R'; 64 * 1024]),
        ("4 KiB pseudo-random", pseudo_random),
        ("all 256 byte values repeated four times", all_bytes),
        ("UTF-8 string", "café — 世界 — miniz".as_bytes().to_vec()),
    ]
}

fn run_c(binary: &Path, mode: &str, input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(binary)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("cannot start C CLI: {error}"));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input)
        .expect("cannot write C CLI stdin");
    let output = child.wait_with_output().expect("cannot wait for C CLI");
    assert!(
        output.status.success(),
        "C CLI {mode} failed for {} bytes with {}: {}",
        input.len(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn rust_compress(input: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(input, 6)
}

fn rust_decompress(input: &[u8]) -> Vec<u8> {
    miniz_oxide::inflate::decompress_to_vec_zlib(input)
        .expect("miniz_oxide cannot decompress zlib data")
}

#[test]
fn external_miniz_and_miniz_oxide_are_independent_production_ports_behavioural_evidence_not_proof()
{
    let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join(VENDOR);
    for (name, expected) in [
        ("miniz.c", C_SHA256),
        ("miniz.h", H_SHA256),
        ("LICENSE", LICENSE_SHA256),
    ] {
        assert_eq!(
            sha256(&vendor.join(name)),
            expected,
            "vendored {name} drift; drift must fail closed; {EVIDENCE}"
        );
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let work: PathBuf =
        std::env::temp_dir().join(format!("c2tv-miniz-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&work).expect("cannot create temporary build directory");
    let source = work.join("miniz_cli.c");
    let binary = work.join("miniz_cli");
    fs::write(&source, C_CLI).expect("cannot write C CLI source");
    let build = Command::new("clang")
        .arg("-O2")
        .arg("-std=gnu99")
        .arg("-I")
        .arg(&vendor)
        .arg(&source)
        .arg(vendor.join("miniz.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("clang must be available");
    assert!(
        build.status.success(),
        "clang failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    for (name, data) in corpus() {
        let c_compressed = run_c(&binary, "c", &data);
        let rust_compressed = rust_compress(&data);
        let c_from_c = run_c(&binary, "d", &c_compressed);
        let rust_from_rust = rust_decompress(&rust_compressed);
        let rust_from_c = rust_decompress(&c_compressed);
        let c_from_rust = run_c(&binary, "d", &rust_compressed);
        assert_eq!(c_from_c, data, "{name}: C self-roundtrip; {EVIDENCE}");
        assert_eq!(
            rust_from_rust, data,
            "{name}: Rust self-roundtrip; {EVIDENCE}"
        );
        assert_eq!(
            rust_from_c, data,
            "{name}: C-compressed to Rust-decompressed; {EVIDENCE}"
        );
        assert_eq!(
            c_from_rust, data,
            "{name}: Rust-compressed to C-decompressed; {EVIDENCE}"
        );
    }
    let _ = fs::remove_dir_all(&work);
}
