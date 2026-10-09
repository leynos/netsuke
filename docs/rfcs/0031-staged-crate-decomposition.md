# RFC 0031: Decompose Netsuke through measured component boundaries

## Preamble

- **RFC number:** 0031
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Initial production workspace decomposition and compatibility.
- **Delivery:** [Crate decomposition roadmap][roadmap], phases 30 to 34.
- **Semantic authority:** [RFC 0026][semantics] and [ADR-035][semantic-adr].

## Summary

Extract a small initial set of independently testable components, retaining
`netsuke-build` as the application package and `netsuke` as its existing library
and executable target names. Prioritize the stdlib boundary, supported by
localisation and shared build semantics, then evaluate Ninja and compiler
extractions independently.

This RFC adds physical compilation boundaries to the semantic programme in
RFC 0026. It does not replace that programme, infer that its proposed work has
landed, or require the optional Paralegal experiment. [RFC 0030][benchmark]
provides the measurements; [RFC 0033][tests] prevents test support from pulling
the whole application back into each component.

## Current state

At commit `b6e7cf502a26d16bf7319c0990441b4920271a95`, the
[root library][library] exposes AST, IR, manifest, stdlib, Ninja, localisation,
CLI, and runner modules from one production package. AST means abstract syntax
tree; IR means intermediate representation. The [workspace manifest][manifest]
has a private `test_support` member, and [that member][support] depends back on
`netsuke-build`. The [build script][build-script] recompiles a deliberately
selected command-schema and localisation slice.

These facts suggest candidates, not measured speed-ups. Cargo compilation,
feature unification, build scripts, and linking can make a split neutral or
slower for some workflows. Reconcile open feature work, including the
[linter PR][linter-pr], before scheduling implementation.

## Goals and non-goals

Reduce unrelated work in representative edit/check/test loops, preserve
capability and semantic invariants, and keep the product releasable through
crates.io and existing binary distributions.

Do not create a crate per module, a backend plugin ecosystem, a generic effects
framework, independent repositories, or independent release schedules. Do not
change Netsukefile syntax, the quickstart, CLI spelling, action identity, or
supported platform behaviour incidentally.

## Proposed design

### Initial ownership

All new names are working package names, not assertions of registry ownership.

| Package | Initial responsibility | Explicit exclusion |
| --- | --- | --- |
| `netsuke-core` | Shared build types, graph algorithms, action identity, and genuinely shared recipe/shell primitives | Application dispatch, process/network adapters, and terminal presentation |
| `netsuke-l10n` | Netsuke catalogue inventory, construction, and diagnostic rendering | Configuration discovery and user-locale selection |
| `netsuke-stdlib` | Existing template capability registration, configuration values, and helper implementations | CLI and application orchestration |
| `netsuke-ninja` | Ninja text generation, escaping, backend validation, and dyndep encoding | Template evaluation and child-process orchestration |
| `netsuke-build` | CLI/configuration mapping, composition, runner orchestration, status, themes, and presentation | Ownership of reusable lower-level implementation |
| `netsuke-compiler`, conditional | Manifest loading, expansion/composition, provenance, validation, and lowering | A second compiler inside linting or testing |

*Table 1: Initial component responsibilities. The compiler extraction becomes
an explicit prerequisite when independent linting or manifest testing needs it.*

Inventory concrete imports before moving each subtree. A shell value genuinely
used by both stdlib and lowering belongs below both; a stdlib-only hashing
helper does not become core merely because `src/hasher` exists. Move the minimum
shared semantics, not every convenience function with a generic name.

Core may initially contain authored and resolved representations in separate
modules. Their semantic dependency direction still follows RFC 0026: resolved
operation definitions do not store authored recipes. [ADR-048][core-adr]
defers additional AST/IR/hash/shell microcrates, not that semantic correction.
Parser/evaluation implementation remains outside core.

### Dependency direction and migration exceptions

The application depends on components; components never depend on the
application facade. The compiler consumes shared build types and template
facilities; the Ninja backend consumes resolved build semantics. Linting and
manifest testing consume compiler-owned outputs. They do not become alternate
routes through the CLI.

The target model reports typed diagnostic facts without depending on localisation.
Do not hide a configuration dependency behind a nominally lightweight facade.
A mechanical extraction may temporarily retain existing `LocalizedMessage`
usage through `netsuke-l10n` if removing it would entangle unrelated work. Such
an edge needs a named owner, forbidden new uses, a focused test, and an explicit
retirement dependency on RFC 0026's diagnostic work. Record it in the existing
architecture ledger when available, otherwise in the extraction's reviewed
inventory. Never describe that transitional core as presentation-independent.
Keep semantic-diagnostic mapping in the application during this transition:
`netsuke-l10n` must not import core and create a cycle.

Likewise, [RFC 0032][localisation] permits a clearly labelled upstream-waiting
localisation stage. It does not count as the completed lightweight boundary.
Benchmark and test-support work do not wait for the upstream release.

The scoped import/metadata checks complement [RFC 0027][architecture]. Cargo
edges alone do not prove constructor safety, absence of ambient effects, or
coverage of every generated/cfg-selected path. Preserve negative fixtures and
state analysis limitations rather than inventing a second architecture checker.

### Staged delivery

First characterize compatibility and record the baseline. Separate reusable
fixtures from application fixtures before claiming independently testable crates.
Prepare localisation and shared-type boundaries, then extract the stdlib as the
first substantial candidate. Evaluate Ninja independently after that result.

Bring the compiler extraction forward when `netsuke-lint` or `netsuke-testing`
needs a production compiler library. It must be available before either would
otherwise acquire a dependency on `netsuke-build`. This condition is architectural,
not a requirement to implement the entire future feature immediately.

Each extraction is a reviewable change with a before/after import graph,
compatibility evidence, focused tests, and the benchmark comparison. Avoid
combining a dependency upgrade or language/toolchain migration with file moves.
If a boundary has no practical build benefit, defer or reverse it unless a
separately reviewed semantic/ownership benefit justifies its measured cost.

### Rust API and package obligations

Keep old facade imports through re-exports where practical. Re-export the actual
types; duplicate definitions can break trait identity even when their fields
match. Record deliberately changed public paths and trait implementations.
Moving types can affect inherent implementations, privacy, coherence, and
serialization, so a directory move is not presumed API-neutral.

Do not make fields or unchecked constructors public merely to cross a crate
boundary. [ADR-054][construction] requires checked construction and replay
validation. Preserve the canonical edge arena, aliases, insertion atomicity,
interpreter binding, deterministic hashing, and backend escaping semantics.

Every component inherits the appropriate workspace lint and toolchain policy,
with explicit features and targets. Preserve `legacy-digests` forwarding to its
actual owner without making unrelated components enable it. Do not use blanket
`--all-features` as the only evidence of default-feature behaviour.

Update resource inclusion and relative paths when code moves. Packaged crates
must not reach outside their package for catalogues, build-audit inputs, or
runtime data. Production/build dependencies in the published closure require
registry versions as well as local paths. Private fixture packages remain
outside that closure. [RFC 0035][release] owns release preparation; the
extraction PR must not silently break the current distribution path.

## Verification and acceptance

For each landed extraction, retain:

- A concrete ownership map and a dependency graph for default and relevant
  feature/target configurations, including host build dependencies.
- A focused check and test-compilation run that does not compile
  `netsuke-build`, plus a negative fixture proving the isolation check works.
- Moved unit, behavioural, property, snapshot, and applicable proof tests with
  their fixtures and regression seeds; preserve proof-gate change detection.
- Equivalent manifest acceptance, diagnostics, graph identity, Ninja output,
  shell handling, and CLI behaviour. Explain any intentional difference before
  accepting new snapshots.
- Reproducible package/consumer checks and benchmark evidence under unchanged
  budgets, including cold-build, linking, memory, and disk trade-offs.

No single benchmark licenses bypassing safety or compatibility gates. Workspace
checks remain necessary even when a focused package build passes. This
programme adds no new v0.1.0 release gate.

## Alternatives considered

A monolith preserves simple packaging but retains coarse invalidation. A large
one-shot workspace split obscures causality and creates many simultaneous API
changes. Microcrates for every type/algorithm add release and dependency costs
without evidence. Extracting tests through an application-dependent support
crate leaves the expensive edge intact.

## Outstanding decisions

Confirm package availability before publication; select exact source ownership
from the implementation-head inventory; and retire transitional diagnostic
edges only when their semantic prerequisites are satisfied. The compiler's
physical AST ownership can evolve without changing the resolved-model contract.

## Recommendation

Use a measured, stdlib-led extraction sequence with explicit stop points. Keep
semantic boundaries authoritative and release/test dependency closure visible.

[roadmap]: ../roadmap-crate-decomposition.md
[benchmark]: 0030-build-performance-benchmark-extension.md
[tests]: 0033-component-test-support.md
[localisation]: 0032-localisation-crate-boundary.md
[release]: 0035-lading-backed-workspace-publication.md
[semantics]: 0026-hexagonal-domain-hardening.md
[architecture]: 0027-executable-architecture-contract.md
[semantic-adr]: ../adr-035-semantic-compiler-boundaries.md
[core-adr]: ../adr-048-defer-core-microcrate-decomposition.md
[construction]: ../adr-054-preserve-validated-operation-construction.md
[library]: ../../src/lib.rs
[manifest]: ../../Cargo.toml
[support]: ../../test_support/Cargo.toml
[build-script]: ../../build.rs
[linter-pr]: https://github.com/leynos/netsuke/pull/621
