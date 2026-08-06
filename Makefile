.PHONY: help all clean test test-nextest doctest test-workflow-contracts test-typos-config build release lint lint-clippy lint-whitaker fmt check-fmt typecheck markdownlint spelling spelling-config spelling-helper-test nixie install-kani kani-check kani-full kani-ir install-verus verus formal-pr install-dev-fast dev-fast-check dev-build dev-test bench-build

APP ?= netsuke
CARGO ?= $(shell command -v cargo 2>/dev/null || printf '%s' "$$HOME/.cargo/bin/cargo")
# The Polonius borrow-checker flag normally flows from .cargo/config.toml, but
# any recipe that sets RUSTFLAGS overrides that table and must re-state it.
POLONIUS_FLAGS ?= -Zpolonius=next
# Extra build-parallelism flags for plain Cargo invocations, e.g. `-j 4`.
BUILD_JOBS ?=
# The same concept for cargo-nextest, which spells build parallelism
# `--build-jobs N` and reserves `-j` for test concurrency. Keep the two
# variables separate so a `-j` value is never reinterpreted as a test-thread
# count.
NEXTEST_BUILD_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
KANI ?= cargo kani
KANI_FLAGS ?=
KANI_INSTALL_FLAGS ?=
KANI_CHECK_FLAGS ?=
KANI_VERSION_FILE ?= tools/kani/VERSION
# Opt-in local build acceleration. The Cargo fragment is deliberately separate
# from `.cargo/config.toml`: that file is auto-discovered and carries the
# repository-wide Polonius flag, whereas Cranelift and mold must stay opt-in so
# release, packaging, coverage, and formal-verification paths keep the
# supported LLVM backend and platform linker. The toolchain is not pinned
# separately — dev-fast uses the repository's own nightly.
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
RUST_TOOLCHAIN_FILE ?= rust-toolchain.toml
DEV_FAST_CONFIG ?= tools/dev-fast/config.toml
DEV_FAST_PREFIX ?= $(HOME)/.local
# Exported rather than interpolated into the recipes. Make hands an exported
# variable to the child process directly, so a path containing a quote cannot
# break the command line the shell parses; a `VAR='$(VAR)'` prefix could.
export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE RUST_TOOLCHAIN_FILE
export DEV_FAST_CONFIG DEV_FAST_PREFIX
DEV_FAST_TOOLCHAIN = $$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $$2; exit }' '$(RUST_TOOLCHAIN_FILE)')
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
NIXIE ?= nixie
# Single source of truth for the typos version; the markdownlint target and CI
# both consume it, so the Makefile and CI cannot drift apart.
TYPOS_VERSION ?= 1.48.0
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
RUFF_VERSION ?= 0.15.12
SPELLING_HELPER_COVERAGE = --cov=generate_typos_config --cov=typos_rollout_check --cov=typos_rollout \
	--cov=typos_rollout_cache --cov=typos_rollout_http
SPELLING_HELPER_FILES = scripts/generate_typos_config.py \
	scripts/typos_rollout_check.py \
	scripts/typos_rollout.py scripts/typos_rollout_cache.py \
	scripts/typos_rollout_http.py scripts/tests/conftest.py \
	scripts/tests/test_typos_rollout.py \
	scripts/tests/test_typos_rollout_check.py \
	scripts/tests/test_typos_rollout_hardening.py \
	scripts/tests/test_typos_rollout_refresh.py \
	scripts/tests/typos_rollout_test_support.py
# Markdown files, excluding build output and tool caches. CRUSH.md is a symlink
# to AGENTS.md, so `-type f` skips it and avoids double-checking the same prose.
MD_FILES_FIND = find . -type f -name '*.md' \
	-not -path './target/*' -not -path './.venv/*' \
	-not -path './.uv-cache/*' -not -path './.uv-tools/*' \
	-not -path './node_modules/*' -print0
PROVER_TOOLS_SOURCE ?= git+https://github.com/leynos/rust-prover-tools@b07ef696f8373d54ae68e517d39d47a5d27a5bd5
PROVER_TOOLS ?= uv tool run --from $(PROVER_TOOLS_SOURCE) prover-tools
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
VERUS_FLAGS ?=
VERUS_INSTALL_FLAGS ?=
WHITAKER ?= whitaker

export PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin:$(PATH)

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: test-nextest doctest ## Run every Rust test with warnings treated as errors

test-nextest: ## Run all non-doctest Rust tests through cargo-nextest
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(CARGO) nextest run --workspace --all-targets --all-features $(NEXTEST_BUILD_JOBS)

doctest: ## Run doctests, which cargo-nextest cannot execute
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(CARGO) test --workspace --doc --all-features $(BUILD_JOBS)

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' pytest tests/workflow_contracts -q

test-typos-config: spelling-helper-test ## Verify the shared spelling-policy integration

target/%/$(APP): ## Build binary in debug or release mode
	RUSTFLAGS="$${RUSTFLAGS-} $(POLONIUS_FLAGS)" $(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: lint-clippy lint-whitaker ## Run Clippy and the Whitaker Dylint suite with warnings denied

lint-clippy: ## Run rustdoc and Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(CARGO) doc --workspace --no-deps
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(WHITAKER) --all --no-deps --package netsuke-build -- --all-targets --all-features
	# Run from the crate directory as well so Whitaker loads the narrow
	# `test_support::fs` exemption from test_support/dylint.toml.
	cd test_support && DYLINT_TOML="$$(cat dylint.toml)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(WHITAKER) --all --no-deps --package test_support -- --all-targets --all-features

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

typecheck: ## Typecheck all targets and features
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)" $(CARGO) check --all-targets --all-features $(BUILD_JOBS)

markdownlint: spelling ## Lint Markdown and enforce en-GB-oxendict spelling
	$(MDLINT) "**/*.md"

spelling: spelling-config ## Enforce en-GB-oxendict spelling in Markdown prose
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.13 scripts/typos_rollout_check.py --repository .
	@$(MD_FILES_FIND) | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude

spelling-config: spelling-helper-test ## Generate and validate the spelling configuration
	@$(UV_ENV) $(UV) run scripts/generate_typos_config.py
	@git ls-files --error-unmatch typos.toml >/dev/null
	@git diff --exit-code -- typos.toml

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_HELPER_FILES)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_HELPER_FILES)
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.13 --with pytest==9.0.2 --with pytest-cov==7.0.0 python -m pytest scripts/tests/test_typos_rollout*.py -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_HELPER_COVERAGE) --cov-fail-under=90

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

install-kani: ## Install the pinned Kani verifier
	@printf 'prover-tools: source=%s\n' '$(PROVER_TOOLS_SOURCE)' >&2
	@printf 'prover-tools: target=install-kani kani-version=%s\n' "$$(cat '$(KANI_VERSION_FILE)')" >&2
	@printf 'prover-tools: command=%s\n' '$(PROVER_TOOLS) kani install <redacted-flags>' >&2
	@$(PROVER_TOOLS) kani install $(KANI_INSTALL_FLAGS) || { status=$$?; printf 'prover-tools: target=install-kani failed exit=%s\n' "$$status" >&2; exit "$$status"; }

kani-check: ## Check the installed Kani verifier version
	@printf 'prover-tools: source=%s\n' '$(PROVER_TOOLS_SOURCE)' >&2
	@printf 'prover-tools: target=kani-check kani-command=%s kani-version=%s\n' '$(KANI)' "$$(cat '$(KANI_VERSION_FILE)')" >&2
	@printf 'prover-tools: command=%s\n' '$(PROVER_TOOLS) kani check-version --kani-command <redacted-command> <redacted-flags>' >&2
	@$(PROVER_TOOLS) kani check-version --kani-command "$(KANI)" $(KANI_CHECK_FLAGS) || { status=$$?; printf 'prover-tools: target=kani-check failed exit=%s\n' "$$status" >&2; exit "$$status"; }

kani-full: ## Run the full Kani verification suite
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(POLONIUS_FLAGS)" $(KANI) $(KANI_FLAGS)

kani-ir: kani-full ## Run the IR Kani verification suite

install-verus: ## Install the pinned Verus verifier
	@printf 'prover-tools: source=%s\n' '$(PROVER_TOOLS_SOURCE)' >&2
	@printf 'prover-tools: target=install-verus\n' >&2
	@printf 'prover-tools: command=%s\n' '$(PROVER_TOOLS) verus install <redacted-flags>' >&2
	@$(PROVER_TOOLS) verus install $(VERUS_INSTALL_FLAGS) || { status=$$?; printf 'prover-tools: target=install-verus failed exit=%s\n' "$$status" >&2; exit "$$status"; }

verus: ## Run the Verus proof entry point
	@printf 'prover-tools: source=%s\n' '$(PROVER_TOOLS_SOURCE)' >&2
	@printf 'prover-tools: target=verus\n' >&2
	@printf 'prover-tools: command=%s\n' '$(PROVER_TOOLS) verus run <redacted-flags>' >&2
	@$(PROVER_TOOLS) verus run $(VERUS_FLAGS) || { status=$$?; printf 'prover-tools: target=verus failed exit=%s\n' "$$status" >&2; exit "$$status"; }

formal-pr: ## Run pull-request formal-verification checks
	$(MAKE) kani-check

install-dev-fast: ## Install the pinned mold linker and Cranelift backend
	@scripts/install-dev-fast.sh

dev-fast-check: ## Check the mold and Cranelift local build prerequisites
	@scripts/dev-fast-check.sh

# Every dev-fast target needs the install prefix ahead of a distribution mold:
# the check probes PATH for it, and `-fuse-ld=mold` resolves by PATH order.
# Target-specific exports do not reach prerequisites, so `dev-fast-check` is
# listed in its own right as well as being a prerequisite of the others.
DEV_FAST_TARGETS = install-dev-fast dev-fast-check dev-build dev-test bench-build
$(DEV_FAST_TARGETS): export PATH := $(DEV_FAST_PREFIX)/bin:$(PATH)

dev-build: dev-fast-check ## Build the debug binary with Cranelift and mold
	RUSTUP_TOOLCHAIN=$(DEV_FAST_TOOLCHAIN) $(CARGO) --config "$$DEV_FAST_CONFIG" build $(BUILD_JOBS) --bin $(APP)

dev-test: dev-fast-check ## Run the nextest pass with Cranelift and mold
	RUSTUP_TOOLCHAIN=$(DEV_FAST_TOOLCHAIN) $(CARGO) --config "$$DEV_FAST_CONFIG" nextest run --workspace --all-targets --all-features $(NEXTEST_BUILD_JOBS)

bench-build: dev-fast-check ## Time clean and incremental debug builds for both paths
	@CARGO="$(CARGO)" scripts/bench-build.sh

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
