# RFC 0026: Harden the compiler and execution boundaries

## Preamble

- **RFC number:** 0026
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Internal architecture; no new Netsukefile syntax.
- **Decision record:** [ADR-029](../adr-029-semantic-compiler-boundaries.md).
- **Delivery:** [Architecture roadmap](../roadmap-hexagonal-hardening.md),
  phases 26 and 27.

## Summary

Strengthen Netsuke's semantic model before adding more abstraction machinery.
Separate authored recipes from resolved operations, bind lowered recipes to
their interpreter, and make application requests and semantic diagnostics
independent of command-line and presentation types. Introduce an execution port
only where substituting the external interaction benefits application callers
and tests.

[RFC 0027](0027-executable-architecture-contract.md) specifies structural and
behavioural enforcement. [RFC 0028](0028-paralegal-architecture-experiment.md)
evaluates an optional semantic analyser. Neither Paralegal adoption nor a
repository-wide layer reorganization is a prerequisite for this hardening.

## Current state and audit reconciliation

The implementation baseline is Netsuke commit
`79545e124b4a13dbe8352fd23df97619a91a0b5c`, inspected on 2026-09-19. The audit
identifiers H1 to H6 are local finding identifiers, not issue numbers.

- H1: [`ir::Action`](../../src/ir/graph.rs) still stores `ast::Recipe`.
  Successful lowering should instead yield a domain operation that cannot
  contain unresolved rule references or a dependency-only empty-command
  sentinel. Lowering may import both representations; model definitions must not
  depend on the authored representation.
- H2: shell-specific interpolation and backend shell selection need one enforced
  ownership contract. Retain the interpreter used for lowering rather than
  trusting every caller to repeat the same choice.
- H3: [`runner`](../../src/runner/mod.rs) still orchestrates Ninja operations
  using `Cli`, while the [process request types][process-requests] already
  provide useful lower-level boundaries.
- H4: semantic error construction still involves localization, including the
  duplicate-output failure in `BuildGraph::insert_edge`. Separate error facts
  from their rendered messages and eliminate semantic parser-error string
  matching as affected recipe validation moves.
- H5: compiler-owned provenance belongs with composition and semantic linting,
  not a competing compiler implementation.
- H6: a template-independent resource-fetching service becomes justified when
  the URL dependency provider needs the same acquisition infrastructure.

Canonical edge ownership has already landed: `BuildGraph` stores an edge arena
and indexes outputs by `EdgeId`. Preserve that result from issue #652 and PR
#714; do not schedule its implementation again. Similarly, inspect the current
redirect boundary and issue #705 before planning any further extraction.

[process-requests]: ../../src/runner/process/request.rs

## Goals and non-goals

The goals are trustworthy successful-lowering results, independently drivable
application use cases, and testable external boundaries. Keep feature-oriented
colocation, deterministic graph and Ninja output, useful diagnostics, and the
ordinary quickstart unchanged.

This proposal does not introduce a backend plugin ecosystem, a generic process
framework, a universal effects service, or a crate split. It does not implement
the structured-command, bundle, state, or maturity-policy programmes by stealth.
A trait is not required for a pure transformation or a better sum type.

## Proposed design

### Resolved operations and graph ownership

Introduce model-owned operation types with an explicit distinction between
execution and dependency-only graph structure. Illustrative names such as
`ResolvedOperation` and `ExecutableRecipe` describe roles, not mandated public
API spellings.

Only the compiler's validated lowering path may construct a resolved executable
operation. Public fields, deserialization, alternative constructors, and
mutation methods must not reopen unresolved states. Avoid restricting harmless
graph inspection or controlled graph-building tests merely to hide a
construction defect.

Resolve every permitted rule reference before backend emission. The first
implementation task must characterize delegated rules, duplicate declarations,
missing rules, and cycles against the existing contract. Either fully resolve
supported delegation with cycle detection or reject unsupported delegation with
a typed compiler diagnostic. Do not silently change declaration precedence or
reinterpret malformed recipes during a representation refactor.

A dependency-only operation is not an executable whose command happens to be
empty. The Ninja adapter owns its encoding. Preserve `phony`, `always`, explicit
and implicit outputs, dependency ordering, and rebuild semantics independently
of the executable/dependency-only distinction.

The canonical edge arena remains authoritative. All aliases identify the same
edge, insertion remains atomic on duplicate outputs, and graph consumers visit
logical edges through the existing accessors. Changing operation storage must
not reintroduce per-output edge cloning or NLL-era borrow workarounds.

### Bind recipes to their interpreter

The initial change retains the resolved interpreter on the shell-lowered recipe
or plan and derives backend execution from it. A compatibility wrapper that
accepts a separate interpreter must reject a mismatch before emission. Private
construction must prevent an arbitrary command/interpreter pair from
masquerading as validated lowering.

Do not add a second quoting pass. Preserve the division between shell quoting
and Ninja text escaping in [ADR-014][escaping]. Preserve the legacy placeholder
contract while characterizing it; the separate command-placeholder work in [PR
#699][placeholder-pr] owns any disputed compatibility decision.

The future structured-command representation may separate logical and executable
plans more completely. Coordinate with RFC 0001, RFC 0011, and ADR-019 without
requiring their full runner before closing the existing shell selection hole. A
shared configuration dependency does not prove that two interpreter values are
equal; types and mismatch tests enforce that invariant.

[escaping]: ../adr-014-backend-text-escaping-seam.md
[placeholder-pr]: https://github.com/leynos/netsuke/pull/699

### Application requests and an owned execution port

Map command-line interface (CLI) arguments, configuration precedence, and
presentation preferences at the inbound boundary into application requests for
build, clean, and supported inspection operations. Do not copy `Cli` into a
nominally new request structure. Presentation settings such as JSON rendering
must not decide build semantics; explicit output-routing options may travel
through a separately owned execution/reporting contract.

The application owns a narrow execution contract. Its production adapter reuses
Ninja process requests, child-environment composition, progress seams, and
clocks. A recording/failing implementation lets application tests inspect
operations, operands, outcomes, and failure propagation without spawning Ninja.
Do not expose arbitrary Ninja subcommands as the domain vocabulary merely
because the adapter supports them.

Preparation and execution must share coherent ownership of temporary manifests,
dyndep publication, and leases. A proposed `PreparedBuild` may express this
relationship, but a decorative token beside an independently chosen pathname is
insufficient. Keep the [dyndep retention contract][retention] and
capability-bearing directories. An unrestricted filesystem repository interface
would weaken rather than improve the boundary.

Specify success, failure, stream delivery, cancellation where already supported,
and cleanup behaviour once. Run the shared applicable contract against both
implementations, with separate real-process and filesystem integration tests for
production-only effects. The first manifest-testing release remains independent
of action execution; reuse its existing compiler path rather than broadening it.

[retention]: ../adr-012-bound-dyndep-sidecar-retention.md

### Semantic diagnostics and presentation

Represent compiler failures as typed facts with stable classifications and
optional source origins. A renderer maps those facts to Fluent messages,
human-readable diagnostics, and structured output. A variant must not store both
an authoritative target/rule and an independently supplied message that can
contradict it.

Preserve useful underlying syntax errors and causes at the frontend boundary.
Recognize recipe-validation failures while they remain typed, not by matching a
parser error's display string. This is a compiler-boundary change, not a new
repository-wide error framework. Characterize existing public diagnostic codes,
exit behaviour, localization, and structured output before changing storage.

### Conditional extensions with existing owners

H5 joins issues #592 and #593 and composition phases 16 to 19. Return semantic
results and a compiler-owned origin map together. Expansion and composition must
retain source identity without duplicating the semantic compiler; unknown
origins stay explicit. Lexical indexing remains permissible inside the compiler.

H6 joins issue #590 only when its second consumer exists. Separate acquisition
from dependency freshness and metadata import. A shared acquisition service must
not expose MiniJinja types. Keep redirects, credentials, network policy, limits,
redaction, and cache-only semantics authoritative on every applicable hop. Reuse
capabilities and streaming abstractions. Do not create another ambient network
route or an HTTP port solely for manifest-helper mocking.

These extensions have entry criteria in the roadmap, not unconditional initial
implementation commitments.

## Compatibility and migration

[ADR-008](../adr-008-environment-seam-taxonomy.md) continues to govern
proportionate environment seams. [ADR-006][toolchain-adr] continues to govern
the Rust toolchain and borrow-centric implementation. Preserve the
next-generation trait solver contract in `AGENTS.md`; no analysis tool justifies
weakening it.

Implementation must characterize manifest acceptance, graph export, generated
Ninja, diagnostics, and library entry points before extraction. Internal Rust
APIs may migrate deliberately, with callers updated together; the refactor must
not accidentally promise new API stability. Keep deterministic action identity,
ordering, and escaping tests. Explain any necessary hash change and its rebuild
consequences rather than accepting unexplained snapshot churn.

Keep the quickstart, existing unannotated manifests, and shallow-end onboarding
unchanged. This programme is not an additional v0.1.0 release gate. A separately
reproduced release-critical defect may justify a focused fix; the broader
transformation must not expand the release stabilization scope.

[toolchain-adr]: ../adr-006-adopt-polonius-nightly-toolchain.md

## Verification and acceptance

Completion requires the following evidence, not merely moved files:

- Compile-fail/API tests exclude unresolved executable states and unchecked
  shell mismatches, with positive controls for legitimate construction.
- Behavioural tests cover rules, delegation, cycles, missing references,
  dependency-only targets, and deterministic repeated emission.
- Property tests retain canonical-edge aliasing and atomic insertion invariants.
  Existing bounded Kani models remain meaningful and synchronized.
- Shell tests cover supported interpreters and adversarial path spellings;
  golden tests distinguish shell quoting from Ninja escaping.
- Application contracts exercise success and injected failures without Ninja;
  production integration tests cover process, stream, artefact, and lease
  behaviour. Restricted manifest evaluation never silently falls back to host
  capabilities when a test implementation is missing.
- Semantic error tests inspect variants independently of localization. Rendering
  and structured-output tests preserve the public contract.
- Remove each remediated exception when RFC 0027's ledger already exists.
  Otherwise retain the boundary's negative fixture for the later checker; the
  refactor does not wait for the enforcement rollout.

## Alternatives considered

A universal effects interface would erase useful distinctions and duplicate
existing seams. A folder-first rewrite would confuse compiler/model separation
with physical layout. A backend-generic plugin contract would introduce an
unproven extension requirement. Merely renaming `ast::Recipe` without
restricting its states would satisfy superficial import checks while retaining
the defect.

## Outstanding decisions

Characterization must settle supported rule delegation and duplicate precedence
before any behavioural change. Review the smallest shell-bound representation
and the preparation/lease ownership contract before implementation. The RFC
fixes their obligations, not an untested Rust signature.

## Recommendation

Accept the semantic and ownership boundaries in ADR-029, then deliver them in
small, independently verified changes. Add ports at real external conversations;
use types for invalid states and functions for pure compilation.
