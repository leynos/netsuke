# RFC 0034: Assign forthcoming features explicit component ownership

## Preamble

- **RFC number:** 0034
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Future crate boundaries, contracts, and dependency direction.
- **Delivery:** [Crate decomposition roadmap][roadmap], phases 32 and 34.
- **Precedence:** Feature RFCs retain syntax, semantics, and capability policy.

## Summary

Give substantial forthcoming feature implementations their own components,
introduced with useful implementation and tests rather than empty crates. Keep
shared semantic contracts below their consumers, and keep the application as
the composition owner. Reuse the production compiler, parameter validator,
process facilities, policy decisions, and resource leases.

This RFC allocates implementation ownership. It does not implement the existing
feature roadmaps again or make optional Netsukefile syntax synonymous with
optional Cargo compilation.

## Current state and authority

The [composition roadmap][composition] and
[progressive-enhancement roadmap][progressive] already assign feature delivery.
[RFC 0026][semantics] owns resolved operations, provenance, typed diagnostics,
and application execution boundaries. [Issue #592][lint-issue] owns semantic
linting; [RFC 0007][testing] owns the manifest-testing language.

[Linter PR #621][lint-pr] is already in flight and proposes `netsuke check`
behind an off-by-default `lint` Cargo feature for release staging. Reconcile its
actual implementation at the extraction head; preserve its explicit release
gate and accepted behaviour rather than implement another linter. This RFC
neither assumes that branch has merged nor removes its feature gate by stealth.

These documents and their reviewed amendments remain authoritative for behaviour.
This proposal answers where that behaviour should live as the workspace evolves
under [RFC 0031][decomposition]. Working package names require availability
checks before publication.

## Goals and non-goals

Enable focused compilation/testing, prevent duplicate semantic engines, retain
one owner for each invariant, and make safety-relevant boundaries reviewable.

Do not introduce a generic plugin platform, universal effects abstraction,
independent service deployment, or a new policy language. Do not move every
shared struct into a dependency-heavy core crate.

## Proposed ownership

| Component | Owns | Must not own |
| --- | --- | --- |
| `netsuke-compiler` | Authored-source processing, expansion/composition, provenance, validation, lowering, and compiler-stage outputs | CLI dispatch or a second implementation for tests |
| `netsuke-lint` | Rules, findings, stable rule identifiers, bounded analysis configuration, and reporting data | Manifest execution as an implicit way to discover facts |
| `netsuke-testing` | Test dialect, discovery, doubles, call journals, assertions, case isolation, and results | Another compiler or private Rust test-support runtime |
| `netsuke-params` | Pure shared parameter types, constraints, defaults, validation, and structured failures | CLI/profile sourcing, bundle acquisition, or Jinja evaluation |
| `netsuke-exec` | Validated structured-command execution, process/stream lifecycle, limits, and cleanup | Authored YAML interpretation, application configuration, or lint policy |
| `netsuke-policy` | Effective policy, trust-aware composition, scoped decisions, and decision evidence | Concrete executor/state/cleanup implementations |
| `netsuke-resource` | Capability-scoped resource identity and bounded ordered lease acquisition | Readiness policy, deletion policy, or a scheduler |
| `netsuke-state` | Readiness/probe interpretation, preparation and evidence lifecycle | An independent process runner or general lock manager |
| `netsuke-artefacts` | Ownership validation, cleanup planning, preview, and capability-scoped deletion | Shell deletion or an independent resource-lock implementation |

*Table 1: Proposed component ownership. Types should remain with their semantic
owner unless an actual dependency cycle requires a smaller shared contract.*

### Compiler, linter, and testing

The compiler returns typed stage products with a compiler-owned origin map.
Rules declare whether they need authored syntax, expanded manifests, or a
resolved graph. Unknown origins remain explicit. Lexical indexing may live
inside the compiler; linting does not maintain a rival parser/lowering model.

Restricted analysis must not silently invoke host capabilities when more data
would be convenient. A rule reports unavailable analysis with its coverage
limits instead of claiming success or acquiring authority. Findings have
stable identifiers and bounded structured results. Existing CLI vocabulary
policy still determines command spelling; a new library is not a new command.

The semantic inventory used by linting and maturity policy has one owner in the
compiler/model contract. Start with a module. Consider a separate analysis crate
only when multiple real consumers and measured/architectural evidence justify
it; do not create one for a handful of shared records.

`netsuke-testing` consumes the same compiler and explicit capability hooks.
Its user-facing YAML dialect, mocks, assertions, and case isolation are
production functionality, not the private support in [RFC 0033][test-support].
Initial manifest testing remains independent of action execution as RFC 0007
requires. Reuse existing process-lifecycle facilities for isolation, but do not
make this release wait for the complete structured-action executor.

The compiler library must exist before linting or testing would otherwise
need `netsuke-build`. The application composes these libraries; no reverse
application dependency is permitted.

### Typed inputs and bundle parameters

`netsuke-params` provides the common pure contract required by
[RFC 0022][inputs] and [RFC 0003][bundles]. It owns type/constraint normalization,
default validation, supplied-value validation, and structured failures. Test
Boolean/integer distinctions, overflow, collection bounds, choices, defaults,
and consistent root-input/bundle-parameter behaviour.

Input sourcing, precedence, provenance, profile discovery, and template
expansion remain with their existing application/compiler owners. A path value
is data, not filesystem authority. Do not introduce another configuration
merger under the parameter name; shared configuration infrastructure belongs
upstream in OrthoConfig.

### Structured execution and plans

`netsuke-exec` accepts validated execution requests, not `Cli`, authored YAML,
or commands that still require semantic interpretation. Reuse existing
process/stream primitives and capability-scoped working directories. Specify
success, failure, cancellation, timeout, bounded capture, partial-startup
cleanup, and pipeline termination consistently with [RFC 0001][commands] and
its [working-directory][cwd], [runtime-binding][bindings], and
[shell-selection][shells] amendments.

Keep plan representation and persisted-plan encoding separate from the executor
implementation. They may initially remain model modules. Extract a `netsuke-plan`
only when its independently used schema/validation machinery warrants it.
Ninja emission and inspection must not import platform process machinery merely
to inspect a plan.

[ADR-054][construction] governs checked construction, interpreter binding,
deserialization, and replay. A serialized path or token must not manufacture
filesystem authority. Revalidate environmental authority at execution time even
when semantic validation already succeeded.

### Policy decisions and enforcement

`netsuke-policy` distinguishes maturity findings from mandatory authority.
[ADR-053][policy-adr] makes this distinction explicit. Quality findings can have
configured severity; suppressing one must not permit a forbidden network hop,
shell, environment access, or cleanup operation.

Keep source trust and effective restrictions in the decision contract. Merge
policy according to each feature's existing authority rules, retaining the
strongest applicable restriction rather than applying an unqualified last-wins
merge. Do not duplicate the existing configuration precedence engine.

Adapters enforce the applicable decision at each effect boundary, including
redirects and persisted-plan replay. The policy component does not depend on
those adapters. Where it needs semantic observations, consume the shared
inventory rather than call the linter or executor.

Crate separation and capability-shaped APIs do not themselves create an
operating-system sandbox. State the enforcement scope and retain behavioural
negative tests alongside import checks.

### States, artefacts, and shared resources

`netsuke-state` owns readiness algebra, default checks, bounded probe results,
preparation, evidence validity, and uncertainty handling under
[RFC 0021][states]. Reuse structured execution; do not implement another child
process manager. A failed or inconclusive probe is not successful preparation.

`netsuke-artefacts` owns exact producer/output scope, cleanup previews, bounded
delete plans, revalidation, and partial-failure outcomes under
[RFC 0023][artefacts]. Use one capability-scoped planner/deleter, never shell
commands assembled from ownership paths.

Both use `netsuke-resource` as specified in [ADR-050][resources]. That component
owns resource identities and bounded, canonically ordered leases independently
of state declarations and Ninja scheduling. [RFC 0024][contention] retains the
different, invocation-scoped meaning of named Ninja contention classes.

The application coordinates cross-feature operations. Before deleting a managed
environment, invalidate its state evidence while holding the same relevant
resource lease; keep that evidence invalid on partial failure. Neither engine
needs to depend on the other. Shared protocols carry the coordination contract
without a state/artefact dependency cycle.

### Creation and reuse policy

A proposed crate must have a coherent invariant/consumer boundary, explicit
normal/build/test dependencies, meaningful focused tests, a publication plan,
and measured costs. Introduce it with a useful implementation, not an empty
placeholder. Single-consumer helpers remain local unless a concrete boundary
requires otherwise.

Optional Cargo features require separate justification, positive/negative
configuration tests, and a documented supported matrix. Defaults preserve
existing user behaviour and reviewed release-stage gates. Avoid combinatorial
feature flags simply to make every manifest capability separately compilable.
[ADR-052][release-adr] keeps the result one product release train.

Host facts and reusable acquisition remain with their existing feature owners
until their own consumer criteria justify extraction. This RFC does not create
additional crates for every item in the broader roadmap.

## Verification and acceptance

Each feature delivery includes its ownership/dependency map, focused tests,
negative architecture controls, applicable capability contracts, and package
consumer checks. Retain the feature RFC's semantic tests: crate placement is not
an alternative acceptance criterion.

Demonstrate that parameter tests do not compile the CLI/executor; lint/test
consumers do not compile the application; plan inspection does not compile the
executor; and policy/resource contracts do not import concrete state/cleanup
engines. Qualify legitimate lower dependencies explicitly.

Use [RFC 0030][benchmark] to measure relevant loops. A safety or semantic
justification may warrant a measured cost, but it requires an explicit decision
rather than an unsupported build-speed claim.

## Alternatives considered

Keeping all future features in the application retains coarse ownership.
Creating all crates now invents interfaces without consumers. One global core
or helper crate concentrates unrelated dependencies. Separate validators,
compilers, executors, and locks per feature create divergent semantics.

## Outstanding decisions

Feature implementation inventories settle exact Rust type placement and shared
contract shape. Revisit optional `netsuke-plan` and analysis crates only against
real consumers. Existing RFCs retain authority over unfinished syntax and
semantics; this proposal does not silently settle their open questions.

## Recommendation

Allocate these boundaries now, instantiate them with feature delivery, and
share only contracts and mechanisms whose ownership and consumers are concrete.

[roadmap]: ../roadmap-crate-decomposition.md
[composition]: ../roadmap-composition.md
[progressive]: ../roadmap-progressive-enhancement.md
[decomposition]: 0031-staged-crate-decomposition.md
[benchmark]: 0030-build-performance-benchmark-extension.md
[test-support]: 0033-component-test-support.md
[semantics]: 0026-hexagonal-domain-hardening.md
[testing]: 0007-netsukefile-testing-framework.md
[inputs]: 0022-typed-task-inputs.md
[bundles]: 0003-versioned-local-bundles.md
[commands]: 0001-structured-command-blocks.md
[cwd]: 0009-structured-command-working-directories.md
[bindings]: 0010-runtime-bindings-and-secure-tempdirs.md
[shells]: 0011-allow-listed-structured-command-shells.md
[states]: 0021-managed-states-and-probes.md
[artefacts]: 0023-artefact-ownership-and-scoped-cleanup.md
[contention]: 0024-named-contention-classes.md
[resources]: ../adr-050-shared-resource-identities-and-leases.md
[policy-adr]: ../adr-053-separate-policy-findings-from-runtime-authority.md
[construction]: ../adr-054-preserve-validated-operation-construction.md
[release-adr]: ../adr-052-lockstep-component-release-and-api-policy.md
[lint-issue]: https://github.com/leynos/netsuke/issues/592
[lint-pr]: https://github.com/leynos/netsuke/pull/621
