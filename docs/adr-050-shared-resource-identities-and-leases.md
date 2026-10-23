# Architectural decision record (ADR) 050: Share resource identities and leases

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

Managed states and owned artefacts need coherent access to resources that can
outlive one build action. Separate lock implementations would allow preparation,
probe evidence, and cleanup to disagree about identity or ordering. Making one
feature engine own the other's locks would instead create an inappropriate
dependency or a cycle.

The [progressive-enhancement roadmap][progressive] already separates bounded
resource leases from state declarations and Ninja scheduling. This decision
assigns that mechanism to a proposed `netsuke-resource` component. It does not
introduce a new distributed scheduler or change the invocation-only scope of
[Ninja contention classes][contention].

## Decision drivers

- Resource identity must derive from an explicitly authorized scope.
- Multi-resource operations need one ordering and bounded acquisition contract.
- State evidence and artefact deletion must agree on the same leased resource.
- Feature engines must remain independently testable and acyclic.
- Existing filesystem and process-lifecycle contracts should be reused.

## Proposed direction

Create `netsuke-resource` with the first useful resource-lease implementation,
not as an empty placeholder. It owns capability-scoped resource identities,
canonical ordering of complete lease sets, acquisition outcomes, release
lifecycle, and the bounded waiting/cancellation contract. Reuse existing
capability and locking primitives after an ownership sweep rather than invent
another general lock framework.

A resource identifier is not an arbitrary absolute pathname carrying ambient
authority. Define how aliases, normalization, workspace identity, and relevant
platform filesystem semantics contribute to identity. A persisted identifier
must reacquire and validate the applicable authority before use.

Specify the supported coordination scope explicitly: in-process, cooperating
processes on one host, and supported filesystem/platform combinations are
separate claims. Do not infer cross-host or hostile-process exclusion from a
local advisory lock. Document unsupported filesystems and failure behaviour.

Acquire a canonically ordered set before beginning a multi-resource operation.
Do not support unbounded lock upgrading or acquire another unordered lease while
holding a set. Bound waiting, report contention separately from invalid identity,
and release partial acquisitions on timeout, cancellation, or failure. Use
ownership-bearing guards so normal exit paths cannot accidentally abandon a
lease; document operating-system behaviour on abrupt process termination.

`netsuke-state`, `netsuke-artefacts`, and relevant execution/application code may
consume this component. It does not import their concrete engines or decide
readiness, ownership, deletion, or scheduling policy. Feature-specific adapters
can turn their own identifiers into the shared contract after validation.

The application coordinates operations that span state and artefact engines.
Invalidate applicable state evidence before deleting a managed environment while
holding the same relevant lease. Keep evidence invalid after partial deletion
or failure. Do not create a state-to-artefact-to-state dependency cycle to encode
that protocol. Preserve existing generated-state and dyndep retention contracts
rather than silently replacing them with a new lease lifetime.

## Alternatives considered

Independent lock managers risk divergent identities and deadlock ordering.
Placing all locks in the state engine gives non-state consumers the wrong
implementation dependency. Treating Ninja pools as resource leases confuses
invocation scheduling with resource identity and lifetime. A generic distributed
lock service exceeds the stated requirement.

## Consequences

One shared mechanism becomes safety-relevant and needs stronger contract tests.
Feature engines remain smaller and can test their policies with controlled lease
outcomes. The component adds packaging cost only when a useful implementation
lands under [RFC 0034][ownership]. It does not automatically grant filesystem
access or enforce an operating-system sandbox.

## Verification and revisit criteria

Test alias identity, deterministic acquisition order, competing processes where
supported, reversed requested order, bounded contention, cancellation, partial
acquisition rollback, and guard release. Include negative controls for attempts
to leave the authorized scope or use an unsupported coordination context.

Integration tests cover state preparation versus cleanup, evidence invalidation
before deletion, partial deletion, and process failure. Assert that state and
artefact packages share this implementation and that it has no reverse engine
dependency. Record platform-specific guarantees and qualifications.

Revisit the contract only when a concrete resource consumer requires a broader
scope or different lifetime. A new distributed requirement needs a separate
proposal and evidence, not a silent extension of local-lock claims.

## Outstanding decisions

The first implementation fixes canonical identity and supported filesystem
semantics with tests. Existing lease and generated-state consumers must be
inventoried before deciding which mechanisms can move unchanged.

[progressive]: roadmap-progressive-enhancement.md
[contention]: rfcs/0024-named-contention-classes.md
[ownership]: rfcs/0034-forthcoming-component-ownership.md
