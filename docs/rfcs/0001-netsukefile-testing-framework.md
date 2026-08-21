# RFC 0001: Netsukefile testing framework

## Preamble

- **RFC number:** 0001
- **Status:** Proposed
- **Created:** 2026-08-17

## Summary

Add a first-class testing framework for Netsukefiles: a `netsuke test`
command, a YAML test dialect with `given`/`when`/`then` steps, a declarative
mocking model for template functions, environment, clock, and macros, and
hermetic fixtures. Tests evaluate manifests through the same compiler
pipeline as `netsuke build` and assert against the rendered manifest, the
intermediate representation (IR) graph, and the generated Ninja text —
deterministically, without executing build commands.

The proposal is specified in two companion documents:
[UX and semantic design](../netsuke-test-framework-ux-design.md) (the test
dialect and command surface) and
[technical design](../netsuke-test-framework-technical-design.md) (the
implementation architecture).

## Problem

Netsukefiles contain real logic — `foreach` expansion, `when` conditions,
macros, environment probes, globbing, `command_available` branches — and
that logic has no verification story. Authors validate manifests by running
builds and eyeballing output. This fails in four ways:

- Negative properties (a target that must not exist, a manifest that must
  fail with a specific diagnostic) cannot be checked at all.
- Behaviour that depends on the environment (installed tools, environment
  variables, the clock, the network) cannot be pinned, so checks are not
  reproducible across machines or continuous integration (CI).
- Refactoring a non-trivial manifest is unprotected: nothing catches a
  `when` condition that silently stops matching.
- Agents and CI systems have no machine-checkable contract for manifest
  behaviour, which undercuts the roadmap's agent-consistency thesis.

## Current state

The compiler pipeline is already shaped for this feature. Manifest loading
is a staged, injectable library path: `from_path_with_policy_and_env`
accepts a network policy, an injected environment reader, and a stage
callback. `BuildGraph::from_manifest` and `ninja_gen::generate` are public,
composable functions over plain data. Deterministic projections exist for
graph output, and Ninja generation is already snapshot-stable.

The `netsuke help targets` work narrowed the gap further. It introduced a
`StdlibRegistration` enum that selects the standard-library boundary per
load mode, a `ManifestQuery` mode that disables the impure helpers with
located diagnostics, a `disabled_env_reader`, and `src/manifest/query.rs`
as the owner of capability-scoped non-build loading. The test runner is a
third mode of that same shape, so it extends the established pattern
instead of introducing a parallel one; the technical design records the
consequences.

What is missing: a clock seam for `now()` (it calls the system clock
directly), a mechanism to substitute manifest macros, any test dialect,
discovery, mock engine, fixture lifecycle, or `test` subcommand. The
manifest schema rejects unknown top-level keys, so the proposed `tests`
configuration block is a schema addition with compatibility consequences
(see below).

## Goals and non-goals

- Goals:
  - Deterministic, machine-independent verification of manifest-time
    behaviour, including negative cases with named diagnostics.
  - A declarative mocking model at named seams (template functions,
    environment, clock, macros) with strict-by-default verification and a
    call journal.
  - Hermetic per-case fixtures with guaranteed teardown.
  - A `netsuke test` command with human and single-document JSON output,
    consistent with the CLI vocabulary and stream-purity contracts.
- Non-goals:
  - Executing builds or fixture shell commands (designed but deferred
    behind explicit allow flags).
  - Replacing Netsuke's own Rust test suites.
  - A general-purpose scripting language for tests.

## Proposed design

A `netsuke-tests/` tree of YAML test files, discovered via an optional
`tests` block in the Netsukefile. Each file holds named `test_*` cases;
each case is a sequence of steps with `given` (fixtures, environment,
doubles, clock), `when` (pipeline actions: `load_manifest`, `build_graph`,
`generate_ninja`), and `then` (expression and structured assertions over
result views and the mock journal). Doubles follow a stub/mock/spy
taxonomy with first-match-wins call configuration, a closed matcher
vocabulary, and opt-in ordering. Macro substitution swaps a manifest macro
for a stand-in declared in the test file.

Implementation reuses the existing pipeline behind a new options-carrying
loader entry point that registers test overlays after stdlib and manifest
macros and before `foreach` expansion. Two seams are added (clock provider;
macro substitution overlay); network mocking needs no transport seam
because the deny-all policy plus function-level doubles make the real
network code unreachable under test.

Positioning within the product: phase 3 of the roadmap makes Netsuke
predictable for humans and automation; phase 4 verifies the compiler
itself; phase 5 compounds value across repeated invocations. This proposal
extends the verification story from the compiler (phase 4) to the user's
manifests, and gives phase 5's agent-facing surface a contract mechanism:
a Netsukefile with a test suite is a manifest whose intended behaviour an
agent can check before and after editing it. The design deliberately
mirrors the prior art the ecosystem has converged on — OpenTofu's
plan-mode-by-default unit testing, Open Policy Agent's FAIL/ERROR
taxonomy and failure ergonomics, and declarative substitution at named
addresses — so the dialect feels familiar to practitioners of those tools.

## Compatibility and migration

- **Manifest schema.** `tests` becomes an optional top-level key, admitted
  from the `netsuke_version` of the release that ships discovery
  configuration. Older binaries reject a manifest containing the block
  with their ordinary unknown-field diagnostic, not a version message;
  this is accepted and documented, because the failure is immediate,
  located, and names the offending key. No existing manifest changes
  behaviour: manifests without the block are unaffected, and the build
  path ignores the block entirely (build-path neutrality is a named
  invariant in the technical design).
- **CLI vocabulary.** `test` joins the canonical top-level command list.
  No existing command changes.
- **Test dialect versioning.** Test files carry `netsuke_test_version`, a
  `MAJOR.MINOR` string with the acceptance policy defined in the UX design
  §4: same major, minor at most the supported minor; every dialect
  addition is a minor-version event. The dialect evolves independently of
  the manifest schema.

## Alternatives considered

### Option A: instrumented Python (or Rust) test harness

Write manifest tests in a general-purpose language against an instrumented
runtime, as Terratest does for infrastructure. Rejected as the primary
story: Netsukefile semantics live in the Rust pipeline, and an
out-of-process harness either re-implements them (permanent drift, the Act
problem) or shells out to the binary and loses seam-level mocking.
Terratest's own rationale — real end-to-end orchestration needs a real
language — applies to *execution* testing, which this proposal defers, not
to manifest-time verification, which is pure evaluation. A black-box
smoke-test harness remains possible on top of `netsuke test --json` later.

### Option B: assertions embedded in the Netsukefile

Add `check`-style blocks to the manifest itself, as Terraform embeds
custom conditions. Rejected: it mixes test concerns into the artefact
under test (the leakage Act users suffer with `if: ${{ !env.ACT }}`),
cannot express mocking or fixtures, and bloats a schema that agents and
humans read constantly.

### Option C: deterministic overrides with external assertions

Ship only the seam work (clock provider, loader options, overlays) and
surface it as `netsuke generate --overrides overrides.yml`, with
assertions left to whatever harness the user already runs: `insta` or
golden files over the deterministic Ninja output, `jq` over graph JSON,
shell test frameworks. This is the strongest alternative: it delivers
determinism with roughly a fifth of the new surface, needs no manifest
schema change, and the overrides file doubles as a reproducible-build
debugging aid. Rejected as the end state because it forfeits exactly the
properties this RFC exists for — mock verification and the call journal,
hermetic fixtures, named-diagnostic negative tests, and a single
machine-checkable contract an agent can run without assembling a bespoke
harness. It is, however, adopted as sequencing: the first implementation
phase is precisely this substrate, and the phase gate below keeps the
option open if the dialect proves unnecessary.

### Option D: external snapshot testing only

Golden-file tests over `netsuke generate` output driven by shell or CI
scripting. Rejected as insufficient: snapshots cannot mock environment or
network, cannot name expected diagnostics, and produce whole-file diffs
instead of targeted assertions. Snapshot assertions are instead a deferred
addition inside the framework.

## Open questions

- Should the dialect's deferred `execute` action reuse the run-ledger
  machinery (roadmap 5.2) for recording test executions, or keep test runs
  out of the ledger?
- Whether `netsuke context --json` (roadmap 5.1) should enumerate the test
  dialect schema alongside the manifest schema, and in what form.
- The diagnostic-stack direction (miette versus anyhow) is in flight; the
  framework binds to whichever lands, and its error surface should be
  reviewed once that migration settles.

## Recommendation

Adopt the two companion designs; roadmap phase 6 tracks the delivery:
overlay spike, seams, and loader options first (no behaviour change), then
parser and mock engine, then fixtures, actions, CLI wiring, and an
author-facing users' guide chapter. One gate is deliberate: after the seam
phase lands, dogfood it by running the differential fidelity suite over
the repository's own example manifests before the parser and mock-engine
phases start — evidence that the substrate is sound, and a natural exit to
Option C if the dialect's demand assumptions fail. The framework closes
the verification gap for manifest authors, and its deferred execution mode
has a designed, gated path when demand arrives.
