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
  document rewrite. `ADR-021` carries the amendment procedure named in `D9`.

- Risk: the reviewer concludes the split is not worth its cost.
  Severity: medium. Likelihood: medium. Mitigation: `Alternatives considered`
  states the fallback plainly, and `EP-M0` and `EP-M3` are both go/no-go points
  before most cost is incurred.

## Progress

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
- [x] (2026-09-11) `EP-M1` Land the coverage test, `ADR-021`, the RFC 0006
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
  both landed in `ADR-021`. The template left this plan for the ADR rather than
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
  ADR-021 claimed the test "transcribes none of them" when it carries four
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
  `ADR-021`'s doubly-listed `duplicate_key`). Two were applied against a *false
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
  fourteen.
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
     themselves accepted Netsuke names. `D10` records why the class
     distinction, while derivable, is deliberately not load-bearing.
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
  wording confirmed by the reviewer.

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

- Decision `D10`: the forbidden set is the **complement of the accepted set**
  — every section 7 reject name and every section 9 deferred name, less the
  registered Netsuke names of accepted helpers — which is **71 names**, not the
  34 this plan first asserted. The reject-class split of table 11 is still
  derived and asserted, but it is not load-bearing for the deny set. Rationale:
  `EP-M0` found that the plan's 34 reconciles only as a **row** count — 28
  class-based forbidden rows plus 6 deferred rows — and not as a name count
  under any derivation; that its assertion that the deny set contains `is_file`
  cannot hold under that same class reading, because the `file` / `is_file` row
  is classed "already provides", so the plan's number and its membership list
  were mutually inconsistent; and that the resolution-note prose does not
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

### The child RFC template

The template is committed to
[ADR-021](../adr-021-focused-child-rfcs-for-survey-rfcs.md), under "The child
RFC template", together with the registry row shape and each column's accepted
vocabulary. It is not restated here: two copies of a parsed artefact drift, and
the copy a child is written from must be the copy the test reads. `EP-M2` moved
it there from this plan for exactly that reason. The template retains
`## Current state` and `## Alternatives considered` as conditional-but-expected
for RFCs 0017 and 0018, which must contrast `relpath` against the existing
`relative_to` and `splitext` against `with_suffix`, and must carry RFC 0006
section 15.5's analysis of the rejected Windows-specific filter family.

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
  probe child RFC mounted from the `ADR-021` worked specimen with the coverage
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
  5.9 of the `ADR-021` worked section named thirteen diagnostic codes and no
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
- `COV-5`, dangling link. The `ADR-021` link in section 14.13 is repointed at a
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

`EP-M2`'s second review ran them ahead of `EP-M3`, by mounting the `ADR-021`
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

Ship as a self-contained pull request. It is independently valuable and is the
fallback if nothing else proceeds.

- Outcome: `tests/rfc_stdlib_coverage_tests.rs` green with all eight groups
  unwritten and `COV-4` reporting 8. `ADR-021` records the convention, its
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

- Outcome: the literal skeleton is committed into `ADR-021` — chosen over the
  developers' guide because the template is part of the convention the ADR
  already states in four parts, and the ADR is where a child's author is
  already sent — and one complete worked section 5 exists for review, for RFC
  0013, the group `EP-M3` delivers. 0013 is the *first* group, not the
  smallest: it owns 5 registry rows and 94 lines of section 8 against RFC
  0020's 2 rows and 80 lines, so "smallest" was wrong in this plan's own risk
  entry as well. The choice of 0013 stands regardless, because the worked
  example earns its keep by being the document the go/no-go is spent on, not by
  being cheap to write.
- Where it landed: the worked section sits in `ADR-021` under **The worked
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

- Outcome: `docs/rfcs/00NN-<slug>.md` exists per the template in `ADR-021`; the
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

`EP-M1` as its own pull request. Then `EP-M2`, then one commit per child. For
each child:

1. Create `docs/rfcs/00NN-<slug>.md` from the literal template in `ADR-021`.
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
3. Run `make test` and observe `rfc_stdlib_coverage_tests` pass with fourteen
   tests — seven obligation checks, three heading tests, four deference tests —
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
- `docs/adr-021-focused-child-rfcs-for-survey-rfcs.md`
- `tests/rfc_stdlib_coverage_tests.rs`
- `tests/rfc_stdlib_coverage/mod.rs` and its submodules

Files modified:

- `docs/rfcs/0006-ansible-inspired-template-standard-library.md`
- `docs/roadmap.md` — task 6.1.1 already amended; steps 6.2 to 6.9 gain a
  child-RFC citation per child
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
