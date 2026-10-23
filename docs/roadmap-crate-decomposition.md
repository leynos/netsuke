# Netsuke crate decomposition roadmap

Status: Proposed.

Created: 2026-10-03.

This continuation of the [main roadmap](roadmap.md) uses Goals, Ideas, Steps,
and Tasks (GIST). Goals state outcomes; numbered phases express ideas and their
hypotheses; subsections group coherent engineering steps; dotted task identifiers
name bounded deliverables. Dependencies, not phase numbers, determine execution
order. All implementation tasks begin unchecked.

## Allocation and authority

This programme allocates phases 30 to 34, RFCs 0030 to 0035, and ADRs 048 to
054. The [hexagonal-hardening roadmap][hardening] owns phases 26 to 29.
Existing task identifiers remain unchanged. [PR #808][ratification-pr] reserves
progressive-enhancement ADRs through 047, and [PR #621][lint-pr] also uses
ADR-042 on its open branch. These allocations count as occupied even before
merge. This programme does not resolve or reuse those other branches' numbers.
Recheck allocations when rebasing; do not renumber published records silently.

The governing proposals are [benchmark evidence][benchmark],
[initial decomposition][decomposition], [localisation][localisation],
[test support][test-support], [feature ownership][ownership], and
[workspace publication][release]. Existing feature RFCs retain behavioural
and syntax authority. [RFC 0026][semantics] retains semantic hardening ownership;
[RFC 0027][architecture] retains architecture-checking ownership. Crate placement
must not become a competing specification for either programme.

This roadmap is a checked-in planning document, not a separate GitHub Gist or a
replacement issue tracker. Implementation issues and execution plans cite its
task identifiers and their owning feature/RFC. They do not copy whole programmes
into disconnected checklists.

## Goals

**G1: Cheaper focused engineering.** Reduce unrelated compilation in selected
edit/check/test loops under unchanged resource budgets. Establish practical
improvement thresholds and acceptable cold-build, memory, and disk trade-offs
before measuring candidates. No percentage improvement is assumed here.

**G2: Honest component boundaries.** Focused component tests must not acquire
`netsuke-build` through normal, build, or test-support dependencies. Shared
semantic contracts must have one owner and checked construction paths.

**G3: One compatible product.** Preserve manifest behaviour, deterministic build
identity/output, supported platforms, localisation, application imports where
promised, and registry/binary distribution. One package train must not become
several independently operated products.

**G4: Evidence before continuation.** Every extraction records a continue,
integrate, defer, or reverse decision. A safety/ownership benefit may justify a
measured cost, but it cannot be reported as an unmeasured build-speed gain.

## Invariants and non-goals

Preserve the pinned Polonius/trait-solver toolchain, existing build flags and
coverage/release exclusions, capability boundaries, proof obligations, shallow-end
quickstart, and existing release-stage feature gates. Do not source-build
expensive external tools or enlarge runners merely to obtain attractive timings.

No task is complete because its RFC or ADR exists. This programme adds no broad
v0.1.0 admission gate. Multi-crate packaging must remain viable as components
land; live publication and publisher configuration require a separately
authorized release operation. No empty feature crates are required in advance.

## 30. Establish measurement and test-graph evidence

Hypothesis: representative edit scenarios and isolated test graphs expose the
work that a useful extraction can remove, without confusing cache retrieval,
linking, host load, or test execution with compilation.

Entry: accepted scope for RFCs 0030 and 0033. Exit: reproducible baseline data,
reviewed thresholds, tested isolation checks, and a classified fixture inventory.
This phase does not depend on an upstream OrthoConfig release.

### 30.1. Characterize the implementation and decision criteria

- [ ] 30.1.1. Record a revision-pinned dependency and consumer inventory.
  - Include normal/build/dev edges, features, targets, build-script consumers,
    source ownership, current proof inputs, and in-flight feature work.
  - Acceptance: every candidate maps to actual code and consumers; landed work
    and still-proposed work are distinguished, including linter PR #621.
  - Dependencies: proposal review; no production extraction.
- [ ] 30.1.2. Register benchmark hypotheses, budgets, and acceptance thresholds.
  - Select decisive workflows, controlled stimuli, resource ceilings, repeated
    sampling, retained evidence, and acceptable secondary regressions.
  - Acceptance: criteria exist before candidate results, with a rule for noisy
    or incomplete measurements and a separately justified ownership benefit.
  - Dependencies: 30.1.1.

### 30.2. Extend and validate the benchmark

- [ ] 30.2.1. Add scenario selection and versioned evidence output.
  - Preserve the existing linker/frontend mode and safeguards. Add no-op,
    implementation-edit, test-only, model/API, and build-metadata scenarios;
    separate compiler-only and CI-cache experiments.
  - Acceptance: explicit packages/targets/features, checked source patches,
    isolated priming, seed/order, timings, build messages, resource collector
    semantics, and unavailable metrics all appear in bounded output.
  - Dependencies: 30.1.2; RFC 0030.
- [ ] 30.2.2. Test harness safety and measurement controls.
  - Cover wrong source bytes, wrapper/flag interference, lock contention,
    unsafe cleanup paths, failed builds, interruption, and unsupported metrics.
  - Acceptance: fake-tool unit tests and bounded integration controls prove
    failures cannot become plausible success measurements or damage live work.
  - Dependencies: 30.2.1.
- [ ] 30.2.3. Capture and review the baseline.
  - Run the approved scenarios on the intended resource envelope; preserve raw
    samples and distinguish compilation, retrieval, linking, and test runtime.
  - Acceptance: reproducible evidence and reviewed validity/limitations; no
    production split proceeds on an unexplained single-sample speed claim.
  - Dependencies: 30.2.2.

### 30.3. Separate fixtures and prove dependency isolation

- [ ] 30.3.1. Classify and separate general versus application support.
  - Keep generic utilities free of Netsuke feature/application implementations;
    retain application-aware fixtures at the root during migration.
  - Acceptance: helper ownership and reuse policy are explicit, with existing
    root tests preserved and no newly exported unchecked test constructors.
  - Dependencies: 30.1.1; RFC 0033.
- [ ] 30.3.2. Implement positive and negative test-graph contracts.
  - Inspect resolved metadata and focused compiler messages, including aliases,
    build edges, relevant features/targets, and warmed artefacts.
  - Acceptance: a small fixture workspace rejects injected application edges
    while accepting legitimate lower dependencies; no per-test cold real build.
  - Dependencies: 30.3.1; coordinate RFC 0027 rather than duplicate its checker.

Evidence decision: retain the harness only after controls pass. If the baseline
cannot separate effects from noise, fix the experiment before moving production
code. Fixture isolation can remain a useful result even if later splits defer.

## 31. Prepare localisation and semantic boundaries

Hypothesis: a genuine lightweight localisation runtime and explicit semantic
ownership remove accidental upward dependencies without requiring a whole-system
rewrite or a new configuration framework.

Entry: accepted RFCs 0031/0032 and the inventory from 30.1.1. Exit: explicit
localisation ownership, a tracked upstream migration, and the semantic preconditions
for each proposed physical move. Independent work may proceed in parallel.

### 31.1. Extract the localisation boundary in coordinated stages

- [ ] 31.1.1. Characterize catalogues, identifiers, and public construction.
  - Cover locale selection versus pure resolution, fallback ordering, builder
    defaults, formatting failures, key audits, and package resource paths.
  - Acceptance: old/new responsibilities and compatibility fixtures include
    both normal and build-script consumers.
  - Dependencies: 30.1.1; RFC 0032.
- [ ] 31.1.2. Establish `netsuke-l10n` and explicit application selection.
  - Move catalogue inventory and construction with tests. Keep environment and
    configuration discovery in the application.
  - Acceptance: no reverse application edge, preserved catalogue/audit behaviour,
    and standalone package checks. An interim `ortho_config` edge has an owner
    and retirement condition and does not count as the lightweight result.
  - Dependencies: 31.1.1, 30.2.3, and 30.3.2.
- [ ] 31.1.3. Adopt the released upstream runtime and documentation contract.
  - Track [OrthoConfig #566][l10n-issue] as the upstream implementation owner.
    Coordinate [Netsuke #779][ir-issue] with [OrthoConfig PR #536][ids-pr], which
    supersedes #420; these concern documentation IR, not the build graph.
  - Acceptance: direct `ortho_l10n` consumers preserve nominal types and defaults
    without configuration/Clap dependencies; both affected dependency sections,
    release-help metadata, identifiers, and packaged catalogues agree.
  - Dependencies: 31.1.2 and an actually released compatible upstream version.
    Do not invent its version or duplicate #566 as downstream implementation.

### 31.2. Reconcile semantic prerequisites without duplicating their roadmap

- [ ] 31.2.1. Map each physical extraction to its semantic obligations.
  - Reuse hardening tasks 26.1, 26.2, 26.3, and 27.1 as applicable for resolved
    operations, interpreter binding, graph identity, and diagnostic facts.
  - Acceptance: landed evidence is reused; unresolved obligations have explicit
    owners/dependencies. No crate move gains public unchecked constructors.
  - Dependencies: 30.1.1; RFC 0026 and ADR-054.
- [ ] 31.2.2. Record bounded transitional dependencies and their removal gates.
  - A retained localisation edge has no new callers and no reverse core import;
    semantic-diagnostic mapping stays at the application during that transition.
  - Acceptance: the architecture inventory distinguishes interim from target
    purity and includes negative controls and retirement conditions.
  - Dependencies: 31.2.1 and the affected extraction design.

Evidence decision: integration may proceed with a reviewed transitional edge,
but the corresponding lightweight/pure goal remains incomplete until removed.
Upstream delay must not hold the benchmark or generic fixture work hostage.

## 32. Extract the initial components in measured increments

Hypothesis: a cohesive core plus stdlib/backend boundaries makes relevant work
independent, while a compiler library enables linting/testing without recreating
the application dependency. Each boundary is an experiment, not a quota.

Entry: baseline and isolation controls from phase 30, accepted extraction scope,
and relevant phase 31 obligations. Exit: only justified components remain, with
compatibility, package closure, and a recorded decision for each.

### 32.1. Establish a cohesive shared core

- [ ] 32.1.1. Extract shared types and algorithms with checked construction.
  - Keep authored/resolved representations semantically separate; move only
    genuinely shared graph, action-identity, and recipe/shell responsibilities.
  - Acceptance: facade compatibility where promised, canonical-edge and API
    negative/positive tests, applicable proofs, and no application dependency.
  - Dependencies: 30.2.3, 30.3.2, 31.2.1, and any approved 31.2.2 exception;
    31.1.2 if existing rendering dependencies remain. ADR-048 defers microcrates.
- [ ] 32.1.2. Evaluate core evidence and publish the ownership map.
  - Compare high-fan-out and sibling-edit cases, resource costs, and package
    inclusion; preserve proof-input ownership and feature forwarding.
  - Acceptance: reviewed integrate/defer/reverse decision against 30.1.2.
  - Dependencies: 32.1.1 and publication-closure validation from 33.1.2.

### 32.2. Extract the template standard library

- [ ] 32.2.1. Move stdlib implementation and focused verification together.
  - Preserve explicit capability/configuration injection, budgets, executable
    resolution, network restrictions, and feature-specific fixture ownership.
  - Acceptance: focused `netsuke-stdlib` tests do not compile `netsuke-build`;
    existing behavioural/property/proof coverage and package resources survive.
  - Dependencies: 32.1.2, 30.3.2, and 31.1.2 where localisation is required.
- [ ] 32.2.2. Decide whether the stdlib boundary earns its cost.
  - Compare real stdlib edits and test-only edits, plus cold application build,
    linking, memory, disk, and relevant CI-cache behaviour.
  - Acceptance: evidence supports integration or an explicit alternative
    semantic justification; otherwise defer or reverse the extraction.
  - Dependencies: 32.2.1; criteria from 30.1.2.

### 32.3. Evaluate Ninja and compiler boundaries independently

- [ ] 32.3.1. Extract and measure the Ninja backend when justified.
  - Preserve escaping, command rendering, dyndep encoding, backend validation,
    and determinism; leave process orchestration in the application.
  - Acceptance: focused backend tests and package consumers exclude unrelated
    frontend/application implementation, with reviewed benchmark results.
  - Dependencies: 32.2.2 and resolved-model obligations from 31.2.1.
- [ ] 32.3.2. Provide the compiler library before independent feature consumers.
  - Move manifest processing, composition/provenance, validation, and lowering
    behind the existing semantic contracts. Reconcile in-flight linter work.
  - Acceptance: linting/testing can consume compiler products without the
    application or a second compiler; restricted evaluation never gains ambient
    capabilities. Retain origin, diagnostic, and compatibility evidence.
  - Dependencies: the needed core/stdlib boundaries and 30.3.2; no dependency on
    32.3.1 or complete structured-action execution. Trigger when those consumers
    need independence, not merely when a phase number is reached.

### 32.4. Review deferred subdivisions explicitly

- [ ] 32.4.1. Evaluate core, command-schema, and runner revisit criteria.
  - Apply ADR-048, ADR-049, and ADR-051 to observed consumers and timings.
  - Acceptance: a recorded retain/defer/extract decision for each; a command
    schema extraction preserves generated metadata and accounts for separate
    host/target compilation, while runner extraction excludes CLI coupling.
  - Dependencies: relevant measurements from phase 30 and the actual consumers.
    Deferral is a valid completed decision, not an obligation to create crates.

Evidence decision: each extraction PR carries its own compatibility and cost
record. A neutral or negative timing result must not disappear into a cumulative
workspace comparison. Retain independently useful semantic fixes when reversing
an unhelpful physical split.

## 33. Extend the existing release train safely

Hypothesis: Lading mechanics, tested shared adapters, and repository-owned
admission make a multi-crate product manageable without weakening release trust.

Entry: accepted RFC 0035. Planning can start alongside phase 30. Exit: verified
package closure, tested failure/recovery paths, and an operator-ready publication
procedure. Actual uploads require a separate release authorization.

### 33.1. Establish prior art and the package plan

- [ ] 33.1.1. Inventory current Lading and shared-actions capabilities.
  - Read Netsuke's existing configuration and the pinned rstest-bdd release
    implementation; distinguish existing behaviour from proposed extensions.
  - Acceptance: explicit owners for missing plan validation, verified receipts,
    resumption/batching, evidence checks, and statistics; no invented CLI flags.
  - Dependencies: RFC 0035 review; no completed production extraction required.
- [ ] 33.1.2. Validate publication membership and dependency order.
  - Configure lockstep Netsuke packages, production/build closure, private
    exclusions, version/path requirements, staged resources, and patch policy.
  - Acceptance: a fixture train and each real extraction's package consumers
    pass; dry-run unpublished-sibling warnings remain visible and qualified.
  - Dependencies: 33.1.1 and the relevant candidate package manifests. This
    validation runs during an extraction before the full publication rollout.

### 33.2. Deliver reusable mechanics and CI adapters

- [ ] 33.2.1. Close required Lading capability gaps upstream.
  - Implement only missing validated-plan, receipt, bounded resume, and
    candidate-content comparison contracts identified in 33.1.1.
  - Acceptance: controlled-registry or mocked tests cover partial publication,
    existing-version mismatch, visibility delay, and safe retry; no disposable
    production uploads serve as failure tests.
  - Dependencies: 33.1.1; actual gaps determine scope.
- [ ] 33.2.2. Extract and adopt tested shared-actions integration.
  - Reuse existing Rust setup, staging, and upload actions. Add generic tool
    setup, plan/evidence verification, bounded publication, and reporting only
    where needed; keep product policy and credentials with the caller.
  - Acceptance: action contracts pass, both intended consumers adopt reviewed
    immutable revisions where applicable, and no large duplicated shell logic
    or workflow-expression injection appears.
  - Dependencies: required 33.2.1 capabilities; coordinate rstest-bdd and
    `leynos/shared-actions` rather than copying implementations into Netsuke.

### 33.3. Prepare trusted publication and exact-candidate admission

- [ ] 33.3.1. Integrate candidate admission and late authentication.
  - Extend RFC 0005's exact-commit evidence path. Keep the publishing workflow
    identity and protected environment in Netsuke; use the official auth action.
  - Acceptance: wrong/missing evidence, unsupported events, broad permissions,
    secret-bearing reports, and expired credentials fail safely. Recheck current
    registry OIDC/bootstrap restrictions and actual token lifetime.
  - Dependencies: 33.1.2 and 33.2.2.
- [ ] 33.3.2. Rehearse the train and document operator-controlled bootstrap.
  - Record per-crate ownership and first-publication/publisher setup, ordered
    uploads, bounded reauthentication, receipts, and partial-release recovery.
  - Acceptance: rehearsal and package/installation/archive/help checks pass;
    the runbook distinguishes preparation from authorized live publication and
    never treats yanking as transactional rollback.
  - Dependencies: 33.3.1 and the actual candidate set. No package is published
    merely to mark this planning task complete.

Evidence decision: adopt only verified capabilities. A short dry run with waived
sibling checks is not evidence of a fully built registry graph; later package
verification and consumer checks must close the documented gap.

## 34. Apply boundaries as forthcoming features arrive

Hypothesis: assigning each real feature an implementation owner, with small
shared contracts, prevents the application from becoming the next monolith.

Entry: the owning feature's accepted scope and first useful implementation or
an existing implementation ready for extraction. These tasks add boundary
acceptance to existing feature delivery, not a duplicate feature roadmap.
Exit: each delivered feature meets RFC 0034's ownership and focused-test rules.

### 34.1. Share pure contracts and compiler products

- [ ] 34.1.1. Establish the shared parameter-validation component.
  - Deliver `netsuke-params` with root-input/bundle-parameter work, keeping
    sourcing, precedence, acquisition, and template evaluation outside it.
  - Acceptance: both consumers use identical normalization/default/value
    contracts; Boolean/integer, overflow, bounds, choices, and path-as-data
    tests pass without compiling the CLI or executor.
  - Dependencies: relevant RFC 0003/RFC 0022 delivery and phase 30 controls.
- [ ] 34.1.2. Integrate linting and manifest testing through compiler contracts.
  - Create or extract `netsuke-lint` and `netsuke-testing` with their actual
    feature work, preserving release gates, compiler-owned provenance, and
    distinct private Rust fixture ownership.
  - Acceptance: no application dependency, duplicate compiler, guessed origin,
    or implicit authority escalation; manifest testing does not wait for action
    execution. Existing feature acceptance remains mandatory.
  - Dependencies: 32.3.2 and the respective feature delivery; reconcile PR #621.

### 34.2. Separate execution plans and policy authority

- [ ] 34.2.1. Deliver structured execution with checked plan boundaries.
  - Put runtime process/stream lifecycle in `netsuke-exec`; keep inspectable
    plan contracts outside it and reuse existing process mechanisms.
  - Acceptance: ADR-054 constructor/replay controls and existing command RFC
    pipeline, timeout, cancellation, capture, and cleanup tests pass. No
    speculative `netsuke-plan` crate is necessary for a small shared module.
  - Dependencies: structured-command RFC delivery and relevant model contracts.
- [ ] 34.2.2. Integrate policy decisions without making findings authoritative.
  - Own trust-aware composition in `netsuke-policy`; preserve one semantic
    inventory and enforcement in the actual effect adapters.
  - Acceptance: suppression/severity cannot permit a denied effect, replay and
    redirects retain checks, and policy has no reverse concrete-engine edge.
  - Dependencies: applicable policy feature delivery; ADR-053 and RFC 0034.

### 34.3. Coordinate resources, states, and artefacts

- [ ] 34.3.1. Introduce `netsuke-resource` with its first lease consumer.
  - Implement scoped identity, ordered complete lease sets, bounded contention,
    and ownership-bearing release under ADR-050; reuse existing primitives.
  - Acceptance: alias/order/concurrency/cancellation/failure tests state their
    supported platform/filesystem scope. Ninja pools remain a different contract.
  - Dependencies: actual resource consumer and its accepted feature scope.
- [ ] 34.3.2. Integrate state and artefact engines without a dependency cycle.
  - Own readiness/evidence in `netsuke-state` and cleanup plans/deletion in
    `netsuke-artefacts`; let the application coordinate shared leases and
    evidence invalidation before deletion.
  - Acceptance: partial failure leaves evidence invalid, deletion stays within
    authorized ownership, and neither engine imports the other. Reuse structured
    execution and the single resource mechanism where applicable.
  - Dependencies: 34.3.1, relevant RFC 0021/RFC 0023 delivery, and required
    execution contracts; do not duplicate those feature implementations.

Evidence decision: create packages with useful code and tests, not placeholders.
Apply phase 30 measurements and phase 33 package checks to each delivered
component. Revisit optional analysis/plan/helper crates only for real consumers.

## Handoff and decision evidence

Each task's execution plan records the implementation revision, owning RFC and
ADR, explicit dependencies, bounded change, expected observations, relevant
quality gates, and rollback/refusal conditions. Record positive and negative
controls, not only successful examples. Keep an extraction's evidence beside its
review or linked artefacts with an explicit retention policy.

At each integration decision, state which goal improved, which hypothesis the
measurements supported or contradicted, and the costs that remain. An unchecked
blocked task names its missing dependency; a completed deferral records its
reopening condition. Completing this documentation set checks no implementation
box and changes no release authority.

[hardening]: roadmap-hexagonal-hardening.md
[benchmark]: rfcs/0030-build-performance-benchmark-extension.md
[decomposition]: rfcs/0031-staged-crate-decomposition.md
[localisation]: rfcs/0032-localisation-crate-boundary.md
[test-support]: rfcs/0033-component-test-support.md
[ownership]: rfcs/0034-forthcoming-component-ownership.md
[release]: rfcs/0035-lading-backed-workspace-publication.md
[semantics]: rfcs/0026-hexagonal-domain-hardening.md
[architecture]: rfcs/0027-executable-architecture-contract.md
[ratification-pr]: https://github.com/leynos/netsuke/pull/808
[lint-pr]: https://github.com/leynos/netsuke/pull/621
[l10n-issue]: https://github.com/leynos/ortho-config/issues/566
[ir-issue]: https://github.com/leynos/netsuke/issues/779
[ids-pr]: https://github.com/leynos/ortho-config/pull/536
