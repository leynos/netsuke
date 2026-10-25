# Repair concurrent manifest telemetry capture

This ExecPlan is a living document. Its `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, and `Verification plan` sections
must be updated as evidence changes.

Status: IN PROGRESS

Implementation is authorized by the detailed scope and coding plan in issue
[#927](https://github.com/leynos/netsuke/issues/927).

## Purpose / big picture

The manifest structure telemetry test must keep observing exactly one bounded
summary under concurrent libtest execution. The work identifies why a callsite
can cache `Interest::never()` after a stale `Rebuilder::JustOne` observation,
proves the schedule with a bounded Loom model and a fresh-process reproduction,
and prevents new stale-short-path selections in test processes. The exact
cardinality oracle must reject zero and duplicate summaries, including a real
seeded duplicate.

## Constraints

- Keep the production telemetry path and dependency resolution unchanged unless
  a released `tracing-core` fix exists.
- Do not initialize a process-wide dispatcher from reusable runner logic or
  test helpers; the existing production initialization belongs to `main`. Do
  not serialize tests, add retries, or weaken the summary assertion.
- Keep the thread-local subscriber caveat accurate. The diagnosis must show
  whether it explains this failure instead of assuming that it does.
- Keep the upstream `which` flake as a separate observation unless post-fix
  evidence establishes a shared mechanism.
- Use a fresh child process for the gated schedule. Pass its mode through the
  child environment only, enforce a deadline, and report child output on error.
- Preserve `test_support::tracing_capture` callback behaviour and current helper
  signatures. Its permanent anchor dispatches must never become defaults.
- Keep `.config/nextest.toml` retry and concurrency policy unchanged.
- Use the repository's Make targets and run gates sequentially for this
  worktree. Never bypass Cargo compile admission or create a private Cargo
  cache.
- Keep each Rust source file below 400 lines and document every new Rust
  function and module as required by `AGENTS.md`.

## Tolerances (exception triggers)

- Scope is limited to the capture helpers, manifest telemetry tests, the
  isolated Loom model and worker, the requested documentation, and the root
  Loom development dependency.
- If the test-support crate cannot be imported from root library tests, keep
  the anchor implementation private to the root crate and record that reuse
  boundary in the ADR.
- If a released `tracing-core` version later than 0.1.36 changes the `JustOne`
  path, test the unmitigated canary before deciding whether anchors remain.
- If a gate, the model, or either worker mode fails for a reason other than its
  expected red control, stop that milestone and diagnose the failure before
  continuing.
- If the upstream report is already open for this exact race, add the model
  and reproduction as evidence there rather than creating a duplicate issue.
- No wall-clock limit is imposed. A healthy full compile-admission pool is
  normal; continue queued work and report only unhealthy admission per the
  repository instructions.

## Risks

- Risk: the stale-short-path hypothesis may not reproduce in the real callsite
  schedule. Severity: high. Likelihood: medium. Mitigation: first model the
  pinned registrar and interest transitions, then gate the real first event
  registration in a fresh process; keep the thread-handoff hypothesis distinct.
- Risk: permanent anchors increase interest rebuilds and `sometimes` results.
  Severity: medium. Likelihood: high. Mitigation: anchors are inert, limited to
  test processes, and document per-thread `enabled` filtering and the read-lock
  implications.
- Risk: an already in-flight `JustOne` registration can outlive anchor setup.
  Severity: medium. Likelihood: low. Mitigation: document the residual window
  and make the worker register anchors before T1 creates its subscriber.
- Risk: the `which` test has a different source of loss. Severity: medium.
  Likelihood: medium. Mitigation: measure it before and after separately; do
  not widen this issue if its failure remains.
- Risk: Loom abstracts the real `tracing-core` runtime. Severity: medium.
  Likelihood: certain. Mitigation: cite the pinned implementation, state the
  omitted allocator/TLS/subscriber details, and pair the model with the real
  fresh-process schedule.

## Progress

- [x] (2026-10-20) Confirmed worktree ownership, renamed the branch to the
  requested issue branch, set the Lody session title, and checked the full
  worktree list before branch mutation.
- [x] (2026-10-20) Loaded the requested CodeGraph, Rust Router, and Hexagonal
  Architecture skills. CodeGraph semantic search and edit-context tools are
  available and were used to check the capture helper and telemetry call paths.
- [x] (2026-10-23) Firecrawl verified that `tracing-core` 0.1.36 remains the
  latest published release. The pinned source was checked through
  `set_interest`, whose callsite store is `SeqCst` at lines 358–365.
- [x] (2026-10-23) Reviewed upstream issue #3611 and its open PR #3614. The PR
  adds `register_callsite_contention`, which covers the concurrent stale-write
  race directly; report this Loom model and worker as additional evidence
  there, rather than opening a duplicate issue.
- [x] (2026-10-20) Measure the unfixed libtest failure rates for the three
  requested module filters at default and explicit high thread counts.
- [x] (2026-10-20) Add the exact-cardinality oracle, its zero/one/duplicate
  controls, the uninstalled capture constructor, the Loom model, and the
  unmitigated worker.
- [x] (2026-10-23) Add the two inert test-process anchors, both worker modes,
  the exact validator in both original tests, and the callsite transition and
  default-dispatch checks. Update the developers' guide and add ADR-057.
- [x] (2026-10-23) Rebased the 11-commit branch range from
  `6572fb956379f47e70af914bfe4307b740a0b15e` onto `origin/main` at
  `8a961000859c02862454338fda6c4dd65bda2aa1`. The replay completed without
  conflicts; each commit passed `cargo check --workspace`, and `range-diff`
  showed all 11 commits unchanged. The target did not change `Cargo.lock`, so
  the branch lockfile was retained without regeneration. The three overlapping
  documentation files preserve both branches' additions.
- [x] (2026-10-23) Revalidated candidate
  `e8acacc4b462da7b0eae2fe49d816b416264b804`, including the source-doc fix for
  Codex finding 4357772141. Formatting, lint, typecheck, doc coverage (98.15%),
  tests (4,686 passed, 7 skipped; 40 doctests passed, 6 ignored), Markdown
  lint, and Mermaid validation passed. Local CodeRabbit review found no
  findings. Published with force-with-lease after confirming `origin/main` at
  `8a961000859c02862454338fda6c4dd65bda2aa1`; refreshed upstream tracking and
  verified remote parity. Prior hosted reviews and Actions were on the old
  head, so current-head review and Actions evidence remain outstanding.
- [x] (2026-10-23) Verified CodeRabbit findings 4357879835 and 4357879842 on
  the rebased source. The registration signal now originates in the capture
  layer's actual summary `register_callsite` callback, and an absent child mode
  makes a direct ignored-worker invocation a no-op. The fresh-process parent
  test passed both modes with the required markers; directly invoking the
  ignored worker without its mode also passed. Full candidate gates and a
  managed current-head review remain outstanding because automatic reviews
  paused after the rebase.
- [x] (2026-10-23) Reran the corrected Loom model and both fresh-process modes.
  At bound 3, Loom explored 312 unanchored executions with 156 lost-interest
  outcomes and 99 anchored executions with none. The child markers showed the
  empty unmitigated capture and one-event mitigated capture.
- [x] (2026-10-23) Disabled anchor registration locally and reran the parent
  worker test. It failed with release cause `installed`, six captured span
  fields, `events: []`, and the exact-cardinality error requiring one event.
  Restored the anchors and reran the worker successfully.
- [x] (2026-10-23) Repeated all three libtest filters sequentially at default
  and 16 threads: 37 and 22 runs, respectively. Every post-fix run passed.
- [x] (2026-10-23) Attached the Loom and real-callsite schedule evidence to
  [upstream PR #3614](https://github.com/tokio-rs/tracing/pull/3614#issuecomment-6357480721)
  using the allocated `buzzybee-df12` identity, then verified the posted
  comment's author, body, server timestamp, and URL.
- [x] (2026-10-23) Resolved the three initial CodeRabbit findings: repaired the
  ADR-0009 link, used British spelling in ADR-057, and extended the unmitigated
  worker's installation window to ten seconds while keeping the mitigated gate
  at 250 ms. The fresh-process regression passed in both modes afterwards.
- [x] (2026-10-23) The first post-fix CodeRabbit review returned zero findings.
  A later review of the updated staged plan suggested two overlapping
  extractions for the worker's duplicated orchestration and snapshot path. The
  code is now in a private `threads` module; its two subscriber boundaries
  remain distinct, and ADR-057 records the helper's scope.
- [x] (2026-10-23) Committed the implementation as `ba17d7c` after the full
  gate sequence passed. Committed the worker orchestration extraction as
  `6e43d16` after its gates passed, then pushed it with SSH and verified the
  named upstream branch.
- [x] (2026-10-23) Created draft PR #943 against `main` with `Closes #927` and
  the Lody session reference. The PR author is `leynos`; its base is
  `6572fb956379f47e70af914bfe4307b740a0b15e` and published head is
  `6e43d160ab59c2881933c88297a5a508a189b805`.
- [x] (2026-10-23) Passed `make check-fmt`, `make test` (4667 passed, 0
  failed, 7 skipped; workspace doctests passed), `make typecheck`, `make lint`
  (Clippy, Whitaker, Python lint, yamllint and actionlint), `make doc-coverage`
  (98.13% against the 80% threshold), `make markdownlint` (198 files, 0
  issues), and `make nixie` (all diagrams validated). The first doc-coverage
  attempt exhausted disk after this worktree's generated `target/` reached 35.1
  GiB; cleaning only that inactive target restored space, and the sequential
  retry passed with at least 121 GiB free.
- [x] (2026-10-23) After the CodeRabbit fixes, reran all seven gates
  sequentially: `make check-fmt`, `make test` (4667 passed, 0 failed, 7
  skipped; doctests passed), `make typecheck`, `make lint`, `make doc-coverage`
  (98.13%), `make markdownlint` (198 files, zero issues), and `make nixie` (all
  diagrams passed). This clean run completed without disk exhaustion.
- [x] (2026-10-23) After extracting the reproduction worker orchestration into
  its private `threads` module, reran `make fmt`, the focused fresh-process
  regression, and all seven repository gates sequentially. The focused
  regression reproduced the empty unmitigated capture and accepted the single
  mitigated event. Final results: `make test` passed 4667 tests with 7 skipped
  and all doctests passing; typecheck and lint passed; doc coverage was 98.13%;
  Markdown lint found zero issues in 198 files; and all diagrams passed. The
  first lint attempt identified one extra borrow in the extracted helper; that
  was corrected before this passing run.
- [x] (2026-10-23) The PR's CodeScene analysis flagged duplicate structure in
  the two T2 capture wrappers. Replaced them with one mode-selected capture
  thread and a common snapshot path, preserving the distinct `with_default` and
  `with_test_subscriber` boundaries. Bundled T2's phase channels after Clippy
  flagged the initial argument count. The latest focused test and all gates
  passed; `cs delta origin/main` reported no issues.
- [x] (2026-10-23) Committed the CodeScene-driven consolidation as `f0457010`
  after the focused regression and all repository gates passed. Force-pushed
  with a lease and verified that PR #943 now points to
  `f0457010039bde5c75eb5cc089fc9caffcc5aecb` over base
  `6572fb956379f47e70af914bfe4307b740a0b15e`.
- [x] The subsequent hosted CI run found that the new reproduction directory
  formed an unrecorded `runner/manifest` prefix group under the repository's
  module-layout contract. Renamed the test-only module to
  `callsite_interest_reproduction`. The repaired candidate passed
  `make test-workflow-contracts` (1337 passed, 3 skipped, plus 27 script
  contracts), `make test` (4667 passed, 7 skipped, all doctests passed),
  `make typecheck`, `make lint`, `make doc-coverage` (98.13%),
  `make markdownlint` (zero issues), `make nixie`, and `cs delta origin/main`
  (no issues). `make fmt` and `make check-fmt` also passed before these gates.
  The hosted failure log was `/tmp/issue-927-build-test-job-120243674734.zip`;
  current-head hosted checks must pass after publishing this repair.
- [x] (2026-10-23) The first completeness assessment for the published head
  identified two in-scope coverage gaps: the mitigated schedule did not
  acknowledge T2 reaching the pre-registration boundary before starting its
  fixed release interval, and V3 lacked wrong-value, duplicate-token, and
  extra-token controls. The response is recorded at
  [CodeRabbit assessment](https://github.com/leynos/netsuke/pull/943#issuecomment-6358510181).
- [x] (2026-10-23) Add a T2 boundary acknowledgement before starting the fixed
  installation deadline, retain the completed-installation signal, and add
  wrong-value, duplicate-token, and additional-token controls. Focused tests
  passed: the fresh-process worker printed the boundary marker in both modes,
  reproduced zero events in unmitigated mode, and accepted exactly one summary
  in mitigated mode; the malformed-token test passed. Logs:
  `/tmp/test-focused-b52a33dc-f184-4eef-82fc-038987db14c6-issue-927-manifest-structure-telemetry-test-loses-its-event-under-concurrent-execution-7.out`
  and `...-8.out`.
- [x] (2026-10-23) Recovered from the prior no-space failures and reran the
  complete gate sequence on the repaired candidate without cache cleanup or
  resource exhaustion. `make fmt`, `make check-fmt`,
  `make test-workflow-contracts` (1337 passed, 3 skipped, plus 27 script tests),
  `make test` (4668 passed, 7 skipped; doctests passed), `make typecheck`,
  `make lint`, `make doc-coverage` (98.13%), `make markdownlint` (zero issues),
  `make nixie` (all diagrams), and `cs delta origin/main` (no issues) passed.
  The host had 220 GiB free at the start and 222 GiB after the Rust gates;
  compile admission remained healthy. Logs use the
  `/tmp/<gate>-b52a33dc-f184-4eef-82fc-038987db14c6-issue-927-manifest-structure-telemetry-test-loses-its-event-under-concurrent-execution-<run>.out`
  pattern.
- [x] (2026-10-23) The current-head completeness assessment explicitly found
  no remaining in-scope correctness defect or implementation omission. It
  confirmed the two prior repairs and recorded the bounded model and
  in-flight-registration limits. The candidate-bound reply is at
  [CodeRabbit completeness assessment](https://github.com/leynos/netsuke/pull/943#issuecomment-6359344851).
- [x] (2026-10-23) The separate proof assessment confirmed the bounded model's
  logic and production correspondence, then identified that `Builder::new`
  inherits Loom environment limits and checkpoint settings. It found no
  evidence that the recorded search was truncated. Explicitly clear those
  settings in the test builder and verify the fixed counts under hostile
  ambient settings before requesting a candidate-bound follow-up. The
  assessment is at
  [CodeRabbit proof assessment](https://github.com/leynos/netsuke/pull/943#issuecomment-6359390997).
- [x] (2026-10-23) Added a configuration regression that pins every Loom
  builder option, including no permutation or duration limit, no checkpoint
  input, and the explicit three-preemption bound. The model still explored 312
  unanchored executions with 156 lost outcomes and 99 anchored executions with
  none under normal settings and with hostile `LOOM_*` limits and checkpoint
  settings; no checkpoint file was created. Focused log:
  `/tmp/loom-model-ambient-clear-issue-927-final.out` and
  `/tmp/loom-model-hostile-environment-issue-927-final.out`.
- [x] (2026-10-23) Reran the full sequential candidate gate sequence after the
  Loom configuration repair: `make check-fmt`, `make test` (4,669 passed, 7
  skipped; 40 doctests passed, 6 ignored), `make typecheck`, `make lint`,
  `make doc-coverage` (98.13%), `make markdownlint` (zero issues), and
  `make nixie` (all diagrams). Logs are
  `/tmp/{check-fmt,test,typecheck,lint,doc-coverage,markdownlint,nixie}-issue-927-post-assessment-fix.out`.
- [x] (2026-10-23) Recovered from the stop-hook disk-full condition: the
  filesystem had 203 GiB free, and the seven gates passed on head
  `a7b662da28644c1ebc10a5ad091872408136ef5f`. The subsequent post-cooldown
  `coderabbit review --agent` found three trivial concerns, two overlapping on
  the DEBUG-to-TRACE test. The transition test now proves that the TRACE
  callsite registers at least once and every observed interest remains `never`
  under the DEBUG filter before the TRACE capture proves the same callsite
  emits. The reproduction worker now iterates `EMPTY_MANIFEST_COUNTS` rather
  than duplicating its six entries. Its focused test passed.
- [x] (2026-10-23) The first gate run on those review fixes exposed denied
  Clippy lints for a deprecated `drop_span` allowance and a shadowed closure
  binding. Removed the unnecessary override and renamed the binding. The fresh
  sequential run passed `make check-fmt`, `make test` (4,669 passed, 7 skipped;
  40 doctests passed, 6 ignored), `make typecheck`, `make lint`,
  `make doc-coverage` (98.13%), `make markdownlint` (zero issues), and
  `make nixie` (all diagrams).
- [x] (2026-10-23) The next local CodeRabbit review found that concurrent
  dispatcher registrations can repeat `register_callsite` on a live DEBUG
  subscriber. Relaxed only the callback-count assertion: it now requires an
  observation and checks every result is `never`. The full seven-gate sequence
  passed (4,669 tests, 7 skipped; 40 doctests passed, 6 ignored); after the
  plan update, `make check-fmt`, `make markdownlint`, and `make nixie` passed.
- [x] (2026-10-23) Committed this correction and its plan evidence as
  `e571df4c69912c6cab397f1a2d3dbd85a488e9c0`, pushed with a lease over the
  verified prior head, and confirmed local, tracking, and remote refs match.
  The exact-head local review returned zero findings.
- [x] (2026-10-23) The candidate-bound Loom proof follow-up confirmed that the
  fixed builder settings prevent successful partial searches and checkpoint
  resumes. It found no issue with the stated three-thread, 10,000-branch, and
  three-preemption bounds. Malformed numeric `LOOM_*` values can still fail
  builder construction, but cannot silently truncate a passing search. Reply:
  [CodeRabbit proof follow-up](https://github.com/leynos/netsuke/pull/943#issuecomment-6361053063).
- [x] (2026-10-23) Reproduced and repaired CodeRabbit findings 4357879835 and
  4357879842. T2 now signals from the capture layer's actual summary
  `register_callsite` callback, and a direct ignored-worker invocation without
  its child-only mode exits as a no-op. The scoped hook is reachable only in
  test builds; the binary's test-only capture module is public within that test
  crate so dead-code lint does not reject a helper used by library tests. The
  focused parent test passed both child modes, and the direct worker invocation
  passed.
- [x] (2026-10-23) Repeated the complete sequential validation on working tree
  `e8acacc4b462da7b0eae2fe49d816b416264b804` plus the six review-fix files.
  Both focused worker checks and all seven gates passed: 4,687 tests passed, 7
  skipped; doctests had 40 passed and 6 ignored; typecheck and lint passed; doc
  coverage was 98.15%; Markdown lint found zero issues; and Nixie validated
  every diagram. Logs use the `/tmp/*-review-fixes-5.out` suffix, with focused
  logs ending in `-focused-reproduction-5.out` and `-focused-worker-5.out`. The
  previous stop-hook failures were caused by a full filesystem; the final run
  completed with disk space and compiler admission healthy.
- [x] (2026-10-23) CodeRabbit found that a post-join callback check could accept
  a T2 registration signal that arrived after T1's deadline. The worker now
  records a boundary signal immediately before `Dispatch::new` and separately
  records the capture layer's actual summary callback. T1 requires the boundary
  signal before release. Unmitigated mode requires the callback before release;
  anchored mode requires it only after release. A unit negative control
  withholds the boundary signal until after release and requires
  `registration-start-timeout`. The parent requires all phase markers.
  `Dispatch::new` allocates the subscriber `Arc` before calling the private
  `callsite::register_dispatch` write-lock path. T2 can be preempted after the
  boundary signal and before that path, so the signal is not proof of lock
  contention in every execution; the negative control rejects a late signal but
  not pre-emption after an on-time signal. The later layer callback confirms
  the actual callback phase. Both focused worker modes and all seven candidate
  gates passed after this correction: 4,688 tests passed, 7 skipped; doctests
  had 40 passed and 6 ignored; typecheck and lint passed; doc coverage was
  98.15%; Markdown lint found zero issues; and Nixie validated every diagram.
  The disk-full stop-hook failures did not recur after filesystem space
  recovered. Logs use the `-3.out` suffix for focused and documentation checks,
  `-4.out` for tests, and `-3.out` for typecheck and lint.
- [x] (2026-10-23) Confirmed PR #943 remains ready for review after the
  candidate-bound assessments and checks passed; it stays ready.
- [x] (2026-10-23) Recovered from the stop-hook disk-full failure. The
  filesystem had 169 GiB available before the rerun. On the current local
  candidate, the focused worker, `make check-fmt`, `make typecheck`,
  `make lint`, `make doc-coverage` (98.15%), `make markdownlint` (199 files,
  zero issues), and `make nixie` all passed. `make test` ran 4,664 of 4,688
  tests before `cli_configuration_fixture_compiles::case_1_public_api` exceeded
  its 300-second limit: 4,663 passed, one timed out, seven skipped, and 24 were
  not run. The failure log contains no nested Cargo diagnostics; its cause is
  not established, and no timeout or retry policy was changed. The source
  adjustment separates the 10-second wait for T2 to reach the registration
  boundary from the 250-millisecond callback window. Focused regression passed
  with both schedule modes. Logs use the current `-4.out` through `-7.out`
  suffixes under `/tmp`.
- [x] (2026-10-23) Reran `make lint` and `make nixie` after the stop-hook
  storage failure. Both passed on unchanged head
  `30e81e0185ee8ecae4a38c7a00f824e3ec8b31af`; the filesystem had 124 GiB free
  before lint and 108 GiB after both gates. Whitaker waited on the shared Cargo
  package-cache lock, and two status queries timed out, but lint completed with
  exit 0 and no disk error. Nixie validated all diagrams and `/tmp` accepted a
  temporary directory. This confirms the original disk and temporary-directory
  failures are resolved.
- [x] (2026-10-23) CodeRabbit accepted the registration-boundary repair and
  resolved the thread at
  [CodeRabbit's ordering response](https://github.com/leynos/netsuke/pull/943#discussion_r4363520093).
  It clarified that the worker checks the pre-registration boundary and
  callback ordering; T2 can be preempted between that signal and the private
  write-lock attempt, so the worker does not prove contention in every run.
  Updated ADR-057 and V2 below to preserve that limit; the bounded Loom model
  supplies separate ordering coverage.
- [x] (2026-10-24) A serial binary test run reproduced a test-order defect from
  `run_with_args` installing a process-wide tracing subscriber. Moved the
  existing production initialization to `main` and ran configuration tests
  under a scoped registry; the focused serial binary suite passed 62 tests.
  Clippy then identified that the new `StartupWriter` parameter should be
  borrowed; the helper and its two callers now pass `&StartupWriter`.
- [x] (2026-10-24) Rebased the 18-commit range from base
  `8a961000859c02862454338fda6c4dd65bda2aa1` onto current `main` at
  `018b81b4311ac5dd99ac4182c51301eeca0dfc55`. The replay completed without
  conflicts. The two overlapping documentation files retain the newer
  Dependabot runner and PowerShell placeholder guidance alongside this plan's
  tracing documentation. The saved tracing patch was reapplied and its SHA-256
  is `d993eb85c8921d16c1daea18d5b1e752405ac35193a71a428c325bb98b6ca9ef`.
- [x] (2026-10-24) Rebased the 19-commit range from base
  `018b81b4311ac5dd99ac4182c51301eeca0dfc55` onto `main` at
  `f8df5d808f1bee4eb8554df0160119864ade6f39`, which advances the pinned
  toolchain to `nightly-2026-10-22`. The replay completed without conflicts;
  `git range-diff` matched all 19 commits. The overlapping developers' guide
  retains both the toolchain history updates and this change's tracing
  documentation. All gates must be rerun on the rebased candidate.
- [x] (2026-10-24) Re-ran the sequential gates after correcting the Clippy
  borrows. Formatting, lint, typecheck, doc coverage (98.19%), Markdown lint,
  Nixie, and local CodeScene checks passed. Nextest passed 4,852 tests with
  seven skipped before `cli_configuration_fixture_compiles::case_1_public_api`
  timed out at 300 seconds; 24 tests and doctests did not run. The captured
  output has no compiler diagnostic, and process inspection found the fixture's
  nested Cargo check compiling `ureq` through the admission wrapper. The same
  fixture passed in 159 seconds in the preceding full run, which passed all
  4,877 tests and doctests; that run differed only by three redundant borrows
  removed in this Clippy correction. The timed-out gate was not retried.
- [x] Commit and publish the rebased candidate, then require exact-head hosted
  CI to clear the local fixture timeout. The currently published candidate and
  hosted checks are recorded below.
- [ ] Refresh CodeRabbit assessments and review state for the final published
  head before merging PR #943.
- [ ] Resolve current-head review concerns, follow checks to equilibrium, and
  merge PR #943 with the verified head commit.

- [x] (2026-10-24) Published and validated head
  `73f83cbdea021143b1d01dee8b1f37012135caad` over base
  `f8df5d808f1bee4eb8554df0160119864ade6f39`. GitHub Actions run `40101269764`
  passed its applicable checks, including `build-test`; local `cs delta`
  between these commits reported no issues. Focused CodeScene review and check
  both scored the manifest telemetry source, its tests, and
  `callsite_interest_reproduction/threads.rs` at 10.0.
- [x] (2026-10-24) Ran the final sequential Make gates on that exact head:
  `make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage`,
  `make markdownlint`, and `make nixie` passed. Doc coverage was 6,180/6,294
  items (98.19%), and the 65-test coverage harness passed. `make test` reported
  4,877 tests: 4,876 passed and one timed out; 7 were skipped.
  `packaged_manifest_retains_build_script_sources` timed out at its configured
  900-second limit while nested `cargo publish --dry-run` was compiling
  `netsuke` through the healthy, saturated shared compiler queue. No source
  diagnostic appeared; nextest stopped before doctests. The exact-head hosted
  `build-test` passed. Gate logs use the `published-final-1.out` suffix under
  `/tmp`.
- [x] (2026-10-24) The exact-head CodeRabbit review identified that existing
  tests could pass if the public capture helper stopped registering its
  anchors, and requested fuller Loom tooling documentation. Added a third
  deterministic fresh-process mode that reaches anchors only through the public
  `with_test_subscriber` helper, retaining the unmitigated control and exact
  one-event oracle. Added thread-local registration hooks solely to coordinate
  this schedule, and documented their scope and the Loom model in the
  developers' guide. Sequential candidate gates and a fresh review are pending.
- [x] (2026-10-24) The first sequential `make test` attempt found the new
  observer function unused in the binary test target, where
  `test_tracing_capture` is also compiled. Removed that extra function and
  moved its metadata check into the library-only reproduction worker, reusing
  the scoped signal already used by the capture layer. The gate sequence is
  paused at this finding; rerun it after formatting this correction.
- [x] (2026-10-24) The next test run showed that the public helper's first lazy
  anchor initialization happened after T1 entered the gate, allowing the
  summary callback before release. The public-helper child now initializes its
  anchors through `with_test_subscriber` before starting the gated schedule;
  the scheduled T2 capture still uses the public helper, and neither path calls
  the anchor function directly. The gate run stopped before later gates, which
  remain pending on this corrected candidate.
- [x] (2026-10-24) The follow-up compile then found the public-helper
      preflight's
  missing `LevelFilter` import. Added the explicit `tracing_subscriber` import;
  that run stopped before tests, so the schedule still requires candidate-bound
  execution evidence and the full gate sequence.
- [x] (2026-10-25) Completed the corrected candidate's sequential gates:
  `make check-fmt`, `make test` (4,877 passed, 7 skipped; all doctests passed),
  `make typecheck`, `make lint`, `make doc-coverage` (6,188/6,303 items;
  98.18%; 65 harness tests passed), `make markdownlint` (199 files, zero
  issues), and `make nixie`. The local CodeScene staged-diff analysis found no
  issues. No test timed out. A fresh review and hosted checks on the published
  candidate remain pending.
- [x] (2026-10-25) Rebased and published candidate
  `e0d8f5a1a9a363d541c0fab55f0dc2686dea44e7` on `main` at
  `de81d5c34ab3c0103a5de71a0518b89980187c11`, then passed the sequential gates
  `make check-fmt`, `make lint`, `make doc-coverage` (6,231/6,346 items;
  98.19%; 65 harness tests passed), `make typecheck`, `make test` (4,725
  passed, 7 skipped; all doctest groups passed), `make markdownlint` (200
  files, zero issues), and `make nixie`. `cs delta origin/main HEAD` found no
  issues in the changed code. The exact-head hosted Netsukefile, Release Dry
  Run, CodeScene, Kani, and build-test checks passed; Windows lint remained
  pending at the latest observation, and CodeRabbit's status was
  `Review paused` for this head. The hosted status and formal review still
  require reconciliation.
- [x] (2026-10-25) Ran the focused public-helper negative control on the
  published candidate by removing only the call to
  `ensure_test_interest_anchors_registered()` from
  `test_support/src/tracing_capture.rs`. The parent regression test failed in
  the public-helper child because its summary callback ran before T1 released
  the gate (`before release=true, after release=false`). The unmitigated and
  anchored child schedules passed first. This test checks registration timing
  before reaching the public-helper exact-cardinality oracle, so this run does
  not establish the requested empty-event oracle diagnostic; record that limit
  for the follow-up assessment. Restored the source byte-for-byte (SHA-256
  `08ccbe5e1077e856aedfd69088bb19a26e58f63a9577796af1b78287b8b6cf56`) and reran
  `cargo nextest run -p netsuke-build --lib
  stale_summary_interest_is_reproduced_in_a_fresh_process`,
  which passed. Mutation and restored-run logs are in the private PR #943
  evidence bundle.
- [x] (2026-10-25) Corrected the focused Loom command in the developers' guide
  to select the Cargo package `netsuke-build`; `netsuke` is the library and
  binary target name. The previous command named a non-existent package. The
  corrected command
  `cargo nextest run -p netsuke-build --test tracing_interest_model` passed
  both tests (log:
  `/tmp/focused-tracing-interest-b52a33dc-f184-4eef-82fc-038987db14c6-issue-927-e0d8f5a.out`).
  The complete sequential candidate gates also passed with this guide
  correction: `make check-fmt`, `make lint`, `make doc-coverage` (98.19%; 65
  harness tests), `make typecheck`, `make test` (4,725 passed, 7 skipped; all
  doctest groups passed), `make markdownlint` (200 files, zero issues), and
  `make nixie`. These results are recorded here after that sequence; run the
  documentation gates on this final plan revision before commit.

## Surprises & discoveries

- Observation: the pinned release has a `JustOne` short path that reads the
  current thread default, while a concurrent dispatcher registration can
  rebuild a newly published callsite before the older short path writes its
  cached interest. Evidence: `tracing-core-0.1.36/src/callsite.rs` lines
  544–573 and 484–506. Impact: the proposed model and worker can test the stale
  write separately from the existing thread-handoff report.
- Observation: the pinned `DefaultCallsite::set_interest` uses a `SeqCst`
  atomic store. Evidence: `tracing-core-0.1.36/src/callsite.rs:358-365`.
  Impact: the Loom model now preserves the real ordering and explores a stale
  last-writer race, not a relaxed-memory reordering.
- Observation: upstream PR #3614 contains a separate regression named
  `register_callsite_contention` for this same race. Evidence:
  [tokio-rs/tracing#3614](https://github.com/tokio-rs/tracing/pull/3614),
  linked from [issue #3611](https://github.com/tokio-rs/tracing/issues/3611).
  Impact: no duplicate report is needed; attach the Loom and actual
  production-callsite schedule as additional evidence after the branch is
  reviewable.
- Observation: the initial real worker never reached the gated TRACE event
  registration because the DEBUG subscriber's `max_level_hint` elided it before
  `register_callsite`. Evidence: the parent initially timed out waiting for
  `ISSUE927_GATE_PAUSED`; returning a permissive TRACE hint from the delegating
  test subscriber made the gate deterministic while the inner subscriber still
  supplies the DEBUG callsite interest. Impact: the worker now gates the actual
  registration path without changing production code.
- Observation: the stale-interest interleaving is confirmed in the bounded
  model and in the fresh-process worker. Evidence: Loom explored 312 unanchored
  executions, with 156 lost-interest outcomes; its anchored variant explored 99
  executions with no lost-interest outcome. The worker printed
  `RELEASE_CAUSE=installed`, six captured span count fields, an empty event
  snapshot, and `ORACLE=zero-rejected`. Impact: the manifest failure is
  explained by the stale `JustOne` write; the thread-local caveat is not the
  mechanism demonstrated by this schedule.
- Observation: the pre-fix `which` failures are distinct test evidence, not
  proof of a shared cause. Evidence: `stdlib::which::telemetry` failed 7/37
  default-thread runs and 6/22 runs at 16 threads, with missing failure-event
  captures; the manifest module failed 1/37 and 1/22, respectively. The adjacent
  `runner::tests` filter passed 37/37 and 22/22. Impact: retain a separate
  post-fix `which` measurement and do not attribute it to this defect without
  evidence.
- Observation: after anchors were added, all 37 default-thread and 22
  16-thread runs passed for each filter, including `stdlib::which::telemetry`.
  Impact: the post-fix sample does not establish whether the `which` flake had
  the same cause; it supplies no remaining failure requiring a separate issue.
- Observation: disabling the anchor function made the mitigated child fail
  before the final assertion. Its error reported release cause `installed`, all
  six span fields, an empty event list, and the validator's exact-one-event
  error. Impact: the regression is non-vacuous and depends on anchor
  registration.

## Decision log

- Decision: investigate stale `JustOne` overwrite as a separate hypothesis
  from thread-local subscriber handoff. Rationale: the failing event and the
  `which` flake are in different emitting paths, and the captured span proves
  the capture dispatch can survive while one event is lost. Date/Author:
  2026-10-20, Codex following issue #927.
- Decision: use two inert permanent `Dispatch` registrars in test processes if
  the model and real schedule confirm the stale path. Rationale: the second
  live registrar makes `has_just_one` false after weak registrars are pruned,
  selecting the registry read-lock path without installing a default or
  serializing tests. Date/Author: 2026-10-20, Codex following issue #927.
- Decision: retain tracing-core 0.1.36 and add Loom 0.7.2 as a root
  development-only dependency. Rationale: Firecrawl verified 0.1.36 remains the
  newest published tracing-core; the open PR #3614 is not a released fix. Loom
  0.7.2 remains the model checker version. Date/Author: 2026-10-23, Codex.
- Decision: add the additional schedule evidence to the existing upstream
  tracking thread rather than create a duplicate. Rationale: PR #3614 already
  contains `register_callsite_contention`, while this branch contributes a
  bounded Loom model and a worker through the real production callsite.
  Date/Author: 2026-10-23, Codex.
- Decision: treat the stale `JustOne` write as the demonstrated manifest event
  loss mechanism. Rationale: both the extracted model and gated real-callsite
  child reproduce the lost event while retaining the structural span; the
  child's `installed` release marker proves T2's dispatcher registration
  overlaps T1's gated registration. Keep the `which` failures separate pending
  post-fix evidence. Date/Author: 2026-10-20, Codex.

## Outcomes & retrospective

The reproduced worker is a test-only orchestration of the real callsite;
production emission remains synchronous in `record_manifest_structure`.

- Pre-fix libtest matrix (runs are sequential, no retry):

  | Filter                                 | Default threads      | 16 threads           |
  | -------------------------------------- | -------------------: | -------------------: |
  | `runner::manifest_structure_telemetry` | 36 pass, 1 fail / 37 | 21 pass, 1 fail / 22 |
  | `runner::tests`                        | 37 pass, 0 fail / 37 | 22 pass, 0 fail / 22 |
  | `stdlib::which::telemetry`             | 30 pass, 7 fail / 37 | 16 pass, 6 fail / 22 |

- Loom model: `preemption_bound=3`, unanchored 312 executions with 156 lost
  outcomes, anchored 99 executions with zero lost outcomes. Both variants
  passed their assertions.
- Unmitigated child markers: `GATE_PAUSED=true`,
  `RELEASE_CAUSE=installed`, six zero-valued span count fields,
  `EVENT_SNAPSHOT=[]`, and `ORACLE=zero-rejected`. The fresh-process parent
  test passed and required each marker.
- Oracle controls and focused module tests passed: 6 passed, 1 ignored in
  `runner::manifest_structure_telemetry`; a real production TRACE capture with
  a seeded duplicate is rejected. The original runner integration assertion
  also passed with the exact-cardinality validator.
- Disabled-anchor negative control: the parent regression failed with
  `RELEASE_CAUSE=installed`, six span fields, `events: []`, and
  `a structural summary must emit exactly one trace event: []`. The anchor
  implementation was restored before the passing run.
- Post-fix sequential libtest matrix (no retries):

  | Filter                                 | Default threads      | 16 threads           |
  | -------------------------------------- | -------------------: | -------------------: |
  | `runner::manifest_structure_telemetry` | 37 pass, 0 fail / 37 | 22 pass, 0 fail / 22 |
  | `runner::tests`                        | 37 pass, 0 fail / 37 | 22 pass, 0 fail / 22 |
  | `stdlib::which::telemetry`             | 37 pass, 0 fail / 37 | 22 pass, 0 fail / 22 |

  The clean `which` sample does not prove a shared cause; no separate `which`
  issue is indicated by these post-fix runs.
- The test wrapper must report a permissive TRACE max-level hint so tracing
  invokes its delegated DEBUG `register_callsite` callback for the target
  event. The actual delegated interest remains restrictive; the hint only
  ensures the registration callback can be gated.
- Follow-up review (2026-10-23): CodeRabbit identified that the same live
  DEBUG subscriber can receive repeated `register_callsite` callbacks while
  other libtest threads register dispatchers. The transition test now requires
  at least one observation and verifies every observed interest is `never`,
  without assuming a single callback. The review found no other concern.
- Follow-up review (2026-10-24): CodeRabbit's serial test-order check found
  that calling `run_with_args` from configuration tests installed a global
  subscriber before the anchor test ran. Keeping production initialization in
  `main` and running the test helper under a scoped registry removes that
  order-dependent process state. The focused serial binary suite passed. The
  fresh final sequence passed every gate except one nested CLI fixture timed
  out while compiling `ureq`; the same test passed in the prior complete run.
  Exact-head hosted CI remains the acceptance check for that environment-bound
  result.
- Follow-up review (2026-10-24): CodeRabbit found that the existing fresh-
  process schedule called the anchor function directly, so it did not protect
  `with_test_subscriber`'s public anchor-registration contract. The new
  `public-helper` child mode uses only that public helper and checks the same
  one-event oracle; the unmitigated zero-event control remains. The developers'
  guide now records Loom 0.7.2, the model path and bounds, its abstractions,
  the focused command, and links to ADR-057 and this plan. The corrected
  candidate still required a complete sequential gate run at that point; see
  the later progress entry for its results.
- The first stop-hook rerun hit a transient full filesystem while Cargo wrote
  build metadata and Nixie created a temporary directory. After space became
  available, all seven gates passed sequentially: `make check-fmt`, `make test`
  (4,669 passed, 7 skipped; 40 doctests passed, 6 ignored), `make typecheck`,
  `make lint`, `make doc-coverage` (98.13%), `make markdownlint` (zero issues),
  and `make nixie`. The available space was 188 GiB at the beginning of the
  rerun and remained ample throughout.

The mutation baseline used libtest. Nextest runs each test in its own process,
so it does not expose this in-process race in the same way; the required
reproduction and rate matrix therefore use `cargo test --lib` directly.

## Context and orientation

`src/runner/manifest_structure_telemetry.rs` emits a TRACE span and event with
six bounded collection counts. Its unit test uses the exact-cardinality oracle
and checks the structural span fields. `src/test_tracing_capture/mod.rs` and
`test_support/src/tracing_capture.rs` install per-thread subscribers through
`tracing::subscriber::with_default`; the former also records span fields.
`src/runner/tests/manifest_structure_telemetry_tests.rs` uses the same oracle
to reject duplicate summary events. The dyndep generation telemetry tests build
a private TRACE subscriber in `src/runner/dyndep/generation_telemetry.rs`.

The candidate defect is in the pinned `tracing-core` 0.1.36 callsite cache.
`Dispatchers::rebuilder` may return `JustOne` after reading `has_just_one`.
That rebuilder reads the current thread default, while registering another
dispatch can publish a new dispatcher and rebuild the callsite. A stale
`JustOne` computation can then overwrite the rebuilt interest. Loom will model
the registrar lock, weak references, callsite cache, dispatcher defaults,
interest combination, and this publish/rebuild/write order. A fresh-process
worker will exercise the actual `record_manifest_structure` callsites.

The repository design source is `docs/netsuke-design.md`; the existing bounded
manifest telemetry decision is
[ADR-009](../adrs/adr-0009-bounded-redacted-telemetry-for-manifest-evaluation.md).
The test-harness decision is recorded in
[ADR-057](../adrs/adr-0057-anchor-tracing-test-callsite-interest-rebuilds.md),
referenced from the design and developers' guides, and indexed from
`docs/contents.md`. No user-facing application behaviour changes.

## Conformance basis

- Issue #927 acceptance criteria -> EP-M1, EP-M2, EP-M3 -> the model,
  fresh-process worker, exact-cardinality controls, and recorded failure-rate
  matrix.
- [ADR-0009](../adrs/adr-0009-bounded-redacted-telemetry-for-manifest-evaluation.md)
  bounded and redacted manifest telemetry -> preserve fixed count fields and
  assert the named structural span ->
  `manifest_structure_telemetry_carries_only_counts`.
- `AGENTS.md` test, documentation, concurrency, and gate rules -> every
  milestone and final quality gates.
- No separate Terms of Reference or task-specific technical design exists.
  The issue's coding plan is the approved implementation scope.

## Verification plan

- **V1: Cached interest reflects every live registrar.** When T1 has selected
  the one-dispatch `JustOne` path and blocks in the target callsite, T2 may
  publish another dispatcher and rebuild the callsite before T1 writes its
  stale restrictive result. Method: bounded Loom model with two threads, weak
  registrars, registrar `RwLock`, `has_just_one`, `JustOne`/`Read`/`Write`
  rebuilders, `for_each`, `Interest::and`, and callsite publication before
  rebuild and `set_interest`. Domain: one TRACE callsite, a DEBUG T1 default, a
  TRACE T2 default, and at most three preemptions. Artefact:
  `tests/tracing_interest_model.rs`. Evidence: the unanchored model must
  explore at least one execution with `Interest::never()` cached while T2's
  TRACE dispatch remains live; the anchored model must explore none. Print
  explored execution counts and the bound. Non-vacuity: assert both dispatches
  are live at the witness and include a second-anchor control. Residual: this
  model omits allocator timing, real tracing TLS, and full subscriber stacks;
  validate the same ordering through V2.
- **V2: Real callsite schedule is explicitly gated.** The first T1 registration
  for the summary event pauses while T2 installs a TRACE capture dispatch.
  Unmitigated mode must release on `installed`, capture the structural span,
  and report zero summaries. In mitigated mode, if T2 reaches `Dispatch::new`
  while T1 holds the registry read lock, its registration waits until T1
  releases on the fixed 250 ms deadline. The capture callback is then observed
  after release, and T2 captures the span and exactly one summary. The worker
  records the pre-registration boundary and callback phases separately. T1
  requires T2's boundary signal before release; a late signal cannot satisfy
  this check. There is a scheduler pre-emption window after that signal and
  before `Dispatch::new` reaches tracing-core's private write-lock attempt, so
  the worker does not prove lock contention in every execution. The boundary
  signal and callback ordering are the observable checks; the bounded Loom
  model supplies separate ordering coverage. In the anchored run the real
  capture-layer callback is observed only after T1 releases the registry lock.
  A third public-helper mode must obtain anchors solely through
  `test_support::tracing_capture::with_test_subscriber`, observe the callback
  after release, and pass the exact one-event oracle. It must not call the
  anchor function or another anchor-registering helper directly. An
  acknowledgement within the callback deadline releases on `installed` and
  fails the worker. Unmitigated mode uses the 10-second thread deadline for the
  boundary and callback phases. Retain the separate installation-completion
  signal inside `with_default`. Method: ignored child worker driven by a parent
  test through `current_exe()`, with a wall-clock deadline. The parent kills a
  child that exceeds its 20-second deadline. Artefact:
  `src/runner/callsite_interest_reproduction/`. Non-vacuity: require all phase
  markers and the exact-cardinality oracle; removing the pre-registration
  anchor from the public helper must reproduce zero summaries and a validator
  error in the public-helper child, which has no direct anchor setup.
- **V3: Summary oracle is exact and non-vacuous.** One exact summary with the
  six expected count tokens succeeds; zero or two summaries fail; wrong or
  duplicate count tokens, an additional count token, and `SENTINEL` fail.
  Method: synthetic controls plus a real TRACE capture seeded with a duplicate.
  Artefact: `src/runner/manifest_structure_telemetry.rs`. Evidence: the zero
  and duplicate controls return errors and the seeded duplicate is rejected.
- **V4: Capture contract remains stable.** A same static callsite transitions
  from DEBUG to TRACE with zero then one captured event, and anchoring does not
  change `dispatcher::get_default` outside a scoped capture. Method: unit tests
  in `src/test_tracing_capture/mod.rs`. Preserve the existing nested-scope,
  poison-recovery, and span-field tests.
- **V5: Scheduling measurements match the issue's requirement.** Run each
  module filter 37 times at libtest's default thread count and 22 times with
  `--test-threads=16`, before and after anchors. Filters are
  `runner::manifest_structure_telemetry`, `runner::tests`, and
  `stdlib::which::telemetry`. Record pass/fail counts per filter and condition.
  Require zero manifest summary failures after the fix. Report `which`
  independently; if it still fails, record a separate cause and file a separate
  issue without widening #927.
- **V6: Repository quality gates.** Run `make check-fmt`, `make test`,
  `make typecheck`, `make lint`, `make doc-coverage`, `make markdownlint`, and
  `make nixie` sequentially before the final review. Do not change nextest
  retry or test-thread policy.

## Plan of work

### EP-M1: Establish the oracle and reproduce the candidate race

Measure the unfixed test modules first. Add a crate-private exact-cardinality
validator and zero/one/duplicate controls to
`src/runner/manifest_structure_telemetry.rs`. Extract an uninstalled capture
subscriber constructor from `src/test_tracing_capture/mod.rs` and delegate the
existing helper to it without changing helper behaviour. Add the two-variant
Loom model in `tests/tracing_interest_model.rs`. Add a test-only worker and
parent driver under `src/runner/` that gate only the target event registration
in the real `record_manifest_structure` callsites.

The unmitigated child must observe the stale overwrite and print the gate,
release, span, and event markers. At this plateau the worker asserts that the
span is captured while the summary list is empty; it does not yet add anchors.
Run the model and focused tests, run all applicable commit gates, commit this
plateau, and request CodeRabbit review before starting EP-M2.

### EP-M2: Prevent new stale short-path selections in test processes

Add two `OnceLock`-guarded inert `Dispatch` values to
`test_support/src/tracing_capture.rs`. Call the registration function before
capture construction in both `with_test_subscriber` helpers and the private
dyndep span-capture helper. In mitigated worker mode, register anchors before
T1 installs its DEBUG subscriber and use `with_test_subscriber` for T2. Require
the fixed-deadline release and exactly one event. Temporarily disable the
pre-registration anchor call and confirm the worker reports zero events and a
validator error. Since tracing-core 0.1.36 remains latest and open PR #3614
already contains the contention regression, attach the model and worker
evidence to the existing upstream thread rather than filing a duplicate. Gate,
commit, and review before EP-M3.

### EP-M3: Apply the oracle, document the decision, and finish validation

Use the oracle in both existing manifest telemetry tests, retaining all
existing assertions and adding the structural span assertions. Add the
DEBUG-to-TRACE transition and outside-default tests. Update the developers'
guide, the design-document cross-reference, ADR-057, and this ExecPlan. Repeat
the exact pre-fix run matrix; keep `which` results separate. Run all required
gates, push the exact branch over SSH, establish `origin/<branch>` tracking,
request the final CodeRabbit review, resolve its concerns, and create a draft
PR whose title contains `(#927)`, summary says `Closes #927`, and terminal
`## References` section contains the Lody session URL.

## Milestones and plateaus

- **EP-M1 (complete):** The exact oracle, non-vacuous controls, Loom variants,
  and unmitigated real worker are complete. The bounded model finds the stale
  witness; the worker captures span fields but no event. The later full gate
  run and CodeRabbit review cover the integrated candidate.
- **EP-M2 (complete):** Test-process anchors prevent the selected stale
  schedule without global defaults or test serialization. Unmitigated mode
  fails validation, mitigated mode passes after its fixed deadline, and the
  disabled-anchor control fails. The implementation, worker extraction, and
  CodeScene-driven consolidation are committed and published.
- **EP-M3 (implementation and validation complete):** Original tests,
  supporting tests, docs, ADR, repeated measurements, and the worker
  consolidation are complete. The latest source change passed the focused
  regression and all repository gates; local CodeScene delta found no issues.
  Post-fix manifest runs are clean in the recorded default/high-thread matrix.
  Remaining work: complete exact-head hosted assessments and replies, then
  follow ready-for-review PR #943 to merge.

## Concrete steps

Run commands from the repository root. The repeated measurements use
`cargo test --lib <filter>` and append one log per run beneath `/tmp`; they are
sequential and use the shared Cargo cache. Run the focused Loom integration
test and fresh-process parent driver before the wider repository gates. Keep
each log's exit status in the plan's outcome record.

The final gate sequence is:

```sh
make check-fmt
make test
make typecheck
make lint
make doc-coverage
make markdownlint
make nixie
```

Expected verification output includes a model witness in Variant A, no witness
in Variant B, `ISSUE927_DISPATCH_REGISTRATION_STARTED_BEFORE_RELEASE=true` in
all three children, `RELEASE_CAUSE=installed` and summary callback markers
`BEFORE_RELEASE=true, AFTER_RELEASE=false` in the unmitigated child,
`RELEASE_CAUSE=deadline` and callback markers
`BEFORE_RELEASE=false, AFTER_RELEASE=true` in both anchored children, a span
snapshot in the original two modes, and one exact summary only in the anchored
modes. The public-helper child must use no direct anchor-registration call. The
delayed-boundary negative control must reject a registration signal sent only
after T1 releases its gate.

## Validation and acceptance

The focused red control is the unmitigated child: after the gated DEBUG
registration races T2's TRACE dispatch registration, T1 must overwrite the
callsite to `never`; T2 then captures the span but no summary, and the exact
validator rejects the empty list. The green control is the anchored child: on
the recorded run, T2's summary callback arrived only after T1 released the
registry read lock, and the captured event passed the exact validator. The
worker's pre-registration boundary is immediately before `Dispatch::new`, but
T2 can be preempted after that signal and before the private write-lock
attempt. The worker therefore checks the boundary and callback order; it does
not establish lock contention for every execution. The bounded model separately
explores the stale-interest ordering.

The negative oracle control is a real TRACE capture containing the production
summary plus a seeded duplicate summary. It must fail for cardinality two. The
synthetic zero and duplicate cases must also fail, while one exact summary with
all six counts succeeds.

Completion requires the model and worker evidence, the recorded before/after
failure matrix with zero post-fix manifest failures, unchanged nextest policy,
all seven Make gates passing, resolved CodeRabbit concerns, the upstream
evidence comment, and a pushed PR with the required title, summary, and session
reference. The PR is created as a draft, marked ready after publication checks,
and followed through merge.
