.PHONY: help all clean test build release lint typecheck fmt check-fmt markdownlint nixie

APP ?= netsuke
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors (single-threaded for env isolation)
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS) -- --test-threads=1

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

typecheck: ## Type-check staging scripts with ty
	uvx --with cyclopts ty check --extra-search-path \
		.github/actions/stage/scripts/ \
		.github/actions/stage/scripts/*.py

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
