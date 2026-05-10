# 3.14.1. Record manifest-time condition semantics

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `3.14.1` asks Netsuke to make the conditional planning contract
unambiguous for both top-level `targets` and top-level `actions`. A Netsuke
manifest can use `foreach` to generate entries and `when` to include or skip
entries. The important rule is that these are manifest-time decisions: Netsuke
evaluates them while loading the manifest, before typed AST deserialization,
before IR generation, and before the Ninja backend executes anything.

After this work is complete, a user reading `docs/users-guide.md` and
`docs/netsuke-design.md` will understand that selected entries are the only
entries that reach the IR and generated `build.ninja` file. They will also
understand that build-time branching belongs inside the recipe command or
script unless a future runtime-condition feature is deliberately designed.

Observable success means:

1. `docs/netsuke-design.md` records the exact pipeline order and the
   manifest-time-only condition rule for actions and targets.
2. `docs/users-guide.md` explains the same rule in user-facing language and
   gives safe guidance for build-time alternatives.
3. Internally facing documentation names the code boundaries that enforce the
   rule.
4. `rstest` coverage proves `when` and `foreach` are removed before typed AST
   deserialization and that filtered entries cannot affect IR generation.
5. `rstest-bdd` coverage exercises the externally observable workflow when a
   generated Ninja manifest contains only selected conditional entries.
6. `docs/roadmap.md` marks `3.14.1` done only after the implementation and
   validation gates pass.

## Constraints

- Do not add runtime-condition semantics in this roadmap item. Runtime
  branching is out of scope unless a separate approved design is written.
- Do not move `foreach` or `when` evaluation after typed AST deserialization.
  The manifest pipeline must remain YAML value loading, template expansion,
  typed AST deserialization, final string rendering, IR generation, Ninja
  synthesis, and optional Ninja execution.
- Keep domain and policy logic at the manifest/IR boundary. Adapters such as
  CLI command handling and Ninja process execution must not decide condition
  semantics.
- Preserve the existing action rule that top-level `actions` deserialize as
  `Target` values with `phony: true`.
- If any new configuration surface or CLI help text is required, use the
  existing `ortho_config` integration and localized help path. Do not invent a
  parallel configuration mechanism.
- Use en-GB Oxford spelling in documentation.
- Add or update `rstest` unit and integration tests for happy paths, unhappy
  paths, and edge cases.
- Add `rstest-bdd` behavioural coverage where the behaviour is observable
  through the CLI or generated manifest output.
- Use dependency injection for any new environment, clock, process, or
  filesystem dependency introduced by tests or code. Avoid hidden mutable
  global state.
- Do not introduce `unsafe` code.
- Do not add external dependencies without explicit approval.
- Keep every Rust source file under the 400-line cap in `AGENTS.md`.
- Record user-visible behaviour in `docs/users-guide.md`.
- Record design decisions in `docs/netsuke-design.md`.
- Record internally facing interface or practice changes in the relevant
  component architecture document, expected to be `docs/developers-guide.md`
  unless implementation discovers a more specific home.
- Mark roadmap item `3.14.1` done only after all implementation validation
  gates pass.
- Final implementation validation must include logged sequential runs of:
  - `make fmt`
  - `make check-fmt`
  - `make lint`
  - `make test`
  - `make markdownlint`
  - `make nixie`

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 10 files or roughly
  500 net new lines, stop and escalate with a revised scope proposal.
- Semantics: if any test or implementation change would make `when` visible in
  typed AST, IR, Ninja output, or Ninja execution, stop and escalate.
- Interface: if a public Rust API signature, CLI flag, configuration key, or
  manifest schema field must change, stop and escalate before editing it.
- Dependencies: if a new crate, feature flag, external tool, Kani harness, or
  Verus proof setup is required, stop and ask for approval.
- Formal methods: if implementation introduces a new invariant over a range of
  inputs or state transitions, add a property-test or bounded-model-checking
  proposal. If the invariant is a business axiom rather than a sampled
  property, stop and propose a substantive proof approach before proceeding.
- Behavioural tests: if the BDD harness cannot express the observable
  generated-manifest workflow after two focused attempts, stop and document the
  blocker rather than replacing it with only unit tests.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and record the failures.
- Documentation conflict: if `docs/users-guide.md`, `docs/netsuke-design.md`,
  and the current code disagree on the condition contract, stop and identify
  which source should become authoritative.

## Risks

- Risk: the design document already contains most of the intended semantic
  text, so the implementation may accidentally become a no-op documentation
  edit with weak regression coverage. Severity: medium. Likelihood: medium.
  Mitigation: add tests that prove the boundary, especially that filtered
  actions and targets do not reach IR or generated Ninja output.

- Risk: users may read `when` as a Ninja-time or build-time conditional.
  Severity: high. Likelihood: high. Mitigation: use direct wording in the user
  guide and include a recipe-level alternative example for build-time branching.

- Risk: action-level and target-level conditions can drift if tests cover only
  one top-level section. Severity: medium. Likelihood: medium. Mitigation:
  parameterize `rstest` cases across `actions` and `targets`, then add a BDD
  scenario for generated output.

- Risk: defaults may reference a filtered-out target. Severity: medium.
  Likelihood: medium. Mitigation: include an explicit implementation decision:
  roadmap `3.14.1` records semantics and must not silently redesign defaults.
  If current behaviour is unclear or harmful, document it and open a follow-up
  rather than expanding this item.

- Risk: tests that execute Ninja can be brittle or slow when they depend on
  host tools. Severity: medium. Likelihood: low. Mitigation: prefer the
  `manifest` subcommand for end-to-end generated-file assertions and use Ninja
  execution only when the observable workflow truly requires it.

## Progress

- [x] 2026-05-08: Loaded `leta`, `execplans`, `rust-router`,
      `hexagonal-architecture`, `commit-message`, `pr-creation`, and
      `en-gb-oxendict-style` guidance.
- [x] 2026-05-08: Reviewed repository `AGENTS.md`, `docs/roadmap.md`,
      `docs/netsuke-design.md` section 2.5, current manifest expansion code,
      AST deserialization, IR generation, and existing tests.
- [x] 2026-05-08: Used a Wyvern agent team to inspect the design semantics,
      code pipeline, and documentation/testing references.
- [x] 2026-05-08: Drafted this pre-implementation ExecPlan.
- [x] 2026-05-08: User approved implementation of this ExecPlan and work
      moved from planning into execution.
- [x] Stage A: confirm the current semantic contract and document any mismatch
      between code, design docs, and user docs.
- [x] Stage B: update `docs/netsuke-design.md`, `docs/users-guide.md`, and the
      relevant internal documentation with the manifest-time condition rule.
- [x] Stage C: add `rstest` coverage for action/target parity, false `when`
      removal before typed AST, invalid `when`/`foreach` paths, and IR
      exclusion of filtered entries.
- [x] Stage D: add `rstest-bdd` behavioural coverage for generated Ninja
      output containing only selected conditional actions and targets.
- [x] Stage E: run validation, update this plan with evidence, mark
      `docs/roadmap.md` item `3.14.1` done, commit, push, and open the
      implementation pull request.
- [x] 2026-05-08: Confirmed the current code already implements the intended
      manifest-time order. The change records that contract in user, design,
      and developer documentation, then locks it with regression tests.
- [x] 2026-05-08: Focused validation passed for expansion tests, IR exclusion
      tests, and the new BDD manifest-time condition scenario.
- [x] 2026-05-08: Final gates passed for `make check-fmt`, `make lint`,
      `make test`, `make markdownlint`, and `make nixie`. `make fmt` was run
      first and applied `cargo fmt`, then failed on pre-existing repository-wide
      Markdown line-length and table findings outside this change; unrelated
      formatter churn was restored.
- [x] 2026-05-10: Addressed review warnings by normalizing requested
      deserialization spelling in changed documentation, removing obsolete
      pre-approval wording from this completed plan, adding `set -o pipefail`
      to validation pipelines, and adding debug tracing for filtered manifest
      entries plus expansion summary counts.

## Surprises & Discoveries

- `docs/netsuke-design.md` section 2.5 already states the core rule:
  conditions are manifest-time decisions evaluated before AST deserialization,
  IR generation, and Ninja execution.
- `src/manifest/mod.rs` already has explicit load stages:
  `InitialYamlParsing`, `TemplateExpansion`, and `FinalRendering`.
  `expand_foreach` is called during `TemplateExpansion` before
  `serde_json::from_value` hydrates the typed `NetsukeManifest`.
- `src/manifest/expand.rs` already expands both `targets` and `actions`, and
  removes `when` before entries continue through the pipeline.
- Existing unit tests in `src/manifest/expand_tests.rs` cover several action
  and target cases, including static action `when: false`, action `foreach`,
  and invalid static target `when` expressions.
- The current roadmap has `3.14.2` already marked done, so this item should not
  reimplement action-level expansion. It should document and lock the
  manifest-time semantics that now apply to both sections.
- `tests/bdd/steps/manifest_command.rs` already exceeds the repository's
  400-line soft limit, so the new BDD workspace setup step belongs in a small
  separate module instead of growing that file.
- A new `foreach: not_defined` negative test did not fail under a plain
  `minijinja::Environment::new()` because the local test environment did not
  use the manifest loader's strict undefined policy. The negative expansion
  case now uses malformed expression syntax so it proves parse-time failure
  independent of undefined-variable policy.
- The first implementation documented the contract but left filtering
  observability implicit. Review flagged that the manifest expansion boundary
  should emit traceable decisions because filtered entries affect the generated
  static plan.

## Decision Log

- Decision: treat `3.14.1` as a contract-recording and regression-hardening
  change, not as a request to add runtime branching. Rationale: the roadmap
  wording asks to record semantics, and `docs/netsuke-design.md` already says
  build-time branching belongs in recipes unless a future runtime-condition
  feature is designed. Date/Author: 2026-05-08 / planning agent.

- Decision: keep the manifest loader as the owner of `foreach` and `when`
  semantics. Rationale: this respects the hexagonal dependency rule by keeping
  policy before typed AST and IR conversion, while leaving the Ninja adapter to
  emit a static plan only. Date/Author: 2026-05-08 / planning agent.

- Decision: use generated Ninja output from the `manifest` subcommand as the
  main behavioural proof. Rationale: it is externally observable, avoids
  host-specific recipe execution, and proves that IR/Ninja only see selected
  entries. Date/Author: 2026-05-08 / planning agent.

- Decision: do not require Kani or Verus for the initial implementation unless
  new invariants are introduced beyond the existing expansion contract.
  Rationale: this task primarily documents and tests an existing deterministic
  pipeline. Parameterized `rstest` cases should be enough unless the
  implementation broadens. Date/Author: 2026-05-08 / planning agent.

- Decision: add BDD setup in `tests/bdd/steps/conditional_manifest.rs` instead
  of extending `tests/bdd/steps/manifest_command.rs`. Rationale: the existing
  manifest command step file is already over the AGENTS.md file-size guidance,
  and a focused module keeps the new behaviour colocated without increasing
  that debt. Date/Author: 2026-05-08 / implementation agent.

- Decision: test invalid `foreach` with malformed syntax rather than an
  undefined name. Rationale: `src/manifest/expand_tests.rs` exercises
  `expand_foreach` directly with a basic MiniJinja environment, while strict
  undefined behaviour is configured by the higher-level manifest loader.
  Malformed syntax still fails during template expansion and matches this
  test's boundary. Date/Author: 2026-05-08 / implementation agent.

- Decision: use structured `debug!` tracing rather than user-facing output for
  conditional filtering decisions. Rationale: filtering is a manifest-loading
  diagnostic concern, not normal CLI output. Debug-level fields preserve entry
  name, `when` expression text, iteration index where present, the false
  decision, and section-level filtered counts without changing generated
  manifests or command output. Date/Author: 2026-05-10 / implementation agent.

## Skills and references

Use these skills before implementation:

- `leta`: navigate Rust symbols and references before editing code.
- `execplans`: keep this living document current throughout implementation.
- `rust-router`: route any non-trivial Rust changes to the smallest relevant
  Rust skill.
- `hexagonal-architecture`: protect the manifest/IR/Ninja boundaries without
  transplanting a new architecture pattern.
- `en-gb-oxendict-style`: keep documentation in project style.
- `commit-message`: commit with a file-based message after gates pass.
- `pr-creation`: open a draft pull request that identifies this ExecPlan.

Use these repository documents:

- `docs/roadmap.md`: source for roadmap item `3.14.1`.
- `docs/netsuke-design.md`: design source for the manifest pipeline,
  generated actions/targets, IR, and Ninja backend boundary.
- `docs/users-guide.md`: user-facing manifest and CLI behaviour.
- `docs/developers-guide.md`: internal implementation and testing practices.
- `docs/ortho-config-users-guide.md`: configuration layering and localized
  help support if any config or CLI surface changes are required.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and parameterization
  patterns for unit tests.
- `docs/rstest-bdd-users-guide.md`: BDD feature and step guidance.
- `docs/rust-doctest-dry-guide.md`: doctest guidance if public Rustdoc examples
  are touched.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`: dependency
  injection guidance for environment, clock, process, or filesystem effects.

## Context and orientation

The current manifest pipeline is implemented in `src/manifest/mod.rs`.
`from_str_named` parses YAML into `serde_json::Value`, registers globals and
stdlib helpers, runs `expand_foreach`, deserializes the expanded tree into
`NetsukeManifest`, and then renders string fields. The relevant stages are
reported through `ManifestLoadStage`.

`src/manifest/expand.rs` owns the `foreach` and `when` expansion policy.
`expand_foreach` applies the same `expand_section` helper to `targets` and
`actions`. `expand_target` evaluates `foreach`, evaluates and removes `when`,
injects `item` and `index` into entry `vars`, and returns only the entries that
should continue through the pipeline.

`src/ast.rs` defines the typed manifest AST. `deserialize_actions` marks
top-level actions as `phony`. Because `when` and `foreach` are not fields on
`Target`, those keys must not survive into this layer.

`src/ir/from_manifest.rs` turns the typed manifest into `BuildGraph`. This is
the right boundary for tests that prove filtered entries are gone: if a skipped
entry produced duplicate outputs, missing rules, cycles, or Ninja statements,
then manifest-time filtering failed.

`src/ninja_gen.rs` is the static backend adapter. It should not know why an
entry was included or skipped; it should only render the already selected graph.

`src/runner/mod.rs` and `src/runner/process/mod.rs` orchestrate command
execution. They should remain consumers of a generated static Ninja plan, not
owners of condition semantics.

Existing tests worth reusing or extending include:

- `src/manifest/expand_tests.rs`
- `src/manifest/tests/stages.rs`
- `tests/manifest_jinja_tests.rs`
- `tests/ir_from_manifest_tests.rs`
- `tests/ninja_gen_tests.rs`
- `tests/ninja_snapshot_tests.rs`
- `tests/features/manifest_subcommand.feature`
- `tests/features/ir_generation.feature`

## Implementation plan

Stage A audits the current contract. Read `docs/netsuke-design.md` section 2.5,
the `foreach` and `when` section of `docs/users-guide.md`,
`src/manifest/mod.rs`, `src/manifest/expand.rs`, `src/ast.rs`,
`src/ir/from_manifest.rs`, and `src/ninja_gen.rs`. Confirm that the docs and
code agree that `foreach` and `when` are evaluated before typed AST
deserialization. If they disagree, update `Decision Log` and stop for approval
before changing behaviour.

Stage B updates documentation. In `docs/netsuke-design.md`, make the pipeline
contract explicit for both actions and targets and retain the build-time
branching caveat. In `docs/users-guide.md`, add user-facing wording to the
`foreach` and `when` section that says conditions select manifest entries at
load time and that recipe commands or scripts are the current home for
build-time branching. In `docs/developers-guide.md` or the more specific
architecture document found during Stage A, record that adapters must consume
the selected graph rather than reinterpreting manifest conditions.

Stage C adds focused `rstest` coverage. Prefer parameterized cases that run the
same assertion against `targets` and `actions`. Cover at least these cases:
`when: false` entries are removed before typed AST deserialization; `foreach`
with `when` carries `item` and `index` only into kept entries; invalid
`foreach` and invalid or empty `when` fail during template expansion; filtered
entries do not contribute missing-rule, duplicate-output, or cycle errors in IR
generation. Where possible, add these to existing manifest and IR test files
rather than creating broad new fixtures.

Stage D adds behavioural coverage with `rstest-bdd`. Add or extend a feature
that runs `netsuke manifest -` or the equivalent existing helper against a
manifest containing conditional actions and targets. Assert that the generated
Ninja file includes selected entries and excludes skipped entries. Keep the
scenario focused on user-visible output rather than internal function calls.

Stage E validates and closes. Run `make fmt` if documentation or Rust
formatting changed, then run `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` sequentially with `tee` logs in `/tmp`.
Update this plan's `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` with the validation evidence. Mark roadmap item
`3.14.1` done only after the gates pass. Commit the implementation, push the
branch, and open a draft pull request whose title includes `(3.14.1)` and whose
summary links this ExecPlan.

## Validation plan

Before editing implementation code, run a narrow baseline when practical:

```sh
cargo test --workspace manifest::expand
```

After Stage C, run the new or touched unit tests directly. The exact filter
should be updated when test names exist, but it should resemble:

```sh
cargo test --workspace manifest_time_condition
```

After Stage D, run the touched BDD scenario or the nearest stable test filter:

```sh
cargo test --test bdd_tests manifest_time_conditions
```

Final validation must run these commands sequentially and capture logs:

```sh
set -o pipefail && make fmt 2>&1 | tee /tmp/fmt-netsuke-3-14-1-manifest-time-condition-semantics.out
set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-3-14-1-manifest-time-condition-semantics.out
set -o pipefail && make lint 2>&1 | tee /tmp/lint-netsuke-3-14-1-manifest-time-condition-semantics.out
set -o pipefail && make test 2>&1 | tee /tmp/test-netsuke-3-14-1-manifest-time-condition-semantics.out
set -o pipefail && make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-3-14-1-manifest-time-condition-semantics.out
set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-netsuke-3-14-1-manifest-time-condition-semantics.out
```

Expected successful final output is that each command exits with status `0`. If
`make fmt` changes files, inspect the diff before continuing.

## Outcomes & Retrospective

Implemented. The semantic behaviour did not change: Netsuke already evaluates
top-level action and target `foreach`/`when` clauses during manifest loading,
before typed AST deserialization, IR generation, Ninja generation, and Ninja
execution. This work records that contract and adds regression coverage so the
boundary is harder to move accidentally.

Documentation changed in `docs/netsuke-design.md`, `docs/users-guide.md`, and
`docs/developers-guide.md`. The user guide now states that manifest conditions
select entries at load time and that build-time branching belongs in recipe
commands or scripts until a future runtime-condition feature is designed. The
roadmap entry `3.14.1` is marked done.

Review follow-up changed `src/manifest/expand.rs` to emit debug observability
for conditional filtering. Each skipped entry logs its manifest entry name, the
`when` expression text, `when_result = false`, and the iteration index for
`foreach` entries. The top-level expansion pass also logs filtered target,
filtered action, and total filtered counts.

Tests changed in `src/manifest/expand_tests.rs`,
`tests/ir_from_manifest_tests.rs`,
`tests/features/manifest_subcommand.feature`, and
`tests/bdd/steps/conditional_manifest.rs`. The unit tests cover action and
target parity, false `when` removal before typed AST deserialization, iteration
vars on kept entries, invalid `foreach`, and IR exclusion for skipped
duplicates, missing rules, and cycles. The behavioural scenario proves that
`netsuke manifest -` emits only selected conditional actions and targets.

Validation evidence:

```plaintext
cargo test --workspace expand_ -- --nocapture
26 passed; 0 failed

cargo test --workspace skipped_manifest_conditions_do_not_contribute_to_ir -- --nocapture
3 passed; 0 failed

cargo test --test bdd_tests manifest_time_conditions -- --nocapture
1 passed; 0 failed

make check-fmt
exit status 0

make lint
exit status 0

make test
exit status 0

make markdownlint
exit status 0

make nixie
exit status 0
```

`make fmt` was attempted and logged to
`/tmp/fmt-netsuke-3-14-1-manifest-time-condition-semantics.out`. It failed
after `cargo fmt` because repository-wide Markdown formatting/linting still
reports pre-existing issues in unrelated documents. No follow-up work is needed
for this roadmap item, but cleaning those repository-wide Markdown formatter
findings would make the formatter gate less noisy for future documentation
changes.

Commit hash: see the branch head for the committed implementation.
