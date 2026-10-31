# 4.3.2. Add Proptest coverage for manifest expansion invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

The plan awaits explicit approval. No implementation step may begin until the
user approves it; silence is not approval.

## Purpose / big picture

Netsuke turns a YAML manifest (a `Netsukefile`) into a Ninja build file. Before
the manifest becomes typed Rust data, a loader pass called *expansion* rewrites
the parsed YAML value: every entry carrying a `foreach` key is copied once per
iteration value, and every entry carrying a `when` key is kept or dropped
according to a Jinja condition. The four roadmap claims for this item are that
expansion

1. copies every field that is not a control key (`foreach`, `when`) unchanged
   into each generated entry,
2. removes `when` once it has been evaluated,
3. gives each generated entry its own `item` (the iteration value) and `index`
   (its zero-based position) in `vars`, and
4. still honours `when` on *static* entries, meaning entries without
   `foreach`.

Today those claims are pinned mainly by hand-picked examples and by a handful
of narrow generated tests. After this change, a dedicated Proptest suite
generates whole manifests (mixtures of static and `foreach` entries, in both the
`targets` and `actions` sections, with arbitrary JSON-shaped field values,
control keys at arbitrary positions, and conditions drawn from a small
expression grammar with an independent Rust evaluator) and checks each claim
against an oracle that does not reuse the production code.

Planning surfaced one real defect, which this plan also fixes. The design
document promises that "downstream consumers see keys in a stable sequence
after foreach expansion" (`docs/netsuke-design.md`, the `ManifestMap`
paragraph). The code breaks that promise: `serde_json::Map::remove`, under the
`preserve_order` feature Netsuke enables, is a `swap_remove`, so removing
`when` or `foreach` from the middle of an entry moves the entry's *last* key
into the vacated slot. The existing key-order property never notices, because
its generator always writes `foreach` last. The fix is two calls to
`shift_remove`, driven by a red regression test.

Success is observable as follows. Running `make proptest` lists the new suite
under `manifest::expand::property_tests` and every property passes. Applying
any committed mutation patch under `docs/verification/mutations/` whose stem
begins `manifest__expand__property_tests__` makes exactly the named property
fail with a shrunk counter-example. The roadmap entry 4.3.2 and its four
sub-items are marked done.

## Signposts: documentation and skills

Read these before starting, in this order:

- `AGENTS.md` (repository root): gates, the 400-line file cap, the ban on
  in-process environment mutation, doc-comment rules, and the Polonius and
  next-solver posture.
- `docs/roadmap.md`, item 4.3.2, and
  `docs/formal-verification-methods-in-netsuke.md`, section *Proptest for
  determinism and manifest semantics*: the upstream statement of the work.
- `docs/netsuke-design.md`, sections 1.2 (*The Six Stages of a Netsuke
  Build*, stage 3), 2.5 (*foreach and when*), 3.3 (template expansion stages),
  and the `ManifestMap` key-order paragraph.
- `docs/developers-guide.md`, sections *Property-based testing with proptest*
  (including *Emission determinism suites*, which this suite mirrors), *Manifest
  `foreach` expansion*, and the `expand_foreach` interface reference.
- `docs/users-guide.md`, section *Generate targets with `foreach` and
  `when`*.
- `docs/adrs/adr-0003-keep-foreach-and-when-scoped-to-manifest-targets.md`,
  `docs/adrs/adr-0050-bound-manifest-template-evaluation.md`, and
  `docs/adrs/adr-0030-ninja-emission-determinism-contract.md` (the model for
  the contract record this plan writes).
- `docs/execplans/4-3-1-proptests-for-deterministic-ninja-emission.md`: the
  sibling plan. Copy its discipline (obligation ledger, mutation patches,
  coverage classification), not its length.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/snapshot-testing-in-netsuke-using-insta.md`.
- `docs/ortho-config-users-guide.md`: read for orientation only. This change
  adds no configuration surface, so OrthoConfig is not touched (see
  `Decision log`).
- `docs/documentation-style-guide.md`: ADR format, ExecPlan `Status:` rules,
  and en-GB-oxendict spelling.

Load these skills when implementing: `rust-router` first, then `proptest`
(strategy design, filtering trap, regression files), `rust-unit-testing`
(rstest tables, googletest and `pretty_assertions` assertions, insta),
`rust-verification` (only if the method selection below is challenged),
`hexagonal-architecture` (to keep test support out of production modules),
`arch-decision-records` (for ADR-0068), `codegraph-mcp` (callers and impact of
`when_allows` and `expand_foreach_item`), `en-gb-oxendict-style`, and
`commit-message`. Use `firecrawl-mcp` for any question about MiniJinja,
`serde_json`, or Proptest behaviour that the local registry sources do not
answer.

## Context and orientation

### The pipeline

`src/manifest/mod.rs::evaluate_manifest` (around lines 106–170) runs the
manifest pipeline:

1. `serde_saphyr::from_str` parses the YAML text into a `ManifestValue`, which
   is `serde_json::Value` (`src/manifest/mod.rs`, lines 41–43). A `ManifestMap`
   is `serde_json::Map<String, ManifestValue>`.
2. A MiniJinja `Environment` is built with `UndefinedBehavior::Strict`, the
   standard library, manifest-level `vars` as globals, and macros.
3. `expand_foreach_with_budget(&mut doc, &jinja, &budget)` performs
   expansion. This is the subject of this plan.
4. `serde_json::from_value::<NetsukeManifest>(doc)` deserializes into the
   typed abstract syntax tree (AST). The target struct (`src/ast/target.rs`,
   line 13) uses `#[serde(deny_unknown_fields)]`, so a leftover `when` or
   `foreach` key would fail here.
5. `render::render_manifest_with_budget` renders the string fields.

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

`expand_foreach_item` clones the source map, calls `entry.remove("foreach")`
(line 242), evaluates `when` through `when_allows`, and on inclusion calls
`inject_iteration_vars`.

`src/manifest/expand/evaluation.rs` (239 lines) holds:

- `parse_foreach_values`: an inline JSON array is converted with
  `minijinja::Value::from_serialize`; any other value must be a string, which
  is compiled and evaluated as a Jinja expression with an empty local context.
- `when_allows`: removes `when` with `map.remove("when")` (line 44), rejects a
  non-string value, builds the condition context, and evaluates it. The result
  is `Include`, `Exclude`, or `Conditional`. `Conditional` arises only in
  manifest-query mode, when a helper is disabled; it inserts a synthetic
  `conditional: true` key.
- `when_context`: the condition sees the entry's own `vars` object as its
  local context, plus `item` and `index` during iteration (overriding colliding
  `vars` keys). Manifest-level `vars`, `env`, `glob`, and the standard library
  are environment globals.
- `eval_when`: rejects an empty or whitespace-only condition, tries the
  expression path (`jinja_macros::evaluate_when_expression`, which takes
  MiniJinja truthiness via `Value::is_true`), and falls back to rendering the
  text as a template, accepting only a trimmed, lower-cased `true` or `1`.
- `inject_iteration_vars`: creates `vars` if absent (an error if present but
  not an object), then inserts `item` as `serde_json::to_value(item)` and
  `index` as a `u64` JSON number. Insertion overwrites colliding user keys.

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
`name, command, after`. The typed AST does not care about map order, but the
deserializer visits keys in map order, so any diagnostic that names the first
offending field (for example, `deny_unknown_fields` naming an unknown key) can
name a different field from the one the author wrote first. Stage EP-M0
measures whether that diagnostic difference is reachable from the command line.

### Existing tests

Expansion already has extensive example coverage under
`src/manifest/expand/tests/` (`condition_cases.rs` is at the 400-line cap),
integration tests in `tests/manifest_jinja_tests.rs`, and rstest-bdd scenarios
in `tests/features/manifest.feature` ("Generating targets with foreach",
"Generating actions with foreach and when"). Two modules already hold generated
tests:

- `src/manifest/expand/tests/property_cases.rs`: entry counts, sequential
  indexes, `foreach` and `when` key removal for foreach entries, determinism of
  filtering, the filtering report, and rejection of whitespace-only conditions.
- `src/manifest/expand/tests/foreach/property_cases.rs`: cardinality-budget
  termination, integer and short-string `item`/`index` injection, collision
  override, threshold filtering, and a key-order property over six fixed
  permutations with `foreach` always last.

Neither module path ends in `property_tests`, so `make proptest` (`Makefile`,
`PROPTEST_FILTER`) does not select them; `make test` runs them. Their gaps
against the roadmap are: no generated non-control fields beyond `name`,
`after`, and `description`; no non-scalar items; no static-entry inclusion
property; no `actions`-section property; no control key at a middle position.

### The mutation-evidence contract

`docs/verification/mutations/` holds unified-diff patches, each seeding one
fault. `tests/kani_mutation_evidence_tests/contracts.rs` keeps
`SUPPLEMENTAL_PROPERTY_PATCHES`, the registry of Proptest-only patch stems. A
stem is the property's module path with `::` written as `__`, followed by the
property's function name. `supplemental_property_location` resolves
`manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order`
to `src/manifest/expand/property_tests/order.rs` (or `order/mod.rs`) and
requires that file to declare the named function.
`every_patch_matches_a_harness` rejects orphan patches and
`every_patch_applies_cleanly` runs `git apply --check` on each; both run under
`make test`. `make test-kani-mutations` compiles every patched tree under
denied warnings.

### Terms used in this plan

- *Control key*: `foreach` or `when`. The synthetic `conditional` key is a
  third, query-mode-only control key and is outside this plan's domain.
- *Non-control field*: any entry key other than a control key and other than
  `vars`, which expansion edits.
- *Oracle*: a test-side function that computes the expected result by a
  method structurally different from production.
- *Mutation patch*: a committed diff that seeds a plausible fault so that a
  named property can be shown to detect it.
- *Coverage classification*: counts, gathered over a fixed-seed run, of how
  often each material input class was generated; used to prove a generator is
  not vacuous.

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
  downstream), and `DES-ORDER` (the `ManifestMap` paragraph promising stable
  key order after foreach expansion).
- `docs/developers-guide.md`, `expand_foreach` interface reference:
  `DG-EXPAND`.
- `docs/users-guide.md`, *Generate targets with `foreach` and `when`*:
  `UG-FOREACH` ("iteration values take precedence over both"; "Top-level
  actions support the same `foreach` and `when` keys").
- ADR-0003 (scope of `foreach` and `when`), ADR-0050 (manifest budget),
  ADR-0030 (contract-record model).
- New: ADR-0068, *Manifest expansion invariant contract* (`ADR-0068`). The
  number is provisional; see `Decision log`.

No Terms of Reference document exists for this phase; the roadmap and the
formal-verification survey stand in for it.

Trace links:

```plaintext
RM-4.3.2.a -> FV-EXP -> DES-FOREACH -> EP-M3
  -> manifest::expand::property_tests::preserve::expansion_copies_every_non_control_field
FV-EXP -> DES-ORDER -> EP-M1
  -> manifest::expand::tests::key_order_cases (rstest regression)
FV-EXP -> DES-ORDER -> EP-M3
  -> manifest::expand::property_tests::order::{foreach,when}_removal_keeps_authoring_key_order
RM-4.3.2.b -> DES-FOREACH -> DG-EXPAND -> EP-M5
  -> manifest::expand::property_tests::when_removed::no_expanded_entry_carries_a_control_key
RM-4.3.2.c -> DES-STAGE3 -> UG-FOREACH -> EP-M4
  -> manifest::expand::property_tests::inject::each_expansion_carries_its_own_item_and_index
RM-4.3.2.c -> DES-FOREACH -> EP-M4
  -> manifest::expand::property_tests::inject::expression_foreach_matches_inline_foreach
RM-4.3.2.d -> DES-FOREACH -> ADR-0003 -> EP-M5
  -> manifest::expand::property_tests::static_when::static_entries_survive_exactly_when_their_condition_holds
RM-4.3.2.d -> DES-FOREACH -> EP-M5
  -> manifest::expand::property_tests::pipeline::typed_manifest_holds_exactly_the_selected_static_targets
FV-EXP -> ADR-0068 -> EP-M2
  -> manifest::expand::property_tests::strategy::generator_reaches_every_material_class
```

## Constraints

- Production code may change only at the two key-removal sites:
  `src/manifest/expand/mod.rs` (`entry.remove("foreach")`) and
  `src/manifest/expand/evaluation.rs` (`map.remove("when")`). Both become
  `shift_remove`. Any other production change is a tolerance breach.
- No item may widen visibility. The suite sits inside
  `src/manifest/expand/` as a `#[cfg(test)]` child module, so it reaches
  private items through `super::` without new `pub(crate)` surface.
- No new dependency. `proptest` 1.11.0, `googletest` 0.14.3,
  `pretty_assertions` 1.4.1, `insta` 1.x, `rstest` 0.26.1, and `rstest-bdd`
  0.6.0 are already dev-dependencies.
- No in-process environment mutation. Pipeline-level properties use
  `manifest::from_str_with_env` with an injected `EnvReader` closure; nothing
  calls `std::env::set_var` or `remove_var`.
- Generated conditions must not call `env()`, `glob()`, `command_available()`,
  `which`, or any helper that touches the filesystem or process state, and must
  never produce `Conditional`.
- The new suite's module path must contain a segment ending in
  `property_tests`, or `make proptest` will not select it.
- Every file stays under 400 lines; every module starts with `//!`; every
  helper function carries a `///` summary; test functions carry none.
- Property bodies use `prop_assert*` and return `TestCaseError`, never
  `panic!`, `unwrap`, or `expect` on a generated value. No `prop_filter` or
  `prop_assume!` on a structural condition; generators construct valid values.
- Each obligation gets a structurally independent oracle. An oracle that
  calls `when_allows`, `inject_iteration_vars`, `Map::remove`, or
  `Map::shift_remove` is a re-implementation and is forbidden.
- Do not delete or weaken the existing `property_cases.rs` modules or any
  example test. Readable examples remain the specification beside the
  properties.
- Do not add a `-Z` flag, a `-Znext-solver` directive, or a defensive
  `.clone()` to satisfy the borrow checker without first compiling the natural
  form.
- ExecPlan `Status:` stays one bare value from the closed set.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 30 non-ExecPlan
  files or more than 3,000 net lines of code and other documentation. This
  ExecPlan file is excluded from both counts.
- Production change: stop if any production edit beyond the two
  `shift_remove` calls is needed, including any change prompted by a property
  failure other than key order. Record the counter-example first.
- Interface: stop if any item's visibility must widen.
- Dependencies: stop if any new crate or feature is needed.
- Fidelity: stop if EP-M0 shows that `item` does not round-trip, meaning
  `serde_json::to_value(Value::from_serialize(x)) != x` for some JSON value the
  YAML loader can produce. That is a defect candidate, not a generator
  restriction to quietly apply.
- Runtime: the whole new suite should add no more than 5 s of test CPU
  under `make test`; stop at 15 s.
- Rejection: stop if a strategy needs filtering to reach valid inputs.
- Assertion shape: stop if a property can only be stated as a disjunction of
  observed behaviours.
- Iterations: stop if a gate still fails after three focused fix attempts.
- Contract test: stop if `tests/kani_mutation_evidence_tests` rejects a stem
  or patch for a reason other than a typo.
- ADR number: re-run the remote-branch sweep before the first push; if 0068
  is taken, renumber above the highest allocation as its own commit.
- Ambiguity: stop if the contract record would have to choose between two
  readings of the design or users' guide that change what a user sees.

## Risks

- Risk: the expression grammar's oracle disagrees with MiniJinja on an edge
  (integer comparison with booleans, `not` precedence). Severity: medium.
  Likelihood: medium. Mitigation: render every sub-expression fully
  parenthesized; restrict comparisons to integers; restrict boolean leaves to
  literal booleans and boolean-typed `vars`; EP-M0 classifies each grammar form
  against MiniJinja before any property is written.
- Risk: the template fallback path (`"{{ e }}"`) behaves differently from the
  expression path for some forms (for example, an integer that renders as `2`
  is false under the fallback but true under `is_true`). Severity: medium.
  Likelihood: high if unrestricted. Mitigation: the template form wraps only
  boolean-typed expressions, whose renderings are `true` or `false`; EP-M0
  confirms that `{{ … }}` fails `compile_expression` and so reaches the
  fallback.
- Risk: `item` fidelity fails for large integers, negative zero, or floats.
  Severity: high (a real defect). Likelihood: low. Mitigation: EP-M0 probes the
  boundaries `i64::MIN`, `i64::MAX`, `u64::MAX`, `-0.0`, and sub-normal floats;
  a failure triggers the fidelity tolerance.
- Risk: fixing key order changes a diagnostic another test pins.
  Severity: low. Likelihood: low. Mitigation: EP-M1 runs the whole suite after
  the fix; any changed expectation is reviewed, not silently updated.
- Risk: the existing key-order property
  (`foreach_preserves_generated_source_key_order`) gives a false sense of
  coverage. Severity: low. Likelihood: certain. Mitigation: record the vacuity
  in `Surprises & discoveries`; the new `order` properties place control keys
  at every position.
- Risk: the `Makefile` comment above `PROPTEST_FILTER` quotes a test census
  ("90 of the 3848 tests") that this suite changes. Severity: low. Likelihood:
  certain. Mitigation: EP-M6 re-measures with the recipe's own flags and
  updates the sentence, or rewrites it to name the command rather than the
  number.
- Risk: ADR-0068 is taken by another branch before merge. Severity: medium.
  Likelihood: medium. Mitigation: the ADR-number tolerance above.
- Risk: ADR-0003 says `foreach` and `when` are "target level only", while the
  design document, developers' guide, and users' guide all state that top-level
  `actions` support them too. Severity: low. Likelihood: certain. Mitigation:
  the code expands both sections; this plan covers both and records the reading
  ("target level" means entry level, as opposed to keys nested inside a field)
  in ADR-0068 without amending ADR-0003.

## Verification plan

### Method selection

The obligations range over an open-ended input space (arbitrary field maps,
arbitrary JSON values, control-key positions, condition expressions), so
Proptest is the primary method. Named rstest cases pin the key-order regression
and each EP-M0 discovery. An insta snapshot pins one representative expanded
document, because key order is exactly a format-consistency concern. One
rstest-bdd scenario covers the user-observable consequence of key order if
EP-M0 shows one exists.

Kani is rejected: expansion runs MiniJinja's parser and evaluator and allocates
`serde_json` maps, which ADR-0047 already found beyond the bounded model
checker's practical reach even for the smaller IR structures. Verus is
rejected: the plan introduces no lemma or contractual algorithm. The fix is a
call to a library function whose contract is an axiom below, and the remaining
claims are about a composition of library calls rather than a pure kernel.
Writing a Verus model would prove a model, not Netsuke.

### Axioms

- `AXIOM-PROPTEST`: Proptest 1.11.0 generates and shrinks as documented.
- `AXIOM-SHIFT`: under `preserve_order`, `serde_json::Map::shift_remove`
  removes a key and preserves the relative order of the rest, and `Map::insert`
  of a new key appends it while insertion of an existing key keeps its position
  (`serde_json` 1.0.151 `src/map.rs`).
- `AXIOM-MJ-TRUTH`: MiniJinja 2.24.0 `Value::is_true` treats `false`, zero,
  `none`, undefined, and empty containers as false and everything else as true.
  Integer comparison operators behave as in Rust for `i64` operands.
- `AXIOM-MJ-SERDE`: MiniJinja `Value::from_serialize` preserves map key
  order and sequence order. EP-M0 measures, rather than assumes, whether it
  round-trips every JSON value back through `serde_json::to_value`.
- `AXIOM-SAPHYR`: the expansion-level properties build `ManifestValue`
  directly instead of through YAML text. The pipeline-level property is the one
  place YAML text is parsed, so the YAML-to-JSON mapping is exercised end to
  end there, not assumed.

### Shared generator and oracle

Generators live in `src/manifest/expand/property_tests/strategy/`. They produce
an `EntrySpec` model, which renders to a `ManifestValue`, so the oracle reads
the model, not the rendered map:

- `section`: `targets` or `actions`.
- `fields`: an ordered list of up to six non-control fields. Keys mix the
  real manifest field names (`name`, `command`, `description`, `sources`,
  `deps`, `order_only_deps`, `after`, `phony`, `always`, `rule`, `script`) with
  generated `[a-z][a-z0-9_]{0,7}` names. A generated name that collides with
  `foreach`, `when`, `vars`, or `conditional` is transformed by a `field_`
  prefix, and duplicates keep their first occurrence; both are transforms, not
  filters. Values are arbitrary JSON up to depth three and collections up to
  four elements: `null`, booleans, `i64` including the extremes, finite `f64`,
  strings (including text that looks like Jinja, such as `{{ item }}`, which
  expansion must not render), arrays, and objects.
- `vars`: absent, or an object of up to four entries, sometimes containing
  user keys named `item` or `index` to exercise override.
- `foreach`: absent (a static entry), or an inline array of up to eight
  arbitrary JSON items, or (for the metamorphic property) a Jinja list literal
  string.
- `when`: absent, or a `WhenExpr` rendered in expression form or, for
  boolean-typed expressions only, template form `{{ … }}`.
- `positions`: the insertion index of each control key among the fields,
  drawn uniformly from `0..=fields.len()`, so first, middle, and last positions
  are all reachable.

`WhenExpr` (`strategy/when_expr.rs`) is a recursive grammar of depth at most
three: boolean literals; references to boolean or integer `vars` keys that the
generator guarantees exist; integer comparisons (`<`, `<=`, `==`, `!=`, `>`,
`>=`) over integer literals, integer `vars`, `index`, and, when items are
integers, `item`; `not`; `and`; `or`. The renderer parenthesizes every
sub-expression. The oracle `WhenExpr::holds(&Bindings) -> bool` evaluates the
tree in Rust with MiniJinja's truthiness for integer leaves (zero is false). It
shares no code with MiniJinja.

The expansion oracle (`property_tests/model.rs`) computes, for an `EntrySpec`,
the expected output list by iterating the model: for each item index `i` (or
once, for a static entry) it decides inclusion from `WhenExpr::holds`, then
builds the expected map by walking the source keys in order, skipping control
keys, copying fields verbatim, and building the expected `vars` as the source
`vars` with `item` and `index` overwritten in place or appended. If the source
has no `vars`, the expected map ends with `vars` holding only `item` then
`index`. The oracle never calls a `Map` removal method.

### Obligations

**OBL-PRESERVE** (`RM-4.3.2.a`). *Expansion copies every non-control field
verbatim.*

- Statement: for every generated document and every output entry derived
  from source entry `E`, each non-control field of `E` is present with a
  deep-equal value, no other non-control field is present, and every `vars` key
  of `E` other than `item` and `index` keeps its value. Static entries
  additionally keep `vars` unchanged (no `item` or `index` is injected).
- Method: Proptest, 128 cases.
- Artefact:
  `src/manifest/expand/property_tests/preserve.rs::expansion_copies_every_non_control_field`.
- Oracle: the model's expected map, compared with `pretty_assertions` output
  on failure.
- Evidence: passes on the fixed tree; fails under `MUT-PRESERVE`.
- Non-vacuity: coverage classification requires static and foreach entries
  in both sections, fields of every JSON kind including nested depth two or
  more, Jinja-looking strings, and `vars` present and absent. Mutation
  `MUT-PRESERVE` replaces `entry.remove("foreach")` in `expand_foreach_item`
  with a whitelist `retain` that keeps only `name`, `command`, `vars`, and
  `when`, which is the realistic fault of rebuilding an entry from known
  fields; the property must fail with a shrunk entry naming a dropped field.

**OBL-ORDER** (`FV-EXP`, `DES-ORDER`). *Expansion keeps authoring key order.*

- Statement: for every output entry, its key sequence equals the source key
  sequence with control keys deleted, followed by `vars` if the source had none.
- Method: rstest regression cases first (EP-M1), then two Proptest
  properties, 128 cases each, one per removal site.
- Artefacts: `src/manifest/expand/tests/key_order_cases.rs` (rstest, every
  position of `when` and `foreach` in a four-field entry, both sections);
  `src/manifest/expand/property_tests/order.rs::foreach_removal_keeps_authoring_key_order`
  (foreach entries, `when` absent) and
  `…::when_removal_keeps_authoring_key_order` (static entries with `when`).
- Evidence: the rstest cases fail on `origin/main` for every middle and first
  position (red), pass after `shift_remove` (green). Each property fails when
  its site is reverted to `remove`.
- Non-vacuity: classification requires the control key at position zero, a
  middle position, and the last position, each at least 10 times in the
  fixed-seed run, with at least two fields after a middle position (the shape
  `swap_remove` corrupts). Mutations `MUT-ORDER-FOREACH` and `MUT-ORDER-WHEN`
  each restore `remove` at one site.

**OBL-WHEN-REMOVED** (`RM-4.3.2.b`). *No output entry carries a control key,
and only included entries survive.*

- Statement: for every generated document with mixed static and foreach
  entries in both sections, every output object lacks `when` and `foreach`, and
  the number of outputs per section equals the oracle's count.
- Method: Proptest, 128 cases.
- Artefact:
  `src/manifest/expand/property_tests/when_removed.rs::no_expanded_entry_carries_a_control_key`.
- Evidence: fails under `MUT-WHEN-KEEP`.
- Non-vacuity: classification requires, per section, entries whose `when`
  held, entries whose `when` failed, and entries with no `when`. Mutation
  `MUT-WHEN-KEEP` reads the condition with `map.get("when").cloned()` in
  `when_allows` instead of removing it.

**OBL-INJECT** (`RM-4.3.2.c`). *Each generated entry carries its own `item` and
`index`.*

- Statement: for a foreach entry over items `x₀ … xₙ₋₁` and condition `P`,
  the outputs are exactly the entries for `i` in ascending order such that
  `P(xᵢ, i)` holds, and output `k` has `vars.item` deep-equal to `xᵢ` and
  `vars.index` equal to `i` as a `u64`, overriding any user `item` or `index`
  while keeping other user `vars`.
- Method: Proptest, 128 cases, over two item domains: arbitrary JSON with
  conditions over `index`, and integers with conditions over `item` and `index`.
- Artefact:
  `src/manifest/expand/property_tests/inject.rs::each_expansion_carries_its_own_item_and_index`.
- Evidence: fails under `MUT-INDEX`.
- Non-vacuity: classification requires empty foreach, all-excluded,
  all-included, and mixed outcomes; every JSON item kind; and user `vars`
  colliding on `item` and on `index`. Mutation `MUT-INDEX` passes
  `expanded.len()` (the count of kept entries) instead of the enumeration
  index, which is the realistic renumbering fault; it is caught only on mixed
  outcomes, which classification guarantees.

**OBL-INJECT-EXPR** (`RM-4.3.2.c`). *A Jinja list-literal `foreach` expands
exactly as the equivalent inline array.*

- Statement: for items drawn from the subset with a Jinja literal spelling
  (`none`, booleans, integers, strings without quotes or backslashes, and
  nested lists of these), expanding `foreach: "[…]"` yields a document equal to
  expanding `foreach: […]`.
- Method: Proptest, 128 cases. This is a metamorphic relation: neither side
  is trusted, but they take different production paths (`Value::from_serialize`
  versus `compile_expression` and evaluation).
- Artefact:
  `src/manifest/expand/property_tests/inject.rs::expression_foreach_matches_inline_foreach`.
- Evidence: fails under `MUT-INLINE-NULL`.
- Non-vacuity: classification requires `none` items, nested lists, and empty
  lists. Mutation `MUT-INLINE-NULL` drops `null` elements on the inline-array
  path only.

**OBL-STATIC-WHEN** (`RM-4.3.2.d`). *A static entry survives exactly when its
condition holds, and the report counts every exclusion.*

- Statement: for every document of static entries in both sections, an entry
  is in the output if and only if it has no `when` or its `WhenExpr` holds over
  its own `vars`; `report.stats.filtered_targets` and `filtered_actions` equal
  the oracle's exclusion counts; and every retained `FilteredEntry` for a
  static entry has `iteration_index == None`.
- Method: Proptest, 128 cases, with both expression and template forms.
- Artefact:
  `src/manifest/expand/property_tests/static_when.rs::static_entries_survive_exactly_when_their_condition_holds`.
- Evidence: fails under `MUT-STATIC-LEAK`.
- Non-vacuity: classification requires held and failed conditions in both
  forms and both sections, and conditions reading `vars`. Mutation
  `MUT-STATIC-LEAK` makes the `Exclude` arm of `expand_static_target` return
  the entry instead of dropping it.

**OBL-STATIC-WHEN-E2E** (`RM-4.3.2.d`, `DES-FOREACH`). *The typed manifest
holds exactly the selected static targets.*

- Statement: for generated YAML manifests of static targets with `name`,
  `command`, optional `vars`, and optional `when`, loaded through
  `manifest::from_str_with_env` with an injected `EnvReader` that reports every
  variable absent, the loaded `NetsukeManifest` target names equal the oracle's
  selection, in order.
- Method: Proptest, 32 cases (each case runs the whole loader).
- Artefact:
  `src/manifest/expand/property_tests/pipeline.rs::typed_manifest_holds_exactly_the_selected_static_targets`.
- Evidence: fails under `MUT-STATIC-LEAK` applied by hand; no separate patch
  is committed, because the contract registry allows one patch per stem and
  this property's purpose is the YAML-to-typed-AST connection, not a distinct
  fault.
- Non-vacuity: classification requires selected and unselected targets and
  at least one manifest whose selection is empty.

**OBL-GEN**. *The generators reach every class the obligations name.*

- Statement: over a fixed-seed run of 512 generated documents, each class
  listed in the non-vacuity entries above appears at least 10 times, and no
  generated condition references an undefined name.
- Method: a `TestRunner` with `RngSeed::Fixed`, following
  `src/stdlib/canonical_value/tests/properties.rs` (`coverage_runner`).
- Artefact:
  `src/manifest/expand/property_tests/strategy/coverage.rs::generator_reaches_every_material_class`.
- Evidence: the test prints per-class counts with `--nocapture`; it fails if
  any class falls below threshold.
- Non-vacuity: a negative control in the same module forces the position
  strategy to `Just(fields.len())` and asserts that the middle-position class
  is then unreached, proving the classifier can report a miss.

### Residual gaps

- Query mode (`Conditional` and the synthetic `conditional` key) is outside
  the domain; existing tests in `action_condition_cases.rs` cover it.
- Error paths are not generated beyond what existing tests cover. In
  particular `expand_section` takes the section array with `std::mem::take`, so
  a failed expansion leaves that section empty in the caller's document. The
  loader discards the document on error, so this is not user-visible; ADR-0068
  records it as outside the contract.
- `foreach` expressions over globals, `glob()`, and stdlib calls are not
  generated.
- MiniJinja and `serde_json` internals are trusted through the axioms.

## Plan of work

Stage A (EP-M0) answers the open questions with throwaway probes. Stage B
(EP-M1) writes the contract record, proves the key-order defect red, and fixes
it. Stage C (EP-M2 to EP-M5) builds the shared generator and oracle, then one
obligation group per milestone, each with its mutation patch. Stage D (EP-M6)
adds the snapshot, the behavioural scenario if warranted, the documentation,
and the roadmap update, then runs the full gate set.

### Files to create

```plaintext
docs/adrs/adr-0068-manifest-expansion-invariant-contract.md
src/manifest/expand/tests/key_order_cases.rs
src/manifest/expand/tests/snapshots/… (insta, one file)
src/manifest/expand/property_tests/mod.rs
src/manifest/expand/property_tests/support.rs
src/manifest/expand/property_tests/model.rs
src/manifest/expand/property_tests/strategy/mod.rs
src/manifest/expand/property_tests/strategy/json.rs
src/manifest/expand/property_tests/strategy/when_expr.rs
src/manifest/expand/property_tests/strategy/coverage.rs
src/manifest/expand/property_tests/preserve.rs
src/manifest/expand/property_tests/order.rs
src/manifest/expand/property_tests/when_removed.rs
src/manifest/expand/property_tests/inject.rs
src/manifest/expand/property_tests/static_when.rs
src/manifest/expand/property_tests/pipeline.rs
docs/verification/mutations/
  manifest__expand__property_tests__preserve__expansion_copies_every_non_control_field.patch
  manifest__expand__property_tests__order__foreach_removal_keeps_authoring_key_order.patch
  manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order.patch
  manifest__expand__property_tests__when_removed__no_expanded_entry_carries_a_control_key.patch
  manifest__expand__property_tests__inject__each_expansion_carries_its_own_item_and_index.patch
  manifest__expand__property_tests__inject__expression_foreach_matches_inline_foreach.patch
  manifest__expand__property_tests__static_when__static_entries_survive_exactly_when_their_condition_holds.patch
```

If EP-M0 confirms a user-visible diagnostic difference, also create
`tests/data/when_middle_unknown_field.yml` and one scenario in
`tests/features/manifest.feature`.

### Files to edit

- `src/manifest/expand/mod.rs`: `shift_remove` for `foreach`; declare
  `#[cfg(test)] mod property_tests;` beside `mod tests;`.
- `src/manifest/expand/evaluation.rs`: `shift_remove` for `when`.
- `src/manifest/expand/tests/mod.rs`: declare `mod key_order_cases;`.
- `tests/kani_mutation_evidence_tests/contracts.rs`: add seven stems to
  `SUPPLEMENTAL_PROPERTY_PATCHES`, keeping the list sorted.
- `docs/netsuke-design.md`: cite ADR-0068 from the `ManifestMap` key-order
  paragraph and from section 2.5.
- `docs/developers-guide.md`: a *Manifest expansion suites* subsection under
  *Property-based testing with proptest*, mirroring *Emission determinism
  suites*; correct the `expand_foreach` bullet "entries without `foreach` are
  passed through unchanged" to say that static entries lose `when` and keep
  every other key in order.
- `docs/formal-verification-methods-in-netsuke.md`: state that the four
  expansion invariants and key order now exist, and where.
- `docs/repository-layout.md`: one entry for
  `src/manifest/expand/property_tests/` and its test-only support scope.
- `docs/contents.md`: index ADR-0068 and this ExecPlan.
- `docs/users-guide.md`: one sentence, only if EP-M0 confirms a
  user-visible diagnostic effect, stating that expansion keeps fields in
  authoring order.
- `CHANGELOG.md`: a `### Fixed` entry under `## Unreleased` for the key
  order.
- `Makefile`: refresh the census sentence above `PROPTEST_FILTER`.
- `docs/roadmap.md`: mark 4.3.2 and its sub-items done with a completion
  note, following the 4.3.1 entry's shape.

## Milestones and plateaus

### EP-M0 — feasibility probes (prototyping)

Outcome: answers to these questions, recorded in `Surprises & discoveries` and
`Decision log`, with every probe deleted afterwards.

1. Does a scratch rstest show `name, when, command, after` expanding to
   `name, after, command` on the current tree? (Expected yes.)
2. Through `manifest::from_str_with_env`, does a target written
   `name, when: "true", zz_first: 1, command, zz_last: 2` report an
   unknown-field error naming `zz_last` before the fix and `zz_first` after?
   Repeat for an action. A yes means the defect is user-visible and EP-M6 adds
   the behavioural scenario and the users' guide sentence.
3. For each `WhenExpr` form, does MiniJinja take the expression path or the
   template fallback, and does its verdict match the Rust oracle on 1,000
   generated cases? Confirm that `{{ … }}` fails `compile_expression`.
4. Does `serde_json::to_value(Value::from_serialize(x)) == x` hold for
   1,000 generated JSON values and for the boundary values `i64::MIN`,
   `i64::MAX`, `u64::MAX`, `-0.0`, `f64::MIN_POSITIVE`, and `5e-324`? Which of
   those can `serde_saphyr` actually produce from YAML text?
5. What is the per-case cost of expansion-level and pipeline-level
   properties? Choose case counts that keep the suite under the runtime
   tolerance.
6. What is the smallest YAML that `from_str_with_env` accepts for a static
   target (including the `netsuke_version` field)?

Requirements and gaps: none discharged; de-risks every obligation. Acceptance:
each answer recorded with the command and output that produced it. Conformance
check:

- No upstream requirement changes.
- The design is unchanged unless question 4 fails, which stops the plan.
- No probe is committed.

Recovery: probes live in a scratch module that `git checkout --` removes.
Remaining gaps: everything else. Compatibility decision: none.

### EP-M1 — contract record and key-order fix

Outcome: ADR-0068 written with status *Proposed*; the rstest regression module
`key_order_cases.rs` committed red-then-green; both removal sites use
`shift_remove`; the design document cites the ADR; `CHANGELOG.md` records the
fix.

ADR-0068 states the contract in numbered clauses so properties can cite them:
`C-1` non-control fields are copied verbatim; `C-2` key order is the source
order minus control keys, with injected `vars` appended; `C-3` `item` and
`index` precedence and types; `C-4` `when` is evaluated over the entry's `vars`
plus iteration values and then removed, for static and foreach entries in both
sections; `C-5` query mode and error-path document state are outside the
contract. It records the ADR-0003 reading and the rejection of Kani and Verus
with reasons.

Red-Green-Refactor:

- Red: add `key_order_cases.rs` with an rstest table over every position of
  `when` (static entry) and `foreach` (foreach entry) in a four-field entry, in
  both sections, asserting the key sequence with googletest
  `assert_that!(keys, elements_are![…])`. Run it and see the first and middle
  positions fail with the swapped order.
- Green: change the two `remove` calls to `shift_remove`. Run the module and
  see every case pass.
- Refactor: none expected; run the whole expansion test tree.

Acceptance: the focused command in `Concrete steps` shows the red failure, then
the pass; `make check-fmt`, `make typecheck`, `make lint`, and `make test`
pass. Conformance check:

- `DES-ORDER` now holds on the tested shapes.
- Only the two permitted production lines changed.
- No interface, dependency, or persisted-format change.
- Trace link `FV-EXP -> DES-ORDER -> EP-M1` current.

Recovery: revert the milestone's commits; nothing else depends on them yet.
Remaining gaps: generated coverage of every obligation. Compatibility decision:
none. Key order inside the in-memory manifest value is not a persisted or wire
format.

### EP-M2 — shared generator, oracle, and coverage

Outcome: `property_tests/{mod,support,model}.rs` and `strategy/` exist; the
`WhenExpr` oracle has rstest witness cases (one per operator and form,
including integer truthiness of zero); `OBL-GEN` passes with its negative
control.

Acceptance: `make proptest` lists
`manifest::expand::property_tests::strategy::coverage::generator_reaches_every_material_class`
and it passes; the four gates pass. Conformance check: no production change;
generator classes match the obligation list; abstraction scope recorded in the
developers' guide draft. Recovery: the module is additive; delete the directory
and its declaration. Remaining gaps: the properties themselves. Compatibility
decision: none.

### EP-M3 — preservation and order properties

Outcome: `preserve.rs` and `order.rs` with their three properties; the three
mutation patches committed and registered.

Acceptance: each property passes; applying each of `MUT-PRESERVE`,
`MUT-ORDER-FOREACH`, and `MUT-ORDER-WHEN` makes exactly its named property fail
with a shrunk counter-example (record the shrunk input); the four gates pass,
including the mutation-registry contract under `make test`. Conformance check:
trace links for `RM-4.3.2.a` and `DES-ORDER` current; no production change.
Recovery: revert the milestone commit. Remaining gaps: `RM-4.3.2.b` to
`RM-4.3.2.d`. Compatibility decision: none.

### EP-M4 — injection properties

Outcome: `inject.rs` with `OBL-INJECT` and `OBL-INJECT-EXPR`; patches
`MUT-INDEX` and `MUT-INLINE-NULL` committed and registered.

Acceptance and conformance as EP-M3, for `RM-4.3.2.c`. Recovery: revert the
milestone commit. Remaining gaps: `RM-4.3.2.b` and `RM-4.3.2.d`. Compatibility
decision: none.

### EP-M5 — condition properties

Outcome: `when_removed.rs`, `static_when.rs`, and `pipeline.rs`; patches
`MUT-WHEN-KEEP` and `MUT-STATIC-LEAK` committed and registered;
`MUT-STATIC-LEAK` also shown, by hand, to fail the pipeline property.

Acceptance and conformance as EP-M3, for `RM-4.3.2.b` and `RM-4.3.2.d`.
Recovery: revert the milestone commit. Remaining gaps: documentation and the
snapshot. Compatibility decision: none.

### EP-M6 — snapshot, scenario, documentation, and closure

Outcome: one insta snapshot test in `key_order_cases.rs`
(`expanded_document_snapshot`) over a fixture mixing static and foreach
entries, both sections, and control keys at middle positions, serialized as
YAML; the behavioural scenario and users' guide sentence if EP-M0 question 2
was answered yes; every documentation edit listed above; ADR-0068 set to
*Accepted*; the roadmap entry marked done; this plan's `Status:` set to
`COMPLETE` with `Outcomes & retrospective` written.

If the scenario is warranted, it reads:

```gherkin
Scenario: Parsing names the first unknown field in authoring order after when
  Given the manifest file "tests/data/when_middle_unknown_field.yml" is parsed
  When the parsing result is checked
  Then parsing the manifest fails
  And the error message contains "zz_first"
```

Acceptance: the full gate set in `Validation and acceptance` passes.
Conformance check: every trace link resolves to a passing test; ADR-0068 is
indexed in `docs/contents.md` (check by set comparison, not by count); no
upstream assumption was falsified without an upstream edit. Recovery:
documentation commits are independent and individually revertible. Remaining
gaps: those listed under *Residual gaps*. Compatibility decision: none.

## Concrete steps

Run every command from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/4a869f11-3ef8-4855-a172-afbc5e70fca2`
in the authoring session, or the root of whichever checkout holds branch
`4-3-2-manifest-expansion-invariant-proptests`. Capture long output with `tee`:

```sh
BRANCH="$(git branch --show-current)"
make check-fmt 2>&1 | tee "/tmp/check-fmt-netsuke-${BRANCH}.out"
make typecheck 2>&1 | tee "/tmp/typecheck-netsuke-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-netsuke-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
```

Run the gates sequentially, never in parallel. A build that prints
`[build-limits] … waiting for a compile slot` is queued, not hung.

Focused EP-M1 red and green runs:

```sh
cargo nextest run --all-features --lib \
  -E 'test(/manifest::expand::tests::key_order_cases/)' \
  2>&1 | tee "/tmp/key-order-netsuke-${BRANCH}.out"
```

Expected red transcript shape (before `shift_remove`):

```plaintext
FAIL [ … ] netsuke manifest::expand::tests::key_order_cases::when_removal_keeps_order::case_2_middle
  Expected: has elements: "name", "command", "after"
  Actual: ["name", "after", "command"]
```

Focused suite and mutation runs:

```sh
make proptest 2>&1 | tee "/tmp/proptest-netsuke-${BRANCH}.out"
PATCH=docs/verification/mutations/manifest__expand__property_tests__order__when_removal_keeps_authoring_key_order.patch
git apply "$PATCH"
cargo nextest run --all-features --lib \
  -E 'test(/property_tests::order::when_removal_keeps_authoring_key_order$/)' \
  2>&1 | tee "/tmp/mutation-netsuke-${BRANCH}.out"
git apply -R "$PATCH"
```

Expected: the property fails with `minimal failing input` naming an entry whose
control key sits before at least two fields; after `git apply -R` it passes.
Persisting the mutation run's seed is not wanted: delete any
`proptest-regressions/` file the mutation run creates before committing.

ADR-number re-check before the first push (needs a fetched remote):

```sh
for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
  git ls-tree -r --name-only "$ref" -- docs/ | grep -oE 'adr-[0-9]{3,4}-' | sort -u \
    | sed "s|\$| ${ref#refs/remotes/origin/}|"
done | sort -u | awk '$1 >= "adr-0060-"'
```

## Validation and acceptance

Acceptance is behavioural:

1. `make proptest` selects nine new tests under
   `manifest::expand::property_tests` (eight properties and the coverage test)
   and all pass.
2. For each of the seven committed patches, applying it makes exactly the
   named property fail with a shrunk counter-example, and reverting it makes
   the property pass. Record each shrunk input in `Artefacts and notes`.
3. The EP-M1 rstest module fails before the fix and passes after, as
   transcribed.
4. The coverage test reports every class at or above threshold, and its
   negative control reports the forced miss.
5. If warranted, the behavioural scenario fails before the fix (the error
   names `zz_last`) and passes after.

Quality criteria:

- Tests: `make test` passes, including
  `tests/kani_mutation_evidence_tests` and
  `tests/execplan_status_contract_tests.rs`.
- Verification: every obligation above discharged with its evidence and
  non-vacuity record; residual gaps stated.
- Lint and typecheck: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` pass.
- Documentation: `make markdownlint` (which runs `make spelling`) and
  `make nixie` pass.
- Mutation compile check: `make test-kani-mutations` passes once, at EP-M6.
- Performance: the new suite's test time, read from the nextest log, stays
  within the runtime tolerance.

Quality method: run `make check-fmt`, `make typecheck`, `make lint`, and
`make test` at the end of every milestone, sequentially, through the
`scrutineer` agent where available. Add `make markdownlint`, `make nixie`, and
`make doc-coverage` whenever documentation or Rust doc comments change, and
`make proptest` at EP-M2 onwards. `make test-workflow-contracts` is not
required, because no workflow or workflow-contract suite changes; an edit to
this ExecPlan is still exercised by `make test`.

## Idempotence and recovery

Every step is re-runnable. Mutation patches are applied and reverted with
`git apply` and `git apply -R`; if a run is interrupted mid-mutation,
`git diff --stat src/` shows the residue and `git checkout -- src/` clears it
(commit first so nothing else is lost). Proptest regression files created by
mutation runs must be deleted, not committed. EP-M0 probes are never committed.
If the ADR number collides, rename the file with `git mv` and update its
heading and every citation as a separate commit.

## Artefacts and notes

Record here, as work proceeds: the EP-M0 answers with transcripts; the EP-M1
red and green transcripts; each mutation's shrunk counter-example; the
coverage-class counts; the suite's measured runtime.

## Interfaces and dependencies

No production interface changes. The test-only interfaces are:

```rust
// src/manifest/expand/property_tests/strategy/mod.rs
pub(super) struct EntrySpec { /* section, fields, vars, foreach, when, positions */ }
pub(super) fn entry_spec() -> impl Strategy<Value = EntrySpec>;
pub(super) fn document_spec() -> impl Strategy<Value = DocumentSpec>;
impl EntrySpec { pub(super) fn to_value(&self) -> ManifestValue; }

// src/manifest/expand/property_tests/strategy/when_expr.rs
pub(super) enum WhenExpr {
    Bool(bool),
    Var(String),
    Cmp(Operand, CmpOp, Operand),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}
impl WhenExpr {
    pub(super) fn render(&self, form: WhenForm) -> String;
    pub(super) fn holds(&self, bindings: &Bindings) -> bool;
}

// src/manifest/expand/property_tests/model.rs
pub(super) fn expected_outputs(spec: &EntrySpec) -> Vec<ManifestValue>;

// src/manifest/expand/property_tests/support.rs
pub(super) fn strict_environment() -> minijinja::Environment<'static>;
pub(super) fn expand_document(doc: &mut ManifestValue) -> Result<ExpansionReport, TestCaseError>;
```

The exact signatures may change during EP-M2; the visibility may not exceed
`pub(super)` within `property_tests`. The suite is test-only support owned by
`src/manifest/expand/`; no other module may import it. A repository sweep found
no existing JSON-value or condition-expression strategy to reuse:
`src/ninja_gen/determinism/strategy/` generates IR graphs and
`src/stdlib/canonical_value/tests/properties.rs` generates MiniJinja values for
canonical keys, neither of which models manifest entries.

Dependencies used, all existing: `proptest` (strategies, `TestRunner`,
`RngSeed::Fixed`), `googletest` (`assert_that!`, `elements_are!`),
`pretty_assertions` (`assert_eq!` for map diffs in rstest cases), `insta`
(`assert_yaml_snapshot!`), `rstest`, `rstest-bdd`, `serde_json`,
`serde_saphyr`, and `minijinja`.

## Progress

- [x] (2026-10-31) Renamed the branch to
  `4-3-2-manifest-expansion-invariant-proptests`.
- [x] (2026-10-31) Reconnaissance of expansion code, existing tests,
  proptest infrastructure, mutation contract, and documentation.
- [x] (2026-10-31) Identified the `swap_remove` key-order defect from the
  locked `serde_json` source.
- [x] (2026-10-31) Drafted this ExecPlan.
- [ ] Expert design review and revision.
- [ ] User approval.
- [ ] EP-M0 to EP-M6.

## Surprises & discoveries

- Observation: `serde_json::Map::remove` is `swap_remove` under
  `preserve_order`, so expansion reorders keys whenever a control key is not
  last. Evidence: `serde_json-1.0.151/src/map.rs`, lines 156–165;
  `src/manifest/expand/mod.rs` line 242 and `src/manifest/expand/evaluation.rs`
  line 44 call `remove`. Impact: a real violation of `DES-ORDER`; EP-M1 fixes
  it.
- Observation: the existing property
  `foreach_preserves_generated_source_key_order` permutes three keys but always
  appends `foreach` last, the one position where `swap_remove` is harmless.
  Evidence: `src/manifest/expand/tests/foreach/property_cases.rs`. Impact: its
  generator cannot reach the defect; `OBL-ORDER` replaces it as evidence
  without deleting it.
- Observation: the existing generated modules are named `property_cases`,
  so `make proptest` does not select them. Impact: the new suite uses a
  `property_tests` segment; relocating the old modules is out of scope.
- Observation: the developers' guide says entries without `foreach` are
  "passed through unchanged", but static entries lose `when`. Impact: EP-M6
  corrects the sentence.

## Decision log

- Decision: place the suite at `src/manifest/expand/property_tests/`, a
  sibling of `tests/`. Rationale: the `property_tests` segment is required for
  `make proptest`; a child of `expand` reaches private helpers without widening
  visibility; the mutation registry resolves stems against
  `src/manifest/expand/property_tests/<leaf>.rs`. Date/Author: 2026-10-31,
  planning agent.
- Decision: fix the key-order defect in this item rather than defer it.
  Rationale: `FV-EXP` names key order as an expansion invariant, the design
  document promises it, and the fix is two calls with a red test; deferring
  would mean writing a property that encodes the defect. Date/Author:
  2026-10-31, planning agent.
- Decision: build `ManifestValue` directly in expansion-level properties and
  parse YAML only in the pipeline property. Rationale: YAML quoting of
  arbitrary strings would force the generator to filter or escape, and the
  expansion contract is stated over the parsed value. Date/Author: 2026-10-31,
  planning agent.
- Decision: use a strict-undefined MiniJinja environment in properties.
  Rationale: production uses `UndefinedBehavior::Strict`; a generator bug that
  references an undefined name then fails loudly instead of evaluating to
  false. Date/Author: 2026-10-31, planning agent.
- Decision: reject Kani and Verus for this item. Rationale: see *Method
  selection*. Date/Author: 2026-10-31, planning agent.
- Decision: no OrthoConfig change. Rationale: the item adds no configuration,
  flag, or help text. Date/Author: 2026-10-31, planning agent.
- Decision: provisionally take ADR-0068. Rationale: a sweep of every remote
  branch on 2026-10-31 found 0059 on `main`, 0060 on many branches, and
  0061–0067 on `20-1-1-ratify-progressive-enhancement-contracts`; 0061 is also
  on `issue-946-…`. No prose reservation of 0068 exists. Date/Author:
  2026-10-31, planning agent.
- Decision: keep the existing `property_cases.rs` modules unchanged.
  Rationale: they pin budget and filtering-report behaviour that this suite
  does not restate; deleting readable specification is forbidden by the
  constraints. Date/Author: 2026-10-31, planning agent.

## Outcomes & retrospective

Not yet started.
