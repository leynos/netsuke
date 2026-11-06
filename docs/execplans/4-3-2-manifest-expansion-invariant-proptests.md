# 4.3.2. Add Proptest coverage for manifest expansion invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

The plan has been through one expert design review and awaits explicit user
approval. No implementation step may begin until the user approves it; silence
is not approval.

## Purpose / big picture

Netsuke turns a YAML manifest (a `Netsukefile`) into a Ninja build file. Before
the manifest becomes typed Rust data, a loader pass called *expansion* rewrites
the parsed YAML value: every entry carrying a `foreach` key is copied once per
iteration value, and every entry carrying a `when` key is kept or dropped
according to a Jinja condition. Roadmap item 4.3.2 asks for generated tests
proving that expansion

1. copies every field that is not a control key (`foreach`, `when`) unchanged
   into each generated entry,
2. removes `when` once it has been evaluated,
3. gives each generated entry its own `item` (the iteration value) and `index`
   (its zero-based position) in `vars`, and
4. still honours `when` on *static* entries, meaning entries without `foreach`.

Today those claims are pinned mainly by hand-picked examples and by a few
narrow generated tests. After this change, a Proptest suite under
`src/manifest/expand/property_tests/` generates manifest entries in both the
`targets` and `actions` sections, with arbitrary JSON-shaped field values,
control keys at arbitrary positions, and conditions drawn from a closed set of
shapes with an independent Rust oracle, and checks each claim against an
expected result built without the production code.

Planning surfaced one real defect, which this plan also fixes. The design
document promises that the ordered map backing a manifest "retains the
insertion order observed in the YAML manifest" so that downstream consumers
"see keys in a stable sequence after foreach expansion, matching the authoring
intent" (`docs/netsuke-design.md`, the `ManifestMap` paragraph). The code
breaks the *authoring intent* half of that promise: `serde_json::Map::remove`,
under the `preserve_order` feature Netsuke enables, is a `swap_remove`, so
removing `when` or `foreach` from an entry moves the entry's *last* key into
the vacated slot whenever two or more keys follow the control key. The one
user-visible consequence is the deserializer's unknown-field diagnostic, which
reports the first unknown key in map order and can therefore name a different
field from the one the author wrote first. The fix is two calls to
`shift_remove`, driven by red tests.

Success is observable as follows. `make proptest` selects the suite under
`manifest::expand::property_tests` and every test passes. For each committed
mutation patch whose stem begins `manifest__expand__property_tests__`, applying
it makes the named property fail with a shrunk counter-example. Three new
behavioural scenarios in `tests/features/manifest.feature` pass. Roadmap item
4.3.2 and its four sub-items are marked done.

## Signposts: documentation and skills

Read these before starting, in this order:

- `AGENTS.md` (repository root): gates, the 400-line file cap, the ban on
  in-process environment mutation, doc-comment rules, and the Polonius and
  next-solver posture.
- `docs/roadmap.md`, item 4.3.2, and
  `docs/formal-verification-methods-in-netsuke.md`, section *Proptest for
  determinism and manifest semantics*: the upstream statement of the work.
- `docs/netsuke-design.md`, sections 1.2 (*The Six Stages of a Netsuke
  Build*, stage 3), 2.5 (`foreach` and `when`), 3.3 (the template expansion
  stages), and the `ManifestMap` key-order paragraph.
- `docs/developers-guide.md`, sections *Property-based testing with proptest*
  (including *Emission determinism suites*, which this suite mirrors), *Manifest
  `foreach` expansion*, and the `expand_foreach` interface reference.
- `docs/users-guide.md`, section *Generate targets with `foreach` and `when`*.
- `docs/adrs/adr-0003-keep-foreach-and-when-scoped-to-manifest-targets.md`,
  `docs/adrs/adr-0050-bound-manifest-template-evaluation.md`,
  `docs/adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md`,
  and `docs/adrs/adr-0030-ninja-emission-determinism-contract.md`.
- `docs/execplans/4-3-1-proptests-for-deterministic-ninja-emission.md`: the
  sibling plan. Copy its discipline (obligation ledger, mutation patches,
  coverage classification), not its length.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/snapshot-testing-in-netsuke-using-insta.md`.
- `docs/ortho-config-users-guide.md`: read for orientation only. This change
  adds no configuration surface, so OrthoConfig is not touched (see
  `Decision log`).
- `docs/documentation-style-guide.md`: ExecPlan `Status:` rules and
  en-GB-oxendict spelling.

Load these skills when implementing: `rust-router` first, then `proptest`
(strategy design, the filtering trap, regression files), `rust-unit-testing`
(rstest tables, googletest and `pretty_assertions` assertions),
`rust-verification` (only if the method selection below is challenged),
`hexagonal-architecture` (to keep test support out of production modules),
`codegraph-mcp` (callers and impact of `when_allows` and `expand_foreach_item`),
`en-gb-oxendict-style`, and `commit-message`. Use `firecrawl-mcp` for any
question about MiniJinja, `serde_json`, or Proptest behaviour that the local
registry sources under `~/.cargo/registry/src/` do not answer.

## Context and orientation

### The pipeline

`src/manifest/mod.rs::evaluate_manifest` (lines 106–169) runs the manifest
pipeline:

1. `serde_saphyr::from_str` parses the YAML text into a `ManifestValue`, which
   is `serde_json::Value` (`src/manifest/mod.rs`, lines 40–43). A `ManifestMap`
   is `serde_json::Map<String, ManifestValue>`.
2. A MiniJinja `Environment` is built with `UndefinedBehavior::Strict`, the
   standard library, manifest-level `vars` as globals, and macros.
3. `expand_foreach_with_budget(&mut doc, &jinja, &budget)` performs expansion.
   This is the subject of this plan.
4. `serde_json::from_value::<NetsukeManifest>(doc)` deserializes into the
   typed abstract syntax tree (AST). `Target` (`src/ast/target.rs`, line 13)
   uses `#[serde(deny_unknown_fields)]` with a flattened recipe, and actions
   pass through `deserialize_actions` into the same `Target` type
   (`src/ast/mod.rs`, lines 60–68). A leftover `when` or `foreach` key would
   fail here, and an unknown key is reported in map order.
5. `render::render_manifest_with_budget` renders the string fields.

The expanded document has exactly one consumer, the `from_value` call in step
four. Lint rules, graph output, and the IR work from the typed manifest, and
`Vars` is a `HashMap`, so map order inside the expanded value matters only to
the deserializer's diagnostics.

### The expansion code

All of it lives in two files.

`src/manifest/expand/mod.rs` (291 lines) holds `expand_foreach` (a
`#[cfg(test)]` wrapper that supplies a default budget),
`expand_foreach_with_budget` (`pub(crate)`), and the private helpers
`expand_section`, `expand_target`, `expand_foreach_target`,
`expand_foreach_item`, and `expand_static_target`. Only the `targets` and
`actions` sections are expanded (`for section in ["targets", "actions"]`).
Non-object entries pass through unchanged. An entry is a *foreach entry* if its
map contains the key `foreach`, and a *static entry* otherwise.

`expand_foreach_target` enumerates the iterator lazily and builds a
`ForeachItem { source, item, index }` per value. `expand_foreach_item` checks
the cardinality budget with that `index`, clones the source map, calls
`entry.remove("foreach")` (line 242), evaluates `when` through `when_allows`
with `(item, index)`, and on inclusion calls `inject_iteration_vars`.
`expand_static_target` charges the budget, calls `when_allows` with no
iteration, and drops or keeps the map. Static entries never receive `vars`.

`src/manifest/expand/evaluation.rs` (239 lines) holds:

- `parse_foreach_values`: an inline JSON array is converted with
  `minijinja::Value::from_serialize`; any other value must be a string, which
  is compiled and evaluated as a Jinja expression with an empty local context.
- `when_allows`: removes `when` with `map.remove("when")` (line 44), rejects a
  non-string value with the localized message `Field 'when' must be a string.`
  (key `manifest.field_not_string`), builds the condition context, and
  evaluates it. The result is `Include`, `Exclude`, or `Conditional`.
  `Conditional` arises only in manifest-query mode, when a helper is disabled.
- `when_context`: the condition sees the entry's own `vars` object as its local
  context; during iteration `item` and `index` are added, overriding colliding
  `vars` keys. A static entry whose `vars` is not an object is silently given
  an empty context (`unwrap_or_default`). Manifest-level `vars`, `env`, `glob`,
  and the standard library are environment globals.
- `eval_when`: rejects an empty or whitespace-only condition, then calls
  `jinja_macros::evaluate_when_expression`. That function falls back to
  template rendering only when `compile_expression` fails
  (`src/manifest/jinja_macros/mod.rs`, lines 62–64); a runtime error
  propagates. The expression path takes MiniJinja truthiness through
  `Value::is_true`. The template path accepts only a trimmed, lower-cased
  `true` or `1`. A condition written `{{ … }}` always takes the template path,
  because MiniJinja's parser reads `{ … }` as a map literal and then demands a
  `:` (`minijinja-2.24.0/src/compiler/parser.rs`, lines 774–789).
- `inject_iteration_vars`: creates `vars` if absent (an error if present but
  not an object), then inserts `item` as `serde_json::to_value(item)` and
  `index` as a `u64` JSON number. Insertion overwrites colliding user keys in
  place.

### The key-order defect

`serde_json` 1.0.151 (the locked version) implements `Map::remove` as
`self.swap_remove(key)` when `preserve_order` is enabled
(`serde_json-1.0.151/src/map.rs`, lines 156–165). `Map::shift_remove` exists
under the same feature and preserves relative order. Netsuke enables
`preserve_order` in both `[dependencies]` and `[build-dependencies]`
(`Cargo.toml`, lines 143 and 164). A target written as

```yaml
targets:
  - name: a
    when: "true"
    command: echo a
    after: done
```

therefore leaves expansion with keys `name, after, command`, not
`name, command, after`. With exactly one key after the control key,
`swap_remove` moves that key into the same slot and order survives, which is
why two existing tests pass by coincidence (see `Surprises & discoveries`).

Two facts make this defect easy to miss in tests:

- `serde_json::Map` equality delegates to `IndexMap` equality, which ignores
  order (`serde_json-1.0.151/src/map.rs`, lines 409–413). `prop_assert_eq!` on
  two `ManifestValue`s cannot see a reordering. Order must be asserted on key
  sequences, `map.keys().collect::<Vec<_>>()`.
- MiniJinja is built without its `preserve_order` feature (`Cargo.toml`, line
  113, enables only `fuel` and `loader`), so its maps are `BTreeMap`s
  (`minijinja-2.24.0/src/value/mod.rs`, lines 246–247). An object-valued `item`
  therefore arrives in `vars.item` with its keys sorted. That is the documented
  behaviour of the evaluator, not a defect in expansion, and is outside the
  key-order contract.

### Existing tests

Expansion already has extensive example coverage under
`src/manifest/expand/tests/` (`condition_cases.rs` is at the 400-line cap),
integration tests in `tests/manifest_jinja_tests.rs`, and rstest-bdd scenarios
in `tests/features/manifest.feature` ("Generating targets with foreach",
"Generating actions with foreach and when", "Parsing fails when a foreach
expression is not iterable"). Steps live in `tests/bdd/steps/manifest/`, and
include `the manifest has {n} targets`, `the manifest has {n} actions`,
`the target {i} name is {s}`, `the action {i} name is {s}`,
`parsing the manifest fails`, and `the error message contains {text}`
(`tests/bdd/steps/manifest/mod.rs`, line 223, which reads the whole error
chain).

Two modules already hold generated tests:

- `src/manifest/expand/tests/property_cases.rs`: entry counts, sequential
  indexes, `description` preservation, `foreach` and `when` key removal for
  foreach entries, determinism of filtering, the filtering report, and
  rejection of whitespace-only conditions.
- `src/manifest/expand/tests/foreach/property_cases.rs`: cardinality-budget
  termination, integer `item`/`index` injection, collision override, threshold
  filtering, and `foreach_preserves_generated_source_key_order`, which permutes
  three keys but always writes `foreach` last.

Neither module path contains a segment ending in `property_tests`, so
`make proptest` (`Makefile`, `PROPTEST_FILTER`) does not select them;
`make test` runs them.

Unhappy paths already covered elsewhere: a non-iterable `foreach`
(`tests/features/manifest.feature`, "Parsing fails when a foreach expression is
not iterable"; `tests/manifest_jinja_tests.rs::foreach_non_iterable_errors`); a
non-object `vars` on a foreach entry
(`tests/manifest_jinja_tests.rs::foreach_vars_must_be_mapping`); and empty,
whitespace-only, and malformed conditions
(`src/manifest/expand/tests/condition_cases.rs`). Not covered anywhere: a
non-string `when`, and an undefined name in a condition under the production
strict-undefined environment.

### Reusable test support

`src/shell_selection/selection/property_tests.rs` (line 12) already defines
`json_values()`, a `prop_recursive` strategy over all six JSON kinds with
bounded containers, and `json_kind()`, a classifier. Its string leaf comes from
`crate::shell_selection::name::property_tests::arbitrary_names()`. The helpers
`targets`, `actions`, `section_entries`, and `indexes` in
`src/manifest/expand/tests/mod.rs` are `pub(super)`, so any descendant of
`manifest::expand` can use them.

### The mutation-evidence contract

`docs/verification/mutations/` holds unified-diff patches, each seeding one
fault. `tests/kani_mutation_evidence_tests/contracts.rs` keeps
`SUPPLEMENTAL_PROPERTY_PATCHES`, the registry of Proptest-only patch stems; it
is grouped by module family, not sorted, and its doc comment explains each
family. A stem is the property's module path with `::` written as `__`,
followed by the property's function name. `supplemental_property_location`
resolves
`manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order`
to `src/manifest/expand/property_tests/order.rs` (or `order/mod.rs`) and
requires that file to declare `fn when_removal_keeps_authoring_key_order(` on a
line of its own, which a `#[test]` function inside `proptest! { … }` satisfies.
`every_patch_matches_a_harness` rejects orphan patches and
`every_patch_applies_cleanly` runs `git apply --check` on each; both run under
`make test`. `make test-kani-mutations` builds every patched tree with `--lib`
under `-D warnings`, so a patch that leaves a variable unused fails.

### Terms used in this plan

- *Control key*: `foreach` or `when`.
- *Non-control field*: any entry key other than a control key and other than
  `vars`, which expansion edits. `conditional` is an ordinary `Target` field
  (`pub conditional: bool`) and counts as non-control; expansion only adds it
  in query mode, which is outside this plan's domain.
- *Oracle*: a test-side function that computes the expected result by a method
  structurally different from production.
- *Mutation patch*: a committed diff that seeds a plausible fault so that a
  named property can be shown to detect it.
- *Failure set*: every test that fails when one mutation patch is applied.
- *Coverage classification*: per-strategy counts, gathered over a fixed-seed
  run, of how often each material input class is generated; used to show that a
  generator is not vacuous.

## Conformance basis

Upstream artefacts, at `origin/main` commit `08d0a918d`:

- `docs/roadmap.md`, item 4.3.2, sub-items `RM-4.3.2.a` (foreach preserves
  non-control fields), `RM-4.3.2.b` (`when` removed after evaluation),
  `RM-4.3.2.c` (`item` and `index` injected correctly), and `RM-4.3.2.d`
  (static targets honour `when`).
- `docs/formal-verification-methods-in-netsuke.md`, *Proptest for determinism
  and manifest semantics*: `FV-EXP`, the bulleted invariant list, including
  "key order is preserved where the implementation intends to preserve it". The
  rendering-idempotence bullet belongs to 4.3.3 and is out of scope.
- `docs/netsuke-design.md`: `DES-STAGE3` (stage 3, variable precedence
  `globals < entry.vars < per-iteration locals`), `DES-FOREACH` (section 2.5:
  `item`, zero-based `index`, filtered entries absent, no `foreach` or `when`
  downstream), and `DES-ORDER` (the `ManifestMap` paragraph promising authoring
  order after foreach expansion).
- `docs/developers-guide.md`, `expand_foreach` interface reference:
  `DG-EXPAND`.
- `docs/users-guide.md`, *Generate targets with `foreach` and `when`*:
  `UG-FOREACH` ("iteration values take precedence over both"; "Top-level
  actions support the same `foreach` and `when` keys").
- ADR-0003 (scope of `foreach` and `when`), ADR-0050 (manifest budget),
  ADR-0047 (bounds of Kani for heap-heavy structures), ADR-0030 (the
  contract-record model for 4.3.1).

No Terms of Reference document exists for this phase; the roadmap and the
formal-verification survey stand in for it. This plan writes no new ADR; the
expansion contract is recorded as numbered clauses `C-1` to `C-5` in a new
subsection of `docs/netsuke-design.md` section 2.5 (see `Decision log`).

Trace links:

```plaintext
FV-EXP -> DES-ORDER -> C-2 -> EP-M1
  -> manifest::expand::tests::key_order_cases (rstest regression)
  -> manifest.feature "Parsing names the first unknown field in authoring order"
FV-EXP -> DES-ORDER -> C-2 -> EP-M3
  -> manifest::expand::property_tests::order::foreach_removal_keeps_authoring_key_order
  -> manifest::expand::property_tests::order::when_removal_keeps_authoring_key_order
RM-4.3.2.a -> DES-FOREACH -> C-1 -> EP-M3
  -> manifest::expand::property_tests::preserve::expansion_copies_every_non_control_field
RM-4.3.2.b -> DES-FOREACH -> DG-EXPAND -> C-4 -> EP-M3
  -> manifest::expand::property_tests::when_removed::no_expanded_entry_carries_a_control_key
  -> manifest::expand::tests::when_type_cases (rstest, unhappy paths)
RM-4.3.2.c -> DES-STAGE3 -> UG-FOREACH -> C-3 -> EP-M3
  -> manifest::expand::property_tests::inject::each_expansion_carries_its_own_item_and_index
RM-4.3.2.d -> DES-FOREACH -> ADR-0003 -> C-4 -> EP-M3
  -> manifest::expand::property_tests::static_when::static_entries_survive_exactly_when_their_condition_holds
RM-4.3.2.d -> DES-FOREACH -> C-4 -> EP-M4
  -> manifest.feature "Static targets and actions honour when"
  -> manifest.feature "Parsing fails when a when condition is not a string"
FV-EXP -> EP-M3
  -> manifest::expand::property_tests::coverage::generators_reach_every_material_class
```

## Constraints

- Production code may change only at the two key-removal sites:
  `src/manifest/expand/mod.rs` (`entry.remove("foreach")`) and
  `src/manifest/expand/evaluation.rs` (`map.remove("when")`). Both become
  `shift_remove`. Any other production change is a tolerance breach.
- No item may widen visibility beyond its current level, except the lifted
  JSON strategy (a `#[cfg(test)]`, `pub(crate)` test-support item). Inside the
  suite, nested modules use `pub(in crate::manifest::expand::property_tests)`
  for items that sibling modules need; plain `pub(super)` in a nested file
  would hide the item from its cousins.
- No new dependency or feature. `proptest` 1.11.0, `googletest` 0.14.3,
  `pretty_assertions` 1.4.1, `rstest` 0.26.1, and `rstest-bdd` 0.6.0 are
  already dev-dependencies.
- No in-process environment mutation. Nothing calls `std::env::set_var` or
  `remove_var`. Behavioural scenarios run through the existing manifest steps.
- Generated conditions must not call `env()`, `glob()`, `command_available()`,
  `which`, or any other helper that touches the filesystem or process state,
  and must never produce `Conditional`. In hexagonal terms, expansion is a pure
  transform of a value; the Jinja environment is its evaluation port, and those
  helpers are adapters that this suite deliberately keeps out of its domain.
- Key order is asserted only by comparing key sequences
  (`keys().collect::<Vec<_>>()`) at the entry level and the `vars` level. Value
  equality (`==`) is used only where order is irrelevant by contract.
- The suite's module path must contain a segment ending in `property_tests`, or
  `make proptest` will not select it.
- Every file stays under 400 lines; every module starts with `//!`; every
  helper function carries a `///` summary; test functions carry none.
- Property bodies use `prop_assert*` and return `TestCaseError`, never
  `panic!`, `unwrap`, or `expect` on a generated value. No `prop_filter` or
  `prop_assume!` on a structural condition; generators construct valid values.
- Each obligation gets a structurally independent oracle. An oracle that calls
  `when_allows`, `when_context`, `inject_iteration_vars`, `Map::remove`, or
  `Map::shift_remove` is a re-implementation and is forbidden.
- Red-Green-Refactor applies to every behaviour change (EP-M1 and the new
  behavioural scenarios). Property-only work runs against production code that
  is already correct, so it cannot go red against production; its red stage is
  the mutation substitute defined in `Decision log`: write the patch, apply it,
  write the property and watch it fail for the intended reason, revert, watch
  it pass.
- Do not delete readable specification. The existing generated tests move into
  the suite unchanged except for imports; the one exception, the vacuous
  key-order property, is retired only because `OBL-ORDER` strictly contains its
  domain (see `Decision log`).
- Each case count is pinned explicitly in `ProptestConfig`, so the workload
  does not depend on an ambient `PROPTEST_CASES`, following
  `src/stdlib/canonical_value/tests/properties.rs`.
- Do not add a `-Z` flag, a `-Znext-solver` directive, or a defensive
  `.clone()` to satisfy the borrow checker without first compiling the natural
  form.
- ExecPlan `Status:` stays one bare value from the closed set.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 50 non-ExecPlan
  files or more than 3,500 net lines of code and other documentation. This
  ExecPlan file is excluded from both counts. The planned set is about 42 files
  (see *Files to create* and *Files to edit*); file moves count once.
- Production change: stop if any production edit beyond the two
  `shift_remove` calls is needed, including any change prompted by a property
  failure other than key order. Record the counter-example first.
- Interface: stop if any production item's visibility must widen.
- Dependencies: stop if any new crate or feature is needed.
- Fidelity: stop if EP-M0 shows that an `item` is not deep-equal, by
  order-insensitive `serde_json` equality and, for floats, by `to_bits()`, to
  the source value after expansion, for any value the YAML loader can produce.
  That would be a defect candidate, not a generator restriction to apply
  quietly. Sorted keys inside an object `item` are expected and are not a
  breach.
- Runtime: the new suite should add no more than 5 s of wall time to the
  `make proptest` run (advisory, given the shared host); stop at 15 s.
- Shrinking: stop if any property's shrink exceeds 30 s on a seeded failure.
- Rejection: stop if a strategy needs filtering to reach valid inputs.
- Assertion shape: stop if a property can only be stated as a disjunction of
  observed behaviours.
- Iterations: stop if a gate still fails after three focused fix attempts.
- Contract test: stop if `tests/kani_mutation_evidence_tests` rejects a stem
  or patch for a reason other than a typo.
- Ambiguity: stop if a contract clause would have to choose between two
  readings of the design document or users' guide that change what a user sees.

## Risks

- Risk: a condition oracle disagrees with MiniJinja on an edge case.
  Severity: medium. Likelihood: low after simplification. Mitigation:
  conditions come from a closed set of shapes (see *Shared generators and
  oracles*); EP-M0 runs every shape against MiniJinja on 1,000 generated
  bindings before any property is written; comparison literals stay small so
  that comparisons against `index` are not constant.
- Risk: the template form (`{{ … }}`) renders a non-boolean and the fallback
  reads it as false while the oracle says true. Severity: medium. Likelihood:
  eliminated by design: only boolean-valued shapes get a template form, and the
  integer-truthiness shape never does.
- Risk: `item` fidelity fails for an extreme value (`u64` above `i64::MAX`,
  `-0.0`, sub-normal floats) on the inline-array path or the expression path.
  Severity: high (a real defect). Likelihood: low; the property-testing
  reviewer read the MiniJinja 2.24.0 source and predicts bit-exact survival.
  Mitigation: EP-M0 question 4; the fidelity tolerance.
- Risk: fixing key order changes an expectation in another test. Severity:
  low. Likelihood: low; no snapshot records an expanded document. Mitigation:
  EP-M1 runs the whole suite after the fix; any changed expectation is
  reviewed, not silently updated.
- Risk: a mutation patch compiles under the focused run but fails
  `make test-kani-mutations` because it leaves a binding unused. Severity:
  medium. Likelihood: low now that the patch text is fixed in this plan.
  Mitigation: run `make test-kani-mutations` at EP-M3, not only at closure.
- Risk: lifting `json_values()` changes the shell-selection suite's behaviour.
  Severity: low. Likelihood: low. Mitigation: the lift keeps the strategy
  byte-for-byte, parameterizes only its string leaf, and runs that suite before
  and after in its own refactor commit.
- Risk: moving the existing `property_cases` modules trips the HashMap
  boundary scan (`tests/hashmap_boundary/`), which looks for
  `#[cfg(test)] mod property_tests;` declarations. Severity: low. Likelihood:
  low. Mitigation: declare the suite exactly that way in
  `src/manifest/expand/mod.rs`; EP-M2 runs `make test`.
- Risk: gate time on the shared host (lint can approach the two-hour detached
  cap). Severity: medium. Likelihood: high. Mitigation: four implementation
  plateaus rather than seven; focused `nextest -E` runs inside a milestone; the
  full gate set once per plateau.
- Risk: other `serde_json::Map::remove` call sites outside the manifest
  pipeline share the swap behaviour (for example
  `src/cli/discovery/project_policy.rs`, lines 188, 253, and 273–276).
  Severity: low. Likelihood: certain. Mitigation: EP-M1 inventories every
  `remove` on a `serde_json::Map` in `src/`, records whether each site is
  order-sensitive, and leaves order-insensitive sites alone. A follow-up issue
  can propose a Clippy `disallowed-methods` guard; this plan does not add one.
- Risk: ADR-0003 says `foreach` and `when` are scoped to "manifest targets",
  while the design document, developers' guide, and users' guide all state that
  top-level `actions` support them too. Severity: low. Likelihood: certain.
  Mitigation: the code expands both sections; this plan covers both and records
  the reading ("targets" means top-level entries, as opposed to keys nested
  inside a field) in the new design-document subsection without amending
  ADR-0003.

## Verification plan

### Method selection

The obligations range over an open-ended input space (arbitrary field maps,
arbitrary JSON values, control-key positions, condition values), so Proptest is
the primary method. Named rstest cases pin the key-order regression, the
condition-shape witnesses, and the unhappy paths. Three rstest-bdd scenarios
cover the behaviour a user can observe through the loader. No insta snapshot is
added: the only format at stake is key order, which the rstest and properties
assert directly on key sequences, and a snapshot would restate them.

Kani is rejected: expansion runs MiniJinja's parser and evaluator and allocates
`serde_json` maps, and ADR-0047 already found the bounded model checker
impractical for far smaller heap structures. Verus is rejected: the plan
introduces no lemma or contractual algorithm. The fix is a call to a library
function whose contract is an axiom below, and the remaining claims concern a
composition of library calls rather than a pure kernel. A Verus model would
prove the model, not Netsuke.

### Axioms

- `AXIOM-PROPTEST`: Proptest 1.11.0 generates and shrinks as documented, and
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1` disables seed persistence.
- `AXIOM-SHIFT`: under `preserve_order`, `serde_json::Map::shift_remove`
  removes a key and preserves the relative order of the rest; `Map::insert` of
  a new key appends it, and insertion of an existing key keeps its position
  (`serde_json` 1.0.151, `src/map.rs`).
- `AXIOM-MAP-EQ`: `serde_json::Map` equality ignores key order (`src/map.rs`,
  lines 409–413). EP-M1 includes an rstest case demonstrating this, so the
  constraint on key-sequence assertions is grounded in evidence.
- `AXIOM-MJ-TRUTH`: MiniJinja 2.24.0 `Value::is_true` treats `false`, zero,
  `none`, undefined, and empty containers as false and everything else as true;
  integer comparisons behave as in Rust for small operands.
- `AXIOM-MJ-SERDE`: MiniJinja `Value::from_serialize` preserves sequence
  order and stores maps as `BTreeMap`, so object keys come back sorted. Whether
  every JSON value round-trips to a deep-equal value through
  `serde_json::to_value` is measured in EP-M0, not assumed.
- `AXIOM-SAPHYR`: expansion-level properties build `ManifestValue` directly.
  The behavioural scenarios parse real YAML, so the YAML-to-JSON mapping is
  exercised end to end there.

### Shared generators and oracles

Generators live in `src/manifest/expand/property_tests/strategy/`. They produce
an `EntrySpec` model, which renders to a `ManifestValue`, so oracles read the
model rather than the rendered map.

- `EntrySpec.section`: `targets` or `actions`.
- `EntrySpec.fields`: an ordered list of up to six non-control fields. Keys mix
  real `Target` field names (`name`, `command`, `description`, `sources`,
  `deps`, `order_only_deps`, `after`, `phony`, `always`, `rule`, `script`,
  `conditional`) with generated `[a-z][a-z0-9_]{0,7}` names. A generated name
  equal to `foreach`, `when`, or `vars` gains a `field_` prefix, and duplicates
  keep their first occurrence; both are transforms, not filters. Values come
  from the lifted `json_values()` strategy plus extra leaves (`i64::MIN`,
  `i64::MAX`, `u64` values above `i64::MAX`, and strings that look like Jinja,
  such as `{{ item }}`, which expansion must not render).
- `EntrySpec.vars`: absent, or an ordered object of up to four entries. Keys
  are drawn from the condition pool `w0`–`w3` (so conditions can reference
  them) plus, for foreach entries only, user keys named `item` and `index` to
  exercise override.
- `EntrySpec.foreach`: absent (a static entry); an inline array of up to eight
  items; or, for the same items, a string holding a Jinja list literal printed
  by a test-side printer that writes JSON with `null` spelled `none`. Items are
  either arbitrary JSON (with conditions over `index` only) or integers in
  `-2..10` (with conditions over `item` and `index`).
- `EntrySpec.when`: absent, or a `Condition` (below) in expression or template
  form.
- `EntrySpec.layout`: the insertion position of each control key and of `vars`
  among the fields, drawn with `prop::sample::Index` so that the position
  shrinks well and survives deduplication changing the field count. First,
  middle, and last positions are all reachable.

A `DocumentSpec` holds up to six entries per section. The bindings a condition
sees (`Bindings`) are modelled explicitly: the entry's `vars` values, then, for
a foreach entry, `item` and `index` overriding them. Static entries never
reference `item` or `index`.

`Condition` (`strategy/condition.rs`) is a closed set of shapes, not a
recursive grammar:

- `Literal(b)` renders `true` or `false`;
- `Flag(w)` renders `w0` for a boolean `vars` key;
- `NotFlag(w)` renders `not w0`;
- `Count(w)` renders `w1` for an integer `vars` key, testing integer
  truthiness (zero is false);
- `VarCmp(w, op, k)` renders, for example, `w1 < 3`;
- `IndexCmp(op, k)` renders, for example, `index >= 2` (foreach only);
- `ItemCmp(op, k)` renders, for example, `item != 4` (integer items only).

`op` ranges over `<`, `<=`, `==`, `!=`, `>`, and `>=`, and `k` over `-2..10`.
Every shape except `Count` also has a template form `{{ … }}`, whose rendering
is always `true` or `false`. Variable names come from the pool `w0`–`w3`, which
cannot collide with a MiniJinja keyword (`and`, `or`, `not`, `in`, `is`, `if`,
`none`, `true`, `loop`, `self`). The generator only emits a reference to a name
that the entry's bindings contain, and the coverage test checks that
structurally, because MiniJinja's short-circuiting would otherwise hide an
undefined name. The oracle `Condition::holds(&Bindings) -> bool` evaluates the
shape in Rust and shares no code with MiniJinja.

The expansion oracle (`property_tests/model.rs`) computes, for an `EntrySpec`,
the expected output list. For each item index `i` (or once, for a static entry)
it decides inclusion with `Condition::holds`, then builds the expected map by
walking the source keys in order, skipping control keys, copying fields
verbatim, and building the expected `vars` as the source `vars` with `item` and
`index` overwritten in place or appended, for foreach entries only. If a
foreach entry has no `vars`, the expected map ends with `vars` holding `item`
then `index`. The oracle never calls a `Map` removal method.

### Obligations

**OBL-PRESERVE** (`RM-4.3.2.a`, `C-1`). *Expansion copies every non-control
field verbatim.*

- Statement: for every generated document and every output entry derived from
  source entry `E`, each non-control field of `E` is present with a deep-equal
  value, no other non-control field is present, and every `vars` key of `E`
  other than `item` and `index` keeps its value. Static entries keep `vars`
  unchanged; no `item` or `index` is injected.
- Method: Proptest, 128 cases.
- Artefact:
  `src/manifest/expand/property_tests/preserve.rs::expansion_copies_every_non_control_field`.
- Oracle: the model's expected map, with field values compared by `==`
  (nested order inside a field value is copied verbatim by `clone`, and no code
  path rewrites it).
- Non-vacuity: classification requires static and foreach entries in both
  sections, field values of every JSON kind including nesting depth two,
  Jinja-looking strings, `vars` present and absent, and user `vars` other than
  `item` and `index`.
- Mutation `MUT-PRESERVE` (realistic fault: rebuilding the entry from known
  fields). In `expand_foreach_item`, replace

  ```rust
  entry.shift_remove("foreach");
  ```

  with

  ```rust
  entry.retain(|key, _| matches!(key.as_str(), "name" | "command" | "vars" | "when"));
  ```

  Expected failure set: this property and
  `order::foreach_removal_keeps_authoring_key_order`.

**OBL-ORDER** (`FV-EXP`, `DES-ORDER`, `C-2`). *Expansion keeps authoring key
order.*

- Statement: for every output entry, its entry-level key sequence equals the
  source key sequence with control keys deleted; for a foreach entry whose
  source has no `vars`, `vars` follows; for a foreach entry, the `vars` key
  sequence equals the source `vars` sequence with `item` and `index` kept in
  place if present and otherwise appended in that order. Static entries gain no
  keys.
- Method: rstest regression cases first (EP-M1), then two Proptest properties,
  128 cases each, one per removal site.
- Artefacts: `src/manifest/expand/tests/key_order_cases.rs` (rstest, every
  position of `when` in a static entry and of `foreach` in a foreach entry,
  with four non-control fields, in both sections);
  `src/manifest/expand/property_tests/order.rs::foreach_removal_keeps_authoring_key_order`
  (foreach entries without `when`) and
  `…::when_removal_keeps_authoring_key_order` (static entries with `when`).
- Evidence: the rstest cases with two or more fields after the control key
  fail on `origin/main` (red) and pass after `shift_remove` (green); cases with
  zero or one following field pass before and after, and are kept as boundary
  witnesses.
- Non-vacuity: classification requires, at a frequency of at least 5% of
  generated entries per strategy, the disjoint classes "control key followed by
  two or more keys" (the shape `swap_remove` corrupts), "followed by exactly
  one key", and "followed by none".
- Mutation `MUT-ORDER-FOREACH` restores `entry.remove("foreach")`; expected
  failure set: `order::foreach_removal_keeps_authoring_key_order` and the
  `key_order_cases` foreach cases. Mutation `MUT-ORDER-WHEN` restores
  `map.remove("when")`; expected failure set:
  `order::when_removal_keeps_authoring_key_order` and the `key_order_cases`
  static cases.

**OBL-WHEN-REMOVED** (`RM-4.3.2.b`, `C-4`). *No output entry carries a control
key, and only included entries survive.*

- Statement: for every generated document mixing static and foreach entries in
  both sections, every output object lacks `when` and `foreach`, and the number
  of outputs per section equals the oracle's count.
- Method: Proptest, 128 cases.
- Artefact:
  `src/manifest/expand/property_tests/when_removed.rs::no_expanded_entry_carries_a_control_key`.
- Non-vacuity: classification requires, per section, entries whose condition
  held, entries whose condition failed, and entries with no condition.
- Mutation `MUT-WHEN-KEEP` changes the first line of `when_allows` from
  `let Some(when_val) = map.shift_remove("when") else {` to
  `let Some(when_val) = map.get("when").cloned() else {`. Expected failure set:
  this property, `preserve::expansion_copies_every_non_control_field`,
  `order::when_removal_keeps_authoring_key_order`, and the existing removal
  checks in the relocated generated tests.

**OBL-INJECT** (`RM-4.3.2.c`, `C-3`). *Each generated entry carries its own
`item` and `index`.*

- Statement: for a foreach entry over items `x₀ … xₙ₋₁` and condition `P`,
  written as an inline array or as the equivalent list-literal string, the
  outputs are exactly the entries for `i` in ascending order such that
  `P(xᵢ, i)` holds; output `k` has `vars.item` deep-equal to `xᵢ` (floats also
  equal by `to_bits()`) and `vars.index` equal to `i` as a `u64`, overriding
  any user `item` or `index` while keeping other user `vars`.
- Method: Proptest, 128 cases, over the two item domains and both `foreach`
  forms.
- Artefact:
  `src/manifest/expand/property_tests/inject.rs::each_expansion_carries_its_own_item_and_index`.
- Non-vacuity: classification requires empty `foreach`, all-excluded,
  all-included, and the class "some excluded item precedes an included one"
  (the only shape on which renumbering is visible, because a prefix-shaped
  selection such as `index < k` keeps renumbered and true indexes equal); every
  JSON item kind, including top-level `null` items in the string form; and user
  `vars` colliding on `item` and on `index`.
- Mutation `MUT-INDEX` (realistic fault: numbering kept entries rather than
  iteration positions). In `expand_foreach_target`, change

  ```rust
  for (index, item) in iter.enumerate() {
      let foreach_item = ForeachItem {
          source: map,
          item: &item,
          index,
      };
  ```

  to

  ```rust
  for item in iter {
      let foreach_item = ForeachItem {
          source: map,
          item: &item,
          index: expanded.len(),
      };
  ```

  The renumbered index reaches the cardinality check, the condition, and the
  injected `vars`. Expected failure set: this property, the relocated
  threshold-filtering and budget properties, and any example test that filters
  before an included item.

**OBL-STATIC-WHEN** (`RM-4.3.2.d`, `C-4`). *A static entry survives exactly
when its condition holds, and the report counts every exclusion.*

- Statement: for every document of static entries in both sections, an entry
  is in the output if and only if it has no condition or its `Condition` holds
  over its own `vars`; `report.stats.filtered_targets` and `filtered_actions`
  equal the oracle's exclusion counts; and every retained `FilteredEntry` for a
  static entry has `iteration_index == None`.
- Method: Proptest, 128 cases, with both condition forms.
- Artefact:
  `src/manifest/expand/property_tests/static_when.rs::static_entries_survive_exactly_when_their_condition_holds`.
- Non-vacuity: classification requires held and failed conditions in each form
  and each section, and conditions reading `vars`.
- Mutation `MUT-STATIC-LEAK` (realistic fault: an excluded entry leaking
  through). In `expand_static_target`, change the `Exclude` arm's
  `Ok(Vec::new())` to `Ok(vec![ManifestValue::Object(map)])`, keeping the
  `record_filtered_entry` call so that `event` stays used. Expected failure
  set: this property and
  `when_removed::no_expanded_entry_carries_a_control_key` (the counts).

**OBL-GEN**. *Each generator reaches every class its properties depend on.*

- Statement: for each strategy that a property uses, a fixed-seed run of 512
  draws classifies every draw, and each class listed in the non-vacuity entries
  above reaches at least 5% of draws. At that frequency, the chance that a
  128-case random-seed run of the property never meets a class is below 0.2%.
  Every condition's free names are contained in its bindings.
- Method: a `TestRunner` built with
  `ProptestConfig { rng_seed: RngSeed::Fixed(COVERAGE_SEED), .. }`, following
  the `coverage_runner` in `src/stdlib/canonical_value/tests/properties.rs`.
  `TestRunner::new` alone seeds from the operating system and would not be
  reproducible.
- Artefact:
  `src/manifest/expand/property_tests/coverage.rs::generators_reach_every_material_class`.
- Evidence: the test prints per-class counts under `--no-capture` and fails if
  any class falls below threshold.
- Non-vacuity: a negative control in the same module,
  `the_threshold_check_reports_an_unreached_class`, feeds the same threshold
  check a strategy whose layout is forced to the last position and asserts that
  the check returns an error naming the "two or more following keys" class.

### Residual gaps

- Query mode (`Conditional` and the synthetic `conditional: true`) is outside
  the domain; `src/manifest/expand/tests/action_condition_cases.rs` covers it.
- Error-path document state is outside the contract: `expand_section` takes
  the section array with `std::mem::take`, so a failed expansion leaves that
  section empty in the caller's document. The loader discards the document on
  error, so this is not user-visible.
- A static entry whose `vars` is not an object gets an empty condition context
  rather than an error. This plan records the behaviour in clause `C-4` and
  does not change it; changing it would be a production change.
- `foreach` expressions over globals, `glob()`, and stdlib calls are not
  generated.
- Sorted keys inside an object `item` are evaluator behaviour, recorded in
  `C-3`, not tested as an order property.
- MiniJinja and `serde_json` internals are trusted through the axioms.

## Plan of work

Stage A (EP-M0) answers the open questions with throwaway probes. Stage B
(EP-M1) proves the key-order defect red at unit and behavioural level and fixes
it. Stage C (EP-M2 and EP-M3) lifts the shared JSON strategy, relocates the
existing generated tests into the new suite, and adds the generators, oracles,
coverage test, properties, and mutation patches. Stage D (EP-M4) adds the
unhappy-path rstest table, the remaining behavioural scenarios, the
documentation, and the roadmap update, then runs the closure gates.

### Files to create

```plaintext
src/json_value_strategy_test_support.rs
src/manifest/expand/tests/key_order_cases.rs
src/manifest/expand/tests/when_type_cases.rs
src/manifest/expand/property_tests/mod.rs
src/manifest/expand/property_tests/support.rs
src/manifest/expand/property_tests/model.rs
src/manifest/expand/property_tests/strategy/mod.rs
src/manifest/expand/property_tests/strategy/condition.rs
src/manifest/expand/property_tests/coverage.rs
src/manifest/expand/property_tests/preserve.rs
src/manifest/expand/property_tests/order.rs
src/manifest/expand/property_tests/when_removed.rs
src/manifest/expand/property_tests/inject.rs
src/manifest/expand/property_tests/static_when.rs
tests/data/when_middle_unknown_field.yml
tests/data/static_when.yml
tests/data/when_not_string.yml
docs/verification/mutations/
  manifest__expand__property_tests__preserve__expansion_copies_every_non_control_field.patch
  manifest__expand__property_tests__order__foreach_removal_keeps_authoring_key_order.patch
  manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order.patch
  manifest__expand__property_tests__when_removed__no_expanded_entry_carries_a_control_key.patch
  manifest__expand__property_tests__inject__each_expansion_carries_its_own_item_and_index.patch
  manifest__expand__property_tests__static_when__static_entries_survive_exactly_when_their_condition_holds.patch
```

### Files to move

```plaintext
src/manifest/expand/tests/property_cases.rs
  -> src/manifest/expand/property_tests/expansion_report.rs
src/manifest/expand/tests/foreach/property_cases.rs
  -> src/manifest/expand/property_tests/foreach_sequences.rs
```

### Files to edit

- `src/manifest/expand/mod.rs`: `shift_remove` for `foreach`; declare
  `#[cfg(test)] mod property_tests;` beside `mod tests;`.
- `src/manifest/expand/evaluation.rs`: `shift_remove` for `when`.
- `src/manifest/expand/tests/mod.rs` and
  `src/manifest/expand/tests/foreach/mod.rs`: declare the new rstest modules;
  drop the moved `property_cases` declarations.
- `src/lib.rs`: declare `#[cfg(test)] mod json_value_strategy_test_support;`
  beside `snapshot_test_support`.
- `src/shell_selection/selection/property_tests.rs`: import the lifted
  strategy instead of its local copy.
- `tests/kani_mutation_evidence_tests/contracts.rs`: append the six stems as a
  `manifest__expand` group and add one sentence to the doc comment above
  `SUPPLEMENTAL_PROPERTY_PATCHES` explaining why that family has no Kani
  harness.
- `tests/features/manifest.feature`: three scenarios (below).
- `docs/netsuke-design.md`: a subsection *Expansion invariants* in section
  2.5 stating clauses `C-1` to `C-5`, the ADR-0003 reading, and the
  test-artefact locations; amend the `ManifestMap` paragraph to cite clause
  `C-2`.
- `docs/developers-guide.md`: a *Manifest expansion suites* subsection under
  *Property-based testing with proptest*, mirroring *Emission determinism
  suites*, including the scope and reuse policy of the lifted JSON strategy;
  correct the `expand_foreach` bullet "entries without `foreach` are passed
  through unchanged" to say that static entries lose `when` and keep every
  other key in order.
- `docs/formal-verification-methods-in-netsuke.md`: state that the four
  expansion invariants and key order now exist, and where.
- `docs/repository-layout.md`: entries for
  `src/manifest/expand/property_tests/` and
  `src/json_value_strategy_test_support.rs` with their test-only scope.
- `docs/contents.md`: index this ExecPlan.
- `docs/users-guide.md`: one prose sentence (no new fenced example, so no
  `tested-example` marker is needed) stating that expansion keeps fields in
  authoring order, so an unknown-field error names the first unknown field as
  written.
- `CHANGELOG.md`: a `### Fixed` entry under `## Unreleased`, worded narrowly:
  when an entry carrying `when` or `foreach` also has two or more unknown
  fields after it, the manifest error now names the first unknown field as
  written rather than a later one.
- `Makefile`: rewrite the census sentence above `PROPTEST_FILTER` to name the
  measuring command rather than a count.
- `docs/roadmap.md`: mark 4.3.2 and its sub-items done with a completion
  note, following the 4.3.1 entry's shape.

The regenerated `typos.toml` that `make spelling` produced on this branch is
unrelated to this item. It is committed alone as the branch's first commit, so
it can be offered as a separate base pull request with this work stacked on it,
as `AGENTS.md` requires.

## Milestones and plateaus

### EP-M0 — feasibility probes (prototyping)

Outcome: answers to these questions, recorded in `Surprises & discoveries` and
`Decision log`, with every probe deleted afterwards.

1. Does a scratch rstest show `name, when, command, after` expanding to
   `name, after, command` on the current tree? (Expected yes.)
2. Through `manifest::from_str_with_env`, does a target written
   `name, when: "true", zz_first: 1, command, zz_last: 2` fail with a message
   containing `zz_last` before the fix? Repeat for an action. (Expected yes, by
   the codebase review's reading of serde's flatten path.)
3. For each `Condition` shape and form, does MiniJinja's verdict match
   `Condition::holds` on 1,000 generated bindings, and does every template form
   take the fallback path?
4. Is `vars.item` deep-equal to the source item, with floats also compared by
   `to_bits()`, for 1,000 generated JSON values and for `i64::MIN`, `i64::MAX`,
   `u64::MAX`, `-0.0`, `f64::MIN_POSITIVE`, and `5e-324`, on both the
   inline-array path and the list-literal string path? Which of those can
   `serde_saphyr` produce from YAML text?
5. What is the per-case wall time of an expansion-level property? Confirm 128
   cases keeps the suite within the runtime tolerance.
6. Is there a reachable production constructor for the expansion
   environment? If not, `support::strict_environment()` builds
   `Environment::new()` with `UndefinedBehavior::Strict`, and the behavioural
   scenarios are the check on the production environment.

Requirements and gaps: none discharged; de-risks every obligation. Acceptance:
each answer recorded with the command and output that produced it. Conformance
check:

- No upstream requirement changes.
- The design is unchanged unless question 4 fails, which stops the plan.
- No probe is committed.

Recovery: probes live in a scratch module that `git checkout --` removes.
Remaining gaps: everything else. Compatibility decision: none.

### EP-M1 — key-order fix

Outcome: `key_order_cases.rs` and the key-order behavioural scenario committed
red-then-green; both removal sites use `shift_remove`; the inventory of other
`serde_json::Map::remove` sites recorded in `Decision log`; the `CHANGELOG.md`
entry and the users' guide sentence written; the design document's clause `C-2`
drafted.

Red-Green-Refactor:

- Red: add `key_order_cases.rs`, an rstest table over every position of
  `when` (static entry) and `foreach` (foreach entry) among four non-control
  fields in both sections, asserting the key sequence with googletest
  `assert_that!(keys, elements_are![…])` and, for the `vars` sequence,
  `pretty_assertions::assert_eq!` on `Vec<&str>`. Add an rstest case that shows
  two maps with swapped keys compare equal (`AXIOM-MAP-EQ`). Add
  `tests/data/when_middle_unknown_field.yml` and this scenario:

  ```gherkin
  Scenario: Parsing names the first unknown field in authoring order
    Given the manifest file "tests/data/when_middle_unknown_field.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails
    And the error message contains "unknown field `zz_first`"
  ```

  Run both; see the cases with two or more following fields fail with the
  swapped order, and the scenario fail because the message names `zz_last`.
- Green: change the two `remove` calls to `shift_remove`. Run both again and
  see every case and the scenario pass.
- Refactor: none expected; run the whole expansion test tree.

Acceptance: the focused commands in `Concrete steps` show the red failures,
then the passes; the full gate set passes (see `Validation and acceptance`).
Conformance check:

- `DES-ORDER` and clause `C-2` hold on the tested shapes.
- Only the two permitted production lines changed.
- No interface, dependency, or persisted-format change.
- Trace link `FV-EXP -> DES-ORDER -> C-2 -> EP-M1` current.

Recovery: revert the milestone's commits; nothing else depends on them yet.
Remaining gaps: generated coverage of every obligation. Compatibility decision:
none. Key order inside the in-memory manifest value is neither a persisted nor
a wire format, and the only user-visible effect is a diagnostic's choice of
field.

### EP-M2 — shared test support and relocation (refactor)

Outcome, in two atomic refactor commits:

1. `json_values()` and `json_kind()` move from
   `src/shell_selection/selection/property_tests.rs` to
   `src/json_value_strategy_test_support.rs`, unchanged except that the string
   leaf becomes a parameter. The shell-selection suite passes before and after.
2. The two `property_cases` modules move into
   `src/manifest/expand/property_tests/` as `expansion_report.rs` and
   `foreach_sequences.rs` with `git mv`, changing only their imports, so
   `make proptest` now selects them.
   `foreach_preserves_generated_source_key_order` stays until EP-M3 lands
   `OBL-ORDER`, then is retired in that milestone.

Acceptance: `make proptest` lists the relocated tests under
`manifest::expand::property_tests`; the full gate set passes. Conformance
check: no production change; no test body changed; the lifted strategy's scope
recorded in the developers' guide draft. Recovery: each commit reverts
independently. Remaining gaps: the new properties. Compatibility decision: none
(test-only surfaces).

### EP-M3 — generators, oracles, coverage, and properties

Outcome: `property_tests/{mod,support,model,coverage}.rs`, `strategy/`, and the
five property modules; condition-shape witness cases (one rstest case per
shape, operator, and form, including integer truthiness of zero); the six
mutation patches committed and registered; the vacuous key-order property
retired.

Order of work, for each obligation: write the mutation patch; write the
property; apply the patch and watch the property fail for the intended reason,
recording the shrunk input and the failure set; revert and watch it pass. Run
mutation runs with `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`. If a property
fails on unmutated code, stop: that is a production defect (see Tolerances);
commit its regression seed under `proptest-regressions/` and promote the shrunk
case to a named rstest before escalating.

Acceptance: `make proptest` passes; `make test-kani-mutations` passes; each
patch's failure set matches the expected set in `Verification plan`, or the
difference is recorded and explained; the full gate set passes. Conformance
check: trace links for `RM-4.3.2.a` to `RM-4.3.2.d` and `DES-ORDER` current; no
production change; no visibility widened. Recovery: revert the milestone
commits; EP-M2 stands on its own. Remaining gaps: unhappy paths, behavioural
scenarios, documentation. Compatibility decision: none.

### EP-M4 — unhappy paths, scenarios, documentation, and closure

Outcome:

- `when_type_cases.rs`: an rstest table over non-string `when` values (YAML
  boolean, integer, null, and list) in both sections and both entry kinds,
  asserting the error contains `Field 'when' must be a string.`; plus cases for
  an undefined name in a condition under the strict environment, asserting an
  error rather than exclusion.
- `tests/data/static_when.yml`, `tests/data/when_not_string.yml`, and two
  scenarios:

  ```gherkin
  Scenario: Static targets and actions honour when
    Given the manifest file "tests/data/static_when.yml" is parsed
    When the manifest is checked
    Then the manifest has 2 targets
    And the target 1 name is "kept-literal"
    And the target 2 name is "kept-by-vars"
    And the manifest has 1 actions
    And the action 1 name is "kept-action"

  Scenario: Parsing fails when a when condition is not a string
    Given the manifest file "tests/data/when_not_string.yml" is parsed
    When the parsing result is checked
    Then parsing the manifest fails
    And the error message contains "Field 'when' must be a string."
  ```

  `static_when.yml` holds four static targets (one with `when: "true"`, one with
  `when: "false"`, one whose `when` reads a true `vars` flag, one whose `when`
  reads a false flag) and two static actions (one kept, one dropped). These
  describe existing behaviour, so their red stage is a deliberate misstatement:
  run each scenario first with the expected count off by one, see it fail on
  the count, then correct it.
- Every documentation edit listed above; the roadmap entry marked done; this
  plan's `Status:` set to `COMPLETE` with `Outcomes & retrospective` written.

Acceptance: the closure gate set in `Validation and acceptance` passes.
Conformance check: every trace link resolves to a passing test; this ExecPlan
is indexed in `docs/contents.md` (check by set comparison, not by count); no
upstream assumption was falsified without an upstream edit. Recovery:
documentation commits are independent and individually revertible. Remaining
gaps: those listed under *Residual gaps*. Compatibility decision: none.

## Concrete steps

Run every command from the root of the checkout that holds branch
`4-3-2-manifest-expansion-invariant-proptests`. Capture long output with `tee`,
and run gates sequentially, never in parallel:

```sh
BRANCH="$(git branch --show-current)"
for gate in check-fmt typecheck lint doc-coverage markdownlint nixie test; do
  make "$gate" 2>&1 | tee "/tmp/${gate}-netsuke-${BRANCH}.out" || break
done
```

A build that prints `[build-limits] … waiting for a compile slot` is queued,
not hung; do not kill or restart it.

Focused EP-M1 red and green runs:

```sh
cargo nextest run --all-features --lib \
  -E 'test(/manifest::expand::tests::key_order_cases/)' \
  2>&1 | tee "/tmp/key-order-netsuke-${BRANCH}.out"
```

Expected red transcript shape (before `shift_remove`):

```plaintext
FAIL [ … ] netsuke manifest::expand::tests::key_order_cases::static_when_removal_keeps_order::case_2
  Value of: keys
  Expected: has elements: "name", "command", "after", "phony"
  Actual: ["name", "phony", "after", "command"]
```

Focused property and mutation runs:

```sh
make proptest 2>&1 | tee "/tmp/proptest-netsuke-${BRANCH}.out"
PATCH=docs/verification/mutations/manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order.patch
git apply "$PATCH"
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo nextest run --all-features --lib \
  -E 'test(/manifest::expand::/)' --no-fail-fast \
  2>&1 | tee "/tmp/mutation-netsuke-${BRANCH}.out"
git apply -R "$PATCH"
git status --short proptest-regressions/ src/
```

Expected: the failure set includes
`order::when_removal_keeps_authoring_key_order`, whose `minimal failing input`
shows a static entry with `when` followed by two fields; after `git apply -R`,
`git status` shows no residue.

## Validation and acceptance

Acceptance is behavioural:

1. `make proptest` selects the tests under `manifest::expand::property_tests`
   (the six properties, the coverage test and its negative control, the
   condition-shape witnesses, and the relocated generated tests) and all pass.
2. For each of the six committed patches, applying it makes the named
   property fail with a shrunk counter-example, and reverting it makes the
   property pass. Record each failure set and shrunk input in
   `Artefacts and notes`.
3. The EP-M1 rstest cases with two or more following fields and the key-order
   scenario fail before the fix and pass after, as transcribed.
4. The coverage test reports every class at or above 5%, and its negative
   control reports the forced miss.
5. The two EP-M4 scenarios pass, after a recorded deliberate-misstatement red
   run.

Quality criteria:

- Tests: `make test` passes, including `tests/kani_mutation_evidence_tests`,
  `tests/hashmap_boundary`, and `tests/execplan_status_contract_tests.rs`
  (which reads this file).
- Verification: every obligation discharged with its evidence and non-vacuity
  record; residual gaps stated.
- Formatting, lint, and types: `make check-fmt`, `make typecheck`,
  `make lint`, and `make doc-coverage` pass.
- Documentation: `make markdownlint` (which runs `make spelling`) and
  `make nixie` pass.
- Mutation compile check: `make test-kani-mutations` passes at EP-M3 and at
  closure.
- Makefile: `mbake validate Makefile` and `make test-workflow-contracts` pass
  at EP-M4, because the Python contract suites parse the `Makefile` comment
  block this plan edits.
- Performance: the suite's wall time, read from the `make proptest` log,
  stays within the runtime tolerance.

Quality method: at the end of every milestone, run `make check-fmt`,
`make typecheck`, `make lint`, `make doc-coverage`, `make markdownlint`,
`make nixie`, and `make test`, sequentially, through the `scrutineer` agent
where available. Every milestone edits this ExecPlan, and `make test` reads it,
so no milestone qualifies for a documentation-only gate set. Within a
milestone, use focused `cargo nextest run -E` invocations and `make proptest`
to iterate.

## Idempotence and recovery

Every step is re-runnable. Mutation patches are applied and reverted with
`git apply` and `git apply -R`; if a run is interrupted mid-mutation,
`git diff --stat src/` shows the residue and `git checkout -- src/` clears it
(commit first so nothing else is lost). Mutation runs disable seed persistence,
so `git status --short proptest-regressions/` must be clean after each;
existing tracked regression files are never deleted. EP-M0 probes are never
committed.

## Artefacts and notes

Record here, as work proceeds: the EP-M0 answers with transcripts; the EP-M1
red and green transcripts; each mutation's failure set and shrunk
counter-example; the coverage-class counts; the suite's measured wall time.

## Interfaces and dependencies

No production interface changes. The test-only interfaces are:

```rust
// src/json_value_strategy_test_support.rs (crate-level, #[cfg(test)])
pub(crate) fn json_values(strings: BoxedStrategy<String>) -> impl Strategy<Value = Value>;
pub(crate) fn json_kind(value: &Value) -> &'static str;

// src/manifest/expand/property_tests/strategy/mod.rs
// `pub(in crate::manifest::expand::property_tests)` is written `pub(suite)` below for brevity.
pub(suite) struct EntrySpec { /* section, fields, vars, foreach, when, layout */ }
pub(suite) struct DocumentSpec { /* targets, actions */ }
pub(suite) fn entry_spec(kind: EntryKind) -> impl Strategy<Value = EntrySpec>;
pub(suite) fn document_spec() -> impl Strategy<Value = DocumentSpec>;

// src/manifest/expand/property_tests/strategy/condition.rs
pub(suite) enum Condition {
    Literal(bool),
    Flag(VarName),
    NotFlag(VarName),
    Count(VarName),
    VarCmp(VarName, CmpOp, i64),
    IndexCmp(CmpOp, i64),
    ItemCmp(CmpOp, i64),
}
impl Condition {
    pub(suite) fn render(&self, form: ConditionForm) -> String;
    pub(suite) fn holds(&self, bindings: &Bindings) -> bool;
}

// src/manifest/expand/property_tests/model.rs
pub(super) fn expected_outputs(spec: &EntrySpec) -> Vec<ManifestValue>;

// src/manifest/expand/property_tests/support.rs
pub(super) fn strict_environment() -> minijinja::Environment<'static>;
pub(super) fn expand_document(doc: &mut ManifestValue) -> Result<ExpansionReport, TestCaseError>;
pub(super) fn key_sequence(map: &ManifestMap) -> Vec<&str>;
```

The exact signatures may change during EP-M3; the visibility may not exceed the
bounds shown. The suite is test-only support owned by `src/manifest/expand/`;
no other module may import it. The lifted JSON strategy is the one shared item:
any `#[cfg(test)]` code in the crate may use it, it must stay free of manifest
knowledge, and new leaf kinds belong at the call site, not in the shared
function. The repository sweep found `json_values()` in
`src/shell_selection/selection/property_tests.rs`, `var_value()` in
`src/manifest/tests/vars/reserved/property.rs`, and `member()` in
`tests/std_filter_tests/collection_filters/compact_property.rs`; only the first
models every JSON kind, and the third lives in an integration test crate that
the library cannot import.

Dependencies used, all existing: `proptest` (strategies, `TestRunner`,
`RngSeed::Fixed`, `prop::sample::Index`), `googletest` (`assert_that!`,
`elements_are!`), `pretty_assertions` (`assert_eq!` for sequence diffs),
`rstest`, `rstest-bdd`, `serde_json`, `serde_saphyr`, and `minijinja`.

## Progress

- [x] (2026-10-31 00:40Z) Renamed the branch to
  `4-3-2-manifest-expansion-invariant-proptests`.
- [x] (2026-10-31 00:55Z) Reconnaissance of expansion code, existing tests,
  proptest infrastructure, mutation contract, and documentation, by three
  Wyvern agents.
- [x] (2026-10-31 01:05Z) Identified the `swap_remove` key-order defect from
  the locked `serde_json` source.
- [x] (2026-10-31 01:15Z) Drafted this ExecPlan.
- [x] (2026-10-31 01:20Z) Committed the regenerated `typos.toml` alone, then
  the draft plan.
- [x] (2026-10-31 02:10Z) Expert design review by four reviewers (property
  testing, codebase accuracy, process, pre-mortem); revised the plan (see the
  revision note).
- [ ] User approval.
- [ ] EP-M0 feasibility probes.
- [ ] EP-M1 key-order fix.
- [ ] EP-M2 shared test support and relocation.
- [ ] EP-M3 generators, oracles, coverage, and properties.
- [ ] EP-M4 unhappy paths, scenarios, documentation, and closure.

## Surprises & discoveries

- Observation: `serde_json::Map::remove` is `swap_remove` under
  `preserve_order`, so expansion reorders keys whenever two or more keys follow
  a control key. Evidence: `serde_json-1.0.151/src/map.rs`, lines 156–165;
  `src/manifest/expand/mod.rs` line 242 and `src/manifest/expand/evaluation.rs`
  line 44 call `remove`. Impact: violates `DES-ORDER`; EP-M1 fixes it.
- Observation: `serde_json::Map` equality ignores order, so no existing
  value-equality assertion could have caught the defect. Evidence:
  `serde_json-1.0.151/src/map.rs`, lines 409–413, delegating to `IndexMap::eq`.
  Impact: the key-sequence constraint.
- Observation: MiniJinja is built without `preserve_order`, so object items
  reach `vars.item` with sorted keys. Evidence: `Cargo.toml`, line 113;
  `minijinja-2.24.0/src/value/mod.rs`, lines 246–247. Impact: `C-3` excludes
  nested item order; the fidelity check uses order-insensitive equality.
- Observation: two existing tests pass by coincidence under `swap_remove`.
  `foreach_preserves_generated_source_key_order` always writes `foreach` last,
  and `structure_cases.rs::expand_foreach_preserves_object_key_order` (keys
  `name, vars, foreach, when, after`) leaves at most one key after each
  removal. Impact: neither reaches the defect; `OBL-ORDER` supersedes the
  first, and the second stays as a boundary example.
- Observation: the existing generated modules are named `property_cases`, so
  `make proptest` does not select them. Impact: EP-M2 relocates them.
- Observation: the developers' guide says entries without `foreach` are
  "passed through unchanged", but static entries lose `when`. Impact: EP-M4
  corrects the sentence.
- Observation: `conditional` is a real `Target` field, not only a synthetic
  query-mode key. Evidence: `src/ast/target.rs`. Impact: it is generated as an
  ordinary non-control field.

## Decision log

- Decision: place the suite at `src/manifest/expand/property_tests/`, a
  sibling of `tests/`, declared `#[cfg(test)] mod property_tests;`. Rationale:
  the `property_tests` segment is required for `make proptest`; a child of
  `expand` reaches private helpers and the `pub(super)` helpers in
  `expand/tests/mod.rs` without widening visibility; the mutation registry
  resolves stems against `src/manifest/expand/property_tests/<leaf>.rs`.
  Date/Author: 2026-10-31, planning agent.
- Decision: fix the key-order defect in this item, as its own atomic commit,
  rather than defer it. Rationale: `FV-EXP` names key order as an expansion
  invariant and the design document promises authoring order; deferring would
  mean writing a property that encodes the defect. The pre-mortem reviewer
  proposed a separate pull request; the fix is two lines with its own red
  tests, so an atomic commit at the head of the stack keeps it reviewable
  without a second pull request. Date/Author: 2026-10-31, planning agent.
- Decision: record the expansion contract as clauses `C-1` to `C-5` in
  `docs/netsuke-design.md` section 2.5, not in a new ADR. Rationale: the
  clauses restate intended behaviour that the design document already claims
  and restore one broken promise; no architecture, dependency, or public
  interface changes. The ADR numbers above 0060 are also in flux on other
  branches. Date/Author: 2026-10-31, planning agent, on review advice. The
  clauses are: `C-1` non-control fields are copied verbatim; `C-2` entry-level
  and `vars` key order is the source order minus control keys, with injected
  keys appended, for foreach entries only; `C-3` `item` is the iteration value
  as the evaluator returns it (sequence order preserved, object keys sorted) and
  `index` is its zero-based `u64` position, both overriding user `vars`; `C-4`
  `when` must be a non-empty string, is evaluated over the entry's own `vars`
  (plus `item` and `index` for foreach entries) and is then removed, for static
  and foreach entries in both sections, and a static entry with non-object
  `vars` sees an empty context; `C-5` query mode and the document state after
  an error are outside the contract.
- Decision: use a closed set of condition shapes rather than a recursive
  expression grammar. Rationale: the roadmap asks about expansion, not about
  MiniJinja's evaluator; the closed set covers `vars`, iteration values,
  integer truthiness, both forms, and mixed outcomes, with a trivial oracle and
  no operand-returning `and`/`or` hazard. Date/Author: 2026-10-31, planning
  agent, on review advice.
- Decision: fold the list-literal `foreach` form into `OBL-INJECT` rather than
  keep a separate metamorphic property. Rationale: the same oracle checks both
  production paths, and the separate property's only mutation was contrived.
  Date/Author: 2026-10-31, planning agent, on review advice.
- Decision: drop the planned pipeline-level property and the insta snapshot.
  Rationale: the property had no distinct fault and depended on ambient
  current-directory state through standard-library registration; the
  behavioural scenarios cover the YAML-to-typed-manifest path. The snapshot
  would restate the key-sequence assertions, and no rendered output format is
  at stake. Date/Author: 2026-10-31, planning agent, on review advice.
- Decision: the red stage for property work is the mutation substitute (write
  the patch, apply it, watch the property fail for the intended reason,
  revert). Rationale: the properties specify behaviour that production already
  has, apart from key order, so they cannot fail against production; a property
  that has never been seen to fail proves nothing. Date/Author: 2026-10-31,
  planning agent.
- Decision: acceptance for a mutation patch is "the named property is in the
  failure set", with the full set recorded. Rationale: realistic faults break
  several tests; "exactly one fails" is unachievable and would push towards
  contrived patches. Date/Author: 2026-10-31, planning agent, on review advice.
- Decision: lift `json_values()` into crate-level test support instead of
  writing a second JSON strategy. Rationale: `AGENTS.md` requires reuse of an
  existing equivalent; the function already covers every JSON kind with bounded
  recursion. Date/Author: 2026-10-31, planning agent, on review advice.
- Decision: relocate the existing `property_cases` modules into the suite,
  and retire `foreach_preserves_generated_source_key_order` once `OBL-ORDER`
  lands. Rationale: relocation makes `make proptest` select them without
  changing a test body; the retired property's domain (six permutations of
  three keys with `foreach` last) is a strict subset of `OBL-ORDER`'s, and its
  generator cannot reach the defect. Date/Author: 2026-10-31, planning agent.
- Decision: build `ManifestValue` directly in expansion-level properties.
  Rationale: YAML quoting of arbitrary strings (including `{{ … }}`, which YAML
  reads as a flow mapping) would force the generator to escape or filter; the
  contract is stated over the parsed value. Date/Author: 2026-10-31, planning
  agent.
- Decision: use a strict-undefined MiniJinja environment in properties.
  Rationale: production uses `UndefinedBehavior::Strict`; a generator bug that
  references an undefined name then fails loudly instead of evaluating to
  false. Date/Author: 2026-10-31, planning agent.
- Decision: reject Kani and Verus for this item. Rationale: see *Method
  selection*. Date/Author: 2026-10-31, planning agent.
- Decision: no OrthoConfig change. Rationale: the item adds no configuration,
  flag, or help text. Date/Author: 2026-10-31, planning agent.
- Decision: commit the regenerated `typos.toml` alone as the branch's first
  commit. Rationale: `make spelling` regenerated it from the pinned builder;
  the change is unrelated to this item, and `AGENTS.md` asks for it to be
  offered as a separate base pull request. Date/Author: 2026-10-31, planning
  agent.

## Outcomes & retrospective

Not yet started.

## Revision note

2026-10-31, after expert design review. Four reviewers (property testing,
codebase accuracy, process compliance, and pre-mortem) found two blockers and a
dozen major issues. Changes:

- Restated `AXIOM-MJ-SERDE` (MiniJinja sorts map keys) and added
  `AXIOM-MAP-EQ` (map equality ignores order), with a constraint that order is
  asserted on key sequences.
- Replaced the recursive condition grammar with a closed set of shapes, with
  a keyword-free variable pool and structural free-name checks.
- Fixed the mutation patches so that they compile under denied warnings,
  re-targeted `MUT-INDEX` at the renumbering fault with its true detection
  class, and changed acceptance to failure sets.
- Corrected the order oracle for static entries and added `vars` to the
  generated layout.
- Dropped ADR-0068, `OBL-INJECT-EXPR` as a separate property, the pipeline
  property, and the snapshot; added three behavioural scenarios and an
  unhappy-path rstest table.
- Added reuse of `json_values()` and relocation of the existing generated
  tests.
- Consolidated seven milestones into five, with the full gate set at each
  plateau, and raised the scope tolerance to match the planned file list.

The remaining work is unchanged in purpose: approval, then EP-M0 to EP-M4.
