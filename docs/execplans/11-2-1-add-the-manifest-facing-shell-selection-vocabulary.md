# Add the manifest-facing shell selection vocabulary (11.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

The user approved this plan on 2026-10-13 and resumed implementation on
2026-10-17 after the rebase. The original CLI-fixture timeout repair in
[PR 913][cli-timeout-pr] merged into `main` as `077cf366`; package-verification
repair [PR 930][timeout-pr] merged on 2026-10-19 as
`051358ee360a97fcbebe2a836e25e106917ed845`. A later full run timed out the
module-slice fixture at its 720-second limit while its package-scoped Cargo
check remained active and nested rustc requests waited in healthy compiler
admission. Lower [PR 934][module-slice-timeout-fix-pr] raises only this test's
allowance to 900 seconds. Its final head, `0a6143c3`, passed all focused
documentation checks, CodeScene, the required CI jobs, Netsukefile Build Test,
and Release Dry Run. The final allocated-token reconciliation found no further
scoped documentation correction. The ready-state CodeRabbit review approved
that exact head without actionable comments. Its ready-state Release Dry Run
also completed successfully; only the PR-conditional publish job was skipped.
PR 934 targets `main`, and #778 targets that branch. The 19-commit feature
series has been rebased onto `0a6143c3` without conflicts; `range-diff` matches
every commit. The target-only timeout-table change is separate from the
feature's shell-selection section. All eight local gates passed sequentially on
upper head `b107341f`; the final ExecPlan status update still needs its
documentation-only checks and commit. The 26-file tolerance applies to this
feature diff and excludes the separate timeout repairs. The plan remains in
progress until the stacked PRs merge and post-merge state is verified.

[cli-timeout-pr]: https://github.com/leynos/netsuke/pull/913
[timeout-pr]: https://github.com/leynos/netsuke/pull/930
[module-slice-timeout-fix-pr]: https://github.com/leynos/netsuke/pull/934

## Purpose / big picture

Netsuke is growing a structured-command recipe form (RFC 0001) in which each
command block may say which shell, if any, interprets its `invoke` text.
ADR-019 and RFC 0011 fix the manifest contract: the block's `shell` field is
either a Boolean or a registry name. `false` (or an absent field) means direct
argument-vector (argv) execution with no shell, `true` means the host's
platform-default shell, and a string such as `bash` or `dash` names an
allow-listed shell.

This task adds the vocabulary for that contract as plain data types, and
nothing else. After the change a Netsuke developer can:

1. enumerate the four built-in shells (`sh`, `bash`, `pwsh`, `powershell`) and,
   for either host family (Unix-like or Windows), ask whether each is
   supported, which executable it names and whether that is an absolute path or
   a bare name, which fixed arguments precede the shell source, and which
   built-in `true` selects;
2. parse any string into a validated `ShellName` that accepts exactly the
   ADR-019 grammar `[a-z][a-z0-9_-]{0,62}` minus the eight reserved YAML 1.1
   Boolean spellings (`true`, `false`, `yes`, `no`, `y`, `n`, `on`, `off`), so
   configured names such as `dash` are representable while paths, upper-case
   text, templates, and Boolean spellings are not; and
3. decode a YAML or JSON `shell` value into a three-way `ShellSelection`
   (`Direct`, `PlatformDefault`, `Named(ShellName)`), rejecting every value
   that is neither a Boolean nor a valid name.

Nothing that Netsuke executes changes. No manifest accepts a `shell` field yet,
because the structured-command abstract syntax tree (AST) from RFC 0001 does
not exist; wiring happens in roadmap task 11.3.1. Success is observable through
the new unit, property, snapshot, doctest, and behavioural tests. Each new test
is shown to fail against a deliberately wrong stub and to pass against the real
implementation, while every existing test continues to pass unchanged.

Terms used throughout:

- _Built-in shell_: one of the four shells Netsuke itself knows about, listed
  in ADR-019 Table 1. Built-in names are reserved.
- _Configured shell_: a shell added by trusted operator configuration in task
  11.2.2. This task only makes its name representable.
- _Host family_: the coarse operating-system class that decides built-in
  support. There are exactly two: Unix-like and Windows.
- _Fixed arguments_: the interpreter arguments Netsuke places before the
  rendered shell source, for example `-c` for `sh`.
- _Front-end_: Netsuke's manifest loader, `evaluate_manifest` in
  `src/manifest/mod.rs`, which parses YAML into a `serde_json::Value` and later
  deserializes typed AST values from that value.
- _Build-script slice_: the handful of library source files that `build.rs`
  recompiles by `#[path]` to generate manual pages (`build.rs`, lines 30 to
  70). Any file in that slice must compile standalone without unused items.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- Do not change command execution. Do not modify `src/ast/`, `src/ir/`,
  `src/ninja_gen/`, `src/ninja_gen_recipe_shell.rs`, `src/runner/`,
  `src/manifest/`, `src/cli/`, `src/stdlib/`, `src/recipe_shell.rs`, or
  `build.rs`. The only permitted production edits are the new
  `src/shell_selection/` module and a single `pub mod shell_selection;` line in
  `src/lib.rs`.
- Do not wire the new types into any AST, IR, configuration, or command-line
  surface. Roadmap 11.2.1 says the types stay data-only until RFC 0001's AST
  exists (11.3.1), and `CliConfig` definitions belong to 11.2.2.
  `ShellSelection` must never appear in `src/ir/` or `src/ninja_gen/`, now or
  later: the IR carries only a resolved shell (ADR-019, "Lowering and execution
  intermediate representation").
- Do not read the process environment, the filesystem, or `PATH`. Executable
  resolution, `PATHEXT` normalization, and probing belong to 11.2.3. The only
  ambient input permitted is the compile-time target family (`cfg!(windows)`),
  read in exactly one function.
- Keep `src/shell_selection/name/mod.rs` free of `crate::` and `super::`
  imports; it may import only `std`, `serde`, and `thiserror`, because 11.2.2
  will need to add it to the build-script slice (Decision D1).
- Preserve ADR-019 exactly: the four built-in names, their supported hosts,
  executables, resolution kinds, fixed arguments, the `true` host mapping (`sh`
  on Unix-like, `powershell` on Windows), and the `ShellName` grammar with the
  reserved Boolean spellings of the 2026-09-24 addendum. Any further wish to
  diverge is an architecture deviation (see `Decision log`).
- Add no new dependency. `serde`, `serde_json`, `serde-saphyr`, and
  `thiserror` are existing runtime dependencies; `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `googletest`, `pretty_assertions`, `insta`, `proptest`,
  and `regex` are existing dev-dependencies (`Cargo.toml` `[dev-dependencies]`).
- Keep every file at or under 400 lines (AGENTS.md; Whitaker
  `module_max_lines`). Begin every module with a `//!` comment; give every
  function, method, enum variant, and struct or variant field a `///` comment
  (`missing_docs_in_private_items` is denied); put public-item examples in
  doctests.
- No in-process environment mutation in tests (AGENTS.md, ADR-008). This task
  needs no environment at all.
- Use en-GB-oxendict spelling (`-ize`, `-yse`, `-our`) in comments and
  documentation.
- Do not add a `-Z` Polonius or next-solver directive; the pinned toolchain
  enables both.
- Never commit a Red state; Red stubs exist only in the working tree.

## Tolerances (exception triggers)

Measure scope from frozen `077cf366`, the current remote target containing the
prerequisite squash landing, excluding the ExecPlan file itself. On 2026-10-14
the user directed the timeout repair into its own PR under #778; that
independent prerequisite is excluded from the feature diff. The approval record
remains `1747496f`, and the original combined candidate is preserved under
`refs/recovery/timeout-stack-20261014/original-head`.

- Scope: stop and escalate if implementation needs more than 26 changed files
  or more than 2,600 net added lines including tests, snapshots, and
  documentation, or if any file outside the permitted set in `Constraints`
  needs a production edit.
- Interface: stop and escalate if any existing public item's signature must
  change. Adding the new `pub mod shell_selection` is approved by this plan.
- Dependencies: stop and escalate if a new crate or a new feature on an
  existing crate appears necessary.
- Contract: stop and escalate if ADR-019, RFC 0011, or RFC 0001 appears wrong
  or ambiguous in a way that changes the types (for example, a built-in's
  argument list); record a proposed deviation in `Decision log` and set the
  status to `BLOCKED`.
- Iterations: stop and escalate if a gate still fails after three fix attempts
  for the same root cause. Lint findings listed in the Green checklist below
  are expected and do not count as attempts.
- Time: stop and escalate if a single milestone exceeds four hours of active
  work.
- Localization: stop and escalate if any gate (for example a Fluent audit or a
  Whitaker lint) demands localized text for the new error type; the plan
  deliberately defers localization (Decision D5).
- Behavioural harness: stop and escalate if the repository's first
  `Scenario Outline` cannot be bound by `scenarios!` after the EP-M2 spike.

## Risks

- Risk: YAML 1.1 Boolean spellings. The front-end uses `serde-saphyr` 1.2.0
  with default options, whose untyped path trims the scalar and then maps any
  ASCII case of `true`, `yes`, `y`, `on` to `true` and `false`, `no`, `n`,
  `off` to `false` before any typed deserializer runs. Unquoted `shell: yes`
  therefore means `PlatformDefault`, and `shell: n` means `Direct`. Severity:
  medium. Likelihood: certain. Mitigation: pin the behaviour with front-end
  parity tests, reserve the eight spellings as names so no quoted form can name
  a shell (Decision D8), document it in the design document, and hand the
  front-end question to 12.1.1; do not change the front-end here (Decision D7).
- Risk: quoted Boolean-looking names. Without a reservation, `shell: "true"`
  would decode to `Named("true")`, and an operator could define a shell called
  `on` that the unquoted `on` (a Boolean) would never select. Severity: medium.
  Likelihood: low. Mitigation: resolved by design. `ShellName` rejects the
  eight lower-case YAML 1.1 Boolean spellings (Decision D8, ADR-019 addendum of
  2026-09-24), which closes both the manifest and the configuration route; O7,
  O11, and the behavioural scenarios pin it.
- Risk: the new module drifts from ADR-019 Table 1. Severity: high (the table
  is an authority boundary). Likelihood: low. Mitigation: an exact per-shell
  `rstest` table plus an `insta` snapshot rendering the table from code.
- Risk: shared compiler admission can leave a nested Cargo check active beyond
  a narrow per-test timeout. Evidence: the rebased module-slice test reached
  720 seconds with rustc requests queued and no compiler diagnostics. Severity:
  medium. Likelihood: observed under contention. Mitigation: allow this test
  900 seconds, matching other cold nested builds and remaining below the
  960-second coverage-run cap; keep the override scoped and do not add blanket
  retries or alter compiler admission.
- Risk: a future fifth built-in is added to the enum but not to `ALL`.
  Severity: medium. Likelihood: low. Mitigation: a test compares `ALL` with a
  handwritten list and calls a helper whose exhaustive `match` fails to compile
  (E0004) for an unlisted variant. A fixed array length alone does not enforce
  this.
- Risk: lint waves. The workspace denies `must_use_candidate`,
  `missing_errors_doc`, `missing_const_for_fn`, `missing_docs_in_private_items`,
  `string_slice`, `indexing_slicing`, and `unwrap_used`, and Whitaker limits
  conditionals to two branches; `make lint` stops at the first failing
  sub-gate. Severity: low. Likelihood: high. Mitigation: the Green checklist in
  `Plan of work` pre-empts each known finding.
- Risk: Windows-only failure. `std::path::Path::is_absolute("/bin/sh")` is
  false on Windows, and CI runs `make test` on Windows. Severity: medium.
  Likelihood: medium. Mitigation: shape checks avoid `std::path` (O5).
- Risk: the repository's first `Scenario Outline`. `scenarios!` does not run
  the compile-time step validation that `#[scenario]` does, so a step typo
  surfaces only at run time. Severity: low. Likelihood: medium. Mitigation:
  pinned step patterns and a one-row spike first (EP-M2).
- Risk: build-script slice. Task 11.2.2 must compile `name/mod.rs` inside
  `build.rs`, where any unused inherent method becomes a dead-code error.
  Severity: medium for 11.2.2. Likelihood: medium. Mitigation: `name/mod.rs`
  stays import-free and minimal (Decision D1), and the hand-off names the risk.

## Progress

- [x] (2026-09-24) Renamed the branch to
  `11-2-1-add-the-manifest-facing-shell-selection-vocabulary` and set its
  upstream.
- [x] (2026-09-24) Reconnaissance of ADR-019, RFC 0011, RFC 0001, the roadmap,
  `RecipeShell`, `DependencyOrder`, `HostPattern`, the manifest front-end,
  `build.rs`, and the test infrastructure.
- [x] (2026-09-24) Drafted this ExecPlan.
- [x] (2026-09-24) Expert design review (structure and cost; contracts and
  alternatives; failure modes and viability). All three panels returned
  "proceed with conditions"; every condition is folded into this revision.
- [x] (2026-09-24) User review: agreed to reserve the eight YAML 1.1 Boolean
      spellings in `ShellName` itself in this task. Added the ADR-019 addendum
      of 2026-09-24, aligned RFC 0011 sections 4.1, 5.2, and 11, and revised
      this plan (Decision D8).
- [x] (2026-10-13) User authorized implementation by asking to use the
  `sdlc-implementation` skill for this plan.
- [x] (2026-10-13) `make fmt` passed for the plan update after the Markdown
  formatter's link-wrapping constraint was resolved with reference links.
- [x] (2026-10-13) Final clean-environment EP-M0 run passed `make check-fmt`,
  `make typecheck`, `make lint`, and `make doc-coverage`; `make test` failed on
  a 300-second timeout in `cli_configuration_fixture_compiles`. See the
  `-4.out` gate logs under `/tmp` and _Outcomes & retrospective_ below.
- [x] EP-M0: repaired pre-feature test baseline recorded on 2026-10-14;
  the original untouched-tree failure is preserved below.
- [x] (2026-10-14) Investigated and repaired the baseline timeout using the
  [CLI configuration debugging plan][cli-timeout-debugging], then validated the
  repaired test baseline before EP-M1.
- [x] (2026-10-14) Reproduced the original child Cargo command once: it
  succeeded after 23 minutes 37 seconds with 263 dependency build/check entries
  and observed admission waits. The isolated-target regression was observed red
  before switching the CLI fixture to the existing harness.
- [x] (2026-10-14) First corrected candidate passed workflow contracts,
      formatting,
  typechecking, lint, and documentation coverage (98.81%). Full tests exposed a
  second 300-second timeout in the build-module boundary fixture before the
  repaired CLI cases ran. H3 investigated its ambient Cargo preparation; that
  failed run did not establish a repaired baseline.
- [x] (2026-10-14) Matching all targets and workspace features alone did not
  clear the module-slice timeout: the next full run also observed empty Bash
  completion content. H4 traced the remaining Cargo command-mode mismatch
  before another gate run. No CLI fixture timing or green baseline is
  established by either failed validation run.
- [x] (2026-10-14) Latest full run passed both repaired CLI cases, the
  module-slice boundary, and completion readers: 3,369 tests passed, with
  packaging alone timing out at 300 seconds. An isolated verified packaging dry
  run then succeeded in 507.99 seconds. Two regression guards were observed red
  before adding a packaging-only 660-second allowance and relocating its fresh
  build target beneath workspace `target/`. The subsequent full test gate
  passed; feature implementation has not begun.
- [x] (2026-10-14) Repaired candidate passed formatting, 667 workflow
  contracts (two skipped), typechecking, lint, and documentation coverage
  (98.81%). Full Nextest passed all 3,370 tests (five skipped) in 1,051.448
  seconds; doctests passed 39 with six ignored. CLI cases took 15.715 and
  17.049 seconds, the module boundary 285.271 seconds, and packaging 385.821
  seconds. Logs use `-netsuke-11-2-1-20261014-9.out` under `/tmp`; typecheck
  uses the unchanged-Rust gate-8 log. Markdown validation initially saw the
  removed manifest in Git's index; staging the intended deletion resolved it.
  Formatting, spelling, Markdown lint, and Mermaid validation then passed in the
  `-20261014-10-doc.out` logs. Local review belongs to the repair delivery
  ledger and PR record.
- [x] (2026-10-14) User directed a separate prerequisite PR beneath #778.
  The fix targets `main`, and #778 targets the fix. The 34-file combined-scope
  proposal is withdrawn; the feature's own PR diff retains the 26-file and
  2,600-net-line tolerances. No shell vocabulary implementation is claimed.
- [x] (2026-10-17) Rebased onto prerequisite squash landing `077cf366` and
  published the clean pre-feature head `5b32a37f`. All baseline gates passed:
  4,080 Nextest tests (six skipped), 136 doctests (32 ignored), typechecking,
  lint, formatting, Markdown/spelling, Mermaid, and documentation coverage
  98.89% (5,074/5,131). Logs: `/tmp/*-netsuke-shell-rebase-20261017.out`.
- [x] (2026-10-19) Rebased the local feature branch onto current `origin/main`
  at `7d1e4c127d621501f3015b0f86b58c2d633e3f62` without conflicts. A fresh
  lower-branch `make test` then exposed
  `netsuke_build::build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  timing out at its 720-second limit: 4,057 passed, one timed out, six
  skipped, and 24 not run. Its nested `cargo metadata` child was observed
  waiting 2m46 on Cargo's shared package-cache lock; this is context, not a
  proven sole cause. The separate candidate narrows the direct-rustc fixture's
  preparation from all workspace targets/features to the owning package's
  library with all features. The focused test passed in 71.043 seconds on its
  first scoped run and 2.822 seconds warm on the final argument layout. Its
  first full gates passed `make check-fmt`, `make test` (4,082 passed, six
  skipped; 39 doctests passed, six ignored), and `make typecheck`. Package
  verification passed in 186.067 seconds and the module-slice fixture in
  122.127 seconds. `make lint` stopped on one Ruff composite-assertion finding
  in the new workflow-contract guard; that assertion is split and `make fmt`
  passes. The corrected lower candidate has diff hash
  `77e42c1ec1102d12cebff436d99748fc0b22cef01ea7afa49a18ad5e8c1015fb`; its
  complete sequential gates are restarting. Semantic review then corrected two
  stale Rust comments about the old workspace-wide selection. The final
  eight-path diff hash is
  `10fbcd50bd67681f0534d362115f3e5e81001aac20d155d5798c9933741dbb8e`. All eight
  gates then passed on that exact candidate: Nextest 4,082 passed/six skipped,
  doctests passed, typecheck and lint passed, doc coverage was 98.89%, workflow
  contracts passed 1,321 tests/three skipped plus 27 script tests, Markdown
  lint reported zero issues, and Mermaid validation passed. The module-slice
  test took 33.70 seconds and package verification 10.90 seconds. Commit
  `36f207d1` was published as draft [PR 930][timeout-pr] against `main` at base
  `7d1e4c12`; hosted CodeScene passed and hosted Actions are running. The seven
  feature commits were replayed onto `36f207d1` without conflicts; `range-diff`
  matches the published series exactly. The resulting #778 candidate still
  needs its own gates and remote stack update.
- [x] (2026-10-17) EP-M1 Red-Green evidence covers 91 cases, 15 doctests,
  both sensitivity unions, and all three O7 negative controls. FULL5's code and
  workflow gates plus FULL6's fresh Rust and documentation gates now pass on
  byte-identical non-plan files: 4,171 Nextest passes (six skipped), 151
  doctest passes (32 ignored), and documentation coverage 98.90%. Commit and
  exact-candidate local review were still pending at that point. No Red state
  is committed.
- [x] (2026-10-19) PR 930 merged into `main` as
  `051358ee360a97fcbebe2a836e25e106917ed845`. Rebased the feature branch onto
  that landing without conflicts; `range-diff` confirms the feature patches are
  unchanged. Candidate `56757f4417b9b0ed79d16d69d82e0d3f9bba3e15` passed,
  sequentially, `make check-fmt`, `make lint`, `make typecheck`, `make test`,
  `make doc-coverage`, `make test-workflow-contracts`, `make markdownlint`, and
  `make nixie`. Results: 4,173 Nextest passes (six skipped), 151 doctest passes
  (32 ignored), 1,324 workflow contract passes (three skipped) plus 27 script
  tests, 98.90% doc coverage, and zero Markdown issues across 186 files. Gate
  logs use the
  `/tmp/*-f900fc37-c6fe-4ed1-8700-ec572d9952c3-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-final56757f4.out`
  suffix. M1's exact-candidate local assessment, M2, and M3 remain open.
- [x] (2026-10-19) The first committed M1 local review covered all 13 changed
  paths on `84b7058e` against `051358ee` and found two overlapping major
  findings: fenced `command | tee` examples could mask a failed gate or allow a
  later passing command to hide it. Source and shell review confirmed the
  issue. Each runnable Bash block now enables fail-fast and `pipefail`, and
  docs gates passed on the correction. The exact-head reassessment confirmed
  that finding is resolved and identified one additional arithmetic edge in the
  near-miss name generator. That repair passed all eight sequential gates; the
- [x] (2026-10-19) Exact-head M1 reassessment on `1b741b82` against
  `051358ee` covered all 13 changed paths and returned zero findings; neither
  earlier review concern recurred. EP-M1 is complete.
- [x] EP-M1: host family, built-in registry, and `ShellName` landed with
  tests.
- [x] (2026-10-19) EP-M2's first one-row Scenario Outline binding spike passed
  its focused generated-scenario check as
  `features_scenarios::shell_selection_vocabulary_a_boolean_shell_selector_binds_to_its_intent::case_1`.
- [x] (2026-10-19) EP-M2 focused Red completed after correcting the
  `!!bool true` direct-decoder expectation from observed serde-saphyr
  behaviour. The reject-all pass ran 172 focused tests: 132 passed and 40
  failed on supported values, round trips, snapshots, and acceptance scenarios.
  The accept-all pass ran 172: 117 passed and 55 failed on unsupported values
  and rejection scenarios. Logs:
  `/tmp/focused-netsuke-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-m2-red1-corrected.out`
  and
  `/tmp/focused-netsuke-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-m2-red2.out`.
  Parser-only tagged-scalar cases passed their pinned stage-one expectations.
  Red stubs were transient; Green reviewed and accepted the selector error-text
  snapshot.
- [x] (2026-10-19) EP-M2 focused Green passes all 172 selected tests, including
  JSON/YAML round trips, the 1,000-sample kind and variant checks, both BDD
  outlines, and the full frontend parity table. Reviewed and accepted the
  selector error-text snapshot. Parser observations are pinned: `!custom yes`
  becomes the platform default; `!!bool true` fails both the generic-value
  stage and direct tagged-scalar deserialization; `.inf` fails the generic
  value stage. Log:
  `/tmp/focused-netsuke-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-m2-green.out`.
  Full gates are still required before closing EP-M2.
- [x] EP-M2: `ShellSelection` codec, parity tests, properties, and behavioural
  scenarios landed. All local gates passed again after restacking onto lower PR
  934; see the 2026-10-20 validation entry below.
- [x] (2026-10-20) Rebased onto `origin/main` at `f16c3176` with `zdiff3`. The
  three-way conflict at the shared insertion point in
  `docs/developers-guide.md` was resolved by preserving the upstream
  `CommandIsolation` paragraph and retaining the shell-selection subsection
  immediately after it. The design guide auto-merged. `range-diff` confirms the
  earlier feature commits are preserved; final updated-base gates remain
  pending.
- [x] (2026-10-20) Ran the post-rebase gates sequentially on local head
  `d4353301`. `make check-fmt`, `make typecheck`, and `make nixie` passed.
  `make test` reported 4,275 passes, then timed out
  `production_build_module_slice_has_expected_boundary` at 720.037 seconds; six
  cases were skipped and 24 were not run after fail-fast. Its child
  package-scoped Cargo check was still alive with compiler requests queued;
  `build-limits-status` reported healthy admission. `make lint` passed Cargo
  doc and Clippy, then Whitaker hit `No space left on device` while compiling
  `unic-langid-impl`. The workflow-contract, doc-coverage, and Markdown gates
  could not produce valid results under the exhausted filesystem. The logs are
  `/tmp/test-f900fc37-c6fe-4ed1-8700-ec572d9952c3-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-2.out`,
  `/tmp/lint-f900fc37-c6fe-4ed1-8700-ec572d9952c3-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-5.out`,
  and the corresponding `check-fmt`, `typecheck`, `test-workflow-contracts`,
  `doc-coverage`, `markdownlint`, and `nixie` logs in `/tmp`. At that point no
  timeout-policy change had yet been made; the follow-up below records the
  remedy and its validation.
- [x] (2026-10-20) Traced the 720-second failure to nested rustc requests
  waiting in healthy shared compiler admission while the package-scoped check
  remained active. Increased only this test's allowance from twelve to fifteen
  60-second periods, matching other cold nested builds and staying below the
  960-second coverage cap. Lower candidate `24aeeecf` passed all sequential
  local gates: format checks; 4,128 Nextest tests (six skipped) and 39 doctests
  (six ignored); typecheck; lint; 98.90% documentation coverage; 1,321 workflow
  contract tests (three skipped) plus 27 script tests; Markdown lint; and
  Mermaid validation. The module-slice test passed in 105.913 seconds; a
  separate nested Cargo metadata case passed in 503.359 seconds. Draft PR 934
  targets `main`. The 16 feature commits were replayed onto its head without
  conflicts; `range-diff` preserves every commit, and the local feature head is
  `9a57834d` before the plan update. The feature's post-rebase local gates now
  pass; its commit and remote update remain pending.
- [x] (2026-10-20) Revalidated the restacked feature candidate after updating
  this plan. `make fmt`, `make check-fmt`, `make test` (4,300 Nextest passes,
  six skipped; 39 doctest passes, six ignored), `make typecheck`, `make lint`,
  `make doc-coverage` (98.92%), `make test-workflow-contracts` (1,324 passes,
  three skipped, plus 27 script tests), `make markdownlint`, and `make nixie`
  all passed sequentially. The change-scope audit still shows only the
  feature's 23 paths; final commit, remote publication, exact-head review, and
  hosted checks remain outstanding.
- [x] (2026-10-20) Simplified the BDD shell-rejection assertion to remove
  duplicated outcome-check scaffolding while retaining both failure
  diagnostics. The full candidate gates passed on that source, and local
  CodeScene reports a 10.00 code-health score. Commit `94b12d8e` records the
  change; publication and hosted review remain outstanding.
- [x] (2026-10-20) Addressed the stale timeout-policy comments identified in
  [CodeRabbit's assessment](https://github.com/leynos/netsuke/pull/934#issuecomment-6283731132)
  with lower-PR commit `3d24441f`. The scoped allowance remains 900 seconds;
  the historical 720-second observation and 600-second mutation budget remain
  correctly described. All local gates passed, including 4,128 Nextest tests
  (six skipped), 39 doctests (six ignored), 98.90% doc coverage, and 1,321
  workflow-contract tests (three skipped) plus 27 script tests. The
  module-slice test passed in 130.729 seconds; hosted checks and the assessment
  follow-up are pending.
- [x] (2026-10-20) Corrected two further timeout-policy inaccuracies found
  during CodeRabbit's follow-up: only the compile-fail stub test has a 900-
  second override, and the guide has three distinct per-test allowances.
  Lower-PR commit `18ab026a` passes all eight local gates, including 4,128
  Nextest tests (six skipped), 39 doctests (six ignored), 98.90% doc coverage,
  and 1,321 workflow-contract tests (three skipped) plus 27 script tests. The
  module-slice test passed in 109.49 seconds. Hosted checks, CodeScene, and a
  focused assessment response remain pending.
- [x] (2026-10-20) Rebased the 19-commit feature series onto PR 934 head
  `3d24441f`, using `24aeeecf` as the recorded old base. There were no
  conflicts; `range-diff` matches all 19 commits, the 23-path feature diff is
  unchanged, and the lower PR's nextest policy remains in the base. The new
  feature head is `390d5f0e` before the pending M3 documentation commit.
- [x] (2026-10-20) Rebased again after lower PR 934 advanced to `18ab026a`,
  the remote head of the branch targeted by #778. The plan compared the
  target-only changes and predicted no overlap; Git confirmed a conflict-free
  replay, and `range-diff` matches all 19 commits. Recovery refs and the
  pre-rebase M3 patch were retained, and the patch checksum matches after
  restoration. The new feature head is `9bfb3646`; only the plan and roadmap
  remain uncommitted while final candidate gates run.
- [x] (2026-10-20) Corrected the last timeout-table ambiguity identified in
  CodeRabbit's reply on `18ab026a`: the 900-second list now names the
  stub-environment compile-fail check. Lower commit `0a6143c3` passes
  `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`, and
  `git diff --check`. CodeScene, all five CI jobs, Netsukefile Build Test, and
  the initial Release Dry Run passed on that exact head. The final allocated-
  token reconciliation confirmed the compile-fail qualifier resolves the
  finding and found no further scoped correction. After #934 became ready,
  CodeRabbit approved exact head `0a6143c3` with no actionable comments and
  passed all 15 pre-merge checks. Its ready-state Release Dry Run passed every
  completed successfully; only the PR-conditional publish-release job was
  skipped.
- [x] (2026-10-20) Compared the lower-only change from `18ab026a` to `0a6143c3`
  with the upper feature diff. It touches the timeout table near line 9609,
  while the feature's shell-selection section is near line 737. The
  `git merge-tree` result is clean; no lockfile changes are involved.
- [x] (2026-10-20) Replayed the 19 feature commits onto the live remote PR 934
  head `0a6143c3`, after verifying the tracking ref and `ls-remote` agreed. Git
  reported no conflicts. `range-diff` matches all 19 commits, and the feature
  diff retains its 23 paths with the lower timeout policy in the base. Recovery
  refs preserve the old head, old base, and target. The M3 patch was restored
  from stash and its SHA256 matches the saved binary diff. The new feature head
  is `b107341f`; all 19 feature commits remain unchanged.
- [x] (2026-10-20) Ran the full candidate gates sequentially on
  `b107341fd5244c03e7afe18fd3b440e175857b09`: `make check-fmt`, `make test`
  (4,300 passed, six skipped; 153 doctests passed, 45 ignored across both
  crates), `make typecheck`, `make lint`, `make doc-coverage` (98.92%, with 65
  validator tests passing), `make test-workflow-contracts` (1,324 passed, three
  skipped, plus 27 script tests), `make markdownlint` (187 files, zero issues),
  and `make nixie` all passed. The final ExecPlan status update is a
  documentation-only change; run `make fmt`, `make check-fmt`,
  `make markdownlint`, `make nixie`, and `git diff --check` sequentially before
  committing. Full gate logs are under `/tmp` with the candidate-specific `-7`,
  `-5`, `-5`, `-6`, `-5`, `-3`, `-6`, and `-6` suffixes, respectively.
- [ ] (2026-10-20) EP-M3 documentation closure is in the worktree: mark roadmap
  task 11.2.1 done and record the timeout and feature hand-offs. The full
  candidate gates passed before this final plan-status edit; the focused
  documentation gates and commit remain. Publish the upper PR, complete its
  exact-head assessment and hosted review, merge the stack, then set this plan
  to `COMPLETE` after post-merge verification.
- [ ] EP-M3: documentation, roadmap entry marked done, plan `COMPLETE` after
  the stacked PRs merge and post-merge state is verified.

## Surprises & discoveries

- Observation (2026-10-20): CodeRabbit's first completeness assessment found
  current-policy comments in `.config/nextest.toml` and
  `docs/developers-guide.md` that still described the former 720-second
  allowance. The runtime policy was already 900 seconds; the mismatch was
  documentation-only and is corrected by lower-PR commit `3d24441f`. The
  [follow-up assessment request](https://github.com/leynos/netsuke/pull/934#issuecomment-6284152518)
  led to two further corrections in `18ab026a`; the candidate-bound reply
  confirmed those passages and identified the same ambiguity in the timeout
  summary table. Commit `0a6143c3` fixes that final occurrence; focused
  documentation gates and hosted CI pass. The final focused reconciliation
  resolved the finding, and the ready-state formal review approved the exact
  head with no actionable comments. Its ready-state Release Dry Run completed
  successfully, with only the PR-conditional publish-release job skipped.
- Observation (2026-10-19): the first committed M1 local review found that
  validation examples piped through `tee` did not preserve gate failures.
  Bash's default pipeline status is the final command's status, and separate
  fenced blocks do not share shell options. The two reported findings are
  duplicate evidence for this one issue. Add fail-fast pipeline settings to
  each validation block. The exact-head reassessment confirmed the repair and
  found that the property generator's literal `64 - name.len()` relied on the
  current 63-character grammar bound. Derive the overlength target from
  `ShellName::MAX_LEN` and use saturating arithmetic so the negative-case
  generator remains safe if the grammar bound changes.
- Observation (2026-10-17): Clippy requires `mod.rs` for the new modules with
  child tests, beyond the workflow contract's prefix grouping. Production
  owners therefore move to `name/mod.rs` and `built_in/mod.rs`, preserving
  module identities, public signatures, and standalone name imports. Seven
  additional test-style diagnostics are repaired without changing their checks.
  Evidence: `/tmp/lint-netsuke-shell-m1-full1-20261017.out`.
- Observation (2026-10-17): current-main timeout policy differs from the
  historical repair measurements below: the module boundary has 720 seconds,
  isolated compile/stub/package tests 900 seconds, and the CI whole run 960
  seconds. The default remains 300 seconds. The fresh 4,080-test baseline
  passed under that current policy; no historical budget is being restored.
- Observation (2026-10-20): the scoped
  `cargo check --package netsuke-build --lib --all-features` in the
  module-slice fixture remained alive when Nextest terminated its 720-second
  allowance. The compiler supervisor was healthy with all four slots held and a
  large queue, and the child rustc wrappers were waiting for admission without
  compiler diagnostics. This identifies queue wait as the cause of this
  occurrence; it does not show a source failure. The follow-up raises only this
  fixture's allowance to 900 seconds, matching related cold builds below the
  960-second CI run cap. The lower candidate's full suite and all local gates
  pass; the fix is isolated in draft PR 934 while #778 is restacked above it.
- Observation (2026-10-17): O4's prose miscounted the unchanged support matrix;
  six of eight cells are true, two false. D6's old `Named("n")` example was
  obsolete after D8 reserved `n`; `Named("null")` is the valid scalar-like
  round-trip witness. Neither correction changes the approved contract.

- Observation (2026-10-19): rebasing onto current `origin/main` exposed a new
  timeout in the build-module boundary fixture before the package smoke test
  ran. The failing run had 4,057 passes, one timeout at 720 seconds, six
  skipped, and 24 not run. Its nested `cargo metadata` process waited 2m46 on
  the shared Cargo package-cache lock, but that observation alone does not
  establish the root cause. The lower-branch candidate now limits fixture
  preparation to the owning package's library targets with all features; the
  exact focused fixture passed in 71.043 seconds cold and 2.822 seconds warm.
  The first full test run on the scoped candidate passed, including 4,082
  Nextest cases and 39 doctests, but lint found a composite assertion in the
  regression contract. The assertion is split; all eight sequential local gates
  now pass on the final candidate. Package verification took 10.90 seconds and
  the module-slice fixture took 33.70 seconds. PR 930 is open as a draft against
  `main`; CodeScene passed and its GitHub Actions are still running. The
  rebased #778 candidate is undergoing its own validation.

- Observation: the untouched-tree test baseline timed out in a nested Cargo
  fixture. Evidence: `tests/command_env_ui_tests.rs` gives
  `cli_configuration_fixture_compiles` a dedicated
  `target/cli-configuration-ui` directory; nextest timed it out at 300 seconds
  with 3,348 of 3,364 tests passed. After termination, the target directory
  contained 492 MB, and the build supervisor still showed three held slots and
  queued requests. This supports, but does not prove, shared compile contention
  as the cause. The log is
  `/tmp/test-f900fc37-c6fe-4ed1-8700-ec572d9952c3-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-4.out`.
  At that point Stage A stopped before feature implementation. The timeout
  blocker was subsequently repaired in [PR 913][cli-timeout-pr], and
  implementation resumed; this observation records the failed historical
  baseline rather than current status.
- Observation: the CLI fixture's isolated Cargo project duplicates work
  that the existing public API harness can reuse. Evidence: the standalone
  manifest selects `target/cli-configuration-ui`, whereas `NetsukeRlib` builds
  the workspace library with the gate's feature selection and passes its
  reported artefacts to the fixture compiler. Impact: investigate that
  avoidable build before changing timeout policy. The controlled run took
  1,418.27 seconds and resolved 42 dependency versions absent from the
  workspace lockfile. Reusing the existing harness removes the independent
  graph and preserves compiler admission through `RUSTC_WRAPPER`.
- Observation: the pinned Python lint dependency initially used Lody's Git
  HTTPS helper even when `/usr/bin/git` was selected. Evidence: setting
  `GIT_EXEC_PATH=/usr/libexec/git-core` in the clean command environment made
  the public dependency fetch succeed; the final `make lint` passed. Impact:
  later Git-backed gates must use the same command-scoped helper path.
- Observation: manifests are not deserialized directly from YAML.
  Evidence: `evaluate_manifest` in `src/manifest/mod.rs` parses YAML into
  `ManifestValue` (`serde_json::Value`) with `serde_saphyr::from_str`, expands
  `foreach`, and only then calls `serde_json::from_value::<NetsukeManifest>`.
  Impact: the selector's decoder sees JSON value kinds. Tests decode through
  the same two stages, and YAML 1.1 Boolean coercion happens before the decoder
  runs (Risk 1).
- Observation: no stage renders Jinja inside arbitrary string values before
  typed deserialization. Expansion evaluates only `foreach` and `when` and
  writes only into `vars` (`src/manifest/expand/`); rendering runs on the typed
  AST afterwards. Evidence: design review (contracts panel) reading
  `src/manifest/expand/mod.rs` and `src/manifest/mod.rs`. Impact:
  `shell: "{{ x }}"` reaches the decoder as literal text and the name grammar
  rejects it. A `foreach` cannot vary the shell per item unless 11.3.1
  deliberately makes `shell` a rendered field.
- Observation: every Fluent key exists in all 35 locale catalogues under
  `locales/`, but no gate forbids literal `#[error("…")]` text; 44 such sites
  exist (for example `src/manifest/env_policy/mod.rs`). Evidence:
  `host_pattern.empty` appears in 35 `messages.ftl` files; the localization
  audit only compares declared keys with catalogues. Impact: deferring
  localization (Decision D5) is consistent with repository practice and trips
  no gate.
- Observation: `build.rs` recompiles a named slice of `src/cli/` and
  `src/host_pattern.rs`, and refuses module-wide dead-code suppression.
  Evidence: `build.rs`, lines 30 to 70. Impact: 11.2.2's `CliConfig` field will
  pull `ShellName` into that slice, which shapes `name/mod.rs` now (Decision
  D1).
- Observation: Netsuke has no host-platform type; `cfg!(windows)` is used
  directly, and `RecipeShell::host_default` is only testable on Windows.
  Evidence: `src/recipe_shell.rs`; the injected-Boolean precedent is
  `validate_recipe_shell_with(is_windows: bool, …)` in
  `src/runner/recipe_shell.rs`. Impact: Decision D2 introduces an injected
  host-family value so both host tables are tested on every platform.
- Observation: upstream `serde-saphyr` 1.2.0 source confirms the plain,
  untagged scalar path uses YAML 1.1 Boolean inference by default. Firecrawl
  retrieved `parse_yaml11_bool` from `src/parse_scalars.rs`, the
  `strict_booleans` default and documentation from `src/de/options.rs`, and the
  dispatch from `deserialize_any` in `src/de/deserializer.rs` on 2026-10-13.
  The parser trims and accepts ASCII case variants of `true`, `yes`, `y`, `on`,
  `false`, `no`, `n`, and `off`; the default for `strict_booleans` is `false`,
  and quoted scalars remain strings. This supports AXIOM-1 and the existing
  plan boundary; no front-end change is needed here. Sources:
  [parser][saphyr-parser], [options][saphyr-options], and
  [untyped deserializer][saphyr-deserializer].

[saphyr-parser]:
  <https://docs.rs/crate/serde-saphyr/1.2.0/source/src/parse_scalars.rs>
[saphyr-options]:
  <https://docs.rs/crate/serde-saphyr/1.2.0/source/src/de/options.rs>
[saphyr-deserializer]:
  <https://docs.rs/crate/serde-saphyr/1.2.0/source/src/de/deserializer.rs>
[cli-timeout-debugging]:
  ../debugging/debugging-plan-2026-10-14-cli-configuration-timeout.md

## Decision log

- Implementation refinement (2026-10-17): use a `pub(super)` tuple field for
  `ShellName` solely for the existing `From<BuiltInShell>` bridge and local
  `parse` construction. No public unchecked constructor or spare inherent
  method is added, and O2 exhaustively checks the finite unchecked names. This
  completes D1/D3 without introducing a reverse dependency or future
  build-script dead code. The initial interpretation that sibling test paths
  needed comments alone was disproved by the executable layout contract; D12
  records the corrective child-module layout.

- Decision D1: place the vocabulary in a new feature module
  `src/shell_selection/`, exported as `pub mod shell_selection` from
  `src/lib.rs`, rather than in `src/ast/`, `src/ir/`, `src/cli/`, or a
  `src/shell/` directory. Rationale: `ShellName` and `BuiltInShell` are shared
  by three later consumers (the configuration definitions in 11.2.2, the
  resolver in 11.2.3, and the AST in 11.3.1). Placing them in `src/ast/` would
  make `src/cli/` depend on the manifest AST, and ADR-019 keeps them outside
  the execution IR. `shell` alone would collide with the MiniJinja `shell()`
  helper and with `RecipeShell`. A leaf module mirrors `src/recipe_shell.rs`,
  which sits below both lowering and rendering. Items are `pub` because the AST
  that will carry `ShellSelection` is public and because the behavioural tests
  in `tests/` are an external crate. The crate is pre-1.0, so no compatibility
  promise attaches. Dependency rules: `src/shell_selection/` holds the
  vocabulary only; the 11.2.3 registry and resolver live in a sibling module
  (for example `src/shell_resolution/`) and take definitions, not `CliConfig`;
  `src/cli/` may depend on `src/shell_selection/`, never the reverse. Inside
  the module, `built_in/mod.rs` depends on `name/mod.rs`, never the reverse, so
  `From<BuiltInShell> for ShellName` lives in `built_in/mod.rs`. `name/mod.rs`
  imports only `std`, `serde`, and `thiserror`, so 11.2.2 can add it to the
  build-script slice by `#[path]` without dragging in the registry or the
  selector. Date/Author: 2026-09-24, planning agent; amended after design
  review.
- Decision D2: model the host family as `HostFamily { UnixLike, Windows }`
  (`shell_selection::HostFamily`) and pass it into every host-dependent query.
  `HostFamily::current()` is the single `cfg!(windows)` read. `HostFamily`
  lives in `built_in/mod.rs`, beside its only consumer. Rationale: RFC 0011
  section 11 requires host-independent table tests for every built-in and host
  rule, which the `RecipeShell` pattern (`cfg!` inside `host_default`) cannot
  provide. An enum rather than a Boolean follows AGENTS.md's newtype guidance,
  and the plan's glossary term "host family" gives the name. Non-Windows
  targets are Unix-like, matching `RecipeShell::host_default`. The target
  family is fixed at compile time, so it is not ambient runtime input of the
  kind ADR-008 governs and needs no `mockable` seam. Scope and reuse policy:
  `HostFamily` belongs to structured-command shell selection only. The 11.2.3
  resolver and 11.3.1 lowering may consume it; legacy `RecipeShell`, MiniJinja
  helpers, and the `which` subsystem must not adopt it without a separate
  decision. Date/Author: 2026-09-24, planning agent; renamed from `ShellHost`
  after design review.
- Decision D3: `BuiltInShell` exposes `ALL`, `name`, `lookup(&ShellName)`,
  `is_supported_on(HostFamily)`, `executable()`, `fixed_args()`, and
  `host_default(HostFamily)`, plus `Display`;
  `From<BuiltInShell> for ShellName` also exists. `BuiltInShell` has no
  `FromStr` and no serde implementation, and `ShellName` does not implement
  `Borrow<str>`. Rationale: all raw text enters through `ShellName::parse`, so
  there is one parser for the grammar. `lookup` answers "is this validated name
  a built-in?" with `Option`, which is exactly the "built-ins first, then
  configured" query 11.2.3 needs (ADR-019 addendum, commit `73c0c0bf`); it is
  written as a search of `ALL` comparing `name()`, so `name()` is the single
  spelling table. Manifests and configuration carry names, never
  `BuiltInShell`, so serde on it would add an unused second spelling.
  `Borrow<str>` would let a registry look names up by unvalidated text, undoing
  the single-parser rule. `executable()` returns
  `BuiltInExecutable::{AbsolutePath, BareName}`: these are the same two
  executable forms ADR-019 allows for configured shells, so 11.2.3 resolves
  both sources with one vocabulary. `powershell` is
  `BareName("powershell.exe")`; ADR-019 Table 1 says it resolves through
  "trusted host `PATH` and `PATHEXT`", and the resolver appends no `PATHEXT`
  suffix to an executable that already has an extension. This plan carries data
  only and leaves that step to 11.2.3. Date/Author: 2026-09-24, planning agent;
  amended after design review.
- Decision D4: implement the `ShellName` grammar with a handwritten validator
  that walks `text.chars()`; use the `regex` crate only in tests, as an
  independent oracle. Rationale: `regex` is a dev-dependency only, adding it at
  runtime would breach the dependency tolerance, and precedent
  (`src/host_pattern.rs`) validates by hand. An independent oracle makes the
  property test meaningful rather than a restatement. Parsing is exact: no
  trimming, no case folding. Error precedence is fixed so every input has one
  deterministic error:
  1. `Empty` for the empty string;
  2. `TooLong { length }` when the UTF-8 byte length exceeds 63, checked
     before any character is examined so the work is bounded (every accepted
     name is ASCII, so bytes equal characters there);
  3. `InvalidFirstCharacter { character }` when the first character is not `a`
     to `z`;
  4. `InvalidCharacter { character, char_index }` for the first later character
     outside `a` to `z`, `0` to `9`, `_`, and `-`, where `char_index` counts
     characters from zero; and
  5. `ReservedWord { word }` when the whole grammatical name equals one of the
     eight reserved YAML 1.1 Boolean spellings (Decision D8). `word` is the
     `&'static str` from the reserved list, so the error stays bounded. The
     check runs last because every reserved word already satisfies the grammar.
  Character classes use `matches!(c, 'a'..='z' | '0'..='9' | '_' | '-')`, which
  satisfies Whitaker's two-branch conditional limit. Date/Author: 2026-09-24,
  planning agent; precedence added after design review.
- Decision D5: `ShellNameError` is a typed `thiserror` enum whose variants carry
  structured, bounded data, with a plain English `Display` for developers.
  Localization is deferred to the serde adapters (`ShellName`'s `Deserialize`
  implementation and the `ShellSelection` visitor) when the first consumer that
  shows the error to a user lands: configuration loading (11.2.2) or manifest
  loading (11.3.1). Rationale: ADR-019 requires typed internal classification
  converted once to a localized diagnostic at a boundary, following ADR-005.
  Serde's `Error::custom` reduces any error to text, so the only place the
  typed error is still available is inside the adapters; they are therefore the
  conversion boundary, and a later task changes them to emit a
  `LocalizedMessage` while `ShellNameError` stays unlocalized. No user can
  trigger this error until 11.2.2 or 11.3.1, and the right wording depends on
  context those tasks own. Adding Fluent keys now would cost 35 catalogue edits
  for text nobody sees, and no gate requires it (see
  `Surprises & discoveries`). The `Display` strings and the visitor's
  `expecting` text are pinned by an `insta` snapshot so any interim consumer
  sees a stable contract. The error never echoes the whole input, so
  diagnostics stay bounded. Date/Author: 2026-09-24, planning agent; boundary
  clarified after design review.
- Decision D6: `ShellSelection` implements `Deserialize` by hand through
  `deserialize_any` and a `Visitor` that accepts only Booleans and strings, and
  implements `Serialize` symmetrically (`Direct` to `false`, `PlatformDefault`
  to `true`, `Named` to its string). `Default` is `Direct`. Rationale: a private
  `#[serde(untagged)]` helper with `try_from` could map the two Booleans, but
  untagged deserialization replaces the precise cause with "data did not match
  any variant". A visitor gives serde's standard
  `invalid type: …, expected a Boolean or a shell name` for every other kind
  and passes the `ShellNameError` text through for invalid strings. Explicit
  `null` is rejected rather than treated as absent: `#[serde(default)]` applies
  only when the key is missing, and a bare `shell:` is a YAML null. `Serialize`
  is needed because `NetsukeManifest` derives it (`src/ast/mod.rs`) and because
  OrthoConfig merging in 11.2.2 goes through `serde_json::to_value`
  (`src/cli/merge/mod.rs`); `serde-saphyr`'s serializer quotes YAML 1.1 Boolean
  spellings, so `Named("null")` round-trips. Date/Author: 2026-09-24, planning
  agent; rationale corrected after design review.
- Decision D7: pin, but do not change, the front-end's YAML 1.1 Boolean
  coercion. Rationale: the coercion applies to every Boolean field in every
  manifest; changing `serde-saphyr` options is a manifest-wide compatibility
  decision that roadmap 12.1.1 (contract consolidation) owns. This task records
  the exact mapping, including tagged scalars and case variants, in tests and
  in the design document so 12.1.1 decides with evidence. Date/Author:
  2026-09-24, planning agent.
- Decision D8: `ShellName` rejects the eight lower-case YAML 1.1 Boolean
  spellings, `true`, `false`, `yes`, `no`, `y`, `n`, `on`, and `off`, with the
  typed error `ReservedWord { word }`. This is an approved narrowing of the
  ADR-019 name grammar, recorded as the ADR-019 addendum of 2026-09-24 and
  mirrored in RFC 0011 sections 4.1, 5.2, and 11. Rationale: the front-end
  reads the unquoted spellings as Booleans (Decision D7), so a quoted spelling
  naming a shell would make one word mean two things. `ShellName` is shared by
  manifest selectors and configured definitions, so one rule in the type closes
  both routes: a quoted `"true"` fails when the manifest loads, with a
  diagnostic that can point the author at the unquoted Boolean, and no operator
  can define a shell with a Boolean-looking name. This replaces the first
  draft's hand-off to 11.2.2, which would have fixed only the configuration
  route and failed late. Upper-case and mixed-case spellings already fail the
  grammar, and no built-in name is reserved. The reserved list is a private
  constant in `name/mod.rs` that `parse` uses, so it adds no dead code to the
  11.2.2 build-script slice; no public Boolean-spelling helper is added. `null`
  is deliberately not reserved: an unquoted `null` fails closed as an invalid
  type, so it cannot misroute. Deviation record: affects `ADR019-GRAMMAR`, O2,
  O7, O8, O10, O11, O12, and the behavioural scenarios; requires the ADR-019
  addendum and the RFC 0011 alignment, both included in this pull request;
  approved by the user in plan review on 2026-09-24. Date/Author: 2026-09-24,
  planning agent with user approval; replaces the draft decision not to reserve.
- Decision D9: verification uses `rstest` tables for the finite registry,
  `proptest` for the grammar and codec invariants, two `insta` snapshots (the
  rendered registry table, and the error and `expecting` text), and
  `rstest-bdd` scenarios for the decoding contract. It uses neither Kani nor
  Verus. Rationale: the registry is four variants by two hosts, which a table
  enumerates exhaustively. The grammar is a regular language whose best
  available independent check is a regular-expression engine; a Kani harness
  would compare the validator against a second handwritten predicate, which
  restates the property rather than proving it. The validator has no indexing,
  arithmetic, or `unsafe` code, so bounded model checking adds no memory-safety
  evidence. No lemma or contractual business logic is introduced that needs an
  unbounded proof, and Verus is outside every gate under the 4.1.3 scope
  boundary. The host-default behavioural scenario from the first draft is
  dropped: it was a pure function already covered by O4 and the snapshot, and
  it added two fields to the shared `TestWorld`. Date/Author: 2026-09-24,
  planning agent; amended after design review.
- Decision D10: documentation targets. Record decisions D1 to D8, including
  the YAML 1.1 Boolean table, in a new `docs/netsuke-design.md` section 5.3
  subsection, "Structured-command shell selection vocabulary", placed
  immediately before "Legacy recipe shell selection". Document the internal
  interface, the dependency rules, and the `HostFamily` scope and reuse policy
  in `docs/developers-guide.md` under "Command and recipe lowering". List
  `src/shell_selection/` in `docs/repository-layout.md`. Make no
  `docs/users-guide.md` change, because users can observe no behaviour; 11.3.3
  owns user guidance. Add no `docs/polonius.md` entry, because the only
  borrow-returning accessors (`as_str`, `fixed_args`) are trivial field or
  static borrows with no lookup or get-or-create shape. Add no new ADR: the
  reserved-word narrowing is recorded as an ADR-019 addendum (already written
  with this plan), and the other decisions refine ADR-019 without changing it.
  Date/Author: 2026-09-24, planning agent.

- Decision D11: separate the independently authorized timeout repair into
  prerequisite PR #913, targeting `main`, with #778 targeting that
  prerequisite. This follows the user's explicit direction on 2026-10-14. The
  original combined candidate is preserved at `1d237e19`; only the plan and
  design changes belong to this upper PR. Scope is measured on this PR's
  exclusive diff, preserving the original 26-file and 2,600-net-line limits.
  The combined 34-file ceiling proposal is withdrawn. Date/Author: 2026-10-14,
  implementation supervisor; user-authorized stack separation.

  Follow-up (2026-10-19): after rebasing to then-current `origin/main`, a
  distinct 720-second timeout appeared in the build-module slice fixture. Per
  the user's renewed direction to isolate the cause and keep the fix below
  #778, the repair was published as [PR 930][timeout-pr] against `main`. Its
  local gates and CodeScene passed; hosted Actions were still running at the
  time of publication. PR 930 merged on 2026-10-19. The seven feature commits
  were replayed onto its head without conflicts, with the published patch
  series preserved exactly. PR #913 remains the earlier, separate CLI-fixture
  repair and is not evidence for this distinct timeout.

  Follow-up (2026-10-20): the post-rebase full suite again reached the
  module-slice fixture's 720-second cap while its scoped Cargo check remained
  active and rustc requests waited for healthy admission. Keep the existing
  source and package scope; raise only this test's allowance to 900 seconds,
  matching other cold nested builds below the 960-second coverage-run cap. The
  lower candidate passed all local gates and is draft
  [PR 934][module-slice-timeout-fix-pr] against `main`; #778 is being restacked
  on its branch.

- Decision D12 (2026-10-17): place production modules in `name/mod.rs` and
  `built_in/mod.rs`, with tests in their plain child-module directories. The
  rebased-main workflow contract rejects repeated first prefixes even when a
  `#[path]` rationale is documented. Add only the exact root exception
  `{shell_selection/, shell_word.rs}` because public selection data and private
  POSIX word tokenization are unrelated feature boundaries. Test exact-set
  acceptance, added-member rejection, and stale missing-member rejection. The
  parent accepted this mechanical layout refinement within the existing 26-file
  and 2,600-net-line ceilings. No dependency, public signature, production
  ownership, or ADR contract changes. Run workflow contracts at each applicable
  full milestone gate. The Rust test and snapshot module identities remain
  unchanged; only source paths and snapshot source metadata move. The first
  full Clippy gate also requires `mod.rs` for modules with children, so the
  production owners move into those directories. `name/mod.rs` keeps its
  standalone imports and future build-script role; no public API changes.

## Outcomes & retrospective

The original CLI configuration timeout was repaired in the separate
prerequisite PR #913. Its fresh validation passed 3,934 Nextest tests (six
skipped), 129 doctests (32 ignored), 1,271 workflow contracts (three skipped),
and 27 script tests. Formatting, typechecking, lint, documentation coverage
(98.87%), spelling, Markdown lint, and Mermaid validation passed. CLI cases
completed in 9.849 and 5.959 seconds; the module boundary took 151.141 seconds
and packaging took 167.123 seconds. That evidence remains valid for its
candidate, but it predates current `origin/main` at `7d1e4c12`.

The 2026-10-19 rebase exposed a separate module-slice timeout on the new base.
The lower candidate scopes direct-rustc fixture preparation to the library each
fixture consumes and lets package verification inherit Cargo's build
configuration, retaining verification while reusing matching dependency
artefacts. All eight local gates passed on commit `36f207d1`: Nextest ran 4,082
tests (six skipped), doctests passed, typecheck and lint passed, doc coverage
was 98.89%, workflow contracts passed 1,321 tests (three skipped) plus 27
script tests, Markdown lint reported zero issues, and Mermaid validation
passed. The module-slice fixture took 33.70 seconds and package verification
10.90 seconds. At publication, draft PR 930 targeted `main`, its CodeScene
check had passed, and hosted Actions were still active; PR 930 then merged on
2026-10-19. The feature commits were replayed onto its head without conflicts,
but #778's updated head still needs its own gates and hosted review. The
earlier combined candidate's 3,370-test result and EP-M1's FULL5/FULL6 results
remain historical evidence. The feature's exclusive diff retains its approved
file and line limits, and the timeout repair stays in its separate lower PR.

The feature rebase onto `f16c3176` was conflict-reviewed and preserved the
feature commit series. Its first final gate sequence did not reach equilibrium:
the module-slice fixture timed out at 720.037 seconds while the healthy
compiler-admission pool was busy, and the later lint and documentation gates
were affected by filesystem exhaustion. Formatting, typechecking, and Mermaid
validation passed, but the remaining candidate gates and exact-head hosted
review were outstanding at that point. The remote PR had not yet been updated;
the later restack and validation are recorded below.

The earlier combined candidate passed 3,370 Nextest tests and 39 doctests;
these are historical results, distinct from the prerequisite's fresh run. That
historical candidate contained no shell vocabulary. EP-M1 now implements the
host family, fixed built-in registry, and validated names, with two complete
Red passes, focused Green, and three meaningful negative controls. All required
pre-commit gates pass using FULL5 and FULL6 receipts on unchanged non-plan
files. FULL6 passed 4,171 Nextest tests (six skipped) and 151 doctests (32
ignored), including the unchanged fixture that timed out in FULL5. These
results do not establish the exact historical timeout cause or imply a source
repair. The combined file-count blocker is resolved by separating the two PR
scopes; the feature's exclusive diff retains its approved limits.

On 2026-10-19, PR 930's module-slice repair merged as
`051358ee360a97fcbebe2a836e25e106917ed845`, and the feature was rebased onto
that landing without conflicts. All eight candidate gates passed sequentially on
`56757f4417b9b0ed79d16d69d82e0d3f9bba3e15`: 4,173 Nextest tests passed (six
skipped), 151 doctests passed (32 ignored), workflow contracts passed 1,324
cases (three skipped) plus 27 script tests, documentation coverage was 98.90%,
and format, type, lint, Markdown, and Mermaid checks passed. The candidate
still needed its exact-head local review and the EP-M2 codec and EP-M3
documentation milestones. The eight passing gates established the rebased M1
candidate's quality; the subsequent repair and clean exact-head assessment
close EP-M1 without marking roadmap 11.2.1 complete.

The committed M1 review follow-up confirmed both overlapping `tee` findings
were fixed. It also found that the near-miss generator hard-coded 64 when
constructing an overlength name. That branch now derives its target from
`ShellName::MAX_LEN` and uses saturating arithmetic. The implementation and
plan edit passed all eight sequential gates on the same source candidate:
check-fmt, lint, typecheck, test, doc-coverage, workflow contracts, Markdown
lint, and Nixie. Nextest passed 4,173 tests (six skipped); Netsuke doctests
passed 110 (26 ignored) plus two compile-fail cases; test-support doctests
passed 39 (six ignored); workflow contracts passed 1,324 (three skipped) plus
27 script tests; documentation coverage was 98.90%, and Markdown lint found
zero issues. Logs use the
`/tmp/*-f900fc37-c6fe-4ed1-8700-ec572d9952c3-11-2-1-add-the-manifest-facing-shell-selection-vocabulary-review-followups.out`
suffix. Compilation proceeded uncached because managed sccache was
unavailable; admission remained healthy. CodeRabbit CLI v0.9.0 then assessed
all 13 paths on `1b741b828a390bfab9891e1c896b417a7c8239fe` against
`051358ee360a97fcbebe2a836e25e106917ed845` and returned zero findings. The two
earlier `tee` findings and the arithmetic finding did not recur. EP-M1 is
closed; EP-M2 and EP-M3 remain open.

The original untouched-tree run is retained as historical evidence. Its clean
Git environment run passed `make check-fmt`, `make typecheck`, `make lint`, and
`make doc-coverage` (98.81%, 4,728/4,785). Nextest reported 3,348 tests passed,
one timeout, and five skipped; it also warned that 15 tests were not run after
the failure.
`netsuke-build::command_env_ui_tests::cli_configuration_fixture_compiles` timed
out at 300 seconds. Doctests did not run. The failure evidence is in the
`-4.out` logs under `/tmp`, especially the test log named in _Surprises &
discoveries_. No product source changes preceded that run. The user
subsequently authorized repairing this prerequisite; the repaired baseline does
not turn that original red run into a pass.

On completion, record here the delivered API, the gate results, and these
hand-offs:

- to 11.2.2: add `src/shell_selection/name/mod.rs` to the build-script slice by
  `#[path]`, as `host_pattern` is, and confirm no inherent method is dead
  there; place the reserved-name check where it does not pull `built_in/mod.rs`
  into the slice; localize `ShellNameError` inside `ShellName`'s `Deserialize`
  implementation, naming the configuration key and definition index. Configured
  names inherit the reserved YAML 1.1 Boolean spellings from `ShellName`
  automatically; 11.2.2 adds only the built-in-name reservation and the
  uniqueness check;
- to 11.2.3: build the resolver in a sibling module, not inside
  `shell_selection`; consume `BuiltInExecutable` and `HostFamily`; `BareName`
  entries have no path separator, and `powershell.exe` already carries an
  extension, so no `PATHEXT` suffix is appended to it;
- to 11.3.1: add `#[serde(default)] pub shell: ShellSelection` to the command
  block and `pub use` the type from `src/ast/`; localize the selector's errors
  inside its visitor, with a `ReservedWord` diagnostic that suggests the
  unquoted Boolean; avoid `#[serde(untagged)]` on the enclosing command-item
  type, or it will swallow the selector's precise errors; decide explicitly
  whether `shell` is a rendered field (as things stand a `foreach` cannot vary
  it per item, and users must be told); and keep `ShellSelection` out of the IR
  and out of the action plans versioned by 12.3.1, so `true` never hashes
  identically across hosts that resolve it differently;
- to 12.1.1: decide whether the manifest front-end should adopt strict Boolean
  parsing, given the pinned YAML 1.1 mapping.

The 2026-10-20 timeout follow-up was delivered separately as PR 934, targeting
`main`. Its original head, `24aeeecf`, raised the production module-slice
allowance from 720 to 900 seconds and passed local and hosted gates; the test
completed in 105.913 seconds in that run. CodeRabbit's completeness assessment
found stale current-policy comments, corrected in `3d24441f`, then identified
two more documentation inaccuracies corrected in `18ab026a`. All eight local
gates pass on `18ab026a`: 4,128 Nextest tests (six skipped), 39 doctests (six
ignored), 98.90% doc coverage, and 1,321 workflow-contract tests (three
skipped) plus 27 script tests. The module-slice test completed in 109.49
seconds. On docs-only head `0a6143c3`, focused documentation gates passed,
CodeScene and CI passed, the final CodeRabbit reconciliation found no further
scoped correction, and the formal review approved the exact head. Its
ready-state Release Dry Run completed successfully on 2026-10-20, with only the
PR-conditional publish-release job skipped.

The 19-commit feature series has been rebased onto lower head `0a6143c3`, using
`18ab026a` as the immediate old base. No conflicts occurred; `range-diff`
matches every commit, the lower timeout changes remain in the base, and the
feature diff retains its 23 paths. The rebased head `b107341f` passed all eight
local gates sequentially: 4,300 Nextest tests (six skipped), 153 doctests (45
ignored across both crates), 98.92% documentation coverage, 1,324 workflow
contracts (three skipped) plus 27 script tests, zero Markdown issues, and all
diagrams valid, along with typecheck and lint. Only the final ExecPlan and
roadmap status edits remain uncommitted; their documentation-only gates are
being completed before publication. The earlier 4,300-test candidate and local
CodeScene score of 10.00 remain historical evidence for their own heads.
Publish the validated upper branch, complete exact-head assessment and hosted
review, merge the stack, and verify post-merge state before marking this plan
complete.

## Context and orientation

Netsuke is a Rust build-system front-end that compiles a YAML manifest (a
Netsukefile) into a Ninja build file. The crate is both a library
(`src/lib.rs`) and a binary. Relevant files:

- `docs/adr-019-structured-command-shell-selection.md` is the governing
  decision. Table 1 lists the built-ins; the addendum separates `BuiltInShell`
  from `ShellName` and gives resolution precedence to built-ins.
- `docs/rfcs/0011-allow-listed-structured-command-shells.md` sections 4, 5.1,
  6.1, and 11 give the schema, the registry, the AST sketch, and the test
  strategy.
- `docs/rfcs/0001-structured-command-blocks.md` section 10.1 amends the
  `shell` field to `Boolean | ShellName`; section 17.1 sketches
  `ShellSelection` and `BuiltInShell`.
- `src/recipe_shell.rs` holds `RecipeShell`, the legacy recipe-wide
  interpreter: a data-only leaf module with a `host_default` constructor. It is
  not reused here (ADR-019 forbids sharing the registry with legacy recipes),
  but its placement and style are the model.
- `src/ast/dependency_order.rs` holds `DependencyOrder`, a small serde-derived
  manifest enum with a documented YAML example. It shows the AST-to-IR
  boundary: the IR owns a separate value and converts from the AST value in
  `src/ir/from_manifest.rs`.
- `src/host/pattern.rs` holds `HostPattern`, the best precedent for a validated
  string newtype: `parse` with a `# Errors` section, `TryFrom<&str>`,
  `TryFrom<String>`, `FromStr`, and a `Deserialize` that reads a `String` and
  maps a parse error with `serde::de::Error::custom`.
- `build.rs`, lines 30 to 70, shows the build-script slice.
- `src/manifest/mod.rs`, function `evaluate_manifest`, is the manifest
  front-end.
- `src/snapshot_test_support.rs` provides `snapshot_settings(subdir)`, which
  roots snapshots at `src/snapshots/<subdir>/`; every existing snapshot
  directory uses it.
- `tests/bdd_tests.rs` is the single behavioural harness. It calls
  `rstest_bdd_macros::scenarios!("tests/features", fixtures = [world:
  TestWorld])`,
  so a new `.feature` file under `tests/features/` is discovered
  automatically, and generated tests are named `<feature_stem>_<scenario>`.
  Step definitions live in `tests/bdd/steps/`, and each new step module must be
  declared in `tests/bdd/steps/mod.rs`. Scenario state lives on `TestWorld` in
  `tests/bdd/fixtures/mod.rs`, in `rstest_bdd::Slot<T>` fields.
- `tests/integration_test_wiring_tests.rs` fails if a test module tree is not
  wired into a Cargo test target.
- CI runs the test gate on Linux and Windows (`.github/workflows/ci.yml`,
  `.github/workflows/ci-windows.yml`).

Skills and documents to consult while implementing:

- skills: `execplans` (this format), `rust-router` then `rust-types-and-apis`
  (newtypes, enum API shape) and `rust-unit-testing` (rstest, googletest,
  pretty_assertions, insta), `proptest` (strategies and non-vacuity),
  `rust-verification` (why Kani and Verus are not used here),
  `hexagonal-architecture` (keep the vocabulary a pure domain leaf with no
  adapter dependencies), `codegraph-mcp` (confirm no caller changes), and
  `en-gb-oxendict` for prose;
- documents: `docs/netsuke-design.md` section 5.3,
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
  (Scenario Outline, Examples, and placeholders),
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md` (why the host
  family is injected), `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/adr-005-typed-which-resolve-error.md`,
  `docs/adr-008-environment-seam-taxonomy.md`, `docs/whitaker-users-guide.md`,
  `docs/documentation-style-guide.md`, and `docs/ortho-config-users-guide.md`.
  OrthoConfig is not used by this task: configuration-layer shell definitions
  are 11.2.2's scope, and nothing here is layered configuration.

Prior art confirms the shape of the registry. GitHub Actions' `shell` keyword
offers a finite built-in set (`bash`, `sh`, `pwsh`, `powershell`, `cmd`,
`python`) with fixed invocation templates and a host-dependent default, and
lets workflow authors supply a custom `command [options] {0}` template.[^1]
ADR-019 deliberately withholds that last capability from manifests and gives it
to trusted operator configuration instead. None of the common alternatives
(Dockerfile exec and shell forms, `just`'s recipe-wide `set shell`, Taskfile's
embedded interpreter) lets one manifest Boolean switch between direct and shell
execution, so the three-way selector is Netsuke-specific.

## Conformance basis

The original design basis was `dcac7634`. Implementation began against the
frozen remote target `077cf366` after rebase to `5b32a37f`; the local branch
has since been rebased onto current `origin/main` `7d1e4c12` as `1619d474`.
Current-main orientation changes are mechanical: `HostPattern` is in
`src/host/pattern.rs` and re-exported as `crate::host_pattern`; the new public
module precedes private `shell_word` in `src/lib.rs`. Owned tests now use plain
child modules to satisfy current-main's executable layout contract; production
filenames and Rust module identities are unchanged.

Upstream artefacts, rechecked against `077cf366`:

- `docs/roadmap.md` task 11.2.1 (and its successors 11.2.2, 11.2.3, 11.2.4,
  and 11.3.1 for scope boundaries).
- ADR-019 (Accepted, 2026-09-02) including both 2026-09-02 addenda and the
  2026-09-24 addendum reserving YAML Boolean spellings, which this pull request
  adds.
- RFC 0011 (Proposed) sections 4.1, 5.1, 5.2 (name grammar only), 6.1, 9, and
  11, as aligned with the 2026-09-24 addendum by this pull request.
- RFC 0001 (Proposed) sections 10.1 and 17.1.
- ADR-005 (typed internal errors converted once) and ADR-008 (environment
  seam taxonomy), as governing standards.
- AGENTS.md code-style, testing, and documentation rules.

There are no Terms of Reference or technical-design revision identifiers beyond
these documents. Traced items:

- `RM-11.2.1-A`: finite `BuiltInShell` with `sh`, `bash`, `pwsh`,
  `powershell`.
- `RM-11.2.1-B`: validated `ShellName` retaining configured names such as
  `dash`.
- `RM-11.2.1-C`: host availability, fixed arguments, parsing, and host-default
  mapping.
- `RM-11.2.1-D`: Boolean-or-name selection with three intent-preserving
  variants; every other YAML type rejected.
- `RM-11.2.1-E`: data-only; no AST wiring; command execution unchanged.
- `ADR019-T1`: ADR-019 Table 1 (names, hosts, executables, resolution, fixed
  arguments) and the `true` mapping.
- `ADR019-GRAMMAR`: `[a-z][a-z0-9_-]{0,62}`, excluding the eight reserved YAML
  1.1 Boolean spellings (addendum of 2026-09-24).

Trace chains:

```plaintext
RM-11.2.1-A, ADR019-T1 -> EP-M1
  -> built_in tests: built_in_names_are_exactly_the_adr_019_set, all_lists_every_variant
RM-11.2.1-B, ADR019-GRAMMAR -> EP-M1
  -> name tests: accepts_*, rejects_*, error_precedence
  -> name_property_tests: parse_agrees_with_regex_oracle, name_round_trips
RM-11.2.1-C, ADR019-T1 -> EP-M1
  -> built_in tests: support_matrix, fixed_invocations, host_default_*
  -> snapshot: built_in_registry_table
RM-11.2.1-D -> EP-M2
  -> selection tests: decodes_*, rejects_*, front_end_parity
  -> selection_property_tests; snapshot: error_texts
  -> tests/features/shell_selection_vocabulary.feature
RM-11.2.1-E -> EP-M1, EP-M2
  -> scope check (git diff); no ShellSelection in src/ir or src/ninja_gen; unchanged make test
```

## Verification plan

The change introduces a finite registry, a regular-language validator, and a
decoding function from JSON value kinds to intents. The obligations below cover
them. Each states why a wrong implementation would fail it.

### Axioms

External assumptions, not verified here:

- AXIOM-1: `serde-saphyr` 1.2.0 with default options, when deserializing an
  untyped value from a plain scalar, trims it with Rust's Unicode `trim()`
  (which also strips U+00A0), then returns a Boolean for any ASCII case of
  `true`, `yes`, `y`, `on`, `false`, `no`, `n`, and `off` (`parse_scalars.rs`,
  `parse_yaml11_bool`). Quoted scalars stay strings; plain `~`, `null`, and
  empty scalars become null; tags change the path (`src/de/deserializer.rs`,
  `deserialize_any`). The parity tests exercise this boundary against the real
  crate rather than assuming it.
- AXIOM-2: `serde_json::Value`'s `deserialize_any` calls `visit_bool` for
  Booleans, the string visitors for strings, `visit_unit` for null, number
  visitors for numbers, `visit_seq` for arrays, and `visit_map` for objects.
  Netsuke enables `serde_json`'s `preserve_order` feature but not
  `arbitrary_precision`.
- AXIOM-3: the `regex` crate correctly decides membership of
  `\A[a-z][a-z0-9_-]{0,62}\z`; `\z` is a true end-of-text anchor.
- AXIOM-4: `cfg!(windows)` is true exactly when compiling for a Windows target.
- AXIOM-5: ADR-019 Table 1 is the authoritative registry.

### Obligations

#### O1: finite and complete built-in vocabulary

`BuiltInShell::ALL` contains each variant exactly once, and the set of `name()`
values is exactly `{sh, bash, pwsh, powershell}`.

- Method: parameterized test plus a compile-time exhaustiveness guard; the
  domain has four elements, so enumeration is exhaustive.
- Artefact: `src/shell_selection/built_in/tests.rs`,
  `built_in_names_are_exactly_the_adr_019_set` and `all_lists_every_variant`.
  The latter compares `ALL` with the handwritten list
  `[Sh, Bash, Pwsh, PowerShell]` and calls, for each element, a test helper
  whose `match` has no wildcard arm and returns the expected name.
- Non-vacuity: every variant is a witness. Removing a variant from `ALL` fails
  the list comparison; adding a variant without updating the helper fails to
  compile with E0004, which is the mechanism that forces the test to be
  revisited.

#### O2: built-in names are valid registry names

For every `b` in `ALL`, `ShellName::parse(b.name())` succeeds and equals
`ShellName::from(b)`. This also shows that no built-in name is a reserved word.

- Method: parameterized test. `From<BuiltInShell> for ShellName` constructs
  without re-validation; this lemma justifies that.
- Artefact: `built_in/tests.rs`, `built_in_names_satisfy_the_grammar`.
- Non-vacuity: a misspelt name such as `Bash` in `name()` fails the parse.

#### O3: lookup round trip and exclusivity

`lookup(&ShellName::from(b)) == Some(b)` for every built-in, and
`lookup(&n) == None` for every valid name outside the four.

- Method: parameterized test for the first half; property test for the
  second.
- Domain: generated valid names from the grammar, filtered to exclude the four
  built-in names (the filter rejects about one sample in 62,000, driven by
  `sh`), plus explicit rows `dash`, `zsh`, `cmd`, `sh2`, `shh`, and `bashx`.
- Artefact: `built_in/tests.rs` and
  `src/shell_selection/name/property_tests.rs`,
  `lookup_is_none_for_non_built_in_names`.
- Non-vacuity: `dash` is a mandatory witness named by the roadmap. A `lookup`
  that matches on prefix fails the `sh2` and `bashx` rows.

#### O4: host support and defaults match ADR-019

The support matrix is exactly `sh` on Unix-like only; `bash` and `pwsh` on both;
`powershell` on Windows only. `host_default(UnixLike) == Sh`,
`host_default(Windows) == PowerShell`, and `host_default(h).is_supported_on(h)`
for both hosts.

- Method: parameterized 4-by-2 truth table and an `insta` snapshot.
- Artefact: `built_in/tests.rs`, `support_matrix`, `host_default_*`, and
  snapshot `built_in_registry_table`, which renders every row of Table 1 (name,
  hosts, executable kind and value, fixed arguments) from code. It is stored at
  `src/snapshots/shell_selection/netsuke__shell_selection__built_in__tests__built_in_registry_table.snap`
  through `snapshot_settings("shell_selection").bind(…)`.
- Non-vacuity: all eight cells are asserted, six true and two false.
  Flipping any cell fails exactly one case.

#### O5: fixed invocations

Executables and arguments match ADR-019 exactly: `sh` is
`AbsolutePath("/bin/sh")` with `["-c"]`; `bash` is `BareName("bash")` with
`["--noprofile", "--norc", "-c"]`; `pwsh` is `BareName("pwsh")` and
`powershell` is `BareName("powershell.exe")`, both with
`["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]`. Every fixed
argument is non-empty and NUL-free; every `AbsolutePath` starts with `/`; no
`BareName` contains `/` or `\`. The shape checks deliberately avoid `std::path`
and `MAIN_SEPARATOR`, whose answers differ on Windows.

- Method: parameterized test with `pretty_assertions::assert_eq` on whole
  slices.
- Artefact: `built_in/tests.rs`, `fixed_invocations` and
  `invocation_shapes_are_well_formed`.
- Non-vacuity: argument order is asserted, so swapping `--norc` and
  `--noprofile` fails.

#### O6: host family tripwire

On the compiling host, `HostFamily::current() == HostFamily::Windows` if and
only if `RecipeShell::host_default() == RecipeShell::PowerShell`.

- Method: named unit test on every CI platform.
- Artefact: `built_in/tests.rs`,
  `current_host_agrees_with_legacy_recipe_default`.
- Non-vacuity: both sides read `cfg!(windows)`, so this is an alignment
  tripwire against a future change to either default rather than independent
  evidence. CI exercises the Linux and Windows sides; a local run exercises
  one. This is a recorded residual gap.

#### O7: grammar

For every string `s`, `ShellName::parse(s)` is `Ok` if and only if `s` matches
`\A[a-z][a-z0-9_-]{0,62}\z` and is not one of the eight reserved YAML 1.1
Boolean spellings, and on success `as_str() == s`.

- Method: property test against an oracle made of the `regex` crate plus the
  reserved list written out independently in the test, plus a boundary and
  precedence table.
- Domain: four generators combined with `prop_oneof!`:
  1. valid names from
     `proptest::string::string_regex("[a-z][a-z0-9_-]{0,62}")`, filtered to
     exclude the reserved words (about one sample in 800 is rejected, mostly
     `y` and `n`);
  2. near misses built by mutating a valid name, where every mutation must
     change the string: upper-case one _letter_ position (index 0 is always a
     letter); prefix a digit, `_`, or `-`; insert one of `/`, `\`, `.`, a
     space, NUL, `é`, or `{` using `String::insert` at a character boundary;
     extend to 64 characters; or replace the name with one of the eight
     reserved words;
  3. printable ASCII strings of length 0 to 70; and
  4. arbitrary Unicode strings.
- Artefact: `name/property_tests.rs`, `parse_agrees_with_regex_oracle`, and
  boundary cases in `src/shell_selection/name/tests.rs`: empty; lengths 1, 63,
  and 64; each forbidden first character; `dash`, `/bin/bash`, `bash.exe`,
  `C:\Windows\System32\bash.exe`, `{{ shell }}`, `bash` with a leading space,
  and `Bash`; each of the eight reserved words (rejected); and near neighbours
  that stay valid: `yess`, `no-op`, `on1`, `nn`, `offline`, `truex`, and
  `null`. Precedence rows in `error_precedence`: 70 characters with an
  upper-case letter at position 2 gives `TooLong`; 70 copies of `é` gives
  `TooLong { length: 140 }`; `Bash` gives `InvalidFirstCharacter('B')`; `é`
  gives `InvalidFirstCharacter('é')`; and `ba/sh` gives
  `InvalidCharacter { character: '/', char_index: 2 }`; `true` gives
  `ReservedWord { word: "true" }`; and `True` gives
  `InvalidFirstCharacter('T')`, because the grammar check precedes the
  reservation.
- Evidence: default 256 cases per property; failures persist under
  `proptest-regressions/`.
- Non-vacuity: a deterministic companion test, written as
  `#[test] fn … -> Result<(), String>` following
  `src/ir/cycle_property_tests.rs`, builds
  `proptest::test_runner::TestRunner::deterministic()` and draws 1,000 samples
  from each generator with `strategy.new_tree(&mut runner)?.current()`
  (importing `proptest::strategy::ValueTree`). It asserts that the valid
  generator yields only accepted strings, the near-miss generator yields only
  rejected strings, and the printable-ASCII generator yields both classes.
  Valid strings make up only about 0.6% of that last generator, so "both
  classes" is seed-dependent; the test's comment says so. Negative controls
  exercised during Red-Green: changing the length limit to 64 fails the
  64-character boundary case; allowing upper case fails `Bash`; dropping one
  word from the reserved list fails that word's row.

#### O8: name round trips

`ShellName::parse(n.as_str()) == Ok(n)`, and JSON and YAML
serialize-then-deserialize return `n`, for every valid `n`.

- Method: property test, plus explicit rows for `null`, `nan`, `inf`, `yess`,
  and `no-op`: valid names that sampling reaches rarely and that a YAML
  serializer might leave unquoted and a parser might then infer as another type.
- Artefact: `name/property_tests.rs`, `name_round_trips` and
  `scalar_like_names_round_trip`.
- Non-vacuity: the generator is O7's valid-name strategy, whose acceptance is
  established there; the explicit rows fail if the serializer stops quoting.

#### O9: selector decoding is exact

Decoding a JSON value `v` yields `Direct` if `v` is `false`, `PlatformDefault`
if `v` is `true`, `Named(n)` if `v` is a string for which `ShellName::parse`
gives `n`, and an error in every other case (invalid-name strings, null,
numbers, arrays, objects).

- Method: parameterized test over every JSON kind, and a property test over
  generated `serde_json::Value`s.
- Domain: a recursive `Value` strategy of depth at most 2 covering all six
  kinds, with strings drawn from O7's generators and numbers from `i64` and
  finite `f64` ranges (`Number::from_f64` rejects NaN and infinities).
- Artefact: `src/shell_selection/selection/tests.rs` (`decodes_*`,
  `rejects_*`) and `selection/property_tests.rs`,
  `decoding_is_ok_exactly_for_booleans_and_valid_names`.
- Non-vacuity: the property compares against an oracle computed independently
  from the value's kind and the regex, and a deterministic companion (as in O7)
  asserts every kind appears. A decoder that treats `null` as `Direct` fails
  the null case; one that accepts numbers fails the `1` case.

#### O10: selection round trip

`decode(encode(s)) == s` for every selection, through `serde_json` and through
`serde-saphyr`.

- Method: property test plus O8's explicit scalar-like names wrapped in `Named`.
- Artefact: `selection/property_tests.rs`, `selection_round_trips`.
- Non-vacuity: the generator produces all three variants, which the
  deterministic companion asserts.

#### O11: front-end parity and YAML 1.1 pinning

Each YAML input is placed in mapping position, as `shell: <text>`, and decoded
into a test-only wrapper `Probe { #[serde(default)] shell: ShellSelection }`
both through the front-end's two stages (`serde_saphyr::from_str::<Value>` then
`serde_json::from_value::<Probe>`) and directly with
`serde_saphyr::from_str::<Probe>`. An empty mapping `{}` checks the absent-key
default. The test uses `serde_json::Value` directly, not
`crate::manifest::ManifestValue`, so the module keeps no test link to
`manifest`.

- Method: parameterized test with one row per input. Each row states the
  result for each path; where the paths agree the row asserts parity, and where
  they differ the row pins both and the difference is recorded under
  `Surprises & discoveries` without any production change.
- Artefact: `selection/tests.rs`, `front_end_parity`.
- Rows:

  | Input                                                                           | Expected result                |
  | ------------------------------------------------------------------------------- | ------------------------------ |
  | absent key (`{}`)                                                               | `Direct`                       |
  | `false`, `False`, `no`, `n`, `N`, `off`, `oFf`                                  | `Direct`                       |
  | `true`, `TRUE`, `yes`, `yEs`, `y`, `Y`, `on`, `ON`                              | `PlatformDefault`              |
  | `yes` followed by U+00A0 (no-break space)                                       | `PlatformDefault`              |
  | `bash`, `dash`                                                                  | `Named` with the text          |
  | `"bash"`, `'dash'`, `"null"`                                                    | `Named` with the unquoted text |
  | `"true"`, `"false"`, `'yes'`, `"n"`, `"off"`                                    | error (reserved word)          |
  | bare `shell:`, `~`, `null`, `Null`, `1`, `0x10`, `""`, `[bash]`, `{name: bash}` | error                          |
  | `Bash`, `/bin/bash`, `"{{ shell }}"`                                            | error                          |

  _Table: O11 inputs and expected results._

  Tagged-scalar characterization rows are confirmed against the real crates at
  Red and then pinned: `!!str true` (expected error, reserved word), `! yes`
  (non-specific tag, expected error, reserved word), `!!str bash` (expected
  `Named("bash")`), `!custom yes` (expected `PlatformDefault`), `!!bool true`
  (expected to fail at stage one), and `.inf` (expected to fail at stage one).
- Non-vacuity: every row is a witness with a concrete expected value, and both
  paths run the real crates (AXIOM-1, AXIOM-2).
- Observable substitute (2026-10-17): a row demonstrably rejected at stage one
  by the external YAML parser cannot reach either repository-owned visitor
  stub. Such parser-only characterization rows may pass both Red runs; record
  their exact stage-one failures independently instead of adding unrelated
  assertions to manufacture sensitivity. Every repository-owned codec row still
  requires a meaningful Red failure. This distinction applies to `!!bool true`
  and `.inf` if confirmed against the real crate.

#### O12: stable error and `expecting` text

The `Display` of each `ShellNameError` variant and the decoder's error for a
number are pinned.

- Method: `insta` snapshot, because error wording is multivariant output whose
  consistency matters to later consumers (Decision D5).
- Artefact: `selection/tests.rs`, `error_texts`, stored under
  `src/snapshots/shell_selection/`.
- Non-vacuity: the snapshot contains one line per variant, including
  `ReservedWord`, plus the `expected a Boolean or a shell name` text, so a
  change to any of them fails.

#### O13: no execution change

No file outside the permitted set in `Constraints` changes; `ShellSelection` is
absent from `src/ir/` and `src/ninja_gen/`; `name/mod.rs` has no `crate::` or
`super::` import; and the full existing suite passes.

- Method: the scope and import checks in `Concrete steps`, and `make test`.
- Non-vacuity: `make test` runs the existing command-lowering and Ninja
  snapshot suites.

### Residual gaps

O6 exercises only one side per host. No Kani or Verus obligation exists
(Decision D9). Localization of errors is deferred (Decision D5). Broader
cross-cutting properties over merged definitions and `PATH` states belong to
11.2.4.

## Plan of work

### Stage A: baseline (no code changes)

Confirm the working tree is clean and on the task branch. Run the gates once to
record a green baseline; if any gate fails on the untouched tree, stop and
report it, because later failures would otherwise be ambiguous.

On 2026-10-14 the user authorized repairing the baseline timeout. Record that
test-harness repair separately, run the full gates on the repaired pre-feature
tree, and require a green result before EP-M1. The original failure and its
diagnostic evidence remain in this plan and the debugging record; do not
present the repaired baseline as the original untouched run.

The 2026-10-17 green M0 run is historical for its then-current tree. The
2026-10-19 rebase onto `origin/main` `7d1e4c12` exposed a separate module-slice
timeout. The narrowed lower candidate now passes all local gates and is
published as draft PR 930 against `main`, with hosted Actions still running.
The local feature branch is replayed on that lower head; validate its own
candidate before publishing or treating PR 778 as ready.

### EP-M1: host family, built-in registry, and names

File layout under `src/shell_selection/`:

- `mod.rs`: module comment, `mod` declarations, and `pub use` lines.
- `name/mod.rs`: `ShellName` and `ShellNameError`; declares plain test-gated
  `mod tests;` and `pub(super) mod property_tests;`, with sources under `name/`.
- `built_in/mod.rs`: `HostFamily`, `BuiltInExecutable`, `BuiltInShell`, and
  `From<BuiltInShell> for ShellName`; declares test-gated `mod tests;`, with
  source `built_in/tests.rs`.

Add `pub mod shell_selection;` to `src/lib.rs` immediately after
`pub mod runner;` and before private `mod shell_word;`, keeping alphabetical
order.

Red, first pass (reject-everything stubs): write the tests for obligations O1
to O8. Write each production item with its final signature and bodies that
return a deliberately wrong value: `lookup` returns `None`, `parse` returns
`Err(ShellNameError::Empty)`, `is_supported_on` returns `false`, and
`fixed_args` returns `&[]`. Name unused parameters with a leading underscore
(`_text`), because the gate flags turn `unused_variables` into errors. Run the
focused tests and record that the acceptance-side tests (`accepts_*`,
`fixed_invocations`, `support_matrix` true cells, properties, lookup round
trip) fail on their assertions.

Red, second pass (accept-everything stubs): switch the stubs so `parse` wraps
any text, `lookup` returns `Some(Sh)`, and `is_supported_on` returns `true`.
Record that the rejection-side tests (`rejects_*`, `error_precedence`, the
exclusivity half of O3, `support_matrix` false cells) now fail. Any test that
passes under both stubs is vacuous and must be fixed before Green.

Green: implement the bodies as specified in `Interfaces and dependencies`,
applying this lint checklist as you go:

- `#[must_use]` on `current`, `name`, `lookup`, `is_supported_on`,
  `executable`, `fixed_args`, `host_default`, `as_str`, and `into_inner`,
  placed after the doc comment (Whitaker `function_attrs_follow_docs`);
- a `# Errors` section on `ShellName::parse`;
- `///` on every variant and every error field;
- `const fn` wherever Clippy's `missing_const_for_fn` asks (including
  `as_str`);
- `matches!` for character classes;
- no `&s[..]` slicing or `[i]` indexing, even in tests (use `char_indices`,
  iterators, and `String::insert`);
- `expect` only inside `#[test]` bodies, never in helpers, and no `unwrap`;
- `Result`-returning doctests ending with `Ok::<(), ShellNameError>(())` or
  hidden `fn main() -> Result<…>` lines per `docs/rust-doctest-dry-guide.md`.

Write the registry snapshot test with
`crate::snapshot_test_support::snapshot_settings("shell_selection").bind(||
insta::assert_snapshot!("built_in_registry_table", rendered))`,
review the pending snapshot against ADR-019 Table 1, and accept it.

Refactor: remove duplication, confirm every file is under 400 lines, rerun the
focused tests, then the gates. Commit.

### EP-M2: selection codec and behaviour

Add `src/shell_selection/selection/mod.rs` (`ShellSelection`, its `Serialize`,
and its visitor) with child modules `selection/tests.rs` and
`selection/property_tests.rs`, declared with plain test-gated `mod` as in EP-M1.

Spike: before writing the whole feature, add only the first outline with one
Examples row and its step module, run the behavioural command in
`Concrete steps`, and confirm the scenario is generated and bound. This is the
repository's first `Scenario Outline`; if it cannot be bound, stop and escalate
(Tolerances).

Red: write obligations O9 to O12 and the full feature file. Run two stub passes
as in EP-M1: a visitor that rejects everything with a `Serialize` that always
writes `false`, then a visitor that maps every string to `Named` without
validation and every other kind to `Direct`. Record which tests fail under each.

Green: implement the visitor. `expecting` writes `a Boolean or a shell name`;
`visit_bool` maps `false` and `true`; `visit_str` calls `ShellName::parse` and
maps errors with `E::custom` (serde's defaults route `visit_string` and
`visit_borrowed_str` here). Every other visitor method keeps serde's default,
which reports `invalid type`. `Serialize` writes a Boolean or the name. Confirm
the tagged characterization rows of O11 and pin them.

Behavioural specification, `tests/features/shell_selection_vocabulary.feature`:

```gherkin
Feature: Structured-command shell selection vocabulary
  A structured command's shell selector records whether the author wants
  direct execution, the host's default shell, or a named shell. Decoding the
  selector never resolves or runs a shell.

  Scenario Outline: A Boolean or a valid name decodes to the author's intent
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then the shell selection is <intent>

    Examples:
      | yaml   | intent                  |
      | false  | direct execution        |
      | true   | the platform default    |
      | bash   | the named shell "bash"  |
      | dash   | the named shell "dash"  |
      | "bash" | the named shell "bash"  |

  Scenario Outline: Any other selector, including a quoted Boolean spelling, is rejected
    Given the shell selector YAML <yaml>
    When the shell selector is decoded
    Then decoding the shell selector fails

    Examples:
      | yaml      |
      | 1         |
      | null      |
      | [bash]    |
      | /bin/bash |
      | Bash      |
      | ""        |
      | "true"    |
      | "yes"     |
```

Step definitions live in a new `tests/bdd/steps/shell_selection.rs`, declared in
`tests/bdd/steps/mod.rs`, with these pinned patterns so that no generic
placeholder can shadow a literal step:

- `#[given("the shell selector YAML {yaml}")]`, a generic placeholder with no
  `:string` type, so cells such as `"true"` and `""` keep their quotes and
  reach the YAML parser verbatim;
- `#[when("the shell selector is decoded")]`;
- `#[then("the shell selection is direct execution")]`;
- `#[then("the shell selection is the platform default")]`;
- `#[then("the shell selection is the named shell {name:string}")]`; and
- `#[then("decoding the shell selector fails")]`.

The `When` step builds the document `shell: <yaml>` and decodes it through the
same two stages as the front-end into a small wrapper struct defined in the
step module. Add two fields to `TestWorld` in `tests/bdd/fixtures/mod.rs`:
`shell_selector_yaml: Slot<String>` and
`shell_selection_outcome: Slot<Result<ShellSelection, String>>`. Steps return
`anyhow::Result<()>` and resolve failures with `?`, not `expect`, because
helpers outside `#[test]` bodies may not panic; an infallible step that still
returns `Result` carries `#[expect(clippy::unnecessary_wraps, reason = "…")]`,
following `tests/bdd/steps/accessibility_preferences.rs`.

Refactor, then run the gates and commit.

### EP-M3: documentation and closure

Write the documentation described in Decision D10. Mark roadmap task 11.2.1 as
done by changing `- [ ] 11.2.1.` to `- [x] 11.2.1.` in `docs/roadmap.md`.
Update this plan's `Progress`, `Surprises & discoveries`,
`Outcomes & retrospective`, and status. Run `make fmt` immediately after each
Markdown edit, then the Markdown gates and the full gates. Commit.

## Milestones and plateaus

- EP-M0, baseline. Outcome: a recorded green repaired pre-feature baseline,
  preserving the untouched-tree failure and separately authorized repair.
  Requirements: none. Acceptance: every gate log exits zero. Conformance check,
  recovery, and compatibility: not applicable. Remaining gaps: everything.
- EP-M1, host family, built-in registry, and names. Outcome: `HostFamily`,
  `BuiltInShell`, `BuiltInExecutable`, `ShellName`, and `ShellNameError` exist
  with doctests, unit tests, the registry snapshot, and grammar properties.
  Requirements: `RM-11.2.1-A`, `-B`, `-C`, `-E`; `ADR019-T1`, `ADR019-GRAMMAR`.
  Acceptance: obligations O1 to O8 pass; gates green. Conformance check: Table
  1 unchanged; no file outside the permitted set touched; `name/mod.rs` has no
  `crate::` or `super::` import; no dependency added; `HostFamily` scope
  recorded; trace links current. Recovery: revert the milestone commit to
  return to the baseline. Remaining gaps: the selection codec. Compatibility
  decision: none (new, pre-1.0 items).
- EP-M2, selection codec and behaviour. Outcome: `ShellSelection` decodes and
  encodes exactly, with front-end parity, properties, error-text snapshots, and
  behavioural scenarios. Requirements: `RM-11.2.1-D`, `-E`. Acceptance: O9 to
  O13 pass; the two outlines pass in `tests/bdd_tests.rs`; gates green.
  Conformance check: the selection type is not referenced from `src/ast/`,
  `src/ir/`, or `src/ninja_gen/`; YAML 1.1 behaviour is pinned rather than
  altered; trace links current. Recovery: revert the milestone commit; EP-M1
  remains a coherent plateau. Remaining gaps: documentation. Compatibility
  decision: none.
- EP-M3, documentation and closure. Outcome: design, developer, and layout
  documents describe the vocabulary; the roadmap marks 11.2.1 done; the plan is
  `COMPLETE`. Requirements: AGENTS.md documentation rules. Acceptance: Markdown
  gates and the full gates green. Conformance check: documentation agrees with
  ADR-019; hand-offs recorded. Recovery: revert the documentation commit.
  Remaining gaps: the hand-offs listed in `Outcomes & retrospective`.
  Compatibility decision: none.

## Concrete steps

Run every command from the repository root. Capture gate output with `tee` and
read the log rather than the truncated terminal. Never run two gates at once;
the build cache relies on sequential runs. Delegating the full gate sequence to
the `scrutineer` agent is preferred.

Baseline and full gates:

```bash
set -euo pipefail
BRANCH="$(git branch --show-current)"
make check-fmt 2>&1 | tee "/tmp/check-fmt-netsuke-${BRANCH}.out"
make typecheck 2>&1 | tee "/tmp/typecheck-netsuke-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-netsuke-${BRANCH}.out"
make doc-coverage 2>&1 | tee "/tmp/doc-coverage-netsuke-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
make test-workflow-contracts 2>&1 | tee "/tmp/workflow-contracts-netsuke-${BRANCH}.out"
```

Each ends with exit status zero. `make test` prints a nextest summary line of
the form `Summary [ …s] N tests run: N passed, … skipped` followed by the
doctest results.

Focused runs reuse the gate's `RUSTFLAGS` and `RUSTDOCFLAGS` composition so
Cargo does not rebuild with a different fingerprint. On Linux the standard
flags are `-Zthreads=8 -Clink-arg=-fuse-ld=mold`; drop the linker flag on other
hosts.

```bash
set -euo pipefail
BRANCH="$(git branch --show-current)"
GATE_FLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$GATE_FLAGS" cargo nextest run --workspace --all-targets --all-features \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/focused-netsuke-${BRANCH}.out"
```

Expected at Red: named failures such as
`netsuke shell_selection::built_in::tests::support_matrix::case_1` with a
`pretty_assertions` diff. Expected at Green: every selected test passes.

Behavioural scenarios only (generated scenario tests are named after the
feature file stem, so the filter matches them):

```bash
set -euo pipefail
BRANCH="$(git branch --show-current)"
GATE_FLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$GATE_FLAGS" cargo nextest run --workspace --all-features --test bdd_tests \
  -E 'test(/shell_selection/)' 2>&1 | tee "/tmp/bdd-netsuke-${BRANCH}.out"
```

Doctests for the new module:

```bash
set -euo pipefail
BRANCH="$(git branch --show-current)"
GATE_FLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$GATE_FLAGS" RUSTDOCFLAGS="--cfg docsrs -D warnings" \
  cargo test --workspace --doc --all-features shell_selection \
  2>&1 | tee "/tmp/doctest-netsuke-${BRANCH}.out"
```

Snapshot review after adding a snapshot test:

```bash
cargo insta pending-snapshots
cargo insta accept
```

Scope and boundary checks before each milestone commit:

```bash
git --no-pager diff --no-ext-diff --stat HEAD
rg -n 'use (crate|super)::' src/shell_selection/name/mod.rs
rg -n 'ShellSelection' src/ir src/ninja_gen src/ast
```

Expected: the diff lists only `src/lib.rs`, files under `src/shell_selection/`,
files under `src/snapshots/shell_selection/`, `proptest-regressions/` (only if
a failure was found and fixed),
`tests/features/shell_selection_vocabulary.feature`,
`tests/bdd/steps/shell_selection.rs`, `tests/bdd/steps/mod.rs`,
`tests/bdd/fixtures/mod.rs`, the exact-prefix layout contract
`tests/workflow_contracts/rust_module_layout_test.py`, and `docs/` files. Both
`rg` commands print nothing.

Markdown gates after documentation edits:

```bash
set -euo pipefail
BRANCH="$(git branch --show-current)"
make fmt 2>&1 | tee "/tmp/fmt-netsuke-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-netsuke-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-netsuke-${BRANCH}.out"
```

Commit after each green milestone with an imperative subject, for example
`Add the built-in shell registry and ShellName`, and gate each commit.

## Validation and acceptance

Acceptance is behavioural:

- `ShellName::parse("dash")` returns a name whose `as_str()` is `dash`, and
  `BuiltInShell::lookup` of it is `None`; `ShellName::parse("/bin/bash")`
  returns `InvalidFirstCharacter('/')`; `ShellName::parse("true")` returns
  `ReservedWord { word: "true" }`. All three appear as doctests on `ShellName`.
- `BuiltInShell::host_default(HostFamily::Windows)` is
  `BuiltInShell::PowerShell`, whose `fixed_args()` are `-NoLogo`, `-NoProfile`,
  `-NonInteractive`, `-Command`, and whose `executable()` is
  `BareName("powershell.exe")`.
- Decoding `shell: true` gives `PlatformDefault`; `shell: bash` gives
  `Named("bash")`; `shell: "true"` (quoted) gives a reserved-word error;
  `shell: 1` gives an error whose text contains
  `expected a Boolean or a shell name`.

Red-Green-Refactor evidence to record in `Artefacts and notes` per milestone:
the focused command and the failing tests for both Red passes; the same command
passing at Green; the gates passing after Refactor.

Quality criteria:

- Tests: `make test` passes, including every new unit, property, snapshot,
  doctest, and behavioural test; every new test failed under at least one Red
  pass for the asserted reason.
- Verification: obligations O1 to O13 discharged as described, with the
  deterministic non-vacuity companions passing.
- Lint and types: `make check-fmt`, `make typecheck`, and `make lint` pass;
  `make doc-coverage` does not fall below its threshold.
- Documentation: `make markdownlint` and `make nixie` pass.
- Security: the vocabulary accepts no path, template, or YAML Boolean spelling
  as a name (O7, O11).

## Idempotence and recovery

Every step is re-runnable. Test and gate commands do not mutate tracked files
except `make fmt`, which only reformats. If `cargo insta accept` accepts a
wrong snapshot, delete the `.snap` file and rerun the test to regenerate the
pending snapshot. If a proptest failure writes a file under
`proptest-regressions/`, keep it (it is the regression seed) and commit it with
the fix. Red stubs are never committed. To abandon a milestone, reset the
branch to the previous milestone commit; each milestone is a coherent plateau.

## Artefacts and notes

The registry snapshot should render Table 1 from code in roughly this shape
(the accepted snapshot is authoritative once reviewed against ADR-019):

```plaintext
name        unix-like  windows  executable                  fixed arguments
sh          yes        no       absolute-path /bin/sh       -c
bash        yes        yes      bare-name bash              --noprofile --norc -c
pwsh        yes        yes      bare-name pwsh              -NoLogo -NoProfile -NonInteractive -Command
powershell  no         yes      bare-name powershell.exe    -NoLogo -NoProfile -NonInteractive -Command
unix-like default: sh
windows default: powershell
```

### EP-M1 Red evidence (2026-10-17)

The reject-all candidate compiled successfully with all targets and features.
The corrected focused Nextest run selected all 91 new cases: 21 passed and 70
failed on assertions (exit 100), in
`/tmp/test-netsuke-shell-m1-red1-corrected-20261017.out`. The complete case
receipt is `/tmp/shell-m1-red1-corrected-cases-20261017.txt`. An earlier
invocation accidentally included positional `.` and selected zero cases; that
command-hand-off error supplies no Red evidence.

The first registry snapshot failure was a missing snapshot, not a sensitivity
check. Its deliberately wrong output was preserved privately, and the expected
snapshot was independently populated from ADR-019 Table 1 for the second pass.
The current-host doctest's original membership assertion was insensitive to
coordinated stub changes; it now checks the meaningful O6 alignment with the
legacy default. No production body changed in that refinement. Focused Red and
Green doctest execution is required alongside Nextest before acceptance.

The first focused doctest run compiled and executed 15 examples: 13 failed at
runtime and two passed (`BuiltInShell::is_supported_on` and `ShellNameError`).
Both passing examples must fail the accept-all second stub. Log:
`/tmp/doctest-netsuke-shell-m1-red1-20261017.out` (exit 101; 121 unrelated
examples filtered). The 70-byte precedence witness was mechanically changed from
`"aB".repeat(35)` to `"aaB".repeat(23) + "a"` before Red2 to match the
specified upper-case character at index 2. Its expected `TooLong` and its
first-pass failure against `Empty` are unchanged.

The second Nextest pass ran all 91 cases: 34 passed and 57 failed on assertions
(exit 100), in `/tmp/test-netsuke-shell-m1-red2-20261017.out`. The union audit
`/tmp/netsuke-shell-m1-red-union-20261017.json` confirms identical case sets,
91/91 failures across the two passes, and no case passing both. The registry
snapshot now failed against the independently reviewed normative table, proving
sensitivity to the two unsupported-host cells. The second doctest pass had ten
passes and five runtime failures; both first-pass survivors failed, giving
15/15 examples a meaningful Red failure. Source relocation follows those
receipts without changing test bodies or Rust module identities.

The bounded layout regression ran against the empty prefix-exception table:
eight tests ran, four passed, and four failed as expected (the live-tree check
and all three new exact-exception cases). There were no collection errors. Log:
`/tmp/test-netsuke-shell-m1-layout-red-20261017.out`. The correction adds only
the exact `{shell_selection/, shell_word.rs}` exception and its disparate
concern rationale; the checker implementation is unchanged. The same bounded
suite must pass before restoring the Green Rust bodies.

The same bounded layout suite passed all eight cases after the exact exception
was added, with no checker implementation change. The source relocation and
snapshot metadata update preserve the 91 Rust case identities and the normative
registry contents. Green production bodies are now restored; focused Nextest
and doctests run with ordinary proptest persistence before the three explicit
O7 sensitivity mutations and full gates. Layout Green log:
`/tmp/test-netsuke-shell-m1-layout-green-20261017.out`.

The first Green compilation rejected the speculative `const` on `into_inner`
with E0493: dropping the `ShellName` wrapper is not permitted in that constant
function under the pinned toolchain. No tests executed. The method returns to
its approved ordinary `pub fn` signature; no compiler feature, toolchain, or
ownership workaround is needed. Log:
`/tmp/test-netsuke-shell-m1-green-20261017.out`. Focused Green is retried only
after this source correction, with ordinary persistence.

Focused Green then passed all 91 Nextest cases and all 15 doctests with
ordinary persistence, after removing the unsupported consuming-method `const`.
Logs: `/tmp/test-netsuke-shell-m1-green2-20261017.out` and
`/tmp/doctest-netsuke-shell-m1-green2-20261017.out`. The three explicit O7
controls now mutate that freshly validated source one at a time and restore the
full original file by SHA-256 before each next candidate. The first changes only
`MAX_LEN` from 63 to 64 and must fail `error_precedence::case_04` because 64
ASCII letters are incorrectly accepted instead of yielding `TooLong`.

The length control compiled and failed its one selected case as predicted: 64
ASCII letters produced `Ok` instead of `TooLong { length: 64 }`. Log:
`/tmp/test-netsuke-shell-m1-control-length-20261017.out`. The complete Green
file was restored and verified against its saved SHA-256. The next control
changes only the first-character check from lower-case ASCII to alphabetic
ASCII and must fail `error_precedence::case_05` by accepting `Bash` instead of
returning `InvalidFirstCharacter`.

The upper-case control compiled and failed its one selected case as predicted:
`Bash` produced `Ok` instead of `InvalidFirstCharacter { character: 'B' }`. Log:
`/tmp/test-netsuke-shell-m1-control-uppercase-20261017.out`. The complete
Green file was restored and verified again. The final control removes only
`true` from the reserved array (adjusting its array length mechanically), and
must fail `rejects_reserved_words::case_1` because `true` is incorrectly
accepted instead of yielding `ReservedWord`.

The reserved-word control compiled and failed its one selected case as
predicted: `true` produced `Ok` instead of `ReservedWord { word: "true" }`. Log:
`/tmp/test-netsuke-shell-m1-control-reserved-20261017.out`. All three O7
controls therefore failed for their intended reasons. The complete Green
`name.rs` was restored byte-for-byte to SHA-256
`96b81c6b46cd288312c8de60dc62f4bbe54872432f9b209391de187070156636`. There are
no pending `.snap.new` files and no new proptest regression seeds; no actual
Green property bug was found. O1 to O8 have focused passing evidence, the
91-case and 15-doctest Red unions, the reviewed registry snapshot, and the
three O7 negative controls. EP-M1 remains open pending all required gates,
commit, and completed local review.

The full-gate repair history is retained separately from the successful
sub-gates. FULL1 failed on two owner-file layout diagnostics and seven
test-style findings. Moving the owners into `built_in/mod.rs` and `name/mod.rs`
preserves module identities and production bytes; rich comparisons, direct
string writes, checked remainders, and fallible audit errors preserve the
checks. FULL2 then needed an explicitly typed empty slice for that comparison
(E0282); its focused case passed after the repair. FULL3 passed all Rust lint
phases but failed Ruff on implicit string concatenation in the layout exception
rationale. FULL4's front-loaded Python lint caught a 90-column continuation
after parenthesization. Splitting that literal preserved its value. Logs:
`/tmp/lint-netsuke-shell-m1-full1-20261017.out`,
`/tmp/lint-netsuke-shell-m1-full2-20261017.out`,
`/tmp/lint-netsuke-shell-m1-full3-20261017.out`, and
`/tmp/lint-python-netsuke-shell-m1-full4-20261017.out`. Historical `name.rs`
hashes above describe the unchanged production bytes before the file move.

FULL5 passed `make fmt`, `make check-fmt`, the Python lint preflight, the
complete `make lint` target, `make typecheck`, `make doc-coverage` (98.90%,
5,143/5,200), and `make test-workflow-contracts` (1,320 passed, three skipped,
plus 27 script tests). Its Ninja-required `make test` run passed all 91 new
cases, but the full Nextest result was 4,152 passed, one timeout, and six
skipped; 18 tests were not run.
`command_env_ui_tests::ir_gen_error_public_api_fixture_compiles` timed out at
300.009 seconds. The module-boundary check passed in 599.561 seconds within its
existing 720-second policy. Doctests were not reached; Markdown and Mermaid
checks did not run. Logs use `/tmp/*-netsuke-shell-m1-full5-20261017.out`.
These partial results do not satisfy the full Rust gate.

A bounded diagnostic first selected zero tests because its name included the
binary namespace; exit 4 is not a passing result. The corrected exact-name
invocation selected one test and passed in 270.883 seconds with the unchanged
300-second policy. Observations of its own child process showed `cargo check`
waiting behind a healthy full compiler pool at 82 and 254 seconds before
fixture compilation. This supports resource contention but does not prove the
exact historical timeout cause. No source, timeout, or admission policy
changed. Evidence:
`/tmp/test-netsuke-shell-m1-timeout-diagnostic2-20261017.out` and
`/tmp/netsuke-shell-m1-timeout-diagnostic2-processes-20261017.out`. The first
zero-selection log is
`/tmp/test-netsuke-shell-m1-timeout-diagnostic1-20261017.out`. The diagnostic
did not replace full validation.

FULL6 then passed the fresh Ninja-required `make test`: 4,171 Nextest tests
passed (six skipped), and 151 doctests passed (32 ignored). The previously
timed out fixture passed in 49.730 seconds under its unchanged 300-second
policy; packaging passed in 861.669 seconds within its existing 900-second
policy. Formatting, format checks, Markdown/spelling, and Mermaid validation
also passed. FULL5's aggregate lint, typecheck, documentation coverage, and
workflow contract receipts remain applicable: all ten non-plan candidate files
match `/tmp/netsuke-shell-m1-post-diagnostic-frozen-hashes-20261017.json`
byte-for-byte. FULL6 logs use `/tmp/*-netsuke-shell-m1-full6-20261017.out`. All
required pre-commit gates now have passing evidence. The final plan-only
evidence update still requires documentation checks; milestone commit and
completed local assessment remain pending, so EP-M1 is not closed.

Intentional Red commands set `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1` only in
the child command environment, as documented by proptest 1.11.0's `Config`
interface. Normal Green runs retain persistence. This prevents injected wrong
stubs from creating spurious regression seeds; real failures remain persisted.

## Interfaces and dependencies

No new dependency. At the end of EP-M2 these items exist in
`netsuke::shell_selection` (re-exported from `src/shell_selection/mod.rs`). Doc
comments are abbreviated here; the real code carries `///` on every item and
variant, `#[must_use]` where the Green checklist requires it, and doctests on
public items.

```rust
// src/shell_selection/built_in/mod.rs

/// The host family that decides built-in shell support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostFamily {
    UnixLike,
    Windows,
}

impl HostFamily {
    pub const ALL: [Self; 2];
    pub const fn current() -> Self;
}

/// How a built-in shell's executable is located.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInExecutable {
    /// A fixed absolute path used without searching `PATH`.
    AbsolutePath(&'static str),
    /// A bare executable name searched on the trusted host `PATH` by 11.2.3.
    BareName(&'static str),
}

/// The finite built-in shell registry from ADR-019 Table 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInShell {
    Sh,
    Bash,
    Pwsh,
    PowerShell,
}

impl BuiltInShell {
    pub const ALL: [Self; 4];
    pub const fn name(self) -> &'static str;
    pub fn lookup(name: &ShellName) -> Option<Self>; // Self::ALL.into_iter().find(…)
    pub const fn is_supported_on(self, host: HostFamily) -> bool;
    pub const fn executable(self) -> BuiltInExecutable;
    pub const fn fixed_args(self) -> &'static [&'static str];
    pub const fn host_default(host: HostFamily) -> Self;
}
// Also: impl Display for BuiltInShell; impl From<BuiltInShell> for ShellName.

// src/shell_selection/name/mod.rs (imports only std, serde, thiserror)

/// A validated shell registry name matching `[a-z][a-z0-9_-]{0,62}`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShellName(pub(super) String); // Only parse and built-in From construct it.

impl ShellName {
    pub const MAX_LEN: usize = 63;
    pub fn parse(text: &str) -> Result<Self, ShellNameError>;
    pub const fn as_str(&self) -> &str;
    pub fn into_inner(self) -> String;
}
// Also: TryFrom<&str>, TryFrom<String>, FromStr, AsRef<str>, Display,
// Serialize (as a string), Deserialize (String, then parse, then
// serde::de::Error::custom). No Borrow<str> (Decision D3).

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShellNameError {
    #[error("a shell name must not be empty")]
    Empty,
    #[error("a shell name must be at most 63 bytes, but has {length}")]
    TooLong { length: usize },
    #[error("a shell name must start with a lower-case ASCII letter, not {character:?}")]
    InvalidFirstCharacter { character: char },
    #[error(
        "a shell name may contain only lower-case ASCII letters, digits, '_', and '-', \
         but has {character:?} at character {char_index}"
    )]
    InvalidCharacter { character: char, char_index: usize },
    #[error("{word:?} is reserved because YAML reads it as a Boolean; use an unquoted Boolean instead")]
    ReservedWord { word: &'static str },
}

// src/shell_selection/selection/mod.rs

/// The manifest author's shell intent for one structured command block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShellSelection {
    #[default]
    Direct,
    PlatformDefault,
    Named(ShellName),
}
// Also: Serialize and a handwritten Deserialize (Decision D6).
```

The error message wording above is indicative; settle it at Green and pin it
with O12's snapshot. Use `concat!()` rather than a backslash continuation if a
literal exceeds the line limit, per AGENTS.md.

`ShellSelection` has no method that resolves anything. Resolution into an
executable path belongs to 11.2.3, and the IR's `ResolvedShell` belongs to
11.3.1.

[^1]: GitHub Docs, "Workflow syntax for GitHub Actions",
    `jobs.<job_id>.steps[*].shell`,
    <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>,
    retrieved 2026-09-24.

## Revision note

- 2026-09-24: initial draft for review.
- 2026-09-24: revised after a three-panel expert design review. Renamed
  `ShellHost` to `HostFamily` and moved it into `built_in.rs`; renamed the
  executable variants to `AbsolutePath` and `BareName`; fixed the dependency
  direction inside the module and outward to 11.2.2 and 11.2.3, including the
  build-script slice constraint on `name.rs`; specified `ShellNameError`
  precedence and character indexing; relocated the localization boundary to the
  serde adapters; expanded the YAML 1.1 and tagged-scalar pinning table and
  moved it to mapping position; added the error-text snapshot (O12) and the IR
  non-leak check (O13); replaced `std::path` absoluteness with a POSIX shape
  check; fixed the near-miss generator and named the deterministic companion
  API; added a second, accept-everything Red pass so rejection tests are proven
  to fail; added the lint checklist, pinned behavioural step patterns and a
  spike, and dropped the host-default scenario; corrected the CI platform list,
  snapshot location, doctest flags, and tolerances; and firmed up the hand-offs
  to 11.2.2, 11.2.3, 11.3.1, and 12.1.1. None of these changes alters the
  ADR-019 contract; the remaining work is unchanged in shape (three
  implementation milestones after approval).
- 2026-09-24: revised after user review. Reserved the eight lower-case YAML 1.1
  Boolean spellings in `ShellName` itself (new `ReservedWord` error, last in
  precedence), replacing the hand-off to 11.2.2. Added the ADR-019 addendum of
  2026-09-24 and aligned RFC 0011 sections 4.1, 5.2, and 11 in this pull
  request. Updated Decision D8 (now an approved deviation), the Purpose,
  Constraints, Risks, Conformance basis, O2, O7, O8, O10, O11, and O12, the
  behavioural examples, the interface sketch, and the acceptance criteria. The
  milestone structure is unchanged.
- 2026-10-13: recorded the final EP-M0 gate results and stopped at Stage A
  after the untouched-tree nextest timeout. The plan was `BLOCKED` at that
  point; the next revision records that blocker’s repair and supersedes that
  status.

- 2026-10-14: repaired the baseline timeout, then followed the user's direction
  to separate the fix into prerequisite PR #913 targeting `main` and restack
  #778 above it. Restored `APPROVED` status for the unstarted feature plan,
  retained the original scope limits, and withdrew the combined 34-file ceiling
  proposal. Recorded fresh prerequisite validation separately from the original
  red baseline and earlier combined candidate.

- 2026-10-17: resumed EP-M1 after a fully green rebased baseline. Refreshed
  current-main provenance, module paths, timeout-policy distinction, private
  construction ownership, and two mechanical specification examples. Feature
  implementation remains data-only; focused Red evidence is pending.

- 2026-10-17: recorded first-pass Nextest and doctest sensitivity, reviewed
  the normative registry snapshot, and prepared the second accept-all stub.
  Clarified the external-parser-only O11 sensitivity substitute; no parser or
  selection contract changes are authorized by that clarification.

- 2026-10-17: completed both Rust Red passes and the 91-case/15-doctest
  sensitivity unions. Adopted child test directories and a bounded, exact
  root-prefix exception to meet the rebased-main executable layout contract.
  Focused Python regression Red/Green precedes restoration of Green Rust.

- 2026-10-17: recorded focused M1 Green and all three negative controls,
  restored the exact validated production source, and froze the candidate for
  full code, workflow, and Markdown gates. EP-M1 remains open pending gates,
  commit, and local review; EP-M2 and EP-M3 have not begun.

- 2026-10-17: the first full Clippy gate exposed owner-file layout and seven
  test-style errors. Moved the new production owners into their `mod.rs` files,
  preserving module identities and standalone name imports. Replaced test-only
  remainder operators, temporary formatting allocations, and assertions in the
  fallible generator audit without changing its checks. Full candidate
  validation is retried; no milestone is closed.

- 2026-10-17: consolidated the actual FULL1–4 bounded lint repairs and
  FULL5 partial gate results. Recorded the unchanged fixture timeout and its
  passing, observed one-case diagnostic without claiming an established
  historical cause or a full Rust pass. EP-M1 acceptance, commit, and review
  remain open; production and test bytes remain frozen.

- 2026-10-17: recorded FULL6's complete Rust and documentation passes and
  verified non-plan byte equivalence with the successful FULL5 gates. Retained
  the earlier timeout as failed history; no product or timeout policy changed.
  The rebased 2026-10-19 gates supersede those candidate-specific receipts.

- 2026-10-19: recorded the merged timeout prerequisite, conflict-free feature
  rebase, and eight sequential gates on the rebased M1 candidate. The
  exact-head local assessment remains outstanding; EP-M2 and EP-M3 are not yet
  complete.
- 2026-10-20: replaced the active 720-second blocker with the measured cause
  and validated, scoped 900-second remedy in lower PR 934. Recorded three
  rounds of lower-PR assessment corrections and their gate results. Commit
  `0a6143c3` fixes the final reported documentation ambiguity; all required
  checks pass, with Windows lint and reconciliation still pending. The 19
  feature commits were replayed onto `0a6143c3` without conflicts; `range-diff`
  matches all 19 and the M3 patch checksum is unchanged. The rebased upper head
  is `b107341f`; full candidate gates and publication remain outstanding.
