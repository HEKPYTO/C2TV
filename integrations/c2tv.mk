C2TV ?= c2tv
C2TV_ARGS ?= --json

.PHONY: c2tv-check
c2tv-check:
	@if [ -z "$(C2TV_FIXTURE)" ]; then echo "c2tv-check: set C2TV_FIXTURE to a fixture directory containing port.toml" >&2; exit 2; fi
	@command -v "$(C2TV)" >/dev/null 2>&1 || { echo "c2tv-check: C2TV='$(C2TV)' not found; set C2TV to the c2tv executable" >&2; exit 2; }
	"$(C2TV)" check --fixture "$(C2TV_FIXTURE)" $(C2TV_ARGS)
