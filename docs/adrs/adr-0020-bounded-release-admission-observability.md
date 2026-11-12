# Architectural decision record (ADR) 0020: Bounded release-admission observability

## Status

Accepted.

## Date

2026-09-01

## Context and problem statement

The release-admission canary scaffold makes GitHub API requests and Git fetches
before the release workflow can publish artefacts. A failed request, fetch, or
evidence check is recorded for operator review, but the current scaffold is
non-blocking: publication will become conditional only when a real RFC 0005
evidence producer is connected. Operators need to distinguish the failed
operation and its latency without turning revisions, run IDs, paths, URLs, or
workflow content into metric dimensions.

The gate is a short-lived process rather than a service with a scrape endpoint.
Its observability therefore needs a durable hand-off to the workflow run, with
a small and reviewable contract that tests can validate independently of a
metrics backend.

## Decision

The release-admission script emits exactly three metrics as JSON Lines (JSONL):

- `netsuke_release_admission_gate_total` is a counter for the final gate
  outcome. Its labels are `outcome=success|failure|unknown` and
  `error_category=none|api_error|fetch_error|stale_evidence|missing_evidence|`
  `mismatch|timeout|unknown`.
- `netsuke_release_admission_operation_total` is a counter emitted once after
  each fixed GitHub API request or Git fetch operation. Its labels are
  `canary=history_scan|release_candidate|none`,
  `operation=resolve_tag_commit|fetch_candidate_revision|fetch_workflow_run|`
  `check_scan_freshness|verify_evidence`, `outcome=success|failure|unknown`, and
  `error_category=none|api_error|fetch_error|stale_evidence|missing_evidence|`
  `mismatch|timeout|unknown`.
- `netsuke_release_admission_operation_duration_seconds` is a histogram
  emitted once after each fixed operation. It has only the label
  `operation=resolve_tag_commit|fetch_candidate_revision|fetch_workflow_run|`
  `check_scan_freshness|verify_evidence`.

The script validates every metric name and label against this closed vocabulary
before writing one JSONL record. `none` is used for `error_category` on a
successful operation or gate. A failed operation is classified as one of
`api_error`, `fetch_error`, `stale_evidence`, `missing_evidence`, `mismatch`, or
`timeout`; an unclassified failure fails closed as `outcome=unknown` and
`error_category=unknown`. Missing or stale admission evidence remains a gate
failure and cannot be converted into a successful result by telemetry.

Operation counters and duration observations are emitted at the end of each
fixed operation boundary. The gate counter is emitted at the gate's final
success or failure boundary, including the error category selected for an early
failure. No revision, run ID, path, URL, workflow content, or other
identifier-derived value may be a label. Duration values are observations only
and carry no operation-specific identifiers beyond the fixed operation name.

Each operation has a 30-second timeout by default. Set
`NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS` only to an integer from 1
through 300 seconds, inclusive. An operation that reaches its timeout emits
`outcome=failure` with `error_category=timeout`; invalid timeout configuration
fails closed as `outcome=failure` with `error_category=unknown`.

The workflow writes JSONL to the configured runner temporary metrics path
(`NETSUKE_RELEASE_ADMISSION_METRICS_FILE`, currently
`${runner.temp}/release-admission-metrics.jsonl`), uploads the completed file
as the `release-admission-metrics` workflow artefact, and appends a concise
gate outcome line to `GITHUB_STEP_SUMMARY`. GitHub Actions' configured workflow
or repository artefact-retention policy governs how long the JSONL artefact is
available. This contract does not define or imply a Prometheus, OTLP, or statsd
endpoint.

The gate keeps its fallible boundaries behind explicit adapters:
`NETSUKE_RELEASE_ADMISSION_GH_ADAPTER` for GitHub API requests,
`NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER` for Git fetches,
`NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER` for monotonic clock readings,
`NETSUKE_RELEASE_ADMISSION_METRICS_SINK` for metric records,
`NETSUKE_RELEASE_ADMISSION_OUTPUT_SINK` for `GITHUB_OUTPUT`, and
`NETSUKE_RELEASE_ADMISSION_TRACE_SINK` for trace records. The defaults are `gh`,
`git`, `python3`, and direct file or output appends; injected adapters retain
the same bounded record contracts.

Metrics are not tracing. The gate separately writes runner-local trace JSONL
records with the ordered fields `event`, `operation`, `outcome`,
`error_category`, and `duration_seconds`. `event` is limited to
`operation_complete|gate_complete|workflow_output_delivery|trace_delivery`; the
other categorical fields use the fixed vocabularies above. Trace records
contain no revisions, run IDs, paths, URLs, workflow content, raw errors, or
other identifiers. The workflow uploads them as the separate
`release-admission-traces` artefact under the same retention condition as the
metrics artefact. Trace delivery is fail-open: a sink failure preserves the
admission metrics and gate outcome and emits a bounded `trace_delivery` failure
with `error_category=unknown` when the sink permits a final record. The job
summary reports the gate outcome independently of trace delivery, and
observation versus enforcement controls publication gating rather than trace
collection.

Metric names and label names are published interfaces. Renaming a metric or a
label is a breaking change and requires an updated ADR, workflow consumers, and
contract tests. Adding a label or vocabulary value also requires an explicit
contract review because it changes the bounded series set.

## Rationale

- **Bounded cardinality.** Every label comes from a fixed enumeration, so
  arbitrary GitHub and Git values cannot create unbounded series or disclose
  sensitive workflow data.
- **Useful failure attribution.** Operation and error-category labels identify
  the class of failure while keeping detailed diagnostics in the workflow log.
- **Durable short-lived reporting.** A JSONL workflow artefact survives the
  runner process and can be downloaded with the run; the job summary exposes
  the top-level outcome without requiring a metrics service.
- **Fail-closed classification.** Telemetry classification cannot turn
  unknown, missing, stale, or contradictory evidence into a successful
  admission result. Publication remains non-blocking until the evidence
  producer is connected.

## Consequences

Operators inspect the gate counter first. A `failure` or `unknown` outcome is
an actionable canary result and should be correlated with operation counter
records, their fixed `error_category`, and duration observations in the
downloaded JSONL artefact. The job summary is an at-a-glance indication only;
it does not replace the per-operation records. The current scaffold does not
block publication; that dependency is enabled only after a real evidence
producer is connected.

The JSONL file is an export artefact, not a continuously aggregated time
series. Consumers must tolerate multiple records for an operation across
workflow runs and must not assume that a missing artefact means a successful
gate. The fixed vocabulary may be extended only through a reviewed contract
change; arbitrary values are rejected by the emitter and by the validation
tests.

## Addendum (2026-10-03): Release staging and publication observability

This is a separate accepted workflow decision. It preserves the release-
admission canary decision above and does not change its gating mode.

The `release` and `publish-release` jobs call
`scripts/release_operation_observability.py` after their staging or publication
steps. Each job writes an always-run outcome to `GITHUB_STEP_SUMMARY`. The
publication job uploads `release-operation-observability.jsonl` as the
`release-operation-observability-${{ github.job }}` workflow artefact. The
staging job keeps its JSONL file on the runner and does not upload a diagnostic
artefact, preserving the dry-run contract. The workflow does not set an
artefact retention override; the configured GitHub Actions retention policy
applies to the publication artefact.

The JSONL records use fixed operation labels (`release_staging` and
`release_publication`), fixed phase labels (`draft_release`,
`artifact_download`, `archive_hoist`, and `upload_plan_validation`), and
outcomes limited to `success`, `failure`, `cancelled`, `skipped`, or `unknown`.
Error categories are limited to `draft_creation`, `artifact_download`,
`archive_hoist`, `upload_plan_validation`, `other`, `cancelled`, or `none`.
Records contain operation and phase counters, duration samples, and phase
start/completion events. If cancellation interrupts a phase before its end
marker, the trace records `release_phase_interrupted`. The summary shows the
operation, dry-run mode, job and phase outcomes, and the fixed error category.
No release tags, artefact filenames, raw error messages, or secrets are
included.

This helper is diagnostic: it does not change the release job guards or turn a
failed release step into success. Existing workflow conditions and step results
remain authoritative for release success and publication.

## Alternatives considered

### Prometheus, OTLP, or statsd export

Rejected. A release runner is short-lived, and opening or configuring a network
endpoint would add credentials, connectivity, and retention concerns to the
publication gate. The workflow artefact and job summary provide the required
operator hand-off without a new service dependency.

### Labels containing revisions, run IDs, paths, or URLs

Rejected. These values are unbounded and may contain sensitive repository or
workflow information. They belong, where needed, in ordinary workflow logs, not
in metric dimensions.

### One aggregate gate metric without operation metrics

Rejected. An aggregate outcome says that the canary result was unsuccessful but
cannot identify whether an API request, fetch, freshness check, or evidence
check failed, nor which operation's latency needs investigation.

### Best-effort or free-form error labels

Rejected. Free-form errors create cardinality and redaction risks, while
best-effort telemetry can obscure an unknown admission result. The fixed
`error_category` vocabulary and `unknown` fail-closed mapping keep the gate
safe and diagnosable.

## Implementation references

- Admission gate:
  [`release_admission.py`](../../.github/scripts/release_admission.py) and its
  implementation package under
  [`_release_admission/`](../../.github/scripts/_release_admission/)
- Release workflow and artefact export:
  [`release.yml`](../../.github/workflows/release.yml)
- Metric allowlist and JSONL validation:
  [`release_admission_metrics.py`](../../tests/workflow_contracts/release_admission_metrics.py)
- Script and workflow contract tests:
  [`test_release_admission_metrics.py`](../../scripts/tests/test_release_admission_metrics.py),
  the rest of the runtime modules beside it under
  [`scripts/tests/`](../../scripts/tests/), and
  [`release_admission_metrics_test.py`](../../tests/workflow_contracts/release_admission_metrics_test.py)
- Operator guidance:
  [`developers-guide.md`](../developers-guide.md#release-admission-observability)
- Release-admission design and sequencing:
  [`RFC 0005`](../rfcs/rfc-0005-harden-release-integrity-and-admission.md)
- Release-operation observability helper and contract:
  [`release_operation_observability.py`](../../scripts/release_operation_observability.py)
  and
  [`release_operation_observability_test.py`](../../tests/workflow_contracts/release_operation_observability_test.py)

## Addendum (2026-09-04)

The accepted decision above remains unchanged. The current release workflow
uses the scaffold in observation mode because no RFC 0005 evidence producer is
available yet. `NETSUKE_RELEASE_ADMISSION_ENFORCE` is a validated mode selector
with the closed values `false` (observation) and `true` (enforcement); an unset
value defaults to `false`.

Observation mode records missing, stale, malformed, unknown, or mismatched
evidence with the existing fail-closed outcome and error category, writes the
gate outputs and summary, and exits successfully so the scaffold does not block
publication. Enforcement mode preserves the accepted fail-closed behaviour and
exits non-zero for those results. An evidence-state value such as `fresh` does
not substitute for a real evidence producer or enable enforcement. Publication
must remain independent of the scaffold until that producer and its validation
are available.

## Addendum (2026-10-11): The bound, the streams, and the statuses

The accepted decision above is unchanged. Three execution properties are now
contractual rather than incidental.

**The bound reaches a process group.** A command that outlives its operation
timeout is signalled as a group, `SIGTERM` first and `SIGKILL` after a
one-second grace, which is the escalation GNU `timeout --kill-after=1s` gave.
The direct child is reaped within the timeout plus that grace. Two descendants
can survive, both shared with GNU `timeout`: one that leaves the group with its
own `setsid()`, and one that ignores `SIGTERM` after the direct child has
already exited, because `SIGKILL` is sent only while the direct child is
unreaped. The limits are recorded in full in the developers' guide.

**The streams follow the shell's redirections.** A command whose output the
gate parses has that stream captured in a pipe the gate reads to end-of-file,
and its standard error inherited, so only the parsed stream is private. A
command whose output is only reported, and a sink adapter, have both streams
inherited. The clock adapter runs with no bound at all, as the shell left it
unwrapped.

**Statuses are reported in Bash's convention.** A child killed by a signal is
reported as `128` plus the signal, and a launch the operating system refuses
keeps the shell's statuses: `127` when the program does not exist, `126` for
any other refusal. Neither translation changes a published `error_category`;
they keep it, because a timed-out operation is identified by the statuses the
shell produced rather than the negative numbers Python reports.

## Addendum (2026-10-26): Crate-package operation telemetry

This addendum extends the release-operation vocabulary without changing the
release-admission gate or the release-versus-publication guards.

The `package-crate` job records the `release_package` operation with exactly
three phases. `crate_packaging` and `crate_candidate_validation` time the two
steps of the package action, and `artifact_upload` brackets the upload of the
`release-crate-package` workflow artefact containing the crate archive and
identity record. The action times its own steps because the boundary between
them is inside it: the candidate check runs after packaging and before the
artefact upload, so no workflow step bracket can separate a rejected candidate
from a failed packaging step. The telemetry artefact upload is separate and is
not a phase. These same phase boundaries apply during rehearsals and tagged
release runs.

An always-run step writes the bounded operation summary to
`GITHUB_STEP_SUMMARY` and writes metric and phase-event records to
`release-operation-observability.jsonl` under `runner.temp`. A separate
always-run step uploads that file as
`release-operation-observability-package-crate`. The package and telemetry
artefacts do not set a retention override, so GitHub Actions applies the
configured retention policy. The records use fixed operation, phase, outcome,
and error-category values and omit release tags, artefact names, raw errors,
and secrets.

The helper remains diagnostic: it does not change the release mode or convert a
failed package or artefact-upload step into success. The package job is not a
dependency of either publication job until the registry publisher consumes its
artefact.

## Addendum (2026-11-08): Attempt-specific telemetry artefact names

This addendum amends, and does not replace, the 2026-10-03 and 2026-10-26
addenda above. Their text is preserved as written; the artefact names they
specify are superseded here.

Every retained observability artefact name now carries
`-${{ github.run_attempt }}` after the job scope, so
`release-operation-observability-${{ github.job }}` becomes
`release-operation-observability-${{ github.job }}-${{ github.run_attempt }}`,
and the package job's `release-operation-observability-package-crate` becomes
`release-operation-observability-package-crate-${{ github.run_attempt }}`.

The suffix is load-bearing. A job re-run keeps the run id and increments only
the attempt, so an artefact named for the job alone collides with the first
attempt's artefact, and `actions/upload-artifact` v7 refuses to overwrite an
existing one rather than replacing it. Resolving that with `overwrite: true`
would be worse than the collision: the retained record exists so an operator
can compare a re-run against the attempt that failed, and overwriting deletes
the comparison. The name distinguishes attempts instead, which costs one extra
artefact per re-run and loses nothing.

Nothing else in either addendum changes. The retention policy, the dry-run
carve-out for the staging job, the fixed operation and phase labels, and the
bounded error-category set all stand.

## Addendum (2026-11-12): Per-call upload trace

This addendum extends the accepted decision to the declared-asset uploader. It
does not change the release-admission gate, the bounds on what an artefact may
contain, or any earlier addendum.

The aggregate boundary timings cannot answer which call failed. `TimedRunner`
sums a boundary across every call that crossed it, and a run downloads once per
undigested name, so four durations stand for as many as eleven `gh` invocations.
`upload-trace.jsonl` is the per-call record: one `trace_event` per invocation,
with the operation (`release_listing`, `asset_download`, `asset_upload`,
`release_verification`), the outcome (`completed` or `failed`), an
`error_category` of `call_failed` on a failed call, the duration in seconds,
and the run identity (`run_id`, `job`, `run_attempt`).

The record follows this ADR's existing bounds. No asset name, URL, payload
path, or command output is written; the duration and the bounded vocabulary are
the whole of it. `TraceRecord` refuses an operation or outcome outside that
vocabulary at construction, so a value a consumer cannot filter on cannot be
written. The run identity is read once by the command line and passed in,
rather than read by the seam: the seam stays free of process state, as the
injected-environment rule requires.

A call that returned a non-zero status is traced as `failed`, not only one that
raised. A `gh` refusal is normally reported through its exit status, and a
trace that marked every one of those `completed` would miss exactly the calls
an operator investigates.

The two vocabularies stay separate. A boundary names a duration to sum and an
operation names a call to trace, and the mapping between them is one table in
`release_upload_trace` rather than a caller's assumption. The file is written
beside `upload-plan.json` and `upload-timings.json` and retained by the same
`always()` artefact step, and it never fails the run: the plan is the contract
and the uploader has already reported a refusal on standard error.
