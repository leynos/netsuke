# Architectural decision record (ADR) 0059: Keep compile-time contracts out of timed tests

## Status

Accepted. Positive compile-time contracts compile as workspace libraries in
Cargo's build phase, negative ones are pinned `compile_fail` doctests, and the
direct-`rustc` harness that used to drive both from inside Nextest tests is
retired. Cargo's package verification build runs as `make test-package-verify`
on the Linux gate rather than inside a test. Supersedes
[ADR-0033](adr-0033-record-split-build-cargo-messages.md).

Amended by [ADR-0073](adr-0073-split-the-kani-mutation-compile-gate.md)
(2026-11-06): the mutation compile gate named below is now a prewarm, a
negative-compile check, and a patch sweep, with 900 s for the prewarm and the
negative check and 2,400 s for the sweep.

## Date

2026-10-23.

## Context and problem statement

[ADR-0051](adr-0051-compile-the-build-script-module-slice-in-the-workspace-graph.md)
moved one compile-time contract, the build-script module slice, out of a timed
test after that test reached its 900 s allowance while its nested compiler work
waited behind healthy shared admission. The same shape remained in three more
families of tests, all members of the serialized `nested-cargo-builds` group:

- `tests/command_env_ui_tests.rs` type-checked ten embedder-facing API fixtures
  against the `netsuke` library. Each test ran
  `cargo check --package netsuke-build --lib --all-features`, read the
  library's metadata path and every dependency directory out of Cargo's JSON
  stream, and invoked `rustc` on a fixture with `--extern netsuke=…`.
- `tests/locale_stub_ui_tests.rs` and `tests/ninja_semantics_ui_tests.rs` did
  the same against `test_support`, proving that `StubEnv::default()` and two
  swaps between generated-Ninja text domains do not compile. The stub's
  compile-fail case carried its own 900 s allowance.
- Two tests in `tests/locale_stub_ui_tests.rs` existed to protect the harness
  itself: a parser regression over recorded split-layout Cargo JSON, and an
  isolated two-crate workspace built under private split roots and compiled
  through the harness's `rustc` response file. The latter also carried a 900 s
  allowance.
- `tests/packaging_smoke_tests.rs` ran `cargo publish --dry-run`, which
  compiles the extracted package with its default features, before asserting
  the packaged file list. That build reached 600 s and then 720 s caps under
  saturated admission, and carried a 900 s allowance of its own.

Each of those tests charged Cargo's package-cache lock, any dependency rebuild
its check mode or feature selection triggered, and every compiler-admission
wait to its own time allowance. The allowance therefore measured how busy the
host was rather than how slow the test was, and widening it changed which run
failed, not whether one would.

## Decision drivers

- A test's allowance must bound the test's own work, not the host's queue.
- Every contract the old tests proved must still be proved, with a control
  that makes each rejection meaningful.
- A regression should fail at the earliest and most accurate point; for an API
  visibility change, that is the build.
- The guarantee must be held by a contract, so that a later test cannot quietly
  reintroduce the shape.

## Options considered

### Option A: Widen the remaining allowances again

Rejected. Two earlier raises for the module slice did not change the outcome,
and the measured spread for the same nested check on this host runs from 8 s to
709 s. No allowance below the 960 s CI whole-run cap covers that reliably.

### Option B: Share one warmed artefact set across the harnesses

Rejected. A `#[once]` fixture already shared preparation within a test binary.
The wait it pays is for the shared compiler pool and Cargo's lock, which no
amount of sharing inside the test suite can remove.

### Option C: Move positive checks into the build graph and negative checks into doctests

Accepted. Cargo compiles a workspace library in its ordinary build phase under
its own jobserver, and rustdoc compiles `compile_fail` examples in the doctest
pass, which Nextest does not schedule. Neither placement has a per-test
allowance for a queue to exhaust.

_Table 1: Where each kind of compile-time contract runs under each option._

| Contract kind              | Option A                       | Option B                         | Option C                          |
| -------------------------- | ------------------------------ | -------------------------------- | --------------------------------- |
| Public API compiles        | Nested Cargo in a timed test   | Shared nested Cargo, still timed | Workspace library, build phase    |
| Item must not compile      | Direct `rustc` in a timed test | Direct `rustc` in a timed test   | Pinned `compile_fail` doctest     |
| Harness self-tests         | Kept, with 900 s allowance     | Kept                             | Retired with the harness          |
| Package verification build | Nested Cargo in a timed test   | Nested Cargo in a timed test     | Makefile target on the Linux gate |

## Decision outcome

- **`public_api_fixture/`.** A non-published workspace library whose only
  dependency is `netsuke-build`, with one module per embedder-facing boundary:
  the explicit Ninja environment API, the manifest environment loaders,
  Clap-independent policy parsing, the cached configuration flow, the cached
  discovery observer API, the command-list AST, the IR-generation error, the
  legacy-recipe metric names, the verbose timing reporter, and, behind a
  forwarded `lint` feature, the lint API. Depending on `netsuke` alone gives
  each module exactly an external crate's view; in particular, Clap is not
  nameable, which is what proves policy parsing needs no Clap import. The
  library has no tests or doctests, because nothing in it runs.
- **`compile_fail` doctests on the owning items.** `StubEnv` carries a
  `compile_fail,E0599` example for `default()`, and `GeneratedNinja::new` and
  `RecipeNeedle::new` each carry a `compile_fail,E0308` example rejecting the
  other text domain. Each sits beside a positive example in the same crate,
  which proves the doctest environment links `test_support` and so keeps the
  rejection from passing for the wrong reason. The error codes are pinned for
  the same reason.
- **The harness is retired, not kept idle.** The Cargo-JSON artefact parser, the
  `rustc` response-file writer, the shared library selection arguments, the
  `test_support` preparation helper, the split-build regression tests, and the
  recorded Cargo messages are deleted. Each existed only to serve the harness.
  The isolated split-build test also checked that `build.rs` watches no
  Cargo-supplied metadata; a source test,
  `build_script_watches_only_its_documented_environment_input`, now holds that
  directly by reading `build.rs`.
- **Contract.** `tests/workflow_contracts/compile_contract_placement_test.py`
  refuses `"--extern"` and `--message-format=json` anywhere under `tests/`,
  holds `public_api_fixture` to its single dependency, its forwarded feature,
  and its module list, and requires the support crate's compile-fail examples
  to stay in place.
- **Package verification is a Makefile target.**
  `make test-package-verify` runs
  `cargo publish --dry-run --allow-dirty -p netsuke-build`, and the Linux gate's
  `Package verification` step runs it. The packaging test keeps only
  `cargo package --list`, which compiles nothing, so the file-list assertions
  still run on every platform. The target is not part of `make test`: the
  Windows lane runs `make test`, and moving the verification off that lane is
  what an earlier change measured at a 240.8 s median saving, since packaged
  content does not vary by platform.
- **Nextest.** The ten public-API tests, the support-crate tests, the two
  harness tests, and the packaging test leave `nested-cargo-builds`, along with
  the three 900 s allowances they carried. With no build left in the default
  suite, the generated-completion readers leave the group too, and the
  `#[ignore]`-gated Kani prewarm and mutation compile sweep are its only
  members; they run as separate Nextest invocations so the cold dependency
  build does not consume the sweep's budget. The prewarm carries 900 s and the
  sweep 2,400 s, in the `kani` profile that runs this gate and nothing else.
- **Discovery.** The child-Cargo discovery used by the workflow contracts no
  longer treats `cargo package --list` as build-capable, and a contract now
  requires that no test outside the mutation compile gate starts a
  build-capable Cargo command at all.

## Reassessment (2026-11-04)

The terminal-rendering branch adds the display-plan module to Kani's proof
scope, increasing the mutation patch set to 30 trees. Hosted run 41840367244
and a local run of the same candidate both reached the previous 600-second
nextest limit while compiling the first sorted patch, without a compiler
diagnostic. The preserved local sandbox confirms that this was the first patch,
not a later patch accumulating slow work.

## Reassessment (2026-11-05)

The targeted 900-second allowance also timed out locally at 900.093 seconds
without a compiler diagnostic. A separate one-patch cold compile completed in
1,864 seconds while shared compiler admission was heavily queued; this is
evidence for a longer allowance but not a stable runtime estimate. Keep the
300-second default and coverage limits unchanged. Give the mutation test 2,400
seconds in a dedicated Nextest profile, with a 2,520-second whole-run cap. The
latter contains the test allowance, Nextest's 70-second termination window, and
a 50-second margin. Hosted run 41840367244 spent 824 seconds before the test
and 7 seconds finalizing. A 60-minute Kani job contains that setup, the full
whole-run allowance, and finalization in 3,351 seconds, with 249 seconds
remaining. The workflow contracts and a successful Kani run must confirm this
budget before it is treated as validated.

## Reassessment (2026-11-09)

The 2026-11-05 addendum is the record that the mutation test's allowance is no
longer 600 seconds. The _Decision outcome_ section still states 600 seconds as
the largest per-test allowance, because that was true of the decision as
accepted, and the terminal-rendering branch briefly edited it in place before
restoring it here. Accepted decision text is a historical record: a later
measurement belongs in an addendum beside it, where a reader can see both what
was decided and what has since changed, and can tell which one a given sentence
is describing. The dedicated Kani profile in `.config/nextest.toml` gives the
same test 2,400 seconds under a 42-minute whole-run cap; that figure now lives
in the 2026-11-05 addendum alone, so these two sections no longer give
competing values for the same limit.

## Known risks and limitations

- A `compile_fail` doctest asserts an error code, not the diagnostic text the
  old tests also matched, such as the name of the rejected domain. An unrelated
  error with the same code in the same three-line example would satisfy it. The
  examples are deliberately minimal so that only the intended error is
  reachable.
- Each compile-fail example is still a separate compiler invocation, and under
  shared admission it waits like any other. The doctest pass carries no
  per-test allowance locally and runs inside its own cargo watchdog in CI, so a
  queue makes it slower rather than failing it. Four such examples now exist
  across the workspace.
- `make test` no longer runs the package verification build, so a local run
  of the full test suite does not prove the packaged crate compiles. The Linux
  gate does, and AGENTS.md names the inputs after which a contributor should run
  `make test-package-verify` locally.
- `public_api_fixture` compiles on every workspace build, including a local
  `make build`. The cost is small and bounded by the same compiler cache as the
  rest of the workspace.
- The `cfg(kani)` and Ninja escaping UI tests still invoke `rustc` directly.
  Each compiles one standard-library-only source and resolves no Cargo
  artefact, so neither waits on Cargo's lock nor rebuilds a dependency, and
  neither subject is reachable from a public item that could carry a doctest.

## Architectural rationale

Compilation is the build's job. Placing a compile-time contract where Cargo
already schedules compilation makes its cost visible as build time and removes
it from the one place, a timed test body, where it was charged against the
wrong budget. The remaining tests are either queries over source text or
ordinary runtime tests, which are what a per-test allowance can meaningfully
bound.

## References

- [ADR-0051](adr-0051-compile-the-build-script-module-slice-in-the-workspace-graph.md)
  applied this placement to the build-script module slice.
- [ADR-0033](adr-0033-record-split-build-cargo-messages.md) recorded the
  split-build harness tests this decision retires.
- [`public_api_fixture/src/lib.rs`](../../public_api_fixture/src/lib.rs) lists
  the type-checked boundaries.
- [`tests/workflow_contracts/compile_contract_placement_test.py`](../../tests/workflow_contracts/compile_contract_placement_test.py)
  holds the placement.
- [`tests/workflow_contracts/nextest_child_cargo_group_test.py`](../../tests/workflow_contracts/nextest_child_cargo_group_test.py)
  refuses a build-capable child Cargo command in any default-suite test.
- [`Makefile`](../../Makefile) defines `test-package-verify`, and
  [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) runs it.
- [`docs/developers-guide.md`](../developers-guide.md), section "Compile-time
  contracts live in the build graph", states the practice for contributors.

## Amendments

### Amendment 1: the mutation compile gate is split (2026-11-06)

[ADR-0073](adr-0073-split-the-kani-mutation-compile-gate.md) amends the
decision outcome above. The text is preserved as written, including its
statement that the `#[ignore]`-gated mutation compile gate is the group's only
member with a 600 s allowance because that is what this ADR decided at the time.

The gate now runs as three `#[ignore]`-gated tests in two Nextest invocations:
a prewarm that compiles the unpatched sandbox, a negative check that requires
the same compile path to reject seeded faults, and the patch sweep. The prewarm
and the negative check carry 900 s each and share one invocation, so the cold
dependency build is paid once; the sweep then runs against that warm target
with the 2,400 s allowance the 2026-11-05 reassessment records. The reason is
measured rather than theoretical: the single-invocation gate passed at 257.7 s
and 261.5 s, failed at 300.008 s, passed at 480.264 s on one base, and reached
its 600 s cap on the rebased base (CI runs 41782146443 and 41826968891). The
placement decision this ADR records is unchanged; only the gate's internal
shape and its allowance moved.
