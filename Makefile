.PHONY: help all clean test test-nextest doctest test-workflow-contracts test-markdown-format test-typos-config build release lint lint-clippy lint-whitaker lint-python doc-coverage doc-coverage-test fmt check-fmt typecheck typecheck-python markdownlint spelling spelling-config spelling-helper-test nixie install-kani kani-check kani-full kani-ir install-verus verus formal-pr install-dev-fast dev-fast-check dev-build dev-test bench-build bench-config-load

RUST_TOOLCHAIN_FILE ?= rust-toolchain.toml
# Export this path before shell probes expand it, so Make does not interpolate
# an override into the shell command line.
export RUST_TOOLCHAIN_FILE
# Threshold and toolchain for the Rustdoc doc-comment coverage gate. The
# threshold mirrors the 80% bar stated in AGENTS.md; the toolchain recollects
# the channel from rust-toolchain.toml the same way the dev-fast variables do,
# so overriding either stays independent.
DOC_COVERAGE_THRESHOLD ?= 80
DOC_COVERAGE_TOOLCHAIN ?= $(shell awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $$2; exit }' "$$RUST_TOOLCHAIN_FILE")
# Exported rather than interpolated into the recipe line. Passing the values
# through the environment means a toolchain or threshold containing a quote
# cannot inject commands into the shell command line; the child script reads
# them from its environment instead.
export DOC_COVERAGE_THRESHOLD DOC_COVERAGE_TOOLCHAIN

APP ?= netsuke
CARGO ?= $(shell command -v cargo 2>/dev/null || printf '%s' "$$HOME/.cargo/bin/cargo")
# CARGO is resolved above before it is exported: `export` alone would define
# the variable empty and shadow the `?=` fallback for every recipe.
export CARGO
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
# Opt-in local build acceleration. The Cargo fragment is deliberately kept out
# of an auto-discovered `.cargo/config.toml`, because Cranelift and mold must
# stay opt-in so release, packaging, coverage, and formal-verification paths
# keep the supported LLVM backend and platform linker. The toolchain is not
# pinned separately — dev-fast uses the repository's own nightly.
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
DEV_FAST_CONFIG ?= tools/dev-fast/config.toml
DEV_FAST_PREFIX ?= $(HOME)/.local
# Exported rather than interpolated into the recipes. Make hands an exported
# variable to the child process directly, so a path containing a quote cannot
# break the command line the shell parses; a `VAR='$(VAR)'` prefix could.
export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE
export DEV_FAST_CONFIG DEV_FAST_PREFIX
DEV_FAST_TOOLCHAIN = $$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $$2; exit }' "$$RUST_TOOLCHAIN_FILE")
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
NIXIE ?= nixie
# Single source of truth for the typos version; the markdownlint target and CI
# both consume it, so the Makefile and CI cannot drift apart.
TYPOS_VERSION ?= 1.48.0
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
# The Python baseline every uv-driven helper pins. Bump this alongside the
# `target-version`/`py-version` settings in pyproject.toml and the
# `python-version` inputs in .github/workflows/ci.yml, release.yml, and
# build-and-package.yml; the workflow contract tests hold the Makefile and CI
# in sync.
PYTHON_BASELINE ?= 3.14
# Pin Ruff so `make` invokes the same version everywhere; floating the version
# causes version-skew lint failures because rule sets differ between releases.
# CI pins the same value in .github/workflows/ci.yml; a contract test in
# tests/workflow_contracts keeps the two from drifting apart.
RUFF_VERSION ?= 0.16.4
RUFF = $(UV_ENV) $(UV) tool run --from ruff==$(RUFF_VERSION) ruff
# Pin ty so `make` and CI invoke the same typechecker release. ty is pre-1.0
# and diagnostics shift between releases, so an unpinned install breaks the
# typecheck gate without any code change. Bump deliberately and fix new
# diagnostics in the same commit.
TY_VERSION ?= 0.0.74
# Every Python source the repository owns. Ruff and Pylint resolve their own
# configuration and exclusions from pyproject.toml, so these paths only bound
# the walk.
PYTHON_SOURCES = scripts tests/workflow_contracts
# Pylint must run on the Python baseline so it parses every repository-owned
# source. `--load-plugins=` clears the shim's default plugin list so this pass
# runs exactly the messages pyproject.toml enables.
PYLINT_PYTHON ?= $(PYTHON_BASELINE)
PYLINT_TARGETS ?= $(PYTHON_SOURCES)
PYLINT_PYPY_SHIM_REF ?= 726d09f968b4d729ee4b29c71fc732e744854f3b
PYLINT_PYPY_SHIM = git+https://github.com/leynos/pylint-pypy-shim.git@$(PYLINT_PYPY_SHIM_REF)
PYLINT = $(UV_ENV) $(UV) tool run --python $(PYLINT_PYTHON) \
	--from '$(PYLINT_PYPY_SHIM)' pylint-pypy --load-plugins=
# The df12 house lints need CPython 3.14: they parse syntax PyPy's 3.11
# runtime cannot, and the baseline-gated messages (R9112, C9112) key off the
# `py-version` in pyproject.toml. They run through `uv tool run` rather than
# `uv run` so the repository never needs a project virtual environment for a
# Rust contributor's sake.
DF12_PYTHON_LINTS_REF ?= v0.3.0
DF12_PYTHON_LINTS = git+https://github.com/leynos/df12-python-lints.git@$(DF12_PYTHON_LINTS_REF)
DF12_PYLINT_MESSAGES = R9101,C9102,R9103,R9104,C9105,C9106,C9107,R9108,R9109,R9110,R9111,R9112,C9112
DF12_PYLINT = $(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
	--from '$(DF12_PYTHON_LINTS)' pylint \
	--disable=all --load-plugins=df12_python_lints \
	--enable=$(DF12_PYLINT_MESSAGES)
AMBRLEAKS = $(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
	--from '$(DF12_PYTHON_LINTS)' ambrleaks
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
	-not -path './.vtcode/*' -not -path './memories/*' \
	-not -path './.pytest_cache/*' \
	-not -path './.uv-cache/*' \
	-not -path './.uv-tools/*' \
	-not -path './node_modules/*' -print0
PROVER_TOOLS_SOURCE ?= git+https://github.com/leynos/rust-prover-tools@b07ef696f8373d54ae68e517d39d47a5d27a5bd5
PROVER_TOOLS ?= uv tool run --from $(PROVER_TOOLS_SOURCE) prover-tools
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
export RUSTDOC_FLAGS
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
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) nextest run --workspace --all-targets --all-features $(NEXTEST_BUILD_JOBS)

doctest: ## Run doctests, which cargo-nextest cannot execute
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) test --workspace --doc --all-features $(BUILD_JOBS)

test-workflow-contracts: ## Validate the mutation-testing caller contract
	$(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' pytest tests/workflow_contracts -q

test-markdown-format: ## Validate the Markdown formatter checker
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		--with pytest==9.0.2 --with hypothesis==6.151.9 \
		python -m pytest scripts/tests/test_check_markdown_format.py -c /dev/null \
		--rootdir=. -p no:cacheprovider

test-typos-config: spelling-helper-test ## Verify the shared spelling-policy integration

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: lint-clippy lint-whitaker lint-python ## Run the Rust and Python lint suites with warnings denied

lint-python: ## Run Ruff, Pylint, the df12 house lints, and ambrleaks over the Python sources
	$(RUFF) check $(PYTHON_SOURCES)
	$(PYLINT) $(PYLINT_TARGETS)
	$(DF12_PYLINT) $(PYLINT_TARGETS)
	$(AMBRLEAKS) $(PYTHON_SOURCES)

lint-clippy: ## Run rustdoc and Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) doc --workspace --no-deps
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	DYLINT_TOML="$$(cat dylint.toml)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(WHITAKER) --all --no-deps --package netsuke-build -- --all-targets --all-features
	# Run from the crate directory as well so Whitaker loads the narrow
	# `test_support::fs` exemption from test_support/dylint.toml.
	cd test_support && DYLINT_TOML="$$(cat dylint.toml)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(WHITAKER) --all --no-deps --package test_support -- --all-targets --all-features

doc-coverage: doc-coverage-test ## Verify aggregate Rustdoc doc-comment coverage meets the threshold
	# Runs under the uv-pinned baseline interpreter, not the system python3:
	# the scripts target Python 3.14 syntax and semantics.
	@RUSTDOCFLAGS="$${RUSTDOC_FLAGS}" $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		scripts/doc-coverage.py --toolchain "$$DOC_COVERAGE_TOOLCHAIN" --threshold "$$DOC_COVERAGE_THRESHOLD"

doc-coverage-test: ## Run documentation-coverage pytest modules
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		--with pytest==9.0.2 --with pytest-cov==7.0.0 --with 'hypothesis>=6' \
		python -m pytest scripts/tests/test_doc_coverage_model.py \
		scripts/tests/test_doc_coverage_cargo.py \
		scripts/tests/test_doc_coverage_cargo_payload.py \
		scripts/tests/test_doc_coverage_runner.py \
		scripts/tests/test_doc_coverage.py -c /dev/null --rootdir=. \
		-p no:cacheprovider --cov=doc_coverage_model --cov=doc_coverage_cargo \
		--cov=doc_coverage_runner --cov=doc_coverage_module

fmt: ## Format Rust, Python, and Markdown sources
	$(CARGO) fmt --all
	$(RUFF) format $(PYTHON_SOURCES)
	$(RUFF) check --select I --fix $(PYTHON_SOURCES)
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(RUFF) format --check $(PYTHON_SOURCES)
	@$(MD_FILES_FIND) | xargs -0 -r scripts/check-markdown-format.sh

typecheck: typecheck-python ## Typecheck all targets and features
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) check --all-targets --all-features $(BUILD_JOBS)

typecheck-python: ## Typecheck the Python sources with ty
	# `uv tool run` materialises one venv holding ty plus the test-suite
	# dependencies, so ty can resolve third-party imports. `uv run --with`
	# would layer the extras through `.pth` chaining, which ty cannot follow.
	$(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
		--from ty==$(TY_VERSION) --with pytest==9.0.2 --with pytest-cov==7.0.0 \
		--with 'pyyaml>=6' --with 'hypothesis>=6' \
		ty check --python-version $(PYTHON_BASELINE) \
		--extra-search-path scripts $(PYTHON_SOURCES)

markdownlint: spelling ## Lint Markdown and enforce en-GB-oxendict spelling
	$(MDLINT) "**/*.md"

spelling: spelling-config ## Enforce en-GB-oxendict spelling in Markdown prose
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) scripts/typos_rollout_check.py --repository .
	@$(MD_FILES_FIND) | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude

spelling-config: spelling-helper-test ## Generate and validate the spelling configuration
	@$(UV_ENV) $(UV) run --no-project scripts/generate_typos_config.py
	@git ls-files --error-unmatch typos.toml >/dev/null
	@git diff --exit-code -- typos.toml

spelling-helper-test: ## Validate the shared spelling-policy integration
	# `--isolated` keeps this gate independent of the repository configuration
	# so the spelling helpers stay self-contained on the shared estate policy.
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py314 --check $(SPELLING_HELPER_FILES)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py314 $(SPELLING_HELPER_FILES)
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) --with pytest==9.0.2 --with pytest-cov==7.0.0 python -m pytest scripts/tests/test_typos_rollout*.py -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_HELPER_COVERAGE) --cov-fail-under=90

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
	$(KANI) $(KANI_FLAGS)

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

bench-config-load: ## Benchmark cached configuration loading without layer copies
	$(CARGO) bench --bench config_load_cached_merge

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
