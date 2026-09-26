# Record a revision-pinned model and consumer inventory (roadmap 26.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 1. See `Revision note` at the foot of this document. This plan was
written against `origin/main` at `ebcedaef683efeb795d0ab65f94ad11dc5b92eb2`
("RFC 0029: first-class host facts with explicit collection (#802)"). It must
not be implemented until it is approved and the entry gate in milestone EP-M0
passes.

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
since, six of them in `src/`. Roadmap task 26.1.1 exists to replace that
snapshot with an inventory pinned to the implementation head.

After this change:

- A new reference document, `docs/hexagonal-hardening-inventory.md`, names the
  exact commit it describes and lists, for authored types, lowering, graph
  storage, backend consumers, command-line interface (CLI) orchestration,
  diagnostics, and existing seams, every production constructor and every
  production mutator of the types the programme will change.
- The same document records which earlier work has already landed (without
  scheduling issue `#652` again), which parts of H1 to H4 remain, and any new
  observations found during the sweep.
- A trace matrix maps every planned phase-26 and phase-27 task (and 28.1.1,
  which also depends on this inventory) to the inventory items it changes and
  the compatibility obligation it must preserve.
- The roadmap entry 26.1.1 is marked done and links to the inventory.

A reviewer can observe success by opening the inventory, choosing any planned
task in the roadmap, and following its row in the trace matrix to named,
revision-pinned code locations and a named compatibility obligation; and by
re-running the recorded sweep commands at the pinned commit and obtaining the
same site set that the register lists.

This task changes no Rust source, no manifest behaviour, no command-line
behaviour, and no generated output. See `Decision log` entry D-2 for why no new
Rust test is added.

## Context and orientation

Assume no prior knowledge of this repository. This section names every file the
plan reads or edits.

### The pipeline being inventoried

Netsuke processes a manifest in stages (see `docs/netsuke-design.md`, Section
1.2, "The Six Stages of a Netsuke Build"):

1. **Authored types.** `src/ast/mod.rs`, `src/ast/target.rs`,
   `src/ast/string_or_list.rs`, and `src/ast/dependency_order.rs` define the
   deserialized manifest: `NetsukeManifest`, `Rule`, `Target`, the `Recipe`
   enumeration (`Command`, `Script`, `Rule` variants), `StringOrList`, and
   `DependencyOrder`. They are built only by `serde` deserialization, via a
   hand-written `Deserialize` implementation for `Recipe`. `src/manifest/**`
   loads, renders, and expands manifests but defines no further authored recipe
   types.
2. **Lowering.** `src/ir/from_manifest.rs` (entry points
   `BuildGraph::from_manifest` and `BuildGraph::from_manifest_for_shell`) and
   `src/ir/from_manifest_support.rs` (`register_action`, `resolve_rule`,
   `resolve_recipe`) turn the manifest into the intermediate representation
   (IR). `src/ir/cmd_interpolate/**` substitutes and shell-quotes input and
   output paths for a chosen interpreter.
3. **Graph storage.** `src/ir/graph.rs` defines `BuildGraph`, `Action`,
   `BuildEdge`, `EdgeId`, and the IR `DependencyOrder`. `Action.recipe` stores
   an `ast::Recipe`: this is finding H1. `src/hasher.rs` derives each action's
   identity by hashing the serialized `Action`.
4. **Backend consumers.** `src/ninja_gen/**`, `src/ninja_gen_validation.rs`,
   `src/ninja_gen_recipe_shell.rs`, and `src/ninja_gen_escape.rs` render Ninja
   text; `src/graph_view/**` projects the graph for the `graph` subcommand.
5. **CLI orchestration.** `src/main.rs` calls `runner::run` in
   `src/runner/mod.rs`, which dispatches through `src/runner/dispatch.rs` and
   spawns Ninja through `src/runner/process/**`. Layered configuration is
   merged by `ortho_config` in `src/cli/config.rs` and `src/cli/merge/**`.
6. **Diagnostics.** `src/ir/graph_error.rs` (`IrGenError`),
   `src/ninja_gen_error.rs` (`NinjaGenError`), `src/manifest/diagnostics/**`
   (`ManifestError`), `src/runner/error.rs` (`RunnerError`),
   `src/localization/mod.rs` (`LocalizedMessage`), and `src/diagnostic_json.rs`
   (structured output).

### Terms used in this plan

- **Revision-pinned.** Every code citation in the inventory refers to one
  named commit, the *pin*. Line numbers are only meaningful at that commit, so
  the inventory cites them as `path:line` beneath a single pin declaration and
  also names the enclosing symbol, so a later reader can relocate the site.
- **Production code.** Code compiled into the `netsuke` library or binary when
  neither `cfg(test)` nor `cfg(kani)` is set, for any supported target and
  feature combination. Test modules (including `#[path]`-included `*_tests.rs`
  files), `tests/**`, `test_support/**`, doctests, and Kani-only (`cfg(kani)`)
  code are *not* production, but the inventory lists them by file in a separate
  appendix so that later migration tasks know which callers to update.
- **Constructor.** Any production site that brings a value of an inventoried
  type into existence: a struct or enum-variant literal, an associated function
  returning `Self`, a `From`/`TryFrom`/`FromStr`/`Default` implementation, a
  `Deserialize` implementation (derived or hand-written), or a `Clone`
  implementation where cloning can duplicate an invalid state.
- **Mutator.** Any production site that changes an existing value: a method
  taking `&mut self`, a write to a `pub` field (including collection mutation
  through a `pub` field, such as `graph.actions.insert(...)`), or an accessor
  returning `&mut` into the value.
- **API-reachable.** A constructor or mutator that an external library caller
  can use because the type, field, or function is public from `src/lib.rs`,
  whether or not any internal production code uses it. For example,
  `BuildGraph::replace_edge_for_output` is `pub`, but at the planning head only
  test modules call it.
- **Compatibility obligation.** A behaviour a later migration must preserve or
  deliberately change with an explained rationale. Netsuke is pre-1.0 (crate
  version `0.1.0-beta3`), so Rust source APIs carry no compatibility commitment
  (see `Constraints`); the obligations are behavioural.

### Governing documents

- `docs/roadmap-hexagonal-hardening.md`, section 26.1, defines the task.
- `docs/rfcs/0026-hexagonal-domain-hardening.md`, section "Current state and
  audit reconciliation", defines H1 to H6 against baseline `79545e12`.
- `docs/adr-035-semantic-compiler-boundaries.md` records the direction.
- `docs/rfcs/0027-executable-architecture-contract.md`, section "Inventory and
  coverage", assigns *automated* source inventory to the phase-28 checker. This
  plan must not build that checker.
- `docs/adr-014-backend-text-escaping-seam.md` (shell quoting versus Ninja
  escaping), `docs/adr-019-structured-command-shell-selection.md`,
  `docs/adr-027-command-placeholder-contract.md`,
  `docs/adr-034-preserve-script-in-out-as-shell-variables.md`,
  `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md`,
  `docs/adr-012-bound-dyndep-sidecar-retention.md`, and
  `docs/adr-008-environment-seam-taxonomy.md` define contracts the inventory
  must name as compatibility obligations.
- `docs/polonius.md` classifies borrow-centric sites, including
  `POLONIUS-REFUSED(id-is-data)` in `register_action`.

## Signposts

Read these before starting, in this order:

1. `AGENTS.md` (repository rules, gates, Markdown rules).
2. `docs/roadmap-hexagonal-hardening.md` sections "Existing work and
   ownership" and 26 to 28.
3. `docs/rfcs/0026-hexagonal-domain-hardening.md` and
   `docs/adr-035-semantic-compiler-boundaries.md`.
4. `docs/rfcs/0027-executable-architecture-contract.md` "Proposed checker",
   so the inventory stays a document, not a checker.
5. `docs/netsuke-design.md` Sections 1.2, 3.2, 5, 6.1, and 7.
6. `docs/developers-guide.md` sections "IR dependency classes", "Graph view
   projection and renderer adapters", and "Internal support module boundaries".
7. `docs/documentation-style-guide.md` (tables, headings, ExecPlan status
   vocabulary) and `docs/contents.md`.
8. For later tasks that consume this inventory (not this one):
   `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/rstest-bdd-users-guide.md`,
   `docs/reliable-testing-in-rust-via-dependency-injection.md`,
   `docs/rust-doctest-dry-guide.md`, `docs/ortho-config-users-guide.md`, and
   `docs/formal-verification-methods-in-netsuke.md`.

Skills to load: `execplans` (this plan's format), `hexagonal-architecture`
(classify each item by boundary role without transplanting a directory pattern),
`rust-router` then `rust-types-and-apis` (classify constructors, visibility,
and invalid-state reachability), `nll-to-polonius` (respect `POLONIUS` tags
when describing borrow-shaped sites), `codegraph-mcp` (caller and callee
sweeps), `en-gb-oxendict-style` (prose), and `firecrawl-mcp` only if an
external format or tool fact is needed.

## Conformance basis

Upstream artefacts and their revisions at planning time:

- Roadmap: `docs/roadmap-hexagonal-hardening.md` at `ebcedaef`, item 26.1.1
  (identifier `RM-26.1.1`). Its acceptance text is `RM-26.1.1-A1` ("every
  planned change maps to a current item and compatibility obligation") and
  `RM-26.1.1-A2` ("the inventory includes every production constructor and
  mutator"). Its scope bullets are `RM-26.1.1-S1` (identify authored types,
  lowering, graph storage, backend consumers, CLI orchestration, diagnostics,
  and existing seams at the implementation head) and `RM-26.1.1-S2` (record
  landed work and remaining H1 to H4 findings without repeating `#652`).
- RFC: `docs/rfcs/0026-hexagonal-domain-hardening.md` at `ebcedaef`, status
  **Proposed**. Traced items: `RFC26-H1` to `RFC26-H4` (audit findings);
  `RFC26-CM1` ("Implementation must characterize manifest acceptance, graph
  export, generated Ninja, diagnostics, and library entry points before
  extraction"); `RFC26-CM2` ("Explain any necessary hash change and its rebuild
  consequences").
- ADR: `docs/adr-035-semantic-compiler-boundaries.md` at `ebcedaef`, status
  **Proposed**. Traced item: `ADR35-RISK` ("Rule delegation, declaration
  precedence, action identity, and diagnostic shape need characterization
  before refactoring").
- RFC: `docs/rfcs/0027-executable-architecture-contract.md`, section
  "Inventory and coverage" (`RFC27-INV`), as a *boundary*: automated inventory
  belongs to roadmap tasks 28.1.1 and 28.2.x.
- No Terms of Reference document exists for this programme; the roadmap and
  RFC 0026 serve that role. No separate technical-design revision exists beyond
  `docs/netsuke-design.md` at `ebcedaef`.

Trace chains this plan must preserve:

```plaintext
RM-26.1.1-S1 -> RFC26-CM1 -> EP-M1 -> inventory §Register (sections A1..A7) -> EV-SWEEP, EV-REVERSE
RM-26.1.1-A2 -> EP-M1 -> inventory §Register + §Method -> EV-SWEEP, EV-SEEDED
RM-26.1.1-S2 -> RFC26-H1..H4 -> EP-M2 -> inventory §Landed work, §Findings -> EV-FINDINGS
RM-26.1.1-A1 -> ADR35-RISK, RFC26-CM2 -> EP-M2 -> inventory §Obligations, §Trace matrix -> EV-TRACE
RM-26.1.1 (done) -> EP-M3 -> roadmap checkbox + docs/contents.md entry -> EV-GATES
```

## Constraints

- Do not modify any Rust source, test, build script, Cargo manifest,
  `Cargo.lock`, Makefile, workflow, Fluent catalogue, or snapshot. This task is
  documentation only. If recording the inventory appears to require a code
  change (for example, to make a site observable), stop and escalate.
- Do not fix any defect discovered during the sweep, however small. Record it
  in the inventory's findings and map it to its owning task (26.1.2 for
  characterization; 26.2.x, 26.3.x, or 27.x for remediation). RFC 0026 permits
  a separately reproduced release-critical fix, but that is a separate task
  with its own plan.
- Do not build, commit, or propose an automated source-inventory tool,
  scanner, contract test, or policy file (`architecture.toml`,
  `architecture-exceptions.toml`). RFC 0027 assigns those to phase 28.
- Do not re-schedule, re-describe as future work, or re-audit the canonical
  edge arena from issue `#652` / PR `#714`. Record it once as landed, with its
  merge commit, and list only the obligations later tasks must preserve.
- Do not decide disputed placeholder semantics; `#699` and ADR-027/ADR-034 own
  them. Do not decide rule-delegation or duplicate-declaration policy; 26.1.2
  owns that decision. The inventory records *observed* behaviour only.
- Every code citation must be valid at the declared pin. Use one pin for the
  whole document; do not mix revisions.
- The inventory must not promise Rust source-API stability. The crate is
  pre-1.0 (`0.1.0-beta3`); per the execplans policy no source-compatibility
  machinery (aliases, facades, deprecated entry points) may be prescribed for
  it. Compatibility obligations are behavioural and persisted-format ones only.
- Markdown must satisfy `docs/documentation-style-guide.md`: en-GB-oxendict
  spelling (`-ize`), 80-column wrapping of prose, unwrapped tables and headings,
  `-` bullets, and footnotes as `[^n]`. Code identifiers go in backticks; bare
  `#NNN` issue references at a line start must be backticked to avoid being
  read as headings.
- Run gates only through the `scrutineer` agent, sequentially, never in
  parallel with another gate.

## Tolerances (exception triggers)

- Entry: if RFC 0026 or ADR-035 is not `Accepted` when implementation is
  requested, stop, set this plan to `BLOCKED`, and ask whether the maintainer
  wants to accept them or explicitly waive the roadmap dependency.
- Scope: if the change touches more than eight files, or any file outside
  `docs/`, stop and escalate.
- Drift: if the chosen pin differs from `ebcedaef` and any commit between them
  touches `src/ast/`, `src/ir/`, `src/ninja_gen*`, `src/graph_view/`,
  `src/runner/`, `src/cli/`, `src/manifest/`, `src/localization/`,
  `src/diagnostic_json*`, `src/hasher.rs`, or `src/recipe_shell.rs`, re-run the
  full sweep rather than patching this plan's planning-time observations. If
  the pin moves again during implementation, stop and ask whether to re-pin or
  proceed on the declared pin.
- Register size: if the forward sweep finds more than 25 production
  constructor or mutator sites *not* anticipated in `Artefacts and notes`, stop
  and report, because the scope definitions may be wrong.
- Findings: if a finding suggests a user-visible defect severe enough to be
  release-critical (for example, a manifest a user can write that panics a
  release build), stop and report it immediately rather than only recording it.
- Ambiguity: if a site's classification (production versus test, constructor
  versus query) is genuinely ambiguous, record both readings in the inventory,
  choose the conservative one (include it), and note it in `Decision log`.
- Iterations: if the Markdown gates still fail after three fix attempts on the
  same file, stop and escalate.

## Risks

- Risk: the pin goes stale before review completes because `main` moves.
  Severity: medium. Likelihood: high. Mitigation: declare the pin prominently,
  cite the enclosing symbol with every line, and accept that the inventory is a
  snapshot; the Drift tolerance defines when to re-sweep.
- Risk: text sweeps miss a constructor shape (a macro-generated literal, a
  `Self { .. }` inside an `impl`, a re-exported alias, or an inline module).
  Severity: high. Likelihood: medium. Mitigation: run several independent
  patterns (literal, `Self {`, `impl` headers, derives, `&mut self`, `pub`
  field writes), cross-check public mutators with CodeGraph callers, and prove
  sensitivity with the seeded-fault control EV-SEEDED.
- Risk: the inventory quietly becomes the phase-28 checker's inventory.
  Severity: medium. Likelihood: low. Mitigation: the Constraints forbid tools;
  the inventory states that RFC 0027 owns automated coverage.
- Risk: recording findings is mistaken for deciding policy (for example,
  "last-declared rule wins" read as the intended contract). Severity: medium.
  Likelihood: medium. Mitigation: every finding carries the label *observed*
  and names the task that owns the decision.
- Risk: Markdown tooling reflows or renumbers the new document unexpectedly
  (`mdtablefix --wrap --renumber` refills paragraphs and can turn a wrapped
  number such as `72.` into a list item). Severity: low. Likelihood: medium.
  Mitigation: run `make fmt` after every edit, reread the diff, and avoid
  sentences that wrap onto a line starting with a number and a full stop.
- Risk: the roadmap dependency on "acceptance of RFC 0026" is not met.
  Severity: high. Likelihood: high (both RFC 0026 and ADR-035 read "Proposed" at
  `ebcedaef`). Mitigation: EP-M0 is an explicit entry gate.

## Progress

- [x] (2026-09-27) Renamed the working branch to
  `26-1-1-record-model-and-consumer-inventory`.
- [x] (2026-09-27) Planning reconnaissance across six boundaries (authored
  types, lowering and graph, backend and shell, CLI and runner, diagnostics,
  documentation and governance) at `ebcedaef`.
- [x] (2026-09-27) Drafted this ExecPlan (revision 1).
- [ ] Expert design review of the draft and revision.
- [ ] Plan approved by the user.
- [ ] EP-M0: entry gate passed and pin declared.
- [ ] EP-M1: register recorded; EV-SWEEP, EV-REVERSE, EV-SEEDED captured.
- [ ] EP-M2: landed work, findings, obligations, and trace matrix recorded.
- [ ] EP-M3: documentation integration, roadmap marked done, gates green.

## Surprises & discoveries

- Observation: both governing records are still `Proposed`.
  Evidence: `docs/rfcs/0026-hexagonal-domain-hardening.md` preamble "Status:
  Proposed"; `docs/adr-035-semantic-compiler-boundaries.md` "## Status /
  Proposed." at `ebcedaef`. Impact: the roadmap dependency "acceptance of RFC
  0026" is unmet; EP-M0 gates implementation on it.
- Observation: a rule whose own recipe is `rule: other` lowers successfully and
  reaches the Ninja backend. `resolve_recipe` passes `Recipe::Rule` through
  (`src/ir/from_manifest_support.rs:89`), `validate_action_recipe` returns
  `Ok(())` for it (`src/ninja_gen_validation.rs:31-35`), and
  `NamedAction::reject_rule_recipe` panics in debug builds and returns
  `NinjaGenError::UnsafeNinjaValue` in release builds
  (`src/ninja_gen/mod.rs:289-301`). Evidence: code reading at `ebcedaef`; not
  yet reproduced with a manifest. Impact: this is the "backend panic
  substitutes for compiler validation" case named by roadmap 26.2.2. The
  inventory records it as an observation for 26.1.2 to characterize. It is not
  release-critical by itself, because release builds do not panic, but the
  release diagnostic is misleading; if reproduction shows otherwise, the
  Findings tolerance applies.
- Observation: `syn` is not a direct dependency anywhere in the workspace; the
  only precedent source scanner is the text scanner in
  `tests/env_access_suppressions/` (about 2,300 lines). Evidence: `Cargo.toml`
  `[dev-dependencies]`; `Cargo.lock` lists `syn` only transitively. Impact: an
  AST-based completeness test would add a dependency and duplicate RFC 0027's
  checker; see D-2.

## Decision log

- Decision D-1: deliver the inventory as a standalone reference document,
  `docs/hexagonal-hardening-inventory.md`, not as an appendix to RFC 0026, a
  section of `docs/netsuke-design.md`, or the body of this ExecPlan. Rationale:
  the inventory is revision-pinned and will be consumed by at least eleven
  later tasks; an RFC appendix would mix a proposal with an observation, the
  design document describes the intended design rather than a dated snapshot,
  and an ExecPlan is a handoff document for one task. The name pairs it with
  `docs/roadmap-hexagonal-hardening.md`, and
  `docs/security-network-command-audit.md` is a precedent for a dated,
  evidence-led reference document. Date/Author: 2026-09-27, planning agent.
- Decision D-2: add no Rust test, contract test, or scanning tool in this task.
  Rationale: 26.1.1 changes no behaviour, so there is nothing for `rstest`,
  `rstest-bdd`, `insta`, `proptest`, Kani, or Verus to specify; the
  characterization fixtures the user's testing guidance calls for are
  explicitly owned by roadmap 26.1.2 ("Establish recipe and declaration
  characterization fixtures"), and adding them here would pre-empt that task's
  observed-versus-intended review. An executable completeness guard would
  either add `syn` as a new dependency (a tolerance breach) or re-implement a
  text scanner, and in both cases would duplicate the automated inventory that
  RFC 0027 assigns to roadmap 28.1.1 and 28.2.x. A guard over a revision-pinned
  snapshot would also fail on every legitimate phase-26 change, turning a dated
  record into a maintenance tax. Completeness is instead evidenced by a
  reproducible two-directional sweep with a seeded-fault control (see
  `Verification plan`). Date/Author: 2026-09-27, planning agent. Subject to
  expert review.
- Decision D-3: pin to the `origin/main` tip at the moment EP-M0 runs, not to
  this plan's planning head, unless the two are identical. Rationale: the
  roadmap requires the inventory "at the implementation head". The
  planning-time observations in `Artefacts and notes` are hypotheses to be
  re-verified at the pin, not results to be copied. Date/Author: 2026-09-27,
  planning agent.
- Decision D-4: include API-reachable constructors and mutators (public fields,
  `pub` methods with only test callers, public entry points taking a separate
  interpreter) in the register even when no internal production code uses them.
  Rationale: RFC 0026 requires that "public fields, deserialization,
  alternative constructors, and mutation methods must not reopen unresolved
  states". An unused public mutator is exactly such a reopening path.
  Date/Author: 2026-09-27, planning agent.
- Decision D-5: no ADR is needed for this task; the inventory records facts
  and assigns them to already-decided tasks. `docs/netsuke-design.md` gains
  only a pointer from its existing "FUTURE" note under Stage 5, and
  `docs/users-guide.md` is unchanged because no user-visible behaviour changes.
  `ortho_config` is not touched because no configuration surface changes.
  Date/Author: 2026-09-27, planning agent.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and before setting the status
to `COMPLETE`.

## Verification plan

This task introduces no executable behaviour, so it introduces no program
invariant for tests, property tests, bounded model checking, or proofs to
discharge. The obligations below are properties *of the document*, checked
mechanically where possible. D-2 records why no Rust verification artefact is
added and which later task owns characterization.

Axioms relied on:

- AX-1: `rg` (ripgrep) and `git grep` report every textual match of a regular
  expression in the files they are given. This is a documented tool interface
  and is not itself verified.
- AX-2: CodeGraph's caller index reflects the workspace at the time of a
  reindex. Because this is a derived index, it is used only to cross-check the
  text sweep, never as the sole evidence for a site.
- AX-3: `cfg(test)`, `cfg(kani)`, and file naming conventions (`*_tests.rs`,
  `*_tests/`, `*_verification.rs`, `tests/**`) identify non-production code in
  this repository. Where a module is test-only only by virtue of a
  `#[cfg(test)] #[path = ...] mod` declaration in its parent, the sweep must
  read the parent declaration rather than trust the file name.

Obligations:

- Obligation OB-FWD (forward completeness): every production constructor or
  mutator site of an inventoried type that the recorded sweep finds at the pin
  appears in the register, with its enclosing symbol, visibility, and
  production callers. Method: a recorded set of independent `rg` sweeps
  (patterns in `Concrete steps`) run over a clean export of the pinned tree,
  followed by a classification pass and a set difference against the register.
  Rationale: the register is a finite list; a set difference over explicit
  patterns is exhaustive for the patterns chosen, and several patterns cover
  the syntactic forms that can construct or mutate a value. Domain: all `.rs`
  files under `src/` at the pin, all features and targets (text sweeps see every
  `cfg` branch). Artefact: inventory section "Method", which lists the exact
  commands, the pin, and each pattern's raw hit count and production hit count.
  Evidence: EV-SWEEP, the recorded counts and an empty "unregistered sites"
  difference. Non-vacuity: EV-SEEDED. In a scratch export (never committed),
  plant one site per sweep pattern (an `Action { .. }` literal in
  `src/runner/`, a `Self { .. }` constructor on `BuildEdge`, a new
  `pub fn f(&mut self)` on `BuildGraph`, a `graph.actions.insert` in a non-test
  file, a new `impl Default for Action`, and a `pub` field write to
  `BuildEdge.action_id`) and one site inside a `#[cfg(test)]` module. The sweep
  and classification must report exactly the six production plants as
  unregistered and must not report the test plant. A sweep that reports nothing
  for a plant fails the obligation for that pattern.
- Obligation OB-REV (reverse soundness): every register entry names a symbol
  that exists at the pin, at the cited line or within the cited enclosing
  symbol. Method: for each entry, `git grep -n` at the pin for the enclosing
  symbol and the cited construct. Rationale: forward completeness alone would
  pass if the register were a superset padded with stale or invented entries;
  checking both sides of the comparison rejects that. Domain: every register
  row. Artefact: inventory section "Method", reverse-check subsection.
  Evidence: EV-REVERSE, a count of rows checked equal to the register size and
  zero misses. Non-vacuity: add one deliberately fictitious row to a scratch
  copy of the register (for example, `BuildGraph::remove_edge`) and confirm the
  reverse check reports it.
- Obligation OB-TRACE (trace totality): every roadmap task that lists 26.1.1
  as a direct or transitive dependency and changes an inventoried type (26.1.2,
  26.2.1, 26.2.2, 26.2.3, 26.3.1, 26.3.2, 27.1.1, 27.1.2, 27.1.3, 27.2.1,
  27.2.2, 27.2.3, and 28.1.1) has at least one register item and at least one
  compatibility obligation in the trace matrix; every register item marked
  "changes" names at least one owning task; and every remaining H1 to H4
  finding names an owning task. Method: a mechanical cross-check: extract task
  identifiers from the roadmap with `rg -o '2[678]\.[0-9]+\.[0-9]+'` and
  compare with those in the trace matrix; then review each row. Rationale: this
  is the literal acceptance text `RM-26.1.1-A1`. Evidence: EV-TRACE, both
  identifier sets and an empty difference. Non-vacuity: delete one matrix row
  in a scratch copy and confirm the identifier comparison reports the missing
  task.
- Obligation OB-652 (no repetition): the inventory describes the canonical edge
  arena only as landed work with preservation obligations, and no trace row
  schedules its implementation. Method: review, plus
  `rg -n '652|714' docs/hexagonal-hardening-inventory.md` to list every mention
  for the reviewer. Evidence: EV-FINDINGS. Non-vacuity: not mechanically
  testable; the reviewer confirms each listed mention is in the "Landed work"
  section or an obligation.
- Obligation OB-PIN (single revision): the inventory declares exactly one pin,
  and every permalink in it uses that SHA. Method:
  `rg -o 'blob/[0-9a-f]{40}' docs/hexagonal-hardening-inventory.md | sort -u`
  returns one value equal to the declared pin. Evidence: EV-PIN. Non-vacuity:
  the command returns two values if any other SHA is present; confirm by
  running it against a scratch copy with one altered permalink.
- Obligation OB-GATES: the repository's gates pass after each milestone:
  `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint` (which runs `make spelling`), and `make nixie`. Method:
  `scrutineer` runs them sequentially with `tee` logs under `/tmp`. Evidence:
  EV-GATES. Non-vacuity: the Rust gates cannot detect documentation faults,
  which is why the Markdown gates are included; `make check-fmt` is sensitive
  to Markdown through `mdtablefix --check`.

## Plan of work

Stage A (understand, no edits) is EP-M0. Stage B, which for code work would add
failing tests, is replaced by the seeded-fault and fictitious-row controls,
which must fail before the register is trusted. Stage C (build the artefact) is
EP-M1 and EP-M2. Stage D (integrate and validate) is EP-M3.

### Inventory document structure

Create `docs/hexagonal-hardening-inventory.md` with these top-level sections,
in this order:

1. **Title and pin.** `# Hexagonal hardening model and consumer inventory`,
   followed by a pin block: commit SHA, commit subject, commit date, toolchain
   from `rust-toolchain.toml`, and the statement that every `path:line` in the
   document is valid only at that commit. State that the document is a dated
   snapshot for roadmap phases 26 and 27, owned by RFC 0026, and that RFC 0027
   owns automated inventory.
2. **Scope and definitions.** The definitions of production code,
   constructor, mutator, API-reachable, and compatibility obligation from this
   plan, and the list of inventoried types.
3. **Method.** The exact sweep commands, the classification rules, each
   pattern's raw and production hit counts (EV-SWEEP), the reverse-check result
   (EV-REVERSE), and the seeded-fault result (EV-SEEDED), each naming the date
   and pin on which they ran.
4. **Register.** One subsection per area, each with a table whose columns are
   `ID`, `Symbol`, `Kind`, `Visibility`, `Location`, `Production callers`,
   `Planned change`, and `Obligations`:
   - A1 authored types (`ast::NetsukeManifest`, `Rule`, `Target`, `Recipe`,
     `StringOrList`, `DependencyOrder`, the private `RawRecipe`, and the
     `deserialize_actions` post-deserialization mutation that forces
     `phony = true`);
   - A2 lowering (`from_manifest`, `from_manifest_for_shell`,
     `process_rules`, `process_targets`, `register_action`, `resolve_rule`,
     `resolve_recipe`, `resolve_command`, `resolve_script`,
     `insert_edge_for_outputs`, `duplicate_output_error`, `detect_cycles`,
     and `NetsukeManifest::validate_recipes`);
   - A3 graph storage (`BuildGraph` and its `Default`, `insert_edge` in both
     `cfg(kani)` and `cfg(not(kani))` forms, `insert_canonical_edge`,
     `index_output_aliases`, `index_output`, `replace_edge_for_output`, the
     `pub` fields `actions` and `default_targets`; `Action` and its `pub`
     fields; `BuildEdge` and its `pub` fields; `EdgeId`; the IR
     `DependencyOrder`; `ActionHasher::hash`);
   - A4 backend consumers (`ninja_gen::generate`, `generate_into`,
     `generate_into_with_shell`, `generate_with_shell`, `write_action_rules`,
     `NamedAction::shell_text`, `reject_rule_recipe`,
     `reject_empty_command_recipe`, `validate_action_recipe`,
     `dyndep::generate_bundle`, `generate_bundle_for_shell`,
     `RecipeShell::command_value`, the escaping functions, and
     `GraphView::from_build_graph`), recording for each which model fields it
     reads and whether it has a panic path;
   - A5 CLI orchestration (`runner::run`, `run_with_ninja_program`,
     `dispatch::execute` and its per-command handlers, `execute_build`,
     `execute_ninja_tool`, `generate_ninja_with_shell`,
     `GraphGenerationContext`, `ninja_process_adapter`, `resolve_recipe_shell`,
     `create_temp_ninja_file`, `materialize_dyndep_bundle`,
     `prune_dyndep_bundle`, `open_effective_dir`), recording which `Cli`
     fields each reads and classifying each field as semantic or presentation;
   - A6 diagnostics (`IrGenError`, `NinjaGenError`, `ManifestError`,
     `RunnerError`, `LocalizedMessage`, every construction site of a variant
     that pairs structured fields with a `LocalizedMessage`,
     `localize_recipe_error`, diagnostic codes, the JSON document types, and
     the exit-code mapping in `src/main.rs`);
   - A7 existing seams (`NinjaBuildRequest`, `NinjaToolRequest`,
     `NinjaProcessOptions`, `CommandEnv`, the `MonotonicClock` parameters,
     `StatusObserver` and `StatusReporter`, `cap_std` directory handles,
     `DyndepPublicationLease`, `DyndepPublication`, `TempNinjaFile`,
     `GraphRenderer`), recording whether each is already injectable and how
     tests substitute it.
   Follow the tables with an appendix listing, by file only, the test, doctest,
   and Kani-only constructors and mutators of each type, so that later
   migrations know which callers they must update.
5. **Landed work.** One short entry per item, each with its merge commit and
   the preservation obligation it creates: `#652` / PR `#714` (edge arena, merge
   `2c030fd1`); `#705` (typed redirect boundary, closed; H6 context only);
   `#699` / ADR-027 (placeholder contract); `#753` / ADR-034 (script `$in`/
   `$out` as shell variables); `#754` (JSON diagnostic excerpt guard); and any
   further relevant commit between `79545e12` and the pin.
6. **Findings.** The remaining parts of H1 to H4, stated as observed facts with
   citations, plus numbered observations (`OBS-n`) found during the sweep. Each
   carries the label *observed*, the owning task, and, where relevant, the
   phrase "policy decision owned by 26.1.2" or "diagnostic contract owned by
   27.1.3".
7. **Compatibility obligations.** A numbered catalogue (`CO-n`), including at
   least: manifest acceptance and rejection (including the established
   `MISSING_RECIPE_ERROR` text); generated Ninja bytes and repeated-run
   determinism; action identity hashes and their rebuild consequences; graph
   export (DOT, HTML, and JSON); diagnostic codes, JSON schema version 1,
   localized text, and exit codes; ADR-014 shell-quoting versus Ninja-escaping
   separation; ADR-011/ADR-012 dyndep publication and retention; the
   `POLONIUS-REFUSED(id-is-data)` action-identity decision; synchronization of
   the Kani harnesses in `src/ir/*_verification.rs` with any `cfg(kani)` type
   change; and the quickstart and unannotated-manifest shallow end. State
   explicitly that Rust source APIs are pre-1.0 and carry no
   source-compatibility commitment, and that any existing external library
   consumer must be named before a compatibility layer is proposed.
8. **Trace matrix.** One row per planned task (the thirteen listed in OB-TRACE),
   with columns `Task`, `Register items changed`, `Findings addressed`, and
   `Obligations`. Add a closing paragraph listing register items no task
   changes, so a reader sees what the programme deliberately leaves alone.
9. **Refreshing this inventory.** A short procedure for re-running the sweep
   against a new pin, stating that a refresh produces a new pinned revision of
   the document rather than editing citations piecemeal.

## Milestones and plateaus

### EP-M0: entry gate and pin

- Identifier and outcome: EP-M0. The prerequisites are confirmed, and the pin
  is declared in `Decision log`.
- Requirements and gaps: roadmap dependency "acceptance of RFC 0026";
  `RM-26.1.1-S1` ("at the implementation head").
- Acceptance evidence: EV-ENTRY: the RFC 0026 and ADR-035 status lines read
  `Accepted` (or the user's explicit waiver is recorded in `Decision log`); the
  pin SHA, subject, and date are recorded; the list of commits between
  `ebcedaef` and the pin touching in-scope paths is recorded.
- Conformance check: no file has changed yet.
- Recovery: if the gate fails, set `Status: BLOCKED` and stop.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1: register

- Identifier and outcome: EP-M1. `docs/hexagonal-hardening-inventory.md`
  exists with sections 1 to 4 (pin, scope, method, register and appendix).
- Requirements and gaps: `RM-26.1.1-S1`, `RM-26.1.1-A2`, `RFC26-CM1`.
- Acceptance evidence: EV-SWEEP (empty unregistered difference), EV-REVERSE
  (zero misses), EV-SEEDED (six plants reported, test plant not reported),
  EV-PIN, and EV-GATES.
- Conformance check: no source change; no tool committed; single pin; every
  API-reachable mutator from D-4 is present.
- Recovery: the document is additive; revert the commit to retry. The seeded
  controls run in a disposable export under `/tmp`, so they leave no state.
- Remaining gaps: findings, obligations, trace matrix, integration.
- Compatibility decision: none.

### EP-M2: findings, obligations, and trace matrix

- Identifier and outcome: EP-M2. Inventory sections 5 to 9 are complete.
- Requirements and gaps: `RM-26.1.1-S2`, `RM-26.1.1-A1`, `ADR35-RISK`,
  `RFC26-CM2`, `RFC26-H1` to `RFC26-H4`.
- Acceptance evidence: EV-FINDINGS (every H1 to H4 part and every `OBS-n`
  carries citations and an owning task; `#652` appears only as landed work),
  EV-TRACE (empty identifier difference and a reviewed matrix), and EV-GATES.
- Conformance check: findings are labelled observed, no policy is decided, and
  each obligation traces to an ADR, RFC clause, or test that already exists.
- Recovery: additive edits; revert to retry.
- Remaining gaps: integration and roadmap update.
- Compatibility decision: none.

### EP-M3: documentation integration and completion

- Identifier and outcome: EP-M3. The inventory is discoverable and the roadmap
  records completion.
- Requirements and gaps: `RM-26.1.1` done.
- Edits:
  - `docs/contents.md`: add an entry for the inventory beside the roadmap
    entry for `roadmap-hexagonal-hardening.md`.
  - `docs/roadmap-hexagonal-hardening.md`: change `- [ ] 26.1.1.` to
    `- [x] 26.1.1.` and add a link to the inventory in the task's bullets,
    keeping the existing text.
  - `docs/rfcs/0026-hexagonal-domain-hardening.md`: in "Current state and audit
    reconciliation", add one sentence stating that the implementation baseline
    for phases 26 and 27 is the pinned inventory, leaving the historical
    `79545e12` audit text intact.
  - `docs/netsuke-design.md`: in the Stage 5 "FUTURE" note, add a pointer to
    the inventory's H1 finding.
  - `docs/developers-guide.md`: in "Internal support module boundaries" (or
    the nearest section about IR ownership), add a short paragraph saying that
    phase-26 and phase-27 changes to inventoried constructors or mutators must
    cite the inventory's register identifier in their ExecPlan, and that a
    refresh follows the inventory's own procedure.
  - This ExecPlan: set `Status: COMPLETE` and complete
    `Outcomes & retrospective`.
- Acceptance evidence: EV-GATES on the final tree; links resolve (checked by
  `make markdownlint` and by reading the rendered links).
- Conformance check: `docs/users-guide.md` is unchanged (no user-visible
  change); no ADR added (D-5); roadmap wording otherwise unchanged.
- Recovery: revert the integration commit.
- Remaining gaps: none for 26.1.1; 26.1.2 begins from the inventory.
- Compatibility decision: none.

## Concrete steps

Run every command from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/87392da2-6f4c-47f5-8990-bf106b4279fe`,
unless stated otherwise.

EP-M0, entry gate and pin:

```bash
git fetch origin
git rev-parse origin/main
sed -n '1,12p' docs/rfcs/0026-hexagonal-domain-hardening.md | grep -n Status
sed -n '1,8p' docs/adr-035-semantic-compiler-boundaries.md
git log --oneline ebcedaef..origin/main -- src/ast src/ir src/ninja_gen src/ninja_gen*.rs \
  src/graph_view src/runner src/cli src/manifest src/localization src/diagnostic_json*.rs \
  src/hasher.rs src/recipe_shell.rs
```

Expected: the status lines read `Accepted`. If either reads `Proposed`, stop
(Entry tolerance). Record `PIN=<sha>`.

EP-M1, export the pinned tree to a scratch directory so that no working-tree
edit, untracked file, or other agent's change can contaminate the sweep:

```bash
PIN=<sha>
SCRATCH=/tmp/inventory-26-1-1-$PIN
rm -rf "$SCRATCH" && mkdir -p "$SCRATCH"
git archive "$PIN" src | tar -x -C "$SCRATCH"
cd "$SCRATCH"
```

Run each forward sweep and save its output with `tee`:

```bash
T='Action|BuildEdge|BuildGraph|EdgeId|Recipe|Rule|Target|NetsukeManifest|StringOrList|DependencyOrder'
rg -n --type rust "\b($T)\s*\{" src | tee /tmp/inv-literals.out
rg -n --type rust "\bSelf\s*\{" src/ast src/ir src/ninja_gen* src/runner | tee /tmp/inv-self.out
rg -n --type rust "impl(<[^>]*>)?\s+([\w:]+\s+for\s+)?($T)\b" src | tee /tmp/inv-impls.out
rg -n --type rust -B3 "pub (struct|enum) ($T)\b" src | rg 'derive' | tee /tmp/inv-derives.out
rg -n --type rust "fn \w+(<[^>]*>)?\(\s*&mut self" src/ast src/ir src/ninja_gen src/runner \
  | tee /tmp/inv-mut-self.out
F='actions|default_targets|recipe|action_id|phony|always|explicit_outputs'
F="$F|implicit_outputs|inputs|implicit_deps|order_only_deps"
M='insert|push|extend|remove|clear|entry|get_mut|retain'
rg -n --type rust "\.($F)\s*(\.($M)\b|=[^=])" src | tee /tmp/inv-field-writes.out
rg -n --type rust "Recipe::(Command|Script|Rule)" src | tee /tmp/inv-recipe-matches.out
rg -n --type rust "LocalizedMessage|localization::message\(" src/ir src/ninja_gen* src/manifest src/runner \
  | tee /tmp/inv-diag.out
rg -n --type rust "RecipeShell" src | tee /tmp/inv-shell.out
rg -n --type rust "cli\.\w+" src/runner | tee /tmp/inv-cli-fields.out
```

Classify each hit as production, test, doctest, or Kani by reading the file's
module declaration in its parent (AX-3), then subtract the register. Expected:
the unregistered production set is empty. Cross-check every `pub` mutator with
CodeGraph (`codegraph_get_callers` on its `nodeId` after
`codegraph_reindex_workspace`) and record callers.

EV-SEEDED, in the scratch export only: apply the six production plants and one
test plant listed under OB-FWD, re-run the sweeps, and confirm the difference
lists exactly the six production plants. Then delete the scratch directory:

```bash
rm -rf "$SCRATCH"
```

EV-REVERSE, back in the repository root: for each register row, run
`git grep -n '<symbol>' "$PIN" -- '<path>'` and confirm a hit at or around the
cited line. Record "N rows checked, 0 misses".

EV-PIN:

```bash
rg -o 'blob/[0-9a-f]{40}' docs/hexagonal-hardening-inventory.md | sort -u
```

Expected: exactly one line, `blob/<PIN>`.

EV-TRACE:

```bash
comm -3 \
  <(rg -o '\b2[678]\.[0-9]+\.[0-9]+\b' docs/roadmap-hexagonal-hardening.md | sort -u) \
  <(rg -o '\b2[678]\.[0-9]+\.[0-9]+\b' docs/hexagonal-hardening-inventory.md | sort -u)
```

Expected: only tasks deliberately outside the matrix appear (27.3.x, 28.1.2
onward), and each is named in the inventory's "not changed by this programme"
paragraph or is out of scope because it does not depend on 26.1.1.

After every edit to Markdown:

```bash
make fmt 2>&1 | tee /tmp/fmt-netsuke-26-1-1-record-model-and-consumer-inventory.out
git --no-pager diff --no-ext-diff --stat
```

Gates, delegated to `scrutineer`, sequentially, after each milestone:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-26-1-1-record-model-and-consumer-inventory.out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-26-1-1-record-model-and-consumer-inventory.out
make lint 2>&1 | tee /tmp/lint-netsuke-26-1-1-record-model-and-consumer-inventory.out
make test 2>&1 | tee /tmp/test-netsuke-26-1-1-record-model-and-consumer-inventory.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-26-1-1-record-model-and-consumer-inventory.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-26-1-1-record-model-and-consumer-inventory.out
```

Expected: each exits `0`.

Commit after each milestone with an imperative subject, for example
`Record the pinned semantic-boundary register (26.1.1)`.

## Validation and acceptance

Acceptance is behaviour a reviewer can verify:

- Opening `docs/hexagonal-hardening-inventory.md` shows one pin; running the
  EV-PIN command prints one SHA.
- Choosing any task from 26.1.2 to 27.2.3 in the roadmap, a reviewer finds its
  row in the trace matrix, follows at least one register identifier to a
  `path:line` that exists at the pin, and reads at least one named
  compatibility obligation.
- Re-running the recorded sweep at the pin yields the recorded counts and no
  unregistered production site.
- The inventory lists `#652` / `#714` only under "Landed work" and as
  preservation obligations.
- Roadmap item 26.1.1 is checked and links to the inventory.

Red-Green-Refactor does not apply to a documentation-only change (D-2). Its
nearest observable substitute is the controls: EV-SEEDED and the fictitious-row
and deleted-row checks must each *fail* on the seeded scratch copy before the
register is accepted, and pass on the real one.

Quality criteria:

- Tests: `make test` passes unchanged (no test added or removed).
- Verification: OB-FWD, OB-REV, OB-TRACE, OB-652, and OB-PIN discharged with
  recorded evidence.
- Lint and type checking: `make check-fmt`, `make typecheck`, `make lint`,
  `make markdownlint`, and `make nixie` exit `0`.
- Performance and security: not applicable; no code or dependency changes.

## Idempotence and recovery

Every step is repeatable. The scratch export is recreated from `git archive`
each time and deleted afterwards. The inventory is a new file, and the other
edits are small insertions; reverting a milestone commit restores the prior
state. If `make fmt` reflows the new document unexpectedly, reread the diff and
rephrase rather than fighting the formatter. Do not use `git stash` (the stash
is shared with other worktrees); use a work-in-progress commit instead.

## Artefacts and notes

Planning-time observations at `ebcedaef`. They are hypotheses for EP-M1 to
re-verify at the pin, not results to copy.

- H1 remains. `Action` (`src/ir/graph.rs:253-270`) derives `Serialize` and
  `Clone`, has all-`pub` fields, and stores `recipe: Recipe` from `crate::ast`.
  Its only production literal is in `register_action`
  (`src/ir/from_manifest_support.rs:54-61`). `BuildEdge`
  (`src/ir/graph.rs:273-295`) has all-`pub` fields and one production literal in
  `process_targets` (`src/ir/from_manifest.rs:165-175`). `BuildGraph`
  (`src/ir/graph.rs:39-49`) derives `Default` and exposes `pub actions` and
  `pub default_targets`. `BuildGraph::replace_edge_for_output`
  (`src/ir/graph.rs:162-176`) is `pub` with test-only callers
  (`src/ir/cycle_issue322_property_tests.rs`, `src/ir/cycle_analyse_tests.rs`).
- The dependency-only sentinel is
  `Recipe::Command { command: StringOrList::Empty }`, produced by `Recipe`'s
  `Deserialize` when no recipe field is present (`src/ast/mod.rs:222`) and
  recognized by `Recipe::is_dependency_only` (`src/ast/mod.rs:180-186`). The
  backend skips it in `write_action_rules` and rejects it in
  `reject_empty_command_recipe`.
- Action identity: `ActionHasher::hash` (`src/hasher.rs:58-66`) hashes the
  canonical JSON of the whole `Action`, so any change to the stored recipe type
  changes every action hash unless its serialization is byte-identical.
- Rule delegation: a rule's recipe may itself be `Recipe::Rule`; see
  `Surprises & discoveries`. Duplicate rule names are silently last-wins in
  `process_rules` (`src/ir/from_manifest.rs:99-103`).
- H2 remains. `BuildGraph` retains no interpreter. `from_manifest_for_shell`
  (`src/ir/from_manifest.rs:64-67`) and `ninja_gen::generate_with_shell`
  (`src/ninja_gen/explicit_shell.rs:17-24`, re-exported publicly at
  `src/ninja_gen/mod.rs:37`) take the interpreter independently; `generate` and
  `generate_into` use `RecipeShell::host_default()`. Only the runner path
  (`GraphGenerationContext.recipe_shell`,
  `src/runner/graph_generation.rs:23,56-69`) threads one value through both.
- H3 remains. `execute_build` and `execute_ninja_tool`
  (`src/runner/mod.rs:192-240,275-328`) interleave generation, dyndep
  publication, temporary manifest creation, and Ninja spawn, reading `Cli`
  throughout; `NinjaBuildRequest` and `NinjaToolRequest`
  (`src/runner/process/request.rs:21-54`) are already free of `Cli`.
- H4 remains. `IrGenError` variants `RuleNotFound`, `MultipleRules`,
  `EmptyRule`, `DuplicateOutput`, `CircularDependency`, `ActionSerialisation`,
  and `InvalidCommand` pair structured fields with a `LocalizedMessage`
  (`src/ir/graph_error.rs:39-231`). `DuplicateOutput` is constructed at two
  sites with different argument shapes (`src/ir/graph.rs:66-70`, a single
  output; `src/ir/from_manifest_support.rs:156-171`, a list).
  `localize_recipe_error` classifies an error by
  `to_string().starts_with(EMPTY_COMMAND_LIST_ERROR)`
  (`src/manifest/registration.rs:15-23`). `IrGenError` and `NinjaGenError`
  derive no `miette::Diagnostic` code. Every runner error exits with status 1.
- No RFC 0027 policy or exception file exists; no H1 to H4 exception entry
  exists to retire.

## Interfaces and dependencies

No Rust interface, crate, or tool is added or changed. The deliverable is one
new Markdown document and small edits to five existing ones. Tools used, all
already present: `git`, `rg`, CodeGraph (MCP), `make`, `mdtablefix` (through
`make fmt`/`make check-fmt`), `markdownlint-cli2` and `typos` (through
`make markdownlint`), and `nixie` (through `make nixie`).

## Revision note

- Revision 1 (2026-09-27): initial draft from six-way reconnaissance at
  `ebcedaef`. Awaiting expert review and user approval.

[rfc-0026]: ../rfcs/0026-hexagonal-domain-hardening.md
[adr-035]: ../adr-035-semantic-compiler-boundaries.md
[roadmap]: ../roadmap-hexagonal-hardening.md
