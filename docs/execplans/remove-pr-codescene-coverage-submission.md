# Keep pull-request coverage enforcement local

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Pull requests should measure coverage and enforce the repository's ratchet
without sending coverage data or gate results to CodeScene. The authoritative
`main` workflow should remain the sole writer of the ratchet baseline and the
sole CodeScene coverage uploader. After this change, a reviewer can observe one
coverage decision on a pull request, backed by the baseline produced after the
previous merge, while CodeScene receives coverage only after changes reach
`main`.

## Constraints

- Preserve `.github/workflows/ci.yml` coverage generation and its
  `with-ratchet: 'true'` comparison on pull requests.
- Preserve `.github/workflows/coverage-main.yml` as the sole ratchet-baseline
  writer and CodeScene coverage uploader on pushes to `main`.
- Do not expose `CS_ACCESS_TOKEN` to pull-request-controlled code or artefacts.
- Do not change Rust application behaviour, public APIs, dependencies, coverage
  test selection, or coverage thresholds.
- Record the reversal of ADR-022 through the repository's ADR process and keep
  `docs/contents.md` and `docs/developers-guide.md` accurate.
- Keep all documentation in en-GB Oxford English and conform to
  `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

- Scope: stop if the implementation needs more than 25 tracked files or more
  than 2,500 net changed lines excluding deleted obsolete implementation and
  tests.
- Interface: stop if the shared `generate-coverage` or
  `upload-codescene-coverage` action contract must change.
- Dependencies: stop if any new external dependency is required.
- External configuration: stop if completion requires changing CodeScene or
  GitHub settings beyond the already agreed project configuration; document the
  exact manual operation instead.
- Iterations: stop if a focused workflow-contract failure remains unexplained
  after three correction attempts.
- Ambiguity: stop if repository evidence shows that another consumer relies on
  the `pr-coverage-lcov` artefact or the custom `CodeScene coverage` check.

## Risks

- Risk: branch rules may still require a retired check name.
  Severity: high. Likelihood: medium. Mitigation: inspect repository rulesets
  before deletion and record any remaining external configuration in the
  pull-request notes.
- Risk: deleting the trusted submission workflow may leave orphaned scripts,
  tests, documentation, runner placement entries, or Make targets. Severity:
  medium. Likelihood: high. Mitigation: search the complete repository before
  and after deletion and make the absence of the obsolete workflow a contract
  test.
- Risk: changing the coverage topology could accidentally stop the main
  baseline from advancing. Severity: high. Likelihood: low. Mitigation: retain
  and strengthen contracts proving that PRs read the ratchet while only a push
  to `main` publishes it and uploads to CodeScene.
- Risk: an ADR number may already exist on an unmerged remote branch.
  Severity: medium. Likelihood: low. Mitigation: inspect all remote branch
  trees before allocating the next ADR number.

## Progress

- [x] (2026-09-16 17:30Z) Diagnose the current topology and establish that the
  trusted PR workflow checks `main`, reports zero measured coverage, and passes
  only because both CodeScene gates are disabled.
- [x] (2026-09-16 17:35Z) Draft this ExecPlan for approval.
- [x] (2026-09-16 17:45Z) Receive explicit approval and begin execution.
- [x] (2026-09-16 18:00Z) Inspect branch rules, remote ADR allocations, and
      every repository
  reference to the PR submission boundary.
- [x] (2026-09-16 18:10Z) Add the focused failing workflow-contract test.
- [x] (2026-09-16 18:25Z) Remove the PR artefact upload, trusted submission
      workflow, owned helper
  modules, and tests that exist only for that workflow.
- [x] (2026-09-16 18:40Z) Record the replacement architecture and update
      contributor documentation.
- [x] (2026-09-16 18:45Z) Run focused tests and every repository quality gate.
- [x] (2026-09-16 18:55Z) Review the complete diff, commit, push
  `code-coverage-failure`, and open draft pull request #724 against `main`.

## Surprises & discoveries

- Observation: the trusted `workflow_run` job deliberately checks out `main`,
  so `cs-coverage check` runs outside pull-request context and compares `main`
  with its first parent. Evidence: run 35117392122 reported `base_ref nil` and
  commit `72256e83f8a52c0b9b0e77f47eafa1427d98d60b` while PR 716 was at
  `776e3bb7749fa6b1bbc92a1dd6158d394fe276c5`. Impact: the custom green check is
  not evidence about the pull request.
- Observation: `.github/workflows/ci.yml` and
  `.github/workflows/coverage-main.yml` already implement the desired local
  ratchet topology. Evidence: both call `generate-coverage` with
  `with-ratchet: 'true'`; the action pin publishes the baseline only on a push
  to `refs/heads/main`. Impact: implementation should remove redundant
  machinery rather than invent a replacement.
- Observation: the active `main-required-checks` ruleset does not require the
  custom CodeScene coverage check. Evidence: its required contexts are
  `build-test`, `kani-smoke`, `netsukefile`, and `release / metadata`. Impact:
  removing the custom publisher needs no GitHub ruleset change.
- Observation: the hostile-artefact validators under `scripts/` have explicit
  standalone Make targets as well as their former workflow consumer. Evidence:
  `test-coverage-artifact` and `validate-coverage-artifact` remain documented
  quality and inspection entry points. Impact: retain them as maintenance
  tools; remove only the private submission implementation under
  `.github/scripts`.

## Decision log

- Decision: make the repository ratchet the only pull-request coverage gate.
  Rationale: it compares the candidate report with a baseline written from
  `main`, needs no secret-bearing PR submission, and already runs in ordinary
  CI. Date/Author: 2026-09-16, user and Codex.
- Decision: keep CodeScene coverage upload confined to
  `.github/workflows/coverage-main.yml`. Rationale: CodeScene remains useful
  for historical analysis and hotspot visualization without participating in
  merge admission. Date/Author: 2026-09-16, user and Codex.
- Decision: remove the obsolete boundary atomically rather than retain a
  compatibility workflow or inert check. Rationale: the workflow, helper
  scripts, and check name are private CI surfaces with no valid consumer once
  the ratchet owns PR enforcement. Date/Author: 2026-09-16, Codex.

## Outcomes & retrospective

Pull-request coverage now stops at the local ratchet enforced by `build-test`.
The PR artefact hand-off, privileged `workflow_run` consumer, private publisher
modules, and their owned contracts have been removed. The main coverage
workflow remains the only CodeScene uploader and the only ratchet-baseline
writer.

ADR-025 records the replacement architecture and supersedes ADR-022. The
developer guide now separates repository behaviour from CodeScene's external
analysis schedule and unavailable-data policy. The active ruleset did not
require the retired custom check, so no GitHub configuration change was
necessary.

All focused and repository gates passed. The change ran to 30 tracked paths —
39 additions and 3,618 deletions, so the great majority of it is removal —
without changing shared actions, dependencies, Rust behaviour, or coverage
thresholds. The plan's scope tolerance was 25; the extra paths are the review
follow-up that strengthened the pull-request boundary contract after the first
push, and they are recorded in the revision note below rather than folded
silently into the original count.

The superseding record it introduced is ADR-025, not ADR-024. ADR-024 was
claimed by main's recursive-workspace-search decision before this branch
rebased, and this branch renumbered rather than colliding.

Two review findings were dispositioned rather than implemented. The request to
add an artefact-publication opt-out to `generate-coverage` and re-pin the
callers landed in a different repository whose own tests assert the archive
step's behaviour; ADR-025 now records the residual artefact instead, and the
repository contract forbids the netsuke-owned publication surface structurally.
The request to index this plan in `docs/contents.md` was declined because that
file indexes the `execplans/` directory and links no individual plan, so adding
one would break the established convention.

## Context and orientation

This section states the topology this plan started from. The outcomes above
describe what replaced it, and each claim below is marked with its disposition
so the two sections cannot be read as disagreeing.

`.github/workflows/ci.yml` generates `lcov.info` for pull requests and invokes
the shared coverage action with the ratchet enabled. At the time of writing it
uploaded that file as the `pr-coverage-lcov` artefact; that step was removed,
and the pull-request job now adds no publication step of its own. The pinned
`generate-coverage` revision still archives the report under its own
unconditional `Archive coverage` step, which takes no opt-out. ADR-025 records
that residual artefact and why no repository contract can observe it.

`.github/workflows/coverage-pr-submit.yml` was a privileged `workflow_run`
consumer. It downloaded and validated the untrusted artefact, ran
`cs-coverage check` with `CS_ACCESS_TOKEN`, and published a custom
`CodeScene coverage` Check Run through modules under `.github/scripts/`.
Because the trusted workflow checked out `main`, the CodeScene CLI did not have
pull-request context. The workflow and its modules were deleted.

`.github/workflows/coverage-main.yml` runs after pushes to `main`, generates
the same coverage shape, advances the shared ratchet baseline, and uploads the
report to CodeScene. This is the topology to retain, and it is unchanged.

`docs/adr-022-pr-coverage-trust-boundary.md` records the boundary that was
retired; it now carries a status of superseded and a dated addendum recording
the supersession. `docs/developers-guide.md` describes the replacement
operation. Tests under `tests/workflow_contracts/` load the workflows, enforce
the publication boundary, and assign jobs to runner classes; those owned
surfaces were updated together, as this plan required.

## Conformance basis

- Repository instructions: `AGENTS.md`, revision in the current branch.
- Documentation rules: `docs/documentation-style-guide.md`, revision in the
  current branch.
- Existing decision being superseded:
  `docs/adr-022-pr-coverage-trust-boundary.md`.
- Existing coverage contracts: `docs/developers-guide.md`, especially the
  coverage action pin, ratchet publication, and PR trust-boundary sections.
- No Terms of Reference or product-facing technical design governs this CI
  topology.

Trace links:

```plaintext
ARCH-PR-LOCAL -> EP-M1 -> workflow contract: PR ratchet present and PR artefact absent
ARCH-MAIN-AUTHORITATIVE -> EP-M1 -> workflow contract: main ratchet and upload present
ARCH-NO-PR-CODESCENE -> EP-M1 -> repository search: no PR CodeScene submission workflow
ARCH-DOCUMENTED-REVERSAL -> EP-M2 -> superseding ADR and developer guide
```

## Verification plan

This change introduces no runtime algorithm, state machine, arithmetic
invariant, or business-logic lemma warranting property testing, model checking,
or formal proof. Its important invariants are finite workflow-shape contracts,
so deterministic structural tests are proportionate and exhaustive over the
owned YAML documents.

- Obligation: `ARCH-PR-LOCAL`. Pull-request CI generates coverage with the
  ratchet enabled and does not upload `pr-coverage-lcov` or call CodeScene.
  Method: deterministic workflow-contract test. Rationale: the relevant
  workflow has a finite step list and explicit action inputs. Domain: every
  step in the PR `build-test` job and every workflow path. Artefact: the
  relevant module under `tests/workflow_contracts/` selected after repository
  inspection. Evidence: the focused pytest command fails before deletion
  because the artefact step and trusted workflow exist, then passes after
  implementation. Non-vacuity: the test also asserts the positive witness that
  `Test and Measure Coverage` remains present with `with-ratchet: 'true'`; a
  fixture mutation that restores the artefact step must fail.
- Obligation: `ARCH-MAIN-AUTHORITATIVE`. The main workflow retains the same
  test selection, ratchet enablement, push trigger, and CodeScene upload.
  Method: deterministic workflow-contract test using the parsed workflow.
  Rationale: exact input assertions detect accidental removal or drift. Domain:
  the `coverage-upload` job's trigger and named steps. Artefact: existing
  ratchet and coverage workflow-contract tests, strengthened only where they do
  not already cover the obligation. Evidence: focused pytest passes and a
  representative mutation removing the upload or ratchet setting fails.
  Non-vacuity: assertions require both named steps and exact action inputs; an
  empty job cannot pass.
- Obligation: `ARCH-NO-PR-CODESCENE`. No repository workflow, script, test, or
  documentation claims that PR coverage is sent to CodeScene. Method:
  structural test plus bounded repository search and diff review. Rationale:
  the obsolete surface consists of a known finite set of tracked files and
  literal integration names. Domain: tracked repository files excluding
  historical ADR text that clearly identifies the superseded design. Artefact:
  workflow-contract test and validation transcript in this plan. Evidence:
  focused test and `rg` inventory show only main upload and explicit historical
  references. Non-vacuity: the pre-change search finds the workflow, action
  invocation, helper modules, and check name.
- Obligation: `ARCH-DOCUMENTED-REVERSAL`. Documentation explains that the
  ratchet owns PR enforcement and CodeScene consumes only `main` uploads.
  Method: documentation review plus Markdown, spelling, and Mermaid gates.
  Rationale: this is prose architecture with no executable semantics beyond the
  linked workflow contracts. Domain: the new superseding ADR,
  `docs/contents.md`, and `docs/developers-guide.md`. Artefact: repository
  documentation files. Evidence: `make markdownlint`, `make nixie`, and
  link/diff inspection pass. Non-vacuity: links point to existing files and the
  prose names both the kept and removed paths.

External axioms are limited to the shared action contract that
`with-ratchet: 'true'` reads the candidate report and publishes a baseline only
on pushes to `refs/heads/main`, and CodeScene's documented behaviour that an
upload on `main` becomes available to a later project analysis. The repository
pins the shared action and owns contract tests for its invocation; this plan
does not attempt to prove third-party service internals.

## Plan of work

Stage A inventories current rulesets, remote ADR numbers, workflow references,
helper modules, tests, and documentation. It ends when the deletion boundary
and any external follow-up are explicit.

Stage B adds or revises the smallest workflow-contract test so it describes the
target topology. The focused test must fail because the PR artefact and trusted
submission workflow still exist. No production workflow changes occur in this
stage.

Stage C removes the PR artefact step, trusted workflow, its private Python
implementation, runner-placement entry, and tests that specify only the retired
design. It retains or strengthens tests for the PR ratchet and main upload. The
focused suite must pass before documentation work proceeds.

Stage D records the superseding ADR, updates the developer guide and contents
index, formats the repository, and runs the complete quality gates. The final
diff is reviewed for unrelated churn before delivery.

## Milestones and plateaus

- Identifier and outcome: `EP-M1`, repository behaviour contains one local PR
  coverage ratchet and one authoritative main upload, with no privileged PR
  submission path. Requirements and gaps: discharges `ARCH-PR-LOCAL`,
  `ARCH-MAIN-AUTHORITATIVE`, and `ARCH-NO-PR-CODESCENE`. Acceptance evidence:
  focused workflow-contract tests and bounded repository inventory pass.
  Conformance check: ratchet and upload action contracts remain unchanged; no
  dependency, application API, persisted format, or new trust boundary is
  introduced. Recovery: restore the atomic deletion and test changes together,
  then rerun the focused contract suite. Remaining gaps: architecture
  documentation and full gates. Compatibility decision: none; the removed
  workflow and helper modules are private CI surfaces, and the retired check
  must not remain as an inert shim.
- Identifier and outcome: `EP-M2`, documentation and full validation agree
  with the delivered topology. Requirements and gaps: discharges
  `ARCH-DOCUMENTED-REVERSAL` and completes the other obligations with
  repository-wide evidence. Acceptance evidence: required quality gates pass,
  the worktree contains only intended changes, and the draft pull request links
  the relevant files. Conformance check: the superseding ADR names the
  reversal, all upstream claims are reconciled, and no external setting is
  misrepresented as a repository change. Recovery: correct the failing owned
  surface; do not weaken or skip a gate. Remaining gaps: none within repository
  scope. Compatibility decision: none.

## Concrete steps

Run all commands from
`/data/leynos/Projects/netsuke.worktrees/code-coverage-failure`.

1. Inspect repository and remote state with `git status`, `git branch -r`,
   `git ls-tree`, GitHub ruleset queries, and bounded `rg` searches.
2. Add the target workflow-contract assertion and run its containing pytest
   module. Expect failure naming the still-present PR artefact or submission
   workflow.
3. Remove the obsolete workflow and owned modules with `apply_patch`; update
   every affected contract and run the focused workflow-contract suite until it
   passes.
4. Add the superseding ADR and update `docs/contents.md` and
   `docs/developers-guide.md`. Run `make fmt` after documentation edits.
5. Run `make check-fmt`, `make lint`, `make doc-coverage`, `make test`,
   `make markdownlint`, and `make nixie`. Run any narrower workflow-contract
   gate named by the diff before the full sequence.
6. Inspect `git diff --check`, the complete diff, and the staged tree. Commit
   with a file-based imperative message, push `code-coverage-failure`, and open
   a draft pull request against `main`.

Focused evidence recorded after implementation:

```plaintext
make test-workflow-contracts: 534 passed, 2 skipped
PR coverage ratchet: present
PR CodeScene submission: absent
main CodeScene upload: present
```

The count above is from the post-rebase revision, which also carries main's
additional workflow contracts. The pre-rebase run reported 456 passed, 3
skipped.

## Validation and acceptance

Red evidence is the focused workflow-contract failure caused by the existing
`Upload PR coverage artefact` step and `coverage-pr-submit.yml` file. Green
evidence is the same module passing after their removal while its positive
ratchet and main-upload assertions remain satisfied. Refactor evidence is the
focused workflow-contract suite passing after obsolete helpers and tests are
deleted and documentation references are reconciled.

Completion requires:

- `make check-fmt` passes.
- `make lint` passes without suppressions.
- `make doc-coverage` passes at the configured threshold.
- `make test` passes.
- `make markdownlint` passes.
- `make nixie` passes.
- `git diff --check` passes.
- The branch diff contains no application behaviour or dependency change.
- GitHub shows a draft pull request from `code-coverage-failure` to `main`.

No performance benchmark is required because the change removes a CI job and
does not alter application execution. Security acceptance requires that no PR
workflow or PR artefact consumer receives `CS_ACCESS_TOKEN`.

## Idempotence and recovery

Searches, formatting checks, and test commands are safe to repeat. The
behavioural removal is an atomic Git change and can be reverted as a unit. Do
not recreate a partial trusted workflow to recover a failing test; either fix
the retained ratchet/main-upload contract or restore the complete pre-change
state while investigating.

Temporary commit-message and pull-request body files must live in `mktemp -d`
directories and be removed after use. Validation artefacts should use
`/data/tmp` where substantial scratch space is needed.

## Artefacts and notes

The diagnosis preceding this plan established that CodeScene project 69281 had
both coverage gates disabled and that the custom check posted a successful
zero-coverage result for `main`. The CodeScene-owned check on PR 716 remained
queued because it awaited PR-associated coverage data. The project owner has
since configured daily analyses and hidden the coverage gate when data is
unavailable; those external settings are inputs to this plan, not repository
changes.

## Interfaces and dependencies

No new interface or dependency is introduced. The retained interfaces are the
existing `generate-coverage` composite action with `with-ratchet: 'true'` in
both workflows and `upload-codescene-coverage` in upload mode on `main` only.
The removed Python modules and `CodeScene coverage` Check Run are private
workflow implementation details and receive no compatibility layer.

## Revision note

2026-09-16: Created the initial draft from the observed PR 716 and main-upload
evidence.

2026-09-16: Marked the plan in progress after explicit approval. The planned
scope and verification obligations are unchanged.

2026-09-16: Implemented, gated, and pushed as draft pull request #724, then
marked complete.

2026-09-17: Completed the review follow-up. Rebased onto `origin/main`
(`6c2b2c4b` to `c32efb8d`), which advanced underneath the branch and landed
main's own ADR-024. The only conflict was a two-sided append in
`docs/contents.md`; both entries were kept, and the superseding record was
renumbered to ADR-025 in a dedicated commit rather than resolved silently
inside the conflicted one.

The pull-request coverage contract was strengthened to match the boundary by
structure rather than by the retired `Upload PR coverage artefact` step and
`coverage-pr-submit.yml` names. The same module now drives its detectors
against synthetic workflow text, so a detector that stopped matching fails the
suite instead of passing it by finding nothing. Each detector was separately
proved to fire by injecting a renamed publisher, a raw-text credential
reference, a resurrected `workflow_run` consumer under a new file name, and a
credential reference inside an existing `pull_request_target` workflow. The
prong reads both pull-request triggers, because `pull_request_target` runs in
the base repository's context and can read its secrets.

ADR-022 gained a dated addendum recording the supersession and its rationale;
ADR-025's decision, consequences, and verification text were corrected, since
its original claim that pull-request CI uploads no artefact was false for the
pinned shared action; and this plan's context, conformance, and verification
text were brought in line with the repository as it stands.

Validation on the rebased revision: `make test-workflow-contracts` 534 passed,
2 skipped; `make check-fmt`, `make lint`, `make typecheck`, and `make test` run
at the commit gate; the semantic post-rebase audit found every target-only path
byte-identical, every deletion intended, and no reconstructed duplication.
