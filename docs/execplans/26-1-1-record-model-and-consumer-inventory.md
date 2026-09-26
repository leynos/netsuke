# Record a revision-pinned model and consumer inventory (roadmap 26.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 4. See `Revision note` at the foot of this document. This plan was
written against `origin/main` at `ebcedaef683efeb795d0ab65f94ad11dc5b92eb2`
("RFC 0029: first-class host facts with explicit collection (#802)") and
revised after two rounds of expert design review, then rebased onto `b0e8547f`
("Set v0.1.0-beta4 release version and status (#804)"). It must not be
implemented until it is approved and the approver has chosen an entry-gate
option in decision D-6.

## Purpose / big picture

Netsuke compiles a YAML manifest (a `Netsukefile`) into a Ninja build file and
then runs Ninja. [RFC 0026][rfc-0026] and [ADR-035][adr-035] propose hardening
the semantic core of that compiler: successful lowering should yield resolved
operations that cannot contain unresolved rule references (finding H1), a
lowered recipe should carry the interpreter that gives its quoting meaning
(H2), the runner should stop orchestrating Ninja through the command-line type
`Cli` (H3), and compiler errors should be typed facts rather than values paired
with independently supplied localized messages (H4).

Every later task in phases 26 and 27 of the
[hexagonal hardening roadmap][roadmap] changes types that other code
constructs, mutates, and consumes. Before anyone changes them, the project
needs one trustworthy answer to "what exists today, who builds it, who changes
it, who reads it, and what must not break?". RFC 0026 recorded its audit
against an older baseline (`79545e12`, 2026-09-19), and 34 commits have landed
since, six of them in `src/`. Roadmap task 26.1.1 replaces that snapshot with
an inventory pinned to the implementation head.

After this change:

- A new reference document, `docs/hexagonal-hardening-inventory.md`, names the
  exact commit it describes and lists, for authored types, lowering, graph
  storage, backend consumers, command-line interface (CLI) orchestration,
  diagnostics, and existing seams, every production constructor, every
  production mutator, and every production consumer of the types the programme
  will change. The site set comes from the Rust compiler itself (a disposable,
  never-committed build that marks each inventoried item `#[deprecated]` so that
  `rustc` reports every use), united with a text sweep for code the build does
  not compile.
- The same document records which earlier work has already landed (without
  scheduling issue `#652` again), which parts of H1 to H4 remain, numbered new
  observations, and a catalogue of compatibility obligations.
- A trace matrix maps every roadmap task that depends on 26.1.1 to the
  inventory items it changes or consumes and to the obligations it must
  preserve.
- A new contract test, `tests/hexagonal_inventory_contract_tests.rs`, keeps
  the document internally consistent: one pin, unique register identifiers,
  trace-matrix task identifiers that exist in the roadmap, every task in the
  roadmap's dependency closure of 26.1.1 either traced or excluded with a
  reason, and every finding owned by an existing task. It never reads Rust
  source, so it cannot fail because later work legitimately changes the code.
- Roadmap entry 26.1.1 is marked done and links to the inventory (subject to
  the entry-gate option chosen in D-6).

A reviewer observes success by running
`cargo nextest run --test hexagonal_inventory_contract_tests` and seeing it
pass; by choosing any dependent roadmap task and following its trace-matrix row
to a register entry and a named obligation; and by reading the inventory's
"Method" section, which records the oracle's and sweeps' commands and counts
and the seeded-fault result showing the method finds sites it was not told
about.

No production Rust source, manifest behaviour, command-line behaviour, or
generated output changes.

## Context and orientation

Assume no prior knowledge of this repository. This section names every file the
plan reads or edits.

### The pipeline being inventoried

Netsuke processes a manifest in stages (see `docs/netsuke-design.md`, Section
1.2, "The Six Stages of a Netsuke Build"):

1. **Authored types.** `src/ast/mod.rs`, `src/ast/target.rs`,
   `src/ast/string_or_list.rs`, and `src/ast/dependency_order.rs` define the
   deserialized manifest: `NetsukeManifest`, `MacroDefinition`, `Rule`,
   `Target`, the `Recipe` enumeration (`Command`, `Script`, `Rule` variants,
   read through a private `RawRecipe`), `StringOrList`, and `DependencyOrder`.
   They are created by `serde` deserialization and by `From` conversions on
   `StringOrList`. `src/manifest/**` defines no further authored recipe types,
   but it is a major *mutator* of them: `src/manifest/render.rs` renders Jinja
   in place through `&mut` borrows of targets, rules, recipes, and string lists
   (including the `rule:` selector), and `src/manifest/mod.rs` exposes the
   library entry points `manifest::from_str` and (via
   `src/manifest/path_loaders.rs`) `from_path`.
2. **Lowering.** `src/ir/from_manifest.rs` (entry points
   `BuildGraph::from_manifest` and the `#[doc(hidden)] pub`
   `BuildGraph::from_manifest_for_shell`) and `src/ir/from_manifest_support.rs`
   (`register_action`, `resolve_rule`, `resolve_recipe`, `ActionBindings`) turn
   the manifest into the intermediate representation (IR).
   `src/ir/cmd_interpolate/**` substitutes and shell-quotes input and output
   paths for a chosen interpreter through `CommandBindings`.
3. **Graph storage.** `src/ir/graph.rs` defines `BuildGraph`, `Action`,
   `BuildEdge`, `EdgeId`, and the IR `DependencyOrder`. `Action.recipe` stores
   an `ast::Recipe`: this is H1. `src/hasher.rs` derives each action's identity
   by hashing the serialized `Action`.
4. **Backend consumers.** `src/ninja_gen/**` (including `dyndep.rs` and
   `explicit_shell.rs`), `src/ninja_gen_validation.rs`,
   `src/ninja_gen_recipe_shell.rs`, `src/ninja_gen_escape.rs` (`ShellText`,
   `NinjaValue`), and `src/ninja_gen_command_list*.rs` render Ninja text;
   `src/graph_view/**` projects the graph for the `graph` subcommand.
5. **CLI orchestration.** `src/main.rs` calls `runner::run` in
   `src/runner/mod.rs`, which dispatches through `src/runner/dispatch.rs`,
   generates through `src/runner/graph_generation.rs` and
   `src/runner/generation.rs`, and spawns Ninja through
   `src/runner/ninja_process_adapter.rs` and `src/runner/process/**`.
   `src/runner/graph.rs` and `src/runner/help_query.rs` also lower manifests.
   Layered configuration is merged by `ortho_config` in `src/cli/config.rs` and
   `src/cli/merge/**`. `build.rs` compiles parts of `src/cli` and
   `src/localization/mod.rs` through `#[path]`, so it is a consumer too.
6. **Diagnostics.** `src/ir/graph_error.rs` (`IrGenError`),
   `src/ninja_gen_error.rs` (`NinjaGenError`), `src/manifest/diagnostics/**`
   (`ManifestError`), `src/runner/error.rs` (`RunnerError`),
   `src/localization/mod.rs` (`LocalizedMessage`, the process-global
   `set_localizer`), `src/localization/keys.rs`, `src/diagnostic_json.rs`,
   `src/result_json.rs`, `src/json_envelope.rs`, and the error rendering and
   exit-code paths in `src/main.rs`.

### Terms used in this plan

- **Pin.** The one commit the inventory describes. Every `path:line` in the
  inventory is valid only at the pin, and every citation also names its
  module-qualified enclosing symbol so a later reader can relocate it.
- **Production code.** Code compiled into the `netsuke` library, the
  `netsuke` binary, or `build.rs` when neither `cfg(test)` nor `cfg(kani)` is
  set, for any supported target and feature combination. Test modules
  (including inline `#[cfg(test)] mod tests { .. }` blocks and
  `#[path]`-included `*_tests.rs` files), `tests/**`, `test_support/**`,
  `benches/**`, doctests, and Kani-only code are *not* production. The
  inventory lists them by file in an appendix, because later migrations must
  update them.
- **Constructor.** A production site that creates a value of an inventoried
  type: a struct literal, an enum-variant construction (including
  `Self::Variant { .. }`, tuple-struct calls such as `EdgeId(..)`, and unit
  variants), an associated function returning `Self`, a `From`, `TryFrom`,
  `FromStr`, or `Default` implementation, or a `Deserialize` implementation
  (derived or handwritten). `Clone` is not a constructor, because it cannot
  create a state the original did not have; derives are instead recorded as
  per-type columns.
- **Mutator.** A production site that changes an existing value: a method
  taking `&mut self`, `self: &mut Self`, or `mut self`; any function taking
  `&mut T` for an inventoried `T`; a write to a field; collection mutation
  through a field (`insert`, `push`, `retain`, `values_mut`, `iter_mut`, and
  similar); `std::mem::{take, replace, swap}`; or an assignment through a
  mutable reference (`*r = ..`).
- **Consumer.** A production site that reads an inventoried type's fields,
  matches its variants, or calls its query methods.
- **Visibility class.** One of: `pub` (reachable by library callers through
  `src/lib.rs`), `pub` with `#[doc(hidden)]`, crate-internal (`pub(crate)`,
  `pub(super)`, `pub(in ..)`), and private.
- **API-reachable.** A constructor or mutator in the `pub` or
  `pub`-`doc(hidden)` class, whether or not internal production code uses it.
  For example, `BuildGraph::replace_edge_for_output` is `pub` with only test
  callers at the planning head.
- **Compatibility obligation.** A behaviour a later migration must preserve,
  or change deliberately with an explained rationale. Netsuke is pre-1.0
  (`0.1.0-beta4` on `main` since #804) and publishes to crates.io as
  `netsuke-build` with library target `netsuke` (ADR-007, Accepted). Rust
  source APIs therefore carry no compatibility commitment, which the
  developers' guide states outright ("Unstable Rust API for embedders": the
  Netsukefile format and the graph export are the only committed surfaces), but
  the project records source-breaking changes in `CHANGELOG.md` as
  "**Breaking:**" entries (for example, the `#652` entry).
- **Oracle.** The compiler-based site discovery described in
  `Verification plan` (OB-FWD).

### Governing documents

- `docs/roadmap-hexagonal-hardening.md`, section 26.1, defines the task.
- `docs/rfcs/0026-hexagonal-domain-hardening.md`, "Current state and audit
  reconciliation", defines H1 to H6 against baseline `79545e12`.
- `docs/adr-035-semantic-compiler-boundaries.md` records the direction.
- `docs/rfcs/0027-executable-architecture-contract.md`, "Proposed checker"
  and "Inventory and coverage", assigns automated root, configuration, and
  import coverage to the phase-28 checker. This plan builds no such checker.
- `docs/adr-014-backend-text-escaping-seam.md`,
  `docs/adr-019-structured-command-shell-selection.md`,
  `docs/adr-027-command-placeholder-contract.md`,
  `docs/adr-034-preserve-script-in-out-as-shell-variables.md`,
  `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md`,
  `docs/adr-012-bound-dyndep-sidecar-retention.md`,
  `docs/adr-008-environment-seam-taxonomy.md`, and
  `docs/adr-007-publish-as-netsuke-build.md` define contracts the inventory
  names as obligations.
- `docs/polonius.md` classifies borrow-centric sites, including
  `POLONIUS-REFUSED(id-is-data)` in `register_action`.

## Signposts

Read these before starting, in this order:

1. `AGENTS.md` (repository rules, gates, Markdown rules, test rules).
2. `docs/roadmap-hexagonal-hardening.md`, "Existing work and ownership" and
   phases 26 to 29.
3. `docs/rfcs/0026-hexagonal-domain-hardening.md` and
   `docs/adr-035-semantic-compiler-boundaries.md`.
4. `docs/rfcs/0027-executable-architecture-contract.md`, "Proposed checker".
5. `docs/netsuke-design.md`, Sections 1.2, 3.2, 5, 6.1, and 7.
6. `docs/developers-guide.md`, "Unstable Rust API for embedders" (the
   documented, tested embedder surfaces), "Test suite map", "IR dependency
   classes", "Graph view projection and renderer adapters", and "Internal
   support module boundaries".
7. `docs/documentation-style-guide.md` (tables, headings, ExecPlan status
   vocabulary) and `docs/contents.md`.
8. For the contract test: `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/reliable-testing-in-rust-via-dependency-injection.md`, and the
   precedent `tests/execplan_status_contract_tests.rs` (capability-scoped reads
   through `cap_std::fs_utf8::Dir`).
9. For tasks that later consume the inventory (not this one):
   `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`,
   `docs/ortho-config-users-guide.md`, and
   `docs/formal-verification-methods-in-netsuke.md`.

Skills to load: `execplans`; `hexagonal-architecture` (classify each item by
boundary role without transplanting a directory pattern); `rust-router`, then
`rust-types-and-apis` (constructors, visibility, invalid-state reachability) and
`rust-unit-testing` (the contract test's fixtures and assertions);
`nll-to-polonius` (respect `POLONIUS` tags); `codegraph-mcp` (caller
cross-checks); `en-gb-oxendict-style` (prose); and `firecrawl-mcp` only if an
external tool fact is needed.

## Conformance basis

Upstream artefacts at planning time (`ebcedaef`):

- Roadmap `docs/roadmap-hexagonal-hardening.md`, item 26.1.1 (`RM-26.1.1`).
  Acceptance: `RM-26.1.1-A1` ("every planned change maps to a current item and
  compatibility obligation") and `RM-26.1.1-A2` ("the inventory includes every
  production constructor and mutator"). Scope: `RM-26.1.1-S1` (identify
  authored types, lowering, graph storage, backend consumers, CLI
  orchestration, diagnostics, and existing seams at the implementation head) and
  `RM-26.1.1-S2` (record landed work and remaining H1 to H4 findings without
  repeating `#652`). Dependency: `RM-26.1.1-D` ("acceptance of RFC 0026").
- RFC 0026, status **Proposed**: `RFC26-H1` to `RFC26-H4`; `RFC26-CM1`
  ("characterize manifest acceptance, graph export, generated Ninja,
  diagnostics, and library entry points before extraction"); `RFC26-CM2`
  ("Explain any necessary hash change and its rebuild consequences");
  `RFC26-OD` (Outstanding decisions: delegation and duplicate precedence are
  settled by characterization, that is, 26.1.2).
- ADR-035, status **Proposed**: `ADR35-RISK` ("Rule delegation, declaration
  precedence, action identity, and diagnostic shape need characterization
  before refactoring").
- RFC 0027, `RFC27-INV`, as a boundary only.
- ADR-007 (Accepted): the published package and library target names.
- No Terms of Reference document exists; the roadmap and RFC 0026 serve that
  role. The technical design is `docs/netsuke-design.md` at `ebcedaef`.

Trace chains:

```plaintext
RM-26.1.1-D -> D-6 option -> EP-M0 (Option A) or EP-M4 (Option B) -> EV-ENTRY
RM-26.1.1-S1 -> RFC26-CM1 -> EP-M2 -> inventory Register A1..A7 -> EV-ORACLE, EV-SWEEP, EV-REVERSE
RM-26.1.1-A2 -> EP-M2 -> inventory Method + Register -> EV-ORACLE, EV-SEEDED
RM-26.1.1-S2 -> RFC26-H1..H4 -> EP-M3 -> inventory Landed work + Findings -> EV-FINDINGS
RM-26.1.1-A1 -> ADR35-RISK, RFC26-CM2 -> EP-M3 -> inventory Obligations + Trace matrix -> EV-CONTRACT
RM-26.1.1-A1 -> EP-M1 -> tests/hexagonal_inventory_contract_tests.rs -> EV-CONTRACT, EV-RED
RM-26.1.1 (done) -> EP-M4 -> roadmap checkbox + docs/contents.md -> EV-GATES
```

## Constraints

- Change no production Rust source, build script, Cargo manifest,
  `Cargo.lock`, Makefile, workflow, Fluent catalogue, or existing test or
  snapshot. The only Rust file added is
  `tests/hexagonal_inventory_contract_tests.rs`, which reads Markdown only.
- The oracle's `#[deprecated]` attributes and the seeded faults exist only in a
  disposable export under the worktree's ignored `target/` directory. They are
  never committed, never applied to the working tree, and deleted afterwards.
  Do not use `/tmp` as a build target; `/tmp` holds logs only.
- Do not fix any defect found during the sweep. Record it as an observation
  owned by its task (26.1.2 for characterization; 26.2.x, 26.3.x, or 27.x for
  remediation). A release-critical defect triggers the Findings tolerance.
- Do not build, commit, or propose an automated source-inventory tool, a
  code-reading test, or a policy file (`architecture.toml`,
  `architecture-exceptions.toml`). RFC 0027 owns those.
- Do not re-schedule, re-describe as future work, or re-audit the canonical
  edge arena from `#652` / PR `#714`. Record it once as landed with its merge
  commit and preservation obligations.
- Record observed behaviour only. Rule-delegation and duplicate-declaration
  policy belongs to 26.1.2; placeholder semantics to ADR-027, ADR-034, and
  `#699`; diagnostic contract changes to 27.1.3.
- One pin for the whole inventory; no mixed revisions.
- Prescribe no source-compatibility machinery (aliases, facades, deprecated
  entry points) for any Rust API; the crate is pre-1.0. Obligations are
  behavioural and persisted-format ones, plus the CHANGELOG "Breaking" practice.
- Markdown follows `docs/documentation-style-guide.md`: en-GB-oxendict
  spelling with `-ize`, prose wrapped at 80 columns, code at 120, tables and
  headings unwrapped, `-` bullets, `[^n]` footnotes, identifiers in backticks,
  and no bare `#NNN` at the start of a line.
- The contract test follows `AGENTS.md`: a `//!` module comment, `///` docs
  on every helper, no in-process environment mutation, no `.unwrap()`, `rstest`
  for cases, capability-scoped reads, and fewer than 400 lines.
- Run gates only through the `scrutineer` agent, sequentially. Never run the
  oracle build while a gate is running.

## Tolerances (exception triggers)

- Entry: follow the option the approver chooses in D-6. If none is chosen,
  stop at EP-M0.
- Scope: stop and escalate if the change touches more than ten files, or any
  file outside `docs/` other than `tests/hexagonal_inventory_contract_tests.rs`
  and (only if the spelling gate requires it) `typos.local.toml`.
- Drift: if the pin differs from `ebcedaef` and any commit between them
  touches `src/`, `build.rs`, `tests/`, `test_support/`, or `benches/`, re-run
  every sweep; do not patch this plan's planning-time observations. If
  `origin/main` moves again before EP-M2 finishes, ask whether to re-pin.
- Register size: stop and report if, in any area other than A6, the oracle
  finds more than 25 production constructor or mutator sites whose enclosing
  symbol is not named in that area's list under `Plan of work`, because the
  scope definitions would then be wrong.
- Oracle failure: if the scratch build fails for a reason other than the
  deliberate attributes (for example, a toolchain download failure), stop and
  report rather than falling back to text sweeps alone. An error caused by an
  attribute in an illegal position (such as rustc's deny-by-default
  `useless_deprecated` on a trait implementation) is a method defect: move the
  attribute and re-run; never add an `allow`.
- Findings: if a finding looks release-critical (for example, a manifest a user
  can write that panics a release build), stop and report it immediately.
- Ambiguity: when a site's classification is genuinely ambiguous, include it
  under the conservative reading, record both readings, and log it in
  `Decision log`.
- Iterations: if a gate still fails after three fix attempts on the same
  cause, stop and escalate.

## Risks

- Risk: the pin goes stale before review completes. Severity: medium.
  Likelihood: high. Mitigation: rebase the branch onto the pin (D-3), cite
  module-qualified symbols, append a "commits since the pin" list at merge
  time, and state in the inventory that each consuming plan re-sweeps
  `git log <pin>..HEAD` for in-scope paths before relying on a citation.
- Risk: the oracle misses code the build does not compile (`cfg(kani)`,
  non-Linux `cfg` branches, doctests) or derive-generated code that suppresses
  deprecation warnings. Severity: high. Likelihood: medium. Mitigation: unite
  the oracle with a corrected text sweep targeted at exactly those classes, and
  seed plants in each class (EV-SEEDED).
- Risk: the register is read as proof. Severity: medium. Likelihood: medium.
  Mitigation: the inventory states that it is evidence at one pin, and that
  26.2.1's API-negative and compile-fail tests remain the backstop.
- Risk: the contract test becomes a maintenance tax. Severity: low.
  Likelihood: medium. Mitigation: it reads only the inventory and roadmap,
  never code; it fails only when the roadmap gains or renumbers a dependent
  task, and its failure message names the missing task and the fix.
- Risk: findings are mistaken for decided policy. Severity: medium.
  Likelihood: medium. Mitigation: every finding is labelled *observed* and
  names the owning task.
- Risk: `mdtablefix --wrap --renumber` refills or renumbers the new document
  (a wrapped `72.` can become a list item). Severity: low. Likelihood: medium.
  Mitigation: run `make fmt` after every edit and reread the diff.
- Risk: the roadmap dependency on "acceptance of RFC 0026" is never met,
  because no RFC in `docs/rfcs/` has ever left `Proposed` (including RFCs whose
  roadmap work is complete) and the house convention is ADR acceptance.
  Severity: high. Likelihood: high. Mitigation: D-6 asks the approver to choose
  the gate explicitly.

## Progress

- [x] (2026-09-27) Renamed the working branch to
  `26-1-1-record-model-and-consumer-inventory`.
- [x] (2026-09-27) Planning reconnaissance across six boundaries at
  `ebcedaef`.
- [x] (2026-09-27) Drafted revision 1.
- [x] (2026-09-27) Expert design review (structure and contracts;
  alternatives and cost; failure modes and viability). Verdict: revise.
- [x] (2026-09-27) Revision 2 addresses every review finding (see
  `Revision note`).
- [x] (2026-09-27) Closing review of revision 2; revision 3 fixes the
  oracle's trait-implementation blind spot, the Kani-only file gap, the
  empty-list flood, the vacuous empty-pin case, and site-identifier collisions.
- [x] (2026-09-27) Rebased onto `origin/main` at `b0e8547f` (#728, #804);
  the three plan commits replayed unchanged. Folded in #804's developers' guide
  section "Unstable Rust API for embedders" (D-10).
- [ ] Plan approved by the user, with a D-6 option chosen.
- [ ] EP-M0: entry gate passed and pin declared.
- [ ] EP-M1: contract test red, then green against a skeleton inventory.
- [ ] EP-M2: register recorded; EV-ORACLE, EV-SWEEP, EV-REVERSE, EV-SEEDED.
- [ ] EP-M3: landed work, findings, obligations, and trace matrix.
- [ ] EP-M4: documentation integration, roadmap update, gates green.

## Surprises & discoveries

- Observation: both governing records are still `Proposed`, and every RFC in
  `docs/rfcs/` reads `Proposed`, including RFCs whose roadmap tasks are
  checked. ADRs are the documents the project actually accepts. Evidence: RFC
  preambles at `ebcedaef`; ADR-001 to ADR-034 are mostly `Accepted`; roadmap
  11.1.1 says "Accept the governing ADR before implementation". Impact: EP-M0's
  gate is ADR-035, and D-6 asks the approver how to satisfy `RM-26.1.1-D`.
- Observation: a rule whose own recipe is `rule: other` lowers and reaches the
  Ninja backend. `resolve_recipe` passes `Recipe::Rule` through
  (`src/ir/from_manifest_support.rs:89`); `validate_action_recipe` returns
  `Ok(())` for it (`src/ninja_gen_validation.rs:30-34`); and
  `NamedAction::reject_rule_recipe` (`src/ninja_gen/mod.rs:286-301`) panics in
  debug builds and returns a misleading `NinjaGenError::UnsafeNinjaValue` in
  release builds. Evidence: code reading at `ebcedaef`; not yet reproduced with
  a manifest. Impact: this is the case roadmap 26.2.2 names ("No backend panic
  substitutes for compiler validation"). It is recorded for 26.1.2 to
  characterize. Release builds do not panic, so it is not release-critical
  unless reproduction shows otherwise.
- Observation: `netsuke graph` (`src/runner/graph.rs:53`, via
  `generation::build_graph`, `src/runner/generation.rs:160-163`) and
  `netsuke help targets` (`src/runner/help_query.rs:98`) lower with
  `BuildGraph::from_manifest`, which uses `RecipeShell::host_default()`, not
  the resolved recipe shell used by `build` and `generate`. Evidence: code
  reading at `ebcedaef`. Impact: with a non-default shell (for example, a Bash
  override on Windows) the action identifiers shown by `graph` can differ from
  those in generated Ninja. An H2 observation for 26.3.1.
- Observation: the hash comment in `src/hasher.rs` says canonical JSON with
  sorted keys, but `serde_json` is built with `preserve_order`
  (`Cargo.toml:135`), so field declaration order is part of the hash input.
  Evidence: `Cargo.toml` and `src/hasher.rs:58-66` at `ebcedaef`. Impact: the
  action-identity obligation must name field order, variant tag names,
  `skip_serializing_if` attributes, and interpolated shell text.
- Observation: text sweeps alone are unsound for this acceptance criterion. The
  review ran revision 1's patterns and found that they miss generic trait
  implementations (`impl<'de> Deserialize<'de> for Recipe`, three
  `From<..> for StringOrList`,
  `From<ast::DependencyOrder> for DependencyOrder`), `Self::Command { .. }`
  constructions, `EdgeId(..)`, `&mut` free-function mutation throughout
  `src/manifest/render.rs`, and `manifest.targets.retain` in
  `src/runner/help_query.rs:110`, while their seeded plants were shaped to
  match the patterns. Evidence: expert review, counts reproduced against
  `ebcedaef`. Impact: D-7 makes the compiler the primary site oracle.
- Observation: `syn` is not a direct dependency; `deprecated` is not denied by
  the workspace lint table or `.cargo/config.toml` (only `-Zthreads=8` and the
  Linux linker flag), so deprecation warnings do not stop the scratch build.
  Evidence: `Cargo.toml` `[workspace.lints.rust]`, `.cargo/config.toml`.
  Impact: the oracle needs no new dependency.
- Observation: during the revision 2 gate run, `make test` reported 3412 of
  3413 tests passing;
  `packaging_smoke_tests::packaged_manifest_retains_build_script_sources` timed
  out at 300.018 s, and again at 300.009 s when re-run alone, with the host
  load average between 11.6 and 19.2 on six cores. The revision 1 run, on the
  same Rust tree, passed it. The doctest pass, skipped by the abort, was run
  separately with `make doctest` and passed. Evidence:
  `/tmp/test-netsuke-26-1-1-record-model-and-consumer-inventory.out` and
  `/tmp/rerun-packaging-netsuke-26-1-1-record-model-and-consumer-inventory.out`.
  Impact: this test's `cargo publish --dry-run` cold-builds by design and sits
  close to its 300 s cap, so its local outcome is load-determined; CI is the
  authoritative check for it. The implementer should expect the same on a
  loaded host and confirm it in CI rather than retrying locally.

## Decision log

- Decision D-1: deliver the inventory as a standalone reference document,
  `docs/hexagonal-hardening-inventory.md`. Rationale: it is revision-pinned and
  consumed by many later tasks. An RFC appendix would mix a dated observation
  with a proposal and reopen the RFC at every refresh; `docs/netsuke-design.md`
  describes intended design; an ExecPlan is a one-task handoff.
  `docs/security-network-command-audit.md` is a precedent for a dated,
  evidence-led reference document. The review endorsed this choice.
  Date/Author: 2026-09-27, planning agent.
- Decision D-2 (revised): add exactly one Rust test,
  `tests/hexagonal_inventory_contract_tests.rs`, that checks the inventory's
  internal integrity against the roadmap and reads no Rust source. Add no
  code-reading test, scanner, or characterization fixture. Rationale: the
  roadmap is groomed with `mapsplice`, which renumbers tasks and rewrites
  dependencies; a renumbering or a newly added dependent task would silently
  invalidate the trace matrix, which is the literal acceptance criterion
  `RM-26.1.1-A1`. A document-integrity test catches that, has in-repository
  precedent (`tests/execplan_status_contract_tests.rs`), and cannot fail on
  legitimate code changes. Characterization fixtures belong to 26.1.2, and a
  code-reading completeness guard would need a new dependency or duplicate RFC
  0027's checker, and would fail on every legitimate phase-26 change to a
  snapshot. `rstest-bdd`, `insta`, `proptest`, Kani, and Verus are not
  applicable: no user-visible behaviour, multivariant output, input-domain
  invariant, bounded state space, or contractual lemma is introduced. The
  approver may strike this test; the rest of the plan does not depend on it.
  Date/Author: 2026-09-27, planning agent, after review.
- Decision D-3 (revised): pin to the `origin/main` tip when EP-M0 runs, rebase
  the branch onto it so the pin is the merge base, and append a "commits since
  the pin" list for in-scope paths at merge time. Rationale: the roadmap
  requires the inventory "at the implementation head"; a pin that is not an
  ancestor of the branch cannot be reproduced from it. Date/Author: 2026-09-27,
  planning agent.
- Decision D-4: include API-reachable constructors and mutators even when no
  internal production code uses them. Rationale: RFC 0026 says "Public fields,
  deserialization, alternative constructors, and mutation methods must not
  reopen unresolved states". Date/Author: 2026-09-27, planning agent.
- Decision D-5: no ADR. The inventory records facts and assigns them to
  decided tasks. `docs/netsuke-design.md` gains a pointer from its Stage 5
  "FUTURE" note; `docs/users-guide.md` is unchanged (no user-visible change);
  `ortho_config` is untouched (no configuration change), though configuration
  precedence is recorded as an obligation for 27.2.1. Date/Author: 2026-09-27,
  planning agent.
- Decision D-6 (open, for the approver): how to satisfy `RM-26.1.1-D`.
  Option A: the maintainer sets ADR-035 to `Accepted` (with a date and summary,
  per the style guide) before implementation starts; all milestones then run.
  Option B (recommended): EP-M1 to EP-M3 run now and produce the inventory as
  the pre-acceptance characterization RFC 0026 itself asks for; EP-M4's roadmap
  checkbox and RFC pointer wait until ADR-035 is `Accepted`, and the plan is
  `BLOCKED` at that point if it is not. Option B respects the roadmap's
  dependency for *completion* while letting the evidence inform acceptance.
  Either way, RFC 0026's own status line is not the gate, because no RFC in
  this repository has ever been moved out of `Proposed`. Date/Author:
  2026-09-27, planning agent. Awaiting the approver.
- Decision D-7: the compiler is the primary site oracle; a text sweep covers
  only what the build does not compile. Rationale: marking each inventoried
  field, variant, and inherent function `#[deprecated]` in a disposable export
  makes `rustc` report every resolved use, including `Self::Variant`, aliases,
  re-exports, generic trait implementations, and uses in `tests/**`,
  `benches/**`, and `build.rs`, without breaking the build. Privacy-based
  discovery (making fields private and collecting E0451/E0616) was rejected
  because a failing library build hides every downstream site and same-module
  uses need sealed submodules; rustdoc JSON was rejected as primary because it
  sees no function bodies (it remains an optional cross-check for impls and
  receivers). Date/Author: 2026-09-27, planning agent, after review.
- Decision D-8: register identifiers are symbol-keyed and append-only, of the
  form `A3/ir::graph::BuildGraph::replace_edge_for_output` for items and
  `<item-id>@<enclosing symbol>#<kind>[-n]` for sites (the numeric suffix
  separates several sites of one kind in one function); trace rows may cite a
  prefix ending in `/*` or `@*`, which the contract test expands. A refresh
  never reuses or renumbers them. Diagnostics (A6) are listed per variant, with
  a site count and files, and per-site citations only where construction shapes
  differ. Rationale: positional identifiers break on refresh; per-site A6 rows
  would roughly double the register without adding decision value. Date/Author:
  2026-09-27, planning agent, after review.
- Decision D-9: the rule that later ExecPlans cite register identifiers lives
  in the inventory's "Consuming this inventory" section, not in
  `docs/developers-guide.md`, which gains only a "Test suite map" entry for the
  contract test. Rationale: an evergreen guide should not carry a convention
  tied to one pinned snapshot. Date/Author: 2026-09-27, planning agent, after
  review.
- Decision D-10: treat #804's developers' guide section "Unstable Rust API
  for embedders" as an upstream artefact. It states that every Rust API is
  private in intent and unstable, and that the Netsukefile format and graph
  export are the only committed surfaces; it also documents, with tested
  snippets, embedder entry points the programme will change. The plan therefore
  cites it in CO-4 and CO-13, adds `manifest::from_str_with_env` and
  `manifest::process_env_reader` to A1, and signposts it. Rationale: the
  section is the project's own statement of what compatibility means for these
  types, and its snippets are executable, so later tasks that change those APIs
  must update it. Date/Author: 2026-09-27, planning agent, after rebasing onto
  `b0e8547f`.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and before setting the status
to `COMPLETE`.

## Verification plan

The production code is unchanged, so this task introduces no program invariant.
The obligations below are properties of the inventory and of the one
document-integrity test.

Axioms relied on:

- AXIOM-1: `rustc` emits a `deprecated` warning, with a primary span, at every
  resolved use of a `#[deprecated]` item in code it compiles, unless the use
  sits inside an `#[allow(deprecated)]` scope. (Documented compiler behaviour.)
  It does not report uses inside derive expansions, and it rejects
  `#[deprecated]` on trait implementation blocks and their items with the
  deny-by-default `useless_deprecated` lint, so calls dispatched through a trait
  (`Default::default`, `From`/`Into`, `FromStr`, `Deserialize`) are invisible
  to the oracle and are listed by a dedicated sweep instead.
- AXIOM-2: `cargo check --message-format=json` reports those warnings as JSON
  objects whose `message.code.code` is `"deprecated"`.
- AXIOM-3: `rg` reports every textual match of a pattern in the files given.
- AXIOM-4: `git archive <pin>` reproduces the tracked tree at the pin.
- AXIOM-5: CodeGraph's caller index reflects the workspace at its last reindex;
  it is a cross-check only, never sole evidence.

Obligations:

- Obligation OB-FWD (forward completeness): every production constructor,
  mutator, and consumer site of an inventoried item in code compiled on the
  build host appears in the register, and every such site in uncompiled code
  (`cfg(kani)`, non-host `cfg` branches) found by the targeted sweep appears
  too. Method: the oracle (D-7) for compiled code, united with a targeted text
  sweep. Rationale: the compiler resolves names; a text sweep only guesses
  them. The union covers the oracle's known blind spots. Domain: `src/`,
  `build.rs`, `tests/`, `test_support/`, and `benches/` at the pin, with
  `--workspace --all-targets --all-features` on the host. Artefact: inventory
  "Method" section: attribute list, commands, warning count per item,
  deduplicated site count, and the sweep patterns and counts. Evidence:
  EV-ORACLE and EV-SWEEP, each with an empty "unregistered" set. Non-vacuity:
  EV-SEEDED. A separate agent (an `alchemist` given this plan's prediction)
  applies nine plants in a second scratch export and records them without
  telling the classifier: P1 a generic trait impl constructing an `Action` in
  `src/runner/`; P2 a `Self::Script { .. }` construction in an `impl Recipe`
  helper; P3 a free function in `src/manifest/` taking `&mut BuildGraph` and
  calling `actions.values_mut()`; P4 `std::mem::take(&mut edge.inputs)` in
  `src/ninja_gen/`; P5 `*s = StringOrList::Empty` through a
  `&mut StringOrList`; P6 a `mut self` builder on `BuildEdge`; P7 a
  `#[cfg(windows)]` function building a `BuildEdge` literal; P8 a `#[cfg(kani)]`
  `BuildEdge` construction; P9 a construction inside a `#[cfg(test)]` module;
  P10 an `EdgeId(0)` call in `src/ninja_gen/`; P11 a
  `#[cfg(windows)] impl BuildGraph { fn f(&mut self) { self.actions.clear() } }`
  block; P12 a `BuildEdge` literal inside `src/ir/cycle_verification.rs` (a
  file that is Kani-only through its parent's `#[cfg(kani)] mod` line, with no
  `cfg` text of its own). The method must report P1 to P8 and P10 to P12 as
  unregistered sites (production, or Kani-only for P8 and P12; P7, P8, P11, and
  P12 through the sweep) and P9 as test only. Any missed plant fails the
  obligation and sends the method back for repair.
- Obligation OB-REV (reverse soundness): every register row names a
  module-qualified symbol that exists at the pin, at or around its cited line.
  Method: `git grep -n` at the pin, per row. Rationale: checking only one side
  would accept a register padded with stale or invented rows. Evidence:
  EV-REVERSE, "N rows checked, 0 misses". Non-vacuity: a fictitious row (for
  example, `BuildGraph::remove_edge`) in a scratch copy must be reported.
- Obligation OB-INTEGRITY (document integrity, executable): the inventory
  declares exactly one pin and contains no other 40-character hexadecimal
  commit identifier; register identifiers are unique and well-formed; every
  trace-matrix task identifier exists in the roadmap; every roadmap task in the
  dependency closure of 26.1.1 has a trace-matrix row or an "Excluded" entry
  with a reason; every register identifier is referenced by a trace-matrix row
  or listed as unchanged; every `CO-n` is referenced by at least one row; every
  `OBS-n` and every H1 to H4 finding names an owning task that exists. Method:
  `rstest` tests in `tests/hexagonal_inventory_contract_tests.rs`. Pure parsing
  helpers (section extraction, pin extraction, task-identifier extraction from
  the trace-matrix section only, roadmap task and dependency parsing, and
  dependency-closure computation) are tested on in-memory fixtures; one test
  applies them to the real `docs/` files through a `cap_std` directory handle.
  Rationale: these are finite, structural properties of two documents;
  parameterized example tests with negative fixtures fully specify them.
  Domain: the real inventory and roadmap, plus fixtures. Evidence: EV-RED (the
  real-document test fails while the inventory is absent, naming the missing
  file) and EV-CONTRACT (all cases pass). Non-vacuity: negative fixtures that
  must each fail with a specific message: two different pins; no `Pin:` line at
  all (so an empty document cannot pass); a trace row naming a task absent from
  the roadmap; a dependent task (including a transitive one) missing from the
  matrix; a task identifier that appears only outside the trace-matrix section;
  a duplicate register identifier; an orphan `CO-n`; and an `OBS-n` without an
  owner. The dependency parser must be shown to follow a two-step chain (a
  fixture where task C depends on B, which depends on 26.1.1) so the closure is
  not trivially direct.
- Obligation OB-652 (no repetition): the canonical edge arena appears only as
  landed work and as obligation `CO-14`. Method: review of the listed mentions
  from `rg -n '652|714' docs/hexagonal-hardening-inventory.md`. Evidence:
  EV-FINDINGS. Non-vacuity: review only; each mention is checked against its
  section.
- Obligation OB-GATES: `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint` (which runs `make spelling`), and
  `make nixie` pass after each milestone. Method: `scrutineer`, sequentially,
  with `tee` logs. Evidence: EV-GATES. Non-vacuity: `make check-fmt` checks
  Markdown through `mdtablefix --check`; `make lint` compiles and lints the new
  test.

## Plan of work

Stage A is EP-M0. Stage B (red) is EP-M1's failing contract test and EP-M2's
seeded controls, which must fail on seeded input before results are trusted.
Stage C is EP-M2 and EP-M3. Stage D is EP-M4.

### Inventory document structure

Create `docs/hexagonal-hardening-inventory.md` with these sections, in order:

1. **Title and pin.** `# Hexagonal hardening model and consumer inventory`, then
   a line `Pin: <40-character SHA>` followed by the commit subject, date, and
   the toolchain from `rust-toolchain.toml`. State that every `path:line` is
   valid only at the pin, that the document is a dated snapshot owned by RFC
   0026, that RFC 0027 owns automated coverage, and that the register is
   evidence, not proof: 26.2.1's API-negative and compile-fail tests remain the
   backstop.
2. **Scope and definitions.** The terms from this plan, the list of
   inventoried items per area, and the visibility classes.
3. **Method.** Oracle attribute list and commands, per-item warning counts,
   deduplicated site counts, sweep patterns and counts, the classification
   procedure, and the EV-SEEDED and EV-REVERSE results, each with its date.
4. **Register.** One subsection per area, each with a type table (columns
   `ID`, `Item`, `Visibility`, `Derives`, `Location`) and a site table (columns
   `ID`, `Site`, `Kind`, `Location`, `Production callers`, `Planned change`,
   `Obligations`), where `Kind` is one of constructor, mutator, consumer, or
   API-reachable. Areas and the enclosing symbols each must cover at minimum
   (anticipated sites for the Register-size tolerance):
   - A1 authored types: `ast::{NetsukeManifest, MacroDefinition, Rule, Target,
     Recipe, RawRecipe, StringOrList, DependencyOrder}`; `Recipe`'s
     `Deserialize`; `StringOrList`'s three `From` impls; `deserialize_actions`
     (forces `phony = true`); `NetsukeManifest::validate_recipes`;
     `Recipe::is_dependency_only`; `manifest::from_str`,
     `manifest::from_str_with_env`, and `manifest::process_env_reader`;
     `from_path`;
     `render_manifest` and every `&mut` helper in `src/manifest/render.rs`;
     `serde_json::from_value` into `NetsukeManifest` in `src/manifest/mod.rs`;
     `help_query::manifest_for_graph_validation` (`retain` on `actions` and
     `targets`).
   - A2 lowering: `from_manifest`, `from_manifest_for_shell`,
     `TargetLoweringContext`, `process_rules`, `process_targets`,
     `process_defaults`, `detect_cycles`, `register_action`, `ActionBindings`,
     `resolve_rule`, `resolve_recipe`, `resolve_command`, `resolve_script`,
     `insert_edge_for_outputs`, `duplicate_output_error`,
     `From<ast::DependencyOrder> for ir::DependencyOrder`, and
     `CommandBindings::new`.
   - A3 graph storage: `BuildGraph` (`Default`), both `cfg` forms of
     `insert_edge`, `insert_canonical_edge`, `index_output_aliases`,
     `index_output`, `replace_edge_for_output`, the `pub` fields `actions` and
     `default_targets`; `Action` and its fields; `BuildEdge` and its fields;
     `EdgeId(..)`; the IR `DependencyOrder`; `ActionHasher::hash`. Record that
     the IR types do not implement `Deserialize` (an obligation for 26.2.1 to
     keep) and that `insert_edge` does not check that `action_id` exists.
   - A4 backend consumers: `ninja_gen::{generate, generate_into,
     generate_into_with_shell, generate_with_shell, write_action_rules}`,
     `edge_requires_gates`, `NamedAction` (literal and `shell_text`,
     `reject_rule_recipe`, `reject_empty_command_recipe`,
     `assert_shell_command`), `validate_action_recipe`,
     `validate_action_metadata`, `dyndep::{generate_bundle,
     generate_bundle_for_shell, generate_bundle_inner, render_edge}`,
     `RenderedAction`, `RecipeShell::command_value`, `ShellText::new`,
     `NinjaValue::from_encoded`, `escape_ninja_value`, `escape_metadata_value`,
     and `GraphView::from_build_graph`, recording the model fields each reads
     and every debug-only panic path.
   - A5 CLI orchestration: `runner::{run, run_with_ninja_program}`,
     `run_with_ninja_program_resolver`, `ExecutionContext`, `dispatch::execute`
     and its `execute_build`, `execute_generate`, `execute_clean`, and
     `execute_help`; `runner::{execute_build, execute_ninja_tool}`;
     `graph_generation::{GraphGenerationContext, generate_ninja_with_shell}`;
     `generation::{ManifestLoadInputs::from_cli, build_graph,
     build_graph_for_shell, ninja_text_for_shell}`;
     `ninja_process_adapter::{ninja_process_options, run_ninja,
     run_ninja_tool}`; `resolve_recipe_shell`; `StderrMode::from_json_enabled`;
     `create_temp_ninja_file`; `materialize_dyndep_bundle`;
     `prune_dyndep_bundle`; `open_effective_dir`; `graph::handle_graph`;
     `help_query`; the `Commands`, `BuildArgs`, and `BuildTargets` types
     (`Deserialize` and `Default` derives); and every `Cli` field each reads,
     classified as semantic or presentation.
   - A6 diagnostics: per variant of `IrGenError` (including `InvalidManifest`),
     `NinjaGenError` (including the variants with fixed English `#[error]`
     text), `ManifestError`, and `RunnerError`; whether each enum is
     `#[non_exhaustive]`; `LocalizedMessage` and `set_localizer`;
     `localize_recipe_error`; the keys used in `src/localization/keys.rs`; the
     diagnostic codes; `DiagnosticDocument` and `DiagnosticEntry`; the success
     envelope in `src/result_json.rs` and `src/json_envelope.rs`; and
     `handle_runner_error`, `render_runtime_error_json`, and
     `parse_cli_or_exit` in `src/main.rs` (exit codes).
   - A7 existing seams: `NinjaBuildRequest`, `NinjaToolRequest`,
     `NinjaProcessOptions`, `CommandEnv`, the `MonotonicClock` parameters,
     `StatusObserver`, `StatusReporter`, the `cap_std` directory handles,
     `DyndepPublicationLease`, `DyndepPublication`, `TempNinjaFile`, and
     `GraphRenderer`, recording whether each is injectable and how tests
     substitute it.
   Close the register with an appendix listing, by file only, every test,
   doctest, bench, `test_support`, and Kani-only site per item.
5. **Landed work.** One entry each, with merge commit (short SHA permitted
   here) and the preservation obligation it creates: `#652` / PR `#714` (edge
   arena, `2c030fd1`); `#705` (typed redirect boundary, closed; H6 context);
   `#699` / ADR-027; `#753` / ADR-034; `#754` (JSON excerpt guard); `#696`
   (clock seam); and any further in-scope commit between `79545e12` and the pin.
6. **Findings.** The remaining parts of H1 to H4 as observed facts with
   citations, and numbered observations `OBS-n`, each labelled *observed* with
   an owning task. Seed from `Artefacts and notes`; re-verify each at the pin.
7. **Compatibility obligations.** `CO-1` to `CO-14` as listed under
   `Artefacts and notes`, each with its evidence (existing tests, snapshots,
   ADRs) and the tasks it binds.
8. **Trace matrix.** Two tables. "Changes": one row per task that changes an
   inventoried item (26.1.2, 26.2.1, 26.2.2, 26.2.3, 26.3.1, 26.3.2, 27.1.1,
   27.1.2, 27.1.3, 27.2.1, 27.2.2, 27.2.3, and 28.1.2 for exception mapping),
   with columns `Task`, `Register items`, `Findings`, and `Obligations`.
   "Consumes": one row per task that only reads the inventory (28.1.1, the rest
   of phase 28 transitively, 29.2.1, 29.2.3, 29.3.2, 29.4.1, and 29.4.2; 27.3.1
   and 27.3.2 get optional rows through the phase-27 Entry line, not the
   dependency closure). Then an "Excluded" list for any task in the dependency
   closure that has no row, each with a reason, and an "Unchanged" list of
   register items no task changes.
9. **Consuming this inventory.** The rule that a consuming ExecPlan cites
   register identifiers in its `Conformance basis`, runs
   `git log <pin>..HEAD -- src build.rs tests test_support benches` first, and
   re-sweeps any in-scope file that changed.
10. **Refreshing this inventory.** A refresh produces a new pinned revision of
    the whole document, keeps existing identifiers, and never reuses retired
    ones.

### Contract test

Create `tests/hexagonal_inventory_contract_tests.rs` modelled on
`tests/execplan_status_contract_tests.rs`: a `//!` module comment, a
`repo_root()` helper returning a `cap_std::fs_utf8::Dir`, and pure helpers,
each with a `///` comment:

- `section(text, heading) -> Option<&str>` returns the body of a `##`
  section up to the next `##` heading.
- `pins(text) -> BTreeSet<String>` returns every 40-character lowercase
  hexadecimal token.
- `declared_pin(text) -> Option<String>` reads the `Pin:` line.
- `task_ids(text) -> BTreeSet<String>` returns identifiers matching
  `\b2[6-9]\.\d+\.\d+\b`, so version strings such as `0.1.0` never match
  (callers pass only the relevant section).
- `roadmap_tasks(roadmap) -> BTreeMap<String, BTreeSet<String>>` parses each
  `- [ ] N.N.N.` or `- [x] N.N.N.` task and the identifiers named in its
  `Dependencies:` bullet, including continuation lines, up to the next bullet
  at the same or lower indentation.
- `dependents_of(tasks, root) -> BTreeSet<String>` computes the transitive
  closure of tasks whose dependencies reach `root`.
- `register_ids(text) -> Vec<String>` returns the first-column identifiers of
  the register tables.

Use `regex` (already a dev-dependency) for tokenization, `rstest` for
parameterized cases, and `pretty_assertions::assert_eq` for set comparisons so
failures show the difference. Every test returns `anyhow::Result<()>`. Keep the
file under 400 lines. `tests/integration_test_wiring_tests.rs` discovers
top-level test files automatically; confirm it passes.

## Milestones and plateaus

### EP-M0: entry gate and pin

- Identifier and outcome: EP-M0. The gate from D-6 is satisfied (or, under
  Option B, recorded as pending for EP-M4); the pin is declared; the branch is
  rebased onto it.
- Requirements and gaps: `RM-26.1.1-D`, `RM-26.1.1-S1`.
- Acceptance evidence: EV-ENTRY: the ADR-035 status read with
  `git show "$PIN":docs/adr-035-semantic-compiler-boundaries.md`; the pin's
  SHA, subject, and date; and the in-scope commits between `ebcedaef` and the
  pin.
- Conformance check: no file changed except this plan.
- Recovery: if the gate fails, set `Status: BLOCKED` and stop.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1: contract test, red then green

- Identifier and outcome: EP-M1. The contract test exists; its fixture cases
  pass; its real-document case passes against a skeleton inventory containing
  the pin, section headings, and empty tables with an "Excluded" list naming
  every dependent task as "pending EP-M3".
- Requirements and gaps: `RM-26.1.1-A1` (mechanism).
- Acceptance evidence: EV-RED (before the skeleton exists, the real-document
  test fails with a message naming `docs/hexagonal-hardening-inventory.md`),
  EV-CONTRACT on the skeleton, and EV-GATES.
- Conformance check: the test reads only `docs/`; no dependency added.
- Recovery: revert the commit.
- Remaining gaps: register, findings, matrix.
- Compatibility decision: none.

### EP-M2: register

- Identifier and outcome: EP-M2. Inventory sections 1 to 4 are complete.
- Requirements and gaps: `RM-26.1.1-S1`, `RM-26.1.1-A2`, `RFC26-CM1`.
- Acceptance evidence: EV-ORACLE and EV-SWEEP (empty unregistered sets),
  EV-SEEDED (P1 to P8 and P10 to P12 found, P9 classified as test), EV-REVERSE
  (zero misses), EV-CONTRACT, and EV-GATES.
- Conformance check: no tracked source change; scratch directories deleted;
  single pin; every D-4 API-reachable item present.
- Recovery: additive; revert to retry. Scratch exports are recreated from
  `git archive` every time.
- Remaining gaps: findings, obligations, matrix, integration.
- Compatibility decision: none.

### EP-M3: findings, obligations, and trace matrix

- Identifier and outcome: EP-M3. Sections 5 to 10 are complete, and the
  "pending EP-M3" exclusions are replaced by real rows or reasoned exclusions.
- Requirements and gaps: `RM-26.1.1-S2`, `RM-26.1.1-A1`, `ADR35-RISK`,
  `RFC26-CM2`, `RFC26-H1` to `RFC26-H4`.
- Acceptance evidence: EV-FINDINGS, EV-CONTRACT, and EV-GATES.
- Conformance check: findings labelled observed; no policy decided; each
  obligation traced to an ADR, RFC clause, or existing test.
- Recovery: additive; revert to retry.
- Remaining gaps: integration.
- Compatibility decision: none.

### EP-M4: integration and completion

- Identifier and outcome: EP-M4. The inventory is discoverable and the roadmap
  records completion.
- Requirements and gaps: `RM-26.1.1` done; `RM-26.1.1-D` satisfied.
- Edits:
  - `docs/contents.md`: add the inventory beside the
    `roadmap-hexagonal-hardening.md` entry.
  - `docs/roadmap-hexagonal-hardening.md`: `- [ ] 26.1.1.` becomes
    `- [x] 26.1.1.`, with a link to the inventory; wording otherwise unchanged.
  - `docs/rfcs/0026-hexagonal-domain-hardening.md`: one sentence in "Current
    state and audit reconciliation" naming the pinned inventory as the
    implementation baseline; the `79545e12` audit text stays.
  - `docs/netsuke-design.md`: a pointer from the Stage 5 "FUTURE" note to the
    inventory's H1 finding.
  - `docs/developers-guide.md`: a "Test suite map" entry for
    `tests/hexagonal_inventory_contract_tests.rs`.
  - The inventory: the "commits since the pin" list, using short SHAs only so
    that the one-pin check still holds.
  - This plan: `Status: COMPLETE`; `Outcomes & retrospective`.
- Acceptance evidence: EV-GATES on the final tree.
- Conformance check: `docs/users-guide.md` unchanged; no ADR added; under D-6
  Option B, ADR-035 reads `Accepted` in
  `git fetch origin && git show origin/main:docs/adr-035-semantic-compiler-boundaries.md`
  before the checkbox is ticked, otherwise set `Status: BLOCKED` and stop.
- Recovery: revert the integration commit.
- Remaining gaps: none; 26.1.2 starts from the inventory.
- Compatibility decision: none.

## Concrete steps

Run commands from the repository root unless stated otherwise. Shell variables
do not persist between separate tool invocations, so each block below sets the
variables it uses.

EP-M0:

```bash
git fetch origin
PIN="$(git rev-parse origin/main)"; echo "$PIN"
git show "$PIN":docs/adr-035-semantic-compiler-boundaries.md | sed -n '1,8p'
git log --oneline "ebcedaef..$PIN" -- src build.rs tests test_support benches
git rebase "$PIN" 26-1-1-record-model-and-consumer-inventory
```

Expected: ADR-035 reads `Accepted` under Option A, or either value under Option
B. Record `PIN` in `Decision log`.

EP-M1, red:

```bash
cargo nextest run --test hexagonal_inventory_contract_tests 2>&1 \
  | tee /tmp/red-netsuke-26-1-1-record-model-and-consumer-inventory.out
```

Expected: the fixture cases pass and `real_inventory_is_consistent` fails with
an error naming `docs/hexagonal-hardening-inventory.md`. Then add the skeleton
and re-run; expect all cases to pass.

EP-M2, oracle. Create a full export under the ignored `target/` directory:

```bash
PIN="<pin>"; ROOT="$(pwd)"
SCRATCH="$ROOT/target/inventory-26-1-1/$PIN"
rm -rf "$SCRATCH" && mkdir -p "$SCRATCH/tree"
git archive "$PIN" | tar -x -C "$SCRATCH/tree"
```

In `$SCRATCH/tree` only, add `#[deprecated(note = "inventory")]` to every
field, every variant, and every inherent method or associated function (inside
`impl Type`, never inside `impl Trait for Type` or on an implementation block,
which rustc's deny-by-default `useless_deprecated` rejects) of the items named
in A1 to A7, to each free function named there, and to the tuple struct
`EdgeId` itself (deprecating its field does not flag `EdgeId(..)` calls, which
resolve to the constructor). Then:

```bash
PIN="<pin>"; ROOT="$(pwd)"; SCRATCH="$ROOT/target/inventory-26-1-1/$PIN"
cd "$SCRATCH/tree" && CARGO_TARGET_DIR="$SCRATCH/target" \
  cargo check --workspace --all-targets --all-features --message-format=json \
  > "/tmp/oracle-netsuke-26-1-1-$PIN.json"
jq -r 'select(.reason == "compiler-message" and .message.code.code == "deprecated")
  | .message as $m | $m.spans[] | select(.is_primary)
  | "\(.file_name):\(.line_start):\(.column_start)\t\($m.message)"' \
  "/tmp/oracle-netsuke-26-1-1-$PIN.json" | sort -u \
  > "/tmp/oracle-sites-netsuke-26-1-1-$PIN.out"
wc -l "/tmp/oracle-sites-netsuke-26-1-1-$PIN.out"
```

Classify each site as constructor, mutator, or consumer, and as production or
not: list test-only modules first with
`rg -n -A1 '#\[cfg\((test|kani)\)\]' src | rg 'mod \w+'`, resolve each
`#[path = ..]`, and treat inline `#[cfg(test)] mod` blocks as test code. Then
subtract the register; expect an empty set.

EP-M2, text sweeps (every file, trait-dispatched construction, and doctests):

```bash
PIN="<pin>"; ROOT="$(pwd)"; TREE="$ROOT/target/inventory-26-1-1/$PIN/tree"
T='Action|BuildEdge|BuildGraph|EdgeId|Recipe|RawRecipe|Rule|Target|NetsukeManifest|StringOrList|DependencyOrder'
F="/tmp/sweep-files-netsuke-26-1-1-$PIN.out"
rg --files --type rust "$TREE/src" "$TREE/build.rs" | tee "$F"
test -s "$F" || { echo "empty sweep list"; exit 1; }
P="\b(Self|$T)(::\w+)?\s*[{(]|&mut\s+($T)\b|mem::(take|replace|swap)"
P="$P|&mut self|\bmut self\b|self\s*:\s*&mut|\.(retain|clear|insert|push|extend|values_mut|iter_mut)\("
xargs -r -d '\n' rg -n --type rust "$P" < "$F" \
  | tee "/tmp/sweep-sites-netsuke-26-1-1-$PIN.out"
rg -n --type rust "(\b($T)::default\(|Default::default|\.into\(\)|from_(str|value|slice|reader)\b)" \
  "$TREE/src" "$TREE/build.rs" | tee "/tmp/sweep-trait-netsuke-26-1-1-$PIN.out"
rg -n --type rust "^\s*//[/!] .*\b($T)\b" "$TREE/src" \
  | tee "/tmp/sweep-doctest-netsuke-26-1-1-$PIN.out"
```

The first sweep covers every file, so Kani-only modules enabled from a parent's
`#[cfg(kani)] mod` line are included; remove the hits the oracle already
reported, then classify the rest (in particular those inside uncompiled `cfg`
regions). The second lists trait-dispatched construction, which the oracle
cannot see; classify each hit by hand and record its count separately in
Method. The third lists doctest sites for the appendix.

Add production hits to the register and Kani, test, and doctest hits to the
appendix. Optionally cross-check implementations and receivers with rustdoc JSON
(`cargo rustdoc --lib -- -Z unstable-options --output-format json
--document-private-items`)
and public mutators with CodeGraph callers.

EV-SEEDED: hand the twelve plants from OB-FWD and the prediction to an
`alchemist` agent, which applies them in a *second* export
(`$ROOT/target/inventory-26-1-1/$PIN-seeded`) and records them in
`/tmp/seeded-netsuke-26-1-1-$PIN.out`. Re-run the oracle and sweep there,
classify blind, and compare. Expected: P1 to P8 and P10 to P12 reported as
unregistered sites, P9 classified as test only. Then delete both exports:

```bash
PIN="<pin>"; rm -rf "$(pwd)/target/inventory-26-1-1"
```

EV-REVERSE: for each register row, `git grep -n '<symbol>' "$PIN" -- '<path>'`;
record "N rows checked, 0 misses", and confirm a fictitious row is reported.

After every Markdown edit:

```bash
make fmt 2>&1 | tee /tmp/fmt-netsuke-26-1-1-record-model-and-consumer-inventory.out
git --no-pager diff --no-ext-diff --stat
```

Gates, through `scrutineer`, sequentially, after each milestone:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-26-1-1-record-model-and-consumer-inventory.out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-26-1-1-record-model-and-consumer-inventory.out
make lint 2>&1 | tee /tmp/lint-netsuke-26-1-1-record-model-and-consumer-inventory.out
make test 2>&1 | tee /tmp/test-netsuke-26-1-1-record-model-and-consumer-inventory.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-26-1-1-record-model-and-consumer-inventory.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-26-1-1-record-model-and-consumer-inventory.out
```

Expected: each exits `0`. Commit after each milestone with an imperative
subject, for example `Record the pinned semantic-boundary register (26.1.1)`.

## Validation and acceptance

- `cargo nextest run --test hexagonal_inventory_contract_tests` passes; before
  the inventory skeleton exists, `real_inventory_is_consistent` fails naming
  the missing file (EV-RED).
- Choosing any task in the dependency closure of 26.1.1, a reviewer finds its
  trace-matrix row (or reasoned exclusion), follows a register identifier to a
  `path:line` that exists at the pin, and reads a named obligation.
- The "Method" section records oracle and sweep counts with empty unregistered
  sets and a seeded-fault result of eleven plants found and one correctly
  classified as test.
- `#652` / `#714` appear only under "Landed work" and in `CO-14`.
- Roadmap item 26.1.1 is checked (subject to D-6) and links to the inventory.

Red-Green-Refactor applies to the contract test (EV-RED, then EV-CONTRACT, then
refactor with the test re-run). For the inventory itself, the seeded controls
are the red stage: they must fail on seeded input before the result on the real
tree is trusted.

Quality criteria:

- Tests: `make test` passes, including the new contract test.
- Verification: OB-FWD, OB-REV, OB-INTEGRITY, OB-652, and OB-GATES discharged.
- Lint and types: `make check-fmt`, `make typecheck`, `make lint`,
  `make markdownlint`, and `make nixie` exit `0`.
- Performance and security: no production change or dependency; the scratch
  build runs once per pin and never concurrently with a gate.

## Idempotence and recovery

Every step is repeatable. Scratch exports are recreated from `git archive` and
deleted afterwards; they live under the ignored `target/` directory, never in
the working tree. The inventory and test are new files; the other edits are
small insertions; reverting a milestone commit restores the prior state. Do not
use `git stash` (the stash is shared between worktrees); use a work-in-progress
commit instead.

## Artefacts and notes

Planning-time observations at `ebcedaef`, to re-verify at the pin.

Remaining H1: `Action` (`src/ir/graph.rs:252-270`) derives
`Debug, Clone, PartialEq, Serialize`, has all-`pub` fields, and stores
`recipe: Recipe` from `crate::ast`; its only production literal is in
`register_action` (`src/ir/from_manifest_support.rs:54-61`). `BuildEdge`
(`src/ir/graph.rs:273-295`) has all-`pub` fields and one production literal in
`process_targets` (`src/ir/from_manifest.rs:165-175`). `BuildGraph`
(`src/ir/graph.rs:39-49`) derives `Default` and exposes `pub actions` and
`pub default_targets`. The dependency-only sentinel is
`Recipe::Command { command: StringOrList::Empty }` (`src/ast/mod.rs:222`),
recognized by `Recipe::is_dependency_only` (`src/ast/mod.rs:180-186`).

Remaining H2: `BuildGraph` retains no interpreter. `from_manifest_for_shell`
(`src/ir/from_manifest.rs:63-67`, `#[doc(hidden)] pub`) and
`ninja_gen::generate_with_shell` (`src/ninja_gen/explicit_shell.rs:17-24`,
re-exported at `src/ninja_gen/mod.rs:37`) take the interpreter independently;
`generate` and `generate_into` use `RecipeShell::host_default()`. The runner's
`build` and `generate` paths thread one value through both stages
(`src/runner/graph_generation.rs:23,56-69`); `graph` and `help targets` do not
(see `Surprises & discoveries`).

Remaining H3: `runner::execute_build` and `runner::execute_ninja_tool`
(`src/runner/mod.rs:192-240,275-328`) interleave generation, dyndep
publication, temporary-manifest creation, and Ninja spawn, reading `Cli`
throughout. `NinjaBuildRequest` and `NinjaToolRequest`
(`src/runner/process/request.rs:21-54`) are already free of `Cli`.
`StderrMode::from_json_enabled(cli.json)` lets a presentation flag decide
stream routing. `open_effective_dir(cli)` re-derives directory authority from
`cli.directory` on each call.

Remaining H4: `IrGenError` (`src/ir/graph_error.rs:39-231`) pairs structured
fields with a `LocalizedMessage` in `RuleNotFound`, `MultipleRules`,
`EmptyRule`, `DuplicateOutput`, `CircularDependency`, `ActionSerialisation`, and
`InvalidCommand`; `InvalidManifest` carries a `&'static str`. `NinjaGenError`
mixes `LocalizedMessage` variants with fixed English `#[error]` text. Neither
enum is `#[non_exhaustive]` or carries a `miette` code, so the JSON `code` is
null for them. `localize_recipe_error` classifies by
`to_string().starts_with(EMPTY_COMMAND_LIST_ERROR)`
(`src/manifest/registration.rs:15-23`).

Seed observations (`OBS-n`): OBS-1 rule-to-rule delegation reaches the backend
(26.1.2, 26.2.2). OBS-2 duplicate rule names are silently last-wins in
`process_rules` (`src/ir/from_manifest.rs:99-103`) (26.1.2). OBS-3
`DuplicateOutput` is built at two sites with different argument shapes
(`src/ir/graph.rs:66-70`; `src/ir/from_manifest_support.rs:156-162`) (27.1.1).
OBS-4 `MissingAction` is built at two sites (`src/ninja_gen/mod.rs`,
`src/ninja_gen/dyndep.rs`) (27.1.1). OBS-5 the `EMPTY_COMMAND_LIST_ERROR`
string match (27.1.2). OBS-6 IR and Ninja errors have no diagnostic code
(27.1.3). OBS-7 `generate_with_shell` and the host-default `generate` paths
accept an unchecked shell (26.3.1). OBS-8 `graph` and `help targets` lower with
the host default shell (26.3.1). OBS-9 the `src/hasher.rs` comment claims
sorted keys (26.2.3). OBS-10 public fields, `Default`, `insert_edge` without an
action-existence check, and `replace_edge_for_output` can create dangling or
swapped `action_id` values (26.2.1). OBS-11 `assert_shell_command` panics only
in debug builds (26.2.2). OBS-12 `render.rs` renders the `rule:` selector and
mutates authored types in place (26.1.2, 26.2.1). OBS-13 the `cfg(kani)`
`insert_edge` returns `EdgeId` unconditionally (26.2.3). OBS-14 the error enums
are exhaustive and publicly constructible (27.1.1). OBS-15 `StderrMode` follows
`cli.json` (27.2.1). OBS-16 `open_effective_dir` re-derives authority per call
(27.2.2).

Seed obligations (`CO-n`): CO-1 manifest acceptance and rejection, including
`MISSING_RECIPE_ERROR`, `EMPTY_COMMAND_LIST_ERROR`, and the mutual-exclusion
message. CO-2 generated Ninja bytes and repeated-run determinism on both
emission paths (`generate*` and the dyndep bundle). CO-3 action identity: the
hash covers field declaration order (`preserve_order`), variant tag names,
`skip_serializing_if`, and interpolated shell text; any change needs a rebuild
rationale. CO-4 graph rendering (DOT and HTML golden snapshots; the developers'
guide names the graph export as one of Netsuke's two committed surfaces) and
the success-result JSON envelope (`schema_version` 1) for `build`, `generate`,
`clean`, and `graph`; there is no JSON graph export. CO-5 diagnostics: existing
codes, today's null codes, the JSON diagnostic schema, the `#754` excerpt
guard, human-mode printing of the outermost context, and exit codes (1 for
runtime errors; for argument errors, 2 in human mode and 1 on the JSON path).
CO-6 localization: `src/localization/keys.rs` and the Fluent catalogues stay in
sync (audited by `build.rs`). CO-7 ADR-014 separation of shell quoting and
Ninja escaping, and the ADR-027 and ADR-034 placeholder contract. CO-8 ADR-011
and ADR-012 dyndep publication and retention, with the lease spanning spawn and
pruning. CO-9 `POLONIUS-REFUSED(id-is-data)`. CO-10 Kani harness
synchronization across every `cfg(kani)` file (at planning time:
`src/ast/mod.rs`, `src/ir/cmd_interpolate/mod.rs`, `src/ir/cycle_detector.rs`,
`src/ir/cycle.rs`, `src/ir/cycle_support.rs`, `src/ir/from_manifest.rs`,
`src/ir/from_manifest_support.rs`, `src/ir/graph_kani_map.rs`,
`src/ir/graph.rs`, `src/ir/sort_utils.rs`, and the `*verification.rs` modules).
CO-11 configuration layering precedence, keys, and `NETSUKE_*` variables through
`ortho_config`. CO-12 the quickstart and unannotated manifests. CO-13 library
API: pre-1.0 and published as `netsuke-build`; every changed or removed `pub`
item gets a CHANGELOG "**Breaking:**" entry, with no shims; and a change to an
API described in the developers' guide's "Unstable Rust API for embedders"
section (for example `BuildGraph::insert_edge`, `manifest::from_str_with_env`,
or the Ninja request types) updates that section and its tested `devguide-*`
snippets registered in `tests/documentation_examples_tests.rs` in the same
change. CO-14 canonical edge arena invariants from `#652`: alias identity,
atomic duplicate rejection, and no per-output cloning.

## Interfaces and dependencies

No production interface or dependency changes. The new test uses existing
dev-dependencies only: `rstest`, `anyhow`, `camino`, `cap_std`, `regex`, and
`pretty_assertions`. The helper signatures are listed under "Contract test" in
`Plan of work`. Tools used: `git`, `rg`, `jq`, `cargo`, CodeGraph (MCP), `make`,
`mdtablefix`, `markdownlint-cli2`, `typos`, and `nixie`.

## Revision note

- Revision 1 (2026-09-27): initial draft from six-way reconnaissance at
  `ebcedaef`.
- Revision 2 (2026-09-27): revised after an expert design review whose
  verdict was "revise". The primary site method is now the compiler (D-7),
  because revision 1's text sweeps missed generic trait implementations,
  `Self::Variant` constructions, tuple constructors, `&mut` free-function
  mutation, and `retain`, and its seeded plants could not fail. Added the
  document-integrity contract test (D-2) and moved EV-PIN and EV-TRACE into it,
  restricting task extraction to the trace-matrix section and deriving the
  expected set from the roadmap's dependency closure. Replaced the RFC-status
  gate with the D-6 choice. Widened the inventoried item set (render mutators,
  library entry points, `ShellText`, `NinjaValue`, runner contexts,
  `build.rs`), tightened the definitions (no `Clone` constructor; visibility
  classes; `&mut T` functions as mutators), corrected the obligations (no JSON
  graph export; `preserve_order` hashing; exit codes; CHANGELOG practice), and
  added observations OBS-8 to OBS-16. Moved scratch builds from `/tmp` to the
  ignored `target/` directory and made every command self-contained. The
  remaining work is unchanged in kind: approval, then EP-M0 to EP-M4.

- Revision 3 (2026-09-27): closing review of revision 2. Record that
  `#[deprecated]` cannot be placed on trait implementations and that derive
  expansions and trait-dispatched calls produce no warnings, and add a
  dedicated sweep for trait-dispatched construction. Deprecate `EdgeId` itself.
  Sweep every file rather than `cfg`-bearing files only, so Kani-only modules
  are covered, and guard the sweep against an empty file list. Keep the item
  name and column in the oracle's output. Add plants P10 to P12, an empty-pin
  negative fixture, and site-level identifiers. Restrict task extraction to
  `2[6-9]` identifiers, correct the dependency-closure list (29.2.3 to 29.4.2
  in; 27.3.x optional), and record the load-bound packaging-test timeout seen
  while gating the second revision. The remaining work is unchanged: approval,
  then EP-M0 to EP-M4.

- Revision 4 (2026-09-27): rebased onto `b0e8547f`. Folded in #804's
  developers' guide section "Unstable Rust API for embedders" (D-10): cited in
  CO-4 and CO-13, signposted, and its extra manifest entry points added to A1.
  Updated the crate version to `0.1.0-beta4`. The remaining work is unchanged:
  approval, then EP-M0 to EP-M4.

[rfc-0026]: ../rfcs/0026-hexagonal-domain-hardening.md
[adr-035]: ../adr-035-semantic-compiler-boundaries.md
[roadmap]: ../roadmap-hexagonal-hardening.md
