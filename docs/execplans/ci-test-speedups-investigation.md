# Investigate CI test speedups with cost and fidelity gates

Status: IN PROGRESS

The investigation is under way. The implementation snapshot exercised by the
earlier local gates was based on `origin/main` at
`051358ee360a97fcbebe2a836e25e106917ed845`. The branch still needs replay onto
the latest target observed on 20 October,
`f16c31760aff94a1d91bbbbfac4d024873942ce5`. The earlier investigation base was
`c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8`. The 10 October 2026 reference
snapshot is `24293a1b3616872100d379e277540bb1df89203c`. These states remain
distinct. No current performance baseline or candidate result is claimed.

## Purpose / big picture

Reduce the elapsed time of Netsuke's existing CI test gate while preserving
test and coverage fidelity and avoiding increased Ubicloud expenditure. The
observable outcome is a provenance-complete comparison of the whole paid job
and its phases, followed by an evidence-based decision for each candidate. An
inconclusive candidate stays disabled.

Issue [#894](https://github.com/leynos/netsuke/issues/894) is the approved
problem statement and scope. September 2026 timings in the issue are historical
context only; they are not today's baseline or expected savings.

## Conformance basis

The governing requirements are issue #894, its hard constraints and acceptance
checklist, and repository instructions in `AGENTS.md`. No separate Terms of
Reference or technical design exists for this investigation. The reference
snapshot is a comparison point, not authority over the current base.

Traceability is by milestone: baseline and analysis policy are EP-1; bounded
telemetry is EP-2; installation fingerprints, shared API preparation and
coverage pooling are EP-3A, EP-3B and EP-3C; decisions and follow-through are
EP-4. Each milestone preserves the required test inventory, coverage semantics,
runner shape and cost boundary.

## Constraints

- Keep the existing runner shape, job topology, Cargo build-worker limits and
  Nextest worker limits. Do not add paid jobs, shards, matrices, scheduled
  benchmarks, manual dispatches or reruns for sample growth.
- Use existing logs first and collect additional observations only through
  ordinary, already-required CI runs. Do not purge production caches or run
  control and candidate suites together in one job.
- Preserve the complete test selection and assertions, including unit,
  integration, behavioural, public-API compilation, real-Ninja, packaging,
  workflow-contract and doctest checks. Keep mutation, Kani and other
  independent workflows unchanged.
- Preserve coverage instrumentation, detailed report, exclusions, denominator,
  ratchet semantics, compiler/linker settings, toolchain pins, failure
  propagation, timeout ordering, cache-writer trust policy and storage policy.
  Do not add a `target/` archive or use reduced reports as a speedup.
- Queue or provisioning reduction is not paid compute savings without billing
  evidence. Runner-seconds on the unchanged shape are a cost proxy.
- Bound new telemetry to at most 10 seconds of execution and 10 MiB of
  uncompressed metadata per job. Collect no telemetry inside the Cargo
  watchdog. Measure upload, post-job and telemetry costs. If either bound is
  exceeded, leave that telemetry and dependent candidates disabled.
- Candidate changes remain opt-in and independently reversible until accepted.
  No default changes on the strength of an isolated microbenchmark.

## Revalidated starting state

At `c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8`, the Linux merge gate retains the
`ubicloud-standard-4-ubuntu-2404` same-repository PR lane; fork PRs use
`ubuntu-latest`. Cargo and Nextest worker limits are four. The job timeout is
90 minutes, Cargo watchdog 1,800 seconds, Nextest global timeout 780 seconds,
and per-test timeouts are 300 seconds by default and 600 seconds maximum.
`rust-toolchain.toml` pins `nightly-2026-08-23`; CI pins Nextest `0.9.133`,
Python `3.14`, Ruff `0.16.4`, ty `0.0.74` and mdtablefix `0.6.1`. These are
source pins, not substitutes for observed runtime versions in each record.
`generate-coverage` remains pinned to
`7d751eded578e48dacf43ab173d5820e6437c474`; setup-rust is pinned to
`6cec89bac47a21cf756d68d638a9a510998e57f8`.

Since the reference snapshot, the taiki installer pin changed from
`83ac0ad63c0167e6f06796fab0fce28db1bf3db0` to
`183e4297cca2404691e9380e1307288dced5c82a`; its version changed from v2.87.22
to v2.87.25. The CodeScene uploader changed from
`a5765019912a8ab6882b12db049c7cde635f3a85` to
`7d751eded578e48dacf43ab173d5820e6437c474`. The cache action, Nextest config
and API harness are byte-identical to the reference. The cache `pins_hash`
still covers all of `ci.yml` and the cache action for tools and Whitaker;
`target/` remains excluded, and only trusted main pushes write these caches.
The API harness retains eight direct-rustc fixture tests, Cargo JSON artefact
discovery, the missing-`--extern netsuke` negative control and a separate real
Cargo consumer test. `TestSupportRlib` reports elapsed tracing, not fresh vs
non-fresh artefact counts.

The source-only installation audit identifies `scripts/install-build-tools.sh`,
`scripts/build-tools-common.sh` and `tools/mold/SHA256SUMS` as indirect build
inputs. mold is pinned to `2.41.0`. The separate Pylint pass pins `4.0.9`, but
the df12 pass requests unversioned `pylint`; its lint suite is `v0.3.0`.
Whitaker's installer is `0.2.9`, with action revision
`6dea5677a84fec60ca51b07202570e3af12ffdb4`. The current Whitaker key hashes
only the root `dylint.toml`, excluding `test_support/dylint.toml`. Future
fingerprints must cover both. The installed suite revision and upstream
installer resolution remain unverified; source pins alone do not establish that
every installation input is immutable.

The original offline evidence set contains 81 job records from 21 distinct
run-attempt pairs: six main-branch coverage jobs, six main-branch cache-writer
jobs, nine same-repository PR Linux gate jobs, and 60 jobs from other cohorts.
Source revisions vary; paired main-branch workflows use the same SHA but remain
distinct cohorts. All records are observational, not controlled
baseline/candidate comparisons. The attempt summaries and eligible PR job times
are in `docs/ci-performance/README.md`. Deduplicate by `(run_id, run_attempt)`
before summing attempt-level runner-second totals, which repeat on each job
record.

A dated supplement retains five jobs from ordinary attempt `38750711833`,
completed on 15 October: 86 records across 22 attempts in total. Its Linux job
ran 3,933 tests, with different application tests and additional CI stages.
Keep it separate from the original descriptive groups; it is not a fifth
matched control. The README's supplement records provenance, identity-set
differences, native phases, resources and all five jobs' cost proxies.

The pinned coverage path runs the instrumented command
`cargo llvm-cov nextest --manifest-path Cargo.toml --workspace --all-targets
--all-features --lcov --output-path lcov.info`,
then a separate uninstrumented
`cargo test --doc --workspace --manifest-path Cargo.toml --all-features` with
coverage environment variables unset. Native logs expose the combined
test-and-coverage step, Nextest boundaries and doctest sections, but no
distinct merge or export duration. Do not infer those durations from Nextest
timestamps. The initial inventory has incomplete skipped-test identities and
missing process RSS and CPU seconds. `coverage_environment` snapshots values at
the coverage-command marker; general `environment` retains the last observed
value and neither is a complete child-process environment.

The September Nextest and report figures in the issue remain historical context
only. Candidate C is deferred because the retained data does not measure a
distinct merge/export tail. A, B and C remain deferred pending a comparable
controlled baseline; that is not a scientific rejection.

## Related fixes and scope boundaries

Revalidated on 15 October: [#732](https://github.com/leynos/netsuke/issues/732)
is closed, with completion recorded on 20 September. Preserve its nested-build
feature alignment; repeated Cargo calls are not evidence of repeated cold
builds. This investigation does not reopen that fix.

[#693](https://github.com/leynos/netsuke/issues/693) is also closed, with
completion recorded on 20 September. Its split-build-directory and Windows
fidelity question is distinct. Preserve that test's scope, split layouts and
real Cargo consumers. Its closure does not authorize weaker stand-ins here.
Independent mutation/Kani workflows, runner expansion, paid benchmark jobs,
`target/` archives and altered coverage scope remain out of scope.

## Analysis protocol

Primary outcomes are whole-job execution elapsed time, including setup,
telemetry, uploads, cleanup and post-job cache work, and summed runner-seconds
per attempt as a cost proxy. Record queue and provisioning separately. Use
verified billing data when available; otherwise say invoice expenditure is
unverified.

The useful-improvement threshold is fixed before candidate data is examined:
the larger of 60 seconds and the control cohort's interquartile range (IQR),
computed with `statistics.quantiles(method="inclusive")`. Candidate C also
requires a measured post-test merge/export tail of at least the larger of 60
seconds or 10% of whole-job time before prototyping.

Collect at least five comparable control and five candidate observations per
condition from ordinary runs. This is a floor, not proof. Report individual
observations, median, range, absolute and percentage differences, and the
Hodges-Lehmann median pairwise candidate-minus-control shift with an exact 95%
permutation-inversion interval under exchangeability only for controlled,
comparable samples. Do not make causal interval claims across source changes or
report small-sample p95. Continue collecting only through naturally occurring
runs when uncertainty prevents a decision.

Invert two-sided exact rank-sum permutation tests over candidate-minus-control
location shifts, using average ranks for ties and all label allocations. This
interval assumes an additive location-shift model as well as exchangeability;
if distribution shapes differ materially, report that limitation and withhold
adoption rather than interpreting it as a general median-difference interval.

Keep PR coverage, main-branch coverage, trusted main-branch cache-writer jobs,
and GitHub-hosted fork jobs in separate cohorts. Separate warm, cold/miss, and
fallback cache observations. Preserve run attempt, event, source SHA, tested
merge SHA, variant SHA, runner image/architecture/capacity, tool versions,
action pins, features/targets, compiler flags, profile, and concurrency limits.
Failed, cancelled, timed-out, retried, and superseded attempts stay in cost and
reliability accounting; omit cancelled and superseded attempts only from
successful-run timing comparisons. Label unmatched source, tests, toolchain,
runner, or cache as observational and name the confounding factor.

Adoption requires the one-sided 95% upper uncertainty bound for
candidate-minus-control consumption to be at most zero. No positive cost
increase is tolerated. Include migration, telemetry, storage, and transfer
costs. Do not claim paid savings from queue time or runner-seconds alone.

## Milestones

### EP-1: Revalidate and establish the baseline

Inspect current source and retained ordinary-run evidence before workflow
edits. EP-1 is offline-only: make no workflow changes and dispatch no CI runs.
Trace the pinned shared coverage action and scripts to establish actual
commands and phase boundaries. Record missing evidence as missing; do not infer
matched cache keys, test identities, per-test durations, memory or profile
counts from aggregate timings.

For each observation, retain run/job URL or ID, attempt, event, source and
tested merge SHAs, variant, relevant diff, runner and capacity, versions,
flags, features, targets, profile, and concurrency. Separate
queue/provisioning; setup/cache/install; compilation and nested Cargo; Nextest
selection, execution, and tail; profile merge/export/report/ratchet; doctests;
upload, cleanup, and post-job work; and whole-job duration. Capture sccache
statistics, cache result by family, compiler freshness, test identity/outcome
sets, profile inventory, resource samples and I/O where available.

The offline collector in `scripts/ci_test_evidence.py` parses saved attempt/job
API JSON and logs without network access. The telemetry reader accepts an
optional bounded JSON file by job ID, verifies schema and attempt identity, and
copies only allowlisted numeric `profiles`, `resources` and `nested_cargo`
fields. It emits per-job JSONL, retaining explicit nulls for missing evidence
and repeated attempt-cost totals. Parser contracts cover missing observations,
attempt separation and cohort classification.

The retained records are in `docs/ci-performance/observations.jsonl`; the
README reports all 21 attempts and current phase attribution. Raw logs remain
under `/tmp`, uncommitted. Source and cache conditions vary, so this set does
not establish a controlled baseline. Acceptance for candidate evaluation still
requires comparable ordinary-run controls and a published evidence gap list.

### EP-2: Add bounded telemetry after the phase-one review

After the offline collector and retained evidence receive phase-one review,
propose one bounded telemetry PR with default execution unchanged: Nextest
JUnit for `ci` without stored success output; raw profile/object inventory
before cleanup; `.profdata`, Rust/LLVM versions and copied JUnit;
fixed-retention uploads of inventory, JUnit, sccache JSON/text and resource
log; epoch timestamps in resource samples; Cargo download matched-key
reporting; and fixed-format per-Cargo elapsed/freshness/lock-wait lines in the
API harness. Call inventory with `always()` after coverage and before cleanup
in PR and main coverage lanes. Preserve timeouts, groups, selection, coverage
inputs, RUSTFLAGS, watchdog, and job ceiling. Do not split phases by rerunning
tests or changing cleanup, objects, or failure behaviour. Source inspection of
cargo-llvm-cov 0.9.0 shows that it loads Cargo configuration and uses the
`CARGO_TERM_VERBOSE` setting for report merge/export `Running` lines; it does
not expose a dedicated `CARGO_LLVM_COV_VERBOSE` switch.[^llvm-cov-verbose] The
v0.9.0 manifest requires `cargo-config2` `^0.1.41`, but the tag has no
committed lockfile, so the binary's resolved transitive version is unverified.
This is static source evidence, not confirmation of the CI binary's runtime
behaviour or its effective environment. Before relying on the indirect switch,
check locally whether its environment propagation affects child Cargo and fits
the telemetry budget. Until the native boundaries are validated, record merge
and export together. The source's default merge command is
`llvm-profdata merge -sparse -f <list> -o <profdata>`; externally supplied
`LLVM_PROFDATA_FLAGS` may alter it, and the actual CI invocation has not been
verified from source alone.

Account for telemetry's broad cache-key rotation and warm-up; do not purge
caches or deliberately cold-start production. Measure execution, upload bytes
and time, and post-job cost. Acceptance: contracts cover ordering, `always()`,
missing-file tolerance, timestamps, matched keys, inventory, JUnit, and
watchdog exclusion; overhead remains within budget.

### EP-3A: Evaluate candidate A, installation-input fingerprints

Inventory actual installation inputs, including indirect Makefile/config
values, versions, action/installer revisions and checksums, Python environment,
toolchain/ABI, runner compatibility, mold/build tools and Whitaker suite
inputs. Define explicit general-tool and Whitaker manifests. Preserve version
checks and trusted boundaries; keep v1 keys byte-identical and Cargo-download
keying separate.

Share key rendering between restore and save. Render v2 keys only as shadow
telemetry; do not restore or save them during evaluation. Contract tests prove
restore/save equality; correct-key invalidation for each relevant mutation;
stability under unrelated workflow edits; and completeness against installer
and Makefile pins. Retain main-only writing and per-save hit gating.

Classify counterfactual v2 hits from earlier successful trusted-main writer
keys within retention. Estimate setup and whole-job effects separately for
source-only warm paths and naturally occurring CI-edit PRs. Include main-branch
population and intervening PR misses in migration cost. If feasible, combine
with telemetry to cause one key rotation. Adopt only with whole-job benefit
past threshold, bounded cost non-increase and unchanged checks; otherwise
reject or leave shadow-only with the exact v1 rollback.

### EP-3B: Evaluate candidate B, run-scoped API preparation

Activate preparation only with `NETSUKE_PUBLIC_API_PREP=run-scoped` and
`NEXTEST_RUN_ID`. An exclusive run-ID-scoped lock lets the first selected
consumer perform existing Cargo preparation and JSON parsing; later processes
validate that run's manifest. No selected consumer means no preparation. Local
`cargo test` retains per-test preparation.

The manifest records schema, run ID, manifest path, feature arguments,
environment fingerprint, selected artefact path/size/SHA-256, dependency
artefact sizes/modification times and ordered dependency directories. Every
consumer validates provenance, digest, stats and file existence. Fail
diagnostically on preparation failure, malformed/mismatched data or missing
files; never silently rebuild or guess the newest rlib. Preserve eight test
identities and rustc invocations, response files, temporary-output isolation,
full search paths, verbose-timing negative control and the real Cargo consumer.
Keep existing nested-Cargo group membership in this first variant. Reject
Nextest setup scripts for this variant because they need experimental opt-in.

Test sharing, no consumers, failures, malformed/mismatched manifests, changed
digests, missing dependencies, split layout, fallback, and long response files
using `tests/ui/split_build_dir_cargo_messages.jsonl`. Contract-test anchored
selectors and group assignment. Enable only for same-repository PR coverage via
`ci-experiment:public-api-prep`; forbid it on fork, main-branch, and main
coverage. Measure prep count/freshness/rebuilds, prep plus fixture time,
serialized tail, whole-job time and peak memory. Reject shifted cost outside
Nextest or extra build configurations. Validate affected existing Windows/macOS
lanes before adoption. Roll back by removing the label and environment
assignment.

### EP-3C: Evaluate candidate C only after a material baseline tail

Record the actual profile pattern, raw-profile and object inventory, and
merge/export times first. If the tail is below the predeclared threshold,
publish a deferred decision and stop this milestone.

Otherwise, test a small, predeclared sequential set of bounded `%Nm` pools with
the pinned LLVM. Inspect the complete pattern because `%p` can defeat
cross-process pooling. Before CI, compare exact counters from concurrent
repeated processes in a deterministic instrumented program using
`llvm-profdata show`. Check `%m` collision avoidance, collection paths,
subprocess contribution, locking, and crashes. Do not rely on `%b`, change
linker settings or override `llvm-profdata` thread selection.

Add a tested LCOV comparator for normalized `SF`, `FN`, `FNDA`, `DA` and
`BRDA`, including uncovered entries and set/count differences. Enable pool four
only via `ci-experiment:profile-pool` on same-repository PR coverage. A second
pool size requires evidence that lock contention explains an inconclusive
result. Measure profile count/bytes, test duration including exit
work/contention, report tail and whole job. Preserve full LCOV scope and exact
deterministic counter accumulation. Roll back by removing the label/variable.
Reject unexplained fidelity differences or a shifted cost without whole-job
gain.

### EP-4: Publish decisions and follow-through

For A, B, and C separately, publish hypothesis/variant and SHAs, cohort run/job
links and attempts, cache states, effect and uncertainty, total cost proxy
including failures/migration, fidelity comparisons, resources/reliability,
confounders, decision, and rollback. Keep inconclusive candidates disabled.
Adoption requires repeatable useful improvement beyond noise, no end-to-end
regression, no increased consumption, and no loss of fidelity. Test any
accepted combination through ordinary CI before enabling it. Shared actions
retain backwards-compatible defaults and use a reviewed immutable pin.

For each adopted candidate, open an adoption PR, add an ADR, update the
developer guide, remove its experiment-only path and document rollback. Keep
lightweight telemetry for the first ten ordinary attempts in each affected
cohort after adoption, including failed and cancelled attempts; publish
follow-through evidence to the issue. Revert on fidelity, cost/resource or
benefit regression. Close #894 only after follow-through is published.

## Verification plan

Invariants: test and doctest identity sets equal control; instrumentation,
normalized detailed coverage records, denominator, and ratchet unchanged; only
one variant runs per existing job; candidate data belongs to the current
invocation and trusted cohort; and cost includes work moved between phases. The
parser preserves attempt identity and represents absent/invalid measurements as
missing. Candidate A's renderer has identical restore/save keys. Candidate B's
manifest matches the producing build before consumption. Candidate C's
deterministic control preserves counter accumulation.

Use parser and workflow-contract tests for telemetry; key mutation tests for A;
injected-builder and workflow tests for B; exact-counter and LCOV tests, plus
ordinary CI evidence for C. Run affected existing Windows/macOS lanes when a
shared harness changes. Run applicable deterministic gates before requesting
CodeRabbit review. Elapsed time alone is not correctness evidence. Missing
fidelity comparisons block adoption.

## Progress

- [x] Record scope, current base, constraints, and analysis protocol before
  candidate evaluation.
- [x] Revalidate current runner, cache, Nextest and API-harness state against
  the reference SHA and record the differences above.
- [x] Implement the offline collector and retain 21 attempts / 81 job records
  in `docs/ci-performance/observations.jsonl`, plus the dated five-job
  supplement.
- [ ] Establish a comparable controlled baseline and fill phase/inventory gaps.
- [x] Validate the collector with the full local gates on the candidate based
  on `051358ee360a97fcbebe2a836e25e106917ed845`; retain earlier failures as
  dated history rather than current gate status.
- [ ] Replay onto the latest `origin/main` target, resolve any conflicts, and
  rerun the full local gates on the exact rebased candidate.
- [x] Keep default CI execution unchanged and defer in-watchdog telemetry to the
  separately reviewed EP-2 milestone.
- [ ] Propose separate bounded telemetry only after phase-one review.
- [ ] Evaluate A, B and C independently, or record why each is
      rejected/deferred.
- [ ] Publish decisions, adoption PRs if warranted, and ordinary-run follow-up.

The retained dataset is available but observational, not a controlled baseline.
The candidates remain deferred; no candidate has been scientifically rejected
or adopted. Update progress and decisions as evidence lands.

## Surprises & discoveries

- 2026-10-15: An additional ordinary CI log exposed the next crate's doctest
  marker arriving before the previous crate's compile-fail batch finished. Two
  regressions reproduced a 90-pass/26-ignored undercount instead of the native
  129 passes and 32 ignored tests. Preserve active and pending targets through
  their respective summaries. Both regressions pass; reprocessing the original
  81 records changes none of them. The log adapter's private scope predicate
  belongs only to this native-doctest parser.

- 2026-10-15: Six focused regressions exposed boolean and floating-point job
  provenance and page counts comparing equal to integers. Reject those
  malformed fields before comparing identities or accepting a complete page.
  The focused cases and all Python gates pass after correction. Regenerating
  the 81 saved observations produces a byte-for-byte identical dataset.

- 2026-10-15: A missing cache-size line could inherit the preceding restored
  archive's bytes. The regression reproduced that attribution error; consume
  each native size once so another archive's missing size stays unknown.

- 2026-10-15: The completed local doctest pass exposed separate merged and
  compile-fail result batches under one target marker, also present in retained
  CI logs. Two regression cases reproduced a two-pass undercount. Preserve all
  batches without attributing a later unrelated harness to that target.

- 2026-10-15: The unchanged local gate retry passed the boundary fixture in
  102.301 seconds, then timed out `cli_configuration_fixture_compiles` at
  300.008 seconds: 3,896 passed, one timed out, six skipped and 15 unrun. A
  read-only snapshot found waiting build-limit wrappers but no queued
  package-cache lock in that fixture's process tree; its cause is unresolved.
  Stop Rust retries while diagnosing the real consumer and complete the
  independent, not-yet-run Python/typecheck gates sequentially.
- 2026-10-15: One delegated local Cargo-preparation probe succeeded in
  266.773 seconds, with 327 fresh and eight non-fresh artefacts plus four
  package-cache lock messages. Lock durations were not measured. This does not
  identify the earlier timeout's cause or demonstrate CI savings. Preparation
  is now complete; repeat the failed full local gate unchanged and retain both
  attempts and the diagnostic in the validation record.
- 2026-10-15: Full lint passed after the house-lint fixes, and documentation
  coverage passed at 98.87%. The local Rust gate then timed out the unchanged
  build-module boundary fixture at 300.009 seconds: 3,895 passed, one timed
  out, six skipped and 16 unrun. The original child-Cargo phase was not
  captured. A [delegated diagnostic plan][local-timeout-plan] distinguishes
  current preparation freshness from assumptions about that original failure.
  No timeout change, commit or CodeRabbit review follows a failed gate.
- 2026-10-15: Rustdoc, Clippy and Whitaker passed on resumption. The house
  Python lint gate required structural matching in two type checks and
  explanatory assertion messages. Split native-log contracts into a separate
  test module to retain the 400-line ceiling while adding those diagnostics.
- 2026-10-15: Retain six existing main-branch cache-writer attempts as a
  separate cohort, bringing the dataset to 21 attempts and 81 job records.
  Report the two-vCPU and four-vCPU cost proxies separately. No CI was
  triggered.
- 2026-10-15: Resume from the unchanged implementation base. Formatting passed
  on 12 October; the full lint gate stopped after successful Rustdoc because
  the OS could not launch the compiler wrapper (error 11, process resource
  exhaustion). No commit or CodeRabbit review was made with that failed gate.
  Preserve runner shapes separately in cost accounting before resuming gates.
- 2026-10-16: The documentation-only checkpoint at
  `55a66ca91d1989a6caa5fc8fe9a0e5e5cff8e130` was pushed in [draft PR #918].
  Evidence checkpoints were published in [the issue][issue-checkpoint-one] and
  [its follow-up][issue-checkpoint-two]. All ordinary required hosted checks
  passed on 15 October on tested merge
  `0ced8474e03ba6ec22d45abf8d81145e90d41a35`, which includes newer `main` at
  `486b876c7244c81b4fa9787a990448beebafea26` and has 78 paths changed relative
  to this PR head, including Rust, tests and Nextest configuration. That run
  reports 3,928 Nextest passes, six skips, 129 doctest passes and 32 ignored
  doctests. It is hosted validation of that tested merge, not
  source/config-equivalent proof for the local commit gate: local selection has
  3,912 tests and the unchanged full local gate still timed out the packaging
  test at 300.028 seconds, while the hosted package allowance is 600 seconds
  and that test passed in 161.125 seconds. Do not treat hosted success as
  waiving the local gate or as causal timing evidence.
- 2026-10-16: The H4 quiet-queue monitor ended without a qualifying pair: 424
  initial observations plus 76 continuation observations, ending at 16:57:23
  UTC on 15 October. It launched no diagnostic. A separate 16 October read-only
  snapshot showed a healthy four-slot pool with three slots held and sccache
  unreachable. `buildlimits-sccache.service` is enabled but inactive/dead;
  systemd reports it inactive since 12 October, `Result=success`,
  `ExecMainStatus=15` and zero restarts. These observations do not establish a
  cause for the local timeout. No service, configuration, or limit change has
  been made; explicit user approval is pending before the proposed
  `systemctl --user start buildlimits-sccache.service` recovery command. Keep
  this local environment evidence out of the CI baseline dataset.

- 2026-10-12: The retained set has 15 attempts and 51 job records. Its source
  SHAs differ, so it supports provenance and descriptive summaries but not a
  controlled candidate comparison.

## Decision log

- 2026-10-12: Keep the 10 October SHA as a historical comparison and use
  `c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8` for current-state revalidation.
- 2026-10-12: Predeclare the 60-second/control-IQR improvement threshold, five
  ordinary observations per arm, exact permutation interval, and
  10-second/10-MiB per-job telemetry ceiling with no telemetry inside the Cargo
  watchdog.
- 2026-10-12: Require candidate C to show a tail of 60 seconds or 10% of
  whole-job time, whichever is larger, before prototype work.
- 2026-10-12: Use the retained records only as observational evidence. Defer
  A, B and C pending a comparable baseline; this is not a scientific rejection.
  Keep the next bounded telemetry PR separate until phase-one review.
- 2026-10-12: Adoption requires the one-sided 95% upper uncertainty bound for
  candidate-minus-control consumption to be at most zero.

## Outcomes & retrospective

The offline collector, observation set and dated supplement form the current
code milestone in draft PR #918. The earlier implementation snapshot was based
on `origin/main` at `051358ee360a97fcbebe2a836e25e106917ed845`; it has not yet
been validated on the latest target. The collector does not change default CI
execution or add telemetry inside the Cargo watchdog. It emits schema 2 with
attempt-scoped start latency, while the published observations remain their
original schema 1 records. Nextest parsing accepts singular and plural start
and summary forms, including a start line without a skipped-count suffix.

The previous local validation passed formatting, Markdown, diagram, lint,
typecheck, documentation coverage, the full Rust suite, and workflow contracts
on base `051358ee360a97fcbebe2a836e25e106917ed845`. It passed 4,082 selected
tests with six skips and 136 doctests with 32 ignored; documentation coverage
was 98.89%. Workflow contracts passed 1,380 tests with three skips, followed by
27 passing script tests. The test run compiled without the managed sccache
service and still passed; that is local correctness evidence, not a CI timing
observation or candidate saving. These results do not validate the latest
`origin/main` target or the review corrections below.

The retained observations remain observational, not a controlled baseline. No
paid CI run was dispatched for this code milestone. A comparable baseline,
distinct coverage merge/export timings, bounded CI telemetry, candidate
evaluations and adoption follow-through remain outstanding. Candidate adoption
still requires ordinary-run evidence under the predeclared cost and fidelity
gates.

The documentation checkpoint and evidence summaries are published through draft
PR #918 and issue #894. A focused review found parser documentation and Nextest
start-line coverage gaps. The collector now recognizes the documented
skipped-binary and filter-source suffixes, preserves unknown skipped-test
counts when only binary counts are available, and resets test identities at
each new run. Regression tests cover those forms and multiple runs in one log.
A fresh review found additional runner-provenance, crashed-test, and
test-clarity cases. The current candidate includes corrections and regression
tests for them; fresh review confirmation remains pending. No CI dispatch
occurred for the offline collector milestone.

## Revision note

Revised 2026-10-20 while preparing to replay the issue commits onto the latest
`origin/main` target, `f16c31760aff94a1d91bbbbfac4d024873942ce5`. The earlier
local gate sequence and review evidence cover the previous base at
`051358ee360a97fcbebe2a836e25e106917ed845`; they do not validate the new
target. A fresh CodeRabbit review also identified that a Nextest `SKIP` record
has no duration and is excluded from the summary's executed count. The parser
now retains that identity and compares only non-skip outcomes with the executed
count. The review also identified partial `finished/selected` summary counts;
the parser now retains those counts and refuses to call a fail-fast run's
selected identity inventory complete. The latest review also caught the
private-module list, missing CLI usage, and dropped non-default Ubicloud
shapes; the guide, collector entrypoint, and runner-provenance contract now
cover those cases. Earlier hosted validation, H4 monitoring, and local
supervisor evidence remain dated history. The controlled baseline, bounded
telemetry, fresh review, and candidate decisions remain outstanding.

[local-timeout-plan]: ../debugging/debugging-plan-2026-10-15-ci-evidence-local-gate-timeout.md
[draft PR #918]: https://github.com/leynos/netsuke/pull/918
[issue-checkpoint-one]: https://github.com/leynos/netsuke/issues/894#issuecomment-6196022869
[issue-checkpoint-two]: https://github.com/leynos/netsuke/issues/894#issuecomment-6196835735

[^llvm-cov-verbose]: Static source references:
  [cargo-llvm-cov 0.9.0 CLI][llvm-cov-cli], [context][llvm-cov-context],
  [report][llvm-cov-report], [manifest][llvm-cov-manifest] and
  [cargo-config2 environment mapping][cargo-config-env].

[llvm-cov-cli]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/cli.rs
[llvm-cov-context]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/context.rs
[llvm-cov-report]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/report.rs
[llvm-cov-manifest]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/Cargo.toml
[cargo-config-env]: https://github.com/taiki-e/cargo-config2/blob/v0.1.41/src/env.rs
