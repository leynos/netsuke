# Debugging plan: restore Rustdoc coverage parsing

**Generated**: 2026-08-25 **Issue ID**: rebase follow-up for PR #577
**Severity**: high **Falsification sub-agent**: `alchemist` **Planning agent
boundary**: This document was prepared by the planning agent. Falsification
must be executed by the named sub-agent, not by the planning agent.

## Problem statement

`make doc-coverage` passes its Python unit tests but fails while measuring the
`test_support` library on the pinned nightly. Cargo exits successfully, yet the
coverage script receives empty standard output where it expects Rustdoc JSON.
The gate must parse the compiler's actual output channel without weakening the
coverage threshold or skipping the workspace member.

## Context summary

Table: Context and initial observations for the coverage failure.

| Aspect              | Details                                                      |
| ------------------- | ------------------------------------------------------------ |
| First observed      | 2026-08-25, after rebasing PR #577 onto `origin/main`        |
| Reproduction rate   | Deterministic through `make doc-coverage`                    |
| Affected components | `scripts/doc-coverage.py`, `test_support` Rustdoc invocation |
| Recent changes      | Pin moved to `nightly-2026-08-23`; `main` added doc coverage |

### Error artefacts

```text
error: cargo rustdoc for test_support lib (lib) did not emit coverage JSON:
Expecting value: line 1 column 1 (char 0)
```

### Information gaps

- Whether the nightly now writes the coverage JSON to standard error.
- Whether the output differs only for workspace member libraries.

______________________________________________________________________

## Hypotheses

### H1: Rustdoc moved coverage JSON to standard error

**Claim**: The pinned nightly returns a successful `cargo rustdoc` invocation
for `test_support`, but writes `--show-coverage --output-format json` output to
standard error rather than standard output.

**Plausibility**: Falsified — the exact command wrote a generated-file notice
to standard output and progress to standard error; neither stream held JSON.

**Prediction**: Running the exact target invocation while capturing both output
streams finds a non-empty, parseable JSON document on standard error and an
empty standard output stream.

#### H1 falsification plan

Table: Falsification steps for the standard-error output hypothesis.

| Step | Action                                                                                                                                                                                           | Expected Negative Result                                                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| 1    | Run the exact `cargo rustdoc` command built by `rustdoc_args` for `test_support --lib`, capture standard output and standard error separately, then inspect their byte counts and leading bytes. | Parseable coverage JSON appears on standard output, or neither stream contains it. |

**Tooling**: `cargo`, the pinned toolchain, `wc`, and `head`.

**Confidence on falsification**: Decisive for the output-channel hypothesis;
the command is exactly the one the gate constructs.

______________________________________________________________________

### H2: Rustdoc writes coverage JSON to its generated output file

**Claim**: Rustdoc writes the requested coverage JSON to the file named in its
successful standard-output notice, `target/doc/test_support.json`, rather than
to either captured stream.

**Plausibility**: High — H1's experiment reported exactly that generated path.

**Prediction**: The named file exists after the invocation and its contents
parse as the per-file coverage object that `aggregate_coverage_payload` accepts.

#### H2 falsification plan

Table: Falsification steps for the generated-file output hypothesis.

| Step | Action                                                                                                                    | Expected Negative Result                                                         |
| ---- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 1    | Read `target/doc/test_support.json` after the exact `test_support --lib` command and parse it with Python's JSON decoder. | The file is absent, invalid JSON, or has a shape the current aggregator rejects. |

**Tooling**: `cargo`, the pinned toolchain, and Python's standard `json` module.

**Confidence on falsification**: Decisive for the generated-file contract and
for whether the existing aggregator can consume it unchanged.

______________________________________________________________________

## Recommended execution order

1. **H1** — falsified: neither output stream contains JSON.
2. **H2** — verify the file Rustdoc announced before changing the collector.

## Termination criteria

- **Root cause identified**: H2 survives its falsification test.
- **Escalation trigger**: H2 is falsified; revise this plan before inspecting
  a third cause.

## Notes for executing agent

Do not edit tracked files or run the full repository gates. Return the exact
stream observed and a verdict of `falsified`, `not-falsified`, or
`inconclusive`.
