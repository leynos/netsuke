# Architectural decision record (ADR) 025: Keep PR coverage local

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
ratchet check. It adds no coverage-publication step of its own: it does not
invoke the CodeScene CLI, receive `CS_ACCESS_TOKEN`, or publish a CodeScene
Check Run. It also passes the action's `publish-artefact: 'false'` input, which
suppresses the action's own `Archive coverage` step, so the report is not
archived at all. That opt-out is an input this repository sets rather than a
publication it requests, and no credential crosses with it: the report stays on
the runner that measured it.

The `coverage-main.yml` workflow remains the sole owner of persistent coverage
state. On pushes to `main`, it runs the same coverage workload, advances the
ratchet baseline, and uploads the resulting LCOV report to CodeScene. It leaves
`publish-artefact` unset, so the action keeps its default and archives the
report that upload reads. Its manual dispatch remains a read-only warm-run
diagnostic and does not replace the ratchet baseline.

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
- The report the shared action generates is not archived on a pull request,
  because that lane passes `publish-artefact: 'false'`. Had it been archived,
  it would be readable by any step in the pull-request job that can read the
  workspace. That is not a trust boundary this repository relies on, because
  nothing downstream consumes it: the credential never enters a pull-request
  job, and no submission path reads the report.
- This decision supersedes ADR-022. Its threat analysis remains the reason a
  CodeScene credential must never return to pull-request-controlled execution.

## Verification

Workflow contract tests require the pull-request coverage step to keep ratchet
mode enabled, and forbid every pull-request-triggered workflow — and any
`workflow_run` consumer under any file name — from publishing the report through
`actions/upload-artifact`, invoking the CodeScene coverage action, or
referencing `CS_ACCESS_TOKEN` in its parsed values or its raw text. The
detectors are matched by structure rather than by the retired step and file
names, and are driven against synthetic workflow text so a detector that
stopped matching cannot pass by finding nothing. A further test verifies that
the main workflow uploads the LCOV report generated earlier in the same job.

The archive step lives inside the pinned shared action, so no scan of this
repository's steps can see it. The contract therefore observes it the only way
it can: the pull-request coverage call must pass the publication opt-out, and a
detector fails any coverage call that omits it or supplies a value the action
does not compare against. A further test holds the two lanes apart, requiring
the pull-request lane to decline the archive and forbidding the main workflow
from passing the input that would suppress the upload CodeScene reads.

## Addendum, 2026-09-23: the dispatch upload and the pull-request closure

The decision above is unchanged. Two things it described needed correcting.

_Manual dispatch._ The context above calls the publisher's manual dispatch a
read-only warm-run diagnostic. That held for the ratchet baseline, which the
coverage action saves only on a push to `refs/heads/main`. It did not hold for
the CodeScene upload, whose only guard was the credential, while the push
trigger's `branches: [main]` filter constrains the push alone. A dispatch from
a feature branch therefore uploaded that branch's report. The upload step is
now also guarded on `github.ref == 'refs/heads/main'`. A dispatch from `main`
uploads that commit's report, as a push would. A dispatch from any other branch
uploads nothing. The baseline still advances only on a push to `main`.

_The pull-request surface._ The verification above enumerated pull-request
workflows by trigger. A `workflow_call` workflow that a pull-request workflow
calls runs on that pull request too, and `secrets: inherit` hands it the
credential. So the surface is now the closure through local reusable-workflow
calls. `secrets: inherit` into another repository's workflow is refused, and so
is any mention of `codescene.io`.

The developers' guide, under _Coverage ratchet and CodeScene publication_,
records the contracts that hold both.
