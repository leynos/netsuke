# 3.14.3. Lower target and action `deps` into implicit IR and Ninja edges

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `3.14.3` closes a long-standing gap in Netsuke's manifest contract.
Today the AST `Target` struct accepts a `deps` field, documentation already
describes it as Ninja's implicit-dependency class, and the user guide shows
authors writing `deps: my_app` and `deps: [build/utils.h]`. None of that flows
through. `Target.deps` is parsed, rendered through Jinja, and then silently
discarded during Intermediate Representation (IR) generation. The generated
`build.ninja` never reflects the author's stated prerequisites, cycle detection
never observes them, and incremental rebuilds skip files that the manifest
clearly asks Netsuke to track.

After this work is complete, target-level and action-level `deps:` entries are
lowered into a dedicated implicit-dependency edge class on the IR `BuildEdge`,
participate in cycle detection alongside explicit `sources` inputs, and appear
in the generated `build.ninja` between the explicit input list and the
order-only marker (`|` between explicit inputs and `||`). Targets and actions
keep `sources` in the explicit recipe-input class, so `$in` and the Jinja `ins`
placeholder continue to expand only to material inputs.

Observable success means:

1. A manifest with `deps: hello` produces a Ninja edge of the form
   `build out: rule <sources> | hello`, where `| hello` appears before any
   `|| order_only_deps` block.
2. The IR `BuildEdge` exposes an `implicit_deps: Vec<Utf8PathBuf>` field
   populated from `target.deps`, while `inputs` continues to mirror
   `target.sources` only.
3. A manifest whose only cycle runs through `deps` (for example, `a` declares
   `deps: b` and `b` declares `deps: a`) fails IR generation with
   `IrGenError::CircularDependency`, the same way explicit-input cycles fail
   today.
4. `docs/users-guide.md` documents which dependency classes participate in
   cycle detection, and
   `docs/formal-verification-methods-in-netsuke.md` records the chosen
   cycle-participation contract.
5. `docs/roadmap.md` marks `3.14.3` done only after the implementation and
   validation gates pass and the draft pull request is ready.

This plan was drafted on 2026-05-23 against the
`3-14-3-lower-target-and-action-deps` branch. Implementation must not begin
until the user explicitly approves the plan.

## Constraints

- Keep `target.sources` as the sole contributor to the explicit recipe-input
  class. `$in` and Jinja `ins` must continue to expand to source paths only.
- Add a separate implicit-dependency class for `deps` at the IR layer. The
  field must affect Ninja's rebuild and ordering decisions without leaking
  into recipe interpolation.
- Cycle detection must traverse `target.sources` and `target.deps` paths
  uniformly. `order_only_deps` remain outside cycle traversal.
- Preserve the existing implicit `phony: true` treatment of top-level
  `actions` after they pass through `deserialize_actions` and the manifest
  expansion pass. `target.deps` lowering applies identically to entries
  loaded via `targets` and via `actions`.
- Do not change action hashing semantics. Implicit deps live on `BuildEdge`,
  not on `Action`; identical recipes with different implicit-dep sets must
  continue to share an action identifier.
- Do not introduce rule-level `deps` lowering. The design doc explicitly
  forbids accepting rule-level `deps` as an alias for the planned `deps_from`
  contract. The existing `Rule.deps` AST field is out of scope here and is
  tracked by roadmap item `3.14.6`.
- Do not introduce runtime-condition semantics or change manifest-time
  `foreach`/`when` evaluation. `deps` are static manifest data and must reach
  the IR through the same selected entries already produced by the manifest
  loader.
- Keep manifest-order stability. `implicit_deps` must appear in the order the
  author wrote them, matching the existing behaviour of `inputs` and
  `order_only_deps`. Do not sort implicit deps during Ninja emission.
- Keep domain and policy logic at the manifest/IR boundary. Hexagonal
  layering: `src/manifest/` owns parsing and template policy, `src/ir/`
  owns the static graph contract, and `src/ninja_gen.rs` is a pure rendering
  adapter. The Ninja generator must not infer or reinterpret dependency
  semantics; it must only render whatever IR class it is handed.
- Use existing `ortho_config` integration for any new command-line or
  configuration surface discovered during implementation. Do not add a
  parallel configuration loader or untranslated help path.
- Obtain explicit approval before adding any new external dependency.
- Do not introduce `unsafe` code.
- Use en-GB Oxford spelling in documentation, except for external API names
  and established computing terms such as `serialization` and
  `deserialization`.
- Use `rstest` for unit and integration tests, with shared fixtures and
  parameterized cases where they remove duplication.
- Use `rstest-bdd` for behavioural coverage that is externally observable
  through generated `build.ninja` output. Reuse the manifest-then-Ninja
  pipeline already exercised by `tests/features/ir.feature` and
  `tests/features/ninja.feature` rather than inventing a new harness.
- If implementation introduces a new invariant over a range of inputs
  (for example, "any cycle that traverses implicit deps is detected"),
  add `proptest` coverage. If the invariant is a contractual business
  axiom, stop and propose a substantive `kani` or `verus` approach before
  proceeding. Do not add a thin proof that merely restates the property.
- Keep every Rust source file below the 400-line cap from `AGENTS.md`.
- Mark roadmap item `3.14.3` done only after the implementation, tests,
  documentation, `coderabbit review --agent` follow-ups, and quality gates
  all pass.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 16 files or roughly
  700 net new lines, stop and request approval of a revised scope.
- Interface: if a public Rust API signature other than the documented
  `BuildEdge.implicit_deps` field addition must change, stop and explain the
  options. Adding the field is expected to ripple through every
  `BuildEdge { ... }` literal in tests and doctests; that ripple is in
  scope. A signature change to `BuildGraph::from_manifest` or to the public
  `Action` shape is out of scope.
- Manifest schema: if implementation needs to reject or rename any AST field
  outside `Target.deps` lowering (for example, removing the unused
  `Rule.deps`), stop and ask whether to fold that into this task or defer to
  roadmap item `3.14.6`.
- Cycle semantics: if any proposed implementation would change cycle
  participation for `order_only_deps`, stop and update the
  `Decision Log` before editing the cycle module.
- Dependencies: if a new crate, Cargo feature, external tool, Kani harness,
  or Verus setup is required, stop and ask for approval.
- Determinism: if any proposed change reorders or sorts implicit-dep paths
  in the generated Ninja text, stop and require approval. Manifest-order
  stability is a user-visible contract.
- Testing: if a host-dependent fixture, sandboxed temporary path, or
  shell-tool probe makes any new test flaky after two focused fixes, stop
  and redesign the test boundary around dependency injection or explicit
  configuration overrides.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and record the failing commands and
  log paths.
- Review: if `coderabbit review --agent` reports unresolved concerns after
  a major milestone, address them before proceeding. If a concern conflicts
  with this plan, stop and ask for direction.

## Risks

- Risk: a `BuildEdge { ... }` field addition ripples through many literals
  in production tests and doctests. Severity: medium. Likelihood: high.
  Mitigation: enumerate every literal during Stage B and add the new
  `implicit_deps: Vec::new()` initialiser as a mechanical sweep before any
  cycle or generator changes go in.

- Risk: the cycle detector currently iterates only `edge.inputs`. Extending
  it to traverse implicit deps could regress missing-dependency reporting if
  the chained iteration is structured carelessly. Severity: medium.
  Likelihood: medium. Mitigation: keep `record_missing_dependency`
  class-agnostic; introduce a single chained iterator and add a regression
  test where a manifest declares a missing implicit dep alongside a present
  explicit input.

- Risk: emitting `| <implicit_deps>` between inputs and `||` is mostly
  mechanical, but Ninja accepts an empty explicit input list with a
  non-empty implicit dep list (`build out: rule | dep`). A naive
  implementation that prefixes the implicit-dep block with a leading space
  could emit `build out: rule  | dep` or worse. Severity: low.
  Likelihood: medium. Mitigation: keep the conditional blocks symmetric with
  the existing `||` block and snapshot two cases: with and without explicit
  inputs.

- Risk: snapshot drift. Existing snapshots in `tests/snapshots/ninja/`
  cover manifests without `deps:` fields, so they should remain stable.
  An accidental change to whitespace, ordering, or default emission could
  silently invalidate them. Severity: medium. Likelihood: low. Mitigation:
  run the snapshot suite first, after only the field addition, before
  changing emission code.

- Risk: documentation drift between
  `docs/users-guide.md`, `docs/netsuke-design.md`,
  `docs/developers-guide.md`, and the new cycle-participation note in
  `docs/formal-verification-methods-in-netsuke.md`. Severity: medium.
  Likelihood: medium. Mitigation: update all four documents in the same
  commit as the cycle-detection change and cross-reference the user-guide
  paragraph from the formal-verification doc.

- Risk: an author who currently treats `deps` as documentation only might
  rely on Netsuke silently ignoring the field. Severity: low. Likelihood:
  low. Mitigation: confirm the user-guide already documents the intended
  semantics (it does, lines 199–252) and rely on the existing public
  contract rather than introducing a deprecation pathway.

- Risk: scope creep into roadmap item `3.14.6` if the change accidentally
  also touches `Rule.deps`. Severity: medium. Likelihood: medium.
  Mitigation: explicit `Constraints` entry above; add a comment on
  `src/ast.rs` near `Rule.deps` only if needed for reviewer clarity, and
  defer any actual cleanup to `3.14.6`.

## Relevant context

The manifest load pipeline is unchanged by this task. `src/manifest/mod.rs`
still parses YAML into `ManifestValue`, registers helpers and macros, runs
`expand_foreach`, deserializes into `NetsukeManifest`, and renders string
fields. `src/manifest/expand.rs` still owns the `foreach`/`when` policy and
emits filtering observability through `tracing::debug!`. None of that needs
to change for `3.14.3`.

`src/ast.rs` defines `Target.deps: StringOrList` at line 222 and
`Rule.deps: StringOrList` at line 130. Top-level `actions` deserialize as
`Target` values with `phony = true` via `deserialize_actions` at lines 40–49.
This is the deserialization boundary that gives both manifest sections the
same downstream treatment.

`src/manifest/render.rs` already renders `target.deps` and
`target.order_only_deps` through `render_string_or_list`. No change is
required there; template expansion in `deps` already works.

`src/ir/mod.rs` re-exports the public IR types. `src/ir/graph.rs` defines
`BuildEdge` with `inputs`, `explicit_outputs`, `implicit_outputs`, and
`order_only_deps`. `BuildEdge` is missing an `implicit_deps` field; the
design doc `docs/netsuke-design.md` §5.3 and the class diagram at lines
1792–1801 already prescribe the field.

`src/ir/from_manifest.rs` constructs each `BuildEdge` in `process_targets`
(lines 53–112). The current code calls `to_paths(&target.sources)` for the
explicit input list and `to_paths(&target.order_only_deps)` for the
order-only list, but never reads `target.deps`. The new field must be
populated next to `inputs` so the construction site stays compact.

`src/ir/cycle.rs` walks `edge.inputs` in `CycleDetector::visit` (line 87).
The current `record_missing_dependency` and `canonicalize_cycle` helpers are
class-agnostic; only the iterator needs to expand to chain implicit deps.

`src/ninja_gen.rs::DisplayEdge::fmt` (lines 281–298) emits the build line
shape `build <outs>[ | imp_outs]: <rule>[ <ins>][ || order_only]`. A new
conditional block must be inserted between the explicit-input block and the
order-only block to emit `| <imp_deps>` when the implicit-dep list is
non-empty. The macro helpers (`write_kv!`, `write_flag!`) are unrelated and
must not change.

`src/hasher.rs::ActionHasher::hash` hashes only the `Action` struct, never
the edge. Adding `implicit_deps` to `BuildEdge` leaves action identities
stable. Implicit-dep paths must not appear in any `Recipe::Command` text;
that is already enforced by the design contract that recipe interpolation
sees only `sources` and `outputs`.

The user-facing dependency-field documentation lives in
`docs/users-guide.md` lines 192–252. The cycle-participation contract is
discussed in `docs/formal-verification-methods-in-netsuke.md` lines 237–248,
which explicitly asks for the chosen scope to be recorded before proofs
become gating. The design doc `docs/netsuke-design.md` §§2.4 and 5.3 already
describes the intended Ninja mapping and implementation narrative.

Relevant skills and documents to keep open while implementing:

- `leta`: navigate Rust symbols and references before editing code. Use
  `leta show`, `leta refs`, and `leta grep` rather than reading entire
  files.
- `rust-router`: route Rust-specific questions to the smallest useful
  follow-on skill. Likely follow-ons are `rust-types-and-apis` for the IR
  field addition, `arch-crate-design` if the boundary between manifest, IR,
  and Ninja modules comes up, `rust-errors` if `IrGenError` needs new
  variants (it should not), and `rust-performance-and-layout` only if the
  cycle detector grows allocation hotspots (it should not).
- `hexagonal-architecture`: keep the dependency-class policy on the
  manifest/IR boundary. The Ninja adapter must not infer dep semantics.
- `execplans`: keep this plan current.
- `commit-message`: commit with a file-based message after gates pass.
- `pr-creation` and `en-gb-oxendict`: use them when opening or revising the
  draft pull request.
- `kani`: load only if the Kani option in `Decision Log` below is chosen.
  Roadmap item `4.2.1` is the proper home for bounded IR cycle proofs;
  `3.14.3` should default to `proptest` plus targeted `rstest` cases
  unless escalation is approved.

Use these repository documents:

- `docs/roadmap.md`: source of roadmap item `3.14.3`.
- `docs/netsuke-design.md`: design source for the dependency-field lowering
  table (§2.4) and the AST-to-IR transformation narrative (§5.3).
- `docs/formal-verification-methods-in-netsuke.md`: cycle-participation
  contract entry that this task must populate.
- `docs/users-guide.md`: user-facing manifest and CLI behaviour for `deps`,
  `sources`, and `order_only_deps`.
- `docs/developers-guide.md`: internal implementation and testing practices,
  including the IR/Ninja module boundary.
- `docs/ortho-config-users-guide.md`: configuration layering and localized
  help support if any config or CLI surface changes are required.
- `docs/rstest-bdd-users-guide.md`: BDD feature and step guidance.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and parameterization
  patterns for unit tests.
- `docs/rust-doctest-dry-guide.md`: doctest guidance because the
  `BuildEdge` field addition ripples into public Rustdoc examples.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`: dependency
  injection guidance for any environment, clock, process, or filesystem
  effects introduced by tests.

## Prior art

External lookup tools (firecrawl MCP) are not connected in this session, so
this section relies on the design doc's own cross-references plus the
behaviour visible in the Ninja documentation already cited there. Add
external references during implementation only if a design ambiguity
appears.

The relevant prior-art points already captured in the repository:

- Ninja's manual (`docs/netsuke-design.md` §2.4 footnote `[^7]`) defines
  the explicit-input, implicit-dep, and order-only-dep classes precisely
  by their punctuation: `build outs: rule ins | imp_deps || order_only`.
  Netsuke must mirror that vocabulary without re-implementing Ninja's
  parser semantics.
- Bazel separates `srcs` (inputs that propagate into actions) from `deps`
  (transitive dependency edges). The lesson worth borrowing is the strict
  separation of "what the recipe consumes" from "what the build depends
  on for freshness". Netsuke already has that separation in name; this
  task makes it true.
- Buck2 distinguishes `srcs`, `deps`, and `exec_deps` similarly. The
  lesson worth borrowing is to keep cycle detection over all freshness-
  affecting dependencies. Order-only edges are an explicit escape hatch
  and remain outside the cycle.

Planning implication: keep the wording "implicit dependency" everywhere
(user guide, design doc, developer guide), in line with Ninja's manual,
and do not invent a Netsuke-specific term that diverges from the backend.

## Progress

- [x] (2026-05-23T00:00Z) Loaded `leta`, `rust-router`,
      `hexagonal-architecture`, `execplans`, and `firecrawl` skills; added
      the worktree to the leta workspace.
- [x] (2026-05-23T00:00Z) Reviewed `AGENTS.md`, `docs/roadmap.md`,
      `docs/netsuke-design.md` §§2.3, 2.4, 5.3, 5.4, the past `3.14.1` and
      `3.14.2` execplans, and the current IR/Ninja/cycle code paths.
- [x] (2026-05-23T00:00Z) Used a Wyvern agent team to validate the
      architectural choices and inventory every test, fixture, and
      snapshot touched by `Target.deps`, `BuildEdge`, or the generated
      Ninja edge shape.
- [x] (2026-05-23T00:00Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-24T00:00Z) User approved implementation of this ExecPlan
      and explicitly requested the `leta`, `rust-router`, and
      `hexagonal-architecture` skills plus a leta workspace.
- [x] (2026-05-24T00:00Z) Stage A confirmed the AST-to-IR gap and ran
      the planned baseline command. The command exited successfully and
      logged to
      `/tmp/baseline-netsuke-3-14-3-lower-target-and-action-deps.out`.
- [x] (2026-05-24T00:00Z) Stage A follow-up identified the meaningful
      snapshot tests: `touch_manifest_ninja_validation`,
      `conditional_manifest_ninja_snapshot`, and
      `generate_multiline_script_snapshot`.
- [x] (2026-05-24T00:00Z) Stage B added
      `BuildEdge.implicit_deps` with empty initialisers at all current
      construction sites. `cargo test --workspace` passed and logged to
      `/tmp/stage-b-netsuke-3-14-3-lower-target-and-action-deps.out`.
- [x] (2026-05-24T00:00Z) Stage B follow-up committed the
      behaviour-neutral IR field addition as `3dc71f5`.
- [x] (2026-05-24T00:00Z) Stage C populated `implicit_deps` from
      `target.deps`, added target/action IR coverage, and preserved
      documented `{{ ins }}` / `{{ outs }}` placeholders through manifest
      rendering so IR interpolation can substitute them from explicit
      sources and outputs. `make check-fmt`, `make lint`, and `make test`
      passed with logs under `/tmp/*stage-c-netsuke-3-14-3-*`.
- [x] (2026-05-24T00:00Z) Stage C follow-up committed manifest lowering
      and placeholder preservation as `9b8d030`.
- [x] (2026-05-24T00:00Z) Stage D extended
      `ir::cycle::CycleDetector` to traverse implicit deps, added
      implicit-only, mixed, missing-implicit, and bounded small-cycle
      regression tests, and passed `make check-fmt`, `make lint`, and
      `make test` with logs under `/tmp/*stage-d-netsuke-3-14-3-*`.
- [x] (2026-05-24T00:00Z) Stage D follow-up committed cycle detection
      changes as `06a3c6f`.
- [x] (2026-05-24T00:00Z) Stage E updated
      `ninja_gen::DisplayEdge::fmt` to emit `| <implicit_deps>` between
      explicit inputs and order-only deps. Added `rstest` edge-shape
      coverage for explicit-plus-implicit, implicit-only,
      explicit-plus-implicit-plus-order-only, and phony action cases.
      `cargo test --test ninja_gen_tests`, `make lint`, and `make test`
      passed with logs under `/tmp/*stage-e-netsuke-3-14-3-*`.
- [x] (2026-05-24T00:00Z) Stage E follow-up committed Ninja emission
      changes as `f6faba3`.
- [x] (2026-05-24T00:00Z) Stage F added a manifest-then-Ninja
      `rstest-bdd` scenario and fixture covering target/action implicit
      deps, explicit recipe inputs, generated Ninja separators, and
      `{{ ins }}` interpolation excluding implicit deps. `cargo test
      --test bdd_tests implicit`, `make lint`, and `make test` passed
      with logs under `/tmp/*stage-f-netsuke-3-14-3-*`.
- [x] (2026-05-24T00:00Z) Stage F follow-up committed behavioural
      coverage as `1a2c686`.
- [x] (2026-05-24T00:00Z) Stage G updated
      `docs/users-guide.md`, `docs/developers-guide.md`,
      `docs/netsuke-design.md`, and
      `docs/formal-verification-methods-in-netsuke.md` with the
      dependency-class and cycle-participation contract. `git diff
      --check`, touched-file markdownlint, `make markdownlint`, and
      `make nixie` passed; logs are under `/tmp/*stage-g-netsuke-3-14-3-*`.
- [x] (2026-05-24T00:00Z) Stage G follow-up committed documentation
      updates as `f875e69`.
- [x] (2026-05-24T00:00Z) Final deterministic gates before CodeRabbit
      passed for `make check-fmt`, `make lint`, `make test`,
      `make markdownlint`, and `make nixie` with logs under
      `/tmp/*netsuke-3-14-3-lower-target-and-action-deps.out`.
- [x] (2026-05-24T00:00Z) `coderabbit review --agent` completed with
      zero findings. Review output is logged at
      `/tmp/coderabbit-netsuke-3-14-3-lower-target-and-action-deps.out`.
- [x] (2026-05-24T00:00Z) Marked roadmap item `3.14.3` and its
      subitems complete after implementation, deterministic gates, and
      CodeRabbit review all passed.
- [x] (2026-05-24T00:00Z) Closing gates passed after the roadmap and
      execplan completion updates: `make check-fmt`, `make lint`,
      `make test`, `make markdownlint`, and `make nixie`. Logs are under
      `/tmp/*final-netsuke-3-14-3-lower-target-and-action-deps.out`.
- [x] (2026-05-24T00:00Z) Pushed the branch and refreshed draft pull
      request [#315](https://github.com/leynos/netsuke/pull/315) with
      full branch context, validation evidence, and review entrypoints.

## Surprises & Discoveries

- (2026-05-24T00:00Z) `leta workspace add` reported that the worktree was
  already registered. No workspace repair was required.

- (2026-05-24T00:00Z) Live code still matches the planned gap:
  `BuildGraph::process_targets` constructs `inputs` from `target.sources`
  and `order_only_deps` from `target.order_only_deps`, but never reads
  `target.deps`; `CycleDetector::visit` traverses only `edge.inputs`; and
  `DisplayEdge::fmt` renders explicit inputs followed directly by
  `order_only_deps`.

- (2026-05-24T00:00Z) The planned baseline command
  `cargo test --workspace ninja_snapshot_tests` is green but selects zero
  tests in this repository state. Treat it as a compile/filter smoke test
  only, and find the actual snapshot test names before relying on snapshot
  coverage.

- (2026-05-24T00:00Z) `make fmt` still triggers the pre-existing
  repository-wide Markdown backlog noted in the validation plan. The
  formatter touched many unrelated Markdown files before failing; those
  unrelated changes were restored, and Rust formatting was applied with
  `cargo fmt --all`.

- (2026-05-24T00:00Z) The planned Stage C command
  `cargo test --workspace ir_from_manifest_tests` has the same filter
  problem as the Stage A baseline: it compiles but selects zero tests.
  The meaningful command is `cargo test --test ir_from_manifest_tests`,
  which executed all 15 tests after the Stage C fixes.

- (2026-05-24T00:00Z) The user-guide-documented `{{ ins }}` and
  `{{ outs }}` placeholders were not surviving manifest rendering in
  recipe command strings when no user variable named `ins` or `outs`
  existed. Stage C now preserves those placeholders through manifest
  rendering and lets IR interpolation substitute them alongside `$in` and
  `$out`.

- (2026-05-24T00:00Z) `proptest` is not present in `Cargo.toml` or
  `Cargo.lock`. Because the plan prohibits adding a dependency without
  explicit approval, Stage D used deterministic bounded coverage over
  small generated cycles instead of adding `proptest`.

- (2026-05-24T00:00Z) `BuildEdge.phony` does not make
  `ninja_gen::DisplayEdge` emit Ninja's built-in `phony` rule by itself;
  the generator still renders the edge's `action_id`. The Stage E phony
  test therefore registers an action named `phony` and verifies
  `build phony_action: phony | dep` in the build line while preserving
  the existing rule-emission behaviour.

- (2026-05-24T00:00Z) Editing only a `.feature` file did not cause Cargo
  to rebuild the `rstest-bdd` generated test binary. Stage F made a
  comment-only touch to `tests/bdd_tests.rs` so the macro expansion picks
  up the new scenario.

- (2026-05-24T00:00Z) The final `make fmt` attempt repeated the
  pre-existing Markdown formatting backlog and rewrote unrelated
  documentation before failing. The unrelated churn was restored, and the
  deterministic validation gates that do not rewrite unrelated files all
  passed before CodeRabbit review.

## Decision Log

- Decision: lower `target.deps` into a new `BuildEdge.implicit_deps`
  field rather than overloading `BuildEdge.inputs`.
  Rationale: the design doc §2.4 table makes the class distinction
  explicit; recipe interpolation must see only `sources`. Adding a
  separate field keeps the contract one-to-one with Ninja's `|` syntax
  and preserves the current `$in` behaviour.
  Date/Author: 2026-05-23 / planning agent.

- Decision: include `implicit_deps` in cycle detection alongside
  `inputs`, and continue to exclude `order_only_deps`.
  Rationale: the cycle-participation contract in
  `docs/formal-verification-methods-in-netsuke.md` enumerates three
  options. `deps` affect Ninja's freshness and ordering decisions; a
  cycle through `deps` is just as much a build cycle as a cycle through
  `sources`, and excluding it would defer the failure to Ninja with a
  worse diagnostic. `order_only_deps` impose ordering without affecting
  rebuild and are correctly outside cycle traversal today.
  Date/Author: 2026-05-23 / planning agent.

- Decision: leave action hashing unchanged.
  Rationale: `ActionHasher::hash` hashes the `Action` struct only, and
  implicit deps live on `BuildEdge`. Two edges with identical recipes
  and identical outputs but different implicit-dep sets should still
  share an action identifier, because the recipe text is identical and
  the action's behaviour is identical. Existing snapshot hashes remain
  stable.
  Date/Author: 2026-05-23 / planning agent.

- Decision: do not sort `implicit_deps` during Ninja emission.
  Rationale: `inputs` and `order_only_deps` already preserve manifest
  order. Manifest order is itself deterministic. Sorting `implicit_deps`
  alone would create an inconsistent public contract; sorting all three
  would break the existing snapshot baseline. Authors may rely on the
  order they wrote.
  Date/Author: 2026-05-23 / planning agent.

- Decision: defer rule-level `Rule.deps` rejection to roadmap item
  `3.14.6`.
  Rationale: `3.14.3` is additive at the target/action layer.
  `Rule.deps` is currently parsed and rendered but never lowered, so the
  field is effectively a documentation lie. Removing or rejecting it now
  would conflate an additive target/action change with a breaking AST
  change, and `3.14.6` already names the `deps_from` rework that
  includes "without accepting rule-level `deps` as an alias". The PR
  description must call out this deferral so the gap is tracked.
  Date/Author: 2026-05-23 / planning agent.

- Decision: prefer `proptest` over `kani` for the cycle-detection
  invariant in this task.
  Rationale: bounded model-checking of cycle detection is the explicit
  scope of roadmap item `4.2.1`, which is still open. `3.14.3` only
  extends the traversal set; a property test over generated manifests
  with mixed `sources`/`deps`/`order_only_deps` lists exercises the
  same invariant without prematurely owning a Kani harness that
  `4.2.1` will redesign. If escalation is requested, add a bounded
  Kani harness under `tools/kani/` per the existing repository
  integration plan.
  Date/Author: 2026-05-23 / planning agent.

- Decision: do not add `proptest` for `3.14.3`; use deterministic
  bounded cycle coverage instead.
  Rationale: `proptest` is not already wired into this crate, and adding a
  new dependency requires explicit approval under this plan. The bounded
  test still exercises mixed explicit/implicit cycles without widening
  the dependency surface.
  Date/Author: 2026-05-24 / implementation agent.

- Decision: emit the new `| <implicit_deps>` block between explicit
  inputs and the `||` block in `ninja_gen.rs`, conditional on a
  non-empty list and structured symmetrically with the existing
  conditional blocks.
  Rationale: Ninja accepts `build out: rule | dep` with no explicit
  inputs; the conditional-block style keeps that case clean without
  whitespace gymnastics.
  Date/Author: 2026-05-23 / planning agent.

## Implementation plan

Stage A audits the gap. Confirm by reading the live code that the AST
deserializes `Target.deps` and the renderer renders it, but
`process_targets` never reads the field. Run the existing snapshot suite
and capture the baseline:

```sh
cargo test --workspace ninja_snapshot_tests
```

Expected: the existing two Ninja snapshots pass unchanged. Record any
unexpected diffs in `Surprises & Discoveries` and stop before changing
behaviour.

Stage B adds the IR field. In `src/ir/graph.rs`, insert
`pub implicit_deps: Vec<Utf8PathBuf>` on `BuildEdge` immediately after
`inputs`, mirroring the design doc's class diagram. Update every literal
construction site in production tests and doctests with
`implicit_deps: Vec::new()`. The construction sites known today are:

- `src/ir/cycle.rs::tests::build_edge` (lines 165–175).
- `src/ir/mod.rs` module doctest (lines 9–25).
- `src/ir/graph.rs` doctest fixtures embedded in `IrGenError`
  examples (lines 160–229 across the variant docs).
- `src/ninja_gen.rs::generate` and `generate_into` doctests
  (lines 66–88 and 100–127).
- `tests/ninja_gen_tests.rs` at every literal `BuildEdge { ... }` block
  (the Wyvern agent counted ten such blocks; recount before editing).
- `tests/ir_tests.rs` at two literal blocks (lines 28–41 and 85–103).

Run the workspace test suite after the field addition with
`implicit_deps: Vec::new()` everywhere. Nothing else should change yet.
Expected: snapshots stay byte-identical and the suite passes.

Stage C populates the field. In `src/ir/from_manifest.rs::process_targets`,
add `let implicit_deps = to_paths(&target.deps);` alongside the existing
`let inputs = ...` and pass it into the `BuildEdge` literal. Keep `inputs`
sourced from `target.sources` only. Add `rstest` coverage in
`tests/ir_from_manifest_tests.rs` that:

1. asserts `BuildEdge.implicit_deps` is populated from `target.deps`
   for a manifest declared in `targets:`;
2. asserts the same for a manifest declared in `actions:` (which
   exercises the `phony: true` default through the
   `manifest.actions.iter().chain(&manifest.targets)` chain);
3. asserts `BuildEdge.inputs` remains empty for a target whose only
   prerequisite is in `deps:`;
4. asserts that a recipe command does not see implicit-dep paths in
   `$in` or in the interpolated `ins` placeholder.

Prefer literal YAML manifests in the test cases for readability, in the
style of the existing `skipped_manifest_conditions_do_not_contribute_to_ir`
parameterized cases.

Stage D extends cycle detection. In `src/ir/cycle.rs::CycleDetector::visit`,
extend the chained iterator on line 87 to walk both `edge.inputs` and
`edge.implicit_deps`. Keep `record_missing_dependency` unchanged; it is
class-agnostic. Add the following coverage:

- A unit test in `src/ir/cycle.rs::tests` that constructs a two-node cycle
  through `implicit_deps` only (`a -> b -> a` where neither edge appears in
  `inputs`) and asserts `CycleDetector::find_cycle` returns the canonical
  cycle.
- A unit test that mixes `inputs` and `implicit_deps` in a single cycle
  and confirms cycle detection still terminates with the canonical
  ordering.
- A unit test that adds a missing implicit dep alongside a present
  explicit input and asserts `missing_dependencies` records the
  implicit-dep gap without crashing the traversal.
- A `proptest` case that generates small bounded manifests (at most ten
  targets, three deps per slot) and asserts the cycle detector reports a
  cycle whenever the union graph of explicit inputs and implicit deps
  contains one. If `proptest` is not already wired in this crate, gate the
  case behind a `cfg(feature = "proptest")` until escalation is approved;
  do not add `proptest` to the default dependency surface without explicit
  approval.

Stage E updates Ninja emission. In `src/ninja_gen.rs::DisplayEdge::fmt`,
add a conditional block between the explicit-input write and the
order-only write:

```rust
if !self.edge.implicit_deps.is_empty() {
    write!(f, " | {}", join(&self.edge.implicit_deps))?;
}
```

Add `rstest` coverage in `tests/ninja_gen_tests.rs` for:

1. Explicit inputs only (existing baseline; should be unchanged).
2. Explicit inputs plus implicit deps:
   `build out: rule in | dep`.
3. Implicit deps only (no explicit inputs):
   `build out: rule | dep`.
4. Explicit inputs, implicit deps, and order-only deps together:
   `build out: rule in | dep || stamp`.
5. Phony edge with implicit deps:
   `build phony_action: phony | dep`.

Stage F adds behavioural coverage. Extend the existing IR or Ninja
feature in `tests/features/ir.feature` or `tests/features/ninja.feature`
with a scenario that:

1. compiles a manifest declaring `targets:` and `actions:` entries with
   `deps:` set;
2. asserts the IR `BuildEdge` for each output exposes the implicit-dep
   class;
3. invokes the Ninja generator and asserts the generated text contains
   the `| <dep>` separator on the expected build line.

Reuse the existing step modules under `tests/bdd/steps/`. If a new
assertion step is required, add it to `tests/bdd/steps/ir.rs` or
`tests/bdd/steps/ninja.rs` and keep each file under 400 lines.

Stage G updates documentation. The four documents to touch are:

- `docs/users-guide.md`: in the dependency-fields section (lines 192–252),
  add a paragraph after the `order_only_deps` description recording which
  dependency classes participate in cycle detection. State explicitly that
  `sources` and `deps` participate and that `order_only_deps` does not.
  This is the user-visible cycle-participation contract.
- `docs/formal-verification-methods-in-netsuke.md`: in the
  "Cycle-participation contract" section (lines 237–248), record the
  decision that explicit inputs and implicit deps participate and that
  order-only deps do not. Cross-reference the new user-guide paragraph.
- `docs/netsuke-design.md`: confirm §§2.4 and 5.3 already describe the
  intended lowering and class diagram. If wording drifted during
  implementation, align the prose with the implementation. Do not
  rewrite the design narrative.
- `docs/developers-guide.md`: add a short subsection in the IR section
  (around the "Test suite map" or "Manifest processing helpers" area)
  explaining the cycle-detection class set and pointing implementers at
  `src/ir/cycle.rs::CycleDetector::visit`.

Stage H validates and closes. Run `coderabbit review --agent` after Stage
F is green; address every concern. Run the final gates sequentially with
`tee` logs, mark roadmap item `3.14.3` done in `docs/roadmap.md`, commit,
push, and open the draft pull request whose title includes `(3.14.3)` and
whose summary links this ExecPlan.

## Validation plan

Before editing implementation code, run a narrow baseline to capture
existing snapshot behaviour:

```sh
cargo test --workspace ninja_snapshot_tests \
  2>&1 \
  | tee /tmp/baseline-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: both Ninja snapshots pass unchanged.

After Stage B (field addition only):

```sh
cargo test --workspace \
  2>&1 \
  | tee /tmp/stage-b-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: the whole suite passes with the new field initialised to
`Vec::new()` everywhere; no snapshot drift.

After Stage C (population from `target.deps`):

```sh
cargo test --workspace ir_from_manifest_tests \
  2>&1 \
  | tee /tmp/stage-c-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: the new parameterized cases pass; existing IR cases continue
to pass.

After Stage D (cycle detection):

```sh
cargo test -p netsuke ir::cycle \
  2>&1 \
  | tee /tmp/stage-d-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: the new cycle tests pass; the existing
`circular.yml` regression continues to pass; the `proptest` case (if
added) terminates within its default budget.

After Stage E (Ninja emission):

```sh
cargo test --workspace ninja_gen_tests \
  2>&1 \
  | tee /tmp/stage-e-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: every new edge-shape case passes; existing baselines and
snapshots remain stable except where a new snapshot intentionally
captures the new shape.

After Stage F (behavioural coverage):

```sh
cargo test --test bdd_tests implicit_deps \
  2>&1 \
  | tee /tmp/stage-f-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected: the new BDD scenario passes; the existing IR and Ninja
scenarios continue to pass.

Final validation must run these commands sequentially with `pipefail`
and capture logs. Sub-agents must not run tests; this list runs in the
implementation session only:

```sh
set -o pipefail
make fmt          2>&1 | tee /tmp/fmt-netsuke-3-14-3-lower-target-and-action-deps.out
make check-fmt    2>&1 | tee /tmp/check-fmt-netsuke-3-14-3-lower-target-and-action-deps.out
make lint         2>&1 | tee /tmp/lint-netsuke-3-14-3-lower-target-and-action-deps.out
make test         2>&1 | tee /tmp/test-netsuke-3-14-3-lower-target-and-action-deps.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-3-14-3-lower-target-and-action-deps.out
make nixie        2>&1 | tee /tmp/nixie-netsuke-3-14-3-lower-target-and-action-deps.out
```

Expected successful final output is that each command exits with status
`0`. If `make fmt` reports the pre-existing repository-wide Markdown
backlog logged by earlier execplans, restore any unrelated formatter
churn before continuing.

After Stage F is green, run the CodeRabbit review pass and address every
concern. Re-run the final gates after each batch of fixes:

```sh
coderabbit review --agent \
  2>&1 \
  | tee /tmp/coderabbit-netsuke-3-14-3-lower-target-and-action-deps.out
```

## Idempotence and recovery

Every stage is additive and re-runnable. Stages B through E commit small
diffs; if a stage fails validation, revert that commit with
`git reset --hard HEAD~1` (after confirming with the user) and retry.
Snapshot regeneration is intentionally a separate, reviewable commit
that uses `cargo insta accept` only after a maintainer has eyeballed the
diff.

If the user later changes the cycle-participation contract decision, the
single behaviour change required is in `src/ir/cycle.rs::visit`. The
field addition, IR population, and Ninja emission do not depend on which
classes participate in cycle detection.

## Interfaces and dependencies

This task introduces one new field on a public IR struct and does not
introduce any new Rust crate dependencies. The exact surface change is:

```rust
// src/ir/graph.rs
pub struct BuildEdge {
    pub action_id: String,
    pub inputs: Vec<Utf8PathBuf>,
    pub implicit_deps: Vec<Utf8PathBuf>,
    pub explicit_outputs: Vec<Utf8PathBuf>,
    pub implicit_outputs: Vec<Utf8PathBuf>,
    pub order_only_deps: Vec<Utf8PathBuf>,
    pub phony: bool,
    pub always: bool,
}
```

The new field is documented with a Rustdoc comment mirroring the design
doc's class-diagram comment. No new error variants are introduced on
`IrGenError` or `NinjaGenError`.

`ir::cycle::CycleDetector::visit` continues to return
`Option<Vec<Utf8PathBuf>>`; only its internal iterator changes.

`ninja_gen::DisplayEdge::fmt` continues to return `fmt::Result`; only its
emission shape changes.

No new CLI flags, configuration keys, manifest schema fields, or
environment variables are introduced.

## Outcomes & Retrospective

Roadmap item `3.14.3` is implemented. Target-level and action-level `deps`
now lower into `BuildEdge.implicit_deps`, participate in cycle detection,
and render as Ninja implicit dependencies between explicit inputs and
order-only dependencies. Recipe interpolation continues to use explicit
`sources` only for `$in` and `{{ ins }}`.

The work also fixed a related placeholder-preservation bug: documented
`{{ ins }}` and `{{ outs }}` recipe placeholders now survive manifest
rendering so IR command interpolation can substitute them.

Validation passed for the Rust formatting check, lint/doc checks, full
test suite, Markdown linting, Mermaid validation, and CodeRabbit review.
`make fmt` remains blocked by unrelated repository-wide Markdown formatting
backlog; the branch restored that unrelated churn and relied on
non-mutating gates for completion.

## Revision note

(populate when revising)
