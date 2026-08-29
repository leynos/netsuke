# Debugging plan: Investigate slow `netsuke-build` integration tests

**Generated**: 2026-08-28
**Issue ID**: PR #588 follow-up
**Severity**: Medium
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem statement

Three passing `netsuke-build` integration tests exceeded the 60-second nextest
slow-test warning threshold on PR #588. They delayed CI feedback after an
unrelated test failed. The goal is to measure each test in isolation, identify
its dominant cost, and remove only work that does not contribute to the test's
behavioural contract.

## Context summary

Table: Initial context and observations for the affected tests.

| Aspect              | Details                                                                   |
| ------------------- | ------------------------------------------------------------------------- |
| First observed      | PR #588 CI run; 2026-08-28 investigation                                  |
| Reproduction rate   | One reported CI run; local measurements pending                           |
| Affected components | Dependabot property fixture, locale stub UI harness, packaging smoke test |
| Recent changes      | PR #588 changes manifest query discovery, not these test contracts        |

### Error artefacts

```plaintext
generated_layouts_include_only_tracked_manifests: 132.949s
harness_compiles_under_a_split_build_dir: 131.395s
packaged_manifest_retains_build_script_sources: 103.115s
```

### Information gaps

- The CI runner's Cargo cache state, CPU allocation, and concurrent test load
  are not available locally.
- The reported run does not expose per-subprocess timings.

______________________________________________________________________

## Hypotheses

### H1: Proptest repeats isolated Git setup and discovery

**Claim**: `generated_layouts_include_only_tracked_manifests` is dominated by
its default proptest case count, where every case creates a temporary Git
repository, writes manifests, runs `git init` and `git add`, then invokes the
Git-backed discovery helper.

**Plausibility**: High — the property body executes those process and
filesystem operations once per generated case.

**Prediction**: Reducing only `PROPTEST_CASES` will reduce elapsed time roughly
proportionally, while a single case still exercises the same operations.

#### H1 falsification plan

Table: Falsification steps for the repeated Git setup hypothesis.

| Step | Action                                                     | Expected Negative Result                        |
| ---- | ---------------------------------------------------------- | ----------------------------------------------- |
| 1    | Time the focused nextest case with the default case count. | It completes quickly despite the default count. |
| 2    | Repeat with `PROPTEST_CASES=1` and compare elapsed time.   | Runtime does not materially decrease.           |
| 3    | Capture `git` child-process time with `strace -f -c`.      | Git time is a small fraction of elapsed time.   |

**Tooling**: `cargo nextest`, `PROPTEST_CASES`, `/usr/bin/time`, and `strace`.

**Confidence on falsification**: High; case-count scaling and child-process
accounting distinguish property execution from a one-off harness cost.

______________________________________________________________________

### H2: Split-layout locale harness recompiles `test_support`

**Claim**: `harness_compiles_under_a_split_build_dir` is dominated by the
intentional fresh `cargo build --manifest-path test_support/Cargo.toml` that
uses two private temporary build directories, rather than by the final
`rustc --emit=metadata` control-fixture check.

**Plausibility**: High — private directories prevent cache reuse to retain
isolation and split-directory coverage.

**Prediction**: Cargo's build child consumes nearly all elapsed time and the
metadata-only `rustc` step is short.

#### H2 falsification plan

Table: Falsification steps for the split-layout locale harness hypothesis.

| Step | Action                                                                | Expected Negative Result                                |
| ---- | --------------------------------------------------------------------- | ------------------------------------------------------- |
| 1    | Time the focused test with child-process accounting.                  | No Cargo child dominates elapsed time.                  |
| 2    | Add scoped diagnostics around `build_with` and `compile`, then rerun. | `compile` is comparable to or slower than `build_with`. |
| 3    | Repeat after a warm shared dependency cache.                          | The fresh private build remains negligible.             |

**Tooling**: `cargo nextest`, `/usr/bin/time`, `strace`, and scoped test
diagnostics if process accounting is insufficient.

**Confidence on falsification**: High; the test's two subprocess boundaries
allow direct attribution without weakening isolation.

______________________________________________________________________

### H3: Packaging smoke coverage runs redundant Cargo work

**Claim**: `packaged_manifest_retains_build_script_sources` is dominated by
`cargo publish --dry-run` using a private target directory, followed by a second
`cargo package --list` subprocess; the latter provides the actual path
assertions and may duplicate package creation.

**Plausibility**: High — the test creates a new target directory and invokes
two package-oriented Cargo commands serially.

**Prediction**: Per-child timing will show one command dominates, and Cargo
documentation or output will establish whether `publish --dry-run` subsumes the
package-list contract.

#### H3 falsification plan

Table: Falsification steps for the redundant Cargo work hypothesis.

| Step | Action                                                                   | Expected Negative Result                                                 |
| ---- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| 1    | Time the focused test with child-process accounting.                     | Neither Cargo child dominates elapsed time.                              |
| 2    | Add scoped diagnostics around publish and package-list calls.            | Both calls are short or the list call alone is essential.                |
| 3    | Run the candidate reduced command sequence against a known omitted file. | It fails to detect a missing required path or publishability regression. |

**Tooling**: `cargo nextest`, `/usr/bin/time`, `strace`, test diagnostics, and
a temporary package-fixture experiment outside the tracked tree.

**Confidence on falsification**: Medium; command overlap must be established
without removing the publication-boundary assertion.

______________________________________________________________________

### H4: Concurrent nested Cargo processes amplify CI latency

**Claim**: The CI-only 100-second-plus runtimes arise when the locale harness
and packaging smoke tests each start an isolated nested Cargo process beside
other build-heavy tests, competing for CPU, I/O, and Cargo's shared cache.

**Plausibility**: High — isolated local runs take 22–29 seconds, whereas the
reported CI runs were four to six times slower.

**Prediction**: A controlled concurrent run of the two focused tests has a
larger maximum per-test duration than a serial run; a targeted nextest test
group can therefore reduce total elapsed time and shorten failure reporting.

#### H4 falsification plan

Table: Falsification steps for the concurrent Cargo process hypothesis.

| Step | Action                                                      | Expected Negative Result                                  |
| ---- | ----------------------------------------------------------- | --------------------------------------------------------- |
| 1    | Run both focused tests with nextest's normal concurrency.   | Their durations match isolated runs.                      |
| 2    | Repeat with `-j 1` and compare per-test and total duration. | Serial execution is not materially faster or more stable. |
| 3    | Inspect resolved nextest configuration.                     | No targeted group can express the required exclusion.     |

**Tooling**: `cargo nextest`, the resolved nextest configuration, and its
per-test timing output.

**Confidence on falsification**: Medium; the local host cannot duplicate CI's
CPU allocation, but a large contention gap establishes a portable cause.

______________________________________________________________________

## Recommended execution order

- **H1** — cheapest and most decisive; it avoids Cargo compilation.
- **H3** — child process count is visible and may yield a direct reduction.
- **H2** — likely intentional compilation cost, so measure before changing
  isolation or test configuration.
- **H4** — determine whether a targeted nextest group is warranted before
  altering scheduling.

## Termination criteria

- **Root cause identified**: Each test has isolated timings that attribute
  most elapsed time to fixture work, a child process, or serial contention.
- **Escalation trigger**: If no operation accounts for most runtime, collect a
  CI trace with runner CPU and cache metadata before changing thresholds.

## Notes for executing agent

Run one focused test at a time for isolated baseline measurements and preserve
raw output under `/tmp`. The controlled H4 experiment is an explicit exception:
run both focused tests concurrently, as required by its falsification plan. Do
not run repository-wide gates, mutate the shared environment in-process, or
change test configuration. Report elapsed wall time, child-process breakdown,
and whether a hypothesis was falsified, not-falsified, or inconclusive.

## Measurement results and decisions

Table: Measured baselines, dominant costs, and resulting decisions.

| Test                                               | Local baseline                      | Dominant cost                                                            | Decision                                                                               |
| -------------------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| `generated_layouts_include_only_tracked_manifests` | 3.180s at 256 cases; 0.634s at 32   | Temporary Git fixture and discovery per property case                    | Use 32 cases; retain the property and generated set coverage.                          |
| `harness_compiles_under_a_split_build_dir`         | 22.339s warm; 44.928s under tracing | Fresh private `cargo build` (43.578s traced); metadata compile is 0.156s | Retain private split directories for E0460 isolation; print scoped subprocess timings. |
| `packaged_manifest_retains_build_script_sources`   | 24.498s warm; 29.483s under tracing | `cargo publish --dry-run` (28.573s traced); package listing is 0.243s    | Retain both publication and package-path contracts; print scoped subprocess timings.   |

The concurrent Cargo experiment reduced total wall time but raised worst-test
latency from 56.102s to 89.175s. It had external workspace activity, so this
investigation does not change nextest scheduling or the 60-second threshold.
Immediate diagnostics identify a future slowdown without hiding it.
