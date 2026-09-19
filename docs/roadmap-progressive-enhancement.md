# Netsuke progressive-enhancement roadmap

This document continues the [active roadmap](roadmap.md) and the
[composition roadmap](roadmap-composition.md) with phases 20 to 25. It tracks
proposed work, not implemented features. Existing task identifiers and
completion states remain unchanged. Dependencies, rather than phase-number
order, determine delivery sequencing.

The product hypothesis is that Netsuke can remove repeated orchestration
machinery while retaining its shallow end. The unchanged quickstart must remain
useful. Each feature must work independently with ordinary commands before a
combined Cuprum migration serves as its integration canary. External bundles,
new named execution contexts, and a universal strict mode are not prerequisites.

## Contract ownership and integration boundaries

- [RFC 0021](rfcs/0021-managed-states-and-probes.md) owns states, built-in and
  external probes, preparation evidence, and operation semantics.
- [RFC 0022](rfcs/0022-typed-task-inputs.md) owns optional root inputs and
  shares parameter validation with RFC 0003, rather than duplicating it.
- [RFC 0023](rfcs/0023-artefact-ownership-and-scoped-cleanup.md) owns declared
  artefact scope and standardized cleanup.
- [RFC 0024](rfcs/0024-named-contention-classes.md) owns public contention
  declarations lowered to Ninja pools.
- [RFC 0025](rfcs/0025-progressive-enhancement-and-maturity-policies.md) owns
  shallow-end compatibility and opt-in policy composition.

RFC 0001 and phases 12 to 14 retain ownership of command parsing, argv,
execution, process cleanup, capability-scoped paths, and persisted action
plans. Phase 11 retains trusted shell selection. Phases 16 to 19 retain include
and bundle composition. Phase 5 and OrthoConfig retain generic profile, schema,
metadata, redaction, and result machinery. The semantic linter tracked by issue
`#592` retains the reusable manifest-analysis boundary. No new task may
duplicate those implementations merely to avoid an explicit integration
dependency.

New public grammar is proposed, not shipped: RFC 0022 proposes
`--input NAME=VALUE` on manifest-compiling commands; RFC 0023 proposes
`clean --artefact NAME`. Register both with the canonical vocabulary and
metadata before delivery. Use existing `check`, `context --json`, `--dry-run`,
`--force`, and `--no-input` contracts. Do not introduce an unreviewed `explain`
command or new exit-code system.

Every implementation task includes relevant unit and behavioural tests. Use
Proptest for normalization and algebraic invariants, bounded Kani harnesses for
pure transition logic where useful, and subprocess end-to-end tests for
process, filesystem, locking, and backend boundaries. Reuse installed or cached
tooling; this roadmap does not require new source-built proof tools for
ordinary gates.

## 20. Preserve the shallow end before adding contracts

Hypothesis: optional semantic annotations can add guarantees without increasing
the prerequisites or required vocabulary of a first Netsuke build.

### 20.1. Establish compatibility and schema admission fixtures

Outcome: a release can demonstrate unchanged basic behaviour rather than merely
assert it. The fixtures expose whether later syntax has become contagious.

- [ ] 20.1.1. Ratify the progressive-enhancement contracts and version gates.
  - [ ] Review RFCs 0021 to 0025, resolve their outstanding schema decisions,
    and record accepted decisions through the normal ADR process.
  - [ ] Coordinate manifest and persisted-plan version allocation with 12.1.1,
    16.1.1, and 17.1.1 without requiring bundle implementation first.
  - [ ] Record operation-union ownership, feature-specific capability reporting,
    and rejection of unsupported syntax. See RFC 0025 sections 2 and 3.
- [ ] 20.1.2. Add unchanged-basic-workflow acceptance fixtures. Requires 20.1.1.
  - [ ] Preserve the exact quickstart manifest, scalar shell recipes, ordinary
    variables, and the list-of-mappings action/target structure.
  - [ ] Test actual child commands, default diagnostics, files, and absence of
    state records, probe execution, installation, and new network activity.
  - [ ] Add a mixed annotated/unannotated aggregate fixture. Success: enabling
    one feature does not require declarations on unrelated nodes.

### 20.2. Make the common boundaries independently consumable

Outcome: features can reuse schema and reporting facilities without forcing all
other advanced features into their minimum implementation.

- [ ] 20.2.1. Publish a feature and metadata integration contract. Requires
  20.1.1; coordinate with phase 5's context/profile work.
  - [ ] Specify optional feature reporting and unknown-version diagnostics in
    the existing JSON envelope and command metadata source of truth.
  - [ ] Allocate shared typed-parameter validation to one owner with 17.1.3;
    keep bundle resolution and root input sourcing separate.
  - [ ] Define the common capability-scoped resource identity and lease seam
    used by states and owned producers/cleanup, including its reuse limits.
- [ ] 20.2.2. Add progressive documentation acceptance checks. Requires 20.1.2
  and 20.2.1.
  - [ ] Keep the first-page example unchanged and stage one-feature-only
    examples outside the mandatory onboarding path.
  - [ ] Check examples against accepted schemas when those schemas land;
    distinguish proposed fragments from runnable examples until then.
  - [ ] Record the before/after onboarding vocabulary and execution footprint.
    Reject required advanced declarations without an explicit compatibility
    decision, not an unnoticed documentation rewrite.

- [ ] 20.2.3. Implement optional capability-scoped resource leases. Requires
  20.2.1.
  - [ ] Supply one bounded, canonically ordered lease implementation for
    cooperating producers, state consumers, and scoped cleanup.
  - [ ] Test cross-process contention, cancellation, path aliases, missing
    resource roots, and unsupported filesystems; create no runtime lease records
    for features that an invocation does not use.
  - [ ] Keep the seam independent of state declarations and Ninja scheduling.
    Feature tasks integrate it rather than creating competing lock systems.

## 21. Typed inputs with one parameter contract

Hypothesis: optional typed public inputs make task configuration predictable
without annotating internal variables or creating a second profile system.

### 21.1. Validate values before executing a manifest

Outcome: root inputs and bundle parameters accept and reject the same values,
with errors local to the responsible declaration or source.

- [ ] 21.1.1. Implement the normalized parameter contract. Requires 20.2.1.
  - [ ] Reuse or extract RFC 0003 section 6's types, constraints, default
    validation, exposure metadata, and bounded diagnostics.
  - [ ] Test Boolean/integer distinction, overflow, collection bounds, duplicate
    keys, choices, empty values, and path capability non-authority.
  - [ ] Share a conformance corpus with bundle work without requiring bundle
    loading. See RFC 0022 sections 4 and 7.
- [ ] 21.1.2. Add optional root input declarations and immutable resolution.
  Requires 21.1.1 and 20.1.2.
  - [ ] Parse `inputs`, preserve ordinary `vars`, detect namespace collisions,
    and validate before Jinja expansion.
  - [ ] Support one-value promotion without implicit aliases or executable
    defaults. Retain declaration and reference spans.
  - [ ] Property-test normalization stability and run the unchanged-basic
    fixture. See RFC 0022 sections 3, 4, and 8.

### 21.2. Bind explicit callers and profiles to the same interface

Outcome: a user can identify which source supplied a value without learning a
new configuration stack. Precedence cases decide whether the contract is clear.

- [ ] 21.2.1. Wire type-directed CLI and configuration input sources. Requires
  21.1.2 and 20.2.1.
  - [ ] Register `--input NAME=VALUE` in canonical metadata; reject unknown and
    duplicate names, shell splitting, and malformed typed JSON collections.
  - [ ] Integrate existing configuration/profile provenance and ratify overlay
    order with phase 5; apply operator constraints after source selection.
  - [ ] Test every source precedence pair and default redaction in errors,
    verbose output, and JSON. See RFC 0022 sections 5 and 7.
- [ ] 21.2.2. Preserve resolved inputs through graph and plan generation.
  Requires 21.2.1 and 12.3.1 for structured-plan integration.
  - [ ] Add used values to existing fingerprints and preserve argv splicing
    through RFC 0001; do not create a second hashing or cache system.
  - [ ] Freeze values for persisted-plan replay and test a later changed ambient
    profile cannot replace them.
  - [ ] Test include/bundle boundaries when 16.3.3 and 17.4.3 are available;
    keep this composition matrix separate from local input delivery.

### 21.3. Demonstrate useful annotation without broad migration

Outcome: the worker-count canary validates one public knob and leaves all
unrelated task configuration unchanged.

- [ ] 21.3.1. Publish and execute the typed-worker canary. Requires 21.2.2.
  - [ ] Observe real Cargo/nextest or fixture-equivalent argv for distinct build
    and test workers, including argument-list conflict rejection.
  - [ ] Add a compact guide example and effective-value provenance output.
  - [ ] Run one-input-only and mixed untyped/typed fixtures. Success: no
    context, state, or bundle is required to use a validated worker count.

## 22. Named contention without a second scheduler

Hypothesis: one optional contention annotation can control shared build
pressure without confusing dependency order or subprocess worker limits.

### 22.1. Resolve one class per executable edge

Outcome: the public model is small enough to lower directly to existing backend
pool machinery, with no hidden overlapping-resource scheduler.

- [ ] 22.1.1. Add bounded contention declarations and reference validation.
  Requires 20.1.1 and 20.2.1.
  - [ ] Implement positive integer capacities, one scalar action/target class,
    operator ceilings, reserved names, and source-local errors.
  - [ ] Reject aggregate annotations, multiple classes, and unsupported
    rule-default forms. See RFC 0024 sections 3 and 4.
  - [ ] Add a legacy-command fixture; typed input support is optional and
    integrates only after 21.1.2.
- [ ] 22.1.2. Lower resolved classes into Ninja pool definitions. Requires
  22.1.1.
  - [ ] Reuse `Action.pool`, emit stable bounded names, and preserve dependency
    and dyndep semantics. Reject console/custom-pool conflicts.
  - [ ] Include complete action sequences and persisted scheduling metadata;
    integrate with 12.3.1 rather than reimplementing the codec.
  - [ ] Property-test declaration-order independence and unannotated-output
    compatibility. See RFC 0024 section 5.

### 22.2. Verify concurrency and communicate its limits

Outcome: measured edge concurrency, rather than syntax snapshots alone, proves
the guarantee and exposes its invocation-only boundary.

- [ ] 22.2.1. Add controlled-process concurrency tests. Requires 22.1.2.
  - [ ] Assert capacities one and two under larger global `-j`, and show an
    unrelated action can run while a class is occupied.
  - [ ] Exercise multi-command edges, failure, cancellation, and console
    rejection without relying solely on sleeps.
  - [ ] Demonstrate that separate Ninja invocations do not share the limit. See
    RFC 0024 sections 6 and 8.
- [ ] 22.2.2. Publish class inspection and the native-build canary. Requires
  22.2.1 and 20.2.2.
  - [ ] Show requested/effective capacity, source, and invocation-only scope.
  - [ ] Document that slots are not CPU/memory quotas or internal worker counts.
  - [ ] Add explicit shared-class bundle binding only after 17.4.3; local class
    support must not depend on external acquisition.

## 23. Verified preparation without mandatory environments

Hypothesis: built-in readiness checks plus optional external functional probes
remove preparation glue while preserving explicit preconditions and tool choice.

### 23.1. Define readiness separately from execution and repair

Outcome: a pure transition model establishes when preparation is permitted
before process execution or durable records complicate the implementation.

- [ ] 23.1.1. Implement state definitions and the readiness algebra. Requires
  20.1.1 and 20.2.1.
  - [ ] Model kinds, declared identity inputs, optional preparation, all four
    outcomes, and the three operations in RFC 0021 sections 3 to 5.
  - [ ] Property-test that unknown never authorizes repair, degraded requires
    explicit acceptance, and ensure performs at most one preparation attempt.
  - [ ] Reject unsupported incremental-target state checks instead of letting
    Ninja skip readiness verification. See RFC 0021 section 8.
- [ ] 23.1.2. Implement default built-in probes through existing seams. Requires
  23.1.1 and 12.2.3 where interpreter execution is needed.
  - [ ] Deliver directory, file, and precisely specified Python-environment
    probes without requiring third-party plugins.
  - [ ] Distinguish object presence, interpreter validity, package verification,
    and preparation identity; never overstate a probe's evidence.
  - [ ] Test missing, corrupted, unreadable, replaced, and incompatible objects
    through injected environment/filesystem adapters.

### 23.2. Execute leased state operations with durable evidence

Outcome: cooperating invocations can validate and prepare mutable state without
stale success records or a second scheduling loop.

- [ ] 23.2.1. Integrate state units into the structured runner and plan codec.
  Requires 23.1.2, 12.3.1, and the phase 12 process-lifecycle contract.
  - [ ] Execute require, ensure, and prepare with fail-fast consumer ordering;
    never probe during check, graph generation, help, or dry-run.
  - [ ] Preserve resolved argv, environment, cwd, provenance, and typed result
    mapping. Do not add state-private command execution.
  - [ ] Reject unsupported replay versions. See RFC 0021 sections 5 and 7.
- [ ] 23.2.2. Implement bounded integrity leases and atomic state records.
  Requires 23.2.1 and 20.2.3.
  - [ ] Hold canonically ordered resource leases through each action's state
    consumers; reject conflicting identities for one mutable path.
  - [ ] Publish success only after post-verification and invalidate observations
    across mutation. Bound retention, acquisition, and interrupted recovery.
  - [ ] Test separate processes, damaged records, replacement races, and
    interruption before publication. See RFC 0021 sections 4 and 8.

### 23.3. Admit external functional checks without implicit repair

Outcome: a repository-owned executable can supply a bounded readiness check
without requiring a provider implementation or a Nagios service.

- [ ] 23.3.1. Implement the optional Nagios-style protocol adapter. Requires
  23.2.1.
  - [ ] Map exits 0 to 3 exactly as RFC 0021 section 6 specifies and preserve
    distinct spawn, signal, timeout, protocol, and output-limit reasons.
  - [ ] Combine built-in and external evidence without allowing stdout to forge
    identity or override a failing result.
  - [ ] Test all codes, partial output, empty summaries, performance suffixes,
    bad encoding, and contradictory success text.
- [ ] 23.3.2. Enforce probe budgets, authority, and process cleanup. Requires
  23.3.1 and the shared bounded process-tree termination facility.
  - [ ] Apply operator-capped deadlines and combined output limits during
    concurrent collection; kill and reap hung descendants on cancellation.
  - [ ] Reject unauthorized external code and unsupported command fields;
    preserve allowed-shell policy and redact diagnostic data.
  - [ ] Use hostile fixture probes to prove bounded behaviour. An observational
    convention must not be described as an operating-system sandbox.

### 23.4. Validate state value in a real migration

Outcome: the canary removes redundant setup while retaining intentionally
strict preconditions, and default builds pay no state-management cost.

- [ ] 23.4.1. Add the Cuprum preparation and extension-guard canaries. Requires
  23.2.2 and 23.3.2.
  - [ ] Verify an environment, remove a required component, and demonstrate
    fresh detection rather than a stale timestamp or probe cache hit.
  - [ ] Preserve the exact restricted extension suite and prove require-only
    execution never invokes Maturin to repair a missing extension.
  - [ ] Test ordinary commands beside stateful actions and assert zero probes
    for the unchanged hello-world selection.
- [ ] 23.4.2. Publish state diagnostics, guarantees, and the probe author guide.
  Requires 23.4.1 and 20.2.2.
  - [ ] Document smallest built-in use, custom probes, result meanings, trust,
    lease scope, failure recovery, and the lack of automatic teardown.
  - [ ] Add metadata and localization through existing surfaces.
  - [ ] Integrate optional typed inputs, named contexts, and bundle runtime
    resources only through their accepted owners, not mandatory prerequisites.

## 24. Owned artefacts and bounded cleanup

Hypothesis: explicit disposable ownership can make cleanup predictable without
requiring a new output-store layout or annotating every existing target.

### 24.1. Establish ownership before permitting deletion

Outcome: one path has one explicit owner, and report registration does not
silently create caching or overlapping output producers.

- [ ] 24.1.1. Implement exact-path artefact definitions and producer references.
  Requires 20.1.1 and 20.2.1.
  - [ ] Add file/directory kind, descriptive roles, optional parent creation,
    and `produces` metadata. See RFC 0023 sections 3 and 4.
  - [ ] Reject overlapping/case-equivalent ownership and duplicate producers;
    reconcile explicit declarations with existing target outputs.
  - [ ] Test missing outputs, retained partial outputs, and always-run reports.
- [ ] 24.1.2. Implement the bounded read-only cleanup planner. Requires 24.1.1.
  - [ ] Resolve explicit names to workspace capabilities, protect source and
    runtime paths, and enumerate complete bounded scope without deletion.
  - [ ] Make directory-subtree ownership and pre-existing contents visible;
    reject invalid, over-budget, or incomplete previews.
  - [ ] Property-test path normalization and scope uniqueness. See RFC 0023
    sections 5 and 6.

### 24.2. Delete only validated scope through one implementation

Outcome: recipe cleanup and explicit CLI cleanup share confirmation,
confinement, partial-failure reporting, and fresh execution-time validation.

- [ ] 24.2.1. Implement capability-scoped deletion and ownership leases.
  Requires 24.1.2 and 20.2.3.
  - [ ] Use supported handle-relative deletion, no-follow traversal, identity
    rechecks, and canonical lease ordering shared with owned producers.
  - [ ] Test symlinks, reparse points, hard links, replacement, permissions,
    interruption, and external sentinels on supported hosts.
  - [ ] Report partial progress honestly and refuse unsupported guarantees. Do
    not substitute lexical containment or shell deletion.
- [ ] 24.2.2. Wire the operation and explicit `clean --artefact` selection.
  Requires 24.2.1, 12.3.1, and existing mutation metadata integration.
  - [ ] Add `clean_owned`, canonical CLI metadata, `--dry-run`, `--force`, and
    `--no-input` handling through the shared planner/deleter.
  - [ ] Preserve ordinary `clean` behaviour when explicit artefact selection is
    absent; never implicitly include caches or environments.
  - [ ] Test zero writes in preview, refusal without consent, fresh replay
    validation, and bounds that `--force` cannot bypass.

### 24.3. Integrate lifecycle invalidation and staged migration

Outcome: cleanup cannot leave trusted readiness evidence for an environment it
has removed, and ordinary workflows retain their independent cleanup choices.

- [ ] 24.3.1. Integrate state invalidation and selected-closure conflicts.
  Requires 24.2.2 and 23.2.2.
  - [ ] Invalidate state records before deleting their owned environments under
    the same resource lease, including partial-failure paths.
  - [ ] Reject simultaneous cleanup and declared production/consumption of the
    same resource; a pool must not be treated as semantic ordering.
  - [ ] Keep dyndep and command-private temporary cleanup under their existing
    owners. See RFC 0023 section 7.
- [ ] 24.3.2. Publish exact-path cleanup examples and a Cuprum canary. Requires
  24.2.2 and 20.2.2; state examples also require 24.3.1.
  - [ ] Migrate disjoint output roots without demanding an artefact for every
    target, and retain unsupported wildcard cleanup as an explicit recipe.
  - [ ] Document ownership assertions, recursive scope, retained caches, preview
    limitations, and external-command boundaries.
  - [ ] Demonstrate that the one-directory feature works without state syntax.

## 25. Opt-in maturity policies without compulsory strictness

Hypothesis: explicit scoped enforcement can support stricter projects without
changing default onboarding, pretending to prove safety, or duplicating linting.

### 25.1. Reuse semantic analysis for narrowly defined coverage rules

Outcome: one typed inventory supports both the semantic linter and policy
findings, with no command-text guessing or competing parser.

- [ ] 25.1.1. Define the reusable policy inventory boundary with issue #592.
  Requires 20.2.1 and the semantic linter's accepted inventory contract.
  - [ ] Retain resolved recipe units, references, ownership, and provenance;
    distinguish policy coverage from schema/runtime correctness.
  - [ ] Implement pure rule/subject selection without executing probes or user
    actions. See RFC 0025 sections 3 to 6.
  - [ ] Add fixtures proving an unrelated selected target does not inherit
    another action's coverage requirements.
- [ ] 25.1.2. Implement the initial rules as independently gated checks.
  Requires 25.1.1 and each rule's relevant delivered feature.
  - [ ] Add structured commands after 12.3.3, typed subjects after 21.2.1,
    contention after 22.1.2, states after 23.2.1, and cleanup after 24.2.2.
  - [ ] Reject unknown/unsupported rule IDs rather than claim incomplete
    enforcement. No rule waits for an unrelated feature.
  - [ ] Test exact coverage semantics and remedies from RFC 0025 section 5; do
    not certify hermeticity from declaration presence.

### 25.2. Compose enforcement without weakening trusted constraints

Outcome: profile and import layering cannot make a project-owned warning
replace an operator error or hide a rule through scope rewriting.

- [ ] 25.2.1. Add opt-in policy declarations and trust-aware severity merging.
  Requires 25.1.1 and the existing configuration provenance boundary.
  - [ ] Keep default rules empty; combine overlapping scopes monotonically and
    retain the strongest applicable authority.
  - [ ] Integrate profiles through phase 5's owner, with no automatic promotion,
    blanket bypass, or new configuration loader.
  - [ ] Property-test overlap, order independence, scope normalization, and
    inability to weaken operator policy. See RFC 0025 section 7.
- [ ] 25.2.2. Wire preflight and existing human/JSON reporting. Requires 25.1.2
  and 25.2.1.
  - [ ] Evaluate applicable errors before execution, keep warnings nonblocking,
    and preserve ordinary structural errors regardless of coverage severity.
  - [ ] Use `check`, `context --json`, shared exit classes, localization, and
    redaction; introduce no separate `explain` command.
  - [ ] Test selected-closure versus whole-manifest checks and persisted plans
    under applicable trusted policy. See RFC 0025 section 8.

### 25.3. Prove progressive enhancement end to end

Outcome: small projects and the Monster Makefile can both use Netsuke without
sharing the same configuration burden.

- [ ] 25.3.1. Run the progressive and strict-policy acceptance matrix. Requires
  25.2.2, 21.3.1, 22.2.2, 23.4.2, and 24.3.2.
  - [ ] Run unchanged hello-world, one-feature-only cases, mixed aggregates,
    report-only adoption, and explicit scoped errors.
  - [ ] Add imported-policy coverage after 17.4.3 without blocking local
    adoption; reject unknown private selectors and weakening imports.
  - [ ] Assert zero default maturity warnings, no unrelated probes, unchanged
    legacy command semantics, and no mandatory external tools or bundles.
- [ ] 25.3.2. Publish release guidance from measured canary results. Requires
  25.3.1 and 20.2.2.
  - [ ] Describe gradual adoption and reversal, exact guarantees, operator
    authority, and the limits of declaration-only checks.
  - [ ] Compare repository-maintained machinery across manifests, helpers, and
    real bundles; do not hide complexity behind nonexistent packages.
  - [ ] Keep all first-page prerequisites and required declarations unchanged.
    Reconsider any feature that cannot demonstrate local benefit before broad
    promotion; no universal strict preset is required for release.
