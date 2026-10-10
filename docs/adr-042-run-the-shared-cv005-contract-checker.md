# Architectural decision record (ADR) 042: Run the shared CV-005 contract checker

## Status

Accepted.

## Date

2026-10-10

## Context and problem statement

netsuke keeps CodeScene coverage owned by `main`, the estate rule CV-005, and
holds it with a large local suite under `tests/workflow_contracts/`
(`codescene_*` and `coverage_*` modules). The estate now has one shared
implementation of the rule, `cv005-contracts` in `leynos/shared-actions`, and
every other repository runs it from a pinned commit. This repository did not,
so a rule fix reached it only as a hand edit, and it could drift from the rule.

## Decision outcome

`make test-workflow-contracts` runs `cv005-contracts check --repository .`
through `uv tool run` under the repository's tooling baseline, from the full
commit named by `CV005_CONTRACTS_REF` in the `Makefile`, before the local
pytest contracts. `.github/cv005.toml` holds the repository's parameters and one
`[[pairing]]` for the pull-request lane, which differs from the publisher's
generator in three environment keys that do not select what is measured:
`BUILD_JOBS`, `NETSUKE_RUST_TOOLCHAIN` and `PYTHON_BASELINE`. CI already runs
the target in its own step. `make all` builds the release binary only and is
not wired to it. `tests/workflow_contracts/cv005_wiring_test.py` holds the
local wiring: it fails if the pin is not a full commit, if the target stops
running the pinned checker with `check --repository .`, if the repository
parameter is wrong, or if CI stops running the target without condition (the
job may only skip scheduled runs).

The local contract is not deleted in this change. Retiring the modules the
library holds is a separate, reviewable step that lists each module against its
library clause, and it keeps the repository-specific report-delivery and lcov
handoff contracts, which the library does not hold.

## Known risks and limitations

- A fix to the rules reaches this repository only as a pin bump.
- The target needs `uv`, which fetches the Python the library runs under.
- Until the local modules are retired, a clause is checked twice.
