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

The GitHub CodeRabbit review has not simply lagged; it has **auto-paused**. Its
own comment on PR `#766` reads "Reviews paused … this branch is under active
development", offering `@coderabbitai resume` and `@coderabbitai review` as the
ways out. So the `CodeRabbit` commit status of `success` posted at 23:19Z on
`7143648c` is the pause path reporting completion, **not** a review of that
head: no inline comment on this pull request is newer than `21:47:53Z` on
2026-09-21, and every one of the three predates the commit that fixed it. The
lingering `CHANGES_REQUESTED` is correspondingly pinned to `e4f93b93`, which a
rebase has since made non-ancestral — it describes a revision on a superseded
history line, so its diff matches no code that exists. Clearing or waiving it
is a maintainer action, not something this branch can do; what the branch can
do is re-run the review and leave the record unambiguous. That is the remaining
work.

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
      conflict whichever lands first.
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
      is the targeted-override case that file's own policy asks for and the
      largest value the ordering permits, since `global-timeout` (780 s) must
      stay strictly above the largest per-test allowance. The figure moved from
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

## Surprises & discoveries

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
  cannot fail for the reason you think is not evidence — and it is worth
  recording that the second, *correct* run reported both defects as caught.
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
  patches at `main` (`00f48f77`) showed `marker_token_match_is_exact` and
  `scanner_agrees_with_independent_specification` also fail — these are exactly
  the two PR `#755` repairs, and `#755` has not merged. `main` and `#755`'s base
  (`61a944fb`) have identical blob hashes for all five affected files, so the
  mechanism reproduces on a clean `main`. They are in scope for `#755`, not
  `#756`.
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

## Decision log

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
  seconds is not a free choice — it is the largest value that keeps
  `global-timeout > largest per-test allowance` — so the two tiers bound each
  other, and the ordering contract is what stops this being raised further
  without also raising the whole-run budget. **Annotated 2026-09-23: the
  reasoning was sound and the value was right, but it was reasoned about a
  filter that bound nothing.** The override's filter omitted the
  `compile_guard::` module path, so it selected no test and the 600 s was never
  in force. Nothing in this entry is withdrawn — the cap was real, a re-run
  would have taught nothing, and the ordering argument is what fixes 600 s —
  but the decision as *executed* bought no allowance until the qualifier
  landed. A value can be correct and its delivery inert, and only the binding,
  not the value, was ever checked here.
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

## Outcomes & retrospective

Task 1 delivered: the three named patches now seed the same faults without dead
code, each validated by apply → compile → harness failure → revert → harness
success. The survey widened the count from three to five, and the two extras
were adopted from `#755`.

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
