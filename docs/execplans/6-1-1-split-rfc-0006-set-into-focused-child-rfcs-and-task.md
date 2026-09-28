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
  document rewrite. `ADR-040` carries the amendment procedure named in `D9`.

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
  review — "your pull request is larger than the review limit of 150,000 diff
  characters" — so it carries no findings and no verdict to clear.

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
- [ ] (2026-09-28) **BLOCKER: the shared Cargo package cache is deadlocked
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
  job on the machine, mine included, was stalled behind it.

  This was diagnosed and left alone deliberately. The house rule says not to
  kill other agents' processes, and the holder belongs to another session; the
  system prompt's remedy for a full or wedged cache is to stop and tell the
  user, not to break someone else's lock. The `podbot` job is also
  self-inflicted in the sense that matters here — it is a nested-Cargo deadlock
  of the kind this repository has hit before (see the nested-Cargo timeout
  records), not a cache that merely needs to drain.

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
  `pgrep -c rustc` or the granted-lock line disappearing from `/proc/locks`. My
  own build attempt was blocked on the lock for its whole life, not failing,
  and was stopped rather than left queued: a queued waiter is itself one more
  entry in the 45, which makes everyone else's diagnosis noisier.

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

Ship as a self-contained pull request. It is independently valuable and is the
fallback if nothing else proceeds.

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

`EP-M1` as its own pull request. Then `EP-M2`, then one commit per child. For
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
- `docs/adr-040-focused-child-rfcs-for-survey-rfcs.md`
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
