# Architectural decision record (ADR) 024: Keep PR coverage local

## Status

Accepted.

## Date

2026-09-16.

## Context and problem statement

ADR-022 introduced a trusted `workflow_run` consumer so a pull request could
submit its coverage to CodeScene without receiving the CodeScene credential.
That design isolated the secret, but it could not make the CodeScene check
evaluate the pull request correctly. The trusted workflow checked out `main`,
ran outside pull-request context, and therefore reported against the default
branch commit rather than the pull-request head.

Pull requests do not need to publish coverage. Their useful invariant is that
changed-line coverage does not regress below the ratcheted value derived from
`main`. CodeScene needs one authoritative report for the analysed branch, and
that report belongs to `main` after each merge.

## Decision

Pull-request CI generates `lcov.info` and enables the shared coverage action's
ratchet check. It does not upload the report as a GitHub artefact, invoke the
CodeScene CLI, receive `CS_ACCESS_TOKEN`, or publish a CodeScene Check Run.

The `coverage-main.yml` workflow remains the sole owner of persistent coverage
state. On pushes to `main`, it runs the same coverage workload, advances the
ratchet baseline, and uploads the resulting LCOV report to CodeScene. Its
manual dispatch remains a read-only warm-run diagnostic and does not replace
the ratchet baseline.

CodeScene's project-analysis schedule and its policy for displaying a coverage
gate when data is unavailable are project settings. They are deliberately not
modelled by repository workflows.

## Rationale

- A local ratchet answers the pull-request question directly without crossing
  a credential or workflow-definition trust boundary.
- Publishing only from `main` gives CodeScene one report associated with the
  branch and commit it analyses.
- Removing the artefact hand-off eliminates validation, Check Run publication,
  telemetry, and retry machinery whose output could not represent the pull
  request accurately.
- The main workflow runs after merged pull requests, so its report and ratchet
  baseline follow the branch that future pull requests compare against.

## Consequences

- Pull-request checks no longer include a repository-published CodeScene
  coverage result. The local coverage ratchet remains part of the existing
  `build-test` job.
- A failed main coverage upload can leave CodeScene without current data, but
  it cannot give a pull request a verdict derived from the wrong commit.
- The historical hostile-artefact validators remain standalone maintenance
  tools; no active workflow downloads pull-request coverage.
- This decision supersedes ADR-022. Its threat analysis remains the reason a
  CodeScene credential must never return to pull-request-controlled execution.

## Verification

Workflow contract tests require the pull-request coverage step to keep ratchet
mode enabled, forbid the coverage artefact upload, require the privileged
submission workflow to remain absent, and verify that the main workflow uploads
the LCOV report generated earlier in the same job.
