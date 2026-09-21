.PHONY: help all clean test test-nextest doctest test-workflow-contracts test-windows-msi-release-rank test-release-admission test-coverage-artifact build release lint lint-clippy lint-whitaker lint-python lint-workflow-scripts github-actions-lint doc-coverage doc-coverage-test validate-coverage-artifact fmt check-fmt typecheck typecheck-python markdownlint spelling nixie install-kani kani-check kani-full kani-ir install-verus verus formal-pr install-build-tools check-build-tools bench-build bench-config-load bench-glob-expansion

RUST_TOOLCHAIN_FILE ?= rust-toolchain.toml
# Export this path before shell probes expand it, so Make does not interpolate
# an override into the shell command line.
export RUST_TOOLCHAIN_FILE
# Threshold and toolchain for the Rustdoc doc-comment coverage gate. The
# threshold mirrors the 80% bar stated in AGENTS.md; the toolchain recollects
# the channel from rust-toolchain.toml the same way the build-tools variables do,
# so overriding either stays independent.
DOC_COVERAGE_THRESHOLD ?= 80
DOC_COVERAGE_TOOLCHAIN ?= $(shell awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $$2; exit }' "$$RUST_TOOLCHAIN_FILE")
# Exported rather than interpolated into the recipe line. Passing the values
# through the environment means a toolchain or threshold containing a quote
# cannot inject commands into the shell command line; the child script reads
# them from its environment instead.
export DOC_COVERAGE_THRESHOLD DOC_COVERAGE_TOOLCHAIN

APP ?= netsuke
# A bare command name, resolved by the recipe shell. That shell is the only one
# that receives the curated PATH below, which is where `$HOME/.cargo/bin` is
# added; a parse-time `command -v` probe would query the environment Make was
# started with instead, and what that environment exports to a shell function
# is neither documented nor stable across Make releases.
CARGO ?= cargo
# The default must be defined before the export: `export CARGO` on its own
# would define the variable empty and shadow the `?=` default for every recipe.
export CARGO
# Pass `--locked` through only when a caller asks for lockfile verification.
# Keeping the default empty preserves the interactive inner-loop behaviour.
CARGO_LOCKED ?=
# Extra build-parallelism flags for plain Cargo invocations, e.g. `-j 4`.
BUILD_JOBS ?=
# The same concept for cargo-nextest, which spells build parallelism
# `--build-jobs N` and reserves `-j` for test concurrency. Keep the two
# variables separate so a `-j` value is never reinterpreted as a test-thread
# count.
NEXTEST_BUILD_JOBS ?=
# Explicit nextest process concurrency. Keep this distinct from compiler build
# parallelism: `cargo-nextest` uses `-j` for test processes, not Cargo jobs.
NEXTEST_TEST_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
KANI ?= cargo kani
KANI_FLAGS ?=
KANI_INSTALL_FLAGS ?=
KANI_CHECK_FLAGS ?=
KANI_VERSION_FILE ?= tools/kani/VERSION
# The development build standard: the `mold` linker and the parallel `rustc`
# frontend are the defaults for development, test, lint, and typecheck builds.
# `.cargo/config.toml` carries them so a bare `cargo` invocation gets them too;
# release and coverage builds are excluded there and below. The toolchain is not
# pinned separately — the standard uses the repository's own nightly from
# `rust-toolchain.toml`.
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
BUILD_TOOLS_PREFIX ?= $(HOME)/.local
# Exported rather than interpolated into the recipes. Make hands an exported
# variable to the child process directly, so a path containing a quote cannot
# break the command line the shell parses; a `VAR='$(VAR)'` prefix could.
export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE
export BUILD_TOOLS_PREFIX

# Cargo picks a single `rustflags` source rather than merging them, and an
# externally set `RUSTFLAGS` outranks every `rustflags` table in
# `.cargo/config.toml`. Every gate target below sets `RUSTFLAGS` to deny
# warnings, and CI's `setup-rust` exports the same value for a whole job, so
# without restating the flags here the gates would silently fall back to the
# platform linker and a single-threaded frontend while still reporting success.
# tests/makefile_test_target/rustflags.rs holds these variables equal to the
# configuration file; changing one without the other fails that test.
STANDARD_THREADS_FLAG ?= -Zthreads=8
STANDARD_MOLD_FLAG ?= -Clink-arg=-fuse-ld=mold
# `mold` ships for Linux only. macOS and Windows keep their platform linker,
# matching the `cfg(target_os = "linux")` gate in `.cargo/config.toml`.
BUILD_HOST_OS := $(shell uname -s)
STANDARD_RUSTFLAGS = $(STANDARD_THREADS_FLAG)$(if $(filter Linux,$(BUILD_HOST_OS)), $(STANDARD_MOLD_FLAG))
# Warnings-as-errors plus the standard, appended to whatever the caller set.
GATE_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(STANDARD_RUSTFLAGS)"
# A debug build that keeps the caller's warning policy rather than imposing one.
DEBUG_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(STANDARD_RUSTFLAGS)"
# Release builds take neither the parallel frontend nor `mold`: assigning
# `RUSTFLAGS` at all, even to an empty inherited value, displaces the
# configuration file's `rustflags` tables, which is the whole mechanism. The
# configuration names no codegen backend at all, so nothing else is needed.
RELEASE_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS-}"
# Kani denies warnings like the gates, but takes none of the standard: it drives
# `rustc` through `kani-compiler` on its own bundled toolchain, so the parallel
# frontend and `mold` would neither apply nor be honoured there. Inherited flags
# still survive, since `cargo kani` appends this value to its own.
KANI_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings"
# Command name, resolved by the recipe shell from the curated PATH, which
# carries `$HOME/.bun/bin` where the global markdownlint install lands.
MDLINT ?= markdownlint-cli2
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# `--git` skips symbolic links, so CRUSH.md (a link to AGENTS.md) is never
# rewritten or checked twice. Both modes need mdtablefix 0.6.0 or later; CI
# pins the version in MDTABLEFIX_VERSION in .github/workflows/ci.yml.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
YAMLLINT ?= yamllint
# Go separates `GOPATH` entries with the platform list separator: `;` on
# Windows, `:` elsewhere. `go install` writes to the `bin` directory of the
# *first* entry, so split on that separator and keep the first one; appending
# `/bin` to a whole list would curate `$GOPATH` itself and omit the directory Go
# actually installs into.
GOPATH_SEPARATOR := $(if $(filter Windows_NT,$(OS)),;,:)
# `go install` writes to `$GOBIN` when set, otherwise that first `$GOPATH` entry's
# `bin`, otherwise `$HOME/go/bin`. The directory is absent from a minimal caller
# PATH, so name it once here and curate it on PATH below, next to the other user
# tool directories.
GO_BIN ?= $(if $(GOBIN),$(GOBIN),$(if $(GOPATH),$(word 1,$(subst $(GOPATH_SEPARATOR), ,$(GOPATH)))/bin,$(HOME)/go/bin))
# Exported so the `github-actions-lint` preflight can name the directory from
# the recipe shell rather than interpolating a path into the command line.
export GO_BIN
# Command name, resolved by the recipe shell like CARGO and MDLINT above, so an
# absolute `ACTIONLINT=/path/to/actionlint` still overrides it (CI does that).
# The `github-actions-lint` preflight reports the Go tool directory when the
# lookup fails, instead of letting the recipe die with a bare exit 127.
ACTIONLINT ?= actionlint
export ACTIONLINT
# Single source of truth for the shared spelling gate; the Makefile and CI
# both consume it, so the pinned builder cannot drift apart.
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.1
YAMLLINT_VERSION ?= 1.38.0
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
# The Python baseline every uv-driven helper pins. Bump this alongside the
# `target-version`/`py-version` settings in pyproject.toml and the
# `python-version` inputs in .github/workflows/ci.yml, release.yml, and
# build-and-package.yml; the workflow contract tests hold the Makefile and CI
# in sync.
PYTHON_BASELINE ?= 3.14
COVERAGE_ARTIFACT_DIR ?= coverage-artifact
# Pin Ruff so `make` invokes the same version everywhere; floating the version
# causes version-skew lint failures because rule sets differ between releases.
# CI pins the same value in .github/workflows/ci.yml; a contract test in
# tests/workflow_contracts keeps the two from drifting apart.
RUFF_VERSION ?= 0.16.4
RUFF = $(UV_ENV) $(UV) tool run --from ruff==$(RUFF_VERSION) ruff
# Pin Interrogate so documentation coverage is stable between local and CI
# runs. The repository uses it only as a quality gate; it is not a Python
# distribution and does not need project metadata.
INTERROGATE_VERSION ?= 1.7.0
INTERROGATE = $(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
	--from 'interrogate==$(INTERROGATE_VERSION)' \
	interrogate --fail-under 100
# Pin ty so `make` and CI invoke the same typechecker release. ty is pre-1.0
# and diagnostics shift between releases, so an unpinned install breaks the
# typecheck gate without any code change. Bump deliberately and fix new
# diagnostics in the same commit.
TY_VERSION ?= 0.0.74
# Every Python source the repository owns. Ruff and Pylint resolve their own
# configuration and exclusions from pyproject.toml, so these paths only bound
# the walk.
PYTHON_SOURCES = .github/scripts scripts tests/workflow_contracts
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
	--from 'pylint' --with '$(DF12_PYTHON_LINTS)' pylint \
	--disable=all --load-plugins=df12_python_lints \
	--enable=$(DF12_PYLINT_MESSAGES)
AMBRLEAKS = $(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
	--from '$(DF12_PYTHON_LINTS)' ambrleaks
# The estate-synchronised spelling helpers retain their standalone coverage
# policy. Keep this explicit so Interrogate covers every other owned Python
# definition in the same source boundary as Ruff and Pylint.
INTERROGATE_EXCLUDES = $(addprefix --exclude ,$(SPELLING_HELPER_FILES))
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
# The shared en-GB-oxendict spelling gate. It regenerates `typos.toml` from
# the live shared dictionary and the `typos.local.toml` overlay on every run,
# then runs Typos and the prohibited-phrase check.
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
	--from "git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
PROVER_TOOLS_SOURCE ?= git+https://github.com/leynos/rust-prover-tools@b07ef696f8373d54ae68e517d39d47a5d27a5bd5
PROVER_TOOLS ?= uv tool run --from $(PROVER_TOOLS_SOURCE) prover-tools
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
unexport RUSTDOC_FLAGS
export RUSTDOCFLAGS := $(value RUSTDOC_FLAGS)
VERUS_FLAGS ?=
VERUS_INSTALL_FLAGS ?=
WHITAKER ?= whitaker

# The build-tools install prefix leads: `-fuse-ld=mold` resolves by PATH order, so
# the pinned release must outrank any distribution `mold`. Every build target
# now links with it, so this is global rather than target-specific.
# GO_BIN is appended after the three fixed directories so a tool present in
# more than one location keeps its current precedence; CI's explicit
# `ACTIONLINT=` override still wins over all of them.
export PATH := $(BUILD_TOOLS_PREFIX)/bin:$(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin:$(GO_BIN):$(PATH)

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: test-nextest doctest ## Run every Rust test with warnings treated as errors

test-nextest: check-build-tools ## Run all non-doctest Rust tests through cargo-nextest
	$(GATE_RUSTFLAGS) $(CARGO) nextest run --workspace --all-targets --all-features $(NEXTEST_BUILD_JOBS) $(NEXTEST_TEST_JOBS)

doctest: check-build-tools ## Run doctests, which cargo-nextest cannot execute
	$(GATE_RUSTFLAGS) $(CARGO) test --workspace --doc --all-features $(BUILD_JOBS)

test-workflow-contracts: ## Validate GitHub Actions workflow contracts
	$(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' --with 'cmd-mox==0.2.0' pytest tests/workflow_contracts -q --doctest-modules

test-windows-msi-release-rank: ## Validate Windows MSI release-rank parsing
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		--with pytest==9.0.2 --with hypothesis==6.151.9 \
		python -m pytest scripts/tests/test_windows_msi_release_rank.py

test-release-admission: ## Validate the release-admission runtime contract
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		--with pytest==9.0.2 --with hypothesis==6.151.9 \
		python -m pytest scripts/tests/test_release_admission_metrics.py \
		scripts/tests/test_release_admission_metric_failures.py \
		scripts/tests/test_release_admission_metric_boundedness.py -c /dev/null \
		--rootdir=. -p no:cacheprovider

test-coverage-artifact: ## Test hostile LCOV artefact validation
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		--with pytest==9.0.2 python -m pytest scripts/tests/test_validate_coverage_artifact.py \
		scripts/tests/test_validate_coverage_archive.py -c /dev/null --rootdir=. \
		-p no:cacheprovider

# Split rather than a single `target/%/$(APP)` pattern: the two profiles no
# longer share a command line. The debug build takes the standard; the release
# build is one of the two exclusions.
target/debug/$(APP): | check-build-tools ## Build the debug binary
	$(DEBUG_RUSTFLAGS) $(CARGO) build $(BUILD_JOBS) --bin $(APP)

target/release/$(APP): ## Build the release binary on the LLVM backend
	$(RELEASE_RUSTFLAGS) $(CARGO) build $(BUILD_JOBS) --release --bin $(APP)

lint: lint-clippy lint-whitaker lint-python github-actions-lint ## Run the Rust, Python, and GitHub Actions lint suites with warnings denied

lint-python: lint-workflow-scripts ## Run Ruff, Pylint, Interrogate, the df12 house lints, and ambrleaks over the Python sources
	$(RUFF) check $(PYTHON_SOURCES)
	$(PYLINT) $(PYLINT_TARGETS)
	$(DF12_PYLINT) $(PYLINT_TARGETS)
	$(AMBRLEAKS) $(PYTHON_SOURCES)
	$(INTERROGATE) $(INTERROGATE_EXCLUDES) $(PYTHON_SOURCES)

lint-workflow-scripts: ## Load every trusted workflow module under the Python baseline
	# The trusted coverage workflow runs these through GitHub Actions' `python`
	# shell, and tests/workflow_contracts/python_shell_interpreter_test.py
	# holds that shell to the baseline. Ruff and ty read the modules; loading
	# them catches a definition-time failure -- a name used at module scope, an
	# invalid decorator -- which no pull request can fix once it is on main,
	# because the workflow runs the default branch's copy.
	#
	# Loading does NOT catch an annotation naming a TYPE_CHECKING-only import,
	# which this comment claimed until the baseline moved to 3.14: PEP 649
	# defers annotation evaluation, so such a module loads cleanly and fails
	# only when something resolves the annotation. The claim held under 3.12
	# and no longer does. Catch it by calling typing.get_type_hints, not by
	# loading; see
	# docs/adr-034-runtime-annotation-introspection-in-workflow-contracts.md.
	@for module in .github/scripts/*.py; do \
		$(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) python -c \
			'import runpy, sys; runpy.run_path(sys.argv[1], run_name="lint_workflow_scripts")' \
			"$$module" || { echo "$$module does not load under Python $(PYTHON_BASELINE)" >&2; exit 1; }; \
	done

lint-clippy: check-build-tools ## Run rustdoc and Clippy with warnings denied
	$(GATE_RUSTFLAGS) $(CARGO) doc --workspace --no-deps
	$(GATE_RUSTFLAGS) $(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: check-build-tools ## Run the Whitaker Dylint suite with warnings denied
	DYLINT_TOML="$$(cat dylint.toml)" $(GATE_RUSTFLAGS) $(WHITAKER) --all --no-deps --package netsuke-build -- --all-targets --all-features
	# Run from the crate directory as well so Whitaker loads the narrow
	# `test_support::fs` exemption from test_support/dylint.toml.
	cd test_support && DYLINT_TOML="$$(cat dylint.toml)" $(GATE_RUSTFLAGS) $(WHITAKER) --all --no-deps --package test_support -- --all-targets --all-features

# actionlint is resolved in the recipe shell below, never while Make parses the
# file: only the recipe shell receives the curated PATH, so a parse-time probe
# can report a Go-installed actionlint as missing. Checking in the shell also
# keeps the diagnostic beside the invocation it explains.
github-actions-lint: ## Validate GitHub Actions workflows
	$(YAMLLINT) --config-file .yamllint.yml .github/workflows
	@command -v "$$ACTIONLINT" >/dev/null 2>&1 || { \
		printf '%s\n' \
			"actionlint could not be run: ACTIONLINT is '$$ACTIONLINT'." \
			"Install it with go (go install github.com/rhysd/actionlint/cmd/actionlint@latest)," \
			"which writes $$GO_BIN/actionlint, or set ACTIONLINT=/path/to/actionlint." >&2; \
		exit 1; \
	}
	$(ACTIONLINT)

doc-coverage: doc-coverage-test ## Verify aggregate Rustdoc doc-comment coverage meets the threshold
	# Runs under the uv-pinned baseline interpreter, not the system python3:
	# the scripts target Python 3.14 syntax and semantics.
	@$(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
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

validate-coverage-artifact: ## Validate downloaded coverage artefact as hostile data
	$(UV_ENV) $(UV) run --no-project --python $(PYTHON_BASELINE) \
		scripts/validate_coverage_archive.py \
		--archive-dir "$(COVERAGE_ARTIFACT_DIR)" \
		--output-dir validated-coverage

fmt: ## Format Rust, Python, and Markdown sources
	$(CARGO) fmt --all
	$(RUFF) format $(PYTHON_SOURCES)
	$(RUFF) check --select I --fix $(PYTHON_SOURCES)
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(RUFF) format --check $(PYTHON_SOURCES)
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

typecheck: check-build-tools typecheck-python ## Typecheck all targets and features
	$(GATE_RUSTFLAGS) $(CARGO) check --all-targets --all-features $(BUILD_JOBS)

typecheck-python: ## Typecheck the Python sources with ty
	# `uv tool run` materialises one venv holding ty plus the test-suite
	# dependencies, so ty can resolve third-party imports. `uv run --with`
	# would layer the extras through `.pth` chaining, which ty cannot follow.
	$(UV_ENV) $(UV) tool run --python $(PYTHON_BASELINE) \
		--from ty==$(TY_VERSION) --with pytest==9.0.2 --with pytest-cov==7.0.0 \
		--with 'pyyaml>=6' --with 'hypothesis>=6' --with 'cmd-mox==0.2.0' \
		ty check --python-version $(PYTHON_BASELINE) \
		--extra-search-path scripts $(PYTHON_SOURCES)

markdownlint: spelling ## Lint Markdown and enforce en-GB-oxendict spelling
	@unset FORCE_COLOR; $(MDLINT) "**/*.md"

spelling: ## Enforce en-GB-oxendict spelling
	$(TYPOS_CONFIG_BUILDER) gate --repository .

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
	$(KANI_RUSTFLAGS) $(KANI) $(KANI_FLAGS)

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

install-build-tools: ## Install the pinned mold linker and the pinned toolchain
	@scripts/install-build-tools.sh

check-build-tools: ## Check the mold linker and toolchain prerequisites
	@scripts/check-build-tools.sh

bench-build: check-build-tools ## Time clean and incremental debug builds for all three paths
	@CARGO="$(CARGO)" STANDARD_THREADS_FLAG="$(STANDARD_THREADS_FLAG)" \
		STANDARD_MOLD_FLAG="$(STANDARD_MOLD_FLAG)" scripts/bench-build.sh

bench-config-load: ## Benchmark cached configuration loading without layer copies
	$(CARGO) bench --bench config_load_cached_merge

bench-glob-expansion: ## Benchmark manifest glob expansion with an injected base
	$(CARGO) bench --bench glob_expansion

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
