# Debugging plan: serial runtime test failure after rebase

**Generated**: 2026-08-27 **Issue ID**: post-rebase gate failure **Severity**:
high **Falsification sub-agent**: alchemist **Planning agent boundary**: This
document was prepared by the planning agent. Falsification must be executed by
the named sub-agent, not by the planning agent.

## Problem Statement

`make test` failed once in
`shared_serial_work_runs_once_while_unrelated_work_progresses`, although all
other serial-dependency tests and the format, typecheck, and lint gates passed.
The test requires real Ninja scheduling with three jobs and returned a
non-success status without diagnostic stderr. The investigation must
distinguish an environmental timing failure from a rebase regression before
changing code.

## Context summary

Failure context after the rebase:

| Aspect              | Details                                                            |
| ------------------- | ------------------------------------------------------------------ |
| First observed      | 2026-08-27, gate run after rebase onto `origin/main`               |
| Reproduction rate   | One failure in one full-suite run; focused reproduction is pending |
| Affected components | `tests/serial_dependency_runtime_tests.rs`, real Ninja             |
| Recent changes      | Rebase integrated main's split Ninja generation and path escaping  |

### Error artefacts

```plaintext
Error: ninja failed:
shared_serial_work_runs_once_while_unrelated_work_progresses
```

### Information gaps

- The failing Ninja invocation emitted no stderr.
- The test has not yet been rerun in isolation.
- The same focused test has not yet been compared with `origin/main`.

______________________________________________________________________

## Hypotheses

### H1: the full-suite failure was an environmental scheduling transient

**Claim**: Contention during the full nextest run delayed the unrelated task
enough for the shared task's bounded polling loop to fail, while the generated
Ninja graph is correct.

**Plausibility**: high — the test polls for one second, uses real processes,
and its full-suite failure produced no Ninja diagnostic.

**Prediction**: A focused rerun with the same test binary and no concurrent
nextest work succeeds.

#### H1 Falsification plan

| Step | Action                                                   | Expected Negative Result                                                                 |
| ---- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 1    | Run the exact test through nextest with one test thread. | A failure with the same empty-stderr Ninja status disproves a full-suite-only transient. |

**Tooling**:
`cargo nextest run -p netsuke-build --all-features -E
'test(shared_serial_work_runs_once_while_unrelated_work_progresses)'`

**Confidence on falsification**: A reproduced focused failure strongly rules
out contention elsewhere in the suite.

______________________________________________________________________

### H2: the rebased generator changed the serial dyndep graph

**Claim**: The shell-text lowering integration altered the generated serial
graph such that the shared task starts before its unrelated prerequisite or
duplicates work.

**Plausibility**: medium — the rebase touched the dyndep action-rendering path,
but nearby serial tests passed in the same run.

**Prediction**: A focused rerun continues to fail, despite an otherwise idle
test process.

#### H2 Falsification plan

| Step | Action                                 | Expected Negative Result                                                     |
| ---- | -------------------------------------- | ---------------------------------------------------------------------------- |
| 1    | Run the focused test described for H1. | A successful focused run disproves a persistent graph-generation regression. |

**Tooling**: Same focused `cargo nextest` invocation as H1.

**Confidence on falsification**: A pass proves this branch can generate and
execute the required graph under the test's own conditions, though it does not
prove the absence of all timing sensitivity.

______________________________________________________________________

### H3: the runtime fixture pre-escapes a shell arithmetic dollar

**Claim**: `shared_work_graph` supplies `$$((...))`, which was correct when the
old renderer wrote recipe text directly, but the new backend doubles every raw
dollar before Ninja reads it. Ninja therefore passes `$$((...))` to the shell
instead of the required `$((...))` arithmetic expansion.

**Plausibility**: high — the focused test fails before writing its log, and its
shared action contains the only dollar-heavy shell expression in the graph.

**Prediction**: The fixture contains `$$((`, while the new backend's escaping
contract is independently covered by the dollar-escaping tests.

#### H3 Falsification plan

| Step | Action                                                         | Expected Negative Result                                             |
| ---- | -------------------------------------------------------------- | -------------------------------------------------------------------- |
| 1    | Search the shared action fixture for the arithmetic expansion. | The absence of `$$((` disproves this stale-pre-escape explanation.   |
| 2    | Inspect the new dollar-escaping regression cases.              | No case showing raw `$` becomes Ninja `$$` weakens this explanation. |

**Tooling**: `rg` over the serial-runtime fixture and
`tests/ninja_dollar_escaping_tests.rs`.

**Confidence on falsification**: The two source-level observations establish
whether the test input is one escaping layer ahead of the current contract.

______________________________________________________________________

## Recommended execution order

1. **H1** — one focused test is the cheapest decisive check.
2. **H2** — the same result falsifies or supports the integration hypothesis.
3. **H3** — source-level inspection directly tests the remaining boundary
   mismatch.

## Termination criteria

- **Environmental transient**: The focused test passes; rerun the full gate
  suite to establish the final result.
- **Fixture mismatch**: H3 is not falsified; change the test to provide raw
  shell text, then rerun the focused test and the full gate suite.
- **Potential regression**: H3 is falsified; inspect the generated Ninja bundle
  and compare it with `origin/main` before changing source.

## Notes for executing agent

Run only the supplied focused experiment during the falsification phase. Do not
edit files, run the full gate suite, or infer a fix before reporting the
hypothesis assessment. After that assessment, the executing agent is
responsible for necessary remediation and must run the full gate suite. Report
whether each hypothesis is falsified, not-falsified, or inconclusive, including
the command and the failure output.

## Outcome

- H1 was falsified: the focused test failed outside the full suite.
- H2 was not falsified by that failure alone.
- H3 was not falsified: the fixture supplied `$$((...))`, while the backend
  contract doubles raw shell dollars for Ninja.
- The fixture was corrected to raw `$((...))` and `$i`; the focused nextest
  command then passed in 0.391 seconds.
