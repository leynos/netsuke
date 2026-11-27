# Establish recipe and declaration characterization fixtures (roadmap 26.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Implementation must not begin until the user approves this plan. Approval also
decides `D-1`, `D-2`, `D-3`, and `D-11` in the `Decision log`; the plan states
a recommendation for each.

Branch: `26-1-2-recipe-and-declaration-characterization-fixtures`

Roadmap item: `26.1.2` in `docs/roadmap-hexagonal-hardening.md`. Planning base:
`a97bd9491ac1b418e8940f5038dd706b5b9114bb` on `origin/main`, inspected on
2026-11-25.

## Purpose / big picture

Netsuke compiles a YAML manifest (a `Netsukefile`) into a Ninja build file and
then runs Ninja. Phase 26 of the [hexagonal hardening roadmap][roadmap]
replaces the compiler's internal recipe representation. Before that happens,
the project needs an executable record of what the compiler does today with
every shape of recipe and declaration, and a separate, reviewed statement of
what it must do. Without both, a refactor cannot tell a preserved behaviour
from an accident, or a deliberate correction from a regression.

Terms used throughout:

- *Loading* parses, renders, and validates a manifest. *Lowering* turns the
  loaded manifest into the build graph. *Emission* writes the graph as Ninja
  text.
- An *entry* is one action or target. An entry *selects* a rule by naming it in
  `rule:`.
- *Delegation* is a rule whose own recipe is `rule:`, naming another rule.
- A *dependency-only* entry has no recipe, only `deps`. Ninja's built-in `phony`
  rule, which runs nothing, represents it.
- The *corpus* is the set of small manifests this task adds. The *ledger* is the
  typed Rust table that states each manifest's expected outcome and intent.
- A *seeded fault* is a deliberate, temporary defect applied to show that a test
  would notice it.
- A *plateau* is a repository state that is safe to stop at.

Two examples show why the record is needed. In the first, rule `a` delegates to
`b`:

```yaml
netsuke_version: "1.0.0"
rules:
  - name: a
    rule: b
  - name: b
    command: echo b
targets:
  - name: out
    rule: a
```

Today `netsuke graph`, `netsuke help targets`, and `netsuke check` accept it.
`netsuke generate`, `build`, and `clean` do not: a debug build panics with
`rules do not reference other rules` (exit status 101), and a release build
exits with status 1 and an error about unsafe Ninja control characters that has
nothing to do with the mistake. In the second, two rules share a name:

```yaml
netsuke_version: "1.0.0"
rules:
  - {name: r, command: echo first}
  - {name: r, command: echo second}
targets:
  - {name: out, rule: r}
```

Lowering silently uses `echo second`. One linter rule follows the *first*
declaration when it resolves a delegate, and two others follow the *last*.

After this change:

- `tests/data/recipe_characterization/` holds one manifest per case, covering
  direct and delegated rules, missing references, cycles, duplicate
  declarations, dependency-only nodes, and command and script variants.
- A typed ledger in `tests/recipe_characterization_tests/` states, for every
  case, the *observed* outcome and the *intent* separately. An intent is one of:
  `Preserve` (the observed outcome is the contract), `Replace` (a named
  roadmap task must change it to a stated outcome), or `Hold` (phases 26 and 27
  must not change it, it is not endorsed, and a named issue decides it).
- A new architectural decision record (ADR) settles the policy for rule
  selection, unsupported delegation, and declaration precedence, including the
  exact typed error that task 26.2.2 must implement.
- Contract tests keep the corpus, ledger, selection model, and ADR in agreement,
  and prove that no case depends on placeholder or shell-text semantics, which
  belong to other owners.

A reviewer observes success by running
`cargo nextest run --test recipe_characterization_tests --all-features`, seeing
it pass, then changing one line of lowering (for example, making the first
duplicate rule win) and seeing named cases fail.

No production Rust source, manifest behaviour, command-line behaviour,
diagnostic, localization catalogue, or generated Ninja output changes in this
task. The typed error is *specified* here and *implemented* by 26.2.2.

[roadmap]: ../roadmap-hexagonal-hardening.md

## Constraints

Violating any of these requires escalation, not a workaround.

- `C-1`. No file under `src/`, `locales/`, `build_l10n_audit/`, or `benches/`
  changes, and neither does `build.rs`. A characterization that changes the
  thing characterized is not a characterization.
- `C-2`. No existing test, snapshot, or fixture is weakened, renamed, or
  deleted. Two existing names mislead (`tests/data/duplicate_rules.yml`
  declares *distinct* names; `build_graph_duplicate_action_ids` in
  `tests/ir_tests.rs` exercises a `HashMap`), but renaming them is out of scope.
- `C-3`. No corpus manifest may contain placeholder or shell-dialect text: no
  `ins` or `outs` marker in any Jinja spelling, no `$`, backtick, backslash,
  heredoc operator, or unbalanced quote. A contract test enforces this, so
  26.1.2 cannot freeze a disputed semantic by accident. That a rule with
  markers yields one action per target is already pinned by
  `manifest_fixture_generates_expected_ir` and
  `command_list_entries_are_interpolated_in_order` in
  `tests/ir_from_manifest_tests.rs`.
- `C-4`. `docs/hexagonal-hardening-inventory.md` is a historical snapshot pinned
  at `8a961000` and is not edited. `docs/execplans/26-1-1-*.md` and completed
  plans are not edited.
- `C-5`. Add no roadmap task and no `Dependencies:` bullet in
  `docs/roadmap-hexagonal-hardening.md`.
  `tests/hexagonal_inventory_contract_tests.rs` pins the number of tasks that
  transitively depend on 26.1.1 at 25. Ticking the checkbox and adding a note
  under 26.1.2 are safe.
- `C-6`. No new dependency and no new `test_support` export. `rstest`,
  `rstest-bdd`, `insta`, `proptest`, `googletest`, `pretty_assertions`,
  `assert_cmd`, `cap-std`, and `camino` are already development dependencies.
- `C-7`. Every Rust file stays at or below 400 lines, opens with a `//!`
  comment, and documents every function. Tests that return `Result` use
  `anyhow::ensure!`, or `expect_that!` under `#[googletest::gtest]`, because
  Clippy denies `panic_in_result_fn`; other assertions carry a message. The new
  crate uses `cap_std`, so `dylint.toml` `excluded_crates` gains no entry.
- `C-8`. Tests never mutate or read the process environment. Corpus manifests
  load through `manifest::from_str_with_env` with an empty injected reader,
  because `del/conditional_entry` consults `env`. Command-line tests that need
  an exit status spawn the binary with
  `assert_cmd::cargo::cargo_bin_cmd!("netsuke")` and `env_clear`, following
  `create_netsuke_command` in `tests/assert_cmd_tests.rs`, which restores the
  variables a child needs on Windows. `test_support::netsuke::run_netsuke_in`
  reports only success, so it is used only where success is enough.
- `C-9`. No Kani harness, Verus proof, `assume`, `admit`, or trusted proof body
  is added. `Verification plan` records why and when that changes.
- `C-10`. Library-boundary cases use `RecipeShell::Posix` explicitly, so the
  same expectations hold on Linux and Windows, and on macOS by construction.
- `C-11`. The typed-error policy is documentation and a test-side model only.
  Implementing it here would pre-empt 26.2.2.
- `C-12`. In the new crate, only `recipe_fact` reads, and only the
  `L-NEVER-BUILT` control replaces, `Action.recipe`. The behavioural step
  module has one reader of its own. Task 26.2.1 may then retype `Action.recipe`
  by editing those three functions and no ledger row.
- `C-13`. This programme adds no v0.1.0 release-admission requirement.

## Tolerances (exception triggers)

The comparison base is the merge base with `origin/main`. ExecPlan files are
excluded from every count, including this one. Counts are net lines (additions
minus deletions) over committed and intent-to-add files:

```sh
git add -N tests docs
BASE="$(git merge-base origin/main HEAD)"
count() { git diff --numstat "$BASE" -- "$@" | awk '{n++; s+=$1-$2} END {print n+0, s+0}'; }
count tests test_support ':!tests/data' ':!tests/snapshots' ':!*.proptest-regressions'   # code
count tests/data tests/snapshots '*.proptest-regressions'                                # fixtures
count docs CHANGELOG.md ':!docs/execplans'                                               # documentation
count Cargo.toml Cargo.lock clippy.toml dylint.toml typos.toml typos.local.toml \
  .cargo .config .codescene .github Makefile                                             # configuration
```

Code is Rust source and `.feature` files. Fixtures are manifests, snapshots,
and regression seeds. Documentation is prose other than ExecPlans.
Configuration is declarative settings. Each category is enforced alone; slack
in one does not offset another.

| Category      | Files | Net lines | Basis                                                                      |
| ------------- | ----: | --------: | -------------------------------------------------------------------------- |
| Code          | 28    | 3,800     | Estimate 23 files and 2,800 lines, plus about a third.                     |
| Fixtures      | 110   | 1,200     | 88 manifests averaging 9 lines, 6 snapshots, and contingency for review.   |
| Documentation | 13    | 850       | One ADR of about 350 lines and nine amended documents.                     |
| Configuration | 4     | 40        | `typos.toml` regeneration or a CodeScene rule, if either proves necessary. |

Stop and escalate if:

- any limit above is exceeded, or any constraint would be violated;
- Interface: any public Rust, command-line, manifest, or diagnostic interface
  would change;
- Dependencies: a new dependency or `test_support` export is needed;
- Architecture: any change to `src/` appears necessary, or the policy in `D-1`
  cannot be expressed as one typed lowering error;
- Ambiguity: a case admits two intents that would lead 26.2 to different
  implementations and no policy below decides it;
- a planned case behaves differently from `Artefacts and notes` and no commit
  landed since the planning base explains it;
- Iterations: the same gate fails three times for the same cause; or
- Time: a milestone takes more than one working day of active effort, excluding
  time queued for compile admission.

`Context and orientation` records the survey and the historical evidence behind
these numbers.

## Risks

- Risk: PR `#1183` (issue `#976`) bounds `IrGenError::MultipleRules` to eight
  names and edits the same users' guide bullet, `docs/netsuke-design.md`,
  `docs/developers-guide.md`, `docs/repository-layout.md`,
  `tests/bdd/steps/ir.rs`, and `tests/features/ir.feature`. Severity: low.
  Likelihood: high. Mitigation: the corpus selectors name two rules, for which
  `#1183` states the message is unchanged; this plan adds its own step module
  and feature file; `EP-M1` builds on `#1183`'s wording if it has merged.
- Risk: PR `#1166` (issue `#1144`) changes Ninja emission for executable phony
  edges and for `always`, and edits the roadmap next to the 26.1.2 entry.
  Severity: medium. Likelihood: high. Mitigation: ledger facts describe the
  intermediate representation (IR), not emitted build lines; the only emission
  fact is success or failure. Rebase before `EP-M2` if it has merged.
- Risk: a characterization suite that records whatever happens cannot fail
  usefully. Severity: high. Likelihood: medium. Mitigation: expectations are
  typed and handwritten from the verified table here rather than
  snapshot-accepted, an independent model must agree with every row, and seeded
  faults must be rejected by named cases.
- Risk: tests that pin a debug-only panic make it look intended. Severity:
  medium. Likelihood: medium. Mitigation: those rows are `Replace`, and
  contracts make a `Replace` row fail once its owning task is ticked.
- Risk: the suite is rewritten by the change it guards. Severity: high.
  Likelihood: medium. Mitigation: `C-12`, and the retirement rule in
  `Interfaces and dependencies`: a `Replace` row may change only to `Preserve`
  of its recorded replacement.
- Risk: the release-profile half of delegation is exercised by no CI lane,
  because every test lane keeps debug assertions enabled. Severity: medium.
  Likelihood: certain. Mitigation: `EP-M4` records a run with debug assertions
  disabled, and 26.2.2 removes the split.
- Risk: no test in this repository uses `#[files]` from `rstest`. Severity: low.
  Likelihood: low. Mitigation: prove it first in `EP-M2` and on the Windows
  lane of the first push; the fallback is one test that iterates the ledger and
  reports every failing row.
- Risk: the next free ADR number is claimed first. Severity: low. Likelihood:
  high. Mitigation: re-sweep every remote branch at `EP-M1`.
- Risk: compile admission on this host is heavily queued. Severity: low.
  Likelihood: high. Mitigation: start each gate once and wait.

## Progress

- [x] (2026-11-25 14:10Z) Reconnaissance by five read-only Wyvern agents, live
  probes of the planning base, prior-art research, and initial draft.
- [x] (2026-11-25 14:35Z) Community-of-experts design review (four reviewers,
  all returning "revise") applied, every corpus row re-probed, and a second
  pass by three of the reviewers applied.
- [ ] Plan approved by the user, deciding `D-1`, `D-2`, `D-3`, and `D-11`.
- [ ] `EP-M0` re-baseline against `origin/main`.
- [ ] `EP-M1` policy ADR, follow-up issues, and corrected live documentation.
- [ ] `EP-M2` library-boundary corpus, ledger, and contracts.
- [ ] `EP-M3` command-line, linter, and behavioural coverage.
- [ ] `EP-M4` selection properties, seeded faults, and profile evidence.
- [ ] `EP-M5` documentation closure and roadmap completion.

## Surprises & discoveries

Found during planning. `Artefacts and notes` records the evidence.

- Observation: delegation has never emitted. The first Ninja generator,
  `fb6dda3a5` (2025-07-31), already contained
  `unreachable!("rules do not reference other rules")`, and the published
  `0.1.0-beta4` release binary fails with status 1. Impact: rejecting selected
  delegation breaks no `build`, `generate`, or `clean` that ever succeeded.
- Observation: rejecting it during lowering does change `graph`, `help targets`,
  and `check`, which succeed today. Impact: a deliberate, bounded exception to
  compatibility obligation `CO-1`, needing approval.
- Observation: `check` uses query loading, which keeps an entry whose `when`
  cannot be evaluated, and then lowers it. (`help targets` also keeps it but
  discards it before lowering.) A delegating selection behind a false `when`
  therefore builds today and would fail `check` after 26.2.2. The same manifest
  with a *missing* rule already behaves that way: `check` exits 1 and
  `generate` exits 0. Impact: consistent with existing behaviour, but it must
  be named in the `CO-1` exception.
- Observation: an unselected delegating rule is harmless today. Impact:
  rejecting every delegating rule at load would break working builds.
- Observation: [RFC 0005][rfc-0005] calls the backend path "the supported
  `Recipe::Rule` rejection path" and requires release-profile tests of the
  panic and of `UnsafeNinjaValue`. Impact: `EP-M4` evidences both outcomes
  manually; RFC 0005 keeps ownership of the release-profile lane, and 26.2.2
  must amend it when the path becomes unreachable from a manifest.
- Observation: three linter tests depend on lowering accepting delegation, named
  in the handoff below. Impact: 26.2.2 must rewrite them.
- Observation: last-wins is not wholly unpinned, as inventory finding `OBS-2`
  might suggest: `same_named_rules_reorder_emission` in
  `src/ninja_gen/determinism/property_tests/declaration.rs` shows that order
  matters, though not which declaration wins.
- Observation: `undeclared-target-input` resolves a delegate with `find`
  (first-wins); `unused-rule` and `directory-dep-not-order-only` collect into a
  `BTreeMap` (last-wins); graph JSON publishes every same-named rule while
  nodes refer by name only. Impact: extends `OBS-18`. `check` lowers before it
  lints, so the first-wins path becomes unreachable when 26.2.2 lands.
- Observation: rule *names* are never rendered. A rule named `{{ which }}`
  cannot be selected by its rendered name. A rule named with an empty string is
  an ordinary rule, selected by `rule: ""`.
- Observation: `command: ""` and `command: "  "` are accepted and emit an empty
  Ninja `command =` binding, which Ninja rejects. Impact: `Hold`, issue 3.
- Observation: output collision uses path-component equality. `x/./out`,
  `x//out`, and `x/out/` collide with `x/out`; `./out` does not collide with
  `out`, nor `x/z/../out` with `x/out`. Ninja canonicalizes them all and
  rejects the resulting file. Impact: `Hold`, issue 2.
- Observation: an unknown `defaults` entry is lowered and emitted, whereupon
  Ninja fails; `help targets` rejects the same manifest. Impact: `Hold`, issue
  4.
- Observation: `docs/netsuke-design.md` §5.5 says two targets share an action
  only when command text *and file sets* match. Two targets whose command has
  no marker share one action whatever their outputs. Impact: correct the prose,
  not the hash (`CO-3`).
- Observation: the roadmap names `#699` as an open decision owner. It merged on
  2026-09-23 as ADR-0027, whose marker set ADR-0034 amends. Impact: `D-12`.

[rfc-0005]: ../rfcs/rfc-0005-harden-release-integrity-and-admission.md

## Decision log

`D-1`, `D-2`, `D-3`, and `D-11` need the approver's agreement.

- `D-1`. Decision (recommended): delegation is unsupported. When an entry
  selects a rule whose own recipe is `rule:`, lowering rejects it with a new
  typed error, `IrGenError::RuleDelegationUnsupported`. Unselected delegating
  rules stay accepted. Options:

  - Option A, reject on selection during lowering (recommended). It never looks
    inside the delegate list, so missing, self-referential, cyclic, plural,
    empty, and blank delegates share one answer and no traversal exists to
    terminate.
  - Option B, resolve delegation with cycle detection. The case for it is real:
    the users' guide lists "`rule`: the name of another rule to use" under
    "Rules must provide exactly one executable recipe", the `Rule` type's
    documentation says a rule may "delegate to another named rule", and three
    linter modules follow delegation. A one-name chain with a visited set would
    be small. It still needs decisions nobody has made (which description is
    authoritative, what a list means, how isolation combines) and creates
    termination and acyclicity obligations.
  - Option C, reject every delegating rule at load. It breaks manifests that
    build today because the delegating rule is unselected.
  - Option D, keep rejecting in the backend with a better error. It leaves
    unresolved states in successful lowering, contrary to ADR-0035.

  RFC 0026 permits A or B. A wins because delegation has never built, no demand
  is recorded, Ninja rules cannot invoke other rules, and A can later be
  relaxed to B without breaking anything. The contrary documentation is treated
  as a defect and corrected.

  Date/Author: 2026-11-25, planning agent; awaiting approver.
- `D-2`. Decision (recommended): settle duplicate rule names as last-wins, whole
  declaration. Phases 26 and 27 preserve it, and changing it needs an ADR
  amendment. ADR-0035 and RFC 0026 require 26.1.2 to *settle* precedence, and
  RFC 0026 forbids changing it during the representation refactor. Contrary
  evidence is recorded rather than hidden: `Rule::name` is documented as a
  "Unique identifier", Ninja rejects a duplicate rule, and `just` rejects
  duplicate recipes by default. RFC 0002 rejects duplicates only under
  composition, and its §12 keeps manifests without `includes` unchanged. The
  users' guide will advise unique names.

  Date/Author: 2026-11-25, revised after review; awaiting approver.
- `D-3`. Decision (recommended): raise four GitHub issues at the start of
  `EP-M1`: (1) whether an uncomposed manifest should reject duplicate rule
  names; (2) lexical aliases of output paths; (3) empty and blank scalar
  commands; (4) unknown `defaults` entries. The ledger cites real issue numbers
  from its first commit. Creating issues is outward-facing, so approval of this
  plan is the authorization.

  Date/Author: 2026-11-25, revised after review; awaiting approver.
- `D-4`. Decision: the ledger is typed Rust and each manifest is a separate YAML
  file. An untyped ledger cannot be checked exhaustively against `IrGenError`; a
  `match` without a wildcard makes a new variant a compile error in the suite,
  which is the moment 26.2.2 must revisit it. Separate manifests are reusable
  by feature files and command-line tests.

  Date/Author: 2026-11-25, planning agent.
- `D-5`. Decision: one `#[files]`-driven test executes the corpus, rather than
  one `#[case]` per row. Cases then equal manifests by construction; a listed
  row without a `#[case]` could otherwise exist and never run. A runtime
  directory scan covers files added without recompilation.

  Date/Author: 2026-11-25, from review.
- `D-6`. Decision: no Verus proof and no new Kani harness in this task. See
  `Formal verification decision`.

  Date/Author: 2026-11-25, planning agent; revised after review.
- `D-7`. Decision: the suite is an integration crate, not a `src/` test module.
  It characterizes the public library and binary boundaries, leaves `src/`
  untouched (`C-1`), and stays out of the Kani proof scope.

  Date/Author: 2026-11-25, planning agent.
- `D-8`. Decision: seeded-fault evidence is recorded in this plan, not committed
  under `docs/verification/mutations/`. A committed patch must apply to
  `src/ir/from_manifest/`, which 26.2 rewrites; it would rot before it was
  useful.

  Date/Author: 2026-11-25, planning agent.
- `D-9`. Decision: ADR-0035's acceptance satisfies the phase entry condition.
  RFC 0026's header still reads `Proposed`, as it did when 26.1.1 completed
  under that plan's `D-13`. This plan does not edit the RFC's status.

  Date/Author: 2026-11-25, planning agent.
- `D-10`. Decision: `ortho_config`, localized help, and layered configuration
  are unaffected. This task adds no option, key, environment variable, or help
  text. 26.2.2 adds one Fluent diagnostic key, not configuration.

  Date/Author: 2026-11-25, planning agent.
- `D-11`. Decision (recommended): approval of this plan accepts the `POL-*` text
  in `EP-M1` as the reviewed policy. The ADR reproduces it verbatim and its
  status cites the approver, date, and link. Any substantive difference sets
  this plan to `BLOCKED`. An approval that leaves `D-1` or `D-2` unanswered
  leaves the ADR `Proposed` and stops work after `EP-M1`; rejecting either sets
  this plan to `BLOCKED`. An approval without `D-3` raises no issue and blocks
  `EP-M2`, whose `Hold` rows need issue numbers.

  Date/Author: 2026-11-25, from review; awaiting approver.
- `D-12`. Decision: placeholder semantics stay with ADR-0027 as amended by
  ADR-0034, and any change amends those records. RFC 0036 (`Proposed`, epic
  `#1104`) covers shell-text admission and is not their owner. Reading the
  roadmap's "defer to `#699`" this way keeps its meaning now that `#699` has
  merged. The acceptance bullet is not rewritten; a dated note is added.

  Date/Author: 2026-11-25, from review.
- `D-13`. Decision: drop cases already pinned at the same boundary unless they
  are a control for a divergent case. Review pulled both ways: one reviewer
  would cut to about 58 rows, another listed 25 missing cells. Cut: mutual
  exclusion pairs, isolation rejections, empty command lists, macro and mapping
  key duplicates, and repeated defaults, all pinned elsewhere or outside the
  task. Added: serial (dyndep) cases, error precedence around delegation, blank
  and templated names, and conditional entries. The result is 88 rows of about
  nine lines, which the `#[files]` runner makes cheap.

  Date/Author: 2026-11-25, from review.

## Outcomes & retrospective

Not started. Complete this section at each milestone and before setting the
status to `COMPLETE`.

## Context and orientation

Assume no prior knowledge of this repository. Relocate symbols by name; line
numbers, where given, are at the planning base.

### The pipeline

1. `src/ast/` defines the parsed manifest. `Recipe` has three variants:
   `Command { isolation, command }`, `Script { script }`, and `Rule { rule }`.
   An entry with none of the three becomes
   `Command { Shared, StringOrList::Empty }`, the dependency-only marker.
   `NetsukeManifest::validate_recipes` accepts the marker only on an entry with
   non-blank `deps`, and never on a rule.
2. `src/manifest/` renders Jinja in place, including `rule:` selectors but not
   rule names, and exposes `manifest::from_str`. *Build* loading drops entries
   whose `when` is false. *Query* loading, used by `check` and `help targets`,
   keeps entries whose `when` it cannot evaluate; `help targets` then discards
   them before lowering and `check` does not.
3. `src/ir/from_manifest/mod.rs` lowers. `process_rules` inserts every rule into
   a map by name, so the last declaration wins. `process_targets` visits
   `actions`, then `targets`; for each entry it checks duplicate outputs,
   resolves the selector through `resolve_rule` in `support/mod.rs`, clones the
   selected recipe, and calls `register_action`, which interpolates and hashes.
   `resolve_recipe` passes `Recipe::Rule` through unresolved. `detect_cycles`
   runs once at the end. Entry points are `BuildGraph::from_manifest` and the
   `#[doc(hidden)]` `BuildGraph::from_manifest_for_shell`.
4. `src/hasher.rs` hashes the whole `Action` as canonical JSON to name it.
5. `src/ninja_gen/` emits. `NamedAction::reject_rule_recipe` panics when debug
   assertions are enabled and otherwise returns
   `NinjaGenError::UnsafeNinjaValue`. The command line always emits a *bundle*
   (the build file plus dyndep sidecars for serial dependencies). The suite
   uses the public `ninja_gen::generate_bundle_with_interpreter` with
   `RecipeShell::Posix`, which ignores the binding; bind
   `C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe` when
   `cfg!(windows)` and `/bin/sh` otherwise, because the constructor requires a
   host-absolute path. `generate_with_shell` would reject serial graphs before
   reaching delegation.
6. `src/runner/` drives commands. `graph`, `help targets`, and `check` lower but
   do not emit. `generate`, `build`, and `clean` emit before starting Ninja.
7. `src/lint/`, compiled only with the `lint` feature, reads the manifest and
   graph and follows delegation itself.

### Existing evidence, and the gaps this plan fills

- Direct rules: `tests/ir_from_manifest_tests.rs` and its `progress_labels.rs`.
  Gaps: one-element, repeated, and blank selectors; selection from an action;
  isolation through a rule; sharing one action between targets.
- Delegated rules: linter tests in `src/lint/rules/graph/tests.rs` and
  `src/lint/rules/hygiene/tests.rs`, a rendering test, and a graph-JSON
  property. Gaps: everything at load, lowering, emission, and the command line.
- Missing references: `manifest_error_cases`, `missing_rule_fails`, and a
  command-line snapshot for target selectors. Gaps: action selectors, error
  order, unrendered rule names, and `defaults`.
- Target cycles: `tests/cycle_participation_e2e_tests.rs`,
  `tests/features/ir.feature`, Kani, and `proptest`. Gaps: path orientation at
  the manifest boundary, self-dependency, dependency-only cycles, and
  precedence against selection errors.
- Duplicate outputs: `duplicate_outputs*.yml` cases for targets. Gaps: actions,
  `foreach`, path aliases, and precedence. Duplicate rule names have only the
  order-sensitivity witness noted above.
- Dependency-only nodes: `tests/ast_tests/dependency_only.rs` and
  `dependency_only_entries_lower_to_deduplicated_phony_actions`. Gaps:
  `sources` or `order_only_deps` alone, several outputs, `always`, null
  recipes, and rules without a recipe that nothing selects.
- Command and script variants: `tests/ast_tests/recipe.rs` and several others.
  Gaps: empty and blank commands and scripts, blank list entries, and nulls.

### Test frameworks and conventions

- `cargo nextest` runs every top-level `tests/*.rs` file as a crate, one process
  per test. A child module needs an explicit `#[path]`, and `mod.rs` is
  required for nested directories (`self_named_module_files` is denied).
- `rstest` 0.26 with `googletest` 0.14: put `#[googletest::gtest]` above
  `#[rstest]`, as in `tests/terminal_rendering/emoji_pty.rs`.
- `pretty_assertions::assert_eq!` compares whole `Observed` values in helpers
  that do not return `Result`; `googletest` matchers state predicates over
  command output and collections.
- `rstest-bdd` 0.6: `tests/bdd_tests.rs` discovers `tests/features/*.feature`;
  step modules are listed in `tests/bdd/steps/mod.rs`; scenario state is
  `TestWorld`, which retains the `BuildGraph`.
- `insta`: snapshots live under `tests/snapshots/<suite>/` with an absolute path
  and `set_prepend_module_to_snapshot(false)`; see
  `tests/manifest_load_options/fixtures/settings.rs`.
- `proptest`: `make proptest` selects a test only if a module segment ends in
  `proptests` or `property_tests`. Pin regression files with
  `FileFailurePersistence::Direct`.
- `tests/manifest_load_options/corpus_fidelity.rs` scans only the top level of
  `tests/data`, so a new subdirectory does not join that corpus.
- Windows checkouts may use CRLF, because the repository has no
  `.gitattributes`. Corpus scripts are single-line scalars, text is read with
  `.lines()`, and row identifiers come from path components.

### Documentation and skills to consult

- `docs/netsuke-design.md` §§1.2, 3, 5.3, and 5.5; ADR-0035; RFC 0026; RFC 0005;
  `docs/hexagonal-hardening-inventory.md` (`OBS-1`, `OBS-2`, `OBS-12`, `OBS-17`,
  `OBS-18`, and `CO-1` to `CO-14`);
  `docs/formal-verification-methods-in-netsuke.md`; `docs/developers-guide.md`;
  `docs/documentation-style-guide.md`.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` for test shape.
- `docs/ortho-config-users-guide.md` governs configuration work; `D-10` records
  that nothing here touches configuration.
- Skills: `rust-router`, then `rust-unit-testing`, `proptest`, and
  `rust-errors`; `hexagonal-architecture` for boundary placement;
  `arch-decision-records` for the ADR; `codegraph-mcp` for callers; `verus`,
  `kani`, and `rust-verification` for the 26.2.2 handoff;
  `addressing-whitaker-findings` for lint remediation; `pr-babysitting` for
  delivery.

### Budget survey and history

The estimate counts 20 Rust files in the new crate, one new step module, one
edited step index, and one feature file; 88 manifests; and ten documents.
Comparable plans underestimated. `4-3-1` was widened from 34 files and 2,600
lines to 56 and 6,000, then breached the wider cap at 88 and 8,785, because its
inventory missed shared strategy modules. `4-4-1` finished at 18 paths and
1,774 lines against 1,600 after review fixes. `26-1-1` grew from ten files to
about 25 when the 400-line cap, spelling regeneration, and CodeScene exemptions
forced splits. The causes were missed shared support and repository constraints
rather than approved growth. The first draft of this plan repeated the mistake,
counting 72 manifests where its own table had 85; review caught it. This
version names every file, separates fixtures from code, and allows about a
third on code.

## Conformance basis

No Terms of Reference exists for this programme.

| Upstream artefact                       | Revision inspected      | Elements used                                                                       |
| --------------------------------------- | ----------------------- | ----------------------------------------------------------------------------------- |
| `docs/roadmap-hexagonal-hardening.md`   | `a97bd9491`             | 26.1.2 coverage, acceptance, and dependency on 26.1.1 (complete)                    |
| RFC 0026                                | `a97bd9491`, `Proposed` | Delegation choice; settle precedence; no silent change; placeholders preserved      |
| ADR-0035                                | `a97bd9491`, `Accepted` | Gate: 26.1.2 settles rule and declaration policy                                    |
| RFC 0005, release test set              | `a97bd9491`             | Profile-specific tests of the delegation rejection path                             |
| ADR-0027, as amended by ADR-0034        | `a97bd9491`, `Accepted` | Settled placeholder contract, excluded by `C-3`                                     |
| `docs/hexagonal-hardening-inventory.md` | pin `8a961000`          | `A1/*`, `A2/*`, `H1`, `OBS-1`, `OBS-2`, `OBS-12`, `OBS-18`, `CO-1`, `CO-6`, `CO-12` |
| `docs/netsuke-design.md`                | `a97bd9491`             | §§1.2 stage 5, 3.4, 5.3, 5.5                                                        |
| RFC 0002 §12 and roadmap 16.2.1         | `a97bd9491`             | Manifests without `includes` keep their behaviour                                   |

As the inventory requires, `git log 8a961000..a97bd9491` over `src/ir`,
`src/ast`, `src/manifest`, `src/ninja_gen`, and `src/hasher.rs` shows 22
commits since the pin. They touch shell-text interpolation, interpreter
binding, console pools, progress labels, diagnostics, and the manifest loader.
`process_rules`, `resolve_rule`, `resolve_recipe`, and `reject_rule_recipe`
keep the behaviour the inventory records; `process_rules` is now at lines 95 to
99.

Requirements and gaps:

- `REQ-COVER`: the corpus covers every family the roadmap names.
- `REQ-DISTINGUISH`: each case records observed behaviour and intent separately.
- `REQ-POLICY`: unsupported delegation has a reviewed typed-error policy, and
  declaration precedence is settled.
- `REQ-DEFER`: disputed placeholder semantics are excluded, not changed.
- `GAP-DELEGATION`: no test exercises delegation beyond the linter (`OBS-1`).
- `GAP-PRECEDENCE`: no test states which duplicate wins (`OBS-2`), or in what
  order errors are reported.
- `GAP-DOCS`: live documentation implies delegation works.

```plaintext
RM-26.1.2 cover       -> REQ-COVER       -> EP-M2, EP-M3 -> characterized_case, every_family_is_covered
RM-26.1.2 distinguish -> REQ-DISTINGUISH -> EP-M2        -> replacement_rows_state_the_policy_outcome
RM-26.1.2 policy      -> REQ-POLICY      -> EP-M1, EP-M2 -> policy_headings_match_cited_policies
RM-26.1.2 defer       -> REQ-DEFER       -> EP-M2        -> corpus_excludes_placeholder_and_shell_text
OBS-1                 -> GAP-DELEGATION  -> EP-M2, EP-M3 -> delegation_is_rejected_only_by_emitting_commands
OBS-2                 -> GAP-PRECEDENCE  -> EP-M2, EP-M4 -> lowering_agrees_with_the_selection_model
RFC 0005 test set     -> GAP-DELEGATION  -> EP-M4 partial -> characterized_case, both outcomes
ADR-0035 gate         -> REQ-POLICY      -> EP-M1        -> accepted ADR, linked from design §5.3
```

`CO-1` is preserved by every row except `Replace` rows, whose exception the ADR
records. `CO-6` and `CO-12` are untouched because no key or quickstart changes.

## Verification plan

### Abstract model

A loaded manifest value is reduced to what selection depends on.

```plaintext
Rules      R = [(name_i, body_i)]     body  ::= Exec(id) | Deleg([name])
Entries    E = actions ++ targets     entry ::= (outputs, choice, deps)
Choice     c ::= DependencyOnly | Inline(id) | Names([name])
win(R, n)  = body_i for the greatest i with name_i = n; absent if there is none
select(R, Names(ns)) =
    EmptyRule                if ns = []
    MultipleRules(sort ns)   if |ns| >= 2
    RuleNotFound(n)          if ns = [n] and win(R, n) is absent
    win(R, n)                otherwise
check(k)   = DuplicateOutput(sorted duplicates) if an output of E[k] repeats within E[k]
             or equals an output of E[j], j < k;
             else the error of select(R, choice(E[k])), if any
```

`deps` is the union of `sources`, `deps`, and `order_only_deps`. Names compare
exactly, without trimming. Outputs compare by path components.

Let `e` be the least `k` with `check(k)` an error, and `d` the least `k` for
which `check(k)` succeeds and selects `Deleg`. Either may be absent.

```plaintext
observed(M) = check(e)                                   if e exists
              CircularDependency                         if the graph over E is cyclic
              Lowered, with unresolved bodies kept       otherwise
intended(M) = RuleDelegationUnsupported(name(E[d]), n)   if d exists and (e is absent or d < e)
              observed(M)                                otherwise
```

Here `name(E[k])` is the first output of the entry, or the empty string if it
has none, and `n` is the selected rule's name, not a delegate's.

Preconditions: the manifest loaded, so every rule has a recipe and every
dependency-only entry has non-blank `deps`; and every selected `Exec` and
`Inline` body interpolates. Lowering otherwise reports `InvalidCommand` or
`AmbiguousSubstitutionBoundary` for an entry after its selection succeeds and
before the next entry. `C-3` keeps such text out of the corpus, so those errors
are outside this model. A rule with no recipe fails the whole manifest at load,
selected or not.

Representation relations:

- `a_in` maps a model manifest to YAML, injective on names and order-preserving.
  The test compares `manifest::from_str(a_in(m))` with `m` field by field, so a
  generator that normalizes its input is detected.
- `a_out` maps a graph to ordered `(outputs, body, description)` triples.

The model shares no code or import with `netsuke::ir`. It finds the winner by
reverse linear search, duplicates by pairwise comparison, and cycles by
repeatedly removing entries without unresolved prerequisites, where production
uses a map, an index, and a depth-first search.

### Invariants and lemmas

These are claims to establish, stated before any resolver is designed. They
pressure-tested the policy and are why `D-1` chose Option A. Each can be false
of an implementation.

- `INV-LAST-WINS`: for manifests that load before and after the edit, replacing
  the body and description of a shadowed rule with any other executable or
  delegating body changes neither the lowered graph (actions, edges, defaults)
  nor the error.
- `INV-UNSELECTED`: under the same proviso, adding, removing, or permuting rules
  whose names no entry selects changes neither, including when they delegate,
  dangle, or form a cycle.
- `INV-ORDER`: the error is that of the earliest failing entry in
  actions-then-targets order; within an entry `DuplicateOutput` precedes
  selection; `CircularDependency` is reported only if no entry fails.
- `INV-TOTAL`: `intended` is total by structural recursion over `E`; it performs
  no traversal of rules. `observed` is exercised within the generated domain
  only. Unbounded depth of the recursive cycle detector is out of scope.
- `L-REFINE`: a separately written operational `intended_fold`, which visits
  entries once and stops at the first failure or selected `Deleg`, equals the
  closed-form `intended`. Hence `intended(M)` differs from `observed(M)` if and
  only if `d` exists and precedes `e`. This is the correspondence between the
  specification and the shape 26.2.2 will implement.
- `L-ERASE` (about production): replacing every selected delegating body with a
  fresh command preserves success or failure, the error payload, and the edge
  shape of today's lowering. It shows that today's lowering never looks past
  the selected rule. After 26.2.2 the compatibility contract is production
  equal to `intended` over the generated domain, plus unchanged `Preserve` rows;
  `L-ERASE` then retires.
- `L-NO-DEREFERENCE`: `intended` depends on a selected delegating rule only
  through its being `Deleg`. Replacing the delegate list with any other list
  leaves `intended` unchanged. `observed` does not have this property, because
  it exposes the names.
- `L-NEVER-BUILT`: for one loaded manifest value, if `intended` differs from
  `observed`, then either `observed` lowers and emission fails, or `observed`
  is already a lowering error that `intended` pre-empts with a different one.
  No Ninja text is produced from such a value today. Build and query loading
  yield different values, so this does not say that `check` is unaffected.

### Axioms (trusted, not verified here)

- `AXIOM-YAML`: `serde-saphyr` maps `null` to an absent optional field.
  Boundary: `RawRecipe`. The corpus exercises it for each recipe key.
- `AXIOM-PATH-EQ`: `camino` path equality compares components, so interior `.`,
  repeated separators, and a trailing separator are ignored, and leading `./`
  and `..` are not. Boundary: `duplicate_output_error`. The corpus exercises
  all five.
- `AXIOM-NINJA`: Ninja rejects a rule with an empty command and canonicalizes
  paths before its own duplicate check (`ParseRule` and `ParseEdge`, Ninja
  1.12.1). Boundary: emitted text. Observed with Ninja 1.11.1 in planning; not
  re-tested here.
- `AXIOM-PROFILE`: `cfg!(debug_assertions)` in an integration test agrees with
  the library and binary Cargo built for that test.
- `AXIOM-UNWIND`: the test profile unwinds, so `catch_unwind` observes the
  panic. `tests/build_tools_cargo_config_tests.rs` records that Cranelift would
  break this; the build standard excludes Cranelift.

### Methods per obligation

- Obligation: `INV-ORDER`, and `observed` against production.
  - Method: property test `lowering_agrees_with_the_selection_model`, plus
    corpus rows.
  - Rationale: the claim ranges over declaration sequences that examples cannot
    exhaust; lowering is cheap and pure.
  - Domain: 1 to 5 rules over 3 names, every body textually unique; 1 to 4
    entries split between actions and targets; choices drawn from
    dependency-only, inline, and names with arity weighted 1:12, 0:1, 2:1, 3:1;
    one output with weight 4 or two with weight 1, over 6 names; prerequisites
    over those outputs through all three fields. A separate arm *constructs*
    each `INV-ORDER` intersection instead of filtering for it. Manifests pass
    through `a_in` and `manifest::from_str`.
  - Artefact: `selection_property_tests.rs`, with the model in
    `selection_model.rs`.
  - Evidence: `make proptest`, 1,024 cases. Red: a first-wins model fails under
    the persisted seed and shrinks to two same-named rules. Discharge: passes
    with the corrected model.
  - Non-vacuity: `generator_reaches_every_class` runs the model alone with a
    fixed `RngSeed` and 1,024 cases and requires at least ten of each outcome
    (lowered with and without a selected delegation, each selection error,
    duplicate output, cycle) and of each constructed intersection; the design
    target is 30. Planning simulation showed that uniform selector arity reached
    only about 11 accepted cases, which is why arity is weighted. `SF-1` to
    `SF-4` must be rejected.
- Obligation: `INV-LAST-WINS`, `INV-UNSELECTED`, `L-ERASE`.
  - Method: metamorphic properties in `relation_property_tests.rs` comparing
    `(actions, edges, default_targets)` or the typed error of two lowered
    manifests. `Action` and `BuildEdge` implement `PartialEq`. `L-ERASE`
    compares edges with `action_id` erased and `default_targets`, not `actions`,
    because erasure changes hashes and action sharing.
  - Rationale: each is a relation between two manifests; agreement with one
    model cannot establish it, and a description or action-sharing leak would
    pass.
  - Evidence: `make proptest`. Red: the two invariants are first written without
    their loading proviso and must fail on a rule that no longer loads;
    `L-ERASE` has no proviso, so its red is `SF-8`.
  - Non-vacuity: each property counts cases where the edit was non-trivial and
    fails below 64. `SF-8` and `SF-9` must be rejected.
- Obligation: `L-REFINE`, `L-NO-DEREFERENCE`.
  - Method: properties over the model alone, varying delegate lists; plus a
    ledger contract that every `Replace` row's outcome equals `intended` of that
    manifest and every other lowered row has `intended` equal to `observed`.
  - Rationale: they constrain the specification 26.2.2 will implement. They say
    nothing about production and are not presented as if they did.
  - Non-vacuity: cases with `d` before `e`, after `e`, and with either absent
    are each counted. A model variant that inspects delegates must fail.
- Obligation: `L-NEVER-BUILT`.
  - Method: every row observed as `BackendRejectsDelegation` asserts the exact
    emission failure under the current profile, and every other `Replace` row
    asserts its lowering error; `EP-M4` repeats with debug assertions disabled.
  - Non-vacuity: in debug builds the panic payload must equal
    `rules do not reference other rules`, and any other panic is resumed,
    because `assert_shell_command` also panics. In both profiles the graph must
    contain a `Recipe::Rule` action, and emission must *succeed* on a clone with
    exactly those recipes replaced, because `UnsafeNinjaValue` has other causes.
    `SF-6` must be rejected.
- Obligation: ledger integrity (`REQ-COVER`, `REQ-DISTINGUISH`, `REQ-POLICY`,
  `REQ-DEFER`).
  - Method: contract tests over finite sets, each a pure validator with a paired
    negative case rejected for its stated reason:
    - `corpus_files_match_ledger_rows`: files equal rows.
    - `every_family_is_covered`: each family is non-empty; `del/one_hop`,
      `del/from_action`, and `del/serial_deps` have controls
      `sel/direct_command`, `sel/from_action`, and `sel/serial_deps`, all
      `Emitted`.
    - `policy_headings_match_cited_policies`: ADR `### POL-` headings, the
      `Policy` enumeration, and the policies cited by rows are the same set.
    - `replacement_rows_state_the_policy_outcome`: every `Replace` has `by`
      unequal to `observed` and equal to `intended(M)`; every other row that
      loads has `intended(M)` equal to `observed(M)`; and every row's recorded
      lowering outcome equals the model's `observed(M)`, cycle paths excepted.
    - `open_intents_have_open_owners`: a `Replace` owner is an existing,
      *unchecked* task in its named roadmap file; a `Hold` has an issue number,
      a non-empty `until`, and a `review` task that is still unchecked.
    - `corpus_excludes_placeholder_and_shell_text`: `C-3`.
  - Non-vacuity: the negative cases are a missing file, a missing row, an
    uncited policy, a wrong replacement, a replacement equal to its observation,
    a checked owner, an absent owner, an empty `until`, and corpus text
    containing `{{ins}}`, `{{- outs }}`, and `$`.

### Seeded faults

Apply each on a committed tree, run the focused command, record the failing
cases in `Artefacts and notes`, and restore with `git checkout -- <file>`.
Never leave one applied across a gate.

| ID      | Fault                                                                                         | Must be rejected by                                                                        |
| ------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `SF-1`  | `process_rules` keeps the first declaration                                                   | `dup/rule_last_wins`, the model property, `INV-LAST-WINS`                                  |
| `SF-2`  | `process_targets` visits targets before actions                                               | `miss/action_before_target`, the model property                                            |
| `SF-3`  | The duplicate check runs after selector resolution                                            | `dup/output_before_selection`, the model property                                          |
| `SF-3b` | The duplicate check is deleted (`insert_edge` still rejects the first)                        | `dup/two_outputs_sorted`                                                                   |
| `SF-4`  | `detect_cycles` runs after each entry                                                         | `cyc/selection_error_first`, the model property                                            |
| `SF-5`  | `resolve_rule` follows the first listed delegate, if any; an undeclared one is `RuleNotFound` | `del/one_hop`, `del/missing_delegate`, `del/plural_delegates`                              |
| `SF-6`  | `reject_rule_recipe` returns empty text                                                       | every `BackendRejectsDelegation` row                                                       |
| `SF-7`  | `validate_recipes` accepts `sources` in place of `deps`                                       | `dep/sources_only`                                                                         |
| `SF-8`  | `process_rules` rejects any delegating rule (Option C)                                        | `del/unselected`, `del/unselected_cycle`, `INV-UNSELECTED`                                 |
| `SF-9`  | A shadowed rule's description is used when the winner has none                                | `dup/rule_description`, `INV-LAST-WINS`                                                    |
| `SF-10` | Outputs compare as strings in `find_duplicate_graph_outputs` and `BuildGraph::insert_edge`    | `dup/path_interior_dot`                                                                    |
| `SF-11` | Selector names are not sorted                                                                 | `sel/multiple`                                                                             |
| `SF-12` | `detect_cycles` is skipped                                                                    | `cyc/two_targets`, `cyc/self_dependency`, `cyc/dependency_only`, `del/in_dependency_cycle` |

### Formal verification decision

Verus is not warranted in this task. The change introduces no production
business logic; the model is non-recursive and has no link to executable code
beyond a test, so a proof of `win` or of the fold would restate its definition.
That is the artificial obligation the development guidance warns against.

Kani's existing harnesses in `src/ir/from_manifest/verification.rs` cover three
*failing* selector shapes on `resolve_rule`, with an empty map, and one
duplicate output. Nothing covers the success path. `C-1` forbids adding it
here, so it is owed by 26.2.2, which changes that code:

- `selected_rule_is_the_mapped_rule`: a two-entry map with a symbolic key;
- `selected_delegation_is_rejected`: the new variant, with names preserved; and
- if tractable, last-wins over two `IrHashMap` insertions. That mainly exercises
  `kani_map` and is not evidence about `std::collections::HashMap`, so the
  property tests remain primary.

The Verus decision reverses if the approver chooses Option B, or a later task
adds delegation. The resolver then becomes a graph walk with contractual
obligations over unbounded inputs. Before designing it, that task must state
and prove the following, and pass `make verus` with the new proof file as a
milestone criterion:

- Precondition: every body is `Exec` or `Deleg`. `D` is the domain of `win`, a
  finite set of distinct names. Policy first fixes arity: empty and plural
  delegate lists are typed failures unless given semantics, in which case the
  walk is a depth-first forest with separate on-stack and finished sets and a
  deterministic error priority.
- Invariant: `visited` is a subset of `D`, gains a name only after a successful
  lookup, and never repeats one. Measure: `|D \ visited|`, a natural number.
  (The obvious `|R| - |visited|` is unsound: an undeclared name marked before
  lookup makes it underflow.)
- Classification: exactly one of resolved, missing, cyclic, or bad arity holds,
  each if and only if its path predicate holds.
- Representation: the map built by `process_rules` equals `win`, as a loop
  invariant.
- Trust boundaries: specifications of `String` and `HashMap` are named. No
  `assume`, `admit`, or external body establishes the result.

Those statements are hypotheses about an eventual proof, not scaffolding to
compile now. The executable suite would stay beside any such proof, because a
proved kernel does not show that loading, rendering, and diagnostics reach it.

Diagnostics wording, command-line plumbing, and linter wiring take conventional
tests only.

### Suggested BDD features

Proposed file: `tests/features/recipe_characterization.feature`, run by
`cargo nextest run --test bdd_tests recipe_characterization`. Each scenario
traces to `REQ-COVER` and `EP-M3`. The unhappy scenario uses the existing typed
cycle step; `IR generation fails` is avoided because it also passes when a path
is mistyped.

```gherkin
Feature: Recipe and declaration characterization

  Scenario: A target runs the last rule declared with the selected name
    When the manifest file "tests/data/recipe_characterization/dup/rule_last_wins.yml" is compiled to IR
    Then the IR recipe for "out" is the command "echo second"

  Scenario: A delegating rule is left unresolved by lowering
    When the manifest file "tests/data/recipe_characterization/del/one_hop.yml" is compiled to IR
    Then the IR recipe for "out" is an unresolved reference to rule "b"

  Scenario: An unselected delegation cycle does not affect the graph
    When the manifest file "tests/data/recipe_characterization/del/unselected_cycle.yml" is compiled to IR
    Then the IR recipe for "out" is the command "echo b"

  Scenario: A dependency-only target runs no recipe
    When the manifest file "tests/data/recipe_characterization/dep/explicit_phony.yml" is compiled to IR
    Then the IR recipe for "agg" is dependency-only

  Scenario: A dependency cycle is reported from its smallest member
    When the manifest file "tests/data/recipe_characterization/cyc/two_targets.yml" is compiled to IR
    Then IR generation fails
    And the IR cycle is "a -> b -> a"
```

26.2.2 replaces the delegation scenario with a rejection scenario.

## Plan of work

### `EP-M0` re-baseline

Rebase onto `origin/main`. Read every commit that touches a symbol named in
`Context and orientation`:

```sh
git log a97bd9491..HEAD -- src/ast src/ir src/manifest src/ninja_gen src/lint src/runner
```

Check whether `#1166`, `#1183`, or any `#1104` child has merged. Build with
`cargo build --features lint`, save the probe script from
`Artefacts and notes`, and run it. Record the new base and any difference in
`Surprises & discoveries`.

### `EP-M1` policy ADR, issues, and corrected live documentation

Raise the four issues in `D-3` first, so that later text can cite them.

Write the ADR. Its number is provisionally 0088: `0084`, `0086`, and `0087` are
held by open pull requests, so sweep every remote branch first. Title: "Select
one executable rule and reject delegation during lowering". Status follows
`D-11`. Its `Decision outcome` reproduces these policies verbatim, each as a
level-three heading whose first token is the identifier:

- `POL-SELECTION`: an entry that uses `rule:` selects exactly one rule. No name
  is `EmptyRule`. Two or more names, repeated or not, is `MultipleRules` with
  the names sorted. One undeclared name is `RuleNotFound`. Names compare
  exactly: an empty or blank name is an ordinary name, selectors are rendered,
  and rule names are not. The selected rule's description, if any, is the
  action's description; an entry's own description is never used for it.
- `POL-DELEGATION`: selecting a rule whose recipe is `rule:` is
  `IrGenError::RuleDelegationUnsupported { target_name, rule_name, message }`.
  `target_name` is the entry's first output, as for its sibling variants.
  `rule_name` is the selected rule. Delegate names are neither inspected nor
  reported, which keeps the diagnostic bounded. The error is raised for an
  entry after its duplicate-output check and after `EmptyRule`,
  `MultipleRules`, and `RuleNotFound`; before its interpolation, hashing, and
  insertion; and before any later entry or cycle detection. It is identical in
  debug and release builds.
- `POL-UNSELECTED`: a rule that no lowered entry selects is not lowered and
  cannot cause a lowering error. It must still have a recipe and still render.
  An entry that loading removes, or that `help targets` discards as
  conditional, is not lowered; `check` lowers entries whose `when` query
  loading cannot evaluate.
- `POL-RULE-PRECEDENCE`: of several rules with one name, the last declared is
  the rule, wholly: recipe and description come from that declaration. Every
  declaration must load. Consumers that resolve rule names should agree with
  lowering; `undeclared-target-input` does not today, and 26.2.2 makes that
  path unreachable from a lowered manifest.
- `POL-OUTPUTS`: an output belongs to one entry. Outputs compare by path
  components, across actions and targets and within an entry. `DuplicateOutput`
  names the colliding outputs, sorted, as spelled by the entry visited later.
- `POL-ERROR-ORDER`: lowering visits actions, then targets, each in declaration
  order, and reports the first failure. Within an entry the order is duplicate
  output, selection, delegation, interpolation. Cycles are reported only if
  every entry lowered.
- `POL-CYCLES`: a cycle through `sources`, `deps`, or `order_only_deps` is
  `CircularDependency`, reported as a closed path that starts at its smallest
  member. Dependency-only entries participate.
- `POL-NON-RULE-REFERENCES`: a dependency that no entry produces is an external
  file. Lowering does not resolve `defaults`.
- `POL-DEPENDENCY-ONLY`: an entry with no recipe is dependency-only if and only
  if its rendered `deps` are non-blank; otherwise loading fails. It has no
  executable command and no progress label. A rule is never dependency-only.
  Its present encoding as an empty command is an implementation detail that
  26.2.1 replaces without changing behaviour.
- `POL-RECIPE-FORMS`: at most one of `command`, `script`, and `rule`. A null
  value is an absent one. A scalar command and a one-element command list are
  distinct recipes. `command_isolation` requires a command list when `isolated`
  and a command when present.

Non-goals: placeholders and shell text, governed by ADR-0027 as amended by
ADR-0034; and output path normalization, empty commands, and `defaults`
resolution, governed by the issues.

The ADR also records the options in `D-1`; the lemmas `L-REFINE`, `L-ERASE`,
`L-NO-DEREFERENCE`, and `L-NEVER-BUILT` as the reason for the shape; the Verus
condition for Option B; and the compatibility consequences and handoff below.

Compatibility consequences of `POL-DELEGATION`, to be accepted by name:

- `CO-1`: `graph`, `help targets` (with and without `--edges`), and `check` stop
  accepting a manifest that selects a delegating rule, so a passing `check` in
  continuous integration can start failing. `check` also rejects one whose
  selecting entry has a `when` that query loading cannot evaluate, as it
  already does for a missing rule.
- `CO-4`: DOT, HTML, and graph JSON for such manifests become errors. Rules with
  a `rule:` recipe still appear in graph JSON when unselected.
- `CO-5`: `generate`, `build`, and `clean` change from status 101 with a panic
  and no JSON document (debug) or status 1 with an unrelated message (release)
  to status 1 with the "Failed to build graph from the manifest." context, a
  null code, and a JSON error document on standard error. A manifest that fails
  today for a later reason reports delegation instead.
- `CO-13`: `IrGenError` is exhaustive and public, so a new variant is a
  **Breaking** changelog entry.

Handoff to 26.2.2, recorded in the ADR: the variant and its Fluent key
`ir.rule_delegation_unsupported` in every catalogue; classification in
`error_category` in `src/runner/graph/generation/telemetry.rs`, whose wildcard
will not force it; the two or three Kani harnesses above, with `CO-10`;
rewriting `undeclared_target_input_follows_delegated_rules`,
`undeclared_target_input_survives_a_delegation_cycle`, and
`unused_rule_terminates_on_a_delegation_cycle`, while keeping
`unused_rule_reports_a_rule_only_another_dead_rule_references`; the `Rule`
documentation in `src/ast/mod.rs`; the error listing in
`docs/netsuke-design.md`; amending RFC 0005's release test set; changelog and
migration notes; and, in the commit that implements the variant, retiring every
`Replace` row, extending the `IrGenError` conversion, pointing
`lowering_agrees_with_the_selection_model` at `intended`, retiring `observed`
and `L-ERASE`, restricting the replacement contract to unretired rows, and
replacing the delegation BDD scenario,
`delegation_is_rejected_only_by_emitting_commands`,
`conditional_delegation_passes_every_command`, and the delegation case in
`lint_consumers.rs`. Hand-built graphs still reach the backend rejection until
26.2.1.

Then correct live documents that misdescribe current behaviour, without
describing future behaviour as present:

- `docs/netsuke-design.md`: link the ADR from §5.3 and the §1.2 `FUTURE` note;
  correct the action-sharing sentence in §5.5 and "exists in the `actions` map"
  in §5.3; correct the list-selector paragraph unless `#1183` has.
- `docs/users-guide.md`, "Rules and recipes": `rule:` selects a rule from an
  action or target; a rule provides a command or script, and one that names
  another rule cannot be built; if several rules share a name the last is used,
  so prefer unique names. Build on `#1183`'s wording if it has merged.
- `docs/contents.md`: index the ADR.

Plateau: documentation and issues only. Acceptance: `make markdownlint`,
`make nixie`, and `make check-fmt` pass and the policy is reachable from the
design document. Recovery: revert the commits; issues remain as valid records.
Remaining gaps: no executable evidence yet.

### `EP-M2` library-boundary corpus, ledger, and contracts

Assigned: `REQ-COVER`, `REQ-DISTINGUISH`, `REQ-POLICY`, `REQ-DEFER`,
`GAP-DELEGATION`, `GAP-PRECEDENCE`.

Red: prove `#[files]` on one manifest. Then add all manifests and the contracts
before any row: `characterized_case` fails for every manifest with "no ledger
row", and `corpus_files_match_ledger_rows` names each one. Behaviour under test
already exists, so for each family also record one deliberately wrong but
*plausible* expectation per outcome class and its failure. For each `Replace`
row, run it first with its replacement as the expectation and record that it
fails because lowering succeeds or reports another error.

Green: write each row from `Artefacts and notes`, not from a failure message.

Write `const fn` edge constructors before any row, so that a row is a few
lines. Refactor: extract repeated constructors; one file per family.

Plateau: the suite passes with debug assertions on. Acceptance: the traced
tests pass; the number of `characterized_case` cases equals the number of
manifests; `SF-3b` and `SF-5` to `SF-12` are rejected by their named corpus
rows; full gates pass. Conformance: `C-1`, `C-3`, `C-12`. Recovery: additive;
revert. Remaining gaps: consumers, properties, profile.

### `EP-M3` command-line, linter, and behavioural coverage

Assigned: `REQ-COVER`, `GAP-DELEGATION`.

- `command_line.rs`: `delegation_is_rejected_only_by_emitting_commands` runs the
  binary on `del/one_hop.yml`. `graph`, `help targets`, and (with the `lint`
  feature) `check` succeed. `generate`, `--json generate`, `build`, and `clean`
  fail with status 101 and the panic text when `cfg!(debug_assertions)`, and
  otherwise with status 1; in the debug case `--json generate` writes the panic
  text and no JSON document to standard error. `sel/direct_command.yml` is the
  control for `graph`, `help targets`, and `generate`. A Unix-only case uses a
  sentinel fake Ninja through `NETSUKE_NINJA`, as
  `order_only_cycle_rejected_before_ninja` does, to show that `build` and
  `clean` start Ninja for the control and not for delegation.
- `command_line.rs`: `conditional_delegation_passes_every_command` pins that
  `del/conditional_entry.yml` succeeds for `generate` and `check`, and a
  missing rule in the same position fails `check` alone. `generate` is asserted
  always; both `check` assertions are behind `#[cfg(feature = "lint")]`.
- `diagnostics.rs`: `insta` snapshots, human and JSON, for `sel/empty_list.yml`,
  `sel/multiple.yml`, and `dep/nothing.yml`, each paired with a semantic
  assertion on the leading cause and, in JSON, the null lowering code. This is
  the baseline 27.1 needs. Normalize temporary paths and bidirectional isolates.
- `lint_consumers.rs`, behind `#[cfg(feature = "lint")]`, with inline manifests
  outside the corpus: a duplicate rule whose *last* body is `mkdir -p d`
  triggers `directory-dep-not-order-only` and whose *first* is does not; a
  delegation to a duplicated name triggers `undeclared-target-input` from the
  *first* body. Use output names of at least three characters, because the rule
  ignores shorter ones.
- `tests/features/recipe_characterization.feature` and
  `tests/bdd/steps/recipe_characterization.rs` with three new `Then` steps,
  registered in `tests/bdd/steps/mod.rs`.

Plateau: every consumer boundary has a characterized delegation and duplicate
case. Acceptance: focused tests pass with all features and with default
features; full gates pass, including `make lint-default-features` and
`make test-default-features`. Recovery: additive; revert. Remaining gaps:
properties and the second profile.

### `EP-M4` selection properties, seeded faults, and profile evidence

Assigned: `INV-LAST-WINS`, `INV-UNSELECTED`, `INV-ORDER`, `INV-TOTAL`,
`L-REFINE`, `L-ERASE`, `L-NO-DEREFERENCE`, `L-NEVER-BUILT`.

Write the generator, its classification test, the model, and the properties in
that order, following `Methods per obligation`. Run `SF-1` to `SF-4`, and re-run
`SF-8` and `SF-9` against the relation properties. Then, with no other gate
running, run the crate with debug assertions disabled and rebuild normally
afterwards, because this leaves a non-asserting binary in `target/debug`:

```sh
F="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$F" CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false \
  cargo nextest run --test recipe_characterization_tests --all-features
RUSTFLAGS="$F" cargo build --all-features
```

Plateau: the model agrees with lowering, every fault is rejected, and both
profile halves are evidenced. Acceptance: `make proptest` and the command above
pass; transcripts recorded. Recovery: additive; revert. Remaining gaps:
documentation.

### `EP-M5` documentation closure and roadmap completion

- `docs/developers-guide.md`, "Test suite map": the suite, the three intents,
  the re-use policy, and the retirement rule.
- `docs/repository-layout.md`: `tests/data/recipe_characterization/` and the
  suite.
- `docs/roadmap-hexagonal-hardening.md`: tick 26.1.2, link the ADR, and add a
  dated note beneath it and beside the allocation bullet stating `D-12`. Do not
  rewrite the acceptance bullet. Respect `C-5`.
- `docs/rfcs/rfc-0026-*.md`, in its own commit: the same `D-12` correction, and
  a note under `Outstanding decisions` that the ADR settles delegation and
  duplicate precedence.
- `docs/adrs/adr-0035-*.md`: if its conventions call for a pointer, add it as an
  amendment; do not rewrite accepted text.
- `CHANGELOG.md`: no entry, because no behaviour changes.

Search for stale claims with
`grep -rn '#699' docs --include='*.md' | grep -v execplans/`.

Plateau and acceptance: all items in `Validation and acceptance`. Recovery:
revert. Remaining gaps after this task: the typed error (26.2.2), the resolved
representation (26.2.1), and the four issues.

## Milestones and plateaus

Each milestone above states its assigned items, plateau, acceptance, recovery,
and remaining gaps. At every boundary also check: no `src/` diff (`C-1`); no
new dependency or export (`C-6`); no public interface, trust boundary, or
persisted format changed; trace links in `Conformance basis` name real tests;
and the state of `#1166` and `#1183` is reconciled. Compatibility decision for
every milestone: none. No released or public interface changes, and nothing
here is a shim.

## Concrete steps

Working directory: the repository root. Confirm the branch with
`git branch --show-current`.

Focused loop, with the flags the gates use on Linux so that fingerprints match.
Do not export them: the gate recipes prepend the caller's value.

```sh
F="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
RUSTFLAGS="$F" cargo nextest run --test recipe_characterization_tests --all-features
RUSTFLAGS="$F" cargo nextest run --test bdd_tests --all-features recipe_characterization
```

Full gates after each milestone, sequentially, keeping the output. Run
`make fmt` first whenever Markdown or Rust changed.

```sh
set -o pipefail
B="$(git branch --show-current)"; P="$(get-project)"; rc=0
for g in check-fmt lint doc-coverage markdownlint nixie test \
         lint-default-features test-default-features; do
  make "$g" 2>&1 | tee "/tmp/$g-$P-$B.out" || { rc=$?; echo "FAILED: $g"; break; }
done
test "$rc" -eq 0
```

`EP-M1` runs the Markdown gates and `make test`, because
`tests/execplan_status_contract_tests.rs` reads this plan. Run `make proptest`
at `EP-M4`. Run `make test-workflow-contracts` if a workflow or
`.config/nextest.toml` changes; neither should. Commit only when every gate
passes, in small steps. Delegate the full run to the `scrutineer` agent. A
build queued for admission is not a failure.

Expected focused output after `EP-M2` (abridged, illustrative until run):

```plaintext
PASS recipe_characterization_tests characterized_case::path_001_..._cyc_dependency_only_yml
...
PASS recipe_characterization_tests contracts::corpus_files_match_ledger_rows
Summary: ... passed, 0 failed
```

## Validation and acceptance

The task is accepted when all of the following hold:

1. `make check-fmt`, `make lint`, and `make test` pass at each milestone.
2. Every corpus manifest is its own passing case, by construction.
3. Deleting any manifest, row, or ADR policy heading makes a named contract
   fail.
4. Every seeded fault is rejected by the named cases, with evidence recorded.
5. The suite passes with debug assertions disabled.
6. The following prints nothing:

   ```sh
   git diff --stat "$(git merge-base origin/main HEAD)" -- \
     src locales build.rs build_l10n_audit benches
   ```

7. The ADR is accepted under `D-11` and linked from the design document and
   roadmap, and roadmap item 26.1.2 is ticked.

Red, green, and seeded-fault transcripts are recorded per milestone in
`Artefacts and notes`.

## Idempotence and recovery

Every step is repeatable. Tests write only to temporary directories. Seeded
faults are applied to a committed tree and reverted by path; acceptance item 6
fails if one is left behind. Property failures write
`tests/recipe_characterization_tests.proptest-regressions`; delete entries that
seeded faults produced and keep those that real defects produced. Raising
issues is the one irreversible step: search for an existing issue first.

## Artefacts and notes

### Probe provenance

Planning probes ran a debug binary with the `lint` feature, built from a
worktree whose `src/` differs from the planning base only in `#[mutants::skip]`
attributes on `#[cfg(kani)]` items, with Ninja 1.11.1. Every row below was
observed on 2026-11-25. The release-profile result came from the published
`0.1.0-beta4` binary. Re-run at `EP-M0`; this is not evidence about a later
commit.

### Corpus inputs

Each key is a row identifier and the path of its manifest under
`tests/data/recipe_characterization/`. Each manifest begins with
`netsuke_version: "1.0.0"`. Flow style keeps this listing short; the committed
manifests may use block style, with a comment stating what the case shows.

```yaml
# characterization-inputs
sel/direct_command:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: r}]
sel/direct_script:
  rules: [{name: r, script: echo r}]
  targets: [{name: out, rule: r}]
sel/isolated_list:
  rules: [{name: r, command: [echo a, echo b], command_isolation: isolated}]
  targets: [{name: out, rule: r}]
sel/from_action:
  rules: [{name: r, command: echo r}]
  actions: [{name: act, rule: r}]
  targets: []
sel/single_item_list:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: [r]}]
sel/repeated_name:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: [r, r]}]
sel/multiple:
  rules: [{name: r, command: echo r}, {name: s, command: echo s}]
  targets: [{name: out, rule: [s, r]}]
sel/empty_list:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: []}]
sel/empty_string:
  targets: [{name: out, rule: ""}]
sel/declared_blank_name:
  rules: [{name: "", command: echo blank}]
  targets: [{name: out, rule: ""}]
sel/templated_selector:
  vars: {which: s}
  rules: [{name: r, command: echo r}, {name: s, command: echo s}]
  targets: [{name: out, vars: {which: r}, rule: "{{ which }}"}]
sel/rule_description:
  rules: [{name: r, command: echo r, description: RD}]
  targets: [{name: out, rule: r, description: TD}]
sel/rule_without_description:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: r, description: TD}]
sel/shared_action:
  rules: [{name: r, command: echo r}]
  targets: [{name: o1, rule: r}, {name: o2, rule: r}]
sel/serial_deps:
  rules: [{name: r, command: echo r}]
  targets: [{name: out, rule: r, deps: [p, q], dependency_order: serial},
    {name: p, command: echo p}, {name: q, command: echo q}]
miss/action_rule:
  actions: [{name: act, rule: nope}]
  targets: []
miss/action_before_target:
  targets: [{name: out, rule: nope_target}]
  actions: [{name: act, rule: nope_action}]
miss/templated_rule_name:
  vars: {which: r}
  rules: [{name: "{{ which }}", command: echo r}]
  targets: [{name: out, rule: r}]
miss/dependency:
  targets: [{name: out, command: echo r, deps: external}]
miss/default:
  targets: [{name: out, command: echo r}]
  defaults: [nope]
del/one_hop:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: a}]
del/two_hops:
  rules: [{name: a, rule: b}, {name: b, rule: c}, {name: c, command: echo c}]
  targets: [{name: out, rule: a}]
del/missing_delegate:
  rules: [{name: a, rule: nope}]
  targets: [{name: out, rule: a}]
del/self_reference:
  rules: [{name: a, rule: a}]
  targets: [{name: out, rule: a}]
del/cycle:
  rules: [{name: a, rule: b}, {name: b, rule: a}]
  targets: [{name: out, rule: a}]
del/plural_delegates:
  rules: [{name: a, rule: [b, c]}, {name: b, command: echo b}, {name: c, command: echo c}]
  targets: [{name: out, rule: a}]
del/empty_delegates:
  rules: [{name: a, rule: []}]
  targets: [{name: out, rule: a}]
del/blank_delegate:
  rules: [{name: a, rule: ""}]
  targets: [{name: out, rule: a}]
del/templated_delegate:
  vars: {which: b}
  rules: [{name: a, rule: "{{ which }}"}, {name: b, command: echo b}]
  targets: [{name: out, rule: a}]
del/from_action:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  actions: [{name: act, rule: a}]
  targets: []
del/serial_deps:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: a, deps: [p, q], dependency_order: serial},
    {name: p, command: echo p}, {name: q, command: echo q}]
del/not_default:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: ok, command: echo ok}, {name: out, rule: a}]
  defaults: [ok]
del/in_dependency_cycle:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: p, rule: a, deps: q}, {name: q, command: echo q, deps: p}]
del/before_later_duplicate:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: a}, {name: p, command: echo 1}, {name: p, command: echo 2}]
del/before_later_missing:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: a}, {name: p, rule: nope}]
del/after_duplicate_output:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, command: echo 1}, {name: out, rule: a}]
del/plural_selector:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: [a, b]}]
del/unselected:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: b}]
del/unselected_cycle:
  rules: [{name: a, rule: a}, {name: b, command: echo b}]
  targets: [{name: out, rule: b}]
del/conditional_entry:
  rules: [{name: a, rule: b}, {name: b, command: echo b}]
  targets: [{name: ok, command: echo ok},
    {name: out, rule: a, when: "env('NETSUKE_CHARACTERIZE', default='') == 'on'"}]
cyc/two_targets:
  targets: [{name: b, command: echo b, deps: a}, {name: a, command: echo a, deps: b}]
cyc/self_dependency:
  targets: [{name: a, command: echo a, deps: a}]
cyc/dependency_only:
  targets: [{name: a, deps: b}, {name: b, deps: a}]
cyc/selection_error_first:
  targets: [{name: a, command: echo a, deps: b}, {name: b, command: echo b, deps: a},
    {name: c, rule: nope}]
dup/rule_last_wins:
  rules: [{name: r, command: echo first}, {name: r, command: echo second}]
  targets: [{name: out, rule: r}]
dup/rule_description:
  rules: [{name: r, command: echo first, description: D1}, {name: r, command: echo second}]
  targets: [{name: out, rule: r}]
dup/rule_hides_delegation:
  rules: [{name: r, rule: b}, {name: r, command: echo second}, {name: b, command: echo b}]
  targets: [{name: out, rule: r}]
dup/rule_hidden_by_delegate:
  rules: [{name: r, command: echo first}, {name: r, rule: b}, {name: b, command: echo b}]
  targets: [{name: out, rule: r}]
dup/rule_shadowed_invalid:
  rules: [{name: r}, {name: r, command: echo second}]
  targets: [{name: out, rule: r}]
dup/output_siblings:
  targets: [{name: [out, out], command: echo 1}]
dup/two_outputs_sorted:
  targets: [{name: [x, w], command: echo 1}, {name: [x, w], command: echo 2}]
dup/action_and_target:
  targets: [{name: out, command: echo t}]
  actions: [{name: out, command: echo a}]
dup/two_actions:
  actions: [{name: act, command: echo 1}, {name: act, command: echo 2}]
  targets: []
dup/foreach_collision:
  targets: [{name: "{{ item }}", foreach: [a, a], command: echo 1}]
dup/path_interior_dot:
  targets: [{name: x/out, command: echo 1}, {name: x/./out, command: echo 2}]
dup/path_double_separator:
  targets: [{name: x/out, command: echo 1}, {name: x//out, command: echo 2}]
dup/path_trailing_separator:
  targets: [{name: x/out, command: echo 1}, {name: x/out/, command: echo 2}]
dup/path_leading_dot:
  targets: [{name: out, command: echo 1}, {name: ./out, command: echo 2}]
dup/path_parent:
  targets: [{name: x/out, command: echo 1}, {name: x/z/../out, command: echo 2}]
dup/output_before_selection:
  targets: [{name: out, command: echo 1}, {name: out, rule: nope}]
dep/explicit_phony:
  targets: [{name: agg, deps: x, phony: true}, {name: x, command: echo x}]
dep/action:
  actions: [{name: agg, deps: x}]
  targets: [{name: x, command: echo x}]
dep/sources_and_deps:
  targets: [{name: agg, sources: w, deps: x}, {name: x, command: echo x}]
dep/several_outputs:
  targets: [{name: [agg, agg2], deps: x}, {name: x, command: echo x}]
dep/two_aggregates:
  targets: [{name: agg, deps: x}, {name: agg2, deps: x}, {name: x, command: echo x}]
dep/always:
  targets: [{name: agg, deps: x, always: true}, {name: x, command: echo x}]
dep/serial:
  targets: [{name: agg, deps: [x, w], dependency_order: serial}, {name: x, command: echo x},
    {name: w, command: echo w}]
dep/null_command:
  targets: [{name: agg, command: null, deps: x}, {name: x, command: echo x}]
dep/nothing:
  targets: [{name: agg}]
dep/sources_only:
  targets: [{name: agg, sources: x}, {name: x, command: echo x}]
dep/order_only:
  targets: [{name: agg, order_only_deps: x}, {name: x, command: echo x}]
dep/rule_without_recipe:
  rules: [{name: r}]
  targets: [{name: out, rule: r}]
dep/unselected_rule_blank:
  rules: [{name: r}]
  targets: [{name: out, command: echo 1}]
var/scalar:
  targets: [{name: out, command: echo a}]
var/list:
  targets: [{name: out, command: [echo a, echo b]}]
var/single_item_list:
  targets: [{name: out, command: [echo a]}]
var/isolated_list:
  targets: [{name: out, command: [echo a, echo b], command_isolation: isolated}]
var/script:
  targets: [{name: out, script: echo a}]
var/empty_script:
  targets: [{name: out, script: ""}]
var/empty_command:
  targets: [{name: out, command: ""}]
var/blank_command:
  targets: [{name: out, command: "  "}]
var/blank_list_entries:
  targets: [{name: out, command: ["", ""]}]
var/null_command:
  targets: [{name: out, command: null}]
var/null_script:
  targets: [{name: out, script: null}]
var/null_rule:
  targets: [{name: out, rule: null}]
var/null_delegate:
  rules: [{name: a, rule: null}]
  targets: [{name: out, rule: a}]
var/all_three:
  targets: [{name: out, command: a, script: b, rule: c}]
var/selector_with_isolation:
  rules: [{name: r, command: [a]}]
  targets: [{name: out, rule: r, command_isolation: isolated}]
```

Save this script outside the repository and run
`python3 probe.py target/debug/netsuke <this plan>`. It prints the exit
statuses of `generate`, `graph`, `help targets`, and `check`, and the last
cause.

```python
import os, pathlib, re, subprocess, sys, tempfile

binary, plan = sys.argv[1], pathlib.Path(sys.argv[2]).read_text()
block = plan.split("# characterization-inputs\n", 1)[1].split("```", 1)[0]
env = {k: v for k, v in os.environ.items() if not k.startswith("NETSUKE")}
for entry in re.split(r"^(?=\S)", block, flags=re.M):
    if not entry.strip():
        continue
    case, body = entry.split(":\n", 1)
    manifest = 'netsuke_version: "1.0.0"\n' + re.sub(r"^  ", "", body, flags=re.M)
    with tempfile.TemporaryDirectory() as workspace:
        pathlib.Path(workspace, "Netsukefile").write_text(manifest)
        runs = [
            subprocess.run([binary, "--locale", "en-US", "--color", "never", *command],
                           cwd=workspace, env=env, capture_output=True, text=True)
            for command in (["generate"], ["graph"], ["help", "targets"], ["check"])
        ]
    cause = [line.strip() for line in runs[0].stderr.splitlines()
             if re.match(r"\s*(x |`-> |X Error: Circular|rules do not)", line)]
    print(case, [run.returncode for run in runs], cause[-1:] or "ok")
```

### Observed behaviour and intent

"Lowers" means lowering succeeds. "Issue 1" to "issue 4" are those of `D-3`,
replaced by real numbers in the ledger. Every `Replace` row becomes
`LowerRejected(RuleDelegationUnsupported { target, rule })`, where `target` is
the first output of the selecting entry and `rule` the selected rule. Of 88
rows, 16 are `Replace` and 5 are `Hold`.

| Row                            | Observed at the planning base                     | Intent                                   |
| ------------------------------ | ------------------------------------------------- | ---------------------------------------- |
| `sel/direct_command`           | Lowers to `echo r`                                | Preserve `POL-SELECTION`                 |
| `sel/direct_script`            | Lowers to the script `echo r`                     | Preserve `POL-SELECTION`                 |
| `sel/isolated_list`            | Lowers to an isolated two-entry list              | Preserve `POL-SELECTION`                 |
| `sel/from_action`              | Lowers to `echo r`; the edge is phony             | Preserve `POL-SELECTION`                 |
| `sel/single_item_list`         | Lowers to `echo r`                                | Preserve `POL-SELECTION`                 |
| `sel/repeated_name`            | `MultipleRules [r, r]`                            | Preserve `POL-SELECTION`                 |
| `sel/multiple`                 | `MultipleRules [r, s]`, sorted                    | Preserve `POL-SELECTION`                 |
| `sel/empty_list`               | `EmptyRule`                                       | Preserve `POL-SELECTION`                 |
| `sel/empty_string`             | `RuleNotFound("")`                                | Preserve `POL-SELECTION`                 |
| `sel/declared_blank_name`      | Lowers to `echo blank`                            | Preserve `POL-SELECTION`                 |
| `sel/templated_selector`       | Lowers to `echo r`; the target variable wins      | Preserve `POL-SELECTION`                 |
| `sel/rule_description`         | Lowers with description `RD`                      | Preserve `POL-SELECTION`                 |
| `sel/rule_without_description` | Lowers with no description                        | Preserve `POL-SELECTION`                 |
| `sel/shared_action`            | Two edges share one action                        | Preserve `POL-SELECTION`                 |
| `sel/serial_deps`              | Lowers; emits a bundle with a sidecar             | Preserve `POL-SELECTION`                 |
| `miss/action_rule`             | `RuleNotFound(act, nope)`                         | Preserve `POL-SELECTION`                 |
| `miss/action_before_target`    | `RuleNotFound(act, nope_action)`                  | Preserve `POL-ERROR-ORDER`               |
| `miss/templated_rule_name`     | `RuleNotFound(out, r)`                            | Preserve `POL-SELECTION`                 |
| `miss/dependency`              | Lowers and emits                                  | Preserve `POL-NON-RULE-REFERENCES`       |
| `miss/default`                 | Lowers and emits `default nope`                   | Hold, issue 4, `POL-NON-RULE-REFERENCES` |
| `del/one_hop`                  | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/two_hops`                 | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/missing_delegate`         | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/self_reference`           | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/cycle`                    | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/plural_delegates`         | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/empty_delegates`          | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/blank_delegate`           | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/templated_delegate`       | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/from_action`              | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/serial_deps`              | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/not_default`              | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/in_dependency_cycle`      | `CircularDependency [p, q, p]`                    | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/before_later_duplicate`   | `DuplicateOutput [p]`                             | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/before_later_missing`     | `RuleNotFound(p, nope)`                           | Replace, 26.2.2, `POL-DELEGATION`        |
| `del/after_duplicate_output`   | `DuplicateOutput [out]`                           | Preserve `POL-ERROR-ORDER`               |
| `del/plural_selector`          | `MultipleRules [a, b]`                            | Preserve `POL-ERROR-ORDER`               |
| `del/unselected`               | Lowers to `echo b` and emits                      | Preserve `POL-UNSELECTED`                |
| `del/unselected_cycle`         | Lowers to `echo b` and emits                      | Preserve `POL-UNSELECTED`                |
| `del/conditional_entry`        | Build loading drops `out`; lowers and emits `ok`  | Preserve `POL-UNSELECTED`                |
| `cyc/two_targets`              | `CircularDependency [a, b, a]`                    | Preserve `POL-CYCLES`                    |
| `cyc/self_dependency`          | `CircularDependency [a, a]`                       | Preserve `POL-CYCLES`                    |
| `cyc/dependency_only`          | `CircularDependency [a, b, a]`                    | Preserve `POL-CYCLES`                    |
| `cyc/selection_error_first`    | `RuleNotFound(c, nope)`                           | Preserve `POL-ERROR-ORDER`               |
| `dup/rule_last_wins`           | Lowers to `echo second`                           | Preserve `POL-RULE-PRECEDENCE`           |
| `dup/rule_description`         | Lowers to `echo second` with no description       | Preserve `POL-RULE-PRECEDENCE`           |
| `dup/rule_hides_delegation`    | Lowers to `echo second`                           | Preserve `POL-RULE-PRECEDENCE`           |
| `dup/rule_hidden_by_delegate`  | Lowers to an unresolved reference; emission fails | Replace, 26.2.2, `POL-DELEGATION`        |
| `dup/rule_shadowed_invalid`    | Load error: missing recipe                        | Preserve `POL-UNSELECTED`                |
| `dup/output_siblings`          | `DuplicateOutput [out]`                           | Preserve `POL-OUTPUTS`                   |
| `dup/two_outputs_sorted`       | `DuplicateOutput [w, x]`                          | Preserve `POL-OUTPUTS`                   |
| `dup/action_and_target`        | `DuplicateOutput [out]`                           | Preserve `POL-OUTPUTS`                   |
| `dup/two_actions`              | `DuplicateOutput [act]`                           | Preserve `POL-OUTPUTS`                   |
| `dup/foreach_collision`        | `DuplicateOutput [a]`                             | Preserve `POL-OUTPUTS`                   |
| `dup/path_interior_dot`        | `DuplicateOutput [x/./out]`                       | Preserve `POL-OUTPUTS`                   |
| `dup/path_double_separator`    | `DuplicateOutput [x//out]`                        | Preserve `POL-OUTPUTS`                   |
| `dup/path_trailing_separator`  | `DuplicateOutput [x/out/]`                        | Preserve `POL-OUTPUTS`                   |
| `dup/path_leading_dot`         | Lowers and emits; Ninja rejects it                | Hold, issue 2, `POL-OUTPUTS`             |
| `dup/path_parent`              | Lowers and emits; Ninja rejects it                | Hold, issue 2, `POL-OUTPUTS`             |
| `dup/output_before_selection`  | `DuplicateOutput [out]`                           | Preserve `POL-ERROR-ORDER`               |
| `dep/explicit_phony`           | Lowers to a dependency-only edge and emits        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/action`                   | Lowers to a dependency-only edge and emits        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/sources_and_deps`         | As above; `sources` are explicit inputs           | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/several_outputs`          | As above; one edge, two outputs                   | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/two_aggregates`           | As above; two edges share one action              | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/always`                   | As above; `always` is retained                    | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/serial`                   | As above; emits a bundle with a sidecar           | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/null_command`             | As above                                          | Preserve `POL-RECIPE-FORMS`              |
| `dep/nothing`                  | Load error: missing recipe                        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/sources_only`             | Load error: missing recipe                        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/order_only`               | Load error: missing recipe                        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/rule_without_recipe`      | Load error: missing recipe                        | Preserve `POL-DEPENDENCY-ONLY`           |
| `dep/unselected_rule_blank`    | Load error: missing recipe                        | Preserve `POL-UNSELECTED`                |
| `var/scalar`                   | Lowers to the scalar `echo a`                     | Preserve `POL-RECIPE-FORMS`              |
| `var/list`                     | Lowers to a shared two-entry list                 | Preserve `POL-RECIPE-FORMS`              |
| `var/single_item_list`         | Lowers to a shared one-entry list                 | Preserve `POL-RECIPE-FORMS`              |
| `var/isolated_list`            | Lowers to an isolated two-entry list              | Preserve `POL-RECIPE-FORMS`              |
| `var/script`                   | Lowers to the script `echo a`                     | Preserve `POL-RECIPE-FORMS`              |
| `var/empty_script`             | Lowers and emits; runs nothing                    | Preserve `POL-RECIPE-FORMS`              |
| `var/empty_command`            | Lowers and emits; Ninja rejects it                | Hold, issue 3, `POL-RECIPE-FORMS`        |
| `var/blank_command`            | Lowers and emits; Ninja rejects it                | Hold, issue 3, `POL-RECIPE-FORMS`        |
| `var/blank_list_entries`       | Lowers and emits; runs nothing                    | Preserve `POL-RECIPE-FORMS`              |
| `var/null_command`             | Load error: missing recipe                        | Preserve `POL-RECIPE-FORMS`              |
| `var/null_script`              | Load error: missing recipe                        | Preserve `POL-RECIPE-FORMS`              |
| `var/null_rule`                | Load error: missing recipe                        | Preserve `POL-RECIPE-FORMS`              |
| `var/null_delegate`            | Load error: missing recipe                        | Preserve `POL-RECIPE-FORMS`              |
| `var/all_three`                | Load error: mutually exclusive                    | Preserve `POL-RECIPE-FORMS`              |
| `var/selector_with_isolation`  | Load error: isolation requires a command list     | Preserve `POL-RECIPE-FORMS`              |

Emission outcome for every row marked "emission fails":

```plaintext
debug assertions on : panic "rules do not reference other rules"; exit status 101
debug assertions off: NinjaGenError::UnsafeNinjaValue; exit status 1
graph, help targets, check: exit status 0 in both
```

### Prior art

- Ninja 1.12.1 `ManifestParser::ParseRule` rejects a second rule with one name
  in one scope ("duplicate rule") and a rule without a command ("expected
  'command ='"). A Ninja rule cannot invoke another.
  <https://github.com/ninja-build/ninja/blob/v1.12.1/src/manifest_parser.cc>
- `just` rejects duplicate recipes unless `allow-duplicate-recipes` is set; then
  the later definition overrides. <https://just.systems/man/en/settings.html>
- Characterization tests record what code does because users depend on it, and
  leave differences from the specification to later, separate decisions.
  <https://bssw.io/items/working-effectively-with-legacy-code>

## Interfaces and dependencies

No dependency is added. New or edited code files:

```plaintext
tests/recipe_characterization_tests.rs              crate root; characterized_case
tests/recipe_characterization_tests/
  observation.rs               observe(), recipe_fact(), classification
  ledger.rs                    Row, Observed, Intent, Policy, Family, Owner
  rows/mod.rs                  ROWS, lookup by identifier
  rows/selection.rs            sel/*
  rows/missing.rs              miss/*
  rows/delegation.rs           del/*
  rows/cycles.rs               cyc/*
  rows/duplicates.rs           dup/*
  rows/dependency_only.rs      dep/*
  rows/variants.rs             var/*
  contracts.rs                 corpus, ledger, ADR, and roadmap agreement
  contract_negative_tests.rs   rejected inputs for each validator
  command_line.rs              EP-M3
  diagnostics.rs               EP-M3, insta
  lint_consumers.rs            EP-M3, feature "lint"
  selection_model.rs           EP-M4; abstraction, observed, intended
  selection_property_tests.rs  EP-M4
  relation_property_tests.rs   EP-M4
tests/features/recipe_characterization.feature      EP-M3
tests/bdd/steps/recipe_characterization.rs          EP-M3
tests/bdd/steps/mod.rs                              EP-M3, one line
```

`selection_model.rs` is introduced in `EP-M2` without its generator, because
`replacement_rows_state_the_policy_outcome` uses `observed` and `intended`.

The types that must exist at the end of `EP-M2`, all private to the crate:

```rust
/// What one edge runs, read from the graph by `recipe_fact` alone.
enum RecipeFact {
    Command(&'static str),
    CommandList { entries: &'static [&'static str], isolated: bool },
    Script(&'static str),
    DependencyOnly,
    Unresolved(&'static [&'static str]),
}

/// One edge, in insertion order; equal `action_group` values share an action.
struct EdgeFact {
    outputs: &'static [&'static str],
    recipe: RecipeFact,
    description: Option<&'static str>,
    inputs: &'static [&'static str],
    implicit_deps: &'static [&'static str],
    order_only_deps: &'static [&'static str],
    flags: EdgeFlags, // phony, always, serial
    action_group: u8,
}

struct Lowered { edges: &'static [EdgeFact], defaults: &'static [&'static str] }

enum LoadRejection { MissingRecipe, MutuallyExclusive, IsolationNeedsCommandList }

/// `RuleDelegationUnsupported` is specified here and implemented by 26.2.2.
enum LowerRejection {
    RuleNotFound { target: &'static str, rule: &'static str },
    EmptyRule { target: &'static str },
    MultipleRules { target: &'static str, rules: &'static [&'static str] },
    DuplicateOutput(&'static [&'static str]),
    CircularDependency(&'static [&'static str]),
    RuleDelegationUnsupported { target: &'static str, rule: &'static str },
}

enum Observed {
    LoadRejected(LoadRejection),
    LowerRejected(LowerRejection),
    BackendRejectsDelegation(Lowered),
    Emitted(Lowered),
}

/// What phases 26 and 27 must do with an observed outcome.
enum Intent {
    Preserve(Policy),
    Hold { policy: Policy, issue: u32, until: &'static str, review: RoadmapTask },
    Replace { policy: Policy, by: Observed, owner: RoadmapTask },
}

enum Family { Selection, Missing, Delegation, Cycle, Duplicate, DependencyOnly, Variant }

/// One variant per `POL-` heading, with its identifier.
enum Policy { Selection, Delegation, Unselected, /* ... one per heading */ }

/// A task identifier and the roadmap file that must contain it, unchecked.
struct RoadmapTask { id: &'static str, roadmap: &'static str }

struct Row { id: &'static str, family: Family, observed: Observed, intent: Intent }
```

If a borrowed expected type and an owned observed type prove awkward to compare,
`observe` returns owned mirrors and the comparison is one function; that is a
mechanical choice. The conversion from `IrGenError` is a `match` with no
wildcard, so a new variant fails to compile here. `InvalidManifest`,
`InvalidCommand`, `ActionSerialisation`, and `AmbiguousSubstitutionBoundary`
map to a test error, because no corpus manifest should produce them.
`catch_unwind` appears in one function that wraps only emission. Load
rejections are classified from the error chain under the English localizer,
holding `test_support::localizer_test_lock`.

`googletest` matchers state predicates in `command_line.rs`, `diagnostics.rs`,
and `lint_consumers.rs`; `pretty_assertions::assert_eq!` reports the difference
between expected and observed outcomes.

Re-use policy: the corpus manifests are shared test data and any suite may read
them. The ledger, observation, and model are private to this crate. A second
manifest classifier or selection model is not permitted; extend these. If
26.2.1 needs `observe` from another crate, move it to `test_support` in that
task, with its own review.

Retirement rule: a `Replace` row may change only to `Preserve` of its recorded
`by`. Any other outcome needs an ADR amendment first. A `Hold` row changes only
when its issue closes with a decision, which the row's `until` text describes.
Each `Hold` names 26.3.2, the last phase 26 task, as its `review`, so ticking
that task fails any unreviewed `Hold`.

## Revision note

- 2026-11-25: Initial draft from reconnaissance and live probes.
- 2026-11-25: Revised after a four-reviewer design review, each of which
  recommended revision. Settled duplicate precedence instead of deferring it.
  Replaced a tautological lemma and a false one, gave the model closed-form
  projections and representation relations, and added metamorphic properties.
  Added the query-loading, serial, and error-precedence cases and RFC 0005.
  Made emission observe the bundle path and the exact panic. Replaced per-row
  cases with a file-driven runner, made the intent types express the table, and
  closed the retirement loophole. A second pass by three of the reviewers
  confirmed the model, table, and lemmas and supplied the last corrections to
  seeded faults, contracts, portability, and the 26.2.2 handoff. Corrected the
  budgets, gate commands, command-line test design, and Verus condition.
  Re-probed all 88 rows. Scope, milestone order, and the no-production-change
  constraint are unchanged.
