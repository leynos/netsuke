# Add a compile gate to the Kani mutation evidence contract

Status: IN PROGRESS

This ExecPlan is a living document. The sections `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to
date as work proceeds.

## Purpose / Big Picture

Three tracked mutation patches under `docs/verification/mutations/` applied
cleanly and passed their contract test, but the tree they produced could not
compile. They therefore contributed zero mutation evidence while appearing
healthy.

**Mechanism.** `make kani-full` gained `RUSTFLAGS="… -D warnings"` on
2026-09-18 (commit `2c030fd1`, PR #714). Each of these patches seeded its fault
by leaving a binding or helper unused — a warning when written, a hard compile
error under `-D warnings`. The patched tree failed to build, so `cargo kani`
never reached the harness.

**Why the contract test missed it.** `tests/kani_mutation_evidence_tests.rs::every_patch_applies_cleanly`
only runs `git apply --check`. That proves a patch *applies*; it does not prove
the patched tree *compiles*.

Outcome: the three patches seed the same faults without dead code, and a new
gated contract test compiles each patched tree under `-D warnings`, so this
failure mode cannot recur silently.

## Constraints

- 400-line cap (Whitaker `module_max_lines`) applies to every Rust module,
  including `tests/*.rs`. The existing test file is 383 lines, so the new test
  needs a split.
- `tests/makefile_test_target.rs` holds `NEXTEST_TARGETS` to exactly the targets
  invoking `nextest run`. A new nextest-invoking target must join both that list
  and the worker-bound contract.
- `tests/makefile_test_target/rustflags.rs` requires every `RUSTFLAGS="`
  assignment in the Makefile to be one of four contracted variables.
- `tests/workflow_contracts/nextest_child_cargo_group_test.py` auto-discovers
  build-capable child `cargo` tests and fails if any is missing from
  `.config/nextest.toml`'s `nested-cargo-builds` group.
- `tests/integration_test_wiring_tests.rs` requires each `tests/*/mod.rs` tree to
  be declared by a Cargo-discovered `tests/*.rs` target.

## Progress

- [x] (2026-09-21) Reconnaissance: file layout, contract tests, CI lanes.
- [x] (2026-09-21) Regenerate the three rotted patches; validate each by
      applying, compiling under `-D warnings`, running the owning harness to a
      named failure, reverting, and observing success.
- [ ] Add the `#[ignore]`-gated compile contract test.
- [ ] Wire it into `.config/nextest.toml`, the Makefile, and CI.
- [ ] Update `docs/developers-guide.md`.

## Surprises & Discoveries

- **A compile survey found five broken patches, not three.** Running
  `RUSTFLAGS="-D warnings" cargo check --lib --all-features` over all 18 patches
  at `main` (`00f48f77`) showed `marker_token_match_is_exact` and
  `scanner_agrees_with_independent_specification` also fail — these are exactly
  the two PR #755 repairs, and #755 has not merged. `main` and #755's base
  (`61a944fb`) have identical blob hashes for all five affected files, so the
  mechanism reproduces on a clean `main`. They are in scope for #755, not #756.
- The issue's suggested `_name` / `#[cfg(test)]`-visibility remedy is weaker than
  the in-place idiom the healthy patches already use: rebinding silences a
  warning without seeding a behavioural fault the harness can catch.

## Decision Log

- Regenerate each patch by swapping an expression in place (a variant, a
  comparator's operands, an `Ordering` constant) rather than deleting a
  statement, so every helper stays referenced and every `mut` binding stays
  reassigned while the seeded fault is still behavioural.
- Keep the new test `#[ignore]`-gated: a `cargo check` per patch (~18 today) is
  too expensive for the default nextest profile.

## Outcomes & Retrospective

(to be completed)
