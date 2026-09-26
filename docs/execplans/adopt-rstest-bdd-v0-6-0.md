# Adopt `rstest-bdd` v0.6.0

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

Roadmap item: none. Origin: `leynos/rstest-bdd` v0.6.0 release and its
`docs/v0-6-0-migration-guide.md`.

## Purpose / big picture

Netsuke's behavioural suite runs on `rstest-bdd` 0.5.0. The upstream project
has published 0.6.0, whose headline behavioural change is a correctness fix: a
step whose return type is a *type alias* of `Result<T, E>` previously had its
`Err` silently discarded, so the scenario passed even though an assertion
failed. Netsuke's step functions are declared `-> Result<()>` where `Result` is
`anyhow::Result`, which is exactly such an alias. After this migration the
suite runs on 0.6.0, under the same 254 scenarios, and the four published
breaking changes that touch this repository are applied deliberately rather
than discovered as compile errors.

Success is observable by running `make test` and seeing all behavioural tests
pass from `tests/features/*.feature` and `tests/features_unix/*.feature` while
`Cargo.toml` declares `rstest-bdd = "0.6.0"` and `Cargo.lock` resolves the whole
`rstest-bdd` family to 0.6.0. The scenario inventory is unchanged at 254
tests, and deliberately so: this plan migrates an existing consumer and
introduces no new BDD. A scenario that turns red because the alias fix exposed
a previously-swallowed `Err` is a finding to investigate, not a regression to
suppress.

## Constraints

- Preserve existing behavioural coverage. No scenario may be dropped, and no
  assertion may be weakened to make the migration compile or pass.
- Imported upstream documentation is byte-for-byte intact. Provenance is
  recorded separately, never inside the imported copies.
- Do not introduce BDD into crates that do not use it. `rstest-bdd` stays a
  dev-dependency of the root package only.
- Do not copy upstream workspace `path` dependencies, the GPUI shim, or
  `vendor/gpui` into this repository.
- Keep the `rstest-bdd-macros` `strict-compile-time-validation` feature. Do not
  relax compile-time validation to make the migration compile.
- Keep caret requirements for all dependencies, per `AGENTS.md`.
- `netsuke-build` is a published library. Its declared `rust-version` must not
  advertise a compiler on which a promised target fails.
- The Polonius nightly pin in `rust-toolchain.toml` is load-bearing and is not
  to be changed unless the dependency graph demonstrably requires it.
- Do not use `--ignore-rust-version`, `#[ignore]`, or blanket snapshot
  blessing.

If satisfying the objective requires violating any constraint, stop and record
the conflict in `Decision log` before proceeding.

## Tolerances (exception triggers)

- Scope: if migration requires touching more than 30 files or 1,500 net lines,
  stop and escalate.
- Interface: if a public API in `src/` must change, stop and escalate.
- Dependencies: if a third-party dependency outside the `rstest-bdd` family
  must be added, stop and escalate.
- MSRV: if the resolved graph requires a compiler floor above the pinned
  nightly, stop and record before changing the pin.
- Iterations: if `make test` fails after 3 full fix cycles, stop and escalate.
- Ambiguity: if the published documentation and the pinned source disagree in a
  way that materially changes the migration, stop and present the conflict.

## Risks

- Risk: upstream `docs/CHANGELOG.md` mixes unreleased v0.7.0 content into the
  same file as 0.6.0, and the migration guide documents guard-based
  `StepContext` borrowing as "v0.7.0" while the published 0.6.0 crate and the
  `v0.6.0` tag both contain it. Following the guide's framing blindly would
  mean assuming an API is unavailable when it is present. Severity: medium
  Likelihood: high Mitigation: resolve every discrepancy against the pinned tag
  source and the published crate; record the evidence. Do not implement
  v0.7-only features that the crate does not contain, and do not avoid 0.6.0
  APIs the crate does contain. Recorded in `Surprises & discoveries` below.

- Risk: 0.6.0 classifies step returns by concrete type, so 41 `#[expect(...)]`
  attributes in `tests/bdd/steps/` that cite upstream issue #381 may become
  "unfulfilled expectation" errors if the generated wrapper no longer triggers
  those lints. Severity: medium Likelihood: medium Mitigation: run `make lint`
  immediately after the version bump; remove an expectation only where the
  compiler proves the lint no longer fires, and update `tests/bdd/steps/mod.rs`
  to match.

- Risk: the 39 `strict-compile-time-validation` checks may reject a step shape
  that 0.5.0 accepted. Severity: low Likelihood: low Mitigation: the feature is
  retained unchanged; compile errors name the offending step.

- Risk: `gherkin 0.16` raises the effective compiler floor to Rust 1.88 and the
  repository builds on a nightly pinned at 2026-08-23, far newer. The pin is
  therefore expected to be unaffected. Severity: low Likelihood: low
  Mitigation: confirm the resolved graph compiles under the existing pin before
  considering any toolchain change. A nightly date alone does not establish
  that a bump is necessary.

- Risk: the `#[expect(clippy::shadow_reuse)]` family is a Rust-2024-related
  lint whose firing depends on generated wrapper shape. Severity: low
  Likelihood: medium Mitigation: run `make lint-clippy` as a focused first
  signal before the full gate.

## Progress

- [x] (2026-09-26) Rename the branch to `adopt-rstest-bdd-v0-6-0`, tracking
  `origin/adopt-rstest-bdd-v0-6-0`.
- [x] (2026-09-26) Read `AGENTS.md` and the repository instructions; confirm the
  working tree is clean at `ebcedaef`.
- [x] (2026-09-26) Fetch `leynos/rstest-bdd` and resolve tag `v0.6.0` to
  `72fb22635670e456545ca368805ba4c1c9d7bd69`.
- [x] (2026-09-26) Read the v0.6.0 migration guide and the v0.5.0 guide.
- [x] (2026-09-26) Inventory manifests, dependencies, step/scenario counts, and
  direct runtime API usage.
- [x] (2026-09-26) Establish the baseline BDD inventory: 254 scenarios, 254
  generated tests.
- [ ] Import `docs/users-guide.md` and `docs/v0-6-0-migration-guide.md` from the
  pinned commit; record provenance.
- [ ] Bump the `rstest-bdd` family in `Cargo.toml` and update `Cargo.lock`.
- [ ] Reconcile lint expectations and step signatures; fix any newly-red
  scenario.
- [ ] Update `docs/developers-guide.md`, `docs/contents.md`, and add the v0.6.0
  migration ExecPlan references.
- [ ] Run the full gate set; compare the migrated inventory against baseline.
- [ ] Push and open a draft pull request.

## Surprises & discoveries

- Observation: the `v0.6.0` tag and the published `rstest-bdd 0.6.0` crate
  contain byte-identical `src/` trees, and both contain APIs the migration
  guide attributes to v0.7.0.

  Evidence: `git rev-parse v0.6.0` resolves to
  `72fb22635670e456545ca368805ba4c1c9d7bd69`. A recursive `sha256sum` over every
  `.rs` file under `crates/rstest-bdd/src` and `crates/rstest-bdd-macros/src`
  from the tag equals the same digest computed over the extracted
  `static.crates.io` `.crate` archives: `rstest-bdd` = `456fe6b2…e4a5`,
  `rstest-bdd-macros` = `acf79195…7e67` for both sources.
  `rg 'pub fn borrow_mut'` finds `&'b mut self` in 0.5.0 but `&'b self` in both
  the 0.6.0 tag and the published 0.6.0 crate, and both export
  `FixtureBorrowError` and `try_borrow`/`try_borrow_mut`.

  Impact: the migration guide's label of guard-based borrowing as "v0.7.0"
  describes a later *documentation* grouping, not the crate contents. The
  task's instruction to avoid "v0.7 guard-based borrowing" therefore applies to
  *adopting the v0.7 migration practice*; it does not mean the 0.6.0 crate lacks
  `&self` borrow receivers. This repository calls no borrow method directly,
  so the distinction is inert here; it is recorded because it would otherwise
  mislead any later reader of the guide.

- Observation: `Slot<T>` and the `strict-compile-time-validation` feature both
  survive into 0.6.0 unchanged.

  Evidence: `crates/rstest-bdd/src/state.rs:30` and
  `crates/rstest-bdd-macros/Cargo.toml` under the tag. The published
  `rstest-bdd-macros-0.6.0` feature table lists
  `strict-compile-time-validation = ["compile-time-validation"]`.

  Impact: the largest consumers of netsuke's suite — `Slot` use in
  `tests/bdd/fixtures/mod.rs` and the strict-validation feature choice — need
  no source change. The migration is materially smaller than the guide's scope
  suggests.

- Observation: netsuke's step functions declare `-> Result<()>` where `Result`
  is `anyhow::Result`, so 177 of 185 steps are aliases of the type whose `Err`
  was previously discarded.

  Evidence: `rg 'use anyhow::\{.*Result'` matches every `tests/bdd/steps/*`
  module. A signature sweep over the 185 step functions finds 177 `Result<()>`,
  7 `()`, and one non-step helper returning a tuple.

  Impact: this is the repository's live exposure to the fix. If any of those
  177 steps contains a step that returns `Err` while its scenario currently
  passes, 0.6.0 will surface it as a real failure. That is the migration's
  acceptance test, not an obstacle.

- Observation: the only step body that can skip, `tests/bdd/steps/fs.rs:62`,
  calls `rstest_bdd::skip!`, which remains present in 0.6.0.

  Evidence: `rg 'skip!' tests/bdd/` returns exactly one site; the macro is
  defined in the 0.6.0 tag's `crates/rstest-bdd/src/`.

  Impact: skip propagation is preserved. The baseline skips, if any, must be
  identical after migration.

- Observation: byte-for-byte upstream Markdown already satisfies this
  repository's Markdown gates.

  Evidence: with the repository's `.markdownlint-cli2.jsonc` and flags,
  `markdownlint-cli2` reports `0 error(s)` and
  `mdtablefix --check --git --include-untracked --wrap --renumber --breaks
  --ellipsis --fences`
  reports `2 files left unchanged` on the raw upstream `docs/users-guide.md`
  and `docs/v0-6-0-migration-guide.md`. `typos` with the repository's
  configuration exits 0 on both.

  Impact: the task's byte-for-byte import requirement and the repository's
  formatting gates do not conflict. No reformatting is needed, so the imported
  checksums are meaningful. This differs from the earlier v0.5.0 import, whose
  local copies are *not* byte-identical to upstream
  (`docs/rstest-bdd-users-guide.md` = `1c0a331c…617d` locally versus
  `ac9340d3…8fd6` upstream at tag v0.5.0); that earlier copy was adapted, and
  this plan deliberately does not repeat that.

## Imported-document provenance and link mapping

Both files were copied with `cp` from the pinned checkout; no editing tool
touched them.

| Destination                                 | Upstream path                    | Upstream commit                            | SHA-256 (verified equal)                                           |
| ------------------------------------------- | -------------------------------- | ------------------------------------------ | ------------------------------------------------------------------ |
| `docs/rstest-bdd-users-guide.md`            | `docs/users-guide.md`            | `72fb22635670e456545ca368805ba4c1c9d7bd69` | `1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2` |
| `docs/rstest-bdd-v0-6-0-migration-guide.md` | `docs/v0-6-0-migration-guide.md` | `72fb22635670e456545ca368805ba4c1c9d7bd69` | `6e76c10028f9962134f6158732cf1645ae1b0beba3018bccd3bc3e88b2c38985` |

The task's naming convention is followed exactly; the repository has no
competing convention for imported upstream documents.

`docs/rstest-bdd-users-guide.md` contains no relative Markdown links, so it
needs no mapping.

`docs/rstest-bdd-v0-6-0-migration-guide.md` links to five upstream files by
relative path. Because the imported text must stay byte-for-byte intact, the
links are resolved by this mapping rather than by rewriting the source. All
five refer to upstream `leynos/rstest-bdd` at commit `72fb2263`:

- `adr-006-fallible-scenario-functions.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-006-fallible-scenario-functions.md>
- `adr-007-harness-context-injection.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-007-harness-context-injection.md>
- `adr-009-consistent-implicit-fixture-name-normalization.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-009-consistent-implicit-fixture-name-normalization.md>
- `rstest-bdd-design.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/rstest-bdd-design.md>
- `developers-guide.md` → **not** this repository's
  `docs/developers-guide.md`, which is a different document; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/developers-guide.md>

The last entry is the one genuine hazard: a reader following that link inside
this repository lands on a plausible-looking but unrelated file. The mapping is
recorded here because the byte-for-byte constraint forbids an inline note at
the link site.

## Decision log

- Decision: import the two required documents byte-for-byte from the pinned
  commit and record provenance in this ExecPlan plus a provenance section in
  `docs/contents.md`, rather than in a separate provenance file. Rationale: the
  task permits either; `docs/contents.md` is the repository's established index
  for the `docs/` tree and already lists the v0.5.0 guide, so provenance
  recorded there is discoverable. Keeping the imported text untouched preserves
  checksum verifiability. Date/Author: 2026-09-26, implementing agent.

- Decision: resolve the `rstest-bdd` family with a targeted lockfile update
  against the published 0.6.0 packages, leaving the rest of the graph alone.
  Rationale: `AGENTS.md` mandates caret requirements and prohibits unrelated
  churn. A `--precise 0.6.0` update for each family member confines the change.
  Date/Author: 2026-09-26, implementing agent.

- Decision: treat upstream relative links from the imported guides as a
  documented link mapping rather than vendoring upstream design documents.
  Rationale: the migration guide links to `adr-006`, `adr-007`, `adr-009`,
  `adr-012`, `developers-guide.md`, and `rstest-bdd-design.md`. Of these,
  `docs/developers-guide.md` exists locally and is a *different* document; the
  rest do not exist. Vendoring them would be a large, unevidenced scope
  increase, and rewriting links would break byte-for-byte fidelity.
  Date/Author: 2026-09-26, implementing agent.

## Outcomes & retrospective

To be completed at the end of the migration.

## Context and orientation

Netsuke is a YAML-powered Ninja/Jinja build system written in Rust. Its
behavioural test suite lives in `tests/bdd_tests.rs`, which declares two
`scenarios!` invocations — one over `tests/features` and one, `#[cfg(unix)]`,
over `tests/features_unix`. The macro autodiscovers every `.feature` file and
generates one test per scenario. Step definitions live under `tests/bdd/steps/`
as attribute-macro functions; fixtures and the shared `TestWorld` live in
`tests/bdd/fixtures/mod.rs`; helpers in `tests/bdd/helpers/`.

The suite is a dev-dependency of the root `netsuke-build` package only. The
workspace has a second member, `test_support`, which does not use `rstest-bdd`,
and a standalone UI-test manifest at
`tests/ui/cli_configuration_pass/Cargo.toml` which declares `[workspace]` and
is not a member.

Key files for this migration:

- `Cargo.toml` — declares the `rstest-bdd` and `rstest-bdd-macros`
  dev-dependencies at lines 165-166, both required at version `0.5.0`, with the
  macros crate also enabling the `strict-compile-time-validation` feature.
- `Cargo.lock` — the single maintained lockfile; resolves the family at 0.5.0.
- `tests/bdd_tests.rs` — the `scenarios!` entry point.
- `tests/bdd/steps/mod.rs` and its 30 sibling modules — 185 step functions,
  41 lint expectations citing upstream issue #381.
- `docs/developers-guide.md` — section `## rstest-bdd v0.5.0 usage` at line
  4384 states the current pin and must be updated.
- `docs/contents.md` — the `docs/` index, listing the imported guides.

A *step function* is a function annotated `#[given]`, `#[when]`, or `#[then]`.
A *wrapper* is the function the step macro generates around it. A *fixture* is
an `rstest` value injected into a step or scenario by parameter name.

## Conformance basis

No Terms of Reference or technical design governs this dependency migration.
The governing upstream artefacts are:

- `leynos/rstest-bdd` tag `v0.6.0`, commit
  `72fb22635670e456545ca368805ba4c1c9d7bd69`.
- `docs/v0-6-0-migration-guide.md` at that commit
  (`sha256 6e76c10028f9962134f6158732cf1645ae1b0beba3018bccd3bc3e88b2c38985`).
- `docs/users-guide.md` at that commit
  (`sha256 1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2`).
- `docs/v0-5-0-migration-guide.md` at tag `v0.5.0`
  (`sha256 508d5b55cb2025915ced366b917a2327e572a6aa920fd290684b858b7afa2abc`),
  consulted because the starting version predates 0.5.0's documented changes.

Repository standards: `AGENTS.md` (code style, dependency management, testing,
documentation maintenance) and `docs/documentation-style-guide.md`.

Trace links:

- EP-M1 (import) → upstream guide checksums above → acceptance: `sha256sum`
  of the two `docs/rstest-bdd-*.md` destinations equals the upstream digests.
- EP-M2 (dependency) → `Cargo.toml:165-166`, `Cargo.lock` → acceptance:
  `cargo tree -p rstest-bdd` resolves 0.6.0 and `make test` compiles.
- EP-M3 (source reconciliation) → migration-guide breaking changes 1, 3, 4, 5
  → acceptance: `make lint` clean, inventory unchanged at 254.
- EP-M4 (documentation) → `docs/developers-guide.md:4384`,
  `docs/contents.md` → acceptance: `make markdownlint` clean.
- EP-M5 (validation) → `make test`, `make lint`, `make check-fmt`,
  `make doc-coverage`, `make typecheck` → acceptance: recorded log excerpts.

## Verification plan

### Obligations

- **INV-1 (coverage preservation).** The set of generated behavioural tests
  after migration equals the baseline set captured at `ebcedaef`. Method:
  `cargo nextest list --test bdd_tests` before and after, diffed as sorted name
  lists. Artefact: `/tmp/bdd-before.txt` (254 names) and the post-migration
  equivalent. Discharge: the diff is empty. Non-vacuity: the list is non-empty
  (254) and is produced by the same command in both runs, so a macro that
  silently generated nothing would be caught as a 254-line deletion rather than
  an empty-vs-empty match.

- **INV-2 (behaviour preservation).** Every scenario that passed before still
  passes, and any new failure is explained as a genuine defect the alias fix
  exposed rather than suppressed. Method: `make test` under nextest, compared
  against the baseline run. Discharge: identical pass/skip/fail counts, with
  every difference named. Non-vacuity: the suite exercises real subprocess
  behaviour — it shells out to Ninja and uses `assert_cmd` — so a green run
  reflects executed assertions, not a vacuous pass.

- **INV-3 (step error propagation).** A step returning `Err` from an
  `anyhow::Result` alias fails its scenario. Method: the alias-classification
  fix is upstream behaviour; this repository verifies it is *in effect* by
  observing that 0.6.0's type-directed classification is reached. Planned
  artefact: a focused regression check only if a plausible false-green is not
  already covered by an existing unhappy-path scenario (see `Decision log` when
  decided). Discharge: a deliberately failing step makes its scenario fail,
  then is reverted. Non-vacuity: a test that always fails, or one whose
  assertion never runs, proves nothing; the check must show the scenario
  *passing* without the injected `Err` and *failing* with it.

- **INV-4 (declared-versus-resolved agreement).** The declared requirement in
  `Cargo.toml` and the resolved version in `Cargo.lock` both denote 0.6.0, and
  no prerelease or duplicate family member remains. Method: `cargo tree` plus
  `rg 'name = "rstest-bdd' -A2 Cargo.lock`. Discharge: every family member at
  exactly 0.6.0; no `-beta`/`-alpha` strings.

- **INV-5 (MSRV soundness).** The declared `rust-version`, if any, and the
  pinned nightly both admit the resolved graph. Method: compile under the
  `rust-toolchain.toml` pin; inspect `cargo metadata` for `rust_version`
  fields. Discharge: the graph builds; any declared floor is consistent with
  the pin.

### Axioms

- The published `rstest-bdd` crates' documented interfaces behave as their
  source indicates. Verified against the extracted `.crate` archives at the
  checksums recorded in `Surprises & discoveries`; not re-verified internally.
- `cargo`'s resolver honors `--precise` for a targeted lockfile update.
- The repository's gate commands are as `AGENTS.md` documents.

### Methods and residual gaps

Parameterized and behavioural tests, plus the repository's own gates, cover
every obligation; no property, model-checking, or proof obligation is
introduced, because this change adds no new invariant to repository-owned logic
— it updates a dependency and reconciles call sites. The residual gap is that
upstream's own correctness is taken as an axiom, which is appropriate: the
migration verifies *this repository's* conformance to the dependency, not the
dependency's internals.

## Milestones

Each milestone ends in a coherent, validated state.

### EP-M1 — Import the authoritative documentation

End state: `docs/rstest-bdd-users-guide.md` and
`docs/rstest-bdd-v0-6-0-migration-guide.md` exist at the repository root's
`docs/` directory, byte-for-byte identical to upstream at
`72fb22635670e456545ca368805ba4c1c9d7bd69`, with provenance recorded here and in
`docs/contents.md`.

Acceptance: `sha256sum docs/rstest-bdd-users-guide.md` equals
`1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2`; the
migration guide's digest equals `6e76c100…8985`.

Recovery: re-copy from the pinned checkout; the step is idempotent.

### EP-M2 — Align the dependency

End state: `Cargo.toml` requires the 0.6.0 family with the existing
`strict-compile-time-validation` feature; `Cargo.lock` resolves every family
member to 0.6.0.

Acceptance: the workspace compiles; `cargo tree` shows 0.6.0; INV-4 holds.

Recovery: revert the manifest and run `cargo update -p <crate> --precise 0.5.0`
for each family member.

### EP-M3 — Reconcile source and lint expectations

End state: the suite compiles and lints clean under 0.6.0. Every one of the 41
issue-#381 expectations is either still fulfilled or removed with evidence.
`tests/bdd/steps/mod.rs` documentation matches reality.

Acceptance: `make lint-clippy` clean; INV-1 holds.

Recovery: each expectation removal is independently revertible.

### EP-M4 — Update repository documentation

End state: `docs/developers-guide.md` describes v0.6.0 usage;
`docs/contents.md` lists both imported guides with provenance.

Acceptance: `make markdownlint` and `make check-fmt` clean.

### EP-M5 — Validate and deliver

End state: the full gate set has run and been recorded; the branch is pushed
and a draft PR is open.

Acceptance: INV-1 through INV-5 discharged with recorded evidence; `make test`,
`make lint`, `make check-fmt`, `make typecheck`, and `make doc-coverage` all
pass; unavailable checks are named rather than claimed.

## Outputs and evidence

To be recorded as each milestone completes: the two imported checksums, the
before/after declared and resolved versions, the lint-expectation disposition
list, the baseline-versus-migrated inventory diff, and the gate log paths under
`/tmp`.

## Revision note

2026-09-26 — initial draft, written after reconnaissance established the
baseline and falsified the assumption that the 0.6.0 crate is materially larger
in scope than the guide's headline suggests. No revisions to earlier text.
