# Release-admission migration canaries

The v0.1.0 release candidate is admitted against three maintained downstream
repositories, not synthetic copies in this repository. Each canary checks out a
pinned commit of a real downstream branch, builds the exact Netsuke candidate,
and runs that repository's own `Netsukefile` gates with it. The three
repositories have materially different Makefile shapes, so together they test
task-graph replacement on real build orchestration rather than on Netsuke's own
manifests and quality gates.

A canary failure blocks publication only when it exposes a defect in behaviour
that v0.1.0 already claims to support. A missing ergonomic feature is a
narrowly scoped follow-up, such as [#572][issue-572], or v0.2.0 RFC work; it is
never a reason to weaken the canary or add manifest syntax to the hardening
release.

## How the canaries run

The release workflow runs the canaries itself, in the `downstream-canaries` job
on `ubuntu-latest` and the `downstream-canaries-windows` job on
`windows-latest`. Both jobs call the shared
[`downstream-canary` composite action][action], which does the same thing for
every canary:

1. Check out the downstream repository at its pinned commit into
   `downstream/`, beside Netsuke's own checkout rather than inside it. Cargo
   reads `.cargo/config.toml` from every ancestor directory, so a nested
   checkout would build with Netsuke's development `mold` flags.
2. Build the candidate with the
   [release-candidate installer][installer], which fetches the exact commit,
   builds it in the shipped release shape, and refuses a binary that reports a
   version other than the candidate's.
3. Install only the tools that canary names, from a closed set.
4. Run `netsuke --verbose generate --output build.ninja` in the checkout, then
   `ninja -f build.ninja <target>` for each selected target.
5. Always write the bounded provenance record described below.

The candidate is resolved once per run by `release-candidate`, using
`.github/scripts/resolve_release_candidate.py`:

- A tag release and a pull request's dry run build the commit the run is for.
- A manual rehearsal builds the ref selected in the Actions UI, unless its
  `candidate-ref` input names another ref. `auto` selects the newest `-rcN` tag
  while it is also the newest version tag, and otherwise `main`.

The canaries run on every tag release and every manual rehearsal. On a pull
request they run only when the pull request is marked ready for review, since
each canary compiles a whole downstream workspace, and they never gate a pull
request: the jobs are `continue-on-error` there, so a downstream failure is
visible without blocking the merge. A release needs both canary jobs to
succeed, and needs the candidate they built to be the release commit itself.

## Pinned v0.1.0 candidate set

This table records the pinned downstream revisions and their selected targets.
`tests/workflow_release_canaries.rs` holds the release workflow's matrix pins
equal to this table.

| Downstream repository      | Pinned revision                            | Selected targets                                                                                               | Lane selector (`MXD_BACKEND`)          | Platforms      |
| -------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------- | -------------------------------------- | -------------- |
| `leynos/repovec-appliance` | `b1393fd70b552aa479c66ae6222ed0b2bd0879ae` | `all`                                                                                                          | —                                      | Linux          |
| `leynos/mxd`               | `737048017901868a885ae23dcd4c0473242eb765` | `check-fmt`, `lint`, `test`                                                                                    | `postgres`, `sqlite`, `wireframe-only` | Linux          |
| `leynos/ortho-config`      | `64cd6cb9526d1b7742373bc466a2121f8d395b50` | Linux: `check-fmt`, `lint`, `test`, `markdownlint`, `generated-config`; Windows: `powershell-wrapper-validate` | —                                      | Linux, Windows |

Every pin is a commit on the downstream `issue-598-v010-netsuke-canary` branch,
and is also retained by a `netsuke-canary/<commit>` tag in that repository. A
branch can be rewritten or deleted; the tag keeps the pinned commit fetchable
for as long as the release needs it. Do not move or delete a retaining tag
while a release, or a rehearsal of one, still names its commit.

## Distinctive contracts

- **Repovec Appliance** runs `all`, a serial aggregate action over
  `check-fmt`, `lint`, `test`, and `package` with `dependency_order: serial`.
  Every gate selects the complete workspace with `--workspace`,
  `--all-targets`, and `--all-features` where the command accepts them, and
  every compiling gate denies warnings through its recipe. Whitaker runs
  unconditionally.
- **MXD** runs one matrix row per feature lane. `MXD_BACKEND` selects the lane,
  and the `Netsukefile` declares `lint` and `test` once per lane with a
  manifest-time `when`, so the other lanes are removed before Netsuke builds
  its graph. Each row also forbids the other lanes' feature arguments in the
  generated `build.ninja`: a match fails the canary before any target runs.
  Only the PostgreSQL row starts a PostgreSQL service.
- **OrthoConfig** runs a mixed Rust, Python, Markdown, and generated
  configuration slice on Linux, and its PowerShell wrapper validation on a
  normal GitHub-hosted Windows runner, driven through PowerShell.

## Provenance

Each canary writes one `downstream-canary-provenance.json` record and one line
in the job summary. The record names the downstream repository, the pinned and
observed downstream revisions, the Netsuke commit and version, the platform,
the lane selectors, and the status of generation, lane isolation, and every
selected target, using only `passed`, `failed`, or `not_run`. A run whose
checkout is not at the pinned revision cannot pass. Command output stays in the
job log, and a service connection string reaches the tools without being
recorded.

When workflow artefact uploads are enabled, each record is uploaded as
`downstream-canary-provenance-<canary>`, beside the release-admission metrics
and traces. A dry run keeps the record in the job summary. The record is a
provenance artefact, not a metric or trace series; see
[ADR-020](adr-020-release-admission-observability.md).

## Deliberate migration boundaries

- Repovec Appliance keeps `command: ":"` on its serial `all` action, because
  v0.1.0 requires a recipe even for a dependency-only action. [#572][issue-572]
  removes that requirement, and the v0.1.1 gate in [#597][issue-597] extends
  this canary to remove the synthetic no-op; that extension is not a
  prerequisite for v0.1.0.
- MXD keeps Whitaker in its own pinned CI job and uses warning-denied Clippy per
  lane, so the canary tests Netsuke's orchestration rather than a Dylint suite.
- OrthoConfig keeps its tool-heavy implementation in focused helpers:
  `scripts/generate_typos_config.py`, its Python tests, and
  `scripts/validate_powershell_wrapper.ps1`. Its `generated-config` target
  renders from the shared spelling dictionary at the commit that produced the
  committed `typos.toml`, so a later dictionary change cannot fail the canary.
  The broader Windows legacy-recipe shell contract is tracked in
  [#599][issue-599].
- Every canary keeps an explicit empty `targets: []`, which the v0.1.0 schema
  requires for an action-only manifest.
- Every Makefile remains for out-of-slice targets and contributor workflows.
  The selected targets execute from each `Netsukefile`, and no action delegates
  back to `make`.

Each downstream branch's `docs/netsuke-release-canary.md` records its own
retained boundaries in more detail.

## Updating a pin

1. Commit the change to the downstream `issue-598-v010-netsuke-canary` branch
   and push it.
2. Push a retaining tag for the new commit:
   `git tag netsuke-canary/<commit> <commit>` and
   `git push origin refs/tags/netsuke-canary/<commit>`. Keep the old pin's tag
   until no release or rehearsal names it.
3. Update the commit in every matrix row for that repository in
   `.github/workflows/release.yml`, and in the table above.
4. Run a manual rehearsal: the **Release Dry Run** workflow with **Run
   workflow**, which runs every canary without publishing.

## Release decision

Publication of v0.1.0 needs every canary to pass against the release commit.
The v0.1.0 gate in [#594][issue-594] requires a green canary run. Ergonomic
gaps remain follow-up work, and cannot weaken this admission rule or provide an
exception to it.

[action]: ../.github/actions/downstream-canary/action.yml
[installer]: ../.github/actions/install-release-candidate/action.yml
[issue-572]: https://github.com/leynos/netsuke/issues/572
[issue-594]: https://github.com/leynos/netsuke/issues/594
[issue-597]: https://github.com/leynos/netsuke/issues/597
[issue-599]: https://github.com/leynos/netsuke/issues/599
