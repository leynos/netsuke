.PHONY: help all clean test build release lint fmt check-fmt typecheck markdownlint nixie install-kani kani kani-full install-verus verus formal-pr

APP ?= netsuke
CARGO ?= $(shell command -v cargo 2>/dev/null || printf '%s' "$$HOME/.cargo/bin/cargo")
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
KANI ?= cargo kani
KANI_FLAGS ?=
KANI_INSTALL_FLAGS ?=
KANI_CHECK_FLAGS ?=
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
NIXIE ?= nixie
PROVER_TOOLS_SOURCE ?= git+https://github.com/leynos/rust-prover-tools@b07ef696f8373d54ae68e517d39d47a5d27a5bd5
PROVER_TOOLS ?= uv tool run --from $(PROVER_TOOLS_SOURCE) prover-tools
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
VERUS_FLAGS ?=
VERUS_INSTALL_FLAGS ?=

export PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(PATH)

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

typecheck: ## Typecheck all targets and features
	RUSTFLAGS="-D warnings" $(CARGO) check --all-targets --all-features $(BUILD_JOBS)

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

install-kani: ## Install the pinned Kani verifier
	$(PROVER_TOOLS) kani install $(KANI_INSTALL_FLAGS)

kani: ## Run the Kani local smoke check
	$(PROVER_TOOLS) kani check-version --kani-command "$(KANI)" $(KANI_CHECK_FLAGS)

kani-full: ## Run the full Kani verification suite
	$(KANI) $(KANI_FLAGS)

install-verus: ## Install the pinned Verus verifier
	$(PROVER_TOOLS) verus install $(VERUS_INSTALL_FLAGS)

verus: ## Run the Verus proof entry point
	$(PROVER_TOOLS) verus run $(VERUS_FLAGS)

formal-pr: ## Run pull-request formal-verification checks
	$(MAKE) kani

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
