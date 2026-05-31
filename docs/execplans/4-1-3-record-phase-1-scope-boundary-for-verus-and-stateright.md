# 4.1.3. Record the phase-1 Verus and Stateright boundary

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `4.1.3` records the phase-1 boundary for formal-verification tools
that Netsuke is deliberately not using as mandatory gates yet. After this work
is implemented, contributors will have one clear policy: Verus is optional and
reserved for a small proof kernel, while Stateright remains deferred until
Netsuke has a stateful concurrent subsystem worth model checking.

This matters because Phase 4 is adding stronger checks around Netsuke's
semantic core without turning every proof tool into a permanent workflow cost.
The observable result is documentation, not runtime behaviour. A reviewer
should be able to read `docs/formal-verification-methods-in-netsuke.md`,
`docs/developers-guide.md`, and `docs/roadmap.md` and see that Kani remains the
supported phase-1 formal-verification gate, Verus has a narrow optional entry
point, and Stateright has explicit re-entry criteria.

This plan is approval-gated. It must be reviewed and explicitly approved before
implementation begins.

## Constraints

- Do not implement this plan until the user explicitly approves it.
- Keep this work documentation-first. Do not add Verus proofs, Verus install
  scripts, Stateright models, Stateright dependencies, new Rust modules, new
  command-line interface (CLI) flags, new configuration fields, new locale
  keys, or new Continuous Integration (CI) jobs unless the user explicitly
  expands the scope.
- Preserve the existing Kani phase-1 contract from roadmap items `4.1.1` and
  `4.1.2`: `make kani`, `make kani-full`, `make formal-pr`,
  `tools/kani/VERSION`, delegated `rust-prover-tools` installation through
  `make install-kani`, and the `kani-smoke` CI job remain the supported
  formal-verification tooling and gating surface.
- Do not add Verus or Stateright to `make test`, `make lint`,
  `make check-fmt`, `make all`, `make kani`, `make kani-full`,
  `make formal-pr`, or CI in this task.
- Keep Stateright out of scope until Netsuke has an accepted design for a
  stateful concurrent subsystem such as a daemon, watch service,
  remote-execution coordinator, actor protocol, or internal scheduler with
  long-lived mutable control-plane state.
- Keep Verus out of production Cargo. If future work introduces a proof, it
  must remain outside the ordinary Cargo build and focus first on a small
  proof model for `src/ir/cycle.rs` cycle canonicalization.
- Use `docs/formal-verification-methods-in-netsuke.md` as the design document
  for this decision. Create an Architecture Decision Record (ADR) only if
  implementation uncovers a new substantive decision beyond the existing
  roadmap policy.
- Update `docs/users-guide.md` only if implementation changes user-facing
  behaviour. The expected implementation does not change the tool's user
  interface, so the users' guide should probably remain unchanged.
- If implementation unexpectedly adds a CLI, configuration, or help surface,
  use the existing OrthoConfig and localization pipeline: derive configuration
  from `src/cli/mod.rs` and `src/cli/config.rs`, merge layers in
  `src/cli/config_merge.rs`, and add localized help in
  `locales/en-US/messages.ftl`, `locales/es-ES/messages.ftl`, and
  `src/localization/keys.rs`.
- Use hexagonal architecture as a boundary check, not as a transplant. Keep
  verification policy in documentation and, if future code appears, keep pure
  domain or proof policy separate from adapters, process orchestration and
  tool-installation concerns.
- Documentation prose must follow `docs/documentation-style-guide.md` and use
  en-GB-oxendict spelling and grammar.
- Run long validation commands sequentially and capture output with `tee`
  under `/tmp`. Do not run format checks, lints, or tests in parallel.
- Use `coderabbit review --agent` after each major milestone and clear all
  concerns before moving to the next milestone.
- Commit only after gates pass. Use the file-based commit-message workflow
  with `git commit -F`, not `git commit -m`.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 4 changed files beyond this
  ExecPlan, stop and escalate. The expected implementation files are
  `docs/formal-verification-methods-in-netsuke.md`,
  `docs/developers-guide.md`, and `docs/roadmap.md`.
- Code: if any Rust production file, test file, Cargo manifest, Makefile, CI
  workflow, script, locale file, or OrthoConfig surface must change, stop and
  get explicit approval before continuing.
- Interface: if a public CLI flag, configuration key, environment variable,
  structured output field, public Rust API, Make target, or CI job is needed,
  stop and present options.
- Dependencies: if a new dependency on `verus`, Stateright, Proptest, Kani,
  or any other crate is needed for this item, stop and escalate.
- Proof surface: if recording the boundary appears to require a real Verus
  proof or Stateright model, stop and split that into a later roadmap item.
- Documentation decision: if the boundary no longer fits naturally in
  `docs/formal-verification-methods-in-netsuke.md` and requires an ADR, stop
  and explain the new decision before creating the ADR.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts, stop and escalate with log locations.
- Review: if `coderabbit review --agent` raises unresolved correctness,
  scope, or documentation concerns, do not proceed until they are addressed or
  explicitly waived.
- Ambiguity: if "stateful concurrent subsystem" could reasonably include a
  non-concurrent batch workflow, ask for direction before weakening the
  Stateright deferral rule.

## Risks

- Risk: The existing formal-verification design document already states most
  of the intended policy, so the implementation could become redundant prose.
  Severity: medium. Likelihood: high. Mitigation: make the change about
  phase-1 boundary traceability, re-entry criteria, and developer workflow
  expectations rather than repeating the executive summary.

- Risk: The planned Verus layout in
  `docs/formal-verification-methods-in-netsuke.md` mentions future
  `scripts/install-verus.sh`, `scripts/run-verus.sh`, and `tools/verus/`
  paths, which could be misread as immediate implementation work. Severity:
  medium. Likelihood: medium. Mitigation: clarify that phase 1 records the
  optional proof-kernel boundary and does not create those paths.

- Risk: "Proof kernel" can sound like a new production component. Severity:
  medium. Likelihood: medium. Mitigation: define it plainly as a small
  proof-specific model of one mathematical contract, kept outside production
  Cargo and outside normal user workflows.

- Risk: Stateright is valuable for concurrent protocols, but Netsuke currently
  emits a static Ninja graph and delegates execution to Ninja. Severity:
  medium. Likelihood: low. Mitigation: document concrete re-entry triggers so
  the deferral is revisitable rather than permanent.

- Risk: Over-applying hexagonal architecture could lead to unnecessary module
  reshaping during a documentation task. Severity: low. Likelihood: medium.
  Mitigation: use the dependency rule as a review lens only; do not restructure
  the codebase for this item.

- Risk: Full validation is expensive for a documentation-only change, but the
  repository instructions require the normal gates before committing. Severity:
  low. Likelihood: high. Mitigation: run `make check-fmt`, `make lint`, and
  `make test` sequentially with logs, and add `make markdownlint` and
  `make nixie` because Markdown changes are expected.

## Progress

- [x] 2026-05-22T19:01:43Z: Loaded the `leta`, `rust-router`,
      `hexagonal-architecture`, `execplans`, `firecrawl-mcp`,
      `commit-message`, `pr-creation`, and `en-gb-oxendict-style` skills
      relevant to this planning task.
- [x] 2026-05-22T19:01:43Z: Created a `leta` workspace for the repository.
- [x] 2026-05-22T19:01:43Z: Renamed the branch to
      `4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright`.
- [x] 2026-05-22T19:01:43Z: Reviewed `AGENTS.md`, `Makefile`,
      `docs/roadmap.md`, `docs/formal-verification-methods-in-netsuke.md`,
      `docs/developers-guide.md`, `docs/users-guide.md`,
      `docs/ortho-config-users-guide.md`, and the previous `4.1.1` and
      `4.1.2` ExecPlans.
- [x] 2026-05-22T19:01:43Z: Used two Wyvern agents for documentation
      reconnaissance and configuration/test-stack reconnaissance.
- [x] 2026-05-22T19:01:43Z: Used Firecrawl to check current Verus and
      Stateright prior-art sources for the boundary rationale.
- [x] 2026-05-22T19:01:43Z: Drafted this approval-gated ExecPlan.
- [x] 2026-05-22T19:01:43Z: Ran `make check-fmt`, `make lint`,
      `make test`, `make markdownlint`, and `make nixie`; all passed for the
      draft ExecPlan.
- [x] 2026-05-22T19:01:43Z: Attempted `coderabbit review --agent` three
      times; all attempts returned a recoverable account rate-limit response
      before producing review findings.
- [x] 2026-05-24T15:43:39Z: The user explicitly approved proceeding with
      implementation from this ExecPlan.
- [x] 2026-05-24T15:43:39Z: Confirmed the branch is
      `4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright` and the
      working tree is clean before implementation.
- [x] Stage A: review the draft plan with the user and obtain explicit
      approval before implementing roadmap item `4.1.3`.
- [x] 2026-05-24T15:43:39Z: Updated
      `docs/formal-verification-methods-in-netsuke.md` to define the Verus
      proof-kernel boundary, exclude Verus from phase-1 build and CI surfaces,
      and record concrete Stateright re-entry criteria.
- [x] 2026-05-24T15:43:39Z: Updated `docs/developers-guide.md` with the
      phase-1 support boundary: Kani supported and gated, Verus optional and
      not installed by default, and Stateright deferred.
- [x] Stage B: update the formal-verification design document with the
      phase-1 boundary, Verus proof-kernel definition, and Stateright re-entry
      criteria.
- [x] Stage C: update the developers' guide with the phase-1 support matrix:
      Kani supported and gated, Verus optional and not installed by default,
      Stateright deferred.
- [x] 2026-05-24T15:43:39Z: Ran `make check-fmt`, `make lint`,
      `make test`, `make markdownlint`, and `make nixie`; all passed before
      requesting CodeRabbit review.
- [x] 2026-05-24T15:43:39Z: Ran `coderabbit review --agent`; it completed with
      zero findings.
- [x] 2026-05-24T15:43:39Z: Marked `docs/roadmap.md` item `4.1.3` and its two
      subitems done after documentation and review passed.
- [x] 2026-05-24T15:59:07Z: Reran the full validation set after the roadmap
      and ExecPlan updates; all checks passed.
- [x] 2026-05-24T15:59:07Z: Reran `coderabbit review --agent` on the final
      documentation diff; it completed with zero findings.
- [x] Stage D: run CodeRabbit review, quality gates, and documentation gates.
- [x] Stage E: mark roadmap item `4.1.3` and its subitems done only after the
      approved implementation lands and passes validation.
- [x] 2026-05-24T16:00:33Z: Committed and pushed the implementation branch for
      review.
- [x] Stage F: commit, push, and update the pull request for the implemented
      feature.

## Surprises & Discoveries

- `docs/formal-verification-methods-in-netsuke.md` already states that Verus
  should remain optional and limited to small proof kernels after Kani and
  Proptest stabilize, and that Stateright should remain out of scope until
  Netsuke gains long-lived mutable state, actor-style coordination, or a more
  complex scheduler than the current Ninja hand-off.
- `docs/roadmap.md` task `4.4.3` depends on `4.1.3`, so this boundary is a
  prerequisite for any later evaluation of a minimal Verus proof kernel.
- The current `Makefile` already exposes `make kani`, `make kani-full`, and
  `make formal-pr`. There are no Verus or Stateright Make targets today, which
  supports keeping this task documentation-only.
- `Cargo.toml` includes `rstest`, `rstest-bdd`, and `rstest-bdd-macros`, but
  no direct `proptest`, `verus`, or Stateright dependencies at the time this
  plan was drafted.
- OrthoConfig is already the active layered configuration mechanism for
  Netsuke. It should be mentioned as a guardrail only if this task unexpectedly
  grows a configuration or localized help surface.
- Firecrawl found the official Verus guide, which describes Verus as using
  specification, proof, and executable modes, reinforcing that a future Verus
  boundary should be proof-specific rather than a general runtime feature.
- Firecrawl found official Stateright documentation and repository material
  describing it as a Rust model checker and actor library for systems such as
  actors executing a distributed protocol, reinforcing the deferral until
  Netsuke has comparable stateful concurrency.
- `coderabbit review --agent` was available locally but blocked by an
  account-level rate limit during all three draft validation attempts. No
  CodeRabbit findings were produced to clear.
- Implementation approval arrived on 2026-05-24, so the approval gate is
  satisfied and the roadmap item can now be implemented within the documented
  tolerances.
- The documentation changes stayed inside the expected scope. No user-facing
  CLI behaviour changed, so `docs/users-guide.md` does not need an update. No
  new architecture decision beyond the existing roadmap policy was introduced,
  so no Architecture Decision Record (ADR) is needed.
- `make fmt` runs `mdformat-all`, which attempted to rewrite many unrelated
  Markdown files and then failed on pre-existing line-length findings outside
  this task. The formatter churn was restored because the working tree was
  clean before the command and those changes exceeded this plan's scope. The
  required checking gates, including `make markdownlint`, passed afterwards.

## Decision Log

- Decision: Keep this ExecPlan pre-implementation and approval-gated.
  Rationale: The user explicitly stated that the plan must be approved before
  implementation. Date/Author: 2026-05-22 / planning agent.

- Decision: Treat the expected implementation as documentation-only.
  Rationale: Roadmap item `4.1.3` asks to record the scope boundary, not to add
  toolchain support, executable proof code, runtime behaviour, or CLI surface.
  Date/Author: 2026-05-22 / planning agent.

- Decision: Record the boundary primarily in
  `docs/formal-verification-methods-in-netsuke.md`, with developer workflow
  implications in `docs/developers-guide.md`.
  Rationale: The formal-verification document is the design source for Phase 4,
  while the developers' guide already explains the Kani toolchain and local
  gate policy. Date/Author: 2026-05-22 / planning agent.

- Decision: Do not plan new unit, behavioural, property, Kani, Verus, or
  Stateright tests for the expected implementation.
  Rationale: A documentation-only boundary record has no runtime behaviour,
  externally observable workflow, invariant over program state, or proof
  obligation. If implementation changes CLI/configuration/code, this decision
  is invalid and the plan must be revised before proceeding.
  Date/Author: 2026-05-22 / planning agent.

- Decision: Proceed with a documentation-only implementation after approval.
  Rationale: The user approved implementation of this ExecPlan, and the
  pre-change review found no need to change code, configuration, CI, locale
  files, OrthoConfig surfaces, or tool dependencies.
  Date/Author: 2026-05-24 / implementing agent.

- Decision: Leave `docs/users-guide.md` unchanged.
  Rationale: This implementation records contributor-facing tool boundaries and
  does not change command-line behaviour, configuration, output, persistence,
  or any other user-visible workflow.
  Date/Author: 2026-05-24 / implementing agent.

- Decision: Do not create an ADR for this item.
  Rationale: The implementation records the phase-1 boundary already requested
  by the roadmap and design document. It does not introduce a new substantive
  architecture decision.
  Date/Author: 2026-05-24 / implementing agent.

## Implementation plan

Start from a clean understanding of the branch. Run:

```sh
git status --short --branch
git branch --show-current
```

The branch should be
`4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright`. If it is not,
rename it before continuing. Confirm that the working tree contains only
expected plan changes or user-authored changes that must be preserved.

Next, update `docs/formal-verification-methods-in-netsuke.md`. Keep the
current conclusion intact, but make the phase-1 boundary explicit. The
implementation should define "proof kernel" in plain language as a small,
proof-specific model for one mathematical contract, not a production subsystem.
It should say that any first Verus work is limited to a cycle canonicalization
model related to `src/ir/cycle.rs`, kept outside ordinary Cargo and CI until a
later approved task makes it stable. It should also clarify that phase 1 does
not create Verus installer scripts, Verus Make targets, or Verus CI.

In the same file, strengthen the Stateright deferral section. The text should
say that Stateright is deferred because Netsuke currently compiles manifests
into a static Ninja build graph and delegates execution to Ninja. The re-entry
criteria should be concrete: reconsider Stateright only after an accepted
design introduces a stateful concurrent subsystem such as a daemon, watch
service, remote-execution coordinator, actor protocol, or internal scheduler
with long-lived mutable control-plane state.

Then update `docs/developers-guide.md` under `## Formal-verification tooling`.
Add a short phase-1 support matrix in prose or compact bullets. It should state
that Kani is supported and gated today, Verus is optional and not installed or
run by default, and Stateright is deferred with no dependency, model, or CI
surface. Do not duplicate long design rationale here; link back to
`docs/formal-verification-methods-in-netsuke.md`.

After the documentation changes, review whether `docs/users-guide.md` needs
changes. It should remain unchanged unless the implementation unexpectedly
changes a user-facing workflow. Review whether an ADR is needed. It should not
be needed if the implementation only records the roadmap boundary already
present in the formal-verification design.

Run CodeRabbit after this documentation milestone:

```sh
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
```

Address every actionable finding before proceeding. If CodeRabbit reports no
findings, record that in this plan's `Progress` section.

Run formatting and validation sequentially:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
make lint 2>&1 | tee /tmp/lint-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
make test 2>&1 | tee /tmp/test-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.out
```

The expected result is that all five commands exit successfully. If
`make check-fmt` fails only because Markdown needs wrapping, run `make fmt`,
inspect the diff carefully, preserve unrelated user changes, and rerun the
checks. Do not run these gates in parallel.

Once the implementation has passed CodeRabbit and the gates, update
`docs/roadmap.md` to mark `4.1.3` and its two subitems done. Do this only at
the end of the approved implementation, not while this ExecPlan is still only a
draft. Update this plan's `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` sections with the validation
evidence.

Commit with a file-based message:

```sh
git status --short
git add docs/formal-verification-methods-in-netsuke.md docs/developers-guide.md docs/roadmap.md docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Record Verus and Stateright phase-1 boundary

Document the phase-1 formal-verification scope so Kani remains the supported
gate, Verus stays optional and proof-kernel-only, and Stateright remains
deferred until Netsuke has stateful concurrent behaviour to model.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Push the branch and create or update a draft pull request. The pull request
title must include `(4.1.3)`, and the summary must mention this ExecPlan:
`docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`.
Include a `## References` section at the end of the pull request body with the
Lody session link from:

```sh
echo ${LODY_SESSION_ID}
```

## Validation strategy

This task's expected implementation changes documentation only. Unit tests with
`rstest`, behavioural tests with `rstest-bdd`, property tests with Proptest,
Kani harnesses, Verus proofs, and Stateright models are not applicable unless
the approved implementation changes code, configuration, CLI behaviour,
localized help, runtime workflows, or semantic contracts beyond documentation.

The validation evidence for the expected implementation is:

- `coderabbit review --agent` reports no unresolved findings.
- `make check-fmt` passes.
- `make lint` passes.
- `make test` passes.
- `make markdownlint` passes.
- `make nixie` passes.
- `git diff` shows no changes outside the expected documentation files.

## Signposts for implementers

- Use the `leta` skill and workspace for code navigation if implementation
  unexpectedly touches Rust code.
- Use the `rust-router` skill before routing any Rust change to a narrower
  Rust skill.
- Use the `hexagonal-architecture` skill only as a boundary check: domain and
  policy remain separated from adapters and tool orchestration.
- Use the `execplans` skill whenever this plan is revised during
  implementation.
- Use the `commit-message` skill for file-based commit messages.
- Use the `pr-creation` skill for the draft pull request title and body.
- Use the `en-gb-oxendict-style` skill for documentation prose.
- Use `docs/roadmap.md` for roadmap acceptance criteria.
- Use `docs/formal-verification-methods-in-netsuke.md` as the design source
  for Kani, Proptest, Verus and Stateright scope.
- Use `docs/developers-guide.md` for contributor-facing workflow details.
- Use `docs/ortho-config-users-guide.md` and existing `src/cli/*`
  OrthoConfig code only if the scope unexpectedly grows a configuration or
  localized CLI help surface.
- Use `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` only if code
  changes make tests applicable.

## Outcomes & Retrospective

The approved documentation-only implementation is complete. The formal
verification design now states that Verus is optional in phase 1,
proof-kernel-only, outside ordinary Cargo, and outside the normal Make and CI
gates. It defines the proof kernel as a small proof-specific model and keeps
the only current Verus entry point to a possible future
`src/ir/cycle.rs` cycle canonicalization proof.

The Stateright section now records the phase-1 deferral explicitly. Netsuke
currently compiles manifests into a static Ninja file and delegates execution
to Ninja, so Stateright remains out of the dependency, model, Make, and CI
surface until an accepted design introduces a stateful concurrent subsystem.

The developers' guide now gives contributors the same support boundary in
workflow terms: Kani is supported and gated today, Verus is optional and not
installed by default, and Stateright is deferred. The users' guide remains
unchanged because no user-facing behaviour changed, and no ADR was created
because no new architecture decision was introduced beyond recording the
roadmap boundary.

Validation passed before the final review with `make check-fmt`, `make lint`,
`make test`, `make markdownlint`, and `make nixie`. CodeRabbit was run twice
during implementation and returned zero findings both times.
