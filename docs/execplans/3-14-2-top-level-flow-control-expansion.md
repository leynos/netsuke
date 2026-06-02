# 3.14.2. Expand top-level action flow control

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `3.14.2` closes the remaining conditional-planning gap for
top-level `actions`. Netsuke already expands `foreach` and `when` entries in
both top-level `targets` and top-level `actions` during manifest loading. This
plan treats that as existing behaviour to preserve, then completes the missing
user-facing branch-selection workflow: authors can write complementary
manifest-time action branches such as
`when: command_available("cargo-nextest")` and
`when: not command_available("cargo-nextest")`.

After implementation, users can declare tool-dependent actions without shelling
out from Jinja and without letting dynamic condition keys leak into the typed
Abstract Syntax Tree (AST), Intermediate Representation (IR), generated
`build.ninja`, or Ninja execution. Observable success is a manifest whose
generated action set contains exactly the branch selected by the host
environment, while top-level `actions` still deserialize as phony targets.

This plan was approved on 2026-05-20 and implemented on the
`3-14-2-top-level-flow-control-expansion` branch.

## Constraints

- Keep `foreach` and `when` as manifest-time controls. They must be evaluated
  before typed AST deserialization, final string rendering, IR generation,
  Ninja synthesis, and Ninja execution.
- Preserve the current expansion path in `src/manifest/expand.rs`:
  `expand_foreach` must continue to process both `targets` and `actions`, and
  downstream layers must not receive `foreach` or `when` keys.
- Preserve existing implicit top-level action behaviour. Entries under
  top-level `actions` must still become `Target` values with `phony: true`
  after expansion, regardless of whether they came from `foreach`.
- Implement `command_available` as a non-throwing executable availability
  predicate for absence only. Missing executables return `false`; invalid
  arguments still return the existing `netsuke::jinja::which::args` style error.
- Reuse the existing `which` resolver and cache in `src/stdlib/which`, rather
  than adding a second process, filesystem, or `PATH` probing implementation.
- Keep domain and policy logic at the manifest/std-library boundary. CLI
  adapters, IR generation, Ninja generation, and process execution must not
  decide conditional branch semantics.
- Do not introduce runtime-condition semantics. Build-time branching remains a
  recipe concern unless a separate approved design adds runtime conditions.
- Use existing `ortho_config` integration for any new command-line,
  configuration, or localized help surface discovered during implementation. Do
  not add a parallel configuration loader or untranslated help path.
- Obtain explicit approval before adding any new external dependency.
- Avoid introducing `unsafe` code.
- Keep every Rust source file below the 400-line cap from `AGENTS.md`.
- Use en-GB Oxford spelling in documentation, except for external API names,
  source identifiers, and established computing terms such as `serialization`
  and `deserialization`.
- Unit tests must use `rstest` where shared setup or parameterized cases remove
  duplication.
- Behavioural tests that describe externally observable manifest or CLI output
  must use `rstest-bdd`.
- If the implementation introduces a new invariant over a range of inputs,
  states, orderings, or transitions, add property testing or a Kani harness. If
  it introduces a contractual business axiom that requires exhaustive
  reasoning, stop and propose a substantive Verus proof before implementation
  continues.
- Do not mark roadmap item `3.14.2` done until the implementation, tests,
  documentation, CodeRabbit review, and quality gates all pass.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 12 files or adding more
  than roughly 650 net lines, stop and ask for approval of a revised scope.
- Interface: if a public Rust API signature, CLI flag, configuration key, or
  manifest schema field other than the documented `command_available` helper
  must change, stop and explain the options.
- Roadmap overlap: if satisfying the complementary-branch requirement requires
  completing all of the roadmap item `3.14.4`, stop and ask whether to merge
  `3.14.4` into this task or split the work.
- Dependencies: if a new crate, Cargo feature, external tool, Kani harness, or
  Verus setup is required, stop and ask for approval.
- Semantics: if a plausible approach would make `when`, `foreach`, or
  `command_available` visible in typed AST, IR, generated Ninja, or runtime
  process execution, stop and reject that approach in the `Decision Log`.
- Testing: if a host-dependent command lookup makes tests flaky after two
  focused fixes, stop and redesign the test boundary around dependency
  injection or explicit `PATH` overrides.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and record the failing commands and log
  paths.
- Review: if `coderabbit review --agent` reports unresolved concerns after a
  major milestone, address them before proceeding. If a concern conflicts with
  this plan, stop and ask for direction.

## Risks

- Risk: action expansion is already implemented, so the remaining work could
  accidentally become a broad rewrite of stable code. Severity: medium.
  Likelihood: medium. Mitigation: add regression coverage around existing
  behaviour first, then make the smallest change needed for `command_available`
  branch predicates.

- Risk: `command_available` is also tracked by roadmap item `3.14.4`, creating
  scope ambiguity. Severity: high. Likelihood: high. Mitigation: implement only
  the non-throwing predicate semantics needed for complementary top-level
  action branches, and leave `3.14.4` open unless the user explicitly approves
  closing the full item.

- Risk: executable discovery can depend on the host `PATH`, current directory,
  platform executable suffixes, and workspace fallback behaviour. Severity:
  high. Likelihood: medium. Mitigation: test through `StdlibConfig` path
  overrides or scoped environment helpers so present and absent commands are
  deterministic.

- Risk: a missing command could be hidden by a workspace fallback search and
  make the fallback branch fail to run in tests. Severity: medium. Likelihood:
  medium. Mitigation: use the existing `cwd_mode="never"` and path override
  options where supported, or disable workspace fallback with documented test
  helpers.

- Risk: invalid `command_available` arguments might be swallowed as `false`,
  hiding manifest author errors. Severity: high. Likelihood: medium.
  Mitigation: preserve `which` argument validation errors and test invalid
  keyword or empty-command cases.

- Risk: generated action names or defaults may reference a branch that was
  filtered out. Severity: medium. Likelihood: medium. Mitigation: keep existing
  downstream validation responsible for missing selected entries; do not
  silently add default-rewriting behaviour in this task.

## Relevant context

The manifest load pipeline is in `src/manifest/mod.rs`. `from_str_named` parses
YAML into `ManifestValue`, registers `env`, `glob`, the stdlib helpers and
manifest macros, then calls `expand_foreach` before deserializing the typed
`NetsukeManifest`. That is the architectural boundary this task must preserve.

The expansion implementation is in `src/manifest/expand.rs`. `expand_foreach`
currently calls `expand_section(doc, "targets", env)` and
`expand_section(doc, "actions", env)`. `expand_target` handles both static and
`foreach` entries, `when_allows` removes `when`, and `inject_iteration_vars`
adds `vars.item` and `vars.index` for generated entries.

Top-level actions are deserialized in `src/ast.rs` by `deserialize_actions`,
which sets `action.phony = true` for every action after expansion. This is why
implicit phony action behaviour is preserved outside `src/manifest/expand.rs`.

Executable discovery lives in `src/stdlib/which`. The current `which` filter
and function share a `WhichResolver` created in `which::register`. The
`command_available` helper should be registered beside those entrypoints and
must call the same resolver with the same parsed options.

The user-facing documentation entry points are `docs/users-guide.md` sections
`Jinja Templating in Netsuke`, ``foreach` and `when``, and
`Executable Discovery (`which`)`. Internal implementation guidance lives in
`docs/developers-guide.md` under `Manifest foreach expansion` and
`Manifest processing helpers`.

Relevant skills and documents to keep open while implementing:

- `leta`: use `leta show`, `leta refs`, and `leta grep` for code navigation.
- `rust-router`: route Rust-specific questions to the smallest useful follow
  on skill. Likely follow-ons are `rust-errors` for error shape and
  `domain-cli-and-daemons` only if CLI behaviour unexpectedly changes.
- `hexagonal-architecture`: keep branch-selection policy inside the manifest
  and stdlib boundary; do not move it into adapters.
- `execplans`: keep this plan current.
- `commit-message`: write commit messages through a temporary file and
  `git commit -F`.
- `pr-creation` and `en-gb-oxendict-style`: use them when opening or revising
  the draft pull request.
- `docs/roadmap.md`
- `docs/netsuke-design.md`, especially sections 1.2, 2.2, 2.5, and 4.5
- `docs/users-guide.md`
- `docs/developers-guide.md`
- `docs/ortho-config-users-guide.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/reliable-testing-in-rust-via-dependency-injection.md`

## Prior art

Firecrawl research compared Netsuke's shape with open-source workflow and build
tools. The useful pattern is not exact syntax copying; it is preserving a clear
phase boundary between structural selection and command execution.

- GitHub Actions matrix jobs use `strategy.matrix` and `if` guards in workflow
  definitions. Firecrawl found that job or step selection is resolved before
  the selected job or step executes, while matrix values are exposed through a
  dedicated context. Source: [github-actions]. Planning implication: Netsuke
  should keep `item`/`index` as expansion context and ensure generated
  manifests contain selected entries only.
- Taskfile.dev separates `for` iteration, `if` soft skips, and `preconditions`
  hard failures. Source: [taskfile]. Planning implication: `when` should stay a
  soft inclusion guard, while invalid `command_available` arguments should
  remain hard authoring errors.
- Just supports conditional expressions for values, while recipe execution
  conditionals are normally shell-level constructs. Source: [just]. Planning
  implication: Netsuke's manifest-time `command_available` branch selection
  should not become a shell recipe feature.
- cargo-make has a richer `condition` model with explicit negated predicates
  such as environment-not-set style checks. Source: [cargo-make]. Planning
  implication: Netsuke does not need a new boolean DSL for this task because
  MiniJinja already supports `not command_available(...)`.
- Bazel's Starlark `select()` resolves configurable attributes during the
  analysis phase, before execution. Source: [bazel]. Planning implication:
  Netsuke should preserve a deterministic analysis-style manifest expansion
  phase for conditional actions.
- GNU Make conditionals such as `ifdef`, `ifndef`, `ifeq`, and `ifneq` control
  makefile structure while recipes still use shell conditionals at execution
  time. Source: [gnu-make]. Planning implication: document the same two-phase
  distinction for Netsuke users: `when` changes the manifest; recipe code
  changes runtime command behaviour.
- Ansible combines `loop` with `when` and uses Jinja-style `not` negation, but
  evaluates per task/host during execution. Source: [ansible]. Planning
  implication: the `when` keyword is familiar, but Netsuke must be explicit
  that its `when` is manifest-time rather than runtime.

## Implementation plan

### Milestone 1: Baseline and red tests

Confirm the current branch is `3-14-2-top-level-flow-control-expansion` and the
worktree is clean enough to proceed. Use `leta` for code navigation, not broad
manual source browsing, once symbol names are known.

Run the existing focused tests before editing, teeing output to `/tmp`:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
cargo test --all-targets --all-features manifest::expand \
  2>&1 | tee "/tmp/baseline-expand-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features --test bdd_tests manifest \
  2>&1 | tee "/tmp/baseline-bdd-manifest-${PROJECT}-${BRANCH}.out"
```

If the BDD filter does not select the desired generated tests, use
`cargo test --all-targets --all-features --test bdd_tests -- --list` to find
the exact generated test names and rerun focused cases.

Add failing tests before implementation:

- In `src/manifest/expand_test_cases/condition_cases.rs`, add `rstest`
  parameterized unit cases for action-level complementary predicates. The tests
  should register a deterministic `command_available` function in a local
  `Environment` where useful, proving the generic `when` expression layer
  accepts both `command_available("tool")` and `not command_available("tool")`.
- In `src/stdlib/which/lookup/tests.rs` or a new narrow test module under
  `src/stdlib/which`, add tests for the real `command_available` function once
  the registration shape is clear. Cover present command, absent command, empty
  command, and invalid keyword argument behaviour.
- In `tests/features/manifest.feature`, add a behavioural scenario proving a
  top-level `actions` manifest with complementary `command_available` branches
  parses to exactly one selected action.
- In `tests/features/manifest_subcommand.feature`, add or extend generated
  output coverage so the emitted `build.ninja` text contains the selected
  action and does not contain the fallback action.

Expected state after this milestone: the new tests fail because
`command_available` is not registered yet, or because absent commands still
raise the existing `which` not-found error instead of returning `false`.

Run `coderabbit review --agent` after this milestone and resolve any concerns
about the test plan before moving to implementation.

### Milestone 2: Add the stdlib predicate

Implement `command_available` next to the existing `which` filter/function in
`src/stdlib/which/mod.rs`. Keep it small:

- clone the same `Arc<WhichResolver>` used by `which`;
- parse the same `WhichOptions` from `Kwargs`;
- trim and validate the command value in the same way as `which`;
- return `Value::from(true)` when `resolver.resolve(...)` succeeds with at
  least one executable;
- return `Value::from(false)` only for the existing not-found error class;
- return the original error for invalid arguments and other unexpected
  resolver failures;
- call `kwargs.assert_all_used()` so unknown keyword arguments remain errors.

If the current not-found error type is difficult to inspect without string
matching, first improve the internal error shape in `src/stdlib/which/error.rs`
so absence can be matched semantically. Keep any such change private to the
stdlib module unless a public error type already exists.

Do not change `src/manifest/expand.rs` unless a test proves that `eval_when`
cannot evaluate the registered function. `eval_when` already uses MiniJinja
expression compilation first, which is the right path for
`command_available(...)` and `not command_available(...)`.

Run the focused unit tests and record output:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
cargo test --all-targets --all-features command_available \
  2>&1 | tee "/tmp/command-available-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features manifest::expand \
  2>&1 | tee "/tmp/expand-after-command-available-${PROJECT}-${BRANCH}.out"
```

If this milestone requires a new public API or a new dependency, stop under the
tolerance rules before editing further.

### Milestone 3: Prove action branch behaviour end to end

Add deterministic test data for complementary top-level action branches. Prefer
a checked-in `tests/data/actions_command_available.yml` manifest if the
existing BDD manifest parser can use scoped environment/path setup for it. If
test setup needs dynamic paths, extend
`tests/bdd/steps/conditional_manifest.rs` with a temporary workspace builder
that writes the manifest and creates a fake executable in a temporary `bin`
directory.

Use the existing BDD environment helpers rather than direct global mutation.
`docs/developers-guide.md` identifies `tests/bdd/helpers/env_mutation.rs` and
`TestWorld` as the correct boundary for scenario-level environment changes.
Hold the environment lock whenever mutating `PATH`, current directory, or
related executable-discovery variables.

The behavioural happy path should show:

- when the fake preferred command is present, the preferred action exists and
  the fallback action does not;
- when the fake preferred command is absent, the fallback action exists and the
  preferred action does not;
- the selected action remains phony after AST deserialization;
- generated manifest output contains only the selected action.

The unhappy path should show that an invalid `command_available` option fails
manifest parsing with an actionable error instead of silently selecting a
branch.

Run focused behavioural coverage:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
cargo test --all-targets --all-features --test bdd_tests command_available \
  2>&1 | tee "/tmp/bdd-command-available-${PROJECT}-${BRANCH}.out"
cargo test --all-targets --all-features --test bdd_tests manifest_time \
  2>&1 | tee "/tmp/bdd-manifest-time-${PROJECT}-${BRANCH}.out"
```

Run `coderabbit review --agent` after this milestone and clear concerns before
documentation and roadmap close-out.

### Milestone 4: Update documentation and roadmap

Update user and internal documentation only after the behaviour is proven:

- In `docs/users-guide.md`, add `command_available` to the executable
  discovery section and include a concise complementary action-branch example.
  State that absence returns `false`, invalid arguments remain errors, and
  selected entries are still decided at manifest load time.
- In `docs/netsuke-design.md`, align section 4.5 with the implementation if
  needed. If the implementation deliberately narrows roadmap item `3.14.4`,
  record that the predicate required by `3.14.2` is present while broader
  follow-up work remains tracked separately.
- In `docs/developers-guide.md`, update the manifest expansion and stdlib
  helper notes so future contributors know that `command_available` belongs in
  the same resolver boundary as `which`.
- In `docs/roadmap.md`, mark `3.14.2` and its remaining subtask done only
  after all validation succeeds. Do not mark `3.14.4` done unless the user has
  approved that scope and its acceptance criteria are fully satisfied.
- If the implementation creates a durable design decision that is larger than
  a design-document paragraph, add an Architecture Decision Record (ADR)
  following `docs/documentation-style-guide.md`, and reference it from
  `docs/netsuke-design.md`.

Run Markdown formatting and documentation checks. If `make fmt` rewrites
unrelated Markdown, inspect and keep only appropriate changes for this task.

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
make fmt 2>&1 | tee "/tmp/fmt-${PROJECT}-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-${PROJECT}-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-${PROJECT}-${BRANCH}.out"
```

Run `coderabbit review --agent` after documentation changes and clear all
concerns.

### Milestone 5: Full validation, commit, push, and PR

Run the required commit gates sequentially, using `tee` for every command:

```sh
set -o pipefail
PROJECT=$(basename "$(git rev-parse --show-toplevel)")
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-${PROJECT}-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-${PROJECT}-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-${PROJECT}-${BRANCH}.out"
```

If all gates pass, inspect the diff, then commit with `git commit -F` using a
temporary message file. Commit the implementation and any later review fixes in
small, reviewable commits.

Push with upstream tracking:

```sh
git push -u origin 3-14-2-top-level-flow-control-expansion
```

Open or update a draft pull request. The title must include the roadmap item:

```plaintext
Expand top-level action flow control (3.14.2)
```

The pull request body must identify this execplan:
`docs/execplans/3-14-2-top-level-flow-control-expansion.md`. It must also
include a `## References` section containing the Lody session link:
`https://lody.ai/leynos/sessions/${LODY_SESSION_ID}`.

## Validation plan

Focused validation:

- `cargo test --all-targets --all-features manifest::expand`
- `cargo test --all-targets --all-features command_available`
- `cargo test --all-targets --all-features --test bdd_tests command_available`
- `cargo test --all-targets --all-features --test bdd_tests manifest_time`

Repository gates:

- `make fmt`
- `make markdownlint`
- `make nixie`
- `make check-fmt`
- `make lint`
- `make test`

Reviewer gate after each major milestone:

- `coderabbit review --agent`

All long-running validation commands must run sequentially and tee output to
`/tmp`, following the repository command policy.

## Progress

- [x] 2026-05-18: Loaded the requested `leta`, `rust-router`, and
      `hexagonal-architecture` skills. Loaded `execplans`, `firecrawl-mcp`,
      `commit-message`, `pr-creation`, and `en-gb-oxendict-style` because the
      task requires a plan, web research, commits, a draft PR, and British
      Oxford documentation.
- [x] 2026-05-18: Created the `leta` workspace for this checkout with
      `leta workspace add`.
- [x] 2026-05-18: Confirmed the starting branch was
      `feat/flowexpansionexecplan`, then renamed it locally to
      `3-14-2-top-level-flow-control-expansion`. The requested remote branch
      did not exist yet, so upstream tracking will be set on first push.
- [x] 2026-05-18: Used a Wyvern agent team in read-only mode. One agent
      reviewed roadmap and documentation requirements; one agent reviewed the
      expansion implementation and test topology.
- [x] 2026-05-18: Started Firecrawl research for open-source prior art around
      matrix or `foreach` expansion and conditional guards.
- [x] 2026-05-18: Incorporated Firecrawl prior-art findings for GitHub
      Actions, Taskfile.dev, Just, cargo-make, Bazel, GNU Make, and Ansible.
- [x] 2026-05-18: Reviewed `docs/roadmap.md`, `docs/netsuke-design.md`,
      `docs/users-guide.md`, `docs/developers-guide.md`,
      `docs/ortho-config-users-guide.md`, `docs/rstest-bdd-users-guide.md`,
      `AGENTS.md`, the `Makefile`, and the current manifest/std-library code.
- [x] 2026-05-18: Drafted this pre-implementation ExecPlan.
- [x] 2026-05-18: Ran CodeRabbit review on the draft plan and addressed its
      grammar, Oxford comma, wrapping, and link-style concerns.
- [x] 2026-05-20: User approved implementation of this ExecPlan and requested
      that work proceed with frequent commits and CodeRabbit review after major
      milestones.
- [x] 2026-05-20: Confirmed the active branch is
      `3-14-2-top-level-flow-control-expansion` and the worktree was clean
      before implementation.
- [x] 2026-05-20: Ran the focused baseline suites before editing:
      `cargo test --all-targets --all-features manifest::expand` passed with
      30 selected tests, and
      `cargo test --all-targets --all-features --test bdd_tests manifest`
      passed with the selected manifest-related coverage. Logs are in
      `/tmp/baseline-expand-netsuke-3-14-2-top-level-flow-control-expansion.out`
      and
      `/tmp/baseline-bdd-manifest-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] 2026-05-20: Added failing coverage for complementary
      `command_available(...)` / `not command_available(...)` action branches
      and stdlib predicate semantics. The first red run showed the stdlib
      helper was unregistered; the manifest expansion unit case used a local
      deterministic MiniJinja function and already proved the generic `when`
      evaluator could handle both branches.
- [x] 2026-05-20: Implemented the stdlib `command_available` function beside
      `which`, reusing `WhichResolver`, `WhichOptions`, and the resolver cache.
      Absence maps to `false`; empty command values and unknown keyword
      arguments remain hard errors.
- [x] 2026-05-20: Ran
      `cargo test --all-targets --all-features command_available`, which passed
      with the four selected stdlib predicate tests and two manifest expansion
      branch tests. Log:
      `/tmp/command-available-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] 2026-05-20: Ran
      `cargo test --all-targets --all-features manifest::expand`, which passed
      with 32 selected manifest expansion tests. Log:
      `/tmp/expand-after-command-available-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] Milestone 1: baseline and red tests.
- [x] Milestone 2: stdlib `command_available` predicate.
- [x] 2026-05-20: Ran CodeRabbit after the stdlib predicate milestone. It
      raised fixture duplication and fallible fixture concerns in
      `tests/stdlib_which_tests.rs`; both were addressed, and the final
      milestone-2 review pass reported zero findings.
- [x] 2026-05-20: Added BDD coverage for the absent-command fallback branch,
      invalid `command_available` options, and a `netsuke manifest -` workflow
      where a scenario-local fake executable selects the preferred action and
      omits the fallback from generated Ninja output.
- [x] 2026-05-20: Ran focused BDD validation:
      `manifest_selecting_a_fallback_action_when_a_command_is_unavailable`,
      `manifest_parsing_fails_when_command_availability_receives_invalid_options`,
      `manifest_subcommand_command_availability_selects_the_preferred_top_level_action`,
      and `manifest_subcommand_manifest_time_conditions_select_generated_actions_and_targets`
      all passed. Logs are in `/tmp/bdd-command-available-fallback-*`,
      `/tmp/bdd-command-available-invalid-*`,
      `/tmp/bdd-command-available-preferred-*`, and `/tmp/bdd-manifest-time-*`.
- [x] 2026-05-20: Attempted CodeRabbit review after Milestone 3 three times.
      The service returned recoverable rate-limit errors before producing
      findings, with requested waits of 3 minutes 54 seconds, 5 minutes
      35 seconds, and 5 minutes 33 seconds. There were no reported concerns to
      clear. Continue to documentation and retry CodeRabbit after the next
      milestone.
- [x] Milestone 3: end-to-end action branch coverage.
- [x] 2026-05-20: Updated `docs/users-guide.md` with
      `command_available` behaviour and a complementary action-branch example.
      Updated `docs/developers-guide.md` with the internal resolver-boundary
      convention. Marked roadmap item `3.14.2` and its complementary-branch
      subtask done while leaving `3.14.4` open.
- [x] 2026-05-20: Ran `make fmt`; it still fails in the older `markdownlint`
      fixer path on broad pre-existing Markdown line-length issues. Restored
      unrelated formatter churn and kept only task-scoped documentation
      changes. Targeted `markdownlint-cli2` over the changed Markdown files
      passes with zero errors. `make markdownlint` and `make nixie` both pass.
- [x] 2026-05-20: Retried CodeRabbit after documentation updates. The first
      attempt was rate-limited; the retry completed and reported zero
      findings.
- [x] Milestone 4: documentation and roadmap update.
- [x] 2026-05-20: Ran `make check-fmt`, which passed. Log:
      `/tmp/check-fmt-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] 2026-05-20: Ran `make lint`; after tightening the new Rust tests to
      satisfy Clippy's `unnecessary_wraps`, `manual_assert`, `assertions_on_result_states`,
      and shadowing checks, the target passed. Log:
      `/tmp/lint-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] 2026-05-20: Ran `make test`, which passed the full workspace suite. Log:
      `/tmp/test-netsuke-3-14-2-top-level-flow-control-expansion.out`.
- [x] 2026-05-20: Attempted a final CodeRabbit review after recording full
      validation results. Both attempts were rate-limited before producing
      findings, with requested waits of 47 seconds and 5 minutes 49 seconds.
      There were no reported concerns to clear.
- [x] 2026-05-20: Committed the implementation as
      `e11f343 Add command availability action branching`, pushed
      `3-14-2-top-level-flow-control-expansion` to origin, and updated draft
      pull request #309 with the implementation summary, validation evidence,
      execplan link, and Lody session reference.
- [x] Milestone 5: full validation, commit, push, and pull request.
- [x] 2026-05-20: Addressed Windows `PATHEXT` review feedback by writing
      `.cmd` fixture commands for bare-name command lookup on Windows in both
      the direct stdlib test and BDD preferred-command workspace. Re-ran
      `cargo test --all-targets --all-features command_available`,
      `cargo test --all-targets --all-features --test bdd_tests manifest_subcommand_command_availability_selects_the_preferred_top_level_action`,
      `make check-fmt`, and `make lint`; all passed. Logs are in
      `/tmp/*-pathext-netsuke-3-14-2-top-level-flow-control-expansion.out`.

## Surprises & Discoveries

- `src/manifest/expand.rs` already expands both top-level `targets` and
  top-level `actions`. The open roadmap item is not a blank implementation
  task; it is a completion task around complementary command-availability
  branches and proof that existing action expansion remains correct.
- `docs/netsuke-design.md` already says top-level `actions` use the same
  manifest-time expansion pass as `targets`, while `docs/roadmap.md` still marks
  `3.14.2` open because the complementary branch subtask remains undone.
- The user guide explains `which`, but does not yet describe
  `command_available` as the non-throwing predicate for manifest-time branch
  selection.
- The existing `which` implementation already has the right resolver and cache
  boundary. The missing piece should be a small helper registered beside
  `which`, not a new discovery adapter.
- The old `tests/std_filter_tests/mod.rs` tree is not compiled as a direct
  Cargo integration-test target. New `command_available` integration coverage
  therefore lives in `tests/stdlib_which_tests.rs`, a direct test binary with
  self-contained fixtures.

## Decision Log

- Decision: Treat the existing `actions` expansion path as behaviour to
  preserve, not code to rewrite. Rationale: both code and tests already show
  `expand_foreach` processes `actions`; the roadmap's remaining unchecked
  subtask is complementary branch support. Date/Author: 2026-05-18 / planning
  agent.

- Decision: Implement `command_available` beside `which` and reuse
  `WhichResolver`. Rationale: the design document says `command_available`
  reuses the `which` resolver and cache, and hexagonal boundary discipline
  keeps executable probing in the stdlib helper boundary rather than IR or CLI
  adapters. Date/Author: 2026-05-18 / planning agent.

- Decision: Do not close roadmap item `3.14.4` as part of this plan unless the
  approved implementation explicitly broadens to cover all of its acceptance
  criteria. Rationale: `3.14.2` needs the predicate for complementary action
  selection, but `3.14.4` is a separately tracked executable-probe task with
  broader wording. Date/Author: 2026-05-18 / planning agent.

- Decision: Do not add Kani or Verus work in the initial plan.
  Rationale: this task should not introduce a new state machine, ordering
  algorithm, unsafe invariant, or contractual axiom. If implementation changes
  that assessment, the formal-methods tolerance requires escalation.
  Date/Author: 2026-05-18 / planning agent.

- Decision: Use a private `is_not_found_error` helper around the stable
  `netsuke::jinja::which::not_found` diagnostic code to distinguish absence
  from other resolver failures for `command_available`. Rationale: MiniJinja's
  error type is the existing stdlib helper boundary, and this keeps the public
  API unchanged while preserving hard errors for invalid arguments.
  Date/Author: 2026-05-20 / implementation agent.

## Outcomes & Retrospective

Implementation landed the smallest boundary change described in this plan:
`command_available` is registered beside `which`, reuses the executable
resolver/cache adapter, maps only the existing not-found diagnostic to `false`,
and keeps invalid arguments or other resolver failures as hard manifest-time
errors.

Top-level `actions` already participated in `foreach` and `when` expansion, so
the implementation preserved that path and added regression coverage around
complementary `command_available(...)` and `not command_available(...)`
branches. The existing implicit `phony: true` behaviour remains covered by the
pre-existing action expansion tests.

The documentation updates describe user-visible command-availability branching
in `docs/users-guide.md`, the internal stdlib boundary convention in
`docs/developers-guide.md`, and the completed roadmap item in
`docs/roadmap.md`. Roadmap item `3.14.4` remains open because it is tracked as
a broader executable-probe task.

[ansible]: <https://docs.ansible.com/projects/ansible/latest/playbook_guide/playbooks_conditionals.html>
[bazel]: <https://bazel.build/docs/configurable-attributes>
[cargo-make]: <https://sagiegurari.github.io/cargo-make/>
[github-actions]: <https://docs.github.com/actions/writing-workflows/choosing-what-your-workflow-does/running-variations-of-jobs-in-a-workflow>
[gnu-make]: <https://web.mit.edu/gnu/doc/html/make_7.html>
[just]: <https://just.systems/man/en/conditional-expressions.html>
[taskfile]: <https://taskfile.dev/docs/guide>
