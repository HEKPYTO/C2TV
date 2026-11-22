# Fixtures — truth table

The release gate. Twenty correct ports and three deliberately broken ones.
Every fixture is a Rust crate with its own tests, a C implementation, a corpus,
and a declared verdict. `c2tv self-test` is the only release gate.

The paper does not name its three incorrect translations. Ours are chosen here
and declared, so nobody attributes them to the authors.

## Harness ABI v1

Each side is a native binary that reads one request from stdin and writes one
response to stdout. Little-endian throughout. No paths, clocks, locale, or
environment in the output.

Request: `u32 len` then `len` bytes.
A request longer than 4096 bytes is a tool error: both harnesses exit non-zero
without writing a response. The runner enforces the same limit.

Response: `u64 status` (`0` ok, `1` domain error), `u64 ret`, `u32 out_len`,
then `out_len` bytes.

The runner starts one process per case, writes one request, closes stdin, and
reads the response. A short read, trailing byte, or non-zero exit is a tool
error, never a divergence.

## Manifest (`port.toml`)

| Key | Values | Meaning |
|---|---|---|
| `schema` | `1` | format version |
| `id` | string | fixture id, equals directory name |
| `kind` | `good` / `bad` | truth-table class |
| `classes` | `ret`, `buf`, `err` | compared fields; anything else is reported `not compared` |
| `expected` | `pass` / `diverge` | verdict the gate must produce |
| `bug` | `overflow` / `aliasing` / `off-by-one` | required for `bad` only |
| `note` | string | one line, why the fixture exists |

Corpus: `corpus.hex`, one case per line, hex-encoded request bytes, `-` for the
empty input. Sorted by case index.

## Truth table

### Good — must show no divergence and pass their crate tests

| # | id | function | classes | why it is here |
|---|---|---|---|---|
| 1 | `varint` | LEB128 u64 roundtrip | ret, buf | loop and continuation-bit edges |
| 2 | `crc32` | CRC-32/IEEE | ret | table-free bit mixing |
| 3 | `base64` | RFC 4648 encode/decode | buf, err | padding and invalid alphabet |
| 4 | `utf8_count` | scalar count | ret, err | overlong and truncated sequences |
| 5 | `ringbuf` | push/pop with wrap | buf, err | state across the wrap index |
| 6 | `fixed_q16` | Q16.16 multiply with round-half-up | ret | rounding mode |
| 7 | `adler32` | Adler-32 | ret | modulo arithmetic |
| 8 | `fletcher16` | Fletcher-16 | ret | two running sums |
| 9 | `rle` | run-length encode with capacity | buf, err | worst-case output bound |
| 10 | `bitrev32` | reverse 32 bits | ret | shift and mask |
| 11 | `popcount` | byte popcount and parity | ret, buf | loop accumulation |
| 12 | `heapify` | binary min-heap sift-down | buf | index arithmetic |
| 13 | `merge_sorted` | merge two sorted arrays | buf, err | empty and equal-key edges |
| 14 | `atoi_sat` | decimal parse with saturation | ret, err | overflow policy |
| 15 | `hexcodec` | hex encode/decode | buf, err | case handling and odd length |
| 16 | `murmur3` | MurmurHash3 x86 32-bit | ret | wrapping multiply and tail |
| 17 | `slide_max` | sliding-window maximum | buf, err | monotonic deque indices |
| 18 | `struct_copy` | `repr(C)` record copy | buf | padding and field order |
| 19 | `memmove_overlap` | overlapping byte copy | buf | overlap direction |
| 20 | `binary_search` | lower-bound search | ret, err | duplicates and absent keys |

### Bad — must diverge on exactly one declared reason

| # | id | bug | broken behaviour | first differing case |
|---|---|---|---|---|
| 21 | `bad_overflow` | `overflow` | C accumulates into `int` and overflows; Rust port saturates. UBSan reports signed overflow on C. | `INT_MAX + 1` input |
| 22 | `bad_aliasing` | `aliasing` | C copies forward with overlapping regions (undefined `memcpy` use); Rust port uses an overlap-safe `copy_within`. | source below destination, ranges overlap |
| 23 | `bad_off-by-one` | `off-by-one` | C writes `len - 1` bytes and drops the final byte; Rust writes `len`. | any non-empty output |

Each bad fixture must pass its crate tests on the Rust side (the Rust code is
internally consistent) and differ from C only on the declared case. A bad
fixture that differs on a second case is a broken fixture, not a finding.

## Comparison rule

1. Build C at `-O0` and `-O2`; build Rust at `opt-level=0` and `opt-level=2`.
2. Run the same corpus at matched levels only.
3. Mask fields not declared in `classes`; report them as `not compared`.
4. A mismatch is `TV0008 divergence`, with fixture, case index, level, and the
   first differing byte.
5. A sanitizer event is `TV0009 ub`, attached to its case; it is evidence, not
   a proof.
6. Determinism: canonical report bytes are identical across two consecutive
   runs. `self-test` fails otherwise.

## Kill gate

If the three bad fixtures cannot be separated from the twenty good ones without
false positives, stop. The gate idea is wrong.

## Not claimed

Refinement proofs, soundness, whole-program equivalence, C++, FFI-heavy crates,
or that the authors' three bugs are these three.
