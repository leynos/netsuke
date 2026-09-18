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

## Decision Drivers

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
  the child RFCs, and the roadmap, and transcribes none of them.
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
fails until all five agree.

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
