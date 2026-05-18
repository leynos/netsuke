# 4.1.2. Add a Kani smoke CI job

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item `4.1.2` adds Netsuke's first formal-verification Continuous
Integration (CI) lane. The existing `build-test` job already runs formatting,
linting, tests, coverage generation, and optional CodeScene upload across the
repository's supported Rust toolchains. This work must leave that job
unchanged and add a separate `kani-smoke` job for the fast Kani smoke gate.

After this work is implemented and approved, pull requests will run a dedicated
Kani job that installs the repository-pinned Kani version, runs only
`make kani`, and caches Kani tool downloads separately from ordinary Cargo
artefacts. The visible success criterion is simple: the GitHub Actions workflow
contains a sibling `kani-smoke` job, `build-test` remains intact, and a pull
request can show `kani-smoke` passing without broadening the normal Rust build
matrix or invoking the full proof suite.

This plan is approval-gated. It must be reviewed and explicitly approved before
implementation begins.

## Constraints

- Do not implement this plan until the user explicitly approves it.
- Do not modify the existing `.github/workflows/ci.yml` `build-test` job. The
  implementation may add a sibling job, but it must not change the `build-test`
  matrix, steps, permissions, environment, coverage upload, or
  `continue-on-error` behaviour.
- Do not add Kani to `make test`, `make lint`, `make check-fmt`, `make all`,
  or any ordinary local gate.
- The new CI job must run only the bounded smoke path, currently `make kani`.
  Do not switch the pull-request job to `make kani-full`.
- Do not add Kani proof harnesses, Proptest tests, Verus proofs, Stateright
  modelling, or new Rust verification logic. Those belong to later roadmap
  items.
- Cache Kani tool downloads separately from ordinary Cargo artefacts. The
  implementation must not rely on a shared, broad Cargo cache that obscures
  whether the Kani toolchain is isolated from `build-test`.
- Reuse the repository pin in `tools/kani/VERSION` and the existing
  `scripts/install-kani.sh` and `scripts/check-kani-version.sh` paths where
  possible.
- Keep OrthoConfig unchanged. This is CI wiring, not a new CLI flag, runtime
  configuration field, localized help string, or configuration precedence
  rule.
- Keep `docs/users-guide.md` unchanged unless implementation unexpectedly
  changes externally observable Netsuke behaviour.
- Documentation prose must follow `docs/documentation-style-guide.md` and use
  en-GB-oxendict spelling and grammar.
- Run long validation commands sequentially and capture output with `tee` under
  `/tmp`. Do not run format checks, lints, or tests in parallel.
- Use `coderabbit review --agent` after each major implementation milestone
  and clear all concerns before moving to the next milestone.
- Commit only after gates pass. Use the file-based commit-message workflow
  with `git commit -F`, not `git commit -m`.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 4 changed files beyond this
  ExecPlan, stop and escalate. The expected implementation files are
  `.github/workflows/ci.yml`, `docs/developers-guide.md`, and
  `docs/roadmap.md`.
- CI surface: if preserving `build-test` byte-for-byte becomes impossible,
  stop and present the conflict.
- Interface: if a new Netsuke CLI flag, OrthoConfig field, Fluent message, Rust
  public API, or Make target is required, stop and escalate.
- Dependencies: if a new GitHub Action beyond `actions/cache` is required, or
  if an action must be referenced without a pinned commit SHA, stop and present
  options.
- Runtime: if the cold `kani-smoke` job is expected to exceed 20 minutes, stop
  and revise the cache/install strategy before implementation proceeds.
- Cache certainty: if local or CI probing shows that Kani downloads cannot be
  isolated from ordinary Cargo artefacts with a clear path and key, stop and
  document the alternatives.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and escalate with log locations.
- Review: if `coderabbit review --agent` raises unresolved correctness, CI, or
  documentation concerns, do not proceed until they are addressed or explicitly
  waived.
- Ambiguity: if the choice between pull-request-only execution and
  `workflow_dispatch` execution materially affects branch-protection behaviour,
  stop and ask for direction.

## Risks

- Risk: `scripts/install-kani.sh` currently uses Cargo's default installation
  locations, so a naive cache may mix Kani installer artefacts with ordinary
  Cargo build artefacts. Severity: high. Likelihood: medium. Mitigation:
  prefer a job-local `CARGO_HOME` for the `kani-smoke` job, cache that path
  with a Kani-specific key, and document the chosen paths in the developer
  guide.

- Risk: `cargo kani setup` downloads support files into Kani's default home,
  which may not be covered by a Cargo cache. Severity: medium. Likelihood:
  high. Mitigation: set or discover the Kani home path during implementation,
  cache it separately from ordinary Cargo artefacts, and key it from
  `tools/kani/VERSION`.

- Risk: Kani's official CI documentation notes limited CI platform support.
  Severity: medium. Likelihood: medium. Mitigation: keep `kani-smoke` on a
  single Ubuntu runner and avoid adding a matrix until the repository has real
  proof harnesses and observed runtime data.

- Risk: `make kani` is currently a version-check smoke target rather than a
  substantive proof harness run. Severity: medium. Likelihood: high.
  Mitigation: keep this item scoped to the `4.1.1` smoke contract, state that
  later `4.2.*` work will populate the bounded harness set, and do not add a
  vacuous proof to create false assurance.

- Risk: GitHub Actions cache keys that include the whole workflow file may
  invalidate the Kani cache for unrelated CI edits. Severity: low. Likelihood:
  medium. Mitigation: key primarily by runner operating system and
  `tools/kani/VERSION`, with restore keys that preserve version boundaries.

- Risk: Pull requests from forks can restore caches visible to the base
  branch. Severity: medium. Likelihood: medium. Mitigation: do not cache
  secrets or credentials, and cache only tool downloads and installer outputs.

## Progress

- [x] 2026-05-18T18:50:55Z: Loaded the `leta`, `rust-router`, `kani`,
      `hexagonal-architecture`, `execplans`, `firecrawl`, `commit-message`,
      `pr-creation`, and `en-gb-oxendict-style` skills relevant to this
      planning task.
- [x] 2026-05-18T18:50:55Z: Created a `leta` workspace for the repository.
- [x] 2026-05-18T18:50:55Z: Renamed the branch to
      `4-1-2-kani-smoke-ci-job`.
- [x] 2026-05-18T18:50:55Z: Reviewed `docs/roadmap.md`,
      `docs/formal-verification-methods-in-netsuke.md`,
      `.github/workflows/ci.yml`, `Makefile`, Kani scripts, and the completed
      `4.1.1` ExecPlan.
- [x] 2026-05-18T18:50:55Z: Created context pack `pk_d3k3ep5l` for the agent
      team with the relevant CI, Makefile, script, roadmap, and design
      excerpts.
- [x] 2026-05-18T18:50:55Z: Used two Wyvern agents for CI/cache
      reconnaissance and documentation/testing/boundary reconnaissance.
- [x] 2026-05-18T18:50:55Z: Used Firecrawl to verify current Kani
      installation and CI guidance and GitHub Actions cache behaviour.
- [x] 2026-05-18T18:50:55Z: Drafted this approval-gated ExecPlan.
- [x] 2026-05-18T18:50:55Z: Ran `coderabbit review --agent`; it returned
      zero findings for the draft ExecPlan.
- [x] 2026-05-18T18:50:55Z: Validated the draft with `make check-fmt`,
      `make lint`, `make test`, `make markdownlint`, `make nixie`, and
      `make kani`.
- [ ] Stage A: review the draft plan with the user and obtain explicit
      approval before implementation.
- [ ] Stage B: discover and finalize isolated Kani cache paths.
- [ ] Stage C: add the dedicated `kani-smoke` CI job.
- [ ] Stage D: document the new CI lane and cache convention for developers.
- [ ] Stage E: run review, quality gates, commit, push, and open a draft pull
      request for the implementation.
- [ ] Stage F: after implementation approval and validation, mark roadmap item
      `4.1.2` and its subitems done.

## Surprises & Discoveries

- Roadmap item `4.1.1` is already complete. It added `tools/kani/VERSION`,
  `scripts/install-kani.sh`, `scripts/check-kani-version.sh`, `make kani`,
  `make kani-full`, and `make formal-pr`. This item should reuse those
  contracts rather than redesign Kani tooling.
- The current CI workflow contains one job, `build-test`, with stable,
  minimum supported Rust version (MSRV), and experimental nightly lanes. The
  new Kani job should be a sibling job, not another matrix entry.
- Official Kani installation documentation states that `cargo kani setup`
  places downloaded support files under `~/.kani/` by default, with
  `KANI_HOME` available for a custom path.
- Official Kani CI documentation describes a Kani GitHub Action, but this
  repository already has a pinned installer script and requires pinned action
  SHAs. Reusing the script keeps the CI and local install path aligned.
- GitHub Actions cache documentation confirms that caches are immutable for a
  given key and should not contain sensitive information because pull requests
  can access relevant base-branch caches.
- The Wyvern review agreed that new Rust unit tests, `rstest-bdd` behavioural
  tests, and end-to-end CLI tests are not applicable to this CI-only change.
  The correct validation is workflow review, `make kani`, the normal gates,
  documentation checks, and CodeRabbit review.

## Decision Log

- Decision: Keep this ExecPlan pre-implementation and approval-gated.
  Rationale: The user explicitly stated that the plan must be approved before
  implementation. Date/Author: 2026-05-18 / planning agent.

- Decision: Add `kani-smoke` as a sibling job in `.github/workflows/ci.yml`
  rather than creating a new workflow file. Rationale: roadmap item `4.1.2`
  refers to the existing `build-test` job and the formal-verification design
  describes the first additional job in the current CI workflow. Date/Author:
  2026-05-18 / planning agent with Wyvern input.

- Decision: Use `make kani` as the CI command. Rationale: `4.1.1` established
  `make kani` as the pull-request smoke path and `make kani-full` as the
  full-suite path for later proof work. Date/Author: 2026-05-18 / planning
  agent.

- Decision: Treat OrthoConfig as unchanged for this item. Rationale: this
  roadmap task changes CI infrastructure only; adding runtime configuration,
  localized help, or CLI surface would violate the boundary between developer
  tooling and Netsuke's domain/runtime code. Date/Author: 2026-05-18 /
  planning agent with `hexagonal-architecture` framing.

- Decision: Do not plan new Rust, `rstest`, or `rstest-bdd` tests for this
  item. Rationale: no Rust production path, user-facing CLI behaviour, or
  business invariant changes. The normal gates remain mandatory as regression
  evidence. Date/Author: 2026-05-18 / planning agent with Wyvern input.

- Decision: Prefer job-local cache isolation over broad default Cargo caching.
  Rationale: the acceptance criterion is to cache Kani tool downloads
  separately from ordinary Cargo artefacts. A job-local `CARGO_HOME` and Kani
  home make that separation explicit and auditable. Date/Author: 2026-05-18 /
  planning agent.

## Outcomes & Retrospective

This section is intentionally empty while the plan is in draft. During
implementation, record what changed, what validation proved, any deviations
from this plan, and whether `4.1.2` was marked done in `docs/roadmap.md`.

## Context and orientation

Netsuke is a Rust build-system compiler. It reads a YAML Ain't Markup Language
(YAML) `Netsukefile`, expands MiniJinja-controlled manifest logic, validates a
static Intermediate Representation (IR), emits a deterministic Ninja file, and
delegates execution to the Ninja subprocess. The formal-verification design
identifies the IR and command-interpolation code as high-value proof targets,
but proof harnesses are not part of this item.

The relevant repository files are:

- `docs/roadmap.md`: the source of truth for roadmap item `4.1.2`.
- `docs/formal-verification-methods-in-netsuke.md`: the design document that
  says formal verification should not be folded into `build-test` and that the
  first additional job should be `kani-smoke`.
- `.github/workflows/ci.yml`: the existing CI workflow. It currently defines
  `build-test`; implementation will add a sibling `kani-smoke` job.
- `Makefile`: the local command surface. `make kani` is the smoke target and
  `make kani-full` is reserved for the full proof suite.
- `scripts/install-kani.sh`: the pinned installer path for `kani-verifier` and
  `cargo kani setup`.
- `scripts/check-kani-version.sh`: the smoke check used by `make kani`.
- `tools/kani/VERSION`: the single repository source of truth for the
  supported Kani version.
- `docs/developers-guide.md`: the place to document internal CI and
  formal-verification practices.
- `docs/users-guide.md`: the user manual. It should not need changes unless
  implementation unexpectedly changes externally observable Netsuke behaviour.

## Skills and references

Use these skills while implementing this plan:

- `execplans`: keep this document current as work proceeds.
- `kani`: use for Kani installation, setup, and smoke/full-suite distinctions.
- `hexagonal-architecture`: use to protect the boundary between CI adapters
  and Netsuke domain logic; do not transplant a pattern.
- `leta`: use for Rust code navigation if implementation unexpectedly touches
  Rust source.
- `rust-router`: route any Rust-specific change to the smallest useful Rust
  skill before editing code.
- `firecrawl`: use again if implementation needs fresh external facts about
  Kani, GitHub Actions caching, or action pinning.
- `commit-message`: use the file-based commit-message workflow.
- `pr-creation`: use when creating the draft pull request.

Primary local references:

- `docs/roadmap.md`, especially the `4.1.2` entry.
- `docs/formal-verification-methods-in-netsuke.md`, especially "Continuous
  integration (CI)".
- `docs/execplans/4-1-1-kani-tooling-and-local-smoke-targets.md`, for the
  completed local Kani tooling contract.
- `docs/ortho-config-users-guide.md`, to confirm that no configuration surface
  is needed.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md`, to justify that no new Rust or BDD tests
  are applicable.
- `docs/documentation-style-guide.md`, for Markdown and ADR rules.
- `docs/developers-guide.md`, for internal contributor workflow updates.

External references consulted with Firecrawl:

- Kani installation guide:
  <https://model-checking.github.io/kani/install-guide.html>.
- Kani GitHub CI guide:
  <https://model-checking.github.io/kani/install-github-ci.html>.
- GitHub Actions dependency caching reference:
  <https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching>.

## Plan of work

Stage A is approval. Review this draft with the user and do not begin
implementation until the user explicitly approves the plan or requests
revisions. If the user changes scope, revise this ExecPlan before editing
workflow or documentation files.

Stage B discovers the isolated Kani cache paths. Run the existing installer in
a controlled local shell or CI-equivalent environment and record which paths it
uses. The expected split is a job-local `CARGO_HOME` for `cargo install` output
and a job-local `KANI_HOME` for `cargo kani setup` downloads. Do not use `/tmp`
as a build or tool-cache target. If Kani ignores `KANI_HOME` or writes a
different path, update this plan's `Surprises & Discoveries` and `Decision Log`
before editing the workflow.

Stage C adds the CI job. Edit `.github/workflows/ci.yml` by adding a sibling
job named `kani-smoke` under `jobs:`. Keep the existing `build-test` block
unchanged. The new job should run on `ubuntu-latest`, set
`permissions.contents: read`, set CI-friendly environment variables, check out
the repository with the same pinned checkout action style, set up stable Rust,
restore a dedicated Kani cache, install the pinned Kani toolchain through
`scripts/install-kani.sh`, and run `make kani`.

The implementation shape should be close to:

```yaml
  kani-smoke:
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    permissions:
      contents: read
    env:
      CARGO_TERM_COLOR: always
      RUSTUP_TOOLCHAIN: stable
      CARGO_HOME: ${{ github.workspace }}/.kani-cargo
      KANI_HOME: ${{ github.workspace }}/.kani-home
    steps:
      - uses: actions/checkout@<pinned-sha>
      - name: Setup Rust
        uses: leynos/shared-actions/.github/actions/setup-rust@<pinned-sha>
        with:
          toolchain: stable
      - name: Cache Kani tools
        uses: actions/cache@<pinned-sha>
        with:
          path: |
            .kani-cargo
            .kani-home
          key: ${{ runner.os }}-kani-${{ hashFiles('tools/kani/VERSION') }}
      - name: Install Kani
        run: scripts/install-kani.sh
      - name: Kani smoke
        run: make kani
```

This block is illustrative, not a copy-paste requirement. During
implementation, replace action references with repository-approved pinned SHAs
and update cache paths if Stage B finds a different correct split. The job may
include `workflow_dispatch` only if the user confirms that manual execution is
desired; otherwise keep it pull-request-only to match the roadmap wording.

Stage D updates developer-facing documentation. Add a concise note to
`docs/developers-guide.md` explaining that CI now has a dedicated
`kani-smoke` job, that it installs the pinned Kani version and runs `make kani`
on pull requests, and that the Kani cache is intentionally separate from
ordinary Cargo artefacts. Do not update `docs/users-guide.md` unless an
externally observable user workflow changes.

Stage E validates, reviews, and commits. Run `coderabbit review --agent` after
the CI and documentation milestone. Run the normal gates required by repository
policy. Run documentation checks because this plan and developer documentation
change. Run `make kani` to validate the same smoke path used by CI. If all
gates and review pass, update this ExecPlan's living sections with the
evidence, mark `4.1.2` done in `docs/roadmap.md`, and commit the atomic
implementation.

Stage F pushes and opens a draft pull request. Push the branch so it tracks
`origin/4-1-2-kani-smoke-ci-job`. Create a draft pull request with roadmap
item `(4.1.2)` in the title, mention this ExecPlan in the summary, and include
the Lody session link in a `## References` section.

## Concrete steps

All commands run from the repository root.

```bash
cd /home/leynos/.lody/repos/github---leynos---netsuke/worktrees/48bbbaef-5f08-49d0-9666-fa4506326e10
```

Before editing, confirm the branch and working tree:

```bash
git branch --show-current
git status --short
```

Expected branch:

```plaintext
4-1-2-kani-smoke-ci-job
```

During implementation, confirm the existing `build-test` block before and
after the workflow edit:

```bash
git diff -- .github/workflows/ci.yml
```

The diff must show an added sibling `kani-smoke` job and must not alter the
existing `build-test` lines.

Probe Kani cache paths before finalizing the workflow:

```bash
CARGO_HOME="$PWD/.kani-cargo" \
KANI_HOME="$PWD/.kani-home" \
scripts/install-kani.sh 2>&1 | tee /tmp/install-kani-netsuke-4-1-2-kani-smoke-ci-job.out

find .kani-cargo .kani-home -maxdepth 2 -type d | sort
```

Remove local probe directories after recording the result if they are created
in the worktree:

```bash
rm -rf .kani-cargo .kani-home
```

Run the smoke path:

```bash
make kani 2>&1 | tee /tmp/kani-netsuke-4-1-2-kani-smoke-ci-job.out
```

Run the project gates sequentially:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-4-1-2-kani-smoke-ci-job.out
make lint 2>&1 | tee /tmp/lint-netsuke-4-1-2-kani-smoke-ci-job.out
make test 2>&1 | tee /tmp/test-netsuke-4-1-2-kani-smoke-ci-job.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-4-1-2-kani-smoke-ci-job.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-4-1-2-kani-smoke-ci-job.out
```

Run the agent review after the CI/docs milestone:

```bash
coderabbit review --agent
```

Inspect and commit with the file-based commit-message workflow:

```bash
git diff --stat
git diff
git status --short
git add .github/workflows/ci.yml docs/developers-guide.md \
  docs/execplans/4-1-2-kani-smoke-ci-job.md docs/roadmap.md
COMMIT_MSG_DIR="$(mktemp -d)"
$EDITOR "$COMMIT_MSG_DIR/COMMIT_MSG.md"
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

The implementation commit message subject should be imperative, for example:

```plaintext
Add Kani smoke CI job
```

## Validation and acceptance

The implementation is accepted only when all of these behaviours are true:

- `.github/workflows/ci.yml` contains a dedicated `kani-smoke` job.
- The existing `build-test` job is unchanged.
- `kani-smoke` runs on pull requests and runs only `make kani` as the
  verification command.
- `kani-smoke` installs Kani through `scripts/install-kani.sh`, which reads
  `tools/kani/VERSION`.
- `kani-smoke` caches Kani tool downloads separately from ordinary Cargo
  artefacts, using a Kani-specific cache key and path.
- `kani-smoke` does not run `make kani-full`, `make test`, `make lint`,
  `make check-fmt`, coverage, or CodeScene upload.
- `docs/developers-guide.md` documents the new CI lane and cache convention.
- `docs/users-guide.md` remains unchanged unless there is a real user-facing
  behaviour change.
- No new Rust unit tests, `rstest-bdd` behavioural tests, or end-to-end CLI
  tests are added unless implementation unexpectedly changes Rust runtime or
  CLI behaviour.
- `make kani`, `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` pass.
- `coderabbit review --agent` has no unresolved concerns.
- After implementation, `docs/roadmap.md` marks `4.1.2` and its three subitems
  done.

## PR preparation

After the implementation commit is ready and pushed, create a draft pull
request. The title should include the roadmap item:

```plaintext
Add Kani smoke CI job (4.1.2)
```

The description should identify this branch as the implementation of roadmap
item `4.1.2`, link this ExecPlan, summarize the dedicated `kani-smoke` job, and
state that `build-test` remains unchanged. Include validation evidence from the
commands above.

Run this command and include its result as a Lody session link in the final
`## References` section:

```bash
echo ${LODY_SESSION_ID}
```

The references section should include:

```markdown
## References

- Lody session:
  <https://lody.ai/leynos/sessions/48bbbaef-5f08-49d0-9666-fa4506326e10>
```
