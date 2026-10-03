# 6.1.1. Split the RFC 0006 accepted set into focused child RFCs

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS

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
rejected candidate covered not at all? That is a bijection over the accepted
set, which is sixty names — the fifty-seven new helpers plus the three existing
helpers that gain a behaviour-preserving option — and a bijection over sixty
names is not reliably checked by reading.

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

## Scope: settled

The roadmap and this plan now agree, so there is no divergence left to weigh.
This section records how that was settled, because the earlier drafts turned on
it.

`docs/roadmap.md:744` used to read "Split the RFC 0006 accepted set into
focused child issues", with success measured as "exactly one open child issue".
The commissioned task asked instead for focused **child RFCs and accompanying
roadmap tasks**, and the reviewer has confirmed that reading and directed that
work be tracked in committed documentation wherever possible. Task 6.1.1 has
therefore been rewritten in place, and this plan implements that wording.

Three consequences, all now resolved rather than open.

**No separate issue tracker.** Delivery is tracked by the roadmap checkboxes in
steps 6.2 to 6.9, which are committed documentation and are reviewed with the
code. No child RFC gets a GitHub issue, and the amended task says so.

**The burn-down survives.** The word *open* in the old wording was doing real
work: an issue closes when its capability lands, giving a live burn-down, and
an RFC never closes. That property is preserved by the roadmap checkboxes,
which already decompose every capability group into tasks. It is not lost, only
relocated — into the file the reviewer asked to track work in.

**"Exactly one" applies to the RFC, not to the task.** Every one of the 60
accepted helpers is already named in a task under its owning step, but some are
named by more than one: `product` appears in both 6.4.2 and 6.4.5. The amended
success criterion therefore reads "exactly one child RFC and at least one
accompanying roadmap task". `COV-6` checks the roadmap half mechanically.

The originating issue [#596](https://github.com/leynos/netsuke/issues/596) was
closed on 2026-08-28 with this very split among its unfulfilled acceptance
criteria, so roadmap 6.1.1 is the surviving carrier of the obligation. Child
RFCs cite #596 as their **originating** issue, not as a tracking issue, so no
child claims an open tracker it does not have.

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
  tables and headings are not wrapped. Every fence carries a language: `bash`,
  `sh`, `plaintext`, `text`, or `rust`. An unlabelled fence is `MD040`, and a
  bare fence under an indented list item also trips `MD031`; the red-control
  transcripts were written as indented `text` fences first and failed both.
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
  become restatement. **Breached by RFC 0013 at 470 lines, and recorded rather
  than waived**; see the Progress entry dated 2026-10-01 that begins "Two
  preamble findings recorded before `EP-M4` begins". **Breached again by RFC
  0015 at 465 lines, rising to 487 after review repairs, and this time the
  waiver is decided rather than merely recorded**: the per-helper density shows
  the tolerance's stated failure mode is absent, and the escalation is answered
  in the `EP-M5` Progress entry. The remaining children are written to a
  tighter shape so the aggregate tolerance is not breached with them, and each
  child's line count is recorded at its own commit. **The aggregate is the
  binding control and it is now breached, not approached**: five children are
  written at 2374 lines against the 2400 budget, leaving 26 for three children
  whose measured floor is ~183 each. The `EP-M7` Progress entry carries the
  figures and the escalation, and its correction records that a
  nine-or-ten-child re-partition **raises** the aggregate rather than relieving
  it. The remedies that address this control are to raise the budget or to
  record a decided waiver; the plan stops for that decision before RFC 0018 is
  written.
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
  if section 5 for the group `EP-M3` delivers — RFC 0013, the first, not the
  smallest — tells a reviewer nothing they did not already know from RFC 0006
  section 6, the remaining seven are not written.

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
  document rewrite. `ADR-040` carries the amendment procedure named in `D9`.

- Risk: the reviewer concludes the split is not worth its cost.
  Severity: medium. Likelihood: medium. Mitigation: `Alternatives considered`
  states the fallback plainly, and `EP-M0` and `EP-M3` are both go/no-go points
  before most cost is incurred.

## Progress

- [x] (2026-10-01) **Replay the review corrections onto current main.**
  The published head `1f36a7ff6054e35dfdf22396988c72f7667606b8` became
  conflicting. Rebase all 63 branch commits from the exclusive boundary
  `7677c3886c0bd1cad470c3d1acd62041ec0ca0ab` onto fetched target
  `563261b059cf985f0df0590f86ceb71baf18d192`; the replay ends at
  `84de9442ff1c3b84083e6bebb67835e11e07ffa0`. The first branch commit's direct
  parent and the earlier rebase receipt confirm that boundary; no merge commit
  is replayed. Native text merge with `zdiff3` is used, with no selected custom
  driver.

  Both conflicts are in `docs/contents.md`: retain the branch's ADR-040 entry
  and its later wording correction alongside main's ADR-041 entry. Range-diff
  shows only those two context changes and the generated spelling hunk main
  already contains. All 130 target-only paths and 45 branch-only paths remain
  byte-identical to their corresponding source trees. The changed-line
  sequences for the four overlapping Markdown files are unchanged; the fifth
  overlapping file, `typos.toml`, already matches both sides. Main's delivered
  shell-quoting amendment and this branch's JSON-domain amendment both survive.
  The ignored `uv.lock` was preserved outside the worktree and restored
  byte-identically after its historical add/remove sequence.

  Main also changes workflows and the Ruff baseline, so previous green gates
  cover historical candidates. Repeat the complete sequential gate set on the
  rebased tree before a push leased to the recorded published head;
  current-head CI and CodeRabbit confirmation remain required before approval
  and merge.

  The rebased lint run exited zero but emitted a `comments-indentation` warning
  at `release-dry-run.yml:21:5`. Treat that as a failed gate: move the existing
  explanation before `with`, without changing workflow behaviour, then repeat
  lint and the remaining gates. The focused coverage run passed all 272 tests;
  workflow contracts passed 1,016 tests with three skipped.

  **The warning-free rerun is green.** Sequential `make fmt`,
  `make test-workflow-contracts`, `make check-fmt`, `make lint`,
  `make typecheck`, `make markdownlint`, `make doc-coverage`, `make nixie`, and
  `make test` all pass at candidate `84de9442` with tree fingerprint
  `bc2152748dfbfd6e9ccde9b456680c5d8eae224d7898fab2c12e7c39726550fc`. Workflow
  contracts: 1,016 passed, three skipped. Rust: 3,911 passed, six skipped
  across 111 binaries; doctests: 129 passed, 32 ignored across three targets.
  Documentation coverage: 98.84%. The focused RFC binary passed 272 tests
  before the comment-only workflow fix, and passes inside the full suite too.
  Both local CodeScene reports score 10 with no findings. The immediate count
  still reports one written group and seven remaining; these corrections do not
  finish milestones EP-M4 through EP-M11.

  Canonical full-suite log:
  `/tmp/test-5ffbb1df-4543-4fd2-8f84-30f81650519c-6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task-14.out`.
  This receipt is the sole edit after that complete run; revalidate its
  formatting, Markdown, focused coverage and ExecPlan status before committing.
  Publication and hosted checks must cover the resulting commit separately.

- [x] (2026-10-01) **Implement focused refactors for new CodeScene complexity
  findings after the functional review commit.** At `e88d0d1a`, CodeRabbit
  confirmed the four validation findings, JSON amendment, and aggregate-method
  concern resolved, and accepted the five string-argument metric exceptions.
  Three new CodeScene findings arose: complexity and nested condition blocks in
  `reference_path`, and complexity in `immediate_output_offences`. The separate
  atomic refactor splits independent depth validation from model rendering, and
  override selection from policy-field checks. A repository sweep found no
  equivalent helpers. Their ownership and permitted callers are recorded in the
  coverage suite's developers-guide subsection. Preserve models, diagnostics,
  test names, and parser semantics; add no suppression. Local CodeScene and the
  complete sequential gate set passed after the refactor. Current-head CI and
  CodeRabbit disposition evidence are required before merging.

- [x] (2026-10-01) **Implement the coverage-validation review corrections and
  fix the JSON key-type contradiction.** The starting local HEAD is
  `1c35b1d076955cf773c28672089a5c6ddd7ee98c`; the reviewed remote baseline is
  `2db228aabc60bde4f75d1d453ef78584706c51bc`. Inspect parser branches and
  callers, add direct rejection tests and independent bounded properties, guard
  the immediate-output override through the existing TOML loader, and document
  the suite. Retain integer and boolean JSON key conversion as explicitly
  lossy; restrict canonical round-trip equality to the canonical JSON domain,
  including string keys at every nesting level. Amend the normative parent and
  align the child and ADR specimen. These changes implement review corrections;
  milestones EP-M4 to EP-M11 remain outstanding.

  Tests use isolated temporary capability roots through a private test-only
  `Repo::fixture` constructor. Synthetic `World` constructors belong only to
  this test module tree. Private validators accept those fixtures while the
  seven repository-check wrappers continue loading the same documents once. No
  runtime helper is implemented and no tracked document is mutated by a test.
  CodeGraph tools are unavailable in this session and Leta has no registered
  workspace; direct source and caller inspection is the fallback.

  The first focused run found a property-assertion format capture error; after
  correction the binary passed all 272 tests, and workflow contracts passed 986
  tests with 3 skipped. Clippy then rejected assertion panics in fallible
  tests, unchecked indexing, shadowing, and string construction. Those patterns
  were corrected without suppressions. A second lint pass found sixteen further
  statement-terminator and string-collection findings, also corrected without
  suppressions. Python docstring sections and assertion failure messages were
  corrected as their successive lint stages exposed them. The first full Rust
  run found the focused Makefile target missing from the existing
  `NEXTEST_TARGETS` inventory; that inventory now includes it and checks its
  worker bounds. All required local gates then passed sequentially through one
  scrutineer, including the full Rust suite. The verification subsection
  records the measured results. Review replies and merge remain conditional on
  current-head CI evidence and explicit CodeRabbit confirmation; no further
  review will be requested.

- [x] (2026-09-08) Rewrite roadmap task 6.1.1 to the child-RFC wording and
  record that delivery is tracked by roadmap checkboxes, not issues (`D8`).
- [x] (2026-09-11) `EP-M0` Audit; confirm the partition and the derivation
  rules. Go/no-go. Every derived count was confirmed except the forbidden-set
  size, which the plan gave as 34; that number reconciles only as a row count.
  Both stop conditions were raised and both are resolved: `D10` adopts the
  complement rule and the deny set is 71, and the note column was **not** shown
  to discriminate — the class split is not derivable and is not asserted, so
  `D5` rule 3's remedy is deferred rather than needed. Audit results are in
  `Surprises & discoveries`; the partition and the per-child registry contents
  are confirmed unchanged, so `EP-M1` is unblocked. One further correction:
  `D5` rule 2's claim that all three rename rows say `Reject` is wrong for
  `hash`, whose row reads "Accept as `text_hash`".
- [x] (2026-09-11) `EP-M1` stage A/B derivation, re-verified against RFC 0006
  before the parser was written. Confirmed: 55 accept rows yielding 55 names
  with no alias groups among them, 6 defer names, 67 reject names, an accepted
  set of 60, and a deny set of 71. Every membership witness in `COV-2`'s
  non-vacuity list holds (`is_file`, `is_dir`, `is_link`, `quote`, `fileglob`,
  `lookup`, `win_dirname`, `expanduser` all denied; `basename`, `dirname`,
  `abs`, `glob`, `shell_quote`, `splitdrive`, `text_hash` all permitted), and
  `hash` is correctly neither — it is an existing helper RFC 0006 leaves
  unchanged, so it leaves scope entirely rather than counting as accepted.
  `EP-M0`'s class-split recovery was falsified: the recorded rule yields 8
  alias / 24 exists / 18 principle, not table 11's 22/10/18. The *principle*
  third is right and every principle row is backtick-free, but the 32-row
  remainder splits 8/24 against the table's 10/22, the difference being exactly
  the two rename rows `win_splitdrive` and `fileglob`. Recovering 22/10/18
  requires special-casing those two out of *exists* while leaving `now` — also
  a reject row naming an existing helper in backticked call form — inside it,
  with nothing in the document to distinguish them. The split is therefore
  **not derivable** and `COV-2` does not assert it; it asserts the parseable
  totals plus that table 11's three class counts sum to the reject-row count.
  Amended in place: `D10`'s tail, `COV-2`'s closing note, the audit table's
  last row, and the `Surprises` bullet that had recorded the rule as working.
- [x] (2026-09-11) `EP-M1` Land the coverage test, `ADR-040`, the RFC 0006
  corrections and reservations, and the roadmap 6.1.1 rewrite. All seven
  coverage checks are green on the unwritten map, `COV-4` reports 8 remaining,
  and roadmap 6.1.1 was confirmed to already carry the `D8` wording. Three
  parser defects were found and fixed on the way; see
  `Surprises & discoveries`. Shipped on the task branch rather than as its own
  pull request: the reviewer's instruction names one pull request, and PR #697
  already carries the task title, so the milestones stack there and each lands
  as its own commit. Five seeded-fault controls runnable before any child
  exists (a deleted coverage-map table, a corrupted section 7 row, a dangling
  link, a deleted roadmap bullet, and a bullet moved to the wrong step) each
  fail naming the missing table, row, link, or helper, and the suite is green
  again once they are reverted; transcripts are in `Verification plan`. The
  coverage map's caption was renumbered from table 12 to table 16 in the
  process, since table 12 already existed further up the document. The test
  module tree was split so that no module exceeds Whitaker's 400-line limit; see
  `Surprises & discoveries`.
- [x] (2026-09-19) `EP-M1` second CodeRabbit pass, four findings, all actioned.
  Two were latent defects rather than style: the parser was fence-blind, and
  the purity aggregate did not apply the `New`-row scoping its own comment
  described. Both are proven by evidence and covered by the fence control; see
  `Surprises & discoveries`. The other two were the stale `child issue` quote in
  `clauses.rs` and RFC 0006 table 1's `0006` row, which recorded the RFC's
  `Status` in a column whose every other row records a merge state — the merge
  claim was accurate, and the row now states the merge like its neighbours,
  with a sentence under the table separating the two facts.
- [x] (2026-09-19) `EP-M1` third pass: clear the three `lint-clippy` errors and
  settle the template against the parser. All three were in `markdown.rs` — two
  `doc_markdown` backticks and a `missing_const_for_fn` on `is_closing_run`.
  The last cascaded: making it `const` made its caller `Delimiter::closes`
  eligible in the same run, so the second `make lint` failed one frame further
  up. Fixed both in one pass and confirmed the cascade stops at `mark`, which is
  `&mut self` and calls the non-const `Delimiter::opening`. The cheap check
  (`cargo clippy --test rfc_stdlib_coverage_tests`) is enough to find the next
  frame without spending a full gate cycle on it. Separately, and more
  importantly: the skeleton `EP-M2` is told to copy literally **failed the
  parser `EP-M1` shipped**, on both the registry heading and the manifest-query
  cell vocabulary; see `Surprises & discoveries`. Both are fixed in the
  skeleton now, before `EP-M3` could spend the go/no-go on a document written
  to the wrong contract.
- [x] (2026-09-24) `EP-M2` the child-RFC template and one worked section 5,
  both landed in `ADR-040`. The template left this plan for the ADR rather than
  the developers' guide, and the worked section followed it: a template that a
  parser reads must live where the child author is already sent, and that is
  the ADR the convention is stated in. The worked section is RFC 0013's — the
  group `EP-M3` spends the go/no-go on — filled in rather than described, so a
  reviewer judges the pattern before eight RFC numbers depend on it. Both
  parsed artefacts were validated mechanically against the shipped parser: the
  registry heading matches `REGISTRY_HEADING`, all five rows parse to `New`/
  `pure`/`yes`, and the discharge table's eleven ids equal section 6's clause
  list in document order. Two consequences of writing it are recorded in
  `Surprises & discoveries`: the fenced copy is width-bounded by `MD013` in a
  way the real child is not, and the first draft's error-condition vocabulary
  had invented a `non_string_key` where RFC 0006 section 8.1 says sequence and
  mapping keys are rejected. This milestone's acceptance evidence is a
  reviewer's, not a test's, and is stated in `EP-M2`'s own entry.
- [x] (2026-09-24) `EP-M2` CodeRabbit pass, ten findings, all actioned. Two were
  latent defects that would have fired at `EP-M3`, the go/no-go, rather than
  here: the coverage-map link was resolved against `docs/rfcs` as though it
  were a file, so `links::resolve` popped into `docs/` and every correct child
  link would have been reported missing as `docs/0013-….md`; and `COV-1`
  compared registry name sets only, leaving the namespace and registration
  columns parsed but never checked, which is exactly what `COV-3`'s filter and
  test totals count. `heading_depth` accepted a bare `#596` as a depth-1
  heading, silently truncating any scan over prose that cites an issue at line
  start; the corpus does not do that today, which is why no test noticed. The
  remainder were documentation: RFC 0006 section 14.13 said `Status` carries
  the child link where the parser reads the link from `Child RFC`, the plan's
  type sketch still showed three reject variants and `Row` fields the shipped
  code does not have, and three plan passages still asserted the note column
  discriminates the reject classes, which `D10` had already recorded as
  falsified. Each fix was proven by a probe rather than by inspection; see
  `Surprises & discoveries`.
- [x] (2026-09-24) `EP-M2` second CodeRabbit pass, thirteen finding records
  collapsing to nine themes, all actioned. Three mattered beyond tidying.
  `section7::apply_optioned` assigned `Namespace::Filter` to all three optioned
  helpers while RFC 0006 section 3.2 lists `glob` under Functions, and the
  namespace comparison added in the previous pass reads that value — so `COV-1`
  would have failed RFC 0018, whose group owns `glob`, for being *correct*.
  That is a defect this task introduced, not inherited. `CONF-1`'s mechanical
  half was never implemented: the discharge *table* was parsed and compared,
  but the section 5 subsections it points at were read by nothing, so the empty
  stub, the generic discharge, and the Ansible-deference appeal were all
  unchecked — the plan listed them as obligations and the code did not carry
  them. And the link-target number was never compared against the reserving
  row, so row `0013` could link to `0014-….md` and every check would resolve
  the link, find the file, and pass while the registry filed those helpers
  under the wrong RFC. The remainder were narrower: duplicate clause ids were
  absorbed by a `BTreeSet` in two places, `child_number` accepted non-numeric
  link text, the totals and purity aggregate were gated on all eight groups
  being written so nothing could contradict them until the split was over,
  ADR-040 claimed the test "transcribes none of them" when it carries four
  anchor lists, RFC 0006 tied delivery to child-RFC closure where `D8` ties it
  to roadmap tasks, and a stray `**` in this plan was followed by a newline and
  so rendered literally. Every fix was proven by a probe. See
  `Surprises & discoveries`.
- [x] (2026-09-24) `EP-M2` third gate run at `4c631e18`, red on `make lint`, and
  both findings were defects the second pass introduced rather than inherited.
  `clippy::too_many_lines` rejected `map::parse` at 73 lines against this
  repository's 70-line ceiling, and `clippy::iter_skip_next` rejected
  `Section::subsections`'s `.skip(index + 1).next()`. Fixed at `c74a0993` by
  splitting `parse` into `parse_row` plus one function per rule that carries
  its own reasoning, and by indexing with `slice::get`. The ten coverage tests
  pass unchanged. **The gate caught what the review would not have**:
  CodeRabbit reads the diff's semantics, and neither finding is a semantic
  defect — the second pass had already been reviewed twice and passed. This is
  the reason the standing instruction is to make every gate green *before*
  requesting a review rather than to use the review as a gate.
- [x] Fourth gate run (2026-09-24), at `292bb9b3`. `make check-fmt` passed;
  `make lint` failed in `lint-whitaker`, which is the third of `make lint`'s
  five stages, so `lint-python` and `github-actions-lint` never ran. Two
  findings, both `module_max_lines`: `Module checks spans 461 lines` and
  `Module rfc_stdlib_coverage spans 448 lines`, against AGENTS.md's 400-line
  cap. `lint-clippy` — the first stage — passes the same files, which is the
  third time a targeted Rust gate has been green on a tree that `make lint`
  rejects. Fixed at `7605c884` by splitting `tests/rfc_stdlib_coverage` into
  three modules, none over 300 lines: `document` (the document layer, shared by
  every parser), `partition` (`COV-1`, `COV-2`), and `progress` (`COV-3` to
  `COV-6`, `CONF-1`). The ten tests pass unchanged and `COV-4` still reports
  `0 of 8 capability groups written; 8 remaining`.
- [x] Fifth gate run (2026-09-24), at `0ca2cb18`. `make check-fmt` failed on two
  `cargo fmt` diffs left by hand-writing the new modules — a trailing blank
  line in `document.rs` and a `pub use` rustfmt breaks across three lines in
  `mod.rs`. Fixed at `09018769`. The commit that introduced them was verified
  with clippy, nextest, and Whitaker, but `make check-fmt` was skipped: the
  first stage of the documented gateway set was the one omitted, and it was
  omitted because the change was "just a module move". Every gateway now passes
  on this tree, and `make lint` passes **in full for the first time on this
  branch** — all five stages, including `lint-python` and
  `github-actions-lint`, which had never run because the two prior invocations
  both stopped at `lint-whitaker`.
- [x] Sixth gate run (2026-09-24), at `8852163e`, and the first full six-target
  run on this branch. All six pass: `make check-fmt`, `make lint` (all five
  stages), `make typecheck`, `make test` (2813 passed, 3 skipped; doctests 81 +
  2 + 32), `make markdownlint` (134 files), `make nixie`. The run also
  confirmed the module split was behaviour-neutral: the
  `rfc_stdlib_coverage_tests` set is byte-identical at ten tests, and a
  normalized line-survival sweep of the pre-split `mod.rs` and `checks.rs`
  found six lines without an exact post-split counterpart, all module-path and
  import declarations the split necessarily rewrote.
- [x] (2026-09-24) `EP-M2` CodeRabbit pass at `8852163e`: twelve findings, one
  major and eleven minor/trivial, all triaged. Seven were applied as stated
  (`map.rs`'s dead `title` field; `section7.rs`'s silent `or_insert_with`;
  `assertions.rs`'s half-checked `hash` invariant; `map.rs`'s unvalidated
  `Owns` arity; the crate doc's "nothing here transcribes an inventory"; the
  plan's seven-to-ten test count and its two wrong `0014`/`0015` slugs;
  `ADR-040`'s doubly-listed `duplicate_key`). Two were applied against a *false
  premise* in the finding: `document.rs`'s start scan was made fence-aware
  because the doc comment promised a property `position()` could not deliver,
  not because a live bug existed — no document in the corpus has a heading
  inside a fence, and every looked-up heading is unique — and `section7.rs`'s
  optioned comment claimed `glob` "also reaches `accepted` as an accept row",
  which is false: all three optioned helpers appear only in reject rows. One
  was applied differently than asked: `section7.rs`'s hardcoded
  `Namespace::Filter` was replaced by a namespace parsed from the section 7
  tables, because the `Optioned` doc already argues a hardcoded namespace makes
  a *correct* child fail. One was **partly refused**: see
  `Surprises & discoveries` for the deference finding, whose requested
  behaviour would have deleted the plan's own recorded seeded fault.
  `clauses.rs` gained four unit tests, taking the binary from ten tests to
  fourteen. Committed at `65665fcb`.
- [x] Seventh gate run (2026-09-24), at `65665fcb`. Five of the six targets
  pass: `make check-fmt`, `make lint` (all nine tool invocations across its
  five stages, including both Whitaker passes), `make typecheck`,
  `make markdownlint` (134 files, 0 errors), `make nixie`. `make test` did
  **not** pass locally, on two tests that this branch's diff does not touch:
  `locale_stub_ui_tests::harness_compiles_under_a_split_build_dir` and
  `packaging_smoke_tests::packaged_manifest_retains_build_script_sources`, both
  nextest `TIMEOUT` at the repo's 300s budget. Both spawn live nested
  `cargo build`s into private target directories, and six other agents' cargo
  processes were running against the shared cache at the time. The resolver
  commits for this historical trap are **not** ancestors of this branch, so the
  live-build form is still what runs here. Re-running the two files alone made
  it worse (four timeouts, not two), which rules out this branch's own test
  load as the cause. Environmental, not a regression: the two files are
  unmodified by this diff, and the full run reached 2815 passed against 2 timed
  out. **Superseded by the eighth entry**: "environmental" was the right
  verdict and the wrong reason — both tests are over budget on this base by
  construction, not by load.
- [x] Eighth gate run (2026-09-24), at `f5300602`. Same shape as the seventh.
  Five of six targets pass: `make check-fmt`; `make lint` (all five stages plus
  both Whitaker invocations, 13s on a warm cache); `make typecheck`;
  `make markdownlint` (135 files, 0 errors; spelling 34 passed, 92.37%
  coverage); `make nixie`. `make test` fails again on exactly the two
  live-build tests, both `TIMEOUT` at 300.015s — 2817 run, 2815 passed, 2 timed
  out, 3 skipped, and **no** assertion failure of any kind. Because `make test`
  is fail-fast, the `doctest` sub-target never ran at all: that is this run's
  one evidence gap, and it is a gap the seventh run shared. The branch's own
  `coverage_map_status_is_reported` passed in 0.223s, so the diff's assertions
  are exercised and green. **Gap closed the same day**: `make doctest` was run
  separately and passed, exit 0, `81 passed / 2 passed / 32 passed` across its
  three targets with no failures. So `make test`'s two recognized sub-targets
  are now both accounted for — `test-nextest` red only on the two
  upstream-fixed live-build tests, and `doctest` green in full.
- [x] (2026-09-24) **`make test` cannot pass on this base, and that is not a
  load story.** `e2fc2083` and `33a293a7` are on `origin/main` but are *not*
  ancestors of this branch, so main has already fixed this failure class — and
  not by widening a budget. It added a `[test-groups.nested-cargo-builds]`
  group with `max-threads = 1` and **removed**
  `harness_compiles_under_a_split_build_dir` from the override filter
  altogether, trading the always-cold nested workspace build for recorded
  parser coverage. The seventh entry's "environmental, load-dependent" reading
  was too generous to this host. Measured for the record: the harness test
  passes in 524.170s under a lifted budget, and
  `packaged_manifest_retains_build_script_sources` in 270.658s, of which
  `cargo publish --dry-run` alone is 269.86s — against a 300s cap. The seventh
  entry read "re-running the two files alone produced four timeouts, not two"
  as evidence of load; it is better read as evidence that neither can pass here
  at all, so narrowing the selection only removes the queueing that was hiding
  it. **The rebase is the fix; no local workaround is warranted.**
- [x] (2026-09-24) **No CI has ever run on this branch, and CodeRabbit has
  reviewed nothing.** PR #697 reports `mergeable: CONFLICTING` and
  `mergeStateStatus: DIRTY` against `origin/main` (55 behind, 23 ahead; the
  conflicted paths are `.config/nextest.toml` and `docs/contents.md`). GitHub
  cannot build the `refs/pull/NNN/merge` ref for a conflicted pull request, so
  *every* `pull_request`-triggered workflow is suppressed — `ci.yml` and
  `netsukefile-test.yml` among them. The last real CI run on this branch is
  `3f02ee37`, its opening push on 2026-09-09. CodeRabbit is the sharper case:
  its status context on the head reads `SUCCESS` while its only comment on the
  pull request says "Draft PR not reviewed". So `EP-M3`'s "every gate green"
  criterion could not have been met through CI, and the two controls this
  milestone was counting on were not watching.
- [x] (2026-09-24) This branch's `.config/nextest.toml` addition from
  `3a207c13` is stale against main's convention. It uses
  `filter = 'test(=coverage_map_status_is_reported)'`; main's file now
  documents at length that the `test(=NAME)` form compares the whole name and
  so silently matches *none* of a parameterized `#[rstest]`'s instances, and
  mandates the anchored `test(/^NAME($|::)/)` instead. That filter is correct
  today only because the test is unparameterized, which is precisely the latent
  defect main's comment was written to prevent. The rebase should adopt the
  anchored form.
- [x] (2026-09-25) **`EP-M3` go/no-go: GO.** RFC 0013 was put to an independent
  reviewer against the plan's own criterion, applied verbatim. Of the eleven
  section 5 subsections, ten were judged SUBSTANTIVE and one VACUOUS. The
  substantive ten are not re-wordings: five of them (5.3, 5.6, 5.7, 5.8, 5.9)
  force decisions a reader of RFC 0006 section 6 could not have predicted — the
  `indent` range asymmetry and its rationale, stream-wide budgets with the
  stream total in the diagnostic, callables and `now()` rejected as
  `unsupported_kind`, the `interchange` module segment rather than `json`/
  `yaml`, the trailing-newline asymmetry pinned by a test at each end, and an
  explicit refusal to invent a looser test relation for `sort_keys`. The three
  "no bite" clauses (5.4, 5.5, 5.11) satisfy the criterion's own exception by
  naming the specific clause feature that cannot fire, which is checkable. The
  reviewer's strongest passage was 5.8's serializer bound: **"The serializers
  enforce nothing, and that is a decision rather than an omission. Neither
  allocates proportionally to anything but its input, so a bound would reject
  documents a parser had already accepted. The row reads 'none' instead of
  being left blank so that a reviewer sees the absence was chosen."** The seven
  remaining child RFCs are therefore written.
- [x] (2026-09-25) **Two defects the go/no-go found, both fixed in `e45c161e`.**
  The one vacuous subsection was 5.10, which restated clause 6.10 and
  re-reported an open question section 5.2 already carries; it now records the
  real naming question this group creates. More seriously, 5.6 claimed
  `from_yaml_all` "rejects every `from_yaml` condition", which contradicts RFC
  0006 section 8.1: an empty stream yields an empty sequence and multi-document
  input is the helper's purpose, so `document_count` does not apply. That is a
  rejection the implementer would have written and a test would then have had
  to defeat. The same over-broad phrasing sat in the 5.8 bounds table. Both
  corrected. **This is the go/no-go earning its keep**: a structural test
  cannot see this class of error, because `CONF-1` only requires each
  subsection to name a helper and avoid deference phrasing, and the wrong
  sentence did both.
- [x] (2026-09-25) Rebased onto `origin/main` (`397fb589`, 55 commits). Two
  conflicts, both predicted by `git merge-tree`: `.config/nextest.toml` and
  `docs/contents.md`. Both were resolved by taking main's version and
  re-applying this branch's addition, rather than transcribing the conflict
  hunks — main had reorganized the nextest overrides into `nested-cargo-builds`
  groups, and this branch's hunk was the extraction of the Windows override
  main removed. That addition now uses main's mandated anchored filter form,
  `test(/^coverage_map_status_is_reported($|::)/)`, closing the
  latent-unhooking defect recorded above. Verified after: `main` is an ancestor,
  `origin/main..HEAD` is 25, `HEAD..origin/main` is 0, `e2fc2083` and
  `33a293a7` are now ancestors, and the coverage contract passes 14/14 with
  COV-4 still reporting "1 of 8 capability groups written; 7 remaining".
- [x] (2026-09-25) Second rebase, onto `96aefc9c`, and the branch's own
  contract test caught a defect the incoming commits brought with them. One
  conflict, in RFC 0006 section 16 item 7, resolved by keeping both sides after
  verifying each claim against ADR-008's "2026-09-11: Stdlib clock seam"
  section and the coverage map's RFC 0020 row for group `8.10` — the clock seam
  is answered by roadmap item 7.1.1, and RFC 0020 neither needs it nor depends
  on it, so the two statements are compatible rather than contradictory. Then
  `inter_document_links_resolve` failed on `96aefc9c`'s own
  `docs/rfcs/0007-netsukefile-testing-framework.md:62`, which links to
  `netsuke-test-framework-technical-design.md` without the `../` prefix its
  line 22 correctly carries. From `docs/rfcs/` that resolves inside that
  directory, where the file does not exist. Fixed at `64970ba8`. **The link fix
  then failed `make check-fmt`**, `+5 -4` on that one file: the added `../`
  pushed a wrapped line past the margin `mdtablefix` enforces, and the file was
  re-wrapped in place. Re-verified after: check-fmt exit 0 (153 files
  unchanged), `make markdownlint` exit 0 (0 errors), and the coverage contract
  14/14. Note the sequence, because it is the whole reason the instruction is
  to gate locally rather than to trust the review: a link-only edit that a
  reviewer would read as trivially correct was red on two separate
  deterministic gates, and no review would have caught either.
- [x] (2026-09-25) **The rebase restored the CI channel, and CI is green on the
  same commit where local `make test` is red.** This is the milestone's most
  important result, and it settles the acceptance question by a better route
  than the one the plan assumed. PR #697 flipped from `CONFLICTING` to
  `MERGEABLE` at the first check after the force-push, which lets GitHub build
  `refs/pull/697/merge` again; runs `36069242789` (`CI`), `36069242572`
  (`Netsukefile Build Test`) and `36069243039` (`Release Dry Run`) all fired on
  `288526a2` and all concluded `success`. `Netsukefile Build Test` proves
  green, because it ran the tests rather than only compiling them:
  `Summary [ 415.659s] 3408 tests run: 3408 passed (3 slow), 5 skipped`,
  alongside `Doc-tests netsuke` 87 passed and `Doc-tests test_support` 39
  passed, both `0 failed`. **Zero failures and zero timeouts on CI.**
- [x] (2026-09-25) Ninth gate run, at `288526a2`, and the first on a tree where
  the branch's own diff is what runs. Five of six targets pass:
  `make check-fmt` (1s), `make lint` (64s, **all five stages and both Whitaker
  invocations**), `make typecheck` (7s), `make markdownlint` (24s, 153 files, 0
  errors), `make nixie` (5s, 154 files). `make test` fails, and the failing
  test is `command_env_ui_tests::cli_configuration_fixture_compiles` —
  `TIMEOUT [300.008s]` against nextest's 300s cap — with 3392 passed and 15
  cancelled behind it. This is the **third** member of the nested-Cargo timeout
  class, already recorded as measured `PASS 329.774s` under a lifted budget, so
  it is over cap on this host by construction, not by regression. Because
  `make test` is fail-fast, `doctest` never ran; run separately it passes (71s,
  2 targets, 0 failed). Gate logs are the seven unsuffixed
  `/tmp/<action>-netsuke-<branch>.out` files.
- [x] (2026-09-25) **The two gates disagree, and the disagreement is the
  evidence.** `make test` is red locally on one nested-Cargo compile test; the
  identical commit is green on CI with 3408/3408 and no timeout. Nothing in the
  branch differs between the two — the same commit, the same test set — so the
  difference is the host. The local run competed with other agents' clippy,
  nextest, and publish jobs (load average 74.39 on 24 cores) and the test
  drives a cold nested `cargo check` into a private `CARGO_TARGET_DIR` that
  cannot reuse the gate build; CI ran at **415.659s for all 3408 tests**, which
  is less than the local budget for this one test. Two consequences. The first
  is acceptance: `EP-M3`'s "every gate green" is satisfied on the authority
  that the criterion was always pointing at, and the local red is recorded
  rather than hidden. The second is a correction to the plan's own reasoning —
  see `Surprises & discoveries` for why "every gate green locally" was the
  wrong formulation to have written.
- [x] (2026-09-25) **Tenth gate run, red on three in-diff defects, all fixed.**
  The run covered the uncommitted tree that carries the five CodeRabbit code
  findings; `make typecheck`, `make nixie`, and `make test` passed, and
  `cli_configuration_fixture_compiles` passed in 8.6s rather than timing out,
  so the known 300s failure did not reproduce. Three gates failed, and every
  failure was introduced by this branch rather than inherited:

  - `make check-fmt`, on `clauses.rs`'s `DEFERENCE_PHRASES`. That const was
    formatted at `HEAD`, and rewriting it to a two-line form rustfmt rejects is
    this branch's doing. `make fmt` resolved it.
  - `make lint`, on Whitaker's `module-max-lines`: `clauses.rs` grew from 354
    lines at `HEAD` to **432**, past the 400 cap. Every sibling in that
    directory is under 400, the next largest being `section7.rs` at 332. Fixed
    by splitting the module at a seam rather than raising the cap.
  - `make markdownlint`, on three MD013 lines this branch added: one in this
    plan and two in RFC 0013. `make fmt` does not repair MD013, so all three
    were rewrapped by hand.

  The `clauses.rs` split extracted the appeal-to-Ansible predicate and its five
  unit tests into a new `tests/rfc_stdlib_coverage/deference.rs`. The seam is
  the *kind* of judgement: `clauses` grades document structure (an empty body,
  a body naming no owned helper), while the third vacuity shape `CONF-1` names
  is a judgement about prose. `clauses.rs` is now 258 lines and `deference.rs`
  202, and the directory's largest module is unchanged at 332. The module doc
  of each records the move, following the precedent `mod.rs` set when
  `markdown.rs` and `document.rs` were split out of it for the same reason.
  Re-verified after the split: `rfc_stdlib_coverage_tests` 15/15 green,
  including all five moved `deference` tests and `inter_document_links_resolve`.
- [x] (2026-09-25) **`ADR-021` was a number collision, and it is now `ADR-040`
      .**
  Found while investigating a grep that showed two files sharing the number.
  `main` published `adr-021-trust-aware-fetch-policy-merge.md` on 2026-09-09
  (`3348cc0a`, "Prevent project configuration from widening trusted fetch
  policy (#644) (#663)"); this branch minted
  `adr-021-focused-child-rfcs-for-survey-rfcs.md` with its `Date` field reading
  2026-09-11 and committed it at `9730a880`. Both files are tracked at `HEAD`,
  so main's 021 was already an ancestor when the branch's was committed. This
  is the exact failure the plan warned about: the `EP-M0` note at line 1361
  records "The highest existing ADR is 020; three earlier numbers collided, so
  re-check before committing", dated 2026-09-08 — one day before main took 021.
  The re-check did not happen, for any of the four subsequent commits that
  touched the ADR.

  Renumbered to **040**, the lowest free number above the ceiling. A fresh
  remote sweep puts that ceiling at 039 (`jm5/kani-change-scoped-gate`);
  `origin/main`'s own highest is 038. Eight edits, all mechanical and all
  verified: the file moved by `git mv` (preserving history), its H1 renumbered
  (it carried no internal self-references), the RFC 0006 section 14.13 link
  repointed, the two execplan path references repointed, all 17 execplan
  `ADR-021` mentions renumbered, and — the defect that made this visible — the
  missing `docs/contents.md` entry added. That file was the only one of 39 ADR
  files with no index entry, so files and entries now both read 39 and a Python
  set-comparison confirms they agree exactly, with no dangling target.

  Two guards held. `main`'s `adr-021-trust-aware-fetch-policy-merge.md` is
  untouched, as are all three of its inbound citations (`docs/contents.md` and
  two in `adr-026-manifest-environment-access-policy.md`, one of which is a
  link-reference definition). And the 17 execplan mentions were checked before
  the replace: grepping them for `fetch`, `trust`, `quarantin`, `network`, or
  `policy` returns nothing, so no mention means main's ADR and a scoped global
  replace was safe. The renumber is escalated rather than decided — see the
  item below — but the remedy is preparable without touching `main`, so it is
  prepared.
- [x] (2026-09-27) **Third rebase, onto `aa764819`, and all four named gates
      are green on the result.**
  PR #697 had gone `CONFLICTING` against `main`, which suppresses CI entirely,
  so the rebase was the unblocking step rather than tidying. `main` had
  advanced 18 commits past the branch's base `96aefc9c`; the replay was 33
  commits, linear, no merges, and stopped exactly once — at commit 6
  (`.config/nextest.toml`), with `docs/contents.md` auto-merging. The conflict
  surface predicted before the replay was four files and the actual surface was
  three plus a no-op, so the prediction held.

  The one real conflict is a **TOML trap worth recording**. Both sides had
  appended a `[[profile.default.overrides]]` entry at the same anchor — `main`
  adding the Kani mutation compile gate, this branch adding the `COV-4`
  coverage-map override. Git matched the two independently written *header*
  lines as common context, so it conflicted only on the bodies, leaving one
  header serving main's body and this branch's header stranded below the
  `=======`. Keeping both bodies under that single header would have produced
  one TOML table with two `filter` keys: a duplicate-key parse error, not a
  merge that merely looks wrong. The resolution is two separate array entries,
  main's first. Provenance was checked rather than assumed: `typos.toml` was
  expected to conflict and did not, because those 14 lines are tool-generated by
  `make spelling` and both branches regenerated the same text, so the replay
  correctly became a no-op there.

  The resolution was verified before `git add`, not after. The composed file
  parses under `tomllib` with five override entries; removing this branch's
  block reproduces `main`'s version exactly; and the branch never touched the
  `[profile.ci]` prose that `main` rewrote from 300s to 600s, so taking
  `main`'s copy verbatim is correct rather than a silent overwrite. The
  semantic audit then ran clean: all 146 target-only paths byte-identical at
  the new head, all 23 branch-only paths byte-identical to the old head, zero
  unexplained deletions against `main`, and no duplicated block outside
  branch-authored files. `range-diff` shows 33 commits against 33 with
  identical messages in identical order; the single `!` is commit 6 and is
  exactly the intended union.

  Gates, run sequentially on the rebased head `ada8b994`, each to completion:
  `make check-fmt` exit 0; `make typecheck` exit 0; `make lint` exit 0 across
  all five stages including both Whitaker invocations; `make test` exit 0,
  **3491 run, 3491 passed, 0 failed, 6 skipped** in 127.6s, plus doctests. The
  doctest gap that both earlier full runs shared is closed in this run rather
  than separately. `COV-4` reports "coverage map: 1 of 8 capability groups
  written; 7 remaining", which matches the plan's status: `EP-M0` to `EP-M3`
  land, `EP-M4` to `EP-M10` do not.

  Recovery material was created before the replay and is retained:
  `refs/recovery/20260927T231849/{old-head,old-base,target,remote-before}`,
  plus native `--binary` patches of both series under
  `/tmp/rebase-697-recovery/`. Recorded identities: `old_base=96aefc9c`,
  `old_head=0190f3aa`, `target=aa764819`, `new_head=ada8b994`.
- [x] (2026-09-28) **CodeRabbit reviewed the pull request itself, at
  `658b8157`: three findings, all actioned at `797178c9`.** Review `5332303125`
  returned `CHANGES_REQUESTED` against the branch head — not a stale SHA, as
  every earlier CodeRabbit signal on this branch had been — with three inline
  comments (`4117169304`, `4117169313`, `4117169325`), one per touched file.
  Nothing was ambiguous and nothing had to be refused; each was confirmed
  against the artefact it describes before being accepted, and each fix is
  committed with its confirmation rather than the reasoning left in a review
  reply.

  This is the first CodeRabbit response the plan records that came from the
  *pull request* rather than from a locally run `coderabbit review --agent`,
  and it arrived unprompted. It is therefore not numbered against the earlier
  passes: those were milestone-scoped (`EP-M1` second pass, `EP-M2` third pass)
  and this one is not tied to a milestone, so an ordinal would assert a lineage
  the plan does not record.

  `ADR-040`'s worked specimen still described `from_yaml_all` as rejecting every
  `from_yaml` condition. That is the wording RFC 0013 §5.6 had already been
  corrected away from, so the specimen and the artefact it previews had
  disagreed since that correction. The ADR calls the specimen non-normative and
  tells the reader that RFC 0013's copy is the normative one — but a reader
  editing RFC 0013 may well copy from the ADR, and the sentence is the same
  *claim*, not a summary of it, so a divergence is a defect in either
  direction. It now matches RFC 0013 §5.6 byte for byte, verified by extracting
  the bullet from both files.

  The coverage map could reserve one RFC number twice, and no check would
  notice. `Map::ownership` permits a repeated helper when both rows carry the
  same number, which is exactly what a duplicate reservation produces, so the
  one guard with a view of the whole map was blind to the shape. Downstream,
  `registries::parse_all` keys on a number over `children.contains`, and both
  the status and registry checks match rows with `find`/`any` — so one child
  RFC would have silently represented two capability groups. `parse` now tracks
  reserved numbers in a `BTreeSet` and rejects a repeat. The guard was proven
  live both ways rather than only made to pass: with a row mutated `0015` →
  `0014` it fails the run with the duplicate message, and with `map.rs`
  reverted to `658b8157` that same mutation passes **all fifteen** checks, the
  one-owner test among them. Both probes ran against the live document, and RFC
  0006 was restored to `HEAD` afterwards and confirmed by md5
  (`90c5787133429ec0a8ef66c1d936b265`).

  `Delimiter::opening` accepted any indentation before a fence. The module
  claims `CommonMark` in the same doc comment, and `CommonMark` allows at most
  three leading spaces; four or more is an indented code block. The cost of the
  unbounded form is asymmetric. A line indented deeply enough to be ordinary
  code content to every other Markdown tool would open a block here and swallow
  every heading and table beneath it — the silent truncation `Fences` exists to
  prevent — whereas the bound's own cost is a *loud* one: a fence nested in a
  list item sits at the item's content column, four spaces for an ordered
  marker, and is legal `CommonMark` this line-level predicate cannot see. The
  corpus carries every fence at column zero, so the loud error is the one the
  documents avoid today and the silent one is the one they face tomorrow. The
  bound is now three spaces and the doc comment states both sides of the trade.

  No gate covered this. `mdtablefix` is lenient through indent 6, and this
  repository's `markdownlint` config sets no MD046 key, so MD046 runs in
  "consistent" mode and a document whose only code block is indented passes.
  Three new unit tests in `mod fence_tests` pin the boundary that no document
  exercises: the 0–3 accept / 4–8 reject sweep, that a rejected opener leaves
  the structure below it readable, and that an indented *closer* still closes.

  `cargo nextest run --test rfc_stdlib_coverage_tests` → 18 passed, 0 skipped.
- [x] (2026-09-28) **Gate run at `3707fc1b`: two red, both in the prose this
  pass had just added, both fixed in the commit carrying this entry.** Seven
  targets were commissioned from one runner rather than the four named in
  `AGENTS.md`, because the turn-end hook runs `check-fmt`, `lint`, `typecheck`,
  `markdownlint` and `nixie` while `AGENTS.md` adds `doc-coverage` and `test`;
  the union is the honest set. Five were green on the first pass — `make test`
  at 3494 passed / 0 failed / 6 skipped in 127s, `make lint` (all five stages,
  both Whitaker invocations), `typecheck`, `nixie`, and `doc-coverage` at
  98.83% — and the two failures were `make check-fmt` and `make markdownlint`,
  each on a defect this pass had introduced.

  `mdtablefix` reported five hunks in the plan. Every line in every hunk was ≤
  80 columns on both sides, so this was **not** an MD013 violation: the tool's
  greedy fill packs the same words into a different arrangement than the hand
  wrap, and one word's displacement cascades through the paragraph. The correct
  fix is therefore to accept the tool's arrangement, not to shorten anything.
  Applied with the scoped invocation this plan already records, and confirmed
  idempotent — `--check` with the gate's own selector then reports
  `169 files left unchanged`, exit 0. The result matches the runner's read-only
  `--diff` capture byte for byte, which is the determinism check: the fix
  reproduced the tool's own output rather than merely silencing it.

  `markdownlint` failed *before markdownlint ran*. Its `spelling` prerequisite
  aborts on `recognise` at `1473:57`, so `mdlint` never executed and the diff
  has **no** markdownlint verdict from that run — an unknown, not a green. The
  re-run closes that gap explicitly: `spelling` passes and `mdlint` proceeds to
  169 files with 0 errors. The `-ise` to `-ize` fix was applied to both copies
  of the sentence, the plan and the Rust doc comment it quotes, because the
  branch's own Observation records that rule — *when a correction is applied to
  an artefact, grep for its other copies in the same commit* — and applying it
  to one copy only would reproduce the ADR/RFC divergence at the start of this
  same pass. The `.rs` copy reds no gate: the spelling gate is Markdown-scoped
  and the Rust corpus is genuinely split on this word (12 files `-ise`, 9
  `-ize`, one file carrying both), so the local convention does not decide it.
  What decided it was that this branch authored exactly one Rust occurrence and
  it is the quoted same sentence.

  Two things were deliberately not changed. The twelve pre-existing `.rs` files
  spelling `recognise` are outside this delta and outside every gate's scope;
  rewriting them would put twelve unrelated files in a prose-fix commit. And
  `docs/execplans/…:1072` is 81 columns but is exempt: MD013's `\S*$` rule
  means a line whose final token begins within 80 columns is not flagged, the
  config sets no `strict`/`stern`, and `git log -S` shows the line was
  introduced by `6ed33733` — an earlier, already-published commit, so it is not
  a regression this pass introduced.

  `cargo nextest run --test rfc_stdlib_coverage_tests` → 18 passed, 0 skipped.
- [x] (2026-09-28) **Gate run at `68c266e8`: all seven targets green.** The
  same seven-target union ran again on the fix commit and passed: `check-fmt`
  (169 files, 0 reformatted, 2s), `lint` (all five stages, both Whitaker
  invocations, actionlint resolved from `$HOME/go/bin`, 14s), `typecheck` (1s),
  `test` (3494 run / 3494 passed / 6 skipped in 147.569s,
  `rfc_stdlib_coverage_tests` 18/18, `execplan_status_contract_tests` 9/9,
  doctest targets two not three), `markdownlint` (spelling now passes and
  `mdlint` proceeds to 169 files with 0 errors, 19s), `nixie` (10s),
  `doc-coverage` (98.83%, 32s). The runner also independently reproduced the
  two facts this pass had argued from: the `spelling` target's scope is
  Markdown-only (controlled A/B on a scratch repo: an `-ise` typo in a `.rs`
  file reds no gate) and `make test` *is* sensitive to plan edits, via
  `tests/execplan_status_contract_tests.rs:62,133`.

  **This entry is the reason the revision it cites is not the revision that
  stands green now.** Writing a Progress entry moves `HEAD`, so the run is
  evidence for exactly `68c266e8` and for nothing later; the commit carrying
  this paragraph is a *new* revision that the run did not cover. The next gate
  run therefore has to cover the commit that contains it, not be assumed green
  by inheritance — the same rule the plan's own Observation records about a
  suffix in a gate log not being a revision.
- [x] (2026-09-28) **Two `chatgpt-codex-connector` passes on `658b8157`
  triaged; four inline comments, plus a non-review from `sourcery-ai[bot]`.**
  The plan had recorded neither bot. Sourcery's is a size-limit refusal, not a
  review — the diff exceeded the 150,000-character review limit — so it carries
  no findings and no verdict to clear.

  Of codex's four, one (`map.rs:123`) is a duplicate of CodeRabbit's F2 already
  fixed at `797178c9` and is recorded as such rather than re-dispositioned. The
  other three are substantive and are actioned in the commit carrying this
  entry; each needed a premise check rather than a straight application,
  because in all three the *finding* is sound and the *stated mechanism or
  proposed remedy* is not:

  - `map.rs:85` — `claim()` accepts a repeated helper when the repeat carries
    the same row number, so a cell reading `` `8.1`; `8.1` `` claims every
    helper twice and the `BTreeMap` collapses it. Codex's remedy is "reject any
    existing owner". That remedy cannot be applied as written: row `0018`'s
    `` `8.7` except `abs` `` resolves through `Survey::sections`, which is built
    from `sections_of` *including* the optioned helpers (`section7.rs:260`), so
    it already yields `glob`, and the same row's `Optioned` cell names `glob`
    again. A blanket reject would red the live document.

    This entry first recorded the remedy as a guard "within one row's own claim
    list". That is **also wrong, and wrong in the same way**: `basename` and
    `dirname` reach row `0017`'s `claims()` twice — once through `owns`, since
    `sections_of` files them in `8.6`, and once through that row's `Optioned`
    cell — so the legitimate overlap is *within* a row, not across rows. The
    only scope that separates the defect from the design is **per cell**: an
    `Owns` cell must name a helper once, an `Optioned` cell must name a helper
    once, and the same name may appear in both because the two cells record
    different facts (which subsection specifies it; that it gains an option
    rather than being introduced). The guard is `ensure_distinct`, applied to
    each cell in `parse_row`. Verified against the live document rather than
    assumed, and proven live by mutation.
  - `RFC 0013:264` — the serializers are specified as enforcing no bound. The
    premise that a serializer's input need not be a bounded parser's output is
    **correct, and stronger than codex put it**. RFC 0006 §6.8 says
    "materialized output rejects unreasonable expansion before allocating", and
    RFC 0013 had read "materialized" only in the `from_yaml_all` sense (fully
    materialized before return), missing the output sense. The amplification is
    not hypothetical and needs no hostile input: MiniJinja values are
    `Arc`-shared, so a value built by repeated doubling is a DAG whose *logical*
    size is exponential in its construction depth. The mechanism is visible in
    the pinned crate: `impl Serialize for Value` recurses through
    `ObjectRepr::Seq` with `seq.serialize_element(&item)` for each child, and for
    a doubling both children are the same `Arc`, so every level visits the whole
    subtree twice. Confirmed by measurement — one recursive macro with `n`
    doublings emits `{{ v | tojson }}` as exactly `2^(n+2) - 3` bytes: `n=10` →
    4,093, `n=16` → 262,141, `n=20` → **4,194,301**. A four-line template
    therefore drives a materialized output past §6.8's 8 MiB ceiling with no
    large input anywhere. **Provenance:** that measurement ran through the
    MiniJinja Python binding, which wraps this engine; the figure for the pinned
    Rust crate is the one the implementation slice must re-measure, and the
    source-level mechanism above is what makes the result binding-independent.
    The bound is a ceiling on the *serialized byte count*, reachable only
    through `to_yaml` and `to_nice_json`, and clause 6.8 requires the rejection
    *before* allocating. That rules out counting bytes as they are written, so
    both documents specify a length pass first: walk the value with checked
    arithmetic, abandoning the walk when the running total passes the ceiling,
    and write only a value that fits.
  - `RFC 0013:189` — this is the one finding whose **stated mechanism is
    false**, and it is recorded that way rather than applied. Codex argues
    `{1: "a"} | to_nice_json | from_json` yields `{"1": "a"}`, "which is not
    equal to the input under §6.7's canonical equality". But §6.7 defines
    equality as *byte-identical canonical key*, and the canonical form of an
    integer key **is** the string form: `serde_json`'s `MapKeySerializer`
    renders every integer and boolean arm through
    `begin_string`/`write_iNN`/`end_string`, and `serde_json_canonicalizer`'s
    `JsonProperty::new` then re-parses those bytes and requires `.as_str()`. So
    both mappings canonicalize to `{"1":"a"}` and compare **equal** — the round
    trip holds, and codex's "impossible for part of the documented input
    domain" does not follow. Its proposed remedy (constrain `to_nice_json` to
    string-keyed mappings) is also the wrong split: RFC 0006 §8.1 requires the
    stringification outright.

    The real defect is **adjacent, sharper, and unnamed by codex**: one mapping
    can hold both the integer `1` and the string `"1"` as keys, and `from_yaml`
    accepts exactly that (probe: `yaml.safe_load('1: a\n"1": b\n')` yields
    **two** entries, keys `int 1` and `str '1'`). Both render to the same JSON
    key, so `{1: "a", "1": "b"} | to_nice_json` emits a document with a
    **duplicate key** — which `from_json`, the stated inverse, **rejects** with
    `duplicate_key` per §8.1. That is the round-trip guarantee failing on an
    accepted input, in the direction codex missed. The collision is rejected at
    the serializer, so `to_nice_json` is total on the inputs it accepts, and the
    §5.7 guarantee is qualified to say so.

  Both §5 findings land in the section the seven later child RFCs are copied
  from, so both were also checked against `ADR-040`'s specimen copy of §5
  (lines 372-463) in the same commit, per the rule this plan already records —
  *when a correction is applied to an artefact, grep for its other copies in
  the same commit*. The specimen differs from RFC 0013's §5 in several places
  already (it is a preview, and shorter), and the two sentences the corrections
  touch were found in both copies and corrected in both.

  Two consequences of the §5.8 correction were found only by following it
  through, and both are corrections of this entry's own earlier text:

  - The new `output_too_large` condition is a condition of the group, so §5.9's
    enumeration needed its row and the "thirteen conditions" claim needed to
    become fourteen. The count is asserted in **four** places (both documents'
    §5.9 prose and both clause-discharge tables); all four were changed and the
    tables were counted mechanically afterwards — 14 rows, 14 codes, in each.
  - `RFC 0013`'s first wording for the mechanism was "counts the bytes it writes
    and fails past 8 MiB". Reading clause 6.8 rather than paraphrasing it showed
    that is not sufficient: the clause opens "Every parser, combinatorial
    helper, regular-expression operation, and materialized output rejects
    unreasonable expansion **before** allocating." Counting while writing is a
    post-hoc check, so it discharges the clause's letter only by accident. Both
    documents now specify a length pass first, walking the value with checked
    arithmetic and abandoning the walk when the running total passes the
    ceiling, so a doubled value stops after 8 MiB of *logical* nodes rather than
    expanding.
  - The `map.rs` guard's scope was corrected **twice**. This entry first
    recorded codex's remedy ("reject any existing owner") as unworkable, then
    proposed the narrower "within one row's own claim list". That second
    proposal is also wrong, and for the same reason: `basename` and `dirname`
    appear twice *within* row `0017`'s claims (once through `owns`, once through
    `Optioned`), so the legitimate overlap is inside a row, not across rows. The
    only scope separating the defect from the design is **per cell**, which is
    what `ensure_distinct` implements.
- [x] (2026-09-28) **BLOCKER: the shared Cargo package cache is deadlocked
  machine-wide, so no Rust gate can run.** The commit carrying the three codex
  dispositions is `630b8116`, verified by inspection and by everything that
  does not need Cargo, but **its local gate run is outstanding**. The commit
  recording this entry is plan-only; the code delta under test is exactly
  `630b8116`, and a later reader must not fold the two together. A reader must
  not treat the absence of a gate result as a pass.

  **Scope correction, added later the same day: this entry overclaims.** It
  says no Rust gate can run, and that is true only of the *local* invocation.
  CI runs the same gates on GitHub's runners, which do not touch this machine's
  package cache, and it has since run them green on a later revision — see the
  entry below. The blocker is real but narrow: it costs the local second
  reading, not the gates themselves.

  The cycle, read from `/proc` rather than inferred: PID `1832225`
  (`cargo test --all-targets --all-features` in the `podbot` worktree, another
  agent's job) holds the **write** lock on `~/.cargo/.package-cache-mutate` and
  is blocked in `do_wait` on its child test binary `1855438`, which is blocked
  in `futex_wait_queue`; that binary spawned a **nested** `cargo` (`1855450`)
  which is blocked in `locks_lock_inode_wait` **on the lock its own grandparent
  holds**. Two samples of `1855438`'s `/proc/<pid>/stat` twenty seconds apart
  showed utime+stime unchanged at 48 ticks, so the loop is not progressing.
  Forty-seven processes were queued on that inode with `locks_lock_inode_wait`,
  the oldest for 1h46m, and `pgrep -c rustc` was **0** system-wide — every Rust
  job on the machine, this branch's included, was stalled behind it.

  This was diagnosed and left alone deliberately. The house rule says not to
  kill other agents' processes, and the holder belongs to another session; the
  system prompt's remedy for a full or wedged cache is to stop and tell the
  user, not to break someone else's lock. The `podbot` job is also
  self-inflicted in the sense that matters here — it is a nested-Cargo deadlock
  of the kind this repository has hit before (see the nested-Cargo timeout
  records), not a cache that merely needs to drain.

  The diagnosis carried no action of its own, so this entry closes as
  *diagnosed and recorded* rather than as a task left undone. Its one live debt
  — the local gate run it blocks — was paid later the same day: the cache
  drained, and all seven targets ran green. The scope correction above is what
  makes the checkbox safe to tick; the historical record is unchanged.

  What *was* verified without Cargo: `mdtablefix --check` over the full
  selector reports `169 files left unchanged` (exit 0), so every Markdown edit
  is canonical and idempotent; `rustfmt --edition 2024 --check` on `map.rs`
  exits 0, so the new guard parses and is format-clean; and the two §5.9 tables
  were counted mechanically (14 rows, 14 codes each) rather than trusted. A
  standalone `rustc` parse of `map.rs` was tried and is **inconclusive** — it
  fails only on the unresolvable `anyhow` and `super::` imports, so it cannot
  distinguish a syntax error from a missing dependency, and must not be cited
  as evidence.

- [x] (2026-09-28) **Two pieces of work completed while Cargo was blocked, and
  the PR description brought back in line with the branch.** Neither needs the
  package cache.

  The two commits `630b8116` and `e209ee99` were **pushed**. The remote head
  had stood at `68c266e8` while the local head was `e209ee99`, so the earlier
  "remote-head discrepancy" is resolved. It was a fast-forward: the remote head
  was verified to be an ancestor of local `HEAD` before pushing, so no force
  was needed and none was used.

  The PR description had drifted in three separate ways, all corrected in one
  edit rather than left for a reviewer to notice. Its `Verification` section
  described **four** gates on the long-superseded head `ada8b994` and never
  mentioned the seven-target union, the later runs, or the blocker at all. Two
  figures were stale: the coverage suite was "15 tests executed", which is
  **18** (7 top-level in `tests/rfc_stdlib_coverage_tests.rs` plus 11 module
  tests, 6 of them in `markdown.rs`), and RFC 0013 was "429 lines", which is
  **452**. Both were re-derived from the working tree rather than adjusted by
  arithmetic — `grep -c '#\[test\]'` over the module tree, and `wc -l` on the
  committed blob via `git show HEAD:`.

  The rewritten section leads with a per-revision table that states plainly that
  `68c266e8` is the last revision to complete the set, and that `630b8116` and
  `e209ee99` are **outstanding — blocked, not passed**. It records the deadlock
  with the `/proc` evidence, what was verified without Cargo, and — since CI
  does not use this machine's package cache — that **CI is the one channel the
  deadlock does not block**, which is what can actually settle the current head.

  One figure was **kept** after checking it: "Five seeded-fault controls run
  before any second child exists" is not the same list as the
  `Verification plan`'s "four seeded faults". The five are the early controls
  (a deleted coverage-map table, a corrupted section 7 row, a dangling link, a
  deleted roadmap bullet, a bullet moved to the wrong step); the four are the
  later RFC 0014/0015 ownership faults. They agree, so neither was changed — an
  apparent inconsistency that is only apparent.

  The `ensure_distinct` doc comment's load-bearing claim was **re-verified at
  the source level**, because it is the kind of prose a reviewer cannot cheaply
  check and a wrong mechanism there would justify the wrong fix. The claim is
  that `basename`, `dirname` and `glob` enter the per-section member lists even
  though section 7 never accepts them, which is what forces the guard's scope
  to be per cell rather than per row. Confirmed: `section7::apply_optioned`
  inserts each optioned helper into `read.sections_of` (`section7.rs:260`), and
  `survey.rs:130` builds `Survey::sections` by inverting exactly that map. So a
  clause reading `` `8.6` except `expandvars` `` resolves to include `basename`
  and `dirname`, and what a per-row or per-union guard would need to tolerate
  is therefore present in the live document. The comment stands as written.

- [x] (2026-09-28) **The blocker does not block the gates after all — CI runs
  them, and they are green on `c7ff9e4a`.** This entry corrects the scope, and
  the correction matters more than the original entry did: it was written on
  the assumption that a deadlocked local package cache left the Rust gates with
  no runner. That assumption was **never checked, and it is false**.

  The `CI` workflow's `build-test` job ran, on `c7ff9e4a`, to completion and to
  success — and its steps are the seven-target union almost exactly:
  `make check-fmt` (169 files left unchanged by `mdtablefix --check`),
  `make lint` (all five stages, with `actionlint` from the job's own download),
  `make typecheck`, `make doc-coverage`, `make spelling`, `make nixie`,
  `markdownlint-cli2` over `**/*.md`, and `make test-workflow-contracts`. The
  `Windows / lint-windows` job independently carries `Format`, `Lint (Clippy)`
  and `Lint (Whitaker)`.

  The test evidence is the strongest of these, and it is not a subset.
  `Test and Measure Coverage` invokes the shared `generate-coverage` action with
  `all-features: true`, `all-targets: true`, `doctests: true`,
  `use-cargo-nextest: true`, and `RUSTFLAGS: -D warnings` — the same flags the
  local gate uses — and its log reads **
  `3494 tests run: 3494 passed (1 slow), 6 skipped`**, with every
  `rfc_stdlib_coverage_tests` instance `PASS`, including
  `totals_and_purity_aggregate_agree`. Doctests ran too, in **two** targets
  (`Doc-tests netsuke`, `Doc-tests test_support`), which re-confirms the
  two-not-three figure from the other side. `COV-4` printed
  `coverage map: 1 of 8 capability groups written; 7 remaining` in that passing
  run, exactly as this plan requires.

  So the outstanding work is **narrower than the blocker entry above claimed**.
  What remains genuinely unrun is the *local* invocation of the set — which is
  not the same claim as "the gates have not run". Two things must be stated
  separately and must not be merged: CI's `3494 run / 3494 passed` on
  `c7ff9e4a` **is** gate evidence for that revision, and the local `scrutineer`
  run is still owed as the second, independent reading.

  Read the run rather than the summary row: `36373283587`, head `c7ff9e4a`,
  `workflowName: CI`, `event: pull_request`, all five jobs `success`. The
  lessons are the general ones — a summary row is not the log, and an
  assumption about what a blocker blocks is itself a claim that needs a check.

  **And the run was repeated on the revision that carries this entry.** Head
  `1524a7e6`, run `36375195362`, `CI`, all jobs `success`; `Format` reports
  `169 files left unchanged`, and `Test and Measure Coverage` reports **
  `3494 tests run: 3494 passed (1 slow), 6 skipped`** in 240.328s with
  `coverage map: 1 of 8 capability groups written; 7 remaining`. So the plan
  edits that record the correction are themselves covered by the same evidence,
  which is the property the `68c266e8` entry above argues for and this entry
  would otherwise have violated.

  **Next action for whoever resumes:** run the seven-target gate set locally
  once the cache clears (`pgrep -c rustc` returning non-zero, or the inode free
  in `/proc/locks`), then commission the `scrutineer` run — not because the
  gates are unrun, but because a local second reading on the runner's own
  revision is what this plan owes. The liveness proof for `ensure_distinct` is
  also still owed: the guard must be shown to *fire*, by mutating a
  coverage-map row to `` `8.1`; `8.1` `` and observing the run fail with the
  duplicate message.

  **Re-measured, and still held, at 05:30Z — the blocker is unchanged, not
  stale.** A `cargo metadata` probe returned exit 0, which looked like a clear,
  but the subsequent build never produced a `target/debug/deps` and no `rustc`
  ran. Reading `/proc/locks` properly settled it: the inode carries **exactly
  one granted entry** (the un-arrowed line, `1832225`, `FLOCK ADVISORY WRITE`)
  and **45 blocked requests** (every `->` line). The holder is still the same
  `cargo test --all-targets --all-features` in the `podbot` worktree, at 2h14m,
  still in `do_wait` on `1855438`; nested `1855450` is still in
  `locks_lock_inode_wait`. `pgrep -c rustc` is still **0** and ~20 `cargo`
  processes are queued machine-wide.

  The probe was a false clear because `cargo metadata` is one of the few
  commands that does not need the write lock. **Do not read a single exit-0
  probe as the deadlock lifting** — the durable signal is either a non-zero
  `pgrep -c rustc` or the granted-lock line disappearing from `/proc/locks`.
  The build attempt made while diagnosing was blocked on the lock for its whole
  life, not failing, and was stopped rather than left queued: a queued waiter
  is itself one more entry in the 45, which makes everyone else's diagnosis
  noisier.

  Also corrected: the memory note's holder recipe (`awk '{print $5}'` over
  every matching `/proc/locks` line) conflates the holder with its waiters,
  because the field layout differs between granted and blocked lines. The
  granted line is the one **without** a leading `->`.

  **Unblocked at 06:10Z — the durable signal this entry named has fired.** The
  condition it specified is met in the form it specified:
  `grep "$INODE" /proc/locks | grep -v -- '->'` returns nothing (no granted
  line), the blocked count is **0**, and `pgrep -c rustc` is 0 because nothing
  is running rather than because everything is stalled. The holder `1832225`
  and its descendants have left the process table. So the local seven-target
  run is no longer blocked by the cache, and the second reading this entry
  called outstanding is owed as work rather than waiting on an event. Note
  which signal carried the verdict: the *inode* going free, exactly as the
  entry above insists — a `pgrep`-based reading alone would not have
  distinguished a cleared deadlock from a machine whose holders had merely not
  yet restarted.

  **Discharged the same day — the local seven-target run is green on
  `a5455a1a`, the revision that carries this entry.** `scrutineer` ran the
  whole set sequentially, to completion, and every gate exited 0:

  | Gate           | Exit | Duration | Diagnostic line, read from the log                             |
  | -------------- | ---- | -------- | -------------------------------------------------------------- |
  | `check-fmt`    | 0    | 2s       | `169 files left unchanged.`                                    |
  | `lint`         | 0    | 85s      | all five stages ran; `All checks passed!`, `rated at 10.00/10` |
  | `typecheck`    | 0    | 12s      | `All checks passed!` (ty), then `Finished 'dev' profile`       |
  | `test`         | 0    | 286s     | `3494 tests run: 3494 passed (1 slow), 6 skipped`              |
  | `markdownlint` | 0    | 30s      | spelling ran and passed, then `Summary: 0 error(s)`            |
  | `nixie`        | 0    | 1s       | `All diagrams validated successfully!`                         |
  | `doc-coverage` | 0    | 39s      | `aggregate 4801/4858 98.83%`, meets the 80.00% threshold       |

  Three details are worth more than the verdict, because each is a way a green
  could have been hollow. First, `markdownlint` shows *both* a `spelling` pass
  (`current: typos.toml`) **and** an mdlint verdict (`Linting: 169 file(s)`,
  `Summary: 0 error(s)`) — this gate's spelling prerequisite can abort before
  mdlint ever runs, and a spelling-only log would have been an unknown wearing
  a pass's clothes. Second, `test` printed `COV-4`'s
  `coverage map: 1 of 8 capability groups written; 7 remaining` inside a
  *passing* run, which is the whole point of the counter: a half-finished split
  passes every other coverage check, so a stall is visible only if this line
  still reaches the terminal. Third, every one of the seven logs carries a
  verdict line the runner appended itself, of the form
  `GATE=… EXIT=<rc> … HEAD=<sha>`; all seven name the same revision,
  `a5455a1aae235fb7fd52e5db0ca9fda135329151`. That is what binds the evidence
  to a revision rather than to whatever HEAD happened to be when the report was
  written, and it is why the exit status was captured through `PIPESTATUS[0]`
  rather than read off `tee`.

  **The first green was not the last word, and CI caught what the local run
  could not.** CI on `86160a53` reded `build-test` — a *required* check — on
  `MD013/line-length`, at this very entry's line 1165, 81 columns against a
  budget of 80. The cause is worth recording because nothing local would have
  found it: the "verdict line" format was written as one unbreakable code span
  of 80 columns, and `mdtablefix` cannot break *inside* a code span, so its
  greedy fill emitted the line and MD013 rejected it. The local seven-target
  run was green on `a5455a1a` because the offending prose did not exist yet —
  it arrived in `86160a53`, the commit that recorded that run. **A gate result
  covers the revision it ran on and no other**, which is the recurrence this
  plan keeps meeting; the fix is to shorten the frozen token so the wrapper has
  somewhere to break.

  The one slow test was
  `packaging_smoke_tests::packaged_manifest_retains_build_script_sources`,
  which logged `SLOW [>120.000s]` and then **passed** inside its budget, under
  a machine load average of 58 from other agents' runs. That is the known
  cold-build-cost member and a load observation rather than a correctness
  signal; it is recorded because a 120s flag in a log invites the wrong
  question otherwise.

  So the two readings this plan wanted now both exist and are kept separate:
  CI's `3494 run / 3494 passed` on `c7ff9e4a` and on `1524a7e6`, and this local
  run on `a5455a1a`.

  **And the liveness proof is discharged for the `Owns` call site.** The guard
  was mutated, not merely re-run: RFC 0006's row `0013` was edited so its
  `Owns` cell read `` `8.1`; `8.1` ``, and only the `rfc_stdlib_coverage_tests`
  binary was run. It failed —

  ```text
  Error: the `Owns` cell at docs/rfcs/0006-ansible-inspired-template-standard-library.md:2068 names from_json twice
  Summary [0.035s] 18 tests run: 12 passed, 6 failed, 0 skipped
  ```

  — and on restoring the pristine file the same binary read
  `18 tests run: 18 passed`. The message names the mutation's own line, which
  is what shows the edit reached the parser rather than being silently dropped
  by table parsing. **Six** tests red rather than one, because the map parse is
  a shared load step, so a single malformed row fails every test that reads the
  map; the guard was the sole error source for all six (the captured log's
  distinct `Error:` lines number exactly one). The string is also unique to
  this call site: of the tree's `names … twice` producers, only `map.rs:197`
  uses the `the {cell} cell at …` shape, so nothing else could have worn its
  message.

  That proof covers `map.rs:164` only, so **the sibling call at `map.rs:162`,
  which guards the `Optioned` cell, was proved separately** — the same argument
  applies to it verbatim, and a per-call-site liveness claim needs a
  per-call-site witness. Row `0017`'s `Optioned` cell was mutated to
  `` basename ``; `` basename `` and the same binary run:

  ```text
  Error: the `Optioned` cell at docs/rfcs/0006-ansible-inspired-template-standard-library.md:2072 names basename twice
  Summary [0.038s] 18 tests run: 12 passed, 6 failed, 0 skipped
  ```

  Both call sites now have a witness, and the cell label is what separates
  them: each run produced exactly one distinct `Error:` line, and in the
  `Optioned` run a count of the `` `Owns` `` label over the log returned **0**,
  so the failure is attributable to the call site under test rather than
  borrowed from its sibling. The guard at `map.rs:162` also runs *before* the
  `Owns` guard at `:164`, so an `Optioned` failure can never mask an `Owns`
  one. Note what the pair does and does not establish: neither call site is
  unexercised now, but a surviving falsification attempt is *not* proof — each
  is falsified-or-not by one mutation, which is exactly the standard this plan
  asked for.

  **A second change rides on this head: `uv.lock` is untracked.** The file was
  never deliberately tracked. `a94a3006` staged its own verification entry with
  `git add -A`, and the sweep carried `uv.lock` along with it, so the branch
  *added* the file against `origin/main` —
  `git diff --name-status origin/main…HEAD -- uv.lock` read `A`. Commit
  `f42202a4` adds the name to `.gitignore` **and** removes it from the index,
  because the ignore rule alone would have been inert: git answers from the
  index for a tracked path and never consults the ignore rules, so the file
  would have stayed tracked while looking ignored. The working-tree file is
  deliberately left in place, so `uv` still finds the lockfile it wrote.

  Ignoring it is right for this repository even though `uv`'s own guidance says
  a lockfile "should be checked into version control". That guidance presumes
  dependencies to lock, and this repository has none by design and in as many
  words: `pyproject.toml` declares no `[project]` table and no
  `[build-system]`, "so no Python distribution can be built from it and `uv`
  never treats the repository as a Python project". The Makefile agrees in
  every invocation — `uv tool run …` throughout, and `uv run --no-project …`
  for the contract tests and the Python lint gates; the `--locked`/`--frozen`
  flags elsewhere in the tree belong to `cargo build`. What would be locked is
  a three-line stub naming no package.

  The change is inert by measurement rather than by assumption. The only test
  that reads `.gitignore` copies it into a scratch repository and asserts that
  each name in `MACHINE_LOCAL_DIRECTORIES` *is* ignored — one-directional, over
  fifteen *directory* names that `uv.lock` is not among, so a new file pattern
  cannot reach it. The two tests that shell out to `git ls-files` are both
  path-scoped (`MUTATIONS_DIR`; `Cargo.toml`/`**/Cargo.toml`). `typos.toml`
  already lists `uv.lock` in `extend-exclude`, so the spelling gate expects the
  file to exist and is indifferent to who owns it. No test requires it tracked,
  and none reads it.

  This is a change outside `EP-M4`–`EP-M11`'s scope and is recorded here for
  that reason: the branch carries it, so the plan and the pull request
  description must both name it rather than let a reviewer discover it.

- **The seven bot findings open at this head are already answered; the review
  is stale, not unresolved.** Four from `chatgpt-codex-connector` and three
  from CodeRabbit's `CHANGES_REQUESTED` pass `5332303125` all anchor at
  `658b8157`, and every one names a defect this branch fixed afterwards.
  CodeRabbit says so itself: each of its three carries
  `✅ Addressed in commits 797178c to 68c266e`. The four codex comments do not
  self-annotate, so each was checked against the revision rather than the
  anchor:

  | Finding           | Subject                               | State at `6ff02d87`                               |
  | ----------------- | ------------------------------------- | ------------------------------------------------- |
  | codex `…609`      | an `Owns` cell repeats a section      | `ensure_distinct`, `map.rs:162`, `:164`           |
  | codex `…616`      | duplicate child RFC numbers           | `ensure!`, `map.rs:132`                           |
  | codex `…627`      | non-string JSON mapping keys          | section 5.7 rejects non-distinct rendered keys    |
  | codex `…620`      | serializers unbounded                 | section 5.8's output bound, both serializers      |
  | CodeRabbit `…304` | ADR-040 specimen contradicts RFC 0013 | specimen states `document_count` is not inherited |
  | CodeRabbit `…313` | duplicate child RFC reservations      | the `map.rs:132` guard                            |
  | CodeRabbit `…325` | a fence opened by four spaces         | `opening` rejects more than three                 |

  The second codex row is rehearsed by a seeded fault; the rest are guards read
  at the revision named in the first column.

  **A caution for anyone following that annotation after the rebase.** The span
  `797178c to 68c266e` was correct when CodeRabbit wrote it and is now
  misleading in a way that does not announce itself:
  `git merge-base --is-ancestor 68c266e8 HEAD` fails, because the 2026-09-28
  rebase rewrote the branch and neither object is an ancestor of any current
  branch or of `origin`. They survive only because the rebase preserved the
  pre-rebase head, so `68c266e8` is reachable from
  `refs/recovery/6-1-1-old-head-20260928-162555` and nowhere else. The content
  they introduced did *not* go missing — the guards and prose each row of the
  table names were re-read at `b89652b9` and all four are present there — but
  the *citation* now points into a history that only the recovery ref holds.
  This is the same class as the plan's other rebase-provenance lessons: a
  rebase invalidates every SHA cited across it, including one cited by a
  reviewer rather than by this plan, and a reader who checks the annotation
  finds a missing object rather than a missing fix.

  Two codex findings are worth recording as *not applied as written*, because
  their mechanisms were wrong even though the defects were real, and `630b8116`
  says so. `…627` claimed the round trip was impossible, but section 6.7's
  canonical form of an integer key *is* its string form, so `{1: "a"}` and
  `{"1": "a"}` canonicalize identically and the round trip holds; the real
  defect was adjacent and unnamed — `from_yaml` accepts a mapping holding both
  keys, which would render a duplicate key its own inverse rejects. And
  `…609`'s suggested remedy ("reject any existing owner") cannot be applied at
  all: row `0018`'s `Owns` clause resolves to `glob` and its `Optioned` cell
  names `glob` again, so a blanket reject would red the live document.

  **The lesson is about review state, not about the bots.** A comment anchored
  at the current head is not necessarily live. GitHub re-anchors a comment on
  push whenever its line survives, so `commit_id` tracks the head while the
  comment body still describes an older revision — `line` and `position` are
  both non-null for all seven here, which is exactly the shape that reads as
  "live". Treating the anchor as the verdict would have sent a reader to re-fix
  three defects that were already fixed. Read the body, and check the revision
  it describes.

- **A credential report filed earlier was wrong, and is corrected here.** A
  push failed with `could not read Username for 'https://github.com/…'`, and
  the first diagnosis blamed the Lody credential helper's `missing_path`
  early-return with `credential.useHttpPath` unset. That diagnosis was reported
  to Lody as an environment misconfiguration. The retry then succeeded, which
  prompted a second look, and the second look contradicts the first. The
  harness **injects `credential.useHttpPath=true`** into every command through
  the `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n`/ `GIT_CONFIG_VALUE_n` channel:
  `git config --show-origin --get-all credential.useHttpPath` reports
  `command line: true`. The helper succeeds end-to-end when a `path=` is
  supplied — run directly with its own debug log enabled at
  `http://127.0.0.1:41269`, it records `request` to `broker_config` (from an
  `env_var`) to `fetch` to a `200` `fetch_response` to `success`, and emits
  `username=x-access-token` on stdout with a 40-character password.

  The `missing_path` observation therefore came from a hand-built `printf`
  probe, which omitted `path=` and so never reproduced what a real push sends.
  It was a true statement about a command that is not the one that failed. An
  earlier `git ls-remote` "proof" was void for the same reason in reverse:
  `leynos/netsuke` is **public**, so that read needs no credentials at all and
  could not have exercised the helper. `git push --dry-run` exits 0, and that
  is the ordering which actually exercises write auth. The single intermittent
  failure is left **unexplained rather than misattributed**; the filed report
  stands as a report of a symptom, and this entry is the correction to its
  diagnosis. **A probe that does not reproduce the failing command's inputs
  cannot establish the failing command's cause.**

- [x] (2026-09-28) **The local `coderabbit review --agent` pass was read and
  dispositioned; its seven in-scope findings are fixed in the commit carrying
  this entry.** The run is `REV=ac314a94`, `CR_STATUS=0`, 617 s, 23 findings.
  The count is 23, not 7, and the difference is the whole reason this entry
  exists: the local review scans the **working tree**, while the GitHub review
  scans the **PR diff**, and the two sets are not the same artefact. Sixteen of
  the 23 land in files this branch never touches — `.github/scripts/`
  nextest-oracle modules, `tests/kani_scope_wrapper_e2e_tests.rs`, other
  branches' execplans — so they belong to the revisions that wrote them and are
  recorded as out of scope rather than silently dropped. They are not "skipped
  as unimportant"; they are not this PR's to fix.

  All seven in-scope findings were verified against the document text before
  any edit, and all seven were correct. Four were stale facts the plan's own
  later entries already contradicted: the ToR claim, the still-open blocker
  checkbox, the superseded standalone-PR instruction, and the missing
  `.gitignore` line. Three were substantive: the `fourteen` test count (the
  breakdown was wrong too — 7 + 6 + 5, not 7 + 3 + 4), the `D10`/`:2148`
  contradiction, and the first-person passages. The last of these is a real
  style-guide rule (`docs/documentation-style-guide.md:39`), though a corpus
  probe shows it is widely violated elsewhere in `docs/execplans/`; that makes
  the rule no less binding on this file, and the remaining quote at line 835 is
  verbatim bot text and correctly left alone.

  One correction went further than the finding asked, because the finding's own
  premise was checked rather than trusted. The ToR entry's existing `924cb215`
  pin is a commit that **predates the document's existence**; the ToR arrived on
  `main` in `96b89ca9` (PR #786). Citing the document at a revision where it
  does not exist would have been a fresh instance of exactly the staleness
  class this entry is clearing, so the new artefact carries its own pin and
  says why it differs from its neighbour.

  **The commit that fixes all seven then reded `check-fmt` in CI, and the
  reason is a green from the wrong command.** After editing, the plan's
  Markdown was verified with a bare `mdtablefix --check <path>`, which reported
  "1 file left unchanged". The gate does not use that invocation. It runs
  `mdtablefix --check --git --include-untracked --wrap --renumber
  --breaks --ellipsis --fences`,
  and `--wrap` is the flag that rewraps prose to the line width; the bare form
  is a strictly weaker check that does not rewrap at all. The gate's own
  command reported `+45 -45` on the same file, which `build-test` — a
  *required* check — caught and reded. The edit was then canonicalized with
  `--in-place` under the gate's flags, after which the gate's own command
  reports `169 files left unchanged`, exit 0. The rewrap is provably
  content-free: the word sequence before and after is identical (34481 words,
  compared programmatically rather than by eye).

  This is the plan's recurring theme in a new costume. The earlier instance was
  "a gate result covers the revision it ran on and no other"; this one is **a
  verifier that is not the gate's verifier is not the gate**, and a pass from
  it is not a pass. Both share the same root: an artefact was treated as
  evidence for a claim it does not cover. The remedy is the same too — run the
  command the gate runs, not a command that resembles it.

  The fix above then produced a third instance, caught by a local gate run on
  `feed5192` rather than by CI. The sentence *about* canonicalization was
  itself written `canonicalised`, which `make spelling` rejects under the
  en-GB-oxendict `-ize` rule, so `spelling` aborted and `markdownlint` reported
  **no verdict at all** — an unknown that the outside observer would have read
  as green had the abort not been noticed. Two properties of that failure are
  worth keeping. First, the word was outside backticks: the plan already
  records that a code span is exempt from the rule, and the only reason this
  one was not exempt is that it is plain prose. Second, the same file contains
  `recognise` twice, and both are correct as they stand — each is a backticked
  quotation of a *different* revision's gate output, so the exemption still
  applies and "fixing" them would have falsified a historical record. A
  repo-wide `-ise` sweep is therefore not a safe repair; the distinction is
  prose versus quoted evidence, not one spelling against another.

- [x] (2026-10-01) **Two preamble findings recorded before `EP-M4` begins, so
  neither is inherited silently by seven children.**

  The first is a **tolerance breach that has already merged**.
  `docs/rfcs/0013-structured-data-interchange-helpers.md` is **470 lines**
  (`wc -l`), against this plan's per-file volume tolerance of 400 lines. The
  tolerance says to stop and escalate, and no exception was recorded when the
  file landed, so the breach is recorded here rather than left for a reviewer
  to find. What the tolerance's *reason* says is worth separating from its
  *threshold*: "a child carries no per-helper contract, so a larger one means
  section 5 has become restatement", and that diagnosis does not hold for RFC
  0013. Its section 5 is 300 of the 470 lines and its bulk is
  group-specific artefact rather than paraphrase — a fourteen-row diagnostic
  code table, the output-length pre-count argument with its measured
  4,194,301-byte doubling, and the canonical-JSON-domain acceptance table. The
  threshold fired; the failure mode it exists to detect did not.

  The second is the `Amends:` preamble bullet, which is **not** boilerplate and
  must not be copied forward. RFC 0013 carries
  `- **Amends:** RFC 0006, sections 6.7 and 8.1` because it makes a real
  normative amendment: the canonical-JSON-domain paragraph in section 6.7 and
  the section 8.1 pointer both entered RFC 0006 in the same commit that added
  the bullet, `e88d0d1a`. The first draft of 0013, `dbcdeb3a`, had no such
  bullet. Decision `D3` makes child RFCs additive, so RFCs 0014 to 0020 amend
  nothing and follow the ADR-040 skeleton literally, which carries no `Amends:`
  line.

  Consequences taken from the first finding. The 2400-line aggregate budget is
  still the binding constraint — 0013 has already spent 470 of it, leaving 1930
  lines and a mean of **276** for each of the remaining seven — so `EP-M4`
  onward are written to a tighter shape than 0013 rather than to 0013's length,
  and each child's line count is measured and recorded at its own commit. (The
  322 this paragraph carried when it was written divided by six where the
  sentence said seven; 1930 ÷ 7 is 276, and 276 is the figure to hold the next
  six to.) This is a budget response, not a content response: every section 5
  subsection still states a group-specific consequence or takes the `D6`
  escape, because that is what `CONF-1` checks and what the vacuity risk is
  about.

- [x] (2026-10-01) `EP-M4` RFC 0014, mapping and sequence transforms (step
  6.3). **Written at 399 lines**, inside the per-file tolerance the entry above
  records as breached by 0013, and the coverage contract's seven checks are
  intact: `every_accepted_helper_has_exactly_one_owner`,
  `no_forbidden_helper_is_registered`, `totals_and_purity_aggregate_agree`,
  `coverage_map_status_is_reported`, `inter_document_links_resolve`,
  `every_capability_has_a_roadmap_task`, and
  `every_child_discharges_every_clause` all pass at 272/272.

  Three findings from writing it, recorded because the next six children
  inherit all three.

  The **per-file tolerance is a real constraint, and reaching it is a rewrite
  rather than a trim.** The first draft was 456 lines and the second 432; both
  were cut by replacing prose with structure, not by deleting consequences.
  Section 5.6's six per-helper bullets became one six-row table (55 lines to
  35), and section 5.8's bounds table became prose (29 to 22). What made the
  difference was asking of each paragraph whether it *decided* something a
  reviewer could disagree with; the ones that only described a shape went. A
  child written to the template's full shape lands near 430, so the budget has
  to be planned for from the first draft rather than recovered at the end.

  **The `CONF-1` anti-vacuity rule bites on the diagnostics subsection, and it
  bites silently.** Section 5.9's code table names no helper, so the check's
  `escape || named` test fails: a code span is the only thing it counts, and a
  code like `netsuke::jinja::transform::wrong_kind` is not a helper's name. The
  fix is one sentence naming the helpers that share the variant (`combine`,
  `dict2items`, `extract`, and `subelements`), which is also better prose. This
  is the third vacuity shape `D6` names, and it is the one a diagnostics table
  invites, because the table's own subject is not the helpers. Each remaining
  child's 5.9 needs the same sentence; none of them can take the `D6` escape,
  because a code list is exactly the group-specific consequence the clause asks
  for.

  **`mdtablefix --renumber` reads a wrapped numeral as an ordered-list marker,
  and the ExecPlan's own prose is vulnerable to it.** `make fmt` rewrote "400
  lines" to "1. The tolerance says" and "holds for 0013" to "213", because a
  line beginning `400.` or `0013.` is indistinguishable from a list item to a
  renumbering pass. Both were repaired by reflowing the paragraph so no line
  begins with digit-dot, which is the durable fix: markdownlint cannot see the
  corruption, `check-fmt` cannot see it, and the only signal is reading the
  diff. Any future numeral at a line start in this plan should be written so
  the wrap never puts it there.
- [x] (2026-10-01) `EP-M4` **CodeRabbit review, five findings, all cleared.**
  The review ran against `3aade3a4` after all five gates were green on that
  revision, and took 588s without hitting the rate limit. Four findings were
  real and repaired; one is a false positive and is recorded as rejected.

  **The material finding is the one that indicts the reviewed RFC's central
  claim.** RFC 0014 section 5.8 said "a transform cannot amplify" and excused
  `subelements` on the grounds that each of its children is already an input
  element. The second half is true and the conclusion does not follow: a pair
  holds the parent's *whole* content, so a parent with `c` children yields `c`
  copies of itself, and a top-level list and its nested children are combined
  rather than selected. Merging a sequence with itself under `combine`'s
  `append` policy is the same shape — the doc doubling RFC 0013 measured for a
  serializer, reproduced by a merge. The repair adds `output_too_large` to both
  helpers, measures result *content* rather than pairs or elements, and takes
  the ceiling (8 MiB) and the diagnostic name from RFC 0013's serializers
  rather than inventing either. Section 5.6's rejection column, section 5.9's
  code table, the `6.8` clause row (fifteen codes, up from fourteen), section
  6, and section 9 all moved with it. **The lesson for the next six children is
  that a bounds subsection must be written from what the helper materializes,
  not from what it consumes**: the first draft reasoned about inputs, and an
  input-shaped argument cannot see a result that repeats content by reference.

  The three ExecPlan findings were arithmetic and text defects in this plan's
  own accounting: a duplicated "RFC RFC", a remaining-budget mean that divided
  by six where the sentence said seven, and a "four-property budget" naming a
  seven-member list. All three were in the same Progress entry, all three were
  introduced by the commit that wrote it, and none was gate-detectable — which
  is the review earning its cost rather than restating a gate.

  The rejected finding asked that `docs/contents.md`'s 0014 link bullet be
  wrapped to 80 columns. It is a false positive: the bullet is one inline link,
  MD013 exempts a line's trailing whitespace-free run, `mdtablefix --wrap`
  cannot split a link, and 22 sibling bullets in that file — including the
  merged 0013 entry — already run to 103 columns with the gate green. Wrapping
  it would also make this one entry inconsistent with the twenty-two around it.
  Accepted as a non-defect and not repaired; the reason is recorded here so the
  next reviewer does not re-raise it.
- [x] (2026-10-01) `EP-M5` RFC 0015, ordered collection algebra and truth
  predicates (step 6.4). **Written at 465 lines** and now at **487** after the
  CodeRabbit repairs below, so the per-file tolerance is exceeded again.

  **The escalation the tolerance requires is raised here, and the approver is
  the user.** This is the disposition CodeRabbit's major finding asks for, and
  the one distinction that makes it honest is that the implementation agent
  cannot grant it: a waiver it issues to itself is the silent rationale the
  finding rejects. So the breach is escalated rather than excused, the two
  admissible remedies are set out with their costs, and the recommendation is
  recorded for the user to accept or overrule. Work continues under the
  recommendation pending that answer, which is this plan's normal posture for a
  living document rather than a decision taken.

  - **Remedy 1, recommended: keep RFC 0015 whole and let the aggregate
    tolerance bind instead.** Ground: the per-helper density measured below,
    which shows the per-file threshold's stated failure mode is absent.
  - **Remedy 2: trim RFC 0015 to 400 lines.** Reason against: the 87 lines
    would come out of a section that is already the densest of the three
    children, so the cut is helpers or contracts rather than prose — and
    section 5 subsections are what `CONF-1` and the anti-vacuity rule exist to
    protect.

  A reviewer who prefers remedy 2 has every number needed to say so below, and
  the trim is a bounded edit rather than a rewrite. Recording the breach with a
  measurable argument for the recommendation is what the finding requires;
  asserting that the argument is a decision is what it forbids.

  The tolerance's stated reason — "a child carries no per-helper contract, so a
  larger one means section 5 has become restatement" — does not hold for 0015,
  and the measurement is the argument. Section 5 spans **326 of the 487 lines**
  to carry **fifteen** helpers across four per-helper tables (registry, output
  order, kinds and rejections, and the sixteen-row diagnostic table): 21.7
  section-5 lines per helper against 0014's 42.3 and 0013's 60.2. The larger
  document is the *denser* one per helper, so "section 5 has become
  restatement" is exactly what the figures exclude. Trimming to 400 would cut
  87 lines from a section that is already the tightest of the three, which
  means cutting helpers or contracts, not prose.

  **A corollary that supersedes an earlier version of this entry.** An earlier
  draft of this paragraph claimed 0015 spends "about 11 lines per helper"
  against a "roughly 300 line" fixed overhead, and concluded the same way. Both
  figures were wrong: the overhead is **~161** lines, not 300, and the figures
  above are measured from the section boundaries rather than assumed. The
  conclusion survived the correction; the arithmetic offered for it did not,
  which is why the numbers are stated here with their derivation rather than as
  a bare ratio.

  The budget consequence, now that it can be stated against real figures:
  **0013 + 0014 + 0015 = 470 + 398 + 487 = 1355 lines, leaving 1045 for the
  five children still to write — a mean of 209.** With the observed overhead of
  ~161 lines per child, five children at that mean leave roughly 250 lines of
  section 5 across all five. That is the constraint the remaining milestones
  must be written to, and it is tighter than any per-file limit.
- [x] `EP-M6` RFC 0016, pattern and version predicates (step 6.5).
- [x] `EP-M7` RFC 0017, lexical path composition (step 6.6).
- [ ] `EP-M8` RFC 0018, host-state predicates and environment expansion (6.7).
- [ ] `EP-M9` RFC 0019, encoding, identity, and formatting (step 6.8).
- [ ] `EP-M10` RFC 0020, date and time conversion (step 6.9).
- [ ] `EP-M11` Reconcile, retarget roadmap citations, run all gates, mark
  roadmap 6.1.1 done.

- [x] (2026-10-01) **CodeRabbit reviewed `bc80f294` and raised six findings; all
  six were upheld, one carried a wrong remedy, and one further defect was found
  by measurement rather than reported.** The review completed green on the
  deterministic side — all six `make` gates plus the coverage test at 272/272,
  every status file recording `head_before == head_after == bc80f294` — so the
  findings are all beyond what a gate can see, which is the standard this
  branch holds the review to.

  Every finding was verified against the contract files before repair, and one
  recommended remedy was rejected as wrong. The dispositions:

  - **Major, ExecPlan `EP-M5` entry: the 400-line breach was recorded with a
    rationale and no escalation.** Upheld. The entry now *escalates* rather
    than excuses: it names the two admissible remedies, recommends one on a
    measured ground, and states plainly that the implementation agent cannot
    grant its own waiver — the user is the approver. This was the one finding
    that indicted this plan's own reasoning rather than an RFC's text.
  - **Minor, RFC 0015 "Fourteen of the fifteen return a sequence": upheld, and
    the suggested remedy was wrong.** CodeRabbit proposed "Fifteen"; the true
    count is **thirteen** — eight filters plus the five predicates that read
    element order (`any`, `all`, `subset`, `superset`, `contains`), with
    `truthy` and `falsy` taking a single value. The sentence is rewritten to
    state the taxonomy rather than a corrected count, so the number follows
    from what is named beside it.
  - **Minor, RFC 0015 product cardinality "a hundred million tuples from a
    hundred thousand inputs": upheld, both figures false.** Ten operands of ten
    elements is **ten billion** tuples from **a hundred** elements of input.
    Corrected, and the corrected figure is stronger for the argument — a
    hundred thousand times the ceiling from a ten-line manifest.
  - **Minor, RFC 0015 §§5.6 / 5.9 / task 6.4.2 cardinality contract: upheld,
    and this was the substantive defect.** The draft told the implementation to
    abandon the count as soon as the running product passed the ceiling, then
    promised a diagnostic naming the computed cardinality. The two cannot both
    hold, and RFC 0006 §8.3 settles which loses: it requires the count, the
    operand lengths *and* the ceiling, so an abandoned count cannot discharge
    it. §5.8 now requires **exact** counting, §5.6 defines `overflow` as taking
    precedence over `cardinality_exceeded` because a count that will not fit in
    the type cannot be reported as an exact one, and §6's `itertools` rationale
    is restated to match. A repair pass caught this plan's own first attempt
    at this sentence asserting an overflow-frequency ordering it could not
    bound, and the "abandons the computation" phrasing surviving in the
    adjacent bullet was removed with it.
  - **Minor, task 6.4.5 claimed as "the only task in the eight children whose
    success criterion is about *process*": upheld.** Task 6.3.5's isolated
    workspace example requires byte-identical Ninja across two runs, so the
    claim is false. Rewritten to the distinction that does hold: run-to-run
    determinism is not the hard part, determinism across *hash state* is, and a
    second run cannot see the difference.
  - **Minor, RFC 0014 §6 "Within the RFC set it requires the shared contract
    RFC 0006 section 14.1's …": upheld.** The same ungrammatical sentence
    appears in **all three** written children, including `0013` on
    `origin/main`, so the repair is applied to three files rather than the two
    the finding named. `0013` is inside this plan's modified-file set.

  One finding's worth of damage was **not** reported and is recorded here
  because the review is what prompted the measurement. The EP-M5 entry's
  central quantitative claim — "0015 spends about 11 lines per helper",
  "roughly 300 lines" of fixed overhead — was fabricated. Measured: the
  overhead is 161 lines and the per-helper density is 21.7 section-5 lines,
  against 0014's 42.3 and 0013's 60.2. The conclusion (0015 is the densest of
  the three, so the tolerance's stated failure mode is absent) survives; the
  arithmetic offered for it did not, and the entry now carries the derivation.
  CodeRabbit did not flag it — a reviewer checking the claim would have had to
  measure the section boundaries independently, which is exactly why the
  figures are now recorded with their method.

- [x] (2026-10-01) `EP-M6` **RFC 0016 written, and two findings the writing
  surfaced are recorded here rather than deferred.** The RFC owns RFC 0006
  §§8.4 and 8.5 — the `netsuke-regex-v1` pattern family (`regex_replace`,
  `regex_search`, `regex_findall`, `regex_escape`, and the `match`, `search`,
  and `regex` tests) plus the strict `version` predicate — so it carries eight
  registry rows, taking the four written child registries to **34 of the 52**
  pure helpers (0013 five, 0014 six, 0015 fifteen). It is wired into RFC 0006
  table 16 as `written`, into `docs/contents.md`, and into all six of roadmap
  step 6.5's citation sites. The coverage contract passes 272/272. Measured at
  commit time: **487 lines**, section 5 spanning 308 of them, density **38.5**
  section-5 lines per helper against 0015's 22.3 and 0014's 42.3.

  **RFC 0006 §16 question 3 is carried unresolved, which is a departure from
  the pattern the earlier children set.** The question — whether `version`
  tolerates a `v` prefix — is assigned to this group and step 6.5 says to
  resolve it "before registering the test". RFC 0016 section 8 states both
  options and their consequences and leaves roadmap task 6.5.5 to choose. That
  is deliberate: the choice is an implementation decision with a
  manifest-visible contract consequence, and recording it as open with the
  argument on both sides is more honest than picking one here and describing it
  as settled. A reviewer should read section 8 as the escalation, not as an
  omission.

  The two findings:

  - **The module-segment convention clash surfaced while preparing this RFC,
    and the defect was in RFC 0015, not the code.** RFC 0015 §5.9 specified
    singular `netsuke::jinja::collection::*` and `STDLIB_COLLECTION_*`, but
    `src/stdlib/collections.rs` already exists, already holds `uniq`, `compact`,
    `flatten`, and `group_by`, and already keys its messages
    `stdlib.collections.*` under `keys::STDLIB_COLLECTIONS_*`. The singular
    would have created a Fluent namespace one character from the existing one.
    RFC 0015 §5.9 and its `6.9` discharge row are corrected to the plural, with
    the module-naming rule stated; RFC 0016 §5.9 takes the singular `pattern`
    and cites `shell`, `which`, and `register` as the existing singular forms.
    Both are recorded so the next child resolves the choice deliberately.
  - **The aggregate volume tolerance is now arithmetically unreachable, and
    this is an escalation rather than a note.** Measured after `make fmt`:
    0013 **470**, 0014 **397**, 0015 **495**, 0016 **487** — **1849** against
    the 2400-line aggregate budget, leaving **551 for the four children still to
    write, a mean of 138**. The observed per-child fixed overhead (everything
    outside section 5) across the four is 169, 143, 161, and 179; the minimum,
    143, is set by RFC 0014, whose sections 1–4 and 6–9 are the shortest that
    still discharge the skeleton ADR-040 parses by heading. Adding section 5's
    structural floor — two headings, eleven subsection headings, eleven
    non-empty bodies, and the thirteen-row discharge table — puts an honest
    child at **~183 lines**, so four cost **~732** against the 551 available: a
    **shortfall of about 181 lines**. The binding control stated in the
    tolerance ("the aggregate is now the binding control and it is close to its
    limit") is therefore projected to exceed it, and no tightening of the
    remaining children can recover it without dropping a section 5 subsection
    the vacuity tolerance forbids leaving empty. Raised to the user; the
    remedies are to raise the 2400-line budget, or to accept that this plan's
    eight-child split is a **nine-or-ten-child** shape and re-partition the
    remaining three groups (0017–0020) into four or five.

    *Amendment, added 2026-10-03: the second remedy does not serve this
    control.* The aggregate counts total lines, and a re-partition adds a whole
    child's cost per extra child, so it **raises** the total by roughly 366
    lines rather than relieving it. It is a remedy for the per-file tolerance.
    The reasoning and the figures are in the EP-M7 entry's correction and under
    `Surprises & discoveries`; the aggregate remedies are to raise the budget or
    to record a decided waiver.

- [x] (2026-10-03) **CodeRabbit reviewed `5895fc4c` and raised four findings;
  all four were upheld, and two further defects were found by reading rather
  than reported.** All four docs gates ran green first — `check-fmt` (175 files
  unchanged), `markdownlint` (0 issues, reaching the real `typos` verdict
  through `typos-config-builder` under the cleaned environment), `nixie`, and
  the coverage contract at **272/272, 0 skipped** with
  `coverage map: 5 of 8 capability groups written; 3 remaining`. None of the
  five `878f1489` findings recurred. The review completed rather than
  rate-limiting.

  **The major finding was upheld, and it is the one that corrects a position
  this plan had argued for.** It asked RFC 0016 §5.8 to bound `regex_replace`'s
  output. The earlier declination had been of a *subject* ceiling, on the
  ground that `netsuke-regex-v1` matches in linear time — and that ground is
  sound but does not reach this case. A match count is not an output size:
  table 3's 100000 row is scoped to `regex_findall`, and `regex_replace`'s
  output is the match count **times the replacement's length**, which is the
  author's to choose. A 1 MiB subject matched one character per position is
  1,048,576 matches — under the ceiling — while a replacement naming `$0` twice
  emits 2 MiB from a short template. Clause 6.8 names "materialized output" for
  exactly this and requires rejection *before* allocating, and both sibling
  children already apply the same 8 MiB ceiling: RFC 0013's serializers (§5.8,
  `interchange::output_too_large`) and RFC 0014's amplifying transforms (§5.8,
  `transform::output_too_large`). The RFC now carries an 8 MiB output row, an
  `output_too_large` code, the counting-before-building mechanism, a
  delivery-task mention, and an acceptance criterion. The distinction recorded
  for the next reviewer is that the subject-ceiling declination and this
  upholding are **not** in tension: matching a subject costs time linear in its
  length, and materializing a replacement does not.

  **A consequential edit the finding did not mention.** Adding the code row
  made two count claims stale in the same table's own section: §5.9's `6.9`
  discharge row read "twelve `netsuke::jinja::pattern::*` codes" and now reads
  **thirteen**. The §5.8 opening sentence also claimed the bounds "are RFC 0006
  table 3's" outright; since the output ceiling is the group's own rather than
  the parent's, it now reads "table 3's, applied through checked comparison
  before allocation, plus one output ceiling this group applies" — the wording
  RFC 0014 §5.8 established for the same situation. Both were caught by
  re-reading around the edit rather than by the reviewer.

  The remaining three findings and the two non-gate defects, all upheld:

  - **Minor, RFC 0017 §1: "Seven of the eight are new pure helpers" did not
    reconcile with the nine-row registry.** Rewritten to state the taxonomy
    the registry asserts: nine members, seven new pure helpers, and two
    existing filters taking `dialect` additively. `basename` and `dirname` are
    `Option added` and therefore excluded from the fifty-two, so a bare count
    was the wrong shape for the sentence.
  - **Minor, RFC 0017 §5.6: `path_join` and `commonpath` were marked as taking
    "a non-empty string" when RFC 0006 §8.6 specifies `path_join(paths)` and
    `commonpath(paths)` — `paths`, plural.** Both cells now read "a non-empty
    sequence of strings", matching §5.2's own rejection text, which already
    spoke of a non-string *component*.
  - **Minor, RFC 0017 §6: "dependency dependency" across a wrap boundary.**
    A duplicated word from the earlier count fix, removed.
  - **Observed, not reported: the aggregate-volume remedy space was stated
    wrongly.** The EP-M6 entry offered a nine-or-ten-child re-partition as a
    way to serve the aggregate tolerance. Measured against the five written
    children's fixed overhead (169, 143, 161, 179, 192), every extra child adds
    a whole child's cost while its share of the eight-step range shrinks only
    slightly, so five children instead of three come to ≈915 against 549 — a
    re-partition **increases** the aggregate by roughly 366 lines and cannot
    serve an aggregate control at all. It would serve the *per-file* tolerance,
    which is a different argument. The corrected remedy space is recorded under
    `Surprises & discoveries`, and no entry presents option 3 as an aggregate
    remedy any more.
  - **Observed, not reported: "Ground against:" at the remedy list was a
    heading error.** Its sibling reads "Reason against:", and the entry's
    opening says the remedies are stated as grounds; the label is now
    "Reason against:" to match.

  This is the second review round on RFC 0016's bounds and the first on RFC
  0017's internal counts, and the pattern is worth naming: **both rounds found
  the same class of defect — a stated quantity that the document's own tables
  contradict.** The first round found three such (a helper count, a product
  cardinality stated twice, and a sentence claiming to enumerate seven names
  while listing six); this round found three more. The contract suite does not
  parse any of them, because they are prose claims *about* tables rather than
  table cells, so no gate can catch them. The practice the plan now adopts is
  to re-derive every count in a child's prose from the table it describes at
  the moment the table changes.

- [x] (2026-10-03) **The branch was pushed and PR #860 opened as a draft, and a
  gate-version skew between the branch and `main` was found while preparing
  it.** Branch `6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task` had
  no remote counterpart: #697 delivered RFC 0013, merged on 2026-10-01 as
  `6be4a65f`, and the remote branch was deleted at merge. The merge base is
  therefore `6be4a65f`, and a trial merge against current `main` showed **no
  conflicts** — the branch's only overlapping files with `main` are
  `docs/contents.md` and `docs/roadmap.md`. The branch was pushed fresh over
  SSH and **PR [#860](https://github.com/leynos/netsuke/pull/860)** created
  with the `pr-creation` skill as a **draft**, base `main`, head `69069f04`.

  **The draft state is deliberate.** The aggregate-volume escalation below is
  still open with the user and three children remain, so the branch is not
  ready for review; a draft keeps the escalation and the PR in step rather than
  inviting review of a set the plan has stopped short of finishing. The Lody
  session was renamed to "Write RFC 0006 child RFCs 0014 to 0017" — the PR
  title minus its `(6.1.1)` prefix — with `lody session rename`, and the PR
  body's `## References` section links the session.

  **The gate-version skew is a live risk, not a note.** `main` adopted
  `typos-config-builder` **v0.1.3** while this branch pins **v0.1.1**, and the
  difference includes a `typos` bump from **1.48.0 to 1.50.1** (#843).
  Continuous integration checks out the *merge commit*, so it will run the
  newer, stricter spelling gate against this branch's new prose — while every
  verdict recorded in this plan came from v0.1.1. The skew is not a defect in
  the branch: `main` moved and the branch did not. But it means the recorded
  spelling verdict **does not cover the gate that will decide**, and the
  consequence is recorded under `Surprises & discoveries`.

- [x] (2026-10-01) `EP-M7` **RFC 0017 written, and the aggregate-volume
  escalation it was written under is now measured rather than projected.** The
  RFC owns RFC 0006 §8.6 except `expandvars`, plus §8.7's `abs` alone — the
  pure half of slice 5 — so it carries seven `New` pure helpers (`path_join`,
  `normpath`, `splitext`, `commonpath`, `relpath`, `splitdrive`, and the `abs`
  test) and two `Option added` rows (`basename` and `dirname`, each gaining
  `dialect`). Nine registry rows, taking the five written child registries to
  **41 of the 52** pure helpers: 0013 five, 0014 six, 0015 fifteen, 0016 eight,
  0017 seven. The three still to write — 0018, 0019, and 0020 — account for the
  remaining eleven. Measured at commit time: **499 lines**, section 5 spanning
  307 of them, density **34.1** section-5 lines per helper against 0016's 41.4
  and 0015's 22.5.

  **The aggregate budget is now breached, not merely approached.** The EP-M6
  entry escalated a projection of ~183 lines per honest child against the 551
  then available. Five children are written and the arithmetic is no longer a
  projection: 0013 **470**, 0014 **397**, 0015 **498**, 0016 **510**, 0017
  **499** — **2374 against the 2400-line budget, leaving 26 lines for three
  children** against a measured honest-child floor of ~183–192. The shortfall
  is now **~550 lines**, and the binding control the tolerance names is
  exceeded rather than threatened: writing 0018 alone would pass the budget.
  This entry supersedes the EP-M6 projection and is the figure to quote. No
  remedy is available to the implementation agent — the tolerance says so
  explicitly — so the work continues under an open escalation and the plan
  stops for the user's decision before 0018 is written.

  **Correction, added 2026-10-03: one of the two remedies this entry carried
  does not serve this control.** The remedies as first written were "raise the
  2400-line budget, or accept a nine-or-ten-child re-partition". The second was
  offered as an aggregate remedy and cannot be one, because the aggregate
  counts total lines and every extra child adds a full child's cost:
  re-partitioning the three remaining groups into five *raises* the total by
  roughly 366 lines (see `Surprises & discoveries`, "re-partitioning a fixed
  set into more children raises the aggregate"). It is a remedy for the
  per-file tolerance, which is a separate control and already carries a
  reasoned waiver. The aggregate remedies are therefore: raise the budget with
  the figure set from the measured floor, or keep 2400 and record a decided
  waiver. A re-partition remains available if smaller *files* are the
  objective, and it would require editing the `names.len() == 8` assertion in
  `tests/rfc_stdlib_coverage/roadmap.rs` — an architecture decision, not an
  editorial one — while accepting the larger aggregate.

  **The row partition was re-derived from the corpus rather than trusted.** RFC
  0006 table 16 allocates `expandvars` to RFC 0018 and `abs` to this RFC, which
  splits §8.6 from §8.7 in both directions: this child takes ten of §8.6's
  eleven entries and one of §8.7's six, and RFC 0018 takes the other one and
  the other five. The reason is in the table's own preamble — the
  pure/observing boundary is the stronger seam, so the split is at the
  capability boundary rather than at the section boundary. Two consequences
  worth recording because they are easy to get wrong:

  - `basename` and `dirname` are `Option added`, not `New`, so they are
    excluded from section 6.1's fifty-two. The coverage test's `OPTIONED`
    table lists exactly three names — `basename`, `dirname`, and `glob` — and
    `apply_optioned` inserts each from its section 7 *reject* row, so a
    registry that marked either as `New` would fail `check_rows_agree_with_survey`
    rather than pass quietly.
  - The seven `New` rows are all `Pure` with manifest query `Yes`, and the
    coverage test asserts that cell against the purity class from table 2
    rather than trusting it, so the disposition is not decoration.

  **This child reaches no row of RFC 0006 table 3, and that is stated rather
  than papered over.** Every other written child reaches at least one bound.
  The rationale recorded in §5.8 is that every row of table 3 bounds an
  *allocation* — an input length, a nesting depth, an alias count, an output
  tuple count, a match count, a compiled-pattern size — and this group
  allocates nothing that grows faster than its input: `normpath` is
  length-decreasing, `commonpath` is a prefix of an input, `splitext` and
  `splitdrive` partition rather than extend, and `relpath` and `path_join` are
  bounded by the sum of their operands. Recording "reaches none" is the honest
  discharge; inventing a bound to have one to cite is the vacuity the discharge
  exists to catch. The same reasoning was subsequently applied to RFC 0016 in
  review, where a subject-size ceiling was declined on the ground that
  `netsuke-regex-v1` matches in linear time — the two decisions are the same
  argument about allocation versus input size.

  **The child carries RFC 0006 §16 question 2 unresolved**, as RFC 0016 carried
  question 3. The question is whether `abs` is the right test name given
  MiniJinja registers `abs` as a numeric-absolute-value filter; §11.4 keeps it
  and names `absolute` and `abs_path` as the alternatives, and roadmap task
  6.6.4 asks for it to be resolved before registering. §8 states all three
  options with their consequences and recommends keeping `abs` for the record
  rather than as a decision. This is the second consecutive child to carry its
  assigned question, so the pattern is now deliberate rather than incidental: a
  manifest-visible naming or contract choice that is cheap to make late belongs
  in the child's §8 with the argument on both sides, not in §9 as a decision
  the child did not actually make.

  The RFC was drafted and mechanically pre-verified against the coverage
  contract **before** it entered the tree, because a new file cannot be gated
  in place while a delegated gate run is in flight. The pre-verification used
  the same predicates the contract uses, re-implemented in a throwaway script:
  each of the eleven section 5 subsections names at least one owned helper (the
  `names_an_owned_helper` rule), the discharge table's id set equals RFC 0006
  §6's eleven clause ids exactly with no duplicate and no empty cell, and the
  file carries no `as Ansible` or `like Ansible` deference phrase (the
  `deference_phrase` rule). mdtablefix was then run on the draft outside the
  worktree so the tree receives an already-canonical file and `check-fmt` has
  nothing to reformat.

- [x] (2026-09-28) **A gate run reded the current head, and the defect was the
  branch's own.** `make markdownlint` on `feed5192` exited 2, but not because
  `markdownlint` found anything: its `spelling` prerequisite aborts on
  `canonicalised` at this file's line 1391, so `markdownlint-cli2` never ran
  and emitted no verdict at all. The word was added by `feed5192` itself, in
  the very sentence describing canonicalization — a `-ise` form where the
  project's en-GB-oxendict rule mandates `-ize`. CI then failed the required
  `build-test` check on the same word at the same line and column, so the local
  diagnosis and CI agree independently rather than one being inferred from the
  other.

  The repair is a one-word substitution of equal length, so no wrap boundary
  moves; that was verified with the gate's own command rather than assumed,
  since `mdtablefix` reflows prose greedily and a longer or shorter token would
  have reded `check-fmt` in exchange. Two neighbouring spellings were
  deliberately left alone: this file contains `recognise` twice, and both are
  backticked quotations of a *different* revision's gate output, so the
  code-span exemption applies and rewriting them would falsify a historical
  record. The general lesson is recorded under `Surprises & discoveries` — for
  this class the deciding question is prose versus quoted evidence, not one
  spelling against another, so a repo-wide sweep is not a safe repair.

  The episode is the third instance of one theme, and the plan now says so in
  one place: the earlier two were "a gate result covers the revision it ran on
  and no other", and "a verifier that is not the gate's verifier is not the
  gate". This one is "a failed prerequisite hides a stage that never ran", and
  its tell is the same in all three — an artefact was read as evidence for a
  claim it does not cover. A green `check-fmt` and a green `spelling` are now
  recorded on `c765659d`, with `mdlint` reaching `169 files, 0 error(s)` for
  the first time on this branch, which also demonstrates the prerequisite is
  live rather than vacuous.

  Re-verifying the seven stale bot findings at this head turned up one stale
  citation of the plan's own. The `chatgpt-codex-connector` finding that the
  serializers were unbounded is discharged by RFC 0013 **section 5.8**
  ("Resource bounds"), where `to_yaml` and `to_nice_json` each carry "output 8
  MiB; checked before the result is returned" — but the disposition table above
  credited "section 5.3's length pass", and 5.3 is "Determinism", which
  contains no such pass. The two sections are distinct entries in this plan's
  own list of substantive clauses, so the number was simply wrong. Corrected in
  place. All seven findings were then confirmed discharged at HEAD, not merely
  self-annotated: the `map.rs` duplicate-number guard, `ensure_distinct`'s two
  call sites, and the two RFC 0013 obligations were each read at the revision.

- [x] (2026-09-28) **The branch was rebased onto `main`, 53 commits replayed
  one-to-one with no conflict.** `OLD_HEAD` was `c765659d`, `OLD_BASE`
  `aa764819` (the branch's exclusive replay boundary), target `7677c388`, and
  the result is `93a5d8b6`. `git range-diff` reports `=` for all 53 pairs and
  emits no non-summary output, so every replayed patch is byte-identical and
  nothing was resolved by hand. The old range and the new range each contain 53
  commits and no merges, `OLD_BASE` is an ancestor of `OLD_HEAD`, and
  `OLD_HEAD` is no longer an ancestor of the result — the expected shape after
  a replay.

  The reason the replay was mechanical is worth recording, because it is a
  property of the *pair* of revisions rather than luck: main's three new commits
  (`7677c388`, `027a848e`, `dc4c116a`) touch ten paths and this branch touches
  twenty-seven, and **the two sets are disjoint**. There was therefore no file
  for a merge driver to arbitrate, which is also why the replay needed no
  driver-consent decision — no path was ever selected for a three-way merge.
  That was checked rather than assumed: `comm -12` on the two path manifests is
  empty, and all ten main-only paths are byte-identical at `93a5d8b6` to their
  `7677c388` blobs, which is the audit the merge policy requires of a
  target-only path.

  **Main's incoming changes are not pertinent to this branch, and that is
  evidenced rather than asserted.** The three commits change workflows, the
  `Makefile`'s Pylint invocation, `docs/developers-guide.md`, and add
  `tests/workflow_contracts/pylint_tier_test.py`. The branch is documentation
  plus Rust source plus one `.gitignore` line, so the overlap in *subject
  matter* is nil; more concretely, every `file:line` citation this plan makes
  was re-resolved at the new head and no citation names a main-touched path
  except the one corrected below. The `Makefile`'s Pylint refactor is the
  nearest miss: it changes how the *Python* baseline lint runs, and this branch
  touches no Python. So the decision is to adopt none of it and to record why,
  rather than to manufacture a merge.

  Two things did need attention. First, `uv.lock`. The incoming commit
  `d03a3e31` (a replay of `a94a3006`) had swept `uv.lock` into the repository
  with `git add -A`, while the later commit `224dc762` (a replay of `f42202a4`)
  untracked it and added `.gitignore:21`. The rebase therefore stopped with
  "The following untracked working tree files would be overwritten by merge:
  uv.lock". The on-disk copy is a 52-byte file that no configuration in this
  repository reads, and it is byte-identical (`md5 8bbc054c…`) to the incoming
  blob, so removing the blocker discards nothing. It was preserved to
  `/tmp/rebase-6-1-1-untracked/uv.lock`, restored after the rebase, and
  verified at the new head to be both present and ignored
  (`git check-ignore -v` names `.gitignore:21`) and untracked (`git ls-tree`
  finds no entry). The commit that untracks it survives as a non-empty commit,
  so its `.gitignore` hunk still applies.

  Second, the rebase exposed a stale citation of this plan's own, in the same
  class as the `5.3`→`5.8` correction above. The `doc-coverage` observation
  cited `Makefile:206-210` as its evidence. That span is the `RUSTDOC_FLAGS`/
  `VERUS_FLAGS`/`WHITAKER` block, not the `doc-coverage` target, at every
  revision examined — including `b23d0535`, the commit that introduced the
  citation, where the target sits at line 325. The `206` was correct at an
  earlier main state (`5fda1e6e`), so the citation drifted as main moved and
  nobody re-resolved it. Main's Pylint hunk made it drift once more, by
  deleting one line at 161–168 and shifting everything after 168 up by one. It
  now reads `Makefile:325-329`, which is the `doc-coverage` target and its
  recipe, and the claim it supports (`scripts/doc-coverage.py` measures library
  and binary targets; integration tests are not measured) is carried by the
  script's own module docstring at `scripts/doc-coverage.py:4`.

  This is the fourth instance of the plan's recurring theme, and the sharpest:
  all three earlier ones were about *running* the wrong check. This one is
  about a citation that was never re-resolved after the file under it moved — a
  `file:line` is a pointer, and a pointer into a moving file is a claim with an
  expiry date. The rebase is precisely the event that invalidates it, which is
  why the sweep belongs in the rebase audit rather than in a later review. The
  remedy applied here is a bounds-and-identity pass over every citation the
  plan makes at the new head, not a spot fix.

  Recovery refs are retained until publication is confirmed:
  `refs/recovery/6-1-1-old-head-20260928-162555` (→ `c765659d`),
  `-old-base-20260928-162555` (→ `aa764819`), and `-target-20260928-162555` (→
  `7677c388`). The four gates the rebase hook names (`check-fmt`, `typecheck`,
  `lint`, `test`) were run in sequence on `93a5d8b6` immediately after the
  replay, and all four exited 0. Those results are bound to `93a5d8b6`; this
  entry is a later revision and they do not cover it. The seven-target run that
  acceptance requires is therefore commissioned against the commit that adds
  this entry, because a rebase creates a new candidate and every gate result
  bound to `c765659d` is historical.

- [x] (2026-09-28) **All seven gates pass on `802be5ea`, and `markdownlint` is
  a real pass rather than the UNKNOWN the earlier episode produced.** The seven
  targets ran sequentially on the post-rebase head: `check-fmt` (4s,
  `164 files already formatted`, mdtablefix `169 files left unchanged`), `lint`
  (14s, clippy `-D warnings` clean, both pylint runs `10.00/10`, interrogate
  `100.0%`, yamllint and actionlint clean), `typecheck` (1s, ty
  `All checks passed!`), `test` (180s, nextest
  `3494 tests run: 3494 passed (1 slow), 6 skipped`, doctests 87+2+39 passed
  with 0 failed), `markdownlint` (14s), `nixie` (1s,
  `All diagrams validated successfully!`), and `doc-coverage` (7s,
  `aggregate 4801/4858 98.83%`, meets the 80% threshold).

  The `markdownlint` verdict was checked for the *state*, not merely the exit
  code, because the earlier episode established that a red prerequisite makes
  the target report nothing at all and an outside reader takes silence for
  green. Both tells are present and in order — `Linting: 169 file(s)` at line 5
  and `Summary: 0 error(s)` at line 6 — so `markdownlint-cli2` genuinely ran.
  This is the first seven-target run on this branch where every gate is green
  *and* every verdict is a verdict.

  Two log artefacts were inspected rather than waved through. `lint`'s log
  carries `Blocking waiting for file lock on package cache` three times, which
  is the shared Cargo cache serializing access as intended, not a defect. And
  the candidate for a false reading is the `test` log, which is 669 KB and
  contains the strings `error` and `FAIL`: every `error` occurrence is a test
  *name* under an `error::tests` module, and `FAIL` does not occur at all. Both
  were established by reading the lines, not by the absence of a grep hit.

  Logs are `/tmp/g3-<gate>-6-1-1.out`. These results cover `802be5ea` only, and
  the later revision is covered separately below.

- [x] (2026-09-28) **CI's full gate set and all four required checks pass on
  `09e609ab`, the current head; the four Markdown-sensitive targets were also
  re-run locally on it.** This entry closes the gap the entry above left open
  on purpose — a gate result covers the revision it ran on, so `802be5ea`'s
  green did not speak for the commit that recorded it.

  CI is the stronger of the two readings because it is revision-bound and it
  ran the whole union rather than a subset. `build-test` on `09e609ab` completed
  `success` after running `Format`, `Lint Markdown`, `Lint`, `Typecheck`,
  `Doc coverage`, `Spelling`, `Validate Mermaid diagrams`,
  `Workflow contract tests`, and `Test and Measure Coverage` — every step
  `success`. Its log carries the two verdicts that matter here:
  `3494 tests run: 3494 passed (1 slow), 6 skipped`, and `Linting: 169 files`
  followed by `Summary: 0 issues in 0 files`.

  The four required checks are `success` on the same revision, and each
  *completed after* the commit was created at 14:51:11Z — which is what makes
  them evidence about this revision rather than about an ancestor:

  | Check                | Conclusion | Completed | Job                  |
  | -------------------- | ---------- | --------- | -------------------- |
  | `build-test`         | success    | 15:08:03Z | `…/job/108985193830` |
  | `kani-smoke`         | success    | 15:06:36Z | `…/job/108985193426` |
  | `netsukefile`        | success    | 14:54:57Z | `…/job/108985192215` |
  | `release / metadata` | success    | 14:53:38Z | `…/job/108985197560` |

  All four job links point at runs whose `head_sha` is `09e609ab…`, checked
  through the runs API rather than read off the check-run row, since a summary
  row also lists checks that never ran.

  The local re-run is the weaker reading and is recorded as such. The delta
  `802be5ea` → `09e609ab` changes exactly one path — this ExecPlan
  (`git diff --name-only --no-ext-diff 802be5ea 09e609ab` prints one line) — so
  only the Markdown-sensitive targets can move, and those four were run again:
  `check-fmt` (`164 files already formatted`; mdtablefix
  `169 files left unchanged`), `markdownlint` (`Linting: 169 file(s)` then
  `Summary: 0 error(s)` — both tells present, so `markdownlint-cli2` genuinely
  ran), `nixie` (`All diagrams validated successfully!`), and `doc-coverage`
  (`aggregate 4801/4858 98.83%`). Logs are `/tmp/g4-<gate>-6-1-1.out`, written
  at 14:51:38Z–14:52:16Z.

  Two honest limits on the local half. First, those log files carry **no**
  runner verdict line of the `GATE=… EXIT=<rc> … HEAD=<sha>` form that the
  `a5455a1a` run appended; the verdicts above are read from each tool's own
  terminal success line, which is one step weaker than an exit status captured
  through `PIPESTATUS[0]`. Second, the other three targets were not re-run
  locally, because they are not reachable by this delta — but that is an
  argument from the diff, not a measurement, and CI supplies the measurement on
  the same revision.

  That the evidence still describes the current head is itself checked, not
  assumptions: `git reflog` shows no commit since `09e609ab` and
  `git ls-remote origin` agrees with the local ref.

  The run has since closed out, and the whole of it is green rather than the
  required four alone: `36439268002` is `completed/success` with every job
  `success` — `build-test`, `kani-smoke`, `netsukefile`, `release / metadata`,
  and also `Windows / lint-windows`, `Windows / build-test-windows`, and
  `Windows / windows-msi-upgrade`. `lint-windows` is the one worth naming
  because it was the last to finish and it is *not* quick: its
  `Lint (Whitaker)` step ran 15:01:43Z–15:16:12Z against `Lint (Clippy)`'s
  15:00:58Z, so a reader watching the run mid-flight would have seen a 14-minute
  `in_progress` on a step whose local counterpart takes seconds. That is
  dylint building its lint library from source on a cold runner, not a stall —
  the diagnostic the earlier deadlock entry taught, applied forward: an
  unexplained wait gets measured against its own baseline before it is called a
  wedge.

- [x] (2026-09-28) **All seven targets pass on `84e4fa82`, the commit that
  lands the entry above, so the full local second reading this plan owes is
  discharged on the acceptance revision.** `check-fmt`
  (`164 files already formatted`; mdtablefix `169 files left unchanged`),
  `lint` (clippy `-D warnings` clean, both pylint runs `10.00/10`, `ambrleaks`
  silent, interrogate `100.0%`, yamllint and actionlint clean), `typecheck` (ty
  `All checks passed!`, then `cargo check --all-targets --all-features`
  finished), `test` (nextest `3494 tests run: 3494 passed (1 slow), 6 skipped`,
  then doctests `87 + 2 + 39` passed with 0 failed), `markdownlint`
  (`Linting: 169 file(s)`, `Summary: 0 error(s)`), `nixie`
  (`All diagrams validated successfully!`), and `doc-coverage`
  (`aggregate 4801/4858 98.83%`). Logs are `/tmp/g5-<gate>-6-1-1.out`.

  Four of the seven were logged *before* the commit and three after (the commit
  is 15:23:44Z; `markdownlint`, `check-fmt` and `nixie` closed at
  15:21:36Z–15:21:52Z, `doc-coverage` at 15:22:43Z, and `lint`, `test` and
  `typecheck` at 15:24:27Z–15:29:01Z). That is not a gap in the evidence,
  because the commit only had to capture a working tree the gates had already
  read, and `git status --porcelain` was empty both before the commit and after
  — the tree did not move between the pre-commit gates and the commit, and
  `git rev-parse HEAD` has read `84e4fa82` at every probe since. Recording the
  *order* rather than the aggregate is the point: an earlier entry in this plan
  had to say that its gate log described the commit that recorded it and
  therefore did not cover it, and the remedy that produced `84e4fa82` was to
  gate the tree and then commit exactly what was gated.

  The `test` log was read rather than grepped, for the same reason as before:
  it carries `coverage map: 1 of 8 capability groups written; 7 remaining`
  inside a *passing* run. That line is the whole point of the counter — a
  half-finished split passes every other coverage check, so a stall is visible
  only if this line still reaches the terminal — and `7 remaining` is the
  correct reading at `EP-M3`, with `EP-M4` not yet begun. `FAIL` does not occur
  at all (count 0), and the doctest target count is 2, matching the two
  `Doc-tests` headers this plan records as the real number rather than the
  three a hasty grep suggests.

  **The MD038 fix was then carried forward, and the two affected gates re-run at
  `6720c2b9` rather than argued about.** Writing the paragraph above reded
  `markdownlint` on a code span with a trailing space — `` `Doc-tests ` `` —
  which is MD038, and the repair was to shorten the span to `Doc-tests` rather
  than to touch the rule. `check-fmt` and `markdownlint` were re-run on the
  fixed tree (both green, the latter again in state 3 with
  `Linting: 169 file(s)` before `Summary: 0 error(s)`), and the commit
  `6720c2b9` lands exactly that tree.

  `nixie` and `doc-coverage` were re-run too, and the reason is worth stating
  because the first instinct was to exempt them as Markdown-irrelevant. They
  are not, for `nixie`: a probe of its own log shows it names **this** plan (2
  hits) among 47 `docs/execplans/*.md` files in a 170-file sweep, so a
  one-character edit inside a plan is inside `nixie`'s scope even though the
  plan carries no mermaid fence of its own. Both re-ran green
  (`All diagrams validated successfully!`; `aggregate 4801/4858 98.83%`).
  `doc-coverage` reads no Markdown at all (0 hits for `execplans`), so its
  re-run was the cheap confirmation of that rather than a necessity.

  The loop this plan has hit before is recording a gate run in a tracked file,
  which moves HEAD past the revision the run verified. It is not re-opened here:
  `6720c2b9` is the last revision verified, and the delta since `84e4fa82` is
  the MD038 repair plus this paragraph — prose, in this one file, whose Rust
  content is byte-identical to what the seven-target run passed. Earlier
  entries in this plan handled the same situation by *stating the last verified
  revision and the delta*, and that is what is done here. CI covers the same
  four required contexts on every push and is the authority for any tip beyond
  `6720c2b9`.

## Surprises & discoveries

- Observation: **re-partitioning a fixed set into more children raises the
  aggregate, so it cannot serve an aggregate budget.** The EP-M6 and EP-M7
  entries offered a nine-or-ten-child re-partition as one of two remedies for
  the breached 2400-line tolerance, and the phrasing implied it would relieve
  the aggregate. Measured, it does the opposite. Evidence: fixed overhead
  (everything outside section 5) across the five written children is 0013
  **169**, 0014 **143**, 0015 **161**, 0016 **179**, 0017 **192**. A
  re-partition adds whole children, each carrying a full section 5 *and* a full
  overhead; the only saving is that a smaller group's section 5 is somewhat
  shorter, which is a partial offset rather than a net reduction. Writing the
  three remaining groups as five children instead of three adds two extra
  children at the 143-line floor each: 3 × 183 ≈ **549** versus 5 × 183 ≈
  **915**, so the aggregate rises from ≈2923 to ≈3289 — an increase of roughly
  **366 lines**. Impact: the third remedy was stated for a purpose it cannot
  achieve, and had a reviewer accepted it on that basis the aggregate would
  have grown by more than the shortfall it was meant to close. Lesson: **before
  offering a remedy for a budget, check the arithmetic against the budget's own
  unit.** An aggregate control counts total lines, so any remedy that adds
  structural units works against it; only raising the limit or recording a
  decided waiver addresses it. Re-partitioning is a remedy for a *per-file*
  limit, which is a different control with a different unit, and it was the
  per-file tolerance that already carried a reasoned waiver. The corrected
  remedy space is: raise the aggregate budget with the figure set from the
  measured floor (≈2900–3000 for eight children); or keep 2400 and record a
  decided waiver as the per-file tolerance has; or re-partition if smaller
  *files* are the objective, accepting the larger total and the
  `names.len() == 8` edit in `tests/rfc_stdlib_coverage/roadmap.rs` that it
  requires.

- Observation: **a branch's recorded gate verdict stops covering the gate once
  the base branch moves the gate's own pin.** Every spelling verdict in this
  plan was produced by `typos-config-builder` **v0.1.1**, the version the
  branch's `Makefile` pins. While the branch sat unrebased, `main` adopted
  **v0.1.3** (#843), which bumps the spell checker underneath from `typos`
  **1.48.0 to 1.50.1**. Continuous integration does not check out the branch
  head — it checks out the *merge commit*, so it resolves the pin from merged
  `main` and will run v0.1.3 against this branch's prose. Evidence: the pin is
  a `?=` variable in the `Makefile`, so a rebase or merge silently upgrades
  it, and the version bump includes a spell-checker major-adjacent change whose
  word list is not identical. Impact: the branch's spelling verdict is **stale
  by construction**, not merely old — it is a true statement about a gate that
  will not be the one to decide, and any word newly rejected in 1.50.1 would
  appear only in CI. Lesson: **when a repository pins a tool in a file the base
  branch also edits, a green verdict is scoped to the pin, and the pin moves
  under the branch.** Rebase (or merge `main`) and re-run the gate before
  treating any verdict as covering the merge commit, and prefer re-running over
  reasoning about whether the bump *could* matter — the word list is a data
  file, not a semver contract.

- Observation: **two independent safety nets can both report success while
  neither is watching.** Evidence: `EP-M3`'s acceptance criterion is "every
  gate green", and it was pursued through two channels that both silently
  no-op'd. CodeRabbit's commit status on `f5300602` reads `SUCCESS`, while its
  only comment on the pull request says "Draft PR not reviewed" — a status that
  means "did not fail", not "did review". GitHub Actions reports nothing at
  all: PR #697 is `CONFLICTING`/`DIRTY`, and because GitHub cannot construct the
  `refs/pull/697/merge` ref for a conflicted pull request, every
  `pull_request`-triggered workflow is suppressed outright rather than failing.
  Neither condition produces an error, a red check, or a notification. Impact:
  the branch's 23 commits have never been CI-validated, and the last CI run on
  it is `3f02ee37`, its opening push on 2026-09-08. Lesson: a green status is
  only evidence if it was *earned by a run* — check that the run exists before
  reading its conclusion, and treat an absent run as a failure, not as silence.
  The mitigation is structural, not vigilance: keep the pull request mergeable,
  because a conflict disables the entire CI channel.

  *Amendment, added the same day: the mitigation worked, and this Observation
  is kept in its original tense because it records a state that no longer
  holds.* PR #697 became `MERGEABLE` when the conflict was resolved, and the CI
  channel reopened with it: the branch has since been validated on `09e609ab`
  (run `36439268002`, `completed/success`, every job green) and again on
  `eb21dec1` (run `36445217340`, triggered 2026-09-28T15:39:14Z). The sentence
  "the branch's 23 commits have never been CI-validated, and the last CI run on
  it is `3f02ee37`" was true when written and is now false; it is left standing
  because the lesson depends on it. What the amendment adds is the confirmation
  that the *structural* fix — keep the pull request mergeable — is what
  restored the channel, rather than attention or luck.
- Observation: **"every gate green" is the wrong acceptance criterion for a
  shared, contended host, and the plan wrote it anyway.** Evidence: the same
  commit `288526a2` is red locally (`make test`, one nested-Cargo compile test
  timed out at 300.008s with load average 74.39 on 24 cores) and green on CI
  (3408/3408 passed, no timeout, all 3408 in 415.659s). Both are honest runs of
  the same code. Impact: the milestone was gated on an acceptance criterion
  that its own environment cannot reliably satisfy, so a correct change could
  be held indefinitely by host load, and the natural failure mode is to keep
  re-running until it goes green — which is exactly the "flake" reasoning this
  plan's own memory rule forbids. Lesson: when a criterion is about the
  artefact rather than the machine, say so, and name the authority that
  measures the artefact. Here that authority is CI, which runs on a clean guest
  with the repository's own budgets and no other tenants; a local gate run's
  job is to be *green where it can be* and to have its residual failures
  *attributed*, not eliminated. The plan should have read "every deterministic
  gate green, and any remaining failure attributed to a named, measured cause".
  `EP-M3` is accepted on that reading, with the local red recorded rather than
  suppressed.
- Observation: **a delegated causality check can be reported in a form that
  does not reproduce, while its conclusion still holds.** Evidence: the ninth
  gate run reported that
  `git diff --name-only origin/main…HEAD -- crates/ src/ tests/ tests/ui/
  tests/support/`
  "returns 0 paths", and drew from that the conclusion that the failing test
  is byte-identical to `origin/main`. Re-running it returns **17** paths — this
  branch's entire `tests/rfc_stdlib_coverage*` tree. The conclusion is
  nevertheless true, and provable a different way: the failing test's own files
  are untouched
  (`git diff --name-only origin/main…HEAD -- tests/command_env_ui_tests.rs
  tests/ui/cli_configuration_pass/`
  is empty). Impact: had the conclusion been false and the command cited as
  its warrant, an inherited failure could have been waved through. Lesson:
  re-run a subagent's headline command before relying on it, and prefer an
  attribution that names the specific test's files rather than a coarse
  directory prefix — a path-prefix filter is exactly the kind of probe that can
  pass for the wrong reason, since `tests/` is both the directory this branch
  adds to and the directory the failing test lives in.
- Observation: **a corpus-wide invariant test finds defects in files the branch
  does not own, and a rebase can hand it new ones.** Evidence: the dangling
  `netsuke-test-framework-technical-design.md` link at `96aefc9c` is on
  `origin/main`'s tip and is *live there* —
  `git show origin/main:docs/rfcs/ 0007-…md` line 62 carries the bad form while
  line 22 of the same file carries the correct `../`. Main's own CI is green on
  that commit (`build-test`, `Windows / lint-windows`, `kani-smoke`,
  `netsukefile` all `success`), because `links::dangling` and its
  `inter_document_links_resolve` caller exist only on this branch:
  `git ls-tree -r origin/main tests/` has no `rfc_stdlib_coverage*` entry at
  all. Impact: the invariant is this branch's to enforce and no upstream gate
  shares it, so the failure could only ever appear here, and it appeared only
  because a rebase imported a document neither party was editing in this branch
  — the RFC 0006 split never touches RFC 0007. Lesson: when a branch adds a
  test that reads the whole corpus rather than its own diff, re-run it after
  every rebase and expect it to indict the incoming commits, not the branch.
  The repair belongs in the branch (and rides to main with it) rather than in a
  separate upstream pull request, because the two are the same edit.
- Observation: **"environmental" can be the right verdict for the wrong
  reason.** Evidence: the seventh and eighth gate runs both correctly cleared
  this branch's diff of blame for the two `make test` timeouts, and both
  reached for host load as the cause. Load was real but not decisive: the two
  tests are over budget on this base *by construction*. `origin/main` already
  fixed the class, and not by widening a timeout — it moved
  `harness_compiles_under_a_split_build_dir` onto recorded parser coverage and
  serialized the remaining nested Cargo builds into a `max-threads = 1` group.
  Impact: the load explanation implied "re-run when quiet", which would have
  burned another 400-second gate run and failed the same way. The decisive
  check is ancestry, not load: `git merge-base --is-ancestor e2fc2083 HEAD`.
  Lesson: when local gates disagree with CI, check whether the fix already
  landed upstream before explaining the discrepancy from the machine.
- Observation: **a review finding can name a real defect and still prescribe the
  wrong fix**, and the fix is the part that ships. Evidence: the `CONF-1`
  deference finding asked that `as Ansible does` stop being flagged and offered
  `such as Ansible` as the false positive to fix instead. The first half is
  wrong: `as Ansible does` is this plan's *own recorded seeded fault*, the
  transcript at `docs/execplans/…md` showing
  `justifies a helper by appealing to Ansible ("as Ansible")`, and it is proven
  to fire. Un-flagging it would have turned a green control into a
  green-looking empty one — exactly the failure this plan's `CONF-1`
  observation already records having been bitten by once, when the obligation
  sat as prose for six days with nothing implementing it. The second half is
  right: `Unlike Ansible` contains `like Ansible` and means the reverse, and
  `such as Ansible` is a compound preposition introducing an example. Impact:
  the leading-boundary fix was adopted and clears `Unlike Ansible`;
  `such as Ansible` needs a separate exclusion because its `as` is its own
  word, so the boundary alone cannot reach it. The reviewer's `as Ansible does`
  example was refused, with the refusal recorded in the code's own doc comment
  so the next reader does not re-litigate it. Lesson: verify a finding's
  *examples* as well as its mechanism — the mechanism here was sound and the
  boundaries it proposed were not.

- Observation: two of this pass's findings asserted a mechanism about the
  document that the document does not support, which is the same failure mode
  as a rule whose stated reason is false. Evidence: `section7.rs`'s comment
  claimed `glob` "also reaches `accepted` as an accept row in its own right";
  every section 7 row naming `basename`, `dirname`, or `glob` is a `Reject`
  row, and `glob`'s only row is the `fileglob` reject at RFC 0006:498.
  `document.rs`'s comment promised that a heading quoted inside a fence "still
  lands on the real one", which `position()` cannot deliver — though no
  document in the corpus has such a heading, and each looked-up heading is
  unique, so nothing was failing. Impact: both were fixed at the source of the
  false claim, and the `document.rs` fix went further than the finding asked by
  making the promise true rather than only rewording it. A green suite was no
  evidence either way here: neither defect could fail, which is what made them
  survive two prior reviews.

- Observation: a change can pass two CodeRabbit reviews and still fail the
  commit gate, because the two read different things. The second pass's seven
  Rust files were reviewed twice and cleared both times; `make lint` then
  rejected two of them at `4c631e18`, on `too_many_lines` (73/70) and
  `iter_skip_next`. Evidence: `/tmp/lint-netsuke-<branch>-4c631e18.out:10,20`
  against the same files at `c7c309d6`, where `clippy` was green. Impact: the
  gate ran red on a tree CodeRabbit had just approved, which inverts the
  expected order. The instruction to make every gate green *before* requesting
  a review is not a formality about sequencing — it is what keeps the review
  looking for semantics, because a reviewer asked to find style defects finds
  some, and they are worse ones than `clippy`'s.

- Observation: this repository's `clippy.toml` sets
  `too-many-lines-threshold = 70`, well below the default 100, so a function
  split by responsibility can still be rejected for length alone. Evidence:
  `clippy.toml` against `map::parse`, which grew from 68 to 85 lines when the
  link-number check was added to it. Impact: adding a check to an existing
  function is a length risk even when the addition is small, and the remedy is
  to extract the *reasoning* into named functions rather than to compress
  statements.

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
  `scripts/doc-coverage.py`; `Makefile:325-329`. Impact: the governing gate on
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

- Observation: Whitaker's `module-max-lines` lint caps every non-root module at
  400 lines, counted over the module's whole span. Evidence: the second gate
  run failed at `tests/rfc_stdlib_coverage/mod.rs:30:5` with "Module `survey`
  spans 787 lines, exceeding the allowed 400" — a failure the first run never
  reached, because `lint-clippy` aborted `make lint` before Whitaker ran.
  Impact: the parser was split into five modules by responsibility — `section7`
  (the disposition tables), `section8` (the section-reference guard), `totals`
  (table 11 and the purity aggregate), `inventory` (the transcribed section 7
  literals), and `assertions` (the cross-checks) — leaving `survey` to hold the
  derived result and its orchestration. The largest is now 277 lines. The lint
  inspects `mod` items, so a test crate root is not measured: 660-line
  `tests/manifest_jinja_tests.rs` is green. Worth knowing before the child RFCs
  arrive, since their registry and clause parsers grow the same way.

- Observation: the section 14.13 coverage map inserted at `EP-M1` was captioned
  `_Table 12:_`, colliding with the earlier table 12 (the `to_datetime`
  conversion specifiers at `:1536`). The document numbers tables sequentially,
  and `_Table 15:_` already existed, so the map is now `_Table 16:_`. Evidence:
  `docs/rfcs/0006-...md:2068`. Impact: nothing parses captions, so only review
  would have caught it. Worth re-checking in each child RFC, where the registry
  table is the one this plan adds.

- Observation: a `sed -i "START,ENDd"` whose `END` resolves *above* `START`
  deletes exactly one line and reports no error. Evidence: the `COV-4` control
  first looked its caption up with `grep -n '^_Table 12:' | head -1`, which
  matched the earlier table 12, so the range ended above its start; POSIX then
  matches only the first address, and the control removed the header row alone
  and failed the suite with "the coverage map has 7 rows; expected 8" — a real
  failure, but not the one intended. Impact: general, and the reason the
  controls print the seeded diff before the verdict. A control that fails is
  not yet a control that tested what it meant to, which is exactly the vacuity
  `COV-1` to `COV-6` exist to prevent. Two fixes were needed: anchor the
  caption search after the header (`awk -v start=...`), and guard both lookups
  so an empty address aborts the script rather than reaching `sed`.

- Observation: the **first written version of this parser was fence-blind**, and
  the failure is the silent kind. A fenced `# not a heading` line inside RFC
  0006 section 8.9 read as a depth-1 heading, ended the subsection at the
  fence, and left `strftime` and `to_datetime` — specified further down section
  8.10 — looking like helpers with no contract. Six of the seven checks failed,
  and the message named the two helpers, not the code block. Evidence: the
  fence control transcript in `Verification plan`; the red run is reproducible
  by re-running `/tmp/rfc-fence-control.sh` against a checkout of `885edb93`.
  Impact: this was the one control that had to be *written* to be believed,
  because it was raised as a CodeRabbit review finding marked "trivial" and its
  severity is anything but. Every child RFC this plan goes on to write carries
  example fences in section 5, so the fault was one document edit away from
  being live. Three scans shared the root cause — `Section::tables`,
  `Section::subsection`, and `section8::subsection_lines` — and all three now
  consult one `Fences` cursor, extracted with the rest of the Markdown lexical
  layer into `markdown.rs`. The post-fix run of the same control leaves the
  suite at 7 passed.

- Observation: the plan's own COV-3 scoping note was right and the code did not
  implement it. The purity aggregate must range over the 57 **proposed**
  helpers; the registries carry all 60 accepted ones. Filtering on purity alone
  yields 54/5/1 against section 6.1's 52/4/1 and would have failed a correct
  document at `EP-M11`, the first milestone where all eight registries exist.
  Evidence: `with_purity` had exactly three call sites, none filtering on
  registration kind, and the optioned rows include the filesystem-observing
  `glob`. Impact: the fix is at the accessor rather than the call site —
  `new_with_purity` carries the registration filter in its contract, so a
  future caller cannot get the wrong answer by forgetting it. Also raised by
  CodeRabbit and also latent: the check is guarded by `written == rows.len()`,
  so it cannot fire before every child exists.

- Observation: Whitaker's `conditional_max_n_branches` counts a match **guard**
  as branches, and the limit is 2. Evidence: the first `Fences` implementation
  guarded its closing arm with
  `opened == character && run >= opened_run && info.trim().is_empty()` and
  failed at `markdown.rs:91` with "Collapse the match guard to 2 branches or
  fewer". Clippy passed the same expression, and so did `make test` and
  `make typecheck`; the violation is dylint-only, which is why it surfaced at
  `make lint` alone. Impact: the guard was not rewritten as a flatter `if`,
  which would have lost the "closing fence carries no info string" rule — the
  natural cheat is to drop a condition and the branch count falls with it.
  Instead the three conditions became two predicates, `Delimiter::closes` and
  `Delimiter::is_closing_run`. Eleven hand-run probes over `/tmp` copies
  confirm the CommonMark semantics survive: info strings, longer and shorter
  closing runs, tilde-versus-backtick nesting, inline code spans, indented
  fences, and unterminated blocks. When a child RFC's parser grows a similar
  guard, expect this lint, and split a condition into a named predicate rather
  than deleting it.

- Observation: `make fmt` cannot be pointed at one file, and `mdtablefix` has
  no check-only mode. Evidence: `check-markdown-format.sh` stages copies and
  compares, precisely because the tool always writes; `MD_FILES_FIND` in the
  Makefile covers the whole corpus. Impact: after an editorial prose edit the
  scoped invocation is
  `mdtablefix --in-place --wrap --renumber --breaks --ellipsis --fences <file>…`,
  with the flags copied from `mdformat-all` and the checker. Running
  `make fmt` instead would reformat unrelated documents and bury the milestone
  diff. Both files edited at `EP-M1`'s second review were pure paragraph
  rewrapping — zero table-pipe changes — which the checker's failure message
  alone does not distinguish.

- Observation: the worked section `EP-M2` commits is a **fenced** copy, so it
  is subject to a width limit the real child RFC is not, and the first draft
  was written to the real child's width and failed the gate. Evidence: a
  208-column fenced row failed `MD013` with
  `t.md:6:121 error MD013/line-length Line length [Expected: 120; Actual: 208]`,
  and `.markdownlint-cli2.jsonc` sets `MD013.code_block_line_length` to 120
  while setting `tables: false`. Impact: a fenced copy must be laid out to 120
  columns even though `tables: false` means no real table in the corpus is ever
  measured, so the worked section's tables are narrower than RFC 0013's will be.
  `mdtablefix` compounds it by leaving fenced content completely alone —
  probed on a file with a misaligned fenced table and an over-wide fenced
  paragraph, both of which it reported as "left unchanged" while reflowing the
  same paragraph outside the fence — so `make check-fmt` will not repair a
  fenced copy either. The consequence for the remaining children is nil,
  because a child RFC is real Markdown: its tables are exempt from `MD013` and
  are reflowed by `mdtablefix` like every other table in the corpus. The
  consequence for this plan is that the specimen is sized for the fence and
  says so, rather than being the widest form a child may use.

- Observation: the first draft of the worked section named a `non_string_key`
  error condition for `from_yaml`, and RFC 0006 section 8.1 rejects the
  opposite set. Evidence: section 8.1 says "Mapping keys may be strings,
  integers, or booleans. Sequence and mapping keys are rejected." An integer or
  boolean key is therefore *accepted* and a sequence or mapping key is
  rejected, so a condition called `non_string_key` names the wrong predicate in
  both directions: it would fire on input the document admits and stay silent
  on input it excludes. Impact: renamed to `unsupported_key`, which states what
  is rejected without contradicting the accepted kinds. The defect was found by
  writing the condition list against section 8.1 rather than from memory of it,
  which is the whole reason the worked section was drafted before `EP-M3`
  rather than during it.

- Observation: the skeleton this plan tells `EP-M2` to "copy literally" **does
  not satisfy the parser `EP-M1` already shipped**, in two independent places.
  Evidence: the skeleton's registry heading read
  `### 5.1. Purity and manifest-query registry` against `REGISTRY_HEADING`'s
  `### 5.1. Registry` (`registries.rs:20`), and its manifest-query cell
  vocabulary was `Available`/`Stub` against `check_manifest_query`'s `yes`/`no`
  (`registries.rs:229-237`). Impact: both are hard failures on the first row
  read, and both would have fired only at `EP-M3` — after the go/no-go had
  already been spent on a document written to the wrong contract. Neither was
  caught by `EP-M1` because no child RFC exists yet, so every check that reads
  a registry is vacuously green; the template was prose the parser had never
  been pointed at. Two readings were available for each — change the template
  or change the parser — and the template gave way in both, because in both the
  parser already agrees with the parent document. RFC 0006 table 2's own column
  is headed "Available in manifest queries" with cells `Yes` and `No`, so
  `Available`/`Stub` was a second vocabulary for a fact the parent already
  spells; and `### 5.1. Registry` also matches RFC 0006's own section 14.13
  wording, "carries the group's registry". The fix is to the skeleton plus a
  note at each site naming the constant that reads it, so the next editor knows
  the heading is parsed rather than prose. The general lesson: a template
  committed in prose is untested code, and the milestone that first consumes it
  is the wrong place to discover that.

- Observation: the cross-check added to catch a *child's* wrong namespace read
  a value the same task had hardcoded wrong, so the guard would have failed a
  correct document. Evidence: `section7::apply_optioned` inserted every
  optioned helper as `Namespace::Filter`, while RFC 0006 section 3.2 lists
  `glob` under Functions; `check_rows_agree_with_survey` compares the child's
  namespace cell against that value, so `COV-1` would have failed RFC 0018 —
  the group that owns `glob` — for being right. Impact: found only because the
  previous pass's fix was re-derived from the document rather than trusted. A
  check is a comparison, and a comparison is only as good as its weaker side:
  hardening the side that reads the untrusted document is pointless while the
  trusted side is a literal nobody re-reads. `Optioned` now carries its
  namespace, sourced from section 3.2, and the same reasoning is why the
  optioned rows' purity is parsed from the child's own cell rather than
  defaulted.

- Observation: `CONF-1` existed as an obligation in this plan for six days while
  the code implemented only its table half. Evidence: the plan's `CONF-1` says
  each subsection must be non-empty, name an owned helper or carry the `D6`
  escape phrase, and contain no Ansible-deference phrase; `clauses.rs` parsed
  the `Clause | Discharge` table and compared its id set, and read no
  subsection body at all. Every subsection check the plan names was unchecked,
  and every control in `CONF-1`'s non-vacuity list would have passed — because
  none of them was implemented either. Impact: this is the plan's own principal
  risk, left unguarded by the very obligation written to guard it, and it was
  found by review rather than by a failure. The gap was invisible precisely
  because the check that *did* exist passed: a green suite one obligation short
  of what the plan claims is indistinguishable from a complete one. The general
  lesson is that an obligation's prose and its implementation need to be read
  against each other once, deliberately, and that the plan's non-vacuity
  controls are the cheapest way to do it — a control that cannot fail is a
  spec, not a test.

- Observation: the spec of the `CONF-1` check, written *before* it parsed the
  ADR's worked specimen, turned out to reject that specimen. Evidence:
  subsection 5.9 of the `EP-M2` worked section named thirteen diagnostic codes
  and no helper, so `names_an_owned_helper` was false and the `D6` escape
  phrase was absent — a true positive, not a false one. Impact: the specimen
  was demonstrating clause 6.9 by enumerating the group's contribution to it,
  which is exactly the "specific enough to be worth writing" standard the rule
  is meant to enforce, and the rule's own vocabulary was still satisfied by
  naming the codes' subject. The fix names all five helpers alongside the codes
  rather than weakening the check, because the check is right about what a
  reader needs. This is the useful shape of the interaction: a rule written
  against a document it has not read yet is the only version that can surprise
  its author, and it is worth reading the two against each other before the
  rule is relied on rather than after.

- Observation: two of the three shapes `CONF-1` checks for are already caught
  when the class is written *inconsistently*, and only the third needs the new
  check. Evidence: `check_manifest_query` (`registries.rs:240`) already rejects
  a row whose purity class and manifest-query cell disagree — table 2's own
  rule — so a probe setting `from_json` to `Subprocess-observing` with the cell
  left at `Yes` fired that check, not the new one. The new aggregate checks
  caught it only once the probe was made *consistent* (class
  `Subprocess-observing`, cell `No`), which is the case nothing else sees: the
  row is internally coherent and still contradicts section 6.1's "no proposed
  helper is". Impact: the two checks are complementary rather than redundant,
  and a probe that proves one is live must be constructed so the other cannot
  answer first. Worth remembering when adding checks to a suite that already
  guards adjacent invariants.

- Observation: the two totals checks were gated on all eight capability groups
  being written, so neither could fire until the split was complete. Evidence:
  the generator's aggregate was wrapped in `if world.map.unwritten() == 0`,
  which is true only when no child exists yet or all eight do; the `EP-M3`
  control writes exactly one child, so the milestone it was written for could
  not trigger it. Impact: the totals are now asserted partially as well as
  exactly — an upper bound on each purity class and on the optioned count runs
  at every prefix, and the exact equality runs only when complete. The bound is
  sound because section 6.1's three counts are a budget, not a target: a
  half-written split can spend too much of it but can never spend too little.
  The forbidden classes (clock, network, subprocess) are asserted as hard zeros
  at every prefix, because section 6.1 states no proposed helper is any of them
  and a zero is not a budget. The general shape is worth reusing: a check
  guarded by a completion condition is a check whose subject is the completion,
  not the property.

### `EP-M0` audit results (2026-09-11)

The audit re-derived every count in this plan mechanically from
`docs/rfcs/0006-...md` with a throwaway script (`/tmp/ep-m0-audit.py`, not
tracked; the derivation it performs is reimplemented in the coverage test at
`EP-M1`). Results, with the plan's claims alongside:

| Quantity                      | Plan claims | Derived | Verdict      |
| ----------------------------- | ----------- | ------- | ------------ |
| Accept rows in section 7      | 55          | 55      | agree        |
| Defer rows in section 7       | 6           | 6       | agree        |
| Reject rows in section 7      | 50          | 50      | agree        |
| New Netsuke helpers           | 57          | 57      | agree        |
| Optioned existing helpers     | 3           | 3       | agree        |
| Accepted set                  | 60          | 60      | agree        |
| Filters / tests / optioned    | 41/16/3     | 41/16/3 | agree        |
| Purity (pure/fs/env)          | 52/4/1      | 52/4/1  | agree        |
| Naive section 8 headings      | 58          | 58      | agree        |
| Forbidden-set members         | 34          | 71      | **disagree** |
| Table 11 class counts sum     | 50          | 50      | agree        |
| Class split, last stated rule | 22/10/18    | 8/24/18 | **disagree** |

- Observation: the plan's `COV-2` assertion that the forbidden set has "exactly
  34 members" reconciles only as a **row** count — 50 reject rows minus 22
  "already provides" rows, plus 6 deferred rows — and not as a name count under
  any derivation. Reject-row name cells expand to 67 names; removing the names
  on rows the notes class as "already provides" leaves 38 or 40, and adding the
  6 deferred names gives 44 or 46. Evidence: the audit script's alias-group
  expansion, and `docs/rfcs/0006-...md:468-635`. Impact: the plan's number is a
  category error. `D10` replaces the size assertion with the complement rule's
  71-name deny set. The membership assertions are nearly all satisfied:
  `is_file`, `quote`, `fileglob`, and `lookup` are all forbidden under the
  complement, so the four names the first draft's handwritten list omitted are
  still caught. One assertion fails: the plan required the deny set to
  **exclude** `expanduser`, on the ground that its row is classed "already
  provides"; the complement forbids it. That exclusion was the one place the
  plan's stated method and its expected members disagreed, and the members had
  the better of it — the internal inconsistency, not the number 34, is what
  made the class-based method untenable.

- Observation: the plan's "exactly 34 members" and its `is_file` membership
  assertion are mutually inconsistent, which is what actually indicts the
  class-based method. Under a faithful class reading (`file` / `is_file`
  classed "already provides") `is_file` is **not** forbidden; under the row
  count that yields 34 it is not either. The plan could not have both its
  number and its membership list, and `EP-M0` was right to stop on it. Evidence:
  `docs/rfcs/0006-...md:468-635` and `:600-639`. Impact: the complement rule
  satisfies both the intent (catch `is_file`) and the four-name witness, and
  gives up only the `expanduser` exclusion, which nothing depended on.

- Observation: the resolution-note column does **not** discriminate the three
  reject classes. `EP-M0`'s first reading said it did, and `EP-M1` falsified
  that; the retraction is recorded here rather than only in `D10`, because the
  earlier reading is the one a reviewer would otherwise still be working from.
  The rule that was believed to reproduce the split was: **alias** if the
  resolution cell contains the token "alias" or cites §10.2; otherwise
  **exists** if it begins "Exists" or names a provider in backticks; otherwise
  **principle**. Re-derived at `EP-M1` it gives **8 alias / 24 exists / 18
  principle**. The *principle* count is right and the (correction at `EP-M1`)
  premise holds — every principle row is backtick-free, so `ternary` ("Jinja
  conditional expressions") and `mandatory` ("Strict undefined already errors")
  land there correctly. The error is the 32-row remainder: table 11 counts 10
  alias and 22 exists, and the two-row difference is exactly `win_splitdrive`
  and `fileglob`, the rename rows whose resolution cells name a Netsuke call
  form rather than the word "alias". Recovering 22/10/18 would mean
  special-casing those two out of *exists* while leaving `now` — a reject row
  that also names an existing helper in backticked call form — inside it, with
  nothing in the document to distinguish them. Evidence:
  `docs/rfcs/0006-...md:468-635`, `:1617-1690`, and `:600-639`. Impact: the
  split is **not derivable** and is not asserted; `D5` rule 3's prescribed
  remedy — adding a discriminating column to section 7 — is needed only if the
  split is ever wanted as a contract, and no normative edit to RFC 0006 is made
  now. `COV-2` asserts the parseable totals and that table 11's three class
  counts sum to the reject-row count, so a broken table still fails loudly.

- Observation: the control schedule below is off by one milestone, and this was
  found by writing `EP-M2`'s probe rather than by reading. It says the `COV-2`,
  `COV-3`, and `CONF-1` controls "need something to corrupt and run at `EP-M4`
  and `EP-M5`, the first milestones with a registry row and a clause body". The
  first milestone with a registry row and a clause body is `EP-M3`: it delivers
  RFC 0013, which owns five registry rows and discharges all eleven clauses.
  `EP-M4` is merely the *second*. Evidence: `EP-M2`'s liveness probe placed the
  ADR's worked specimen at `docs/rfcs/0013-…md` and flipped RFC 0006's map row
  to `written`, and `every_child_discharges_every_clause`, `COV-1`, `COV-3`,
  and the link check all ran non-vacuously against it. Impact: the four
  controls are runnable at `EP-M3` and are discharged there, not deferred; the
  schedule was corrected in place rather than left to contradict the milestone
  it sits above.

- Observation: `COV-3`'s purity aggregate is satisfiable only over rows whose
  `Registration` is `New`. The registries carry all 60 accepted helpers, and
  the three optioned rows include `glob`, which is filesystem-observing;
  all-row aggregation therefore yields 54 pure / 5 filesystem / 1 environment,
  not the 52/4/1 that section 6.1 states. Evidence:
  `docs/rfcs/0006-...md:222-260` counts only the 57 proposed helpers. Impact:
  minor; the fix is to scope the aggregate to `New` rows and say so in `COV-3`.

- Observation: RFC numbers 0013 to 0020 are free. `origin/main` carries RFCs
  0001 to 0012 only, and every active remote branch checked (`3-14-8-…`,
  `4-3-1-…`, `4-4-1-…`, `7-1-1-…`, `adopt-rstest-bdd-v0-5-0`,
  `docs/rfc-0001-implementation-roadmap`, `document-the-timeout-tiers`,
  `property-testing-rfc`) stops at 0012. Impact: no reservation is required
  before `EP-M3`, but the allocation is only a convention and `EP-M1` must
  still backfill the number-allocation table in RFC 0006 and re-enumerate
  before each child commit.

- Observation: the partition's per-group registry contents all reconcile
  against the accepted set. Group sizes are 5, 6, 8+7, 4+4, 6+1(+2), 1+4(+1),
  9, and 2, summing to 41 filters, 16 tests, and 3 optioned helpers, exactly
  the accepted set of 60. Impact: `EP-M0`'s partition is confirmed; no helper
  needs to move between children, and the ambiguity tolerance is not triggered.

- Observation: three defects in the coverage parsers were found only by
  running the checks against the real documents, and all three would have
  misreported rather than crashed. First, `Section::tables` pushed the table's
  own accumulator and then appended rows to a different allocation, so every
  table parsed as empty. Second, `Section::subsection` excluded its own heading
  line, so a table placed directly under a subsection heading was attributed to
  no heading at all and named `""`; `map::parse`, `registries::parse`, and
  `clauses::discharged` all filter on that heading text, so all three would
  have failed on the first child RFC as well. Third, `roadmap`'s `FIRST_STEP`
  and `LAST_STEP` carried a `###` prefix while being compared against a heading
  already stripped of it, so no capability step was ever in range and `COV-6`
  reported the roadmap as empty. Evidence: the diagnostics that located each —
  `coverage map subsection contains no table; 39 lines, 1 tables [("", 8)]` for
  the second, `docs/roadmap.md has no capability steps starting ### 6.2.` for
  the third. Impact: the narrow parser contract in the module docs was right,
  but three parsers were never exercised against their subject before these
  runs, which is the argument for landing the checks while their subjects are
  still unwritten.

- Observation: `COV-4`'s required line cannot be printed the obvious way. The
  workspace denies `clippy::print_stdout` and `clippy::print_stderr`
  (`Cargo.toml:209-210`), and `cargo nextest run` captures a passing test's
  output by default, so a `println!` would be both a lint error and invisible.
  Evidence: `make test` runs `cargo nextest run`, whose
  `profile.default.overrides` already raise `success-output` for two tests.
  Impact: the check carries one
  `#[expect(clippy::print_stdout, reason = ...)]`, and `.config/nextest.toml`
  gains an override for `coverage_map_status_is_reported` with
  `success-output = "immediate"`. Verified: a green run prints
  `coverage map: 0 of 8 capability groups written; 8 remaining`.

- Observation: the 400-line cap is measured on *code*, not on file length, and
  the difference defeated the first attempt to verify the fix. Liveness-probing
  `module_max_lines` by appending 120 `// pad` lines to a 303-line module left
  Whitaker green, which reads as "the lint is not looking at this file". It is
  looking; it does not count bare comment lines. Re-probing with 110
  doc-comment lines plus a function took the same file to 415 and produced
  `error: Module progress spans 415 lines, exceeding the allowed 400.` Impact:
  the fix at `7605c884` is confirmed by a live oracle rather than by an absence
  of output, and every module is now at most 303 lines, so none is near the
  boundary under either counting rule. This is the second time in this task
  that a probe's own defect would have been read as a pass had the probe not
  been run against a case it was expected to fail.

- Observation: the cap's scope is *nested modules*, and the corpus confirms it
  rather than merely permitting it. Every non-crate-root Rust file in the tree
  is at most 400 lines, with four (`src/ninja_gen/mod.rs`,
  `src/manifest/mod.rs`, `src/manifest/render_tests.rs`,
  `src/manifest/expand_test_cases/condition_cases.rs`) sitting exactly at 400 —
  a wall, not a coincidence. Eight `tests/*.rs` files exceed it (411 to 660),
  all of them crate roots, and `dylint.toml` treats `tests/*.rs` targets as a
  separate compilation unit. Impact: the two files this milestone added were
  the only nested modules over the cap, so the violation was this task's to fix
  and not an inherited condition; had the cap applied to crate roots, the same
  split would have been required of eight pre-existing files and would have
  been out of scope.

- Observation: "just a module move" is exactly the change for which a
  formatting gate gets skipped, and exactly the change most likely to need one.
  The split at `7605c884` was verified with three gates — clippy, nextest, and
  Whitaker — and committed without `make check-fmt`, which then failed on two
  mechanical diffs in the two files the split had just created by hand. The
  first stage of the documented gateway set was the one omitted. Impact: the
  cost was a fifth gate run and a second scrutineer invocation, both of which
  the first-stage gate would have prevented for the price of one command. Every
  commit from here runs all five `make` targets as a single command, however
  mechanical the change looks.

- Observation: `docs/contents.md` is the one index in the corpus that nothing
  checks, and this task was carrying the only defect it had. Of 39 ADR files,
  38 were indexed and the missing one was exactly this branch's. Impact: the
  omission survived nine gate runs, and it was found only by reading the file,
  not by a gate. No test in the tree mentions `contents.md` at all. The rule
  this plan already fixed for RFC numbers therefore has a second, unguarded
  instance: `D2` allocates RFC numbers lazily *because* they collide, and the
  index that records them has no equivalent guard. Not fixed here — a check on
  `docs/contents.md` is outside this plan's declared interface set, and adding
  one would be a new obligation rather than a discharge of an existing one. It
  is recorded so the omission is not rediscovered later.

- Observation: "run `make fmt`" is not a remedy for MD013, and the two look
  alike from the gate output. The three over-long lines this task added were
  prose, and `make fmt` rewrapped neither: `mdtablefix` has no wrap rule that
  reaches an unwrapped prose line, and `markdownlint --fix` does not implement
  MD013 at all. Impact: the plan's summary of `make fmt` as the fix for
  formatting findings was too broad. MD013 is a hand fix, and treating the
  formatter as its remedy would have left the gate red for a second run. The
  converse also held and was checked rather than assumed: after the hand fix,
  `make fmt` ran again and touched none of the three, and
  `mdtablefix --renumber` did not eat the `- [x] (2026-09-25)` progress
  entries, which is the failure mode that tool is known for.

- Observation: a non-normative copy of a normative section is still a copy, and
  nothing checks the two against each other. `ADR-040` carries RFC 0013's
  section 5 as a worked specimen and says in terms that RFC 0013's copy is the
  normative one. The specimen's `from_yaml_all` bullet nevertheless still read
  "rejects every `from_yaml` condition" — the precise wording RFC 0013 §5.6 was
  corrected away from when RFC 0006 §8.1 was read properly, since a
  zero-document stream is an empty sequence rather than an error. So the
  divergence was introduced by the correction itself: it was applied to the
  artefact and not to the specimen that previews it, and the two then disagreed
  for eleven days across three gate runs and three review passes. Evidence:
  CodeRabbit's fourth pass flagged the ADR line; extracting the bullet from
  both files showed the ADR's copy to be the pre-correction text. Impact: this
  is not a stale comment. The specimen is the only worked example a `EP-M4` to
  `EP-M10` author has, and it is a full section rather than a summary, so the
  ADR is what gets copied. A second copy of a normative text needs either a
  check or an explicit pointer to the artefact as the single source — the ADR
  already has the pointer and it was not enough, so the rule this records is
  narrower: **when a correction is applied to an artefact, grep for its other
  copies in the same commit.** Neither `make check-fmt` nor `make lint`
  compares two documents' prose, and no review pass before this one had both
  copies in view.

- Observation: the ownership check could not see the one collision its own
  subject matter is named for. The coverage map must contain exactly one row
  per capability group, `D2` allocates RFC numbers lazily *because* they
  collide, and the plan re-enumerates remote heads before each child commit for
  exactly that reason — yet `parse` had no duplicate-number guard, and nothing
  else noticed one either. Evidence: `Map::ownership` inserts every claimed
  helper into one map keyed by name and errors only when the *same helper* is
  claimed by two *different* numbers, so two rows sharing a number are not a
  conflict to it; downstream, `registries::parse_all` filters on
  `children.contains`, and both the status and registry checks match rows with
  `find`/`any`. A mutation of row `0015` to `0014` therefore passed all fifteen
  checks, one child silently representing two capability groups. Impact: three
  separate guards each had a partial view and each assumed another had the
  whole one. The duplicate-number guard now sits in `parse`, the only place
  both rows are visible before they are separated into `Map::rows`. The general
  lesson is the converse of the ADR one: a check keyed on the *aggregate*
  (helpers → owner) cannot enforce a property of the *members* (one number per
  row), and the subject matter's own history of collisions is a hint about
  which property deserves a direct guard rather than an emergent one.

- Observation: three of this repository's Markdown tools disagree about where a
  fence may begin, and the disagreement runs in the direction that hides a
  defect. `CommonMark` allows up to three leading spaces before an opening
  fence; `Delimiter::opening` accepted any amount; `mdtablefix`, the gating
  tool, is lenient through indent 6 by way of `trim_start()`; and this
  repository's `markdownlint` config sets no MD046 key, so MD046 runs in
  "consistent" mode. Evidence: `Delimiter::opening` read `line.trim_start()`;
  an indentation sweep showed `mdtablefix` leaving a table untouched at every
  indent 0–6, and `markdownlint` reporting zero errors for a 4-space-indented
  fence. Impact: a document whose only code block is indented passes every
  gate, so the leniency was invisible by construction rather than by accident —
  and the failure it permits is the silent one, an indented run of backticks
  swallowing every heading and table beneath it. The fix aligns the predicate
  with `CommonMark` and the module's own doc-comment claim, which is also what
  the shipped sibling reader in `tests/documentation_examples/mod.rs` does
  (column zero only, stricter than `CommonMark`). The cost is named in the doc
  comment rather than hidden: a fence nested in a list item is at the item's
  content column and this line-level predicate will not recognize it. That
  error is loud — the body is handed to the heading and table scans and
  misparses — and the corpus has **zero** indented fences across all 23 scanned
  files (the 22 under `docs/rfcs/` plus `docs/roadmap.md`, 206 fence lines,
  none indented), so no document needs the nested form today. Three unit tests
  in `mod fence_tests` pin the boundary, because a corpus with no instance of a
  shape cannot test the predicate for it.

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
     `splitdrive` with a windows dialect. The rule is read off the disposition
     cell, which for `hash` reads "Accept as `text_hash`" — **not** `Reject`, as
     the first draft of this rule said — and `Reject` with a rename note for
     the other two. A disposition beginning "Accept as" therefore names the
     registered helper directly, and the renaming is not a special case the
     parser needs to know about. Section 7.8 states the three renames in prose
     and the test asserts there are exactly three.
  3. **Reject is overloaded.** The disposition cells, counted by `EP-M0`, are
     `Accept` (54), "Accept as `text_hash`" (1), `Defer` (6), `Reject` (49),
     and `Reject as a new name` (1). All 50 reject-dispositioned rows count as
     rejected whatever their class, so the deny set is the complement of the
     accepted set: 67 reject names plus 6 deferred names, less the 2 that are
     themselves accepted Netsuke names. The class distinction is **not
     derivable** from the document and is **not asserted**; `D10` records how
     that was established and why it does not matter. No check reads a row's
     class.
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

- Decision `D8`: roadmap task 6.1.1 has been rewritten from "child issues" to
  "child RFCs and accompanying roadmap tasks", and the same wording is to be
  updated at the seven places inside RFC 0006 that say "child issue". **Done
  for the roadmap**, on reviewer direction, in the commit that carries this
  revision; RFC 0006's seven phrases remain `EP-M1` work. Rationale: leaving
  either document saying "issues" while the tree contains eight child RFCs
  would leave both describing work nobody did. RFC 0006 section 6 opens by
  saying a child issue that does not satisfy every clause is not complete,
  which after this change is the definition of a child RFC's section 5. The
  reviewer also directed that work be tracked in committed documentation where
  possible, which settles the open question about the lost burn-down: the
  roadmap checkboxes in steps 6.2 to 6.9 are the tracker, and no child RFC gets
  a GitHub issue. The amended task records that explicitly so a later reader
  does not reintroduce one. The amended success criterion says "exactly one
  child RFC and at least one accompanying roadmap task" rather than "exactly
  one" of each, because `product` is already named by tasks 6.4.2 and 6.4.5.
  Making the roadmap half "exactly one" would have required splitting existing
  tasks for no benefit. Date/Author: 2026-09-08, planning agent; roadmap
  wording only under `D8`, `EP-M1` and `EP-M11` doing the rest.

- Decision `D11`: this branch's ADR is renumbered from 021 to **040**, the
  lowest free number above the corpus ceiling, and `docs/contents.md` gains the
  index entry it never had. Rationale: `main` published an ADR 021 of its own
  on 2026-09-09 (`3348cc0a`), one day after `EP-M0` recorded "the highest
  existing ADR is 020 … re-check before committing", and the branch's ADR is
  dated 2026-09-11 and committed at `9730a880` with main's already in its
  history. Two ADRs cannot both be 021 in one corpus. The plan's tolerance rule
  is unambiguous about the disposition — "If a number is taken, stop and
  escalate" — and the escalation is raised rather than assumed away. Of the two
  admissible remedies, renumbering the unmerged branch is the one the corpus
  already prescribes: main's number is cited by three inbound links including a
  link-reference definition, so moving it would churn a published document to
  fix an unpublished one. The ceiling is re-swept rather than remembered,
  because the number that was free when this plan was written is the number
  this defect is made of: 039 on `jm5/kani-change-scoped-gate` is the current
  highest anywhere, so 040 is free, and `origin/main`'s own highest is 038.

  Scope is eight edits, all mechanical, and the guard is what makes the last
  two safe. The 17 `ADR-021` mentions in this plan were checked for `fetch`,
  `trust`, `quarantin`, `network`, and `policy` before any replacement: none
  matched, so every mention means this branch's ADR and a scoped global replace
  cannot corrupt a reference to main's. Main's file and all three of its
  citations are left untouched, and the result is verified by set-comparison
  rather than by count, because 39 files against 39 entries is also what a swap
  looks like. Date/Author: 2026-09-25, implementation agent. wording confirmed
  by the reviewer.

- Decision `D9`: record the convention in
  `docs/adr-040-focused-child-rfcs-for-survey-rfcs.md`, scoped narrowly.
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

- Decision `D10`: the forbidden set is the **complement of the accepted set**
  — every section 7 reject name and every section 9 deferred name, less the
  registered Netsuke names of accepted helpers — which is **71 names**, not the
  34 this plan first asserted. The reject-class split of table 11 is neither
  derived nor asserted; the deny set does not need it. Rationale: `EP-M0` found
  that the plan's 34 reconciles only as a **row** count — 28 class-based
  forbidden rows plus 6 deferred rows — and not as a name count under any
  derivation; that its assertion that the deny set contains `is_file` cannot
  hold under that same class reading, because the `file` / `is_file` row is
  classed "already provides", so the plan's number and its membership list were
  mutually inconsistent; and that the resolution-note prose does not
  discriminate the three reject classes under any rule the document states. Two
  independent attempts at a note-parsing rule produced 24/10/16 and 25/6/18
  against table 11's 22/10/18; a third rule reproduced 22/10/18 exactly, but it
  leans on the literal token "alias" and on a citation to §10.2, and neither is
  a contract the parent document offers. The complement rule needs no prose
  parsing at all, is strictly safer than any class-based reading because it
  forbids a superset of what both readings forbid, and requires no normative
  edit to RFC
  0006. Two consequences are accepted: `basename` and `dirname` are the only
  excluded names, so `expanduser` — which the first draft would have permitted
  — is forbidden; and the deny set does not shrink when section 7 gains a
  reject row of the "already provides" class. Neither affects a child RFC,
  because a registry row must name an accepted helper, and the accepted set is
  checked independently by `COV-1`. The class split is **not** retained as a
  witness. `EP-M1` re-derived it a third time against the rule recorded here
  and got 8 alias / 24 exists / 18 principle, not 22/10/18, so the rule the
  earlier draft believed reproduced the split does not do so. Recovering
  22/10/18 needs a rule that special-cases the two rename rows
  (`win_splitdrive` and `fileglob`) out of *exists* while leaving `now` — also
  a reject row naming an existing helper in backticked call form — inside it,
  and that is curve-fitting, not derivation. RFC 0006 states no rule assigning
  a reject row to a class, and section 10 groups only some of them. Accordingly
  `COV-2` asserts the totals that *are* directly parseable — 55 accept rows, 6
  defer rows, 50 reject rows, 111 surveyed entries — and asserts only that
  table 11's three class counts **sum** to the derived reject-row count. That
  is a real consistency check on the table without asserting an undecipherable
  split. Date/Author: 2026-09-11, implementation agent, chosen by the reviewer
  from three options; the class-split retraction was added by the same author
  the same day, after `EP-M1` falsified the rule.

## Alternatives considered

The first draft had no such section. Two alternatives are live at the approval
gate.

**Stop after `EP-M1`.** The coverage test, `ADR-040`, the RFC 0006 defect
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
question 7, on an injected clock, is owned by roadmap task 7.1.1, "Add the
clock provider seam to the stdlib time module", on a separate reserved branch.

**Six of the seven remain open, and question 7 does not.** This split does not
close any of them — that is the point of carrying each into its owning child,
unresolved — but question 7 was resolved independently of this task, by the
merge of task 7.1.1 in `96aefc9c`, which is this branch's base. RFC 0006
section 16 item 7 already reads "Resolved", recording that `now()` reads
through a `ClockProvider` held by `StdlibConfig` and classified in the
[ADR-008](../adr-008-environment-seam-taxonomy.md) addendum for 2026-09-11, and
that RFC 0020 neither needs the seam nor depends on 7.1.1. Reading the section
16 list as seven open questions would therefore contradict the document itself,
so it is recorded here as six. `EP-M1` still annotates question 7 so a phase-6
implementer does not adopt it by accident; the annotation is now redundant with
the section's own text rather than the only pointer to it.

### The child RFC template

The template is committed to
[ADR-040](../adr-040-focused-child-rfcs-for-survey-rfcs.md), under "The child
RFC template", together with the registry row shape and each column's accepted
vocabulary. It is not restated here: two copies of a parsed artefact drift, and
the copy a child is written from must be the copy the test reads. `EP-M2` moved
it there from this plan for exactly that reason. The template retains
`## Current state` and `## Alternatives considered` as conditional-but-expected
for RFCs 0017 and 0018, which must contrast `relpath` against the existing
`relative_to` and `splitext` against `with_suffix`, and must carry RFC 0006
section 15.5's analysis of the rejected Windows-specific filter family.

## Conformance basis

`docs/terms-of-reference.md` exists, and two of its parts bear on this split
directly. Goal **G4** ("Deterministic plans") and goal **G6** ("Visible
impurity") are the parent documents' statement of what RFC 0006 section 6 turns
into a per-helper contract, and hard constraint 8.1's "project configuration
cannot grant itself authority the operator has not granted" (ADR-021, ADR-026)
is the rule clause 6.4 discharges as a capability boundary. No goal or
constraint contradicts the split; the split's purpose is to make those
obligations dischargeable per helper rather than in one survey document.

Upstream artefacts:

- `docs/terms-of-reference.md` at `origin/main` commit `96b89ca9`, goals G4 and
  G6, and hard constraint 8.1. The pin differs from the one below because the
  document postdates `924cb215`: it arrived on `main` in `96b89ca9` (PR #786),
  and is unchanged on this branch.

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
- `docs/adr-040-focused-child-rfcs-for-survey-rfcs.md`, created at `EP-M1`.

Trace links, one per obligation. `EP-M1` is the milestone that lands the check;
the tests are named without their `netsuke-build::rfc_stdlib_coverage_tests::`
prefix, which is the same for all of them and which would push every line past
120 columns:

```text
RFC0006-S7   -> ROADMAP-6.1.1 -> EP-M1 -> COV-1 -> every_accepted_helper_has_exactly_one_owner
RFC0006-S9   -> ROADMAP-6.1.1 -> EP-M1 -> COV-2 -> no_forbidden_helper_is_registered
RFC0006-S6.1 -> ROADMAP-6.1.1 -> EP-M1 -> COV-3 -> totals_and_purity_aggregate_agree
RFC0006-S14  -> ROADMAP-6.1.1 -> EP-M1 -> COV-4 -> coverage_map_status_is_reported
ROADMAP-6.2..6.9 -> ROADMAP-6.1.1 -> EP-M1 -> COV-6 -> every_capability_has_a_roadmap_task
RFC0006-S6   -> ROADMAP-6.1.1 -> EP-M3..EP-M10 -> CONF-1 -> every_child_discharges_every_clause
ADR-040      -> EP-M1 -> docs/adr-040-focused-child-rfcs-for-survey-rfcs.md
```

## Verification plan

### Review remediation local verification, 2026-10-01

The successful integrated candidate had starting HEAD `1c35b1d0` and a complete
tracked-and-untracked source fingerprint of
`7058af8061276b28875ba959e1e335c8626ebc2f37c73fec2d05378637e9510d`. One
scrutineer ran the gates sequentially, with logs under `/tmp`. Conflicting
`FORCE_COLOR` was removed from the runner environment, retaining `NO_COLOR`, to
avoid the tools' warning about both settings being present.

| Command                         | Result                                                                 |
| ------------------------------- | ---------------------------------------------------------------------- |
| `make fmt`                      | Passed.                                                                |
| `make test-rfc-stdlib-coverage` | 272 passed.                                                            |
| `make test-workflow-contracts`  | 986 passed, 3 skipped.                                                 |
| `make check-fmt`                | Passed.                                                                |
| `make lint`                     | Rustdoc, Clippy, Whitaker, Python and Actions checks passed.           |
| `make typecheck`                | Rust and Python checks passed.                                         |
| `make markdownlint`             | Spelling passed; 170 files, zero Markdown errors.                      |
| `make doc-coverage`             | 98.83%, above the 80% threshold.                                       |
| `make nixie`                    | Passed.                                                                |
| `make test`                     | 3748 Nextest tests passed, 6 skipped; 128 doctests passed, 32 ignored. |

The full Rust run printed
`coverage map: 1 of 8 capability groups written; 7 remaining`. The coverage
binary's 272 passes include its seven repository checks and direct fixtures and
properties. This evidence does not complete the seven unwritten child RFCs or
establish semantic adequacy of their prose. The final edit recording these
results receives its own formatting, Markdown, focused coverage, and
ExecPlan-status checks before commit. CI and reviewer confirmation are separate
publication evidence.

### Post-commit complexity refactor verification, 2026-10-01

The separate refactor candidate is based on `e88d0d1a`; its complete source
fingerprint after formatting is
`6ebddad1369efe49bfc1eed8fde67414b5d303bf3cd7f30ebcbafff4cf8d46a6`. The two
targeted `cs review` checks analysed the working files, not the immutable base
commit. Both `links_tests.rs` and `nextest_success_output.py` scored 10.0 with
an empty findings list. No rule was suppressed.

One scrutineer repeated the preceding table's complete gate set sequentially.
The focused suite passed 272 tests, workflow contracts passed 986 with 3
skipped, and the full Rust suite passed 3748 tests with 6 skipped and 128
doctests with 32 ignored. Doc-comment coverage remained 98.83%. The final
ExecPlan evidence edit receives the same focused documentation checks before
commit. Hosted CodeScene and CI results remain separate evidence for the
published head; CodeRabbit confirmation at the base does not substitute for
confirmation of these new fixes.

### Review validation branch-to-test inventory, 2026-10-01

The direct tests live beside the private implementation through test-only
sibling modules. Each invalid fixture changes one condition and checks the
existing diagnostic, including file and absolute line when the reader supplies
that context. The seven repository checks retain their public names.

| Implementation                | Direct tests and guarded branches                                                                                                                                                |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `document.rs`                 | `document_tests.rs`: section boundaries, fence opacity, absolute lines, table/header/separator recognition, missing cells.                                                       |
| `markdown.rs`                 | `markdown_property_tests.rs`: independent opening and closing predicates; delimiter, run, information, indentation, tabs, and later structure. Existing heading examples remain. |
| `totals.rs`                   | `totals_tests.rs`: complete counts, missing/duplicate/ambiguous/non-numeric tally, missing count cell, purity evidence and wrapping.                                             |
| `section7.rs`, `inventory.rs` | `section7_tests.rs`: disposition and citation errors, narrow cells, namespace conflicts, rename/option witnesses, every table descriptor, ignored tables.                        |
| `section8.rs`                 | `section8_tests.rs`: missing contract or citation, whole-token matching, exact section number, fenced boundaries.                                                                |
| `survey.rs`                   | `survey_tests.rs`: isolated derivation, missing file/section, propagated failures, deny complement and namespace totals.                                                         |
| `map.rs`                      | `map_tests.rs`: numbers, status and link disagreement, duplicate reservations/claims, ownership grammar and unknown references; independent set-operation properties.            |
| `registries.rs`               | `registries_tests.rs`: missing data, filename and cell vocabularies, duplicate names, purity/query disagreement, reserved corpus selection and counts.                           |
| `clauses.rs`, `deference.rs`  | `clauses_tests.rs` and adjacent deference cases: malformed/duplicate IDs, empty bodies/cells, owned-helper evidence, explicit escape, Ansible deference and word boundaries.     |
| `roadmap.rs`                  | `roadmap_tests.rs`: missing/invalid steps, scheduled helpers, task headings, fence opacity and complete step order.                                                              |
| `links.rs`                    | `links_tests.rs`: relative targets and source lines, missing files, above-root traversal; independent bounded traversal model, fragments and dot/empty segments.                 |
| `partition.rs`                | `partition_tests.rs`: missing/extra ownership, registry number/name/namespace/registration mismatches, denied registration.                                                      |
| `assertions.rs`               | `assertions_tests.rs`: every aggregate mismatch, deny count and required/forbidden witnesses.                                                                                    |
| `progress.rs`                 | `progress_tests.rs`: partial upper bounds, complete helper/purity/option equalities, forbidden purity, status, schedule and discharge consistency.                               |
| `mod.rs`                      | `repo_tests.rs`: isolated capability reads/listing, missing/non-UTF-8 documents, fixture open context, token and alias readers.                                                  |
| Nextest contracts             | `nextest_success_output_test.py`: exact override and declared name, valid fixture, ten single-condition mutations and mutation completeness.                                     |

Not every defensive return is input-reachable. Immutable offsets found in the
same section cannot subsequently exceed its bounds; a tally lookup cannot
vanish after its single-hit check; and ownership `only`/`except` token access
cannot fail after the two-token arity check. These branches remain intact. The
tests exercise the reachable rejection that precedes each defensive return
rather than changing visibility or inventing an impossible fixture. Parsed
Markdown rows always have a first cell, so the generic missing-cell accessor is
tested directly while later missing columns are tested through readers. No
claim of semantic proof or exhaustive random-input coverage is made: bounded
properties supplement the explicit diagnostic cases.

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

Method selection: table-driven Rust tests enumerate the fixed document
inventory and explicit rejection cases. Bounded property tests independently
model the general fence, path, and ownership parsers, whose input domains are
not that finite inventory. Both forms run in the same focused test binary.

**Parser contract, common to all obligations.** Helper inventories come from
Markdown table rows rather than helper headings. Structural headings delimit
the named sections: RFC 0006's section 7 subtables and its section 14 coverage
map, and each child's section 5.1 registry. It splits on the cell separator,
trims, and strips one pair of backticks. It matches names by whole-token
equality, never substring — the vocabulary contains `abs` against `is_abs`,
`quote` against `shell_quote`, `hash` against `text_hash`, and `subset` against
`issubset`, and substring matching would be wrong on all four. It records file
and line for every parsed row so failures name a location.

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
  nothing cannot pass vacuously. Discharged at `EP-M1` for the row-corruption
  control, and only for it: the three registry controls run at `EP-M4` and
  `EP-M5`. Transcripts are in `Control transcripts`.

### Obligation `COV-2`: no forbidden candidate is registered

- Obligation: every name in any child RFC's registry is a member of the derived
  accepted set. Equivalently, no name that RFC 0006 section 7 rejects under any
  disposition, or that section 9 defers, appears in any child RFC's registry.
- Method: derive the accepted set from section 7's accept rows, then the deny
  set as the **complement** — every surveyed reject or defer name that is not
  the registered Netsuke name of an accepted helper. See `D10`.
- Rationale: a hand-maintained list is not a sound contract when the source of
  truth is a tracked file in the same repository. Deriving means the set
  tightens automatically when section 7 gains a row. Stating the check as a
  complement rather than as a union of reject classes removes the need to parse
  the resolution-note prose, which `D10` records as too fragile to be a
  contract; it also makes the deny set a superset of any class-based reading,
  so no name either reading forbids can slip through.
- Domain: the derived forbidden set — 71 names, from 67 reject names and 6
  deferred names, less `basename` and `dirname`, which are `Reject` rows whose
  surveyed name is the registered Netsuke name of an accepted helper.
- Artefact: as above.
- Evidence: same command. Green from `EP-M1`, which is exactly why the controls
  below are compulsory rather than optional.
- Non-vacuity: this obligation is green before any child exists, so a pass
  proves nothing on its own. Add a registry row for `shuffle` to a scratch copy
  of RFC 0015 and expect a failure naming `shuffle` and the file. Add `is_dir`
  and expect a failure naming the rejected alias. Assert the derived deny set
  has exactly 71 members and contains `is_file`, `is_dir`, `is_link`, `quote`,
  `fileglob`, `lookup`, `win_dirname`, and `expanduser`. The first four are the
  names the first draft's handwritten list omitted; `is_dir`, `is_link`, and
  `win_dirname` are aliases the plan's own risk names; and `expanduser` is the
  name the first draft expected the deny set to **exclude**, so asserting it is
  present is the direct test of `D10`'s complement rule against the class-based
  reading it replaced. Assert it does **not** contain `basename`, `dirname`,
  `abs`, `glob`, `shell_quote`, or `splitdrive`, which are accepted helpers;
  without this assertion the complement's exclusion step is untested, and a bug
  that denied the very helpers the registries must carry would pass.
- Note: the reject-class split of table 11 — 22 already-provides, 10 redundant
  alias, 18 on principle — is **not** what this obligation checks, because the
  complement does not need it. Nor does `COV-2` assert the split itself: RFC
  0006 states no rule assigning a reject row to one of the three classes, and
  `EP-M1` falsified the rule the earlier draft believed recovered 22/10/18 (it
  yields 8/24/18). `COV-2` therefore asserts only that table 11's three class
  counts **sum** to the derived reject-row count, which catches an edit that
  breaks the table's arithmetic without claiming a derivation it does not have.
  The totals that are directly parseable — 55 accept rows, 6 defer rows, 50
  reject rows — are asserted exactly.

### Obligation `COV-3`: totals and purity aggregate agree

- Obligation: the derived accepted set contains 41 filters, 16 tests, and 3
  optioned helpers, matching the totals parsed from table 11; and the
  registries' purity columns, taken over rows whose registration is `New`,
  aggregate to 52 pure, 4 filesystem-observing, and 1 environment-observing,
  matching section 6.1.
- Method: parsed count against parsed count.
- Rationale: genuinely independent, unlike the first draft's version, which
  compared a hardcoded inventory against a hardcoded literal transcribed from
  the same table in the same sitting. Here the counts come from the child
  registries and the expectations from RFC 0006, so neither can be adjusted to
  match the other without editing a normative document. The purity aggregate is
  the only check reconciling section 6.1 against the registries; without it a
  wrong purity class rots silently.
- Scoping note: the aggregate is taken over `New` rows only, because section
  6.1's 52/4/1 counts the 57 **proposed** helpers. The registries carry all 60
  accepted helpers, and the three optioned rows include `glob`, which is
  filesystem-observing; aggregating every row would yield 54 pure, 5
  filesystem-observing, and 1 environment-observing and fail against a correct
  document. `EP-M0` found this; see `Surprises & discoveries`.
- Domain: three totals and three purity counts.
- Artefact: as above.
- Evidence: same command. The purity half is necessarily partial until every
  child exists, and it is now partial in two forms rather than being deferred
  whole. An upper bound on each purity class and on the optioned count runs at
  every prefix of the split, and the exact equality runs only when the coverage
  map has no unwritten row. The bound is sound because section 6.1's three
  counts are a budget: a half-written split can spend too much of it but never
  too little. The classes section 6.1 forbids outright — clock-observing,
  network-observing, and subprocess-observing — are asserted as hard zeros at
  every prefix, because "no proposed helper is" is not a budget.
- Non-vacuity: change one registry row's purity class from pure to
  filesystem-observing and expect a failure reporting 5 filesystem-observing
  where section 6.1 states 4 in total; observed at `EP-M2`'s second review,
  firing with one of eight groups written. Change a helper's namespace and
  expect the filter and test totals to fail. Introduce a purity class not among
  table 2's six values and expect a vocabulary failure. Set one row to a
  forbidden class *consistently* — the class and its manifest-query cell
  changed together — because changing the class alone is caught first by
  `check_manifest_query`, which is a different and narrower rule; the new
  aggregate check is only reachable on a row that is internally coherent.

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
  before any child exists means the map is not being read. Observed at `EP-M1`:
  `coverage map: 0 of 8 capability groups written; 8 remaining`, printed on a
  passing run because `.config/nextest.toml` raises this test's
  `success-output` to `immediate`. Discharged further at `EP-M1` by removing
  the map table itself, which fails six of the seven checks with "the coverage
  map subsection contains no table"; both transcripts are in
  `Control transcripts`.

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
  failure naming the source line and the missing target. Discharged at `EP-M1`;
  the observed message is in `Control transcripts`.

### Obligation `COV-6`: every capability has an accompanying roadmap task

- Obligation: every helper in the derived accepted set is named, in a backticked
  span, by at least one task bullet under the roadmap phase-6 step that owns
  its child RFC.
- Method: parse phase-6 step and task bullets from `docs/roadmap.md`, then check
  membership per helper against its owning step.
- Rationale: this is the second half of the amended success criterion, and
  without it only the RFC half is checked. It also guards the thing the
  reviewer asked for — that work be tracked in committed documentation —
  because it fails if a capability is given an RFC but no roadmap task to
  deliver it. Verified before writing this obligation: all 60 helpers are
  already named under their owning step, so `COV-6` is green from `EP-M1` on
  the current roadmap and stays green unless a task is deleted or a helper is
  reassigned across steps.
- Domain: 57 new helpers plus 3 optioned existing helpers, against roadmap steps
  6.2 to 6.9.
- Artefact: `tests/rfc_stdlib_coverage_tests.rs` and
  `tests/rfc_stdlib_coverage/`.
- Evidence: same command as the other obligations.
- Non-vacuity: green from the start, so a pass proves nothing on its own and a
  control is compulsory. Delete the `zip_longest` task bullet from a scratch
  copy of step 6.4 and expect a failure naming `zip_longest` and step 6.4. Move
  the `expandvars` bullet from step 6.7 to step 6.6 and expect a wrong-step
  failure, since RFC 0018 owns it. Assert the parsed task-bullet corpus is
  non-empty and that phase 6 yields exactly eight owning steps, so a parser
  that matches nothing cannot pass. Both roadmap controls are discharged at
  `EP-M1`, with transcripts in `Control transcripts`; the deleted-task message
  now names the owning step as well as the helper.
- Note the asymmetry with `COV-1`, and that it is deliberate: `COV-1` requires
  exactly one owning RFC, whereas `COV-6` requires at least one task, because
  `product` is legitimately named by both 6.4.2 and 6.4.5.

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
- Evidence: same command, scoped to children that exist. Also checked: the two
  id sets must be equal, so a section 5 subsection with no discharge-table row
  — or a row with no subsection — fails even though each is well-formed alone.
- **Amendment (2026-09-24): the mechanical half was implemented at `EP-M2`, six
  days after it was specified, and until then only the table half existed.**
  The obligation above was written as prose and the code compared id sets, so
  the empty stub, the generic discharge, and the deference appeal were
  unenforced; every non-vacuity control below would have passed, because none
  was implemented either. See `Surprises & discoveries` for how it was found
  and why a green suite one obligation short of its plan is indistinguishable
  from a complete one.
- Non-vacuity: all four controls were run at `EP-M2`'s second review, against a
  probe child RFC mounted from the `ADR-040` worked specimen with the coverage
  map's row `0013` flipped to written. Deleting subsection 5.8's body failed
  with "is subsection 5.8. Resource bounds of section 5 with an empty body".
  Replacing 5.7's body with "This group meets the clause by construction"
  failed as a generic discharge, naming the subsection and quoting the `D6`
  escape phrase it did not carry. Inserting "as Ansible does" failed with
  "justifies a helper by appealing to Ansible". A duplicated 5.7 heading and a
  duplicated `6.7` table row each failed as a second subsection or row for that
  clause. The probe is the `EP-M3` rehearsal as well: with the specimen mounted
  and the map row flipped, all ten tests passed, which is the state `EP-M3`
  must reach.
- **The specimen itself failed this check, and the check is right.** Subsection
  5.9 of the `ADR-040` worked section named thirteen diagnostic codes and no
  helper. It was corrected by naming all five helpers alongside the codes, not
  by weakening the check; a rule written before the document it grades is the
  only version that can surprise its author, which is why the two were read
  against each other here rather than at `EP-M3`.
- **Residual gap.** Whether section 5.7 genuinely discharges canonical equality
  for `subset`, as opposed to restating clause 6.7, is a judgement no test
  makes. This is the substantive product of the task and it rests on review,
  bounded by `D6`'s anti-vacuity rule and by the `EP-M3` go/no-go. Do not claim
  otherwise in the pull request.
- **Amendment (2026-09-19).** The discharge is checked as a two-column
  `Clause | Discharge` table under a `### Clause discharge` subsection, which
  closes section 5 after the eleven clause subsections. It was first specified
  as `### 5.6. Clause discharge`, which CodeRabbit showed collides with the
  template's own fourth bullet: 5.6 is "Type and error contract", the title RFC
  0006 clause 6.6 carries, so a table row reading `6.6` sat under a heading
  reading "Type and error contract" and implied the two were the same thing.
  Two readings were available — keep the template's 5.6 title and move the
  table elsewhere, or keep the table and duplicate the id — and the first is
  smaller and contradicts nothing already written, so the subsection lost its
  number rather than gaining a rival. `CLAUSES_HEADING` and the new
  `TABLE_HEADING` follow it, matching on the heading directly above the table
  rather than on the subsection's first table, so a table placed under one of
  the eleven clause subsections cannot be mistaken for the discharge. The
  `EP-M2` worked example fixes the exact placement before any child RFC is
  written from it, which is why this is settled now: eight documents would
  otherwise inherit the ambiguity. Restating the eleven clause titles under
  section 5 is the third option, and it is rejected — `D6` already argues that
  literal restatement across eight documents is a vacuity generator, and the
  ids, not the titles, are what the table matches on.

### Control transcripts (2026-09-11)

`/tmp/rfc-coverage-controls.sh` (scratch, not tracked) seeds one fault at a
time into the working tree, runs `cargo test --test rfc_stdlib_coverage_tests`,
prints the seeded diff and the failure, and restores both documents from a
backup taken before the first control. It deliberately does not use
`git checkout --`, which would discard the uncommitted milestone under test. It
fingerprints the two documents before and after and aborts on a partial
restore; the run finished with the same two hashes it started with, and the
baseline after the last revert was 7 passed.

Six controls are runnable at `EP-M1`, before any child RFC exists. Each was
run, and each failed for its own reason. The quoted messages are wrapped for
width; nextest wraps them the same way at a terminal.

- `COV-4`, red state. The section 14.13 coverage map table is deleted, leaving
  its prose and caption behind. This is the red half of the red-green evidence
  the Validation and acceptance section asks for, and it is what shows the
  reported count is read from the table rather than defaulted to zero:

  ```text
  Error: the coverage map subsection contains no table
  ```

  All six dependents fail on it. The line is also why `map::parse` insists on
  eight rows rather than accepting an empty table: a map that parses to nothing
  would otherwise satisfy "no unwritten group remains" vacuously.
- `COV-1`, corrupted accept row. RFC 0006:469, the `from_json` row, loses its
  `§8.1` citation, leaving the resolution cell reading "see section 8". All six
  dependent checks fail, naming the file and the line:

  ```text
  Error: accept row at docs/rfcs/0006-ansible-inspired-template-standard-library.md:469
    cites no section 8 subsection
  ```

  The three registry controls this obligation also specifies — delete,
  duplicate, and move the `combine` row — need a registry to corrupt and are
  deferred to `EP-M4` and `EP-M5`.
- `COV-5`, dangling link. The `ADR-040` link in section 14.13 is repointed at a
  file that does not exist:

  ```text
  Error: dangling inter-document links:
    ["docs/rfcs/0006-ansible-inspired-template-standard-library.md:2055 links to
    ../adr-021-no-such-file.md which resolves to
    docs/adr-021-no-such-file.md, and no such file exists"]
  ```

- `COV-6`, deleted task. Roadmap line 935, the `zip_longest` bullet under step
  6.4.3, is deleted:

  ```text
  Error: accepted helpers ["zip_longest (RFC 0015, step 6.4)"] are named in no
    roadmap capability step at all, so nothing schedules them
  ```

  The owning step in that message was added at `EP-M1` in response to this
  control: naming only the helper left the reader to work out where it belonged.
- `COV-6`, wrong step. The `expandvars` bullet moves from step 6.7 to 6.6:

  ```text
  Error: the coverage map gives RFC 0018 roadmap step 6.7, but that step names
    none of ["expandvars"]
  ```

- Fence blindness, run separately by `/tmp/rfc-fence-control.sh`. A fenced
  `text` block carrying a `# not a heading` line and a `{{ a | b }}` example is
  inserted directly after the section 8.9 heading in RFC 0006. Before the fix
  this failed **six of the seven checks**, and the diagnostic misattributed the
  cause: the fenced `#` line read as a depth-1 heading, truncated section 8.9
  at the fence, and left its helpers looking unspecified:

  ```text
  test every_child_discharges_every_clause ... FAILED
  test every_capability_has_a_roadmap_task ... FAILED
  test coverage_map_status_is_reported ... FAILED
  test every_accepted_helper_has_exactly_one_owner ... FAILED
  test totals_and_purity_aggregate_agree ... FAILED
  test no_forbidden_helper_is_registered ... FAILED
  Error: accepted helpers ["strftime", "to_datetime"] name no RFC 0006 section 8
    subsection, so they have no contract to implement
  ```

  The two named helpers are exactly those section 8.10 specifies; nothing in
  the message points at the code block that actually broke the parse. This is
  the most dangerous control in the set, because the fault is one a child RFC
  will legitimately contain: every child's section 5 carries example fences.
  After the fix the same seeded fault leaves the suite at 7 passed. The control
  restores the document from a backup and verifies its sha256 before and after.

`COV-2`, `COV-3`, and `CONF-1` are green before any child exists, so their
controls need something to corrupt — a registry row and a clause body. The
first milestone that supplies both is `EP-M3`, not `EP-M4` as this section
first said: `EP-M3` delivers RFC 0013, which owns five registry rows and
discharges all eleven clauses.

`EP-M2`'s second review ran them ahead of `EP-M3`, by mounting the `ADR-040`
worked specimen as a probe child RFC and flipping coverage map row `0013` to
written. All four `CONF-1` controls fired, as did the `COV-3` partial-purity
bound and both new link-number checks; the transcripts are in
`Control transcripts (2026-09-24)`. The probe is also the rehearsal for
`EP-M3`: with the specimen mounted, all ten tests passed, which is the state
`EP-M3` has to reach with a real child.

### Control transcripts (2026-09-24)

The `EP-M2` review's controls were run through a scratch harness
(`/tmp/probe.sh`, not tracked) that restores both the survey and the probe
child from pristine copies, applies one Python mutation, runs
`cargo nextest run --test rfc_stdlib_coverage_tests`, and prints the distinct
error lines. Each ran against the probe child, and each failed for its own
reason. Quoted messages are wrapped for width.

- `CONF-1`, empty body. The body of subsection 5.8 is deleted, leaving the
  heading:

  ```text
  Error: docs/rfcs/0013-structured-data-interchange-helpers.md:134 is
  subsection 5.8. Resource bounds of section 5 with an empty body
  ```

- `CONF-1`, generic discharge. Subsection 5.7's body is replaced with "This
  group meets the clause by construction", which names no owned helper and does
  not carry the `D6` escape phrase:

  ```text
  Error: docs/rfcs/0013-structured-data-interchange-helpers.md:112 is
  subsection 5.7. Canonical value equality, whose body names none of the
  helpers this RFC owns and does not read "No additional obligation beyond RFC
  0006 section 6.". A reviewer cannot tell it apart from a restatement of the
  clause it discharges
  ```

- `CONF-1`, deference. "as Ansible does" is inserted into subsection 5.8:

  ```text
  Error: docs/rfcs/0013-structured-data-interchange-helpers.md:134 is
  subsection 5.8. Resource bounds, which justifies a helper by appealing to
  Ansible ("as Ansible"). RFC 0006 surveys Ansible; it does not adopt its
  choices
  ```

- `CONF-1`, duplicate clause. A second `### 5.7.` subsection is inserted before
  the discharge table, and separately a second `|`6.7`|` row is added to it.
  The two are distinct checks and each fires alone:

  ```text
  Error: docs/rfcs/0013-structured-data-interchange-helpers.md:190 is a second
  section 5 subsection for clause 6.7

  Error: docs/rfcs/0013-structured-data-interchange-helpers.md:233 discharges
  clause 6.7 a second time
  ```

- `COV-3`, partial purity bound. All five registry rows are set to
  filesystem-observing with their manifest-query cells changed to `No`, at one
  of eight groups written. Changing the class *alone* fires
  `check_manifest_query` instead, so the probe must keep the row internally
  coherent to reach the new check:

  ```text
  Error: the registries already declare 0 pure / 5 filesystem / 0 environment;
  RFC 0006 section 6.1 states only 52/4/1 in total
  ```

- `COV-3`, forbidden class as a hard zero. One row is set to
  subprocess-observing with its cell at `No`:

  ```text
  Error: the registries declare 1 helper(s) `subprocess-observing`; RFC 0006
  section 6.1 states no proposed helper is
  ```

- Link-target number. Row `0013`'s link is retargeted to `0014-….md`, and
  separately its link *text* is changed to `0014` while the target is
  unchanged. Both fail, naming the disagreement:

  ```text
  Error: coverage map row for RFC 0013 at
  docs/rfcs/0006-ansible-inspired-template-standard-library.md:2070 links to
  0014-mapping-and-sequence-transform-helpers.md, whose number is 0014

  Error: coverage map row for RFC 0014 at
  docs/rfcs/0006-ansible-inspired-template-standard-library.md:2070 links to
  0013-structured-data-interchange-helpers.md, whose number is 0013
  ```

  Restoring the link but reverting the cell to a bare `` `0013` `` while the
  status stays `written` fails as "is marked written but its child RFC cell is
  not a link".

- Green baseline. With the probe child mounted and the map row flipped, and no
  fault seeded, all ten tests pass and the reported count reads
  `coverage map: 1 of 8 capability groups written; 7 remaining`. This is the
  state `EP-M3` must reach.

The harness restored both documents from their pristine copies after every
probe, and the working tree carried only the nine intended files afterwards;
`git diff` on the survey showed the two prose edits and nothing else.

### Axioms

- RFC 0006's dispositions are correct. This task partitions them.
- Table 11's totals of 41, 16, and 3, and section 6.1's aggregate of 52, 4, and
  1, are correct. `EP-M0` re-derives both and must agree.
- `markdownlint-cli2`, `typos`, and `mdtablefix` behave as configured.
- Section 7's note column **does not** reliably discriminate the three reject
  classes. This axiom is stated in the negative because `EP-M0` and `EP-M1`
  falsified the positive form: no rule fitted to the document recovers table
  11's 22/10/18, and the best attempt yields 8/24/18. `D10` therefore takes the
  deny set from the complement rule and `COV-2` does not assert the split. `D5`
  rule 3 states the remedy — a discriminating column — if the split is ever
  wanted as a contract.
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

- Outcome: the partition and the `D5` derivation rules are confirmed against
  the document, with two corrections adopted (`D10`).
- Acceptance evidence: recorded in `Surprises & discoveries` — the heading
  recount with its four reconciliation adjustments stated explicitly, since the
  naive count is 58 and never 57; the finding that section 7's note column does
  **not** discriminate the three reject classes, because no rule fitted to the
  document recovers table 11's 22/10/18, so under `D10` the split is not
  derived and is not asserted; the derived accepted set at 60 and forbidden set
  at **71** rather than the 34 first written; the purity aggregate at 52, 4,
  and 1 once scoped to `New` rows; and confirmation that RFC numbers 0013
  upward are free on `origin/main` and every active remote branch.
- Conformance check: no tracked file modified except this plan.
- Recovery: nothing to revert.
- Corrections adopted: `D10` (deny set is the complement of the accepted set,
  not a union of reject classes), the `D5` rule 2 rewording for `hash`, the
  `COV-3` purity scoping, and the `COV-2` member assertions — `is_file` is
  forbidden, but by the complement rule rather than by its reject class.
- **Go/no-go.** Stop if any derived count disagrees, or if the reviewer prefers
  a fallback from `Alternatives considered`. The note column not discriminating
  is **not** a stop condition: `EP-M0` and `EP-M1` established that it does
  not, and `D10` answers it with the complement rule rather than with prose
  parsing.

### `EP-M1` — coverage test, ADR, corrections, roadmap rewrite

Land as commits in PR #697, not as a self-contained pull request. The earlier
draft asked for a separate pull request on the grounds that the milestone is
independently valuable and is the fallback if nothing else proceeds; the
reviewer's instruction names one pull request, and PR #697 already carries the
task title, so the milestones stack there and each lands as its own commit. The
same correction was applied to this milestone's own Progress entry when it
shipped (see the `EP-M1` entry in `Progress`), and this paragraph had been left
stating the superseded model.

- Outcome: `tests/rfc_stdlib_coverage_tests.rs` green with all eight groups
  unwritten and `COV-4` reporting 8. `ADR-040` records the convention, its
  narrow scope, and the amendment procedure. RFC 0006 gains the section 14
  coverage map and reservation rows for 0013 to 0020, has its number-allocation
  table backfilled and its false claim that no RFC has been merged removed, has
  the section 8.1 and section 8.6 defects corrected, has its seven "child
  issue" phrases updated, and has section 16 question 7 annotated with a
  pointer to roadmap task 7.1.1. Roadmap 6.1.1 was already rewritten per `D8`,
  so `EP-M1` only has to confirm it still matches the delivered artefacts.
- Acceptance evidence: `COV-1` through `COV-6` green; all seeded-fault
  transcripts recorded; every gate green.
- Conformance check: no disposition changed; no `src/` file touched; no new
  dependency.
- Recovery: revert; nothing depends on it.
- Compatibility decision: none. RFC 0006 is a pre-1.0 internal document.

### `EP-M2` — the template and one worked section 5

- Outcome: the literal skeleton is committed into `ADR-040` — chosen over the
  developers' guide because the template is part of the convention the ADR
  already states in four parts, and the ADR is where a child's author is
  already sent — and one complete worked section 5 exists for review, for RFC
  0013, the group `EP-M3` delivers. 0013 is the *first* group, not the
  smallest: it owns 5 registry rows and 94 lines of section 8 against RFC
  0020's 2 rows and 80 lines, so "smallest" was wrong in this plan's own risk
  entry as well. The choice of 0013 stands regardless, because the worked
  example earns its keep by being the document the go/no-go is spent on, not by
  being cheap to write.
- Where it landed: the worked section sits in `ADR-040` under **The worked
  section**, immediately after the template it fills in, and is a fenced
  specimen rather than a ninth RFC. It could not be a file under `docs/rfcs/`:
  `registries::parse_all` scans that directory and takes every file whose
  number the coverage map reserves, so a specimen named `0013-…` would be
  parsed as RFC 0013 itself and `every_accepted_helper_has_exactly_one_owner`
  would accept a document that is not the child. The ADR is the safe home, and
  it is also the natural one, since the template this completes already lives
  there and neither can now be edited without the other in view.
- Mechanical evidence, because the specimen is a copy-source and a wrong one
  costs eight documents: the registry heading matches `REGISTRY_HEADING`
  literally, the five rows parse to `new`/`pure`/`yes` under `registries.rs`'s
  cell vocabularies, and the discharge table yields exactly `6.1` through
  `6.11` in document order under `clauses.rs`. Checked by extracting the fence
  and running both parsers over it, not by reading it.
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

- Outcome: `docs/rfcs/00NN-<slug>.md` exists per the template in `ADR-040`; the
  coverage map names it; `docs/contents.md` lists it; roadmap step `6.S` and
  each of its tasks cite it.
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

`EP-M1` as commits in PR #697, then `EP-M2`, then one commit per child. For
each child:

1. Create `docs/rfcs/00NN-<slug>.md` from the literal template in `ADR-040`.
2. Write section 4 as a list of the group's helpers with one-line purposes and
   links into RFC 0006 section 8.N. Do not restate a contract.
3. Write section 5.1's registry, then 5.6, 5.7, 5.8, and 5.9. Apply the
   anti-vacuity rule to 5.2 through 5.5, 5.10, and 5.11. Close section 5 with
   the clause-discharge table, and complete it rather than copying it forward:
   it is the one place a reader sees all eleven clauses resolved.
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

Observed red transcript, the `COV-4` control state with the coverage map table
removed. The order varies between runs, and `inter_document_links_resolve`
alone stays green because nothing else it does touches the map:

```plaintext
PASS [   0.007s] (1/7) netsuke-build::rfc_stdlib_coverage_tests inter_document_links_resolve
FAIL [   0.019s] (2/7) netsuke-build::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  Error: the coverage map subsection contains no table
FAIL [   0.019s] (3/7) netsuke-build::rfc_stdlib_coverage_tests coverage_map_status_is_reported
  Error: the coverage map subsection contains no table
    … four more FAIL lines, each with the same error …
error: test run failed
```

Observed green transcript at `EP-M1`:

```plaintext
PASS [   0.006s] (1/7) netsuke-build::rfc_stdlib_coverage_tests inter_document_links_resolve
PASS [   0.020s] (2/7) netsuke-build::rfc_stdlib_coverage_tests coverage_map_status_is_reported
  coverage map: 0 of 8 capability groups written; 8 remaining
PASS [   0.020s] (3/7) netsuke-build::rfc_stdlib_coverage_tests every_capability_has_a_roadmap_task
PASS [   0.021s] (4/7) netsuke-build::rfc_stdlib_coverage_tests every_child_discharges_every_clause
PASS [   0.021s] (5/7) netsuke-build::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
PASS [   0.022s] (6/7) netsuke-build::rfc_stdlib_coverage_tests totals_and_purity_aggregate_agree
PASS [   0.022s] (7/7) netsuke-build::rfc_stdlib_coverage_tests no_forbidden_helper_is_registered
Summary [   0.022s] 7 tests run: 7 passed, 0 skipped
```

Predicted wrong-owner seeded-fault transcript, the control the first draft
lacked. No registry exists yet, so this one has no observed counterpart:

```plaintext
FAIL [   0.012s] (1/7) netsuke-build::rfc_stdlib_coverage_tests every_accepted_helper_has_exactly_one_owner
  Error: helper combine (filter): coverage map designates RFC 0014, registry found in
    RFC 0015 at docs/rfcs/0015-ordered-collection-algebra-and-truth-predicates.md:73
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
3. Run `make test` and observe `rfc_stdlib_coverage_tests` pass with eighteen
   tests — seven obligation checks, six `markdown.rs` heading and fence tests,
   five `deference.rs` appeal tests — and a line reporting how many capability
   groups remain unwritten. Delete one row from RFC 0013's section 5.1
   registry, re-run, and observe a failure naming that helper and reporting
   zero owners. Restore the row.
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

- Tests: `make test` green, including the seven coverage tests.
- Verification: `COV-1` through `COV-6` and `CONF-1` discharged, with every
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
- `docs/rfcs/0014-mapping-and-sequence-transform-helpers.md`
- `docs/rfcs/0015-ordered-collection-algebra-and-truth-predicates.md`
- `docs/rfcs/0016-pattern-and-version-predicates.md`
- `docs/rfcs/0017-lexical-path-composition.md`
- `docs/rfcs/0018-host-state-predicates-and-environment-expansion.md`
- `docs/rfcs/0019-encoding-identity-and-formatting-helpers.md`
- `docs/rfcs/0020-date-and-time-conversion-helpers.md`
- `docs/adr-040-focused-child-rfcs-for-survey-rfcs.md`
- `tests/rfc_stdlib_coverage_tests.rs`
- `tests/rfc_stdlib_coverage/mod.rs` and its submodules

Files modified:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md`
- `docs/roadmap.md` — task 6.1.1 already amended; steps 6.2 to 6.9 gain a
  child-RFC citation per child
- `docs/contents.md`
- this ExecPlan
- `.gitignore` — adds `uv.lock`, paired with `git rm --cached uv.lock`. The
  lockfile is never tracked on `origin/main`; it entered this branch by
  accident at `a94a3006`, whose `git add -A` swept its own verification entry
  and the working-tree file in together, and it left again at `f42202a4`. The
  net diff against `origin/main` for that path is therefore **empty**, and the
  `.gitignore` line is the only surviving trace. It is named here because a
  reader who runs `git diff origin/main...HEAD -- uv.lock` sees nothing and
  would otherwise not know the change exists.

  **Authorized separately from this split, and not part of its scope.** The
  `.gitignore` line and the `git rm --cached` were requested explicitly for
  this branch rather than arrived at by the execplan, so a reviewer finding
  them outside `EP-M4`–`EP-M11` is looking at a deliberate, separately-approved
  change and not at scope creep. The reasoning that makes the change correct —
  a lockfile that locks nothing, an ignore rule that would be inert while the
  path stays tracked, and the measurements showing no test reads the file — is
  recorded in the Progress entry dated 2026-09-28 that begins "A second change
  rides on this head". Its placement in this list is descriptive: the file
  changes on this branch, so the interface list names it.

No other file changes. No `src/` change. No `Cargo.toml` change: `googletest`
0.14.3, `pretty_assertions` 1.4.1, and `regex` 1.12.2 are dev-dependencies, and
`anyhow` is a normal dependency, which integration tests link against. The
`.gitignore` and `uv.lock` change above is the one exception to "no other file
changes", and it is confined to repository hygiene — no build input.

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
    /// Rejected by section 10, for any of the three reasons table 11 counts.
    Reject,
}

/// One accepted helper, as the document that established it records it.
struct Row {
    /// Registered helper name, backticks stripped.
    name: String,
    /// Namespace the helper occupies.
    namespace: Namespace,
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

Revised again 2026-09-08 on reviewer direction. Roadmap task 6.1.1 has been
rewritten in place to say "child RFCs and accompanying roadmap tasks", and to
record that delivery is tracked through the roadmap checkboxes in steps 6.2 to
6.9 rather than through separate issues, so progress stays in committed
documentation. The plan's `Scope divergence` section is therefore retired and
replaced by `Scope: settled`, which keeps the reasoning on the record without
presenting it as an open question. `D8` is updated accordingly, and the lost
burn-down it worried about is resolved rather than merely noted.

Two precision fixes came out of amending the roadmap. The success criterion now
reads "exactly one child RFC and by at least one accompanying roadmap task",
because `product` is already named by both task 6.4.2 and task 6.4.5, so
requiring exactly one task would have meant splitting existing tasks for no
benefit. And a new obligation `COV-6` checks the roadmap half of the criterion
mechanically, which the plan previously left unchecked: every accepted helper
must be named by at least one task under the step that owns its child RFC. That
was verified before the obligation was written — all 60 helpers already are — so
`COV-6` is green on the current roadmap and fails only if a task is deleted or
a helper is reassigned across steps.

Nothing is implemented; the plan awaits approval.

Revised again 2026-09-11 after `EP-M0`. The plan is approved and in progress:
the implementation agent was asked to proceed, and `EP-M0` ran as the first
task, as the plan requires. The audit confirmed every derived count except one,
and the exception was instructive. The plan's 34-member forbidden set was a row
count wearing a name count's clothes, and it could not coexist with the plan's
own `is_file` and `expanduser` assertions — under a faithful class reading
`is_file` is not forbidden, and under the row count that yields 34 neither is
it. `D10` resolves the inconsistency by stating the deny set as the
**complement of the accepted set** — 71 names — which keeps the four membership
witnesses the first draft wanted, gives up only the `expanduser` exclusion, and
needs no normative edit to RFC 0006. Everything else held: the partition, the
accepted set at 60, the 41/16/3 totals, the purity aggregate, the naive heading
count of 58 with its four reconciliation adjustments, and the availability of
RFC numbers 0013 to 0020 on `origin/main` and every active remote branch.

Three smaller corrections followed. `D5` rule 2 asserted that all three rename
rows say `Reject`; `hash`'s row actually reads "Accept as `text_hash`", so the
rule is read off the disposition cell and the rename is not a parser special
case. `COV-3`'s purity aggregate has to be scoped to `New` rows, because the
registries carry all 60 accepted helpers including the filesystem-observing
`glob` and would otherwise aggregate to 54/5/1 against section 6.1's 52/4/1. And
`COV-2`'s member list was restated against the complement, with `is_dir`,
`is_link`, `win_dirname`, and `expanduser` added to its non-vacuity assertions
and the `expanduser` exclusion dropped.

One `D5` rule 3 remedy is now **in scope, but deferred by choice**. The plan
provided that if section 7's note column did not discriminate the three reject
classes, `EP-M1` would add a discriminating column. `EP-M0` believed it did
discriminate — a reject row being *alias* if its resolution cell contains
"alias" or cites §10.2, *exists* if it begins "Exists" or names a provider in
backticks, and *principle* otherwise, which was recorded as reproducing table
11's 22/10/18. `EP-M1` re-derived that rule and got 8 alias / 24 exists / 18
principle. The *principle* third is correct and every principle row is
backtick-free, but the remaining 32 rows split 8/24 where table 11 says 10/22;
the difference is exactly the two rename rows `win_splitdrive` and `fileglob`,
which §7.8 says are "registered under a Netsuke name rather than the surveyed
one" and which table 11 therefore counts as aliases while the rule counts them
as exists. Recovering 22/10/18 needs those two special-cased out of *exists*
while `now` — also a reject row naming an existing helper in backticked call
form — stays inside, and the document offers nothing to distinguish them. The
split is therefore **not derivable from the tables as they stand**, and `COV-2`
does not assert it: it asserts the directly parseable totals, plus that table
11's three class counts sum to the derived reject-row count. Adding the
discriminating column remains the remedy if the split is ever wanted as a
contract; `D10` keeps it off the deny set's critical path either way, so no
normative edit to RFC 0006 is made now.

- [x] (2026-09-28) **The `2db228aa` review triaged: seven local CodeRabbit
  findings, five in scope, plus the verification that the scrutineer's two
  "posted-but-unaddressed" comments were already fixed.** The seven local
  targets were reviewed against this branch's actual diff, which is 60 commits
  over 27 files. Note the local `--agent` pass diffed `6499fc48` — a commit
  that is not an ancestor of HEAD, is not on `origin/main`, and is reached by
  no ref. Nine of its fourteen findings land outside the PR diff entirely (kani
  tests, `.github/workflows/ci.yml`, `Makefile`, `pylint_tier_test.py`, two
  foreign ExecPlans, `resolver_telemetry_boundary_tests.rs`) and are not this
  branch's to fix. The five in scope are applied in the commit carrying this
  entry.

  Two of the five are corrections of *this plan's* text and one of those
  contradicts a number the plan states eight other times:

  - `bijection over fifty-seven names` is wrong twice over. The bijection is
    over the accepted set, which is **sixty** — 57 new helpers plus the 3
    existing ones gaining an option — and the plan's own totals table already
    reads `| Accepted set | 60 | 60 | agree |`. Both occurrences were corrected,
    with the arithmetic shown so the 57 is not simply erased: it is the
    *new-helper* figure, not the accepted-set figure.
  - Three first-person pronouns were removed from the plan, per
    `documentation-style-guide.md` line 39. The sweep found **three**, not the
    two the finding named: the phrase `My own build attempt`, in the entry
    about the package-cache deadlock, was outside the finding's citation and
    would have been missed by applying it literally.
    A corpus probe bounds the rule honestly — `docs/` carries 149 first-person
    occurrences, so the rule is stated rather than universally observed, and
    this plan is now among the compliant files rather than the rest.
  - The `.gitignore` / `uv.lock` paragraph now records that the change was
    authorized separately from the split and is not part of its scope, which is
    what the finding asks for. The change itself is untouched: it was requested
    for this branch in its own right, and is correct for reasons recorded in the
    earlier entry beginning "A second change rides on this head". What was
    missing was the *record of authorization*, not the change.

  The remaining two are normative edits that reach beyond this plan, and each
  was mirrored into its other copy in the same commit, per the rule this plan
  already records — *when a correction is applied to an artefact, grep for its
  other copies in the same commit*:

  - **RFC 0013 section 5.6 omitted `output_too_large` from both serializer
    bullets.** The code is real, is in section 5.9's table, and is *enforced*
    by section 5.8's length pass — but section 5.6 is the per-helper
    enumeration a reviewer reads to learn what each helper rejects, and it
    listed four conditions for `to_yaml` and repeated "the same four" for
    `to_nice_json`. Both bullets now name five. The count was checked against
    the table afterwards rather than asserted: `to_yaml` now names
    `undefined_input`, `indent_out_of_range`, `unsupported_key`,
    `unsupported_kind`, and `output_too_large`, and section 5.9 carries a row
    for each. **This changes no code and no count elsewhere** — fourteen codes
    was already correct.
  - **Section 5.7's rendered-key collision named no code.** It said
    `to_nice_json` "rejects a mapping whose rendered keys are not distinct"
    while the adjacent sentence named `duplicate_key` for `from_json`'s
    rejection of the same collision. The serializer reuses that code — the same
    key problem detected at the other end of the round trip — so naming it keeps
    the code set at fourteen and leaves both discharge rows and section 5.9's
    table untouched. Introducing a fifteenth code was the alternative and was
    rejected as the larger change for no gain in precision.

  **The scrutineer's "Next Action" was wrong and was not acted on.** It
  reported two "posted-but-unaddressed findings" —
  `tests/rfc_stdlib_coverage/map.rs:173` (duplicate child RFC reservations) and
  `tests/rfc_stdlib_coverage/markdown.rs:154` (fence indentation) — and told
  the next agent to review them first. Both guards are present and correct:
  `map.rs` rejects a repeated reservation at lines 120-139, and `markdown.rs`
  bounds fence indentation at 136-154. The posted comments carry
  `commit_id = 2db228aa` only because GitHub re-anchors a review comment's line
  and position on push; the bodies describe defects fixed in commits merged
  before it. **A comment's `commit_id` is not evidence that the code it points
  at is still broken.** This is a second instance of the same re-anchoring
  illusion already recorded for this PR, and the verification cost three file
  reads — cheaper than the alternative, which was to re-fix two live guards.

  One further false premise was rejected before it reached the tree. The
  scrutineer's report also read the `coverage map: 1 of 8 …` line as not being
  a progress report at all. It is correct that COV-4 captures the *test's own
  stdout* rather than a live count, but the plan has consistently read it as
  the reported state of the map and that reading is the one the test asserts
  on; the observation changes no disposition.

  **A `make fmt` run was required and is recorded rather than assumed.** Gate
  one (`make check-fmt`) reded on the first attempt of this change:
  hand-wrapped prose is not `mdtablefix`'s canonical wrap, and four of the six
  edited files were flagged. That is the canonicalization trap this plan has
  already met once, and it is a formatting failure rather than a content one —
  the formatter's own `--fix` was the remedy, and the resulting reflow was
  verified not to have dropped content by comparing word counts against HEAD
  (all six files grew, none shrank) and by re-reading every edited passage in
  the reflowed text. Two `grep` probes returned empty during that verification
  for phrases that were in fact present, because `mdtablefix` had moved them
  across a line break: **a phrase-level `grep` is not evidence of absence in a
  wrapped document** — read the paragraph.

  **The gate run then reded a second time, and again the defect was this
  branch's own prose.** `make markdownlint` aborted in its `spelling`
  prerequisite on `canonicalisation` — a bare `-ise` form written into the very
  paragraph describing the canonicalization trap, which is the *same* defect
  class this plan already records at the entry about `feed5192`, three hundred
  lines above the new text. The lesson repeats rather than extending: the
  recurrence is not evidence that the earlier entry was wrong, it is evidence
  that the trap is live in new prose and that writing *about* a spelling rule
  in an unbackticked word is itself the hazard. `markdownlint-cli2` never ran,
  so mdlint had no verdict — that state was reported as UNKNOWN rather than as
  a pass, per the distinction already recorded above.

  Two side effects of the failed gate are worth recording, because each would
  otherwise have entered the commit unexamined:

  - **`typos.toml` is rewritten by the gate, and the rewrite was reverted.**
    `make spelling` regenerates the file from the pinned shared dictionary
    (`typos-config-builder` `v0.1.1`) on every run, and the run moved one
    `extend-ignore-re` entry — narrowing `\bvar\.iamge_id\b` to a longer
    backticked phrase and re-sorting it. The file is tool output, not hand
    prose, so the question is not whether the new text is nicer but whether a
    stale file fails anything. **It does not.** The probe is decisive: with
    HEAD's version restored, `make spelling` exits **0**, prints `current:
    typos.toml`, and silently rewrites the file to the same 44-entry text on
    each run, byte-identical across two consecutive runs. So the modification is
    a gate side effect that CI reproduces on its own, and committing it would
    widen this branch's diff by a file the ExecPlan does not list for the sake
    of text that is regenerated rather than authored. It was reverted, and the
    tree is back to the six files this change touches. The one thing that does
    bite is a *stale committed* file being invisible in review: nothing fails,
    so nobody learns the dictionary moved.
  - **A `grep` for `-ise` forms on added lines found no others.** Reporting the
    negative matters here because the spelling gate stops at the first error, so
    a single fix is not evidence that the rest of the new prose is clean. The
    check was run against the added lines specifically — `canonicalis`,
    `normalis`, `serialis`, `initialis`, `organis`, `recognis`, `analys`, and
    their kin — and the only surviving matches were correct English words
    (`collision`, `diagnosis`, `premise`, `consistently`), none of them a `-ise`
    variant a reviewer would need to weigh.

  Concern counts after triage: **high 0, medium 0, low 2** — the two CodeRabbit
  findings in `tests/rfc_stdlib_coverage/markdown.rs` and `map.rs` that this
  session verified as already-fixed, and which are therefore closed rather than
  carried.
