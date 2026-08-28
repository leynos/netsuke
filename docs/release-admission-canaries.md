# Release-admission migration canaries

The v0.1.0 release candidate is admitted against three maintained downstream
repositories, not synthetic copies in this repository. Each downstream branch
pins the candidate's full Git revision and invokes the
[`install-release-candidate` action][installer]. The action checks out that
revision, builds its release binary, and rejects a version mismatch before a
canary invokes `netsuke`.

The installer is the shared infrastructure adapter. It owns only candidate
retrieval and identity verification; each downstream `Netsukefile` owns its
commands, target names, and platform-specific toolchain. This keeps release
admission from substituting a Netsuke-local imitation for downstream build
orchestration.

Every canary emits one bounded JSON record for each requested target. The
record identifies the repository, pinned downstream revision, candidate
revision and version, target, platform, and final status. Command output stays
in the CI log, so the structured record remains appropriate for release review.

## Pinned v0.1.0 candidate set

This table records the pinned downstream migration revisions and their selected
release-admission targets.

| Downstream repository      | Pinned migration revision                  | Migration branch                | Selected targets                                                                                                          |
| -------------------------- | ------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `leynos/repovec-appliance` | `6be365b4b30ef48537add5719a9b387ccc41777f` | `issue-598-v010-netsuke-canary` | `all`, `check-fmt`, `lint`, `test`                                                                                        |
| `leynos/mxd`               | `8146278cc82506c222bb78d4f3fc05c12ed95b41` | `issue-598-v010-netsuke-canary` | `check-fmt`, `lint-postgres`, `lint-sqlite`, `lint-wireframe-only`, `test-postgres`, `test-sqlite`, `test-wireframe-only` |
| `leynos/ortho-config`      | `b42b5d0adfacd79456d2a2f9edbf9f561aac943b` | `issue-598-v010-netsuke-canary` | Linux: `check-fmt`, `lint`, `test`, `markdownlint`, `generated-config`; Windows: `powershell-wrapper-validate`            |

The candidate revision is intentionally recorded in each downstream commit and
workflow, rather than inferred from an action tag. The v0.1.0 candidate carries
the beta2 package version until final release packaging changes it.

Admission is fail-closed. For each table row, the release workflow reads the
downstream `netsuke-canary.yml` at the pinned migration revision and requires
both the installer action reference and its `revision` input to identify the
exact published `GITHUB_SHA`. It then queries the pinned workflow ID with
`head_sha` set to the migration revision and accepts only a run whose repository
and workflow ID and path, `push` event, migration branch, head SHA, candidate
name, completed status, and successful conclusion all match. A new candidate SHA
therefore requires fresh downstream workflow evidence; earlier canary runs
cannot admit a later release candidate.

## Deliberate migration boundaries

- Repovec Appliance uses `command: ":"` for its serial `all` action. v0.1.0
  requires a recipe even for dependency-only actions; [#572][issue-572] tracks
  its removal for v0.1.1. [#597][issue-597] tracks the v0.1.1 gate that will
  remove the synthetic no-op and exercise the replacement, while
  [#599][issue-599] tracks the native Windows legacy-recipe shell contract and
  smoke coverage as a separate Windows boundary.
- MXD keeps separate target names for PostgreSQL, SQLite, and wireframe-only.
  The manifest does not collapse mutually exclusive feature lanes into an
  all-features command.
- OrthoConfig keeps tool-heavy implementation in existing Make targets and
  helper scripts where that is its stable cross-platform boundary. Its
  Netsukefile selects those gates without translating PowerShell or Python
  logic into opaque shell strings.
- Existing Makefiles remain for out-of-slice targets and contributor workflows.
  The selected release-admission targets execute from Netsukefiles in CI.

## Release decision

The release workflow checks the successful, identity-bound run for each pinned
revision before it publishes v0.1.0. The OrthoConfig migration workflow
additionally runs its Windows target set on `windows-latest`; its successful run
is a required review input. A failure blocks v0.1.0 only when it violates a
supported v0.1.0 manifest or runtime contract. Ergonomic gaps remain follow-up
work, not release scope expansion.

[installer]: ../.github/actions/install-release-candidate/action.yml
[issue-572]: https://github.com/leynos/netsuke/issues/572
[issue-597]: https://github.com/leynos/netsuke/issues/597
[issue-599]: https://github.com/leynos/netsuke/issues/599
