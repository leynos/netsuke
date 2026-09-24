# Architectural decision record (ADR) 021: Focused child RFCs for survey RFCs

## Status

Accepted.

## Date

2026-09-11

## Context and problem statement

RFC 0006 is a survey RFC: it enumerates every ansible-core candidate it
considered and records a disposition for each — accept, defer, or reject. Its
section 7 is the candidate matrix, section 8 specifies the accepted helpers,
and sections 9 and 10 record the deferred and rejected remainder. The survey
accepted 57 new helpers, plus three options on existing ones.

A survey RFC of that shape reaches a size at which its accepted set can no
longer be specified to the standard a normative contract needs. RFC 0006
section 6 states the cross-cutting contract every accepted helper must
discharge — capability scoping, diagnostics, localization, purity, tests, and
documentation — and closes by saying that a child which does not satisfy every
clause is not complete. Discharging that contract for 60 helpers inside a
2,100-line survey is not reviewable. The per-helper obligations are the part a
reviewer must read closely, and they sit in the part of the document least
likely to be read closely, because the surrounding survey is a catalogue.

The difficulty is not specific to RFC 0006. Any survey RFC with a large
accepted set has the same shape, and the same absence of a mechanical check
that its accepted set is fully allocated. Nothing in the build would notice a
helper that the survey accepted, no child RFC owns, and no roadmap task
schedules.

## Decision drivers

- **Reviewability.** A reviewer should be able to hold one capability group's
  obligations in mind at once, without the rest of the catalogue competing for
  attention.
- **Traceability.** Every accepted helper must resolve to one owner and one
  scheduled task, and no candidate the survey rejected may reappear as a
  registered helper under a rejected spelling.
- **Reversibility.** The split must be abandonable part-way. The survey's own
  sections 7 to 10 must keep their meaning whether one child RFC exists or
  eight do.
- **Derivation over transcription.** The enforcement must read the documents
  rather than restate them, so a document edit and a test edit cannot disagree.
- **Narrow scope.** The convention must not be mistaken for a general
  amendment mechanism, because the corpus already has one.

## Requirements

### Functional requirements

- The survey RFC's accepted set is partitioned into capability groups, each
  owned by exactly one focused child RFC and one roadmap step.
- Each child RFC carries the survey's cross-cutting contract, discharged
  per clause, and a registry of the helpers it introduces.
- The survey RFC records the allocation in a coverage map, one row per child,
  naming the section 8 subsections that child owns.
- A rejected or deferred candidate is absent from every child registry.

### Technical requirements

- The allocation is checked by a repository test that reads the survey RFC,
  the child RFCs, and the roadmap, and derives from them every fact it asserts:
  the accepted and deny sets, each helper's namespace and registration kind,
  the table 11 counts, the section 6.1 purity aggregate, section 6's clause
  list, each child's registry, and the roadmap steps. The test does carry a
  small number of constants, and they are deliberately not facts about the
  survey but anchors for reading it — which subsection numbers hold candidate
  tables, the three renamed helpers, the three optioned helpers, and the
  proposed-helper count used to cross-check the derived one. An anchor is prose
  the parser has to be told where to look for; a fact is prose the parser is
  asked to reproduce. Only the latter can go stale in a way a reviewer would
  miss, and the amendment procedure below is what keeps the former aligned.
- The test names the file and line of a violation and states the violated
  obligation.
- The survey RFC's own disposition sections are not moved, so an abandoned
  split leaves every existing cross-reference and contract intact.

## Options considered

### Option A: focused child RFCs with a coverage map

One child RFC per capability group, each owning a contiguous slice of the
survey's section 8, with the allocation recorded in the survey and enforced by
a derived test. The survey keeps its candidate matrix, its dispositions, and
its clause list; it gains only the map.

### Option B: registries in place, no child RFCs

Keep the accepted set in the survey RFC and add a registry table per capability
group inside section 8, roughly 140 lines in total. No RFC number is spent and
nothing is irreversible, but the per-helper obligations still live in the
survey, and a normative contract discharge — which is what a reviewer signs off
— never receives its own review.

### Option C: unnumbered per-group design documents

Write one design document per group on the pattern the repository already uses
for roadmap step 6.11. Cheaper than Option A and fully reversible, but a
document that is not an RFC cannot carry a normative contract discharge, and
the commissioned task called for RFCs.

| Topic                | Option A      | Option B     | Option C        |
| -------------------- | ------------- | ------------ | --------------- |
| Reviewable unit      | One group     | Whole survey | One group       |
| RFC numbers spent    | Eight         | None         | None            |
| Reversible part-way  | Yes           | Yes          | Yes             |
| Contract discharged  | Per child RFC | In place     | In a design doc |
| Mechanically checked | Yes           | Yes          | Yes             |

_Table 1: Comparison of the split options._

## Decision outcome / proposed direction

A survey RFC whose accepted set is too large to specify in place is split into
focused child RFCs, one per capability group, and records the allocation in a
coverage map in its own delivery section. Option A is adopted for RFC 0006.

The convention has four parts:

1. **The survey keeps its dispositions.** Sections 7 to 10, the clause list in
   section 6, and the numbering are unchanged. The split adds a coverage map;
   it does not move a specification.
2. **One child per capability group.** Each child RFC owns a contiguous slice
   of the survey's accepted-helper sections, and carries that group's registry,
   its per-clause discharge, and its per-helper obligations. A group is sized
   so a reviewer can read the whole child in one sitting.
3. **The map is the allocation.** One row per child, naming the section 8
   subsections owned, the existing helpers gaining an option, the roadmap step
   that delivers the group, and whether the child has been written. The rows
   partition the accepted set exactly.
4. **A derived test is the guard rail.** The test reads the survey's section 7
   disposition column, section 8, section 14's map, each child registry, and
   the roadmap, and fails on a dropped, double-owned, or forbidden name.

The scope is deliberately narrow. The convention applies to **survey RFCs**:
documents that enumerate a large candidate set with a per-candidate
disposition. It does not apply to design documents, and it does not replace the
`Amends` convention that RFCs 0009 to 0011 use for normative amendments to RFC
0001. A normative change to an existing RFC is still an amendment to that RFC,
not a new child of it.

### The child RFC template

A child RFC follows the repository's RFC sections in the order the
[documentation style guide](documentation-style-guide.md) requires, and adds
section 5, which is where the cross-cutting contract is discharged. Copy the
skeleton below literally. Three of its headings are parsed by name, so a child
that renames one fails before any of its content is read; each carries a note
naming the constant that reads it.

```markdown
# RFC 00NN: <title>

## Preamble

- **RFC number:** 00NN
- **Status:** Proposed
- **Created:** YYYY-MM-DD
- **Parent RFC:** RFC 0006, Ansible-inspired template standard-library
  expansion
- **Roadmap step:** 6.N
- **Originating issue:** [#596](https://github.com/leynos/netsuke/issues/596)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

<What this group buys a manifest author, in three or four sentences.>

## 2. Problem

<The subprocess contortion this group removes, from RFC 0006 section 2.>

## 3. Goals and non-goals

- Goals:
  - <Goal>
- Non-goals:
  - <Every deferred or rejected candidate adjacent to this group, named.>

## 4. Capability set

<One entry per helper: name, one-line purpose, and a link to its contract in
RFC 0006 section 8.N. This section does not restate the contract.>

## 5. Cross-cutting contract conformance

### 5.1. Registry

<The five-column table. Mandatory. The heading is parsed literally by
`registries.rs`, which matches `### 5.1. Registry`; a child that retitles it
fails with "has no registry table at ### 5.1. Registry" before any row is
read.>

### 5.2. Manifest-query availability

### 5.3. Determinism

### 5.4. Capability boundary

### 5.5. Platform contract

### 5.6. Type and error contract

### 5.7. Canonical value equality

### 5.8. Resource bounds

### 5.9. Diagnostics and localization

### 5.10. Naming and alias policy

### 5.11. Documentation and testing obligations

### Clause discharge

<The two-column discharge table: one row per clause of RFC 0006 section 6, the
clause id in backticks, and how this group meets it. It closes section 5,
after the eleven clause subsections, because it resolves all eleven rather than
adding a twelfth. Unnumbered on purpose: numbering it 5.6 would collide with
the clause subsection of that number, whose title — "Type and error contract" —
RFC 0006 clause 6.6 carries too. `clauses.rs` matches this heading, and the
table under it, as `Clause discharge`.>

| Clause | Discharge |
| ------ | --------- |
| `6.1`  | <...>     |
| `6.11` | <...>     |

## 6. Dependencies

<Crates from RFC 0006 section 13.4, and the child RFCs this one requires.>

## 7. Delivery

<The roadmap tasks that implement this RFC, by number.>

## 8. Open questions

<RFC 0006 section 16 questions assigned to this group, carried over
unresolved.>

## 9. Recommendation

<One paragraph: why this group's helpers belong in the v0.1.x line.>
```

### The registry row shape

Section 5.1's table has exactly these five columns. The cells are parsed, not
prose, and each column has one accepted vocabulary:

| Helper      | Namespace | Registration | Purity class | Manifest query |
| ----------- | --------- | ------------ | ------------ | -------------- |
| `path_join` | Filter    | New          | Pure         | Yes            |
| `abs`       | Test      | New          | Pure         | Yes            |
| `basename`  | Filter    | Option added | Pure         | Yes            |

_Table 2: The registry row shape._

- **Helper** — backticked; a name registered twice in one registry is an error.
- **Namespace** — `Filter`, `Test`, or `Function`.
- **Registration** — `New`, or `Option added` for one of the three existing
  helpers gaining a behaviour-preserving option. The distinction is
  load-bearing rather than descriptive: the purity aggregate counts `New` rows
  only, because RFC 0006 section 6.1's 52/4/1 counts the 57 proposed helpers,
  and an `Option added` row is an existing helper being extended.
- **Purity class** — one of RFC 0006 table 2's six values.
- **Manifest query** — `Yes` or `No`, matching the column of the same name in
  RFC 0006 table 2 rather than introducing a second vocabulary for the same
  fact. The cell is cross-checked against the purity class: `Yes` is admissible
  only for a pure helper, because clause 6.2 admits only pure helpers to the
  manifest-query environment and registers the non-pure ones as always-failing
  stubs rather than omitting them. A non-pure row therefore reads `No` and
  still resolves in both environments; the `No` is the stub's disposition, not
  its absence.

### The worked section

The template above is a shape. This is that shape filled in, for RFC 0013, the
group roadmap step 6.2 delivers: the first child written from it, and so the
one that settles what a discharge reads like before seven more inherit whatever
it settles. It is a specimen, not the artefact. When RFC 0013 is written it
carries this section, and that copy is the normative one.

It is reproduced filled in rather than described, because the two things a
reviewer has to judge — whether a clause subsection says something the clause
does not, and whether the discharge table resolves all eleven clauses — are
visible only in a completed copy. One formatting note carries over with it:
`markdownlint` caps a fenced block at 120 columns and `mdtablefix` does not
reflow one, so the tables below are sized to fit rather than laid out for the
page. A child RFC is real Markdown rather than a fence, so it inherits neither
limit, and its tables may be as wide as they read best.

```markdown
## 5. Cross-cutting contract conformance

This section discharges RFC 0006 section 6 for the five helpers section 4
lists. Where a clause's consequence follows from RFC 0006 alone and is the same
for every group, the subsection says so in one line; where this group forces a
decision, the subsection states the decision.

### 5.1. Registry

| Helper          | Namespace | Registration | Purity class | Manifest query |
| --------------- | --------- | ------------ | ------------ | -------------- |
| `from_json`     | Filter    | New          | Pure         | Yes            |
| `from_yaml`     | Filter    | New          | Pure         | Yes            |
| `from_yaml_all` | Filter    | New          | Pure         | Yes            |
| `to_yaml`       | Filter    | New          | Pure         | Yes            |
| `to_nice_json`  | Filter    | New          | Pure         | Yes            |

All five rows are `New`: this group introduces five helpers and extends none.
All five are pure, so all five fall inside RFC 0006 section 6.1's 52, and this
RFC accounts for 5 of them.

### 5.2. Manifest-query availability

All five are pure, so `from_json`, `from_yaml`, `from_yaml_all`, `to_yaml`, and
`to_nice_json` all register in `register_query_helpers`, and none registers in
`register_disabled_query_helpers` as a stub. The consequence is that
`netsuke help targets` gains five working helpers and no new always-failing
stub. One caveat, carried rather than resolved here: if RFC 0006 section 16
question 1 is answered by registering `to_nice_yaml` solely to raise a
diagnostic naming `to_yaml(indent=4)`, that registration introduces no accepted
helper and so is not a section 5.1 row; it is decided in section 8.

### 5.3. Determinism

Two group-specific obligations follow from clause 6.3, and both are testable
without reading prose.

- **Key order is an output, not a side effect.** `from_json` preserves object
  order because `serde_json` is built with `preserve_order`; `from_yaml`
  preserves mapping order; `to_yaml` and `to_nice_json` emit insertion order
  unless `sort_keys=true`. A round trip therefore returns a mapping in the
  order it went in, and the tests assert order, not merely equality.
- **Trailing-newline behaviour is a contract, not a convention.** `to_yaml`
  ends with exactly one trailing newline and `to_nice_json` with none, so one
  composes as a whole document and the other composes inside a larger one. The
  asymmetry is deliberate and is pinned by a test at each end.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. The clause's substantive
rules all address helpers that reach outside their arguments, and no helper in
this group does: none takes a `cap_std` handle, none takes an injected reader,
and none can violate the clause's trapdoor rule about a filesystem predicate
reporting `false` for an out-of-scope path.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. The clause's obligations
attach to helpers whose behaviour varies by platform or that parse another
platform's syntax: no helper here takes a `dialect` argument, none can fail
with a platform diagnostic, and all five emit LF on every platform.

### 5.6. Type and error contract

RFC 0006 section 8.1 specifies the kinds each helper accepts. What follows is
the conditions under which a kind, a key, or an option value is rejected, each
carrying a code from section 5.9. Per helper, because the three parsing
helpers and the two serializers do not share a rejection set.

- `from_json` accepts a string. It rejects `wrong_kind`, `syntax`,
  `duplicate_key`, `depth_exceeded`, and `length_exceeded`.
- `from_yaml` accepts a string. It rejects every `from_json` condition, and adds
  `unsupported_key` for a sequence or mapping key, `special_tag`, `merge_key`,
  `alias_budget`, and `document_count` for a stream that is not exactly one
  document.
- `from_yaml_all` accepts a string and rejects every `from_yaml` condition,
  except that the input-length and node budgets apply to the whole stream
  rather than to each document.
- `to_yaml` accepts any value except undefined. It rejects `undefined_input`
  and `indent_out_of_range` outright, plus `unsupported_key` when
  `sort_keys=true` meets a mapping key with no canonical JSON form, and
  `unsupported_kind` for a value that has none.
- `to_nice_json` accepts any value except undefined, and rejects the same four
  conditions as `to_yaml`, with the difference that section 8.1 states its key
  rule directly: integer and boolean keys are rendered in canonical string
  form and every other key kind is rejected rather than coerced.

Three decisions this group adds:

- **`none` is accepted; undefined is not.** Clause 6.6 makes undefined an error
  and makes `none` a value. Every helper here follows that split, and the line
  between them is where a manifest author is most likely to be surprised, so
  each of the five documents it explicitly rather than leaving the clause to
  imply it.
- **A duplicate key is rejected with a position.** `from_json` and `from_yaml`
  name the duplicated key and the offset of its second occurrence. This is the
  one place the group adds detection the underlying parser does not perform,
  and the reason is that last-key-wins is a silent data-loss trapdoor in a
  build manifest rather than a tolerable convenience.
- **The two `indent` ranges differ on purpose.** `to_yaml` accepts 1 to 8 and
  `to_nice_json` accepts 0 to 8. Zero is meaningful for JSON, where it means
  compact output, and meaningless for block YAML, where it would mean no
  indentation at all. Both reject an unknown value with an error enumerating
  the accepted range, satisfying clause 6.6's enumerated-option rule and
  roadmap task 3.15.5.

### 5.7. Canonical value equality

Clause 6.7 governs `to_yaml`'s and `to_nice_json`'s `sort_keys=true` and
nothing else in this group. The obligation here is to say which of the clause's
exclusions this group can meet, and to fix how the round trips are stated.

- `sort_keys=true` sorts mapping keys by their RFC 8785 canonical key, so two
  mappings that differ only in insertion order serialize identically. That is
  clause 6.7's relation applied to a key-ordering decision, and it is why the
  option can promise deterministic output at all.
- The clause excludes undefined, callables, and the `now()` timestamp object,
  because none has a canonical JSON form. Undefined is already rejected on
  input by clause 6.6; the other two are rejected on output as
  `unsupported_kind`, because a manifest can hold a callable or the result of
  `now()` and pass it to a serializer, and clause 6.7 makes that a typed error
  naming the value kind rather than a silent rendering.
- The group defines **no second equality relation** for round-trip testing.
  `value | to_yaml | from_yaml` and `value | to_nice_json | from_json` are
  asserted equal under clause 6.7's relation. A looser relation for tests would
  make the property tests agree with an implementation the clause does not
  describe.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation. What this group adds is where each one is enforced, because the JSON
and YAML parsers do not share a code path.

| Helper          | Bounds enforced                                      |
| --------------- | ---------------------------------------------------- |
| `from_json`     | input 8 MiB; nesting depth 128                       |
| `from_yaml`     | input 8 MiB; depth 128; alias expansion 100000 nodes |
| `from_yaml_all` | the same three, over the whole stream                |
| `to_yaml`       | none; output is a function of a bounded input        |
| `to_nice_json`  | none; output is a function of a bounded input        |

Three consequences this group decides:

- **`from_yaml_all`'s budget is stream-wide.** A stream of many small documents
  is bounded in total, so a caller cannot evade the input ceiling by splitting
  an expansion bomb across document boundaries. The diagnostic reports the
  stream total rather than a per-document figure, which is what makes the bound
  auditable after the fact.
- **The alias budget is the one bound that can fail the implementation slice
  rather than the input.** Clause 6.8 requires the expansion to be rejected
  before allocating, and if `serde-saphyr` cannot bound alias expansion then
  this RFC rejects aliases outright and records that in the guide, per RFC 0006
  section 8.1 and roadmap task 6.2.2. That choice is carried as section 8's
  open question 4 and is not resolved here.
- **The serializers enforce nothing, and that is a decision rather than an
  omission.** Neither allocates proportionally to anything but its input, so a
  bound would reject documents a parser had already accepted. The row reads
  "none" instead of being left blank so that a reviewer sees the absence was
  chosen.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `InterchangeError`, and
exactly one `impl From<InterchangeError> for minijinja::Error`, per clause 6.9.
Every message is a Fluent key and every error carries a machine code. The codes
are this group's contribution to clause 6.9's policy, so they are enumerated
rather than described.

| Condition     | Code                                            |
| ------------- | ----------------------------------------------- |
| not a string  | `netsuke::jinja::interchange::wrong_kind`       |
| syntax        | `netsuke::jinja::interchange::syntax`           |
| duplicate key | `netsuke::jinja::interchange::duplicate_key`    |
| key kind      | `netsuke::jinja::interchange::unsupported_key`  |
| special tag   | `netsuke::jinja::interchange::special_tag`      |
| merge key     | `netsuke::jinja::interchange::merge_key`        |
| alias budget  | `netsuke::jinja::interchange::alias_budget`     |
| document count | `netsuke::jinja::interchange::document_count`  |
| depth         | `netsuke::jinja::interchange::depth_exceeded`   |
| length        | `netsuke::jinja::interchange::length_exceeded`  |
| undefined     | `netsuke::jinja::interchange::undefined_input`  |
| indent        | `netsuke::jinja::interchange::indent_out_of_range` |
| value kind    | `netsuke::jinja::interchange::unsupported_kind` |

Each code's Fluent key is the code's reason in upper snake case under
`STDLIB_INTERCHANGE_`, so `wrong_kind` pairs with
`STDLIB_INTERCHANGE_WRONG_KIND` and `indent_out_of_range` with
`STDLIB_INTERCHANGE_INDENT_OUT_OF_RANGE`, per clause 6.9's
`keys::STDLIB_<MODULE>_<CONDITION>` form.

The module segment is `interchange` rather than `json` or `yaml`, because one
enum serves both parsers and both serializers and the code names the capability
group, not the syntax. All five helpers — `from_json`, `from_yaml`,
`from_yaml_all`, `to_yaml`, and `to_nice_json` — reach their errors through this
enum: every `Error::new` call in the group's leaf functions is replaced by a
variant of it, so a caller can tell an interchange failure from a manifest
diagnostic by the code alone. Clause 6.9 rejects ad hoc construction at this
scale, and a group with thirteen conditions is the case it names.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds five, none an alias. One of the clause's
own examples is still live here rather than settled: `to_nice_yaml` is rejected
by RFC 0006 section 10.2 as redundant with `to_yaml(indent=...)`, and whether
that rejection is expressed as outright absence or as a diagnostic-raising
registration is RFC 0006 section 16 question 1, carried unresolved to section 8
and decided at roadmap task 6.2.3.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. The clause's seven
obligations apply unmodified. One group-specific note rather than a
restatement: the round-trip property tests clause 6.11.4 requires are the two
named in section 5.7 under clause 6.7 canonical equality, and the
serialization-determinism property it names is the same proposition as section
5.3's key-order requirement, tested from the other side.

### Clause discharge

| Clause | Discharge                                                     |
| ------ | ------------------------------------------------------------- |
| `6.1`  | Five pure `New` helpers; the five section 5.1 rows are 5 of 52. |
| `6.2`  | All five pure, so all register in `register_query_helpers`, none stubbed. |
| `6.3`  | Mapping order in and out; one trailing newline for `to_yaml`, none for `to_nice_json`. |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken. |
| `6.5`  | No `dialect` argument; all five emit LF everywhere.            |
| `6.6`  | Undefined rejected, `none` accepted; duplicates rejected positionally; both `indent` ranges enumerated. |
| `6.7`  | `sort_keys` sorts by canonical key; both round trips under canonical equality. |
| `6.8`  | Table 3's input and depth bounds, stream-wide for `from_yaml_all`, plus the alias budget. |
| `6.9`  | One enum, one `From` impl, thirteen `netsuke::jinja::interchange::*` codes. |
| `6.10` | Five new names, no alias family, none reused across namespaces. |
| `6.11` | The clause's seven obligations, plus the two round trips and the determinism property. |
```

## Goals and non-goals

### Goals

- Make the survey's accepted set fully allocated and mechanically checked.
- Give each capability group one reviewable, normative document.
- Keep the split reversible at every milestone before the last.

### Non-goals

- Changing any disposition the survey recorded.
- Generalizing the convention beyond survey RFCs.
- Replacing the amendment convention for normative changes to existing RFCs.
- Scheduling the work outside the roadmap. Delivery is tracked by roadmap
  tasks, not by per-child issues.

## Amendment procedure

When a helper is added, removed, or renamed after the split, edit the
touchpoints below in this order. The order matters only in that each step's
subject must exist before the next step can cite it; the coverage test then
fails until all of them agree.

1. **The survey's section 7 row** — record or change the disposition, and cite
   the section 8 subsection that specifies the helper.
2. **The survey's section 8 subsection** — add, remove, or rename the helper so
   the specification matches.
3. **The survey's section 14 coverage map** — the owning row's `Owns` clause
   must still claim the helper. A new section 8 subsection needs a row to own
   it, and a ninth capability group needs a new child RFC number and a new row.
4. **The owning child RFC's registry** — its section 5.1 row carries the
   helper, namespace, registration kind, purity class, and manifest query.
5. **The roadmap task** in the owning step — name or rename the helper there,
   so the capability remains scheduled.

Three further touchpoints are not per-helper, and each is reached by a change
of a different kind. They are listed separately because a helper edit does not
touch them and an edit that does is easy to forget:

1. **The survey's section 3.2 namespace lists** — a helper added to a namespace
   the survey does not already list there belongs in the corresponding list.
   The test derives each helper's namespace from section 7, so a section 3.2
   list is not what it reads; the lists are what a _reviewer_ reads, and a
   helper absent from both is a helper the survey never introduces.
2. **The survey's section 6.1 purity statement and table 11 counts** — both are
   prose counts written in number words, and both are derived rather than
   transcribed by the test. Changing a helper's purity class therefore means
   editing the sentence that states how many helpers are pure, and adding or
   removing an accepted helper means editing table 11's totals. The test fails
   when they disagree, so this step is enforced rather than merely advised.
3. **The test's own anchors** — the subsection numbers, the renamed and
   optioned helper lists, and the proposed-helper count named in the technical
   requirements above. These are the only places a survey fact is written down
   twice. A change that moves a candidate table to a different subsection, or
   that changes which helpers are renamed or optioned, must edit the anchor and
   the survey together; a change that edits only one of them stops the suite
   reading the survey at all, which is a louder failure than a wrong count.

Removing a helper without removing its registry row fails the ownership check,
which reports a name the survey no longer accepts. Renaming one moves the old
spelling into the deny set, because the deny set is the complement of the
accepted set; a child registry that reintroduces the old spelling then fails
the forbidden-name check. Neither failure needs a new rule: both follow from
the derivation.

## Known risks and limitations

- **The survey is not ratified.** RFC 0006 is `Proposed`, and no RFC in the
  corpus has been ratified. Freezing its dispositions into a test may prove
  premature. The mitigation is structural: because section 8 does not move, a
  later change costs a registry row and a map row rather than a document
  rewrite.
- **Eight consecutive numbers are spent.** Allocation is irreversible once
  merged. Numbers are therefore allocated lazily, one per child at the commit
  that creates it, and a reservation recorded in a branch reserves nothing.
- **The `Owns` grammar is small by design.** It expresses "every helper in
  this section", "all but one", and "only this one", with clauses joined by
  semicolons. A capability group that needs a different split requires a new
  clause form and a parser change, which is the intended cost of keeping the
  grammar reviewable.
- **The split is disproportionate for a small accepted set.** The convention
  earns its cost only when the accepted set cannot be specified in place; for a
  handful of helpers, Option B is cheaper and is the fallback.

## Architectural rationale

The repository's documents are the source of truth for their own contracts, and
its tests derive what they check rather than transcribing it. This decision
applies both: the survey remains the only place a disposition is recorded, and
the child RFCs remain the only place a group's obligations are specified, with
the coverage map as the single join between them. Splitting by capability
rather than by delivery slice keeps each child aligned with one roadmap step
and one purity profile, so a child's contract is homogeneous enough to review
as a unit.

## Implementation references

- Survey and coverage map:
  [RFC 0006 section 14.13](rfcs/0006-ansible-inspired-template-standard-library.md)
- Coverage contract test: `tests/rfc_stdlib_coverage_tests.rs` and the
  `tests/rfc_stdlib_coverage/` module tree
- Delivery tracking: [Netsuke roadmap section 6](roadmap.md)
- Child RFCs 0013 to 0020, reserved by RFC 0006 section 14.13
