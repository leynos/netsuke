# Mutation baseline reaches real build-tool preflight without the required pinned mold (issue #875)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE

All four milestones are closed and draft pull request #911 carries the work.
The status describes this plan, not the issue: the issue's acceptance clause
that required linker selection to remain intact — its second clause — is unmet
under its strict reading and is **recorded as a gap rather than resolved**. See
*Outcomes & retrospective*.

Two corrections to the plan's earlier account of that gap, made 2026-10-18.
First, the exported `RUSTFLAGS` is emitted by the downstream
`setup-rust-toolchain` action, not by `setup-rust` itself; `setup-rust`'s
non-empty default is what keeps that export alive. Second, shared-actions#628
is **not** the remedy this plan had been carrying: it supplies a child-only
flag-replacement mechanism for the cargo-mutants child. It does not preserve
configuration-owned flags, and it does not, by itself, resolve this issue's
linker-selection criterion. A replacement value *can* name
`-Clink-arg=-fuse-ld=mold`, so the reason it is rejected here is not incapacity
but that this plan forbids caller-owned flag replacement.

A three-way build measurement then established that a config-declared
`rustflags` entry is displaced by an exported `RUSTFLAGS` while a
config-declared `linker` entry is not, so the consumer-repository route the
shared workflow documents survives only through the `linker` key. Review on
2026-10-18 held that route valid in principle, rejected the upstream-only
reading, and directed that it be delivered as a separate build-policy change
under #875 rather than inside #911 — an unconditional Linux wrapper is not the
design, because the release, packaging and coverage exclusions depend on the
present displacement. See *Surprises & discoveries* and *Outcomes &
retrospective*.

## Purpose / big picture

The scheduled mutation-testing run fails intermittently, and when it fails it
fails at its *baseline*. The baseline is the unmutated `make test` run the
worker performs before it mutates anything; when it fails, cargo-mutants
reports no mutant results at all, so one failing test suppresses the entire
mutant set.

Measured from `gh run list --workflow "Mutation testing"`, which is the only
honest way to state the rate: of **30 runs** between 14 September and 12
October (one further run was still in progress at the time of counting),
**seven failed** — `1e60fb18` 18 Sep, `024a9014` 19 Sep, `ee0e5523` 22 Sep,
`a6430c9e` 25 Sep, `027a848e` 28 Sep, `5e6c0fbf` 2 Oct and `b6e7cf50` 3 Oct,
the last being this issue's — and the other 22 concluded `success`. An earlier
draft of this paragraph claimed "at least two weeks, so no mutant has been
evaluated since"; that was an over-claim, and it is corrected here rather than
left to be discovered at review. The separate and real skips — eight
consecutive scheduled runs from 5 to 12 October that evaluated nothing because
no source file changed inside the detection window — are a different
phenomenon, recorded under *Surprises & discoveries*.

Two distinct faults are tangled together in the reports, and only one of them
is a CI provisioning problem:

1. **A genuine CI gap.** The mutation caller
   (`.github/workflows/mutation-testing.yml`) selects the shared reusable
   workflow `leynos/shared-actions/.github/workflows/mutation-cargo.yml` pinned
   at `0606ca89…`, whose `install-mold` input defaults to `'false'`. The worker
   therefore has no `mold` on `PATH`. Every netsuke build target depends on the
   `check-build-tools` prerequisite, which fails with
   `build-tools: mold not found on PATH (pinned 2.41.0)`. Fixing the CI job is
   a two-line caller change.

2. **A test-boundary fault.** Two tests in
   `tests/makefile_test_target/rustdocflags.rs` are supposed to be *hermetic*:
   they drive the real `doctest` recipe with a fake `cargo` on the `PATH`, and
   assert that a caller-supplied `RUSTDOC_FLAGS` reaches Cargo's `RUSTDOCFLAGS`
   unchanged. They are not hermetic, because the `doctest` recipe has a
   `check-build-tools` prerequisite and the test inherits the ambient host
   `PATH`. On a developer machine with the pinned `mold` installed the tests
   pass; on the mutation worker they fail with the `mold not found` diagnostic
   above. That sentence is the *entire* actionable content of the CI failure —
   the two tests' own assertions never ran, so the failure tells a reader
   nothing about `RUSTDOCFLAGS`.

After this change a manifest reader can observe that:

1. `make test-workflow-contracts` pins the caller's mold provisioning: the
   contract test asserts the caller enables `install-mold` and runs
   `make check-build-tools` in `setup-commands`, so a later edit that drops
   either fails on the pull request rather than in a scheduled run.
2. The two rustdoc tests pass on a machine with **no host `mold` and no host
   `rustup`**, because they inject the build-tools boundary through the shared
   `Sandbox` helper exactly as `tests/build_tools_check_tests.rs` already does.
   A reader can reproduce this without disturbing the installed toolchain:
   build the binary with `mold` present, then run the compiled
   `makefile_test_target` binary under a `PATH` that mirrors the system
   directories minus `mold`, as EP-M2 spells out. The rustdoc tests pass while
   `build_tools_check_tests` still exercises the real preflight. Do **not**
   move an installed `mold` aside: the build itself links with
   `-Clink-arg=-fuse-ld=mold`, so that route cannot reach a compiled test.
3. Real-preflight coverage of the `doctest` target does not disappear: it
   moves to `tests/build_tools_check_tests.rs`, where `doctest` joins the
   existing parameterized "targets stop when the check fails" table.

The distinction matters because the two halves have different owners. (1) is
provisioning: a worker that has never had `mold` installed. (2) is a design
choice: whether a test of *argument propagation* should also be, incidentally,
a test of *host capability validation*. It should not be — the capability gate
has its own suites, and a test that crosses both boundaries reports whichever
one fires first, masking the other.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not a workaround.

- **`cmd`/`interfaces` scope.** This change touches a GitHub Actions caller,
  one Python contract suite under `tests/workflow_contracts/`, and two Rust
  test modules. It must not change `.cargo/config.toml`, the `Makefile`'s flag
  composition, `scripts/check-build-tools.sh`, or any production Rust source
  under `src/`.
- **The build standard must survive.** `RUSTFLAGS` must not be set or
  overridden by the caller workflow. The plan's whole reason for existing is
  that the worker needs the pinned linker; silently replacing the project's
  `rustflags` tables would "fix" the diagnostic while removing the thing the
  diagnostic protects. See
  <https://github.com/leynos/shared-actions/issues/544> for the flag-ownership
  question this leaves open.
- **No new dependencies.** The Rust side already depends on
  `test_support::build_tools`; the Python side already depends on
  `workflow_loading`. No crate or Python package is added.
- **No compatibility machinery.** The two rustdoc tests are test-only surfaces.
  They are rewritten, not dual-pathed; the old direct `Command::new("make")`
  helper is deleted outright.
- **Existing runner, cache, and concurrency limits stay untouched.**
  `concurrency`, `permissions`, and the triggers in the caller keep their
  current values. No `runs-on`, cache, or shard setting is added.
- **`mold` must not be added to the sandbox utility allowlist.** The allowlist
  in `test_support/src/build_tools/sandbox/utilities.rs` is deliberately
  explicit so a new dependency surfaces as a test failure. Adding `mold`,
  `rustup`, or `cargo` to it would defeat the injection the plan is introducing.
- **British-oxendict spelling throughout**, including in comments and new
  prose, per `AGENTS.md`.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- **Scope:** if implementation requires changes to more than 8 tracked files,
  or more than 250 net lines, stop and escalate.
- **Interface:** if `test_support::build_tools`'s public surface must gain a
  new public function or type, stop and escalate. (Reusing `Sandbox::run_make`,
  `Sandbox::write_mold`, `Sandbox::write_rustup`, `MakeInvocation`,
  `pinned_mold_version`, `pinned_toolchain`, and `combined` needs no new
  surface. Adding a sandbox utility to the allowlist is not an interface change
  but is governed by the constraint above.)
- **Dependencies:** any new external dependency requires escalation. The
  expected count is zero.
- **Iterations:** if the two rustdoc tests still fail under a mold-free `PATH`
  after 3 fix attempts, stop and escalate with the captured `PATH`, the
  `.stderr` text, and the sandbox layout.
- **Ambiguity:** if the caller cannot express "install mold, then verify the
  repository pin" with the pin's declared inputs, stop and present options
  rather than inventing an input.
- **Worker dispatch:** the manual dispatch in Milestone 3 costs a long build on
  a shared runner. If the dispatch cannot be started within the first three
  attempts, record the failure and continue with local evidence; do not retry a
  fourth time without escalation.

## Risks

Known uncertainties that might affect the plan. Each notes severity,
likelihood, and mitigation.

- **Risk:** the shared workflow's `install-mold` input is typed `string`, not
  `boolean`, in the pinned revision. Passing a YAML boolean would be a type
  error at workflow-call time. Severity: high (the job would not start at all).
  Likelihood: medium. Mitigation: pass the literal quoted string `'true'`,
  matching the documented usage; assert the *type* in the contract test by
  reading the raw YAML value and requiring it to be the string `"true"` rather
  than the boolean `True`. Verified during reconnaissance: the input is declared
  `type: string` / `default: 'false'` at the pin.

- **Risk:** `setup-commands` may run *after* the baseline rather than before it,
  in which case verifying the pin there would not protect the baseline.
  Severity: high. Likelihood: low. Mitigation: reconnaissance confirmed the job
  order is Setup Rust (which handles `install-mold`) → Install cargo-mutants →
  Run setup commands → baseline. The `make check-build-tools` in
  `setup-commands` therefore runs before the baseline. If a later observation
  contradicts this, escalate rather than reordering.

- **Risk:** a warm sandbox `PATH` in the new rustdoc tests could accidentally
  resolve a real `mold`, making the test pass on a machine that has one and
  giving a false sense of hermeticity. Severity: medium. Likelihood: low.
  Mitigation: `Sandbox` uses `env_clear` and sets `PATH` to the sandbox `bin`
  alone, so only files the test writes are visible. Verify by running the suite
  with `$HOME/.local/bin` removed from the ambient `PATH`, which is the
  mold-free reproduction below.

- **Risk:** removing the direct `Command::new("make")` path also removes
  coverage of "the recipe works when `BUILD_TOOLS_PREFIX` is the caller's real
  prefix". Severity: low. Likelihood: low. Mitigation: that coverage is not
  lost but relocated — `build_tools_check_tests.rs` already exercises the real
  recipes through `Sandbox::make`, and the `doctest` cases added there run
  against the same sandbox.

- **Risk:** the `Text file busy` (`ETXTBSY`) report from the 18 September run
  may be a real defect in a Ninja fixture, or may be a transient from the same
  window as the mold failure. Severity: low (it has not recurred). Likelihood:
  low. Mitigation: Milestone 3 performs a *read-only* audit of the fixture
  write paths (`test_support::exec::write_exec_with_content` →
  `test_support::fs::write` → `set_mode`) and changes a helper only if a write
  handle can outlive a spawn. No retries are added.

- **Risk:** a mutation shard can stop before it finishes, so the *run* concludes
  `failure` and the mutant set is incomplete. Severity: medium — it affects
  neither the named tests nor the baseline, both of which are already proven on
  the worker. Likelihood: **high — every shard of the run recorded above ended
  early, and four shards of the preceding run did too.** The complete per-shard
  durations show **two distinct mechanisms**, and this entry is written from
  the full data rather than the first two shards an earlier draft had:

  - Shards 0, 1, 3 and 4 ended on the runner shutdown signal at 59, 65, 106 and
    128 minutes — *not* clustered and *not* at a round number. Shards 0 and 1
    were read directly: each log ends `ERROR interrupted` →
    `##[error]The runner has received a shutdown signal…`.
  - Shards 2 and 5 ended at **301 minutes**, which is the caller's
    `timeout-minutes: 300` step budget, and are marked `cancelled` (a step
    timeout) rather than `success`. The preceding run's shard 5 also ended at
    **301 minutes**.

  So there is no uniform platform cap, and the `timeout-minutes: 360` job
  ceiling — the platform maximum — is not what ended these. Treat the early
  shutdowns as unexplained intermittent infrastructure and the 301-minute ones
  as the configured budget doing its job. Note a source of imprecision to carry
  forward rather than paper over: the API's `started_at`/`completed_at` are
  *runner* ENDED/STARTED events, not the step's own boundaries, so the "301
  minutes" reading is consistent with, but not proof of, the step timeout.

  Mitigation: none is available to this branch — a shorter matrix does not
  shrink the mutant total, only redistributes it. The risk is bounded instead
  by what survives a kill: the shard logs keep the baseline line and every
  per-mutant outcome, and the `summarize` job aggregated them into
  `mutation_summary_targets={"root": 111}` and `mutation_summary_mutants=1267`
  and concluded `success`. Report this to the pull request as an upstream
  observation, link `shared-actions#590`, and do not treat it as a defect in
  this change. Reclaims are **intermittent, not uniform** — run `35499357086`
  (20 September) concluded `success` with its `mutants` job succeeding.

## Progress

Every stopping point is documented here. Timestamps are UTC.

Three entries dated `2026-10-16` were corrected to UTC after being written in
local time; the defect, its proof and the limits of the audit are recorded in
the last entry of this section rather than recited here.

- [x] (2026-10-12 14:45Z) Reconnaissance: confirmed the upstream diagnostic
  reproduces locally. With `PATH` and `HOME` both free of `mold`/`rustup`,
  `make … doctest` exits 2 at
  `make: *** [Makefile:476: check-build-tools] Error 1` with
  `build-tools: mold not found on PATH (pinned 2.41.0)`.
- [x] (2026-10-12 14:50Z) Reconnaissance: confirmed the byte-identical
  assertion travels through `run_doctest_with_override` →
  `assert_doctest_record` (`rustdocflags.rs:158-192`) and that the helper uses
  `env!("CARGO_MANIFEST_DIR")` + direct `Command::new("make")`, which under
  `cargo nextest` is the *worktree* root. Noted that whether `doctest` inherits
  `CARGO_TARGET_DIR` from the agent shell is unproven and is *not* load-bearing
  for this plan.
- [x] (2026-10-12 14:55Z) Reconnaissance: read the pinned shared workflow's
  `workflow_call.inputs`. `install-mold` is `type: string`, `default: 'false'`;
  `setup-commands` is `type: string`, `default: ""`. The shared revision
  already declares both — upstream `9a279509` ("Forward install-mold from
  mutation-cargo to setup-rust (#545)") is an ancestor of the pin. **No pin
  bump is needed.**
- [x] (2026-10-12 15:19Z) **Milestone 1 complete.** `mutation-testing.yml`
  gained `install-mold: 'true'` and a `setup-commands` block running
  `make check-build-tools` then printing `RUSTFLAGS` and
  `CARGO_ENCODED_RUSTFLAGS`. `mutation_testing_test.py` gained two typed
  assertions (`EXPECTED_WITH` extended; a standalone test for each input).
  `make test-workflow-contracts` green (317s, run in background). Committed as
  `1b7b01f8`.
- [x] (2026-10-12 15:50Z) **Milestone 2 complete, with invariant I2
  discharged.** `rustdocflags.rs` now drives `doctest` through
  `Sandbox::run_make` with a fake `mold` under the sandbox prefix and a fake
  `rustup`; `doctest` joined the fail-before-Cargo table in
  `build_tools_check_tests.rs`. Focused suite: 55/55 green with `mold` present.
  **The same compiled binary, run under a verified mold-free `PATH`, passes all
  7 `rustdocflags` tests** (`7 passed; 0 failed`, RC=0) — the two named in the
  issue among them. Evidence: `/tmp/i2-moldfree-run-issue-875-*.out`, written
  `15:49:35Z`; the probe soundness block at the head of that log reports
  `mold: NONE` with `make` and `grep` both resolving, so the run is evidence
  about mold and not about a broken `PATH`.
- [x] (2026-10-12 16:35Z) Committed Milestone 2 as `b92dcbf4` ("Make the
  rustdoc override tests hermetic"), after fixing two defects a gate run found
  in the new code: a `clippy::shadow-reuse` violation on the fake-Cargo path
  binding, and an unformatted expression at the same site. Both were in the
  `cargo fmt --all -- --check` and a focused
  `cargo clippy --test makefile_test_target --all-features -- -D warnings
  -D clippy::shadow-reuse`
  then ran clean.
- [x] (2026-10-12 16:45Z) **Upstream CI evidence frozen.** Job
  `111169751801` (run `37111347135`, head `b6e7cf50`) carries the decisive
  proof that the caller passed *no* setup commands: step 7, "Run setup
  commands", concluded `skipped`, and step 11, "Run mutation testing", concluded
  `failure`. A skipped step is stronger than an absent one — it is the shared
  workflow saying the input it declares arrived empty. Recorded because the job
  log itself fetched as 0 bytes, so the step table is the primary evidence
  rather than a fallback.
- [x] (2026-10-12 15:35Z) **ETXTBSY audit complete, read-only.** No defect
  found; no helper changed; no retry added. See EP-M3 for the full chain.
- [x] (2026-10-12 19:20Z) **The dispatch's mutation report read from the
  artefact, not the run conclusion.** `mutation-report-root-2` (artefact
  `11760992373`, job `114916843738`) proves on `bc812986` that step 7 "Run
  setup commands" = `completed/success` and the baseline's **Build** phase =
  `process_status: "Success"` after 119.9 s of real Cargo — so the mold fix
  works and the worker can now compile. It equally proves the baseline's
  **Test** phase failed (`Failure: 101`) on
  `runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`
  with `[]`, so `outcomes.json` has `total_mutants: 0`. Both conclusions are
  recorded in *Surprises & discoveries*; the second is **out of scope** for
  this plan and is escalated rather than fixed.
- [x] (2026-10-12 19:30Z) **Green-run audit — the finding that reframes the
  issue.** All eight runs from `3f86c09d` (5 Oct) to `c65ad970` (12 Oct)
  concluded `success` with the `mutation / mutants` job `skipped` and zero
  artefacts: they evaluated no mutants at all. Logged in *Surprises &
  discoveries*.
- [x] (2026-10-12 19:35Z) **`make test` and `make test-workflow-contracts` are
  BLOCKED on disk exhaustion, not skipped and not failed.** A dispatched gate
  runner was held before it launched either, on the plan's own 3 GB
  kill-threshold being met by the *idle* machine. Measured evidence, all on
  `/dev/md2` (one 1.8 TB device carrying `/`, `/tmp` and every worktree): free
  space 15.47 GB at 19:16:30 → 3.43 GB at 19:23 → **2.95 GB at 19:25**, with
  nothing of the runner's running; load average **54.12 / 43.02 / 36.84** on 6
  cores; `pids.current` **7406 / 8192 (91%)**, the level at which an earlier
  `make test` died with `os error 11`; `build-limits-status` reporting
  `capacity 4, held 4, healthy, sccache unreachable, compiling directly` with
  ~20 compiler groups queued, including for `netsuke/worktrees/af59be38-…` and
  two `whitaker` worktrees. This worktree's `target/` alone is 30 GB; all six
  sibling `target/` directories total ~117 GB and belong to other worktrees, so
  **nothing has been deleted**. Consequence: no log exists for either gate, the
  `doctest` stage has still never run at this revision, and the root
  `rustls-webpki` compiler diagnostic remains *unavailable* rather than merely
  unfound. Escalated to the operator for a pruning decision: any failure
  produced now is un-attributable between a real defect and ENOSPC, which is
  exactly how the previous attempt's two failures came to be uninterpretable.
  Note also that `sccache unreachable` means sibling builds are compiling
  uncached, so the collapse is faster than a cached build's.
- [x] (2026-10-12 19:32Z) **Disk blocker cleared, without any deletion by this
  session.** `/dev/md2` recovered from 2.95 GB free / 100% used to **525 GB
  free / 69% used** between 19:25 and 19:29 — roughly 522 GB released. The
  cause is outside this session: nothing here deleted anything, and the plan's
  own constraint against touching sibling worktrees was honoured throughout.
  Most likely the sibling worktrees completed their builds and reclaimed
  intermediate artefacts, or another session pruned its own. The hold was
  lifted and both gates re-dispatched. Recorded because a blocker that clears
  itself is exactly as worth logging as one that does not — the next person
  reading the previous entry needs to know the 2.95 GB figure is historical,
  and that the ~117 GB of sibling `target/` directories still exist and are
  still not in this branch's remit to remove. Load average remains ~70 on 6
  cores, so gates will be slow; the sampled disk log is retained so an ENOSPC
  failure stays distinguishable from a real one.
- [x] (2026-10-12 20:50Z) **The worker's `setup-commands` text is read, and
  the #544 observation is recorded.**
  `gh api repos/leynos/netsuke/actions/jobs/<id>/logs --allow-escape-sequences`
  downloads a *completed* job's log even while its run is still in progress;
  only the `gh run view --job` form waits for the whole run. Downloaded all
  four cancelled shards. Every one prints, immediately after
  `build-tools: mold 2.41.0 at /opt/hostedtoolcache/mold/2.41.0-a3696680d99e692970590a178bc3a33d78d60d1c6dc9db7a11b557b02b751f5d/x86_64/bin/mold`
  and `build-tools: toolchain nightly-2026-08-23 available`:

  ```plaintext
  RUSTFLAGS=-D warnings
  CARGO_ENCODED_RUSTFLAGS=<unset>
  ```

  So the linker-selection half of the risk is **confirmed, not resolved**:
  `RUSTFLAGS` is exported with a value, and `CARGO_ENCODED_RUSTFLAGS` is unset.
  Since an assigned `RUSTFLAGS` replaces every `rustflags` table in
  `.cargo/config.toml`, the `-Clink-arg=-fuse-ld=mold` entry cannot apply. This
  is exactly <https://github.com/leynos/shared-actions/issues/544> ("Installed
  mold does not imply Cargo actually selects it if `RUSTFLAGS` has replaced
  project flags"). Per the plan's constraint, **no local workaround was added**
  and no pin bump is made, because no merged revision carries a fixing input.
  Milestone 3's observation obligation is now discharged with a literal
  transcript.

  The `setup-commands` print alone would not have settled it, because it shows
  what the *environment* holds, not what the *compiler* received. The
  `--verbose` baseline log carries every rustc argv, so the flags were counted
  directly. The measurement is over the `mutation-report-root-2` artefact's
  `log/baseline.log` — the only shard whose baseline log was uploaded, since
  the other four were cancelled before their upload step — across 569 compiler
  invocations:

  | Flag                       | Source                            | Occurrences |
  | -------------------------- | --------------------------------- | ----------- |
  | `-D warnings`              | exported `RUSTFLAGS`              | **569**     |
  | `-Clink-arg=-fuse-ld=mold` | `.cargo/config.toml`, Linux table | **0**       |
  | `-Zthreads=8`              | `.cargo/config.toml`, both tables | **0**       |

  One `-D warnings` per invocation, and not a single occurrence of either
  project flag: `RUSTFLAGS` wholly displaced both `rustflags` tables, exactly
  as the build standard's own documentation says an assigned `RUSTFLAGS` does.
  This is the decisive form of the #544 finding — an installed mold the
  compiler was never told to use — and it is now evidenced rather than inferred
  from the shared workflow's guide.
- [x] (2026-10-12 20:50Z) **Acceptance criteria are met on the real worker.**
  The four shards that were not killed ran the *unmodified* baseline to
  completion and every one reported it green:

  ```plaintext
  Found 636 mutants to test
   INFO Auto-set test timeout to 580s
  ok      Unmutated baseline in 98s build + 193s test
  ```

  (the other three: `120s build + 213s test`, `107s build + 177s test`,
  `95s build + 177s test`). An `ok Unmutated baseline` line describes the whole
  `cargo test` run, not one target, so it entails that the
  `makefile_test_target` binary ran and passed — that binary being the two
  named rustdoc tests. Two facts carry the entailment: cargo stops at the first
  failing target, which shard 2 shows in the negative — its `log/baseline.log`
  holds exactly **one** `test result:` line, pinning its failure to `--lib`,
  where the 3 Oct log holds 54 over a full run — and on 3 Oct
  `makefile_test_target` was a target that failed. State the limit of that
  evidence, though: the four green shards' per-target output is **not in
  hand**. Their job logs contain no `Running tests` or `makefile_test_target`
  lines, and only shard 2's artefact carries a `log/baseline.log`, so the step
  from "baseline ok" to "these two tests passed" is an inference from cargo's
  first-failure behaviour plus the 3 Oct ordering — not a line read out of
  those four logs. The same shards then evaluated mutants and reported real
  outcomes (`MISSED`, `TIMEOUT`) with per-mutant timings, so **a nonempty set
  of mutants executes**. All six shards also show step 7
  `Run setup commands = success` where the failing run had it `skipped`, and
  the runner annotations carry
  `notice: mold 2.41.0 installed for x86_64 (tool cache: miss)`.
- [x] (2026-10-12 20:50Z) **The four `cancelled` shards are an infrastructure
  event, not a defect.** Each ends after 57–86 minutes of legitimate work, at
  staggered times (17:51, 17:53, 17:58, 18:20 UTC), with a shutdown signal:

  ```plaintext
  ##[error]The runner has received a shutdown signal. This can happen when the runner service is
  stopped, or a manually started runner is canceled.
  ```

  Quoted verbatim, hence the United States spelling in the runner's own
  message. The caller sets `cancel-in-progress: false` and no other run of this
  workflow exists for this ref, so this is GitHub reclaiming runners, not a
  concurrency decision. Recorded because the job conclusions read `failure` and
  would otherwise look like the change failing.
- [x] (2026-10-12 20:50Z) **The one genuine failure is the unrelated flake,
  now measured.** Shard 2 exited `4` ("baseline tests failing") in 3 minutes on
  `runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`
  with `Error: a structural summary must emit exactly one trace event: []`.
  Four of the five completed shards ran that same lib suite green, so it is
  nondeterministic. It is the out-of-scope failure already logged below, and
  the branch does not touch that test or its helper.
- [x] (2026-10-12 21:05Z) **The green-run skip is explained, and the earlier
  guess at it is refuted.** The mechanism is settled from the pinned revisions
  rather than inferred: `mutants` carries
  `if: ${{ needs.detect.outputs.has_changes == 'true' }}`, and `detect` runs
  `workflow_scripts/mutation_detect_changes.py`, whose `detect()` returns
  `full_run_matrix()` only for `workflow_dispatch` and otherwise takes
  `changed_files()` — a
  `git log --since=<window-hours> <base-ref> -- <pathspec>` over the caller's
  `paths` (`src/,examples/,benches/`) with `window-hours` 25 (one hour wider
  than a daily cron). An empty result writes `has_changes=false`, and all six
  shards skip. So the green runs were not a mystery nor a defect in the caller:
  on a quiet day the scheduled mutation workflow deliberately does nothing.
  This supersedes the earlier Progress entry's guess of "the pinned shared
  workflow, or a `paths` filter on the caller" — right in implication, wrong in
  mechanism, and unfalsifiable as written.
- [x] (2026-10-12 21:05Z) **The 3 Oct worker genuinely had no mold, now shown
  at the step level rather than inferred from the preflight message.** Job
  `111169751801`'s log carries `install-mold: false` (lines 48 and 388) and
  `mold-version: 2.41.0` (line 401), then
  `##[start-action display=Install mold;id=__self.mold]` →
  `##[end-action id=__self.mold;outcome=skipped;conclusion=skipped]` followed by
  `Skip mold off Linux`. The install was skipped because the caller never
  asked for it, so `build-tools: mold not found on PATH (pinned 2.41.0)` was
  reporting a true absence. The caller at `b6e7cf50` passes only
  `exclude-globs` and `extra-args` — no `install-mold`, no `setup-commands` —
  and pins `ff1dd759…`, a different revision from today's `0606ca89…`.
- [x] (2026-10-12 21:05Z) **There is a second, unexposed source of truth for
  the mold version, and it is a latent hazard this branch does not create.** The
  `setup-rust` action at the pinned revision declares
  `mold-version: default='2.41.0'` alongside `install-mold: default='false'`.
  `mutation-cargo.yml` forwards **only** `install-mold` to that action, so the
  version the worker installs comes from the action's default, not from this
  repository. Today both read `2.41.0`, so the preflight passes and this branch
  is correct as it stands. But the two are independent: a bump to
  `tools/mold/VERSION` would leave the caller asserting nothing (the preflight
  reads the file itself) while the *worker* still installed the action's
  default, and `make check-build-tools` would then fail the `setup-commands`
  step loudly — which is the intended behaviour, and better than compiling
  against a drifting linker. The plan's Task 1 instruction to pass a
  mold-version input "if one exists" is discharged as *does not exist*: no
  input of that name is declared by `mutation-cargo.yml`. Recorded as an
  upstream improvement for `shared-actions#591`, not as a defect fixed here.
- [x] (2026-10-12 21:10Z) **Pushed over SSH and opened draft PR #911.** The
  branch had been pushed to `172dd4ff` and the tracking ref refreshed, with
  `ls-remote` confirming the remote head equals `HEAD`. Two environment facts
  made this worth recording. First, the harness injects **ten** `GIT_CONFIG_*`
  slots, including an SSH→HTTPS `url.insteadOf` rewrite; the earlier habit of
  stripping only `GIT_CONFIG_KEY_0` would have left the rewrite in place. A
  push over such a rewrite still *succeeds*, so a green result proves nothing
  about which transport ran. The push here stripped every `GIT_CONFIG_*`
  variable, and the clean environment was verified before use by reading back
  `git config --get-regexp '^url\.'` as empty. Second, the push itself took
  over two minutes and appeared to hang with an empty log, because `fork` was
  failing with `Resource temporarily unavailable`: the cgroup
  `lody-daemon.service` was at `pids.current=8157` against `pids.max=8192`. The
  push had in fact landed before the wrapper was stopped — `ls-remote` showed
  the remote at `172dd4ff` — so the apparent hang was process starvation, not a
  failed push.
- [x] (2026-10-12 21:12Z) **PR #911 created as a draft**, MERGEABLE, 5 files,
  +1449/−71, base `main` at the unchanged merge base `c65ad970`. The body
  carries `Closes #875`, the Lody session link, and the closing attribution
  line. CI began on `172dd4ff` (CI, Release Dry Run, Netsukefile Build Test).
  CodeRabbit reported `Review skipped: draft pull request`, so the
  `coderabbit review --agent` pass is still owed at a later milestone.
- [x] (2026-10-12 21:12Z) **A follow-up comment was posted to #875**
  (`#issuecomment-6130825428`) superseding two claims in the earlier comment:
  the mechanism of the skipped green runs (now read from the pinned detector
  script rather than guessed), and the retracted "shards run ⇒ red" relation.
  The follow-up also records the 569/0/0 flag census and the two-source mold
  version hazard for `shared-actions#591`.
- [x] (2026-10-13 01:27Z) **The first `make test` attempt failed for an
  environmental reason, and the distinction matters.** The run reached
  `1479/3914 tests run: 1475 passed, 4 failed, 6 skipped` and then stopped. All
  four failures are in `netsuke-build::advanced_usage_tests`, and none is a
  claim about this branch's subject:

  - three report `Error: run netsuke command` / `Caused by: Resource
    temporarily unavailable (os error 11)`;
  - the fourth fails in `std::thread::spawn` itself —
    `failed to spawn thread: Os { code: 11, kind: WouldBlock, message:
    "Resource temporarily unavailable" }` — i.e. *before* any netsuke code
    runs.

  This is exhausted thread/PID admission on the host, not a defect: the session
  cgroup's `pids.events` records **18516** cumulative refusals, with
  `pids.current` around 10.4k against `pids.max=16384`, and the host load
  average was **35** on a 6-core machine because other agents were building.
  Untouched sibling tests in the same binary passed in the same run. The
  correct response is to re-run when the host is quieter, not to change any
  source: a test that cannot spawn a thread has not been evaluated.
- [x] (2026-10-13 01:40Z) **`make test-workflow-contracts` passes**, so Task 3's
  contract half is discharged. A gate run on this branch reported exit `0` in
  200 s with `1266 passed, 3 skipped in 173.22s` followed by
  `27 passed in 12.31s`; the log is
  `/tmp/GATE5-issue-875-workflow-contracts.out`. In the same run `make test`
  failed again with the same `os error 11` admission refusals, this time at
  load ~31, so the environmental reading above is corroborated rather than a
  one-off. That attempt also hit nextest fail-fast: `2435/3914` tests never ran
  and the doctest half never started, so its verdict is incomplete evidence
  rather than a red one.
- [x] (2026-10-13 01:40Z) **CI went red on `172dd4ff` and the cause was this
  plan's own Markdown, not the code.** Both failing jobs — `build-test` and
  `Windows / lint-windows` — failed at `make check-fmt`, at the same step and
  on the same line: `mdtablefix --check` reporting
  `docs/execplans/issue-875-…mold.md +61 -62`. The prose written in the two
  preceding commits had been hand-wrapped and so was not in the canonical form
  the gate enforces. Fixed by running `mdtablefix --in-place` (a
  whitespace-only rewrap plus table-rule padding, verified by collapsing
  whitespace across before and after: the only surviving difference is the
  table's `|---|` rule). The spelling gate then rejected `canceled`, which
  appears solely inside a verbatim quotation of GitHub Actions' own runner
  shutdown message; fencing that quotation as `plaintext` — the pattern this
  file already uses for log excerpts — places it outside the prose scan without
  misquoting the runner. Committed as `5460bd89`. `make markdownlint` (which
  includes `make spelling`) and `mdtablefix --check` both pass locally
  afterwards.
- [x] (2026-10-13 02:10Z) **The named rustdoc tests pass under the real gate,
      and
  the remaining `make test` failures are a second environmental cause with a
  one-line diagnostic.** A re-run with `--no-fail-fast` (so a single admission
  failure cannot truncate the suite) advanced from `1479/3914` to **`3901/3914`
  ** with **3897 `PASS` and 0 `FAIL`**, and the four `TIMEOUT`s are all
  nested-Cargo fixture compiles
  (`build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  and three `command_env_ui_tests::*_fixture_compiles`). The two tests this
  branch exists to fix are green, as is the property test's full 16-case run:

  ```plaintext
  PASS [0.261s] (2679/3914) makefile_test_target rustdocflags::behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags::case_1
  PASS [0.155s] (2683/3914) makefile_test_target rustdocflags::behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags::case_2
  PASS [5.805s] (3268/3914) makefile_test_target rustdocflags::property_doctest_preserves_rustdoc_override_values
  ```

  The timeouts are attributable to a cold compile cache, not to load and not to
  this branch: the same document-only diff produced **1**
  `sccache server is unreachable; compiling without the cache` message in the
  earlier run (those tests passed) and **573** in this one, and
  `sccache --show-stats` then reported `Compile requests 0`, i.e. a reset
  server rather than an idle one. The nested-cargo tests carry the profile's
  own 300 s cap; the `nested-cargo-builds` group serializes them but does not
  widen it. So the diagnostic for this family is a count, not a test name. The
  doctest half still has not run to completion, because the nextest half
  aborted first both times.
- [x] (2026-10-13 02:25Z) **The markdown fix is confirmed by CI on both jobs
  that were red.** At `d8e03753`, `build-test` concludes `success` in 18m14s
  with the `Format` step green, where at `172dd4ff` the same job failed there;
  `Windows / build-test-windows` passes; and `Windows / lint-windows` now clears
  `Format` and `Lint (Clippy)`, which is as far as its earlier run got. So the
  two red jobs were one defect, and it is repaired. A gate run of
  `Test and Measure Coverage` inside `build-test` covers the Rust suite on the
  merge path, independent of this host's cold sccache.
- [x] (2026-10-13 00:55Z) **The worker evidence is complete, and both dead
  shards are runner reclaims rather than test failures.** Run `38323348294` at
  head `a7769224` logged, in the setup phase:

  ```plaintext
  build-tools: mold 2.41.0 at /opt/hostedtoolcache/mold/2.41.0-a3696680…/x86_64/bin/mold
  build-tools: toolchain nightly-2026-08-23 available
  RUSTFLAGS=-D warnings
  CARGO_ENCODED_RUSTFLAGS=<unset>
  Found 636 mutants to test
  ok       Unmutated baseline in 122s build + 218s test
  ```

  `build-tools: mold 2.41.0` equals `tools/mold/VERSION`, and the install lands
  on `PATH` because `setup-rust` writes `GITHUB_PATH`. The baseline is `ok`, so
  the full unmodified suite passes on the worker with the named rustdoc tests
  inside it. Mutants were then evaluated — shard 0 logged six `MISSED` lines
  (e.g.
  `src/host/pattern.rs:200:19: replace += with *= … in 47s build + 135s test`) —
  so "a nonempty set of mutants executes" is discharged.

  The run finished with every shard ending early and `summarize`
  **succeeding**, which aggregated the partial reports into
  `mutation_summary_targets={"root": 111}` and `mutation_summary_mutants=1267`.
  Shard durations were 59, 65, 106, 128, 301 and 301 minutes. Shards 0 and 1
  conclude `failure` with `step 11 Run mutation testing` = `cancelled`, each
  log ending `ERROR interrupted` →
  `##[error]The runner has received a shutdown signal…` →
  `The operation was canceled.`; shards 2 and 5 ended at the 301 minutes that
  is the caller's `timeout-minutes: 300` step budget. The job ceiling is
  `timeout-minutes: 360`, the platform maximum, so neither mechanism is a cap
  below what the caller asked for. The *Risk* above carries the full breakdown;
  this entry states only what the logs show.

  On flag ownership, the chain is now evidenced end to end:
  `mutation-cargo.yml` calls `setup-rust` with **only** `install-mold`, and
  `setup-rust`'s `rustflags` input defaults to `'-D warnings'` and passes it
  straight through to `actions-rust-lang/setup-rust-toolchain`, which is the
  action that actually appends `RUSTFLAGS=$NEW_RUSTFLAGS` to `GITHUB_ENV`; the
  non-empty default is what keeps that export alive. So mold is installed, on
  `PATH`, and asserted at the pin, while `.cargo/config.toml`'s
  `cfg(target_os = "linux")` `rustflags` table — the one carrying
  `-Clink-arg=-fuse-ld=mold` — is displaced. That is shared-actions#544, open
  at 2026-10-07 with no merged fixing release (the newest `setup-rust` commit,
  `3c0fa8e7` on 2026-10-10, is `(#583)`, unrelated). Per *Surprises*, the
  response is **no local workaround**, with the finding linked from the pull
  request. `ci.yml:279`'s own `RUSTFLAGS: -D warnings` is the *coverage*
  exclusion from the build standard, so this repository's workflows stay
  consistent with it.

  (Corrected 2026-10-18: this entry previously attributed the export to
  `setup-rust`'s own description rather than naming the downstream action. The
  earlier wording pointed at the wrong layer of the chain, which matters when
  deciding which upstream change fixes it.)

  Runner, cache and concurrency are unchanged:
  `git diff c65ad970 HEAD -- .github/workflows/` is `+39` lines, all inside
  `mutation-testing.yml`, with no `runs-on`, `concurrency`, `cache`,
  `permissions` or `on:` line touched.
- [x] (2026-10-13 05:50Z) **The nextest half now reports zero failures on this
  host: `3914 tests run: 3905 passed (3 slow), 9 timed out, 6 skipped` in 3399
  s.** This supersedes the earlier `3897 PASS / 0 FAIL / 4 TIMEOUT` reading —
  the same suite, a longer run, and the same environmental residue. All nine
  `TIMEOUT`s are nested-Cargo fixture compiles (`command_env_ui_tests::*`,
  `build_module_slice_ui_tests`, `locale_stub_ui_tests`,
  `packaging_smoke_tests`), and the diagnostic is measured rather than
  inferred: **573** `sccache server is unreachable` lines. Every other test
  passed, including the three named ones.

  The doctest half did **not** complete and is not claimed as evidence. It had
  emitted only `Doc-tests netsuke` and `Doc-tests test_support` — all green,
  `88`/`2`/`39` passed, 0 failed — before hanging for three hours on an idle
  process holding a `-j2` build-limits slot (`build-limits-status` showed
  `capacity 4, held 4`, `sccache unreachable, compiling directly`, while other
  agents' crates queued). It was stopped, recovering the slot. The doctest half
  is therefore **not** evidence for this change either way, and Milestone 4
  should not treat it as a pass.
- [x] (2026-10-13 07:20Z) **The "pin a revision that fixes the flag ownership"
  half of the mold-selection finding is closed by measurement, not argument.**
  CodeRabbit's second pass re-raised the finding in a form the first pass did
  not: it asked to "resolve the shared workflow's flag ownership and pin a
  revision that preserves the project linker flags". That is a different claim
  from "mold is installed but unused", and it is testable. Three probes:

  1. `shared-actions#544` is `OPEN` with `closedAt: null`, **zero** comments,
     and `updatedAt` of 2026-10-07 — three days before the pinned revision was
     cut. Re-checked 2026-10-16: still open, still zero comments, so no
     *merged* revision fixes it. What has changed is that a related change now
     exists unmerged — shared-actions#628, "Add optional mutation Rustflags
     override (#544)", open, `mergeStateStatus` `BLOCKED` — but see the
     *Surprises* entry of 2026-10-18: #628 overrides `RUSTFLAGS` for the
     cargo-mutants **child only** and therefore does not add linker selection.
     It is a candidate to pin for the Cranelift consumer case it was written
     for, not for this one.
  2. `mutation-cargo.yml` at the pin (`0606ca89`) declares 13 `workflow_call`
     inputs and **none** of them is `rustflags`; re-checked at the upstream tip
     `e693d81d` (2026-10-14) it declares 17, the additions being
     `install-clang-lld`, `install-mdtablefix`, `mdtablefix-sha256` and
     `mdtablefix-version` — and still none is `rustflags`. The only setup-rust
     call there is `with: install-mold: ${{ inputs.install-mold }}`. So no
     merged revision of this reusable workflow exposes a flag-ownership surface
     a caller could pin to. #628's head (`73e09854`) does declare an 18th
     input, `rustflags`, defaulting to `""` — which is the input this caller
     needs, available the moment that pull request merges.
  3. `setup-rust`'s `rustflags` input still defaults to `'-D warnings'` at
     shared-actions `HEAD` (`action.yml:85`), so the default that displaces
     `.cargo/config.toml` is unchanged upstream.

  The finding is therefore **confirmed as diagnosed and confirmed as not
  actionable here**: adding a local `RUSTFLAGS` override would violate the
  plan's own constraint and would be the workaround the finding explicitly
  forbids ("do not override `RUSTFLAGS` in this workflow caller"). No change
  made; the finding stands as an upstream dependency against #544.

  (Qualified 2026-10-18: #628's input is a child-scoped literal replacement, so
  it is **not** the remedy for this clause. The route that survives measurement
  is a config-owned `linker` entry, which an exported `RUSTFLAGS` does not
  displace. See the *Surprises* entry of that date.)

  **Recurrence.** The finding was raised a third time on 2026-10-13 with one
  new clause: "verify a compiler invocation". That clause is already
  discharged, and by the stronger method the finding asks for — the
  569-invocation census above reads the `--verbose` baseline log's rustc argv
  directly, which is why the count of `-Clink-arg=-fuse-ld=mold` is **0**
  rather than a claim about the environment. A third raise does not change the
  disposition, because the finding's own prescribed remedy ("pin the fixing
  revision") has been measured to have nothing to pin; the plan records the
  probe rather than re-litigating it silently.
- [x] (2026-10-13 08:15Z) **The full eight-gate set is green on `3d761c8d`,
  and it supersedes the earlier nextest reading — including the diagnosis of
  the nine timeouts.** The gate scope was widened rather than narrowed: the
  non-Markdown source is byte-identical across `17e12e31`, `36a7f68c` and
  `3d761c8d` (verified by `git diff --name-only <a> <b> -- . ':(exclude)*.md'`,
  empty for every pair), so the full set was run instead of the
  Markdown-sensitive subset. All eight exit 0: `check-fmt` 2 s, `lint` 2003 s,
  `typecheck` 25 s, `doc-coverage` 111 s, `markdownlint` 16 s, `nixie` 2 s,
  `test-workflow-contracts` 94 s, `test` 215 s. Each gate's `HEAD` was read
  immediately before and after it. `make lint` ran with no stale-cache report
  and zero `error`/`warning` lines; `doc-coverage` reproduced
  `aggregate 4972/5029 98.87%`.

  **`make test` now passes both halves with no timeouts.** nextest:
  `3914 tests run: 3914 passed (1 slow), 6 skipped` in 142 s. Doctests:
  `Doc-tests netsuke` `88 passed` and `2 passed`, `Doc-tests test_support`
  `39 passed`, all `0 failed`, both `all doctests ran`. The doctest half that
  the 05:50Z entry recorded as incomplete is therefore **now discharged**, and
  the earlier three-hour hang is explained: an idle process held a `-j2`
  build-limits slot, which is an environment condition rather than a property
  of the code.

  **The earlier reading's diagnosis does not survive, and that is worth
  recording rather than quietly replacing the numbers.** The nine timeouts were
  attributed to nested-Cargo fixture compiles on a host whose managed sccache
  server was down, on the evidence of 573 `sccache server is unreachable`
  lines. The clean run logged **1034** such lines and `build-limits-status`
  still reported `sccache unreachable, compiling directly` — more, not fewer.
  The degraded cache was present in both runs, so it cannot be what changed.
  The most plausible difference is that `make lint` preceded `make test` in the
  clean run and had already warmed `target/`, leaving the nested-Cargo fixture
  compiles cheap enough to finish inside their deadlines. That is a hypothesis
  consistent with the two logs, not something the run establishes; it is
  recorded as a hypothesis. The lesson is to treat a correlation between an
  environment diagnostic and a symptom as a hypothesis to test by counter-run,
  which this run provided by accident.

  **Refined at 2026-10-14 03:10Z by direct probe.** Warming the outer `target/`
  is only part of it, and not the part that decides this test. Probed with a
  1500 s allowance, `command_env_ui_tests::cli_configuration_fixture_compiles`
  passes in 977 s wall: 4 m 28 s to compile the test binary, then **709 s**
  inside the test body, where it spawns a child `cargo check` against a fresh
  `target/.tmp…/cli_configuration_pass/` tree. `build-limits-status` mid-run
  read `capacity 4, held 4` with a peer worktree's crates queued behind it, so
  the dominant term is admission-queue latency on a cold *per-run* tree — which
  a warm outer `target/` cannot remove. The earlier `timeout 400` probe's
  `rc 124` is thereby explained rather than merely superseded: 268 s compile,
  the rest queue.
- [x] (2026-10-13 09:05Z) **Two numbers in the committed text were wrong, and
  re-deriving them from the source rather than from the plan's own summary
  caught both.** Neither was caught by a gate: `make check-fmt`,
  `make markdownlint` and `make test-workflow-contracts` all pass on a wrong
  count. Recorded here because the same class of error will recur and the cheap
  check is not obvious.

  1. **The census run. A committed workflow comment cited run `38323348294` for
     the 569-rustc-invocation flag census.** That run is the *verification*
     dispatch at `a7769224`, where the census was not taken; the census is over
     the `mutation-report-root-2` artefact, which
     `gh api repos/leynos/netsuke/actions/jobs/114916843738` resolves to run
     **`38280609203`** at `bc812986`. The two runs were being used in the same
     document for different purposes, and the census's own provenance had been
     written down once, correctly, in `Progress` — then transcribed wrongly
     into the workflow comment. The plan's other citations of `38323348294`
     (the `skipped` → `success` step transition) are correct and unchanged.
  2. **The shared-workflow input count. "fifteen" was a miscount, and it had
     propagated to the workflow comment, this plan, and the PR description.**
     Measured properly — read the key names out of the `workflow_call.inputs`
     block at each revision and compare the sets — `mutation-cargo.yml`
     declares **13** inputs at the pin `0606ca89` and the tip has since grown
     (17 at `e693d81d`, adding `install-clang-lld`, `install-mdtablefix`,
     `mdtablefix-sha256` and `mdtablefix-version`). The tip does not declare
     `rustflags`, which is the load-bearing claim and is unaffected.
     The over-count is exactly two, which is the signature of a loose
     `grep -c "^      [a-z-]*:$"` inside an `on:`..`jobs:` span matching the
     `workflow_call:` and `inputs:` lines as well as the input names. The
     corrected numbers make the argument *stronger*, not weaker: the tip has
     grown an input since the pin and it still is not `rustflags`.

  The lesson is narrow and worth keeping: a count that appears in prose is a
  measurement, and a measurement re-derived from a summary instead of from the
  source is a transcription. Reading the key *names* out, rather than counting
  lines that look like keys, makes the off-by-two impossible to miss.
- [x] (2026-10-14 01:45Z) **The first CodeRabbit pass produced one genuine
  defect in this plan, and it was in the plan's own evidence procedure.**
  CodeRabbit's finding at the M2 conformance check was correct and is now
  fixed: the check required `make test-nextest` to pass *with mold absent*, and
  that target declares `check-build-tools` as a prerequisite (`Makefile:228`)
  whose script returns non-zero when `command -v mold` fails
  (`scripts/check-build-tools.sh:24-25`). The plan was asking for a command
  that can only fail — while its own procedure a few paragraphs above already
  described the sound form. The check now names the compiled test *binary*
  under the mold-free `PATH`, which is what the surrounding procedure does and
  what the property under test actually is. The sentence contradicted the
  plan's own findings, which is worth recording: a conformance check is a claim
  about what evidence is obtainable, and this one had drifted from the
  procedure it was meant to summarize.

  Two smaller items from the same review were also actioned: `_require_str` in
  `mutation_testing_test.py` replaces the two empty `case str() as value: pass`
  branches the module had grown, and "the mold install" at `:1139` became "the
  mold installation". That last finding's own anchor (`:63`) was wrong — line
  63 is a `rustup` sentence with no such phrase — so the correction was applied
  where the text actually is, which is the general rule for a finding whose
  location and claim disagree.

  Fixing the Python helper surfaced a gate lesson: the helper needed a NumPy
  `Returns` section for Ruff's `DOC201`, and the ExecPlan edit needed
  mdtablefix's canonical wrap (`make fmt`), because hand-wrapping Markdown fails
  `make check-fmt` even when `make markdownlint` passes. Both were caught by
  the gate runner and both are mechanical. Recorded because `DOC201` fires on
  any `-> str` helper carrying a prose-only docstring, which is the shape the
  two emptied branches were replaced with.
- [x] (2026-10-14 02:05Z) **The two red gates in the binding re-run are
  environmental, and each was settled by measurement rather than assertion.**
  Neither failing file appears anywhere in this branch's diff from the merge
  base `c65ad970`, and the complete delta from the fully-green `3d761c8d` is
  two non-Rust files.

  1. `local_reference_property_test.py::test_coverage_matches_oracle_independent_of_reference_order`
     failed with `DeadlineExceeded: Test took 694.92ms, which exceeds the
     deadline of 200.00ms`, reported by Hypothesis as `FlakyFailure` ("Failed
     on the first call but did not on a subsequent one"). Re-run in isolation
     at load average 44 — while four compile slots were held and a peer
     worktree queued 117 seconds deep — it **passed in 2.50s**. A 200 ms
     deadline under a load-44 box is the failure mode, not the suite.
  2. `netsuke-build::packaging_smoke_tests::packaged_manifest_retains_build_script_sources`
     was terminated by nextest at `TIMEOUT [300.013s]`. This one is a **known
     local property with no timeout headroom**, not a branch finding: it
     cold-builds through `cargo publish --dry-run` by design, and this host is
     roughly 3× slower than CI for it. CI on this exact revision is the stable
     oracle — job `115213351527` in run `38372699508` at `35473e8e` shows
     `PASS [ 176.613s] (3930/3930)` and `3930 tests run: 3930 passed (1 slow),
     6 skipped`. **No local re-run is needed to read a verdict CI has already
     given for the same bytes**, and a widened local allowance would not be a
     fix for a test whose isolated local cost sits at 288s of a 300s cap.

  Both failures aborted their suites partway, so the two halves that never ran
  were run separately and both are green: `make doctest` (39 passed, 0 failed,
  plus the `88`/`2` distributed results) and stage 3 of
  `make test-workflow-contracts` (`27 passed in 13.21s`). The remaining stages
  of each suite had already passed in the same run.
- [x] (2026-10-14 03:10Z) **The third nested-Cargo timeout — the one that
  aborted `gate 8`'s second run — is admission-queue latency, and the test it
  killed passes when the queue clears.** The verification re-run on `52084776`
  terminated `command_env_ui_tests::cli_configuration_fixture_compiles` at
  `TIMEOUT [300.008s]`. Probed directly with a 1500 s allowance, the same test
  binary reports `test result: ok. 1 passed; 0 failed … finished in 709.20s`,
  rc 0, 977 s wall. The split is the finding:

  - **4 m 28 s compiling** the test binary itself, on a host reporting every
    crate as `the managed sccache server is unreachable; compiling without the
    cache`.
  - **709 s in the test body**, because the test spawns its own child
    `cargo check` against a **fresh** `target/.tmp…/cli_configuration_pass/`
    tree. `build-limits-status` during the run read
    `capacity 4, held 4, healthy, sccache unreachable, compiling directly`
    with 20+ crates from a peer worktree queued behind it. The child compile
    was the wait, not the test's own work.

  So the earlier `timeout 400` probe (`rc 124`) is now explained rather than
  merely superseded: 268 s of it was compile and the rest was queue. The three
  local timeouts across this branch's gate runs are one environmental story — a
  cold sccache plus a four-slot admission pool shared with other agents — and
  none of them is a property of the diff. The ExecPlan's 08:15Z entry named the
  plausible cause as a warm `target/`; the probe refines that to the fixture's
  per-run *cold* child tree plus queue depth, which warming the outer `target/`
  cannot fix. No allowance was widened and no retry added; CI is the oracle for
  this test, as recorded in *Constraints*.
- [x] (2026-10-14 03:10Z) **All three workflows are green on `52084776`, and
  the CodeRabbit re-pass found one further documentation defect.** CI
  `38489674825`, Netsukefile Build Test `38489674558` and Release Dry Run
  `38489674799` each conclude `success`, with `CI`'s five jobs — `build-test`,
  `kani-smoke`, `Windows / build-test-windows`, `Windows / lint-windows`,
  `Windows / windows-msi-upgrade` — all `completed success`. That is CI on the
  exact bytes the gates ran against.

  Two CodeRabbit passes over the frozen revision, per the sampling caveat that
  a single zero-finding pass proves nothing: the first returned **0 findings**,
  the second returned 2. The new finding was real and is fixed — the M2
  conformance check said the caller must "still pin the `.github/actions/...`
  shape", but the caller is `.github/workflows/mutation-testing.yml` and the
  path it pins is `leynos/shared-actions/.github/workflows/mutation-cargo.yml`;
  no `.github/actions/` path exists anywhere in this branch. The check now
  names the real workflow path and the 40-character SHA shape the contract test
  asserts. The other finding is the standing mold-selection gap, dispositioned
  unchanged in *Decision log*.

  The pass count also confirms the sampling behaviour directly: the same
  revision, reviewed twice, produced 0 and 2 findings. Any claim that this
  branch "reviewed clean" holds only for a particular sample, which is why the
  plan states the disposition of each finding rather than a count.
- [x] (2026-10-14 04:20Z) **The docs-only delta on `009cbf02` is gated and CI is
  green there.** `009cbf02` changes one file against the fully-gated
  `52084776` — this ExecPlan — and the non-Markdown source is byte-identical:
  `git diff --name-only 52084776 009cbf02 -- . ':(exclude)*.md'` prints
  nothing, the same identity test the `3d761c8d` entry uses. The four non-Rust
  gates — the three Markdown-sensitive ones, `check-fmt`, `markdownlint` and
  `nixie`, plus `test-workflow-contracts` — were therefore re-run in place of
  the full set, and the Rust-bearing four were not. All four non-Rust gates
  exit 0, with `test-workflow-contracts` reporting `1266 passed, 3 skipped` in
  `tests/workflow_contracts` plus `27 passed` in the scripts suite — a complete
  run rather than the previously aborted one. The revision hash was read before
  the first gate and after the last and is unchanged, so the gates describe the
  committed bytes and not a dirty tree. CI on `009cbf02` is green across all
  three workflows (`CI` `38492475380`, `Netsukefile Build Test` `38492475298`,
  `Release Dry Run` `38492475387`).

  The PR body's CI section is deliberately written as an invariant rather than
  as a claim about one head: `009cbf02`'s run set was still `in_progress` when
  the section was last edited, and a conclusion is not carried across a push.
  The body therefore names the last *concluded* run set and states that every
  later head is docs-only on top of `52084776`, rather than saying "the current
  head" — a phrase written once and falsified by the next push within a single
  turn.
- [x] (2026-10-16 15:40Z) **The upstream-state claims were refreshed, and the
  review census re-derived after the eleventh pass.** The eleventh
  `coderabbit review --agent` pass raised the mold-selection finding for the
  eleventh time, at `mutation-testing.yml:78`. Checking it rather than
  re-asserting the recorded gap found that the record had *decayed*: the
  workflow comment named `aea9c14c24b7` as the upstream tip (now `e693d81d`, 17
  inputs) and said #544 "has no fixing commit" (shared-actions#628 then
  appeared to propose the missing input, `BLOCKED` — though the 2026-10-18
  entry above records that its input is child-scoped and does not add linker
  selection). The verdict is unchanged — no merged revision declares
  `rustflags` — but the cited reasons had rotted. Every copy now names only
  stable anchors and states the claim as "no merged revision", cites #628 with
  its state, and carries a re-check instruction. The census re-derived to 17
  invocations, 23 finding records, 11 mold instances, still eleven distinct
  defects. **This head is the first since `52084776` that is not docs-only**,
  so it takes the full gate set rather than the four-target subset.
- [x] (2026-10-14 05:20Z) **`df475f2e` is gated, and the plan stopped editing
  itself there.** Superseded as a stopping point by every later commit,
  including this document's own revision notes; retained as the record of that
  head. `df475f2e` records the `009cbf02` entry above and passes the same
  byte-identity test: `git diff --name-only 52084776 df475f2e` names this
  document alone and the same diff excluding `*.md` is empty. The four non-Rust
  gates were re-run against it and all exit 0 — `check-fmt` 2 s, `markdownlint`
  30 s (with `spelling` inside it), `nixie` 2 s and `test-workflow-contracts`
  109 s reporting `1266 passed, 3 skipped` plus `27 passed` — with the revision
  hash read before the first gate and after the last and identical at both
  points, and `git status --porcelain` empty at both.

  **The commit that carries this entry, `8c0490eb`, is the freeze head, and its
  gate evidence belongs in the pull request description rather than in this
  document.** The reason is a regress that this entry exists to stop: editing
  the document produces a new head, and a new head needs its own gate run to
  record, which edits the document again. The terminating move is to put the
  remaining evidence on a surface that does not move the revision. So the
  freeze head's gate logs are cited by path from the PR description, with the
  logs themselves under `/tmp`.

  **The freeze was then broken once, deliberately, and the carve-out above is
  why.** Closing Milestone 4 uncovered a contradiction rather than a cosmetic
  defect: the milestone line named "PR marked ready" as its final gate, while
  the brief asked for a *draft* pull request and this plan's own *Outcomes*
  records the linker-selection acceptance clause as unmet. Leaving that would
  have made the plan's terminal instruction point the opposite way from its
  evidence. The correction landed in the commit that closes Milestone 4, and no
  later head is named here on purpose: naming one would be falsified by the
  next edit, which is the regress itself. Every head after `8c0490eb` is
  docs-only on top of `52084776` and is gated accordingly, which is the stable
  way to state it and the reason the PR description cites the invariant rather
  than a SHA. The carry-over covers the Markdown-sensitive targets plus
  `test-workflow-contracts`, and of the Rust-bearing gates it covers `lint`,
  `typecheck` and `doc-coverage` — not `test`, whose local run did not complete
  and whose oracle is CI.

  **Two citation errors were made in the `009cbf02` version of the entry above
  and fixed in the `df475f2e` commit.** That version cited "the delta rule in
  *Constraints*" for the narrowing; no such rule exists in *Constraints*, whose
  entries are all scope and invariant constraints. The real basis is the
  byte-identity test the `3d761c8d` entry states, and the entry above now names
  that test instead. It also called the Markdown-sensitive set "four gates";
  there are **three** (`check-fmt`, `markdownlint`, `nixie`) plus
  `test-workflow-contracts`, which is its own target rather than
  Markdown-sensitive. Both are prose defects of the kind the plan's *Surprises
  & discoveries* entry describes — a number or a provenance written from memory
  rather than read from the artefact — and neither changes a verdict.

  **The PR body is written to survive the head advancing.** It states the
  invariant (the non-Markdown tree is byte-identical to `52084776`, so the
  gates green there carry — three of the four Rust-bearing ones; see the
  correction in the `2026-10-14 04:20Z` entry above) and names the last
  *concluded* run set rather than saying "the current head" — a phrase that was
  written once and falsified by the next push within a single turn.
- [x] Milestone 4: ExecPlan finalized, `coderabbit review --agent` passes
  dispositioned, CI green, draft pull request open.
  **`coderabbit review --agent` cannot be reported clean while the
  linker-selection acceptance clause is unmet:** every pass re-raises the
  mold-selection finding as `major`, correctly, and four probes have confirmed
  the underlying condition rather than resolved it. Milestone 4 is therefore
  discharged by CI green plus a dispositioned review and a pull request stating
  the unmet criterion, not by a zero-finding review — which is what the
  retained passes delivered: ten non-mold defects fixed, and the mold-selection
  finding dispositioned as a recorded gap (the census is re-derived in
  *Outcomes & retrospective*; it stands at eleven mold instances across
  seventeen passes, unsuperseded as of 2026-10-16). **The pull request stays a
  draft**, as instructed; "marked ready" appeared in an earlier version of this
  line and was the plan overreaching its brief.

  **The PR-description half is done.** PR #911's body now carries a section
  headed "One acceptance criterion is unmet, and it is recorded rather than
  claimed", which sets out the strict and no-regression readings, states that
  the strict one is unmet and why the rectification is out of reach from this
  repository, and says the worker's results must be read with that caveat. The
  body also records the two corrected numbers and the review tally.

  **The CI half is done.** All three workflows concluded `success` on every
  head pushed after the fix: `36a7f68c` (`CI` `38361956641`,
  `Netsukefile Build Test` `38361956431`, `Release Dry Run` `38361956933`) and
  the final head `d2d1963f` (`CI` `38369104515`, `Netsukefile Build Test`
  `38369104160`, `Release Dry Run` `38369104932`). `CI`'s five jobs —
  `build-test`, `kani-smoke`, `Windows / build-test-windows`,
  `Windows / lint-windows` and `Windows / windows-msi-upgrade` — are each
  `completed success`.

  **Still green on `35473e8e`**, the head as of this entry: `CI` `38372699508`,
  `Netsukefile Build Test` `38372699186`, `Release Dry Run` `38372700780`, all
  `completed success`, with `CI`'s five jobs each `completed success`.

  **And green on `52084776`**, the head as of 2026-10-14 03:10Z: `CI`
  `38489674825`, `Netsukefile Build Test` `38489674558`, `Release Dry Run`
  `38489674799`, each `completed success`, with `CI`'s five jobs each
  `completed success`. Both CodeRabbit passes were bound to this revision, and
  the full gate set was re-run against it. The all-green eight-gate run is a
  different revision — see the correction below.

  **And green on `009cbf02`**: `CI` `38492475380`, `Netsukefile Build Test`
  `38492475298`, `Release Dry Run` `38492475387`, each `completed success`.
  `009cbf02` differs from `52084776` by exactly one file, the ExecPlan this
  document is, and its non-Markdown tree is byte-identical to it, so the
  Rust-bearing gates were not re-run against it while the Markdown-sensitive
  gates and `test-workflow-contracts` were. `make check-fmt`,
  `make markdownlint` (which runs `spelling` as its prerequisite), `make nixie`
  and `make test-workflow-contracts` each exit 0, with the revision hash read
  immediately before the first gate and immediately after the last and
  identical both times. Of the four Rust-bearing gates, `lint`, `typecheck` and
  `doc-coverage` are covered by their green result on `52084776`, whose
  non-Markdown tree is byte-identical, and `test` is covered by CI's
  `build-test` on `009cbf02` and every head after it — not by the local run,
  which did not complete.

  **Correction (2026-10-16).** This entry previously said all four Rust-bearing
  gates were covered by their green result on `52084776`. Three were.
  `make test` on that revision terminated at `TIMEOUT [300.008s]` on
  `command_env_ui_tests::cli_configuration_fixture_compiles` and exited
  `Error 100`; the timeout is recorded and diagnosed in the `2026-10-14 03:10Z`
  entry above, which attributes it to admission-queue latency on the fixture's
  cold per-run child tree and clears it with a 1500 s probe. The gate label was
  wrong even though the verdict it supported was not, because that test's
  oracle is CI, which is green on every head. Reading the retained logs is what
  settles it: `gate-{lint,typecheck,doc-coverage}-issue875-final.out` each end
  clean and `gate-test-issue875-final.out` ends
  `make: *** [Makefile:229: test-nextest] Error 100`. The `-final` suffix is
  not a revision marker — it is reused across batches, so a log's binding comes
  from its timestamps against the commit times, not from its name.

  **There is no remaining gate: a draft pull request is the instructed end
  state, not a step on the way to a ready one.** This entry previously said the
  remaining gate was "clearing the draft flag". That was wrong, and it is
  corrected here rather than delivered: the request that opened this work asked
  for a draft pull request, and nothing in it asks for one marked ready.
  Clearing the flag would also contradict what this plan itself records — the
  linker-selection acceptance clause is unmet, and a ready-for-review pull
  request is a merge signal. CodeRabbit's GitHub-side review is suppressed on a
  draft (`success CodeRabbit Review skipped: draft pull request`), which is why
  the review evidence on this branch comes from foregrounded
  `coderabbit review --agent` passes rather than from the app's own check. The
  count is 17 invocations over 2026-10-13 05:59 .. 2026-10-16 16:57 UTC; the
  census method and its deduplication are in *Outcomes & retrospective*.

- [x] (2026-10-16 20:05Z) **`6558fd1f` is green in CI on every workflow, and the
  ExecPlan contract suite is observed passing there — which retires the one
  local gate this branch could not close.** The three workflows bound to
  `6558fd1fd1231a9701686734335c85b293890e30` each concluded `completed success`
  on attempt 1: `CI` `38947760136`, `Netsukefile Build Test` `38947760053` and
  `Release Dry Run` `38947760127` (plus `dependabot-automerge`, `skipped` by
  design). Reading each run's own job list rather than a summary: `CI`'s five
  jobs are all `success` (`kani-smoke`, `build-test`,
  `Windows / windows-msi-upgrade`, `Windows / build-test-windows`,
  `Windows / lint-windows`); `Netsukefile Build Test`'s one job is `success`;
  and `Release Dry Run`'s eleven jobs are nine `success` with
  `release / windows-native-recipe-smoke` and `release / publish-release`
  `skipped` by design. That is 16 jobs, 14 success, 2 skipped, 0 failed and 0
  cancelled, with nothing pending.

  **The `build-test` job log settles the question this plan has carried since
  the freeze: whether `tests/execplan_status_contract_tests.rs` runs.** It
  does. The job's nextest pass reports
  `3990 tests run: 3990 passed (1 slow), 6 skipped` in 253.798 s, and the log
  names the suite nine times, every one a `PASS` — including
  `netsuke-build::execplan_status_contract_tests every_execplan_header_status_is_within…`,
  the closed-set check that reads every `docs/execplans/*.md` at test time and
  is the specific reason a docs-only change still required `make test`.
  Grepping those nine lines for `fail`, `panic` or `error` returns nothing. The
  same job then runs the doctests successfully.

  **This is the right oracle, and the reason is provenance rather than
  convenience.** The local `make test` has now failed to reach a verdict three
  times on this branch, each time for infrastructure reasons rather than code:
  at `52084776` on a 300 s nextest timeout, and twice tonight on admission
  refusals. CI's `build-test` is a different machine with its own runner and no
  shared token pool, it is bound to this exact SHA, and it completed. So the
  `test` row of the eight-gate set is discharged by CI on `6558fd1f` — not by a
  local run — and this entry says so rather than implying a local green.

  **The local set, on this head, is not complete and the outstanding row is
  infrastructure.** `make check-fmt` passed (`RC=0`, 4 s, `cargo fmt` + ruff
  0.16.4 + mdtablefix all clean) and `make lint` exited `RC=2` having never
  linted a single netsuke item: `cargo doc --workspace --no-deps` completed
  cleanly first (13 m 09 s, `Generated … target/doc/netsuke/index.html`), then
  `cargo clippy` was refused at the admission layer. `grep -c 'error\['` over
  that log is `0`, all 18 `could not compile` lines name third-party registry
  crates only (`serde`, `thiserror`, `pear_codegen`, `rand`, `multiversion`,
  `zerofrom-derive`, `yoke-derive`, `miette-derive`, `tracing`, `futures-util`,
  `mockall`, `zerotrie`, `clap`), and `grep -cE 'Compiling netsuke v'` is `0` —
  so this is INDETERMINATE and establishes no defect in `6558fd1f`. Gates 3–8
  were deliberately not run after it: a later green gate on a tree whose
  earlier gate never executed proves nothing.

  **The cause is a machine-level fault in the admission path, and it is
  recorded here rather than worked around.** Three symptoms, read from the
  supervisor's own journal and socket state: the systemd user manager became
  unresponsive to D-Bus (`systemctl --user is-system-running` and a raw
  `dbus-send … org.freedesktop.DBus.Peer.Ping` both time out at 20 s); the
  supervisor then logged repeated
  `group gNNNN: cannot create buildlimits-2867217-gNNNN.scope`, each ending
  `Call failed: Connection timed out` — that is, it could not make the
  transient scope that bounds each compiler group; and with 256
  `build-limits-rustc` wrappers queued its socket accept queue saturated
  (`u_str LISTEN 129` against backlog `128`), which is why
  `build-limits-status` reports `cannot query the supervisor …: timed out` while
  `ss` shows the socket still listening. The supervisor also restarted twice
  tonight (`18:28:14Z` and `19:42:16Z`), and `sccache` was absent for part of
  the window, which is the `sccache unreachable, compiling directly` clause —
  an uncached build, not a failure. **No recovery action was taken:** restarting
  `systemd --user` would terminate every other agent's processes on this
  machine, which is out of scope for this branch, and build-limits admission
  must never be bypassed by unsetting `RUSTC_WRAPPER`. This is reported to the
  operator instead.

- [x] (2026-10-16 20:52Z) **The amended plan's own head `06124400` is green on
  the same three workflows, so the edit that this plan describes is itself
  covered by the suite it describes.** The three workflows bound to
  `061244001b5a7a249a552f508b3e311e06ec362a` each concluded `completed success`
  on attempt 1: `CI` `38962538616` (started 20:28:54Z, updated 20:50:46Z, five
  jobs all `success`), `Netsukefile Build Test` `38962538566`, and
  `Release Dry Run` `38962538751`. This matters beyond a repeat of the entry
  above, because the only delta from `6558fd1f` is this document, and this
  document is read at test time by `tests/execplan_status_contract_tests.rs`.
  The `build-test` job log for the new head reports
  `3990 tests run: 3990 passed (1 slow), 6 skipped` in 250.641 s and names the
  suite nine times, every one a `PASS`, including
  `every_execplan_header_status_is_within…` — so the amended `Status:` line,
  the new `Progress` entry above and the new `Surprises & discoveries` bullet
  all passed the contract that reads them. The in-line `Status:` shape was also
  checked directly against the committed blob before pushing:
  `git show HEAD:<plan>` is byte-identical to the working tree (so the checked
  object is the pushed one), and the value is a bare `COMPLETE` on exactly one
  line-initial `Status:` occurring above the first level-two heading.

  **A poll reading is not a conclusion, and one transient read was
  misleading.** A single sample showed `Release Dry Run` at `queued` *after* it
  had been `in_progress`, which reads as a restart or a regression. Querying
  the run directly instead of the poll's snapshot showed that it had in fact
  concluded `success` at 20:48. The run list endpoint's `status`/`conclusion`
  pair is the authority; a polled line is a point-in-time rendering of it.

- [x] (2026-10-16 21:25Z) **Three of this section's `2026-10-16` entries carried
  local times labelled as UTC, and they are corrected above.** The defect is
  worth stating rather than quietly fixing, because it is a provenance error in
  an artefact whose whole purpose is to bind claims to evidence, and it is the
  same class of error this plan spends its length warning about: a value that
  *looks* like a revision-bound fact while actually recording something else.
  The host runs at UTC+02:00. Two tools conspired. `journalctl -o short-iso`
  appends an offset, which is easy to read past and then drop; and a shell
  command formatting its own output prints local time with **no** offset at
  all, which is indistinguishable from UTC in the output itself.

  What makes this provable rather than suspected is the *committer date* of the
  commit that introduces an entry: an entry cannot post-date the commit
  carrying it. All three violated that, by almost exactly the local offset.

  ```plaintext
  entry as written  introducing commit  committer date (UTC)  corrected entry
  17:40Z            84b1e46f            17:05:35Z             15:40Z
  22:05Z            06124400            20:28:40Z             20:05Z
  22:52Z            b2efc6c4            20:57:48Z             20:52Z
  ```

  Each commit's own timestamp was written in the same mislabelled local frame,
  so the column is the committer date converted to UTC; author and committer
  agree on all three, which rules out a rebase having rewritten them. The
  writer-to-commit lag was 1h 25m, 23m and 5m, each well inside the two-hour
  offset, which is what makes the violation visible: an entry labelled local
  overstates its UTC time by exactly the offset, so it lands *later* than the
  commit that carries it. **The test has a blind spot worth naming** — had the
  lag exceeded two hours for any entry, that entry would have read as clean
  while carrying the same defect. The restart times quoted inside the `20:05Z`
  entry were read from `journalctl -o short-iso` as `20:28:14+02:00` and
  `21:42:16+02:00`, so as UTC they are `18:28:14Z` and `19:42:16Z`, and the
  entry now says so.

  This entry is itself the fourth instance, found by applying the same test to
  it. It was first written and labelled `21:40Z`, but the file's last write
  before the correction was `21:29:03Z` (mtime `23:29:03.948942034 +0200`), so
  the label post-dated the bytes it describes by about eleven minutes and was
  local time mislabelled again. Applying the offset gives `19:40Z`, which
  invents a different error; the truth is that the notice was written in local
  time without an offset and the honest UTC value is not recoverable from that
  label alone. Two different facts are being carried by one value, and
  separating them is the whole repair.

  The **unknown** is when this entry was originally written. The mislabelled
  local label plus the mtime interval does not determine it, and no clock
  reading recovers it; it stays unknown.

  The **measured window** is what the same evidence does establish. The entry
  cannot precede the artefacts it corrects — the corrected entries at `20:05Z`
  and `20:52Z`, whose introducing commits are `20:28:40Z` and `20:57:48Z` — and
  it cannot follow the file's last write before the correction, `21:29:03Z`.
  The written value `21:25Z` sits inside that window. It is a **chosen bound
  within a measured interval, not a recovered measurement of the original
  write**, and it is written here to say so rather than to imply an accuracy
  the evidence does not support.

  An earlier version of this paragraph justified `21:25Z` by calling it
  *earlier* than every artefact it cites. That claim was false on the
  paragraph's own figures: `21:25Z` is later than the corrected entries at
  `20:05Z` and `20:52Z` and later than their committer dates. The comparison
  also conflated *position in this list* with *clock order*, which need not
  agree. The claim is retracted rather than reworded; only the window argument
  above survives.

  **The honest limit of the check.** Only the three `2026-10-16` entries were
  audited this way. The `2026-10-14` entries (`02:05Z`, `03:10Z`, `04:20Z`,
  `05:20Z`) were **not** re-derived from their commits, so they are not
  certified by this correction — an entry whose clock happened to be handled
  correctly and one whose clock was not are indistinguishable from the text
  alone. A future reader wanting certainty should re-derive each from its
  introducing commit's committer date, which is the only durable witness
  available.

- [x] (2026-10-17 15:41Z) **The two `mod rustdocflags` line citations were
  quoted one line early, and are corrected.** Two places cited
  `tests/makefile_test_target.rs:347` for `mod rustdocflags`; the attribute is
  on 347 and the declaration is on **348**. The citation was repeated rather
  than re-read, which is the same propagation failure this plan records in
  *Surprises & discoveries* — a number copied from the plan's own prose instead
  of from the source. Confirmed by `grep -n 'mod rustdocflags'`, which prints
  `348:mod rustdocflags;`. No claim depended on the ordinal, and both sites now
  carry 348. At the EP-M3 site the surrounding sentence also states the
  entailment honestly: `ok Unmutated baseline` is a whole-run result, so "the
  named tests ran" follows from the target being in scope plus cargo stopping
  at the first failing target, not from a per-target line.

  The label's provenance, given the preceding entry: it was first written as
  `11:30Z`, a figure with no clock reading behind it. The reading taken
  immediately afterwards was `15:41:04Z`, and the label is that reading
  truncated to the minute — an artefact of a measurement rather than a second
  guess. It precedes the file's own mtime (`17:41:10 +0200`, i.e. `15:41:10Z`)
  by ten seconds, because setting the label is itself a write.

- [x] (2026-10-17 16:05Z) **`927da59e`, the head carrying the provenance
  corrections, is green on all three workflows, so the correction is itself
  covered by the suite that reads it.** The three workflows bound to
  `927da59ec363086a24f051041aa841f3092020e7` each concluded `completed success`
  on attempt 1: `CI` `39046361009` (five jobs all `success`; `build-test`
  15:44:40Z–16:02:25Z), `Netsukefile Build Test` `39046360928`, and
  `Release Dry Run` `39046360989` (11 jobs: 9 `success`, and `publish-release`
  and `windows-native-recipe-smoke` `skipped` by design). The delta from
  `6558fd1f` is this document alone (`git diff --name-only 6558fd1f 927da59e`
  names one file), and this document is read at test time by
  `tests/execplan_status_contract_tests.rs`, so the `build-test` pass is the
  remote check that the amended entries did not break the header contract.

  **Locally, before the push,** `make check-fmt` (exit 0,
  `181 files left unchanged`), `make markdownlint` with its `spelling`
  prerequisite (exit 0, `Linting: 181 files`, `0 issues in 0 files`) and the
  contract target itself
  (`cargo nextest run --test execplan_status_contract_tests --all-features`,
  `9 tests run: 9 passed, 0 skipped`) all ran sequentially on this content. The
  full eight-gate set was **not** re-run for a Markdown-only delta whose only
  consumer is the contract target just run; that omission is stated rather than
  glossed, and the earlier eight-gate verdicts stand for the revisions they
  were taken on.

  One operational note for anyone repeating this: the earlier attempt to patch
  the pull request description with `gh api … -f body=@/tmp/file` wrote the
  literal string `@/tmp/file` as the body, because the `-f` flag does not
  perform `@` file substitution. The body was replaced correctly by passing a
  JSON payload built in Python to `gh api --input -`, and the result read back
  afterwards. A 26-byte body is the tell.

- [x] (2026-10-17 17:10Z) **`7d48c32c`, carrying that record, is green on all
  three workflows, and the description is back to 772 lines.** The three
  workflows bound to `7d48c32c0bbb6f1cc005aa7c27356a5db865d237` concluded on
  attempt 1: `CI` `39050706857` (five jobs all `completed success`),
  `Netsukefile Build Test` `39050706879`, and `Release Dry Run` `39050706917` (9
  `success`, 2 `skipped` by design). The check-run census on the commit is 17
  rows, every one `success`; the PR's `mergeStateStatus` reads `CLEAN` for the
  first time in this session, which is a statement about the merge path and
  about nothing else — the pull request is still a draft by design, and a draft
  is not a merge signal.

  **What `CLEAN` does and does not mean here.** It means the branch's required
  checks pass against the base and GitHub sees no conflict. It does not mean
  the change is ready: the one unmet acceptance criterion — mold *selection* on
  the mutation worker, which needs an unmerged `rustflags` input upstream — is
  unchanged by anything this head does. The description states that criterion
  and the local `make test` discharge on `6558fd1f`, and those remain the
  reasons the draft state is correct rather than an oversight.

- [x] (2026-10-17 19:45Z) **An external completeness assessment found three
  gaps this plan had promised to close and had not, and all three are now
  closed.** The assessment was requested on `91a8ec69` and answered in
  [PR #911 comment 6250838137][assess-911]. Each finding was verified against
  the source before being actioned; none was accepted on the reviewer's word.

  1. **I1 had no assertion.** The invariant promised a contract check over the
     parsed caller plus a seeded negative control, and the plan's sketch of it
     — `"env" not in job` — had not been written at all:
     `mutation_testing_test.py` validated the `with:` block by whole-block
     equality and never looked for an `env:` key. The check that was added is
     deliberately wider than the sketch, for the reason given in
     *Verification plan*: `_flag_override_problems`
     now walks the entire parsed workflow (not just the job) for `RUSTFLAGS`
     and `CARGO_ENCODED_RUSTFLAGS`, and
     `test_no_compiler_flag_variable_is_set_anywhere_in_the_caller` asserts it
     is empty. `test_flag_guard_rejects_a_seeded_override` supplies the
     promised control as a permanent test rather than a one-off edit, and the
     control was **also** run out of process against a scratch copy of the
     workflow with `env: {RUSTFLAGS: "-D warnings"}` seeded onto the job: the
     clean tree yielded `[]` and the seeded copy yielded
     `['workflow.jobs.mutation.env.RUSTFLAGS (a RUSTFLAGS key)']`. The
     focused suite passes 10 tests.
  2. **The provenance paragraph overstated what it knew.** It called `21:25Z`
     *earlier* than every artefact it cited, which is false against its own
     table (`20:05Z`, `20:52Z` and the committer dates are all earlier), and
     it presented a chosen value as though it were a recovered measurement.
     The paragraph now separates the genuinely unknown original write time
     from the measured window the evidence does bound — it cannot precede the
     corrected entries (`20:28:40Z`, `20:57:48Z`) and cannot follow the last
     write of the file (`21:29:03Z`) — and states that `21:25Z` is a chosen
     bound inside that window, not a measurement. The retracted claim is
     named as retracted rather than silently reworded.
  3. **Two verification obligations were undischarged.** The I3 negative
     control (remove `check-build-tools` from `doctest`'s prerequisites in a
     scratch copy and confirm the case fails) had no result recorded, and the
     *Tolerances* escalation at 250 net lines had been exceeded without an
     entry. The control is recorded below; the escalation is recorded here.

  **Tolerance escalation — the scope threshold was exceeded, and the repair
  widened it.** *Tolerances* sets "more than 250 net lines" as a
  stop-and-escalate trigger. At the assessed head `91a8ec69` the branch's
  non-Markdown delta was `git diff --numstat c65ad970 91a8ec69` summed over the
  four code files: `58 + 7 + 136 + 142 = 343` added, `0 + 0 + 62 + 9 = 71`
  removed, so **272 net lines excluding Markdown** against a 250 limit, and
  2,739 net lines including the ExecPlan. Repairing the assessment findings
  adds the I1 guard and its negative control, taking the same measurement to
  `58 + 7 + 136 + 231 = 432` added and the same 71 removed — **361 net
  non-Markdown lines**. Both numbers are stated because reporting only the
  smaller one would describe a head that is no longer the candidate, and
  because the first repair attempt was itself amended twice: the plan's
  Markdown needed `mdtablefix` canonicalization, and the Python guard needed a
  `Returns` section and a `match` statement before `make lint` would accept it.
  Neither amendment changed behaviour, but both changed the count, which is why
  the figure is re-derived from the artefacts rather than carried forward. The
  overrun is small and entirely accounted for by the two contract tests the
  findings required and by provenance prose; it is nonetheless recorded as an
  **escalation** rather than absorbed silently, because post-hoc approval of a
  threshold is exactly the kind of quiet drift the threshold exists to prevent.
  No authorization for the overrun is claimed from anywhere; the reviewer is
  asked to accept it explicitly or to direct a split.

  **I3 negative control — executed, passed, and not on the first attempt.** The
  control runs the real `Makefile` in a scratch copy outside the worktree, with
  a fake `cargo` that records its invocations, and compares the target with and
  without the prerequisite. It took four attempts for the control to be
  faithful, and the three failures are recorded for the same reason the
  provenance corrections are: each one produced a non-zero exit that a careless
  reading would score as success. The first was *silently wrong in a way that
  still looked green-ish*: the host carries a real but wrong-version
  `/usr/bin/mold` (2.40.4), so a `PATH` that merely omitted a mold *directory*
  still resolved that one and the restored case failed for the wrong reason —
  `mold 2.40.4 at /usr/bin/mold does not match the pin 2.41.0`, which is
  version drift, not the missing-tool diagnostic the test asserts. The control
  establishes nothing until the failure is the one under test. The faithful
  environment is a `bin/` holding every utility the recipe calls — `make`,
  `bash`, `sh`, `awk`, `sed`, `grep`, `tr`, `cut`, `sort`, `uname`, `dirname`,
  `readlink`, `cat` — plus a `rustup` that answers the toolchain query, and
  **no** `mold` on it. In that environment: restored
  (`doctest: check-build-tools`) exits `2` after `0` Cargo invocations with
  `build-tools: mold not found on PATH (pinned 2.41.0)`; with the prerequisite
  seeded back out (`doctest:`) it exits `0` after `1` Cargo invocation. That is
  the relation, exercised in both directions, and the diagnostic is the exact
  string `build_targets_stop_when_the_check_fails` asserts. The same trap bit
  the control twice more before it worked. A `grep` symlink was created as the
  relative name `grep`, which resolves nowhere from an unrelated `PATH`, so
  `read_pin` reported an empty count
  (`expected one line in version pin: tools/mold/VERSION, found` with nothing
  after it) and the run failed on a pin-read rather than on mold. An
  `env -i PATH=…` without `make` on it fails with
  `make: No such file or directory`. Each of those produces a non-zero exit
  that a careless reading would score as "the control fired". Non-vacuity here
  is therefore not a formality: of the four environments tried, three broken
  ones all exit non-zero and only the fourth exits non-zero *for the asserted
  reason*.

[assess-911]: https://github.com/leynos/netsuke/pull/911#issuecomment-6250838137
[follow-up-927]: https://github.com/leynos/netsuke/issues/927

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.

- **Observation:** no gate can see a wrong number in prose, and the two numbers
  that were wrong here had been re-derived from the plan's own summary rather
  than from the source. Impact: a committed workflow comment cited the wrong
  run id for the 569-invocation census, and "fifteen `workflow_call` inputs" —
  a loose grep counting the `workflow_call:` and `inputs:` lines as keys — had
  propagated to three files. The load-bearing claim (`rustflags` is absent at
  every revision) survived both errors, so no conclusion changed, and that is
  precisely why nothing caught them: `make check-fmt`, `make markdownlint` and
  `make test-workflow-contracts` are all green on a false count. Corrected to
  run `38280609203` and to 13 inputs at the pin / 14 at the tip. Recourse: when
  a number in prose is a measurement, re-derive it from the artefact and read
  the *names* out rather than counting lines that look like names.

  A third instance of the same class surfaced later and was worse, because it
  was not a measurement but an **ordinal with no enumeration behind it**: the
  plan recorded the issue's acceptance position as "the fourth acceptance
  criterion", a phrase that appears in this file exactly once and matches no
  list it ever writes. The issue's `## Acceptance` section states **five**
  clauses and puts linker selection **second**; this plan's own *Purpose*
  paragraph separately lists five items and puts it **third**. Two numbering
  schemes were being quoted interchangeably and neither was named, so the
  ordinal survived six plan sites and the pull-request body unchallenged. A
  count that is wrong is at least checkable against its artefact; an ordinal
  with no enumeration is unfalsifiable, and no gate and no reviewer can catch
  one. Recourse: name the clause by quotation, and when two enumerations are in
  play, say which one is being counted.

- **Observation:** a descriptive log **filename** is not provenance, and one
  was read as though it were. The plan used one name for two different
  revisions: `52084776` was called "the eight-gate revision", but the all-green
  eight-gate run is bound to `3d761c8d`, and the batch that ran against
  `52084776` is green on three of the four Rust-bearing gates with `make test`
  ending `Error 100`. Impact: four passages carried the wrong label, and each
  was self-consistent enough that re-reading the plan confirmed rather than
  challenged it — the error propagated by being *restated*, not by being
  re-derived. The verdicts were unaffected, which is why no gate, no reviewer
  and no CI run could see it. Recourse: bind a gate run to its revision from an
  embedded `HEAD` marker or from its timestamps against the commit times, and
  treat a name like `-final` as what it is — a review round, not a revision.
  Two batches both used `gate-*-issue875-final.out` on different dates, so the
  suffix discriminates nothing. This is the third instance in this plan of the
  root cause recorded in the first bullet above, and the sharpest: an artefact
  existed, was cited, and was not opened.

- **Observation:** an `RUSTFLAGS` that is *set to the empty string* is not the
  same as one that is *unset*, and treating the two alike would have recorded a
  false green on the linker question. Measured against a two-line test crate
  whose `.cargo/config.toml` sets `-Clink-arg=-fuse-ld=mold`: with `RUSTFLAGS`
  unset, the flag reached the `rustc` command-line; with `RUSTFLAGS=` (empty
  but exported), it did not appear at all. Impact: the acceptance step reads "if
  `RUSTFLAGS` is empty … the linker requirement is intact", which is wrong as
  written — an empty value still displaces every `rustflags` table. The
  requirement is that both variables be **unset**, and the check belongs on the
  compiler arguments rather than on the environment. Mitigating factor found:
  the caller's `printf "…%s\n" "${RUSTFLAGS-<unset>}"` already distinguishes
  the three states — `<unset>`, an empty line, and a value — so no workflow
  change is needed; only the reading rule was wrong. Raised by CodeRabbit's
  third pass and confirmed by measurement rather than accepted on citation.

- **Observation:** the repository already documents the exact bug this plan
  fixes, in the shared workflow's own guide. Evidence:
  `https://github.com/leynos/shared-actions/blob/main/docs/mutation-cargo-workflow.md`
  states "Cargo takes `RUSTFLAGS` over every `rustflags` entry in
  `.cargo/config.toml`", "so a mold flag set there does not apply in these
  runs", and "mold is installed, and used only where the configuration selects
  it through `linker`". Impact: installing mold is necessary but is **not**
  proven sufficient. The workflow does not change `setup-rust`'s `rustflags`
  default of `RUSTFLAGS="-D warnings"`, which displaces `.cargo/config.toml`'s
  `cfg(target_os = "linux")` table — the very table that carries
  `-Clink-arg=-fuse-ld=mold`. Milestone 3 therefore *observes* linker selection
  rather than asserting it, and records the finding against
  <https://github.com/leynos/shared-actions/issues/544> instead of adding a
  local workaround. The plan's constraint forbidding a caller-side `RUSTFLAGS`
  follows directly from this.

- **Observation:** the pin in the caller is *not* the revision named in the
  issue text. Evidence: the issue says the caller is "pinned at
  `ff1dd759dfffc0db3459e30e833f52437ee62b57`"; the file at `main` (and at this
  branch's merge base `c65ad970`) pins
  `0606ca899ce2995a0c4bc78e660e80832e6e8855`. `ff1dd759` is a *different*
  shared-actions revision that this repository uses elsewhere, for
  `ensure-cargo-version` in `release.yml`. Impact: the issue's SHA is stale or
  was transcribed from another workflow. The conclusion is unchanged — the pin
  already exposes `install-mold`, it is merely not enabled. No pin bump is
  required, contrary to the issue's conditional "with a workflow revision that
  exposes it".

- **Observation:** the failure surfaces at the `check-build-tools`
  *prerequisite*, before the `doctest` recipe's own command line is composed.
  Evidence: `make: *** [Makefile:476: check-build-tools] Error 1`, with the
  fake Cargo never invoked. Impact: this is why the diagnostic is so unhelpful,
  and why the fix must be split — the CI half makes the worker capable, the
  test half stops the test depending on worker capability at all.

- **Observation:** a "mold-free `PATH`" is harder to construct on this host than
  the plan's EP-M2 red command assumed, and the first attempt *silently tested
  nothing*. Evidence: `PATH="/usr/bin:/bin"` still resolves a real
  `/usr/bin/mold` (a root-owned 22 MB ELF build of the very tool), and the
  Makefile re-prepends `$(BUILD_TOOLS_PREFIX)/bin`, `$(HOME)/.cargo/bin` and
  `$(HOME)/.local/bin` to `PATH` (line 216), so `$HOME/.local/bin/mold` is
  reachable from a child process whatever the caller passes. A probe built by
  symlinking only a hand-picked subset of utilities also failed for the wrong
  reason: it omitted `grep`, so `scripts/build-tools-common.sh` died at line 56
  with `grep: command not found` and `[ : integer expression expected`, which
  is a probe bug reported as a linker fault. Impact: the working probe mirrors
  `/usr/local/bin`, `/usr/bin`, `/usr/sbin`, `/bin` and `/sbin` into a
  temporary directory, symlinking every entry *except* `mold`, and is verified
  before use. With it, the real preflight reproduces the CI failure exactly:
  `build-tools: mold not found on PATH (pinned 2.41.0)`, exit 2. The plan's
  EP-M2 commands below are corrected accordingly. A probe that removes the
  wrong thing, or verifies nothing, returns a green that means nothing.

- **Observation (2026-10-12, milestone 3): the green runs were green because
  they evaluated no mutants, and this inverts the issue's framing of the
  evidence.** Evidence: every `Mutation testing` run from at least
  `2026-10-05T10:07:08Z` (`3f86c09d`) through `2026-10-12T10:24:10Z`
  (`c65ad970`) concluded `success` in GitHub, but in each one the
  `mutation / mutants` job — the *only* job that can evaluate a mutant —
  concluded **`skipped`**, and the run published **zero** artefacts. The
  `detect` job's own step table ends at
  `Detect changed files = completed/success`, which is why the mechanism was
  worth reading rather than guessing at. It is now settled, from the pinned
  shared workflow and its script at `0606ca89…`: the `mutants` job carries
  `if: ${{ needs.detect.outputs.has_changes == 'true' }}`, and `detect` runs
  `workflow_scripts/mutation_detect_changes.py`, whose `detect()` dispatches to
  `full_run_matrix()` only for `workflow_dispatch` and otherwise calls
  `changed_files()`, a
  `git log --since=<window-hours> <base-ref> -- <pathspec>` over the caller's
  `paths` (`src/,examples/,benches/`) with `window-hours` 25. An empty result
  sets `has_changes=false` and the shards skip. So the skip is neither a
  mystery nor a fault in the caller: on a quiet day the scheduled mutation
  workflow deliberately does nothing. (The earlier text here guessed "the
  pinned shared workflow, or a `paths` filter on the caller" — both halves were
  wrong in mechanism and right in implication.)

  The two runs in the same window that *did* launch shards both went **red**:
  `b6e7cf50` (3 Oct) and `bc812986` (12 Oct), though see the correction below:
  that red was `bc812986`'s recorded *conclusion*, and it is not the whole of
  what its shards did.

  Corrected, same session, once the shard logs were read: "shards run ⇒ red"
  was drawn from run *conclusions* and does not survive contact with the job
  logs. Letting the shards run past the baseline at `bc812986` shows four of
  the six reaching `ok Unmutated baseline` and then evaluating mutants —
  `MISSED` and `TIMEOUT` outcomes with per-mutant timings — before GitHub
  killed them with a runner shutdown signal at 57–86 minutes. So the honest
  relation is narrower: green ⇔ shards skipped, and the red runs are red for
  reasons that differ per run and must be read from the artefacts. The "shards
  run ⇒ red" clause is retracted; it would have understated the branch's
  effect, because it implies running the shards is itself the fault.

  Impact, recorded because it changes what "the documented missing-mold
  failure" is evidence *for*: the mold diagnostic is real and this plan
  reproduces it, but it is **not** the reason eight consecutive runs evaluated
  no mutants. Confirming it requires artefact inspection, not the run
  conclusion.

  A second over-claim, caught while preparing the PR body and corrected here:
  this entry first said "no green run that predates this branch executed a
  mutant". That is false. Run `35499357086` (20 September, head `c7cf1485`)
  concluded `success` with `mutation / mutants` **and** `mutation / summarize`
  both `success`, so it did evaluate mutants. The true statement is narrower
  and more useful: a `skipped` shard and a `successful` shard both produce a
  green run-level tick, so the run conclusion alone cannot distinguish "the
  mutants passed" from "no mutant was evaluated" — which is why the shard job
  is the only thing worth reading. The eight consecutive skip-only runs remain
  a real gap; they are just not evidence that *no* earlier run ever worked. See
  also the next observation, which shows the same artefact-inspection
  discipline applied to the 3 Oct failure.

- **Observation (2026-10-12, milestone 3): two *different* baseline tests have
  failed the mutation worker, and cargo-mutants' phase split is what hides
  which.** The 3 Oct report (`mutation-report-root-0`, artefact from run
  `37111347135`) and the 12 Oct report (`mutation-report-root-2`, artefact
  `11760992373`, job `114916843738`) agree on a structure that misleads: in
  both, `phase_results[0]` is the Build phase and reports
  `process_status: "Success"` — 122.4 s and 119.9 s of real Cargo respectively
  — while `phase_results[1]`, the Test phase, reports `Failure: 101`. Both
  record `total_mutants: 0`, and cargo-mutants states the consequence in
  `debug.log`:
  `cargo test failed in an unmutated tree, so no mutants were tested`.

  The Build phase succeeding does **not** mean the worker could build what
  matters. Its argv is `cargo test --no-run --package=netsuke-build@…`, which
  compiles and links but never executes a test body, so it never reaches the
  `make doctest` invocation that needs the linker. Everything this plan is
  about happens in the Test phase. Reading `Build: Success` as "the mold
  problem is not involved" would have been exactly the error this section
  exists to warn about; it very nearly was.

  The two failures, now identified test-by-test:

  - **3 Oct (`b6e7cf50`) is the issue's documented mold failure, verbatim.**
    `log/baseline.log` carries 20 repetitions of `Error: doctest with a
    environment override should call fake Cargo successfully: build-tools: mold
    not found on PATH (pinned 2.41.0)`, and the failing pair is
    `rustdocflags::behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags`
    and `rustdocflags::property_doctest_preserves_rustdoc_override_values` —
    the two tests named in issue #875, in the module this branch rewrites. The
    last result line is `test result: FAILED. 36 passed; 2 failed`.
  - **12 Oct (`bc812986`) is a different and out-of-scope failure.** This time
    the two rustdoc tests are *not* among the failures. `log/baseline.log`
    records `--- runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`
    with `Error: a structural summary must emit exactly one trace event: []`,
    and `test result: FAILED. 1468 passed; 1 failed`. Its helper is
    `test_support::tracing_capture::with_test_subscriber`
    (`test_support/src/tracing_capture.rs:103`), which installs its subscriber
    via `tracing::subscriber::with_default`; the sibling
    `src/test_tracing_capture.rs:177` documents the identical caveat in the same
    words — thread-local, "so only events emitted on the calling thread are
    captured". This shares a symptom with the measured ~5%-under-multi-thread
    flake in `stdlib::which::telemetry_tests::tracing_capture`, but the module,
    test and helper all differ, so it is **not** asserted to be the same defect.
    Neither the failing test nor any tracing helper is touched by this branch
    (`git diff --name-only c65ad970..HEAD` lists five files, none of them
    telemetry).

  Impact: the two runs failed for different reasons, and only the first is in
  scope. This branch removes the first from the worker entirely — the rustdoc
  tests no longer cross the build-tools preflight at all — and there is direct
  evidence of that landing (see Progress, 19:20Z). It does **not** address the
  second, so the mutation baseline may still go red for an unrelated reason
  after this merges. That is escalated rather than silently widened into scope,
  and the escalation now has an owner: [#927][follow-up-927] records the
  reproduction and diagnostic requirement, and states explicitly that the
  `manifest_structure_telemetry` failure and the `which::tracing_capture` flake
  share a symptom and a helper family but **not** a proven cause. The Worker
  dispatch section states what to look for in a future baseline.

  Two smaller inferences recorded because they are traps. First, the 3 Oct
  shard used a *different* pin (`ff1dd759…`, not the current `0606ca89…`) and
  its `mutants` job name carried a large explicit source-file list rather than
  the `0..5` shard indices, so the two runs are not the same regime. Second,
  the step rows are not trustworthy evidence that a phase ran: on 12 Oct, step
  12 `Upload mutation report = completed/success` while step 11 was `failure`,
  and on 3 Oct the same pattern holds — the artefact is what proved the phases'
  outcomes, and the step table alone would not have.

  Retracted, same session: this entry first recorded that
  `manifest_structure_telemetry_carries_only_counts` had *also* failed in the
  local `make test` at this revision, making two independent modules with the
  same symptom. **That was false.** `/tmp/GATE3-issue-875-test.out` line 1469
  records
  `PASS [0.041s] (872/3914) netsuke-build runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`;
  the two failures in that log are
  `build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  and
  `ninja_status_process_rss_tests::status_parsing_rss_stays_bounded_at_the_process_boundary`,
  neither of them a tracing-capture test. Kept rather than deleted: the same
  trap — reading a familiar symptom into a log without grepping for the name —
  is what this whole bullet is about.

- **Observation (2026-10-13, milestone 3): "four tests failed" and "the code
  is broken" are different statements, and `os error 11` is the difference.**
  The first `make test` of this milestone reported
  `1479/3914 tests run: 1475 passed, 4 failed, 6 skipped`, and four failures
  reads like a finding. It is not one. Three of the four failed with
  `Resource temporarily unavailable (os error 11)` while spawning the netsuke
  child process, and the fourth failed in `std::thread::spawn` itself:

  ```plaintext
  thread 'generate_to_unwritable_path_fails_with_path_error' (1113978) panicked at
    .../library/std/src/thread/functions.rs:131:29:
  failed to spawn thread: Os { code: 11, kind: WouldBlock, message: "Resource temporarily unavailable" }
  ```

  A panic inside `thread::spawn` precedes every line of netsuke's own logic, so
  no assertion in that test was ever reached — it is a statement about the
  host, not about the branch. Two measurements place it: the session cgroup
  (`lody-daemon.service`) recorded **18516** cumulative `pids` refusals with
  `pids.current` near 10.4k against `pids.max=16384`, and the host load average
  was **35** on 6 cores because other agents were compiling. Untouched sibling
  tests in the same binary passed in the same run, which is what rules out a
  binary-wide or module-wide cause. The response is a re-run under lower load;
  editing source here would be chasing a ghost. This is the same shape as the
  earlier retraction above — a familiar-looking symptom read as a verdict —
  with the twist that the useful check is not the test's name but *where* its
  failure occurred.

- **Observation (2026-10-13, milestone 3): in a job log, `##[error]` is often an
  action echoing its own script, so the *step* conclusions — not a grep — say
  what failed.** Two jobs went red on `172dd4ff`: `build-test` and
  `Windows / lint-windows`. Grepping the Windows log for `##[error]` returned,
  among others:

  ```plaintext
  echo "::error title=Install mdtablefix failed::mdtablefix ${MDTABLEFIX_VERSION}
    publishes no prebuilt release for ${RUNNER_OPERATING_SYSTEM:-unknown}/..."
  ```

  which reads exactly like the known upstream gap where mdtablefix ships no
  Windows binary — an appealing story that would have exonerated this branch.
  It is false. That line is the action's own script text printed verbatim; the
  `Install mdtablefix` step concluded `success`. Asking the API for the step
  list instead shows the real failure, and it is identical to `build-test`'s:
  step `Format` (`make check-fmt`) failed on
  `docs/execplans/issue-875-…mold.md +61 -62`. Both red jobs had one cause —
  this plan's hand-wrapped Markdown. The general form: a composite action's
  guard branches appear in the log whether or not they execute, so their text
  is not evidence. Enumerate `steps[].conclusion` and read the log around the
  first non-`success` step.

- **A claim about upstream state decays, and this plan made three of them.**
  The mold-selection evidence is partly a statement about a *moving*
  repository, and it was written once and then quoted. By 2026-10-16 all three
  copies had gone stale in the same direction: the upstream tip had moved from
  `aea9c14c24b7` (14 `workflow_call` inputs) to `e693d81d` (17), so every
  sentence naming the tip as "its tip (`aea9c14c24b7`)" was simply false; and
  "#544 is open with no fixing commit" had become a claim about the absence of
  something that now exists — shared-actions#628, `BLOCKED`, whose head adds
  the very `rustflags` input this branch needs. None of this changes the
  verdict — no *merged* revision declares the input, so the acceptance clause
  is still unmet and no pin bump is possible — but the *reasons* had rotted,
  and a reviewer checking the cited SHAs would have found the citations wrong.
  The lesson is that an external-state claim needs either a date and a re-check
  instruction or an anchor that cannot move, and ideally both. What changed
  here: the workflow comment now names only the pinned revision (stable) and
  says "no merged revision" rather than naming a tip, cites #628 by number with
  its state, and carries "re-check #628 before assuming it still stands"; the
  same treatment is applied at every other copy. A pinned SHA is checkable
  forever; "the current tip" is checkable for about a day.

- **Observation:** a build queue can fail by *saturation* rather than by
  refusal, and the two look nothing alike from the caller's side. Impact: the
  admission path on this machine degraded in a way this plan had no prior for.
  The forms previously recorded here are a queued build printing
  `[build-limits] CRATE: waiting for a compile slot` (wait, do not kill) and a
  refusal printing `build-limits: the supervisor refused: unhealthy` (a fast,
  loud failure). Tonight produced a third: the supervisor stayed **alive and
  healthy** while becoming unable to do its job, because the systemd user
  manager it calls to create a per-group scope stopped answering D-Bus. So
  `build-limits-status` reports `cannot query the supervisor …: timed out`
  *while `ss` shows the socket still listening with 129 queued against a
  backlog of 128* — the diagnosis is in the accept-queue depth, not in the
  process table. The supervisor's journal is what names the real cause:
  `cannot create buildlimits-2867217-gNNNN.scope: Call failed:
  Connection timed out`.
  Recourse: when a status probe times out, check the listener's queue depth
  and read `journalctl --user -u buildlimits-supervisor.service` before
  concluding the supervisor is down — and note that a *restarted* supervisor
  whose socket is recreated (`mtime` on `supervisor.sock`) means the queue was
  reset, so `held` counts taken before and after a restart are not comparable.
  The distinction matters because the responses differ: a refusal is a reason
  to re-dispatch, saturation is a reason to stop adding work to the machine.

- **Observation (2026-10-18): `RUSTFLAGS` displaces a config-declared
  `rustflags` table but *not* a config-declared `linker`, and the plan's
  recorded remedy for the linker clause was wrong.** Two claims in this plan
  had been carried without measurement: that a consumer can regain the mold
  flag by declaring it in `.cargo/config.toml`, and that shared-actions#628 is
  the upstream fix. Both are now settled by direct measurement and by reading
  #628's actual diff.

  The measurement is a two-line crate compiled three ways, with the `rustc`
  argv read each time:

  | Configuration                                                   | `RUSTFLAGS`                            | `-C linker=` in argv | `-Clink-arg=-fuse-ld=mold` |
  | --------------------------------------------------------------- | -------------------------------------- | -------------------- | -------------------------- |
  | `linker = "clang"` + `rustflags = ["-Clink-arg=-fuse-ld=mold"]` | unset                                  | `clang`              | present                    |
  | same                                                            | `-D warnings`                          | `clang`              | **absent**                 |
  | same                                                            | `-D warnings -Clink-arg=-fuse-ld=mold` | `clang`              | present                    |

  A fourth configuration puts a wrapper script in the `linker` key and was
  invoked with `-D warnings` exported and without it, so the `linker` key is
  measured to survive the export while the `rustflags` table is measured not
  to. netsuke's `.cargo/config.toml` selects mold through the `rustflags` table
  and declares no `linker` key, which is the precise reason the worker's
  exported `-D warnings` drops the mold flag — and why the census reads 0 of
  569 invocations carrying it while all 569 carry `-D warnings`.

  The #628 reading is that its `rustflags` input is documented as a "literal
  `RUSTFLAGS` replacement for the Cargo child" and is passed to the Python
  command builder for the cargo-mutants child process only. That serves a
  Cranelift consumer who must *remove* development backend flags from the
  mutant child. This issue needs the mutant child to carry *more* flags, and
  the replacement value could in principle name `-Clink-arg=-fuse-ld=mold`, but
  a caller-owned replacement does not preserve configuration-owned flags and
  does not, by itself, resolve the selection criterion. Even at upstream head,
  `mutation-cargo.yml` calls `setup-rust` with `install-mold` alone, so the
  `-D warnings` export survives. So #628 is not the remedy for this issue's
  linker clause, and the plan's earlier framing of it as "the proposed fix" is
  retracted.

  Impact: the surviving in-repository route is the `linker` key — either a
  wrapper script, which makes mold the linker for *every* build rather than
  only mutation and is therefore a design change rather than a configuration
  tweak, or an upstream change to what the shared workflow exports. Neither was
  acted on unilaterally; the boundary question is put to CodeRabbit on the pull
  request, and the clause stays open in the meantime. The lesson is the one
  this plan has already recorded twice in other forms: a plausible mechanism is
  a hypothesis until a command distinguishes it, and here the distinguishing
  command was cheap.

## Decision log

Record every significant decision made while working on the plan.

- **Decision:** do not bump the `uses:` SHA. Add only `install-mold` and
  `setup-commands` to the existing `with:` block. Rationale: the pinned
  revision already declares both inputs (verified by reading the workflow at
  that exact SHA and by confirming upstream commit `9a279509` is an ancestor).
  Bumping would widen the diff, re-pin an unrelated set of actions, and
  contradict the caller's own contract test, which deliberately asserts the
  *shape* of the pin — a 40-character lowercase hex SHA — rather than its
  value, because Dependabot owns the value. Date/Author: 2026-10-12,
  implementer.

- **Decision:** pass `install-mold: 'true'` as a quoted YAML string.
  Rationale: the input is declared `type: string`. A YAML boolean would fail
  the workflow-call type check. The contract test asserts the *string* form so
  a later `true` → `'true'` edit cannot silently reintroduce the fault.
  Date/Author: 2026-10-12, implementer.

- **Decision:** verify the pin with `make check-build-tools` in
  `setup-commands`, printing `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` for the
  log, and set no `RUSTFLAGS` in the caller. Rationale:
  `make check-build-tools` is the repository's own definition of "the pinned
  mold is present and usable", and its diagnostics are exactly the ones the
  issue asks to retain. Printing the two flag variables turns the open
  `shared-actions#544` question into recorded evidence rather than speculation.
  Setting `RUSTFLAGS` is forbidden by the constraints. Date/Author: 2026-10-12,
  implementer.

- **Decision:** the two rustdoc tests inject the build-tools boundary rather
  than reaching the real preflight; the real preflight gains `doctest` in the
  `tests/build_tools_check_tests.rs` parameterized table. Rationale: the tests'
  subject is argument propagation across the Make-to-Cargo boundary, not host
  capability. Coupling the two made the tests fail for a reason unrelated to
  their assertions. The capability gate already owns real-preflight validation
  in the two dedicated suites named in its own module docstring; adding
  `doctest` there keeps coverage of the exact prerequisite relation that broke
  CI. Date/Author: 2026-10-12, implementer.

- **Decision:** take versions from the committed-pin readers
  (`pinned_mold_version()`, `pinned_toolchain()`) rather than hard-coding
  `2.41.0`; where a literal must appear in a contract test, read
  `tools/mold/VERSION`. Rationale: `sandbox/pins.rs` exists specifically so
  tests "agree with the repository rather than hard-coding a value that drifts
  on the next bump". Date/Author: 2026-10-12, implementer.

- **Decision:** use the `Sandbox` Make runner, which sets
  `BUILD_TOOLS_PREFIX=<sandbox prefix>`, so `doctest`'s prerequisite runs
  against the sandbox's fake mold instead of the caller's real toolchain.
  Rationale: this is the same arrangement
  `tests/build_tools_check_tests.rs::healthy_sandbox` already uses, and it is
  the only arrangement under which the rustdoc tests are hermetic. The
  `BENCH_ROOT`/`doctest` bookkeeping risk is therefore not applicable.
  Date/Author: 2026-10-12, implementer.

- **Decision:** change no helper for the 18 September `Text file busy`
  (`ETXTBSY`), and add no retry. Rationale: the audit found the write handle
  provably closed before the mode change and before any spawn —
  `std::fs::write` returns `io::Result<()>` and so cannot leak a handle — and
  the code is byte-identical to the revision that ran on 18 September. A change
  without a reproduction would be a guess, and a retry would mask the next real
  occurrence. The issue itself calls the earlier `ETXTBSY` "separate from the
  demonstrated missing-mold failure". Date/Author: 2026-10-12, implementer.

- **Decision:** probe a mold-free `PATH` with a full mirror minus `mold`,
  and verify the probe before using it. Rationale: the first two attempts
  produced results that looked meaningful and were not — `PATH="/usr/bin:/bin"`
  still found `/usr/bin/mold`, and a hand-picked subset of symlinks omitted
  `grep`, failing inside `scripts/build-tools-common.sh` for an unrelated
  reason. Impact: the recorded evidence names the probe's construction and the
  checks that it was sound, because a negative control that cannot fail is
  worse than none. Date/Author: 2026-10-12, implementer.

- **Decision:** pass `install-mold` alone and no version, because the pinned
  revision declares no mold-version input. Rationale: the pin's full
  `workflow_call.inputs` table is `paths`, `extra-crate-dirs`, `exclude-globs`,
  `window-hours`, `base-ref`, `timeout-multiplier`, `timeout-minutes`,
  `shard-count`, `cargo-mutants-version`, `extra-args`, `allow-no-mutants`,
  `setup-commands` (`type: string`, `default: ""`) and `install-mold`
  (`type: string`, `default: 'false'`). Nothing names a mold version, so there
  is no value to pass and no "if a mold-version input exists" branch to take;
  the version stays owned by `tools/mold/VERSION`, which `check-build-tools`
  reads. Impact: the plan's Task 1 conditional resolves to its negative arm,
  and the version-equality test the task made conditional is not written —
  `make test-workflow-contracts` would otherwise assert an input the shared
  workflow rejects. Date/Author: 2026-10-12, implementer.

- **Decision:** leave the caller's `setup-commands` to run *after* the mold
  installation without any ordering guard. Rationale: in the pinned revision
  the mutants job installs mold inside "Setup Rust" (the `setup-rust` action is
  given `install-mold: ${{ inputs.install-mold }}`), and "Run setup commands"
  is a later step, so the preflight cannot observe a worker that has not yet
  had its chance to install the linker. Impact: the two inputs are genuinely a
  pair, which is what the contract test asserts; a future reordering upstream
  would make the preflight fail loudly rather than silently pass. Date/Author:
  2026-10-12, implementer.

- **Decision:** evaluate "runner, cache and concurrency unchanged"
  mechanically by parsing both revisions and comparing the fields, not by
  reading the diff. Rationale: a comment-only edit and a settings edit look
  identical in `git diff`, and the plan's acceptance criterion is about
  settings. Impact: triggers (`on`), top-level and job `permissions`,
  `concurrency`, the `uses` pin, `runs-on` and `env` compare equal between the
  merge base `c65ad970` and `HEAD`, while `with:` differs by exactly the two
  added keys. No cache key exists in either revision, so there is none to
  preserve. Date/Author: 2026-10-12, implementer.

## Outcomes & retrospective

Measured against `Purpose / big picture`, which asked for five things: a worker
that has the pinned mold, rustdoc tests that stop depending on worker
capability, the linker requirement kept intact, the flag-ownership question
recorded upstream rather than patched locally, and an ETXTBSY audit that
changes nothing unless it reproduces. The first, second, fourth and fifth are
met. The third is met only under one of its two readings, and the split is set
out below rather than averaged away.

**The issue's Acceptance clause that names linker selection is unmet; the
others are met.** The two numbering schemes in play are distinct and both are
now stated by name. The paragraph above counts the five items of this plan's
own *Purpose*, where linker selection is the third. The issue's `## Acceptance`
section separately states five clauses — the named tests and baseline pass;
linker selection remains intact; nonempty mutants execute; diagnostics are
retained on failure; runner, cache and concurrency are unchanged — and linker
selection is the **second** of those. An earlier revision of this plan called
it "the fourth criterion", an ordinal matching no enumeration it ever wrote;
that is corrected here and the clause is referred to by name below. The worker
provisions mold — `build-tools: mold 2.41.0` matching `tools/mold/VERSION`, on
`PATH` via `GITHUB_PATH`, with the `Run setup commands` step moving from
`skipped` to `success`. The baseline then reports
`ok Unmutated baseline in 122s build + 218s test` after
`Found 636 mutants to test`, and mutants are evaluated and reported. The two
named tests pass locally and run inside that baseline — an inference rather
than a per-target log line, because `ok Unmutated baseline` describes the whole
`cargo test` run: `tests/makefile_test_target.rs:348` declares
`mod rustdocflags` and the mutation run passes no `--test` filter, so the
target is in scope, and cargo stopping at the first failing target means an
`ok` run left no target failed. The rustdoc tests now inject the build-tools
boundary through the shared sandbox, so they measure `RUSTDOCFLAGS` propagation
instead of host capability, while `build_tools_check_tests.rs` keeps the real
prerequisite under test — including the new `doctest` case that reproduces the
original failure. Flag ownership is recorded against shared-actions#544 with
**no local workaround**, and the ETXTBSY audit was read-only with no helper
changed and no retry added.

**The linker-selection clause, and the reading it turns on.** The issue's
Acceptance asks that "required linker selection remains intact". That phrase
has two readings and they disagree here, so both are stated.

Under the **strict** reading — the worker's baseline is linked as the build
standard requires — it is **not met**, and this change does not meet it. The
census is unambiguous: `-Clink-arg=-fuse-ld=mold` appears on 0 of 569 rustc
invocations while `RUSTFLAGS=-D warnings` appears on all 569, so the baseline
compiles through the platform linker. `make check-build-tools` cannot close
that gap, because it verifies the pin is *installed* rather than that Cargo
*selects* it — which is why the preflight can be green on a worker whose
baseline is not built to the standard.

Under the **no-regression** reading — this change does not *remove* selection
that was working — it is satisfied, and vacuously so: the 3 October worker had
no mold at all, so it had no selection to lose. The change moves the worker
from "no mold, no selection" to "mold installed, still no selection", which is
the improvement the issue asked for but not the whole of it.

The strict reading is the one to carry forward, because the no-regression
reading is satisfied by doing nothing. **A worker-scoped `RUSTFLAGS` override
is the one remedy explicitly forbidden** — by the issue's own wording, by
*Constraints*, and by CodeRabbit's finding — and it would displace the
`.cargo/config.toml` state the build standard is defined by. So the strict
criterion is left unmet and recorded rather than silently satisfied, and the
mutation worker's results must be read with that caveat until selection is
owned upstream.

The remediation route recorded here on 2026-10-16 — "a `rustflags` input on
`mutation-cargo.yml`, now proposed as shared-actions#628" — is **retracted**,
because two measurements do not support it. #628 supplies a child-only
flag-replacement mechanism: a literal `RUSTFLAGS` value applied to the
cargo-mutants child only. Its replacement *can* name
`-Clink-arg=-fuse-ld=mold`, but it does not preserve configuration-owned flags
and does not, by itself, resolve the selection criterion; caller-owned flag
replacement is what this plan's constraints forbid. And a config-declared
`rustflags` entry does not survive an exported `RUSTFLAGS` at all — the
three-way compile measurement in *Surprises & discoveries* shows the table
being displaced while the `linker` key survives. What is left is the route the
shared workflow's own `install-mold` description points at, "select mold in the
project's Cargo configuration", expressed through the one key an exported
`RUSTFLAGS` does not displace. That is either a repository-owned `linker`
wrapper — which would make mold the linker for every build, mutation or not,
and so is a design change — or an upstream change to what the shared workflow
exports. The boundary question went to review on the pull request on 2026-10-18
and was answered: repository-owned selection is valid in principle, but it is a
build-policy change to be delivered **separately from #911**, because the
release, packaging and coverage exclusions rely on an assigned `RUSTFLAGS`
displacing the configured table, and a wrapper that survives that displacement
would survive in those builds too. That separate change is tracked under #875;
#544 keeps the broader flag-ownership defect. So the criterion stays open in
this plan, and the wrapper is deliberately not written into this branch. This
is the second finding on the branch that a green gate cannot settle; the first
was that installing a linker on another job does not provision this one.

**Three lessons worth carrying forward.** First, the issue's premise needed
re-cutting: the reported symptom was accurate, but "the baseline reaches the
preflight without mold" and "the rustdoc tests are non-hermetic" were two
faults whose fixes belong in different files, and fixing only the CI half would
have left the tests measuring the wrong thing. Second, an early Progress entry
attributed the shard endings to a single ~60-minute platform cap after reading
two shards; the complete data showed two mechanisms and no such cap. Naming a
cause from the first convenient sample is the failure mode to watch — the retry
is recorded in `Risks` rather than quietly corrected. Third, a gate result is
only as good as the environment it ran in: this host produced 4 and then 9
timeouts on an unchanged diff, and finally a clean run with none. The count of
`sccache server is unreachable` lines looked like the diagnostic and was not —
the clean run logged more of them than the failing one (1034 against 573) while
passing. Correlation with an environment diagnostic is a hypothesis to test by
counter-run, and the counter-run here arrived by accident.

**Now claimed as evidence, after being discharged.** An earlier doctest attempt
never completed on this host — the run hung on an idle process holding a
build-limits slot and was stopped — and this section previously recorded the
half as incomplete rather than as a pass. The 08:15Z gate run now passes it:
`Doc-tests netsuke` `88 passed` / `2 passed` and `Doc-tests test_support`
`39 passed`, each `0 failed`, both `all doctests ran`. The earlier hang is
therefore an environment condition, not a property of the change. The two jobs
that were red on `172dd4ff` (`build-test` and `Windows / lint-windows`) were
confirmed repaired at `d8e03753`; because every commit after that one touches
Markdown only, their re-run is a formality, and the standing requirement is
that CI be green on every head, including the draft pull request's final one.
That requirement is met on `8c0490eb` and on every head after it, each of which
is docs-only over the byte-identical non-Markdown tree of `52084776`. **Census
of the retained `coderabbit review --agent` logs:** 17 invocations between
2026-10-13 05:59 UTC and 2026-10-16 16:57 UTC, all `currentBranch` = this
branch and all `workingDirectory` = this worktree. Thirteen carried findings
and four returned none. The twenty-three finding records are eleven instances
of the mold-selection finding plus twelve others standing at ten distinct
sites, two of which were raised twice. So there are **eleven distinct defects:
ten fixed, and the mold-selection gap standing as a recorded upstream
blocker**. The ten fixed ones, by site, are: first-person pronouns in this
plan; a second-person pronoun with a subject-verb agreement error; a
reproduction recipe that could not work; a mold-free conformance command that
cannot pass and was re-raised at the same line in a later pass; an empty
`RUSTFLAGS` treated as proof that Cargo selects mold; an acceptance line
demanding evidence that was never obtainable; a conformance check naming
`.github/actions/...` where the caller pins
`.github/workflows/mutation-cargo.yml`; a *Context and orientation* section
still describing the caller's `with:` block and the rustdoc tests as they stood
before this branch changed them; a flag-precedence comment in the contract test
stated inaccurately; and, in that same test, an empty `case … : pass` arm
replaced by a typed narrowing helper. The mold-selection finding is one defect
raised eleven times, not eleven defects. The "nine passes" figure this
paragraph previously carried, and the "seven foregrounded passes" in
*Progress*, were both impressions rather than counts, and the "six prose
defects" they carried understated the fixed work by four. Re-derive a count
like this from the retained logs, keyed on `(fileName, startLine)`, rather than
from recollection. A zero-finding review is not reachable while the
linker-selection clause is unmet, and treating one as the exit condition would
mean either deleting the finding or writing a workaround the issue forbids.

Two of those seventeen passes were run back to back over the identical frozen
revision `52084776`, and they returned **0 and 2** findings. That is the
sampling behaviour stated as a measurement rather than a caveat: a "clean
review" is a property of one sample, so the exit condition this plan uses is a
stated disposition per finding, not a finding count.

**Upstream, unresolved by design.** shared-actions#544 remains open, and it now
has no settled remedy: shared-actions#628 is `BLOCKED` and unmerged, and as
recorded above its child-scoped replacement input is not the change this clause
needs. So a caller still cannot both request mold and have Cargo select it
through the project's own `rustflags` table. shared-actions#590 covers the
shard evidence, and shared-actions#591 the preflight consolidation. None of the
four is reconcilable within this repository, and `Conformance basis` records
them as external dependencies rather than gaps in this change. Because #628 is
a claim about upstream state that upstream moves, and because its *scope* was
misread here once already, re-read it before treating this paragraph as current.

**Live on `main`, not historical (recorded 2026-10-18).** The issue's symptom
is not a 3 October artefact. Four scheduled runs that actually executed a
baseline failed with the exact diagnostic, all with `install-mold: false` and
empty `setup-commands`: 13 Oct (`38382037308`, `Found 151`), 14 Oct
(`38535334915`, `Found 151`), 16 Oct (`38870493586`, `Found 62`) and 17 Oct
(`39014183758`, `Found 140`), the last three reporting the diagnostic after
`FAILED Unmutated baseline`. The 16 and 17 October logs also state
`has_changes=True`, which is the cleanest witness of a non-empty change set
reaching a linker-less worker. Two further scheduled runs concluded `success`
while measuring nothing — 15 Oct (`38702941267`) and 18 Oct (`39109192091`),
both `no matching changes` with `has_changes=False` and every downstream job
`skipped` — so their green badges must not be read as mutation testing being
healthy. That pairing is the completeness gap in shared-actions#590.

## Context and orientation

The reader needs five things to follow this plan.

**The build standard.** Netsuke compiles with the `mold` linker and the parallel
`rustc` frontend by default. `.cargo/config.toml` declares them in a
`[target.'cfg(target_os = "linux")']` table, so a bare `cargo build` gets them.
Cargo selects *one* `rustflags` source, so an externally set `RUSTFLAGS`
displaces that table entirely — the reason the `Makefile` restates the flags in
`GATE_RUSTFLAGS` (`Makefile:78`).

**The capability gate.** Every build, test, and lint target depends on
`check-build-tools`, a phony target whose recipe is
`@scripts/check-build-tools.sh`. The script reports `build-tools: <message>` on
stderr and exits non-zero if `mold` is missing, unrunnable, or at a version
other than the one in `tools/mold/VERSION` (currently `2.41.0`), or if the
pinned toolchain is absent.

**The mutation caller.** `.github/workflows/mutation-testing.yml` is a thin
caller of `leynos/shared-actions/.github/workflows/mutation-cargo.yml`. It has
one job, `mutation`, whose `with:` block carries `exclude-globs`,
`extra-args: "--all-features"`, `install-mold: 'true'` and a `setup-commands`
script; the last two are this branch's change, described under *Milestones*.
The default `setup-rust` on the shared worker exports
`RUSTFLAGS="-D warnings"`, and the caller must not fight that. `install-mold`
is a string input rather than a boolean, because the shared workflow declares it
`type: string` and a bare `true` is rejected at workflow-call time.

**The shared sandbox.** `test_support/src/build_tools/sandbox/mod.rs` builds a
temporary `PATH` and `HOME` "from nothing": `env_clear`, then only the sandbox
`bin` and `home`. `Sandbox::run_make(&MakeInvocation)` runs
`make --no-print-directory -f Makefile BUILD_TOOLS_PREFIX=<prefix> <target>`
inside that environment, where `<prefix>` is empty and the test populates
`<prefix>/bin` with fakes. `Sandbox::write_mold(dir, version)` writes a fake
`mold` that echoes `mold <version> (compatible with GNU ld)`, which is the
format `installed_mold_version` parses. Pins are read from the committed files
by `sandbox/pins.rs`.

**The two failing tests.** `tests/makefile_test_target/rustdocflags.rs` holds
`behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags` and
`property_doctest_preserves_rustdoc_override_values`. Both run the `doctest`
target through `DoctestHarness`, which supplies the build-tools prerequisite
from fakes instead of crossing it: the harness seeds a `Sandbox` with
`Sandbox::write_mold` and `Sandbox::write_rustup` at the committed pins, adds a
fake `CARGO`, and calls `Sandbox::run_make`, so the recipe reaches the fake
Cargo however the host is provisioned. That boundary is what this branch's
change introduced; before it, both tests ran `make` against the ambient host.
`CARGO` is a `#!/bin/sh` script that records `RUSTDOCFLAGS`, whether
`RUSTDOC_FLAGS` is present, and `"$*"`, tab-separated, into
`$RUSTDOC_ENVIRONMENT_LOG`; the harness asserts on those three fields.

Terms used in this plan:

- **baseline** — the unmutated build-and-test run cargo-mutants performs before
  mutating, which establishes the expected green result.
- **mutant** — a source-level mutation cargo-mutants applies to test whether the
  suite notices; a mutated tree that still passes is a *survivor*.
- **preflight** — `check-build-tools`, which runs before a target's recipe.
- **hermetic** — a test that depends only on what it created, never on the host
  machine's installed tools.

## Conformance basis

This repository has no Terms of Reference or technical-design revision for
developer-tooling CI repairs; the governing artefacts are the repository's own
checked-in contracts and the upstream shared workflow.

- **Upstream requirement, provisioning:** `leynos/shared-actions` issue #591
  (shared prevention and diagnostic contract) and #590 (result completeness).
  These are cross-repository and are *not* discharged by this change; the
  caller change is this repository's contribution to #591.
- **Upstream open question:** `leynos/shared-actions` issue #544 (linker-flag
  ownership when `setup-rust` exports `RUSTFLAGS`). Traced to EP-M3's
  linker-selection observation. Deliberately left open.
- **Repository contract, `cmd`:** `AGENTS.md` "The build standard", which
  states that changing one `rustflags` source without the other fails a
  contract test, and that the release and coverage shapes must stay excluded.
- **Repository contract, `interfaces`:**
  `tests/workflow_contracts/mutation_testing_test.py`, whose `EXPECTED_WITH` is
  documented as "assert the whole block so an added or dropped input fails
  loudly". This plan changes that block deliberately and updates the test in
  the same change.
- **Repository contract, `interfaces`:** `tests/build_tools_check_tests.rs`
  module docstring, which states that the check's purpose is "to turn a missing
  tool into an actionable message instead of an opaque codegen-backend or
  linker failure", and that "none [of the cases] needs mold, rustup, or a
  network". The new `doctest` cases extend the parameterized table that
  discharges this.
- **Trace chain:**
  `shared-actions#591 → EP-M1 → tests/workflow_contracts/mutation_testing_test.py::test_with_block_carries_the_caller_configuration`
  and `shared-actions#544 → EP-M3 → the dispatch log's printed RUSTFLAGS`.

## Milestones

Each milestone ends in a coherent, validated repository state.

### EP-M1 — Provision mold on the worker and pin the provisioning

**Assigned requirements:** `shared-actions#591` (this repository's half).

**End state.** `.github/workflows/mutation-testing.yml` passes
`install-mold: 'true'` and a `setup-commands` value that runs
`make check-build-tools` and prints the two flag variables. Its header comment
states that mold is provisioned by the shared installer and checked by the
repository preflight. `tests/workflow_contracts/mutation_testing_test.py`
asserts both, and its `EXPECTED_WITH` matches the caller exactly.

**Red.** Add the `install-mold` and `setup-commands` entries to `EXPECTED_WITH`
and add the typed tests *first*, then run `make test-workflow-contracts`.
Expect failure: the caller does not yet pass either input. The failure must
name the missing input, not merely differ in `repr`.

**Green.** Edit the caller. Re-run the suite.

**Exact assertions for the contract test:**

- `test_with_block_carries_the_caller_configuration` gains the two keys in
  `EXPECTED_WITH`; its docstring and failure message mention mold provisioning.
- A new standalone test asserts the *type* of `install-mold` is the string
  `"true"` (not a YAML boolean) and that `setup-commands` invokes
  `make check-build-tools`. Each assertion carries a message.
- If the caller passes a mold version, a further test reads
  `tools/mold/VERSION` and asserts equality. **Decision:** the caller does
  **not** pass one, because the pinned shared revision declares no
  `mold-version` input. The test is therefore not added; the plan says so
  explicitly rather than leaving a gap. The repository pin is still verified, by
  `make check-build-tools` at run time.
- The SHA-format, permission, concurrency, and trigger tests are unchanged.

**Conformance check.** Does the caller still pin its `uses:` reference to
`leynos/shared-actions/.github/workflows/mutation-cargo.yml` at a full
40-character commit SHA, keep `permissions: {}` at the top level and exactly
`contents: read` + `id-token: write` on the job, and keep
`concurrency.group: mutation-testing-${{ github.ref }}` and
`cancel-in-progress: false`? All four tests must still pass.

**Acceptance evidence:** the full output of `make test-workflow-contracts`,
green, with the new tests named in the run.

**Recovery:** revert the two files; nothing depends on them yet.

**Remaining gaps:** at M1 close no worker had run this. **Closed at EP-M3:** run
`38323348294` shows `Run setup commands` moving from `skipped` to `success`
and logging `build-tools: mold 2.41.0`.

### EP-M2 — Make the rustdoc tests hermetic; keep real-preflight coverage

**Assigned requirements:** the `interfaces` contract in
`tests/build_tools_check_tests.rs`; the issue's "Decide explicitly whether
rustdoc-override tests intend to exercise real host-tool validation or should
inject that boundary alongside fake Cargo".

**End state.** Both rustdoc tests pass with no `mold` and no `rustup` anywhere
on the host. `doctest` is one of the parameterized cases in
`tests/build_tools_check_tests.rs`, asserting that a missing or drifting mold
stops `make doctest` before the fake Cargo runs.

**Red.** Under a mold-free `PATH`, the two rustdoc tests fail at
`check-build-tools`. Build the probe first, then verify it before trusting it:

```sh
build-nomold-path() {
  dest="$1"; rm -rf "$dest"; mkdir -p "$dest"
  for dir in /usr/local/bin /usr/bin /usr/sbin /bin /sbin; do
    [ -d "$dir" ] || continue
    for entry in "$dir"/*; do
      name="$(basename "$entry")"
      [ "$name" = mold ] && continue
      [ -e "$dest/$name" ] && continue
      ln -s "$entry" "$dest/$name"
    done
  done
}
build-nomold-path /tmp/nomold/bin
PATH=/tmp/nomold/bin command -v mold  || echo "mold: NONE (expected)"
PATH=/tmp/nomold/bin command -v make  || echo "make MISSING (probe is broken)"
PATH=/tmp/nomold/bin command -v grep  || echo "grep MISSING (probe is broken)"
```

Only once those say `mold: NONE` and show real paths for `make` and `grep` is
the probe meaningful. The real preflight then reproduces CI exactly:

```sh
PATH=/tmp/nomold/bin HOME=/tmp/nomold/home make check-build-tools
# build-tools: mold not found on PATH (pinned 2.41.0)
# make: *** [Makefile:476: check-build-tools] Error 1   (exit 2)
```

After the change, the two rustdoc tests pass under that same probe. The test
*binary* still needs `mold` to link, so build normally and run it under the
probe — the compiled binary's `PATH` is what the test passes to its children:

```sh
cargo nextest run --test makefile_test_target --all-features -E 'test(rustdocflags)'
PATH=/tmp/nomold/bin "$(ls -t target/debug/deps/makefile_test_target-* | grep -v '\.d$' | head -1)" \
  rustdocflags
```

Note the apparent contradiction: the build itself uses
`-Clink-arg=-fuse-ld=mold`, so it needs a real `mold`; the *test* run does not.
That is precisely the CI worker's situation inverted — and it proves the tests
no longer depend on `mold` being resolvable, because they never invoke a real
compiler.

`NETSUKE_REQUIRE_NINJA` is *not* needed here; these tests do not use Ninja.

**Green.** Rewrite the helper to use the sandbox.

**Shape of the change in `tests/makefile_test_target/rustdocflags.rs`:**

- Delete the direct `Command::new("make")` construction from
  `run_doctest_with_override`; take `&Sandbox` instead of a `make` command.
- The `Sandbox` is constructed once per test with `Sandbox::new()`, then
  `write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)` and
  `write_rustup(&pinned_toolchain()?)?`, following
  `build_tools_check_tests.rs::healthy_sandbox`.
- `write_fake_cargo` moves the fake Cargo into the sandbox `bin` (so `doctest`
  can find it) **and** the `CARGO` Make variable still names it explicitly, so
  the recipe's `${CARGO}` use is unchanged.
- `RUSTDOC_ENVIRONMENT_LOG` is passed through `MakeInvocation::environment`
  (the fake Cargo script reads it directly from the environment; it is not a
  Make variable).
- The `log` path stays inside the sandbox home, so repeated runs cannot collide.
- The command-line-override case uses
  `MakeInvocation::variable("RUSTDOC_FLAGS", value)`; the environment case uses
  `MakeInvocation::environment(…)`. Both routes must remain, because the
  distinction between them is the point of the test.
- On failure the message includes the full combined stderr/stdout from
  `combined(&output)`, so a future failure is diagnosable without re-running.
- `assert_doctest_record` is unchanged. Its assertions — exact
  `RUSTDOCFLAGS`, `RUSTDOC_FLAGS` absent, and the doctest argument list — are
  the test's subject and must not be weakened.
- The module docstring gains a short paragraph stating that these tests inject
  the build-tools boundary, and that `build_tools_check_tests.rs` and
  `build_tools_pin_tests.rs` own real-preflight validation.

For `property_doctest_preserves_rustdoc_override_values`, one sandbox is built
per proptest case (the test body already creates its temporary directory per
case, so the shape is unchanged) and the environment log is truncated by the
fake Cargo's `>` redirection on every invocation, so the 16 cases · 2 sources =
32 assertions each read the record just written.

**Shape of the change in `tests/build_tools_check_tests.rs`:**

- `doctest` joins the `#[case(...)]` list on
  `build_targets_stop_when_the_check_fails`, whose body already arranges "no
  mold anywhere, everything else present and recording", runs the target, and
  asserts the check's message appears while the recording Cargo saw nothing.
- No new helper is required, because the existing body is target-agnostic.
- `tests/build_tools_pin_tests.rs` is not changed.

**Verify the allowlist is not touched.**
`test_support/src/build_tools/sandbox/utilities.rs`'s `SANDBOX_UTILITIES` must
not gain `mold`, `rustup`, or `cargo`. The fake Cargo is a shell script using
only `printf` (a shell builtin) and `:`, so no utility is added.

**Conformance check.** Does `make test-nextest` still pass with `mold` present,
and does the compiled test *binary* still pass with `mold` absent from `PATH`?
The first proves nothing regressed on a capable machine; the second proves the
injection works. Both are required.

The mold-free leg cannot be `make test-nextest`: that target declares
`check-build-tools` as a prerequisite (`Makefile:228`), and the preflight
returns non-zero when `command -v mold` fails
(`scripts/check-build-tools.sh:24-25`). Asking the *target* to pass without
mold is asking it to fail, so the leg is run against the already-built binary
instead — exactly as the procedure above does. The binary links at build time,
when mold is present; the test run then passes `PATH=/tmp/nomold/bin` to its
children, which is the property under test.

**Acceptance evidence:** the focused nextest output on a capable machine, the
compiled binary's output under `PATH=/tmp/nomold/bin`, and the
`make test-workflow-contracts` output unchanged from EP-M1.

**Recovery:** `git revert` the M2 commit; `build_tools_check_tests.rs` and the
sandbox module are untouched by it except for the added case.

**Remaining gaps:** at M2 close the worker had still not been observed.
**Closed at EP-M3:** the baseline is green on the worker
(`ok Unmutated baseline in 122s build + 218s test`), and
`tests/makefile_test_target.rs:348` declares `mod rustdocflags`, so the named
tests run inside it.

### EP-M3 — Validate on the real worker, observe linker selection, audit ETXTBSY

**Assigned requirements:** the issue's Acceptance section, and
`shared-actions#544`.

**End state.** A manual dispatch of `mutation-testing.yml` on this branch
shows, in order: `make check-build-tools` reporting the pinned mold, both
rustdoc tests passing, a green baseline, and a non-empty mutant set. The
`RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` printed by `setup-commands` are
recorded in the pull request against `shared-actions#544`.

**Steps.**

1. Run the full local gate set: `make check-fmt`, `make lint`,
   `make doc-coverage`, `make test`, `make markdownlint`,
   `make test-workflow-contracts`, and `make actionlint` (via
   `make github-actions-lint`). Delegate the full set to `scrutineer`.
2. Dispatch the workflow: `gh workflow run mutation-testing.yml --ref <branch>`
   and watch the run.
3. Read the `setup-commands` step output. Record the literal `RUSTFLAGS` and
   `CARGO_ENCODED_RUSTFLAGS` lines.
4. Confirm a non-empty mutant set executed, not merely a green exit.
5. Confirm `concurrency`, `permissions`, and the runner/cache settings are
   byte-identical to `main` except for the two added `with:` keys.

**Linker selection.** Two outcomes are possible and both are recorded, neither
is "fixed locally":

- Only if **both** `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` are *unset* does
  the project configuration select mold and the linker requirement count as
  intact — and even then the record should be confirmed against the compiler
  arguments, not the environment. An `RUSTFLAGS` that is set but **empty** is
  *not* the same as an unset one: Cargo selects it ahead of every `rustflags`
  table, so an empty value still displaces `.cargo/config.toml` and drops
  `-Clink-arg=-fuse-ld=mold`. Measured on this host against a two-line test
  crate: unset produced one `-Clink-arg=-fuse-ld=mold` on the `rustc`
  command-line, an empty-but-set value produced zero. `setup-commands` prints
  both variables with `<unset>` standing in for an unset one, which is why the
  print distinguishes the cases rather than reporting an empty value.
- If the shared workflow's `setup-rust` export displaces the project table, the
  worker will link with the platform linker. Add **no** local workaround; link
  the finding to `shared-actions#544` and pin a fixing shared revision only if
  one exists.

**ETXTBSY audit (read-only unless it reproduces).** The 18 September job
(105527415501) reported `Text file busy` when launching a temporary Ninja
fixture. The audit traced every writer on that path and found no defect:

- `test_support/src/exec.rs::write_exec_with_content` calls
  `crate::fs::write(&path, content)` and then `make_executable(&path)`.
- `test_support/src/fs.rs::write` is `std::fs::write`, which opens, writes and
  closes before returning; its signature is `io::Result<()>`, so it has no way
  to hand a `File` back to its caller. The handle provably cannot outlive the
  call, let alone a later spawn.
- `make_executable` → `set_mode` uses `fs::metadata` plus
  `fs::set_permissions`, neither of which opens the file for writing. A
  permission change is not a write, so it cannot hold the file busy either.
- The two fake-Ninja writers that reach this primitive —
  `test_support/src/lib.rs::write_fake_ninja` (writes `ninja`, or `ninja.cmd`
  on Windows) and
  `test_support/src/check_ninja/mod.rs:: write_fake_ninja_script_in_dir` (writes
  `ninja`) — each return a `TempDir` *and* the path, so the directory outlives
  the write and is kept alive by the caller across the spawn. That is the
  correct ownership shape.
- `git log -S 'write_fake_ninja'` and `git log -S 'write_exec_with_content'`
  place both helpers before the September run, and `git show` of
  `test_support/src/exec.rs` and `test_support/src/fs.rs` at `1e60fb18` (the 18
  September revision) shows the write path byte-identical to today's. There is
  no earlier revision in which a handle escaped.
- `git log --all -i --grep='ETXTBSY\|text file busy'` returns nothing: the
  repository has never recorded a fix for this, consistent with it being
  environmental rather than a fixture defect.

Conclusion: **no reproduction and no defect — no helper is changed, and no
retry is added.** The observed `Text file busy` is consistent with an external
writer still holding the file open (an antivirus/indexer, or a container
overlay mid-copy), not with this repository's write path. It remains distinct
from the demonstrated missing-mold failure, as the issue states.

**Acceptance evidence:** the run URL; the `check-build-tools` line; the
`ok Unmutated baseline` line for the green shards; the named rustdoc test
results **from the focused local run**, not from the worker's log, because the
four green shards' per-target output is not in hand; the mutant count; and the
printed flag values. The worker's contribution to "the named tests pass" is
cargo's first-failure behaviour plus the 3 Oct ordering, as `Progress` states —
the direct per-test evidence is the local run.

**Recovery:** the workflow edit is reverted by a commit; the dispatch is
informational and changes no state.

### EP-M4 — Finalize, commit, push, open the draft pull request

Update `Progress`, `Outcomes & retrospective`, and the `Status:` header; re-run
`make check-fmt` and `make markdownlint` because the plan itself is Markdown.
Commit in small units: (1) the ExecPlan, (2) the caller plus its contract test,
(3) the rustdoc sandboxing plus the `doctest` cases, (4) any documentation.
Push over SSH, then open a draft pull request titled
`Fix mutation baseline mold provisioning and rustdoc test boundary (#875)` with
`Closes #875` and a `## References` section carrying
`https://lody.ai/leynos/sessions/4b6172f4-962c-41b6-a91d-7caa693b2a36`.

## Verification plan

### Invariants

- **I1 — The caller never sets a compiler-flag variable.** No `RUSTFLAGS` or
  `CARGO_ENCODED_RUSTFLAGS` key at any scope in `mutation-testing.yml`, not
  merely on the job. Verified by `_flag_override_problems`, which walks the
  entire parsed workflow (mappings and sequences, at any depth) and reports
  every offending dotted path;
  `test_no_compiler_flag_variable_is_set_anywhere_in_the_caller` asserts the
  walk returns empty, and the existing whole-block `with:` equality still pins
  the shipped configuration. The walk is deliberately wider than the original
  `"env" not in job` sketch: a future step, job, or reusable-workflow argument
  could introduce the override where a job-scoped predicate would never look.
  *Non-vacuity:* two controls: a permanent in-suite test and a recorded
  out-of-process run. In-suite, `test_flag_guard_rejects_a_seeded_override`
  deep-copies the parsed workflow, seeds
  `job["env"] = {"RUSTFLAGS": "-D warnings"}`, asserts the predicate rejects
  it, and asserts the unmodified caller still passes — so the control stays
  runnable rather than being a one-off edit. Out of process, against a scratch
  copy of the real workflow, the clean tree yielded `[]` and the seeded copy
  yielded `['workflow.jobs.mutation.env.RUSTFLAGS (a RUSTFLAGS key)']`.
- **I2 — The rustdoc tests reach the fake Cargo on a mold-free host.** Verified
  by running the focused suite with `PATH` stripped of any real `mold`, and by
  the positive control of the same suite on a `PATH` that has one.
  *Non-vacuity:* the mold-free run must **fail before** the change and **pass
  after**. The pre-change failure was observed (see `Progress`); the
  post-change pass is the discharge.
- **I3 — The `doctest` prerequisite relation is still tested.** Verified by the
  `doctest` case in `build_targets_stop_when_the_check_fails`, which asserts
  both the diagnostic and that the recording Cargo saw zero invocations.
  *Non-vacuity:* remove `check-build-tools` from `doctest`'s prerequisites in a
  scratch copy and confirm the failure the case asserts; restore. The control
  drives the real `Makefile` target the case exercises rather than the Rust
  fixture itself, because the fixture only ever passes a target name to `make`;
  both directions, the exact diagnostic, and all three failed attempts at a
  faithful `PATH` before the fourth succeeded are recorded under `Progress`.
- **I4 — The sandbox allowlist is unchanged.** Verified by the existing
  `check-fmt`/`lint` gates plus an explicit diff inspection of `utilities.rs`.
- **I5 — The two rustdoc assertions still carry their full strength.** The
  exact-`RUSTDOCFLAGS`, absent-`RUSTDOC_FLAGS`, and argument-list assertions in
  `assert_doctest_record` are byte-for-byte unchanged.

### Lemmas and axioms

- **Lemma L1:** `Sandbox::write_mold` produces output that
  `installed_mold_version` parses to the requested version. *Method:* already
  covered by `build_tools_check_tests.rs`, which relies on the same property
  through the real script. Treated as an axiom here, with the contract-level
  evidence cited rather than re-derived.
- **Axiom A1:** GitHub's `workflow_call` rejects a boolean for a `string`
  input. *Method:* not directly testable from this repository; mitigated by
  passing the documented string form and by the contract test pinning the
  *type*.
- **Axiom A2:** `setup-commands` runs before the baseline in each mutants job.
  *Method:* read from the pinned workflow's job steps during reconnaissance;
  re-checked at EP-M3 from the run log itself, where the `setup-commands`
  output appears before the baseline compile.
- **Axiom A3:** `shared-actions#544` (whether `setup-rust`'s `RUSTFLAGS` export
  displaces the project's linker flag) is an upstream question this repository
  cannot settle. *Method:* the printed flag values at EP-M3 are recorded as
  evidence for that issue rather than resolved locally. The axiom's premise was
  measured on 2026-10-18 rather than assumed: an exported `RUSTFLAGS` does
  displace a config-declared `rustflags` table, and does not displace a
  config-declared `linker` key. The measured part is recorded in *Surprises &
  discoveries*; the remaining upstream part, which change makes the workflow
  stop exporting, stays an axiom.

### No new formal obligations

This change introduces no invariant over generated inputs and no contractual
business logic. The correct levels on the `rust-router` testing hierarchy are
the ones already chosen: a parameterized table for the finite set of failing
targets (EP-M2), and behavioural assertions over a parsed configuration for the
caller (EP-M1). No `proptest`, `kani`, or `verus` obligation is warranted, and
saying so explicitly is the honest discharge. The existing `proptest` in
`property_doctest_preserves_rustdoc_override_values` remains, with a 16-case
bound, because its subject (order-independence and byte preservation of a
generated flag string) genuinely spans inputs.

### Evidence to capture

- `make test-workflow-contracts` output (EP-M1), showing the new tests.
- Focused nextest output for `makefile_test_target` under a mold-free `PATH`
  (EP-M2), showing the 2 rustdoc tests passing and no `check-build-tools`
  diagnostic.
- Focused nextest output for `build_tools_check_tests` with the new `doctest`
  case (EP-M2).
- The `scrutineer` gate bundle (EP-M3).
- The run URL, `check-build-tools` line, baseline result, mutant count, and
  printed flag values (EP-M3).

## Idempotence and recovery

Every step is re-runnable. The workflow edit is declarative. The Rust test
edits are pure test code with no side effects outside their temporary
sandboxes. The local evidence runs create only `target/` and `~/.cache` state.
The `PATH`-restricted runs use a sub-shell `PATH` assignment, so they cannot
leak into the caller's environment. If a focused run is interrupted, re-running
it is safe. Nothing on the path moves an installed tool: the mold-free runs use
a mirrored `PATH` built from symlinks, so the host's `mold` is never touched
and the build still links.

## Interfaces and dependencies

No public interface changes. No new dependencies. The Rust change reuses the
already-public `test_support::build_tools` surface (`Sandbox`, `MakeInvocation`,
`combined`, `pinned_mold_version`, `pinned_toolchain`); the Python change
reuses `workflow_loading`'s `load_workflow`, `require_mapping`, and the
module's existing `_mutation_job`. `RecordingCargo` is deliberately **not**
used in the rustdoc tests: the existing bespoke fake Cargo records exactly the
three fields the assertions need, whereas `RecordingCargo` records a different,
larger schema.

## Revision note

Initial draft, 2026-10-12. Established by reconnaissance that (a) the failure
reproduces locally under a mold-free `PATH` and (b) the existing pin already
exposes `install-mold`, so no pin bump is required — contrary to the issue's
conditional. Added `Axiom A3` and the linker-selection observation after
reading the shared workflow's own guide, which states that a mold flag in
`.cargo/config.toml` does not apply when `setup-rust` exports `RUSTFLAGS`.

Revisions, 2026-10-13. Status moved from `DRAFT` to `IN PROGRESS` once the
implementation milestones were committed. Corrected the run census in
`Purpose / big picture` from "twenty-nine runs, six failures" to **30 runs,
seven failures** — the earlier figure listed seven SHAs against a smaller
total, so it was internally inconsistent as well as wrong. Added the
`make test-workflow-contracts` pass and the `os error 11` repetition to
`Progress`, and two `Surprises & discoveries` entries: that a spawn-side
admission failure is a statement about the host rather than the branch, and that
`##[error]` lines in a job log are frequently an action echoing its own
script, so a job's cause must be read from its step conclusions. Canonicalized
the Markdown through `mdtablefix --in-place` after CI's `make check-fmt`
rejected the hand-wrapped prose, and fenced the runner's shutdown message as
`plaintext` so its verbatim United States spelling stays out of the prose
spell-check. The first dispatched mutation run of this revision reused the
existing shared-workflow pin: the pinned revision already declares both
`install-mold` and `setup-commands`, so the issue's conditional pin bump was
not required.

Revisions, 2026-10-14. Recorded the CodeRabbit tally — later re-derived to the
17-invocation census in *Outcomes & retrospective* — and the 0-and-2 sampling
measurement over the frozen revision `52084776`; fixed the M2 conformance check
to name the caller's real `uses:` target; and extended the CI-green record to
`009cbf02`, whose only delta from `52084776` is this document and which is
therefore gated by the three Markdown-sensitive gates plus
`test-workflow-contracts`.

Revisions, 2026-10-14 (final). Consolidated the plan onto its instructed end
state. Milestone 4 is closed: the draft pull request #911 is the deliverable,
CI is green on `8c0490eb` and on the later docs-only heads, and the review
outcome is a disposition per finding rather than a clean count. Corrected a
Milestone 4 line that named "PR marked ready" as a gate — the brief asked for a
*draft* pull request, and a ready one would contradict this plan's own recorded
gap — and the three sentences elsewhere that had inherited the same assumption.

This section deliberately does not say the plan stops editing here, because
that sentence is falsified by whatever commit carries it. The operative
statement is narrower and stays true: every head after `8c0490eb` *up to and
including `b9724625`* changes this document and nothing else, so each is gated
by the three Markdown-sensitive targets plus `test-workflow-contracts`, with
`lint`, `typecheck` and `doc-coverage` carried by the byte-identical
non-Markdown tree of `52084776`, and `test` resting on CI. A reader who needs
the gate evidence for a specific head should read it from the pull request
description, which states that invariant rather than a SHA.

Revisions, 2026-10-16. Two changes, both prompted by the eleventh CodeRabbit
pass rather than by a gate. First, the eleven-times-raised mold finding is now
joined by a re-derivation of the review census after that pass (17 invocations,
23 finding records, still eleven distinct defects). Second — and this is the
first non-Markdown delta since `52084776` — the upstream-state claims in
`.github/workflows/mutation-testing.yml` were corrected. They had gone stale:
the comment named `aea9c14c24b7` as the upstream tip, which is no longer true,
and asserted that #544 "has no fixing revision", which shared-actions#628 now
falsifies. The replacement names only the pinned revision, states the claim as
"no merged revision declares `rustflags`", cites #628 with its `BLOCKED` state,
and tells the next reader to re-check it. Behaviour is unchanged: the edit is
confined to YAML comments, and the `uses:` pin, the `with:` block and
`setup-commands` bytes are identical. Because this head is not docs-only, the
full gate set is required rather than the four-target subset.

Revisions, 2026-10-16 (correction). A provenance audit against the retained
gate logs falsified a label this plan had been using, and it is corrected above
rather than left standing. The plan called `52084776` "the eight-gate
revision". It was not: the all-green eight-gate run is bound to `3d761c8d`, by
the `HEAD_AFTER` field its `rc` artefacts carry, and `3d761c8d → 52084776` is
not Markdown-only — it changes
`tests/workflow_contracts/mutation_testing_test.py`. The second full batch, run
10-14 01:35–02:18 after `52084776` landed at 01:28, is the `-final` log set,
and it is green on three of the four Rust-bearing gates: `lint`, `typecheck` and
`doc-coverage` each end clean. `make test` in that batch does not — it ends
`make: *** [Makefile:229: test-nextest] Error 100` on the same
`TIMEOUT [300.008s]` the `2026-10-14 03:10Z` entry diagnoses. So four passages
that said the Rust-bearing gates were "covered by their green result on
`52084776`" now say three are, with `test` resting on CI, which is what the
plan's own timeout entry already implied. The verdicts are unchanged; the label
under them was wrong.

The methodological finding is worth more than the correction: **the `-final`
suffix names a review round, not a revision.** Two different batches both used
it, `gate-check-fmt-issue875-final.out` from 10-14 04:04 and
`gate-lint-issue875-final.out` from 10-14 01:35, so a log's binding has to be
derived from its timestamps against the commit times — or from an embedded
`HEAD` marker — rather than read off its name. Checking a claim against the
artefact it cites is what caught this; the claim had been re-asserted across
several entries on the strength of having been written down.

Revisions, 2026-10-18. Corrected the plan's account of the linker clause on two
points, both measured rather than reasoned. First, the exported `RUSTFLAGS`
comes from the downstream `setup-rust-toolchain` action's `GITHUB_ENV` append,
not from `setup-rust`'s own description; the earlier wording pointed one layer
too high and is corrected in `Progress`. Second, shared-actions#628 is **not**
the remedy this plan had been carrying: it supplies a child-only
flag-replacement mechanism. Its replacement value *can* name
`-Clink-arg=-fuse-ld=mold`, but it does not preserve configuration-owned flags
and does not, by itself, resolve the selection criterion, and caller-owned
replacement is forbidden here. A three-configuration compile measurement
established that an exported `RUSTFLAGS` displaces a config-declared
`rustflags` table but leaves a config-declared `linker` key intact, which makes
the `linker` key the only surviving in-repository route and makes it a design
change rather than a configuration tweak. The retraction is recorded in
`Outcomes & retrospective`; the measurement and the #628 reading are in
`Surprises & discoveries`; Axiom A3 now records the measured half separately
from the part that remains upstream.

Same day, later: review answered the boundary question the plan had left open.
Repository-owned `linker` selection is valid in principle; it must be delivered
as a separate build-policy change under #875 rather than inside #911, because
the release, packaging and coverage exclusions depend on an assigned
`RUSTFLAGS` displacing the configured table; macOS and Windows are unchanged;
and the upstream-only reading is rejected, so #544 remains the broader
flag-ownership defect rather than the criterion's sole home. The plan records
that disposition in its header, `Outcomes & retrospective` and the workflow
comment, and does not adopt the wrapper here.

Also recorded in this revision, and live rather than historical: four scheduled
mutation runs on `main` between 13 and 17 October failed with the issue's exact
mold diagnostic (`install-mold: false`, empty `setup-commands`, non-empty
change sets), while two others — 15 and 18 October — concluded `success` having
evaluated zero mutants because the detection window matched nothing. The former
says the defect is current; the latter says a green badge on that workflow is
not by itself evidence of a measured run.

The status line stays `COMPLETE` for the plan and the acceptance clause stays
open for the issue: nothing in this revision changes the code, and the boundary
question — whether a repository-owned `linker` change discharges "required
linker selection remains intact", and whether it may ride in this pull request
— is put to CodeRabbit on #911 and is unanswered as of this entry.
