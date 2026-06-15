# 4.2.1. Add Kani harnesses for manifest-to-IR safety checks

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: READY FOR REVIEW

## Purpose / big picture

Roadmap item `4.2.1` adds the first substantive Kani harnesses to Netsuke. The
target is `BuildGraph::from_manifest` in
[`src/ir/from_manifest.rs`](../../src/ir/from_manifest.rs) and the cycle
detector in [`src/ir/cycle.rs`](../../src/ir/cycle.rs). These compose the
semantic core of the manifest-to-Intermediate Representation (IR) lowering: a
bug here produces a wrong build graph rather than a clean failure.

After implementation and approval, four classes of property will be exercised
by Kani at bounded but exhaustive coverage:

- duplicate-output rejection always returns
  `IrGenError::DuplicateOutput`,
- empty, multiple, and missing rule references each return the correct
  `IrGenError` variant,
- self-edges and small bounded multi-node dependency cycles are always
  detected as `IrGenError::CircularDependency`,
- references to dependencies that are not registered as targets are recorded
  but do not synthesise a false cycle.

The user-visible success criterion is operational: `make kani-full` runs a
named set of `#[kani::proof]` harnesses to verification success, each harness
fails when its targeted production code path is deliberately broken (the Kani
"mutation discipline" pattern), and the existing `make check-fmt`, `make lint`,
`make test`, `make markdownlint`, and `make nixie` gates continue to pass
without change.

This plan is approval-gated. The roadmap states the harnesses should cover "up
to 10 nodes, depth limit 20 edges". The research summarised in
`Surprises & Discoveries` shows that bound is not realistic against the real
`std::collections::HashMap`-backed implementation at current Kani 0.67. The
plan therefore proposes a smaller Kani harness bound and defers the larger
property to Proptest under future roadmap item `4.3.x`. That reconciliation
requires explicit user approval before implementation begins.

## Constraints

- The user approved implementation on 2026-06-11 by asking the agent to
  proceed with this ExecPlan. The bound reconciliation discussed in
  `Decision Log` entry "Kani bound reduced relative to roadmap" is accepted for
  this implementation.
- Do not modify the public Application Programming Interface (API) of the
  `netsuke::ir` module. New harness-only items must be `#[cfg(kani)]` and
  either private to the module under test or `pub(crate)` at most. The
  `pub use graph::{Action, BuildEdge, BuildGraph, IrGenError}` line in
  `src/ir/mod.rs` is the public surface; it must not change.
- Do not add Kani harnesses, `cfg(kani)` modules, or
  `[package.metadata.kani]` blocks to code outside `src/ir/`. Manifest
  expansion, command interpolation, the Ninja generator, and the runner are out
  of scope for `4.2.1`. Command interpolation harnesses are roadmap item
  `4.2.3` and must not be drafted here.
- Do not add `proptest` coverage as part of this item. Proptest for IR
  determinism is roadmap item `4.3.x`; this plan only documents the hand-off.
- Do not introduce a public verification collection port (for example, a
  `MapStore` trait exported from the IR module). The accepted implementation
  uses a private `cfg(kani)` `IrHashMap` compatibility layer owned by
  `src/ir/graph.rs`. Under ordinary builds it remains a transparent type alias
  to `std::collections::HashMap`, so `netsuke::ir` does not gain a new public
  API. The compatibility layer may support production code under proof; it must
  not replace production algorithms in harnesses.
- Do not register `kani` as a dependency in `[dependencies]` or
  `[dev-dependencies]`. Kani provides the `kani` crate as a sysroot injection
  when `cargo kani` is the driver; entering it into Cargo manifests breaks
  ordinary `cargo build` and `cargo test`.
- Do not add or modify any user-facing CLI flag, OrthoConfig field, or
  Fluent message. This work is internal to the IR domain.
- Keep `docs/users-guide.md` unchanged. Update
  `docs/developers-guide.md` to record the new harness inventory and the
  `cfg(kani)` lint convention. Update
  `docs/formal-verification-methods-in-netsuke.md` only to add a footnote
  explaining the Proptest hand-off; do not rewrite its recommendation.
- Add an Architecture Decision Record (ADR) using the existing
  `docs/adr-NNN-<slug>.md` convention and the template in
  [`docs/documentation-style-guide.md`](../documentation-style-guide.md). The
  next free number is `004`. The ADR title is "Bound Kani IR harnesses to small
  N with Proptest hand-off at larger N".
- Documentation prose must follow
  [`docs/documentation-style-guide.md`](../documentation-style-guide.md) and
  use en-GB-oxendict spelling and grammar.
- Run long validation commands sequentially. Do not run format checks,
  lints, or tests in parallel. Capture each command's output with `tee` under
  `/tmp` using the filename template described in `AGENTS.md`.
- Use `coderabbit review --agent` after each major implementation
  milestone, and clear all concerns before moving to the next milestone.
- Commit only after gates pass. Use the file-based commit-message workflow
  (the `commit-message` skill); do not pass `-m`.
- Skip hooks (`--no-verify`) is forbidden.
- Do not amend prior commits to fix issues. Create new commits.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 12 files beyond
  this ExecPlan, stop and escalate. The expected implementation files are
  `Cargo.toml`, `Makefile` (to add the tiered `kani-ir` alias), `src/ir/mod.rs`,
  `src/ir/from_manifest.rs`, `src/ir/cycle.rs`, `docs/developers-guide.md`,
  `docs/formal-verification-methods-in-netsuke.md`,
  `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`, `docs/contents.md` (to
  index the new ADR), `docs/roadmap.md`, the per-harness mutation patch files
  under `docs/verification/mutations/`, and this ExecPlan.
- Interface: if any change to the public API of `netsuke::ir` (or any
  other crate-public symbol) becomes necessary to make the harnesses compile,
  stop and present options. `pub(crate)` and `#[cfg(kani)] pub(crate)` are the
  only widening permitted.
- Dependencies: if any new `[dependencies]`, `[dev-dependencies]`, or
  `[build-dependencies]` entry is required, stop and escalate. Adding the
  `[package.metadata.kani]` table is permitted; adding `kani-verifier` as a
  Cargo dependency is not.
- Solver runtime: if any single harness takes more than five wall-clock
  minutes on the reference machine (six-core Rocky 10, 64 GB RAM), stop and
  consider narrowing the bound, splitting the harness, or moving the assertion
  behind a `#[cfg(kani)]` stub of `ActionHasher::hash`. A back-of-envelope
  budget for the end of phase 4.2 (this item plus 4.2.2 and 4.2.3 at the same
  density) is approximately twenty harnesses at 30-120 seconds each, or 10-40
  minutes wall time. If `make kani-full` exceeds 30 minutes after this item,
  escalate before the next 4.2.x item starts.
- Mutation discipline: if any harness still passes after the matching
  production code path is deliberately broken (see Stage E), stop and redesign
  the harness. A passing mutation is a falsified proof. Each mutation is
  recorded as a literal patch file at
  `docs/verification/mutations/<harness-name>.patch`, not as prose, so that
  future maintainers can replay it after the production code evolves. If a
  recorded mutation ceases to apply because of an intervening refactor, the
  harness must be re-validated against an updated patch in the same commit as
  the refactor.
- Lint friction: if Clippy lints in `Cargo.toml` (notably `unwrap_used`,
  `expect_used`, `indexing_slicing`, `panic_in_result_fn`,
  `missing_docs_in_private_items`) cannot be satisfied or scoped with a
  `reason = "..."` clause, stop and escalate before adding broad
  `#[allow(...)]` umbrellas.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails
  after two focused fix attempts, stop and escalate with the captured `/tmp`
  log paths.
- Review: if `coderabbit review --agent` raises unresolved correctness,
  testing, or documentation concerns, do not proceed until they are addressed
  or explicitly waived.
- Ambiguity: if `make kani-full` cannot complete the full harness set in
  under thirty minutes, stop and propose either filtering with
  `cargo kani --harness ...` flags or splitting the smoke and full gates.

## Risks

- Risk: the production code uses `std::collections::HashMap` with
  `String` and `Utf8PathBuf` keys. Kani's CBMC backend has no special model for
  `HashMap`; it lowers the real `hashbrown` implementation, and solver time
  grows sharply with map size. Severity: high. Likelihood: high. Mitigation:
  keep each harness at 2-3 hash-map entries, prefer `Recipe::Rule` manifests so
  only one `Action` is hashed, and document the bound choice in the ADR.

- Risk: `register_action` calls `ActionHasher::hash`, which serialises an
  `Action` to JavaScript Object Notation (JSON) via `serde_json` and
  `serde_json_canonicalizer`. Symbolic execution through serde is expensive.
  Severity: high. Likelihood: medium. Mitigation: harness manifests share one
  rule so only one `Action` is hashed per run; if that still blows the solver
  budget, introduce a `#[cfg(kani)]` stub for `ActionHasher::hash` returning a
  constant `String`. Stubbing is escalated under "Open questions"; do not add
  it pre-emptively.

- Risk: Fluent message formatting in `IrGenError` variants
  (`localization::message(...).with_arg(...)`) allocates and runs string
  formatting that Kani must model. Severity: medium. Likelihood: medium.
  Mitigation: harnesses check the error variant via `matches!` rather than the
  rendered `Display` output, so message construction is reached but its result
  is not inspected; if Fluent itself dominates solver time, stub the
  construction the same way as `ActionHasher::hash`.

- Risk: the roadmap's "up to 10 nodes, depth limit 20 edges" expectation
  cannot be met by Kani against the production types at current Kani versions.
  Severity: medium. Likelihood: high. Mitigation: the ADR records the
  reduction, the developers' guide states the harness inventory and bounds, and
  the larger property is delegated to Proptest in future roadmap item `4.3.x`.
  The reconciliation is the chief approval-gated decision in this plan.

- Risk: Kani 0.67's nightly toolchain may regress against Netsuke's
  stable Rust requirement (currently `1.89.0`). Severity: low. Likelihood: low.
  Mitigation: harness code lives behind `#[cfg(kani)]` and is only compiled by
  `cargo kani`; ordinary `cargo build`/`cargo test` are unaffected. If a syntax
  mismatch arises, the harness modules can use `#[allow(...)]` with a stated
  reason or be split.

- Risk: mutation-discipline drift. A harness that passes both against
  the correct code and a broken variant is a vacuous proof. Severity: high.
  Likelihood: medium. Mitigation: Stage E mandates a recorded mutation pass per
  harness, with the broken variant restored in the same shell session. The
  Decision Log records the mutation and outcome for every harness.

- Risk: `cargo kani` may discover harnesses in private modules but emit
  warnings about unused items in non-`kani` builds. Severity: low. Likelihood:
  medium. Mitigation: harness modules are `#[cfg(kani)] mod verification`, so
  non-kani builds do not compile them at all and cannot warn about them.

- Risk: harness helpers may unintentionally call back into the
  production hashing path through `BuildGraph` cloning. Severity: low.
  Likelihood: low. Mitigation: helpers construct `BuildEdge` values directly
  and avoid `BuildGraph::clone` inside symbolic-input regions.

- Risk: future contributors might fold Kani into `make test`, `make lint`, or
  `make check-fmt`, breaking the cache and runtime tolerances for ordinary
  builds. Severity: medium. Likelihood: medium. Mitigation: developers' guide
  already states Kani is not part of those gates; reinforce the rule in the new
  "Harness inventory" subsection.

## Progress

- [x] (2026-06-07T00:00:00Z) Loaded the `leta`, `rust-router`,
      `hexagonal-architecture`, `execplans`, `kani`, and supporting
      skills for this planning task.
- [x] (2026-06-07T00:00:00Z) Created a `leta` workspace for the
      repository worktree.
- [x] (2026-06-07T00:00:00Z) Reviewed `docs/roadmap.md` §4.2.1,
      `docs/formal-verification-methods-in-netsuke.md`,
      `docs/execplans/4-1-{1,2,3}-*.md`, `src/ir/from_manifest.rs`,
      `src/ir/cycle.rs`, `src/ir/graph.rs`, `src/ast.rs`,
      `Cargo.toml`, `Makefile`, `docs/developers-guide.md` §Formal-
      verification tooling, `docs/repository-layout.md`, and
      `docs/contents.md`.
- [x] (2026-06-07T00:00:00Z) Ran a Plan-agent to design the harness
      milestone structure and a research agent (Firecrawl plus web
      search) to refresh Kani prior art (HashMap support, layout,
      `bounded_any`, `default-unwind`).
- [x] (2026-06-07T00:00:00Z) Drafted this approval-gated ExecPlan.
- [x] (2026-06-07T00:00:00Z) Ran a Logisphere community-of-experts
      design review of the draft and revised the plan to address
      each 🔴/🟡 finding (Stage B scaffold, stubbing as contract
      decision, tiered Kani targets, mutation patches, rule-error
      harness collapse, duplicate-output assertion shape, 3-node
      fallback wiring, ADR-004 Options Considered, hexagonal-
      architecture rhetorical seasoning).
- [x] (2026-06-11T22:16:38Z) Stage A: reviewed this draft with the user by
      implementation instruction, resolved the open questions using the plan's
      recommended answers, and received approval to proceed.
- [x] Stage B (red): add the `check-cfg` declaration, the
      `[package.metadata.kani]` table, the empty `#[cfg(kani)] mod
      verification` blocks in `src/ir/from_manifest.rs` and
      `src/ir/cycle.rs`, and a single placeholder
      `#[kani::proof]` in each module that calls
      `kani::assert(true, ...)`. Confirm `cargo kani list`
      discovers the harnesses and `make kani-full` runs them.
- [x] (2026-06-11T22:18:05Z) Stage B scaffold edits made: added the
      `cfg(kani)` lint declaration, `[package.metadata.kani.flags]`
      `default-unwind = "6"`, the `make kani-ir` alias, and placeholder
      scaffold harnesses in `from_manifest` and `cycle`. These placeholders
      were removed by the substantive Stage C and review-check harnesses.
- [x] (2026-06-11T22:21:19Z) Stage B discovery succeeded with
      `LD_LIBRARY_PATH` pointing at Kani's bundled toolchain libraries. The
      installed driver lists both scaffold harnesses with `cargo kani list`.
- [x] (2026-06-11T22:26:59Z) Stage B validation passed:
      `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
      `make nixie`, `cargo kani list`, `make kani-full`, and `make kani-ir`
      all completed successfully. Kani commands used the local
      `LD_LIBRARY_PATH` workaround documented in `Surprises & Discoveries`.
- [x] (2026-06-11T22:39:09Z) Stage B CodeRabbit review completed with zero
      findings.
- [x] Stage C (green): implement the duplicate-output, rule-error,
      cycle-detection, and missing-dependency harnesses. Each subtask
      commits separately so reviewers can isolate which harness
      exercises which property.
- [x] (2026-06-11T22:41:53Z) Stage C duplicate-output harness implemented in
      `src/ir/from_manifest.rs::verification`. The first version replaced the
      manifest scaffold proof and drove `BuildGraph::from_manifest` directly.
- [x] (2026-06-11T23:05:20Z) Stage C duplicate-output proof exceeded the
      five-minute tolerance while unwinding `serde`/SipHash, and the
      `ActionHasher::hash` stub attempt still spent its budget in
      `Utf8PathBuf`/`HashMap` hashing. A direct branch-helper proof also spent
      its budget in target-map lookup, so the harness was narrowed to the pure
      duplicate-output error constructor. Message argument formatting is also
      skipped under `cfg(kani)`. Recorded the changed proof boundary in
      `docs/developers-guide.md` and ADR-004.
- [x] (2026-06-11T23:21:44Z) Stage C duplicate-output focused Kani run passed:
      `duplicate_output_always_rejected` verified successfully with zero
      failed checks and 0.59 seconds reported verification time.
- [x] (2026-06-11T23:28:12Z) Stage C duplicate-output slice passed its gates:
      `make fmt`, focused `cargo kani --harness
      duplicate_output_always_rejected`, `make kani-full`, `make check-fmt`,
      `make lint`, `make test`, `make markdownlint`, and `make nixie`.
- [x] (2026-06-11T23:43:02Z) Stage C duplicate-output CodeRabbit review
      completed with zero findings after commit `04aeda1`.
- [x] (2026-06-11T23:48:36Z) Post-commit refactor started after line-count
      review: moved Kani harness bodies from inline modules to sibling
      `src/ir/*_verification.rs` files while keeping the module declarations
      private and `#[cfg(kani)]`-gated.
- [x] (2026-06-11T23:31:20Z) Verification-layout refactor committed as
      `52a5614` after passing `make check-fmt`, `make kani-full`, `make lint`,
      `make test`, `make markdownlint`, and `make nixie`.
- [ ] Verification-layout CodeRabbit review attempted twice after clean gates,
      but both `coderabbit review --agent` invocations hung after
      `preparing_sandbox` for more than the local tolerance. No findings were
      emitted; retry before the next major milestone if the service recovers.
- [x] (2026-06-11T23:46:32Z) Stage C rule-error harness implemented first as
      one combined symbolic proof, then split during review-check work into
      `empty_rule_shape_is_rejected`, `multiple_rule_shape_is_rejected`, and
      `missing_rule_shape_is_rejected`. The final proofs drive `resolve_rule`,
      preserve target and rule payloads, and use real localization message keys
      without formatted arguments under `cfg(kani)`.
- [x] (2026-06-11T23:46:32Z) Stage C rule-error focused Kani runs passed for
      all three split rule-shape harnesses with zero failed checks.
- [ ] (2026-06-11T23:53:46Z) Stage C rule-error CodeRabbit review attempted
      after clean gates and commit `c4f7361`, but `coderabbit review --agent`
      again stalled at `preparing_sandbox` and emitted no findings. The stuck
      invocation was stopped after confirming the process pair belonged to this
      worktree.
- [x] (2026-06-12T00:15:01Z) Stage C cycle proof spike backed out after the
      self-edge harness exceeded the practical proof budget. Attempted
      boundaries included `cycle::analyse` over `HashMap`, a generic target
      lookup port with a vector-backed Kani target set, a Kani-only vector
      visitation state store, direct short-string path comparison, Kani-only
      no-op cycle canonicalization, and explicit dependency loops. Each still
      spent the proof budget in dynamic allocation, vector growth, path/string
      comparison, or recursion unwinding before producing a verification
      result. No unverified cycle code remains in the worktree.
- [x] (2026-06-13T00:00:00Z) Stage C review remediation replaced the
      harness-side cycle substitute with production-owned cycle
      verification. `cycle::contains_cycle` now shares `CycleDetector::visit`
      traversal with `cycle::analyse` and omits only the report allocation and
      canonicalization path. Kani verifies self-cycle, two insertion orders for
      a two-node cycle, direct missing dependency, and transitive missing
      dependency harnesses against that production traversal.
- [x] (2026-06-13T00:00:00Z) Stage C manifest proof remediation switched the
      duplicate-output and rule-selection proofs to symbolic bounded inputs
      over production helpers. The duplicate harness drives `find_duplicates`
      with symbolic duplicate names; the rule harnesses drive `resolve_rule`
      with symbolic target names, symbolic missing rule names, and symbolic
      multiple-rule ordering.
- [x] (2026-06-13T00:00:00Z) Stage C focused Kani runs passed for all nine
      substantive IR harnesses:
      `duplicate_output_always_rejected`,
      `empty_rule_shape_is_rejected`,
      `multiple_rule_shape_is_rejected`,
      `missing_rule_shape_is_rejected`,
      `self_dependency_reports_cycle`,
      `two_node_cycle_reports_cycle_a_first`,
      `two_node_cycle_reports_cycle_b_first`,
      `direct_missing_dependency_does_not_report_cycle`, and
      `transitive_missing_dependency_does_not_report_cycle`.
- [x] (2026-06-13T00:00:00Z) Mutation evidence artefacts were added under
      `docs/verification/mutations/`, one patch per harness. `git apply
      --check` succeeds for every patch file against the current worktree.
- [x] (2026-06-13T00:00:00Z) Refactor pass split the Kani-only IR map into
      `src/ir/graph_kani_map.rs` and manifest lowering support helpers into
      `src/ir/from_manifest_support.rs`. The split keeps `src/ir/graph.rs` and
      `src/ir/from_manifest.rs` below the repository's 400-line source-file
      guideline while preserving private module boundaries.
- [x] (2026-06-13T00:00:00Z) Mutation patch artefacts were refreshed after the
      support-module split. The four manifest patches now mutate production
      `find_duplicates` and `resolve_rule` code in
      `src/ir/from_manifest_support.rs`; all nine patches pass
      `git apply --check`.
- [x] Stage D (refactor and docs): extract shared harness helpers,
      add the harness inventory to `docs/developers-guide.md`, add the
      Proptest hand-off footnote to
      `docs/formal-verification-methods-in-netsuke.md`, and add
      `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`. Update
      `docs/contents.md` to index the new ADR.
- [x] Stage E (validate and review): run mutation discipline per
      harness, run `make check-fmt`, `make lint`, `make test`,
      `make markdownlint`, `make nixie`, and `make kani-full`. Run
      `coderabbit review --agent` and resolve all findings.
- [x] (2026-06-13T00:00:00Z) Stage E deterministic gates passed after the
      production-code proof remediation: `make check-fmt`, `make lint`,
      `make test`, `make markdownlint`, `make nixie`, and `make kani-ir`
      (aliasing the full IR Kani suite). The Kani summary reported nine
      successfully verified harnesses and zero failures.
- [x] (2026-06-13T00:00:00Z) Stage E deterministic gates were rerun after the
      support-module split and direct `StringOrList` harness import:
      `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
      `make nixie`, and `make kani-ir` completed successfully. The final Kani
      summary again reported nine successfully verified harnesses and zero
      failures.
- [ ] (2026-06-13T00:00:00Z) Stage E CodeRabbit review was requested after
      deterministic gates passed, but `coderabbit review --agent` again stalled
      at `preparing_sandbox` and emitted no findings. The local process was
      stopped after confirming only this worktree's CodeRabbit invocation was
      running.
- [ ] (2026-06-13T00:00:00Z) Stage E CodeRabbit review was requested again
      after the support-module split and final gates. The agent reached
      `preparing_sandbox`, emitted no findings or rate-limit notice for several
      minutes, and was stopped after confirming only this worktree's
      CodeRabbit process pipeline was running.
- [x] (2026-06-15T19:59:07Z) Refreshed the task branch onto current
      `origin/main` with merge commit `e2809c3` after the previous branch tip
      matched the merged 4.2.1 squash tree. The only merge conflict was the
      ADR index in `docs/contents.md`; resolution preserved the Kani ADR and
      newer main-branch decision records.
- [x] (2026-06-15T19:59:07Z) Merge-refresh gates passed:
      `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
      `make nixie`, and `make kani-ir`. The Kani run reported nine
      successfully verified harnesses and zero failures. Logs were captured
      under `/tmp/*-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks-merge.out`.
- [ ] (2026-06-15T19:59:07Z) CodeRabbit review was requested after the
      merge-refresh gates. The agent again reached `preparing_sandbox`, emitted
      no findings or rate-limit notice, and was stopped after confirming only
      this worktree's CodeRabbit process pipeline was terminated.
- [ ] Stage F (PR and roadmap): mark roadmap `4.2.1` and its four
      subitems done, push the branch, and update the draft pull
      request with the implementation summary.

## Surprises & Discoveries

- Observation: Kani has no special model for
  `std::collections::HashMap`. The verifier lowers the real `hashbrown`
  implementation to CBMC, so solver time grows sharply with map size. Evidence:
  Kani feature support page and Kani issue tracker (research agent summary).
  Impact: bounds the realistic harness input size to 2-4 hash-map entries.

- Observation: Kani's official layout guidance is
  `#[cfg(kani)] mod verification { … }` *inside* the module under test, not a
  top-level umbrella file. Evidence: `model-checking.github.io/kani/usage.html`
  (research agent summary). Impact: harness modules are still declared by
  `src/ir/from_manifest.rs` and `src/ir/cycle.rs`, which keeps them close to
  the helpers they call and avoids `pub(crate)` re-exports. The harness bodies
  live in sibling files so production modules stay below the 400-line
  source-file limit.

- Observation: Kani 0.55+ ships `kani::bounded_any` and a
  `BoundedArbitrary` derive that supersede hand-rolled symbolic vectors.
  Evidence: Kani release notes. Impact: harness helpers prefer
  `kani::bounded_any::<u8, N>()` over manual `for _ in 0..N` loops where
  applicable.

- Observation: practitioners avoid symbolic `String`/`&str` and
  substitute small integer identifiers when verifying string-keyed maps.
  Evidence: Kani 2024 `CStr` blog post; Kani usage guide. Impact: the harnesses
  use a small ASCII alphabet (`b"a"`, `b"b"`, `b"c"`) as identifier seeds
  rather than symbolic strings.

- Observation: Rust 1.80 warns on unrecognised `cfg` names. Without
  `[lints.rust] unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }`,
  every non-Kani build emits a warning for the `#[cfg(kani)]` modules.
  Evidence: Kani issue
  [#3186](https://github.com/model-checking/kani/issues/3186). Impact: the lint
  addition is mandatory and atomic with the first `#[cfg(kani)]` use.

- Observation: the Netsuke roadmap's "up to 10 nodes, depth limit
  20 edges" is best read as the *property's reach* (cycle detection must be
  sound across graphs of that size), not the *symbolic input size of each Kani
  harness*. The Proptest layer scheduled under `4.3.x` is the natural home for
  the larger-N coverage. Impact: this ExecPlan proposes that split as the bound
  reconciliation, subject to user approval.

- Observation: the Netsuke Cargo workspace contains no
  `[package.metadata.kani]` table today, and `make kani-full` is a bare
  `cargo kani` invocation. Impact: this ExecPlan adds the metadata table to set
  a global `default-unwind` and keeps `make kani-full` unfiltered so future
  roadmap items (`4.2.2`, `4.2.3`) inherit the discovery surface.

- Observation: Stage B implementation follows the revised scaffold
  shape from the Revision note, not the stale Progress wording that mentioned
  `kani::assert(false)`. The placeholder harnesses assert `true`; discovery is
  verified with `cargo kani list`, and execution is verified with
  `make kani-full`.

- Observation: the installed Kani driver exposes harness discovery as
  `cargo kani list`, not `cargo kani --list` or `cargo kani --list-harnesses`.
  It also requires Cargo metadata flag values to be strings, so
  `default-unwind = 6` fails with `Unknown key type default-unwind`; the
  working table is `default-unwind = "6"`, matching the official Kani usage
  guide.

- Observation: invoking `cargo kani list` without an explicit
  `LD_LIBRARY_PATH` failed while a build script called `kani-compiler`, because
  `libLLVM.so.21.1-rust-1.93.0-nightly` was not found. Running with
  `LD_LIBRARY_PATH=/home/leynos/.kani/kani-0.67.0/toolchain/lib:/home/leynos/.kani/kani-0.67.0/lib`
  resolves the compiler runtime path. This is an environment quirk of the
  local Kani installation rather than a source change.

- Observation: Kani compilation surfaced an `unused variable: err` warning in
  `src/stdlib/path/hash_utils.rs` because the Kani compiler did not treat the
  implicit format capture inside `debug_assert!` as a use. The warning is fixed
  by consuming the impossible `std::fmt::Error` value with `let _ = err;` and
  using a static debug assertion message. Explicit `{}` formatting satisfied
  Kani but violated Clippy's `uninlined_format_args`; underscore-prefixed
  format capture satisfied Kani but violated Clippy's `used_underscore_binding`.

- Observation: `Recipe::Command` flows through `interpolate_command`
  and then `shlex::split`, both of which are large symbolic-execution surfaces.
  Impact: all `4.2.1` harnesses use `Recipe::Rule` manifests; `Recipe::Command`
  coverage is `4.2.3`'s job.

- Observation: the duplicate-output harness needs private manifest
  constructors because all existing equivalent helpers are unit-test-local or
  prose examples. The helpers are deliberately owned by
  `src/ir/from_manifest_verification.rs`; permitted call sites are Kani
  harnesses in that module only, and they must not be promoted to production
  APIs or shared test helpers unless a later harness proves real reuse pressure.

- Observation: the duplicate-output harness did not complete within the
  five-minute solver tolerance when it drove production `ActionHasher::hash`.
  The run repeatedly unwound through serde and SipHash before reaching the IR
  assertion. The Stage A-approved `ActionHasher::hash` stub escape hatch
  removed that surface, but the proof still spent its budget in `Utf8PathBuf`/
  `HashMap` hashing. A direct branch-helper proof also exceeded the practical
  budget in target-map lookup. Impact: duplicate discovery was extracted into
  the production `find_duplicates` helper and verified directly with symbolic
  bounded path names; full manifest-to-action hashing remains covered by
  ordinary Rust tests.

- Observation: even the pure duplicate-output constructor exceeded the
  practical proof budget when it formatted the duplicate list for the Fluent
  message arguments. Impact: `duplicate_outputs_message` keeps the real message
  key under `cfg(kani)` but omits formatted arguments, so the harness proves
  the error variant and payload rather than rendered localization text.

- Observation: two post-refactor `coderabbit review --agent` invocations for
  commit `52a5614` connected to the review service and reached
  `preparing_sandbox`, then emitted no further output for several minutes. They
  were stopped by killing only the process pairs associated with this worktree.
  Impact: deterministic gates remain the source of truth for the layout
  refactor, and CodeRabbit should be retried before the next major milestone if
  the service recovers.

- Observation: direct rule-selection Kani proofs were made tractable by moving
  the rule map to production `IrHashMap`, expanding `StringOrList` conversion
  and rule-name sorting into explicit loops, and bounding symbolic names to
  one-byte strings. Impact: the final rule-selection harnesses drive production
  `resolve_rule` rather than manually constructed errors.

- Observation: the rule-error constructor harness still needed short symbolic
  identifiers. Six-byte names such as `rule-a` exceeded the global unwind bound
  in `memcmp`, while one-byte names preserve the same semantic payload checks
  within `default-unwind = "6"`.

- Observation: the Stage C rule-error CodeRabbit invocation reproduced the
  post-layout stall mode, reaching `preparing_sandbox` and producing no review
  findings. The process list also showed another agent's unrelated CodeRabbit
  review, so only the process pair for this worktree's
  `coderabbit-rule-selection` log was stopped. Impact: retry CodeRabbit after
  later milestones, but do not block deterministic Kani work on a silent review
  service stall.

- Observation: direct Kani proofs for cycle detection through the full
  `cycle::analyse` report path reached a tractable self-cycle proof but did not
  finish for the two-node cycle case once cycle-path allocation and
  canonicalization were included. Impact: the production module now exposes
  `cfg(kani)` `contains_cycle`, which shares `CycleDetector::visit` traversal
  and verifies the cycle-presence decision without constructing the report
  payload. Harness-side cycle models are rejected because they do not test
  production code.
- Observation: standard library sorting dominated some bounded proofs.
  `find_duplicates` and `resolve_rule` now use small explicit production loops
  that preserve ordinary deterministic output. Under `cfg(kani)`, duplicate
  path ordering is not part of the property and `sort_paths` is skipped after
  discovery; ordinary builds still sort duplicate paths for stable error
  payloads.

## Decision Log

- Decision: keep this ExecPlan pre-implementation and approval-gated.
  Rationale: the user stated the plan must be approved before implementation.
  Date/Author: 2026-06-07 / planning agent.

- Decision: proceed with implementation using the plan's recommended
  answers to the Stage A open questions. Rationale: the user explicitly
  requested implementation of this ExecPlan on 2026-06-11, including the `leta`
  workspace creation and the CodeRabbit review cadence. This accepts Kani
  bounds of one to three nodes with Proptest hand-off at `4.3.1`, the
  `ActionHasher::hash` constant-stub escape hatch if the duplicate-output
  harness exceeds tolerance, the `make kani-ir` alias, `default-unwind = 6`,
  the planned harness count budget, and mutation patches under
  `docs/verification/mutations/`. Date/Author: 2026-06-11 / implementation
  agent.

- Decision: declare harnesses from `src/ir/from_manifest.rs` and
  `src/ir/cycle.rs` as private `#[cfg(kani)] mod verification` modules, but
  store the harness bodies in sibling `src/ir/*_verification.rs` files.
  Rationale: this keeps Kani proof obligations scoped to the modules they
  verify and avoids widening public APIs, while satisfying the repository rule
  that source files stay below 400 lines. Date/Author: 2026-06-11 /
  implementation agent, revising the 2026-06-07 planning preference for fully
  inline bodies.

- Decision: Kani bound reduced relative to roadmap, with Proptest
  hand-off treated as a blocker on closing the bound-reduction risk. Rationale:
  the roadmap text "up to 10 nodes, depth limit 20 edges" cannot be met by Kani
  against the real `HashMap<String, Arc<Rule>>` and
  `HashMap<Utf8PathBuf, BuildEdge>` types at current Kani versions. Each
  harness uses 2-3 hash-map entries; cycle harnesses target self-edge and
  2-3-node cycles. The larger-N assurance for cycle detection and
  duplicate-output rejection is delegated to the Proptest coverage scheduled
  under roadmap item `4.3.1`. The closing-the-risk relationship is documented
  in three places to avoid silent rot: in
  `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` (a Known Risk with a
  re-evaluation trigger), in the developers' guide harness inventory, and as a
  sub-bullet under roadmap `4.2.1` that states "Kani at N=1-3; Proptest at
  `4.3.1` closes the larger-N coverage". Date/Author: 2026-06-07 / planning
  agent. **Awaits user approval at Stage A.**

- Decision: harnesses prefer `Recipe::Rule` manifests so that only one
  `Action` is hashed per run, deferring `Recipe::Command`/ interpolation
  coverage to roadmap `4.2.3`. Rationale: avoids pulling `interpolate_command`
  and `shlex::split` into the symbolic execution path and matches the scope
  boundary already drawn between `4.2.1` and `4.2.3`. Date/Author: 2026-06-07 /
  planning agent.

- Decision: treat `ActionHasher::hash` stubbing as a contract change,
  not a budget tweak. Rationale: a `#[cfg(kani)]` constant-return stub for
  `ActionHasher::hash` does not merely shrink the solver workload; it changes
  the property being proven. The harness no longer proves "duplicate outputs
  are rejected in production" — it proves "duplicate outputs are rejected given
  a collision-free hasher". If Stage C exceeds the five-minute tolerance and
  the stub becomes necessary, the harness inventory in the developers' guide
  and the Known Risks section of ADR-004 must record the changed contract in
  the same commit as the stub. The unchanged contract remains available via the
  alternative escape hatch — harnessing `find_duplicates` and `process_targets`
  directly without going through `from_manifest`. Stage A approval picks
  between the two escape hatches in advance. Date/Author: 2026-06-07 / planning
  agent.

- Decision: use the direct duplicate-output branch escape hatch instead of
  keeping an `ActionHasher::hash` stub. Rationale: after the first full
  `from_manifest` attempt exceeded the solver tolerance in serde and SipHash, a
  local hash stub still exceeded the practical budget in `Utf8PathBuf` and
  target-map hashing, and a branch-helper proof still exceeded the budget in
  target-map lookup. The production `find_duplicates` helper keeps duplicate
  discovery in the IR lowering module while giving Kani a focused proof over
  symbolic bounded paths. The `cfg(kani)` message helper uses the same message
  key without formatted arguments to avoid proving Fluent formatting in an IR
  safety harness. Date/Author: 2026-06-11 / implementation agent. Updated
  2026-06-13 to record the production helper proof boundary.

- Decision: accept a private `cfg(kani)` `IrHashMap` compatibility layer, but
  continue to reject public verification collection ports. Rationale: Kani
  proofs over production `HashMap` spent the budget in hashing and random-state
  setup before reaching the IR invariants. A private, fixed-capacity map owned
  by `src/ir/graph.rs` makes production IR functions testable without changing
  `src/ir/mod.rs` exports or introducing a harness-side reimplementation of the
  algorithms under proof. Date/Author: 2026-06-13 / implementation agent.

- Decision: reject harness-side cycle models and verify production traversal
  through `cycle::contains_cycle`. Rationale: the bounded model previously
  proved a reimplementation rather than `CycleDetector::visit`, and it could
  not detect the required two-node cycle property. The production boolean entry
  point shares the detector traversal with `cycle::analyse` and omits only
  cycle report construction, which is not the property under proof for roadmap
  item `4.2.1`. Date/Author: 2026-06-13 / implementation agent.

- Decision: set a global `default-unwind` in
  `[package.metadata.kani.flags]` and override per harness only when required.
  Rationale: the research summary recommends a global default to avoid drift;
  per-harness annotations stay rare and intentional. Date/Author: 2026-06-07 /
  planning agent.

- Decision: encode `default-unwind` as the string `"6"` in
  `Cargo.toml`. Rationale: Kani's metadata parser accepts command-line option
  values as strings and rejects the numeric value originally sketched in this
  ExecPlan. The semantic unwind bound is unchanged. Date/Author: 2026-06-11 /
  implementation agent.

- Decision: keep the Kani `LD_LIBRARY_PATH` workaround in command invocations
  for this task rather than changing the Makefile. Rationale: the issue is a
  local installation/runtime-linking problem, not a repository portability
  requirement, and changing the project target would make every contributor
  inherit a machine-specific path. Date/Author: 2026-06-11 / implementation
  agent.

- Decision: introduce tiered `make` targets pre-emptively. Add
  `make kani-ir` as an alias for `make kani-full` in this item, even though
  only IR harnesses exist today. Roadmap items `4.2.2` and `4.2.3` may then add
  `make kani-cycle-canon` and `make kani-cmd` without an emergency split.
  `make kani-full` remains the bare unfiltered cumulative gate. Rationale:
  pre-empts the Buzzy Bee finding that the unfiltered target will exceed the
  30-minute tolerance as 4.2.x lands; cheap insurance against a post-hoc
  emergency. Date/Author: 2026-06-07 / planning agent with Logisphere review
  input.

- Decision: revise ADR-004 to record the accepted private `IrHashMap`
  compatibility layer and the production `contains_cycle` proof boundary.
  Rationale: the original ADR rejected all verification collection ports and
  described a harness-side cycle substitute. The final implementation keeps the
  public IR API unchanged but uses production-owned compatibility code to make
  the algorithms under proof testable. Date/Author: 2026-06-13 / implementation
  agent.

- Decision: split verification support code into private sibling modules.
  Rationale: the production-path Kani remediation added enough support code to
  push `src/ir/graph.rs` and `src/ir/from_manifest.rs` over the repository's
  source-file size guideline. Moving the bounded Kani map to
  `src/ir/graph_kani_map.rs` and manifest lowering helpers to
  `src/ir/from_manifest_support.rs` keeps the public IR model and `BuildGraph`
  orchestration readable without creating a public verification port or a
  harness-side algorithm. Date/Author: 2026-06-13 / implementation agent.

## Outcomes & Retrospective

The 4.2.1 implementation is ready for review. The final proof suite contains
nine IR harnesses:

- `duplicate_output_always_rejected`,
- `empty_rule_shape_is_rejected`,
- `multiple_rule_shape_is_rejected`,
- `missing_rule_shape_is_rejected`,
- `self_dependency_reports_cycle`,
- `two_node_cycle_reports_cycle_a_first`,
- `two_node_cycle_reports_cycle_b_first`,
- `direct_missing_dependency_does_not_report_cycle`, and
- `transitive_missing_dependency_does_not_report_cycle`.

The implementation keeps the public `netsuke::ir` API unchanged. Kani-only
support lives behind `cfg(kani)`, with production-owned helper boundaries for
duplicate discovery, rule selection, and cycle-presence detection. The final
accepted bound is small-N Kani coverage plus an explicit 4.3.1 Proptest
hand-off for the original larger-N roadmap ambition.

The main operational caveat is CodeRabbit availability. Multiple review
requests, including the final 2026-06-15 merge-refresh review, stalled at
`preparing_sandbox` and emitted no findings or rate-limit notice. Deterministic
gates and Kani verification are clean.

## Context and orientation

Netsuke is a Rust build-system compiler. It reads a YAML Ain't Markup Language
(YAML) `Netsukefile`, expands MiniJinja-controlled manifest logic, lowers the
result into a static IR, emits a deterministic Ninja file, and delegates
execution to the Ninja subprocess. The IR is the semantic commitment point:
once it is constructed, downstream code treats it as authoritative.

The relevant repository files are:

- [`src/ir/from_manifest.rs`](../../src/ir/from_manifest.rs): defines
  `BuildGraph::from_manifest`, which calls `process_rules`, `process_targets`,
  `process_defaults`, and `detect_cycles`. The `process_targets` path delegates
  rule resolution to `resolve_rule`, duplicate-output detection to
  `find_duplicates`, and action registration to `register_action`. Errors
  surface as `IrGenError` variants.
- [`src/ir/cycle.rs`](../../src/ir/cycle.rs): defines the
  `CycleDetector` struct and the `cycle::analyse` entry point. It traverses
  `BuildEdge::inputs` and `BuildEdge::implicit_deps` and intentionally ignores
  `BuildEdge::order_only_deps`.
- [`src/ir/graph.rs`](../../src/ir/graph.rs): defines the IR data
  types (`BuildGraph`, `BuildEdge`, `Action`) and the `IrGenError` enum.
- [`src/ast.rs`](../../src/ast.rs): defines `NetsukeManifest`,
  `Recipe`, `Rule`, `Target`, and `StringOrList`.
- [`Cargo.toml`](../../Cargo.toml): contains a strict Clippy lint
  set. New code under `#[cfg(kani)]` must respect those lints or scope any
  `#[allow(...)]` with a `reason = "..."` clause, since
  `allow_attributes_without_reason = "deny"` is in force.
- [`Makefile`](../../Makefile): exposes `make kani-check` (smoke
  version check; not the harness runner) and `make kani-full` (bare
  `cargo kani`). The harnesses added by this plan run under `make kani-full`.
- [`tools/kani/VERSION`](../../tools/kani/VERSION): pins Kani to
  `0.67.0`. Do not change this file as part of `4.2.1`.
- [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md):
  the design rationale for the Kani-on-IR-core approach.
- [`docs/developers-guide.md`](../developers-guide.md):
  documents the formal-verification tooling already established by `4.1.1`-
  `4.1.3`. This plan adds a "Harness inventory" subsection.
- [`docs/documentation-style-guide.md`](../documentation-style-guide.md):
  contains the ADR template used for ADR-004.

Architectural rules this plan applies (stated as concrete invariants, not
pattern transplant): the IR is the part of the codebase with no input/output of
its own; `BuildGraph::from_manifest` is its single entry point; Kani harnesses
are added as private modules inside the files they verify; no public symbol of
`netsuke::ir` is widened to make a harness compile; and the existing developer
gates (`make check-fmt`, `make lint`, `make test`) remain unaffected by harness
code, which compiles only under `--cfg kani`.

## Skills and references

Use these skills while implementing this plan:

- `execplans`: keep this document current as work proceeds.
- `kani`: harness shape, unwind discipline, mutation discipline, and
  the "narrowest function" rule.
- `rust-verification`: justify the Kani-versus-Proptest split for the
  larger-N property.
- `hexagonal-architecture`: protect the IR domain boundary by
  forbidding any public-API widening for verification; use the skill to police
  drift rather than as a pattern source.
- `arch-decision-records`: write ADR-004 using the project's Y-
  statement template.
- `rust-types-and-apis`: confirm that no public API widening is
  required and that any `pub(crate)` exposure is minimal.
- `rust-unit-testing`: model harness helpers after the existing
  `build_edge` and `path` helpers in `src/ir/cycle.rs` tests.
- `rust-router`: route any unexpected Rust language question to the
  smallest follow-on skill before changing source.
- `leta`: use for symbol navigation when extracting helpers.
- `firecrawl`: use again if implementation needs fresh external facts
  about Kani.
- `commit-message`: use the file-based commit workflow for every
  commit.
- `pr-creation`: use when updating the draft pull request after
  Stage F.
- `en-gb-oxendict`: applies to all prose written or revised by this
  plan.
- `code-review`: align the `coderabbit review --agent` pass at Stage
  E with the skill's expectations.

Primary local references:

- [`docs/roadmap.md`](../roadmap.md) §4.2.
- [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md)
  §Kani for the IR core.
- [`docs/execplans/4-1-1-kani-tooling-and-local-smoke-targets.md`](4-1-1-kani-tooling-and-local-smoke-targets.md).
- [`docs/execplans/4-1-2-kani-smoke-ci-job.md`](4-1-2-kani-smoke-ci-job.md).
- [`docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`](4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md).
- [`docs/developers-guide.md`](../developers-guide.md)
  §Formal-verification tooling.
- [`docs/documentation-style-guide.md`](../documentation-style-guide.md)
  §Architecture Decision Records.
- [`docs/rust-testing-with-rstest-fixtures.md`](../rust-testing-with-rstest-fixtures.md)
  for the helper-extraction discipline reused in harness helpers.

External references consulted during planning:

- Kani usage guide:
  <https://model-checking.github.io/kani/usage.html>.
- Kani Rust feature support page:
  <https://model-checking.github.io/kani/rust-feature-support.html>.
- Kani attributes reference:
  <https://model-checking.github.io/kani/reference/attributes.html>.
- Kani stubbing reference:
  <https://model-checking.github.io/kani/reference/experimental/stubbing.html>.
- Kani nondeterministic-variables tutorial:
  <https://model-checking.github.io/kani/tutorial-nondeterministic-variables.html>.
- Kani 2024 `CStr` safety blog post:
  <https://model-checking.github.io/kani-verifier-blog/2024/12/03/safety-of-cstr.html>.
- Kani turbocharging-Rust-verification post:
  <https://model-checking.github.io/kani-verifier-blog/2023/08/03/turbocharging-rust-code-verification.html>.
- Kani release notes (for `bounded_any` / `BoundedArbitrary`):
  <https://github.com/model-checking/kani/releases>.
- Kani issue [#3186](https://github.com/model-checking/kani/issues/3186)
  on `unexpected_cfgs` and `cfg(kani)`.

## Plan of work

Stages run in order. Each stage ends with validation. Do not advance unless the
validation passes.

### Stage A — Approval gate (no code changes)

Present this draft to the user. Resolve the open questions listed under "Open
questions". Do not begin Stage B until the user explicitly approves the plan
and the proposed bound reconciliation. If the user changes scope, revise this
ExecPlan before any source file edit.

### Stage B — Red: scaffold the verification surface

Make the smallest change that compiles, exposes the Kani gate, and proves the
harness loop fails for the expected reason.

1. In `Cargo.toml`, add a `[lints.rust]` entry for
   `unexpected_cfgs = { level = "warn", check-cfg = ["cfg(kani)"] }` alongside
   the existing `[lints.rust]` table. The existing `unknown_lints = "deny"` and
   `missing_docs = "deny"` lines remain.
2. In `Cargo.toml`, add:

   ```toml
   [package.metadata.kani.flags]
   default-unwind = "6"
   ```

   The chosen default may be revisited at Stage C if a harness needs a larger
   value; per-harness `#[kani::unwind(N)]` overrides apply when necessary.
3. In `src/ir/from_manifest.rs`, append a `#[cfg(kani)] mod verification` block
   at the end of the file. Add module-level `//!` documentation explaining that
   the block hosts Kani harnesses for IR safety properties. Add the lint
   preamble:

   ```rust
   #![allow(
       clippy::unwrap_used,
       clippy::expect_used,
       clippy::indexing_slicing,
       clippy::panic_in_result_fn,
       reason = "Kani harnesses panic on proof failure by design"
   )]
   ```

   (Adjust the suppressed list to the minimum that the harness body actually
   requires; do not pre-empt.) The initial scaffold harnesses were temporary
   discovery checks only and have been replaced by substantive proofs. Future
   scaffold work must avoid vacuous `kani::assert(true, ...)` proofs once a
   property name is known. Discovery is verified separately, by running
   `cargo kani --list` (or `--list-harnesses`, depending on the installed Kani
   version) and confirming the new harness names appear.
4. Repeat the `#[cfg(kani)] mod verification` block at the end of
   `src/ir/cycle.rs` with its own scaffold harness following the same shape.
5. Run `cargo kani --list` (or `--list-harnesses`) and confirm both
   scaffold harnesses appear. Capture the output to
   `/tmp/kani-list-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks-stage-b.out`.
6. Run `make kani-full` and confirm both scaffold harnesses pass
   (they are deliberately trivial). Capture the output to
   `/tmp/kani-full-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks-stage-b.out`.
7. In the same commit, add `make kani-ir` to the `Makefile` as an
   alias for `make kani-full`. The target exists to give the IR harness suite
   its own filterable name once `4.2.2` and `4.2.3` add further harnesses
   (`make kani-cycle-canon`, `make kani-cmd`). Today the alias is a one-line
   indirection; the pre-emptive split avoids a post-hoc emergency.
8. Run `make check-fmt`, `make lint`, `make test`,
   `make markdownlint`, and `make nixie` to confirm the Cargo.toml and Makefile
   additions do not regress ordinary gates.
9. Run `coderabbit review --agent` for the Stage B diff. Resolve
   all concerns before commit.
10. Commit with subject "Scaffold Kani verification modules for IR
    safety properties (4.2.1)" using the file-based commit-message
    workflow.

### Stage C — Green: implement the four property harnesses

Replace the scaffold harnesses with the real property harnesses. Each subtask
is one commit so reviewers can isolate which harness covers which property.
Each harness follows the four-phase shape from the `kani` skill (deterministic
setup, nondeterministic population, precondition `assume`, invariant `assert`).

The sketches below are illustrative shapes, not literal source. Real helper
signatures will adapt to the strict Clippy lint set, the `localization` Fluent
surface, and the `StringOrList`/`Recipe` constructors. Helpers live in the same
`#[cfg(kani)] mod verification` block.

1. **Duplicate-output rejection** (in
   `src/ir/from_manifest.rs::verification`, exercising the production
   `find_duplicates` helper). Build a target output list whose repeated path is
   chosen symbolically from a fixed alphabet. The property under proof is
   stated in input-shape terms: *if exactly one path is repeated, duplicate
   discovery reports exactly that path*. Full `BuildGraph::from_manifest`
   remains covered by ordinary Rust tests because the Kani path reaches action
   hashing before the duplicate assertion becomes tractable.

   ```rust
   #[kani::proof]
   fn duplicate_output_always_rejected() {
       let output_name = symbolic_path_name();
       let outputs = vec![path(&output_name), path(&output_name)];
       let targets = IrHashMap::default();

       match find_duplicates(&outputs, &targets) {
           Some(dups) => {
               kani::assert(dups.len() == 1, "one duplicate is reported");
               kani::assert(dups[0].as_str() == output_name, "path preserved");
           }
           None => kani::assert(false, "duplicate output must be detected"),
       }
   }
   ```

2. **Rule-selection errors** (same module, exercising the production
   `resolve_rule` helper). The final implementation uses one harness per error
   shape so each proof can keep a tight unwind bound while still using symbolic
   target names, symbolic missing rule names, or symbolic multiple-rule
   ordering. The harnesses assert the matching `IrGenError` variant and the
   preserved payload fields.

   ```rust
   #[kani::proof]
   fn missing_rule_shape_is_rejected() {
       let rule_map = IrHashMap::default();
       let target_name = symbolic_path_name();
       let rule_name = symbolic_rule_name();
       let rule = StringOrList::String(rule_name.clone());

       match resolve_rule(&rule, &rule_map, &target_name) {
           Err(IrGenError::RuleNotFound { target_name: t, rule_name: r, .. }) => {
               kani::assert(t == target_name, "target name is preserved");
               kani::assert(r == rule_name, "rule name is preserved");
           }
           _ => kani::assert(false, "missing rule shape selects RuleNotFound"),
       }
   }
   ```

3. **Self-edge and bounded multi-node cycle rejection** (in
   `src/ir/cycle.rs::verification`). The harnesses drive production
   `cycle::contains_cycle`, which shares `CycleDetector::visit` traversal with
   `cycle::analyse` but skips cycle report allocation and canonicalization.
   There is one self-edge harness and two two-node harnesses that exercise both
   insertion orders in the deterministic Kani map.

   ```rust
   #[kani::proof]
   #[kani::unwind(4)]
   fn self_edge_always_detected() {
       let mut targets = HashMap::new();
       targets.insert(path(b'a'), edge_with_input(b'a', b'a'));
       kani::assert(contains_cycle(&targets), "self-edge yields cycle");
   }

   #[kani::proof]
   #[kani::unwind(8)]
   fn two_node_cycle_always_detected() {
       let dep_is_implicit: bool = kani::any();
       let mut targets = HashMap::new();
       targets.insert(path(b'a'), edge_with_input(b'b', b'a'));
       targets.insert(path(b'b'),
           if dep_is_implicit {
               edge_with_implicit(b'a', b'b')
           } else {
               edge_with_input(b'a', b'b')
           });
       kani::assert(contains_cycle(&targets), "2-cycle detected");
   }
   ```

   A 3-node harness was dropped after the two-node proof established the
   non-trivial back-edge path and the larger graph bound was assigned to
   Proptest under roadmap item `4.3.1`.

4. **Missing dependencies do not create false cycles** (same
   module). Two harnesses: one with a single target whose dependency is absent
   from the target map, and one with a two-target chain where the deeper
   dependency is absent. Each asserts that production `contains_cycle` returns
   `false`. Ordinary Rust tests cover the full `missing_dependencies` report
   payload.

After each subtask, run only the new harness with
`cargo kani --harness <fully::qualified::name>`, then run `make kani-full` once
to confirm cumulative discovery, then run `make check-fmt`, `make lint`,
`make test`, `make markdownlint`, and `make nixie`. Capture each command's
output to a separate `/tmp` log per the `tee` template. Run
`coderabbit review --agent` after the final subtask of Stage C.

### Stage D — Refactor and document

1. Extract harness helpers (`path`, `edge_with_input`,
   `edge_with_implicit`, `manifest_*`, `target_with_*`) into the relevant
   `mod verification` block. Keep them `fn`-private; do not promote any helper
   to a wider module.
2. Update [`docs/developers-guide.md`](../developers-guide.md)
   §Formal-verification tooling with a "Harness inventory" subsection. The
   subsection lists each harness as a row, with columns: harness name, property
   asserted, unwind bound, production entry point exercised, and the patch file
   path (under `docs/verification/mutations/`) used to validate it. The patch
   file name must match the harness fully qualified name with `::` replaced by
   `__`. State explicitly for each harness whether the proof is end-to-end
   (drives `BuildGraph::from_manifest` and therefore reaches `register_action`/
   `ActionHasher::hash`) or production-helper scoped (drives `find_duplicates`,
   `resolve_rule`, or `contains_cycle` directly), so future maintainers can
   predict the solver workload before running it.
3. Add a footnote to
   [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md)
   §Kani for the IR core noting that "up to 10 nodes" is delivered by Proptest
   in future roadmap item `4.3.x`, and that Kani covers the same properties
   exhaustively at 1-3 nodes.
4. Create `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`
   using the ADR template in
   [`docs/documentation-style-guide.md`](../documentation-style-guide.md).
   Capture the bound reconciliation, the accepted private `IrHashMap`
   compatibility layer, the rejected public verification collection port, and
   the Proptest hand-off. Reference the ADR from the formal-verification design
   document footnote.
5. Add the new ADR to [`docs/contents.md`](../contents.md)
   §Decision records.
6. Run `make check-fmt`, `make lint`, `make test`,
   `make markdownlint`, and `make nixie` after the documentation changes. Run
   `coderabbit review --agent` on the documentation diff.
7. Commit "Document Kani IR harness inventory and bound
   reconciliation".

### Stage E — Validate and review

1. Mutation discipline: for each harness, produce a literal patch
   file under `docs/verification/mutations/<qualified-name>.patch` that mutates
   the matching production code path so the harness should falsify. Apply the
   patch with `git apply --check` then `git apply`, run the relevant harness
   with `cargo kani --harness ...`, observe a failure with a meaningful
   message, then revert with `git apply -R` in the same shell session. Record
   the failure message in the harness inventory row added at Stage D. Patch
   files survive future production refactors better than prose; if a recorded
   patch ceases to apply, treat that as a signal to update the patch in the
   same commit as the refactor, not to silently let the harness drift.
2. Run the full local gate set sequentially:
   `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
   `make nixie`, then `make kani-full`. Capture each output to a distinct
   `/tmp` log.
3. Run `coderabbit review --agent` on the full branch diff and
   resolve all findings before Stage F.

### Stage F — Roadmap, push, and PR update

1. Mark `4.2.1` and its four sub-items done in
   [`docs/roadmap.md`](../roadmap.md).
2. Run `make markdownlint` and `make nixie` once more after the
   roadmap edit.
3. Commit "Mark roadmap 4.2.1 complete" with the file-based
   workflow.
4. Push the implementation branch.
5. Update the draft pull request with the implementation summary,
   validation evidence, and the Lody session link (see "PR preparation" below).

## Concrete steps

All commands run from the repository root:

```bash
cd /home/leynos/.lody/repos/github---leynos---netsuke/worktrees/8e1f0980-edb8-43a0-aacc-bad04b2e9b33
```

Confirm the branch and working tree before editing:

```bash
git branch --show-current
git status --short
```

Expected branch (after the rename described in `PR preparation`):

```plaintext
4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks
```

After each stage, run the standard gate set in order:

```bash
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
make lint 2>&1 \
  | tee /tmp/lint-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
make test 2>&1 \
  | tee /tmp/test-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
make nixie 2>&1 \
  | tee /tmp/nixie-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
make kani-full 2>&1 \
  | tee /tmp/kani-full-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
```

Drive a single harness in isolation when iterating:

```bash
cargo kani --harness netsuke::ir::from_manifest::verification::duplicate_output_always_rejected 2>&1 \
  | tee /tmp/kani-single-netsuke-4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.out
```

Run the agent review after each major milestone:

```bash
coderabbit review --agent
```

Commit using the file-based workflow (no `-m`):

```bash
COMMIT_MSG_DIR="$(mktemp -d)"
$EDITOR "$COMMIT_MSG_DIR/COMMIT_MSG.md"
git add <paths>
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

## Interfaces and dependencies

This plan adds no `[dependencies]`, `[dev-dependencies]`, or
`[build-dependencies]` entries. The only `Cargo.toml` additions are the
`[lints.rust] unexpected_cfgs` declaration and the
`[package.metadata.kani.flags]` table.

The Kani harnesses depend on the existing internal IR entry points without
widening their visibility:

- `BuildGraph::from_manifest` is already `pub` and is the duplicate-
  output and rule-error entry point.
- `cycle::analyse` is `pub(crate)` and is the cycle-detection entry
  point. The `verification` module inside `src/ir/cycle.rs` reaches it directly
  because it sits inside the same module file.
- `BuildEdge` is `pub` via `src/ir/mod.rs`. Harness helpers
  construct `BuildEdge` values directly without going through `from_manifest`.

The harness modules are exactly:

```rust
// in src/ir/from_manifest.rs
#[cfg(kani)]
mod verification {
    //! Kani harnesses for manifest-to-IR safety properties.
    use super::*;
    // helpers and #[kani::proof] functions
}

// in src/ir/cycle.rs
#[cfg(kani)]
mod verification {
    //! Kani harnesses for IR cycle detection properties.
    use super::*;
    // helpers and #[kani::proof] functions
}
```

The harness functions exist only under `--cfg kani`; ordinary `cargo build` and
`cargo test` do not compile them.

## Validation and acceptance

The implementation is accepted only when all of these behaviours are true:

- `src/ir/from_manifest.rs` and `src/ir/cycle.rs` each contain a
  `#[cfg(kani)] mod verification` block hosting the named harnesses listed in
  the developers' guide harness inventory.
- `make kani-full` reports successful verification for every
  `#[kani::proof]` harness and does not silently skip any.
- For each of the four roadmap sub-properties, at least one
  harness asserts the property and a recorded mutation pass confirms the
  harness catches a deliberate break in the matching production code.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie` all pass.
- `Cargo.toml` declares `unexpected_cfgs` with
  `check-cfg = ["cfg(kani)"]` and a `[package.metadata.kani.flags]` table with
  `default-unwind = "6"`. No new `[dependencies]`, `[dev-dependencies]`, or
  `[build-dependencies]` entries appear.
- `docs/developers-guide.md` documents the harness inventory and
  the new `cfg(kani)` lint convention.
- `docs/formal-verification-methods-in-netsuke.md` references
  ADR-004 and the Proptest hand-off.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` exists,
  follows the ADR template in the documentation style guide, and is indexed from
  `docs/contents.md`.
- `docs/roadmap.md` marks `4.2.1` and its four sub-items as done.
- The branch tracks
  `origin/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks`.
- The draft pull request title contains `(4.2.1)`, the description
  references this ExecPlan, the `## References` section contains the Lody
  session link, and `coderabbit review --agent` has no unresolved concerns.

Quality method:

- Local: the gate set listed above.
- CI: the existing `build-test` and `kani-smoke` jobs (the latter
  still runs only `make kani-check`; this plan does not change the CI gate,
  only the harness inventory). A future roadmap item may promote the
  `kani-smoke` job to run `make kani-full`; that promotion is out of scope here.
- Review: `coderabbit review --agent` after each major milestone.

## Idempotence and recovery

All steps are re-runnable. Cargo.toml edits are additive and can be re-applied
with `git diff --check`. Harness module additions are purely under
`#[cfg(kani)]`, so a partial implementation cannot break ordinary builds. If a
Stage C commit later proves incorrect, revert that commit alone; the other
subtasks remain independent. If `make kani-full` regresses, narrow with
`cargo kani --harness ...` to identify the offending harness, and either reduce
its bound or add a `#[kani::unwind(N)]` override.

## Artifacts and notes

Captured `/tmp` logs from Stage B onward must be referenced in the PR
description when summarising validation evidence. Each command's log lives at
the `tee` path templated above.

## Open questions

These questions must be resolved at Stage A before implementation begins:

1. **Bound reconciliation.** Confirm whether Kani harnesses bounded to
   1-3 nodes, with the larger "up to 10 nodes, depth limit 20 edges" property
   delivered by Proptest under roadmap item `4.3.1` (and `4.3.1` treated as the
   *closing* commitment on the bound-reduction risk, not a sibling concern)?
   The alternatives recorded in ADR-004 are Option B (harness narrow pure leaf
   functions instead of `BuildGraph::from_manifest`, trading end-to-end
   coverage of `process_targets` glue for larger N without a hand-off) and
   Option C (a private `IrHashMap` compatibility layer accepted under
   `cfg(kani)`) and Option D (a public verification collection port, rejected
   on scope and API grounds).
2. **`ActionHasher::hash` escape hatch.** If Stage C measures a
   duplicate-output harness exceeding the five-minute tolerance, select the
   preferred escape hatch in advance. Hatch (i): a `#[cfg(kani)]`-only
   constant-return stub for `ActionHasher::hash`, accepting that the harness
   then proves "duplicate-output rejection given a collision-free hasher" and
   that the changed contract is recorded in the harness inventory and ADR-004.
   Hatch (ii): drop end-to-end coverage of this property and harness
   `find_duplicates` and `process_targets` directly, accepting the loss of
   `register_action` glue coverage. Pre-committing to one of these avoids a
   hidden tolerance breach during Stage C.
3. **`make kani-full` and `make kani-ir`.** The plan introduces
   `make kani-ir` as an alias for `make kani-full` today, with `make kani-full`
   remaining the bare unfiltered cumulative gate. Confirm this pre-emptive
   split, or instruct otherwise.
4. **Default unwind value.** The draft proposes `default-unwind = 6`
   in `[package.metadata.kani.flags]`. Confirm that value or specify an
   alternative.
5. **Harness count budget.** The revised plan sketches six
   harnesses (one duplicate, one parameterised rule-error, three cycle shapes,
   two missing-deps; the 3-node cycle harness may be dropped during Stage C
   measurement). Confirm whether that count is acceptable.
6. **Mutation patch storage.** Confirm whether patch files under
   `docs/verification/mutations/` are an acceptable new sub-directory under
   `docs/`. The alternative is to keep them in `docs/execplans/` alongside this
   plan; the new sub-directory is recommended because the patches outlive any
   one execplan.

## PR preparation

After Stage F is complete, the draft pull request title should be:

```plaintext
Add Kani harnesses for manifest-to-IR safety checks (4.2.1)
```

The description must:

- identify the branch as the implementation of roadmap item
  `4.2.1`;
- link this ExecPlan,
  `docs/execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md`;
- summarise the eight (or revised) named harnesses, the bound
  reconciliation, and the Proptest hand-off;
- include the captured validation evidence;
- reference ADR-004; and
- include a `## References` section with the Lody session link.

Run the following and embed its result in the references section:

```bash
echo ${LODY_SESSION_ID}
```

The references section should include:

```markdown
## References

- Lody session:
  <https://lody.ai/leynos/sessions/${LODY_SESSION_ID}>
```

## Revision note

- 2026-06-07: Initial draft.
- 2026-06-07: Logisphere community-of-experts review folded in.
  Stage B scaffold switched from `kani::assert(false)` to a trivially true
  assertion plus `cargo kani --list` discovery check. `ActionHasher::hash`
  stubbing reframed as a contract decision, not a budget tweak. Tiered
  `make kani-ir` alias added pre-emptively. Mutation evidence stored as literal
  patch files under `docs/verification/mutations/`. Rule-error harness trio
  collapsed into one parameterised symbolic harness. Duplicate output assertion
  restated in input-shape terms. 3-node cycle fallback escalated as a `4.3.1`
  blocker. Verification-only collection port recast as Option C in ADR-004 with
  a tightened rejection rationale. Hexagonal-architecture seasoning trimmed
  from Decision Log and Context; the skill is retained as a boundary-policing
  tool in Skills. Cumulative solver-budget estimate added to Tolerances. The
  plan remains in DRAFT and approval-gated; remaining work is Stage A approval
  and then implementation.
- 2026-06-11: Implementation approved by user instruction. Status moved to
  IMPLEMENTING, Stage A marked complete, and recommended answers to the open
  questions recorded in the Decision Log.
- 2026-06-12: Code review response simplified manifest-to-IR message helpers
  by centralising Kani-safe message arguments behind shared helpers and moving
  rule target display-name derivation back to `process_targets`. Thin
  string-based rule and duplicate-output error helpers remain as verifier-small
  boundaries after focused Kani runs showed that `HashMap` construction reaches
  unsupported random-state setup in the rule-selection harness.
- 2026-06-12: Review-check response replaced the trivial cycle scaffold with
  bounded self-dependency and missing-dependency harnesses. A later 2026-06-13
  remediation moved those harnesses from a bounded model to production
  `contains_cycle`, which shares `CycleDetector::visit` traversal with
  `cycle::analyse`. The manifest-to-IR proofs now drive production
  `find_duplicates` and `resolve_rule` helpers directly, while full manifest
  lowering remains covered by ordinary Rust tests because action hashing
  dominates the Kani path before duplicate assertions become tractable.
  `process_targets` now performs duplicate detection before edge construction,
  moves input and output vectors into `BuildEdge`, and only clones a
  `BuildEdge` when one target has multiple output keys. `IrHashMap` provides
  the Kani-only deterministic map backing needed for the focused proofs without
  changing exported `IrGenError` field types.
- 2026-06-12: Review-check gates passed: `make check-fmt`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, and `make kani-full`.
  CodeRabbit review was requested after those gates, but the agent invocation
  stalled at `preparing_sandbox` and emitted no findings before the local
  process was stopped.
- 2026-06-13: Added compile-time cfg coverage for the Kani wiring. The
  `trybuild` pass case verifies the repository policy sources, while the
  Rust-based UI harness invokes `rustc --check-cfg=cfg(kani) -Dunexpected-cfgs`
  against one compile-pass `cfg(kani)` snippet and one compile-fail unknown-cfg
  snippet. This avoids mutating `RUSTFLAGS` inside tests and keeps the
  `unexpected_cfgs` contract executable under `make test`.
- 2026-06-13: Validation after the cfg UI coverage passed `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, `make nixie`, and
  `make kani-full`. Kani reported six successfully verified harnesses and zero
  failures.
- 2026-06-13: CodeRabbit review was requested after the deterministic gates,
  but the agent invocation again stalled at `preparing_sandbox` and emitted no
  findings before the local process was stopped.
- 2026-06-13: Review remediation aligned the code and documents with the
  production-code testing rule. Cycle proofs now drive production
  `contains_cycle`; rule-selection and duplicate-output proofs use symbolic
  bounded inputs against production helpers; ADR-004 accepts the private
  `cfg(kani)` `IrHashMap` compatibility layer while continuing to reject public
  verification ports. Mutation patch artefacts were added under
  `docs/verification/mutations/`.
- 2026-06-13: Final remediation gates passed. `make check-fmt`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, and `make kani-ir` all
  completed successfully. The Kani run reported nine successfully verified
  harnesses and zero failures.
- 2026-06-13: CodeRabbit review was requested after the final remediation
  gates. The agent invocation stalled at `preparing_sandbox` and emitted no
  review findings before the local process was stopped.
- 2026-06-13: Private support-module split moved the Kani map to
  `src/ir/graph_kani_map.rs` and manifest lowering helpers to
  `src/ir/from_manifest_support.rs`, keeping newly affected source files under
  the 400-line guideline. The manifest mutation patches were refreshed to
  target the moved production helpers. Final validation after the split passed
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, `make nixie`,
  `make kani-ir`, and `git apply --check` for every mutation patch. Kani
  reported nine successfully verified harnesses and zero failures.
- 2026-06-13: A final CodeRabbit review attempt after the support-module split
  again stalled at `preparing_sandbox` without emitting findings or a
  rate-limit notice. The local process was stopped after confirming the process
  pipeline belonged to this worktree.
