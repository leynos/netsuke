# Architectural decision record (ADR) 020: Isolate PR coverage submission

## Status

Accepted.

## Date

2026-09-05.

## Context and problem statement

The pull-request Continuous Integration (CI) job executes repository-controlled
commands. Those commands can write persistent runner state, including
`GITHUB_ENV`, `GITHUB_PATH`, and `BASH_ENV`, for later steps in the same job.
When that job also ran the CodeScene coverage action with `CS_ACCESS_TOKEN`,
PR-controlled state could influence a secret-bearing shell context and misuse
or exfiltrate the credential.

Pinning the CodeScene action does not isolate it from state created by earlier
steps. Moving the action to a second job in the same pull-request workflow is
also insufficient because a pull request controls that workflow definition. The
design therefore needs both a runner boundary and a workflow-definition trust
boundary.

## Decision Drivers

- Keep pull-request builds and tests available to contributors, including fork
  contributors, without granting them the CodeScene credential.
- Ensure the secret-bearing workflow definition comes from a trusted ref and
  cannot be rewritten by the pull request it evaluates.
- Treat downloaded coverage as hostile data and keep it outside command or
  interpreter execution paths.
- Preserve the existing CodeScene gate semantics and identify the originating
  pull request commit precisely.
- Keep logs and checks useful for correlation without exposing credentials or
  unbounded pull-request-controlled data.

## Requirements

### Functional requirements

- The unprivileged `pull_request` CI workflow builds, tests, measures coverage,
  and uploads only the fixed `pr-coverage-lcov` artefact. It does not receive
  `CS_ACCESS_TOKEN`.
- A trusted default-branch `workflow_run` workflow downloads that artefact on a
  fresh runner and submits it to CodeScene.
- Submission is eligible only when the source run succeeded, was triggered by
  a `pull_request` event, and has a same-repository head. The submission step
  also requires the step-local token-presence guard.
- The trusted workflow creates the `CodeScene coverage` Check Run against the
  originating `workflow_run.head_sha`, not the trusted workflow commit.
- Fork pull requests continue through unprivileged CI. Their trusted
  submission is excluded by the same-repository guard and, independently, an
  absent token skips submission without turning successful prerequisites into a
  failure.

### Technical requirements

- The trusted workflow must not check out or execute the pull-request tree and
  must not execute uploaded artefact contents.
- Validation must accept exactly one member named `lcov.info`, reject links and
  non-regular files, resolve and contain the member within the artefact
  directory, enforce a bounded size, decode UTF-8, and accept only recognized
  LCOV records with the required record types and terminator.
- `CS_ACCESS_TOKEN` must be available only through the CodeScene submission
  step's local environment. It must not be placed in job-wide state or
  persisted environment files.
- Correlation output is bounded to the originating workflow-run ID, originating
  commit SHA, fixed artefact name, download/validation/submission outcomes, and
  final conclusion. It must not include token values, secret names, LCOV
  content, artefact-derived paths, PR titles, or branch names.

## Options considered

### Option A: Trusted `workflow_run` submission

The pull-request workflow produces only `pr-coverage-lcov`. A default-branch
`workflow_run` consumer downloads it, validates it as hostile data, and invokes
CodeScene on a fresh runner with a step-scoped token. This creates independent
execution and workflow-definition boundaries while retaining the coverage gate.

### Option B: A second job in the pull-request workflow

Rejected. Although jobs use separate runners, the pull request can modify the
workflow definition for both jobs. A second job therefore does not establish a
trusted workflow-definition boundary for a secret-bearing action.

### Option C: Clean the environment before the CodeScene step

Rejected. Removing selected environment entries or resetting `PATH` cannot
provide a complete guarantee about state persisted by untrusted commands, and
it leaves secret use on the same runner as PR-controlled execution.

### Option D: Submit the entire downloaded artefact or execute its helpers

Rejected. The artefact is untrusted input. Only the validated LCOV data file is
passed as data to the pinned submission action; no member, path, script, or
other uploaded content is executed.

## Decision outcome / proposed direction

Adopt Option A. `.github/workflows/ci.yml` remains an unprivileged
`pull_request` workflow and publishes only the bounded `pr-coverage-lcov`
artefact. `.github/workflows/coverage-pr-submit.yml` is a trusted default-branch
`workflow_run` consumer. Its successful-source, pull-request, and
same-repository-head guards establish eligibility before download and
submission; its token-presence guard keeps fork or otherwise secretless runs
graceful.

The trusted runner checks out only validation tooling from the trusted default
branch. It downloads the fixed artefact into a dedicated directory, validates
the exact member and LCOV format, and then supplies the resulting data path to
the CodeScene action. The action receives `CS_ACCESS_TOKEN` only through its
step-local environment. The final Check Run uses `head_sha` from the source run
and stores that run's ID as its external correlation ID.

The Check Run is `success` after a successful submission. It is `neutral` only
when download and validation succeed and submission is skipped solely because
the token is absent. Download, validation, or submission failures produce a
failing Check Run.

Observability is limited to the workflow-run ID, source commit SHA, fixed
artefact name, three stage outcomes, and final conclusion in the Check Run and
workflow summary. The implementation must redact secrets and omit all
pull-request-controlled text and artefact content from these outputs.

## Goals and non-goals

- Goals:
  - Separate PR-controlled execution from secret-bearing CodeScene submission.
  - Make hostile artefact validation and the Check Run trust relationship
    reviewable and contract-tested.
  - Preserve fork usability and the existing coverage gate for eligible
    same-repository runs.
  - Give repository administrators clear controls for eligibility and required
    checks.
- Non-goals:
  - Granting the trusted workflow permission to execute or inspect arbitrary PR
    source files.
  - Treating LCOV source-file paths as filesystem paths to resolve or run.
  - Replacing repository or organization policy with workflow checks alone.

## Migration plan

1. Keep `build-test` responsible for coverage generation and upload, with no
   CodeScene secret.
2. Run the trusted `workflow_run` consumer from the default branch with the
   validator and step-local submission environment.
3. Configure branch protection to require the `CodeScene coverage` Check Run
   produced for the originating `head_sha`, and remove any obsolete required
   check for the former in-job submission step.
4. Retain repository and organization Actions policies that restrict the
   CodeScene credential to the trusted phase. Where required, place submission
   behind a protected environment with independent reviewers, actor
   restrictions, or equivalent policy controls.

## Known risks and limitations

- A same-repository actor that is eligible to trigger the trusted phase can
  still cause a submission using the privileges administrators grant to the
  CodeScene token; token permissions must therefore remain least-privilege.
- A missing token produces a neutral Check Run after successful prerequisites;
  branch protection must require the correct Check Run and administrators must
  decide whether neutral satisfies their repository policy.
- `workflow_run` and artefact retention are GitHub Actions controls. Changes to
  repository Actions policy, protected environments, action permissions, or
  branch protection can weaken this decision and require review.
- Correlation deliberately omits branch names, titles, and artefact content,
  so operators must use the workflow-run ID and source SHA to investigate a
  submission.

## Outstanding decisions

- Repository administrators must retain the Actions policy and secret access
  restrictions that make the trusted phase eligible.
- Repository administrators must decide whether a protected environment or
  independent approval is required in addition to the workflow guards.
- Organization administrators must update branch-protection required-check
  names if the old in-job coverage check remains configured.

## Implementation references

- Unprivileged workflow: [`ci.yml`](../.github/workflows/ci.yml)
- Trusted submission workflow:
  [`coverage-pr-submit.yml`](../.github/workflows/coverage-pr-submit.yml)
- Hostile artefact validator:
  [`validate-coverage-artifact.py`](../scripts/validate-coverage-artifact.py)
- Workflow contracts:
  [`trust_boundary_test.py`](../tests/workflow_contracts/trust_boundary_test.py)
  and
  [`trust_boundary_properties_test.py`](../tests/workflow_contracts/trust_boundary_properties_test.py)
- Developer guidance:
  [`PR coverage trust boundary`](developers-guide.md#pr-coverage-trust-boundary)
