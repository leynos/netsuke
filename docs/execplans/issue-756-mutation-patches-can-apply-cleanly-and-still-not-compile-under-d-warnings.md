# Add a compile gate to the Kani mutation evidence contract

Status: IN PROGRESS

Re-opened on 2026-09-22. The plan was `COMPLETE`; the ready pull request's
CodeRabbit pass then found that the gate's checker could not see the code the
mutations touch, which invalidated the gate's design and its CI placement
together. The status returns to `IN PROGRESS` until the moved gate is validated
on CI and that review concludes.

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

## Surprises & discoveries

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

## Decision log

- Replace `cargo check` with `cargo kani --only-codegen` in the compile gate,
  after measuring that the checker was blind to one whole class of fault. This
  is the third CodeRabbit finding's subject and it is confirmed, not declined:
  `cargo check` never parses `#[cfg(kani)]` code, so a patch that seeds its
  fault inside a Kani-gated arm compiles clean under the gate while
  `cargo kani` rejects the very same tree. `--only-codegen` is a superset (it
  is a full compile under `cfg(kani)`), so it replaces the check rather than
  joining it, and it is affordable: a measured 172 s for all 18 patches into a
  fresh shared target directory, 8–13 s per patch once warm.
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
that ran, not as a description of where it runs now. On the `ci.yml` run for
`028ac566` (`build-test`, job `106523333782`, 14m27s, success) the new step is
step 32, sitting between "Test and Measure Coverage" and "Show sccache
statistics" exactly as its comment claims, and it executed rather than being
skipped:

```text
PASS [  59.139s] (1/1) netsuke-build::kani_mutation_evidence_tests compile_guard::every_patched_tree_compiles_under_denied_warnings
Summary [  59.139s] 1 test run: 1 passed, 3 skipped
```

Two details from that log are worth keeping. The gate ran under the `ci`
nextest profile, which `build-test` sets job-wide, and still picked up the
`nested-cargo-builds` registration because `ci` declares no `[[overrides]]` of
its own and inherits `default`'s. And "3 skipped" is the
`--run-ignored ignored-only` flag doing exactly its job: one ignored test
selected, the three fast contract checks left alone. Every other lane was green
on the same SHA, including `kani-smoke` (6m51s) and the Windows jobs.

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
