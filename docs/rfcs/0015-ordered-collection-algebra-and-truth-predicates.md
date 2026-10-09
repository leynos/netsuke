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
| `product`              | sequences, plus `repeat` and `*others`  | `wrong_kind`; `repeat_range`; `cardinality_exceeded`; `output_too_large`; `overflow`                                                    |
| `combinations`         | a sequence, plus `r`                    | `wrong_kind`; `r_kind` for a negative or non-integer `r`; `cardinality_exceeded`; `output_too_large`; `overflow`                        |
| `permutations`         | a sequence, plus `r` or `none`          | `wrong_kind`; `r_kind`; `cardinality_exceeded` naming the lower ceiling; `output_too_large`; `overflow`                                 |
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
- **`output_too_large` is a third bound rather than a fourth cardinality code.**
  It bounds a different quantity — what the admitted tuples hold — so it can
  reject a request `cardinality_exceeded` has already passed and must not be
  folded into it. An author fixes this one by shortening the values in the
  input, which is neither of section 5.8's other two remedies: the counts are
  already legal and the arithmetic has already succeeded. Precedence follows
  the same rule as the other two, and the order is cardinality first because a
  tuple that will never be built need not be measured.
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
  non-string `value` is an error rather than `false`. The two container kinds
  therefore fail for different reasons, and the difference is worth stating
  because the symmetry is tempting and wrong. A mapping queried by a key the
  canonical domain excludes raises `uncanonical_value` naming the key's kind
  rather than reporting `false`. A sequence has no such exclusion: an integer
  element is an ordinary JSON scalar with a canonical form, so
  `[1, 2] is contains(1)` is `true`, and a `value` that is itself uncanonical —
  undefined, a callable, the `now()` timestamp — is what raises the diagnostic.
- **The canonical-JSON domain excludes integer *keys*, not integer *scalars*.**
  Clause 6.7 requires "string keys at every mapping level", and JSON objects
  take string keys, so a mapping with an integer key cannot participate in any
  relation here. That restriction is about the *key position* and reaches no
  further: RFC 8785 admits integers, booleans, strings, nulls, sequences, and
  string-keyed mappings, and an integer used as a sequence element or as a
  `value` argument is inside the domain. The two cases are separate, and
  deriving the scalar rule from the key rule — as an earlier draft of this
  section did, reading "an integer key is outside the canonical domain just as
  an integer element is" — would contradict this section's own set-operation
  examples, which operate on integer elements. RFC 0014's section 5.7 separates
  an integer key `1` from a string key `"1"` when it detects a derived-key
  collision, but that is a rule about *which derived keys collide*, not an
  admission of integer keys to the canonical-JSON domain; the two documents
  agree rather than conflict, and section 5.7 says so in its own third bullet.
  The key diagnostic names the kind, per the clause's "typed error naming the
  value kind".

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation, plus one output ceiling this group applies. This group reaches
three of table 3's rows, more than any other child: `product` and
`combinations` share the 100000-tuple ceiling, and `permutations` takes the
lower 10000-tuple one. The other twelve enforce none beyond the input already
materialized.

| Bound         | Value  | Where it applies                              |
| ------------- | ------ | --------------------------------------------- |
| Output tuples | 100000 | `product`, `combinations`                     |
| Output tuples | 10000  | `permutations`                                |
| Output length | 8 MiB  | the three combinatorial helpers in this group |

Four consequences this group decides:

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
- **A tuple ceiling is not an output ceiling, and this group needs both.** The
  two quantities are independent, and neither bounds the other. Cardinality is
  the number of tuples; output is what those tuples hold, and a tuple's width
  is the *third* quantity again — the number of elements in each. Section 8.3
  defines that width for `product` as `(1 + others | length) * repeat`, so
  width is a function of the operand *count* and `repeat` and never of the
  operand *lengths*. A count ceiling cannot bound it, because cardinality and
  width move independently: `[[0]] | product(repeat=1000000)` is a single
  operand of one element, so its cardinality is one and the check passes, while
  its one tuple is 1 000 000 elements wide — one million, not two, because the
  operand count is one and `1 * 1000000` is the clause's own figure. A hundred
  such operands would be 100 000 000 wide at the same cardinality of one, and
  it is `repeat` alone that makes the gap reachable without a large input. The
  `combinations` tail makes the same gap without `repeat` at all:
  `C(100000, 99999)` is exactly 100000, which sits *on* the ceiling rather than
  over it, so a 100000-element input yields a result of 100000 tuples each
  99999 wide — nine billion elements, from a request the cardinality check is
  obliged to admit. `permutations` is the nearest to safe, since its lower
  10000-tuple ceiling caps it at 35,280 elements, but that figure is a count of
  elements rather than of content, and each element is a value the author
  supplies: seven elements of 8 MiB each become 282 GB of materialized tuples
  through 5,040 of them. So all three count *content* rather than tuples or
  elements, with checked arithmetic, abandoning the walk the moment the running
  total passes 8 MiB, and fail with `output_too_large` at the ceiling RFC
  0013's serializers, RFC 0014's amplifying transforms, and RFC 0016's
  `regex_replace` already apply. The count is taken before any tuple is built,
  per clause 6.8, which is what makes the guarantee hold for the inputs the
  cardinality check admits rather than only for the ones it refuses.
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
| materialized content over 8 MiB      | `netsuke::jinja::collections::output_too_large`     |
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

| Clause | Discharge                                                                                                                                               |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Fifteen pure `New` helpers; the fifteen section 5.1 rows are 15 of 52.                                                                                  |
| `6.2`  | All fifteen pure, so all register in `register_query_helpers`, none stubbed.                                                                            |
| `6.3`  | Order defined from input order by all eight filters; the seven predicates reorder nothing.                                                              |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken.                                                                                      |
| `6.5`  | No `dialect` argument; the helpers are platform-invariant.                                                                                              |
| `6.6`  | Undefined rejected; three enumerated option sets; overflow separate from the cardinality comparison.                                                    |
| `6.7`  | Deduplication, subset, superset, and contains all keyed on the canonical key, never a hash set.                                                         |
| `6.8`  | Table 3's 100000-tuple ceiling for `product` and `combinations`, 10000 for `permutations`, plus an 8 MiB output ceiling, all counted before allocation. |
| `6.9`  | One enum, one `From` impl, eleven `netsuke::jinja::collections::*` codes.                                                                               |
| `6.10` | Fifteen new names, two alias families resolved in section 5.10.                                                                                         |
| `6.11` | The clause's seven obligations, with the algebra and complement properties the parent states.                                                           |

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
14.1's "slice 0" describes, which roadmap steps 6.1.2, 6.1.3, and 6.1.4
deliver: the canonical value key (roadmap task 6.1.2), needed by every relation
in section 5.7; the bounded-materialization helper (roadmap task 6.1.3, which
requires 6.1.2), needed by the three cardinality checks in section 5.8; and the
domain-error and diagnostic scaffolding (roadmap task 6.1.4) that section 5.9's
`CollectionError` enum is built on, per clause 6.9. It requires no other child
RFC, and none requires it. RFC 0006 section 14.4 names canonical equality and
the cardinality bound as this slice's slice 0 requirement; the scaffolding
reaches it through section 14.1's own sentence that "a slice that adds no
helper still uses the domain-error scaffolding for the helpers it does add",
which section 6.9 makes a contract for every group that registers a code.

## 7. Delivery

Roadmap step 6.4, which implements this RFC in five tasks:

- 6.4.1. `union`, `intersect`, `difference`, and `symmetric_difference`, with
  first-appearance ordering and canonical-key deduplication.
- 6.4.2. `product`, `combinations`, and `permutations`, with checked
  cardinality, checked output content, and the lower ceiling for `permutations`.
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

Two of those criteria are the ones that check section 5.8, and both name their
input, because an over-large request is only decisive when the ceiling it
crosses is stated. The two ceilings need two *different* inputs, and an earlier
draft of this section used one input for both by mistake:
`[[]] | product(repeat=100001)` has a single operand containing one empty
sequence, so its cardinality is one and section 8.3 makes it an empty result —
it crosses neither ceiling and demonstrates nothing.

Task 6.4.2 therefore succeeds on **cardinality** when `range(100001) | product`
fails naming the computed cardinality (100001), the operand lengths, and the
ceiling — and it should be read alongside the empty-operand case, which must
return an empty result rather than an error, because the two together are what
show the check is a comparison rather than a blanket refusal of large `repeat`
values. It succeeds on **output** when a request the cardinality check is
obliged to admit does not: `[[0]] | product(repeat=1000000)` is a single tuple
of one million elements, so it fails `output_too_large` rather than returning,
and `range(100000) | combinations(99999)` fails the same way instead of
materializing 100000 tuples of width 99999. Both are checked *before* the first
tuple is built, which is what distinguishes the bound from a post-hoc
measurement.

## 8. Open questions

RFC 0006 section 16 assigns no question to this group, and that is a
consequence rather than an omission. Questions 1 and 4 belong to RFC 0013,
question 3 to RFC 0016, question 2 to RFC 0017, and question 6 to RFC 0019;
question 5 concerns the shared bounds of roadmap step 6.1, and question 7 is
resolved by roadmap task 7.1.1.

The one a reader might expect here is question 5, because section 5.8 reaches
table 3 three times. It chooses no table 3 value: `product`, `combinations`, and
`permutations` enforce the ceilings table 3 already states, and the group
inherits those numbers rather than deciding them. The output ceiling is a
different case and is stated as such — it is the group's own rather than a
table 3 row, applied at the 8 MiB RFC 0013's serializers, RFC 0014's amplifying
transforms, and RFC 0016's `regex_replace` already use, so it too is inherited
rather than chosen. Making any of the four configurable would change the
parent's table rather than this RFC, which states that defaults are constants
in the first slice.

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
