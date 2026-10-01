# RFC 0015: Ordered collection algebra and truth predicates

## Preamble

- **RFC number:** 0015
- **Status:** Proposed
- **Created:** 2026-10-01
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.4
- **Originating issue:** [#596](https://github.com/leynos/netsuke/issues/596)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies the ordered collection algebra and the collection and truth
predicates: the filters `union`, `intersect`, `difference`,
`symmetric_difference`, `product`, `combinations`, `permutations`, and
`zip_longest`, and the tests `any`, `all`, `subset`, `superset`, `contains`,
`truthy`, and `falsy`. Together they let a manifest express a target matrix, a
feature-set intersection, or a filtered selection without a handwritten nested
loop and without an ordering that depends on a hash table. All fifteen are
pure, so all fifteen are available to manifest queries as well as to recipes,
and none needs a capability handle. RFC 0006 section 14.4 defines the algebra
as slice 3 and section 14.11 puts it in the recommended first wave, calling it
"slice 3 in full". The seven predicates are slice 8, which section 14.9 calls
"small, and a good candidate for pairing with whichever larger slice lands
alongside it"; this RFC is the pairing, so one document owns the whole of
roadmap step 6.4 rather than splitting a step across two review rounds.

## 2. Problem

A build matrix is today either written out by hand or produced by `shell()` out
to `python -c` or `jq`, which RFC 0006 section 2 records as the cost: a pure,
cacheable, capability-free planning expression becomes a subprocess with
ambient authority. The set operations are the worse half, because Ansible's own
set-backed filters return an ordering derived from a hash table, so a manifest
that used them could not promise the byte-identical Ninja clause 6.3 requires —
which is why RFC 0006 section 6.3 "explicitly rejects Ansible's set-backed
collection filters" rather than adopting them. The predicates are the other
direction: `selectattr('tags', 'contains', 'rust')` reads naturally and has no
spelling today, so a manifest writes the comparison into a `map` or drops to
`shell()`.

## 3. Goals and non-goals

- Goals:
  - Express a Cartesian product of feature sets, target triples, and
    optimization levels with the cardinality checked before allocation.
  - Perform set union, intersection, difference, and symmetric difference over
    a defined, first-appearance ordering rather than a hash order.
  - Answer "is this a subset of that" and "does this container hold that"
    over canonical equality, with the operands the right way round for
    `selectattr`.
  - Answer truthiness questions, including the string-to-boolean conversion
    Ansible performs, without reproducing its permissive fallback.
- Non-goals:
  - `zip`, rejected in RFC 0006 section 7.2 as already provided by MiniJinja.
  - `unique` and `uniq`, rejected in the same table and resolved in section
    11.3.
  - `version` and `version_compare`, which RFC 0016 owns.
  - `defined` / `undefined` and the task-result predicates, rejected in RFC
    0006 section 7.4 as meaningless in a build graph.
  - `issubset` / `issuperset`, rejected in section 7.6 as aliases, and the
    `nan` / `isnan` pair rejected there on principle.
  - The multiset semantics RFC 0006 section 8.8 declines to offer alongside
    the set ones, and Ansible's permissive string-to-boolean fallback.

## 4. Capability set

Fifteen helpers: eight filters and seven tests, all pure, all newly registered.

- `union` — every element of the subject, then the other operand's absent ones.
- `intersect` — the subject's elements the other operand also holds.
- `difference` — the subject's elements the other operand does not hold.
- `symmetric_difference` — each operand's elements the other lacks.
- `product` — the Cartesian product, with the rightmost operand varying
  fastest.
- `combinations` — combinations of a required length, by ascending index.
- `permutations` — permutations of a length defaulting to the input length.
- `zip_longest` — zip to the longest operand, with a **required** fill value.
- `any` — true when some element is truthy.
- `all` — true when every element is truthy.
- `subset` — set containment ignoring duplicates and order.
- `superset` — the same relation with the operands reversed.
- `contains` — membership, with the container as the test subject.
- `truthy` — truthiness, optionally converting the eight accepted spellings.
- `falsy` — the exact complement of `truthy`.

This section does not restate any contract. Each helper's kinds, options,
rejection conditions, and bounds are specified in
[RFC 0006 section 8.3](0006-ansible-inspired-template-standard-library.md#83-ordered-collection-algebra)
and
[section 8.8](0006-ansible-inspired-template-standard-library.md#88-collection-and-truth-predicates),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

This section discharges RFC 0006 section 6 for the fifteen helpers section 4
lists. Where a clause's consequence follows from RFC 0006 alone and is the same
for every group, the subsection says so in one line; where this group forces a
decision, the subsection states the decision.

### 5.1. Registry

| Helper                 | Namespace | Registration | Purity class | Manifest query |
| ---------------------- | --------- | ------------ | ------------ | -------------- |
| `union`                | Filter    | New          | Pure         | Yes            |
| `intersect`            | Filter    | New          | Pure         | Yes            |
| `difference`           | Filter    | New          | Pure         | Yes            |
| `symmetric_difference` | Filter    | New          | Pure         | Yes            |
| `product`              | Filter    | New          | Pure         | Yes            |
| `combinations`         | Filter    | New          | Pure         | Yes            |
| `permutations`         | Filter    | New          | Pure         | Yes            |
| `zip_longest`          | Filter    | New          | Pure         | Yes            |
| `any`                  | Test      | New          | Pure         | Yes            |
| `all`                  | Test      | New          | Pure         | Yes            |
| `subset`               | Test      | New          | Pure         | Yes            |
| `superset`             | Test      | New          | Pure         | Yes            |
| `contains`             | Test      | New          | Pure         | Yes            |
| `truthy`               | Test      | New          | Pure         | Yes            |
| `falsy`                | Test      | New          | Pure         | Yes            |

All fifteen rows are `New`: this group introduces fifteen helpers and extends
none. All fifteen are pure, so all fifteen fall inside RFC 0006 section 6.1's
52, and this RFC accounts for 15 of them — the largest single share of the
eight children.

### 5.2. Manifest-query availability

All fifteen are pure, so all fifteen register in `register_query_helpers` and
none registers in `register_disabled_query_helpers` as a stub. The consequence
this group adds is that a manifest query can answer a matrix question without
evaluating a target: `netsuke help targets` can enumerate the target triples a
build would produce by composing `product` over the declared feature sets, and
filter them with `selectattr('tags', 'contains', …)`, so "what will this build
produce" is computed by the same expression the recipe uses rather than by a
second traversal.

### 5.3. Determinism

Clause 6.3 is the clause this group exists to test, and RFC 0006 states the
objection to Ansible's implementation in the clause's own opening line: "No
helper may expose an iteration order derived from a hash table. This explicitly
rejects Ansible's set-backed collection filters." Thirteen of the fifteen
derive their result from the elements of an input collection — the eight
filters and the five quantifier and containment predicates — so their order is
a contract rather than an implementation detail.

| Helper                                         | Output order                                  |
| ---------------------------------------------- | --------------------------------------------- |
| `union`                                        | subject's order, then the other's absent ones |
| `intersect`, `difference`                      | the subject's order                           |
| `symmetric_difference`                         | each operand's own order, subject first       |
| `product`                                      | rightmost operand varying fastest             |
| `combinations`, `permutations`                 | ascending source index                        |
| `zip_longest`                                  | position, to the longest operand              |
| `any`, `all`, `subset`, `superset`, `contains` | none observable; boolean result               |
| `truthy`, `falsy`                              | none; single value in, boolean out            |

Three consequences this group decides:

- **The order is contractual, so a property test may not permute distinct
  elements.** `[1, 1, 2] | union([2, 3])` is `[1, 2, 3]`, and reordering an
  input's *duplicates* leaves it unchanged while reordering distinct elements
  does not. Roadmap task 6.4.5's property test holds the logical input order
  fixed for exactly this reason, and its warning is part of the contract rather
  than a test note.
- **The predicates read the canonical-key machinery the algebra builds.**
  Their results are booleans, so no ordering reaches an author through them,
  but a hash-set implementation of `subset` would be the observed defect one
  call frame away from an observable one — which is why clause 6.7's rule is
  stated for the seven tests as well as the eight filters.
- **No helper sorts, and none depends on how an operand was built.** A
  mapping's insertion history never reaches an output, which is what makes task
  6.4.5's hash-state property meaningful.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. That clause's substantive
rules address helpers that reach outside their arguments, and no helper in this
group does: none takes a `cap_std` handle, reads a path, or can violate the
clause's trapdoor rule about a filesystem predicate reporting `false` for an
out-of-scope path. The clause scopes itself to those helpers — "Filesystem
access goes through the injected `cap_std` workspace handle" — and this group
takes no filesystem access to route.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. No helper here takes a
`dialect` argument, none parses a platform path, and none renders a path. The
clause's per-platform documentation obligation still applies to each of the
fifteen entries, and the answer is the same for all of them, which is worth
stating once: the helpers are defined over MiniJinja values and canonical JSON,
neither of which has a platform variant.

### 5.6. Type and error contract

RFC 0006 sections 8.3 and 8.8 specify the kinds each helper accepts. What
follows is when a kind, an operand, or an option value is rejected, each
carrying a code from section 5.9.

| Helper                 | Accepted as                             | Rejects                                                                                                                                 |
| ---------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `union`                | two sequences                           | `wrong_kind` naming the operand; `uncanonical_value`                                                                                    |
| `intersect`            | two sequences                           | as `union`                                                                                                                              |
| `difference`           | two sequences                           | as `union`                                                                                                                              |
| `symmetric_difference` | two sequences                           | as `union`                                                                                                                              |
| `product`              | sequences, plus `repeat` and `*others`  | `wrong_kind`; `repeat_range`; `cardinality_exceeded`; `overflow`                                                                        |
| `combinations`         | a sequence, plus `r`                    | `wrong_kind`; `r_kind` for a negative or non-integer `r`; `cardinality_exceeded`; `overflow`                                            |
| `permutations`         | a sequence, plus `r` or `none`          | `wrong_kind`; `r_kind`; `cardinality_exceeded` naming the lower ceiling; `overflow`                                                     |
| `zip_longest`          | sequences, plus a required `fill_value` | `wrong_kind`; `fill_value_required` when it is omitted                                                                                  |
| `any`                  | a sequence                              | `wrong_kind`; `undefined_input` naming the index                                                                                        |
| `all`                  | a sequence                              | as `any`                                                                                                                                |
| `subset`               | two sequences                           | `wrong_kind`; `uncanonical_value` naming the element's kind                                                                             |
| `superset`             | two sequences                           | as `subset`                                                                                                                             |
| `contains`             | a sequence, mapping, or string          | `wrong_kind` for any other container; `uncanonical_value` naming the key's kind; `value_kind` for a non-string `value` against a string |
| `truthy`               | any value, plus `convert_bool`          | `spelling_unknown` enumerating the eight accepted spellings; `undefined_input`                                                          |
| `falsy`                | any value, plus `convert_bool`          | as `truthy`                                                                                                                             |

Three decisions this group adds:

- **`uncanonical_value` is a distinct code from `wrong_kind`.** A container of
  the wrong kind and an element with no canonical JSON form are different
  manifest bugs, and the second is the one a reader is likelier to meet: a
  callable or a `now()` timestamp cannot participate in canonical equality, per
  clause 6.7. The code names the element's kind.
- **Cardinality and overflow are two codes, not one.** `overflow` is checked
  arithmetic failing before a comparison can be made; `cardinality_exceeded` is
  an exact cardinality meeting table 3's ceiling. An author fixes the first by
  asking for fewer operands and the second by asking for a smaller `repeat` or
  a shorter input, so the two must not read the same — and the second is only
  actionable because section 5.8 requires the count to be exact rather than
  abandoned at the first product past the ceiling.

  The rule that keeps them distinct is that a diagnostic reports **an exact
  cardinality or the overflow code, never an estimate**, and `overflow` takes
  precedence: `cardinality_exceeded`'s message carries the exact count, so a
  count that cannot be expressed at all has nothing to report under it. All
  three helpers can reach `overflow` — a falling factorial of a long sequence
  outgrows the type for the same reason a `product` of long operands does — and
  the ceiling comparison decides only once the count is in hand.
- **`fill_value_required` names the omission rather than the type.** A missing
  `fill_value` is the error an author will actually hit, so it must not be
  reported as "wrong kind of none". Section 8.3 makes the argument required
  "because a silent `none` from entering a build graph unnoticed"; the
  diagnostic has to say what was omitted for that reasoning to arrive.

### 5.7. Canonical value equality

Clause 6.7 opens by naming this group: "Ordered set algebra, `subset`,
`superset`, `contains`, and duplicate detection all need a value-equality
relation that is deterministic and does not force values through a hash set."

- **Deduplication is by canonical key, and first appearance survives.** `union`
  removes duplicates within either input keeping first appearance; the other
  three filter helpers each deduplicate their half. Integer `1` and string
  `"1"` are different keys and both survive, because canonical JSON
  distinguishes them.
- **`subset` and `superset` are set relations, not multiset ones.** `[1, 1, 2]`
  is a subset of `[1, 2]`, and the two tests are exact converses. Section 8.8
  declines the multiset spellings: "no consumer has needed them, and offering
  both spellings would invite confusion".
- **No relation routes through a hash set.** Section 8.8 states the rule —
  "equality never routes through a hash set, so no unstable ordering can leak"
  — and adds that it "also governs the ordered set algebra in section 8.3". The
  implementation uses the order-preserving map clause 6.7 permits and never
  exposes its iteration order.
- **`contains` reads membership the same way `in` does.** For a sequence it is
  canonical equality against an element; for a mapping it is membership over
  **keys**, matching Jinja's `in`; for a string it is a substring test, and a
  non-string `value` is an error rather than `false`. Because a mapping is
  queried by key, an integer key is outside the canonical domain just as an
  integer element is, and raises `uncanonical_value` naming the key's kind
  rather than reporting `false`.
- **The canonical-JSON domain is narrower than the accepted kinds.** Clause 6.7
  defines it as "string keys at every mapping level", so a mapping with an
  integer key cannot participate in any relation here — the collision RFC
  0014's section 5.7 separates rather than a defect of this group. The
  diagnostic names the kind, per the clause's "typed error naming the value
  kind".

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation. This group reaches three of them, more than any other child:
`product` and `combinations` share the 100000-tuple ceiling, and `permutations`
takes the lower 10000-tuple one. The other twelve enforce none beyond the input
already materialized.

Three consequences this group decides:

- **Every cardinality is computed exactly, never estimated, and reported.**
  RFC 0006 section 6.8 requires that "the diagnostic reports both the computed
  cardinality and the ceiling", and section 8.3 that it report "the computed
  cardinality, the operand lengths, and the ceiling". An approximate count
  satisfies neither, so `product` multiplies the operand lengths,
  `combinations` evaluates a binomial, and `permutations` a falling factorial,
  each in checked arithmetic over the full value. The arithmetic therefore runs
  even when the result is certain to be rejected; what the check saves is the
  tuple materialization, not the multiplication. Counting costs one factor per
  operand, `r` factors, or `min(r, n)` respectively — never the result size —
  so it is cheap for exactly the inputs the clause exists to refuse.
- **`product`'s ceiling is on tuples, not on methods of reaching them.** The
  bound applies to the result's length, so `repeat` multiplies into the same
  ceiling rather than getting one of its own, and an empty operand yields an
  empty result without a check at all, per section 8.3.
- **An over-large request fails without allocating, which is a testable
  claim.** RFC 0006 section 6.8 opens with "rejects unreasonable expansion
  **before** allocating", and roadmap task 6.4.2's success criterion restates
  it: "an over-large request fails naming the computed cardinality and the
  ceiling, without allocating the result". A `product` of ten operands of ten
  elements each is ten billion tuples from a hundred elements of input — a
  hundred thousand times the ceiling, written in a manifest ten lines long — so
  the gap between checking and allocating is the whole point of the clause
  rather than a performance note.

The two `any`/`all` edge cases are not bounds and are decided here rather than
left to the implementation: an empty sequence yields `false` for `any` and
`true` for `all`, "following the usual quantifier convention", and an undefined
element is an error naming its index rather than a `false`. The second is
clause 6.6's strict-undefined rule reaching an element rather than an argument,
and it is the case a reader is likeliest to assume the other way round.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `CollectionError`, and exactly
one `impl From<CollectionError> for minijinja::Error`, per clause 6.9. Every
message is a Fluent key and every error carries a machine code, so the codes
are enumerated rather than described.

| Condition                            | Code                                                |
| ------------------------------------ | --------------------------------------------------- |
| not the expected kind                | `netsuke::jinja::collections::wrong_kind`           |
| element or mapping key outside it    | `netsuke::jinja::collections::uncanonical_value`    |
| undefined value or element           | `netsuke::jinja::collections::undefined_input`      |
| `repeat` below one or not an integer | `netsuke::jinja::collections::repeat_range`         |
| `r` negative or not an integer       | `netsuke::jinja::collections::r_kind`               |
| `zip_longest` without `fill_value`   | `netsuke::jinja::collections::fill_value_required`  |
| `contains` value of the wrong kind   | `netsuke::jinja::collections::value_kind`           |
| unrecognized boolean spelling        | `netsuke::jinja::collections::spelling_unknown`     |
| exact cardinality over ceiling       | `netsuke::jinja::collections::cardinality_exceeded` |
| checked arithmetic overflowed        | `netsuke::jinja::collections::overflow`             |

Each code's Fluent key is its reason in upper snake case under
`STDLIB_COLLECTIONS_`, per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>`
form, so `cardinality_exceeded` pairs with
`STDLIB_COLLECTIONS_CARDINALITY_EXCEEDED`. The `From` impl is what lets the
cardinality codes carry numbers rather than sentences: the error variant holds
the exact cardinality, the operand lengths, and the ceiling, and the conversion
renders them into the Fluent message.

The module segment is `collections`, in the plural, because it names the module
the helpers live in rather than describing the capability:
`src/stdlib/collections.rs` already exists and already holds `uniq`, `compact`,
`flatten`, and `group_by`, whose messages are already keyed
`stdlib.collections.*` and whose constants are already
`keys::STDLIB_COLLECTIONS_*`. A singular `collection` would put this group in a
Fluent namespace one character from an existing one, so a translator or a test
author reading `stdlib.collection.wrong_kind` would have no way to tell a typo
from a real key. Clause 6.9's `<MODULE>` is the module, and the module is
`collections`.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds fifteen, none an alias. The group does
carry two of RFC 0006's alias families, and a reviewer should see why the
outcome is fifteen names rather than the twenty spellings the survey lists.

`subset` and `superset` are registered and `issubset` and `issuperset` are not.
The two pairs are the same relation with the operands reversed, so registering
both would be the "redundant spelling" clause 6.10 forbids; RFC 0006 section
7.6 rejects the `issubset` / `issuperset` pair for that reason. The choice is
not arbitrary: the accepted names read as a statement about the *subject*,
`values is subset(other)`, which is what the test subject position gives an
author, whereas `issubset(a, b)` reads as a function of two arguments.

The second family is `contains` against MiniJinja's `in`: the same relation
with the operands the other way round, and RFC 0006 section 11.8 records why
both survive rather than one retiring. `selectattr` passes the attribute's
value as the test subject, so `selectattr('tags', 'contains', 'rust')` is the
only spelling that composes with it. Section 8.8 requires the guide to state
the relationship "explicitly so authors can pick the readable one", and roadmap
task 6.4.4 restates it as a success criterion at both entries.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. The clause's seven
obligations apply unmodified. Two group-specific notes rather than a
restatement, because this group's obligations are stated in the parent rather
than delegated.

Clause 6.11.4 requires property tests and RFC 0006 section 8.3 names the
properties for the algebra: "idempotence, commutativity where it holds (`union`
and `intersect` commute as sets but not as sequences, so the property is stated
over the canonical key set), and the identity that `a | union(b) | length`
equals `a | uniq | length + (b | difference(a) | uniq | length)`". Section 8.8
adds one more for the predicates: `truthy` and `falsy` "are exact complements
for every input on which both succeed". The commutativity qualification is the
part to read twice — the property is over the canonical key set, because the
sequences themselves legitimately differ in order.

Roadmap task 6.4.5 adds the end-to-end case, which is about *process* rather
than a helper: compile a representative matrix twice from the same inputs and
require byte-identical Ninja, then hold the logical input order fixed while
varying the internal hash state of the mappings consumed, and require the same
output. The first half is not unique to this child — task 6.3.5's isolated
workspace example also requires byte-identical Ninja across two runs — so the
distinction is the second half. Determinism across runs is what a build already
promises; determinism across *hash state* is what this clause needs and what a
run-to-run comparison cannot see, because the same process on the same machine
will usually reach the same insertion order twice. The task is explicit that
the property must **not** permute logical input order, because section 8.3
makes first-appearance order observable and "requiring otherwise would
contradict the contract".

### Clause discharge

| Clause | Discharge                                                                                                                     |
| ------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Fifteen pure `New` helpers; the fifteen section 5.1 rows are 15 of 52.                                                        |
| `6.2`  | All fifteen pure, so all register in `register_query_helpers`, none stubbed.                                                  |
| `6.3`  | Order defined from input order by all eight filters; the seven predicates reorder nothing.                                    |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken.                                                            |
| `6.5`  | No `dialect` argument; the helpers are platform-invariant.                                                                    |
| `6.6`  | Undefined rejected; three enumerated option sets; overflow separate from the cardinality comparison.                          |
| `6.7`  | Deduplication, subset, superset, and contains all keyed on the canonical key, never a hash set.                               |
| `6.8`  | Table 3's 100000-tuple ceiling for `product` and `combinations`, 10000 for `permutations`, counted exactly before allocation. |
| `6.9`  | One enum, one `From` impl, ten `netsuke::jinja::collections::*` codes.                                                        |
| `6.10` | Fifteen new names, two alias families resolved in section 5.10.                                                               |
| `6.11` | The clause's seven obligations, with the algebra and complement properties the parent states.                                 |

## 6. Dependencies

No new crate. RFC 0006 section 13.4 adds nothing for sections 8.3 and 8.8: the
group compiles against MiniJinja's value type, `indexmap` for first-appearance
ordering, and the canonical key from `serde_json_canonicalizer`, all of which
Netsuke already carries. `product`, `combinations`, and `permutations` are
implemented directly rather than through the `itertools` crate, because the
cardinality has to be counted and compared before any tuple is produced, and a
lazily yielding adapter would either move the count somewhere the clause does
not put it or leave the count to be inferred from a partially consumed iterator.

Within the RFC set, it requires the shared contract that RFC 0006 section
14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3 deliver: the
canonical value key, needed by every relation in section 5.7, and the
bounded-materialization helper, needed by the three cardinality checks in
section 5.8. It requires no other child RFC, and none requires it.

## 7. Delivery

Roadmap step 6.4, which implements this RFC in five tasks:

- 6.4.1. `union`, `intersect`, `difference`, and `symmetric_difference`, with
  first-appearance ordering and canonical-key deduplication.
- 6.4.2. `product`, `combinations`, and `permutations`, with checked
  cardinality and the lower ceiling for `permutations`.
- 6.4.3. `zip_longest`, with a required fill value.
- 6.4.4. The seven predicates, with `convert_bool` restricted to the closed
  eight-spelling vocabulary.
- 6.4.5. The matrix-determinism end-to-end suite over `product`, the set
  algebra, and `selectattr` with `contains`.

Each task carries the acceptance criteria that make this RFC checkable: the
idempotence and key-set commutativity properties, the length identity, an
over-large request failing without allocating, `truthy` and `falsy` as exact
complements, and two compilations of the same matrix emitting byte-identical
Ninja with the hash-state property holding them there.

## 8. Open questions

RFC 0006 section 16 assigns no question to this group, and that is a
consequence rather than an omission. Questions 1 and 4 belong to RFC 0013,
question 3 to RFC 0016, question 2 to RFC 0017, and question 6 to RFC 0019;
question 5 concerns the shared bounds of roadmap step 6.1, and question 7 is
resolved by roadmap task 7.1.1.

The one a reader might expect here is question 5, because section 5.8 reaches
table 3 three times. It decides no value: `product`, `combinations`, and
`permutations` enforce the ceilings table 3 already states, and the group
inherits the numbers rather than choosing them.

## 9. Recommendation

This group should be implemented third, at v0.1.x or later, and RFC 0006
section 14.11's recommended first wave says so: it takes "slice 3 in full"
alongside three helpers from slice 2. The group is the RFC set's strongest test
of its own determinism claim, because it is the case where Ansible's
implementation is *known* to be unordered and the replacement has to be
observably so — a matrix built from these helpers either emits byte-identical
Ninja twice or section 5.3's contract is false. It is also the largest single
share of the accepted set, fifteen of the fifty-seven, and the only one
spanning both namespaces, so it is where the registry's handling of tests
rather than filters is settled for the children that follow.
