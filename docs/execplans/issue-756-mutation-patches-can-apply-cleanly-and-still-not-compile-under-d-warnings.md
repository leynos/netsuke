# Add a compile gate to the Kani mutation evidence contract

Status: IN PROGRESS

Re-opened on 2026-09-22. The plan was `COMPLETE`; the ready pull request's
CodeRabbit pass then found that the gate's checker could not see the code the
mutations touch, which invalidated the gate's design and its CI placement
together. Of the two conditions for returning to `COMPLETE`, the first is now
met: the moved gate is validated on CI (run `35796798133`, the first of five
runs to reach it, compiling all 18 patches in 361 s, with all five jobs of that
workflow green). The second is outstanding, and its shape has since been
measured rather than assumed.

The review history here was rewritten on 2026-09-26 and the earlier reading is
kept rather than deleted, because the way it went wrong is instructive. On
2026-09-23 the GitHub CodeRabbit review had **auto-paused** — its own comment
on PR `#766` read "Reviews paused … this branch is under active development" —
and the record correctly described that state, including the fact that the
`CodeRabbit` commit status was reporting the pause path rather than a review.
What it then inferred was that the pause would hold: *"no inline comment on
this pull request is newer than `21:47:53Z`"* was true when written and false
by 2026-09-25, and the *"lingering `CHANGES_REQUESTED`"* pinned to `e4f93b93`
acquired a second sibling pinned to the head of the day. A description of a
state that a later event overturns is a forecast wearing the clothes of an
observation, and this one was written as an observation. The lesson is not to
avoid describing transient state — the pause was real and worth recording — but
to date the claim and name what would falsify it, so that the reader can tell
which of the two they are holding.

As of 2026-09-26 the pull request has **two** `coderabbitai[bot]` review events:
`changes_requested` on `e4f93b93` at `2026-09-21T21:47:54Z`, and
`changes_requested` on `d8d0ba6d` at `2026-09-25T23:35:01Z`. The second named
two findings, both actioned — a first-person pronoun in this document, and the
gate's use of the developer's working checkout. Clearing or waiving a
`CHANGES_REQUESTED` review remains a maintainer action, not something a branch
can do; what the branch owes the record is a fresh pass on the corrected head.

Four local `coderabbit review --agent` passes have since run against corrected
heads and are the branch's own evidence that the findings are cleared: one
against `ebd70e4e` returning seven findings over five sites; one against
`674e266f` clearing all five of those sites with coverage `33/33` and raising
two `trivial` duplications of its own; one against `c1497d4c` clearing both of
those with coverage `3/3` over the changed set and raising one `minor`
first-person finding in this document's own new prose; and one against
`03780568`, which returned **zero findings** at coverage `33/33`. The last is
the branch's clearest evidence, and its zero was checked for liveness rather
than trusted: the same whole-document sweep that finds nothing on `03780568`
finds exactly the two cited sites (`I` and `me`) on `c1497d4c`, so the sweep
can see what it is looking for.

These are distinct from the GitHub review events counted above —
`review --agent` reads the local workspace, not the pull request — so the count
of two `coderabbitai[bot]` review events stands, and a green local pass does
not clear a `CHANGES_REQUESTED` review. The GitHub review is requested with a
new top-level comment once the gates are green and the fixes are pushed.

This ExecPlan is a living document. The sections `Progress`,
`Surprises & discoveries`, `Decision log`, and `Outcomes & retrospective` must
be kept up to date as work proceeds.

## Purpose / big picture

Three tracked mutation patches under `docs/verification/mutations/` applied
cleanly and passed their contract test, but the tree they produced could not
compile. They therefore contributed zero mutation evidence while appearing
healthy.

**Mechanism.** `make kani-full` gained `RUSTFLAGS="… -D warnings"` on
2026-09-18 (commit `2c030fd1`, PR `#714`). Each of these patches seeded its
fault by leaving a binding or helper unused — a warning when written, a hard
compile error under `-D warnings`. The patched tree failed to build, so
`cargo kani` never reached the harness.

**Why the contract test missed it.**
`tests/kani_mutation_evidence_tests.rs::every_patch_applies_cleanly` only runs
`git apply --check`. That proves a patch *applies*; it does not prove the
patched tree *compiles*.

Outcome: the three patches seed the same faults without dead code, and a new
gated contract test compiles each patched tree under `-D warnings`, so this
failure mode cannot recur silently.

## Constraints

- 400-line cap (Whitaker `module_max_lines`) applies to every Rust module,
  including `tests/*.rs`. The existing test file is 383 lines, so the new test
  needs a split.
- `tests/makefile_test_target.rs` holds `NEXTEST_TARGETS` to exactly the targets
  invoking `nextest run`. A new nextest-invoking target must join both that
  list and the worker-bound contract.
- `tests/makefile_test_target/rustflags.rs` requires every `RUSTFLAGS="`
  assignment in the Makefile to be one of four contracted variables.
- `tests/workflow_contracts/nextest_child_cargo_group_test.py` auto-discovers
  build-capable child `cargo` tests and fails if any is missing from
  `.config/nextest.toml`'s `nested-cargo-builds` group.
- `tests/integration_test_wiring_tests.rs` requires each `tests/*/mod.rs` tree
  to be declared by a Cargo-discovered `tests/*.rs` target.

## Progress

- [x] (2026-09-21) Reconnaissance: file layout, contract tests, CI lanes.
- [x] (2026-09-21) Regenerate the three rotted patches; validate each by
      applying, compiling under `-D warnings`, running the owning harness to a
      named failure, reverting, and observing success.
- [x] (2026-09-21) Add the `#[ignore]`-gated compile contract test as
      `tests/kani_mutation_evidence_tests/compile_guard.rs`, declared from the
      parent by an explicit `#[path]` (the parent is near the 400-line cap).
      Liveness proven: on the pre-adoption tree it failed and named exactly the
      two `#755`-owned patches.
- [x] (2026-09-21) Adopt PR `#755`'s two repairs verbatim, so the gate is green
      here without waiting for an unmerged PR and the two branches cannot
      conflict whichever lands first. `#755` merged on 2026-09-22 (`ee0e5523`),
      before this branch's own base, so the two files now arrive from `main`
      and the adoption was dropped as redundant when rebasing onto `30c50e27`.
      The blobs are byte-identical to `#755`'s repair commit (`3a282018`)
      either way; only the provenance changed.
- [x] (2026-09-21) Wire it into `.config/nextest.toml` (the
      `nested-cargo-builds` group), the Makefile (`test-kani-mutations`, with
      both worker bounds, plus `.PHONY` and `NEXTEST_TARGETS`), and CI
      (`build-test`).
- [x] (2026-09-21) Update `docs/developers-guide.md`.
- [x] (2026-09-22) Confirm the third CodeRabbit finding and act on it: the
      gate's `cargo check` is blind to `#[cfg(kani)]` code, so it is replaced by
      `cargo kani --only-codegen`, which is a strict superset.
- [x] (2026-09-22) Move the gate from `build-test` to `kani-smoke`, because
      `build-test` has no Kani at all and cannot run a Kani-frontend compile at
      any price. Contracts updated in step with the move:
      `tests/workflow_contracts/workflow_loading.py` (`NEXTEST_JOBS`),
      `runner_shape_test.py` (the job's worker bound), and `tests/workflow_ci.rs`
      (the cold-run ceiling).
- [x] (2026-09-22) Update `docs/developers-guide.md` again for the move: the
      cost sentence, the `build-test`-runs-it claim, the executed-test-set
      table row, and a new passage on why the gate needs the Kani frontend.
- [x] (2026-09-22) Fix the defect CI found on the gate's first real run: drop
      `kani-smoke`'s `RUSTUP_TOOLCHAIN: stable`, which pinned the gate's
      `cargo nextest` to a toolchain that rejects `GATE_RUSTFLAGS`'s
      nightly-only `-Zthreads=8`. Mechanism reproduced locally before and after.
- [x] (2026-09-22) Fix the *second* CI run, which failed identically because
      removing the variable had not been the whole fix. The step's `toolchain`
      input was changed to name the nightly pin, matching `build-test`.
- [x] (2026-09-23) Fix the *third* and *fourth* CI runs, which failed
      identically again — the change above had no effect because the name never
      resolved. `kani-smoke` read `${{ env.NETSUKE_RUST_TOOLCHAIN }}` without
      declaring the variable, and `env` resolves per job, so it expanded to the
      empty string: the action took its empty-input arm (`override: false`),
      and the stale stable entry a restored `.kani-rustup` carries kept
      selecting stable. The pin is now declared in `kani-smoke`'s own `env`
      block. The regression guard is new and both of its directions were
      falsified before being trusted; see the surprise below for the two
      mechanisms this displaced.
- [x] (2026-09-22) Rebase onto `origin/main` once `#732` landed, resolving three
      conflicts: the `test(=NAME)` filter this branch registered is converted to
      the anchored grammar `#732` enforces (allow-listed by
      `nextest_child_cargo_group_invariants.py`, which rejects the old form), and
      `build-test` keeps `#732`'s anchored-filter verification step while
      dropping the gate block that the later `1cd39169` relocates to
      `kani-smoke`. All branch-authored files verified byte-identical across the
      replay by whole-tree diff, not ancestry.
- [x] (2026-09-23) Confirm the gate runs and compiles on CI, and read its real
      cost and the job's headroom against the 30-minute ceiling. Run
      `35796798133` at `7143648c` is the first to reach the gate: `kani-smoke`
      concludes `success`, the compile gate runs **361 s**
      (`23:25:40Z → 23:31:41Z`) and nextest reports
      `Summary [ 257.703s] 1 test run: 1 passed (1 slow), 3 skipped` — the first
      nextest summary this job has ever produced, the four earlier runs having
      aborted before compiling a patch. The job totals **776 s** against its
      1,800 s ceiling, leaving 1,024 s unspent. The toolchain resolves as
      intended: `toolchain: nightly-2026-08-23`, the action's `override: true`
      arm, and `rustc 1.100.0-nightly (c54751567 2026-08-22)`.
- [x] (2026-09-23) Repeat that confirmation on the final head, so the gate's
      cost is a property of the change rather than of one runner. Run
      `35800582469` at `2be420d1` is read from the job log, not the job's
      conclusion: the gate step runs `00:13:02Z → 00:18:43Z` (**341 s**),
      nextest reports `PASS [ 261.514s] (1/1) … compile_guard::every_patched_tree_compiles_under_denied_warnings`,
      and the active toolchain prints
      `nightly-2026-08-23-x86_64-unknown-linux-gnu unchanged - rustc 1.100.0-nightly`.
      The two runs agree within 6 % on different heads from cold caches.
- [x] (2026-09-23) Give the gate a per-test allowance sized for its cost, after
      a third CI run showed the default 300 s was not one. Run `35802301416` at
      `b787475f` failed `kani-smoke` with
      `TIMEOUT [ 300.008s] … every_patched_tree_compiles_under_denied_warnings`
      and `Summary [ 300.009s] 1 test run: 0 passed, 1 timed out, 3 skipped` —
      a real cap being hit, not a flake, on a head whose only diff was eight
      lines of Markdown. The override at `.config/nextest.toml` now carries
      `slow-timeout = { period = "60s", terminate-after = 10 }` (600 s), which
      is the targeted-override case that file's own policy asks for, and it
      sits *inside* the ordering rather than at its edge: `global-timeout`
      (780 s) must stay strictly above the largest per-test allowance, which at
      60 s periods admits up to 720 s, so 600 s is a chosen figure leaving 180 s
      of margin. The figure moved from
      300 s to 600 s in the four contract modules that state it, in the
      developers-guide tier table and arithmetic, and in the `kani-smoke`
      ceiling comment; the ceiling still contains the worst case, 518 s of
      non-gate work plus 600 s against 1,800 s.
      **Amended 2026-09-23: the override was inert as written.** Its filter
      named the bare test name, and Nextest matches that regex against the
      qualified name, so the `^` anchor selected nothing and neither half of
      the override — the 600 s or the group assignment — ever applied. The
      value and the reasoning were right; the filter was not. Fixed in a
      follow-up by adding the `compile_guard::` prefix; the item above is kept
      as written because the discovery, the arithmetic, and the ceiling
      argument all still hold, and it is the record of how the fault was found.
- [x] (2026-09-23) Qualify the gate's filter with its module path and prove the
      override binds. `.config/nextest.toml` now reads
      `filter = 'test(/^compile_guard::every_patched_tree_compiles_under_denied_warnings($|::)/)'`.
      Three proofs replace the reasoning that had been standing in for one:
      `cargo nextest show-config test-groups` reports **4** overrides for
      `nested-cargo-builds` where it reported **3** before, naming the gate;
      `cargo nextest list` with the filter selects exactly that one test; and a
      deliberate `slow-timeout = { period = "5s", terminate-after = 1 }` probe
      run through `make test-kani-mutations` reports
      `TIMEOUT [   5.003s] … compile_guard::every_patched_tree_compiles_under_denied_warnings`,
      which is the override's own allowance terminating the test. The negative
      control is the unqualified filter, which under the same probe printed
      `SLOW [> 60.000s]` — the profile's period, not the override's.
- [x] (2026-09-23) Widen the accepted filter grammar to admit a module path, in
      both copies of it. `nextest_child_cargo_group_invariants.py` grew
      `MODULE_PATH = r"(?:[a-z0-9_]+::)*"` in `GROUP_FILTER`,
      `LEGACY_EXACT_FILTER`, and `ACCEPTED_TEST_SELECTOR`, and
      `nextest_child_cargo_group_test.py` pins that the qualified form is
      admitted *and* that it yields a bare name. The prefix is deliberately
      non-capturing: every consumer compares the captured name against
      `declared_test_names`, which computes no module path, so capturing it
      would report every module-scoped test as unresolved. The runtime copy in
      `.github/scripts/verify_nextest_anchored_filters.py` followed, together
      with a new whole-file liveness check described below. `make
      test-workflow-contracts` stayed at 605 passed, 2 skipped.
- [x] (2026-09-23) Widen the runtime liveness guard and prove it live. The
      script gained a whole-file replay — every `filter = '…'` value in the
      configuration is replayed verbatim through Nextest and must select at
      least one test — and the reading that carries it was split into a
      `_nextest_oracle` package beside the entry script, because the file was
      at 385 of the 400-line cap and inlined checks would have breached it.
      The package holds the configuration grammar, the listing oracle, and the
      instrumented-tree environment; the entry script keeps its top-level path,
      which is how the workflow calls it.
      Two facts were resolved by measurement rather than reasoning. First,
      `--run-ignored all` is load-bearing for the listing: without it Nextest
      reports an `#[ignore]`-gated test as `mismatch` whatever the filterset
      says, so a selector naming only ignored tests — the mutation compile gate
      is one — would read as selecting nothing. Probed directly: the gate's
      filter returns `[]` without the flag and the gate itself with it.
      Second, the reading grammar had to be widened here too. It still matched
      a bare `[a-z0-9_]+`, so the qualified gate filter was invisible to
      `configured_names` — the "unwritable in one place, unreadable in the
      other" split predicted in the plan, confirmed by probing the old and new
      patterns against the live filter.
      Both directions were liveness-checked, because a green exit proves
      nothing on its own. An in-process probe against a scratch copy of the
      config with the qualifier removed exits 1 naming that filter; against a
      scratch copy carrying a module-scoped legacy-form filter exits 1 naming
      the test. The real configuration exits 0. The first attempt ran the guard
      as a child process and reported both defects as missed — the child loads
      the module fresh and reads the real config, so the patch never reached
      it. An in-process call is the only form of this probe that measures
      anything.
- [x] (2026-09-23) Split the per-declaration `slow-timeout` readers out of
      `nextest_budgets.py` into `nextest_slow_timeouts.py`, after CI reported
      `C0302 (404/400)` on the merge result. `_multiplier_of` and `budget_of`
      are the seam: the whole of the per-declaration reading, with no importer
      outside the module. The four public functions stay put, so no importer
      changed. `budget_of` is public in its new home because it is now imported
      rather than called lexically; the `UnboundedTestError` import left with
      the code that raised it, and a stranded comment describing a humantime
      pair — orphaned above `_table` by the earlier `nextest_durations` split —
      went with it. Verified against the real merge, not the branch:
      `git merge-tree` with the work staged gives 305 lines with both sides'
      edits intact.
- [x] (2026-09-25) Repair the oracle masker's two-pass ordering, which the
      round-eight review surfaced and which is the same defect class this issue
      is about, one level up. `_nextest_oracle.listing.masked` blanked line
      comments before string literals, and no global pass can know which of the
      two is *inside* the other: a `//` within a literal ate the closing quote
      and blanked the rest of its line. Neither ordering is correct — the
      suggested swap breaks the other way and still misreads `r#"…"#` — so the
      fix is a single left-to-right scan that consumes each span as it is
      found. Replaced the two constants with six span helpers
      (`_line_comment_end`, `_block_comment_end`, `_raw_string_end`,
      `_string_literal_end`, `_character_literal_end`, `_non_code_end`) and made
      `masked` drive them positionally. Added a `rust-suites` guard to
      `selected`, so a listing that cannot be read is refused rather than
      reported as "selects nothing" against innocent filters.
      `nextest_oracle_masking_test.py` pins the behaviour, liveness-checked
      against all three variants rather than the fixed one alone: the shipped
      masker fails 3 of its 8 cases, the pass-swap variant fails 2, the
      single-pass scan passes all 8. Measured over `tests/` with the committed
      code: 187 functions / 752 cases corrected against 173 / 688 as shipped —
      16 functions the shipped reader never saw, 2 it reported that do not
      exist, 6 found but miscounted, 24 names in all. Every one was latent,
      because `_check` runs only for names both configured and present in the
      oracle's dict, and no affected name appears in `.config/nextest.toml`.
- [x] (2026-09-26) Pay for that scanner honestly. The six span helpers above
      fixed a real defect and cost the module real complexity: CodeScene scored
      `listing.py` 10.00 before the rewrite and 9.38 after, failing the
      `Overall Code Complexity` gate on the pushed head. The three recognizers
      that are genuinely simple — plain strings, character literals, and the
      nesting walk of block comments — are now anchored regular expressions
      (`STRING_LITERAL`, `CHARACTER_LITERAL`, `BLOCK_COMMENT_TOKEN`) rather than
      handwritten loops, leaving a helper per construct as a one-line
      predicate. Measured with the vendor's own analyser, `cs delta`, which
      reports the rule the PR check failed on: `Code Health: (9.38 -> 10.00)`,
      `Fixed issue: Overall Code Complexity`. `SUM_cx` falls 42 -> 33 and the
      worst function 8 -> 5, below both siblings' shape, without moving code to
      satisfy a metric. Held to the committed masker by a differential harness
      over all 33 adversarial synthetics plus every tracked `.rs` file — 744
      cases, 0 byte-level mismatches — which caught one real divergence while
      writing it: `.` does not match a newline by default, so a Rust literal
      continued with a backslash-newline would have been read as unterminated.
- [x] (2026-09-25) Repair the two defects the round-twelve gate run surfaced in
      the head that carried the oracle fix, both in this branch's own work.
      First, `python_toolchain_sync_test.py` went to `C0302 (402/400)`: the file
      stood at **exactly 400 on `main` and on this branch's own `HEAD`** before
      the edit, so it had no headroom, and the pinned `typecheck-python`
      assertion cannot be compressed back — the Makefile splits its two
      `--extra-search-path` flags across a line continuation, and the contract
      pins each substring separately, so the assertion cannot go below two
      entries and the file cannot come back to 400 in place. Established the
      boundary by probe rather than by assumption: pylint rates a 400-line copy
      10.00/10 and fires `C0302` at 401. Second, two en-GB-oxendict violations
      in this plan (`artifact`, `mis-ordering`).
      Fixed by extraction rather than by trimming, following the seam this
      directory already documents: `makefile_recipes.py` states that it "owns
      reading the Makefile" and that every contract wanting its text comes
      through `load_makefile`, yet this module reached past it to
      `MAKEFILE_PATH.read_text()` three times. The three readers move to that
      module as `makefile_variable`, `makefile_target`, and `makefile_command`,
      which is the same remedy `acab98a4` ("Split the report-validation contract
      under the 400-line cap"), `c49ef8fc`, `7813dea1`, `8af95537`, and
      `acf85a22` used, and the same one `makefile_variables.py` and
      `workflow_loading.py` were themselves created by. The test module drops
      to **363** and its new home to 192. Behaviour-preservation is measured,
      not asserted: the old and new implementations were run side by side over
      four variables, seven targets, and four command variables — 15 of 15
      identical verdicts. `ruff check`, `ruff format --check`, pylint,
      interrogate (100.0%), and `ty` all pass on both files.
      Both are instances of this plan's own subject: the round-eleven run
      aborted at ruff, so pylint never ran, and the `C0302` was only *unmasked*
      by fixing the earlier failure — the same "green because the reader never
      reached it" shape the whole issue is about.
- [x] (2026-09-26) Repair the round-thirteen run's two `MD038` findings: this
      document's own prose described the `ruff: ignore` convention with a code
      span beginning with a space, which `markdownlint` rejects. Rewritten as
      prose — the suffix really does begin with a space, so no span can carry
      it, and the autofix would have stated the opposite. The prerequisite
      mechanism that hid the finding is recorded under Surprises.
- [x] (2026-09-26) Re-run the gates the repair touches: `markdownlint` (0
      errors over 148 files) and `spelling` green, `check-fmt` green with
      `mdtablefix --check` agreeing after its refill.
- [x] (2026-09-26) Gate the masker re-factoring and commit it, then record the
      run here. `d8d0ba6d` carries exactly the two intended paths
      (`.github/scripts/_nextest_oracle/listing.py` and this document);
      `typos.toml` was excluded as tool-owned. Thirteen gate invocations are
      reconciled against it below. Eleven are green on the frozen tree:
      `check-fmt` (log 01, and again as 10 after a later edit), `lint` (02),
      `typecheck` (03), `lint-python` (05, and again as 12), `test` (08, 3315
      passed / 6 skipped / 123 doctests), `test-kani-mutations` (07, the gate
      this whole issue is about: `1 passed, 0 failed, 0 ignored, 3 filtered
      out`, 162.44 s, and it reports the patched tree compiling rather than
      merely applying), `test-workflow-contracts` (06, and again as 11),
      `markdownlint` (09, 0 errors over 148 files, and again as 13), and
      `nixie` (14). One of the fourteen is a red superseded by a green:
      `markdownlint` (04) failed on five en-GB-oxendict findings the Surprises
      entry above records, and 09 is the green re-run on the corrected tree.
      The counts are stated as invocations rather than as a table because
      neither `test-kani-mutations` (07) nor `test` (08) was re-run after the
      last prose edit: both were already green at the revision whose sources
      the later edits did not touch, and re-running a 162 s Kani gate to
      re-observe an unchanged Rust tree would buy nothing. The frozen tree is
      the one CI and CodeRabbit both saw at `d8d0ba6d`.
- [x] (2026-09-26) Confirm CI's own verdict at the pushed head rather than
      inferring it from a local run. All four required checks are `success` at
      `d8d0ba6d`: `build-test` 19m5s (run `36200886143`), `kani-smoke` 13m45s
      (same run) — whose `Mutation patch compile gate` step is precisely this
      branch's subject, so CI reproduces the local gate's verdict — and
      `netsukefile` 1m31s (run `36200885975`) and `release / metadata` 12s (run
      `36200886337`). `CodeScene Code Health Review (main)` is also `success`
      on the same head, which is the pushed-head confirmation of the local
      `cs delta` reading recorded above.
- [x] (2026-09-26) Repair the first-person pronouns CodeRabbit's review
      flagged in this document. Four instances, not the one it named: the
      reported possessive at line 764; a quoted self-report of the shape "the
      first error was fixed and a different one appeared", which stood in two
      places (under Surprises and in the Revision note); and a
      possessive-pronoun pair distinguishing a defect this branch introduced
      from a pre-existing condition it happened to expose. All four are now
      impersonal, which is what the path instruction asks for, and the two
      quotations keep the reported-speech sense that made them useful. Swept
      the whole document rather than the reported line, because a rule stated
      for one line is stated for the file, and the reviewer saw one of four.
      The rewrite of this very entry is the same lesson a third time: the
      first draft of it quoted the removed pronouns verbatim to say what had
      been fixed, which would have re-introduced the finding it recorded.

- [x] (2026-09-26) Move the compile gate off the working checkout and onto an
      isolated sandbox, closing CodeRabbit's Major finding. The gate seeded
      each mutation into the tree under test and relied on `Drop` to revert.
      Nextest terminates a timed-out test by signalling its process group, so
      `Drop` cannot run and a seeded mutation could survive in the working
      tree. `tests/kani_mutation_evidence_tests/sandbox.rs` now exports a
      revision with `git archive` into `target/kani-mutation-sandbox/`, and
      every `git apply` and every `cargo kani` runs there, with
      `GIT_CEILING_DIRECTORIES` pinned to the sandbox root so the upward search
      for a `.git` cannot reach the real checkout. Proved rather than argued,
      in both directions: with the gate green, `git hash-object` on a patched
      source is byte-identical before and after the run and `git status` shows
      only this branch's own edits; and with the gate handed a patch that
      applies cleanly but leaves a helper unused, it fails on the *compile* —
      `mutation patches apply but their patched trees do not compile` — not on
      the apply, which is the distinction that shows the check itself fired
      rather than an adjacent one. Cost is unchanged: 158.5 s against the
      162.4 s the live-checkout version measured over the same 18 patches.
      The revision compiled is the working tree, captured with `git stash
      create`, not `HEAD`: compiling `HEAD` would have reintroduced this
      issue's own failure mode one level up, reporting a committed patch
      healthy while the developer was editing it. The capture is proven to
      reach the sandbox (an uncommitted `Ordering::Greater` → `Equal` edit is
      visible in the exported revision) and proven inert on the shared stash
      stack, which stays at zero entries because `git stash create` writes a
      commit without pushing. Two defects were found by probing rather than by
      review and are recorded below: the first liveness injection failed for
      the wrong reason, and the patch listing initially disagreed with the
      revision the patches were resolved from.

- [x] (2026-09-26) Close the blind spot the sandbox migration itself opened:
      an untracked mutation patch would have been compiled by the old gate
      (which read the working tree) and silently skipped by the new one (which
      reads the sandbox, and `git stash create` captures tracked content
      only). That is this issue's exact failure mode — a run going green over
      evidence it never read — so `ensure_no_untracked_patches` now refuses it
      before the sandbox is built, naming the offending paths and the fix. The
      premise was measured rather than assumed: in a scratch repository, with
      one tracked patch modified and one added untracked, `git stash create`
      produced a revision that `git archive` exports as `tracked.patch` alone.
      The guard's own liveness was then probed in three states — a clean tree
      (silent), a tree with an untracked patch (names it), and a tree whose
      patch is untracked *and* ignored (silent, since an ignored file is not
      evidence anyone intends to gate) — with `git check-ignore` confirming
      the third state's premise actually held rather than passing vacuously.
      One cost, reported rather than buried: the ignored-file probe wrote to
      `bare.git/info/exclude`, a file shared by every worktree of this
      repository. It has been restored byte-for-byte to git's default template
      and re-verified inert, but its pre-truncation content is not
      recoverable, so any ignore rule another session had placed there is lost.
      The probe should have used a throwaway repository.

- [x] (2026-09-26) Clear the second `coderabbit review --agent` pass on
      `674e266f`. All five round-one sites were confirmed cleared and the pass's
      coverage count was exact at `33/33`, but two `trivial` findings were
      raised, both real and both duplication: `SANDBOX_DIR` in `sandbox.rs`
      spelled a path the same file already spelled as `SANDBOX_NAME` under
      `target/`, and the oracle's `fail` was annotated `-> None` while its body
      ends in a raise. The first is fixed by deleting the redundant constant and
      deriving the path from the name, so the directory created and the
      directory emptied cannot drift; the deletion was checked against the whole
      repository first, because the constant was `pub(super)` and could have had
      an in-crate consumer, and it has none. The second is fixed with
      `typ.NoReturn`, the house convention already used twice in
      `scripts/tests/test_doc_coverage.py`. Nothing else in the pass's output
      required action. Both fixes are verified — the derived path is
      byte-identical to the old literal, and the annotation resolves at run time
      (`fail.__annotations__` returns `typing.NoReturn`, and a call exits 1).
      The full nine-target gate set was then re-run on the resulting head, and
      is green as one set: `check-fmt`, `lint`, `typecheck`, `markdownlint`,
      `lint-python`, `nixie`, `test-workflow-contracts` (613 passed, 2 skipped),
      `test-kani-mutations` (the gate this issue is about, 171.6 s against a
      300 s default), and `test` (3315 passed, 6 skipped, 2 doctest targets).
      The runner recorded each changed blob's hash identical before and after
      the sweep, so the set is evidence about one frozen revision rather than a
      suite reconciled gate by gate. Its subject is the two code blobs, and
      both are unchanged since: the sweep still covers them exactly. The
      document's own blob has moved on, twice, recording the review history
      after the sweep — so as at `d8d0ba6d` above, the heavyweight Rust gates
      are declared as holding for the earlier revision whose sources the later
      prose did not touch, rather than re-run to re-observe an unchanged tree.
      Both later deltas are prose-only and were gated on the set a prose delta
      actually needs.
- [x] (2026-09-26) Clear the third `coderabbit review --agent` pass, which ran
      against `c1497d4c` at coverage `3/3` over the three changed files. It
      confirmed both round-one duplications fixed in the tree — `SANDBOX_DIR`
      gone with `SANDBOX_NAME` the single source, and `fail` annotated
      `typ.NoReturn` — and raised one `minor` finding of its own: two
      first-person passages in the revision-note prose that `c1497d4c` had just
      added. Both were reworded impersonally. That is the same rule the GitHub
      review raised against an earlier head, so rewording was the consistent
      answer even though no gate enforces it here — `markdownlint` configures
      only `MD004`, `MD010`, `MD013` and `MD029`, no prose linter is installed,
      and the ExecPlan status contract reads no pronouns. Worth noting because
      the branch had already swept this file for first-person prose at
      `21d4363c`, and that sweep's record of four repairs was accurate when
      written: the two new instances arrived later, in prose a subsequent commit
      added. A document-wide property is re-established by its gates, not by the
      commit that last checked it.
- [x] (2026-09-26) Re-run the pass as a verification pass on the result,
      `03780568`: it returned **zero findings** at coverage `33/33`, and its
      premise was checked in both directions — the 33 reviewed paths and the 33
      paths from `git diff --name-only origin/main...HEAD` compare as identical
      sets, and the reviewed revision matches the pushed head before and after
      the run. The docstyle finding is therefore cleared.

## Surprises & discoveries

- **A liveness probe that fails proves nothing until it fails for the stated
  reason.** The first attempt to prove the sandboxed gate could still detect a
  bad patch replaced one mutation patch with a handwritten one and ran the
  gate. It went red, which reads as the proof. It was not: the failure was
  `git apply to apply …: error: corrupt patch at line 16`, which is the *apply*
  check firing, and the compile check never ran. Writing the patch out by hand
  had made it malformed, so the probe tested an adjacent assertion.
  Regenerating it with `git diff` over a real source edit produced a patch that
  applies cleanly, and the gate then failed with
  `mutation patches apply but their patched trees do not compile` — the check
  under test, named in the failure. Both runs were red; only the second was
  evidence.
- **A sandbox changes what the gate is a statement about, and the change is
  easy to make by accident.** Exporting `HEAD` with `git archive` is the
  obvious way to populate an isolated tree, and it silently changed the gate's
  subject: the parent contract `every_patch_applies_cleanly` checks patches
  against the working tree, so the compile gate would have applied patches from
  `HEAD` while the sibling checked edits the developer had not committed. On CI
  the two agree and nothing shows; locally the gate would report a committed
  patch healthy while the developer looked at a different one — the same shape
  as the dead override this branch already fixed, one level up. A probe of the
  first liveness injection surfaced it: the sandbox compiled the committed
  patch, not the edit under test, because the two read different trees.
  `git stash create` captures the working tree without touching it or the
  shared stash stack, so the gate now compiles what the developer has while CI
  still resolves to `HEAD`.

- **A file at the 400-line cap fails on the merge, not on the branch.** CI went
  red on `e2987679` at `make lint-python`, and not for anything this branch
  wrote wrongly:

  ```text
  tests/workflow_contracts/nextest_budgets.py:1:0: C0302: Too many lines in
  module (404/400) (too-many-lines)
  ```

  The arithmetic is entirely positional. The merge base carried 395 lines;
  `main` added 395→399, a four-line ADR-038 paragraph; this branch added
  399→400, its 600-second docstring correction. The two edits are in different
  regions, so Git merges them cleanly, and pylint then refuses the *result*.
  Nothing local could have caught it: the branch head is exactly 400 and passes
  bare, and `make lint-python` over the working tree reports success. The
  failure exists only in the merge that GitHub Actions builds. A file sitting
  precisely at a per-file cap therefore has zero headroom by construction, and
  the place its overrun appears is the one place a local gate never looks. The
  repair is the split that `nextest_durations` and `nextest_totals` were each
  produced by, for this same cap. Stated generally, and this is the part worth
  keeping: **a per-file limit is not a property of either side of a merge, so
  neither side's green run can establish it.** The narrower lesson is to leave
  headroom when a file approaches the cap, rather than landing exactly on it.

- **The same cap failed again, one file over, the day after that entry was
  written.** The C0302 repair above (`nextest_budgets.py`) pushed CI red a
  second time, in a *different* file:

  ```text
  tests/workflow_contracts/workflow_loading.py:1:0: C0302: Too many lines in
  module (403/400) (too-many-lines)
  ```

  This branch's entire contribution to that file is one tuple element — a
  `kani-smoke` entry in `NEXTEST_JOBS`, which is why the branch touches it at
  all — and the file stood at 399 on `main` before the merge. The instance is
  unremarkable; what matters is that it recurred. The entry above had already
  stated the general rule, in bold, and the rule did not prevent this, because
  a rule about merges is not checkable by reading the file it applies to. The
  repair restores this branch's *net* contribution to zero (3 lines added, 3
  removed) rather than dropping the line that mattered, so the merge result is
  399, `main`'s own count.

  Two implementation notes survive this, and the first nearly cost a false
  confirmation: **`git write-tree` writes the index**, so the first probe of
  this fix reported the pre-fix count and would have "verified" a tree that
  never existed, because the edit was unstaged. The second is that `wc -l`
  counts newlines, so a net line-count delta and a `wc -l` delta can disagree
  on a file with no trailing newline; `git diff --stat` is the cross-check.
  Both probes were liveness-checked — pylint fires C0302 on the padded merge
  result at 404 and is silent at 399 — and the scoped sweep of every `.py` file
  this branch touches, run with the repository's own pylint wrapper against the
  real merge tree, rates 10.00/10.
- **A liveness probe can pass by measuring the wrong process.** The first
  attempt at liveness-checking the new guard patched
  `_nextest_oracle.grammar.NEXTEST_CONFIG` in the probe's own interpreter and
  then ran the script with `subprocess.run`. Both probes reported the defect as
  *missed*, which reads as a weak guard. The guard was fine; the probe was not.
  A child process re-imports the module from disk and reads the real
  configuration, so the patch never reached the code under test. The fix is to
  load the entry script with `importlib` and call `main([])` in-process, where
  the patched module is the one the guard actually uses. This is the same class
  of error as the green exit it was meant to guard against — a probe that
  cannot fail for its stated reason is not evidence — and it is worth recording
  that the second, *correct* run reported both defects as caught.
- **A widening can be correct and still be untested by the suite.** Writing the
  widened reading grammar in `_nextest_oracle/grammar.py` changed nothing
  observable in the current configuration, because the only qualified filter it
  newly reads is one that already passes the whole-file replay. A green run
  therefore proved nothing about it. Demonstrating the difference needed a
  probe against a *fictional* filter instead: the old pattern captured nothing
  for a module-scoped legacy-form filter, the new one captures the bare name
  and fires the guard. Without that probe the widening would have shipped
  unverified, and it is exactly the change whose absence the plan predicted as
  the "unreadable in the other half" failure.

- **A gate that passes twice can still be failing, and the cap is the tell.**
  `kani-smoke` passed the compile gate at `257.7 s` and `261.5 s` of a `300 s`
  per-test allowance, then failed at `300.008 s` — on a head whose only diff
  was eight lines of Markdown. The temptation is to call that a flake, and it
  is wrong twice over: the delta proves the *code* did not change, not that the
  failure is intermittent, and reading the job log shows a real cap being hit
  rather than an error. The gate compiles into `target/kani-mutation-compile`,
  which no cache restores — `kani-cache` restores the Kani payloads under
  `.kani-rustup` and `.kani-home`, not a build tree — so each run pays a cold
  build of the whole dependency graph plus 18 incremental recompiles, and its
  cost is the host's to decide. A 38 s margin on a 300 s cap is the difference
  between two runners, not a budget. Local runs pass in `151.5 s`, which is why
  this only ever appears in CI: the local figure is nearly twice as fast as the
  CI one, so the local gate says nothing about the CI margin.
- **The largest per-test allowance is not the profile's own.** Once an override
  widens a `slow-timeout`, `largest_test_allowance` — which
  `whole_run_ordering` uses to check `global-timeout > largest` — reads the
  override, not `[profile.default]`. So widening one test's budget moves the
  figure the whole-run budget is compared against, and the documented "largest
  per-test allowance" moves with it: the guide and four contract modules all
  stated `300 s` and had to restate `600 s`. The two readings are separate
  concerns and the separation is real —
  `base_allowance_test.the_default_profile_bounds_a_test_it_matches_no_override_for`
  exists precisely because deleting the profile's own `slow-timeout` while
  leaving an override would still report a bounded subset — but the *ordering*
  reads the maximum. Widening an override without moving that figure would
  leave the guide stating a number the file no longer has.
- **A compile survey found five broken patches, not three.** Running
  `RUSTFLAGS="-D warnings" cargo check --lib --all-features` over all 18
  patches at the then-current `main` (`00f48f77`, 2026-09-21) showed
  `marker_token_match_is_exact` and
  `scanner_agrees_with_independent_specification` also fail — these are exactly
  the two PR `#755` repairs, which had not yet merged. At that head, `main` and
  `#755`'s base (`61a944fb`) had identical blob hashes for all five affected
  files, so the mechanism reproduced on a clean `main`. They were in scope for
  `#755`, not `#756`.
- **That survey's framing was overtaken by the merge, and read as false once
  the base moved.** `#755` merged on 2026-09-22 (`ee0e5523`), and this branch's
  base (`30c50e27`, 2026-09-22 23:01) sits after it, so the two repairs now
  arrive from `main` rather than from this branch. The earlier revision of this
  entry said `#755` "has not merged", which was true when written and is not
  now. The adoption itself was real: it was folded into `a5b8a928` ("Gate
  mutation patches on compiling under `-D warnings`"), which touched both patch
  files, and the rebase onto `30c50e27` dropped it as redundant. The blobs are
  byte-identical either way — verified against `#755`'s own repair commit,
  `3a282018`, not inferred from the merge — so the tree this branch ships is
  the one the adoption produced and only the provenance changed. What this
  branch reseeds is therefore **four** patches, and the issue's "three" and the
  survey's "five" are both correct at their respective heads.
- The issue's suggested `_name` / `#[cfg(test)]`-visibility remedy is weaker
  than the in-place idiom the healthy patches already use: rebinding silences a
  warning without seeding a behavioural fault the harness can catch.
- The Python contract `nextest_child_cargo_group_test.py` does **not** classify
  the new test as build-capable, contrary to the prediction: `mask_non_code`
  retains only the literal `"cargo"` as a `CARGO_COMMAND` anchor, and the test
  locates Cargo with `env!("CARGO")`, whose string is masked. Registration in
  `.config/nextest.toml` is therefore not contract-forced — but it is still
  correct, because the test genuinely spawns nested Cargo builds.
- `test_execution_coverage_test.py`'s `LINUX_TEST_EXEMPTIONS` already exempts
  `kani-smoke`, but no exemption covers `build-test`. Its forbidden-suite
  regexes are `\bcargo nextest\b`, `\bcargo test\b`, and
  `\bmake test(?![\w-])` — the last excludes `-`, so `make test-kani-mutations`
  is deliberately not read as a second suite execution. Verified by running the
  regexes directly and then the whole contract suite (595 passed).
- `mdtablefix` without `--in-place` only *prints* the reformatted document; it
  never writes. A hash comparison "proved" idempotence while the check still
  failed. Only `make fmt` (which passes `--in-place`) applies it. `--check` and
  `--diff` are mutually exclusive, so the diff must be read from `--diff` alone.
- The gate's `Drop` guard rewrites `src/` files and restores them, which bumps
  their mtime and makes the editor report them as changed.
  `git diff --quiet -- src/` is the check that matters: the revert is exact.
- The GitHub CodeRabbit bot does not review draft pull requests. Its own comment
  on PR `#766` reads "Draft PR not reviewed", so the local
  `coderabbit review --agent` pass is the only CodeRabbit signal available
  while the pull request stays a draft. This differs from the pull-request
  review object, which is a separate mechanism with its own staleness behaviour.
- `coderabbit review --agent` returned three findings, and measuring each
  reversed two of them:
  - The one `major` finding claimed `tests/makefile_test_target.rs` "already
    extends beyond line 730" and demanded a 400-line split. The file is **299
    lines**, and was 298 at base: this change adds exactly one. The cited line
    730 is not a line count this file has ever had, and `AGENTS.md`'s 400-line
    bar is a contributor guideline with no enforced gate over `tests/`, where
    seven tracked files already exceed it.
  - The two `minor` findings asked to delete the comma before `because` in
    `docs/developers-guide.md` and in this plan. `developers-guide.md` uses the
    comma form **59** times against **44** without, so the finding would have
    moved the line away from its own file's majority. Both clauses are
    non-restrictive: "A fourth check", which is already identified, is gated
    *for a stated reason*, so the clause is supplementary and the comma is
    grammatical.
- **A draft review and a ready review are not the same review, and the second
  one found real defects.** Marking the pull request ready produced a genuine
  GitHub CodeRabbit pass (`CHANGES_REQUESTED`, pinned to `e4f93b93`) whose two
  live findings were both correct and both reproduce — unlike the three above,
  which came from the local CLI and were dismissed on measurement.
  Internalizing the first pass's dispositions as "CodeRabbit is unreliable on
  this PR" would have shipped two real defects. Each was reproduced before
  being accepted:
  - **The gate could silently become a no-op.** Deleting
    `--run-ignored ignored-only` from `test-kani-mutations` made
    `make test-kani-mutations` **exit 0 while compiling nothing** — nextest
    reported "3 tests run, 1 skipped" and the ignored compile gate never ran —
    and all **36** Makefile contract tests still passed. A CI step reporting
    success while checking nothing is precisely the failure class this task
    exists to eliminate, so the recipe now has its own contract,
    `behavioural_kani_mutation_target_selects_the_ignored_compile_gate`, which
    pins the test binary, `--run-ignored ignored-only`, and `GATE_RUSTFLAGS`
    against the `nextest run` line. Liveness proven both ways: it fails with the
    flag removed naming the exact recipe line, and passes with it restored.
  - **A failed revert could pass unnoticed.** `AppliedPatch::drop` logged and
    returned `()`, so the test could return `Ok(())` with a seeded mutation
    still in the working tree. `revert()` is now a fallible method called on the
    normal path and aggregated into an `unreverted` list, with `Drop` demoted to
    the unwind fallback it is actually good for and an `applied` flag so the
    fallback does not double-reverse. Liveness proven by injecting a revert
    failure: the gate exits 2 and names every patch.
- **The third finding was right, and it invalidated the placement the first two
  findings had already approved.** CodeRabbit said the gate checks compilation
  with a checker that cannot see the mutated code. Measuring it confirmed the
  mechanism exactly: `cargo check` does not parse `#[cfg(kani)]` code, and on
  `ir__cycle__verification__self_dependency_reports_cycle` — whose only changed
  line is a `cfg(kani)` match arm — `cargo check --lib --all-features` exited 0
  and called the tree healthy while `cargo kani` rejected that same tree with
  `error: variant Present is never constructed`. Two consequences followed, and
  the second was the expensive one:
  - `cargo check` is replaced, not supplemented, by
    `cargo kani --only-codegen` (a full compile under `cfg(kani)` with
    verification skipped). A census of all 18 patches found exactly **one**
    changing a `cfg(kani)`-gated line, so this was a real but narrow blind spot
    rather than a wholesale failure of the gate.
  - **The gate cannot live in `build-test`.** Only Kani parses that surface, and
    `build-test` installs no Kani — a Kani-frontend compile there is not
    expensive, it is impossible. Placement is not a preference here; the
    checker's identity determines it. This is the part the first two findings
    could not have caught, because both were raised against a gate whose checker
    had not yet been questioned.
- A bare `--cfg kani` RUSTFLAG is not a substitute for the Kani frontend.
  Measured: it applies to the whole dependency graph, and third-party crates
  (e.g. `zerocopy`) carry their own `cfg(kani)` arms that need the `kani`
  crate, producing 66 errors. The `kani` crate is in fact not a declared
  dependency at all — it is absent from `Cargo.lock` despite 92 `kani::` usages
  in `src/` — because the frontend supplies it.
- **CI found a second defect of exactly the same shape as the first: a check
  that passed while its subject was unmet.** The moved gate failed on its first
  real run (run `35782659415`, job `106931720330`) at toolchain selection,
  having compiled nothing, with
  `error: the option Z is only accepted on the nightly compiler`.
  `GATE_RUSTFLAGS` unconditionally appends the nightly-only `-Zthreads=8`, and
  the job's own `RUSTUP_TOOLCHAIN: stable` — inherited from when the job ran
  only `make kani-ir` and never reached this code — makes rustup's env override
  beat `rust-toolchain.toml`'s directory override. Reproduced locally in one
  command: `RUSTUP_TOOLCHAIN=stable rustc --version` reports `1.98.1` stable
  where the bare invocation reports the pinned `1.100.0-nightly`, and the same
  `cargo metadata` invocation the gate makes exits 101 under the variable and 0
  without it.
  - The trap is that `cargo kani` *does* ignore the variable — it resolves its
    own bundled toolchain — so the reasoning that once justified `stable` here
    was sound for the command it was written against and became wrong the moment
    the gate ran `cargo nextest` instead. A comment can be true of one command
    and false of the next, and this one was never re-measured for the new one.
  - `check-build-tools` cannot catch the mismatch: it asserts the pinned
    toolchain is *installed*, not *active*, so it passed in the same run that
    the gate aborted. Its own header comment already names the dependency
    ("the standard's parallel frontend is a nightly-only flag") without checking
    it. Hardening it to assert the active toolchain is deliberately **out of
    scope** here: it gates 11 Makefile targets and every developer path, and a
    false positive there would break all of them at once. The gate's own env
    now documents why a channel value must never be reintroduced.
- **Two diagnoses of this failure were wrong before the third was measured, and
  the second was written into this plan as settled.** Four CI runs aborted at
  toolchain selection having compiled nothing, and each repair moved the story
  without moving the outcome.
  - The first explanation blamed the redirected `RUSTUP_HOME`: the theory was
    that rustup looks for `rust-toolchain.toml`'s pin inside that home and, not
    finding it, falls back. The job log falsifies it twice over. The pin *was*
    found there — `check-build-tools` prints `toolchain nightly-2026-08-23
    available`, grepping `rustup toolchain list` under the same redirected home
    — and a redirected `RUSTUP_HOME` does not disable `rust-toolchain.toml`.
  - The second blamed a **directory override**. Run `35788180077` logged
    `info: override toolchain for /home/runner/work/netsuke/netsuke set to
    stable-x86_64-unknown-linux-gnu` during `Setup Rust`, and the step's
    `toolchain` input was the literal `stable`, so the envelope fit: `setup-rust`
    sets an override that outranks `rust-toolchain.toml`, and that was taken to
    be what selected stable. The step was changed to name the pin instead.
  - That change made the run *worse in a way that identified the truth*: run
    `35792995207` logged **no override line at all**. The action has three
    mutually exclusive arms, and only the one gated on a *non-empty*
    `toolchain` input passes `override: true`; the others pass `override:
    false`. No override line therefore means the empty-input arm ran, which
    means `${{ env.NETSUKE_RUST_TOOLCHAIN }}` expanded to nothing. It does:
    `env` resolves per job, and the variable was declared in `build-test`'s
    job-scope block, which `kani-smoke` does not inherit. The rendered step
    input is empty in the log, the action's `explicit toolchain` arm is
    `skipped`, and `rustup show active-toolchain` answers
    `stable-x86_64-unknown-linux-gnu (directory override for '…')`.
  - So a directory override *was* selecting stable, but not one this job set on
    that run. `kani-cache` restores `.kani-rustup` wholesale — `settings.toml`
    and its `[overrides]` table included — and that table was written by an
    earlier run whose input was the literal `stable`. The cache preserved the
    toolchain decision across the fix that was meant to revoke it. Naming a
    non-empty pin is what displaces the entry, because it selects the
    `override: true` arm.
  - The fix is therefore to declare the pin in `kani-smoke`'s own `env` block,
    as every other job on this contract does. The regression guard is the
    sharper half, and it is new: the existing assertion compared a job's pinned
    channel against `rust-toolchain.toml`, so it could catch a pin that
    *disagreed* and never one that nothing read. It now also requires that a job
    pinning the variable passes it to the action, and that a job passing it pins
    the variable. Both directions were falsified against the tree before being
    trusted.
  - The lesson sharpened twice, and the second version is the one worth keeping.
    The first was "a mechanism that explains the symptom is not yet a mechanism
    that has been located". The second is that the *repaired* mechanism was
    recorded as fact on the strength of one corroborating log line — an
    `override` line that was genuinely there, and genuinely not on the path that
    mattered. Reading the action's source, which was also in the log, would have
    shown three arms where one had been assumed.
- **The branch's own Nextest filter was written in the spelling `#732` had just
  retired, and the conflict was the least of it.** Rebasing onto `origin/main`
  after `#732` landed replayed eleven commits, three of which conflicted. Two
  were ordinary context drift, but `.config/nextest.toml` conflicted on
  substance: this branch registered its new test with
  `filter = 'test(=every_patched_tree_compiles_under_denied_warnings)'`, and
  `#732` had replaced that grammar file-wide with `test(/^NAME($|::)/)`. The
  resolution was forced rather than discretionary, and the contract is what
  proves it: `nextest_child_cargo_group_invariants.py` holds the accepted forms
  in an **allow-list** regex, and `test(=NAME)` is not among them. Probing that
  regex directly over the resolved file judges all **17** selectors accepted,
  **0** legacy, and rejects the old spelling — so the conversion is not a
  stylistic preference but the only form the contract admits. The same `=` form
  that silently unhooked parameterized tests from their policy is the one a
  new, non-parameterized test would have carried happily: it would have worked,
  which is why nothing would have flagged it.
- **A rebase whose conflicts are resolved by hand needs a proof that is not
  ancestry.** `git diff <old-head> <new-head>` over the whole tree showed only
  files that `origin/main`'s four incoming commits had themselves touched, plus
  the two conflict files — every file this branch authored came back
  **byte-identical**, all 18 patches included. That is the check worth having:
  the eleven replayed commits have new SHAs, so citing the old ones proves
  nothing, and a green suite after a rebase proves only that the suite passes.
- **The issue's own subject recurred in this branch's fix, one layer up.** The
  whole point of the mutation compile gate is that a patch can apply cleanly
  and still contribute nothing. The override added to *fund* that gate had
  exactly that shape: it parsed, it was documented, it was reasoned about in
  five files, and it applied to no test at all, because Nextest matches a
  filter's regex against the fully qualified name and the test lives in a
  submodule. Three independent facts say so, and each was cheap once looked for
  — of the 17 filter literals in the file it is the only one selecting nothing;
  `cargo nextest show-config test-groups` reports 3 overrides for a group the
  file declares 4 for; and a 5 s probe printed `SLOW [> 60.000s]`, the
  profile's period rather than the override's. The trap generalizes past this
  file: any filter for a test declared in a submodule needs its `module::`
  prefix.
- **Both guards were green, and their scopes were the gap, not their
  mechanisms.** The static contracts compare filter names to declared names
  **bare-to-bare**, so a filter naming the right test under the wrong or absent
  module still resolves and passes. That is not a defect in the comparison —
  `declared_test_names` computes no module path at all, so capturing the prefix
  would break it — but it means the static half cannot see this fault by
  construction. The runtime oracle *does* ask Nextest which tests a filter
  selects, which is authoritative; it simply only asked about filters naming a
  parameterized test, and this filter names a plain one. The mechanism was
  right the whole time and the coverage was one intersection short. The same
  blind spot was latent in a second place: `_check`'s stray-name assertion
  anchors at the bare name, so a filtered *parameterized* test moved into a
  submodule would have reported its own `mod::name::case_1` as a stray.
- **A negative control is what makes a liveness probe mean anything.** The
  unqualified filter's `SLOW [> 60.000s]` and the qualified filter's
  `TIMEOUT [ 5.003s]` under the same deliberately tiny allowance are the two
  halves of one experiment; either alone proves nothing. The passing CI runs
  were the misleading half — 257.7 s and 261.5 s both sit inside the 300 s
  default that an inert override predicts, so they are consistent with the
  override working and with it doing nothing, and only the pair of probes
  separates those.
- **A gate that passes is evidence about the tree it ran against, and the pass
  and the tree have to be named together.** Three gates failed on the round-8
  head — `check-fmt`, `lint`, `typecheck` — and all three failed on the work
  this branch had just committed, `nextest_oracle_masking_test.py` from
  `632e5952`, not on the Rust change under test. The Rust change was clean:
  clippy `--all-targets`, `cargo check --all-targets`, and a full `make test`
  of 3315 tests all passed around it. Two of the three failures were of a kind
  the gate set cannot see until the *next* run: ruff's
  `docstring-missing-returns` fires only on a multi-line docstring, and ty's
  `unresolved-import` fires only because `make typecheck-python` passes
  `--extra-search-path scripts` while the test imports from `.github/scripts`
  through a `sys.path` insert that ty does not follow. The third is a Markdown
  refill: a one-line pronoun correction shortened a line, and mdtablefix
  `--wrap` is a *refill*, not a cap, so an otherwise 80-column-clean line can
  still fail it. None of these is a correctness defect in the logic the commit
  was about; all three are the committed artefact failing the project's own
  standards, which is what the gates measure.
- **`make test-workflow-contracts` passing does not mean a test in
  `tests/workflow_contracts/` was exercised by the gates that matter.** The
  masking regression test ran green inside that target's 613 passes throughout,
  and its file was still red under `make lint-python` in the same run. The two
  cover different things: the contract target *executes* the test, and the lint
  target *reads* the file with ruff, ty, pylint, and interrogate. A green test
  run says the assertions hold; it says nothing about whether the file
  satisfies the linters, and only the lint target does. Recording the
  distinction because the reflex — "the file was exercised, so it was checked"
  — is wrong in the direction that lets a defect through.
- **A lint gate that stops at its first failing stage can *manufacture* a
  repair that produces the next failure.** `make lint-python` runs ruff,
  pylint, the df12 lints, ambrleaks, and interrogate in sequence and aborts on
  the first non-zero stage. The round-eleven run died at ruff
  (`docstring-missing-returns`), so pylint never executed, and its `C0302` on
  the same file could not appear in that run's output. Repairing the ruff
  finding therefore did not clear the file — it *unmasked* the pylint one on
  the next run. The sequence reads as two independent findings arriving one
  after another; it is one file's condition, sampled one stage at a time. A
  green stage is evidence about that stage only, and the gate's overall verdict
  says nothing about the stages behind the first failure.
- **The 400-line cap is a property of the file, not of the edit, so a correct
  edit can be blocked by a file that was already full.**
  `max-module-lines = 400` refuses the module, and pylint's own rating of a
  400-line copy is 10.00/10 — so `main` sat on the last passing value, and the
  two lines this branch's wiring fix needed were not available at any price.
  The probe that established this is the one worth repeating: rate a copy at
  the boundary and one line past it, rather than reasoning about whether the
  cap is inclusive. The remedy the repository already uses, and the one
  `makefile_recipes.py`'s own docstring names, is a split along a seam — which
  also leaves headroom rather than re-landing on the ceiling, the lesson
  recorded the day before in `6fe4976a`.
- **A stage that has never executed has said nothing, and the cascade runs
  deeper than the stage list.** The df12 lints are the third stage of
  `lint-python`, and on this branch they had never run: round eleven aborted at
  ruff, and round twelve's pylint `C0302` aborted one stage later. Repairing
  pylint let df12 execute for the first time, and it found five defects — four
  `C9102` (a bare `assert`) and one `C9106` (a suppression without an
  explanation) — in `nextest_oracle_masking_test.py`, the very file the earlier
  repairs had been clearing. Each successive repair therefore *looked* like it
  introduced a new fault, and did nothing of the kind: the file was in that
  condition throughout, and each stage reported only what it was the first to
  be able to see. "The first error was fixed and a different one appeared" is
  not evidence of a new defect; here it was evidence of the opposite.
- **The convention a lint rule encodes is worth measuring before repairing
  against it.** The cheap way to read `C9102` is "add messages until the lint
  goes quiet", which would have produced four plausible strings and no evidence
  they matched the house style. Counting instead showed the rule is total —
  **882 `assert` statements across the three owned Python roots, and not one
  bare**, with every `ruff: ignore` in the tree followed by a hyphen and an
  explanation — so the five findings were this branch's file being the only
  violation in the repository, and the repair had one unambiguous shape. The
  same audit is what distinguishes a defect this branch introduced from a
  pre-existing condition it happened to expose, which is the distinction that
  decides whether to fix it here at all.
- **A Makefile prerequisite short-circuits its target, so a target that never
  started is indistinguishable from one that passed.** `markdownlint: spelling`
  runs `spelling` first; round twelve's `spelling` was red on two en-GB
  spellings, so `markdownlint-cli2` never executed on this branch at all. The
  two `MD038` findings it would have reported were therefore absent from every
  round-twelve report — not because the file was clean, but because the linter
  had not run. This is the `lint-python` cascade's shape moved one level up: a
  stage cannot report before the stage ahead of it passes, and neither can a
  target report before its prerequisite does. Three distinct mechanisms have
  now produced the same false "clean" reading in this plan — a stop-at-first-
  failure *stage*, a never-executed *job*, and a never-reached *prerequisite* —
  and in all three the exit code alone cannot tell "passed" from "never
  started". The generalization worth keeping: before trusting any green result,
  ask what had to succeed for it to have been printed.
- **The vendor's own analyser runs locally, so a code-health failure can be
  measured rather than pushed at.** `CodeScene Code Health Review (main)` went
  `success` on `f6c227f7` and `failure` on `fd8899c9`, naming `listing.py` and
  `Overall Code Complexity` — a regression this branch introduced, and exactly
  the shape this branch's own notes warn not to pattern-match away as the
  trunk-only `Coverage (main)` timeout. The `cs` CLI is installed
  (`~/.local/bin/cs`), and `cs delta` with no arguments reproduces the gate on
  uncommitted changes, quoting the same rule under the same name. It reported
  `Code Health: (9.38 -> 10.00)` and `Fixed issue: Overall Code Complexity`, so
  the remedy was confirmed before it was committed instead of after a CI round
  trip. The recorded figure agrees with the check's own `9.39` to the last
  place its rounding allows, which is what makes the local reading an
  instrument rather than a proxy. The generalization: a repository can fail a
  gate that a locally-installed vendor tool decides, and when it does, the tool
  is the oracle — reading its rule name and score beats inferring a threshold
  from the diff.
- **A scanner's complexity is a real cost even when the scanner is right.** The
  six span helpers fixed a genuine misreading, and they were the honest way to
  fix it, but the module went from 3 functions / `SUM_cx` 8 to 9 / 35 doing so,
  and the code-health gate caught that as its own finding. Both statements are
  true at once: the correctness fix was not optional, and the complexity it
  added was not free. What resolves it is not a choice between them but a third
  factoring — three of the six recognizers are simple enough to be anchored
  regular expressions, so the loop-per-construct shape survives only where the
  construct genuinely needs one (`_block_comment_end`'s nesting walk). Left
  measured with the vendor tool at `10.00`, `SUM_cx` 33, worst function 5.
- [x] (2026-09-26) The `spelling` prerequisite short-circuited `markdownlint`
      for a fifth time, on the prose this entry and the two above were written
      in. Six findings: five `-ise`/`-iser` forms (`recognisers`,
      `hand-written`) and, after correcting those, one over-correction —
      `analyzer` for `analyser`. Both directions were wrong, and the second is
      the instructive one: `recognizer` is the oxendict form and `analyser` is
      not, so a pass that converts the whole `-iser` family introduces a finding
      while fixing five. The tool is the authority in exactly the case where the
      two categories look identical, which is why the correction was verified by
      running `typos-config-builder gate` on the result rather than by reasoning
      about the words. The gate was green on the corrected tree.

- **A fix that narrows its input can reintroduce the bug it fixes.** The
  sandbox migration changed what the compile gate reads: from the working tree
  to a captured revision. The reason for the change is sound, but it moves the
  gate's input set from "every patch in the checkout" to "every *tracked*
  patch", and nothing was watching the difference. An untracked patch would
  have been silently dropped from the run — a green gate reporting on fewer
  patches than exist on disk, which is precisely what this issue is about, one
  turn deeper. It surfaced only by asking what the capture *cannot* hold rather
  than what it holds, and it took a scratch-repository probe to confirm:
  `git stash create` exports `tracked.patch` and not the untracked sibling. A
  mechanism's blind spot is a different question from its behaviour, and the
  behaviour was already proven.

- **A shared-state probe must not run against the shared state.** The
  liveness probe for the ignored-file case wrote its ignore rule to
  `bare.git/info/exclude` — the file every worktree of this repository reads.
  Writing it was the mistake; discovering it was worse than it needed to be,
  because the file was passed to a truncating redirect before its contents were
  read. The rule that would have prevented it is not "be careful with `git`"
  but "probe in a repository you own": a throwaway `git init` under `/tmp`
  answers the same question with nothing at risk. The cost here is real and
  bounded, and stated in the Progress entry rather than repaired silently: the
  file is now git's default template, and any rule another session had put
  there is gone.

- **A gate result is evidence about the revision it read, so a suite that spans
  a change has to be re-run rather than reconciled.** The first nine-target run
  of this branch returned eight green and one red, and eight of its nine
  results were unusable — not because any gate was flaky, but because the three
  changed files kept being edited while it ran, so six gates judged revisions
  that no longer existed by the time they were read. The runner noticed on its
  own and reported that its evidence was invalidated, which is the behaviour
  worth wanting from a gate runner: it said so rather than reporting a green
  set of unknown provenance.

  The instructive part is that the one red was still a real finding. It failed
  on the `spelling` prerequisite with `artifacts` where en-GB-oxendict requires
  `artefacts`, in prose this branch had just written. That result was about a
  revision that had already been superseded, and it was nonetheless true — so
  the temptation was to keep the finding and believe the eight greens beside
  it. Those two verdicts have different validities, and a run that mixes them
  cannot be cited as a set. Making the finding actionable meant first repairing
  the *other* instance of the same word, at line 1940, which the sweep could
  never have seen: a gate can only report what was on disk when it read the
  file.

  Two hardenings follow, and the second is the one that generalizes. Freeze the
  tree, then gate it, then push — the discipline this repository already
  recorded for a *frozen SHA* turns out to apply to the working tree as well,
  and there is no version of it that works while edits continue. And because the
  `spelling` prerequisite short-circuits `markdownlint-cli2`, a green
  `markdownlint` must be shown to have run *both* stages; the re-run's log
  names the prerequisite and then `linting 148 file(s)` with `0 error(s)`,
  which is what makes the difference between a green gate and a gate that never
  reached its assertions. The same shape appears in this branch's other
  evidence — the `-tf1` sweep's failure proved nothing about the Markdown rules
  either, in the opposite direction.

## Decision log

- Seed the compile gate's mutations into an isolated sandbox rather than the
  working checkout, and do it with `git archive` over a captured revision.
  CodeRabbit's Major finding is confirmed, not declined: the mechanism is real,
  since Nextest signals a timed-out test's process group and `Drop` cannot run,
  so the gate's revert is not what protects the checkout. Options weighed: keep
  the live checkout and document recovery (rejected — a stranded mutation is a
  change nobody made, and this issue is about evidence that looks healthy while
  being wrong); a `git worktree` (rejected — it writes a registration under the
  *shared* git dir, and a SIGKILLed run leaves one that `worktree prune` will
  not clear, so every later run dies with `already exists`; proved by probe);
  an ambient copy of the checkout (rejected — 264 MB, drags in `target/`, and
  would compile uncommitted scratch that `HEAD` does not contain).
  `git archive` costs 0.1 s and 16 MB, preserves the tracked symlink and the
  executable bits, and produces no `.git`, so isolation does not rest on an
  environment variable alone; `GIT_CEILING_DIRECTORIES` is still set and
  asserted at runtime, because the sandbox sits inside the checkout and an
  unceiled `git` would walk up into it. The sandbox path is fixed under
  `target/` rather than a fresh temporary directory: a build tree is keyed to
  the absolute paths it was compiled from, so a new path each run would
  invalidate the shared `CARGO_TARGET_DIR` and pay a cold build every time.
  Measured: 36.3 s cold, 5.1 s for a re-extracted sandbox at the same path, and
  158.5 s for the whole gate against 162.4 s before, so the isolation costs
  nothing measurable.
- Capture that revision with `git stash create`, not `HEAD`. Compiling `HEAD`
  would have made the compile gate disagree with `every_patch_applies_cleanly`,
  which reads the working tree; a developer checking an uncommitted patch edit
  would get a green run describing a revision they are not editing.
  `git stash create` writes a commit holding the tracked working tree and
  returns its id, or nothing when the tree is clean, and it does neither of the
  things that make `stash push` hazardous here: it does not touch the working
  tree, and it does not touch the stash stack, which this repository shares
  across worktrees. Both halves are measured — the uncommitted edit is visible
  in the exported revision, and the stack stays at zero entries.

- Replace `cargo check` with `cargo kani --only-codegen` in the compile gate,
  after measuring that the checker was blind to one whole class of fault. This
  is the third CodeRabbit finding's subject and it is confirmed, not declined:
  `cargo check` never parses `#[cfg(kani)]` code, so a patch that seeds its
  fault inside a Kani-gated arm compiles clean under the gate while
  `cargo kani` rejects the very same tree. `--only-codegen` is a superset (it
  is a full compile under `cfg(kani)`), so it replaces the check rather than
  joining it, and it is affordable: the whole gate measured 159.988 s over all
  18 patches on a warm shared target directory, against 172 s for the same
  sweep through a fresh one.
- Run the gate in `kani-smoke` rather than `build-test`. The checker decides the
  host: only the Kani frontend parses `#[cfg(kani)]` code, `build-test` has no
  Kani installed, and adding it there would mean installing the verifier and
  `install-build-tools` into a lane whose whole shape — `use-sccache: 'false'`,
  no `cargo binstall` — exists to keep it cheap. `kani-smoke` already has the
  frontend, so the move costs three steps and two env vars and buys the gate
  the only checker that can see its subject. Rejected: a third job, which would
  have needed registration in `run_and_job_pairs`, `DIRECT_RUNNER_SOURCES`,
  `REQUIRED_RUNNER_ASSIGNMENTS` (exact-equality), `CACHE_ACTION_CALLERS`,
  `KEY_WRITERS`, `SETUP_RUST_DELEGATING_JOBS`, `UBICLOUD_CACHE_SOURCES`,
  `SCCACHE_WRAPPER_JOBS`, and `LINUX_TEST_EXEMPTIONS`, plus a
  `runner-placement` Hypothesis example — a contract surface far larger than
  the gate it would host.
- Raise `kani-smoke`'s ceiling from 20 to 30 minutes as part of the move, and
  update `tests/workflow_ci.rs` with it. The gate builds the entire dependency
  graph through the Kani frontend into a cold `CARGO_TARGET_DIR`, on top of the
  harness run the ceiling was originally sized for; the old bound would have
  killed the job it now hosts.
- Remove `kani-smoke`'s `RUSTUP_TOOLCHAIN: stable`, pin the `Setup Rust` step's
  `toolchain` input to the nightly, and declare `NETSUKE_RUST_TOOLCHAIN` in the
  job's own `env` so that expression resolves. Three halves, each doing a
  different job, and each required: the variable must stay gone because a
  *channel* value there is always wrong for the gate; the step input must name
  the pin because a non-empty input is what selects the action's
  `override: true` arm and displaces the stale stable entry in the cached
  `RUSTUP_HOME`; and the job must own the declaration because `env` resolves
  per job, so a reference to another job's variable is the empty string rather
  than an error. The value is `${{ env.NETSUKE_RUST_TOOLCHAIN }}`, the same
  expression `build-test` and the Windows jobs use, so the repository still
  owns the version in exactly one place. The pinning declaration is duplicated
  across jobs by necessity, not by preference, and
  `workflows_agree_with_the_pinned_toolchain` holds every copy equal to
  `rust-toolchain.toml` and to the input that consumes it.
- Leave `check-build-tools` asserting availability rather than activity, and
  document the gap instead of closing it here. It gates 11 targets and every
  developer path, so widening it is a change with its own blast radius and its
  own review; folding it into this issue would put a shared prerequisite check
  behind a gate-placement fix. Recorded so the next reader does not mistake the
  green `check-build-tools` line for proof the active toolchain is right.
- Reseed the one affected patch in `CycleVisitResult::is_cycle` rather than
  re-pointing the gate at it. The old mutation swapped a `cfg(kani)` arm's body
  for `None`, which strands the `Present` variant; the replacement flips
  `is_cycle`'s predicate so `Present` is still constructed and the fault is
  behavioural. Validated on all three axes: clean under `cargo check`, clean
  under `cargo kani --only-codegen`, and the named harness fails with
  `Failed Checks: self-dependency reports a cycle` while the unmutated tree
  passes.
- Regenerate each patch by swapping an expression in place (a variant, a
  comparator's operands, an `Ordering` constant) rather than deleting a
  statement, so every helper stays referenced and every `mut` binding stays
  reassigned while the seeded fault is still behavioural.
- Accept the CodeRabbit GitHub review's two live findings and fix both, after
  reproducing each rather than reasoning about it. The pass that dismissed the
  earlier three findings was the local CLI against a draft; marking the PR
  ready produced a genuine review with different findings, and the difference
  is the lesson: a draft review and a ready review are not the same review. See
  Surprises & discoveries.
- Keep the new test `#[ignore]`-gated: one Kani codegen per patch (~18 today) is
  far too expensive for the default nextest profile, and costs more since the
  move from `cargo check`.
- Convert this branch's test filter to
  `test(/^every_patched_tree_compiles_under_denied_warnings($|::)/)` when
  rebasing onto `#732`, rather than keeping the `test(=NAME)` form the branch
  was authored with. The form is contract-enforced: the accepted spellings are
  an allow-list, so the old one is not a variant the gate tolerates but a
  selector the contract rejects. Keeping it would have meant weakening a rule
  `#732` had just established across the file, to spare one line of a filter
  that names a test which does not need the anchoring. The cheap correction is
  the correct one here.
- Adopt `#755`'s two repairs *verbatim* rather than repairing the same faults
  independently. Identical content means whichever PR merges second sees a
  no-op hunk instead of a conflict, and this branch's gate is green on its own.
- Dismiss the two CodeRabbit findings that measurement refused, on the evidence
  recorded under Surprises & discoveries above. The `major` finding rests on a
  line count the file does not have, so acting on it would split a 299-line
  file for no reason; the `minor` finding would move prose away from the
  reviewed file's own prevailing style on a point of discretion with no
  correctness content. The third is a separate case and is confirmed rather
  than dismissed — see the `--only-codegen` entry above. The distinction is the
  point: two findings died on measurement, and one survived it, and only
  measurement separates them.
- Widen the gate's `slow-timeout` to ten periods rather than re-running the
  failed job. A re-run would have cleared the check and taught nothing: the
  same head would sit within seconds of the same cap, and the next runner to be
  slightly slower would fail it again. The cost is the host's, so the budget
  has to absorb a slow host rather than assume a median one. Six hundred
  seconds is a chosen allowance inside the ordering, not the ceiling the
  ordering permits: `global-timeout > largest per-test allowance` admits up to
  720 s at 60 s periods under the 780 s budget, so 600 s leaves 180 s of
  margin. The two tiers do bound each other, and the ordering contract is what
  stops this being raised to 780 s without also raising the whole-run budget --
  720 s being the last value permitted rather than the first refused.
  **Annotated 2026-09-23: the reasoning was sound and the value was right, but
  it was reasoned about a filter that bound nothing.** The override's filter
  omitted the `compile_guard::` module path, so it selected no test and the 600
  s was never in force. Nothing in this entry is withdrawn — the cap was real,
  a re-run would have taught nothing, and the ordering argument is what fixes
  600 s — but the decision as *executed* bought no allowance until the
  qualifier landed. A value can be correct and its delivery inert, and only the
  binding, not the value, was ever checked here.
- Qualify the gate's filter with its module path, rather than approximating Rust
  module naming in a static contract. The static contracts compare bare names
  to bare names, which is why they passed on a filter that selected nothing:
  the written name resolves, and only Nextest can say whether the *selector*
  does. Re-deriving `mod` nesting from source to catch the prefix statically
  would approximate — `#[path]` attributes, `cfg`-gated modules, and inline
  `mod` blocks all break a syntactic reading — and would still be a second
  opinion beside the runner's. So the module prefix is admitted by the grammar
  but left unverified there, and the whole-filter replay in the runtime oracle
  closes the gap by asking Nextest directly. The cost is one deliberate hole in
  the static half; the alternative was a check that could be confidently wrong.
- Read the per-test cap from `.config/nextest.toml` rather than from the job's
  `timeout-minutes` or the cargo watchdog. Both outer tiers had ample room (776
  s of 1,800 s), which is why the failure read as a mystery until the innermost
  tier was checked: the tiers are independent, and an outer one having room
  says nothing about an inner one being exceeded.
- Split the per-declaration `slow-timeout` reading into
  `nextest_slow_timeouts.py` rather than trimming prose to fit the cap. The
  module sat at exactly 400 lines and the merge took it to 404, so some line
  had to go; the question was which. Deleting a docstring paragraph would have
  recovered four lines and left the file at 400 again with the next commit
  against it, which is the same failure scheduled rather than repaired. The
  seam is the one already there — `budget_of` reads a single declaration's two
  numbers and nothing else in the module calls it except the aggregate — and it
  is the seam `nextest_durations` and `nextest_totals` were cut along for this
  same cap. The four public functions stay in `nextest_budgets`, so every
  importer is untouched and the split is invisible from outside. Rejected:
  folding the reading back into `nextest_durations`, which is about parsing a
  duration's text and would have acquired a second responsibility; and raising
  the cap, which is a repository-wide policy change and not this issue's to
  make.
- Read `MODULE_PATH` from `_nextest_oracle.grammar` in
  `verify_nextest_anchored_filters.py` rather than restating it. The file
  carried a third copy of the module-qualification rule as a bare literal in
  its instance prefix. The two copies elsewhere — the contracts' and the
  oracle's — are deliberate and documented: the two trees cannot share code, so
  each owns a copy and the comment on each says the other must agree. That
  rationale does not reach this third one, which sits in a file that already
  imports from the package holding the constant. A copy that can drift is the
  defect this script exists to catch, so this is the script's own subject
  matter turned inward. Probed before and after: the imported pattern is `is`
  the grammar's, its `pattern` string and `groupindex` are unchanged, and the
  guard still reports a `compile_guard::neighbour::case_9` stray — so the
  consolidation is behaviour-preserving and the detector is still live, rather
  than merely importable.
- Replay each arm of a top-level union separately, rather than each
  `filter = '…'` value whole. A union is satisfied by any one of its arms, so
  the whole-value replay this branch added could be satisfied by a live arm
  while a dead one beside it left its test running under the defaults — the
  branch's own subject matter, one level down. Every `filter` the configuration
  declares is such a union (17 arms between them), so this was live rather than
  latent. The split tracks bracket depth, because `|` is overloaded: inside
  `test(...)` it belongs to the regular expression, where `($|::)` means "end
  of name or a module separator". A naive split would have cut every selector
  in half and replayed fragments; the probe confirms 17 arms, none fragmentary,
  and confirms the injected dead arm appears as its own arm where the
  whole-union replay would have hidden it.
- Replay the anchored selector *as the configuration wrote it*, rather than
  rebuilding it from the extracted bare name. `_check` did
  `ANCHORED_SELECTOR.format(name=name)`, and that constant carries no module
  path while the config's own `ANCHORED_SELECTOR_IN_CONFIG` admits one — the
  read-side/write-side asymmetry `grammar.py` already warns about, in the one
  reader that had not been brought into line. The A/B is decisive: for the
  gate's test the old code replayed
  `test(/^every_patched_tree_compiles_under_denied_warnings($|::)/)` — the
  *original dead selector from this very bug* — while the file contains the
  `compile_guard::`-qualified one. The guard was verifying a filter the
  configuration does not have. `configured_names` is kept for the name-keyed
  comparisons, which genuinely need bare names; `anchored_selectors` is the new
  read-side function that returns both.
- Fix the masker by scanning positionally, and **decline the review's suggested
  swap** on measurement. The round-eight review proposed reordering the two
  global passes so literals are masked first. That is a real improvement in one
  direction and a regression in the other: it lets a comment containing an
  unmatched quote pair with the next quote in the file and blank everything
  between them, and it still reads `r#"…"#` as a plain literal, which in
  `tests/` (534 raw-string openings across 135 files) invents a phantom
  `probe2` carrying 22 cases. Both orderings fail, in different directions: the
  defect lies in the *ordering* itself, not in which order was chosen. Two
  global passes cannot know which of them is inside the other, so no
  arrangement of them is correct. The fix has to change the shape — one
  left-to-right scan that consumes each span it finds, raw strings before plain
  literals — which is also the shape the repository's own
  `tests/workflow_contracts/rust_source_scan.py::mask_non_code` already uses.
  That scanner was not imported, because `listing.py` documents a wall against
  depending on the test tree ("this script runs on the coverage lane and must
  not depend on the test tree"), so the package keeps its own copy. The
  liveness check is what settles it rather than the argument: three variants
  run against the same eight cases, the shipped masker failing 3, the swap
  failing 2, and the positional scan failing none.
- Restore the code-health score by re-factoring the three simple recognizers as
  anchored regular expressions, **rather than** adding a scoped CodeScene
  exemption. The repository has a precedent for the exemption —
  `.codescene/code-health-rules.json` already zeroes
  `String Heavy Function Arguments` for two hand-rolled parsers, and its
  rationale reaches scanners of borrowed source text — so this was a real
  option, not a straw man. It was rejected because the condition that rationale
  attaches has not been met: the exemption says to reassess "if these parsers
  grow beyond one screen each", and `_non_code_end`'s dispatch plus six helpers
  was doing exactly that, with `SUM_cx` 42 against siblings at 17 and 8. A
  score of 10.00 reached by declaring the rule inapplicable would say "this
  complexity is fine", which was not true. Reached by simplifying, it says what
  is actually the case. The cost of the honest route was one differential
  harness; the cost of the exemption would have been a module whose measured
  health understated its real weight, which is this issue's own subject matter
  turned on the branch fixing it.
- Keep the private `_block_comment_end` walk rather than expressing nesting as
  a regular expression too. Nesting is not regular, and the token walk that
  counts `/*` against `*/` is the one recognizer whose subject genuinely
  demands a loop; replacing it would need a recursive pattern or a depth
  emulation, either of which reads worse than the six lines it covers. Three of
  six helpers became patterns, not six of six, and the split is the construct's
  own: strings, character literals, and the two-token nesting scan are each
  expressible, while nesting is not.
- Hold the re-factoring to the replaced code by differential execution over the
  whole corpus, **before** trusting the contract test that guards it. The eight
  masking cases pin the *behaviours* the rewrite must preserve, not the
  *outputs*; a rewrite can pass all eight while diverging on a construct none
  of them exercises. Running the committed masker and its replacement over
  every tracked `.rs` file and byte-comparing the results is what makes "no
  behaviour change" a measurement. It earned its cost immediately: `.` does not
  match a newline without `DOTALL`, so a Rust literal continued with a
  backslash-newline would have been read as unterminated and blanked the rest
  of the file — a divergence no case in the contract test would have caught,
  and one the 744-case comparison surfaced while the rewrite was still
  uncommitted.
- Refuse an untracked mutation patch rather than skipping it, and refuse it
  *before* the sandbox is created rather than while listing it. Skipping would
  keep the gate's own failure mode alive in a new place: the run would go green
  over a patch it never compiled, and the developer would have no way to tell
  that from a patch that compiled. Failing early also means the error names the
  checkout's paths — the sandbox does not contain the file, so a check placed
  after the export could only report that something is missing, not what. The
  check is deliberately narrow: it asks only whether a patch under
  `docs/verification/mutations/` is untracked, because that directory is
  evidence by definition. Ignored files are excluded, since a file git is told
  to ignore is not something anyone intends the gate to read.

## Outcomes & retrospective

Task 1 delivered: the four patches this branch reseeds now seed the same faults
without dead code, each validated by apply → compile → harness failure → revert
→ harness success. The survey at `00f48f77` widened the count from three to
five; of those two extras, `#755` merged its own repairs on 2026-09-22 and they
now arrive from `main`, while the fourth patch this branch reseeds
(`self_dependency_reports_cycle`) was found separately, by the `cfg(kani)`
census below, and is not one the survey could have seen.

Task 2 delivered: `compile_guard` closes the gap the issue describes. It is
gated (one Kani codegen per patch), registered in the `nested-cargo-builds`
group, reachable via `make test-kani-mutations`, run on every pull request from
`kani-smoke`, and documented in the developer's guide, including the rule that
regeneration must swap an expression in place rather than delete a statement.
The group registration is real as of 2026-09-23 and was not before: the
override's filter named the bare test name, so it selected nothing and neither
the group slot nor the widened allowance it carried applied. It is registered
now, and the widened budget is in force — both proved by probe rather than
argued, and neither claim should be read back onto the earlier runs.

Validation closing the task: the eight deterministic gates pass, `make test`
passes with 3309 tests and 6 skipped, and `make test-kani-mutations` passes
over all 18 patches. The local CodeRabbit pass returned three findings, all
dismissed on the measurements above and none changing a tracked file. The ready
pull request then produced a second pass whose findings *did* change tracked
files — two defects in the gate itself — and a third that moved it between CI
jobs. Marking the pull request ready therefore re-opened validated work, which
is the correct outcome and not a sign the original validation was careless: the
first pass had no way to question a checker the second pass rejected.

CI confirmed the original wiring end to end, in `build-test`. That placement is
superseded by the third finding — the gate needs the Kani frontend, which only
`kani-smoke` has — so the evidence below is retained as the record of a gate
that ran, not as a description of where it runs now. It is worth keeping
because it is the only `build-test` run in this plan where the gate both
compiled and was measured.

The moved gate has since been measured in its own home. The move's first four
runs measured nothing, because the toolchain mismatch described under
`Surprises & discoveries` aborted each before it compiled a single patch. The
fifth, run `35796798133`, is the one that ran: `kani-smoke` concludes
`success`, compiles all 18 patches in **361 s**, and reports the job's first
nextest summary,
`Summary [ 257.703s] 1 test run: 1 passed (1 slow), 3 skipped`. The job totals
**776 s** of its 1,800 s ceiling. Both figures are therefore measured rather
than inferred from headroom — the 1,024 s unspent confirms the ceiling raised
alongside the move was sufficient, on the only run that could say so.

On the `ci.yml` run for `028ac566` (`build-test`, job `106523333782`, 14m27s,
success) the new step is step 32, sitting between "Test and Measure Coverage"
and "Show sccache statistics" exactly as its comment claims, and it executed
rather than being skipped:

```text
PASS [  59.139s] (1/1) netsuke-build::kani_mutation_evidence_tests compile_guard::every_patched_tree_compiles_under_denied_warnings
Summary [  59.139s] 1 test run: 1 passed, 3 skipped
```

Two details from that log are worth keeping. The gate ran under the `ci`
nextest profile, which `build-test` sets job-wide; `ci` declares no
`[[overrides]]` of its own and inherits `default`'s, and that inheritance is
sound. It was **not**, however, carrying the `nested-cargo-builds`
registration, as this plan first recorded — see the correction below. And "3
skipped" is the `--run-ignored ignored-only` flag doing exactly its job: one
ignored test selected, the three fast contract checks left alone. Every other
lane was green on the same SHA, including `kani-smoke` (6m51s) and the Windows
jobs.

**Correction (2026-09-23).** The paragraph above originally claimed the gate
"still picked up the `nested-cargo-builds` registration". It did not, and could
not have: the override's filter was
`test(/^every_patched_tree_compiles_under_denied_warnings($|::)/)`, and Nextest
matches that regex against the fully qualified name, which for this test is
`compile_guard::every_patched_tree_compiles_under_denied_warnings`. The `^`
anchor stopped it matching, so the override — both the group assignment and the
widened `slow-timeout` — applied to nothing. Inheritance was never in question:
a profile does inherit `default`'s overrides, but there was nothing live to
inherit. The 59.1s and 106.6s figures above are real; they simply ran under the
profile's own 300s allowance, not the 600s the override was meant to grant,
which is why the later 300.008s timeout was possible at all.

The final SHA `e4f93b93` re-ran the whole matrix (run `35658680354`) and is
green on every lane: 18 checks pass, none failing, none pending, and GitHub
reports the pull request `CLEAN` / `MERGEABLE`. The gate's cold run there took
106.6s, which the 60s `slow-timeout` warning flags as slow and the 300s
`terminate-after` leaves ample room for — a cold `CARGO_TARGET_DIR` on that
runner compiles libc and the rest of the dependency graph for the first time.
Marking the pull request ready for review also triggered Codex, which reviewed
`e4f93b9` and returned no suggestions.

The retrospective's sharpest lesson is that the two review passes disagreed,
and the second was right. The local pass against a draft produced three
findings that measurement dismissed; the GitHub pass against the ready pull
request produced three that measurement confirmed. Treating the first pass as
the verdict on CodeRabbit's usefulness here would have shipped a CI gate that
can exit 0 having compiled nothing. The habit that caught it is the same one
this plan applies to the patches themselves: reproduce the claim before
believing or rejecting it, and prefer a failing experiment over a plausible
story.

The second lesson is narrower and cost more. A gate is only as good as its
checker, and a checker's blind spot is invisible from inside the gate: every
signal the gate produced was green, the contract test passed, CI ran the step
and reported success, and the patches still contributed no evidence for the one
case that mattered. The question that broke it open was not "does the gate
pass?" but "what does this checker not parse?" — asked of the mechanism rather
than the behaviour. When a gate's subject is a mutation, the checker must at
minimum see the code the mutation touches, and for `#[cfg(kani)]` code only the
Kani frontend does.

The third lesson arrived with the dead override, and it is the same lesson one
layer up. The gate's funding was a filter that selected nothing, so the policy
never applied — and every signal stayed green there too. Two contract modules
passed, because they compared bare names to bare names and the written name
resolved. The runtime oracle passed, because its scope was filters naming a
*parameterized* test and this one names a plain test in a submodule. CI passed
twice, inside the default allowance an inert override predicts. The question
that broke it open had the same shape as before: not "does the guard pass?" but
"what does this guard not replay?" The answer was every filter that does not
name a parameterized test, which is why the repair replays all of them.

The three commits closing this out are gated as one change set: `check-fmt`,
`lint`, `typecheck`, `markdownlint`, `lint-python`, `test-workflow-contracts`
(605 passed, 2 discards) and `make test` (3315 passed, 6 skipped, 123
doctests). The gate's own `make test-kani-mutations` is deliberately outside
that set — the filter, the grammar, and the runtime replay touch nothing it
compiles — and the override it funds was proven bound by the three probes
recorded under `Progress`.

One further commit follows, and it is the only one whose subject was chosen by
CI rather than by this plan: the split of `nextest_slow_timeouts` out of
`nextest_budgets`, after `C0302 (404/400)` appeared on the merge result. It
touches two files under the Python lint gate's own subject matter, so unlike
the change set above it is not exempt from any gate, and the whole set is
re-run over it. The retrospective's fourth lesson is the one the plan did not
see coming, and it is a fifth instance of the same shape: the merge base
passed, the branch head passed, the merge did not, and *no side's green run
could have said so*, because a per-file limit is not a property of either side.
The guard that would catch it is a lint run on the merge result, which is
exactly what CI is.

Task 3 delivered: the runtime liveness guard now replays **each filter as
written** and **each arm of a top-level union separately**. The first half
removes a reconstruction: `_check` used to rebuild the selector from the bare
name it had extracted, so it verified a string the configuration does not
contain — and in the case that motivated the work it replayed
`test(/^every_patched_tree_compiles_under_denied_warnings($|::)/)`, the very
dead selector this plan exists to repair, against a file holding the qualified
form. The second half tracks bracket depth, because `|` is overloaded: at top
level it joins filter alternatives, and inside `test(...)` it belongs to the
regex. All four configured filters are unions — 17 arms between them — so a
dead arm could hide behind a live one, which is this plan's own subject matter
one level down.

The second merge-result failure arrived after all of the above was written. CI
went red on `workflow_loading.py` at `C0302 (403/400)`, a *different* file from
the one the first instance named and by the same mechanism: 367 on this branch,
399 on `main`, 403 merged, with this branch's whole contribution to the file
being one tuple element. The repair restores the branch's net contribution to
zero (3 added, 3 removed) rather than dropping the `kani-smoke` entry, leaving
the merge result at 399. It was verified with the repository's own pylint
wrapper against a real merge tree — 10.00/10 over every `.py` file this branch
touches — and liveness-checked by padding the merged file past the cap and
confirming C0302 fires at 404 while staying silent at 399.

Raw per-run figures for the code change above, all nine gates green:
`check-fmt` 2 s, `lint` 13 s, `typecheck` 1 s, `markdownlint` 12 s,
`lint-python` 8 s, `test-workflow-contracts` 27 s, `test` 222 s, `nixie` 2 s,
`doc-coverage` 7 s. Two caveats keep that honest, and both are the reason this
plan does not treat the set as covering the document it appears in. Those nine
ran against the tree holding the Python fix; this ExecPlan's own entries were
written afterwards, so the four Markdown- and Rust-sensitive gates among them —
`check-fmt`, `lint`, `typecheck`, `markdownlint` — measured a tree that did not
contain the paragraphs above, and the set is re-run over the final tree rather
than assumed to carry. This is the same distinction the plan makes elsewhere: a
green gate is evidence about the tree it ran against, and nothing more. Per the
run-id convention the figures are recorded as durations against the branch head
rather than against a self-series SHA, which a rebase would invalidate.

**One residual this work leaves open, recorded because it bears on the
`kani-smoke` entry this branch added.** `NEXTEST_JOBS` is *iterate-only*. Its
sole consumer, `tests/workflow_contracts/ci_lint_test.py:262`, loops over the
tuple asserting each listed job installs `nextest@${{ env.NEXTEST_VERSION }}` —
which holds the listed jobs to their contract but cannot notice a job that
should be listed and is not, nor a listed job that is removed. Deleting the
`kani-smoke` element this branch added would therefore fail no test; the loop
would simply run one fewer time, and the gate's install step would go
unchecked. The Makefile-level analogue is bidirectional — `NEXTEST_TARGETS` is
checked against the targets the file actually invokes, in both directions — so
the workflow-level list is the weaker of the two, and it is the one this branch
extended. Closing it means deriving "which jobs run nextest" from the workflow
rather than from a hand-kept tuple, which is a broader change than this issue
and is deliberately not made here.

The `kani-smoke` entry is not unguarded meanwhile, and the guard is closer than
"nearby". `runner_shape_test.py:60` lists the job against `NEXTEST_TEST_JOBS` in
`UBICLOUD_WORKER_BOUNDS`, and the parametrized test at line 129 fails when a
listed job does not declare that variable
(`jobs.kani-smoke must declare ['NEXTEST_TEST_JOBS']`). Since only a nextest
lane sets it, removing the job's nextest wiring breaks that test. What remains
unguarded is therefore the narrower thing than first written here: not the
presence of the entry, but `NEXTEST_JOBS` being the authority on which jobs
install nextest. A job that began running nextest without joining the tuple
would install no nextest and fail no test in this suite — which is exactly the
"evidence that looks healthy" shape this plan is about, recorded rather than
fixed because the fix is a broader change than the issue.

The round-twelve findings are repaired and the repair is measurable. The
extraction takes `python_toolchain_sync_test.py` from 402 lines to **363** — 37
lines of headroom under the cap, against the zero it had before this branch
began — and `makefile_recipes.py` to 192, both well clear. Behaviour is
preserved by side-by-side measurement rather than by inspection: the old and
new implementations were run together over four variables, seven targets, and
four command variables, and returned **15 of 15 identical verdicts**. Ruff,
pylint, interrogate at 100.0%, and `ty` all pass on both files, and the pinned
`typecheck-python` assertion still holds each search root to its own substring,
so the Makefile and the contract remain matched. The two en-GB-oxendict
spellings are corrected in place.

What that repair cost, and what it leaves open, is worth stating plainly. It
was needed because the file had **no headroom**: `main` sat on the last passing
value of a hard cap, so a two-line wiring fix had nowhere to land. That is not
a property of this branch's change and not something a green run could have
surfaced — CI compares trees, and the tree was over. The general remedy is the
one this directory already applies: keep extraction as the answer when a file
approaches the ceiling, as `makefile_recipes.py` itself records having done.

## Revision note

- 2026-09-21 — Initial ExecPlan for `#756`: regenerate the rotted patches, add
  the gated compile contract, wire it into the build and CI, and document it.
- 2026-09-22 — Re-opened on the ready pull request's CodeRabbit pass. The gate's
  checker was replaced (`cargo check` to `cargo kani --only-codegen`) and the
  gate moved from `build-test` to `kani-smoke`, with the CI contracts, the
  developer's guide, and the `kani-smoke` timeout updated in step. Status
  returned to `IN PROGRESS`.
- 2026-09-22 — Section headings restated in sentence case per
  `docs/documentation-style-guide.md`'s `## Headings` rule. The
  `Surprises & discoveries` and `Decision log` names now match the spellings
  the style guide's own `### ExecPlan` section uses.
- 2026-09-22 — The moved gate failed on its first real CI run: the job's
  inherited `RUSTUP_TOOLCHAIN: stable` met `GATE_RUSTFLAGS`'s nightly-only
  `-Zthreads=8`, and the gate aborted at toolchain selection having compiled
  nothing. The variable is removed, the mechanism recorded under
  `Surprises & discoveries`, and a final CI confirmation left open as the last
  unchecked Progress item.
- 2026-09-22 — The second CI run failed identically, so the removal had not
  been the whole fix. `Setup Rust`'s `toolchain: stable` input writes a
  `rustup` directory override that outranks `rust-toolchain.toml`, and it
  outlived the variable's removal; the step now names the nightly pin. The
  first explanation recorded here — that the redirected `RUSTUP_HOME` hid the
  pin from rustup — is **disproved** and was replaced, along with the "remove
  rather than pin" decision it justified. Corrected above.
- 2026-09-23 — The third and fourth CI runs failed identically again, which
  **disproved the second explanation too**. The step's `toolchain` input was
  reading `${{ env.NETSUKE_RUST_TOOLCHAIN }}` from a job that never declared
  the variable, so it expanded to the empty string; the directory override that
  selected stable was a *stale entry in the restored `.kani-rustup` cache*, not
  one this job set. The pin is now declared in `kani-smoke`'s own `env` block,
  `workflows_agree_with_the_pinned_toolchain` covers the job and both
  directions of the pin-versus-input invariant, and the Surprises & discoveries
  entry records why two plausible mechanisms were accepted before the third was
  measured.
- 2026-09-22 — Rebased onto `origin/main` after PR `#732` landed, replaying
  eleven commits. Three conflicted. `.config/nextest.toml` conflicted on
  substance: this branch's filter used the `test(=NAME)` grammar `#732` had
  just replaced file-wide with the anchored form, so it was converted rather
  than preserved. The two `ci.yml` conflicts resolved in opposite directions
  and for the same underlying reason — `build-test` keeps `#732`'s new
  anchored-filter verification step while dropping the mutation gate, because
  the later commit `1cd39169` relocates that gate to `kani-smoke`, which only
  parses `#[cfg(kani)]` code. Every file this branch authored survived the
  replay byte-identical; verified by whole-tree diff against the pre-rebase
  head rather than by ancestry, since a rebase rewrites every SHA.
- 2026-09-23 — The fifth CI run (`35796798133`) is the first to reach the gate
  and compile, which closes the last Progress item: `kani-smoke` succeeds, the
  gate takes **361 s** over all 18 patches, and the job totals **776 s** of its
  1,800 s ceiling. All five jobs of that workflow conclude `success`. The
  `Outcomes & retrospective` section no longer describes the gate's CI cost as
  unmeasured, and the header splits its conjunction — the CI half of the
  condition for returning to `COMPLETE` is met, the review half is not.
- 2026-09-23 — Established that the GitHub CodeRabbit review has **auto-paused**
  rather than merely lagged. The pull request timeline holds exactly one
  `coderabbitai` review event, `changes_requested` on `e4f93b93` at
  `2026-09-21T21:47:54Z`; every head since receives a `success` commit status
  within about three seconds, which is the pause marking the commit handled.
  The header now states this, and notes that the pinned `CHANGES_REQUESTED` is
  non-ancestral after the rebase — verified with
  `git merge-base --is-ancestor`, not inferred from its age. Recorded because a
  green `CodeRabbit` status on this pull request means "nothing pending", not
  "reviewed and passed".
- 2026-09-23 — Found that the override funding the gate had never applied, and
  repaired it. `.config/nextest.toml`'s fourth `[[profile.default.overrides]]`
  carried both the 600 s allowance and the `nested-cargo-builds` registration,
  and its filter
  `test(/^every_patched_tree_compiles_under_denied_warnings($|::)/)` selected
  no test at all, because Nextest matches that regex against the fully
  qualified name and the test is declared in `compile_guard`. The filter now
  carries the module prefix. The accepted filter grammar was widened in both of
  its copies to admit an optional, **non-capturing** module path, so the
  captured name stays bare for the consumers that compare it against
  `declared_test_names`, and the developers-guide entry for this rule gained
  the third silent-failure cause alongside the two it already listed. The
  runtime oracle's scope was the real gap — it asked Nextest which tests a
  filter selects, but only about filters naming a parameterized test — so
  `verify_nextest_anchored_filters.py` now replays every filter expression
  verbatim and requires each to select something. Both the Progress item
  recording the widening and the Decision log entry arguing it are annotated
  rather than rewritten: the reasoning and the value held, the delivery did not.
- 2026-09-23 — Corrected two passages this discovery had falsified in place.
  The Progress narrative claimed the gate "still picked up the
  `nested-cargo-builds` registration" on the `build-test` run and that profile
  inheritance was what carried it; it did not, and a profile inheriting
  `default`'s overrides was never the mechanism at issue. The
  `Outcomes & retrospective` claim of registration is now time-qualified. Both
  were written from the configuration's intent rather than from the runner's
  behaviour, which is the same mistake one layer below the issue this plan
  exists to close.
- 2026-09-23 — The runtime guard was widened and the three-commit change set
  pushed. The guard now replays every filter verbatim, and the reading moved
  into a `_nextest_oracle` package beside the entry script because the file sat
  at 385 of the 400-line cap. Two corrections landed with it:
  `--run-ignored all` is load-bearing for the listing (without it an
  `#[ignore]`-gated test reads as `mismatch` whatever the filterset says, so
  the gate's own filter would report as selecting nothing), and this file's
  reading grammar needed the same module-path widening the contracts received,
  or the qualified filter stays invisible to `configured_names`. The heading
  above the entry-script reading in the developers-guide gained the package's
  existence.
- 2026-09-23 — **A method error worth recording, caught by the gate runner.**
  Every mdtablefix "convergence check" run while preparing these commits was a
  dry run: the tool rewrites only under `--in-place`, and without it, it merely
  reports what it *would* change. So a `cmp` between two consecutive no-op runs
  compared two identical unmodified files and reported success, and the
  developers-guide paragraph shipped over-wrapped. `make check-fmt` caught it
  (scrutineer proved the file canonical at `origin/main`, `HEAD`, and the
  index, and non-canonical only in the working tree), and the fix is one
  `--in-place` run. The general form matters more than the flag: a check that
  cannot fail — here, diffing a file against itself — is not evidence, which is
  the same lesson as the probe under `Surprises & discoveries` that measured
  the wrong process.
- 2026-09-23 — CI's first verdict on the pushed head was **red**, and the
  failure was one the branch could not have caught locally: `C0302 (404/400)` on
  `nextest_budgets.py`, in the merge result only. The branch head is exactly
  400 lines and passes bare; `main` independently added four more. `kani-smoke`
  was green in the same run, and the compile gate passed at `269.637 s` under
  the 600-second override — the first head where that override binds, so the
  widening this plan is largely about is now confirmed on CI rather than only
  probed. The split is recorded under `Progress`, its general statement under
  `Surprises & discoveries`, and both are the same lesson the plan already
  carries: evidence that looks green on the visible surface, while the surface
  that decides is a different one. `make test-kani-mutations` was deliberately
  not re-run locally; the gate's cost is what the override exists to
  accommodate, and CI is where it binds.
- 2026-09-23 — Consolidated the module-qualification rule to one definition.
  `verify_nextest_anchored_filters.py`'s instance prefix restated `MODULE_PATH`
  as a bare literal, making three copies of a rule this branch had just widened
  in two places. The duplication in the contracts and in the oracle is
  deliberate — the two trees cannot import from each other, and each comment
  says the other must agree — but the third copy had no such justification,
  since the file already imports from the package that owns the constant. A
  rule written twice is a rule that drifts once, silently, which is the exact
  failure this script was widened to catch. The probe is recorded under the
  Decision log; a green import would not have been evidence, so liveness was
  re-established by confirming the guard still reports a stray in a submodule.
- 2026-09-23 — **CodeRabbit's review of the frozen head returned six findings,
  and all six were accepted.** Two were the branch's own subject matter turned
  on its own new code, which is the outcome the plan should have expected: a
  guard widened to catch a dead selector was itself replaying a dead selector
  (the bare-name rebuild above), and the whole-value replay it was widened with
  could still hide a dead union arm. Those two are recorded in the Decision log
  with their A/B and their liveness probes. The rest: the branch had written
  `serialised` in `runner_shape_test.py` where the repository spells
  `serialized`; the `workflow_ci.rs` ceiling comment claimed no CI run had yet
  measured the cold case, which run `35796798133` had already done at
  `257.703 s` — the claim was true when written (22:29Z) and was falsified an
  hour later by the run that passed it, so it is corrected with the measured
  figures rather than deleted; and two passages carried second-person pronouns.
  One finding's arithmetic is the most useful of the six: the config, the
  guide, and this plan all asserted that 600 s is "the largest value the
  ordering permits", but `whole_run_ordering._per_test_faults` faults only when
  `whole_run <= largest`, so at 780 s and 60 s periods the ceiling is **720 s**
  and 600 s is a chosen allowance with 180 s of margin. Probed rather than
  reasoned: driving the real reader over `terminate-after` 5, 10, 11, 12, 13
  gives 300 s / 600 s / 660 s / 720 s / fault, so 720 s is the last permitted
  value and 780 s is where the inversion begins. Those are two figures, not
  one, and the first correction conflated them in two of the five places it
  landed. A claim about why a number was chosen is exactly the kind that
  survives every gate, because no gate reads prose for its arithmetic.
- 2026-09-25 — Task 3 delivered, and the plan's own subject matter recurred at
  a third level. The runtime guard now replays each filter as written and each
  union arm alone; the first half removes a reconstruction that had been
  replaying the dead selector this plan was opened to repair, and the second
  tracks bracket depth because `|` joins alternatives at top level while inside
  `test(...)` it belongs to the regex. All four configured filters are unions,
  17 arms between them, so the union half was live rather than latent.
- 2026-09-25 — The C0302 merge-result failure happened a **second** time, in
  `workflow_loading.py` (403/400) rather than `nextest_budgets.py` (404/400),
  the day after the first was recorded here in bold as a general rule. The rule
  did not prevent it, and could not: it is a rule about merges, and no reading
  of either side's file can check it. Contributing one tuple element to a file
  sitting at 399 is sufficient. Repaired by restoring the branch's net
  contribution to zero rather than by dropping the entry that mattered, and
  verified with the repository's own pylint wrapper over a real merge tree
  (10.00/10 across every `.py` file this branch touches), liveness-checked at
  404 lines. Two probe traps are recorded under `Surprises & discoveries`, of
  which `git write-tree` writing the *index* is the one that nearly produced a
  false confirmation.
- 2026-09-25 — A correction worth keeping because of its shape rather than its
  size: the four-line comment removed to reclaim the cap margin was described
  here and in the pull request as restating the docstring above it. It did not.
  The docstring answers *which jobs install nextest*; the comment answered *why
  the gate lives in `kani-smoke`*, and that rationale survives at length in
  `ci.yml` beside the gate's step and in the developer's guide beside the
  measured `cargo check` counter-example. The removal was still correct — it
  was the third copy, and the cheapest to spend — but the reason first written
  for it was not, and was corrected in both places rather than quietly reworded.
- 2026-09-25 — **The `#755` provenance read as false once the base moved.** PR
  `#755` merged on 2026-09-22 (`ee0e5523`), before this branch's own base
  (`30c50e27`), so the two patch files it repaired now arrive from `main` and
  the rebase dropped this branch's verbatim adoption of them as redundant. The
  Progress item, the survey entry, and the `Outcomes` passage all described the
  adoption in the present tense — "`#755` has not merged", "whichever PR merges
  second" — which was accurate when written and is not now. Corrected in the
  direction the tree actually is, and the adoption is recorded as having really
  happened rather than deleted: it was folded into `a5b8a928`, which touched
  both patch files, and the blobs are byte-identical to `#755`'s own repair
  commit (`3a282018`). The pull request's scope note and References section
  carried the same staleness and were corrected with it. Recorded because this
  is the plan's own subject one level up: a claim that was true on the tree
  where it was written, and that nothing re-checks when the tree moves.
- 2026-09-25 — **CI confirmed both fixes on the pushed head `10270c37`.** The
  C0302 repair is verified in the place the failure actually occurred: the
  `build-test` job's `Lint` step is `success`, where the previous head
  `3e88ff63` failed at the same step with:

  ```text
  tests/workflow_contracts/workflow_loading.py:1:0: C0302: Too many lines in module (403/400)
  ```

  That step number is the same in both runs (24), so the comparison is like for
  like, and the new log carries no `C0302` or `too-many-lines` anywhere.
  Separately, `kani-smoke` concludes `success` and its **"Mutation patch
  compile gate"** step runs `19:45:33Z → 19:51:12Z` — **339 s** inside the 600
  s allowance, 261 s of margin. That is the first head on which the whole chain
  is green together: the reseeded patches, the module-qualified filter, the 600
  s override actually binding, and the gate compiling all 18 patched trees.
  Nothing in this entry is inferred from a job conclusion alone; each figure is
  read from the step that produced it.
- 2026-09-25 — The round-eight review read the oracle's masker and found the
  two-pass ordering: `LINE_COMMENT` ran before `STRING_LITERAL`, so a `//`
  inside a literal ate its closing quote and blanked the rest of the line.
  Fixed by scanning left to right and consuming each span as it is found,
  rather than by the swap the review suggested — which breaks the opposite way
  and invents a phantom `probe2` from the raw strings in `tests/`. A regression
  test pins the behaviour and was liveness-checked against all three variants.
  Measured over `tests/` with the committed code: 187 functions / 752 cases
  corrected against 173 / 688 as shipped, 24 names differing. Every one was
  latent, because `_check` runs only for names both configured and in the
  oracle's dict, which is this issue's own failure mode one level up. The same
  head's gates then failed on three counts, all in the work the previous commit
  had just added and none in the Rust change it was gating: a missing `Returns`
  section (ruff), an import `ty` could not follow (fixed by naming
  `.github/scripts` as a second search root, with the wiring contract updated
  in step), and a Markdown refill on the ExecPlan. Recorded because the pattern
  is the point — the gates were red on the tree, and the tree was the one
  carrying the commit.
- 2026-09-25 — **The round-twelve run's two findings are repaired.** The
  `makefile_recipes` extraction takes `python_toolchain_sync_test.py` from 402
  to 363 lines, so the cap is cleared with 37 lines of headroom rather than
  re-landed on its ceiling; the two en-GB-oxendict spellings (`artifact`,
  `mis-ordering`) are corrected in place. Both were in this branch's own work
  rather than in the Rust change it gates, and the `C0302` was only visible
  because the preceding ruff failure had been repaired — the same "green
  because the reader stopped short" relationship this plan is about, now on the
  sixth recurrence. The extraction is the repository's own remedy, named in
  `makefile_recipes.py`'s docstring and used six times before it. The wiring
  contract's pinned `typecheck-python` assertion is unchanged: the Makefile
  still passes both search roots, and the contract still asserts each as a
  separate substring, which is what keeps the two sides matched.
- 2026-09-26 — **Repairing the `C0302` unmasked a fifth stage, which found five
  more defects in the same file the cascade had been hiding.** The chain is now
  three deep and every link is the same link. The round-eleven run stopped at
  ruff, so pylint's `C0302` never printed. Fixing ruff's finding let
  `lint-python` run past pylint, and the **df12 stage executed for the first
  time** on this branch:

  ```text
  tests/workflow_contracts/nextest_oracle_masking_test.py:133:4: C9102: Assert
  statement lacks a failure message (assert-missing-message)
  ```

  Five findings, four `C9102` and one `C9106`, all in
  `nextest_oracle_masking_test.py` — the file `c4b18859` added. That the
  convention they encode is absolute rather than advisory was established by
  measurement against `main`, not by reading the rule's description: of **882
  `assert` statements** across `.github/scripts`, `scripts`, and
  `tests/workflow_contracts`, **zero lack a message**, and every `ruff: ignore`
  in the tree carries a hyphen-and-explanation suffix. This branch's file was
  the only one in the repository violating either. All five are repaired, and
  the stage now rates 10.00/10. The lesson is the cascade's own, stated
  exactly: a stage that has never executed has said nothing, and "the first
  error was fixed and a different one appeared" describes a file being sampled
  one stage at a time, not a defect arriving.
- 2026-09-26 — **The round-thirteen run found a fourth masking mechanism, and
  this one is not a lint stage.** Two `MD038` findings landed on this
  document's own prose, on the phrase describing the `ruff: ignore` convention.
  The offending span opened with a space ahead of its hyphen — the very space
  that separates the suffix from the rule name it explains — and `markdownlint`
  calls that "spaces inside code span elements" and rejects it. The finding was
  **new** — both lines are additions in the working tree, absent from `HEAD` —
  and it was invisible in round twelve for a reason worth recording:
  `markdownlint: spelling` declares a **prerequisite**, so the `spelling` gate
  runs first and, when it fails, `markdownlint-cli2` never executes at all.
  Round twelve's `spelling` was red on the two en-GB spellings, so the Markdown
  linter had never once run on this branch. That is the same shape as the
  `lint-python` cascade with the mechanism moved one level up: a *prerequisite*
  short-circuits its target exactly as a failing stage short-circuits the
  stages behind it, and a target that never ran has said nothing. "The linter
  passed" and "the linter never started" are indistinguishable from the exit
  code alone. The repair is prose rather than a rewritten span: the suffix
  genuinely does begin with a space, so the hyphen-then-explanation form is
  described in words, since no code span may hold a leading space here —
  `markdownlint --fix` would silently strip it and state something false about
  the convention. The two gates that own the question (`markdownlint`,
  `spelling`) and `check-fmt` are green on the result, with
  `mdtablefix --check` agreeing after its own refill.
- 2026-09-26 — `CodeScene Code Health Review (main)` failed on the pushed head,
  and the finding was the branch's own. The positional masker added in
  `c4b18859` fixed a real defect and, in doing so, took `listing.py` from 3
  functions and `SUM_cx` 8 to 9 and 42 — scoring it 9.38 where it had been
  10.00, against siblings at 17 and 8. Three of the six new recognizers were
  simple enough to be patterns rather than loops, so the module was re-factored
  to `SUM_cx` 33 with the worst function at 5 and the nesting walk left as a
  walk. Measured with the vendor's own CLI, which reproduces the gate locally
  and named the failed rule as fixed: `Code Health: (9.38 -> 10.00)`. The
  rewrite was held to the code it replaced by a differential harness over 744
  cases (33 adversarial synthetics and every tracked `.rs` file) comparing
  byte-for-byte, which caught a `DOTALL` divergence the eight contract cases
  did not. The eight remain green.
- 2026-09-26 — The sandbox migration turned out to narrow the gate's input, and
  the narrowing was closed in the same work. Reading a captured revision
  instead of the working tree drops any *untracked* patch from the run, which
  would have made the gate pass over strictly fewer patches than exist while
  looking identical — this issue's own shape, one level deeper.
  `ensure_no_untracked_patches` now refuses that state before the sandbox is
  built, with the premise measured in a scratch repository (a revision produced
  by `git stash create` exports the tracked patch and not its untracked
  sibling) and the guard itself probed in three states, including that an
  ignored file stays silent. A cost is recorded rather than hidden: the
  ignored-file probe wrote to `bare.git/info/exclude`, shared by every worktree
  of this repository. The file is restored to git's default template and
  re-verified inert; its prior content is not recoverable. The probe belonged
  in a throwaway repository, and a shared-state probe that truncates before
  reading is the reason that rule exists.
- 2026-09-26 — The header's account of the review was corrected, because a
  later event falsified it. It had read "The GitHub CodeRabbit review has not
  simply lagged; it has **auto-paused**", which was accurate on 2026-09-23, and
  drew from that the conclusion that no review was newer than `21:47:53Z` on
  2026-09-21. On 2026-09-25 a second pass ran unprompted and posted
  `changes_requested` on `d8d0ba6d` at `23:35:01Z`, naming two findings that
  were both accepted. The count of `coderabbitai[bot]` review events on this
  pull request is therefore **two**, not one, and the newest is not `e4f93b93`.
  The old text is kept above the correction rather than deleted: it described a
  real transient state and then read a forecast out of it, presenting the
  forecast as an observation. A claim about the present that a future event can
  overturn should say when it was made and what would falsify it.
- 2026-09-26 — A `coderabbit review --agent` pass was run against `ebd70e4e`
  and returned seven findings, all `minor` or `trivial`, none `major`. Every
  premise was checked before it was acted on and all seven were real, spread
  over five sites, and `git diff` confirmed four of the five files were this
  branch's own — so this was not inherited drift. Two sites were introduced by
  this branch's own earlier commits (`fd8899c9` and `c6363342`), which is worth
  recording plainly: the gate findings had been cleared round after round while
  these sat inside the reviewed set.

  One finding was **declined as stated and fixed another way**, and the reason
  matters more than the fix. The reviewer's remedy for `patch_paths` was to
  "apply the same filter here" — skip anything without a `.patch` suffix. That
  would have made the gate green over fewer patches than the directory holds,
  which is this issue's exact failure mode one turn deeper: silent shrinkage
  rather than silent success. The sibling contract `patch_stems` already had
  the right shape, refusing a stray by name, so the fix mirrors it and the
  message names the offending path.

  Two further lessons. First, `shlex.split` had to be applied to the whole
  `NAME=value` word and not to the captured value: the fragment `\'` is
  unterminated once the surrounding quotes are stripped, so the obvious
  spelling of the fix raises on exactly the input it was written for. Verified
  against the real shell across six shapes, including the apostrophe case and
  the live `show-env` output. Second, a fix can be correct and still wrong in
  shape — `shlex.split` also needed to refuse an unquoted value containing
  whitespace by name, because truncating at the first space would export a
  plausible-looking wrong path, the same class of fault as the escaping bug.

  The `404` figure restored to the entry above was eaten by
  `mdtablefix --renumber` because it began a wrapped continuation line, where
  the pass reads a numeral followed by a period as an ordered-list marker and
  renumbers it. The lost content was a measurement rather than a list number.
  The repair keeps the numeral mid-line, which is where the pass cannot read it
  as a marker; `make check-fmt` is what proves the repair holds, since it runs
  the same pass that caused the damage.
- 2026-09-26 — A second `coderabbit review --agent` pass ran against `674e266f`
  and cleared all five round-one sites, with its coverage count exact at
  `33/33`, but raised two new `trivial` findings. Both were real and both were
  duplication rather than defects, which is the shape worth recording: neither
  would have failed a gate, and each was a place where a fact was written twice
  and could therefore drift.

  The first was `SANDBOX_DIR` in `sandbox.rs`, a second constant spelling a
  path the file already spelled as `SANDBOX_NAME` under `target/`. The fix
  removes the redundant constant rather than reconciling the two, so the
  directory created and the directory emptied are now built from one name.
  Before deleting it, the file was checked for consumers outside it — the
  constant was `pub(super)`, so an in-crate consumer was possible — and there
  are zero hits repo-wide; the derived path is byte-identical to the old
  literal.

  The second was `fail` in the oracle's runner, annotated `-> None` while its
  body ends in a raise. That is the same class of defect this branch exists to
  catch, one level down: a type that describes a reachable path which is not
  reachable, so a caller reading the two as equivalent would treat the lines
  after a `fail` call as live. Annotating it `typ.NoReturn` is the house
  convention already (`scripts/tests/test_doc_coverage.py` uses it twice) and
  needs no new dependency, since `typing` is imported as `typ` throughout.

  **The gate caught the second fix twice, and both times the rule was right.**
  Widening the one-line docstring into a body gave ruff's pydocstyle checks
  something to inspect, and `docstring-missing-exception` fired naming
  `SystemExit` — the raise the old annotation had hidden from the checker as
  well as from callers. Adding a NumPy-style `Raises` section, matching the
  house format in `scripts/coverage_artifact_archive.py`, satisfies it. The
  lesson is that a docstring with no body is not held to the same standard as
  one with a body, so expanding a docstring can surface a real omission rather
  than a spurious one; the fix is to answer the rule, not to retract the prose.

- 2026-09-26 — The pull request body was re-read against the live pull request
  rather than against the prepared artefacts, and it is already correct: body
  and prepared copy differ only by a trailing blank line, so the update landed
  before this check. Verified in the live body: the title carries `(#756)`,
  `closingIssuesReferences` resolves to exactly `[756]`, the `## References`
  section carries the session URL, the attribution line is present, and the
  "The widening was inert as first written, and is not now" section carries the
  correction. The lesson is to read state from the system of record before
  acting on a note-to-self, because a pending-task list decays: it recorded
  work that had in fact been done, and re-applying the prepared body would have
  been a no-op at best. One detail was still worth correcting, and only because
  it was checked rather than trusted — the prepared block labelled the pronoun
  finding `` `low` ``, but the review's own inline comment labels it
  `🟡 Minor`. The label now matches the source.

- 2026-09-26 — **A falsification that was nearly recorded, and was false because
  the tool version was guessed.** Having annotated `fail` as `NoReturn` on the
  reviewer's suggestion, the obvious next question was whether the annotation
  does anything — so a probe was built: a union narrowed by a guard whose arm
  calls `fail`. `ty` rejected it under *both* annotations, which reads as the
  annotation being inert and the docstring's stated reason being false. That
  reading is wrong, and the reason is worth keeping: the probe was run with
  `--from ty==0.0.1a34`, a version invented on the spot from the shape of ty's
  release tags. `0.0.1a34` is an *alpha of 0.0.1*, which sorts **older** than
  the `0.0.74` this repository pins, so the probe measured a six-month-old
  checker and reported a limitation the pinned one does not have. Re-run
  against the pinned `0.0.74` the result inverts exactly as the docstring
  claims: `-> None` fails to narrow and the use site is rejected, `-> NoReturn`
  narrows and it passes. The docstring now states the measurement rather than
  the intuition.

  Two lessons, and the second is the sharper. A control that isolates the
  variable is not enough if the *instrument* is unverified — the "control" here
  (a direct `raise` in the guard, which narrowed under every version) correctly
  proved that narrowing was implemented at all, and that is exactly what made
  the union result look like a real limitation rather than a version artefact.
  The instrument's own version had to be pinned to the repository's, and that
  check had no equivalent of the direct-raise control guarding it. The
  falsification also arrived late enough to be tempting: a fixed finding, a
  green gate, and a probe that agreed with neither — the moment to distrust a
  probe is when it contradicts a verified fix.

- 2026-09-26 — **An enumeration that did not sum to the total stated beside
  it, in the pull request body, found by re-reading the live pull request
  rather than the prepared text.** The passage listing the `reviewed` events by
  actor read "**25** `codescene-access[bot]` approvals, **2**
  `coderabbitai[bot]`, **1** `sourcery-ai[bot]`, and **1**
  `chatgpt-codex-connector[bot]` — 30 in all". Those four figures sum to
  **29**, not 30, so at least one was wrong and the sentence could not be
  self-consistent at any instant. Reconstructing the running counts event by
  event shows the two figures came from different moments: CodeScene passed 25
  at `23:25:13Z` on 2026-09-25 (total 28), and the total reached 30 at
  `01:47:13Z` on 2026-09-26 (CodeScene 26). The pair `(25, 30)` never
  co-occurred. Both are now restated as of one named instant,
  `2026-09-26T03:00:02Z`, with the total computed from the parts.

  Two things are worth keeping. First, the defect was invisible to every gate:
  no formatter reads arithmetic, and the sentence parses perfectly — it is only
  wrong. Second, the count had *already* decayed once before, from a claim of a
  fixed figure to a dated observation, and the dating is what made the error
  findable now: because the prose said which moment it described, the figures
  could be checked against that moment instead of against the present, and the
  mismatch between the parts and the whole is what exposed two different
  moments wearing one date. A hedge that names its instant is not just more
  honest, it is more testable.
