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
- Do not install a global dispatcher, serialize tests, add retries, or weaken
  the summary assertion.
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
- [x] (2026-10-23) Rebase the empty branch series onto verified `origin/main`
  at `6572fb956379f47e70af914bfe4307b740a0b15e`. Reapplying the saved worktree
  changes produced no conflicts. Rebuilt `Cargo.lock` from the target baseline;
  metadata added Loom 0.7.2 and four transitive packages without unrelated
  version upgrades.
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
- [ ] Complete the separate completeness and bounded-model assessments for PR
  #943, mark it ready only after those replies and current checks clear, then
  follow it to merge.

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
- **V2: Real callsite schedule is deterministic.** The first T1 registration
  for the summary event pauses while T2 installs a TRACE capture dispatch.
  Unmitigated mode must release on `installed`, capture the structural span,
  and report zero summaries. Mitigated mode must release on the fixed deadline
  because the registry read lock blocks T2 registration, then capture the span
  and exactly one summary. Method: ignored child worker driven by a parent test
  through `current_exe()`, with a wall-clock deadline. The unmitigated gate
  uses the existing 10-second thread deadline so loaded hosts have time to
  construct and register the TRACE capture; the mitigated gate retains its
  fixed 250 ms delay because T2 cannot install while T1 holds the registry read
  lock. The parent kills a child that exceeds its 20-second deadline. Artefact:
  `src/runner/callsite_interest_reproduction/`. Non-vacuity: require all phase
  markers and the exact-cardinality oracle; removing the pre-registration
  anchor must reproduce zero summaries and a validator error.
- **V3: Summary oracle is exact and non-vacuous.** One exact summary with the
  six expected count tokens succeeds; zero or two summaries fail; wrong or
  duplicate count tokens and `SENTINEL` fail. Method: synthetic controls plus a
  real TRACE capture seeded with a duplicate. Artefact:
  `src/runner/manifest_structure_telemetry.rs`. Evidence: the zero and
  duplicate controls return errors and the seeded duplicate is rejected.
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
  Remaining work: complete hosted assessments, mark PR #943 ready when current
  checks and replies clear, and follow it to merge.

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
in Variant B, `RELEASE_CAUSE=installed` in the unmitigated child,
`RELEASE_CAUSE=deadline` in the mitigated child, a span snapshot in both modes,
and one exact summary only in mitigated mode.

## Validation and acceptance

The focused red control is the unmitigated child: after the gated DEBUG
registration races T2's TRACE dispatch registration, T1 must overwrite the
callsite to `never`; T2 then captures the span but no summary, and the exact
validator rejects the empty list. The green control is the anchored child: T1
holds the registry read lock while paused, T2 cannot publish until the fixed
deadline releases T1, and T2's subsequent write-locked rebuild restores the
interest before the capture emits.

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
