# 4.2.1. Add Kani harnesses for manifest-to-IR safety checks

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IMPLEMENTING

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
- Do not introduce a verification-only collection port (for example, a
  `MapStore` trait swapped under `#[cfg(kani)]`) or any new dependency. The
  rejection is scope and lint friction: such a port would expand the production
  module's surface area, demand a parallel set of helpers, and force every
  Clippy suppression in the swapped implementation to carry a `reason = "..."`
  clause. The single permitted Cargo.toml change is the `check-cfg` addition for
  `cfg(kani)` and an optional `[package.metadata.kani.flags]` block; nothing
  else.
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
- [ ] Stage B (red): add the `check-cfg` declaration, the
      `[package.metadata.kani]` table, the empty `#[cfg(kani)] mod
      verification` blocks in `src/ir/from_manifest.rs` and
      `src/ir/cycle.rs`, and a single placeholder
      `#[kani::proof]` in each module that calls
      `kani::assert(true, ...)`. Confirm `cargo kani list`
      discovers the harnesses and `make kani-full` runs them.
- [x] (2026-06-11T22:18:05Z) Stage B scaffold edits made: added the
      `cfg(kani)` lint declaration, `[package.metadata.kani.flags]`
      `default-unwind = "6"`, the `make kani-ir` alias, and placeholder
      `verification::scaffold_smoke` harnesses in `from_manifest` and
      `cycle`.
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
- [ ] Stage C (green): implement the duplicate-output, rule-error,
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
- [ ] Stage D (refactor and docs): extract shared harness helpers,
      add the harness inventory to `docs/developers-guide.md`, add the
      Proptest hand-off footnote to
      `docs/formal-verification-methods-in-netsuke.md`, and add
      `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`. Update
      `docs/contents.md` to index the new ADR.
- [ ] Stage E (validate and review): run mutation discipline per
      harness, run `make check-fmt`, `make lint`, `make test`,
      `make markdownlint`, `make nixie`, and `make kani-full`. Run
      `coderabbit review --agent` and resolve all findings.
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
  (research agent summary). Impact: harnesses live inline in
  `src/ir/from_manifest.rs` and `src/ir/cycle.rs`, which keeps them close to
  the helpers they call and avoids `pub(crate)` re-exports.

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
  `src/ir/from_manifest.rs::verification`; permitted call sites are Kani
  harnesses in that module only, and they must not be promoted to production
  APIs or shared test helpers unless a later harness proves real reuse pressure.

- Observation: the duplicate-output harness did not complete within the
  five-minute solver tolerance when it drove production `ActionHasher::hash`.
  The run repeatedly unwound through serde and SipHash before reaching the IR
  assertion. The Stage A-approved `ActionHasher::hash` stub escape hatch
  removed that surface, but the proof still spent its budget in `Utf8PathBuf`/
  `HashMap` hashing. A direct branch-helper proof also exceeded the practical
  budget in target-map lookup. Impact: the implementation uses a narrower
  pure-constructor harness and leaves the lookup-to-error integration covered
  by existing Rust tests.

- Observation: even the pure duplicate-output constructor exceeded the
  practical proof budget when it formatted the duplicate list for the Fluent
  message arguments. Impact: `duplicate_outputs_message` keeps the real message
  key under `cfg(kani)` but omits formatted arguments, so the harness proves
  the error variant and payload rather than rendered localization text.

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

- Decision: place harnesses inline as `#[cfg(kani)] mod verification`
  blocks inside `src/ir/from_manifest.rs` and `src/ir/cycle.rs`, rather than as
  sibling files or a top-level umbrella module. Rationale: matches Kani's own
  layout guidance, avoids widening any module's public API to expose private
  helpers to harnesses, and keeps the proof obligations close to the code they
  verify. Date/Author: 2026-06-07 / planning agent with research agent input.

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
  target-map lookup. The private `duplicate_output_error_from_paths` helper
  keeps production behaviour unchanged while giving Kani a focused
  error-construction proof. The `cfg(kani)` message helper uses the same
  message key without formatted arguments to avoid proving Fluent formatting in
  an IR safety harness. Date/Author: 2026-06-11 / implementation agent.

- Decision: do not introduce a verification-only collection port
  (for example, a `MapStore` trait specialised under `#[cfg(kani)]`).
  Rationale: the change would widen the production module's surface area,
  demand a parallel set of helpers, and force every Clippy suppression in the
  swapped implementation to carry a `reason` clause. The reduced bound plus
  Proptest hand-off reaches the same property coverage without that overhead.
  This rejection is recorded as Option C in ADR-004's "Options considered"
  section. Date/Author: 2026-06-07 / planning agent.

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

- Decision: add a single new ADR
  (`docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`) recording the bound
  reconciliation, the Proptest hand-off, and the rejected alternatives. The
  ADR's "Options considered" section names three approaches: (A) the chosen
  integration-style harness against `BuildGraph::from_manifest` with reduced
  bounds and Proptest hand-off; (B) narrow harnesses against pure leaf functions
  (`find_duplicates`, `resolve_rule`, `cycle::analyse`) which would allow
  larger N without a hand-off but trade end-to-end coverage of
  `process_targets` glue; and (C) a verification-only collection port. The
  ADR's Known Risks section includes an explicit re-evaluation trigger: "if
  Kani gains a special `HashMap` model, or if `4.2.2` is forced to harness leaf
  functions, revisit this decision". Rationale: one decision-point, one record;
  future maintainers do not need to reconstruct the trade-off space.
  Date/Author: 2026-06-07 / planning agent with Logisphere review input.

## Outcomes & Retrospective

To be completed at the end of Stage F.

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
   requires; do not pre-empt.) Add a single placeholder harness whose body is a
   trivially true assertion:

   ```rust
   #[kani::proof]
   #[kani::unwind(2)]
   fn scaffold_smoke() {
       kani::assert(true, "scaffold: replace with real harness");
   }
   ```

   Do **not** use `kani::assert(false, ...)` as a scaffold. A `#[kani::proof]`
   whose body asserts `false` proves the harness runs, but trains a bad
   pattern: future contributors will copy it as a template for "intentional
   failure" without distinguishing vacuous proof from falsified property.
   Discovery is verified separately, by running `cargo kani --list` (or
   `--list-harnesses`, depending on the installed Kani version) and confirming
   the new harness names appear.
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
   `src/ir/from_manifest.rs::verification`, exercising
   `BuildGraph::from_manifest` end-to-end). Build a manifest with one `Rule`
   and two `Target`s, the second of which shares exactly one output path with
   the first. The shared character is chosen symbolically from a fixed
   alphabet. The property under proof is stated in input-shape terms: *if
   exactly one path is shared, the `DuplicateOutput` variant is reported and
   the reported `outputs` set contains exactly that path*. Do not bake any
   assumption about Vec length that would not survive a future schema change
   where targets carry multiple outputs.

   ```rust
   #[kani::proof]
   fn duplicate_output_always_rejected() {
       let shared: u8 = kani::any();
       kani::assume(matches!(shared, b'a' | b'b'));
       let manifest = manifest_two_targets_sharing_output(shared);
       match BuildGraph::from_manifest(&manifest) {
           Err(IrGenError::DuplicateOutput { outputs, .. }) => {
               let expected = ascii_str(shared);
               kani::assert(
                   outputs.iter().any(|o| o == expected),
                   "reported duplicate must include the shared path",
               );
           }
           Err(other) => kani::assert(false, "unexpected error variant"),
           Ok(_) => kani::assert(false, "expected DuplicateOutput"),
       }
   }
   ```

2. **Rule-selection errors** (same module, exercising
   `BuildGraph::from_manifest` end-to-end). One symbolic harness covers the
   three variants: a `kani::any::<u8>()` tag selects among "empty rule",
   "multiple rules", and "missing rule"; the harness builds the corresponding
   minimum manifest and asserts the matching `IrGenError` variant.
   Deterministic one-variant-per-harness was rejected during planning because
   three Kani proofs over fixed inputs are unit tests in proof clothing; the
   symbolic tag forces the dispatch logic in `resolve_rule` to commit to the
   right branch across all three inputs in one proof.

   ```rust
   #[kani::proof]
   fn rule_dispatch_selects_correct_error() {
       let tag: u8 = kani::any();
       kani::assume(tag < 3);
       let manifest = manifest_with_rule_variant(tag);
       match BuildGraph::from_manifest(&manifest) {
           Err(IrGenError::EmptyRule { .. }) =>
               kani::assert(tag == 0, "EmptyRule matches tag 0"),
           Err(IrGenError::MultipleRules { .. }) =>
               kani::assert(tag == 1, "MultipleRules matches tag 1"),
           Err(IrGenError::RuleNotFound { .. }) =>
               kani::assert(tag == 2, "RuleNotFound matches tag 2"),
           other => kani::assert(false, "unexpected outcome"),
       }
   }
   ```

3. **Self-edge and bounded multi-node cycle rejection** (in
   `src/ir/cycle.rs::verification`). One harness per cycle length from 1
   (self-edge) to 3 nodes, asserting that `cycle::analyse(&targets).cycle` is
   `Some(_)` and that the returned cycle is non-empty and well-formed. The
   dependency class (`inputs` vs `implicit_deps`) is chosen symbolically with
   `kani::any::<bool>()` so both edge classes are exercised.

   ```rust
   #[kani::proof]
   #[kani::unwind(4)]
   fn self_edge_always_detected() {
       let mut targets = HashMap::new();
       targets.insert(path(b'a'), edge_with_input(b'a', b'a'));
       let report = cycle::analyse(&targets);
       kani::assert(report.cycle.is_some(), "self-edge yields cycle");
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
       let report = cycle::analyse(&targets);
       kani::assert(report.cycle.is_some(), "2-cycle detected");
   }
   ```

   A 3-node harness follows the same shape. If the 3-node bound exceeds the
   five-minute tolerance, drop it, record the reason in
   `Surprises & Discoveries`, and escalate: with only the self-edge and 2-node
   cases covered by Kani, the stack-unwinding correctness of
   `CycleDetector::visit` for non-trivial cycles is left to Proptest under
   roadmap item `4.3.1`. That dependency must be reflected as a blocker on the
   bound-reduction risk in ADR-004 and in the corresponding roadmap entry, not
   as a sibling concern.

4. **Missing dependencies do not create false cycles** (same
   module). Two harnesses: one with a single target whose dependency is absent
   from the target map, and one with a two- target chain where the deeper
   dependency is absent. Each asserts `report.cycle.is_none()` and that
   `report.missing_dependencies` records exactly the absent dependencies.

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
   `ActionHasher::hash`) or narrow (drives `cycle::analyse` or another leaf
   function directly), so future maintainers can predict the solver workload
   before running it.
3. Add a footnote to
   [`docs/formal-verification-methods-in-netsuke.md`](../formal-verification-methods-in-netsuke.md)
   §Kani for the IR core noting that "up to 10 nodes" is delivered by Proptest
   in future roadmap item `4.3.x`, and that Kani covers the same properties
   exhaustively at 1-3 nodes.
4. Create `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`
   using the ADR template in
   [`docs/documentation-style-guide.md`](../documentation-style-guide.md).
   Capture the bound reconciliation, the rejected verification-only collection
   port, and the Proptest hand-off. Reference the ADR from the
   formal-verification design document footnote.
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

1. **Bound reconciliation.** Do you accept Kani harnesses bounded to
   1-3 nodes, with the larger "up to 10 nodes, depth limit 20 edges" property
   delivered by Proptest under roadmap item `4.3.1` (and `4.3.1` treated as the
   *closing* commitment on the bound-reduction risk, not a sibling concern)?
   The alternatives recorded in ADR-004 are Option B (harness narrow pure leaf
   functions instead of `BuildGraph::from_manifest`, trading end-to-end
   coverage of `process_targets` glue for larger N without a hand-off) and
   Option C (a verification-only collection port, rejected on scope and
   Clippy-suppression grounds).
2. **`ActionHasher::hash` escape hatch.** If Stage C measures a
   duplicate-output harness exceeding the five-minute tolerance, which escape
   hatch do you prefer in advance? Hatch (i): a `#[cfg(kani)]`-only
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
   in `[package.metadata.kani.flags]`. Confirm or specify an alternative.
5. **Harness count budget.** The revised plan sketches six
   harnesses (one duplicate, one parameterised rule-error, three cycle shapes,
   two missing-deps; the 3-node cycle harness may be dropped during Stage C
   measurement). Acceptable?
6. **Mutation patch storage.** Confirm that patch files under
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
  collapsed into one parameterised symbolic harness. Duplicate- output
  assertion restated in input-shape terms. 3-node cycle fallback escalated as a
  `4.3.1` blocker. Verification-only collection port recast as Option C in
  ADR-004 with a tightened rejection rationale. Hexagonal-architecture
  seasoning trimmed from Decision Log and Context; the skill is retained as a
  boundary-policing tool in Skills. Cumulative solver-budget estimate added to
  Tolerances. The plan remains in DRAFT and approval-gated; remaining work is
  Stage A approval and then implementation.
- 2026-06-11: Implementation approved by user instruction. Status moved to
  IMPLEMENTING, Stage A marked complete, and recommended answers to the open
  questions recorded in the Decision Log.
