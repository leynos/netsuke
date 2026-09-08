# 6.1.1. Split the RFC 0006 accepted set into focused child RFCs

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

[RFC 0006](../rfcs/0006-ansible-inspired-template-standard-library.md) surveys
every filter, test, and function exposed by `ansible-core` 2.21.3, records an
accept, defer, or reject disposition for each, and specifies fifty-seven new
Netsuke helpers across ten capability groups in 2132 lines. Its section 14 says
it is deliberately not one implementation change, and its section 17 recommends
scheduling the capability groups as focused children after v0.1.0 final.

Two things are missing, and only two.

First, there is no per-helper record of the obligations that RFC 0006 section 6
imposes. Section 6.1 requires every helper to carry a purity label "recorded in
its documentation entry and asserted by a test", and gives an aggregate —
fifty-two pure, four filesystem-observing, one environment-observing — but **no
table anywhere assigns a class to a named helper**. The same is true of the
resource bounds in table 3, the diagnostic codes in section 6.9, and the
manifest-query disposition in section 6.2. An implementer of a capability group
must currently re-derive all four from prose, for every helper, and no reviewer
can check the derivation.

Second, there is no mechanical way to answer the question the roadmap asks: is
every accepted capability covered exactly once, and is every deferred or
rejected candidate covered not at all? That is a bijection over fifty-seven
names, and bijections over fifty-seven names are not reliably checked by
reading.

After this change each of roadmap phase 6's eight capability steps has a
focused child RFC that discharges the cross-cutting contract for its own
helpers, headed by a five-column registry table naming every helper in the
group with its namespace, registration kind, purity class, and manifest-query
availability. That registry is simultaneously the artefact section 6.1 asks for
and the machine-readable anchor for the coverage check. RFC 0006 keeps its
survey and its per-helper contracts unchanged and gains a coverage map.

The observable result is documentation plus one executable contract. A reader
can run `make test` and see a test binary derive the accepted set from RFC
0006's own section 7 disposition tables, derive the forbidden set the same way,
and assert that each accepted helper appears in exactly one child RFC's
registry and no forbidden name appears in any. Dropping a helper, listing it
twice, assigning it to the wrong child, or reintroducing a rejected Ansible
spelling all fail by name.

This plan is approval-gated. It must be reviewed and explicitly approved before
implementation begins.

## Scope divergence from the roadmap's literal wording

Read this before anything else; it explains why the plan does not match the
roadmap text in the working tree.

`docs/roadmap.md:744` currently reads:

```plaintext
- [ ] 6.1.1. Split the RFC 0006 accepted set into focused child issues.
```

with a success bullet requiring "exactly one open child issue". The
commissioned task, and this branch, ask instead for focused **child RFCs and
accompanying roadmap tasks**. This plan implements the commissioned reading and
therefore also rewrites task 6.1.1's own roadmap text, and the identical "child
issue" wording at seven places inside RFC 0006 itself, so that the documents
and the artefacts agree.

Two consequences a reviewer should weigh before approving.

The word *open* is doing work. An issue closes when its capability lands, so
"exactly one open child issue" is a live burn-down. An RFC never closes. The
burn-down is not lost — roadmap checkboxes 6.2.1 through 6.9.2 already provide
it — but this plan does not add a second tracker, and no child RFC gets its own
GitHub issue.

The originating issue is already closed.
[#596](https://github.com/leynos/netsuke/issues/596) was closed on 2026-08-28
with its acceptance criteria, including "accepted capabilities are split into
focused child issues", unfulfilled. Roadmap 6.1.1 is the surviving carrier of
that obligation. Child RFCs therefore cite #596 as their **originating** issue,
not as a tracking issue, so no child RFC claims an open tracker it does not
have.

If the reviewer prefers the literal roadmap wording, stop before `EP-M2`. The
audit and the coverage test in `EP-M0` and `EP-M1` are useful under either
reading; nothing after them is.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- Do not implement this plan until the user explicitly approves it.
- **RFC 0006 section 8 is not moved, gutted, or reduced.** Its per-helper
  contracts stay exactly where 39 roadmap task bullets already cite them. Child
  RFCs are additive. See decision `D3`.
- This work is documentation-only apart from one new test binary and its
  wiring. Do not add or change any Netsuke helper, template registration,
  locale key, command-line flag, configuration field, or Cargo dependency.
- Do not renumber, rename, or delete RFCs 0001 to 0012.
  `docs/documentation-style-guide.md:231` forbids renumbering after publication.
- Do not change any accept, defer, or reject disposition in RFC 0006 sections
  7, 9, or 10. This task partitions the accepted set; it does not relitigate
  it. A disposition that appears wrong goes in `Surprises & discoveries` and is
  escalated.
- Do not resolve any of RFC 0006's seven section 16 open questions. Each is
  assigned to a delivery step and must be settled by that step's implementer.
  Carry each into the child RFC that owns it, unresolved.
- Every child RFC records a release target of v0.1.x or later and must not
  widen the v0.1.0 hardening release defined by
  [#594](https://github.com/leynos/netsuke/issues/594). Note that #594 is
  closed but the latest tag is `v0.1.0-beta3`, so v0.1.0 final has not shipped
  and the constraint is live.
- No child RFC may register a capability that RFC 0006 section 9 defers or
  section 10 rejects **on principle or as a redundant alias**. Names rejected
  because Netsuke or MiniJinja already provides the capability are a different
  class; see decision `D5`.
- Documentation prose follows `docs/documentation-style-guide.md` and uses
  en-GB-oxendict spelling. Body prose wraps at 80 columns; code blocks at 120;
  tables and headings are not wrapped.
- Markdown must be `mdtablefix`-canonical. Run `make fmt` before
  `make check-fmt`. Never write a backtick inside a backticked span in prose;
  `mdtablefix --wrap` corrupts it. The first draft of this plan proved that.
- Run validation commands sequentially, never in parallel, capturing output
  with `tee` under `/tmp`.
- Commit only after gates pass, using `git commit -F`, not `git commit -m`.
- Use the shared default Cargo cache. Do not create an isolated one.

## Tolerances (exception triggers)

- **Aggregate volume.** If the eight child RFCs together exceed 2400 lines,
  stop and escalate. Per-file limits alone cannot catch a set that is
  individually reasonable and collectively disproportionate.
- **Per-file volume.** If any child RFC exceeds 400 lines, stop and escalate.
  A child carries no per-helper contract, so a larger one means section 5 has
  become restatement.
- **Vacuity.** If any section 5 subsection cannot state a group-specific
  consequence — a bound, a registry row, a diagnostic code, a purity
  assignment, a named error condition — and cannot honestly say "no additional
  obligation beyond RFC 0006 section 6.N", stop and escalate. See `D6`.
- **Effort stop-loss.** If any single child-RFC milestone needs more than four
  gate cycles, stop and reassess against `Alternatives considered`.
- **Gate false positives.** If `rfc_stdlib_coverage_tests` fails on a change
  that touches no section 5 registry table and no RFC 0006 section 7 table,
  stop and rescope the parser. Nobody may add an ignore attribute to this test.
- **Number collision.** Re-enumerate remote branches before every child-RFC
  commit, not once. If a number is taken, stop and escalate.
- **Scope.** If implementation requires changing a file outside the set in
  `Interfaces and dependencies`, stop and escalate.
- **Dependencies.** If the coverage test needs a crate not already available,
  stop and escalate.
- **Interface.** If the coverage test would require changing any `src/` file,
  stop and escalate.
- **Ambiguity.** If two child RFCs both have a defensible claim on a helper,
  stop and present options rather than choosing silently.
- **Iterations.** If `make markdownlint` or `make check-fmt` still fails after
  three focused attempts on one file, stop and escalate with the log path.

## Risks

- Risk: the section 5 subsections are structurally perfect and semantically
  empty — each a restatement of the clause it discharges. A structural test
  would certify the emptiness, because heading presence is exactly the property
  vacuous prose has. Severity: high. Likelihood: high. Mitigation: this is the
  plan's principal risk and the reason for three controls. `D6` cuts section 5
  to five mandatory substantive clauses and permits a single declarative line
  for the rest. The anti-vacuity rule gives a reviewer a one-line test.
  `CONF-1` requires each subsection to name at least one helper the RFC owns
  and to contain no Ansible-deference phrase. And `EP-M3` is a hard go/no-go:
  if section 5 for the smallest group tells a reviewer nothing they did not
  already know from RFC 0006 section 6, the remaining seven are not written.

- Risk: the split stalls half-finished, and nothing is red because the coverage
  invariant is satisfied by both the finished and the abandoned state.
  Severity: high. Likelihood: medium. Mitigation: because section 8 is not moved
  (`D3`), a half-finished split leaves every roadmap citation working and
  every RFC 0006 contract intact — the abandoned state is incomplete, not
  incoherent. The coverage map carries a status column and `COV-4` reports the
  unwritten count in the passing test output, so a stall announces itself on
  every run.

- Risk: an RFC number is taken by a concurrent branch. The repository's
  measured base rate is not low: `docs/` contains `adr-003` twice, `adr-004`
  three times, and `adr-014` twice — three collisions that all merged.
  Severity: medium. Likelihood: medium. Mitigation: `D2` allocates lazily, one
  number per child at the commit that creates it, and lands the reservation
  rows in RFC 0006 as the first mergeable commit. A reservation on a branch
  reserves nothing. Remote heads are re-enumerated before every child commit
  and once more before merge.

- Risk: a rejected Ansible alias is reintroduced while writing a child RFC,
  because the alias reads naturally. Severity: medium. Likelihood: medium.
  Mitigation: `COV-2`, whose forbidden set is **derived** from RFC 0006 section
  7's disposition column rather than typed by hand. The first draft hand-wrote
  that list and omitted sixteen names, including `is_file` — the sibling of the
  very alias this risk names. That is the evidence for deriving.

- Risk: RFC 0006 is `Status: Proposed` and has never been ratified; no RFC in
  the corpus ever has. Freezing its dispositions and encoding its totals in a
  gate may prove premature. Severity: medium. Likelihood: medium. Mitigation:
  nothing here changes a disposition, and because section 8 is not moved, a
  later change costs a registry row and a coverage-map row rather than a
  document rewrite. `ADR-021` carries the amendment procedure named in `D9`.

- Risk: the reviewer concludes the split is not worth its cost.
  Severity: medium. Likelihood: medium. Mitigation: `Alternatives considered`
  states the fallback plainly, and `EP-M0` and `EP-M3` are both go/no-go points
  before most cost is incurred.

## Progress

- [ ] `EP-M0` Audit; confirm the partition and the derivation rules. Go/no-go.
- [ ] `EP-M1` Land the coverage test, `ADR-021`, the RFC 0006 corrections and
  reservations, and the roadmap 6.1.1 rewrite. Ship as its own pull request.
- [ ] `EP-M2` Write the literal child-RFC template and one worked section 5.
- [ ] `EP-M3` RFC 0013, structured data interchange (step 6.2). **Go/no-go.**
- [ ] `EP-M4` RFC 0014, mapping and sequence transforms (step 6.3).
- [ ] `EP-M5` RFC 0015, ordered collection algebra and truth predicates (6.4).
- [ ] `EP-M6` RFC 0016, pattern and version predicates (step 6.5).
- [ ] `EP-M7` RFC 0017, lexical path composition (step 6.6).
- [ ] `EP-M8` RFC 0018, host-state predicates and environment expansion (6.7).
- [ ] `EP-M9` RFC 0019, encoding, identity, and formatting (step 6.8).
- [ ] `EP-M10` RFC 0020, date and time conversion (step 6.9).
- [ ] `EP-M11` Reconcile, retarget roadmap citations, run all gates, mark
  roadmap 6.1.1 done.

## Surprises & discoveries

- Observation: RFC 0006 section 8.1 opens "All six helpers in this group are
  pure" but specifies five. Evidence: `docs/rfcs/0006-...md:650` against the
  five headings at 654, 671, 694, 707, and 728. Impact: correct to "five" in
  `EP-M1`. The group totals do reconcile, so the defect is the word, not the
  accepted set.

- Observation: RFC 0006 section 8.6's group preamble states "Every helper in
  this group is **pure and lexical**", but `expandvars` is in that group and
  section 8.6 itself calls it the one environment-observing helper in the RFC.
  Evidence: `docs/rfcs/0006-...md:1099` against `:1184`. Impact: a second
  defect of the same class, sitting exactly on the seam this plan cuts. Correct
  in `EP-M1` by scoping the sentence to the lexical helpers.

- Observation: the naive heading count under section 8 is 58, not 57.
  Evidence: three headings are prose rather than helpers (`:953`, `:1084`,
  `:1502`); three headings each name two helpers (`:1280`, `:1289`, `:1321`);
  `glob` (`:1261`) is an existing helper; and `basename` and `dirname` have
  **no heading at all**, appearing only in the prose at `:1095`. Impact:
  decisive. Any heading-based parser is wrong before it is written. This is the
  strongest single reason the coverage test anchors on tables. See `D4`.

- Observation: section 7.1 gives `basename`, `dirname`, and `expanduser` the
  disposition `Reject`, with a note saying the helper already exists and gains a
  `dialect` argument. Evidence: `docs/rfcs/0006-...md:468-470`. Impact:
  `Reject` is overloaded three ways — table 11 splits it into 22 "already
  provides", 10 "redundant alias", and 18 "on principle". Only the latter two
  classes are forbidden. Two of the three helpers table 11 counts as gaining a
  behaviour-preserving option are `Reject` rows. The derivation rules in `D5`
  must handle this, or the test will forbid the very helpers it requires.

- Observation: section 7 contains no accept row for `splitdrive` or
  `shell_quote`. Both arrive from `Reject` rows — `win_splitdrive` and
  `quote` — via the rename note in section 7.8. Evidence:
  `docs/rfcs/0006-...md:478` and `:485`, reconciled at `:635-640`. Impact:
  "every accepted capability covered once, every rejected covered never" is
  unsatisfiable read naively for three helpers. The rejected thing is the
  *Ansible spelling*; the *Netsuke capability* is accepted. `D5` records this
  as an explicit three-row exception table.

- Observation: the roadmap already deep-links every phase-6 task to its RFC
  0006 section. Phase 6 contains 51 such references, 39 of them to a section 8
  subsection. Evidence: `docs/roadmap.md:715-1267`; for example task 6.3.1 at
  `:863`. Impact: this falsifies the first draft's premise that a contributor
  must read 2132 lines. Reaching the `combine` contract today costs about 80
  lines. The plan's stated value had to be rebuilt around what is genuinely
  absent — the per-helper contract obligations — rather than around navigation.

- Observation: `make doc-coverage` runs `cargo rustdoc --show-coverage` over
  library and binary targets only; integration tests are not measured. Evidence:
  `scripts/doc-coverage.py`; `Makefile:206-210`. Impact: the governing gate on
  the new test file is `missing_docs_in_private_items = "deny"`
  (`Cargo.toml:252`) under `make lint`, which does apply and covers enum
  variants and struct fields. `unwrap_used` and `expect_used` are also denied
  (`Cargo.toml:213-214`), so parser helpers must return `anyhow::Result`.

- Observation: `mdtablefix --wrap` does not wrap headings or table rows. A
  113-character heading sits on green `main` at `docs/rfcs/0006-...md:982`.
  Impact: both candidate anchors are safe from reflow, so the choice between
  them rests on semantics. Backticked spans in **prose** are not safe, which is
  what corrupted the first draft.

- Observation: no tooling parses `docs/roadmap.md`, and `markdownlint-cli2` is
  configured with no cross-file link or anchor validation. Impact: a stale
  relative link between documents is caught by nothing. `COV-5` adds that check
  to the coverage test, which is already reading every file in `docs/rfcs/`.

## Decision log

- Decision `D1`: eight child RFCs, one per roadmap phase-6 capability step 6.2
  to 6.9. Step 6.1 gets no child RFC. Rationale: the roadmap's steps are a
  complete, disjoint cover of RFC 0006 sections 8.1 to 8.10, and they cut on
  the **purity seam**, which section 14's delivery slices do not. Slice 5
  bundles pure lexical path helpers with filesystem-observing predicates; the
  roadmap instead puts the lexical helpers and the pure `abs` test in step 6.6
  and the observing helpers in step 6.7. Since section 5.1 of each child is a
  purity registry, a purity-aligned partition makes seven of eight children
  uniformly pure and one uniformly non-pure, and makes every clause about
  capability, determinism, and manifest-query availability uniform within a
  child. Slice alignment would produce a child straddling two roadmap steps and
  mixing purity classes. Step 6.1 gets no child because its subject is the
  shared machinery of sections 6 and 14.1, which is not a capability in section
  7's sense; such a child would own no helper and restate what RFC 0006 already
  specifies. Date/Author: 2026-09-08, planning agent.

- Decision `D2`: allocate RFC numbers 0013 to 0020 **lazily**, one per child at
  the commit that creates it, after landing reservation rows in RFC 0006's
  number-allocation table as part of `EP-M1`. Rationale: numbers become
  irreversible on merge, and this repository has a measured collision base rate
  — `adr-003`, `adr-004`, and `adr-014` are all duplicated on disk. A
  reservation that lives only on this branch reserves nothing. The partition is
  stated by slug, so nothing depends on contiguity. `EP-M1` also backfills 0007
  to 0012 into that table and deletes its false claim that no RFC has been
  merged to `main`. Date/Author: 2026-09-08, planning agent.

- Decision `D3`: **RFC 0006 section 8 stays intact and normative.** Child RFCs
  are additive. This reverses the first draft, which migrated the per-helper
  contracts out. Rationale: five lines of evidence. The repository's
  established convention is additive — RFCs 0009, 0010, and 0011 each declare an
  `Amends` preamble bullet naming RFC 0001 and instruct that the amendment be
  folded back into the parent before promotion, keeping the parent normative
  throughout. Migration would leave 39 roadmap task bullets citing a stub, with
  no link gate to notice. Two capability groups split across two milestones,
  and the shared `dialect` block that section 8.6 defines is consumed by
  helpers in both, so the intermediate states would be genuinely incoherent
  rather than merely incomplete. Migration accounts for only 935 of roughly
  3500 lines of first-draft deliverable, so it buys little and risks most. And
  the measured contributor read path gets worse, not better. What is lost: a
  child RFC is not self-contained; a reader consults RFC 0006 section 8.N for
  the contract. That is accepted, and the Purpose section no longer claims
  otherwise. Date/Author: 2026-09-08, planning agent.

- Decision `D4`: coverage is anchored on **tables, not headings**. Each child
  RFC's section 5.1 is a five-column registry — helper, namespace,
  registration, purity class, manifest query — with one row per helper the RFC
  owns. That registry is the ownership signal. Rationale: heading anchoring is
  unworkable, provably. `basename` and `dirname` have no heading, so two of the
  three optioned helpers would be permanently unowned. Three headings name two
  helpers each. Three headings under section 8 are prose. And RFC 0006's own
  sections 10.3, 11.1, 11.4, 11.5, 11.7, 11.8, and 15.3 are headings containing
  backticked helper names in non-normative contexts, so a heading scan produces
  false duplicate-ownership and false forbidden-name hits against the parent
  document. A table cell cannot be mentioned in passing. The registry is also
  the artefact section 6.1 requires and which exists nowhere today, so it is
  worth writing regardless of the test. Date/Author: 2026-09-08, planning agent.

- Decision `D5`: the accepted and forbidden sets are **derived from RFC 0006
  section 7's disposition tables and section 7.8's totals**, not hardcoded.
  Rationale: section 7 states its own purpose as the index — every surveyed
  name with an explicit disposition and, for accepted names, the owning section
  8 subsection. Hardcoding would create a third copy of the inventory in a
  different language, drifting independently of the two it mirrors, which is
  the exact failure this task exists to prevent. The evidence that hand
  maintenance fails is that the first draft's list omitted sixteen names. Three
  derivation rules, which the test must state:
  1. **Alias groups.** Section 7.8 notes that rows cover alias groups as single
     entries. Split the name cell on its separator.
  2. **Renames.** Three accepted capabilities are registered under a Netsuke
     name rather than the surveyed one: Ansible's `hash` becomes `text_hash`,
     its `quote` becomes `shell_quote`, and its `win_splitdrive` becomes
     `splitdrive` with a windows dialect. Their section 7 rows say `Reject`.
     This is an explicit three-row exception table transcribed from section
     7.8, asserted to have exactly three rows.
  3. **Reject is overloaded.** Table 11 splits it into 22 "already provides",
     10 "redundant alias", and 18 "on principle". Only the latter 28, plus the
     6 deferred, are forbidden. Rows whose note says the capability already
     exists are not forbidden — `basename`, `dirname`, and `expanduser` are
     such rows, and two of them are required helpers. `EP-M0` must confirm the
     note column discriminates the three classes reliably; if it does not,
     `EP-M1` adds a discriminating column to section 7, which changes no
     disposition and makes the document machine-readable by design.
  Date/Author: 2026-09-08, planning agent.

- Decision `D6`: section 5 has five mandatory substantive clauses; the other
  six may be a single declarative line. Rationale: seven of the eight children
  own only pure helpers, so clauses 6.2 to 6.5 discharge to the same four
  sentences in each — roughly 336 lines of literal restatement across the set,
  with 6.5, 6.10, and 6.11 adding more. Mandating eleven prose subsections per
  child is a vacuity generator. Substantive and per-helper: **5.1** the
  registry, **5.6** type and error contract, **5.7** canonical value equality,
  **5.8** resource bounds, and **5.9** diagnostics, which must give a code of
  the form `netsuke::jinja::<module>::<reason>` per error condition. Permitted
  to be one line: 5.2, 5.3, 5.4, 5.5, 5.10, and 5.11. **Anti-vacuity rule**,
  applied by a reviewer in one pass: every subsection states either a
  group-specific consequence — a bound, a registry row, a diagnostic code, a
  purity assignment, a named error condition — or the exact words "No
  additional obligation beyond RFC 0006 section 6.N." A bare restatement of the
  clause is a review reject. Date/Author: 2026-09-08, planning agent.

- Decision `D7`: roadmap steps 6.10 and 6.11 get no child RFC.
  Rationale: 6.10 covers section 9's deferred candidates, which the success
  criterion requires be covered by none. 6.11 is outside RFC 0006's
  Ansible-derived set and already has
  `docs/git-change-detection-helpers-design.md` and
  [ADR-015](../adr-015-use-bounded-git-cli-for-change-detection.md).
  Date/Author: 2026-09-08, planning agent.

- Decision `D8`: rewrite roadmap task 6.1.1 from "child issues" to "child RFCs
  and accompanying roadmap tasks", and the same wording at the seven places
  inside RFC 0006 that say "child issue". Rationale: leaving either document
  saying "issues" while the tree contains eight child RFCs would leave both
  describing work nobody did. RFC 0006 section 6 opens by saying a child issue
  that does not satisfy every clause is not complete, which after this change
  is the definition of a child RFC's section 5. See `Scope divergence` for what
  the word *open* cost. Date/Author: 2026-09-08, planning agent.

- Decision `D9`: record the convention in
  `docs/adr-021-focused-child-rfcs-for-survey-rfcs.md`, scoped narrowly.
  Rationale: allocating eight numbers under a particular partition is hard to
  reverse. But the ADR must not claim to generalize. It applies to **survey
  RFCs** — documents that enumerate a large candidate set with a per-candidate
  disposition — and explicitly does **not** replace the `Amends` convention
  that RFCs 0009 to 0011 use for normative amendments to RFC 0001. The first
  draft claimed the convention would extend to RFC 0001; that would have
  contradicted those three amendments' own fold-back instruction. The ADR must
  also carry the **amendment procedure**: the ordered touchpoints to edit when
  a helper is added, removed, or renamed after the split — the section 7 row,
  the section 8 subsection, the section 14 coverage map, the owning child's
  registry, and the roadmap task — so the coverage test is a guard rail rather
  than a ratchet. The highest existing ADR is 020; three earlier numbers
  collided, so re-check before committing. Date/Author: 2026-09-08, planning
  agent.

## Alternatives considered

The first draft had no such section. Two alternatives are live at the approval
gate.

**Stop after `EP-M1`.** The coverage test, `ADR-021`, the RFC 0006 defect
corrections, and the roadmap rewrite together solve the mechanical half of the
problem — the bijection nobody can check by reading — for roughly a tenth of
the cost and none of the irreversibility. No RFC number is spent. The
per-helper obligations would remain underived, which is the other half of the
value, but they could later be added to RFC 0006 section 8 in place as a
registry table per group, at roughly 140 lines rather than roughly 1800. If the
reviewer judges eight child RFCs disproportionate, this is the fallback, and
`EP-M0` and `EP-M3` are positioned so it can still be taken.

**Six children rather than eight.** RFC 0020 owns two helpers and RFC 0018 owns
one filter plus four tests and an option. Merging date and time into encoding
and formatting, and the version predicate into collection algebra, would give
six children with no group under about 120 lines of section 8 content. The cost
is two broken one-to-one step mappings, which weakens the success criterion's
phrasing. Cheaper, but not recommended.

Rejected: aligning to section 14's ten delivery slices, for the purity-seam
reason in `D1`. Rejected: unnumbered per-group design documents on the pattern
of roadmap step 6.11 — genuinely cheaper and fully reversible, but the
commissioned task asks for RFCs, and a normative contract discharge belongs in
the RFC review process.

## Outcomes & retrospective

To be completed at `EP-M11`. Before marking `COMPLETE`, reconcile every
discovery against RFC 0006, `docs/roadmap.md`, and the style guide, and confirm
no disposition changed.

## Context and orientation

Netsuke reads a YAML manifest called a `Netsukefile`, expands Jinja templates
in it with MiniJinja, and generates a Ninja build file. The "template standard
library" is the set of filters, tests, and functions Netsuke registers with
MiniJinja.

Terms, defined once:

- **Filter**: invoked as `value | name(args)`. **Test**: invoked as
  `value is name(args)`. **Function**: invoked as `name(args)`. Jinja keeps the
  three in separate namespaces.
- **Manifest query**: the read-only environment serving `netsuke help targets`.
  A helper that reads the clock, environment, filesystem, network, or a
  subprocess is registered there only as a failing stub.
- **Purity class**: RFC 0006 section 6.1's label — pure, clock-observing,
  environment-observing, filesystem-observing, network-observing, or
  subprocess-observing.
- **Canonical key**: the RFC 8785 canonical JSON form of a value, giving a
  deterministic equality relation so no helper's output order comes from a hash
  table. RFC 0006 section 6.7.
- **Survey RFC**: an RFC that enumerates a large candidate set with a recorded
  disposition per candidate. RFC 0006 is the only one.
- **Registry**: the five-column table at section 5.1 of each child RFC.

Files that matter:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` — 2132 lines,
  status `Proposed`. Section 6 is the eleven-clause contract; section 7 is the
  disposition matrix and the derivation source; section 8 is the per-helper
  contracts, which stay put; section 14 becomes the coverage map.
- `docs/roadmap.md` — phase 6 spans lines 715 to 1267. Step 6.1 is shared
  machinery; 6.2 to 6.9 are the capability groups; 6.10 is deferred; 6.11 is
  Git change detection.
- `docs/contents.md` — the `## Requests for comments` section at lines 35 to 82
  is the only RFC index. Note how RFCs 0009 to 0011 are described there, as a
  normative amendment adding something to RFC 0001; child RFCs need an
  analogous phrasing.
- `docs/documentation-style-guide.md:222-360` — RFC naming, required and
  conditional sections, formatting guidance, and a literal template.
- `tests/documentation_installation_tests.rs` — the precedent for splitting a
  test binary across a sibling module.
  `tests/integration_test_wiring_tests.rs` — the contract governing how the new
  binary must be wired; note at lines 101 to 146 that a sibling module tree
  must be explicitly declared or it is flagged orphaned, and that the
  path-attribute form requires the module name to match the directory name.

For wider architecture see `docs/netsuke-design.md`. For the testing idioms the
child RFCs impose on their implementers, see
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/rust-doctest-dry-guide.md`, and
`docs/snapshot-testing-in-netsuke-using-insta.md`. When writing RFC 0018, load
the `hexagonal-architecture` skill: `expandvars` and the filesystem predicates
are this plan's only ports, and the boundary to preserve is between pure domain
policy — path lexing, purity classification — and the injected adapters, the
`cap_std` workspace handle and the environment reader, that RFC 0006 section
6.4 and [ADR-008](../adr-008-environment-seam-taxonomy.md) mandate. Route Rust
questions for the coverage test through `rust-router`, which points at
`rust-unit-testing`. Use `scrutineer` for gate runs and `scribe` for the
`docs/contents.md` entries and the roadmap citation retargeting; do not
delegate any section 5, which is new normative content.

### The partition

Every later milestone is bookkeeping against this table. The ownership
qualifiers are exact: the first draft wrote unqualified group numbers and
thereby double-assigned seven helpers.

| Child RFC | Title                                           | Owns                                 | Step |
| --------- | ----------------------------------------------- | ------------------------------------ | ---- |
| 0013      | Structured data interchange helpers             | §8.1                                 | 6.2  |
| 0014      | Mapping and sequence transform helpers          | §8.2                                 | 6.3  |
| 0015      | Ordered collection algebra and truth predicates | §8.3, §8.8                           | 6.4  |
| 0016      | Pattern and version predicates                  | §8.4, §8.5                           | 6.5  |
| 0017      | Lexical path composition                        | §8.6 except `expandvars`; §8.7 `abs` | 6.6  |
| 0018      | Host-state predicates and environment expansion | §8.7 except `abs`; §8.6 `expandvars` | 6.7  |
| 0019      | Encoding, identity, and formatting helpers      | §8.9                                 | 6.8  |
| 0020      | Date and time conversion helpers                | §8.10                                | 6.9  |

*Table 1: Child RFC allocation against RFC 0006 groups and roadmap steps.*

The two divisions are the roadmap's own and are affirmatively supported by RFC
0006. Section 8.7 calls `abs` pure and lexical and says that because it is
pure, it is available during manifest queries unlike the other four tests in
its group; roadmap task 6.6.4 places it in step 6.6 for that reason. Section
14.7 separates `expandvars` from the lexical helpers because it is the only
environment-observing helper in the RFC and therefore the only one needing an
injected reader, a manifest-query stub, and its own capability review. Together
the cuts make RFC 0017 uniformly pure and RFC 0018 the only child with non-pure
helpers.

The registry contents, which are the coverage test's expected ownership:

- RFC 0013, five filters: `from_json`, `from_yaml`, `from_yaml_all`, `to_yaml`,
  `to_nice_json`.
- RFC 0014, six filters: `combine`, `dict2items`, `items2dict`, `extract`,
  `subelements`, `rekey_on_member`.
- RFC 0015, eight filters — `union`, `intersect`, `difference`,
  `symmetric_difference`, `product`, `combinations`, `permutations`,
  `zip_longest` — and seven tests: `any`, `all`, `subset`, `superset`,
  `contains`, `truthy`, `falsy`.
- RFC 0016, four filters — `regex_replace`, `regex_search`, `regex_findall`,
  `regex_escape` — and four tests: `match`, `search`, `regex`, `version`.
- RFC 0017, six filters — `path_join`, `normpath`, `splitext`, `commonpath`,
  `relpath`, `splitdrive` — one test, `abs`, and the optioned existing filters
  `basename` and `dirname`.
- RFC 0018, one filter, `expandvars`; four tests — `exists`, `link_exists`,
  `same_file`, `mount` — and the optioned existing function `glob`.
- RFC 0019, nine filters: `b64encode`, `b64decode`, `urldecode`, `to_uuid`,
  `shell_quote`, `comment`, `human_readable`, `human_to_bytes`, `text_hash`.
- RFC 0020, two filters: `to_datetime`, `strftime`.

That is 41 new filters, 16 new tests, and 3 optioned existing helpers, matching
table 11. Aggregating the purity columns must yield 52 pure, 4
filesystem-observing, and 1 environment-observing, matching section 6.1 — an
independent cross-check the coverage test performs as `COV-3`.

RFC 0006's section 16 open questions distribute as follows. Question 1 on
`to_nice_yaml` and question 4 on alias bounding go to RFC 0013; question 3 on a
version prefix goes to RFC 0016; question 2 on the `abs` test name goes to RFC
0017; question 6 on a truncating hash sibling goes to RFC 0019. Questions 5 and
7 belong to no child: question 5 concerns the shared bounds of step 6.1, and
question 7, on an injected clock, is already owned by roadmap task 7.1.1, "Add
the clock provider seam to the stdlib time module", on a separate reserved
branch. Both stay in RFC 0006 section 16, and `EP-M1` annotates question 7 with
a pointer to task 7.1.1 so a phase-6 implementer does not adopt it by accident.
All seven stay open.

### The child RFC skeleton

Copy this literally. It is the style guide's template, retaining
`## Current state` and `## Alternatives considered` as conditional-but-expected
for RFCs 0017 and 0018, which must contrast `relpath` against the existing
`relative_to` and `splitext` against `with_suffix`, and must carry RFC 0006
section 15.5's analysis of the rejected Windows-specific filter family.

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

### 5.1. Purity and manifest-query registry

<The five-column table. Mandatory.>

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

## 6. Dependencies

<Crates from RFC 0006 section 13.4, and the child RFCs this one requires.>

## 7. Delivery

<The roadmap tasks that implement this RFC, by number.>

## 8. Open questions

<RFC 0006 section 16 questions assigned to this group, carried over
unresolved.>

## 9. Recommendation
```

The registry at section 5.1 has exactly these columns and this row shape. The
example is RFC 0017's, and is the worked example `EP-M2` must produce in full:

| Helper      | Namespace | Registration | Purity class | Manifest query |
| ----------- | --------- | ------------ | ------------ | -------------- |
| `path_join` | Filter    | New          | Pure         | Available      |
| `abs`       | Test      | New          | Pure         | Available      |
| `basename`  | Filter    | Option added | Pure         | Available      |

*Table 2: The registry row shape.*

`Registration` is `New` or `Option added`, distinguishing the 57 new helpers
from the 3 existing helpers gaining a behaviour-preserving option.
`Purity class` is one of the six values in RFC 0006 table 2. `Manifest query` is
`Available` or `Stub`. The coverage test parses exactly these cells.

## Conformance basis

There is no Terms of Reference document. Upstream artefacts:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md` at
  `origin/main` commit `924cb215`, status `Proposed`. Sections 6, 7, 7.8, 8, 9,
  10, 13.4, 14, and 16 are load-bearing.
- `docs/roadmap.md` at the same commit, phase 6, task 6.1.1 and steps 6.2
  to 6.9.
- `docs/documentation-style-guide.md:222-360`.
- [ADR-008](../adr-008-environment-seam-taxonomy.md) governs the `expandvars`
  seam RFC 0018 discharges;
  [ADR-010](../adr-010-scope-glob-capability-to-literal-prefix.md) governs the
  `glob` option; and
  [ADR-001](../adr-001-replace-serde-yml-with-serde-saphyr.md) governs the YAML
  stack RFC 0013 discharges.
- `docs/adr-021-focused-child-rfcs-for-survey-rfcs.md`, created at `EP-M1`.

Trace links:

```plaintext
RFC0006-S7   -> ROADMAP-6.1.1 -> EP-M1 -> COV-1 -> tests::rfc_stdlib_coverage::every_accepted_helper_has_exactly_one_owner
RFC0006-S9   -> ROADMAP-6.1.1 -> EP-M1 -> COV-2 -> tests::rfc_stdlib_coverage::no_forbidden_helper_is_registered
RFC0006-S6.1 -> ROADMAP-6.1.1 -> EP-M1 -> COV-3 -> tests::rfc_stdlib_coverage::totals_and_purity_aggregate_agree
RFC0006-S14  -> ROADMAP-6.1.1 -> EP-M1 -> COV-4 -> tests::rfc_stdlib_coverage::coverage_map_status_is_reported
RFC0006-S6   -> ROADMAP-6.1.1 -> EP-M3..EP-M10 -> CONF-1 -> tests::rfc_stdlib_coverage::every_child_discharges_every_clause
ADR-021      -> EP-M1 -> docs/adr-021-focused-child-rfcs-for-survey-rfcs.md
```

## Verification plan

This change adds no runtime behaviour, so there is no invariant over program
state. It introduces a combinatorial invariant over documents — a bijection
between the accepted helper set and its owning child RFCs — which is statable
and mechanically checkable, and a semantic obligation about the adequacy of the
prose discharges, which is not.

The plan is explicit about that asymmetry, because the first draft was not. The
mechanical obligations cover the coverage bijection. The substantive product of
this task is section 5's prose, and no test can establish that it says
anything. Three controls bound the gap: `D6` cuts the clause count and states
an anti-vacuity rule a reviewer applies in one pass; `CONF-1` mechanically
rejects the three commonest vacuity shapes; and `EP-M3` is a hard go/no-go on
the first completed child. The pull request must not claim more.

Method selection: table-driven Rust tests over parsed Markdown tables. The
domain is a fixed finite set, fully enumerable, so exhaustive enumeration is
the strongest available evidence and property testing would add nothing.
Recorded explicitly because repository guidance otherwise prefers property
tests for invariants over ranges.

**Parser contract, common to all obligations.** The parser reads only Markdown
table rows, never headings, and only within a named section: RFC 0006's section
7 subtables and its section 14 coverage map, and each child's section 5.1
registry. It splits on the cell separator, trims, and strips one pair of
backticks. It matches names by whole-token equality, never substring — the
vocabulary contains `abs` against `is_abs`, `quote` against `shell_quote`,
`hash` against `text_hash`, and `subset` against `issubset`, and substring
matching would be wrong on all four. It records file and line for every parsed
row so failures name a location.

### Obligation `COV-1`: exactly one designated owner

- Obligation: every helper in the accepted set derived from RFC 0006 section 7
  appears in the section 5.1 registry of exactly one child RFC, and that RFC is
  the one the section 14 coverage map designates.
- Method: set comparison across three independently sourced signals — the
  accepted set from section 7, the expected owner from the coverage map, and
  the actual owner from the child registries.
- Rationale: three signals from two documents, none hardcoded. A count-only
  check would pass when a helper is assigned to the wrong child, leaving the
  plan's most contestable decisions unguarded.
- Domain: 57 new helpers plus 3 optioned existing helpers, against RFC 0006 and
  RFCs 0013 to 0020.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs` and
  `tests/rfc_stdlib_coverage/`.
- Evidence: `cargo nextest run --test rfc_stdlib_coverage_tests`. Green at
  `EP-M1` with every coverage-map row marked not yet written, and green at
  every plateau thereafter as rows flip to a written child.
- Non-vacuity: four seeded faults, each applied to a scratch copy and reverted,
  with transcripts recorded. Delete the `combine` row from RFC 0014's registry
  and expect a zero-owner failure naming `combine` and the file. Add `combine`
  to RFC 0015's registry as well and expect a two-owner failure naming both.
  **Move `combine` wholesale from RFC 0014 to RFC 0015** and expect a
  wrong-owner failure — the control the first draft lacked, without which the
  partition is untested. Corrupt one section 7 accept row and expect the
  derived set to shrink and the failure to name the row. The test also asserts
  the derived accepted set has exactly 60 members, so a parser returning
  nothing cannot pass vacuously.

### Obligation `COV-2`: no forbidden candidate is registered

- Obligation: no name that RFC 0006 section 9 defers, or that section 7 rejects
  as a redundant alias or on principle, appears in any child RFC's registry.
- Method: derived deny set from section 7's disposition column, minus the
  "already provides" class per `D5` rule 3, plus section 9's six names.
- Rationale: a hand-maintained list is not a sound contract when the source of
  truth is a tracked file in the same repository. Deriving means the set
  tightens automatically when section 7 gains a row.
- Domain: the derived forbidden set, expected to be 34 names.
- Artefact: as above.
- Evidence: same command. Green from `EP-M1`, which is exactly why the controls
  below are compulsory rather than optional.
- Non-vacuity: this obligation is green before any child exists, so a pass
  proves nothing on its own. Add a registry row for `shuffle` to a scratch copy
  of RFC 0015 and expect a failure naming `shuffle` and the file. Add `is_dir`
  and expect a failure naming the rejected alias. Assert the derived deny set
  has exactly 34 members and contains `is_file`, `quote`, `fileglob`, and
  `lookup` — four names the first draft's handwritten list omitted. Assert it
  does **not** contain `basename`, `dirname`, or `expanduser`, which are
  `Reject` rows of the already-provides class and are required helpers; without
  this assertion `D5` rule 3 is untested.

### Obligation `COV-3`: totals and purity aggregate agree

- Obligation: the derived accepted set contains 41 filters, 16 tests, and 3
  optioned helpers, matching the totals parsed from table 11; and the
  registries' purity columns aggregate to 52 pure, 4 filesystem-observing, and
  1 environment-observing, matching section 6.1.
- Method: parsed count against parsed count.
- Rationale: genuinely independent, unlike the first draft's version, which
  compared a hardcoded inventory against a hardcoded literal transcribed from
  the same table in the same sitting. Here the counts come from the child
  registries and the expectations from RFC 0006, so neither can be adjusted to
  match the other without editing a normative document. The purity aggregate is
  the only check reconciling section 6.1 against the registries; without it a
  wrong purity class rots silently.
- Domain: three totals and three purity counts.
- Artefact: as above.
- Evidence: same command. The purity half is necessarily partial until every
  child exists; it compares against the aggregate over written children and
  asserts the full totals only when the coverage map has no unwritten row.
- Non-vacuity: change one registry row's purity class from pure to
  filesystem-observing and expect a failure reporting 51 against 52. Change a
  helper's namespace and expect the filter and test totals to fail. Introduce a
  purity class not among table 2's six values and expect a vocabulary failure.

### Obligation `COV-4`: the coverage map reports progress honestly

- Obligation: the section 14 coverage map has one row per capability group with
  an explicit owner and status, and the test reports in its passing output how
  many groups remain unwritten.
- Method: parse and report.
- Rationale: a split that stalls half-finished satisfies every other obligation
  — the abandoned state and the finished state are indistinguishable to a
  bijection check. This makes a stall announce itself on every test run rather
  than waiting for someone to notice.
- Artefact: as above.
- Evidence: passing output includes a line naming the unwritten count. `EP-M11`
  asserts it is zero.
- Non-vacuity: at `EP-M1` the reported count must be 8, not 0. A count of 0
  before any child exists means the map is not being read.

### Obligation `COV-5`: inter-document links resolve

- Obligation: every relative Markdown link from a file in `docs/rfcs/` to
  another repository path resolves to an existing file.
- Method: path existence check over parsed links.
- Rationale: this plan adds eight documents, eight coverage-map links, and
  eight `docs/contents.md` entries, and retargets 39 roadmap citations.
  `markdownlint-cli2` is configured with no link validation and there is no
  link checker in the repository, so a stale link is caught by nothing. The
  parser already reads every file in `docs/rfcs/`, so this is nearly free.
- Artefact: as above.
- Evidence: same command.
- Non-vacuity: point a scratch copy's link at a non-existent file and expect a
  failure naming the source line and the missing target.

### Obligation `CONF-1`: every child discharges every clause

- Obligation: each child RFC contains one section 5 subsection per clause of
  RFC 0006 section 6; each subsection's body is non-empty and either names at
  least one helper that RFC owns or contains the exact escape phrase from `D6`;
  and no subsection contains an Ansible-deference phrase.
- Method: structural and lexical checks, plus reviewer judgement for the rest.
- Rationale: the mechanical half catches the three commonest vacuity shapes —
  the empty stub, the generic discharge naming no helper, and the appeal to
  Ansible. The last is literally this plan's own acceptance criterion and is a
  substring search, so leaving it to review would be indefensible. The clause
  list is derived by parsing section 6's subsection headings rather than
  hardcoding eleven, so a future section 6.12 does not break eight documents.
  Subsections are located by position under section 5 and by title match, not
  by literal numerals, because two independent formatters in this repository
  are empowered to renumber.
- Domain: RFCs 0013 to 0020 against section 6's clause list.
- Artefact: as above.
- Evidence: same command, scoped to children that exist.
- Non-vacuity: delete subsection 5.8 from a scratch copy of RFC 0015 and expect
  a failure naming the missing clause and the file. Replace a subsection body
  with a comment and expect an empty-body failure. Replace one with prose
  naming no owned helper and lacking the escape phrase, and expect a
  generic-discharge failure. Insert the words "as Ansible does" and expect a
  deference failure.
- **Residual gap.** Whether section 5.7 genuinely discharges canonical equality
  for `subset`, as opposed to restating clause 6.7, is a judgement no test
  makes. This is the substantive product of the task and it rests on review,
  bounded by `D6`'s anti-vacuity rule and by the `EP-M3` go/no-go. Do not claim
  otherwise in the pull request.

### Axioms

- RFC 0006's dispositions are correct. This task partitions them.
- Table 11's totals of 41, 16, and 3, and section 6.1's aggregate of 52, 4, and
  1, are correct. `EP-M0` re-derives both and must agree.
- `markdownlint-cli2`, `typos`, and `mdtablefix` behave as configured.
- Section 7's note column reliably discriminates the three reject classes. This
  is the weakest axiom in the plan; `EP-M0` must confirm it and `D5` rule 3
  states the remedy if it fails.
- Jinja namespaces are separate, so a filter and a test may share a name. No
  name in the accepted set does; `abs` collides only with a pre-existing
  MiniJinja built-in that is not in the inventory. The registry's namespace
  column is carried for failure-message quality and forward insurance, not
  because it disambiguates anything today.

## Milestones and plateaus

Because section 8 is not moved, every plateau is coherent by construction: RFC
0006 is unchanged as a specification throughout, and each child adds a
conformance record. An abandoned split leaves a repository that is incomplete,
never one that is inconsistent. This is a materially weaker claim than the
first draft made, and it is true.

### `EP-M0` — audit and derivation rules. Go/no-go

- Outcome: the partition and the three `D5` derivation rules are confirmed
  against the document.
- Acceptance evidence: recorded in `Surprises & discoveries` — the heading
  recount with its four reconciliation adjustments stated explicitly, since the
  naive count is 58 and never 57; confirmation that section 7's note column
  discriminates the three reject classes; the derived accepted set at 60 and
  forbidden set at 34; the purity aggregate at 52, 4, and 1; and confirmation
  that RFC numbers 0013 upward are free.
- Conformance check: no file modified.
- Recovery: nothing to revert.
- **Go/no-go.** Stop if any derived count disagrees, if the note column does
  not discriminate, or if the reviewer prefers a fallback from
  `Alternatives considered`.

### `EP-M1` — coverage test, ADR, corrections, roadmap rewrite

Ship as a self-contained pull request. It is independently valuable and is the
fallback if nothing else proceeds.

- Outcome: `tests/rfc_stdlib_coverage_tests.rs` green with all eight groups
  unwritten and `COV-4` reporting 8. `ADR-021` records the convention, its
  narrow scope, and the amendment procedure. RFC 0006 gains the section 14
  coverage map and reservation rows for 0013 to 0020, has its number-allocation
  table backfilled and its false claim that no RFC has been merged removed, has
  the section 8.1 and section 8.6 defects corrected, has its seven "child
  issue" phrases updated, and has section 16 question 7 annotated with a
  pointer to roadmap task 7.1.1. Roadmap 6.1.1 is rewritten per `D8`.
- Acceptance evidence: `COV-1` through `COV-5` green; all seeded-fault
  transcripts recorded; every gate green.
- Conformance check: no disposition changed; no `src/` file touched; no new
  dependency.
- Recovery: revert; nothing depends on it.
- Compatibility decision: none. RFC 0006 is a pre-1.0 internal document.

### `EP-M2` — the template and one worked section 5

- Outcome: the literal skeleton above is committed into `ADR-021` or the
  developers' guide, and one complete worked section 5 exists for review — RFC
  0013's, being the smallest group.
- Acceptance evidence: a reviewer reads the worked section 5 and can state one
  thing it told them that RFC 0006 section 6 did not.
- Recovery: revert.

### `EP-M3` — RFC 0013, structured data interchange. Hard go/no-go

- Outcome: `docs/rfcs/0013-structured-data-interchange-helpers.md` exists,
  complete, listed in `docs/contents.md`, named in the coverage map, and
  referenced from roadmap step 6.2 and its three tasks.
- Acceptance evidence: `COV-1` shows five helpers owned by RFC 0013; `CONF-1`
  green for it; `COV-4` reports 7 unwritten; every gate green.
- Conformance check: no disposition changed; open questions 1 and 4 carried
  unresolved; the anti-vacuity rule satisfied for all eleven subsections.
- **Go/no-go.** This is the most important checkpoint in the plan. Everything
  before it is reversible and spends no number that matters. Everything after
  commits seven more numbers to a pattern nobody has yet seen finished. If
  section 5 for five simple pure filters does not tell a reviewer something
  they did not already know, the pattern does not work and the remaining seven
  must not be written. Fall back to `Alternatives considered`.
- Recovery: revert the commit; the coverage-map row returns to unwritten.

### `EP-M4` to `EP-M10` — one capability RFC each

Identical in shape, so stated once. For child `00NN` owning the groups in
`Table 1` for roadmap step `6.S`:

- Outcome: `docs/rfcs/00NN-<slug>.md` exists per the skeleton; the coverage map
  names it; `docs/contents.md` lists it; roadmap step `6.S` and each of its
  tasks cite it.
- Acceptance evidence: `COV-1` shows exactly that RFC's registry helpers owned
  by it; `CONF-1` green; `COV-4`'s unwritten count decremented; gates green.
- Conformance check: no disposition changed; `COV-2` proves mechanically that
  no forbidden name was introduced; open questions match the assignment.
- Recovery: revert the single commit. Because section 8 is untouched, the prior
  plateau is fully coherent.
- Compatibility decision: none.

Order: `EP-M4` RFC 0014, `EP-M5` RFC 0015, `EP-M6` RFC 0016, `EP-M7` RFC 0017,
`EP-M8` RFC 0018, `EP-M9` RFC 0019, `EP-M10` RFC 0020. RFC 0017 precedes RFC
0018 because `expandvars` and `abs` both depend on the dialect mechanism that
section 8.6 defines and RFC 0017 discharges. Ship `EP-M4` to `EP-M7` as one
pull request and `EP-M8` to `EP-M11` as another, so no reviewer faces the whole
set at once.

### `EP-M11` — reconcile and close

- Outcome: all 39 roadmap task-level section 8 citations additionally cite
  their child RFC; `COV-4` reports 0 unwritten; task 6.1.1 marked done; this
  plan `COMPLETE`.
- Acceptance evidence: `make check-fmt`, `make lint`, `make doc-coverage`,
  `make test`, `make markdownlint`, and `make nixie` green on the merge commit,
  captured under `/tmp`. `COV-5` green over every new link.
- Conformance check: reconcile every `Surprises & discoveries` entry. Both RFC
  0006 defect corrections present. No disposition changed. Re-enumerate remote
  heads a final time to confirm no allocated number collided.
- Recovery: not applicable.
- Remaining gaps: none. Implementing the helpers is steps 6.2 to 6.9.

## Plan of work

### Stage A — understand and propose

Read RFC 0006 sections 6, 7, 7.8, 8, 9, 10, 13.4, 14, and 16. Perform the
`EP-M0` derivations. Confirm numbers are free:

```bash
git ls-remote --heads origin | grep -oP 'refs/heads/\K.*' | sort
git ls-tree --name-only origin/main docs/rfcs/
```

Ends at the go/no-go.

### Stage B — red

Write the coverage test before any child RFC. Wire it per
`tests/integration_test_wiring_tests.rs`, following the plain module
declaration precedent in `tests/documentation_installation_tests.rs`, not the
path-attribute precedent in `tests/dependabot_config_tests.rs`, which sidesteps
the orphan check rather than satisfying it.

Split across `tests/rfc_stdlib_coverage_tests.rs` and
`tests/rfc_stdlib_coverage/` **from the start**, not if it grows. The
repository caps code files at 400 lines and both comparable precedents already
exceed it at 411 and 419. Because `D5` derives the inventory from RFC 0006
there is no large data table to hold, so the split is a parser module plus an
assertions module.

Run it and confirm it fails for the expected reason — the coverage map does not
exist. Add the map, confirm green. Then apply every seeded fault from the
`Verification plan` in turn, confirm the expected failure, revert, and capture
each transcript.

Lint constraints that shape the code: `unwrap_used` and `expect_used` are
denied outside test bodies, so every parser helper returns `anyhow::Result`;
`panic_in_result_fn` is denied; and `missing_docs_in_private_items` is denied,
covering struct fields and enum variants, not merely types.

### Stage C — implementation

`EP-M1` as its own pull request. Then `EP-M2`, then one commit per child. For
each child:

1. Create `docs/rfcs/00NN-<slug>.md` from the literal skeleton.
2. Write section 4 as a list of the group's helpers with one-line purposes and
   links into RFC 0006 section 8.N. Do not restate a contract.
3. Write section 5.1's registry, then 5.6, 5.7, 5.8, and 5.9. Apply the
   anti-vacuity rule to 5.2 through 5.5, 5.10, and 5.11.
4. Flip the child's row in the coverage map from unwritten to the new number.
5. Add the `docs/contents.md` entry, phrased like the RFC 0009 to 0011 entries.
6. Add a `See RFC 00NN` sub-bullet to roadmap step `6.S`, and the same to each
   of the step's tasks alongside the existing section 8 citation.
7. Re-enumerate remote heads, run `make fmt`, run the gates, commit.

### Stage D — reconcile

`EP-M11`. Sweep the remaining citations, confirm `COV-4` reports zero, run the
full gate set sequentially.

## Concrete steps

Run everything from the worktree root. Gate commands, sequential and tee'd:

```bash
make fmt
make check-fmt    2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
make lint         2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
make test         2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$(git branch --show-current).out
make nixie        2>&1 | tee /tmp/nixie-netsuke-$(git branch --show-current).out
```

Focused test during Stage B:

```bash
cargo nextest run --test rfc_stdlib_coverage_tests 2>&1 \
  | tee /tmp/covtest-netsuke-$(git branch --show-current).out
```

Expected red transcript before the coverage map exists:

```plaintext
FAIL [   0.011s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  RFC 0006 section 14 contains no coverage map table; expected 8 rows
```

Expected green transcript at `EP-M1`:

```plaintext
    PASS [   0.014s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
    PASS [   0.009s] netsuke::rfc_stdlib_coverage_tests no_forbidden_helper_is_registered
    PASS [   0.008s] netsuke::rfc_stdlib_coverage_tests totals_and_purity_aggregate_agree
    PASS [   0.007s] netsuke::rfc_stdlib_coverage_tests coverage_map_status_is_reported
      coverage map: 0 of 8 capability groups written; 8 remaining
    PASS [   0.010s] netsuke::rfc_stdlib_coverage_tests inter_document_links_resolve
```

Expected wrong-owner seeded-fault transcript, the control the first draft
lacked:

```plaintext
FAIL [   0.012s] netsuke::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  helper combine (filter): coverage map designates RFC 0014, registry found in
    RFC 0015 at docs/rfcs/0015-ordered-collection-algebra.md:73
```

Commit messages use `git commit -F`. The per-child template:

```plaintext
Add RFC 00NN for the <group> helpers (6.1.1)

Records the RFC 0006 section 6 contract obligations for the <n> helpers in
section 8.N, with a purity and manifest-query registry, and links the group
to roadmap step 6.S.

RFC 0006 section 8 is unchanged; this RFC is additive.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
```

Pull request titles carry the roadmap number in parentheses, as the repository
convention requires: for example `Add the RFC 0006 coverage contract (6.1.1)`.

Delegate gate runs to `scrutineer` and the `docs/contents.md` and roadmap
citation work to `scribe`. Do not delegate any section 5.

## Validation and acceptance

Acceptance is behavioural.

1. Open any child RFC's section 5.1 and read a table giving every helper in the
   group its purity class and manifest-query availability. Confirm no such
   table exists anywhere in the repository today — that is the artefact this
   task creates.
2. Read that child's sections 5.6 through 5.9 and find, for each, a
   group-specific consequence: a named bound from RFC 0006 table 3, a
   diagnostic code, a named error condition. Nowhere does a justification
   reduce to matching Ansible.
3. Run `make test` and observe `rfc_stdlib_coverage_tests` pass with six tests
   and a line reporting how many capability groups remain unwritten. Delete one
   row from RFC 0013's section 5.1 registry, re-run, and observe a failure
   naming that helper and reporting zero owners. Restore the row.
4. Move a registry row from one child to another, re-run, and observe a
   wrong-owner failure naming both the designated and the actual RFC.
5. Search the registries in `docs/rfcs/` for `shuffle`, `is_dir`, `is_file`,
   `quote`, and `fileglob`, and find none of them. Each remains discussed in
   RFC 0006 sections 9, 10, and 11, and each may be named as a non-goal in a
   child's section 3 — the check is scoped to registry tables, which is what
   `COV-2` parses.
6. Open `docs/roadmap.md` at step 6.5 and find both the existing section 8.4
   citation and a new RFC 0016 citation on each task.
7. Confirm `docs/contents.md` lists all eight new RFCs.

Red-Green-Refactor evidence:

- Red: the focused test fails because RFC 0006 section 14 has no coverage map.
- Green: it passes once the map lands, with all eight groups unwritten.
- Refactor: it still passes at `EP-M11` with zero groups unwritten, and the
  reported remaining count has gone 8, 7, and so on to 0 across the milestones.

No behaviour-driven scenarios apply: this changes no observable tool behaviour,
adds no command-line surface, and touches no persistence or network boundary.
`docs/users-guide.md` is therefore not updated; RFC status is not user-facing
behaviour. Recorded so the omission is a decision, not an oversight.
`ortho_config` is likewise not used: it governs layered configuration surfaces
and this change adds none. If a later milestone unexpectedly introduces one —
for instance by resolving RFC 0006 open question 5 to expose the section 6.8
bounds through `StdlibConfig` — that is a tolerance breach requiring escalation.

Quality criteria:

- Tests: `make test` green, including the six coverage tests.
- Verification: `COV-1` through `COV-5` and `CONF-1` discharged, with every
  seeded-fault transcript recorded. `CONF-1`'s semantic residual gap stated
  plainly and not overclaimed.
- Lint: `make lint`, `make markdownlint`, `make check-fmt`, and `make nixie`
  green. Note `make doc-coverage` does not measure integration tests; the
  governing gate is `missing_docs_in_private_items` under `make lint`.
- Performance and security: not applicable; no capability, dependency, or trust
  boundary changes.

## Idempotence and recovery

Every step is re-runnable; `make fmt` is idempotent. Each milestone is one
commit.

Rollback is genuinely simple here, and only because of `D3`. Since RFC 0006
section 8 is never modified, reverting any child-RFC commit removes a
conformance record and returns a coverage-map row to unwritten. No contract is
lost and no citation breaks. Reverts conflict only on the coverage map and
`docs/contents.md`, both single-line edits.

The one irreversible act is allocating an RFC number, and only on merge. `D2`
allocates lazily, so an abandoned split spends only the numbers it used. Before
merge, renumbering is a rename plus a link sweep; `COV-5` catches a stale link
that `make markdownlint` cannot.

Do not use bare `git stash`; the stash stack is shared across worktrees. Use a
temporary work-in-progress commit.

## Artefacts and notes

To be filled during implementation. Record at minimum: the `EP-M0` derivation
results with the heading-count reconciliation stated; every seeded-fault
transcript; the `git diff --stat` per child commit; and the final gate log
paths under `/tmp`.

## Interfaces and dependencies

Files created:

- `docs/rfcs/0013-structured-data-interchange-helpers.md`
- `docs/rfcs/0014-mapping-and-sequence-transforms.md`
- `docs/rfcs/0015-ordered-collection-algebra-and-truth-predicates.md`
- `docs/rfcs/0016-pattern-and-version-predicates.md`
- `docs/rfcs/0017-lexical-path-composition.md`
- `docs/rfcs/0018-host-state-predicates-and-environment-expansion.md`
- `docs/rfcs/0019-encoding-identity-and-formatting-helpers.md`
- `docs/rfcs/0020-date-and-time-conversion-helpers.md`
- `docs/adr-021-focused-child-rfcs-for-survey-rfcs.md`
- `tests/rfc_stdlib_coverage_tests.rs`
- `tests/rfc_stdlib_coverage/mod.rs` and its submodules

Files modified:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md`
- `docs/roadmap.md`
- `docs/contents.md`
- this ExecPlan

No other file changes. No `src/` change. No `Cargo.toml` change: `googletest`
0.14.3, `pretty_assertions` 1.4.1, and `regex` 1.12.2 are dev-dependencies, and
`anyhow` is a normal dependency, which integration tests link against.

The test's private shape, in `tests/rfc_stdlib_coverage/mod.rs`. Every field
and variant needs a documentation comment, since
`missing_docs_in_private_items` is denied.

```rust
/// Jinja namespace a helper occupies.
enum Namespace {
    /// Invoked as a filter on a piped value.
    Filter,
    /// Invoked as a test after `is`.
    Test,
    /// Invoked as a bare function call.
    Function,
}

/// Whether a registry row introduces a helper or adds an option to one.
enum Registration {
    /// One of the 57 new helpers.
    New,
    /// One of the 3 existing helpers gaining an option.
    OptionAdded,
}

/// A surveyed name's disposition, derived from RFC 0006 section 7.
enum Disposition {
    /// Accepted, with the owning section 8 subsection.
    Accept,
    /// Deferred by section 9.
    Defer,
    /// Rejected because Netsuke or MiniJinja already provides it.
    RejectExists,
    /// Rejected as a redundant alias or on principle.
    RejectForbidden,
}

/// One parsed table row, with its source location for failure messages.
struct Row {
    /// Registered helper name, backticks stripped.
    name: String,
    /// Namespace the helper occupies.
    namespace: Namespace,
    /// Repository-relative path of the file the row came from.
    file: String,
    /// One-indexed line number of the row.
    line: usize,
}
```

## Revision note

Revised 2026-09-08 after a six-lens design review. The review verified the
partition — all 57 helpers, none lost, duplicated, or misassigned — and
rejected almost everything around it.

The plan no longer migrates RFC 0006 section 8 into the children (`D3`): the
repository's convention is additive, migration would strand 39 roadmap
citations with no link gate to notice, two groups split across two milestones
would leave genuinely incoherent intermediate states, and the measured
contributor read path would have got worse rather than better. The Purpose
section was rewritten because its original premise, that a contributor must
read 2132 lines, was false; the roadmap already deep-links every task. The
coverage test is now anchored on tables and derived from RFC 0006's own section
7 rather than on headings and hardcoded constants (`D4`, `D5`): heading
anchoring cannot see `basename` or `dirname`, miscounts three compound
headings, and false-positives on the parent's own sections 10, 11, and 15,
while the handwritten forbidden list had already omitted sixteen names. A ninth
child RFC for roadmap step 6.1 was dropped as empty. Section 5 was cut from
eleven mandatory prose clauses to five substantive ones with an anti-vacuity
rule (`D6`), because seven of eight children own only pure helpers and the
original structure was a vacuity generator. Numbers are now allocated lazily
(`D2`) against a measured in-repository collision base rate of three. A hard
go/no-go was added after the first completed child, an
`Alternatives considered` section was added with an `EP-M1`-only fallback, and
five tolerances were added covering aggregate volume, effort, gate false
positives, number collision, and vacuity. Two further RFC 0006 defects were
recorded, and several factual errors corrected: issues #596 and #594 are
closed, the naive section 8 heading count is 58 rather than 57,
`make doc-coverage` does not measure integration tests, and `basename` and
`dirname` carry a reject disposition the derivation rules must not confuse with
the forbidden set.

Nothing is implemented; the plan awaits approval.
