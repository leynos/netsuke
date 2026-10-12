# Mutation baseline reaches real build-tool preflight without the required pinned mold (issue #875)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

The scheduled mutation-testing run has failed its *baseline* build for at least
two weeks, so no mutant has been evaluated on this repository since. The
baseline is the unmutated `make test` run the worker performs before it mutates
anything; when it fails, cargo-mutants reports no mutant results at all.

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
   A reader can reproduce this: move `$HOME/.local/bin/mold` aside, run
   `make test-nextest` restricted to the `makefile_test_target` binary, and see
   the rustdoc tests pass while `build_tools_check_tests` still exercises the
   real preflight.
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

## Progress

Every stopping point is documented here. Timestamps are UTC.

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
  binding, and an unformatted expression at the same site. Both were mine;
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
  still not ours to remove. Load average remains ~70 on 6 cores, so gates will
  be slow; the sampled disk log is retained so an ENOSPC failure stays
  distinguishable from a real one.
- [x] (2026-10-12 20:50Z) **The worker's `setup-commands` text is read, and
  the #544 observation is recorded.** `gh api
  repos/leynos/netsuke/actions/jobs/<id>/logs --allow-escape-sequences`
  downloads a *completed* job's log even while its run is still in progress;
  only the `gh run view --job` form waits for the whole run. Downloaded all
  four cancelled shards. Every one prints, immediately after
  `build-tools: mold 2.41.0 at
  /opt/hostedtoolcache/mold/2.41.0-a3696680d99e692970590a178bc3a33d78d60d1c6dc9db7a11b557b02b751f5d/x86_64/bin/mold`
  and `build-tools: toolchain nightly-2026-08-23 available`:

  ```plaintext
  RUSTFLAGS=-D warnings
  CARGO_ENCODED_RUSTFLAGS=<unset>
  ```

  So the linker-selection half of the risk is **confirmed, not resolved**:
  `RUSTFLAGS` is exported with a value, and `CARGO_ENCODED_RUSTFLAGS` is unset.
  Since an assigned `RUSTFLAGS` replaces every `rustflags` table in
  `.cargo/config.toml`, the `-Clink-arg=-fuse-ld=mold` entry cannot apply.
  This is exactly
  <https://github.com/leynos/shared-actions/issues/544> ("Installed mold does
  not imply Cargo actually selects it if `RUSTFLAGS` has replaced project
  flags"). Per the plan's constraint, **no local workaround was added** and no
  pin bump is made, because #544 has no fixing revision. Milestone 3's
  observation obligation is now discharged with a literal transcript.
- [x] (2026-10-12 20:50Z) **Acceptance criteria are met on the real worker.**
  The four shards that were not killed ran the *unmodified* baseline to
  completion and every one reported it green:

  ```plaintext
  Found 636 mutants to test
   INFO Auto-set test timeout to 580s
  ok      Unmutated baseline in 98s build + 193s test
  ```

  (the other three: `120s build + 213s test`, `107s build + 177s test`,
  `95s build + 177s test`). Because a `cargo test` baseline runs *every* test
  target and only stops at the first failing one, an `ok` line means the
  `makefile_test_target` binary ran and passed — which is the two named
  rustdoc tests, the pair that failed on 3 Oct. The same shards then evaluated
  mutants and reported real outcomes (`MISSED`, `TIMEOUT`) with per-mutant
  timings, so **a nonempty set of mutants executes**. All six shards also show
  step 7 `Run setup commands = success` where the failing run had it
  `skipped`, and the runner annotations carry
  `notice: mold 2.41.0 installed for x86_64 (tool cache: miss)`.
- [x] (2026-10-12 20:50Z) **The four `cancelled` shards are an infrastructure
  event, not a defect.** Each ends with
  `##[error]The runner has received a shutdown signal. This can happen when
  the runner service is stopped, or a manually started runner is canceled.`
  after 57–86 minutes of legitimate work, at staggered times (17:51, 17:53,
  17:58, 18:20 UTC). The caller sets `cancel-in-progress: false` and no other
  run of this workflow exists for this ref, so this is GitHub reclaiming
  runners, not a concurrency decision. Recorded because the job conclusions
  read `failure` and would otherwise look like the change failing.
- [x] (2026-10-12 20:50Z) **The one genuine failure is the unrelated flake,
  now measured.** Shard 2 exited `4` ("baseline tests failing") in 3 minutes on
  `runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`
  with `Error: a structural summary must emit exactly one trace event: []`.
  Four of the five completed shards ran that same lib suite green, so it is
  nondeterministic. It is the out-of-scope failure already logged below, and
  the branch does not touch that test or its helper.
- [ ] Milestone 3 remainder: `make test` and `make test-workflow-contracts` at
  the current revision.
- [ ] Milestone 4: ExecPlan finalized, commit, push, draft PR.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.

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
  `Detect changed files = completed/success`; it never decides to skip, so the
  skip originates outside the caller's job table (the pinned shared workflow,
  or a `paths` filter on the caller). The two runs in the same window that
  *did* launch shards both went **red**: `b6e7cf50` (3 Oct) and `bc812986` (12
  Oct). The relation is therefore green ⇔ shards skipped; shards run ⇒ red.

  Impact, recorded because it changes what "the documented missing-mold
  failure" is evidence *for*: the mold diagnostic is real and this plan
  reproduces it, but it is **not** the reason this repository has evaluated no
  mutants for at least a week. Confirming it requires artefact inspection, not
  the run conclusion. The acceptance criterion "a non-empty mutant set
  executes" cannot be discharged by any green run that predates this branch,
  because none of them executed a mutant. See also the next observation, which
  shows the same artefact-inspection discipline applied to the 3 Oct failure.

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
  after this merges. That is escalated rather than silently widened into scope:
  the Worker dispatch section states what to look for.

  Two smaller inferences recorded because they are traps. First, the 3 Oct
  shard used a *different* pin (`ff1dd759…`, not the current `0606ca89…`) and
  its `mutants` job name carried a large explicit source-file list rather than
  the `0..5` shard indices, so the two runs are not the same regime. Second,
  the step rows are not trustworthy evidence that a phase ran: on 12 Oct, step
  12 `Upload mutation report = completed/success` while step 11 was `failure`,
  and on 3 Oct the same pattern holds — the artefact is what proved the phases'
  outcomes, and the step table alone would not have.

  Retracted, same session: I first wrote here that
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
  install without any ordering guard. Rationale: in the pinned revision the
  mutants job installs mold inside "Setup Rust" (the `setup-rust` action is
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

To be completed at Milestone 4. The comparison is against
`Purpose / big picture`, and no discovery may remain unreconciled with
`Conformance basis`.

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
one job, `mutation`, whose `with:` block currently carries `exclude-globs` and
`extra-args: "--all-features"`. The default `setup-rust` on the shared worker
exports `RUSTFLAGS="-D warnings"`; the caller must not fight that.

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
`property_doctest_preserves_rustdoc_override_values`. Both call
`run_doctest_with_override`, which today runs `make` directly with the ambient
environment plus `CARGO` and `RUSTDOC_ENVIRONMENT_LOG`. `CARGO` is a
`#!/bin/sh` script that records `RUSTDOCFLAGS`, whether `RUSTDOC_FLAGS` is
present, and `"$*"`, tab-separated, into `$RUSTDOC_ENVIRONMENT_LOG`.
`assert_doctest_record` then asserts on those three fields.

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

**Conformance check.** Does the caller still pin the `.github/actions/...`
shape, keep `permissions: {}` at the top level and exactly `contents: read` +
`id-token: write` on the job, and keep
`concurrency.group: mutation-testing-${{ github.ref }}` and
`cancel-in-progress: false`? All four tests must still pass.

**Acceptance evidence:** the full output of `make test-workflow-contracts`,
green, with the new tests named in the run.

**Recovery:** revert the two files; nothing depends on them yet.

**Remaining gaps:** the worker has not actually been observed running this.

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

**Conformance check.** Does `make test-nextest` still pass with `mold` present
*and* with it absent? The first proves nothing regressed on a capable machine;
the second proves the injection works. Both are required.

**Acceptance evidence:** the focused nextest output under both `PATH`s, and the
`make test-workflow-contracts` output unchanged from EP-M1.

**Recovery:** `git revert` the M2 commit; `build_tools_check_tests.rs` and the
sandbox module are untouched by it except for the added case.

**Remaining gaps:** the worker has still not been observed.

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

- If `RUSTFLAGS` is empty or already contains `-Clink-arg=-fuse-ld=mold`, the
  project configuration selects mold and the linker requirement is intact.
  Record that as the observation.
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

**Acceptance evidence:** the run URL, the `check-build-tools` line, the two
test names in the baseline output, the mutant count, and the printed flag
values.

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

- **I1 — The caller never sets `RUSTFLAGS`.** No `env:` block, no `RUSTFLAGS`
  key, anywhere in `mutation-testing.yml`. Verified by a contract assertion
  over the parsed job mapping (`"env" not in job`), plus the existing
  whole-block `with:` equality. *Non-vacuity:* a seeded edit adding
  `env: {RUSTFLAGS: ""}` to the job must fail that assertion. Run it once as a
  negative control.
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
  scratch copy and confirm the case fails; restore.
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
  evidence for that issue rather than resolved locally.

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
it is safe. The only destructive-sounding step, "move `mold` aside", is
implemented as a `PATH` restriction and never moves a file.

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
