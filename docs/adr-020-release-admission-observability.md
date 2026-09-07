# Architecture decision record (ADR): Bounded release-admission observability

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

The gate is a short-lived shell process rather than a service with a scrape
endpoint. Its observability therefore needs a durable hand-off to the workflow
run, with a small and reviewable contract that tests can validate independently
of a metrics backend.

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

The script keeps its fallible boundaries behind explicit Bash adapters:
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

- Admission script:
  [`require-release-admission-canaries.sh`](../.github/scripts/require-release-admission-canaries.sh)
- Release workflow and artefact export:
  [`release.yml`](../.github/workflows/release.yml)
- Metric allowlist and JSONL validation:
  [`release_admission_metrics.py`](../tests/workflow_contracts/release_admission_metrics.py)
- Script and workflow contract tests:
  [`test_release_admission_metrics.py`](../scripts/tests/test_release_admission_metrics.py)
  and
  [`release_admission_metrics_test.py`](../tests/workflow_contracts/release_admission_metrics_test.py)
- Operator guidance:
  [`developers-guide.md`](developers-guide.md#release-admission-observability)
- Release-admission design and sequencing:
  [`RFC 0005`](rfcs/0005-release-hardening.md)

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
