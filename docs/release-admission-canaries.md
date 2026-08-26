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

| Downstream repository      | Base revision                              | Migration branch                | Selected targets                                                                                                               |
| -------------------------- | ------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `leynos/repovec-appliance` | `4881161b10592530ea878a41a4f043ee061442ca` | `issue-598-v010-netsuke-canary` | `all`, `check-fmt`, `lint`, `test`                                                                                             |
| `leynos/mxd`               | `7eefacd9f915f80fe93de9f68afa0d7b8e83dec3` | `issue-598-v010-netsuke-canary` | `check-fmt`, `lint-postgres`, `lint-sqlite`, `lint-wireframe-only`, `test-postgres`, `test-sqlite`, `test-wireframe-only`      |
| `leynos/ortho-config`      | `dbeed53c4c2e8e0f94568bb0e24ed9e1864aa5b6` | `issue-598-v010-netsuke-canary` | Linux: `check-fmt`, `lint`, `test`, `markdownlint`; Windows: `check-fmt`, `lint-clippy`, `test`, `powershell-wrapper-validate` |

The candidate revision is intentionally recorded in each downstream commit and
workflow, rather than inferred from an action tag. The v0.1.0 candidate carries
the beta2 package version until final release packaging changes it.

## Deliberate migration boundaries

- Repovec Appliance uses `command: ":"` for its serial `all` action. v0.1.0
  requires a recipe even for dependency-only actions; [#572][issue-572] tracks
  its removal for v0.1.1.
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

The release workflow's admission job runs the pinned Linux canaries and checks
their structured results. The OrthoConfig migration workflow additionally runs
its Windows target set on `windows-latest`; its successful run is a required
review input. A failure blocks v0.1.0 only when it violates a supported v0.1.0
manifest or runtime contract. Ergonomic gaps remain follow-up work, not release
scope expansion.

[installer]: ../.github/actions/install-release-candidate/action.yml
[issue-572]: https://github.com/leynos/netsuke/issues/572
