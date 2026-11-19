# Architectural decision record (ADR) 0082: Authorize release writes from the caller's policy

## Status

Accepted on 2026-11-18. Every release write is authorized by the release
workflow's own decision over typed values, which the trigger-derived resolver
can neither widen nor replace.

## Date

2026-11-18

## Context and problem statement

`release.yml` is both a tag-push entry point and a `workflow_call` workflow.
The pinned `determine-release-modes` action answers `should-publish: true` for
every `push` before it consults the reusable `publish` input, so a caller that
inherits a push trigger reached `publish-release` — a job holding
`contents: write` that creates the draft release and fills it from the run's
staged bytes — even when it asked for no publication. Issue #1001 records the
resulting cross-workflow authorization gap (CWE-863).

The resolver answers a question about the ref and the event. It cannot answer
the caller's question, which is whether this particular invocation asked to
publish. Deciding that from the resolver's output alone conflates the two.

## Decision Drivers

- A reusable caller's `publish` and `dry-run` inputs must bind the run, whatever
  trigger reached it.
- A direct `v*.*.*` tag push of `release.yml` must keep publishing as it did.
- The decision must be testable without running a release.
- Workflow-contract checks must exercise the same rule a live run applies, not a
  second copy of it.
- A malformed input must fail the step rather than be interpreted charitably.

## Requirements

### Functional requirements

- A caller that passes `dry-run: true` never publishes, even with
  `publish: true`.
- A reusable invocation publishes only when it passes `publish: true`.
- A direct tag push of `release.yml` may publish on the resolver's answer.
- `publish-release` runs only when the authorized `should_publish` is true and
  the run is not a dry run.
- A supported non-publishing request succeeds; only a malformed input fails.

### Technical requirements

- The decision is a pure function over typed values with no environment,
  filesystem, or workflow-key access.
- The step's inputs arrive through `env:`, never interpolated into `run:`.
- A malformed boolean writes an `::error` annotation and exits with status 2.
- Every decision writes a bounded record to the job summary and a notice
  annotation, unconditionally, and records only fixed fields.
- The invocation origin is classified from `github.workflow_ref`, not from the
  event name alone.
- Build diagnostic artefact upload is suppressed on a dry run. The
  `package-crate` job's bounded operation telemetry is a separate record that a
  rehearsal still retains, because its consumer is a later job.

## Options considered

### Option A: Authorize in the workflow with the resolver's own answer

Keep the resolver as the sole authority and document that callers must not
inherit a push trigger. This leaves the write path reachable for any caller the
documentation does not reach, which is the reported defect.

### Option B: Remove the `push` trigger from the reusable workflow

Splitting the entry points would remove the ambiguity at its source, but the
resolver's inputs are trigger-derived and the workflow is published for reuse;
dropping the trigger would break supported invocations and the pull-request
rehearsal, which relies on the same inputs.

### Option C: Decide in a dedicated step between the resolver and its consumers

A `release_authorization` step resolves the reusable inputs, the GitHub
context, and the resolver's answers into one decision, and every consumer reads
that decision. A requested dry run is an unconditional veto, a reusable caller
must pass `publish: true`, and only a direct tag push of this very workflow may
publish on the resolver's word alone. The resolver remains necessary but not
sufficient: its `should_publish` must agree and its `dry_run` can veto, but
neither can authorize a run the caller's policy refuses.

## Decision outcome / proposed direction

Adopt Option C. The `release_authorization` step runs in the `metadata` job
after `determine-release-modes` and before `validate_publish_ref`,
`ensure_version`, and every other consumer of the resolver's answer. It reads
`inputs.publish`, `inputs.dry-run`, `github.event_name`, `github.ref`,
`github.workflow_ref`, `github.repository`, and the resolver's `should-publish`,
`dry-run`, and `should-upload-workflow-artifacts` outputs through `env:`, and
writes `should-publish`, `dry-run`, `should-upload-workflow-artifacts`,
`origin`, and `reason` to `GITHUB_OUTPUT`.

Every decision is also recorded where a reader looks first. The adapter writes
the origin, the effective publication, dry-run, and artefact-upload flags, and
a bounded reason category to `GITHUB_STEP_SUMMARY`, and emits the same record
as a `Release authorization` notice annotation. The category names the rules
that refused publication, in policy order, or `authorized` when none did; the
free text `reason` stays in the step outputs. Only fixed fields are recorded,
so no caller-supplied text or secret reaches the summary or the log. The record
is written unconditionally by the step's entry point, and a workflow contract
reads that script's syntax tree to refuse a record a branch, loop, or handler
can skip.

The rule lives in `scripts/release_write_policy.py` as a pure function over
typed value objects, and the step's adapter is
`scripts/authorize_release_writes.py`, which parses the raw spellings,
classifies the origin, renders the outputs, and records the decision. The split
follows [ADR-0074](adr-0074-typed-records-for-release-policy-boundaries.md):
the policy knows no field names, and the adapter owns decoding.

Origin classification fails closed. A run is direct only when it carries no
reusable inputs, was triggered by a `push`, and names
`<repository>/.github/workflows/release.yml@<ref>` in `github.workflow_ref`.
Every other shape, including a missing or unparsable workflow ref, is treated
as reusable, so an unexpected context narrows authority instead of granting it.

Malformed input is refused at the adapter boundary, where a spelling can be
checked, rather than inside the policy. The accepted spellings are the ones
GitHub materializes — absent, `true`, and `false` — and anything else fails the
step with status 2 so no decision is published for a run whose inputs were not
understood.

## Goals and non-goals

- Goals:
  - Make the caller's inputs decisive for release writes.
  - Make the rule unit-testable without a release run.
  - Keep the workflow contracts and live runs on one implementation.
- Non-goals:
  - Change the resolver action, its inputs, or its other outputs.
  - Grant publication to any caller that does not pass `publish: true`.
  - Replace the job-level dry-run guard, which stays as defence in depth.

## Migration plan

1. Reusable callers that relied on an inherited push trigger to publish must
   pass `publish: true`; the migration guide records the change.
2. Callers that pass neither input keep their current non-publishing behaviour.
3. Direct tag pushes need no change.
4. `publish-release` keeps its own dry-run veto; the authorization step only
   makes the same refusal reachable one stage earlier.

## Known risks and limitations

- Classification depends on `github.workflow_ref`, which is unavailable in some
  contexts; an absent value is treated as reusable and therefore fails closed.
- The policy authorizes writes; it does not verify artefacts, checksums, or the
  upload plan, which the publication path still checks separately.
- A later change that adds a consumer reading the resolver directly would bypass
  the decision; the workflow contracts check the consumers they model, and a
  new consumer must be added to them.

## Outstanding decisions

- None. The rule, its adapter, and its consumers are recorded here; changes to
  any of them require an amendment to this record.
