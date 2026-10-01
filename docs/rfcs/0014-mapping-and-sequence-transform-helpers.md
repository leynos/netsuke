# RFC 0014: Mapping and sequence transform helpers

## Preamble

- **RFC number:** 0014
- **Status:** Proposed
- **Created:** 2026-10-01
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.3
- **Originating issue:** [#596](https://github.com/leynos/netsuke/issues/596)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies the mapping and sequence transform group: `combine`,
`dict2items`, `items2dict`, `extract`, `subelements`, and `rekey_on_member`.
Together they let a manifest layer a default configuration with a platform
overlay and a per-target overlay, convert between a mapping and a sequence of
key-value records, resolve one field out of a nested structure, and expand or
re-index a sequence of records, without a merge loop written by hand. All six
are pure, so all six are available to manifest queries as well as to recipes,
and none needs a capability handle. RFC 0006 section 14.3 defines this group as
slice 2 and calls it the highest-value slice for `vars`, `foreach`, per-entry
overrides, and configuration layering.

## 2. Problem

Layering configuration in a manifest is today three nested `foreach` loops and
a conditional per key. MiniJinja's `dict` and `update` idioms express a
right-biased merge over one level only, so a recursive merge, or one that
accumulates two sequences at a colliding key rather than replacing them, has no
expression at all and falls back to `shell()` out to `jq` or `yq` — turning a
pure, cacheable, capability-free planning expression into a subprocess with
ambient authority, which RFC 0006 section 2 records as the cost. The
re-indexing half is the same contortion in reverse: a manifest holding a
sequence of toolchain records has no way to build a lookup but a handwritten
loop. RFC 0006 section 15.2 rejects adding nothing here, on the grounds that
layering is the ordinary case rather than an advanced one.

## 3. Goals and non-goals

- Goals:
  - Merge mappings left to right with an explicit recursion flag and an
    explicit policy for colliding sequences, and state the merge laws that hold
    and the ones that deliberately do not.
  - Convert a mapping to a sequence of records and back, with the field names
    configurable and the duplicate policy explicit.
  - Resolve a key, or a key path, against a container, with missing-value
    behaviour stated at the point of use.
  - Expand parents against a nested child sequence, and re-index records by one
    member, keeping absence and shape mismatch apart.
- Non-goals:
  - `flatten`, rejected in RFC 0006 section 7.1 as already provided.
  - The `map`, `select`, `reject`, `selectattr`, `rejectattr`, and `groupby`
    wrappers, rejected in the same table as redundant.
  - `unique` and `uniq`, rejected in RFC 0006 section 7.2 and resolved in
    section 11.3.
  - `log`, `pow`, and `root`, deferred by RFC 0006 section 9.2. They share a
    surveyed table with `rekey_on_member` and nothing else.
  - The two accumulating list policies and the mapping-of-mappings input that
    RFC 0006 table 13 records as deliberate divergences. Sections 5.6 and 5.8
    state what replaces each.

## 4. Capability set

Six helpers, all filters, all pure, all newly registered.

- `combine` — merge mappings left to right under an explicit recursion flag and
  sequence policy.
- `dict2items` — convert a mapping into a sequence of two-key records.
- `items2dict` — convert a sequence of two-key records back into a mapping,
  under an explicit duplicate policy.
- `extract` — resolve a key, or a key path, against a container.
- `subelements` — expand parents against a nested child sequence.
- `rekey_on_member` — re-index a sequence of records by one member.

This section does not restate any contract. Each helper's kinds, options,
rejection conditions, and bounds are specified in
[RFC 0006 section 8.2](0006-ansible-inspired-template-standard-library.md#82-mapping-and-sequence-transforms),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

This section discharges RFC 0006 section 6 for the six helpers section 4 lists.
Where a clause's consequence follows from RFC 0006 alone and is the same for
every group, the subsection says so in one line; where this group forces a
decision, the subsection states the decision.

### 5.1. Registry

| Helper            | Namespace | Registration | Purity class | Manifest query |
| ----------------- | --------- | ------------ | ------------ | -------------- |
| `combine`         | Filter    | New          | Pure         | Yes            |
| `dict2items`      | Filter    | New          | Pure         | Yes            |
| `items2dict`      | Filter    | New          | Pure         | Yes            |
| `extract`         | Filter    | New          | Pure         | Yes            |
| `subelements`     | Filter    | New          | Pure         | Yes            |
| `rekey_on_member` | Filter    | New          | Pure         | Yes            |

All six rows are `New`: this group introduces six helpers and extends none. All
six are pure, so all six fall inside RFC 0006 section 6.1's 52, and this RFC
accounts for 6 of them.

### 5.2. Manifest-query availability

All six are pure, so all six register in `register_query_helpers` and none
registers in `register_disabled_query_helpers` as a stub. The consequence this
group adds is that a manifest query answers a layering question without
evaluating a target: `netsuke help targets` can resolve a target's effective
variables by composing `vars` through `combine` and print one field of the
result with `extract`, so "what does this target inherit" is computed by the
same expressions a recipe would use rather than by a second merge.

### 5.3. Determinism

Clause 6.3 requires every helper returning a sequence or mapping to define its
output order in terms of input order. This group is where that costs the most,
because four of its six helpers reorder or rebuild a collection, and each
defines its order separately.

- **A merge is not a re-sort.** `combine` keeps a key's first-appearance
  position and replaces its value in place; a key first seen in a later operand
  is appended in encounter order. A manifest merging the same overlays in the
  same order therefore emits byte-identical Ninja, which is the independence
  from hash iteration order roadmap task 6.3.1 names.
- **`subelements` inherits its order from both operands.** Parents appear in
  input order and each parent's children in their own order, so the result is a
  materialized sequence a `foreach` binding can rely on.
- **Both converters and the re-index agree on first appearance.** `dict2items`
  emits the input mapping's order, and `items2dict` and `rekey_on_member` emit
  the first appearance of each derived key, with `overwrite` replacing in place
  so a later record never moves an earlier key.
- **`extract` carries no ordering obligation**, returning a value rather than a
  collection; it is named only so a reader does not look for one.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. That clause's substantive
rules all address helpers that reach outside their arguments, and no helper in
this group does: none takes a `cap_std` handle or an injected reader, and none
can violate the clause's trapdoor rule about a filesystem predicate reporting
`false` for an out-of-scope path.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. No helper here takes a
`dialect` argument and none parses a platform path. One boundary is worth
fixing in place: `subelements`' `path` argument is a **key path**, whose
components are mapping keys split on `.`, and not a filesystem path. It must
never acquire a `dialect` argument, and its documentation entry says so,
because the two are the same shape and a reader who has just met `path_join`
will otherwise expect the argument to behave like one. A key containing a
literal dot is supplied as a sequence, which is the escape hatch a dialect
argument would have pretended to be.

### 5.6. Type and error contract

RFC 0006 section 8.2 specifies the kinds each helper accepts. What follows is
when a kind, a key, or an option value is rejected, each carrying a code from
section 5.9. Options are listed in section 5.9 rather than repeated here.

| Helper            | Accepted as                                   | Rejects                                                                                                                                |
| ----------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `combine`         | mapping, subject and every positional operand | `wrong_kind` naming the position and kind found; `list_merge_invalid`; `depth_exceeded`; `output_too_large`                            |
| `dict2items`      | mapping                                       | `wrong_kind`; `empty_name`; `names_equal`                                                                                              |
| `items2dict`      | sequence of mappings                          | `wrong_kind`; `missing_field` naming index and field; `key_kind`; `duplicates_invalid`; `duplicate_key` under the default policy       |
| `extract`         | mapping or sequence                           | `wrong_kind`; `index_kind` for a negative or non-integer index; `missing_key` naming the failing step; `not_a_container`               |
| `subelements`     | sequence of mappings                          | `wrong_kind` for a non-mapping parent; `path_kind`; `missing_path`, including `none` at the path; `not_a_sequence`; `output_too_large` |
| `rekey_on_member` | sequence of mappings                          | `wrong_kind`; `missing_member` naming the index; `key_kind`; `duplicates_invalid`; `duplicate_key` under the default policy            |

Three decisions this group adds:

- **Absence and shape mismatch are never the same error.** `subelements`
  separates `missing_path` from `not_a_sequence`, and `extract` separates
  `missing_key` from `not_a_container`. In both, the first is what a default or
  a skip flag may absorb and the second never is, because the wrong shape at a
  key that exists is a manifest bug rather than an absent entry. `skip_missing`
  and `default` therefore narrow exactly one condition each.
- **`none` is a value, and this group is where that shows.** Clause 6.6 makes
  undefined an error and `none` a value. `subelements` treats `none` at the
  path as absent, because the section 8.2 contract says so; `extract` returns
  `none` only when a `default` supplies it. Undefined is rejected by all six as
  `undefined_input`.
- **Every enumerated option is enumerated in its error.** `list_merge` names
  its four values, `items2dict`'s `duplicates` names its three, and
  `rekey_on_member`'s names its two, satisfying clause 6.6's enumerated-option
  rule and roadmap task 3.15.5. The two duplicate policies differ in size on
  purpose: a mapping cannot hold two values under one key, so `error` and
  `overwrite` are all it has, where a merge could be reordered.

### 5.7. Canonical value equality

Clause 6.7 governs equality, and this group is the first whose helpers derive
keys rather than only consuming them, so the clause decides when two derived
keys collide.

- **Duplicate detection is keyed on the canonical key.** `items2dict` and
  `rekey_on_member` compare derived keys by their RFC 8785 canonical form.
  Every derived key is a scalar by contract, so every one has a canonical form
  and the comparison never needs the clause's exclusions. Integer `1` and string
  `"1"` are different keys and both survive; two integer `1`s collide.
- **First appearance survives both the drop and the diagnostic.**
  `rekey_on_member(duplicates='overwrite')` replaces in place and keeps the
  original position, and the default policy's error names the repeated key and
  the index of its second occurrence.
- **The mappings this group produces need not be in the canonical-JSON
  domain.** Section 8.2 admits integer and boolean keys, and clause 6.7 defines
  its domain as string-keyed at every level, so a merged or rebuilt mapping
  with such a key is outside it. The round trip is therefore stated twice:
  `mapping | dict2items | items2dict` returns an equal mapping under clause 6.7
  for string-keyed input, and a structurally identical mapping, compared by key
  and value, for input whose keys are integers or booleans.
- **`combine` matches keys by identity, not by canonical key.** Integer key
  `1` and string key `"1"` produce two entries rather than one override, which
  is deliberate: collapsing them would make a merge depend on a rendering the
  author never wrote.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation, plus one output ceiling this group applies. `combine` enforces
table 3's nesting depth 128 when `recursive=true` and, with `subelements`, the
ceiling; the other four enforce none beyond the input already materialized.

Three consequences this group decides:

- **Two helpers amplify; both are bounded rather than excused.** RFC 0013's
  serializers needed an output pre-count because a shared value's logical size
  far exceeds its footprint, and two helpers here reproduce that shape.
  `subelements` gives each of a parent's `c` children a pair carrying the
  parent's *whole* content, so one parent becomes `c` copies of itself; a
  sequence merged with itself under `combine`'s `append` doubles its content
  while both operands still hold one copy. Both count *content* rather than
  pairs or elements, with checked arithmetic, and reject above 8 MiB before
  allocating, per clause 6.8's rule for materialized output. A nested
  application inherits the bound.
- **The other four cannot amplify.** Each rearranges its operands' elements —
  `dict2items` pairs a value with its key, `items2dict` and `rekey_on_member`
  re-index records without copying a value, and `extract` returns a sub-value —
  so none's logical size exceeds its inputs' sum times the operand count.
- **Depth is the recursion flag's bound.** `combine`'s `recursive=true` is the
  only descending path; it fails `depth_exceeded` before the result is built.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `TransformError`, and exactly
one `impl From<TransformError> for minijinja::Error`, per clause 6.9. Every
message is a Fluent key and every error carries a machine code, so the codes
are enumerated rather than described.

| Condition                   | Code                                            |
| --------------------------- | ----------------------------------------------- |
| not the expected kind       | `netsuke::jinja::transform::wrong_kind`         |
| empty field name            | `netsuke::jinja::transform::empty_name`         |
| equal field names           | `netsuke::jinja::transform::names_equal`        |
| unknown list policy         | `netsuke::jinja::transform::list_merge_invalid` |
| unknown duplicate policy    | `netsuke::jinja::transform::duplicates_invalid` |
| element missing a field     | `netsuke::jinja::transform::missing_field`      |
| key not found on the path   | `netsuke::jinja::transform::missing_key`        |
| traversal into a non-value  | `netsuke::jinja::transform::not_a_container`    |
| derived key of wrong kind   | `netsuke::jinja::transform::key_kind`           |
| absent path on a parent     | `netsuke::jinja::transform::missing_path`       |
| value present but not a seq | `netsuke::jinja::transform::not_a_sequence`     |
| repeated derived key        | `netsuke::jinja::transform::duplicate_key`      |
| nesting too deep            | `netsuke::jinja::transform::depth_exceeded`     |
| result content too large    | `netsuke::jinja::transform::output_too_large`   |
| undefined input             | `netsuke::jinja::transform::undefined_input`    |

Each code's Fluent key is its reason in upper snake case under
`STDLIB_TRANSFORM_`, per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>` form,
so `list_merge_invalid` pairs with `STDLIB_TRANSFORM_LIST_MERGE_INVALID`. The
module segment is `transform` rather than `mapping` or `sequence`, because one
enum serves all six helpers and reads as the capability group rather than one
subject's kind. `combine`, `dict2items`, `extract`, and `subelements` share
`wrong_kind` rather than defining near-identical variants, and what tells their
failures apart is a field rather than a second message — the operand position,
the element index, the path step — which is what makes each diagnostic
actionable without re-running the manifest.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds six, none an alias. The group does
carry one of the six collisions roadmap task 6.1.7 requires a resolution for,
and a reviewer should see why the outcome is six names rather than five.

`dict2items` sits beside MiniJinja's own `items`, and RFC 0006 section 11.7
records the resolution. The two are not aliases and the second is not a
redundant spelling of the first: `items` yields two-element *sequences*, while
`dict2items` yields two-*key mappings* whose field names are arguments. The
difference is what consumers bind to — `selectattr`, `groupby`, and a `foreach`
binding read a field by name and cannot address a positional pair — so both
names register and neither retires. The near-miss is that with default field
names and a positional-only consumer the two are interchangeable; that overlap
is harmless, and it is why clause 6.10's one-name-per-capability line cannot be
read as one-*shape*-per-capability.

The group's other pressure is the reverse of an alias, and is settled in the
document rather than by a name: `rekey_on_member` and `items2dict` both build a
mapping from a sequence of records, and `rekey_on_member` is the narrower one —
it consumes the record's own fields, where `items2dict` consumes two named
ones. A manifest holding `[{name, value}]` records can reach either, and
section 8.2 directs the mapping-of-mappings case to `items2dict` rather than
widening `rekey_on_member` to accept it.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. The clause's seven
obligations apply unmodified. One group-specific note rather than a
restatement: clause 6.11.4 names "composition laws for `combine`" among the
property tests it requires and section 8.2 fixes their scope, so this is the
one place where the parent clause delegates a *law* rather than a technique.
Identity and self-merge idempotence are asserted across the policies that
satisfy them, associativity only when `recursive=false` under `replace` and
`keep`, and the recursive non-associativity counterexample is a regression test
rather than a property, being a single input triple that demonstrates the law's
absence. Roadmap task 6.3.5 adds the end-to-end example over three composed
layers, the acceptance case for the group rather than for any one helper.

### Clause discharge

| Clause | Discharge                                                                                                               |
| ------ | ----------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Six pure `New` helpers; the six section 5.1 rows are 6 of 52.                                                           |
| `6.2`  | All six pure, so all register in `register_query_helpers`, none stubbed.                                                |
| `6.3`  | Merge keeps first-appearance position; pair, converter, and re-index orders all defined from input order.               |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken.                                                      |
| `6.5`  | No `dialect` argument; `subelements`' key path is not a filesystem path.                                                |
| `6.6`  | Undefined rejected, `none` accepted; absence and shape mismatch separated; three enumerated option sets.                |
| `6.7`  | Duplicate detection keyed on the canonical key; round trip stated over the canonical-JSON domain and tested outside it. |
| `6.8`  | Table 3's nesting depth in `combine`'s descent, plus an 8 MiB content ceiling on `subelements` and `combine`.           |
| `6.9`  | One enum, one `From` impl, fifteen `netsuke::jinja::transform::*` codes.                                                |
| `6.10` | Six new names, no alias family, and the `items` collision resolved in section 5.10.                                     |
| `6.11` | The clause's seven obligations, plus `combine`'s merge laws at the scope section 8.2 fixes.                             |

## 6. Dependencies

No new crate. RFC 0006 section 13.4 adds nothing for section 8.2: the group
compiles against MiniJinja's value type, `indexmap` for first-appearance
ordering, and the canonical key from `serde_json_canonicalizer`, all of which
Netsuke already carries. Adding no dependency is one reason it can lead the
second wave.

Within the RFC set it requires the shared contract RFC 0006 section 14.1's
"slice 0" describes, which roadmap steps 6.1.2 and 6.1.3 deliver: the canonical
value key, needed by section 5.7's duplicate detection and by `combine`'s merge
laws, and the bounded-materialization helper, needed by section 5.8's depth and
output checks. It requires no other child RFC, and none requires it. RFC 0006
section 14.3 states the same two prerequisites as slice 0.

## 7. Delivery

Roadmap step 6.3, which implements this RFC in five tasks:

- 6.3.1. `combine`, with the recursion flag, the four list policies, and the
  merge laws of section 5.11.
- 6.3.2. `dict2items` and `items2dict`, with the duplicate policy.
- 6.3.3. `extract`, with the missing-value behaviour of section 5.6.
- 6.3.4. `subelements` and `rekey_on_member`.
- 6.3.5. The end-to-end layered-configuration manifest example.

Each task carries the acceptance criteria that make this RFC checkable: the
recursive non-associativity counterexample as a regression test, the property
tests over identity, self-merge idempotence, associativity, and hash-order
independence, the two converters' round trip, and an example whose generated
Ninja is byte-identical across two runs.

## 8. Open questions

RFC 0006 section 16 assigns no question to this group, and that is a
consequence rather than an omission. Question 1 and question 4 belong to RFC
0013, question 3 to RFC 0016, question 2 to RFC 0017, and question 6 to RFC
0019; question 5 concerns the shared bounds of roadmap step 6.1, and question 7
is resolved by roadmap task 7.1.1.

The one question a reader might expect here is question 5, because section 5.8
applies bounds. It decides no value: `combine` enforces table 3's nesting
depth, and the 8 MiB output ceiling is the one RFC 0006 section 6.8's input row
and RFC 0013's serializers already use, so the group inherits both rather than
choosing them. Making either configurable would change the parent's table, not
this RFC, which states that defaults are constants in the first slice.

## 9. Recommendation

This group should be implemented second, at v0.1.x or later, and RFC 0006
section 14.11's recommended first wave already says so: it takes `combine`,
`dict2items`, and `items2dict` from slice 2 ahead of all of slice 3. Layering
is what `vars`, `foreach`, and per-entry overrides exist to express, and a
manifest that cannot merge two layers writes the loop by hand or reaches for
`shell()`. It is also a good second step through the contract: the first with
helpers that derive keys, so clause 6.7 becomes a decision a helper makes
rather than a statement about serializers, and the first where a helper's
*output* rather than its input decides which bound applies.
