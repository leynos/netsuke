# RFC 0033: Make component tests independent of the application

## Preamble

- **RFC number:** 0033
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Rust test-support ownership and executable isolation contracts.
- **Delivery:** [Crate decomposition roadmap][roadmap], phases 30 and 32.

## Summary

Split general test utilities from application-specific fixtures before relying
on crate decomposition for focused test speed. Colocate component-specific
tests and helpers with their owner. Keep the future user-facing
`netsuke-testing` product component distinct from private Rust test support.

## Current state

The [existing private support crate][support] depends on `netsuke-build` under
the `netsuke` alias and forwards `legacy-digests` into it. An extracted stdlib
that reuses this support crate for its tests would still compile the entire
application dependency closure.

This is a normal Cargo dependency, not a problem that moving production source
files alone can solve. [RFC 0031][decomposition] therefore treats test-graph
isolation as part of every extraction's acceptance criteria.

## Goals and non-goals

Make focused component tests actually focused, preserve behavioural and proof
coverage, and give fixtures one clear owner. Keep dependency injection and
existing bounded process/filesystem seams rather than replace them with ambient
state or monkeypatching.

Do not implement the Netsukefile testing language here, upgrade testing
frameworks incidentally, create a universal mocking framework, or move every
helper into a shared crate solely to avoid a small amount of local test code.

## Proposed design

### Three ownership zones

| Zone | Location and ownership | Permitted dependencies |
| --- | --- | --- |
| General fixtures | A private `netsuke-test-support` member, introduced only for genuinely shared utilities | Small platform/test libraries; no Netsuke application or feature implementation |
| Component fixtures | Unit-test modules, component `tests/support`, and local regression data | The component under test and its legitimate lower-level contracts |
| Application fixtures | Root `tests/support`, or the existing `test_support` as a temporary application-only package | Application composition, CLI, binary discovery, and end-to-end helpers |

*Table 1: Test-support zones. The working package name does not imply a
publishable public testing API.*

Inventory helpers by their actual dependencies before moving them. General
fixtures can own temporary directories, deterministic clocks, bounded stream
fixtures, and reusable test assertions. A helper that constructs a BuildGraph,
loads a Netsukefile, or locates the application binary is not general merely
because multiple tests call it.

Move feature-aware helpers with their component. Put cross-component contract
fixtures at the lowest legitimate common owner, or retain them as application
integration tests. Do not parameterize away every import while leaving a
callback that secretly loads the application at runtime.

The existing `test_support` name can remain temporarily for root tests. New
lower-level components must not acquire it. Avoid a mass alias rename before
ownership is clear. Any replacement support package remains `publish = false`
and outside production/build dependency closures.

### Test and proof migration

Move unit and behavioural tests, property strategies, regression seeds,
snapshots, corpus data, and component-local fixtures with the implementation.
Record old/new test identities so a green run cannot conceal lost discovery.
Preserve meaningful test groups, serialization constraints, slow-test budgets,
and platform-specific coverage.

Move applicable Kani harnesses and update the proof-input ownership manifest and
change-scoped gate together. Preserve existing model bounds and positive
controls. Do not broaden a source move into a proof-tool upgrade or spend
unbounded CI time on source-building external tools.

Tests needing private invariants should normally remain unit tests with their
owning types. Do not add public unchecked constructors, or a publicly selectable
production feature, merely to let an external fixture manufacture invalid
resolved operations. [ADR-054][construction] governs that boundary.

Keep dependency injection at the existing feature seams. Shared fixture libraries
must not mutate process-wide environment, install a global tracing recorder
without the existing isolation contract, or turn missing mocks into real host
network/process access. Reuse [ADR-008][environment] and the existing
[Ninja isolation guidance][ninja-tests].

### Dependency-isolation checks

For each component, define a forbidden dependency set and allowed lower-level
closure for default features and the relevant feature/target variants. Inspect
Cargo metadata, including normal, build, and development dependencies, and
record the selected target/configuration. A manifest-text search alone is
insufficient because aliases and transitive edges can hide the application.

Pair graph checks with an isolated focused build, for example:

```bash
cargo test -p netsuke-stdlib --locked --no-run --timings
```

This command becomes applicable after the package exists. Run it without an
application package selected in the same invocation. Inspect the compiler
messages to assert that `netsuke-build` does not compile; distinguish fresh
artefacts from new work and retain enough package identity to catch aliases.
An already-warm application artefact is not proof that the dependency vanished.

Use a small fixture workspace to prove the graph check rejects an injected
application edge, feature-activated edge, and build-dependency edge. Positive
controls must still accept legitimate lower-level fixtures. Do not spawn a cold
copy of the real workspace for each checker unit test.

These checks feed [RFC 0027][architecture] rather than establish another
architecture language. State coverage limits for cfg, target features,
procedural macros, and generated code. Both graph checks and runtime boundary
contracts remain necessary.

### Publication and feature behaviour

Path-only private development dependencies may not be usable in a packaged
consumer. Define separately the workspace test suite, package validation, and
standalone consumer fixtures. Do not publish private support packages simply
to make an otherwise inappropriate package test work.

Keep component feature tests independent. Forward `legacy-digests` only to its
owning implementation, test its disabled/default and enabled cases, and prevent
application support from enabling it globally. A workspace `--all-features`
pass does not replace isolated default-feature consumer checks.

## Compatibility and migration

First classify and separate helpers while production remains in the root
package. Retain root tests during this step. As each component moves, migrate
its fixtures/tests and add its isolation contract in the same bounded change.
Remove obsolete root coverage only after the replacement demonstrates the same
behaviour and discovery count, with intentional count changes explained.

The future [Netsukefile testing framework][testing-rfc] is a production feature
with its own compiler dependency and release obligations. It must not reuse the
private support package as its runtime implementation or depend on the CLI to
obtain compiler services.

## Verification and acceptance

Completion requires a helper ownership inventory, negative and positive checker
fixtures, independently compiling component test targets, preserved regression
and proof evidence, and explicit package-consumer tests. The full workspace
suite still exercises cross-component integration.

Measure test-only and implementation-edit cases using [RFC 0030][benchmark].
Report compile time separately from test execution; do not claim a test-runtime
speed-up from removing compilation alone.

## Alternatives considered

Keeping the existing support dependency everywhere defeats focused builds.
Copying helpers into every component creates divergent capability and timing
semantics. One generic mega-fixture crate recreates the dependency concentration.
Public test-only escape hatches weaken production invariants.

## Outstanding decisions

Choose the smallest genuinely common helper set from current consumers. Keep
single-consumer helpers local until real reuse appears. The exact package/path
rename is an implementation detail; the forbidden application dependency is not.

## Recommendation

Separate fixture ownership before the first substantial extraction and require
an executable test-graph isolation check for every new component.

[roadmap]: ../roadmap-crate-decomposition.md
[decomposition]: 0031-staged-crate-decomposition.md
[benchmark]: 0030-build-performance-benchmark-extension.md
[architecture]: 0027-executable-architecture-contract.md
[testing-rfc]: 0007-netsukefile-testing-framework.md
[construction]: ../adr-054-preserve-validated-operation-construction.md
[environment]: ../adr-008-environment-seam-taxonomy.md
[ninja-tests]: ../test-isolation-with-ninja-env.md
[support]: ../../test_support/Cargo.toml
