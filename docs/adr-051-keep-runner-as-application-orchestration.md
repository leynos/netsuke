# Architectural decision record (ADR) 051: Keep runner orchestration in the application

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

[RFC 0026][semantics] identifies a current coupling between runner orchestration
and `Cli`, while existing process request types already provide useful lower-level
boundaries. Moving the current runner subtree into `netsuke-runner` would
preserve that coupling behind a new package name rather than resolve it.

The forthcoming structured-command runtime has a different responsibility:
execute validated operations with explicit stream, environment, working-directory,
limit, and cleanup contracts. It must not inherit application configuration and
presentation merely because both responsibilities involve child processes.

## Decision drivers

- Application requests must not be disguised copies of CLI structures.
- Structured execution, Ninja orchestration, and test-case isolation have
  different policies but should reuse appropriate process mechanisms.
- Independent components must not depend on the application facade.
- Extraction should follow actual ownership and consumer evidence.

## Proposed direction

Keep runner orchestration inside `netsuke-build` during the initial extraction.
Proceed with RFC 0026's CLI-to-application request mapping and narrow execution
boundary; this decision does not defer that semantic work. Separate presentation
preferences from build semantics and pass explicit reporting/stream contracts
where an operation needs them.

The application owns preparation and dispatch of build, clean, and inspection
use cases. Preserve coherent ownership of temporary manifests, dyndep publication,
leases, cancellation, and cleanup. A request structure containing an embedded
`Cli`, or a decorative prepared token beside an unrelated pathname, does not
satisfy the boundary.

Create `netsuke-exec` for the validated structured-command runtime when that
feature lands under [RFC 0034][ownership]. Do not rename the whole current runner
into that crate. Keep Ninja process adaptation distinct from Ninja text generation,
and reuse existing process/stream primitives where their contracts genuinely
match. General reuse does not require a universal process or effects service.

The first manifest-testing release still reuses the compiler without acquiring
action execution as a prerequisite. Its bounded case-isolation mechanism may
reuse existing process facilities; it must not wait for the complete structured
runtime or depend on application dispatch to obtain them.

Consider a separate application-use-case or runner crate later only when an
actual non-CLI consumer, a demonstrated dependency boundary, or measured
compilation cost justifies it. By then its interface must accept application
requests, not parsing/configuration objects, and its effect contracts must have
shared tests.

## Alternatives considered

Immediate mechanical runner extraction creates an application-shaped library
and may force lower components to depend upwards. Combining all process-related
features in one executor would mix policy with mechanism. Implementing separate
process managers for every consumer duplicates cancellation and cleanup risks.

## Consequences

Some orchestration changes continue to rebuild the application. That cost is
acceptable until the proposed revisit criteria establish a useful boundary.
Semantic improvements and recording/failing execution tests can proceed without
a physical package move.

The product composition layer may shrink as true services move out, but it
continues to own cross-feature coordination rather than delegate it through
cyclic component dependencies.

## Verification and revisit criteria

Use recording and failing execution implementations to exercise application
requests without Ninja, plus real-process integration tests for production-only
behaviour. Preserve operand handling, environment composition, progress,
resources, temporary artefacts, and failure propagation.

A future extraction must show no dependency on `Cli` or `netsuke-build`, retain
shared success/failure/cleanup contracts, preserve existing user behaviour, and
include focused build/package evidence from [RFC 0030][benchmark]. A second
folder or another caller inside the same composition layer is not enough by
itself to reopen this decision.

## Outstanding decisions

RFC 0026 retains ownership of the exact application request and preparation
lifetime design. Structured-command RFCs retain runtime semantics. This record
only fixes initial physical placement and the criteria for changing it.

[semantics]: rfcs/0026-hexagonal-domain-hardening.md
[ownership]: rfcs/0034-forthcoming-component-ownership.md
[benchmark]: rfcs/0030-build-performance-benchmark-extension.md
