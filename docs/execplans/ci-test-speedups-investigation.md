# Investigate CI test speedups with cost and fidelity gates

Status: IN PROGRESS

On 26 October, the full review of published head `da69b1b` identified missing
step-output and fractional-telemetry assertions, optional-file read errors
being hidden, and a stale publication status in this plan. The local repair
adds those contracts, propagates non-missing filesystem errors, and records the
actual publication state. The repair was published as
`8a32912012c705bdfbd246f27acc1b487df4bb14` after its sequential local gates
passed: 4,725 Rust tests, 1,508 workflow-contract tests plus 27 supplementary
script tests, typecheck, lint, and formatting; Rustdoc coverage is 98.19%.
Exact-head CI
[40353596192](https://github.com/leynos/netsuke/actions/runs/40353596192),
Netsukefile
[40353595949](https://github.com/leynos/netsuke/actions/runs/40353595949),
Release Dry Run
[40353596050](https://github.com/leynos/netsuke/actions/runs/40353596050),
CodeScene [8136311](https://codescene.io/projects/69281/delta/results/8136311),
and Gecko passed. CodeRabbit's focused reconciliation confirmed the Testing and
Unit Architecture rows; its follow-up confirmed the Developer Documentation
row. The candidate-bound EP-1 assessment for `ec53ee08` accepted the scoped
implementation. A fresh formal review of the corrected plan, final approval,
and merge remain outstanding.

On 26 October, formal CodeRabbit review `5645777679` of published head
`1ef4de4d11c4157a11bd2f573c5bf29a927dbdd7` requested changes for two missing
regressions: rejection of an unknown experiment variant and a stable schema-2
JSONL projection across variants and cohorts. The worktree adds both contracts
in `ci_test_evidence_output_test.py` and pins the projection in
`ci_test_evidence_schema2.json`, including nulls and raw-log exclusion. The
candidate passed `make check-fmt`, `make test` (4,712 passed, six skipped, and
doctests passed), `make typecheck`, `make lint`, and
`make test-workflow-contracts` (1,494 passed, three skipped, plus 27 script
tests), sequentially. These results cover the test changes before this plan
update; the final documentation candidate still needs its gates, then a
current-head hosted review and CI before merge.

After publishing `bb8aad8ad3c13863afa416af33bdcaa361db9cd5`, CodeScene result
8122706 failed its advisory code-health gate because the new output-contract
test was one 89-line method. Its follow-up keeps saved-input setup, variant
execution, and schema projection helpers private to that test module; the
developers' guide records their ownership and reuse boundary. CodeRabbit's
status for this head was `Review paused`, and its summary still named
`1ef4de4d11c4157a11bd2f573c5bf29a927dbdd7` as the last reviewed commit. This
candidate's focused CodeScene review now scores 10.0 with no findings; it needs
fresh local gates, hosted checks, and formal review after the bounded refactor.

On 25 October, the 39-commit branch was rebased from
`0a7a0d58dbc4930d12c9ae4ed1c52e7fff039458` onto `origin/main` at
`88f20d588f936b810a2f700a76ceda970451ca7f`. The replay had no conflicts, and
the range-diff matched all 39 commits. The only overlapping files were
`docs/contents.md` and `docs/developers-guide.md`; the target's ADR-0059 and
compile-contract guidance were preserved, and `Cargo.lock` came from the target.

The complete post-rebase local gates passed on code candidate
`8705f25879e47293b1175147d9decb8dcb4decb5`: `make check-fmt`, `make test`,
`make typecheck`, `make lint`, `make doc-coverage`, and
`make test-workflow-contracts`. Nextest passed 4,710 tests with six skipped;
workspace doctests passed. Workflow contracts passed 1,493 tests with three
skipped plus 27 script tests. Rustdoc coverage was 98.20% (6,221/6,335).
Published checkpoint `a4315d26c4a3ce1ad6ae3531a70bece227214426` changed only
`docs/ci-performance/README.md` and this ExecPlan after that code candidate;
the documentation-only checkpoint passed `make fmt`, `make check-fmt`,
`make markdownlint` (zero issues across 201 files), and `make nixie`.

On published checkpoint `a4315d26c4a3ce1ad6ae3531a70bece227214426`, hosted CI
run 40182895134, Netsukefile run 40182895177, Release Dry Run 40182895178, and
CodeScene all passed. The hosted coverage result was 91.09% against the 91.7%
baseline, within the configured ±1.00 percentage-point tolerance; it did not
exceed the baseline. CodeRabbit's EP-1 assessment in comment 6396445399 found
no confirmed in-scope defect or omission. The five findings from the previous
review were confirmed fixed and their threads resolved. This assessment is
separate from formal review; formal review and merge remained outstanding after
this checkpoint.

Published checkpoint `5c49f5d726ea6c940f5ecf9b2c2508aeac5f88ef` changed only
this ExecPlan after `a4315d26c4a3ce1ad6ae3531a70bece227214426`. It passed
`make fmt`, `make check-fmt`, `make markdownlint` (zero issues across 201
files), and `make nixie`. Exact-head CI run 40185149633, Netsukefile run
40185149568, and Release Dry Run 40185149614 passed on attempt 1; the release
run skipped its expected Windows smoke and publishing jobs. CodeScene passed
(result 8117285). Hosted coverage was 91.08% against the 91.7% baseline: within
the configured ±1.00 percentage-point tolerance, but below baseline. The
exact-head EP-1 assessment in comment 6396804980 found no confirmed in-scope
omission. Formal review and merge remain outstanding.

On the previously published head `189b22560816b40b352c7812938f0db44f1c12e2`,
formal CodeRabbit review `5643469912` returned `CHANGES_REQUESTED`, with 14
passed pre-merge rows and one ExecPlan warning. Its inline findings were
`4371043447` (quadratic backtracking in the Nextest result pattern) and
`4371043455` (the ambiguous-cohort test omitted matching saved PR metadata).
Repair commit `9c4e929f9f0a34ed06d984bcfb606f68c9fe2049` keeps duration
whitespace inside the optional duration group, adds an unclosed-bracket
regression case, and gives the ambiguous-association cases matching, fully
populated saved metadata. On published head
`2d31ee8788bee43828a31df6a11b90692c626335`, CodeRabbit confirmed both findings
as addressed and the threads are resolved. The old formal review still covers
only `189b2256`; a current-head formal review remains outstanding.

After remote `main` advanced to `de81d5c34ab3c0103a5de71a0518b89980187c11`, the
42-commit branch replay completed without conflicts and `git range-diff`
matched all 42 commits. On the tree recorded by repair commit `9c4e929`, the
post-rebase `make check-fmt`, `make test`, `make typecheck`, `make lint`,
`make doc-coverage`, and `make test-workflow-contracts` gates passed. Nextest
passed 4,712 tests with six skipped, and workspace doctests passed. Workflow
contracts passed 1,492 tests with three skipped plus 27 supplementary script
checks. Rustdoc coverage was 98.20% (6,221/6,335). The ExecPlan update passed
`make fmt`, `make check-fmt`, `make markdownlint` (zero issues across 201
files), and `make nixie`. On published candidate
`2d31ee8788bee43828a31df6a11b90692c626335`, CI run `40201221279`, Netsukefile
run `40201221305`, Release Dry Run `40201221407`, CodeScene result `8118709`,
and Gecko all passed on attempt 1. The CI run passed all six jobs; the release
run passed its metadata, admission, and platform artefact jobs, with its
expected native-recipe smoke and publishing jobs skipped.

The CodeRabbit summary's Developer Documentation warning was anchored to the
review of `189b2256`. Focused reconciliation comment `6399113511` supplied the
current plan and gate evidence for base `de81d5c` and head `4e84a4d`;
CodeRabbit confirmed in `6399125097` that the warning is addressed and needs no
further documentation change. On exact head `4e84a4d`, CI run `40204065517`,
Netsukefile run `40204065477`, Release Dry Run `40204065491`, CodeScene result
`8118935`, and Gecko passed. CI passed all six jobs. Release passed nine jobs
and skipped the expected Windows recipe smoke and publish jobs. The CodeRabbit
status remains `Review paused`, with one managed resume request queued. A fresh
EP-1 assessment remains pending because the prior assessment predates this
rebase. No proof was added or materially changed, so a separate proof
assessment does not apply. The current-head formal review and merge also remain
outstanding at this checkpoint.

The investigation is under way. On 24 October, the 28-commit branch was rebased
onto `origin/main` at `f8df5d808f1bee4eb8554df0160119864ade6f39`, after the
base advanced from `018b81b4311ac5dd99ac4182c51301eeca0dfc55`. The replay had
no conflicts and the range-diff matched all commits. The preceding candidate at
`018b81b` passed `make check-fmt`, `make test`, `make typecheck`, and
`make lint`, plus the workflow-contract, documentation-coverage, Markdown, and
diagram gates. `make test` passed all 4,864 Nextest tests (six skipped) and the
workspace doctests. Documentation coverage was 98.19% against the 80%
threshold; workflow contracts passed 1,458 tests (three skipped) and 27
supplementary script doctests. Markdownlint found zero issues across 200 files,
and Nixie validated every diagram. Those results predate the latest toolchain
update; the new head passed `make check-fmt`, `make lint`, and
`make doc-coverage` (98.19%). Its `make test` run passed 4,839 tests, skipped
six, and timed out `cli_configuration_fixture_compiles::case_1_public_api` at
300 seconds; 24 tests were not run and doctests were not reached. The captured
log has no underlying Cargo diagnostic, so the timeout cause is unresolved. The
same candidate passed `make typecheck` and `make test-workflow-contracts`
(1,460 passed, 3 skipped, plus 27 supplementary script tests), and
`make markdownlint` (zero issues across 200 files) and `make nixie` (all
diagrams validated). Hosted CI on published head `282d99e` also passed: the
4,864-test Nextest suite passed (six skipped, one slow), including the
previously timed-out fixture in 22.476 seconds, in 208.778 seconds overall; all
workspace doctests passed. CI reported 91.79% coverage against the 91.7%
baseline. Netsukefile Build Test and Release Dry Run passed, and local and
hosted CodeScene reported no issues. The local timeout's cause remains unknown.
The later published checkpoint `52bbd367cf8b059baa2404d6357e6dd3e2c973d9`
passed CI run 40114633754: Nextest ran 4,864 tests (all passed, six skipped) in
74.154 seconds, the workspace doctests passed, and the previously timed-out
fixture passed in 6.246 seconds. Netsukefile run 40114633693 and Release Dry
Run 40114633743 passed. CodeScene reported no issues on that checkpoint. This
hosted success still does not explain the local timeout. The controlled
baseline, bounded telemetry, and candidate decisions remain outstanding.

The exact-head provenance follow-up found that the workflow-run API does not
include the associated PR author's login. The collector now accepts a saved PR
response and joins it only when PR ID, number and base repository ID match the
single run association; missing or mismatched author data stays `other`. The
CLI and classifier regression tests cover the native API shape and mismatches.
On this staged candidate, `make check-fmt`, `make typecheck`, `make lint`, and
`make test-workflow-contracts` passed; the latter ran 1,465 tests (three
skipped) plus 27 supplementary script tests. The full Rust suite's latest
confirmed local result remains the 300-second timeout in
`cli_configuration_fixture_compiles::case_1_public_api`, after 4,839 tests
passed and six were skipped; 24 tests and doctests were not reached. A later
test invocation after rebase produced no terminal result, so it is not counted
as a pass. Revalidate hosted checks and review on the published exact head.

The first hosted CodeScene check on the provenance repair flagged the new
metadata matcher for complexity. The refactor reuses the existing
`ci_test_evidence_types.validate_identities` validator and separates the
optional run-base repository check into a module-private predicate. The
CI-performance protocol records the predicate's ownership and permitted use.
Local `cs delta origin/main` now reports no issues. On this staged Python-only
refactor, `make check-fmt`, `make typecheck`, `make lint`, and
`make test-workflow-contracts` passed; workflow contracts reported 1,472 passed
and three skipped, plus 27 supplementary script tests. The Rust-only
`make test` was not repeated because no Rust source changed; its latest
confirmed result remains the previously recorded public-API fixture timeout.
Publish this refactor and obtain fresh hosted checks before accepting review.

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

### Pre-rebase source comparison at the assessed PR head

Rechecked the source at PR base `8a961000859c02862454338fda6c4dd65bda2aa1` and
candidate `3928a0d1fff4f1d62349ef38f6ad435f55135f4a` on 23 October 2026. The
candidate does not change `.github/workflows/ci.yml` or `.config/nextest.toml`.
Compared with the historical `c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8`
description above, the current limits are:

| Limit                         | Historical source | Current source |
| ----------------------------- | ----------------: | -------------: |
| Linux job timeout             | 90 minutes        | 100 minutes    |
| Cargo watchdog per invocation | 1,800 seconds     | 1,980 seconds  |
| Nextest CI global timeout     | 780 seconds       | 960 seconds    |
| Largest per-test allowance    | 600 seconds       | 900 seconds    |

The default per-test allowance remains 300 seconds. The current-source values
are read from `ci.yml` and `.config/nextest.toml`; the largest per-test
allowance is the scoped 900-second override. These timeout changes are an
additional reason that the retained historical runs cannot serve as matched
current controls. They do not change the timing or cost accounting of those
observations.

Since the 10 October reference at `24293a1b3616872100d379e277540bb1df89203c`,
the taiki installer pin changed from `83ac0ad63c0167e6f06796fab0fce28db1bf3db0`
(v2.87.22) to `ab6895351a88b908e96a4697860d4ffea5e57dbe` (v2.87.27). The
`generate-coverage` pin remains `7d751eded578e48dacf43ab173d5820e6437c474`, and
the Linux gate cache action remains byte-identical to that reference. The
current `.config/nextest.toml` and `tests/command_env_ui_tests.rs` are not
byte-identical: Nextest filters, groups and timeouts have changed, and the
public-API harness now uses Cargo's `check` metadata artefact followed by
direct rustc fixtures. In particular, `cli_configuration_fixture_compiles` no
longer creates and checks a real Cargo consumer project; it is now a
direct-rustc positive/negative fixture. This change is already present at the
PR base and is outside this EP-1 diff. The issue's fidelity requirement remains
binding: candidate B must not treat this direct-rustc fixture as a substitute
for a real Cargo consumer, and that boundary needs a fidelity-preserving
resolution before B is evaluated.

The cache `pins_hash` still covers all of `ci.yml` and the cache action for
tools and Whitaker; `target/` remains excluded, and only trusted main pushes
write these caches. The current harness retains separately identifiable
public-API fixture cases, Cargo JSON artefact discovery and the missing
`--extern netsuke` negative control. `TestSupportRlib` reports elapsed tracing,
not fresh vs non-fresh artefact counts.

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

### Current target source at the rebased base

The current base `88f20d588f936b810a2f700a76ceda970451ca7f` includes the
workflow-contract changes from pull request 932, PowerShell-placeholder
hardening from pull request 924, the nightly toolchain update from pull request
947, the build-footprint settings from pull request 948, and the
default-features Dependabot runner placement from pull request 949. It pins
`nightly-2026-10-22`, disables development incremental compilation and debug
data, and sets Cargo's `embed-metadata = false`; its contract tests cover those
settings. It also starts coverage baseline v2. The eligible same-repository
Linux PR gate remains on `ubicloud-standard-4-ubuntu-2404`, with four Cargo
workers and four Nextest workers. In both the main and default-features lanes,
fork and Dependabot pull requests use `ubuntu-latest`; runner-contract tests
cover the exact job assignments. Pull request 942 moves public-API compile
contracts into a workspace fixture and compile-fail doctests, and moves package
verification to `make test-package-verify`. These changes alter the timed test
set and compile boundaries relative to the retained observations. The workflow
checks the PR author's `user.login` for Dependabot because the actor can change
when another person pushes to that PR. A workflow-run response does not include
the PR author, so the offline collector accepts a separately saved pull request
response. It uses that author only when the PR ID, number and base repository
ID match the run's single PR association. If the saved PR response is absent or
does not match, author-dependent classification remains `other`; the collector
never substitutes the run actor. Fork classification continues to require
distinct base and head repository identities. The run association fields are
documented in GitHub's [Get a workflow run response][workflow-run-response],
and PR authors in its [Get a pull request response][pull-request-response].

The current base retains pull request 932's workflow-contract changes and
documentation. Pull request 924's change is unrelated to the CI source
comparison. Pull request 947 changes the compiler toolchain and coverage
baseline, but reports no test-selection changes. Pull request 948 changes the
development build profile and adds build-footprint contract tests; it does not
change CI test selection. Pull request 949 changes the default-features runner
placement for Dependabot and adds runner-contract coverage; it does not change
test selection. Pull request 942 changes compile-contract placement and moves
package verification out of the timed suite. No candidate optimization has been
enabled. The original 10 October reference and the earlier 8a961 comparison
above remain historical snapshots, not the current source baseline.

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
and GitHub-hosted fork and Dependabot jobs in separate cohorts. Separate warm,
cold/miss, and fallback cache observations. Preserve run attempt, event, source
SHA, tested merge SHA, variant SHA, runner image/architecture/capacity, tool
versions, action pins, features/targets, compiler flags, profile, and
concurrency limits. Failed, cancelled, timed-out, retried, and superseded
attempts stay in cost and reliability accounting; omit cancelled and superseded
attempts only from successful-run timing comparisons. Label unmatched source,
tests, toolchain, runner, or cache as observational and name the confounding
factor.

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
the pinned cargo-llvm-cov 0.9.0 CLI and shared action found no verified
environment switch for exposing separate report merge/export
timings.[^llvm-cov-verbose] The indirect `CARGO_TERM_VERBOSE` configuration
path is not enough to establish that the pinned action exposes those
boundaries. Keep merge and export combined unless a local check against the
pinned tool proves otherwise. The source's default merge command is
`llvm-profdata merge -sparse -f <list> -o <profdata>`; externally supplied
`LLVM_PROFDATA_FLAGS` may alter it, and the actual CI invocation remains to be
captured.

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
- [x] Replay from historical target
  `6572fb956379f47e70af914bfe4307b740a0b15e` onto current target
  `8a961000859c02862454338fda6c4dd65bda2aa1`; no conflicts occurred and the
  full branch range-diff matched all 21 commits. This is historical; the 24
  October replay is recorded below.
- [x] On the previous corrected candidate, pass `make check-fmt`, `make test`,
  `make typecheck`, `make lint`, `make test-workflow-contracts`,
  `make doc-coverage`, and `make markdownlint`; those results remain history,
  not current-head gate results.
- [x] Resolve the exact-head parser and provenance assessment findings,
  including generated properties for retry and post-summary isolation.
- [x] Rebase the 27-commit issue series from the initial
  `596cf74c` target onto current `origin/main` at `018b81b`; the range-diff
  matched and no conflicts occurred. This is the preceding candidate;
  revalidate the Dependabot hosted-runner cohort against saved PR-author and
  repository data on the current base.
- [x] Complete the separate doctest pass with the existing selection and
  ignore policy.
- [x] Confirm `make nixie` exits successfully on the final documentation
  revision; the follow-up gate returned exit 0 and validated every diagram.
- [x] Reconcile the offline assessment by adding a current-source timeout
  comparison and documenting the legacy duplicate identity without
  reclassifying it as a retry.
- [x] Add generated parser properties for retry and post-summary isolation,
  and malformed inventory values, within a fixed 80-example budget per case.
- [x] Keep default CI execution unchanged and defer in-watchdog telemetry to the
  separately reviewed EP-2 milestone.
- [x] On the candidate based on `018b81b`, pass `make check-fmt`,
  `make test-workflow-contracts` (1,458 passed, 3 skipped, plus 27
  supplementary script doctests), and `make typecheck`.
- [x] Resolve the Ruff boolean-expression finding and pass `make lint` on the
  corrected candidate, including Clippy, both Whitaker checks, Python lint,
  yamllint, and actionlint.
- [x] Pass `make doc-coverage` at 98.19%, above
  the 80% threshold.
- [x] Pass `make markdownlint` with zero issues across 200 Markdown files and
  `make nixie` with all diagrams validated after the final plan edit.
- [x] Pass local `make test`: 4,864 Nextest tests passed, six were skipped,
  and all workspace doctests passed. The previously slow public-API and
  locale-stub compile fixtures completed within their configured timeouts.
- [x] Rebase all 28 commits onto `origin/main` at `f8df5d8`; the range-diff
  matched and no conflicts occurred.
- [x] After the nightly toolchain update, pass `make check-fmt`, `make lint`,
  and `make doc-coverage` (98.19%).
- [x] Re-run the local and hosted suites after the earlier
  `cli_configuration_fixture_compiles::case_1_public_api` timeout. Both later
  suites passed, including that fixture; the cause of the isolated 300-second
  timeout remains unestablished, and no source failure was reproduced.
- [x] Pass `make typecheck` and `make test-workflow-contracts` on the new base
  (1,460 passed, 3 skipped, plus 27 supplementary script tests).
- [x] Validate final documentation with `make check-fmt`, `make markdownlint`
  (zero issues across 200 files), and `make nixie` after this plan update.
- [x] Confirm hosted CI run 40112221490, Netsukefile run 40112221417, Release
  Dry Run 40112221464, and CodeScene passed on published head `282d99e`.
- [x] Confirm hosted CI run 40114633754, Netsukefile run 40114633693, Release
  Dry Run 40114633743, and CodeScene passed on published head `52bbd36`. The
  4,864-test Nextest suite passed with six skips, workspace doctests passed, and
  `cli_configuration_fixture_compiles::case_1_public_api` passed in 6.246
  seconds. This does not explain the earlier local timeout.
- [x] Use a separately saved pull request response to classify Dependabot only
  after matching PR ID, number and base repository ID to the workflow run;
  cover the native API shape, missing metadata and mismatches.
- [x] Pass `make check-fmt`, `make typecheck`, `make lint`, and
  `make test-workflow-contracts` on the staged provenance repair. The
  workflow-contract suite passed 1,465 tests (three skipped) plus 27
  supplementary script tests.
- [x] Resolve the hosted CodeScene complexity finding without a suppression by
  reusing the positive-ID validator and extracting the optional run-base
  predicate; add missing, malformed and mismatched base regressions and record
  its scope in the CI-performance protocol.
- [x] Pass `make check-fmt`, `make typecheck`, `make lint`, and
  `make test-workflow-contracts` on the refactor (1,472 passed, 3 skipped, plus
  27 supplementary script tests); local `cs delta origin/main` reports no
  issues.
- [x] Obtain a full CodeRabbit review on published head `f7368b9`; it found
  five actionable documentation and Python findings, resolved below.
- [x] Resolve the five findings in commits `68c1ed184`, `47dca9a05`, and
  `fac6db1d6`: clarify historical source comparisons and Make instructions,
  correct pull-request API provenance, validate jobs pages before extraction,
  and preserve list-only PR identity matching with structural patterns. The
  functional fix and structural refactor each passed all six local gates.
- [x] Rebase the 39-commit candidate onto `origin/main` at `88f20d5`; no
  conflicts occurred and the range-diff matched all 39 commits. On the rebased
  candidate, all six local gates passed: Nextest passed 4,710 tests with six
  skipped, doctests passed, workflow contracts passed 1,493 tests with three
  skipped plus 27 script tests, and Rustdoc coverage was 98.20%.
- [x] Revalidate hosted checks and CodeScene on checkpoint `a4315d2`; all three
  Actions runs passed. Obtain the EP-1 assessment on that exact base/head and
  reconcile the five previous findings; the assessment accepted the scoped
  milestone and all five threads were resolved.
- [x] Validate plan-only head `5c49f5d` with `make fmt`, `make check-fmt`,
  `make markdownlint` (zero issues across 201 files), and `make nixie`. Exact-
  head CI, Netsukefile, Release Dry Run, and CodeScene passed; the EP-1
  assessment found no confirmed in-scope omission. Coverage was 91.08% against
  the 91.7% baseline, within tolerance and not above baseline.
- [x] Rebase the 42-commit candidate onto remote `main` at `de81d5c`; no
  conflicts occurred and the range-diff matched all 42 commits.
- [x] Fix the two inline findings from review `5643469912` in
  `9c4e929f9`; pass `make check-fmt`, `make test`, `make typecheck`,
  `make lint`, `make doc-coverage`, and `make test-workflow-contracts`. Nextest
  passed 4,712 tests with six skipped, workflow contracts passed 1,492 with
  three skipped plus 27 script checks, and documentation coverage was 98.20%
  (6,221/6,335).
- [x] Publish repair candidate `2d31ee8`; its exact-head CI, Netsukefile,
  Release Dry Run, CodeScene, and Gecko checks passed. Reply to the two inline
  findings; CodeRabbit confirmed both repairs and the threads are resolved.
- [x] Validate plan-only head `3229ccf` with `make check-fmt`,
  `make markdownlint` (203 files, zero issues), and `make nixie`; exact-head
  hosted checks and the full CodeRabbit review completed on successor `222ba09`
  with all 15 pre-merge rows passing.
- [x] Resolve the three findings from the full review of `222ba09`: clarify
  that `trunk` is a prefix, apply the Oxford commas requested by the style
  guide, and bound JSON sccache counters to 18 digits with a 19-digit
  regression. Publish the repair as `da69b1b`; its exact-head CI, Netsukefile,
  Release Dry Run, CodeScene, and Gecko checks passed. The full CodeRabbit
  review completed on `da69b1b` and reported two Errors (missing step and
  fractional-telemetry assertions; optional file reads can hide filesystem
  errors) and one Developer Documentation Warning.
- [x] Receive the EP-1 completeness/correctness assessment for `da69b1b` in
  comment `6415566707` (request `6415539638`); it found no confirmed in-scope
  omission. No proof assessment applies.
- [x] Repair all current pre-merge rows from review `5654165349`: add contract
  tests for populated step output, optional-file I/O failures, and fractional
  telemetry, and update this plan. The sequential gates pass on the repair
  candidate: 4,725 Rust tests passed with seven skipped and doctests passed;
  typecheck and lint passed; workflow contracts passed 1,508 tests with three
  skipped plus 27 supplementary script tests; doc coverage is 98.19% (6,231/
  6,346); formatting, Markdownlint (203 files), Nixie, and CodeScene passed.
- [x] Commit and publish the validated repair as `8a329120`. Its exact-head
  CI, Netsukefile, Release Dry Run, CodeScene, and Gecko checks passed.
- [x] Complete the Developer Documentation row reconciliation and obtain the
  candidate-bound EP-1 assessment for `ec53ee08`; it accepted the scoped
  implementation.
- [ ] Complete a fresh managed formal review of this plan correction, resolve
  any findings, then request final approval and merge.
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

- 2026-10-23: Rechecking the 10 October reference against current `origin/main`
  found that `.config/nextest.toml` and `tests/command_env_ui_tests.rs` have
  changed. The current CLI configuration fixture is direct-rustc, not a real
  Cargo consumer project. This is already in the target branch, not this PR.
  Keep the issue's real-consumer fidelity requirement: candidate B cannot use
  that fixture as a substitute and must resolve the boundary before evaluation.

- 2026-10-23: The completeness assessment found that a second Nextest summary
  could overwrite counts and duration after the run closed. Freeze the parser
  at the first summary until another start marker, and keep native phase
  timestamps aligned with that boundary.
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
code milestone in PR #918. The collector does not change default CI execution
or add telemetry inside the Cargo watchdog. It emits schema 2 with
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
observation or candidate saving.

On the 20 October candidate based on
`37fc364a3caf6845dc4550666dea779d8b9328e2`, formatting, lint, typecheck,
documentation coverage, workflow contracts, Markdown lint, and Mermaid
validation passed. Workflow contracts passed 1,407 tests with three skips,
followed by 27 passing script tests. Documentation coverage was 98.92%.
`make test` passed 4,275 tests and skipped six, but one nested Cargo fixture
timed out after waiting on the shared package-cache lock for 900 seconds; 24
tests did not run. This remains dated history and was not treated as a green
full local Rust suite.

On the 23 October candidate replayed onto
`6572fb956379f47e70af914bfe4307b740a0b15e`, the final serial local gate
sequence passed. `make test` completed all 4,658 Nextest cases (six configured
skips) and the separate doctest pass. Typecheck passed for Python and Rust;
lint passed Rustdoc, Clippy, both Whitaker suites, Ruff, Pylint, df12 lints,
ambrleaks, Interrogate, yamllint, and actionlint. Workflow contracts passed
1,424 tests with three skips plus 27 supplemental tests. Documentation coverage
was 98.13%, and Markdown lint found no issues across 199 files. A
documentation-only follow-up returned exit 0 for `make check-fmt`,
`make markdownlint`, and `make nixie`; every diagram validated. The added
inventory tests preserve malformed runtime identity cases through a type-only
cast and enumerate the fixed evidence sections explicitly. These are local
correctness results, not CI timing or cost evidence.

The retained observations remain observational, not a controlled baseline. No
paid CI run was dispatched for this code milestone. A comparable baseline,
distinct coverage merge/export timings, bounded CI telemetry, candidate
evaluations and adoption follow-through remain outstanding. Candidate adoption
still requires ordinary-run evidence under the predeclared cost and fidelity
gates.

The documentation checkpoint and evidence summaries are published through pull
request 918 and issue #894. A focused review found parser documentation and
Nextest start-line coverage gaps. The collector now recognizes the documented
skipped-binary and filter-source suffixes, preserves unknown skipped-test
counts when only binary counts are available, and resets test identities at
each new run. Regression tests cover those forms and multiple runs in one log.
A fresh review found additional runner-provenance, crashed-test, and
test-clarity cases. The candidate includes corrections and regression tests for
them. Confirmation of that parser pass was pending at this checkpoint; a later
review on 23 October is recorded below. No CI dispatch occurred for the offline
collector milestone.

The exact-head completeness assessment on
`3928a0d1fff4f1d62349ef38f6ad435f55135f4a` confirmed the Task 1 scope and
identified a late-summary parser defect plus an outdated current-source
comparison. The worktree now freezes parsed summary fields and phase timestamps
until a new Nextest start, with regressions for both paths. The plan now
records the target-branch change to the public-API harness and keeps the real
Cargo consumer requirement active for candidate B. These repairs passed the
post-rebase workflow-contract and full test gates.

At this checkpoint, repair commit `9c4e929f9f0a34ed06d984bcfb606f68c9fe2049` is
the local code candidate rebased onto `de81d5c`. Its full serial validation
passed: check-fmt, test, typecheck, lint, doc-coverage, and workflow-contract
gates. These are local correctness results, not hosted validation or CI timing
evidence. The PR's published head is still `189b2256`, whose formal review
requested changes. The repairs are on its successor; hosted checks, a fresh
review, thread replies and resolution, pre-merge reconciliation, and merge
remain outstanding.

## Revision history

On 23 October, the full branch range from prior target
`6572fb956379f47e70af914bfe4307b740a0b15e` and head
`b2a5117e495be9149b63c2a2fa054be2c49facdb` was replayed onto the then-current
PR target `origin/main` at `8a961000859c02862454338fda6c4dd65bda2aa1`. No
conflicts occurred; `range-diff` matched all 21 commits. The prior target is
historical, not the current PR base.

An earlier replay note called its issue series ten commits. Rechecking the
saved ranges shows seven old issue commits, five unchanged and two with
documentation-only differences; four additional issue commits were present
after that replay, for eleven issue commits in total. The earlier ten-commit
count and its eight-identical/two-different summary were inaccurate and are
superseded by this range accounting. The current target's `Cargo.lock` is
preserved; the issue series does not change it. Five local review fixes were
restored from a verified stash. The first rebased typecheck found three typing
errors in the inventory contract test; its runtime cases are unchanged and the
static checks now pass. The corrected candidate passed the final serial gate
sequence; documentation-only follow-ups captured successful format, Markdown,
and diagram gate exits.

The 20 October revision replayed onto the then-current PR target `origin/main`
at `37fc364a3caf6845dc4550666dea779d8b9328e2`. The earlier local gate sequence
and review evidence cover the previous base at
`051358ee360a97fcbebe2a836e25e106917ed845`; the post-rebase results are
recorded above. A fresh CodeRabbit review also identified that a Nextest `SKIP`
record has no duration and is excluded from the summary's executed count. The
parser now retains that identity and compares only non-skip outcomes with the
executed count. The review also identified partial `finished/selected` summary
counts; the parser now retains those counts and refuses to call a fail-fast
run's selected identity inventory complete. The latest review also caught the
private-module list, missing CLI usage, and dropped non-default Ubicloud
shapes; the guide, collector entrypoint, and runner-provenance contract now
cover those cases. The 20 October hosted PR run passed its Linux coverage,
Windows, and release checks, but the CodeScene check failed on collector
complexity. CodeScene CLI 1.0.52 reproduced those findings against the current
base; the collector has since been decomposed by parsing and validation
responsibility, with no rule suppressions. Its current local delta reports no
new findings, and the four added helper modules each score 10.0. The 20 October
candidate's full Rust suite hit the documented shared-lock timeout. The 23
October rebased candidate passed the complete local gate sequence, as recorded
above. Ordinary PR workflows passed on head
`99522c517c13e0414370df408f2648ee0bb7312a`. A subsequent CodeRabbit review on
`4a38dc13f961ddeef915c4d4b0c2a5ab2fee1e83` identified stale branch-state prose;
the current-source comparison, range accounting, retained-record caveat, and
Python pattern-match validation were added in response. Earlier hosted
validation, H4 monitoring, and local supervisor evidence remain dated history.
The first 24 October rebase advanced the target to
`596cf74c2850c1d2088f5822828dfe556aad717e`. The run response supplies the PR
association and base/head repository objects, but not the PR author's login. A
separately saved PR response supplies `user.login` for identifying the
Dependabot hosted lane; the collector leaves malformed or incomplete provenance
in `other`. The first post-rebase `make lint` found an overlong boolean
condition in the repository-ID predicate; extracting the ID check preserved the
exclusion of booleans and passed the rerun. The final workflow-contract run
includes the hosted-cohort, late-summary, and phase boundary regressions. On 24
October, `main` advanced to `018b81b4311ac5dd99ac4182c51301eeca0dfc55`, and the
branch was rebased again. All 28 commits matched the prior range and no
conflicts occurred. That candidate passed the complete local gate sequence:
format, lint, typecheck, documentation coverage at 98.19%, workflow contracts
(1,458 passed, 3 skipped, plus 27 script doctests), the full Rust suite (4,864
passed, six skipped, doctests passed), Markdownlint, and Nixie. Hosted CI also
passed on that head. The base then advanced to
`f8df5d808f1bee4eb8554df0160119864ade6f39` with pull request 947's nightly
toolchain update. The 28-commit replay onto this target had no conflicts and
matched the range-diff. On the new candidate, `make check-fmt`, `make lint`, and
`make doc-coverage` passed; coverage is 98.19%. `make test` passed 4,839 tests
and skipped six before `cli_configuration_fixture_compiles::case_1_public_api`
timed out at 300 seconds. Twenty-four tests were not run, doctests did not
start, and the log contains no diagnostic from the fixture's child Cargo
command. Typecheck and workflow contracts passed (1,460 passed, 3 skipped, plus
27 supplementary script tests), as did Markdownlint (zero issues across 200
files) and Nixie. Exact-head hosted CI run 40112221490 later passed 4,864 tests
(six skipped, one slow) in 208.778 seconds, including the timed-out fixture in
22.476 seconds; workspace doctests passed. CI reported 91.79% coverage against
the 91.7% baseline. Netsukefile run 40112221417 and Release Dry Run 40112221464
also passed, and local and hosted CodeScene found no issues. On the later
published checkpoint `52bbd36`, CI 40114633754 passed all 4,864 Nextest tests
with six skips and the workspace doctests; the previously timed-out fixture
passed in 6.246 seconds. Netsukefile 40114633693, Release Dry Run 40114633743,
and CodeScene also passed. This evidence does not establish why the local
fixture timed out. The controlled baseline, bounded telemetry, and candidate
decisions remain outstanding.

The follow-up assessment identified a provenance gap: the workflow-run API
association omits the PR author's login. The current collector reads author
metadata only from a separately saved PR response whose ID, number and base
repository match the run association. Classifier tests cover each mismatch, and
a CLI test exercises the saved-response path. On this candidate,
`make check-fmt`, `make typecheck`, `make lint`, and
`make test-workflow-contracts` passed; the workflow-contract suite reported
1,465 passed, three skipped, and 27 supplementary script tests passed. The
confirmed local Rust suite result remains 4,839 passed and six skipped before
the public API compile fixture timed out at 300 seconds; 24 tests and doctests
were not reached. A later post-rebase invocation has no terminal result and is
not counted as validation. Hosted CI and review must be refreshed for the
published candidate before merge.

The subsequent CodeScene advisory on the PR-author matcher identified
`Complex Method` and overall code complexity at
`_pull_request_metadata_matches`. The refactor reuses
`ci_test_evidence_types.validate_identities` and confines optional base
repository matching to `_run_association_base_matches_repository`, composed
only by the author-metadata join. `docs/ci-performance/README.md` records that
scope and preserves separate fork classification. The added regression cases
cover omitted, null, malformed, boolean and conflicting run-base data. Local
`cs delta origin/main` reports no issues. `make check-fmt`, `make typecheck`,
`make lint`, and `make test-workflow-contracts` passed on the staged code;
workflow contracts passed 1,472 and skipped three, plus 27 supplementary script
tests passed. The Rust-only full suite was not rerun for that staged refactor
because it changed only Python and Markdown. The subsequent rebase validation
below ran the full suite and passed; the public-API compile fixture completed
in 287.593 seconds, after earlier candidates had timed out at 300 seconds.
Hosted validation and review still need to cover the published refactor commit.

The 25 October rebase moved the 34-commit branch onto `origin/main` at
`0a7a0d58dbc4930d12c9ae4ed1c52e7fff039458`, which adds pull request 949's
default-features Dependabot runner placement and runner-contract tests. No
conflicts occurred, and the range-diff matched all 34 commits. On candidate head
`bc3e52e5cc3601484f1e7c84643d3a60eb527082`, `make check-fmt`, `make test`,
`make typecheck`, `make lint`, `make test-workflow-contracts`, and
`make doc-coverage` all passed. Nextest passed 4,867 tests, with six skipped;
workspace doctests passed. Workflow contracts passed 1,472 tests, skipped
three, and ran 27 supplementary script tests. Documentation coverage was 98.19%
(6,178/6,292 items). The slow CLI configuration fixture passed in 287.593
seconds, and all three split-build fixtures passed. `cs delta origin/main HEAD`
with CodeScene CLI 1.0.52 found no issues. A full CodeRabbit review of
published head `f7368b9` found five actionable documentation and Python
findings. The documentation corrections are in `68c1ed184`; the malformed jobs
response fix is in `47dca9a05`; and the separate structural-pattern refactor is
in `fac6db1d6`. Both code commits passed `make check-fmt`, `make test`,
`make typecheck`, `make lint`, `make doc-coverage`, and
`make test-workflow-contracts`. On the refactor candidate, Nextest passed 4,867
tests with six skipped, workspace doctests passed, workflow contracts passed
1,478 tests with three skipped plus 27 script tests, and Rustdoc coverage was
98.19% (6,178/6,292). Fresh hosted checks and review must cover the published
head before merge.

After PR #942 advanced `main` to `88f20d588f936b810a2f700a76ceda970451ca7f`,
the 39-commit candidate previously based at
`0a7a0d58dbc4930d12c9ae4ed1c52e7fff039458` was rebased onto the new target. No
conflicts occurred, and `git range-diff` matched all 39 commits. The only
overlapping files were `docs/contents.md` and `docs/developers-guide.md`; the
target's ADR-0059 and compile-contract guidance were preserved alongside this
branch's CI evidence documentation. `Cargo.lock` came from the target. The
post-rebase local gates passed: `make check-fmt`, `make test`, `make typecheck`,
`make lint`, `make doc-coverage`, and `make test-workflow-contracts`. Nextest
passed 4,710 tests with six skipped; doctests passed. Workflow contracts passed
1,493 tests with three skipped plus 27 script tests, and Rustdoc coverage was
98.20% (6,221/6,335).

The plan-only successor at `5c49f5d` passed the Markdown and diagram gates and
all exact-head hosted checks. Its current-head EP-1 assessment found no
confirmed in-scope omission. The observed 91.08% hosted coverage is within the
91.7% baseline tolerance but does not exceed the baseline. Formal review,
finding reconciliation, and merge remain open work.

Formal CodeRabbit review `5646916239` of published head `7cf6867` then found
three parser robustness gaps: sccache JSON integer and nesting failures could
abort collection, and native count and size fields accepted unbounded decimal
runs. Commit `638971ef` catches those JSON decoding failures and bounds every
converted count and size, with regressions for oversized values and deep JSON.
The candidate passed `make check-fmt`, `make test` (4,712 passed, six skipped,
doctests passed), `make typecheck`, `make lint`, `make doc-coverage` (98.20%),
and `make test-workflow-contracts` (1,498 passed, three skipped, plus 27 script
tests). At that stage, the candidate still needed publication, bot-thread
reconciliation, fresh exact-head review and hosted checks before the guarded
merge.

Remote `main` advanced from `de81d5c` to `0a28fee` before publication. The
50-commit branch range was rebased onto that target with Git's built-in
`zdiff3` merge; the user-wide Weave attribute was disabled for the replay.
There were no conflicts. `git range-diff` matched all 50 commits, the 31
branch-owned paths are unchanged, all 22 target-only paths remain
byte-identical, and `Cargo.lock` matches the target. The rebase candidate was
published with an exact force-with-lease; PR #918 now targets `main` at
`0a28fee` and its published head is `3229ccf`.

All post-rebase local gates passed on code head `6e109c6`: `make check-fmt`,
`make test` (4,725 passed, seven skipped, doctests passed), `make typecheck`,
`make lint`, `make doc-coverage` (98.19%, 6,231/6,346), and
`make test-workflow-contracts` (1,498 passed, three skipped, plus 27 script
tests). The subsequent plan-only head `3229ccf` passed `make check-fmt`,
`make markdownlint` (203 files, zero issues), and `make nixie`.

The hosted CI run `40266573227`, Netsukefile run `40266573252`, Release Dry Run
`40266573521`, CodeScene result `8124723`, and Gecko checks all passed on head
`3229ccf`. This includes the Linux and Windows test jobs and Windows Whitaker
lint. Three CodeRabbit findings from review `5646916239` on superseded head
`7cf6867` were repaired in `eb0c1cf`. Candidate-bound replies were posted to
each original thread; CodeRabbit confirmed all three repairs and resolved all
three threads. Its current-head status then read `Review paused`, while the
aggregate review decision remained `CHANGES_REQUESTED` from the superseded
review. Plan-only head `222ba09` subsequently passed its documentation gates
and all exact-head hosted checks. CodeRabbit completed a full review on that
head, with all 15 pre-merge rows passing and three new findings: the `trunk`
prefix wording, four Oxford-comma corrections, and a missing upper bound on
JSON sccache counters. Commit `da69b1b` published those repairs and a 19-digit
counter regression; its hosted CI and the eight sequential local gates passed,
and `cs delta origin/main` found no issues. The completed EP-1 assessment found
no confirmed in-scope omission. A further CodeRabbit review on `da69b1b` found
missing populated-step and fractional-telemetry assertions, optional-file I/O
tests, and requested that this plan reflect the published state. Commit
`8a329120` repairs the tests and readers; the focused reconciliation confirmed
the Testing and Unit Architecture rows and its follow-up confirmed the
Developer Documentation row. The candidate-bound EP-1 assessment for `ec53ee08`
accepted the scoped implementation. The latest formal review found the
plan-status correction recorded above; a fresh review of that correction, final
approval, and merge remain outstanding.

The published head `1339fd73` passed CI, Netsukefile, and Release Dry Run, but
CodeScene rejected the new job-field validation module at mean cyclomatic
complexity 4.22 across nine functions. The local follow-up separated the steps
and labels checks into private helpers and documented their module-local scope
in the CI performance guide. `cs delta origin/main` then reported no issues.

Follow-up head `0ada0961` was published on base `0a28fee`. Its sequential local
gates passed: `make check-fmt`, `make test` (4,725 passed, seven skipped, and
doctests passed), `make typecheck`, `make lint`, `make test-workflow-contracts`
(1,519 passed, three skipped, plus 27 script tests), `make doc-coverage`
(98.19%, 6,231/6,346), `make markdownlint` (203 files, zero issues), and
`make nixie`. Exact-head CI, Netsukefile, Release Dry Run, CodeScene, and Gecko
checks passed.

CodeRabbit review `5661877547` on `0ada0961` requested structural `match`
patterns for both job-container checks; the existing workflow-contract tests
cover malformed values and error messages. The repair replaces both
`isinstance()` checks with `list()` patterns and preserves their existing
errors and the per-step object guard. It passed the sequential local gates:
`make check-fmt`, `make test` (4,725 passed, seven skipped, doctests passed),
`make typecheck`, `make lint`, `make doc-coverage` (98.19%),
`make test-workflow-contracts` (1,519 passed, three skipped, plus 27 script
tests), `make markdownlint` (203 files, zero issues), and `make nixie`. The
semantic diff found only the two intended files, and `cs delta origin/main`
found no issues. This repair was published as `40f4ef41` on base `0a28fee`; its
CI, Netsukefile, Release Dry Run, CodeScene, and Gecko checks passed. The PR
title now has the recommended `Plan:` prefix, and CodeRabbit confirmed that the
structural matching repair leaves no defect.

The `40f4ef41` walkthrough also warned that the step snapshot included
timestamps and elapsed seconds. Commit `2af03115` projects those fields out of
the schema snapshot while retaining focused assertions for the fixed start
timestamp, completion timestamp, and elapsed duration. Invalid-time cases and
the raw-log marker assertion remain covered. All eight sequential local gates
passed on these test changes: `make check-fmt`, `make test` (4,725 passed,
seven skipped, doctests passed), `make typecheck`, `make lint`,
`make doc-coverage` (98.19%), `make test-workflow-contracts` (1,519 passed,
three skipped, plus 27 auxiliary tests), `make markdownlint` (203 files, zero
issues), and `make nixie`.

In comment `6433548305`, CodeRabbit confirmed the warning resolved on exact head
`2af03115`, verified the snapshot projection and separate assertions, and
withdrew the claim that literal fixture timestamps established nondeterminism.
It did not rerun tests or hosted checks, and its response is not PR approval.
The current formal review and approval, exact-head hosted checks, and guarded
merge remain outstanding.

[local-timeout-plan]: ../debugging/debugging-plan-2026-10-15-ci-evidence-local-gate-timeout.md
[draft PR #918]: https://github.com/leynos/netsuke/pull/918
[issue-checkpoint-one]: https://github.com/leynos/netsuke/issues/894#issuecomment-6196022869
[issue-checkpoint-two]: https://github.com/leynos/netsuke/issues/894#issuecomment-6196835735

[^llvm-cov-verbose]: Static source references:
  [cargo-llvm-cov 0.9.0 CLI][llvm-cov-cli], [context][llvm-cov-context],
  [report][llvm-cov-report], [manifest][llvm-cov-manifest], and the
  [cargo-config2 environment mapping][cargo-config-env].

[llvm-cov-cli]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/cli.rs
[llvm-cov-context]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/context.rs
[llvm-cov-report]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/src/report.rs
[llvm-cov-manifest]: https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.0/Cargo.toml
[cargo-config-env]: https://github.com/taiki-e/cargo-config2/blob/v0.1.41/src/env.rs
[workflow-run-response]: https://docs.github.com/en/rest/actions/workflow-runs#get-a-workflow-run
[pull-request-response]: https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request
