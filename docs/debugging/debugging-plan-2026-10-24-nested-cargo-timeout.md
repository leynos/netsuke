# Debugging plan: nested Cargo fixture timeout

- **Generated**: 2026-10-24
- **Issue ID**: `make test` timeout in `case_2_compiler_diagnostic`
- **Severity**: high
- **Falsification sub-agent**: `alchemist`
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem statement

The `make test` run timed out
`split_build_fixture_compiles_through_the_direct_rustc_harness::case_2_compiler_diagnostic`
after 900 seconds. The case creates a temporary two-crate workspace, asks
Cargo to build it, and checks that the expected compiler diagnostic is
retained. The full run had 4,856 passing tests and one timeout. It is not yet
clear whether the child Cargo build was delayed by shared compiler admission or
stalled in the test and fixture logic.

## Context summary

| Aspect              | Details                                                                                                              |
| ------------------- | -------------------------------------------------------------------------------------------------------------------- |
| First observed      | 2026-10-24 full-suite gate                                                                                           |
| Reproduction rate   | One timeout in one full-suite run                                                                                    |
| Affected components | `tests/locale_stub_ui_tests.rs`, Nextest policy, shared build admission                                              |
| Recent changes      | A scoped 900-second allowance was added for this test group after an earlier cold fixture run exceeded five minutes. |

### Error artefacts

```plaintext
TIMEOUT [900.139s] ... split_build_fixture_compiles_through_the_direct_rustc_harness::case_2_compiler_diagnostic
Summary [2935.749s] 4857/4864 tests run: 4856 passed, 1 timed out, 6 skipped
```

The full log is
`/tmp/test-7d2cdc6e-6293-4c60-8e07-9edea901ad54-3-12-3-terminal-rendering-regression-tests-2.out`.
The captured admission status is
`/tmp/build-limits-status-test-gate-7d2cdc6e.out`.

### Information gaps

- The admission snapshot is from while the test was active, not at the exact
  900-second termination point.
- The child process tree was not captured at termination.
- No isolated run of this single case has been made under a healthy, quiet
  compiler pool.

## Hypotheses

### H1: detached cached work held all four compiler slots

**Claim**: Detached cached compiler work held all four shared compiler tokens
through the timed case, leaving its child Cargo build queued long enough to
exceed the 900-second budget.

**Plausibility**: High — the captured status during the test reports a healthy
but full pool, `live_work` retained by sccache, and many requests waiting
across other worktrees.

**Prediction**: The status snapshot was captured during the timed test and
shows all four tokens held by detached work, with no active compiler groups and
queued compiler requests.

#### H1 falsification plan

| Step | Action                                                                             | Expected negative result                                                                                                                     |
| ---- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | Correlate the Nextest case timeline with the saved `build-limits-status` snapshot. | Falsify H1 if the snapshot predates or follows the case, if a slot was available, or if any held token belonged to an active compiler group. |

**Tooling**: Captured Nextest log, saved build-limits status, and the source
references in context pack `netsuke-pr809-test-timeout`.

**Result**: Falsified. The status file timestamp overlaps the test at about 225
seconds elapsed. It shows one `server-drain` group and three active cached
compiler groups, not four detached groups. It also lists a `rustc` request
under this worktree's temporary fixture target waiting for 57 seconds. This
proves contention at that snapshot, but does not attribute the full 900-second
duration to contention.

**Confidence on falsification**: High that the all-detached claim is wrong;
limited for attributing the full timeout without a process timeline.

### H2: shared admission delayed the fixture compiler request

**Claim**: The timed test's Cargo child had to wait for a compiler slot while
other worktrees occupied all four slots, contributing to its long runtime.

**Plausibility**: High — the overlapping status snapshot names the fixture's
temporary target path as a waiting `rustc` request and shows all slots held by
one retained drain and three active compiler groups.

**Prediction**: The saved status snapshot overlaps the failing case and lists
both the fixture's waiting compiler request and other work holding the pool.

#### H2 falsification plan

| Step | Action                                                                                             | Expected negative result                                                                                             |
| ---- | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 1    | Compare the snapshot timestamp and path with the case timeline and `create_split_build_workspace`. | Falsify H2 if the snapshot does not overlap the case, or if the waiting target path is not produced by this fixture. |

**Result**: Not falsified. The snapshot is contemporaneous with the active
case, its waiting path is under this worktree's `target/`, and the fixture
creates its temporary workspace there. The snapshot proves a wait of at least
57 seconds, not that this pressure lasted for the full test budget.

**Confidence on falsification**: High for the observed wait; limited for the
whole-run cause because no process timeline was captured at termination.

## Recommended execution order

1. **H1** — Inspect the existing log and snapshot; this was read-only and
   falsified the claim that all four slots were detached.
2. **H2** — Correlate the waiting compiler path with the fixture workspace;
   this confirmed that shared admission delayed at least part of the build.

## Termination criteria

- **Root cause identified**: H2 confirms shared admission delayed the fixture
  compiler request; it does not explain the full 900-second duration.
- **Escalation trigger**: If this case times out again, capture its child
  process tree and admission snapshots through termination before changing the
  fixture or timeout.
- Do not raise the per-test allowance above 900 seconds: the CI whole-run cap is
  960 seconds, leaving insufficient room for a longer per-test budget.

## Notes for executing agent

The planning agent is not the execution agent. The alchemist completed the
read-only correlation steps for H1 and H2. Do not change the fixture, widen the
timeout, or intervene in compiler admission based on this partial timeline.
