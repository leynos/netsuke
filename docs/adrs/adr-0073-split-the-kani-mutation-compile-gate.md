# Architectural decision record (ADR) 0073: Split the Kani mutation compile gate

## Status

Accepted on 2026-11-06. The Kani mutation compile gate runs as a prewarm, a
negative-compile check, and a patch sweep, in two Nextest invocations under a
Nextest profile dedicated to it: 900 s for the prewarm and the negative check,
and 2,400 s for the sweep. Amends
[ADR-0059](adr-0059-keep-compile-time-contracts-out-of-timed-tests.md).

## Date

2026-11-06

## Context and Problem Statement

[ADR-0059](adr-0059-keep-compile-time-contracts-out-of-timed-tests.md) left one
build-capable test in the `nested-cargo-builds` group: the `#[ignore]`-gated
mutation compile gate, which applied every patch under
`docs/verification/mutations/`, compiled the patched tree through the Kani
frontend, and reverted it. It carried a 600 s allowance, the largest in
`.config/nextest.toml`.

That single invocation paid two costs in one budget. The first patch compiled
against a cold target directory, so the dependency build was charged to the
same test that then swept the patches. The measured spread shows the budget was
not the cost: the gate passed at 257.7 s and 261.5 s, failed at 300.008 s on a
head whose only diff was eight lines of Markdown, passed at 480.264 s on one
base, and reached its 600.019 s cap on the rebased base (CI runs 41782146443
and 41826968891). A margin of 38 s on a 300 s cap is the difference between one
runner and another, not a budget.

The gate also had no negative control. It asserted that every patch compiles,
and that assertion is only meaningful while the same compile path can fail. The
gate compiles with `cargo kani --no-codegen -Z unstable-options` rather than
`cargo check`, because a fault behind `#[cfg(kani)]` is invisible to
`cargo check`; if `--no-codegen` ever stopped type-checking, every patch would
look healthy and the gate would prove nothing.

## Decision Drivers

- The cold dependency build must not consume the patch sweep's allowance.
- The sweep's "every patch compiles" result must not be vacuous.
- The whole gate must stay inside the `kani-smoke` job's `timeout-minutes`,
  which has to contain both whole-run caps rather than one.
- No test the default suite runs may start a Cargo build (ADR-0059).

## Options Considered

### Option A: Widen the single invocation's allowance again

Raising the one test's allowance to 900 s would have covered the observed 600
s, but it leaves the cold build and the sweep sharing one budget, so the next
runner that is slower at either cost fails the same way. It also leaves the
gate without a negative control.

### Option B: Split the invocation and add a negative check

Run the cold build as its own `#[ignore]`-gated prewarm, then the patch sweep
in a second Nextest invocation against the same sandbox path and target
directory. Add a third test, in the prewarm's invocation, that compiles the
unmodified sandbox and then two deliberately broken trees, one fault visible to
`cargo check` and one behind `#[cfg(kani)]`, and requires the compile path to
reject each.

### Option C: Move the gate out of Nextest entirely

A Makefile target that shells out to `cargo kani` per patch would remove the
per-test allowance question, but it would also remove the test harness that
discovers patches, checks they apply, and reverts them fallibly. The gate's
value is that it is a test with assertions, not a shell loop.

## Decision Outcome

Option B. `make test-kani-mutations` runs two Nextest invocations:

1. `prewarm::prewarm_kani_mutation_target` and
   `negative::compile_kani_tree_rejects_trees_that_do_not_compile`, which share
   the cold build and the negative control.
2. `compile_guard::every_patched_tree_compiles_under_denied_warnings`, the
   patch sweep, which now starts against a warm target.

The prewarm and the negative check carry 900 s each and the sweep 2,400 s, all
of them under `[profile.kani]` in `.config/nextest.toml`, and all three hold the
`nested-cargo-builds` group. The negative check seeds its faults into the
sandbox library root, so the `#[cfg(kani)]` fault is parsed by the Kani
frontend and the ordinary fault by any frontend; each rejection must name the
seeded marker, so a rejection for an unrelated reason fails the test. The
unmodified tree is compiled first, so a rejection cannot be explained by a
sandbox that never built.

`make test-kani-mutations` selects the profile explicitly with `--profile kani`,
so these invocations receive its 2,520 s whole-run cap, which sits above every
allowance they carry, and each of the two invocations is bounded by that cap
separately. The job's `timeout-minutes` therefore has to contain two caps, and
the 2026-11-05 reassessment in ADR-0059 records the arithmetic. The coverage
lanes keep their 960 s cap and their 300 s per-test allowance, so no Kani
allowance reaches a lane that does not select the profile.

## Known Risks and Limitations

- The negative check compiles three trees, so it adds two compiles to the
  prewarm's invocation. They reuse the warm target, but a cold runner pays for
  them inside the same 900 s allowance as the prewarm.
- The seeded faults are appended to `src/lib.rs`, so a future change that makes
  the library root a thin re-export module would need the faults moved to
  whichever module the Kani frontend actually parses.
- The gate still cannot prove that a patch's fault is the one its harness
  detects; it proves only that the patched tree compiles. That is the
  pre-existing boundary, unchanged here.

## References

- [ADR-0059](adr-0059-keep-compile-time-contracts-out-of-timed-tests.md) placed
  compile-time contracts in the build graph and left this gate as the group's
  only member.
- [`Makefile`](../../Makefile) defines `test-kani-mutations`.
- [`.config/nextest.toml`](../../.config/nextest.toml) carries the three
  allowances.
- [`tests/kani_mutation_evidence_tests/`](../../tests/kani_mutation_evidence_tests)
  holds the prewarm, negative check, and sweep.
- [`docs/developers-guide.md`](../developers-guide.md), section "Test timeouts:
  the tiers this repository sets", states the arithmetic.
