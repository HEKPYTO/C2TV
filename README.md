# C2TV

A deterministic validation gate for C→Rust ports. Compile both sides, run the
same corpus, compare observable behaviour, and fail CI with the first
counterexample.

Informed by *C-to-Rust Translation Validation at the LLVM IR Level via AI-Assisted
Refinement Proof* (arXiv:2611.18036). This project is independent work. It is not
affiliated with, endorsed by, or developed by the paper's research team. The
paper proves refinement. This tool does not. It is a gate: a heuristic that
catches divergence, not a verifier.

## Status

Working gate on 26 fixtures: 20 correct ports, 3 deliberately broken ports, and
3 real algorithms (AES-128, SHA-256, QOI). No soundness claim, no refinement
proof, no automatic repair.

Verified on macOS arm64 and Ubuntu 24.04 arm64 with the full gate, including
the five transpiled fixtures. Hosted CI runs the full gate on Ubuntu 24.04
x86_64, including lock, tamper, missing-lock, and transpile checks.

## Use

```sh
cargo build --offline
cargo run --offline -- self-test --fixtures fixtures
```

`self-test` builds every fixture with clang and cargo, runs each corpus case at
matched optimisation levels, compares the declared response classes, and exits
non-zero on any unexpected verdict. Undeclared fields are reported as not
compared. A full run takes under two minutes on an arm64 laptop.

Publish measured detection rates from the same pipeline:

```sh
cargo run --offline -- bench --fixtures fixtures
cargo run --offline -- bench --fixtures fixtures --json
```

`bench` runs the `self-test` pipeline once and reports counts plus false-positive
and false-negative rates with explicit numerators and denominators. Tool errors
get their own count, exit 2, and are excluded from both rates.

Measured on the shipped corpus:

| Metric | Count | Denominator |
|---|---:|---:|
| True positives | 3 | 3 broken ports |
| False negatives | 0 | 3 broken ports |
| False positives | 0 | 23 good + real ports |
| True negatives | 23 | 23 good + real ports |
| Tool errors | 0 | 26 fixtures |

These are corpus-specific rates, not a population estimate. `bench` exits 1 on
any false positive or false negative and 2 on any tool error.

Check one port:

```sh
cargo run --offline -- check --fixture fixtures/good/crc32 --json
```

Pin fixture inputs before releasing:

```sh
cargo run --offline -- lock --fixtures fixtures
cargo run --offline -- self-test --fixtures fixtures
```

Validate the transpiled artifact itself (requires `c2rust` 0.22.1):

```sh
cargo run --offline -- self-test --transpile --fixtures fixtures
```

The lane transpiles five fixtures — crc32, base64, AES-128, SHA-256, and QOI —
strips compiler-specific features, rebuilds with stable Rust at the matched
optimisation level, runs the same corpus, and compares the transpiled binary
with the C reference. It also compares the rebuilt binary hash across two
independent builds. A missing or mismatched transpiler is exit 2, never a skip.

`cargo test` skips the two transpile integration tests when `c2rust` is absent;
`self-test --transpile` fails closed instead.

Report toolchain drift without changing the work tree:

```sh
cargo run --offline -- doctor
cargo run --offline -- doctor --json
cargo run --offline -- doctor --strict --transpile --json
```

`doctor` reports the c2tv, rustc, cargo, clang, and c2rust versions and the
`rust-toolchain.toml` channel. It only runs `--version` probes and reads the
toolchain file. Missing rustc or clang is exit 2 with a typed diagnostic.
Channel drift is a warning and exits 0 by default; `--strict` makes it exit 2.
An absent c2rust is informational unless `--transpile` is set, which requires
exactly `transpile 0.22.1` and exits 2 otherwise. JSON has fixed field order and
carries no paths or times. CI runs the strict transpiler form before tests.

Exit codes: `0` all expected verdicts, `1` divergence or expectation failure,
`2` tool error. A missing compiler, malformed manifest, corrupt lock, or protocol
violation is never reported as a divergence.

## Truth table

| Class | Count | Meaning |
|---|---:|---|
| good | 20 | correct ports; must show no divergence |
| bad | 3 | overflow, aliasing, off-by-one; must diverge once, for the declared reason |
| real | 3 | AES-128, SHA-256, QOI; crate tests plus differential frames |

The paper does not name its three incorrect translations. Ours are declared in
[`fixtures/README.md`](fixtures/README.md), with the harness ABI, corpus format,
diagnostic codes, and comparison rules.

## External validation

`external/miniz` checks miniz 3.1.0 (vendored, MIT, hashes pinned) against
`miniz_oxide` 0.9.1, its production Rust port. Six deterministic inputs exercise
24 assertions: C self-roundtrip, Rust self-roundtrip, C-compressed to
Rust-decompressed, and Rust-compressed to C-decompressed. Run it with:

```sh
cargo test --offline --test external_miniz
```

This is behavioural evidence on an independent production implementation, not a
proof and not an endorsement.

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

## What it catches

- Behavioural divergence between a C function and its Rust port on a declared
  corpus, at `-O0` and `-O2`.
- Signed-integer overflow in C, through a UBSan lane attached as evidence.
- Nondeterministic reports, unpinned fixture inputs, and malformed fixtures.

## Limitations and non-soundness contract

A pass means no observed divergence on the declared corpus and comparison
classes at the tested optimisation levels. It says nothing about unobserved
inputs, and it is not a proof of equivalence.

- Refinement or soundness. Passing the gate is not proof of equivalence.
- Whole-program equivalence, C++, or FFI-heavy crates.
- Bugs outside the declared corpus or comparison classes.
- Differences that both implementations share.

## Offline and pinned

Every check path runs with `--offline`; no network fetch. Fixtures are trusted
code and may still open sockets themselves. The report records
compiler versions, `c2rust` availability, and sha256 digests of fixture inputs.
The lock file makes input drift fail closed.

## Releases

Tags matching `v*` build `--locked` binaries on Ubuntu 24.04 x86_64 and macOS 15
arm64. Archives use normalized metadata and `gzip -n`, and each release publishes
the binary, `LICENSE`, `README.md`, and a sha256 file. Inputs and archives are
locked; binaries are not guaranteed bit-identical across hosts, so compare the
published checksums.

## Community

Apache-2.0. Contributions through GitHub issues and pull requests. Report
vulnerabilities through GitHub security advisories. Every release claim stays
within the measured evidence above.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
