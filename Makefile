.PHONY: help all clean test test-documentation-examples test-workflow-contracts test-typos-config build release lint lint-clippy lint-whitaker fmt check-fmt typecheck markdownlint spelling spelling-config spelling-helper-test nixie install-kani kani-check kani-full kani-ir install-verus verus formal-pr

APP ?= netsuke
CARGO ?= $(shell command -v cargo 2>/dev/null || printf '%s' "$$HOME/.cargo/bin/cargo")
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
CMD_MOX_SOURCE ?= git+https://github.com/leynos/cmd-mox@fc53a62f1a40d920e56ad70c96920d80ea2afd5e
CUPRUM_SOURCE ?= git+https://github.com/leynos/cuprum@4037b7250abd5729722b72ffc35581a0a8e238d1
DOCUMENTATION_EXAMPLE_TEST_FILES = tests/documentation_examples_py/conftest.py \
	tests/documentation_examples_py/documentation_examples.py \
	tests/documentation_examples_py/test_documentation_examples.py
KANI ?= cargo kani
KANI_FLAGS ?=
KANI_INSTALL_FLAGS ?=
KANI_CHECK_FLAGS ?=
KANI_VERSION_FILE ?= tools/kani/VERSION
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

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	$(MAKE) test-documentation-examples

test-documentation-examples: target/debug/$(APP) ## Validate user-facing examples in isolated environments
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --target-version py313 --check $(DOCUMENTATION_EXAMPLE_TEST_FILES)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --target-version py313 $(DOCUMENTATION_EXAMPLE_TEST_FILES)
	@NETSUKE_BIN="$(abspath target/debug/$(APP))" $(UV_ENV) $(UV) run --no-project --python 3.13 \
		--with pytest==9.0.2 --with $(CMD_MOX_SOURCE) --with $(CUPRUM_SOURCE) \
		python -m pytest tests/documentation_examples_py -c /dev/null \
		--rootdir=. -p no:cacheprovider -q

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

test-typos-config: spelling-helper-test ## Verify the shared spelling-policy integration

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: lint-clippy lint-whitaker ## Run Clippy and the Whitaker Dylint suite with warnings denied

lint-clippy: ## Run rustdoc and Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="-D warnings" $(WHITAKER) --all -- --all-targets --all-features

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

typecheck: ## Typecheck all targets and features
	RUSTFLAGS="-D warnings" $(CARGO) check --all-targets --all-features $(BUILD_JOBS)

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

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
