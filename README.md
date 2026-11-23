# C2TV

[![CI](https://github.com/HEKPYTO/C2TV/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/HEKPYTO/C2TV/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/HEKPYTO/C2TV)](https://github.com/HEKPYTO/C2TV/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

C2TV is a deterministic pre-merge gate for C→Rust ports: it compiles the C
source and the Rust port, runs the same corpus at `-O0` and `-O2`, compares the
declared response classes, and fails CI with the first counterexample. It is a
heuristic gate, not a verifier; a pass means no observed divergence on the
declared corpus, not equivalence. This is independent work, not affiliated with
the authors of arXiv:2611.18036; that paper proves refinement, C2TV does not.

## Status

26 fixtures: 20 correct ports, 3 deliberately broken ports, and 3 real
algorithms (AES-128, SHA-256, QOI). The full gate is green on hosted Ubuntu
24.04 x86_64, including lock, tamper, missing-lock, transpile, and adoption
checks. Local verification covers macOS arm64 and Ubuntu 24.04 arm64.

## Quick start

Download the Linux x86_64 or macOS arm64 archive from
[Releases](https://github.com/HEKPYTO/C2TV/releases), verify its `.sha256` file,
then extract and run `./c2tv --version`. The archive contains only the binary,
`LICENSE`, and `README.md`. From a checkout, build and run the full corpus:

```sh
cargo build --release --locked
./target/release/c2tv self-test --fixtures fixtures
```

`self-test` builds every fixture with clang and cargo, runs each corpus case at
matched optimisation levels, and exits non-zero on any unexpected verdict. A
full run takes under two minutes on an arm64 laptop.

Check one port and inspect the JSON report:

```sh
./target/release/c2tv check --fixture fixtures/good/crc32 --json
```

Exit codes: `0` all expected verdicts, `1` divergence or expectation failure,
`2` tool error. A missing compiler, malformed manifest, corrupt lock, or
protocol violation is never reported as divergence.

## What it catches

- Behavioural divergence between a C function and its Rust port on a declared
  corpus, at `-O0` and `-O2`.
- Signed-integer overflow in C, through a UBSan lane attached as evidence.
- Nondeterministic reports, unpinned fixture inputs, and malformed fixtures.

Fields not declared for comparison are reported as not compared.

## Measured detection rates

```sh
./target/release/c2tv bench --fixtures fixtures --json
```

`bench` runs the self-test pipeline once and reports counts plus false-positive
and false-negative rates with explicit numerators and denominators. Tool errors
get their own count and are excluded from both rates; `bench` exits `1` on any
false positive or false negative and `2` on any tool error.

| Metric | Count | Denominator |
|---|---:|---:|
| True positives | 3 | 3 broken ports |
| False negatives | 0 | 3 broken ports |
| False positives | 0 | 23 good + real ports |
| True negatives | 23 | 23 good + real ports |
| Tool errors | 0 | 26 fixtures |

These are corpus-specific rates, not a population estimate.

## Adoption

GitHub Actions:

```yaml
- uses: HEKPYTO/C2TV@v0.1.0
  with:
    fixture: path/to/fixture
```

Pre-commit: add the `c2tv-check` hook from `.pre-commit-hooks.yaml`.
Make and CMake: see [`integrations/README.md`](integrations/README.md).
A complete caller fixture and released-tag workflow live in
[`examples/adoption/`](examples/adoption/README.md).

Every adopter authors `port.toml`, `corpus.hex`, and matching C/Rust harnesses,
then commits `digests.toml`. C2TV is not a generic binary diff.

## External validation

`external/miniz` checks miniz 3.1.0 (vendored, MIT, hashes pinned) against
`miniz_oxide` 0.9.1, its production Rust port. Six deterministic inputs exercise
24 assertions across C and Rust roundtrips and cross-language roundtrips.

```sh
cargo test --offline --test external_miniz
```

This is behavioural evidence on an independent production implementation, not a
proof and not an endorsement.

## Transpiled lane

```sh
./target/release/c2tv self-test --transpile --fixtures fixtures
```

The lane transpiles five fixtures — crc32, base64, AES-128, SHA-256, and QOI —
strips compiler-specific features, rebuilds with stable Rust at the matched
optimisation level, runs the same corpus, and compares the transpiled binary
with the C reference. It also compares the rebuilt binary hash across two
independent builds. A missing or mismatched transpiler is exit `2`, never a
skip. Requires `c2rust` 0.22.1.

## Development

```sh
cargo fmt --check
cargo clippy --offline -- -D warnings
cargo test --offline
cargo run --offline -- self-test --fixtures fixtures
cargo run --offline -- doctor --strict --transpile --json
```

`cargo test` skips the two transpile integration tests when `c2rust` is absent;
`self-test --transpile` and CI fail closed instead.

| Path | Purpose |
|---|---|
| `src/` | CLI, fixture loading, build and execution, reporting |
| `fixtures/` | 26 fixture corpus and the harness ABI in [`fixtures/README.md`](fixtures/README.md) |
| `external/miniz/` | Vendored miniz used by the external validation test |
| `integrations/` | GitHub Action, pre-commit hook, Make and CMake targets |
| `examples/adoption/` | Stand-alone adopter fixture and released-tag workflow |

## Limitations and non-soundness contract

A pass means no observed divergence on the declared corpus and comparison
classes at the tested optimisation levels. It says nothing about unobserved
inputs and is not a proof of equivalence.

- Refinement or soundness: passing the gate is not proof of equivalence.
- Whole-program equivalence, C++, or FFI-heavy crates.
- Bugs outside the declared corpus or comparison classes.
- Differences that both implementations share.

## Offline and reproducible

Every check path runs with `--offline`; no network fetch. Fixtures are trusted
code and may open sockets themselves. Reports record compiler versions,
`c2rust` availability, and sha256 digests of fixture inputs. The lock file makes
input drift fail closed.

Release tags matching `v*` build `--locked` binaries on Ubuntu 24.04 x86_64 and
macOS 15 arm64. Archives use normalized metadata and `gzip -n`, and publish the
binary, `LICENSE`, `README.md`, and a sha256 file. Binaries are not guaranteed
bit-identical across hosts; compare the published checksums.

## Community

Apache-2.0. Contributions through GitHub issues and pull requests. Report
vulnerabilities through GitHub security advisories. Every release claim stays
within the measured evidence above.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
