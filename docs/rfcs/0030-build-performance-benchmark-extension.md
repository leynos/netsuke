# RFC 0030: Extend build benchmarks for component extraction

## Preamble

- **RFC number:** 0030
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Build-performance evidence, not production decomposition.
- **Delivery:** [Crate decomposition roadmap][roadmap], phase 30.

## Summary

Extend `make bench-build` before extracting production crates. Measure the
edit, check, and test workflows that decomposition should improve, alongside
cold application builds and cache-backed continuous integration (CI). Preserve
the existing benchmark's safeguards and linker/frontend comparison mode.

A useful extraction removes unrelated compilation from a real workflow under
the existing resource budget. A larger workspace or a faster single sample is
not evidence of that result. [RFC 0031][decomposition] consumes these results;
it does not assume a percentage improvement.

## Current state

The inspected Netsuke baseline is commit
`b6e7cf502a26d16bf7319c0990441b4920271a95`. The existing
[benchmark script][script] compares platform-linker, `mold`, and parallel
frontend variants. It uses isolated target directories, a recorded shuffle
seed, repeated measurements, exclusive execution, source timestamp restoration,
and explicit compiler-wrapper/flag handling. Its incremental stimulus defaults
to touching `src/main.rs`.

A timestamp touch is a useful invalidation control, but it does not represent
an implementation change inside the library. [ADR-029][build-standard] already
sets the development build standard; this RFC does not change those flags,
the toolchain, release profiles, or coverage exclusions.

## Goals and non-goals

Establish reproducible before/after evidence, independently buildable component
tests, bounded resource costs, and a durable result format. Separate compiler
work from compiler-cache retrieval and from test execution.

Do not introduce hourly full-workspace benchmarks, larger runners, mandatory
privileged page-cache flushing, a new build system, or a production crate split
in the benchmark implementation pull request (PR).

## Proposed design

### Scenario matrix

Represent scenarios as data with stable identifiers and explicit package,
target, feature, command, stimulus, and cache-mode selections. Preserve the
legacy invocation while adding a decomposition-focused suite.

| Scenario | Stimulus | Main question |
| --- | --- | --- |
| Cold application | Empty owned target/build directories | What is the full critical path? |
| No-op application | Repeat an unchanged primed build | What overhead remains without edits? |
| CLI implementation | Small checked source patch | Do lower components stay reusable? |
| Stdlib implementation | Small checked source patch | Does focused stdlib development avoid the application? |
| Ninja implementation | Small checked source patch | Does backend work avoid unrelated frontend work? |
| Shared model | Representative AST/IR implementation and API changes, reported separately | What is the high-fan-out case? |
| Test-only | Small test-body patch | Do focused tests remain independent? |
| Build metadata | Command-schema or catalogue change | What rebuilds in host and target contexts? |

*Table 1: Required scenario families. AST means abstract syntax tree; IR means
intermediate representation.*

For applicable scenarios measure `cargo check`, test compilation with
`cargo test --no-run`, and the application build independently. Keep actual
test execution in a separate series. Retain the touch-only case under a name
that identifies it as a control.

Each implementation stimulus must specify the expected input bytes, intended
replacement, file identity, and resulting content digest. Refuse a mismatched
patch instead of silently measuring another change. Do not treat whitespace
edits as representative implementation changes. Record both equivalent old/new
source locations when an extraction moves the edited function.

### Isolation and repeatability

Run source mutations in an owned disposable checkout of the selected commit,
not in an artisan's live working tree. A dirty source checkout requires an
explicit captured patch and digest, or refusal. Never reset or clean unrelated
work. Preserve exclusive execution and clean up only directories whose
ownership the harness established; guard against empty paths, path traversal,
and symlink substitution before removal.

A sample starts from its documented primed state. Re-prime or restore a verified
snapshot before each different stimulus so one edit cannot warm another case.
Exclude checkout preparation, dependency download, and priming from an edit
measurement, but record their costs separately. Do not call a Cargo-cold build
an operating-system-cache-cold build.

Record the sample order and seed. Compare baseline and candidate on the same
host, with matching limits and flags. Treat changed workspace paths, target
paths, compiler versions, features, and cache namespaces as possible confounders.
A compiler-cache comparison must either preserve comparable paths or explain
and verify the remapping; unrelated cache misses must not masquerade as a
layout regression.

Preserve the current minimum of two repetitions for exploratory use. A merge
recommendation requires at least ten valid paired samples for the decisive
scenario, or a reviewed alternative sampling plan established before seeing
the result. Report every sample, the median, spread, and paired differences.
Do not silently remove outliers. An unstable result triggers investigation or
deferral, not a larger claimed speed-up.

### Distinct cache modes

The compiler-only mode retains local incremental compilation and clears both
compiler wrappers and conflicting encoded flags as the existing script does.
The CI-cache mode explicitly disables incremental compilation, records the
wrapper and cache configuration, and distinguishes cold-cache from warm-cache
runs. Do not share a mutable cache experiment with unrelated jobs or clear a
shared remote cache.

The [sccache Rust documentation][sccache] explains why these modes differ and
why final linking is not a cache hit. Measure relinking and dependent crate
work rather than promising that only the edited crate rebuilds.

### Evidence format and budgets

Write a versioned JSON result and a human-readable summary. Required fields
include source and lockfile digests; tool versions; host/target triples;
packages, targets, and features; exact argument arrays; relevant flags and
wrapper state; Cargo jobs and compiler frontend threads; CPU/memory limits;
cache mode; stimulus digest; seed/order; per-sample wall time and exit status;
and references to raw logs and Cargo timing reports.

Use Cargo's supported timing output and compiler-message stream. Preserve
`compiler-artifact` identities and `fresh` values, distinguishing them from a
claim to enumerate every internal compiler task. Record build-script execution
and host/target contexts where observable. Do not require an unstable JSON
format merely because the repository uses a nightly compiler.

Record target-directory size and peak-memory evidence with the collector's
semantics. Prefer an available process-group/cgroup measurement. A largest-child
RSS value is not simultaneous aggregate compiler memory; label weaker
measurements and do not compare incompatible collectors as the same metric.
Missing measurements are explicit unavailable values, never zero.

A suite has declared time, disk, memory, and concurrency ceilings. Stop cleanly
when a ceiling is exceeded and retain partial evidence. Reuse the agreed CI
resource limits; changing runner size invalidates a like-for-like comparison.
The full suite is opt-in or change-scoped, not a new every-commit build matrix.

## Requirements and acceptance

- Contract tests use a fake Cargo/timer boundary to cover command selection,
  source-patch rejection, repetitions, seed replay, feature selection, wrapper
  sanitation, failure reporting, and interruption cleanup without cold Rust
  builds in every test.
- Integration tests cover owned-directory refusal, lock contention, stale
  outputs, failed compilation, and restoration after interruption. Unsupported
  host metrics produce a visible qualification.
- A captured baseline includes every required scenario family or an explicit
  not-yet-applicable reason. New component names do not make a scenario silently
  disappear before those packages exist.
- Before evaluating an extraction, record the decisive workflows, practical
  improvement threshold, and acceptable cold-build/memory/disk regressions.
  Retain, defer, or reverse the extraction against those declared criteria.
- Dependency-isolation checks from [RFC 0033][test-support] accompany timing
  results. Faster compilation cannot excuse a reintroduced application edge.

## Compatibility and migration

Keep `make bench-build` and the existing variant controls working. Put substantive
new orchestration and result processing in tested scripts following the
[repository scripting standards][scripting]; a thin Bash compatibility entry
point may remain. Update the developer guide with actual commands when they
exist, not speculative flags from this RFC.

The first implementation changes only the benchmark and its tests/documentation.
No performance claim is complete until measurements run on the implementation
head. This programme adds no new v0.1.0 release gate.

## Alternatives considered

A single clean build cannot reveal focused test improvements. Timing only
`src/main.rs` touches hides library invalidation. Simultaneously tuning flags
and decomposing crates prevents attribution. A full CI Cartesian product would
spend the expected savings before establishing them.

## Outstanding decisions

Choose the smallest scenario-manifest format, memory collectors for supported
hosts, result retention limit, and practical acceptance thresholds during the
baseline work. Record these before comparing candidate layouts.

## Recommendation

Land the benchmark extension first and use its evidence as an extraction gate,
not as retrospective decoration for a predetermined workspace diagram.

[roadmap]: ../roadmap-crate-decomposition.md
[decomposition]: 0031-staged-crate-decomposition.md
[test-support]: 0033-component-test-support.md
[script]: ../../scripts/bench-build.sh
[build-standard]: ../adr-029-mold-and-parallel-frontend-as-build-defaults.md
[scripting]: ../scripting-standards.md
[sccache]: https://github.com/mozilla/sccache/blob/main/docs/Rust.md
