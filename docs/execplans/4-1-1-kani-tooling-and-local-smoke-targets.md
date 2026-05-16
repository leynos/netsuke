# 4.1.1. Add Kani tooling and local smoke targets

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `4.1.1` starts Netsuke's formal-verification work by adding the
local tooling contract for Kani, a bounded model checker for Rust. This work
does not add proof harnesses yet. It creates the repeatable local path that
later roadmap items can use to run Kani proofs without disturbing the existing
`make check-fmt`, `make lint`, and `make test` workflow.

After this work is complete, a developer can run one script to install the
repository-supported Kani version, run `make kani` for fast local smoke
verification, run `make kani-full` for the full Kani suite once real harnesses
exist, and run `make formal-pr` as the formal-verification pull-request gate.
The supported Kani version will be pinned in `tools/kani/VERSION`, so local
development and future Continuous Integration (CI) jobs use the same tool.

The implementation must keep the ordinary build, lint, and test paths intact.
Formal verification remains opt-in until later roadmap work adds a dedicated CI
job and substantive proof harnesses.

## Constraints

- Do not implement Kani proof harnesses, Proptest tests, Verus proofs, or a
  CI `kani-smoke` job in this roadmap item. Those belong to later items
  `4.1.2`, `4.2.*`, `4.3.*`, and `4.4.*`.
- Do not add Kani execution to `make test`, `make lint`, `make check-fmt`,
  `make all`, or the existing `.github/workflows/ci.yml` `build-test` job.
- Use `tools/kani/VERSION` as the single repository source of truth for the
  supported Kani version.
- Implement `scripts/install-kani.sh` in the existing script style: Bash,
  `set -euo pipefail`, clear usage/failure messages, and no hidden mutation of
  repository files.
- Keep Makefile additions consistent with the current target style: variables
  near the top, `.PHONY` declarations, target descriptions using `##`, and
  recipes that honour overrideable variables.
- Preserve the existing OrthoConfig and localised CLI help surfaces. This
  item adds developer tooling, not a user-facing Netsuke subcommand or flag.
- If any Kani-specific Rust source is introduced later during implementation,
  it must be gated with `#[cfg(kani)]` and `Cargo.toml` must declare
  `cfg(kani)` under `unexpected_cfgs` before normal builds see it. This plan
  does not require source-level Kani code.
- Documentation prose must follow `docs/documentation-style-guide.md`, using
  en-GB-oxendict spelling and wrapped Markdown.
- Before committing the plan or later implementation changes, run the relevant
  quality gates and capture long outputs with `tee` under `/tmp`.
- Use `coderabbit review --agent` after each major milestone and clear all
  plan concerns before moving to the next milestone.
- Commit only after gates pass. Use a file-based commit message with
  `git commit -F`, not `git commit -m`.

## Tolerances

- Scope: if implementation requires more than 6 changed files or more than
  roughly 250 net new lines, stop and escalate.
- Interface: if satisfying the task requires a new Netsuke CLI flag,
  subcommand, OrthoConfig field, Fluent message, or public Rust API, stop and
  escalate.
- CI scope: if adding or editing `.github/workflows/ci.yml` appears necessary,
  stop and confirm whether roadmap item `4.1.2` should be pulled forward.
- Harness scope: if useful `make kani` behaviour seems to require adding real
  proof harnesses, stop and record the split between `4.1.1` and `4.2.*`.
- Versioning: if the latest stable Kani release cannot be installed by
  `cargo install --locked kani-verifier --version <version>` on this
  environment, stop and present version options with evidence.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and escalate with logs.
- Review: if `coderabbit review --agent` raises unresolved correctness,
  safety, or documentation concerns, do not proceed to the next milestone until
  they are addressed or explicitly waived.

## Risks

- Risk: The pinned Kani version may require a Rust nightly or support bundle
  that changes outside the repository's stable `rust-toolchain.toml`. Severity:
  medium. Likelihood: medium. Mitigation: use the official `cargo kani setup`
  path, document that Kani manages its own supporting toolchain, and keep the
  normal Cargo workflow unchanged.

- Risk: Kani is slower than unit tests, so adding it to existing quality gates
  would make routine development more expensive. Severity: high. Likelihood:
  high. Mitigation: keep `make kani`, `make kani-full`, and `make formal-pr`
  explicit opt-in targets for this item.

- Risk: `make kani` has no real proof harnesses immediately after `4.1.1`.
  Severity: medium. Likelihood: high. Mitigation: define it as a smoke target
  that proves the pinned Kani command is wired correctly and document that
  later roadmap items populate the harness set.

- Risk: Installing Kani with the moving `latest` version would drift from
  documented local behaviour. Severity: high. Likelihood: medium. Mitigation:
  make `scripts/install-kani.sh` read `tools/kani/VERSION` and pass the
  resulting version to `cargo install --locked kani-verifier --version`.

- Risk: Platform support differs from the ordinary Rust toolchain. Severity:
  medium. Likelihood: medium. Mitigation: document official Kani support for
  Linux and macOS in developer-facing notes, and leave Windows CI decisions to
  `4.1.2`.

- Risk: The script might reinstall Kani unnecessarily or mask a mismatched
  installed binary. Severity: low. Likelihood: medium. Mitigation: have the
  script print the pinned version, install that exact package version, run
  `cargo kani setup`, and finish with a version/help check.

## Progress

- [x] 2026-05-10T20:26:51Z: Loaded the `execplans`, `leta`, `rust-router`,
      `kani`, `hexagonal-architecture`, `firecrawl`, `en-gb-oxendict-style`,
      and `commit-message` skills relevant to this planning task.
- [x] 2026-05-10T20:26:51Z: Confirmed the branch is
      `feat/kanismokeexecplan`, not the main branch.
- [x] 2026-05-10T20:26:51Z: Reviewed `docs/roadmap.md`,
      `docs/formal-verification-methods-in-netsuke.md`, `Makefile`,
      `Cargo.toml`, script conventions, OrthoConfig documentation, and the
      testing guides.
- [x] 2026-05-10T20:26:51Z: Used two Wyvern agents for repository-local and
      tooling/prior-art reconnaissance.
- [x] 2026-05-10T20:26:51Z: Used Firecrawl against official Kani
      documentation and GitHub releases to verify installation, usage, and
      release facts.
- [x] 2026-05-10T20:26:51Z: Drafted this approval-gated ExecPlan.
- [x] 2026-05-10T20:26:51Z: Ran `coderabbit review --agent` on the draft and
      addressed its terminology concern. The prose wrapping concern was
      already satisfied for non-code lines after local formatter processing;
      remaining over-80-column lines are command examples inside code fences.
- [x] 2026-05-10T20:26:51Z: Reran `coderabbit review --agent` and clarified
      that the absolute repository path in the command examples is
      machine-specific.
- [x] 2026-05-10T20:26:51Z: Validated the draft with `make check-fmt`,
      `make lint`, `make test`, `make markdownlint`, and `make nixie`.
- [x] 2026-05-11T00:00:00Z: User explicitly approved implementation by asking
      to proceed with the planned functionality.
- [x] Stage A: review the draft plan with the user and obtain explicit
      approval before implementation.
- [x] 2026-05-11T00:00:00Z: Rechecked current public release information and
      confirmed `kani-verifier` `0.67.0` remains the latest visible release.
- [x] Stage B: add the Kani version pin and installer script.
- [x] 2026-05-11T00:00:00Z: Ran `coderabbit review --agent` for the staged
      Kani version pin and installer milestone; it returned zero findings.
- [x] 2026-05-11T00:00:00Z: Validated the first milestone with
      `make markdownlint` and `make check-fmt`.
- [x] 2026-05-11T00:00:00Z: Committed the version pin and installer milestone
      as `6d17699`.
- [x] Stage C: add Makefile formal-verification targets.
- [x] Stage D: document developer-facing usage and validate the local
      workflow.
- [x] 2026-05-11T00:00:00Z: Ran `scripts/install-kani.sh`; it installed
      `kani-verifier` `0.67.0`, ran `cargo kani setup`, and reported
      `cargo-kani 0.67.0`.
- [x] 2026-05-11T00:00:00Z: Ran `make kani`, `make formal-pr`, and
      `make kani-full`; all exited successfully.
- [x] 2026-05-11T00:00:00Z: Ran `coderabbit review --agent` for the staged
      Makefile and developer-documentation milestone; it returned zero
      findings.
- [x] 2026-05-11T00:00:00Z: Validated the implementation with
      `mbake validate Makefile`, `make check-fmt`, `make markdownlint`,
      `make nixie`, `make lint`, and `make test`.
- [x] 2026-05-11T00:00:00Z: Attempted `make fmt`; it still fails on
      pre-existing Markdown line-length and table issues in unrelated files.
      Formatter-only edits outside the planned scope were restored.
- [x] 2026-05-11T00:00:00Z: Marked roadmap item `4.1.1` and its three
      subitems done after validation passed.
- [x] 2026-05-11T00:00:00Z: Ran final `coderabbit review --agent` after the
      roadmap and completion updates; it returned zero findings.
- [x] Stage E: run review, quality gates, commit, and update the roadmap.
- [x] 2026-05-16T00:00:00Z: Addressed review feedback by routing
      `make kani` through `scripts/check-kani-version.sh`, which compares the
      installed `cargo-kani` version with `tools/kani/VERSION`; corrected
      `localized` to `localised`; and expanded YAML on first use.
- [x] 2026-05-16T00:00:00Z: Validated the review fix with `make kani`,
      `make formal-pr`, `mbake validate Makefile`, `make check-fmt`,
      `make markdownlint`, `make nixie`, `make lint`, `make test`, and a
      negative smoke check using a fake mismatched Kani command. `make fmt`
      still fails on pre-existing repository-wide Markdown line-length issues.

## Surprises & Discoveries

- The repository already contains a focused formal-verification design in
  `docs/formal-verification-methods-in-netsuke.md`, so `4.1.1` is a tooling
  task rather than a proof-design task.
- No `tools/` directory exists yet. This item establishes the first
  `tools/<tool>/VERSION` convention in the repository.
- The current `Makefile` is intentionally small and uses `##` comments to
  populate `make help`; the new targets should remain visible through that
  convention.
- OrthoConfig and localised help are important to the project, but this item
  does not need a CLI change. The correct boundary is developer tooling and
  developer documentation.
- Official Kani documentation recommends `cargo kani` for Cargo package
  integration and recommends wrapping proof harnesses in `#[cfg(kani)]` so
  ordinary builds remain unaffected.
- Firecrawl found the latest visible upstream GitHub release as `kani-0.67.0`,
  published on 2026-01-16. Treat this as the candidate pin during
  implementation, and verify it with `cargo install` before finalizing.
- A fresh release check on 2026-05-11 still showed `kani-verifier` `0.67.0`
  as the current version. The implementation pins `tools/kani/VERSION` to
  `0.67.0`.
- `shellcheck` is documented as available in `AGENTS.md`, but it is not
  installed in this worktree environment (`shellcheck: command not found`).
  Shell validation for this item relies on review, Bash strict mode, and direct
  command execution.
- `make kani` is implemented as `cargo kani --version` for `4.1.1` because no
  substantive proof harnesses exist yet. `make kani-full` already invokes the
  full `cargo kani` path so later `4.2.*` harness work can populate it without
  changing the target name.
- `make kani-full` exits successfully before proof harnesses exist and reports
  "No proof harnesses (functions with #[kani::proof]) were found to verify."
  This confirms the full target can already be wired without adding a dummy
  proof. During Kani's build path, existing `src/stdlib/path/hash_utils.rs`
  code emits an unused-variable warning for `err`; this is not introduced by
  this tooling change and will be judged by the normal lint gate.
- `make fmt` currently fails on pre-existing Markdown line-length violations
  across unrelated repository documentation. The formatter's unrelated edits
  were restored, and the new ExecPlan passes `make markdownlint`.
- The original smoke target only executed `cargo kani --version`, which proved
  that some Kani command was callable but did not enforce the repository pin.
  The smoke path now reads `tools/kani/VERSION` and fails when the installed
  `cargo-kani` version differs.

## Decision Log

- Decision: Keep `4.1.1` scoped to Kani tooling, installer script, and local
  Make targets. Rationale: The roadmap separates tooling (`4.1.1`), CI
  (`4.1.2`), Kani harnesses (`4.2.*`), Proptest (`4.3.*`), and contracts/Verus
  (`4.4.*`). Mixing them would make approval and rollback harder. Date/Author:
  2026-05-10 / planning agent with Wyvern input.

- Decision: Use `tools/kani/VERSION` as the only Kani version pin.
  Rationale: The formal-verification design already specifies `tools/*/VERSION`
  files, and a single pin prevents local/CI drift. Date/Author: 2026-05-10 /
  planning agent.

- Decision: Prefer the official Cargo install path over a bespoke binary
  download for the first Kani integration. Rationale: Kani's installation guide
  documents `cargo install --locked kani-verifier --version <VERSION>` followed
  by `cargo kani setup`; using that path avoids maintaining per-platform asset
  selection before the project has CI requirements for it. Date/Author:
  2026-05-10 / planning agent, based on Firecrawl research.

- Decision: `formal-pr` should invoke the fast smoke target rather than
  `kani-full`. Rationale: The design document states that `make kani` is the
  pull-request smoke path and `make kani-full` is the deeper on-demand path.
  Date/Author: 2026-05-10 / planning agent.

- Decision: Do not change Netsuke's CLI or OrthoConfig schema for this item.
  Rationale: These targets are repository developer tools, not end-user runtime
  behaviour. Date/Author: 2026-05-10 / planning agent with Wyvern input.

- Decision: Validate `tools/kani/VERSION` as a bare `MAJOR.MINOR.PATCH`
  version in `scripts/install-kani.sh`. Rationale: the version file is a small
  human-edited contract, and rejecting prefixed or empty values prevents
  accidentally installing a moving or malformed tool version. Date/Author:
  2026-05-11 / implementation agent.

- Decision: Make `make kani` a Kani command smoke check using `cargo kani
  --version` until real harnesses land. Rationale: this keeps `4.1.1` honest
  as a tooling item, avoids adding a vacuous proof, and leaves `make
  kani-full` as the future full-suite entry point. Date/Author: 2026-05-11 /
  implementation agent.

- Decision: Route `make kani` through `scripts/check-kani-version.sh` rather
  than calling `cargo kani --version` directly. Rationale: the smoke path must
  enforce the pinned Kani contract, not merely prove that any installed Kani
  binary is callable. Date/Author: 2026-05-16 / review-fix agent.

## Outcomes & Retrospective

This section is intentionally empty while the plan is in draft. During
implementation, update it after each milestone with what was achieved, what
changed from the plan, and what later formal-verification work should inherit.

Implementation completed on 2026-05-11. Netsuke now has a pinned Kani version
in `tools/kani/VERSION`, an idempotent `scripts/install-kani.sh` installer, and
local `make kani`, `make kani-full`, and `make formal-pr` targets. The
developer guide documents how to install and run the pinned tool without
folding Kani into the ordinary `make test`, `make lint`, `make check-fmt`, or
`make all` workflow.

The main deviation from the draft plan is deliberate: `make kani` is a command
smoke target that compares the installed `cargo-kani` version with
`tools/kani/VERSION`, while `make kani-full` runs the full `cargo kani`
command. This avoids introducing a non-substantive proof harness before roadmap
item `4.2.*`. Actual execution confirmed that `make kani-full` exits
successfully today and reports that no `#[kani::proof]` harnesses exist.

## Context and orientation

Netsuke is a Rust build-system compiler. It reads a YAML Ain't Markup Language
(YAML) `Netsukefile`, expands MiniJinja-controlled manifest logic, validates a
static Intermediate Representation (IR), emits a deterministic Ninja file, and
delegates execution to the Ninja subprocess. The formal-verification design
identifies the IR and command-interpolation code as high-value proof targets,
but those proof harnesses are not part of this item.

The relevant repository files are:

- `docs/roadmap.md`: the source of truth for roadmap item `4.1.1`.
- `docs/formal-verification-methods-in-netsuke.md`: the design document that
  specifies `tools/kani/VERSION`, `scripts/install-kani.sh`, `make kani`,
  `make kani-full`, and `make formal-pr`.
- `Makefile`: the local developer command surface. It currently contains
  build, release, clean, test, lint, format, Markdown lint, Mermaid validation,
  and help targets.
- `scripts/`: the existing home for small Bash helper scripts.
- `docs/developers-guide.md`: the place to document internal developer
  practices, including the new Kani workflow.
- `docs/users-guide.md`: the user manual. It should not need changes unless
  implementation unexpectedly changes externally observable Netsuke behaviour.
- `Cargo.toml`: the package manifest. It does not need a Kani dependency for
  this item, unless later implementation introduces `#[cfg(kani)]` source.

Kani is a bounded model checker for Rust. In this plan, a smoke target means a
fast target that proves the local Kani command is installed and wired
correctly. A full target means a target intended to run all Kani proof
harnesses once later roadmap items add them.

## Skills and references

Use these skills while implementing this plan:

- `leta`: use for Rust code navigation if implementation unexpectedly touches
  Rust source.
- `rust-router`: route any Rust-specific change to a smaller Rust skill before
  editing code.
- `kani`: use for Kani command structure, `#[cfg(kani)]` boundaries, and
  harness-tier conventions.
- `hexagonal-architecture`: use only to protect the domain/tooling boundary;
  do not force a pattern transplant into a Makefile/script change.
- `execplans`: keep this document current as work proceeds.
- `commit-message`: use the file-based commit-message workflow.

Primary local references:

- `docs/roadmap.md`, especially the `4.1.1` entry.
- `docs/formal-verification-methods-in-netsuke.md`, especially the
  "Repository integration plan" and "Makefile and local workflow" sections.
- `docs/ortho-config-users-guide.md`, to confirm that no configuration
  surface is needed.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md`, to decide whether implementation tests are
  applicable.
- `docs/documentation-style-guide.md`, for Markdown and ADR rules.
- `docs/developers-guide.md`, for internal contributor workflow updates.

External references consulted with Firecrawl:

- Kani installation guide:
  <https://model-checking.github.io/kani/install-guide.html>.
- Kani usage guide:
  <https://model-checking.github.io/kani/usage.html>.
- Kani GitHub releases:
  <https://github.com/model-checking/kani/releases>.

## Plan of work

Stage A is approval. Review this draft with the user and do not begin
implementation until the user explicitly approves the plan or requests
revisions. If the user changes scope, revise this ExecPlan before editing
tooling files.

Stage B adds the version pin and installer. Create `tools/kani/VERSION` with
the chosen supported version, expected initially to be `0.67.0` after
verification against upstream and local installation. Create
`scripts/install-kani.sh` as a strict Bash script. The script should resolve
the repository root, read the version pin, print the version it is installing,
run:

```bash
cargo install --locked kani-verifier --version "$KANI_VERSION"
cargo kani setup
```

Then it should run a lightweight command such as `cargo kani --help` or
`kani --version` to prove the installed command is callable. Do not write into
the repository during installation. If a custom Kani home is needed, make it an
environment variable such as `KANI_HOME` that callers may set; do not force an
isolated cache by default.

Stage C adds the Makefile targets. Extend the `.PHONY` line with `kani`,
`kani-full`, and `formal-pr`. Add overrideable variables near the top, for
example `KANI ?= cargo kani`, `KANI_FLAGS ?=`, and, if useful,
`KANI_SMOKE_FLAGS ?= --no-default-checks` only after confirming the flag is
valid for the pinned version. `make kani` must be the fast smoke target.
`make kani-full` must run the full Kani command. `make formal-pr` must depend
on or invoke `kani`. Each target needs a `##` help comment.

Because this item may land before proof harnesses exist, choose smoke-target
behaviour deliberately during implementation:

1. Prefer a `cargo kani` invocation that succeeds cleanly with zero harnesses
   if the pinned Kani version supports that behaviour.
2. If Kani exits non-zero when no harnesses exist, make `make kani` run a
   documented tool smoke such as `cargo kani --help` for this item and record
   in `docs/developers-guide.md` that `4.2.*` will replace it with harness
   execution.
3. Do not add a trivial proof harness only to make the target pass. A proof
   that proves nothing would weaken the formal-verification story.

Stage D updates developer-facing documentation. Add a concise section to
`docs/developers-guide.md` describing how to install the pinned Kani version,
when to run `make kani`, when to run `make kani-full`, and why Kani is not part
of `make test`. Update `docs/formal-verification-methods-in-netsuke.md` only if
implementation discovers a concrete compatibility rule that changes the design.
Update `docs/users-guide.md` only if there is a real user-facing behaviour
change, which is not expected.

Stage E validates, reviews, and commits. Run formatting and documentation
checks after Markdown or Makefile/script edits. Run the normal code gates even
though no Rust code should change, because the roadmap request explicitly
requires them. Run Kani smoke validation after installing the pinned tool. Run
`coderabbit review --agent` after the tooling/docs milestone and address all
concerns before committing. Once implementation is complete and all gates pass,
mark roadmap item `4.1.1` and its subitems done in `docs/roadmap.md`, then
commit the complete atomic change.

## Concrete steps

All commands run from the repository root. The absolute path below is the
current worktree path for this planning session; replace it with the actual
repository location if continuing from a different checkout.

```bash
cd /home/leynos/.lody/repos/github---leynos---netsuke/worktrees/0ba36694-0c58-4f5d-a924-3ed261d77321
```

Before editing, confirm the branch and working tree:

```bash
git branch --show-current
git status --short
```

Expected branch:

```plaintext
feat/kanismokeexecplan
```

Create the version pin and installer:

```bash
mkdir -p tools/kani
$EDITOR tools/kani/VERSION
$EDITOR scripts/install-kani.sh
chmod +x scripts/install-kani.sh
```

The version file should contain a single version string with no leading `v` or
`kani-` prefix:

```plaintext
0.67.0
```

Install and smoke-check Kani:

```bash
scripts/install-kani.sh 2>&1 | tee /tmp/install-kani-netsuke-feat-kanismokeexecplan.out
make kani 2>&1 | tee /tmp/kani-netsuke-feat-kanismokeexecplan.out
make kani-full 2>&1 | tee /tmp/kani-full-netsuke-feat-kanismokeexecplan.out
```

Run the project gates sequentially, not in parallel:

```bash
make fmt 2>&1 | tee /tmp/fmt-netsuke-feat-kanismokeexecplan.out
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-feat-kanismokeexecplan.out
make lint 2>&1 | tee /tmp/lint-netsuke-feat-kanismokeexecplan.out
make test 2>&1 | tee /tmp/test-netsuke-feat-kanismokeexecplan.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-feat-kanismokeexecplan.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-feat-kanismokeexecplan.out
```

Run the agent review after the major tooling/docs milestone:

```bash
coderabbit review --agent
```

Inspect and commit with the file-based commit-message workflow:

```bash
git diff --stat
git diff
git status --short
git add Makefile scripts/install-kani.sh scripts/check-kani-version.sh \
  tools/kani/VERSION docs/developers-guide.md docs/roadmap.md
COMMIT_MSG_DIR="$(mktemp -d)"
$EDITOR "$COMMIT_MSG_DIR/COMMIT_MSG.md"
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

The commit message subject should be imperative, for example:

```plaintext
Add Kani smoke tooling
```

## Validation and acceptance

The implementation is accepted only when all of these behaviours are true:

- `tools/kani/VERSION` exists and contains the supported Kani version.
- `scripts/install-kani.sh` reads that version and installs
  `kani-verifier` with `cargo install --locked --version`.
- `scripts/install-kani.sh` runs `cargo kani setup` and reports clear failure
  messages if Cargo, Kani, or setup fails.
- `make help` lists `kani`, `kani-full`, and `formal-pr`.
- `make kani` succeeds as the local smoke target.
- `make kani-full` succeeds or, if no harnesses exist and Kani treats that as
  non-success, has a documented temporary no-harness behaviour that will be
  replaced by `4.2.*`.
- `make formal-pr` invokes the fast formal-verification smoke path.
- `make check-fmt`, `make lint`, and `make test` all pass.
- `make markdownlint` and `make nixie` pass if documentation changed.
- `coderabbit review --agent` has no unresolved concerns.
- `docs/developers-guide.md` documents the developer workflow.
- `docs/roadmap.md` marks `4.1.1` done only after the above checks pass.

The expected successful summaries are:

```plaintext
make check-fmt: exits 0
make lint: exits 0
make test: exits 0
make kani: exits 0 and reports the pinned Kani version
make formal-pr: exits 0
```

## Testing strategy

This item is mostly Makefile, shell, and documentation work. Unit tests with
`rstest` and behavioural tests with `rstest-bdd` are not automatically required
unless implementation introduces Rust logic, a CLI behaviour, or a user-visible
workflow. If implementation adds a Rust parser/helper for Kani configuration,
add focused `rstest` cases covering happy paths, missing version files, invalid
version strings, and command construction. If implementation changes externally
observable CLI behaviour, add a `rstest-bdd` scenario under `tests/features/`.

Do not add a dummy Kani proof harness only to satisfy the test requirement.
Formal proof work must be substantive and belongs to `4.2.*`.

## Interfaces and dependencies

The new file interfaces are:

- `tools/kani/VERSION`: plain text, one version string, no prefix. Example:
  `0.67.0`.
- `scripts/install-kani.sh`: executable Bash script, invoked from any working
  directory, resolving the repository root relative to its own path.
- `scripts/check-kani-version.sh`: executable Bash script, invoked by the
  smoke target, that verifies the installed Kani command matches
  `tools/kani/VERSION`.
- `Makefile` targets:
  - `kani`: fast Kani smoke path.
  - `kani-full`: full Kani verification path.
  - `formal-pr`: pull-request formal-verification alias for `kani`.

The external dependency is the Cargo package `kani-verifier`, installed as a
developer tool rather than added to `[dependencies]` or `[dev-dependencies]`.
Do not add `kani-verifier` to `Cargo.toml`.

No new OrthoConfig fields, Fluent message keys, Rust public APIs, or
application services are expected. From a hexagonal-architecture perspective,
this is infrastructure tooling around the repository; it must not leak into
Netsuke's domain model or manifest semantics.

## Idempotence and recovery

The installer should be safe to rerun. Re-running it may rebuild or reinstall
the same Kani package, but it must not rewrite repository files or alter the
normal Rust toolchain pin in `rust-toolchain.toml`.

If installation fails because of network access, rerun
`scripts/install-kani.sh` after network access is available. If the pinned Kani
version is incompatible with the local environment, stop and update this plan's
Decision Log with version options before changing `tools/kani/VERSION`.

If `make kani` fails because no proof harnesses exist, do not add a vacuous
proof. Either use a tool smoke command for `4.1.1` or stop and ask whether
`4.2.1` should be implemented at the same time.

Rollback is straightforward before commit:

```bash
git restore Makefile docs/developers-guide.md docs/roadmap.md
rm -rf tools/kani scripts/install-kani.sh
```

Do not use destructive rollback commands after user or other-agent edits appear
in the same files; inspect `git diff` first and preserve unrelated changes.

## Artifacts and notes

Firecrawl research confirmed these external facts:

- Kani's official installation flow is:

```bash
cargo install --locked kani-verifier --version <VERSION>
cargo kani setup
```

- Kani's Cargo-package integration is `cargo kani [OPTIONS]`.
- Official Kani documentation recommends gating proof harnesses with
  `#[cfg(kani)]` so normal `cargo build` and `cargo test` remain unaffected.
- The latest visible Kani GitHub release during planning was `kani-0.67.0`,
  published on 2026-01-16.

Wyvern reconnaissance confirmed these local facts:

- The target ExecPlan did not already exist.
- The current `Makefile` has no Kani targets.
- The repository has no existing `tools/` version-pin convention.
- No CLI or OrthoConfig change is expected for this item.

## Revision note

- 2026-05-10: Initial draft created from roadmap item `4.1.1`, the formal
  verification design document, Wyvern agent reconnaissance, and Firecrawl
  research. The plan is in draft and must be approved before implementation.
