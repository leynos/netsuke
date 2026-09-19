# RFC 0027: Check an executable architecture contract

## Preamble

- **RFC number:** 0027
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Repository architecture policy and its enforcement.
- **Decision record:**
  [ADR-030](../adr-030-layered-architecture-enforcement.md).
- **Delivery:** [Architecture roadmap](../roadmap-hexagonal-hardening.md), phase
  28, alongside phases 26 and 27.

## Summary

Combine a small Rust dependency checker with type-level restrictions, executable
port contracts, effect-policy probes, and reviewed migration exceptions. Enforce
[RFC 0026](0026-hexagonal-domain-hardening.md)'s semantic boundaries rather than
a prescribed directory tree. Treat semantic flow analysis separately under [RFC
0028](0028-paralegal-architecture-experiment.md); structural hardening must
remain useful when that experiment fails.

## Precedents and adaptation

[Wildside's checker][wildside] separates source acquisition from an in-memory
`syn`-based lint and enforces Rust module dependencies. Borrow that
decomposition and its executable negative-probe approach. Do not copy Wildside's
fixed `domain`/`inbound`/`outbound` directory classification or assume syntax
inspection provides complete Rust symbol resolution.

[Corbusier's contributor policy][corbusier] requires an existing-abstraction
search and records ownership, permitted callers, reuse, and composition before
introducing a port. Netsuke already carries that discipline in `AGENTS.md`.
Apply it to application execution contracts and share behavioural fixtures
between implementations; do not equate ordinary Clippy checks with architectural
proof.

[Hecate][hecate] supplies useful declarative groups, deterministic diagnostics,
and re-export-aware dependency concepts. Its [exception controls][hecate-config]
and [origin-resolution concerns][hecate-104] motivate visible debt and explicit
uncertainty. Borrow these concepts, not its Python engine or an assumption of
complete analysis. A Rust port of Hecate is not a prerequisite.

[wildside]: https://github.com/leynos/wildside/blob/main/tools/architecture-lint/src/lib.rs
[corbusier]: https://github.com/leynos/corbusier/blob/main/AGENTS.md
[hecate]: https://github.com/leynos/hecate/blob/main/README.md
[hecate-config]: https://github.com/leynos/hecate/blob/main/docs/configuration.md
[hecate-104]: https://github.com/leynos/hecate/issues/104

## Logical responsibilities

Classify module responsibilities independently of feature colocation. A file
that mixes model definitions and lowering may need a focused split; naming all
of `ir` as the model would wrongly prohibit legitimate frontend-to-model
conversion.

| Role                      | Responsibility                                          | Permitted dependency direction                                     |
| ------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------ |
| Model                     | Validated operations, edges, identities, semantic facts | Model and approved value-oriented libraries                        |
| Frontend                  | Authored schema, parsing, template adaptation           | Frontend libraries and declared evaluation contracts               |
| Compiler                  | Resolution and lowering                                 | Frontend representations, model, owned compiler services           |
| Application and ports     | Requests, orchestration, outcomes                       | Compiler entry points, model, owned contracts                      |
| Adapters and presentation | Execution, external effects, rendering                  | Their implemented or invoked contracts and approved infrastructure |
| Composition               | Production wiring                                       | Explicitly enumerated construction entry points                    |

_Table 1: Proposed responsibility map, not a claim about current compliance._

Ports belong to the application or compiler service that needs the interaction,
not to the concrete adapter. A composition module is an explicitly authorized
wiring site, not a blanket exception. Pure transformations remain functions.
Value types such as paths and durations do not themselves acquire filesystem or
clock authority. Classify backend hints deliberately rather than replacing them
with an untyped property bag or banning every serialization derive.

## Proposed checker

### Separate acquisition, analysis, policy, and rendering

Use four independently tested stages: inventory sources, analyse dependencies,
evaluate policy, and render diagnostics. Policy evaluation consumes facts and
does not access the filesystem, environment, GitHub, or localization machinery.
A repository-local Rust tool may reuse appropriately licensed Wildside code;
retain attribution and isolate portable logic from Netsuke-specific policy.

The implementation may introduce `architecture.toml` and
`architecture-exceptions.toml`. These are proposed maintainer files, not new
Netsukefile syntax or files installed by this documentation change. Version the
policy schema and diagnostic JSON independently. Validate the policy before
analysing any code; malformed or ambiguous classifications are errors.

### Inventory and coverage

Inventory every first-party Rust target and root, including support crates,
build scripts, integration tests, and tooling. Assign tests and intentional
negative fixtures explicit roles or narrowly justified exclusions. Account for
inline modules, `#[path]`, and each supported target/feature configuration.

A missing or unreadable required root, parse failure, unknown internal module,
ambiguous role, or unsupported protected-boundary form must not produce a clean
verdict. Compare discovered inventory with expected roots and target metadata.
An empty scan is an error. Newly added source cannot disappear because the
scanner only walks yesterday's three directories.

The initial dependency analysis may scan syntax across conditional branches.
Record whether a result describes that conservative union or a specific compiled
configuration. Do not imply macro-expansion, generated-code, or target coverage
that the implementation lacks. Exclusions must state what other check owns the
omitted boundary, or report it as incomplete.

### Bounded origin-aware analysis

Support ordinary and grouped imports, aliases, relative module paths, and
statically resolvable local re-exports. Resolve `self`, `super`, and `crate`
using module context. Match module path segments, not raw string prefixes. A
facade must not hide a forbidden dependency merely by changing its spelling.

For unresolved cross-role first-party imports or wildcards, require an explicit
import or emit an incomplete-analysis diagnostic. Do not guess permission. Keep
the supported subset documented and tested. Compiler-aware checks may later
handle a justified narrow gap; do not gradually reimplement rustc's type or
macro resolver inside a syntax linter.

### Initial rules

| Rule      | Obligation                                                                                   |
| --------- | -------------------------------------------------------------------------------------------- |
| NSARCH001 | Model types do not depend on authored manifest AST types                                     |
| NSARCH002 | Semantic diagnostics and port contracts do not expose presentation/parser framework types    |
| NSARCH003 | Application requests and orchestration do not depend on `Cli` or concrete execution adapters |
| NSARCH004 | Ambient effects occur only at approved acquisition sites                                     |
| NSARCH005 | Required roots, internal dependencies, and classifications have explicit analysis coverage   |

_Table 2: Initial rule identifiers and their intended obligations._

Each rule needs documented scope, permitted value-library dependencies, a
positive example, a violating example, and a remedy. Do not interpret NSARCH002
as prohibiting parser libraries in the frontend adapter. NSARCH004 combines
structural ownership rules with existing compiler lints; a syntax pass alone
does not prove that all effectful calls were found.

Diagnostics carry schema version, rule, source item, target origin, location,
classification, and a useful origin witness where available. Include remediation
such as moving AST-to-model conversion into lowering, rather than only reporting
an import. Order results deterministically; paths must not depend on a temporary
checkout location. Keep human text readable without colour and JSON facts
independent of localized wording.

## Migration exceptions and governance

Do not generate the allowed dependency matrix from today's implementation. Keep
approved policy, observed edges, temporary debt, and uncertainty separate.

An exception records a stable identifier, rule, source item, target origin,
limited use-site scope, reason, remediation issue/task, accountable owner or
review role, and removal condition. Avoid line-number-only identity and broad
module-wide permits. Record a reviewed maximum scope so one deleted violation
cannot authorize an unrelated replacement under the same exception.

Normal gate output includes exempted violations. Fail on unmatched, expired,
ambiguous, broadened, or unowned exceptions. Where an expiry uses wall time,
record the evaluation date and inject it in tests; retain a deterministic
removal condition independent of the clock. A diagnostic distinguishes
permitted, forbidden, exempted, unclassified, and unresolved facts.

Begin with reviewed existing debt and forbid new violations. Remove the exact
exception when its remediation lands. Do not allow a newly introduced violation
into the baseline automatically. Policy, roots, markers, exclusions, exceptions,
and CI invocation changes are architecture-sensitive review surfaces. Tests
cannot prevent an authorized contributor from changing both a rule and its
expected result; review remains part of governance.

## Effect and semantic contracts

Retain Netsuke's existing `clippy.toml` environment and current-directory rules
and ADR-008's proportional seams. Use compiler lint probes for concrete API bans
and the architecture checker for module ownership. Qualified `#[expect]` sites
need reasons and coverage; broad suppression or malformed exclusions must fail
negative probes. Do not claim an unfulfilled-expectation lint judges whether an
exception's architectural reason is valid.

Type/API tests enforce unresolved-state and shell-binding restrictions. Port
contract tests exercise recording/failing and production adapters where the
contract applies. Integration tests retain production-specific process and
filesystem obligations. Compile-fail tests need positive controls or specific
failure assertions so unrelated compilation errors cannot satisfy the test.

Keep `make test`'s separate nextest and doctest passes. Doctest compile-fail
coverage must not disappear because nextest does not execute it. Pure checker
helpers warrant property tests and, where useful, small existing proof tools; no
new formal-verification stack is required to bootstrap enforcement.

## Gate qualification and operational requirements

Provide independently specified negative fixtures for every rule. Include
aliases, facade re-exports, nested and conditional modules, missing roots,
malformed policy, stale exceptions, suppressed effect lints, and legitimate
adapter wiring. Seed policy-weakening mutations that remove a prohibition,
broaden an exception, omit a source root, and disable the gate invocation. Each
must fail the relevant executable contract.

Introduce a named `make lint-architecture` target only with its implementation.
Wire it into normal linting and the required CI path. Execute probes through the
actual command and shell to establish non-zero failure propagation; checking
that a workflow contains a reassuring step name is insufficient. Coordinate
workflow-policy ownership with RFC 0008 instead of creating another gate policy.

Keep non-build analysis within 1 vCPU and 2 GiB. Building the Rust checker may
use at most 4 vCPU and 8 GiB. Cache every installation/build, prefer vetted
binaries where available, and run most fixtures in memory. Measure cold and warm
runtime, peak memory, and billed runner usage before making a gate required. The
rollout must record the accepted budget; do not quietly increase runners or add
scheduled jobs. RFC 0028 owns its separate experimental budget.

## Compatibility and migration

No runtime behaviour or repository gate changes in this proposal. Phase 28 first
establishes ownership and a checked baseline, then implements coverage and rule
checks, and finally qualifies required execution. Phases 26 and 27 retire debt
incrementally. Their delivery does not wait for a complete generic checker or
for Paralegal. Preserve the existing release scope and contributor workflow
until the new gate has passed its own tests.

## Alternatives considered

Documentation-only guidance cannot detect regressions. A directory-only rule
misclassifies compiler lowering. A compiler-complete custom resolver is too
large an initial dependency. Pure import checking cannot establish semantic
resolution or resource lifetime. Crate boundaries may help later, once the
semantics justify them; a broad split is not the initial enforcement mechanism.

## Outstanding decisions

The policy review must settle the exact first-party inventory, approved
value-library dependencies, and exception reviewer before enabling a strict
gate. Record a measured performance budget at qualification. These choices must
not silently default to unrestricted permission.

## Recommendation

Adopt layered enforcement with explicit coverage and debt. Require every
important boundary to have an owner, an executable obligation, a failing
example, and a reviewed exception mechanism.
