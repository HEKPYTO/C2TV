# Adopting C2TV

Adopters author `port.toml`, a corpus, and C/Rust harnesses. This is not a generic binary diff: C2TV compares declared observable classes from the two harnesses at matched optimisation levels.

Every check needs `digests.toml`. Generate it from the fixtures root and commit it:

```sh
c2tv lock --fixtures fixtures
git add fixtures/digests.toml
```

`check` accepts a lock in the fixture directory or any ancestor. A missing or drifted lock is a tool error, not a divergence.

## GitHub Actions

The action does not check out or change your repository. Check out your fixtures first.

```yaml
- uses: actions/checkout@v4
- uses: HEKPYTO/C2TV@main
  with:
    fixture: fixtures/good/crc32
    args: --json
    version: "1.100.0"
```

The action builds c2tv from this repository with the pinned `Cargo.lock` and runs `c2tv check` from the caller workspace. `fixture` is required; `args` and `version` default to `--json` and `1.100.0`. Pin the action reference to a commit SHA for production use.

## pre-commit

Install c2tv on PATH, then declare the hook with the fixture as its first arg.

```yaml
repos:
  - repo: https://github.com/HEKPYTO/C2TV
    rev: main
    hooks:
      - id: c2tv-check
        args: [fixtures/good/crc32, --json]
```

The manifest entry is `c2tv check --fixture`, so required args are `[<fixture-directory>, --json]`. Filenames are not appended.

## Make

Vendor `c2tv.mk` and point the variables at your binary, fixture, and flags.

```make
C2TV := /path/to/c2tv
C2TV_FIXTURE := fixtures/good/crc32
C2TV_ARGS := --json
include path/to/C2TV/integrations/c2tv.mk
```

Run `make c2tv-check`. `C2TV` defaults to `c2tv` on PATH and `C2TV_ARGS` to `--json`; an unset `C2TV_FIXTURE` fails with a message.

## CMake

Vendor `C2TV.cmake`, define a target, then build that target.

```cmake
include(path/to/C2TV/integrations/C2TV.cmake)
c2tv_add_check(c2tv-check "${CMAKE_SOURCE_DIR}/fixtures/good/crc32")
```

```sh
cmake -S . -B build -DC2TV_EXECUTABLE=/path/to/c2tv
cmake --build build --target c2tv-check
```

`c2tv_add_check(TARGET FIXTURE)` creates a custom target that runs `c2tv check --json` with quoted paths from the source root. Without `-DC2TV_EXECUTABLE`, the function searches PATH.
