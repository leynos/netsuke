# Architectural decision record (ADR) 027: Defer replacing the split-build-dir harness test

## Status

Accepted. The trim of `harness_compiles_under_a_split_build_dir` is deferred,
not closed: the Windows lane keeps the real `test_support` build and its 420s
budget, and the question reopens at the ten-run revisit gate below.

## Date

2026-09-17, with the measurements behind it taken through 2026-09-18.

## Context and problem statement

`harness_compiles_under_a_split_build_dir` is the regression test for the
Windows `CreateProcessW` command-line limit. It forces a split layout with its
own private `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` roots, confirms the
collected dependency directories span the split, and compiles a fixture against
them. Every `-L dependency=` pair it produces is required to avoid `E0463`, so
the list cannot be shortened; it moves off the command line entirely into a
`rustc` response file. That is the behaviour under test.

Because its roots are private — sharing the ambient target directory would race
the `#[once]` `test_support_rlib` fixture and fail with version-skew errors
(`E0460`) — the test compiles `test_support` and roughly 350 dependencies from
scratch. On the four-vCPU GitHub-hosted `windows-latest` gate, that build is
98.8% of the test's wall time, which makes it the most expensive test in the
lane and an obvious target for trimming: build a minimal fixture crate under
the split layout instead of the real `test_support`.

This record exists because that obvious target was measured, and the
measurement did not support spending the fidelity risk when the ticket that
proposed it assumed.

The figure has moved twice, in both cases because the system around the test
changed rather than the test itself.

Before [#687](https://github.com/leynos/netsuke/pull/687), the two
isolated-Cargo tests ran concurrently, each with four compile jobs on a
four-vCPU runner, so each roughly halved the other. In that contended shape
trimming **both** tests was worth 156s. Once
`packaged_manifest_retains_build_script_sources` left the Windows lane, this
test got faster without being touched, and the implied value of trimming it
fell to about **85s** — measured as its exclusive tail, the period after every
other test had reported.

A serialization group then changed the arithmetic a second time. A later change
to `.config/nextest.toml` added `nested-cargo-builds`, a `[test-groups]` entry
with `max-threads = 1`, which puts this test in a group with the other tests
that spawn a build-capable child Cargo command. It landed for the coverage
lane's benefit: four nextest workers each starting a four-job child Cargo build
on a four-vCPU runner is what the group exists to prevent. The Windows lane runs
`make test` with no `NEXTEST_PROFILE`, so it selects `[profile.default]` and
inherits the same group.

Under that group the test is no longer merely a slow finisher. It is a **serial
link**: every other member waits while it holds the single slot, and the chain
cannot finish until it releases it. Removing it therefore returns its whole
group occupancy rather than its exclusive tail.

The question this record answers is therefore not whether the test is expensive
— it is — but whether the fidelity risk of replacing it is currently worth
paying, given that the number has never been stable enough to plan against.

## Decision drivers

- The test guards a Windows-specific failure
  (`Os { code: 206, kind: InvalidFilename }`) that cannot be reproduced on most
  local hosts, so weakening it silently is the most expensive possible outcome.
- The figure the decision rests on has moved twice, both times because the
  lane changed rather than the test. A decision taken against an unstable
  number reopens on its own.
- The repository has already spent effort making this lane cheaper by other
  means. The lane fell from a 1468s median to about 848s across
  [#687](https://github.com/leynos/netsuke/pull/687), [#690](https://github.com/leynos/netsuke/pull/690)
  and [#691](https://github.com/leynos/netsuke/issues/691), so the remaining
  saving is roughly ten percent of what is left.
- Any replacement owes coverage that is not the group's rationale to supply.
  The response-file pressure this build generates is a separate requirement
  from the serialization the group provides.

## Options considered

### Option A: Replace the test with a minimal fixture crate

Build a one-dependency crate under the split layout in place of `test_support`,
keeping the private roots and the split-directory assertion but dropping the
roughly 350-dependency compile.

This returns most of the figure, because the test is the group's heaviest
member: removing it shortens the chain for every member behind it. It also
drops coverage in two ways, neither of which is optional. A one-dependency
fixture still exercises the split-directory derivation, but no longer covers it
at the real crate's scale, nor against the proc-macro and dynamic-library
artefacts that make the directory enumeration non-trivial. And it produces far
fewer `-L dependency=` entries, so it stops exercising the response-file path
that the test exists to protect.

### Option B: `cargo check` instead of `cargo build`

Timed cold at `-j 4` on a 32-core host: 114s against 102s, twelve percent. It
also writes nothing into the target directory, so the uplift the regression
exists to catch stops happening and the test passes vacuously.

### Option C: Warm the lane's compiler cache

The cache already reaches the spawned build, because `ci-windows.yml` sets
`RUSTC_WRAPPER` at job scope and the test adds to the child environment rather
than clearing it. Warming it is worth about three percent: 281.0s on the cold
run against a 271.9s warm median. There is no reuse left to claim.

### Option D: Share a target directory

The test needs private roots to avoid racing the `#[once]` fixture with
`E0460`, so its build cannot reuse the lane's artefacts or the other test's.

### Option E: Keep the test and defer the decision to a gate

Leave the test, its subject and its budget as they are, and record the
conditions under which the question is worth reopening.

| Topic                     | A: fixture crate | B: `cargo check` | C: cache | D: shared target | E: defer       |
| ------------------------- | ---------------- | ---------------- | -------- | ---------------- | -------------- |
| Returns the figure        | Most of it       | ~12%, then none  | ~3%      | None             | No             |
| Keeps split-dir coverage  | At reduced scale | Vacuously        | Yes      | Yes              | Yes            |
| Keeps response-file path  | No               | No               | Yes      | Yes              | Yes            |
| Reproduction on this host | Yes              | Yes              | Yes      | Fails `E0460`    | Not applicable |

_Table 1: Comparison of the measured options._

## Decision outcome

**Option E.** The repository does not trim
`harness_compiles_under_a_split_build_dir` yet. The test keeps its real
`test_support` subject and its 420s budget, and the trimming question reopens
at the revisit gate below rather than on a schedule.

## Rationale

What a trim returns is the test's whole group **occupancy**, because the
duration _is_ the occupancy it gives back. That gives the figure both its
ceiling and its shape: **a trim can never return more than the test's own
duration**, and it returns exactly that whenever the shortened group chain is
still what bounds the run. It returns less only when unrelated non-group work
becomes the run's next binding constraint once the harness is gone.

The harness is never the last test to finish. The group's cheap tail members
cannot start until it frees the slot, so they necessarily finish after it. The
mechanism is what generalizes past the sample; the sample is its evidence.
Measured from the same `build-test-windows` job logs, over the Windows runs
available on 2026-09-18:

| Run         | Test duration | Group chain end, trim applied | Trim returns |
| ----------- | ------------- | ----------------------------- | ------------ |
| 35266003414 | 152.0s        | 162.4s                        | 152.0s       |
| 35266979317 | 149.0s        | 178.3s                        | 149.0s       |
| 35272793454 | 124.7s        | 134.9s                        | 113.4s       |
| 35400200137 | 115.7s        | 154.1s                        | 95.0s        |
| 35403273264 | 141.6s        | 154.4s                        | 136.3s       |
| 35405043577 | 141.9s        | 167.1s                        | 141.9s       |
| 35407132087 | 162.9s        | 174.6s                        | 162.9s       |

_Table 2: The harness test as a serialized group member, after the group
landed._

The rightmost column is the whole-run saving: the run's own end, less whichever
of the trimmed group chain and the last non-group test finishes later. Four of
the seven runs return the test's full duration; the other three return less, at
113s against 125s, 95s against 116s, and 136s against 142s. So the figure
tracks the test's own cost and moves with it, which is why the sample's
durations span 115.7s to 162.9s while its savings span 95s to 163s.

The 85s reading is not a floor this settles back to. It was the exclusive tail
under an uncontended lane that no longer exists, and the group's arrival is
what retired it.

Two cautions belong with that table, and they are why the decision is to defer
rather than to proceed on a larger number. The sample is small and it is not a
uniform one: it mixes trunk pushes with pull-request lanes, which start from
different tree states, so it sets an order of magnitude rather than a value.
And the group's own scheduling, not the test alone, produces the chain ends, so
those figures are readings of a serialized system rather than isolated
measurements of the test.

The strongest argument for deferring is not the size of the number. It is that
the number has moved twice for reasons outside the test, so a decision taken
against it would be a decision taken against the lane's current shape. The
fidelity risk, by contrast, is real and one-directional: the coverage a fixture
crate would drop is exactly the coverage that fails only on Windows, where it
is least likely to be noticed.

## Revisit gate

Wait until **ten runs** of the split Windows lane exist under the serialization
group. That is enough for the harness test's share of the `build-test-windows`
job to be known under the shape that now exists, rather than estimated from the
sample above.

The criterion has moved with the evidence. With the group in place the test
holds a serial slot, so the question is no longer whether its exclusive tail
has settled below 85s — it plainly has not. The question is whether the run
still ends when the test ends. If the group chain stops being what bounds the
run, or if the `Test` step stops being the lane's critical path, then the trim
is not worth the fidelity risk and the work closes without it.

## Consequences

- The Windows lane keeps its most expensive test, and the group keeps its
  heaviest member. Every other member of `nested-cargo-builds` waits behind
  that member, so the serialization the group provides costs the lane more than
  the test's own duration.
- A future trim is not blocked, only deferred. Option A remains available and
  its requirements are recorded in the developer's guide, which owns them: the
  fidelity argument a replacement owes in a doc comment beside the test, and the
  `rustc` response-file pressure it must either keep generating or move into a
  dedicated test.
- The 420s budget stays sized against the older, contended distribution. It is
  conservative by roughly a third against the measured 312.9s worst case, and
  it remains a candidate for tightening or deletion once the gate is met.
- Any future change that removes the test from the lane also removes the
  group's heaviest member, which changes every other member's scheduling. That
  is a lane-wide effect, not a local one, and belongs in the decision that
  takes it.
- Because the estimate is recorded as a mechanism with a dated sample rather
  than a range, later runs do not falsify it. They belong to the revisit gate.

## Related decisions

- [ADR-011: Serial `deps` ordering via Ninja dyndep][adr-011] is the other
  decision in this repository that trades throughput for ordering guarantees.
- [ADR-025: Persistent coverage data owned by `main`][adr-025] governs the
  coverage lane whose contention the `nested-cargo-builds` group was added to
  prevent.

## References

- [#673](https://github.com/leynos/netsuke/issues/673) measured the Windows
  lane.
- [#687](https://github.com/leynos/netsuke/pull/687) relocated the packaging
  verification build and so changed this test's cost.
- [#690](https://github.com/leynos/netsuke/pull/690) folded the native-recipe
  smoke job into the gate job.
- [#691](https://github.com/leynos/netsuke/issues/691) split the Windows lints
  from the tests.
- [Developer guide: Windows budget for the isolated-Cargo-build tests and what
  a fixture-crate replacement would have to preserve][dev-guide].

[adr-011]: adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md
[adr-025]: adr-025-main-owned-coverage-publication.md
[dev-guide]: developers-guide.md#what-a-fixture-crate-replacement-would-have-to-preserve
