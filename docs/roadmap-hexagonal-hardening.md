# Netsuke hexagonal hardening and checking roadmap

This proposed continuation of the [main roadmap](roadmap.md) owns phases 26 to
29. It covers internal semantic hardening, application boundaries, executable
architecture policy, and a bounded Paralegal experiment. No task is complete
merely because its RFC or ADR has been written.

## Allocation and scope

Existing task identifiers remain unchanged. Phases 16 to 19 belong to the
[composition roadmap](roadmap-composition.md). [PR #741][progressive-pr]
reserves phases 20 to 25 and RFCs 0021 to 0025 for progressive orchestration.
[PR #697][stdlib-pr] reserves RFCs 0013 to 0020. This programme therefore uses
RFCs 0026 to 0028 and phases 26 to 29. Open ADR allocations through 028 were
checked before selecting ADR-029 to ADR-031; published identifiers must not be
silently reused.

The governing proposals are [RFC 0026][hardening], [RFC 0027][checking], and
[RFC 0028][experiment], with [ADR-029][adr-model], [ADR-030][adr-checks], and
[ADR-031][adr-experiment]. Explicit dependencies, not phase numbers, define the
implementation order. Phase 29 starts with toolchain compatibility and may stop
without affecting the other phases. Its preflight does not wait for a refactor.

This programme adds no v0.1.0 release-admission requirement. Preserve the
ordinary quickstart, unannotated manifests, deterministic outputs, supported
platforms, capability boundaries, and the pinned Polonius/trait-solver compiler
contract. No broad crate split, generic backend framework, or universal effects
service is required. All proposed implementation tasks begin unchecked.

[progressive-pr]: https://github.com/leynos/netsuke/pull/741
[stdlib-pr]: https://github.com/leynos/netsuke/pull/697
[hardening]: rfcs/0026-hexagonal-domain-hardening.md
[checking]: rfcs/0027-executable-architecture-contract.md
[experiment]: rfcs/0028-paralegal-architecture-experiment.md
[adr-model]: adr-029-semantic-compiler-boundaries.md
[adr-checks]: adr-030-layered-architecture-enforcement.md
[adr-experiment]: adr-031-gate-paralegal-on-measured-evidence.md

## Existing work and ownership

- H1 to H4 are the initial audit findings owned here: resolved operations, shell
  binding, application execution, and semantic diagnostics. They are not
  fabricated GitHub issue numbers. Implementation issues should cite these task
  identifiers and their owning RFC rather than duplicate the programme.
- Issue #652 / PR #714 supplied canonical edges. The inspected baseline already
  has the arena and output index; new work preserves and tests them.
- Issue #705 owns the typed redirect boundary. Verify its current disposition;
  the experiment must not recreate completed extraction work.
- H5 is conditional integration with #592/#593 and composition phases 16 to 19.
  H6 is conditional reuse under the URL-provider work in #590.
- #591 and RFC 0007 continue to own restricted manifest testing. Initial
  manifest testing does not acquire action execution as a new prerequisite.
- RFC 0001 and amendments retain structured-command ownership; #593 and phase 11
  retain their existing integration scope. #699 owns disputed legacy placeholder
  decisions. RFC 0008 owns shared workflow-policy machinery.

## 26. Trustworthy resolved build semantics

Hypothesis: model-owned operations and interpreter-bound lowering eliminate
invalid executable states without changing ordinary manifest behaviour.

Entry: RFC 0026 and ADR-029 have accepted scope. A focused release-critical fix
may proceed separately; that does not authorize the whole transformation. Exit:
successful lowering excludes unresolved execution, shell mismatch is
unrepresentable or rejected, and compatibility/canonical-edge evidence passes.

### 26.1. Characterize the semantic boundary

- [ ] 26.1.1. Record a revision-pinned model and consumer inventory.
  - Identify authored types, lowering, graph storage, backend consumers, CLI
    orchestration, diagnostics, and existing seams at the implementation head.
  - Record landed work and remaining H1 to H4 findings without repeating #652.
  - Acceptance: every planned change maps to a current item and compatibility
    obligation; the inventory includes every production constructor and mutator.
  - Dependencies: acceptance of RFC 0026; no Paralegal dependency.
- [ ] 26.1.2. Establish recipe and declaration characterization fixtures.
  - Cover direct and delegated rules, missing references, cycles, duplicate
    declaration precedence, dependency-only nodes, and command/script variants.
  - Acceptance: observed and intended behaviour are distinguished, unsupported
    delegation has a reviewed typed-error policy, and disputed placeholder
    semantics defer to #699 rather than changing incidentally.
  - Dependencies: 26.1.1.

### 26.2. Own resolved operations in the model

- [ ] 26.2.1. Introduce restricted resolved-operation types.
  - Distinguish executable and dependency-only semantics; remove authored-rule
    references and empty executable sentinels from the successful model.
  - Acceptance: positive/API-negative tests cover construction, deserialization,
    and mutation paths; model definitions do not import `ast::Recipe`.
  - Dependencies: 26.1.2.
- [ ] 26.2.2. Migrate lowering and backend consumers to resolved operations.
  - Resolve or reject every rule before emission and preserve command, script,
    phony, always, dependency, output, and serial-ordering behaviour.
  - Acceptance: all production consumers use the new representation; malformed
    input yields consistent typed failures in debug and release configurations.
    No backend panic substitutes for compiler validation.
  - Dependencies: 26.2.1.
- [ ] 26.2.3. Verify canonical graph and emission compatibility.
  - Preserve alias identity, atomic duplicate rejection, cycle traversal, action
    identity, and deterministic repeated Ninja generation.
  - Acceptance: golden/property tests and applicable existing Kani harnesses
    pass; changed hashes have an explicit rebuild rationale, not unexplained
    snapshot acceptance. Remove the exact retired H1 exception when present.
  - Dependencies: 26.2.2; retain the existing #652 implementation.

### 26.3. Bind interpreter selection to lowering

- [ ] 26.3.1. Retain interpreter identity in the lowered executable contract.
  - Derive emission from the bound interpreter or reject a mismatched explicit
    argument at any retained compatibility entry point.
  - Acceptance: constructors and mutation APIs cannot manufacture an unchecked
    pair; positive and mismatch controls exercise every library entry point.
  - Dependencies: 26.2.2; coordinate with ADR-019, not the entire future runner.
- [ ] 26.3.2. Validate supported shells and backend escaping independently.
  - Exercise spaces, apostrophes, dollar signs, and interpreter-specific path
    cases, with separate expectations for shell quoting and Ninja escaping.
  - Acceptance: supported-shell tests and unchanged quickstart fixtures pass; no
    second quoting pass or new legacy placeholder behaviour appears. Update the
    design/developer references and retire the exact H2 exception when present.
  - Dependencies: 26.3.1 and 26.2.3.

## 27. Application and diagnostic ownership

Hypothesis: independently drivable application use cases and typed diagnostic
facts improve substitution and review without replacing existing capabilities.

Entry: accepted RFC 0026 scope and the current inventory from 26.1.1. Exit:
CLI-independent orchestration and presentation-free semantic diagnostics have
shared contracts and production integration evidence. Conditional H5/H6 work is
not required to close the initial H3/H4 delivery.

### 27.1. Separate semantic errors from presentation

- [ ] 27.1.1. Extract semantic diagnostic facts and classifications.
  - Remove independently supplied localized messages from affected compiler
    errors while retaining structured causes and available source origins.
  - Acceptance: domain consumers inspect variants without Fluent, MiniJinja, or
    parser display strings; contradictory fact/message pairs are not
    constructible.
  - Dependencies: 26.1.1; coordinate type changes with 26.2.1.
- [ ] 27.1.2. Classify recipe validation before parser error rendering.
  - Replace semantic string matching in affected frontend conversions with typed
    validation outcomes; preserve ordinary parser syntax causes.
  - Acceptance: negative recipe fixtures select semantic variants independently
    of localized text and library message wording.
  - Dependencies: 27.1.1 and 26.1.2.
- [ ] 27.1.3. Preserve human and structured diagnostic contracts.
  - Map semantic facts to localized rendering, codes, exit behaviour, and JSON.
  - Acceptance: rendering snapshots and behavioural tests preserve the reviewed
    public contract; typed tests and presentation tests remain separate. Retire
    the exact H4 exception when present and update component documentation.
  - Dependencies: 27.1.2.

### 27.2. Introduce the owned application execution contract

- [ ] 27.2.1. Map inbound CLI/configuration into application requests.
  - Separate semantic options from presentation and reuse existing precedence
    and process request machinery instead of copying `Cli` wholesale.
  - Acceptance: a library caller requests supported build/clean/inspection use
    cases without constructing `Cli`; tests verify operand and option mapping.
  - Dependencies: 26.1.1 and the approved execution scope in ADR-029.
- [ ] 27.2.2. Implement the execution port and coherent preparation ownership.
  - Add recording/failing and production Ninja implementations. Reuse child
    environments, reporting, clocks, dyndep publication, capabilities, and
    leases.
  - Acceptance: preparation and execution cannot diverge through a decorative
    token or unrelated path; application tests need no Ninja subprocess.
  - Dependencies: 27.2.1; preserve ADR-011 and ADR-012 semantics.
- [ ] 27.2.3. Qualify shared contracts and production adapter behaviour.
  - Cover success, injected errors, output routing, cleanup, and existing
    cancellation semantics; retain real-process and filesystem integration.
  - Acceptance: shared applicable contract cases pass for both implementations;
    production-only effects retain explicit tests on relevant supported
    platforms. Restricted evaluation never falls back silently to host effects.
    Retire the exact H3 exception when present and document ownership/callers.
  - Dependencies: 27.2.2 and the shell contract from 26.3.2.

### 27.3. Join existing feature work only when justified

- [ ] 27.3.1. Integrate compiler-owned provenance with composition and linting.
  - Entry condition: #592/#593 consumers or composition require shared origins.
  - Acceptance: one compiler-owned mapping handles expansion and included
    sources, ambiguous origins remain explicit, and diagnostic/suppression tests
    cannot select a different declaration by guessing its source location.
  - Dependencies: the applicable composition/linter implementation; no new
    semantic compiler. This conditional task is not an initial hardening gate.
- [ ] 27.3.2. Reuse acquisition outside MiniJinja for the URL provider.
  - Entry condition: #590 supplies a second real resource-acquisition consumer.
  - Acceptance: shared requests/results contain no MiniJinja types; redirect,
    credential, deadline, resource-limit, redaction, and cache-only contracts
    pass. Freshness and metadata import remain separate from byte acquisition.
  - Dependencies: #590's approved provider scope and current redirect policy. Do
    not add this port merely to support helper mocks or Paralegal.

## 28. Executable architecture policy and governance

Hypothesis: explicit coverage, narrow debt, and qualified checks prevent new
boundary regressions while permitting an incremental transformation.

Entry: accepted RFC 0027 and ADR-030. Exit: the structural/effect policy runs
through the real required gate, fails its seeded violations, reports its scope,
and remains within a measured approved budget. No Paralegal prerequisite
applies.

### 28.1. Define policy and migration debt

- [ ] 28.1.1. Map first-party targets to logical responsibilities.
  - Separate model from lowering, inventory source roots/configurations and
    exclusions, and authorize composition entry points and value libraries.
  - Acceptance: every discovered first-party Rust module has an unambiguous role
    or explicit tested exclusion; policy review rejects blanket composition
    privilege and unexpected coverage gaps.
  - Dependencies: 26.1.1.
- [ ] 28.1.2. Register narrowly scoped existing architectural debt.
  - Add versioned policy and exception schemas with owners, stable identities,
    reasons, limited use sites, issue/task links, and removal conditions.
  - Acceptance: the baseline reports every exception, never learns permission
    automatically, and rejects stale/broadened entries. Map H1 to H4 removals to
    their owning phase-26/27 tasks; do not exempt already remediated code.
  - Dependencies: 28.1.1.

### 28.2. Implement the bounded checker

- [ ] 28.2.1. Separate inventory, analysis facts, and pure policy evaluation.
  - Reuse appropriate Wildside logic with attribution, not its directory rules.
  - Acceptance: unit/property tests establish deterministic classification,
    segment-aware paths, and failure on missing/unreadable roots or parse
    errors.
  - Dependencies: 28.1.2.
- [ ] 28.2.2. Resolve supported import origins and expose analysis limits.
  - Cover grouped imports, aliases, relative modules, local re-exports, nested
    modules, and documented conditional-source treatment.
  - Acceptance: facade violations remain visible; unsupported protected-boundary
    forms and unclassified dependencies fail as incomplete, never permitted.
    Macro/generated-source coverage is explicitly qualified.
  - Dependencies: 28.2.1.
- [ ] 28.2.3. Emit actionable versioned architecture diagnostics.
  - Implement NSARCH001 to NSARCH005 with human and deterministic JSON output,
    origin witnesses, exception reporting, and remediation guidance.
  - Acceptance: golden tests distinguish forbidden, exempted, unresolved, and
    unclassified outcomes and remove checkout-specific path instability.
  - Dependencies: 28.2.2.

### 28.3. Qualify enforcement before requiring it

- [ ] 28.3.1. Add independently specified enforcement counterexamples.
  - Test every rule with positive/negative controls and policy mutations that
    weaken prohibitions, broaden debt, remove roots, or disable invocation.
  - Acceptance: every applicable mutation fails the correct executable test; an
    empty or incomplete scan can never satisfy the suite.
  - Dependencies: 28.2.3.
- [ ] 28.3.2. Exercise concrete effect lints and compile-fail contracts.
  - Use actual repository compiler configuration for forbidden environment/CWD
    operations and legitimate sites; test broad suppression and stale
    exceptions.
  - Acceptance: positive controls compile, intended violations fail for the
    expected reason, and nextest plus separate doctest execution remain wired.
  - Dependencies: 28.1.1; may proceed independently of 28.2 implementation.
- [ ] 28.3.3. Integrate and qualify the real architecture gate.
  - Add `make lint-architecture` and required CI invocation with failure
    propagation; reuse RFC 0008 workflow contracts and update contributor docs.
  - Acceptance: executable gate-disable probes fail; cold/warm runtime, memory,
    cache usage, and billed cost satisfy an approved measured budget within
    4-vCPU/8-GiB build and 1-vCPU/2-GiB non-build ceilings. No recurring or
    oversized runner is added. Existing debt is visible; new debt fails.
  - Dependencies: 28.3.1 and 28.3.2.

## 29. Measured semantic-analysis experiment

Hypothesis: a compatible Paralegal configuration detects useful architectural
flow regressions beyond the baseline checks at acceptable cost.

Entry: approve only RFC 0028/ADR-031's bounded P0 initially. Exit: a recorded
retain, defer, or reject outcome, not necessarily a deployed analyser. An
incompatible or inconclusive P0 prevents later trial work. Completing a task
that records a failed experiment never means its hypothesis passed. Unreached
tasks remain unchecked and explicitly deferred/not applicable in the result.

### 29.1. Test actual required-toolchain compatibility first

- [ ] 29.1.1. Preregister the compatibility trial and its ceilings.
  - Freeze actual Netsuke/analyser revisions, compiler requirements, production
    roots, target/features, lockfile, commands, expected symbols, and artefact
    provenance. Use RFC 0028's proposed budgets or an explicitly approved
    change.
  - Acceptance: record the pin mismatch as a source observation, not a measured
    result, and preserve the existing compiler/borrowing/trait-solver contract.
  - Dependencies: approval for P0 only; no phase-26/27/28 completion
    requirement.
- [ ] 29.1.2. Execute and record the bounded native compatibility preflight.
  - Establish the ordinary production build, actual analyser compiler identity,
    non-empty production graph, intended path coverage, and repeat extraction.
  - Acceptance: publish compatible/incompatible/inconclusive evidence within
    P0's budget. A toy, copied implementation, downgraded compiler, excluded
    relevant feature, or mere analyser compilation is not a compatible result.
  - Dependencies: 29.1.1. Only a compatible result authorizes 29.2/29.3 work.

### 29.2. Qualify at most two semantic policies

- [ ] 29.2.1. Pilot resolved-recipe flow through the actual production boundary.
  - Scope NSFLOW001 to executable payloads and successful resolved construction.
  - Acceptance: report expected sources, checkpoints, and consumers; legitimate
    metadata paths are not treated as recipe bypasses. Demonstrate value beyond
    types and import checks rather than adding a redundant required check.
  - Dependencies: compatible 29.1.2 and 26.2.2.
- [ ] 29.2.2. Pilot accepted redirect-target flow through actual dispatch.
  - Scope NSFLOW002 to the accepted transition for the dispatched target, with
    an explicit statement of initial-hop coverage and stateful-flow limitations.
  - Acceptance: no unnecessary production rewrite to please the analyser; a
    different approved value cannot justify dispatch of an unchecked target.
  - Dependencies: compatible 29.1.2; inspect #705's current implementation
    first.
- [ ] 29.2.3. Evaluate independent controls and incremental detection value.
  - For each retained policy, run six seeded violations, three legitimate
    controls, and two reviewer-selected hold-out violations from RFC 0028.
  - Acceptance: detect every applicable violation, accept every control without
    suppressing it, and identify at least one non-redundant caught regression.
    Report which layer catches marker removal and other coverage defects.
  - Dependencies: the applicable 29.2.1/29.2.2 policy; stop redundant policies.

### 29.3. Measure coverage and operational fitness

- [ ] 29.3.1. Reject vacuous, stale, missing, and unsupported analysis.
  - Independently inventory roots/markers/consumers and qualify cache identity,
    approximation settings, and every claimed production configuration.
  - Acceptance: missing consumers, empty roots, crashes, unsupported paths, and
    stale graphs produce incomplete results, never success. Policy weakening is
    reviewable and covered by explicit negative controls.
  - Dependencies: compatible 29.1.2 and the applicable pilot policy.
- [ ] 29.3.2. Measure a provisioned policy suite within the trial budget.
  - Record one cold and three warm runs, all failures, phase-separated timing,
    peak memory, cache/artefact bytes, runner allocation, and billed usage.
  - Acceptance: satisfy or explicitly fail RFC 0028's approved ceilings; reuse
    pinned cached tooling, with no per-PR analyser source installation or silent
    runner growth. Publish the evidence even when the experiment stops.
  - Dependencies: 29.2.3 and 29.3.1.

### 29.4. Decide rather than assume adoption

- [ ] 29.4.1. Shadow qualifying policies on ten representative changes.
  - Include real boundary edits and benign refactors, not repeated identical
    code; triage every finding and record maintenance effort and all failures.
  - Acceptance: complete declared coverage, reproducible approved cost, and no
    unexplained control failures; no small-sample universal accuracy claims.
  - Dependencies: passing outcomes from 29.2.3, 29.3.1, and 29.3.2.
- [ ] 29.4.2. Record the retain, defer, or reject decision and clean up.
  - Explain the earliest failing gate or shadow result, scope, reviewer,
    incremental value, cost, and concrete revisit trigger where appropriate.
  - Acceptance: retain evidence and remove unused instrumentation/dependencies
    for defer/reject. A blocking check requires a separate acceptance decision;
    the initial hardening is not held hostage to this outcome.
  - Dependencies: an outcome from 29.1.2 or the last reached subsequent gate;
    29.4.1 is required only for a retain/adoption recommendation.
