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

## Addendum (2026-11-15): Enforced admission before publication

This addendum amends the 2026-09-04 addendum and the "non-blocking" statements
in the accepted decision. Their text is preserved as written; the gating mode
they specify is superseded here.

Publication is conditional on the gate before any RFC 0005 evidence producer
exists. The four jobs holding `contents: write`, `packages: write`, or
`id-token: write` — `publish-crates-io`, `publish-release`,
`verify-release-inventory`, and `finalize-github-release` — depend on
`release-admission-canaries` and require, as top-level conjuncts of their
job-level condition, that it succeeded, that `outputs.enforcement == 'true'`,
that `outputs.gate_outcome == 'success'`, and that
`outputs.candidate_commit == github.sha`. The gate derives
`NETSUKE_RELEASE_ADMISSION_ENFORCE` from
`needs.metadata.outputs.should_publish`, so a pull-request rehearsal still
observes and exits zero while a publishing run enforces. No override exists: no
input, environment variable, or workflow-dispatch field selects a mode beside
the release mode the metadata job already resolved.

The ordering is deliberate. Connecting a producer first would leave the
publishing path able to proceed without evidence for however long the two
changes landed apart. Until a producer uploads `release-admission-evidence`,
every publishing run therefore refuses with `error_category=missing_evidence`,
and it refuses a supplied manifest with `error_category=untrusted_evidence`
until a producer attests its upload; that is the intended fail-closed state,
not a defect to work around by loosening the gate. The producer is tracked by
[#598](https://github.com/leynos/netsuke/issues/598), which also supplies the
downstream canaries admission is to require; issue
[#1065](https://github.com/leynos/netsuke/issues/1065), which tracked the
release pipeline, is closed, and publication is now gated on packaging and on
an enforced, successful admission by `#1070`.

Enabling publication remains a separate step. It requires the trusted producer,
an admission job that waits for the producer's upload instead of tolerating an
absent artefact, a required set naming the downstream canaries, and an
end-to-end demonstration of both the successful and the refused publishing run.
The verifier already refuses an unsuccessful result for any declared canary,
including one it does not yet require, so a producer that reports a downstream
failure cannot have it ignored while the required set is still being extended.

The download step is not a provenance boundary and must not be cited as one.
`actions/download-artifact` resolves the current run by default, so any
artefact under that name reaches the verifier, and a reusable-workflow caller
defines jobs in the same run. Comparing the manifest's identity fields with the
run shows that the document agrees with the candidate; it does not show who
produced it. Establishing a producer-bound trust boundary, and requiring the
admission job to wait for that producer, belongs to `#598`.

The gate therefore carries an interim publication block rather than relying on
the absence of a producer. `NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED` is set
by this repository's workflow and not by a caller, and it ships as `false`.
While it is not `true`, an enforcing run refuses every manifest with
`untrusted_evidence`, including a fresh, matching document that a same-run job
uploaded; observation mode still reports what it finds, so a rehearsal is
unaffected and the read-only lane stays reachable. Enabling publication means a
producer that attests its upload, an admission step that derives this value
from that attestation, the downstream canaries, and an end-to-end demonstration
of both the successful and the refused publishing run; those requirements
belong to `#598`. Until they land this decision records a gate that fails
closed, not one that authenticates its evidence.

The rehearsal lane stays reachable. The `release` job holds no write scope and
must not depend on either `package-crate` or `release-admission-canaries`, so a
failure in either cannot skip the read-only lane; the same prohibition applies
in reverse, and neither `release` nor `package-crate` nor the gate may hold a
write scope. `tests/workflow_contracts/release_write_authority_contract.py`
asserts both directions and derives the write-authority set from the workflow's
own permission scopes, so a new publisher added beside the gate fails the
contract until it sits behind it.

Two error categories join the closed vocabulary so the two ways a supplied
manifest can be unusable are separable: `malformed_evidence` for a document
that cannot be read as the schema at all, and `candidate_mismatch` for one that
reads perfectly and describes another repository, commit, run, or tag. Both
extend the gate and operation `error_category` vocabularies recorded in the
accepted decision; an operator fixes them in different places, which is why
they are not one category. Metric and label names are unchanged, and the
extension is reviewed as the accepted decision requires.

`untrusted_evidence` extends the same closed vocabulary for a third condition:
the document is readable, fresh, and about the candidate, but the run cannot
show that a trusted producer wrote it. It is not a variant of
`candidate_mismatch`, because every identity field may agree while the origin
remains unknown; an operator fixes it by connecting the producer, not by
correcting the document.

The window is read twice, not once. `check_scan_freshness` refuses a document
whose timestamps lie outside the maximum age, and `verify_evidence` re-reads
the window once identity and results have been accepted, refusing with
`stale_evidence` if the document expired in between. Two clock commands and the
telemetry writes separate the two readings -- 14 ms per clock command on the
development host -- so a manifest observed a moment inside its window can be
outside it by the time the gate admits on it; the accepted decision's
requirement that stale admission evidence remains a gate failure then holds at
the decision rather than only at the observation. The recheck runs only after a
success and can only replace it with a refusal, so it adds no path to
admission. The category and metric vocabulary are unchanged: a recheck refusal
is attributed to `verify_evidence`, which is the operation that decided it.

`tests/workflow_contracts/release_admission_gate_contract.py` evaluates the
enforcement expression in every modelled scenario rather than matching its
text, so either hard-coded mode is reported for the run that exposes it. Each
scenario declares only the leaf outcomes whose failure it exists to exercise;
`release_publish_path_results.py` derives every other job's result from the
workflow's own dependency edges and guards, in dependency order, so a scenario
cannot pair a successful publisher with the failed build it publishes. The
derived results then drive one evaluation per job: the staging guard, the
publication guard, the registry publisher, the inventory verifier, and
`finalize-github-release`, whose six-job dependency set and guard are checked
against an independent expectation rather than against their own wording.
`tests/workflow_contracts/release_publication_gate_mutations.py` states
twenty-one regressions — a removed dependency, a removed result or output
check, either hard-coded mode, an `always()` publication bypass, an appended
always-true disjunct that keeps every clause as a conjunct, a publisher
declared with the `write-all` shorthand, a publisher inheriting write from the
workflow, the gate and rehearsal adopting each other, and finalization adopting
the rehearsal or dropping its verifier dependency — each with the diagnostics
that must notice it. Effective scopes and the conjunction requirement are read
by the same module, so a job's permissions and its guard are judged from what
GitHub would actually honour rather than from the spelling the workflow
happened to use.

## Addendum (2026-11-17): The gate runs from this repository's own commit

This addendum extends the 2026-11-15 addendum: the interim producer-trust block
is a boundary only if the code that reads it is the code this repository
shipped.

`release.yml` is a reusable workflow, and a called workflow's default
`actions/checkout` resolves the *caller's* repository. The admission job
therefore read the caller's candidate, so a caller could run an older or
modified `release_admission.py` that reported the mode, outcome, and candidate
the write guards consume while ignoring the `false` literal. The job now checks
out `leynos/netsuke` at `${{ job.workflow_sha }}` with
`persist-credentials: false`. `job.workflow_sha` is the commit of the workflow
file that defines the job, so the gate, the literal in its step environment,
and the summary scripts come from the same commit as the guards that trust
them, for a reusable-workflow call and for a directly triggered run alike.
`github.sha` would not serve: on a call it is the caller's candidate, which is
the commit the pin exists to refuse. The property postdates actionlint's
vendored context types, so `.github/actionlint.yaml` exempts that one
expression from the undefined-property check.

The pin removes the caller's choice of implementation. It is not producer
authentication: a caller that replaces the whole workflow also replaces the
guards, and attributing an upload to a named producer remains
[#598](https://github.com/leynos/netsuke/issues/598)'s work.
`check_admission_source_pin` in
`tests/workflow_contracts/release_admission_gate_contract.py` holds the
repository, the ref, and the ordering of the checkout before the gate step.
`tests/workflow_contracts/release_publication_gate_mutations.py` gains three
cases -- the checkout unpinned, pointed at `${{ github.repository }}`, and
reduced to `${{ github.sha }}` -- bringing its stated regressions to
twenty-four.
