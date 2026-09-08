# 6.1.1. Split the RFC 0006 accepted set into focused child RFCs

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

[RFC 0006](../rfcs/0006-ansible-inspired-template-standard-library.md) is a
2132-line document that surveys every filter, test, and function exposed by
`ansible-core` 2.21.3, records an accept, defer, or reject disposition for each
one, and specifies fifty-seven new Netsuke helpers across ten capability
groups. It is deliberately not one implementation change: its section 14 says
so in as many words, and its section 17 recommends scheduling the capability
groups as focused children after v0.1.0 final.

Nothing has yet performed that split. Today a contributor who wants to
implement, say, the mapping transforms must read all 2132 lines to discover
which of them bind, and a reviewer has no document smaller than the whole RFC
against which to review one capability group. Worse, there is no mechanical way
to answer the question the roadmap actually asks: is every accepted capability
covered exactly once, and is every deferred or rejected candidate covered not
at all?

After this change a contributor gains nine focused, independently reviewable
RFCs — one per phase-6 capability step — each of which states the full
cross-cutting contract obligations for its own helpers rather than deferring to
Ansible or to a section number in a much larger document. RFC 0006 remains, but
as an umbrella: it keeps the survey, the dispositions, the rejections, the
naming-collision resolutions, and the normative cross-cutting contract, and it
gains a coverage map naming the child RFC that owns each accepted capability.

The observable result is documentation plus one executable contract. A reader
should be able to run `make test` and see a new test binary assert that every
accepted helper name in RFC 0006 is specified by exactly one RFC, that no
deferred or rejected name is specified by any RFC, and that the helper counts
agree with RFC 0006 table 11. Deleting a helper from a child RFC, adding it to
two, or quietly reintroducing a rejected Ansible spelling all make that test
fail by name.

This plan is approval-gated. It must be reviewed and explicitly approved before
implementation begins.

## Scope divergence from the roadmap's literal wording

Read this section before anything else; it explains why the plan does not match
the roadmap text you will find in the working tree.

`docs/roadmap.md` line 744 currently reads:

```plaintext
- [ ] 6.1.1. Split the RFC 0006 accepted set into focused child issues.
```

and its success bullet requires "exactly one open child issue". The task as
commissioned instead asks for focused **child RFCs and accompanying roadmap
tasks**, with success measured as "exactly one RFC and accompanying roadmap
task". Child GitHub issues and child RFC documents have materially different
content requirements and different gates: an issue needs no `## Preamble`, no
markdownlint pass, and no `docs/contents.md` entry, whereas an RFC needs all
three.

This plan implements the commissioned interpretation, and therefore also
rewrites task 6.1.1's own roadmap text so the roadmap and the delivered
artefacts agree. That rewrite is itself part of the deliverable, recorded as
decision `D8` below. If the reviewer prefers the literal roadmap wording —
GitHub issues rather than RFC documents — stop before Milestone `EP-M1` and say
so; the audit work in `EP-M0` is useful under either reading, but everything
after it is not.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- Do not implement this plan until the user explicitly approves it.
- This work is documentation-only apart from one new test binary and its
  wiring. Do not add or change any Netsuke helper, template registration,
  locale key, command-line flag, configuration field, or Cargo dependency.
  Adding a helper is what the child RFCs *specify*; it is not what this task
  *does*.
- Do not renumber, rename, or delete RFCs 0001 to 0012.
  `docs/documentation-style-guide.md` forbids renumbering after publication.
- Do not change any accept, defer, or reject disposition recorded in RFC 0006
  sections 7, 9, or 10. This task partitions the accepted set; it does not
  relitigate it. If the audit finds a disposition that appears wrong, record it
  in `Surprises & discoveries` and escalate rather than editing it.
- Do not resolve any of RFC 0006's seven open questions in section 16. Each is
  assigned to a delivery step and must be settled by that step's implementer,
  not by this split. Carry each one into the child RFC that owns it.
- Every child RFC must carry a release target of v0.1.x or later and must not
  widen the v0.1.0 hardening release defined by
  [#594](https://github.com/leynos/netsuke/issues/594).
- No child RFC may specify a capability that RFC 0006 section 9 defers or
  section 10 rejects. This is the half of the success criterion that is easiest
  to breach by accident when lifting prose.
- Documentation prose must follow `docs/documentation-style-guide.md` and use
  en-GB-oxendict spelling. Body prose wraps at 80 columns; code blocks at 120;
  tables and headings are not wrapped.
- Markdown must be `mdtablefix`-canonical. `make check-fmt` runs
  `scripts/check-markdown-format.sh`, which reformats a staged copy and diffs
  it against the tracked file. Run `make fmt` before `make check-fmt`.
- Run validation commands sequentially, never in parallel, and capture output
  with `tee` under `/tmp`. The repository relies on build caching.
- Commit only after gates pass, using the file-based message workflow
  (`git commit -F`), not `git commit -m`.
- Use the shared default Cargo cache. Do not create an isolated one.

## Tolerances (exception triggers)

- Scope: if implementation requires changing a file outside the set listed in
  `Interfaces and dependencies`, stop and escalate.
- Dependencies: if the coverage test needs any crate not already in
  `Cargo.toml`'s dev-dependencies, stop and escalate. The parsing required is
  line-oriented and needs nothing new.
- Disposition drift: if the audit in `EP-M0` finds that RFC 0006's own totals,
  helper lists, and prose disagree in a way that changes which helpers are
  accepted, stop and escalate before writing any child RFC.
- Interface: if the coverage test would require changing any `src/` file, stop
  and escalate. It is a repository-contract test over `docs/`, not a unit test
  over library code.
- Ambiguity: if two child RFCs both have a defensible claim on the same helper,
  stop and present the options rather than picking one silently.
- Iterations: if `make markdownlint` or `make check-fmt` still fails after
  three focused attempts on the same file, stop and escalate with the log path.
- Volume: if any single child RFC exceeds 700 lines, stop and escalate. That
  is a signal the capability group needs splitting further, which is a decision
  for the reviewer.

## Risks

- Risk: prose lifted from RFC 0006 section 8 into a child RFC diverges from the
  original during editing, so the two documents disagree on a contract.
  Severity: high. Likelihood: medium. Mitigation: migrate rather than copy. Each
  `EP-M2` to `EP-M10` milestone moves the per-helper contract bodies out of
  RFC 0006 section 8 and leaves a one-line pointer, so there is never a second
  normative copy. The coverage test's ownership invariant makes a stranded
  duplicate detectable.

- Risk: the split loses a helper. Fifty-seven names spread over ten groups is
  exactly the volume at which manual checking fails. Severity: high.
  Likelihood: medium. Mitigation: the machine-checked coverage invariant
  `COV-1`, established red in `EP-M1` before any child RFC is written.

- Risk: a rejected Ansible alias is reintroduced while writing a child RFC,
  because the alias reads naturally in prose (`is_dir`, `issubset`,
  `version_compare`). Severity: medium. Likelihood: medium. Mitigation:
  invariant `COV-2` asserts that no name RFC 0006 section 10.2 or section 6.10
  rejects appears as a specified helper in any child RFC.

- Risk: nine new RFC numbers are allocated, and a concurrent branch allocates
  one of them first. Severity: medium. Likelihood: low. Mitigation: `EP-M0`
  re-runs the remote-branch enumeration recorded below immediately before
  allocation, and RFC 0006's own `### Number allocation` table gains rows
  reserving 0013 to 0021. If a collision has appeared, shift the whole block
  upward rather than interleaving.

- Risk: the reviewer disagrees with aligning child RFCs to roadmap steps rather
  than to RFC 0006's section 14 slices. Severity: medium. Likelihood: medium.
  Mitigation: decision `D1` states the trade-off explicitly and `EP-M0` ends at
  a go/no-go point before any RFC file is created, so the alignment can be
  changed at the cost of one milestone.

- Risk: this ExecPlan's own scope reading (RFCs, not issues) is wrong.
  Severity: high. Likelihood: low. Mitigation: the `Scope divergence` section
  above makes it the first thing a reviewer reads, and `EP-M0` produces value
  under either reading.

## Progress

- [ ] `EP-M0` Audit the accepted set and settle the partition (no new files).
- [ ] `EP-M1` Land the coverage contract test red, the ADR, the umbrella
  scaffolding in RFC 0006, and the roadmap 6.1.1 rewrite.
- [ ] `EP-M2` RFC 0013, the shared contract and inventory foundation.
- [ ] `EP-M3` RFC 0014, structured data interchange.
- [ ] `EP-M4` RFC 0015, mapping and sequence transforms.
- [ ] `EP-M5` RFC 0016, ordered collection algebra and truth predicates.
- [ ] `EP-M6` RFC 0017, pattern and version predicates.
- [ ] `EP-M7` RFC 0018, lexical path composition.
- [ ] `EP-M8` RFC 0019, host-state predicates and environment expansion.
- [ ] `EP-M9` RFC 0020, encoding, identity, and formatting.
- [ ] `EP-M10` RFC 0021, date and time conversion; tighten the coverage
  invariant to forbid any remaining RFC 0006 section 8 ownership.
- [ ] `EP-M11` Reconcile, cross-reference every roadmap step, run all gates,
  mark roadmap 6.1.1 done.

## Surprises & discoveries

- Observation: RFC 0006 section 8.1 opens "All six helpers in this group are
  pure" but specifies five: `from_json`, `from_yaml`, `from_yaml_all`,
  `to_yaml`, and `to_nice_json`. Evidence:
  `docs/rfcs/0006-ansible-inspired-template-standard-library.md:650` against
  the five `####` headings at lines 654, 671, 694, 707, and 728. Impact: the
  sentence must be corrected to "five" when the group migrates in `EP-M3`.
  Independently, the group totals do reconcile: counting the `####` headings
  across sections 8.1 to 8.10 yields 41 filters and 16 tests, matching RFC 0006
  table 11. So the defect is the word "six", not the accepted set. This is
  exactly the class of error the coverage test exists to catch, and it was
  found by hand before the test existed; expect more.

- Observation: the roadmap's phase-6 success criterion paragraph sits mid-phase,
  at `docs/roadmap.md:1178-1183`, between step 6.10 and step 6.11, because 6.11
  was appended later. Evidence: reported by reconnaissance over
  `docs/roadmap.md:715-1267`. Impact: none for this task, which adds no phase-6
  step. Do not "fix" it here; it is out of scope and would enlarge the diff.

- Observation: no tooling parses `docs/roadmap.md`. There is no roadmap linter,
  no `mapsplice` in the repository, and no Makefile target referencing it.
  Evidence: reconnaissance grep over `Makefile`, `scripts/`, `tools/`,
  `tests/`. Impact: roadmap grammar is enforced by review alone, so the plan
  must state the grammar precisely rather than relying on a gate to catch a
  deviation.

## Decision log

- Decision `D1`: align child RFCs one-to-one with roadmap phase-6 steps 6.1 to
  6.9, giving nine child RFCs, rather than with RFC 0006 section 14's ten
  delivery slices. Rationale: the roadmap has already re-partitioned the
  slices, and its partition is a complete, disjoint cover of RFC 0006 sections
  8.1 to 8.10. Step 6.4 merges slices 3 and 8; step 6.7 merges the filesystem
  half of slice 5 with slice 6. Aligning to the slices instead would leave RFC
  boundaries cutting across roadmap steps, so a single roadmap task would draw
  its contract from two RFCs, and "exactly one RFC and accompanying roadmap
  task" would become untrue by construction. Section 14's slice graph is
  retained as the delivery-ordering view and reproduced as an inter-RFC
  dependency graph in the umbrella. Date/Author: 2026-09-08, planning agent.

- Decision `D2`: allocate RFC numbers 0013 to 0021 in roadmap-step order.
  Rationale: 0001 to 0012 are merged to `origin/main`; enumeration of every
  remote branch on 2026-09-08 found no branch allocating 0013 or above. The
  style guide numbers RFCs in allocation order, not by date. Date/Author:
  2026-09-08, planning agent.

- Decision `D3`: RFC 0006 becomes an umbrella of record. It retains sections 1
  to 7 and 9 to 17, including the normative cross-cutting contract in section
  1. Its section 8 subsections are reduced to a one-line purpose plus a pointer
  to the owning child RFC; the per-helper contract bodies migrate into the
  children. Rationale: leaving both a full section 8 and a full child
  specification would create two normative sources for one contract, which is
  the drift failure the success criterion is written to prevent. Deleting
  section 8 outright would destroy the document's value as a single readable
  survey. Reducing it to an index preserves navigation, keeps the diff bounded,
  and makes ownership unambiguous. This matches the umbrella-plus-topic-RFC
  pattern used by Adobe's Spectrum design-data specification, where the
  umbrella keeps the narrative and a coordination index while each child owns
  its topic normatively. Date/Author: 2026-09-08, planning agent.

- Decision `D4`: the cross-cutting contract stays single-sourced in RFC 0006
  section 6; each child RFC carries a mandatory
  `## Cross-cutting contract conformance` section that discharges all eleven
  clauses for its own helpers. Rationale: the roadmap's instruction is "give
  each the full cross-cutting contract from RFC 0006 §6 **rather than a
  reference to Ansible**", and RFC 0006 section 14 phrases the same requirement
  as "carries the full cross-cutting contract from section 6 rather than saying
  only 'match Ansible'". The contrast drawn is with an Ansible reference, not
  with a reference to section 6. Copying section 6's 220 lines into nine
  documents would produce nine copies that drift; discharging its eleven
  clauses concretely, per helper, is both what the clause asks for and what a
  reviewer can actually check. A child RFC that merely cites section 6 without
  the discharge section fails invariant `CONF-1`. Date/Author: 2026-09-08,
  planning agent.

- Decision `D5`: make the success criterion executable as
  `tests/rfc_stdlib_coverage_tests.rs`. Rationale: "every accepted capability
  is covered by exactly one RFC" is a bijection over 57 names. Bijections over
  57 names are not reliably checked by review. RFC 0006 section 14.1 already
  mandates a drift test over the registered-helper inventory for the same
  reason, and the repository has precedent for contract tests that parse
  tracked files (`tests/dependabot_config_tests.rs`,
  `tests/documentation_installation_tests.rs`). Date/Author: 2026-09-08,
  planning agent.

- Decision `D6`: record each child RFC's release target in its `## Preamble`
  as a `**Release target:**` bullet rather than inventing a roadmap field.
  Rationale: RFC 0006 already uses exactly this preamble bullet
  (`docs/rfcs/0006-...md:10-12`), and reconnaissance confirmed the roadmap has
  no per-item release-target convention anywhere in its 2306 lines.
  Date/Author: 2026-09-08, planning agent.

- Decision `D7`: roadmap steps 6.10 and 6.11 get no child RFC.
  Rationale: 6.10 covers RFC 0006 section 9's deferred candidates, which the
  success criterion requires be covered by no RFC. 6.11's Git change-detection
  capability is explicitly outside RFC 0006's Ansible-derived candidate set and
  already has `docs/git-change-detection-helpers-design.md` and
  [ADR-015](../adr-015-use-bounded-git-cli-for-change-detection.md).
  Date/Author: 2026-09-08, planning agent.

- Decision `D8`: rewrite roadmap task 6.1.1's own text from "child issues" to
  "child RFCs and accompanying roadmap tasks", and update its success bullet to
  match. Rationale: the commissioned task and the branch name both specify
  child RFCs. Leaving the roadmap saying "issues" while the tree contains nine
  child RFCs would leave the roadmap describing work nobody did. Recorded here
  as a deliberate divergence from the tracked roadmap text rather than a silent
  edit. See `Scope divergence from the roadmap's literal wording`. Date/Author:
  2026-09-08, planning agent.

- Decision `D9`: record the split convention in a new ADR,
  `docs/adr-021-split-umbrella-rfcs-into-focused-child-rfcs.md`. Rationale: RFC
  numbers cannot be renumbered after publication, so allocating nine of them
  under a particular partition is hard to reverse — the style guide's own test
  for when an ADR is warranted. The convention will also apply to RFC 0001,
  which already has three amendment RFCs and may face the same pressure. The
  highest existing ADR is 020, so 021 is free. Date/Author: 2026-09-08,
  planning agent.

## Outcomes & retrospective

To be completed at `EP-M11`. Before marking this plan `COMPLETE`, reconcile
every discovery against RFC 0006, `docs/roadmap.md`, and
`docs/documentation-style-guide.md`, and confirm that no disposition changed.

## Context and orientation

You are working in the `netsuke` repository. Netsuke is a build tool that reads
a YAML manifest called a `Netsukefile`, expands Jinja templates in it using the
MiniJinja crate, and generates a Ninja build file. "Template standard library"
means the set of filters, tests, and functions Netsuke registers with MiniJinja
so manifest authors can transform values without shelling out.

Some terms used throughout, defined once:

- **Filter**: a helper invoked as `value | name(args)`.
- **Test**: a helper invoked as `value is name(args)`. Jinja keeps filters,
  tests, and functions in separate namespaces, so one word can name two
  different helpers.
- **Manifest query**: the read-only evaluation environment that serves
  `netsuke help targets`. A helper that reads the clock, the environment, the
  filesystem, the network, or a subprocess is not admitted to it.
- **Purity class**: RFC 0006 section 6.1's label for what a helper observes:
  pure, clock-observing, environment-observing, filesystem-observing,
  network-observing, or subprocess-observing.
- **Canonical key**: the RFC 8785 canonical JSON form of a value, used as a
  deterministic equality relation so no helper's output order comes from a hash
  table. Specified in RFC 0006 section 6.7.
- **Umbrella RFC**: an RFC that retains a survey and a coverage map while
  delegating each capability's normative specification to a child RFC.

The files that matter, by full repository-relative path:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` — the RFC
  being split. 2132 lines, status `Proposed`, tracking issue
  [#596](https://github.com/leynos/netsuke/issues/596).
- `docs/roadmap.md` — phase 6 spans lines 715 to 1267. Step 6.1 is the shared
  contract; steps 6.2 to 6.9 are the capability groups; 6.10 is the deferred
  set; 6.11 is Git change detection.
- `docs/contents.md` — the documentation index. Its
  `## Requests for comments` section, lines 35 to 82, is the only RFC index in
  the repository; a new RFC that is not listed there is undiscoverable.
- `docs/documentation-style-guide.md` — lines 222 to 360 define the RFC naming
  convention, the required and conditional sections, the formatting guidance,
  and a literal RFC template. Follow the template.
- `docs/stdlib-yaml-and-jinja-guide.md` — the standard-library guide the
  eventual implementations must extend. This task does not edit it.
- `docs/developers-guide.md` — records the `ResolveError` boundary pattern that
  RFC 0006 section 6.9 makes policy for new helpers.

For orientation on the wider architecture see `docs/netsuke-design.md`. For the
testing idioms the child RFCs will require of their implementers, see
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/rust-doctest-dry-guide.md`, and
`docs/snapshot-testing-in-netsuke-using-insta.md`. When drafting the
capability-boundary prose in RFC 0019, load the `hexagonal-architecture` skill:
`expandvars` and the filesystem predicates are the plan's only ports, and the
distinction to preserve is between pure domain policy (path lexing, purity
classification) and the injected adapters (`cap_std` workspace handle,
environment reader) that RFC 0006 section 6.4 mandates. For Rust idiom
questions while writing the coverage test, route through the `rust-router`
skill, which points at `rust-unit-testing` for fixture and table-test shape.

### The partition

This is the heart of the plan. Every later milestone is bookkeeping against
this table. "Group" is the RFC 0006 section 8 subsection; "Step" is the
`docs/roadmap.md` phase-6 step; "Slice" is the RFC 0006 section 14 delivery
slice, retained for sequencing only.

| Child RFC | Title                                           | Group        | Step | Slice   |
| --------- | ----------------------------------------------- | ------------ | ---- | ------- |
| 0013      | Shared standard-library contract and inventory  | §6, §14.1    | 6.1  | 0       |
| 0014      | Structured data interchange helpers             | §8.1         | 6.2  | 1       |
| 0015      | Mapping and sequence transform helpers          | §8.2         | 6.3  | 2       |
| 0016      | Ordered collection algebra and truth predicates | §8.3, §8.8   | 6.4  | 3, 8    |
| 0017      | Pattern and version predicates                  | §8.4, §8.5   | 6.5  | 4       |
| 0018      | Lexical path composition                        | §8.6 lexical | 6.6  | 5 lex   |
| 0019      | Host-state predicates and environment expansion | §8.7, §8.6   | 6.7  | 5 fs, 6 |
| 0020      | Encoding, identity, and formatting helpers      | §8.9         | 6.8  | 7       |
| 0021      | Date and time conversion helpers                | §8.10        | 6.9  | 9       |

*Table 1: Child RFC allocation against RFC 0006 groups and roadmap steps.*

Note the two places where a group is divided. RFC 0006 section 8.6 specifies
seven filters, six of which are pure lexical path operations belonging to RFC
0018, while `expandvars` is the RFC's only environment-observing helper and
belongs to RFC 0019 with the other injected-seam helpers; this follows the
section 14.7 rationale for separating slice 6 from slice 5. Section 8.7
specifies five tests plus an option on the existing `glob` function, of which
the `abs` test is purely lexical and belongs to RFC 0018 — roadmap task 6.6.4
already places it in step 6.6 for exactly that reason, and RFC 0006 section
8.7's own text concedes that `abs` performs no filesystem access.

The helper inventory, which is the coverage test's input, is:

- RFC 0014, five filters: `from_json`, `from_yaml`, `from_yaml_all`, `to_yaml`,
  `to_nice_json`.
- RFC 0015, six filters: `combine`, `dict2items`, `items2dict`, `extract`,
  `subelements`, `rekey_on_member`.
- RFC 0016, eight filters — `union`, `intersect`, `difference`,
  `symmetric_difference`, `product`, `combinations`, `permutations`,
  `zip_longest` — and seven tests: `any`, `all`, `subset`, `superset`,
  `contains`, `truthy`, `falsy`.
- RFC 0017, four filters — `regex_replace`, `regex_search`, `regex_findall`,
  `regex_escape` — and four tests: `match`, `search`, `regex`, `version`.
- RFC 0018, six filters — `path_join`, `normpath`, `splitext`, `commonpath`,
  `relpath`, `splitdrive` — one test, `abs`, and the `dialect` option on the
  existing `basename` and `dirname` filters.
- RFC 0019, one filter, `expandvars`; four tests — `exists`, `link_exists`,
  `same_file`, `mount` — and the `files_only` option on the existing `glob`
  function.
- RFC 0020, nine filters: `b64encode`, `b64decode`, `urldecode`, `to_uuid`,
  `shell_quote`, `comment`, `human_readable`, `human_to_bytes`, `text_hash`.
- RFC 0021, two filters: `to_datetime`, `strftime`.

That totals 41 filters and 16 tests, matching RFC 0006 table 11, plus the three
existing helpers gaining a behaviour-preserving option (`basename`, `dirname`,
`glob`), which table 11 also records as three. RFC 0013 specifies no helper; it
specifies the shared machinery every other child depends on.

### The child RFC skeleton

Every child RFC uses the style guide's template with two additions. The section
order is:

1. `# RFC 00NN: <title>` — the title line form the style guide requires.
2. `## Preamble` — bullets in this order: `**RFC number:**`, `**Parent RFC:**`
   naming RFC 0006, `**Status:** Proposed`, `**Created:**` the ISO date,
   `**Roadmap step:**` naming the phase-6 step, `**Tracking issue:**`
   [#596](https://github.com/leynos/netsuke/issues/596), and
   `**Release target:**` per decision `D6`.
3. `## 1. Summary` — what the group buys a manifest author.
4. `## 2. Problem` — the `shell()` contortion this group removes, lifted from
   RFC 0006 section 2's corresponding bullet.
5. `## 3. Goals and non-goals` — with the deferred and rejected candidates
   adjacent to this group named explicitly as non-goals. This is the prose that
   protects invariant `COV-2`.
6. `## 4. Proposed design` — the migrated per-helper contracts from RFC 0006
   section 8, one `### 4.N. <signature>` per helper, unchanged in substance.
7. `## 5. Cross-cutting contract conformance` — the mandatory section from
   decision `D4`, with one `### 5.N` subsection per RFC 0006 section 6 clause,
   numbered 5.1 to 5.11 to mirror 6.1 to 6.11, each discharging that clause for
   this group's helpers. Section 5.1 carries the per-helper purity and
   manifest-query disposition table.
8. `## 6. Dependencies` — the RFC 0006 section 13.4 crates this group needs and
   the child RFCs it requires, drawn from the section 14 slice graph.
9. `## 7. Delivery` — the roadmap tasks that implement this RFC, by number.
10. `## 8. Open questions` — the RFC 0006 section 16 questions assigned to this
    group, carried over verbatim, unresolved.
11. `## 9. Recommendation`.

Sections 4 and 5 are the substance; the rest is scaffolding. RFC 0013 replaces
section 4 with the shared-machinery specification migrated from RFC 0006
section 14.1 and omits section 5, since it is the thing conformance is measured
against.

RFC 0006's open questions distribute as follows: question 1 (`to_nice_yaml`) to
RFC 0014; question 2 (the `abs` test name) to RFC 0018; question 3 (a `v`
prefix on `version`) to RFC 0017; question 4 (`serde-saphyr` alias bounding) to
RFC 0014; questions 5 (configurable bounds) and 7 (an injected clock) to RFC
0013; question 6 (`text_digest`) to RFC 0020. All seven stay open.

## Conformance basis

There is no Terms of Reference document for this work. The upstream artefacts
are:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` at
  `origin/main` commit `924cb215`, status `Proposed`. Sections 6, 7, 8, 9, 10,
  13.4, 14, and 16 are load-bearing here.
- `docs/roadmap.md` at the same commit, phase 6, task 6.1.1 and steps 6.1 to
  6.9.
- `docs/documentation-style-guide.md`, the RFC section at lines 222 to 360.
- [ADR-008](../adr-008-environment-seam-taxonomy.md) governs the `expandvars`
  environment seam that RFC 0019 will specify.
- [ADR-010](../adr-010-scope-glob-capability-to-literal-prefix.md) governs the
  `glob(files_only=...)` option that RFC 0019 will specify.
- [ADR-001](../adr-001-replace-serde-yml-with-serde-saphyr.md) governs the YAML
  stack that RFC 0014 will specify.
- `docs/adr-021-split-umbrella-rfcs-into-focused-child-rfcs.md` is created by
  this plan at `EP-M1` and becomes upstream of every later milestone.

Trace links:

```plaintext
RFC0006-S14 -> ROADMAP-6.1.1 -> EP-M0 -> Table 1 partition
RFC0006-S7  -> ROADMAP-6.1.1 -> EP-M1 -> COV-1 -> tests::rfc_stdlib_coverage::every_accepted_helper_has_exactly_one_owner
RFC0006-S9  -> ROADMAP-6.1.1 -> EP-M1 -> COV-2 -> tests::rfc_stdlib_coverage::no_deferred_or_rejected_helper_is_specified
RFC0006-S7.8 -> ROADMAP-6.1.1 -> EP-M1 -> COV-3 -> tests::rfc_stdlib_coverage::helper_counts_match_table_11
RFC0006-S6  -> ROADMAP-6.1.1 -> EP-M2..EP-M10 -> CONF-1 -> tests::rfc_stdlib_coverage::every_child_rfc_discharges_all_contract_clauses
RFC0006-S14 -> ROADMAP-6.2..6.9 -> EP-M11 -> roadmap step cross-references
ADR-021     -> EP-M1 -> docs/adr-021-split-umbrella-rfcs-into-focused-child-rfcs.md
```

## Verification plan

This change adds no runtime behaviour, so there is no invariant over program
state. It does introduce a non-trivial combinatorial invariant over documents —
a bijection between 57 helper names and their owning RFCs — and that invariant
is both statable and mechanically checkable. Verifying it by review is not
adequate: fifty-seven names across nine documents is precisely the scale at
which human checking fails silently, and the whole point of the roadmap's
success criterion is that the coverage be exact in both directions.

Method selection: parameterized and table-driven Rust tests, not property tests
and not model checking. The domain is a fixed finite set of 57 names and 9
documents, fully enumerable, so exhaustive enumeration *is* the strongest
available evidence; generating random helper names would test nothing real.
This is recorded explicitly because the repository's testing guidance otherwise
prefers property tests for invariants over ranges.

### Obligation `COV-1`: exactly-one ownership

- Obligation: every helper name in the RFC 0006 accepted inventory is specified
  by exactly one RFC — either its designated child RFC, or RFC 0006 section 8
  if that group has not yet migrated.
- Method: table-driven test enumerating the inventory from `Table 1` and the
  helper lists above, parsing each `docs/rfcs/*.md` for its specified-helper
  headings, and asserting the count of owners is exactly one per name.
- Rationale: finite, fully enumerable domain; exhaustive is achievable.
- Domain: all 57 accepted names plus the 3 optioned existing helpers, against
  RFC 0006 and RFCs 0013 to 0021.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs`.
- Evidence: `cargo nextest run --test rfc_stdlib_coverage_tests`. Red at
  `EP-M1` because the inventory names nine child RFCs that do not yet exist and
  the disjunct "or RFC 0006 section 8" is not yet implemented; green from the
  moment that disjunct lands, and green at every milestone thereafter.
- Non-vacuity: the test must fail for the intended reason under three seeded
  faults, each applied to a scratch copy and reverted: delete `combine` from
  RFC 0015 and expect a zero-owner failure naming `combine`; add `combine` to
  RFC 0016 as well and expect a two-owner failure naming both files; leave a
  full section 8.2 body in RFC 0006 after migrating and expect a two-owner
  failure. Record all three transcripts. A test that passes under any of these
  is not checking ownership and must be rewritten. Additionally the test must
  assert the inventory is non-empty, so a parsing change that yields zero names
  cannot pass vacuously.

### Obligation `COV-2`: no deferred or rejected candidate is specified

- Obligation: no name that RFC 0006 section 9 defers or section 10 rejects is
  specified as a helper by any RFC in `docs/rfcs/`.
- Method: table-driven test over the explicit deny list.
- Rationale: the failure mode is a single rejected alias slipping in while
  prose is edited; a deny list catches exactly that and nothing else.
- Domain: the deferred names `random`, `shuffle`, `log`, `pow`, `root`,
  `type_debug`; the rejected alias names enumerated in RFC 0006 section 6.10
  and section 10.2, including `directory`, `is_dir`, `link`, `is_link`,
  `is_abs`, `is_same_file`, `is_mount`, `issubset`, `issuperset`, `failure`,
  `success`, `successful`, `change`, `skip`, `version_compare`, `to_nice_yaml`,
  `hash_text`, `checksum`, and the `win_*` family; and the section 10.1
  orchestration tests `failed`, `succeeded`, `changed`, `skipped`, `reachable`,
  `unreachable`, `timedout`, `started`, `finished`.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs`.
- Evidence: same command. Green from `EP-M1`, because no child RFC exists yet
  to violate it — which is why the non-vacuity control below is mandatory
  rather than optional.
- Non-vacuity: this obligation is green from the start, so a passing result
  proves nothing on its own. The control is compulsory. Add a section 4 heading
  specifying a `shuffle` filter to a scratch copy of RFC 0016 and confirm the
  test fails naming `shuffle` and the file; then do the same for the rejected
  alias `is_dir` and confirm it fails naming that alias. Record both
  transcripts at `EP-M1`. The deny list must also be asserted non-empty, and
  asserted to intersect the parsed heading vocabulary in at least the seeded
  case, so a parser that never matches any heading cannot pass.

### Obligation `COV-3`: totals agree with RFC 0006 table 11

- Obligation: the inventory contains exactly 41 filters, 16 tests, and 3
  existing helpers gaining an option.
- Method: parameterized assertion over the parsed totals.
- Rationale: an independent arithmetic check on `COV-1`'s inventory. `COV-1`
  proves each listed name has one owner; `COV-3` proves the list itself is the
  right size, so a name dropped from both the inventory and every RFC cannot
  pass unnoticed.
- Domain: the three totals.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs`.
- Evidence: same command.
- Non-vacuity: remove one filter from the inventory constant and confirm the
  test fails reporting 40 against 41. This control is what makes `COV-1` and
  `COV-3` jointly non-circular: neither alone would catch a name deleted
  everywhere.

### Obligation `CONF-1`: every child RFC discharges every contract clause

- Obligation: each of RFCs 0014 to 0021 contains a
  `## 5. Cross-cutting contract conformance` section with subsections 5.1 to
  5.11, and its 5.1 table names every helper that RFC owns with a purity class
  drawn from RFC 0006 table 2 and a manifest-query disposition.
- Method: structural contract test over the headings, plus review of the prose.
- Rationale: the structural half is mechanical and worth automating; whether
  the prose in 5.7 genuinely discharges canonical equality for `subset` is a
  judgement a test cannot make, and the plan says so rather than pretending
  otherwise. This is the plan's declared residual gap.
- Domain: RFCs 0014 to 0021; RFC 0013 is exempt by construction.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs`.
- Evidence: same command. Red for each child RFC until that child's milestone
  lands, so it must be scoped to child RFCs that exist — see the plateau
  discussion in `Milestones and plateaus`.
- Non-vacuity: delete subsection 5.8 from a scratch copy of RFC 0016 and
  confirm the test fails naming the missing clause and the file; give a helper
  a purity class not in table 2 and confirm that fails too.
- Residual gap: semantic adequacy of each discharge is reviewer-checked, not
  test-checked. Do not claim otherwise in the pull request.

### Axioms

These are assumed, not verified:

- RFC 0006's accept, defer, and reject dispositions are correct. This task
  partitions them; it does not audit them.
- `markdownlint-cli2`, `typos`, and `mdtablefix` behave as their configuration
  states. The repository owns the configuration, not the tools.
- RFC 0006 table 11's totals of 41, 16, and 3 are correct. `EP-M0` re-derives
  them by counting section 8's `####` headings and must agree before the plan
  proceeds; the count already performed during planning does agree.
- Jinja keeps filters, tests, and functions in separate namespaces, so `abs`
  as a test and `abs` as a MiniJinja filter do not collide. RFC 0006 section
  11.4 records this; the coverage test keys on the name-plus-namespace pair,
  not the bare name, so that this assumption cannot silently produce a false
  duplicate-ownership failure.

## Milestones and plateaus

Each milestone ends in a repository state where every gate passes and the
coverage test is green. That is achievable at every intermediate step because
of how `COV-1` is stated: a helper is owned by its child RFC **or** by RFC 0006
section 8. This is a real invariant, not a compatibility shim — at every
plateau exactly one document normatively specifies each helper, which is
precisely the property the success criterion demands. `EP-M10` tightens the
disjunct away once no group remains in section 8.

### `EP-M0` — audit and partition (no files created)

- Outcome: the partition in `Table 1` is confirmed or revised against the
  actual document, and the helper inventory is written down.
- Requirements: `RFC0006-S7`, `RFC0006-S14`.
- Acceptance evidence: a comment on the pull request, or an update to this
  plan's `Surprises & discoveries`, recording the recount of section 8's `####`
  headings by group, the confirmation that no remote branch allocates RFC 0013
  or above, and any further discrepancy of the section 8.1 "six helpers" kind.
- Conformance check: dispositions unchanged; no file modified.
- Recovery: nothing to revert.
- Remaining gaps: everything.
- Compatibility decision: none required.
- **Go/no-go.** If the recount disagrees with table 11, or if the reviewer
  prefers slice alignment over step alignment, stop here.

### `EP-M1` — contract test, ADR, umbrella scaffolding, roadmap rewrite

- Outcome: `tests/rfc_stdlib_coverage_tests.rs` exists and is green with all
  eight capability groups still owned by RFC 0006 section 8; ADR-021 records
  the convention; RFC 0006 gains its `### Number allocation` reservations for
  0013 to 0021, a `## 14. Coverage map` restructure, and a `**Parent of:**`
  preamble note; roadmap 6.1.1 says "child RFCs".
- Requirements: `RFC0006-S7`, `RFC0006-S9`, `RFC0006-S7.8`, `ADR-021`.
- Acceptance evidence: `COV-1`, `COV-2`, and `COV-3` green; all six seeded-fault
  transcripts recorded; `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` green.
- Conformance check: no disposition changed; no `src/` file touched; no new
  dependency; `docs/contents.md` still lists every RFC that exists.
- Recovery: revert the commit; nothing downstream depends on it yet.
- Remaining gaps: no child RFC exists.
- Compatibility decision: none. RFC 0006 is a pre-1.0 internal document with no
  external consumer; its section 8 is restructured in place across `EP-M2` to
  `EP-M10` with no alias, pointer stub, or dual specification retained beyond
  the one-line index entry that decision `D3` deliberately keeps for navigation.

### `EP-M2` — RFC 0013, shared contract and inventory foundation

- Outcome: `docs/rfcs/0013-shared-standard-library-contract.md` exists,
  specifying the canonical value key, the bounded-materialization helper, the
  domain-error and diagnostic-code scaffolding, the manifest-query disposition
  test, the closure of the sixteen disclosure gaps, and the maintained
  inventory — all migrated from RFC 0006 section 14.1. Listed in
  `docs/contents.md`. Roadmap step 6.1 references it.
- Requirements: `RFC0006-S6`, `RFC0006-S14.1`.
- Acceptance evidence: all gates green; the coverage test still green (RFC 0013
  owns no helper, so `COV-1` is unaffected); RFC 0013 carries open questions 5
  and 7 unresolved.
- Conformance check: RFC 0006 section 6 remains in RFC 0006 per decision `D4`;
  RFC 0013 references it and does not copy it.
- Recovery: revert the commit; `COV-1` returns to its `EP-M1` state.
- Remaining gaps: eight capability RFCs.
- Compatibility decision: none.

### `EP-M3` to `EP-M10` — one capability RFC each

Each milestone follows the same shape, so it is stated once. For child RFC
`00NN` owning group `§8.G` and roadmap step `6.S`:

- Outcome: `docs/rfcs/00NN-<slug>.md` exists with the skeleton from
  `The child RFC skeleton`; the per-helper contract bodies have **moved** out
  of RFC 0006 section `8.G`, which now holds a one-line purpose plus a pointer;
  RFC 0006's coverage map names 00NN as the owner; `docs/contents.md` lists it;
  roadmap step `6.S` references it.
- Requirements: the section 8 subsections in `Table 1` for that row, plus
  `RFC0006-S6` via `CONF-1`.
- Acceptance evidence: `COV-1` green with that group's helpers now owned by the
  child rather than by RFC 0006; `CONF-1` green for that child; all gates
  green; the migrated contracts are substantively unchanged, evidenced by a
  `git diff` in which section 8's removals and the child's additions correspond
  line for line except for heading renumbering.
- Conformance check: no disposition changed; no deferred or rejected name
  introduced (`COV-2` proves this mechanically); the child's open questions
  match the assignment in `The child RFC skeleton`.
- Recovery: revert the single commit. The plateau before it is coherent because
  the group returns to RFC 0006 ownership and `COV-1`'s disjunct still holds.
- Remaining gaps: the child RFCs not yet written.
- Compatibility decision: none.

The milestone order is `EP-M3` RFC 0014, `EP-M4` RFC 0015, `EP-M5` RFC 0016,
`EP-M6` RFC 0017, `EP-M7` RFC 0018, `EP-M8` RFC 0019, `EP-M9` RFC 0020,
`EP-M10` RFC 0021. Write RFC 0014 first because it is the smallest group at
five helpers and will expose skeleton problems cheaply; write RFC 0019 after
RFC 0018 because `expandvars` depends on the `dialect` mechanism RFC 0018
defines, mirroring the section 14 slice graph.

`EP-M10` additionally removes the "or RFC 0006 section 8" disjunct from
`COV-1`, since no group remains there. Removing it must make no test fail; if
it does, a group was missed.

### `EP-M11` — reconcile and close

- Outcome: every roadmap step 6.1 to 6.9 references its child RFC; RFC 0006's
  coverage map has no unowned row; task 6.1.1 is marked `- [x]`; this plan's
  living sections are current and its status is `COMPLETE`.
- Requirements: all of the above.
- Acceptance evidence: `make check-fmt`, `make lint`, `make doc-coverage`,
  `make test`, `make markdownlint`, and `make nixie` all green on the merge
  commit, each captured under `/tmp`.
- Conformance check: reconcile every entry in `Surprises & discoveries` against
  RFC 0006 and the style guide. The section 8.1 "six helpers" correction must
  be present. No disposition changed.
- Recovery: not applicable; this milestone only marks state.
- Remaining gaps: none. Implementation of the helpers themselves is roadmap
  steps 6.2 to 6.9, not this task.
- Compatibility decision: none.

## Plan of work

### Stage A — understand and propose (no file changes)

Read RFC 0006 sections 6, 7, 8, 9, 10, 13.4, 14, and 16 in full. Recount the
`####` headings under each section 8 subsection and check the group totals
against table 11. Re-enumerate remote branches to confirm RFC numbers 0013 to
0021 are free:

```bash
git ls-remote --heads origin | grep -oP 'refs/heads/\K.*' | sort
```

Record the recount and any discrepancy in `Surprises & discoveries`. This is
`EP-M0` and ends at a go/no-go point.

### Stage B — red

Write `tests/rfc_stdlib_coverage_tests.rs` before writing any RFC. Wire it into
the integration-test module contract the repository enforces; see
`tests/integration_test_wiring_tests.rs` for what that contract checks, and
follow the pattern in `tests/dependabot_config_tests.rs` for reading a tracked
file from a test.

The test's shape: a module-private constant table of `(name, namespace, owner)`
triples derived from `The partition`; a parser that walks `docs/rfcs/*.md` and
extracts, for each file, the helper names it specifies; and the four assertions
`COV-1`, `COV-2`, `COV-3`, and `CONF-1`. Keep the file within the repository's
400-line cap by putting the inventory table in a sibling module under
`tests/rfc_stdlib_coverage/` if it grows, following the split precedent
recorded at the top of `tests/documentation_installation_tests.rs`.

Run it and confirm it fails for the expected reason — the nine child RFCs do
not exist. Then implement the "or RFC 0006 section 8" disjunct and confirm it
goes green. Then apply each seeded fault from the `Verification plan` in turn,
confirm the expected failure, and revert. Capture every transcript.

### Stage C — implementation

`EP-M1` first: ADR-021, the RFC 0006 umbrella scaffolding, the roadmap 6.1.1
rewrite, `docs/contents.md` untouched at this point since no RFC exists yet.

Then `EP-M2` through `EP-M10`, one commit per child RFC. For each:

1. Create `docs/rfcs/00NN-<slug>.md` from the skeleton.
2. Move the per-helper bodies out of RFC 0006 section 8 into its section 4.
   Move, do not copy: the section 8 subsection is reduced to its heading, a
   one-sentence purpose, and `Specified by [RFC 00NN](00NN-<slug>.md).`
3. Write section 5, discharging RFC 0006 section 6 clauses 6.1 to 6.11 for
   this group's helpers. Start from section 5.1's purity table; every later
   subsection is easier once purity is fixed.
4. Add the row to RFC 0006's coverage map naming the new owner.
5. Add the entry to `docs/contents.md`'s `## Requests for comments` section,
   using the inline-link style the neighbouring RFC-0006 entry uses.
6. Add `- See [RFC 00NN](rfcs/00NN-<slug>.md).` to roadmap step `6.S` as a
   sub-bullet under the step's prose, at zero indentation, matching the grammar
   used elsewhere in phase 6.
7. `make fmt`, then the gates, then commit.

### Stage D — refactor and validate

`EP-M11`. Remove the `COV-1` disjunct. Sweep every cross-reference. Run the
full gate set sequentially. Update this plan's living sections.

## Concrete steps

Run everything from the worktree root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/5ffbb1df-4543-4fd2-8f84-30f81650519c`.

Gate commands, always sequential, always tee'd:

```bash
make fmt
make check-fmt   2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
make lint        2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
make test        2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$(git branch --show-current).out
make nixie       2>&1 | tee /tmp/nixie-netsuke-$(git branch --show-current).out
```

The focused test command during Stage B:

```bash
cargo nextest run --test rfc_stdlib_coverage_tests 2>&1 \
  | tee /tmp/covtest-netsuke-$(git branch --show-current).out
```

Expected red transcript at the start of Stage B, before the disjunct exists:

```plaintext
FAIL [   0.012s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  helper `from_json` (filter) has 0 owners; expected exactly 1
  helper `combine` (filter) has 0 owners; expected exactly 1
  ... 55 more
```

Expected green transcript once the disjunct lands:

```plaintext
    PASS [   0.014s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
    PASS [   0.009s] netsuke::rfc_stdlib_coverage_tests no_deferred_or_rejected_helper_is_specified
    PASS [   0.008s] netsuke::rfc_stdlib_coverage_tests helper_counts_match_table_11
    PASS [   0.011s] netsuke::rfc_stdlib_coverage_tests every_child_rfc_discharges_all_contract_clauses
```

Expected seeded-fault transcript after adding `combine` to RFC 0016 as well as
RFC 0015:

```plaintext
FAIL [   0.013s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  helper `combine` (filter) has 2 owners: docs/rfcs/0015-mapping-and-sequence-transforms.md,
    docs/rfcs/0016-ordered-collection-algebra-and-truth-predicates.md; expected exactly 1
```

Delegate each full gate run to the `scrutineer` subagent rather than running it
in the planning conversation, and delegate the mechanical prose migration in
Stage C step 2 to `scribe`, which is well suited to move-without-changing-
substance edits. Do not delegate section 5 of any child RFC to `scribe`; it is
new normative content, not an editorial move.

## Validation and acceptance

Acceptance is behavioural, phrased as things a reader can do.

1. Open `docs/rfcs/0006-ansible-inspired-template-standard-library.md`, go to
   section 14, and read a table that names, for each accepted capability group,
   the child RFC that specifies it. Follow any row's link and land on a
   document under 700 lines that specifies only that group.
2. Open any child RFC and find, in its section 5, a statement of what each of
   RFC 0006 section 6's eleven clauses requires *of that group's helpers* —
   including a table giving every helper's purity class and manifest-query
   disposition. Nowhere in that section does the justification reduce to "as
   Ansible does".
3. Run `make test` and observe `rfc_stdlib_coverage_tests` pass with four
   tests. Delete the `### 4.1. \`text | from_json\`` heading from `docs
   /rfcs/0014-structured-data-interchange-helpers.md
   `, re-run, and observe a failure naming`from_json
   ` and reporting zero owners. Restore the heading.
4. Grep the `docs/rfcs/` tree for `shuffle`, `is_dir`, and `version_compare`
   and find them only in RFC 0006's deferred and rejected sections, never as a
   specified helper.
5. Open `docs/roadmap.md` at step 6.5 and find a `See [RFC 0017](...)` bullet;
   follow it and land on the pattern-and-version RFC.
6. Confirm `docs/contents.md` lists all nine new RFCs.

Red-Green-Refactor evidence to record:

- Red: `cargo nextest run --test rfc_stdlib_coverage_tests` fails with 57
  zero-owner errors before the RFC 0006 section 8 disjunct is implemented.
- Green: the same command passes after the disjunct lands, with RFC 0006 still
  the sole owner of every group.
- Refactor: the same command passes after `EP-M10` removes the disjunct, with
  every group owned by a child RFC.

No behaviour-driven development scenarios apply: this task changes no
externally observable tool behaviour, adds no command-line surface, and touches
no persistence or network boundary. `docs/users-guide.md` is therefore not
updated — RFC status is not user-facing behaviour. Recorded here so the
omission is visible as a decision rather than an oversight.

Quality criteria:

- Tests: `make test` green, including the four new coverage tests.
- Verification: `COV-1`, `COV-2`, `COV-3`, and `CONF-1` discharged, with all
  seeded-fault transcripts recorded in `Artefacts and notes`. `CONF-1`'s
  semantic residual gap stated, not papered over.
- Lint: `make lint`, `make markdownlint`, `make check-fmt`, `make nixie` green.
- Doc coverage: `make doc-coverage` green. The new test file needs module and
  item documentation comments to satisfy it.
- Performance: not applicable.
- Security: not applicable; no capability, dependency, or trust boundary
  changes.

## Idempotence and recovery

Every step is re-runnable. The gate commands are read-only apart from
`make fmt`, which is idempotent. Each milestone is one commit, so recovery is
`git revert` of that commit; because `COV-1` holds under both the child-owned
and section-8-owned dispositions, reverting any single child-RFC commit leaves
a green tree.

The one irreversible act is allocating RFC numbers, and it is only irreversible
once merged to `main`. Before merge, renumbering is a rename plus a
cross-reference sweep; `make markdownlint` will not catch a stale link, so grep
for the old filename explicitly.

Do not use bare `git stash`; the stash stack is shared across worktrees. Use a
temporary work-in-progress commit instead.

## Artefacts and notes

To be filled during implementation. At minimum, record:

- the `EP-M0` recount of section 8 `####` headings per group;
- the six seeded-fault transcripts from the `Verification plan`;
- the `git diff --stat` for each child-RFC commit, which should show
  approximately balanced insertions in the child and deletions in RFC 0006 for
  the migrated section 4;
- the final gate log paths under `/tmp`.

## Interfaces and dependencies

Files created:

- `docs/rfcs/0013-shared-standard-library-contract.md`
- `docs/rfcs/0014-structured-data-interchange-helpers.md`
- `docs/rfcs/0015-mapping-and-sequence-transforms.md`
- `docs/rfcs/0016-ordered-collection-algebra-and-truth-predicates.md`
- `docs/rfcs/0017-pattern-and-version-predicates.md`
- `docs/rfcs/0018-lexical-path-composition.md`
- `docs/rfcs/0019-host-state-predicates-and-environment-expansion.md`
- `docs/rfcs/0020-encoding-identity-and-formatting-helpers.md`
- `docs/rfcs/0021-date-and-time-conversion-helpers.md`
- `docs/adr-021-split-umbrella-rfcs-into-focused-child-rfcs.md`
- `tests/rfc_stdlib_coverage_tests.rs`

Files modified:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md`
- `docs/roadmap.md`
- `docs/contents.md`
- `docs/execplans/6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task.md`

No other file may change. No `src/` file changes. No `Cargo.toml` change: the
coverage test uses `std`, plus `anyhow` and the `googletest` and
`pretty_assertions` assertion crates already present as dev-dependencies.

The test's public shape, in `tests/rfc_stdlib_coverage_tests.rs`:

```rust
/// Namespace a helper occupies. Jinja keeps these separate, so `abs` as a
/// test and `abs` as a filter are distinct entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Namespace {
    Filter,
    Test,
    Function,
}

/// One accepted helper and the RFC expected to specify it.
#[derive(Debug, Clone, Copy)]
struct Helper {
    name: &'static str,
    namespace: Namespace,
    owner: &'static str,
}
```

`ortho_config` is not used by this task. It governs layered command-line and
configuration surfaces; this change adds none. It is named here only to record
that the question was asked and answered, since the commissioning brief raises
it: if a later milestone unexpectedly introduces a configuration surface — for
instance if RFC 0006 open question 5 were resolved by exposing the section 6.8
bounds through `StdlibConfig` — that would be a tolerance breach requiring
escalation, not a quiet addition. See `docs/ortho-config-users-guide.md`.

## Revision note

Initial draft, 2026-09-08. Establishes the nine-child partition aligned to
roadmap phase-6 steps, the umbrella treatment of RFC 0006, the executable
coverage invariant, and the divergence from the roadmap's literal "child
issues" wording. Nothing implemented; awaiting approval.
