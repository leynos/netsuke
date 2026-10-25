# CI performance evidence

This directory holds published evidence for issue
[#894](https://github.com/leynos/netsuke/issues/894). It describes how ordinary
CI observations are collected and compared. Each JSON Lines record represents
one job attempt. The original harvest is `observations.jsonl`; dated
supplements remain separate. Keep raw CI logs outside the repository, such as
under `/tmp`.

Here, the fixed `trunk` cohort label names push-triggered runs on the
repository's `main` branch. The collector retains `trunk_coverage` and
`trunk_cache_writer` as fixed cohort labels.

For `ci.yml` pull-request jobs, `pr_coverage` identifies the Ubicloud lane,
`fork_hosted` requires distinct base and head repository identities, and
`dependabot_hosted` uses the PR author's `user.login` from a separately saved
[pull request response][pull-request-response]. A workflow-run response
includes the associated PR identity but not its author. The collector joins the
saved PR response only when its PR ID, number and base repository ID match the
workflow run. The actor is not a substitute for the PR author because it
changes when someone else pushes to a PR. Missing or mismatched author
provenance stays in `other`; the runner label alone does not prove that a run
came from a fork.

The saved-author join is owned by `ci_test_evidence_cohorts.py`.
`_run_association_base_matches_repository` is composed only by
`_pull_request_metadata_matches`, which is called only by
`_is_dependabot_pull_request`. Keep this join limited to Dependabot author
classification; fork classification uses the workflow run's base and head
repository identities. Reuse `ci_test_evidence_types.validate_identities` for
positive API identities; do not generalize the PR-author join to other cohorts.

The retained observation set is published in `observations.jsonl`: 81 job
records across 21 distinct run attempts. It contains six main-branch coverage
jobs, six main-branch cache-writer jobs, nine same-repository PR Linux gate
jobs, and 60 jobs from other cohorts. These are observational records across
different source commits, not a controlled baseline or candidate comparison. Do
not fill missing fields from historical September timings or infer them from
aggregate durations. The [15 October supplement](#15-october-supplement) adds
one ordinary attempt and five job records with different source and test
conditions.

## Collection

The offline collector is `scripts/ci_test_evidence.py`. It consumes saved run
and job API JSON, logs and optional telemetry artefacts without contacting
GitHub or another service. Run it with:

```console
uv run --script scripts/ci_test_evidence.py \
  --run RUN_JSON --jobs JOBS_JSON --pull-request PULL_REQUEST_JSON \
  --log-dir LOG_DIR \
  --telemetry-dir TELEMETRY_DIR --variant control \
  > /tmp/ci-test-evidence-v2.jsonl
```

The optional `--pull-request` file is the saved response for the run's single
associated pull request. It is needed only to identify the author of a hosted
PR run; without a matching response, the collector leaves author-dependent
classification in `other`. The collector reads only local files.

Before collection, choose a separately named destination for schema `2`, such as
`/tmp/ci-test-evidence-v2.jsonl`, and write the output there. Never append
schema `2` records to the retained schema `1` files `observations.jsonl` or
`2026-10-15-observations.jsonl`. Review the output before publishing it.
Exclude credentials, raw errors and raw logs. Use fixed labels for cohorts and
cache results.

The collector emits schema `2`. The retained observation files preserve their
original schema `1` records. This abbreviated example shows current top-level
fields, not an observation. Each record also contains `steps` and a nested
`evidence` object with versions, runner facts, cache states, sccache
statistics, Nextest/doctest outcomes, resources, phases and profile inventory:

```json
{
  "schema": 2,
  "run_id": null,
  "job_id": null,
  "run_attempt": null,
  "run_url": null,
  "job_url": null,
  "event": null,
  "source_sha": null,
  "tested_merge_sha": null,
  "variant": null,
  "relevant_diff": null,
  "comparison": "observational",
  "cohort": null,
  "runner_labels": [],
  "outcome": "unknown",
  "attempt_outcome": "unknown",
  "started_at": null,
  "completed_at": null,
  "whole_job_seconds": null,
  "attempt_start_to_job_start_seconds": null,
  "queue_and_provisioning_seconds": null,
  "steps": [],
  "evidence": {},
  "log_available": false,
  "telemetry_available": false,
  "attempt_runner_seconds": null,
  "attempt_observed_runner_seconds": null,
  "attempt_ubicloud_runner_seconds": null,
  "attempt_ubicloud_seconds_by_shape": {},
  "attempt_unknown_duration_jobs": null,
  "cost_basis": "runner_seconds_proxy",
  "billing_consumption": null,
  "storage_transfer_bytes": null
}
```

Represent absent, malformed or backwards timestamps as JSON `null`, and name
them in the nested evidence fields or the companion explanation of gaps. Never
substitute zero for missing time. Keep attempts separate, including failures,
cancellations, timeouts, retries and superseded attempts. Attempt totals are
repeated on each job record. Deduplicate by `(run_id, run_attempt)` before
summing them.

## Cohorts and comparison

Keep PR coverage, main-branch coverage, trusted main-branch cache-writer jobs,
and GitHub-hosted fork jobs separate. Record exact hits, prefix restores, and
misses separately for Cargo downloads, installed tools, Whitaker, and sccache.
Separate warm and cold/miss observations. Record queue and provisioning apart
from paid execution.

Compare successful, comparable attempts for timing. Exclude cancelled and
superseded attempts from successful-run timing summaries, but retain every
attempt in cost and reliability accounting. Label comparisons with a different
source/test set, toolchain, runner, or unmatched cache as observational and
explain the confounding factor. Per-test durations can overlap; do not sum them
as wall-clock savings.

The predeclared primary outcomes are whole-job elapsed time, including setup,
telemetry, uploads, cleanup and post-job cache work, and summed runner-seconds
per attempt as a cost proxy. Queue and provisioning are separate. Report each
observation, median, range, absolute and percentage differences, and the
Hodges-Lehmann median pairwise candidate-minus-control shift with an exact 95%
permutation-inversion interval under exchangeability for controlled, comparable
samples. Compute IQR with `statistics.quantiles(method="inclusive")`. Do not
make causal interval claims across source changes or report small-sample p95.
Five observations in each arm are a collection floor, not proof. The minimum
useful improvement is the larger of 60 seconds and the control cohort IQR.

Invert exact rank-sum permutation tests over additive location shifts, using
average ranks for ties and all label allocations. The interval also assumes a
location-shift model; materially different distribution shapes prevent an
adoption claim from that interval.

Use billing evidence when available. Otherwise call runner-seconds a cost
proxy, not invoice savings. For adoption, the one-sided 95% upper uncertainty
bound for candidate-minus-control consumption must be at most zero. No positive
cost increase is tolerated. Include failed attempts, telemetry, migration,
storage and transfer costs.

## Evidence inventory

Each record should include run/job links or IDs, attempt, event, source and
tested merge SHAs, variant and relevant diff, runner label/image/architecture/
capacity, Rust/Cargo/LLVM/Nextest/cargo-llvm-cov versions, shared action SHAs,
feature and target selection, compiler flags, profile and worker limits.
Extract action pins only from native runner `Download action repository` log
records with immutable SHA references. Each `action_pins` key is the full
`repository@sha` reference, so multiple revisions of one repository remain
distinct. A pinned Git dependency is not an action pin.

Keep cache keys and exact/fallback/miss state separate for Cargo downloads,
installed tools, Whitaker and sccache. Include sccache hits/misses and compiler
statistics, installer work, archive size where available, test identity and
outcome sets, failure/skip/ignore/retry counts and slow-test tail. Record
native timestamps and durations for setup, compilation, nested Cargo, Nextest,
coverage merge/export/report/ratchet, doctests, uploads, cleanup and whole-job
execution. Include raw-profile count/bytes, `.profdata` size,
instrumented-object count, peak memory, CPU and available I/O indicators.

Missing matched keys, inventories, per-test durations, memory or phase
boundaries must be recorded as missing. Do not reconstruct them from historical
durations. Do not add overlapping per-test times or claim queue changes as paid
compute savings.

## Telemetry budget and provenance

New telemetry is bounded to 10 seconds of job execution and 10 MiB of
uncompressed metadata per job, with no telemetry inside the Cargo watchdog.
Measure execution, upload and post-job cost. If either bound is exceeded, leave
telemetry and dependent evaluation disabled until the plan is revised with
evidence.

Workflow telemetry edits may invalidate broad installed-tool keys. Mark those
misses as instrumentation-induced and track warm-up separately from baseline
and candidate effects. Do not clear production caches or create extra paid
runs. Commit only redacted, parsed JSONL and a short explanation of its cohort
and evidence gaps; keep raw logs temporary.

The optional `--telemetry-dir` input contains one JSON file per job ID. Each
file must use schema `1` and identify its `run_id`, `run_attempt` and `job_id`.
The collector accepts no more than 10 MiB per job and copies only finite,
non-negative numeric values from the `profiles`, `resources` and `nested_cargo`
allowlists. Invalid or mismatched artefacts fail collection; unknown fields are
ignored.

## Retained observations

The table summarizes each distinct run attempt. The runner-second totals are
repeated on every job record and counted once here. They are resource-cost
proxies, not billing data. The last column shows a PR Linux gate job duration
only when that job and its complete attempt both succeeded. A dash means no
eligible PR gate timing for that attempt; keep it in cost and reliability
accounting.

| Run attempt                                                                  | Source SHA | Cohort(s)                       | Attempt outcome | Runner seconds | Ubicloud runner seconds | Successful PR gate seconds |
| ---------------------------------------------------------------------------- | ---------- | ------------------------------- | --------------- | -------------: | ----------------------: | -------------------------: |
| [38103238271](https://github.com/leynos/netsuke/actions/runs/38103238271), 1 | `3029d496` | main-branch coverage            | success         | 330            | 330                     | —                          |
| [38123563613](https://github.com/leynos/netsuke/actions/runs/38123563613), 1 | `a75420e6` | main-branch coverage            | success         | 337            | 337                     | —                          |
| [38125672660](https://github.com/leynos/netsuke/actions/runs/38125672660), 1 | `b73f8d62` | main-branch coverage            | success         | 399            | 399                     | —                          |
| [38132248056](https://github.com/leynos/netsuke/actions/runs/38132248056), 1 | `7539e32e` | main-branch coverage            | success         | 416            | 416                     | —                          |
| [38147234655](https://github.com/leynos/netsuke/actions/runs/38147234655), 1 | `557966c8` | main-branch coverage            | success         | 372            | 372                     | —                          |
| [38165545414](https://github.com/leynos/netsuke/actions/runs/38165545414), 1 | `9e1bf27e` | main-branch coverage            | success         | 388            | 388                     | —                          |
| [38174992874](https://github.com/leynos/netsuke/actions/runs/38174992874), 1 | `dc07efdd` | other, PR coverage              | cancelled       | 3,750          | 1,885                   | —                          |
| [38176320016](https://github.com/leynos/netsuke/actions/runs/38176320016), 1 | `c43bb6fc` | other, PR coverage              | cancelled       | 2,967          | 1,503                   | —                          |
| [38177218518](https://github.com/leynos/netsuke/actions/runs/38177218518), 1 | `f88b12ef` | other, PR coverage              | cancelled       | 1,530          | 708                     | —                          |
| [38177653022](https://github.com/leynos/netsuke/actions/runs/38177653022), 1 | `7faf1f1d` | other, PR coverage              | cancelled       | 4,093          | 1,965                   | —                          |
| [38178639487](https://github.com/leynos/netsuke/actions/runs/38178639487), 1 | `70b8b0b7` | other, PR coverage              | success         | 4,365          | 2,294                   | 1,438                      |
| [38179177460](https://github.com/leynos/netsuke/actions/runs/38179177460), 1 | `c89535d0` | other, PR coverage              | success         | 4,097          | 2,024                   | 1,213                      |
| [38194233518](https://github.com/leynos/netsuke/actions/runs/38194233518), 1 | `3317c8ce` | other, PR coverage              | success         | 4,338          | 2,181                   | 1,341                      |
| [38194408698](https://github.com/leynos/netsuke/actions/runs/38194408698), 1 | `5ec58f1a` | other, PR coverage              | failure         | 2,357          | 1,917                   | —                          |
| [38276341866](https://github.com/leynos/netsuke/actions/runs/38276341866), 1 | `ec0afc1c` | other, PR coverage              | success         | 4,338          | 2,108                   | 1,062                      |
| [38103238466](https://github.com/leynos/netsuke/actions/runs/38103238466), 1 | `3029d496` | other, main-branch cache writer | success         | 3,567          | 1,292                   | —                          |
| [38123563894](https://github.com/leynos/netsuke/actions/runs/38123563894), 1 | `a75420e6` | other, main-branch cache writer | success         | 3,307          | 1,345                   | —                          |
| [38125672832](https://github.com/leynos/netsuke/actions/runs/38125672832), 1 | `b73f8d62` | other, main-branch cache writer | success         | 3,585          | 1,308                   | —                          |
| [38132247949](https://github.com/leynos/netsuke/actions/runs/38132247949), 1 | `7539e32e` | other, main-branch cache writer | success         | 3,529          | 1,256                   | —                          |
| [38147234790](https://github.com/leynos/netsuke/actions/runs/38147234790), 1 | `557966c8` | other, main-branch cache writer | success         | 3,628          | 1,557                   | —                          |
| [38165545712](https://github.com/leynos/netsuke/actions/runs/38165545712), 1 | `9e1bf27e` | other, main-branch cache writer | success         | 3,913          | 1,914                   | —                          |

Across these attempts, deduplicated totals are 55,606 runner-seconds, of which
27,499 are labelled Ubicloud runner-seconds. Keep cache conditions separate
even for descriptive summaries:

- PR tools/Whitaker exact, Cargo exact: n=1, median/range 1,213 seconds.
- PR tools/Whitaker miss, Cargo exact: n=3, median 1,341 seconds,
  range 1,062–1,438 seconds.
- Main-branch Cargo exact: n=5, median 372 seconds, range 330–416 seconds.
- Main-branch Cargo miss: n=1, median/range 388 seconds.

Source revisions vary; paired main-branch workflows use the same SHA but belong
to different cohorts. Sccache hit/miss ratios also vary within these groups.
These summaries are observational and do not establish a controlled baseline,
setup-time savings or a causal comparison. No candidate observations exist. The
paid proxy separates 12,567 runner-seconds on the two-vCPU shape and 14,932 on
the four-vCPU shape; these are not interchangeable billing units. The control
IQR used for adoption has not been measured.

The 81 source records retain per-job outcomes, phases, action pins,
environment, cache evidence and available test/resource facts. Use them for
detail; the table is an attempt-level summary. Their action-pin fields were
reconstructed from the retained job logs using only native runner download
records; Git dependency revisions were removed.

## Workflow edits and installed-tool misses

The pinned cache implementation hashes the whole workflow. Two inspected
Kani-only workflow diffs rotated installed-tool keys and missed tools and
Whitaker archives in ordinary PR runs:

- [Run 38178639487](https://github.com/leynos/netsuke/actions/runs/38178639487)
  used tools fingerprint `5409bac39810a489`. Its [source diff][kani-diff-1]
  changes Kani metrics wiring rather than Linux tool installation.
- [Run 38276341866](https://github.com/leynos/netsuke/actions/runs/38276341866)
  used tools fingerprint `d1c4ba2288780e3a`. Its [source diff][kani-diff-2] has
  the same Kani-only distinction against its own merge base.
- [Run 38194233518](https://github.com/leynos/netsuke/actions/runs/38194233518)
  used fingerprint `635d321a4f3beb9a`. Its
  [installer revision diff][installer-diff] is an installation input and must
  invalidate a compatible cache generation.

These observations establish misses and their workflow inputs. They do not
prove counterfactual v2 hits, whole-job savings or complete compatibility
fingerprints. No cache was purged or populated for the investigation.

[kani-diff-1]: https://github.com/leynos/netsuke/compare/9e1bf27ee80a3389bef71d1b7e6e45aa86ea3a5b...70b8b0b776e8124c3c4d0b971bcc5dc7f3e48c9f
[kani-diff-2]: https://github.com/leynos/netsuke/compare/c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8...ec0afc1c5dd168076fb17c19910db8c2a79c14d1
[installer-diff]: https://github.com/leynos/netsuke/compare/9e1bf27ee80a3389bef71d1b7e6e45aa86ea3a5b...3317c8ced8caff844cd2f47f4ee373b67125d7bf

## Phase attribution and gaps

The latest [PR Linux job][latest-pr-job] in the original 21-attempt set
supplies this example of native attribution. It is observational, not a
controlled baseline. Step timestamps have whole-second precision; do not sum
these rows as savings.

| Component                            | Observed seconds | Boundary                                             |
| ------------------------------------ | ---------------: | ---------------------------------------------------- |
| Whole job                            | 1,062            | Job API start to completion, including post-job work |
| Restore gate caches                  | 4                | API step                                             |
| Setup Rust                           | 14               | API step                                             |
| Install Whitaker                     | 25               | API step                                             |
| Lint                                 | 250              | API step                                             |
| Documentation coverage               | 43               | API step                                             |
| Workflow contract tests              | 86               | API step                                             |
| Anchored Nextest filter verification | 65               | API step                                             |
| Test and Measure Coverage            | 476              | Combined build, tests, reporting and doctests        |
| Nextest execution                    | 338.232          | Native timer within the combined coverage step       |
| Merge/export separately              | —                | No distinct native duration retained                 |

[latest-pr-job]: https://github.com/leynos/netsuke/actions/runs/38276341866/job/114902625996

The pinned coverage action runs
`cargo llvm-cov nextest --manifest-path
Cargo.toml --workspace --all-targets --all-features --lcov --output-path
lcov.info`,
followed by the separate, uninstrumented
`cargo test --doc --workspace --manifest-path Cargo.toml --all-features` pass
with coverage environment variables unset. The collector observes the combined
test-and-coverage step, native Nextest boundaries and doctest result batches.
Merged doctests and their separate compile-fail batch retain the same target;
all native result sections contribute to the counts. It has no distinct native
merge or export duration in this retained dataset. Do not infer either duration
from Nextest summary timestamps.

Executed Nextest identities are recorded, but the selected identity inventory
includes skipped tests and is incomplete when those skipped identities are
absent. The legacy schema `1` record for job
[114633818860](https://github.com/leynos/netsuke/actions/runs/38194408698/job/114633818860)
contains 2,922 result entries but 2,921 unique identities. The test
`netsuke-build::polonius_toolchain_contract coverage_workflow_setup_matches_the_pinned_toolchain`
appears twice as a failure with the same 0.027-second duration and different
timestamps. Both completeness flags are false and `executed_count` is unknown.
Retain both entries, count one unique identity for identity-set analysis, and
do not infer that the repeated record represents a retry; the retained data
does not establish why it appears twice. This legacy exception does not affect
the attempt-level runner-second totals.

Coverage-command `coverage_environment` is a snapshot at the command marker;
the general `environment` mapping contains the last observed value for each
allowlisted variable. Neither proves the complete environment inherited by
every child process. System-used peak memory appears in some records; CPU
seconds and process RSS are missing. Native raw-profile and object inventories
are also missing until bounded telemetry is enabled.

## Current state

The source comparison in the ExecPlan section “Pre-rebase source comparison at
the assessed PR head” used PR base `8a961000859c02862454338fda6c4dd65bda2aa1`
and assessed candidate head `3928a0d1fff4f1d62349ef38f6ad435f55135f4a`. The PR
base has since moved to `88f20d588f936b810a2f700a76ceda970451ca7f`; see
“Current target source at the rebased base” in the ExecPlan for the current
source comparison. Timeout differences from the historical
`c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8` snapshot are recorded there. The
issue's source snapshot, `24293a1b3616872100d379e277540bb1df89203c`, is
historical. Run `38276341866` is one of the four successful PR Linux gate
observations: its gate job took 1,062 seconds, with 3,912 passes, six skips,
3,103 sccache hits, 390 misses and 3,120 MiB peak system-used. Its source
differs from the other PR observations. September timings are historical
context, not baseline measurements.

An exact-SHA run query on 15 October found only scheduled CI and mutation runs
for this implementation base. In the [scheduled CI attempt][base-schedule],
`kani-smoke` succeeded and `build-test` was skipped. It supplies no Linux gate
timing and is outside the retained 21-attempt dataset. No eligible observation
at this exact implementation SHA was available from that query.

[base-schedule]: https://github.com/leynos/netsuke/actions/runs/38249230403

Candidates A, B and C remain deferred pending a controlled baseline and the
missing attribution evidence. Keep their decisions and supporting observation
links in the living ExecPlan and issue. No default optimization is enabled.

## 15 October supplement

`2026-10-15-observations.jsonl` retains five jobs from ordinary
[attempt 38750711833, 1][current-attempt]. Its source SHA is
`2b01a3e3fd51e02d16bb5485ddb3b2f6dd3b7a31`; the Linux job tested merge SHA
`820bfdaa957bc75f28ea72159cf301b29ec6ccc7`. Only the Linux job log was
downloaded; missing logs for the other four jobs remain explicit.

The [Linux job][current-job] took 1,369 seconds, including 495 seconds for the
combined coverage step and 344.029 seconds of native Nextest execution. It
reported 3,933 passes, six skips and native doctest batches totalling 129
passes and 32 ignored tests. Cargo can announce the next doctest target before
the active batch ends. The log adapter preserves that active batch and pending
target separately, retaining all three batches without counting later unrelated
harnesses.

Cargo downloads, tools and Whitaker all had exact archive hits; sccache
reported 2,934 hits and 559 misses. Peak system-used memory was 2,591 MiB;
process RSS remains missing. The attempt consumed 4,230 runner-seconds as a
proxy: 1,369 on the four-vCPU Ubicloud shape, 787 on the two-vCPU shape and
2,074 on GitHub-hosted jobs. These are not invoice measurements.

Keep this supplement separate from the earlier descriptive summaries. Its
[source diff][current-diff] changes application tests and CI stages. Compared
with job `114902625996`, the executed identity sets contain 25 additions and
four removals; skipped identities are still missing. Additional native stages
include release-admission runtime checks and Linux package metadata checks. It
is not a fifth matched control or evidence of a performance regression.
Merge/export timings and raw-profile inventory remain missing.

[current-attempt]: https://github.com/leynos/netsuke/actions/runs/38750711833/attempts/1
[current-job]: https://github.com/leynos/netsuke/actions/runs/38750711833/job/116476064720
[current-diff]: https://github.com/leynos/netsuke/compare/c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8...2b01a3e3fd51e02d16bb5485ddb3b2f6dd3b7a31

## Dataset footprint

The JSONL contains full per-test identities and native outcomes. Its Git blob
is approximately 10.3 MiB before compression; the supplement adds approximately
0.75 MiB. Raw logs are not committed. No telemetry artefact upload or new cache
archive is introduced here. Provider storage and transfer billing for
repository checkout is unavailable, so this milestone makes no claim that the
additional data has zero transfer cost.

[pull-request-response]: https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request
