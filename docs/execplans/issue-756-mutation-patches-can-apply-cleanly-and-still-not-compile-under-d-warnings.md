# Add a compile gate to the Kani mutation evidence contract

Status: IN PROGRESS

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

## Purpose / Big Picture

Three tracked mutation patches under `docs/verification/mutations/` applied
cleanly and passed their contract test, but the tree they produced could not
compile. They therefore contributed zero mutation evidence while appearing
healthy.

**Mechanism.** `make kani-full` gained `RUSTFLAGS="… -D warnings"` on
2026-09-18 (commit `2c030fd1`, PR `#714`). Each of these patches seeded its
fault by leaving a binding or helper unused — a warning when written, a hard
compile error under `-D warnings`. The patched tree failed to build, so
`cargo kani` never reached the harness.

**Why the contract test missed it.**
`tests/kani_mutation_evidence_tests.rs::every_patch_applies_cleanly` only runs
`git apply --check`. That proves a patch *applies*; it does not prove the
patched tree *compiles*.

Outcome: the three patches seed the same faults without dead code, and a new
gated contract test compiles each patched tree under `-D warnings`, so this
failure mode cannot recur silently.

## Constraints

- 400-line cap (Whitaker `module_max_lines`) applies to every Rust module,
  including `tests/*.rs`. The existing test file is 383 lines, so the new test
  needs a split.
- `tests/makefile_test_target.rs` holds `NEXTEST_TARGETS` to exactly the targets
  invoking `nextest run`. A new nextest-invoking target must join both that
  list and the worker-bound contract.
- `tests/makefile_test_target/rustflags.rs` requires every `RUSTFLAGS="`
  assignment in the Makefile to be one of four contracted variables.
- `tests/workflow_contracts/nextest_child_cargo_group_test.py` auto-discovers
  build-capable child `cargo` tests and fails if any is missing from
  `.config/nextest.toml`'s `nested-cargo-builds` group.
- `tests/integration_test_wiring_tests.rs` requires each `tests/*/mod.rs` tree
  to be declared by a Cargo-discovered `tests/*.rs` target.

## Progress

- [x] (2026-09-21) Reconnaissance: file layout, contract tests, CI lanes.
- [x] (2026-09-21) Regenerate the three rotted patches; validate each by
      applying, compiling under `-D warnings`, running the owning harness to a
      named failure, reverting, and observing success.
- [x] (2026-09-21) Add the `#[ignore]`-gated compile contract test as
      `tests/kani_mutation_evidence_tests/compile_guard.rs`, declared from the
      parent by an explicit `#[path]` (the parent is near the 400-line cap).
      Liveness proven: on the pre-adoption tree it failed and named exactly the
      two `#755`-owned patches.
- [x] (2026-09-21) Adopt PR `#755`'s two repairs verbatim, so the gate is green
      here without waiting for an unmerged PR and the two branches cannot
      conflict whichever lands first.
- [x] (2026-09-21) Wire it into `.config/nextest.toml` (the
      `nested-cargo-builds` group), the Makefile (`test-kani-mutations`, with
      both worker bounds, plus `.PHONY` and `NEXTEST_TARGETS`), and CI
      (`build-test`).
- [x] (2026-09-21) Update `docs/developers-guide.md`.

## Surprises & Discoveries

- **A compile survey found five broken patches, not three.** Running
  `RUSTFLAGS="-D warnings" cargo check --lib --all-features` over all 18
  patches at `main` (`00f48f77`) showed `marker_token_match_is_exact` and
  `scanner_agrees_with_independent_specification` also fail — these are exactly
  the two PR `#755` repairs, and `#755` has not merged. `main` and `#755`'s base
  (`61a944fb`) have identical blob hashes for all five affected files, so the
  mechanism reproduces on a clean `main`. They are in scope for `#755`, not
  `#756`.
- The issue's suggested `_name` / `#[cfg(test)]`-visibility remedy is weaker
  than the in-place idiom the healthy patches already use: rebinding silences a
  warning without seeding a behavioural fault the harness can catch.
- The Python contract `nextest_child_cargo_group_test.py` does **not** classify
  the new test as build-capable, contrary to the prediction: `mask_non_code`
  retains only the literal `"cargo"` as a `CARGO_COMMAND` anchor, and the test
  locates Cargo with `env!("CARGO")`, whose string is masked. Registration in
  `.config/nextest.toml` is therefore not contract-forced — but it is still
  correct, because the test genuinely spawns nested Cargo builds.
- `test_execution_coverage_test.py`'s `LINUX_TEST_EXEMPTIONS` already exempts
  `kani-smoke`, but no exemption covers `build-test`. Its forbidden-suite
  regexes are `\bcargo nextest\b`, `\bcargo test\b`, and
  `\bmake test(?![\w-])` — the last excludes `-`, so `make test-kani-mutations`
  is deliberately not read as a second suite execution. Verified by running the
  regexes directly and then the whole contract suite (595 passed).
- `mdtablefix` without `--in-place` only *prints* the reformatted document; it
  never writes. A hash comparison "proved" idempotence while the check still
  failed. Only `make fmt` (which passes `--in-place`) applies it. `--check` and
  `--diff` are mutually exclusive, so the diff must be read from `--diff` alone.
- The gate's `Drop` guard rewrites `src/` files and restores them, which bumps
  their mtime and makes the editor report them as changed.
  `git diff --quiet -- src/` is the check that matters: the revert is exact.

## Decision Log

- Regenerate each patch by swapping an expression in place (a variant, a
  comparator's operands, an `Ordering` constant) rather than deleting a
  statement, so every helper stays referenced and every `mut` binding stays
  reassigned while the seeded fault is still behavioural.
- Keep the new test `#[ignore]`-gated: a `cargo check` per patch (~18 today) is
  too expensive for the default nextest profile.
- Adopt `#755`'s two repairs *verbatim* rather than repairing the same faults
  independently. Identical content means whichever PR merges second sees a
  no-op hunk instead of a conflict, and this branch's gate is green on its own.

## Outcomes & Retrospective

Task 1 delivered: the three named patches now seed the same faults without dead
code, each validated by apply → compile → harness failure → revert → harness
success. The survey widened the count from three to five, and the two extras
were adopted from `#755`.

Task 2 delivered: `compile_guard` closes the gap the issue describes. It is
gated (one `cargo check` per patch), registered in the `nested-cargo-builds`
group, reachable via `make test-kani-mutations`, run on every pull request from
`build-test`, and documented in the developer's guide, including the rule that
regeneration must swap an expression in place rather than delete a statement.
