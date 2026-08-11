# Repository layout

This document explains the major paths in the Netsuke repository and the
responsibilities attached to each area. It is an orientation guide for
contributors and does not replace the source code, design document, or
developer's guide as the source of truth for behaviour.

## Top-level structure

The following tree is a simplified orientation map. It omits generated build
output and some leaf files so the long-lived structure remains visible.

```plaintext
.
├── .github/
│   ├── actions/
│   └── workflows/
├── cyclopts/
├── docs/
│   ├── archive/
│   ├── execplans/
│   └── rfcs/
├── examples/
│   └── hello-world/
├── installer/
├── locales/
├── scripts/
├── src/
│   ├── cli/
│   ├── ir/
│   ├── localization/
│   ├── manifest/
│   ├── runner/
│   ├── snapshots/
│   └── stdlib/
├── test_support/
├── tests/
│   ├── bdd/
│   ├── cli_tests/
│   ├── data/
│   ├── features/
│   ├── features_unix/
│   ├── fixtures/
│   └── snapshots/
└── tools/
    ├── dev-fast/
    ├── kani/
    └── mold/
```

## Path responsibilities

- `AGENTS.md`: Repository-specific agent instructions, quality gates, and
  coding rules.
- `Cargo.toml` and `Cargo.lock`: Workspace package metadata and locked
  dependency graph.
- `Makefile`: Canonical quality-gate and workflow commands. Prefer these
  targets over direct tool invocations.
- `README.md`: Public project overview and first contact documentation.
- `.github/actions/`: Reusable GitHub Actions used by workflow definitions.
- `.github/workflows/`: Continuous Integration (CI), release, packaging, and
  repository automation workflows.
- `cyclopts/`: Local Python typing support for release and packaging helper
  scripts.
- `docs/`: Long-lived project documentation, guides, design documents, decision
  records, and planning material.
- `docs/archive/`: Historical planning documents retained for traceability
  after active roadmap work moves on.
- `docs/execplans/`: Execution plans used as implementation handoff documents
  for scoped tasks.
- `docs/rfcs/`: Numbered Requests for Comments that propose reviewable changes
  before they become binding decisions.
- `examples/`: Example Netsuke manifests and minimal runnable sample projects.
- `installer/`: Installer packaging assets and platform-specific packaging
  definitions.
- `locales/`: Fluent localization catalogues, one `<tag>/messages.ftl` per
  supported locale tag, so regional and script variants such as `es-419`,
  `pt-BR`, and `zh-Hant` each keep their own directory. The authoritative list
  of tags lives in `src/locale_catalogues.rs`; see the
  [translator guide](translators-guide.md).
- `scripts/`: Shell and helper scripts used by quality gates, release help
  generation, packaging, and formal checks.
- `src/`: Main `netsuke-build` Rust package source code.
- `src/cli/`: Command-line configuration, parsing, validation, and merge logic.
- `src/ir/`: Intermediate representation generation, interpolation, graph, and
  cycle logic.
- `src/localization/`: Localization key definitions and runtime localization
  support.
- `src/manifest/`: Manifest parsing, expansion, rendering, diagnostics, and
  manifest-specific tests.
- `src/runner/`: Process execution, path handling, runner errors, and runtime
  command orchestration.
- `src/snapshots/`: Checked-in `insta` snapshots for source-level snapshot
  tests.
- `src/stdlib/`: Netsuke standard library modules exposed to manifest
  rendering.
- `test_support/`: Shared Rust test-support crate used by integration and
  behavioural tests.
- `tests/`: Integration tests, behavioural tests, test data, fixtures, and
  snapshots.
- `tests/bdd/`: `rstest-bdd` step definitions, fixtures, and behavioural-test
  support code.
- `tests/cli_tests/`: Command-line interface integration test modules.
- `tests/data/`: Manifest fixtures and other structured test inputs.
- `tests/features/`: Cross-platform behavioural feature files.
- `tests/features_unix/`: Unix-specific behavioural feature files.
- `tests/snapshots/`: Checked-in integration-test snapshots.
- `tools/dev-fast/`: Non-auto-loaded Cargo configuration fragment for the
  opt-in Cranelift and `mold` build path. Cargo never discovers this file on
  its own; only the `make dev-*` targets pass it through `cargo --config`.
- `tools/kani/`: Kani formal-verification harness configuration and related
  local tooling.
- `tools/mold/`: Pinned `mold` linker release version and the SHA-256 checksums
  used to verify the downloaded release artefacts.

## Placement conventions

Place user-facing documentation under `docs/`, then link it from
[contents.md](contents.md). Use [users-guide.md](users-guide.md) for behaviour
that users or operators need to understand,
[developers-guide.md](developers-guide.md) for maintainer workflows, and
[netsuke-design.md](netsuke-design.md) for architecture and design rationale.

Place new production Rust modules under the `src/` subtree that owns the
feature boundary. Use `test_support/` for reusable integration-test helpers and
keep one-off fixtures close to the tests that consume them.

Place proposed, cross-cutting changes under `docs/rfcs/` using the numbering
and status conventions in the documentation style guide. Link each RFC from
`docs/contents.md` when it is first committed.

The crates.io package is named `netsuke-build`, while its library and binary
targets remain named `netsuke`. Keep command-line help, manual pages, release
artefacts, and operating-system packages aligned with the `netsuke` target name
rather than the Cargo package name.
[ADR-007](adr-007-publish-as-netsuke-build.md) records the decision, and the
[developer guide](developers-guide.md) describes how the build script, release
packaging, and `cargo binstall` metadata honour it.

Place feature files in `tests/features/` unless the behaviour depends on
Unix-specific platform contracts, in which case use `tests/features_unix/`.
Place generated or approved snapshot files under the existing `src/snapshots/`
or `tests/snapshots/` hierarchy that matches the test owner.
