# C2RUST-TV

A deterministic validation gate for C→Rust ports. Compile both sides, run the
same corpus, compare observable behaviour, and fail CI with the first
counterexample.

Informed by *C-to-Rust Translation Validation at the LLVM IR Level via AI-Assisted
Refinement Proof* (arXiv:2611.18036). Independent work; not affiliated with the
authors. The paper proves refinement. This tool does not. It is a gate: a
heuristic that catches divergence, not a verifier.

## Status

Working gate on 26 fixtures: 20 correct ports, 3 deliberately broken ports, and
3 real algorithms (AES-128, SHA-256, QOI). No soundness claim, no refinement
proof, no automatic repair.

## Use

```sh
cargo build --offline
cargo run --offline -- self-test --fixtures fixtures
```

`self-test` builds every fixture with clang and cargo, runs each corpus case at
matched optimisation levels, compares the declared response classes, and exits
non-zero on any unexpected verdict. Undeclared fields are reported as not
compared. A full run takes under two minutes on an arm64 laptop.

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

## What it catches

- Behavioural divergence between a C function and its Rust port on a declared
  corpus, at `-O0` and `-O2`.
- Signed-integer overflow in C, through a UBSan lane attached as evidence.
- Nondeterministic reports, unpinned fixture inputs, and malformed fixtures.

## What it does not catch

- Refinement or soundness. Passing the gate is not proof of equivalence.
- Whole-program equivalence, C++, or FFI-heavy crates.
- Bugs outside the declared corpus or comparison classes.
- Differences that both implementations share.

## Offline and pinned

Every check path runs with `--offline`; no network fetch. Fixtures are trusted
code and may still open sockets themselves. The report records
compiler versions, `c2rust` availability, and sha256 digests of fixture inputs.
The lock file makes input drift fail closed.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
