# C2RUST-TV

A translation-validation gate for C→Rust ports.

Compile both sides at matched optimisation levels, run the same corpus, and
compare the declared result classes (return, buffer, status); undeclared fields
and exact response frames are not compared. Undefined-behaviour signals are
reported as evidence, and CI fails with a counterexample.

Informed by:

> *C-to-Rust Translation Validation at the LLVM IR Level via AI-Assisted
> Refinement Proof*, arXiv:2611.18036, 13 Nov 2026.
> https://arxiv.org/abs/2611.18036

Independent work. Not affiliated with or endorsed by the authors.

## Status

**Research only.** No design approval, no code. Paper notes in `PAPER.md`.

## Score (portfolio rubric)

| I | N | E | Fit | C | Lang | Total |
|---|---|---|---|---|---|---|
| 9 | 9 | 4 | 22 | 10 | +2 | **24/33** |

**Credibility:** B+ — Kim, Ryu, Hong (KAIST), 26-page preprint, no artifact.

## What the paper actually shows

- Proves refinement between C and Rust **after both are compiled to LLVM IR**,
  because no shared formal semantics exists at source level.
- `R2C2` evaluates 519 C programs from a prior C→Rust translation benchmark.
- Proves 486 translations correct without bounding executions.
- Finds **three incorrect translations** with valid counterexamples that existing
  translation-validation tools miss.

## Honest positioning

The paper's contribution is proof. A solo product cannot reproduce R2C2's
refinement proofs in three weeks. The shippable slice is the *gate*: deterministic
differential validation at IR level, UB instrumentation, and a report a porting
team can act on. Claim "translation validation", never "verified refinement".

## MVP (≤3 weeks)

1. Input: a C file, its Rust port, and a harness function both expose.
2. Build both with `-emit-llvm` at `-O0` and `-O2`.
3. Differential runner: same corpus, compare return values, memory writes, exit
   codes; sanitize both sides (ASan/UBSan vs Miri where possible).
4. IR diff: flag functions whose instruction mix diverges beyond a threshold —
   a triage signal, not a proof.
5. `c2tv check` → JSON + human report, non-zero exit on divergence.
6. Fixture set: 20 known-good ports, 3 intentionally broken (bug classes chosen
   by us, since the paper does not name its three).

## Kill gate

If the harness cannot detect the three deliberately broken fixtures without
false-positiving the 20 good ones, stop — the signal is too weak to ship.

## Risks / non-goals

- Not a proof tool. Do not claim soundness.
- Fixtures are trusted code: their build scripts and harnesses execute with
  network access. `cargo --offline` only blocks dependency fetches.
- Rust UB and C UB differ; Miri cannot run FFI-heavy code.
- Benchmark suites may not be redistributable; ship generators instead.
- LLM-generated translations are the paper's setting; the tool should work on
  human ports too, but validate on both.

## Next step

Design doc with the fixture list first. The fixtures decide whether the product
is real.
