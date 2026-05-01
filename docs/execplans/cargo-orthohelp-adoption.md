# Adopt `cargo orthohelp` for release help artefacts

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This plan was approved for implementation on 2026-05-01.

## Purpose / big picture

Netsuke currently generates its Unix manual page from `build.rs` using
`clap_mangen`. Release packaging then discovers the generated file under
`target/generated-man/<target>/release` or falls back to Cargo's `OUT_DIR`.
This couples ordinary Cargo builds to release documentation output and prevents
the release workflow from using the richer `OrthoConfigDocs` metadata described
in `docs/ortho-config-users-guide.md`.

After this change, the release workflow generates both the Unix manual page and
Windows PowerShell external help by calling `cargo orthohelp` explicitly. A
normal `cargo build` still runs the localization audit in `build.rs`, but it no
longer writes release help files. Users observe the change in release
artefacts: Linux and macOS packages still include `netsuke.1`, and Windows
release artefacts include Microsoft Assistance Markup Language (MAML) help
suitable for `Get-Help Netsuke -Full`. The Netsuke command-line interface
itself does not change.

## Constraints

- Do not implement this plan until the user explicitly approves it.
- Keep the branch named `cargo-orthohelp-adoption`.
- Keep `build.rs` for the localization-key audit unless a later approved
  implementation discovery proves the audit must move.
- Remove `clap_mangen` from the build path when `cargo orthohelp` owns release
  help generation.
- Preserve reproducible manual dates. An unset or invalid
  `SOURCE_DATE_EPOCH` must continue to produce `1970-01-01`, with a warning for
  invalid values.
- Generate release help from `OrthoConfigDocs`, not directly from Clap.
- Use `rstest` for new unit-style tests.
- Use `rstest-bdd` v0.5.0 for behavioural tests.
- Cover happy paths, unhappy paths, and relevant edge cases in tests.
- Keep documentation in en-GB Oxford English, following
  `docs/documentation-style-guide.md`.
- Update `docs/users-guide.md` for user-visible release help behaviour.
- Update `docs/developers-guide.md` for developer tooling, workflow, and
  architecture guidance.
- Run `make check-fmt`, `make lint`, and `make test` successfully before each
  implementation commit. For documentation edits, also run `make fmt` and
  `make markdownlint`; run `make nixie` if Mermaid diagrams are edited.
- Use `tee` for long gate commands, with logs under `/tmp`.
- Do not run format, lint, or test gates in parallel.
- Do not create an isolated Cargo cache.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 18 files changed, or more than
  900 net non-documentation lines changed, stop and ask for approval to expand
  the scope.
- Public interface: if any user-facing CLI flag, subcommand, or exit status
  must change, stop and ask for approval.
- Release packaging: if the shared `windows-package` action cannot include the
  PowerShell help files in the Microsoft Installer (MSI), it is acceptable to
  ship them as Windows release artefact sidecars. If neither route is possible,
  stop and ask for direction.
- Dependencies: if a new runtime dependency is needed, stop and ask for
  approval. Installing `cargo-orthohelp = 0.8.0` in the release workflow is
  within scope because it is a release tool, not a runtime dependency.
- Tool availability: if the pinned `cargo-orthohelp` installation command
  fails on a target runner, stop and record the runner, error, and alternatives
  before changing strategy.
- Tests: if `make check-fmt`, `make lint`, or `make test` still fails after
  three focused fix attempts, stop and ask for direction.
- Ambiguity: if `cargo orthohelp` output paths or flags differ from
  `docs/ortho-config-users-guide.md` and `cargo info cargo-orthohelp`, stop and
  reconcile the versioned tool behaviour before proceeding.

## Risks

- Risk: `cargo-orthohelp` may not already be installed on GitHub-hosted
  runners. Severity: medium. Likelihood: high. Mitigation: add an explicit
  workflow install step using
  `cargo install cargo-orthohelp --version 0.8.0 --locked`, and contract-test
  that the release workflow installs the tool before generating help.

- Risk: `cargo orthohelp` cache behaviour may produce stale documentation.
  Severity: medium. Likelihood: medium. Mitigation: do not use `--cache` or
  `--no-build` in the release workflow for the first adoption. Generate from a
  fresh bridge build each release.

- Risk: `OrthoConfigDocs` metadata may be incomplete for Netsuke's CLI.
  Severity: medium. Likelihood: medium. Mitigation: add
  `[package.metadata.ortho_config]` in `Cargo.toml`, then add a smoke
  validation step and tests that confirm expected man and PowerShell help
  output paths are produced.

- Risk: release staging may not support directory artefacts for the PowerShell
  module tree. Severity: medium. Likelihood: medium. Mitigation: stage explicit
  files: `Netsuke.psm1`, `Netsuke.psd1`, `en-US/Netsuke-help.xml`, and
  `en-US/about_Netsuke.help.txt`. Prefer a directory only if the shared action
  already supports it.

- Risk: Linux packaging currently relies on a staged `dist` man-page path
  created implicitly by the shared build action. Severity: high. Likelihood:
  medium. Mitigation: make staging explicit for all platforms, including Linux,
  before package creation. Pass the staged `man_path` output to the Linux
  packaging action.

- Risk: removing `clap_mangen` may expose build-script dead-code warnings
  because `build.rs` still imports shared CLI modules for the localization
  audit. Severity: medium. Likelihood: medium. Mitigation: keep only the symbol
  anchors needed by the audit, remove anchors used solely for manual
  generation, and let `make lint` identify any missing anchors.

## Progress

- [x] (2026-05-01T15:50:05Z) Loaded the `execplans`, `leta`,
  `rust-router`, `arch-crate-design`, `domain-cli-and-daemons`, and
  `commit-message` skills relevant to this planning task.
- [x] (2026-05-01T15:50:05Z) Renamed the current branch from
  `feat/cargo-orthohelp-plan` to `cargo-orthohelp-adoption`.
- [x] (2026-05-01T15:50:05Z) Used a Wyvern agent team for read-only
  reconnaissance of release packaging, `cargo-orthohelp` documentation, and
  test strategy.
- [x] (2026-05-01T15:50:05Z) Confirmed `cargo-orthohelp = 0.8.0` is available
  on crates.io with Rust version 1.88 and repository
  `https://github.com/leynos/ortho-config`.
- [x] (2026-05-01T15:50:05Z) Drafted this ExecPlan for review.
- [x] (2026-05-01T16:37:52Z) Received explicit approval to implement the
  planned functionality.
- [x] (2026-05-01T16:37:52Z) Committed the approval/status update after
  `make markdownlint` passed.
- [x] (2026-05-01T16:37:52Z) Added the first implementation patch for the
  `scripts/generate-release-help.sh` boundary and its `rstest` integration
  tests.
- [x] (2026-05-01T16:37:52Z) Validated the release-help script slice with
  `make check-fmt`, `make markdownlint`, `make lint`, `make test`, and the
  focused `cargo test --test release_help_script_tests` run.
- [ ] Implement the approved migration.
- [ ] Run all required gates and commit the implementation.

## Surprises & discoveries

- Observation: the current branch was already a task branch, not `main`, but
  its name did not match the requested name. Evidence:
  `git branch --show-current` returned `feat/cargo-orthohelp-plan`; it was
  renamed to `cargo-orthohelp-adoption`. Impact: no blocker; the branch now
  matches the request.

- Observation: the current `build.rs` does two jobs, not one. It generates the
  manual page and audits Fluent localization keys. Evidence: `build.rs` imports
  `clap_mangen::Man` and also calls
  `build_l10n_audit::audit_localization_keys()`. Impact: implementation should
  remove only release help generation from `build.rs`, leaving the localization
  audit in place.

- Observation: `cargo-orthohelp` is published as version `0.8.0`, matching the
  `ortho_config = "0.8.0"` dependency already used by Netsuke. Evidence:
  `cargo search cargo-orthohelp --limit 5` and `cargo info cargo-orthohelp`
  reported version `0.8.0`. Impact: the release workflow can pin the
  installation to `0.8.0`.

## Decision log

- Decision: keep this plan in `Status: DRAFT` until the user approves it.
  Rationale: the user explicitly requested planning and reminded that the plan
  must be approved before implementation. Date/Author: 2026-05-01T15:50:05Z /
  Codex.

- Decision: generate release help in an explicit workflow/script step rather
  than in `build.rs`. Rationale: this directly replaces the build-script
  approach, keeps ordinary Cargo builds focused on compilation and audits, and
  makes release help generation observable and testable. Date/Author:
  2026-05-01T15:50:05Z / Codex.

- Decision: do not use `cargo orthohelp --cache` or `--no-build` in release
  automation for the first adoption. Rationale: release documentation should be
  regenerated from current sources; stale-cache risk is not worth the small
  speed gain. Date/Author: 2026-05-01T15:50:05Z / Codex.

- Decision: make the release workflow install
  `cargo-orthohelp = 0.8.0` explicitly. Rationale: GitHub-hosted runner images
  are not a stable contract, and pinning the release tool keeps the workflow
  reproducible. Date/Author: 2026-05-01T15:50:05Z / Codex.

- Decision: stage explicit PowerShell help files rather than assuming directory
  staging support. Rationale: the local staging configuration currently lists
  file artefacts, and explicit file paths are easier to contract-test.
  Date/Author: 2026-05-01T15:50:05Z / Codex.

## Outcomes & retrospective

The plan is drafted and awaiting approval. No implementation has started.
Implementation approval has been received. The work is now in progress.

## Context and orientation

The relevant release and documentation files are:

- `build.rs`: currently generates `netsuke.1` with `clap_mangen`, writes it to
  `target/generated-man/<target>/<profile>`, mirrors it into `OUT_DIR`, and
  audits localization keys.
- `Cargo.toml`: declares `clap_mangen = "0.2.29"` under
  `[build-dependencies]`, declares `ortho_config = "0.8.0"` for runtime and
  build script use, and already uses `rstest-bdd = "0.5.0"` for behavioural
  tests.
- `.github/workflows/build-and-package.yml`: reusable workflow that builds one
  target, packages Linux artefacts, stages non-Linux artefacts, builds Windows
  MSI packages, and builds macOS packages.
- `.github/workflows/release.yml`: tag and reusable release workflow that
  invokes `build-and-package.yml` for Linux, Windows, and macOS.
- `.github/release-staging.toml`: declares staged artefact sources. It
  currently finds the manual page under `target/generated-man` or Cargo's
  `OUT_DIR`.
- `scripts/package-artifact.sh`: legacy local packaging helper that still
  expects `target/generated-man/<target>/release/<bin>.1`.
- `tests/workflow_build_and_package.rs`,
  `tests/workflow_release.rs`, and `tests/workflow_shared_actions_pins.rs`:
  existing workflow contract tests.
- `tests/bdd_tests.rs`, `tests/features`, and `tests/bdd/steps`: current
  `rstest-bdd` v0.5.0 behavioural test harness.
- `docs/netsuke-design.md`: design source of truth for manual pages and
  release automation.
- `docs/users-guide.md`: user-facing tool behaviour.
- `docs/developers-guide.md`: developer practices and tooling guidance.

The required documentation and skill signposts are:

- `docs/ortho-config-users-guide.md`: source for `cargo orthohelp`, output
  layouts, metadata, and PowerShell options.
- `docs/netsuke-design.md`: release automation and current manual-page design.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and parameterized test
  conventions for `rstest`.
- `docs/rust-doctest-dry-guide.md`: guidance for any new Rust documentation
  examples.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`: test isolation
  and fake process boundaries for release helper testing.
- `docs/rstest-bdd-users-guide.md`: behavioural test style and
  `rstest-bdd` v0.5.0 semantics.
- Skills used for this plan: `execplans`, `leta`, `rust-router`,
  `arch-crate-design`, `domain-cli-and-daemons`, and `commit-message`.

Terms used in this plan:

- `cargo orthohelp`: Cargo subcommand installed from the `cargo-orthohelp`
  crate. It builds a bridge binary, asks `OrthoConfigDocs` for documentation
  metadata, localizes that metadata, and writes help artefacts.
- MAML: Microsoft Assistance Markup Language, the XML format used by
  PowerShell external help.
- IR: intermediate representation, the localized JSON representation generated
  before man or PowerShell output.
- Staging: copying build outputs into a predictable `dist` directory with
  paths exported for downstream package steps.

## Plan of work

Stage A is approval. Do not edit implementation files until this plan is
approved. After approval, start by re-reading this plan, `AGENTS.md`, and the
documentation signposts above. Re-run `git status --short --branch` and confirm
the branch is still `cargo-orthohelp-adoption`.

Stage B adds a small release-help boundary before changing workflows. Create
`scripts/generate-release-help.sh`. The script accepts the target triple,
binary name, and output root, computes a reproducible manual date from
`SOURCE_DATE_EPOCH`, runs `cargo orthohelp --format man`, and verifies that
`<out>/man/man1/<bin>.1` exists. When the target triple contains `windows`, it
also runs `cargo orthohelp --format ps --ps-module-name Netsuke` and verifies
that the four expected PowerShell files exist under
`<out>/powershell/Netsuke/en-US`. Invalid `SOURCE_DATE_EPOCH` values should
warn and fall back to `1970-01-01`. The script must not use `--cache` or
`--no-build`.

Stage C adds tests around that boundary. Add `rstest` integration tests in a
new file such as `tests/release_help_script_tests.rs`. Use temporary
directories and a fake `cargo` executable placed first on `PATH` so tests do
not need network access or a real `cargo-orthohelp` install. Cover at least:
successful man generation, successful Windows MAML generation, valid
`SOURCE_DATE_EPOCH`, invalid `SOURCE_DATE_EPOCH`, unset `SOURCE_DATE_EPOCH`,
fake `cargo orthohelp` failure, missing generated man output, and missing
generated MAML output.

Stage D updates release metadata and workflow wiring. In `Cargo.toml`, remove
`clap_mangen` from `[build-dependencies]`, keep build dependencies still needed
by the localization audit, and add:

```toml
[package.metadata.ortho_config]
root_type = "netsuke::cli::Cli"
locales = ["en-US", "es-ES"]

[package.metadata.ortho_config.windows]
module_name = "Netsuke"
include_common_parameters = true
split_subcommands_into_functions = false
```

In `build.rs`, remove manual-generation functions and the `clap_mangen` import.
Keep `emit_rerun_directives()`, the localization audit, and only the symbol
anchors needed for the build script to compile cleanly with strict lints.

In `.github/workflows/build-and-package.yml`, add a pinned install step for
`cargo-orthohelp = 0.8.0` after checkout and before help generation. After the
release binary build, call `scripts/generate-release-help.sh` with
`target/orthohelp/${{ inputs.target }}/release` as the output root. Make
staging run for all platforms, not only non-Linux, and pass
`${{ steps.stage_paths.outputs.man_path }}` to Linux packaging. Keep Windows
and macOS package steps consuming staged paths.

In `.github/release-staging.toml`, replace the manual source with
`target/orthohelp/{target}/release/man/man1/{bin_name}.1`, remove the `OUT_DIR`
fallback, and add Windows PowerShell artefact entries for:

- `target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1`
- `target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1`
- `target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml`
- `target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt`

If the staging helper cannot mark those files as Windows-only, keep them
`required = false` so Linux and macOS staging do not fail.

Update `scripts/package-artifact.sh` to read the man page from
`target/orthohelp/<target>/release/man/man1/<bin>.1`, or remove it only if a
separate approved decision confirms the script is obsolete.

Stage E adds workflow contract and behavioural tests. Extend
`tests/workflow_build_and_package.rs` and related helpers in `tests/common` so
`rstest` cases assert that the workflow installs `cargo-orthohelp`, invokes the
release-help script after the binary build, stages artefacts for every
platform, sends the staged man path to Linux packaging, and wires Windows and
macOS package inputs from staged outputs. Extend staging tests or add
`tests/release_staging_tests.rs` so `rstest` cases assert that
`target/generated-man`, `OUT_DIR`, and `clap_mangen` fallbacks are absent, the
new man source is present, and all PowerShell help files are declared.

Add `tests/features/release_help_generation.feature` and
`tests/bdd/steps/release_help_generation.rs`, registered from
`tests/bdd/steps/mod.rs`. Use `rstest-bdd` v0.5.0 scenarios to describe
observable release behaviour:

- A release build generates and stages a manual page from `cargo orthohelp`.
- A Windows release build generates and stages PowerShell MAML help.
- Invalid `SOURCE_DATE_EPOCH` falls back to `1970-01-01` with a warning.
- Missing generated help files fail the release-help step with a clear error.
- The release workflow no longer relies on `build.rs`, `target/generated-man`,
  or Cargo `OUT_DIR` for help artefacts.

Stage F updates documentation. In `docs/netsuke-design.md`, replace the
`clap_mangen` manual-page section with the `cargo orthohelp` release-help
design and record the decision that `build.rs` remains only for the
localization audit. In `docs/users-guide.md`, document that release artefacts
include a Unix manual page and Windows PowerShell help, and describe the
Windows `Get-Help Netsuke -Full` entry point without implying a CLI behaviour
change. In `docs/developers-guide.md`, document the release-help tooling,
`cargo install cargo-orthohelp --version 0.8.0 --locked`, the script boundary,
and the rule that CLI documentation metadata must be kept current when CLI
fields change. Update `README.md` and `docs/netsuke-cli-design-document.md` to
remove stale `clap_mangen` references.

Stage G validates and commits. Run `make fmt` after Markdown edits. Then run
the gates sequentially with `tee`, inspect logs for warnings or failures, fix
issues, and commit with a file-based message using the `commit-message` skill.

## Concrete steps

Run all commands from the repository root:

```bash
cd /home/leynos/.lody/repos/github---leynos---netsuke/worktrees/ff42107e-75fb-4342-9998-243bdfe1c09b
```

Before implementation, confirm branch and state:

```bash
git branch --show-current
git status --short --branch
```

Expected branch output:

```plaintext
cargo-orthohelp-adoption
```

After approval, install or verify the release helper tool locally:

```bash
cargo install cargo-orthohelp --version 0.8.0 --locked
cargo orthohelp --help
```

The release workflow will perform its own install step, so local installation
is only for manual smoke checks.

For implementation validation, run these commands sequentially:

```bash
make fmt 2>&1 | tee /tmp/fmt-netsuke-cargo-orthohelp-adoption.out
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-cargo-orthohelp-adoption.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-cargo-orthohelp-adoption.out
make lint 2>&1 | tee /tmp/lint-netsuke-cargo-orthohelp-adoption.out
make test 2>&1 | tee /tmp/test-netsuke-cargo-orthohelp-adoption.out
```

If any command fails, inspect the corresponding `/tmp` log, fix the underlying
issue, rerun the failed gate, and then rerun any later gates that could be
affected.

If Mermaid diagrams are edited, also run:

```bash
make nixie 2>&1 | tee /tmp/nixie-netsuke-cargo-orthohelp-adoption.out
```

## Validation and acceptance

The implementation is accepted when all of these are true:

- `build.rs` no longer imports or calls `clap_mangen`.
- `Cargo.toml` no longer declares `clap_mangen`.
- `Cargo.toml` declares `package.metadata.ortho_config` with
  `root_type = "netsuke::cli::Cli"` and locales `en-US` and `es-ES`.
- The release workflow installs `cargo-orthohelp = 0.8.0` explicitly.
- The release workflow calls the release-help script after building the
  release binary and before packaging or staging consumers need help files.
- Manual pages are generated under
  `target/orthohelp/<target>/release/man/man1/netsuke.1`.
- Windows PowerShell help files are generated under
  `target/orthohelp/<target>/release/powershell/Netsuke`.
- `.github/release-staging.toml` stages the new man-page path and no longer
  references `target/generated-man` or `OUT_DIR`.
- Linux packaging consumes the staged `man_path`.
- Windows release artefacts include staged PowerShell help files.
- New `rstest` tests cover script behaviour and workflow/staging contracts.
- New `rstest-bdd` v0.5.0 scenarios cover release help happy paths, unhappy
  paths, and edge cases.
- `docs/users-guide.md`, `docs/developers-guide.md`,
  `docs/netsuke-design.md`, `README.md`, and
  `docs/netsuke-cli-design-document.md` no longer describe `clap_mangen` as the
  release help generator.
- `make check-fmt`, `make lint`, and `make test` all succeed.

## Idempotence and recovery

The release-help script should be safe to rerun. It may overwrite files under
its output root, but it must not delete unrelated `target` or `dist` content.
If a local smoke run produces unwanted generated files, remove only the chosen
scratch output root, for example:

```bash
rm -rf target/orthohelp-smoke
```

If workflow changes fail tests, revert only the last uncommitted edit by
editing the affected file directly. Do not run destructive Git commands such as
`git reset --hard` or `git checkout --` unless the user explicitly requests
them.

If `cargo install cargo-orthohelp --version 0.8.0 --locked` fails because of a
transient network issue, rerun it once. If it fails because of a compiler or
dependency incompatibility, stop and record the error in `Decision Log` before
choosing a different installation strategy.

## Artifacts and notes

Reconnaissance evidence gathered before drafting:

```plaintext
git branch --show-current
feat/cargo-orthohelp-plan

git branch -m cargo-orthohelp-adoption
git branch --show-current
cargo-orthohelp-adoption
```

```plaintext
cargo search cargo-orthohelp --limit 5
cargo-orthohelp = "0.8.0"    # OrthoConfig documentation tooling for IR generation.
```

```plaintext
cargo info cargo-orthohelp
version: 0.8.0
license: ISC
rust-version: 1.88
repository: https://github.com/leynos/ortho-config
```

Current path to replace:

```plaintext
target/generated-man/{target}/release/{bin_name}.1
target/{target}/release/build/*/out/{bin_name}.1
```

Planned primary paths:

```plaintext
target/orthohelp/{target}/release/man/man1/{bin_name}.1
target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1
target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1
target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml
target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt
```

## Interfaces and dependencies

The release-help script interface should be stable and simple:

```bash
scripts/generate-release-help.sh <target> <bin-name> <out-dir>
```

The script must call `cargo orthohelp` rather than `cargo-orthohelp` directly
so it uses Cargo's normal subcommand resolution:

```bash
cargo orthohelp \
  --format man \
  --out-dir "$out_dir" \
  --locale en-US \
  --man-section 1 \
  --man-date "$man_date"
```

For Windows targets, it must also call:

```bash
cargo orthohelp \
  --format ps \
  --out-dir "$out_dir" \
  --locale en-US \
  --ps-module-name Netsuke \
  --ensure-en-us true
```

The release workflow dependency is `cargo-orthohelp = 0.8.0`, installed with:

```bash
cargo install cargo-orthohelp --version 0.8.0 --locked
```

No new runtime dependency is planned. No new crate split is planned.

## Revision note

Initial draft. This records the approved-design boundary, current release help
state, intended `cargo orthohelp` workflow, validation strategy, documentation
updates, and quality gates.

2026-05-01 implementation revision. The plan was approved and moved to
`Status: IN PROGRESS`; implementation is proceeding milestone by milestone.
