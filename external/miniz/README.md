# Vendored miniz C release

- Upstream: https://github.com/richgel999/miniz
- Release tag: `3.1.0`
- Tag commit: `174573d60290f447c13a2b1b3405de2b96e27d6c`
- Source archive: https://github.com/richgel999/miniz/releases/download/3.1.0/miniz-3.1.0.zip
- Archive sha256: `f9d3e769ec5fd5583955a3c22eea457fe806be0d507805966d1a5b0410c77dda`

The files below come from the self-contained amalgamated release archive.

| File | sha256 |
|---|---|
| `miniz.c` | `7487d4c8cd761b951a99d182672bb3badfa72a5ad5760b70b392fafa95223657` |
| `miniz.h` | `13a6940a8f33b3a76d4b64fb6d482b246954bae3bf5dc713f7f054d2096d0aba` |
| `LICENSE` | `0115478d567121238cf6cc1c0c361926cf07a49d9e4c9e66da97fac6a01646b3` |

The upstream MIT licence is preserved in `LICENSE`. The C2TV project licence remains Apache-2.0.

`tests/external_miniz.rs` compiles a small CLI from the vendored `miniz.c` with clang at runtime. It runs C self-roundtrip, Rust self-roundtrip, C-compressed to Rust-decompressed, and Rust-compressed to C-decompressed checks over a fixed corpus, including empty input. It asserts the three file hashes above so drift fails closed, and it does not compare encoder output bytes.

miniz and miniz_oxide are independent production implementations. This project is not affiliated with, endorsed by, or developed by either upstream. Passing this test is behavioural evidence on the declared corpus, not proof of equivalence, refinement, or soundness.
