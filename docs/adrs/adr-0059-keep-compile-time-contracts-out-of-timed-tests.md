# Architectural decision record (ADR) 0059: Keep compile-time contracts out of timed tests

## Status

Accepted. Positive compile-time contracts compile as workspace libraries in
Cargo's build phase, negative ones are pinned `compile_fail` doctests, and the
direct-`rustc` harness that used to drive both from inside Nextest tests is
retired. Supersedes [ADR-0033](adr-0033-record-split-build-cargo-messages.md).

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

| Contract kind         | Option A                       | Option B                         | Option C                       |
| --------------------- | ------------------------------ | -------------------------------- | ------------------------------ |
| Public API compiles   | Nested Cargo in a timed test   | Shared nested Cargo, still timed | Workspace library, build phase |
| Item must not compile | Direct `rustc` in a timed test | Direct `rustc` in a timed test   | Pinned `compile_fail` doctest  |
| Harness self-tests    | Kept, with 900 s allowance     | Kept                             | Retired with the harness       |

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
- **Nextest.** The ten public-API tests, the support-crate tests, and the two
  harness tests leave `nested-cargo-builds`, along with the two 900 s
  allowances they carried.

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
- `public_api_fixture` compiles on every workspace build, including a local
  `make build`. The cost is small and bounded by the same compiler cache as the
  rest of the workspace.
- The `cfg(kani)` and Ninja escaping UI tests still invoke `rustc` directly.
  Each compiles one standard-library-only source and resolves no Cargo
  artefact, so neither waits on Cargo's lock or rebuilds a dependency, and
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
- [`docs/developers-guide.md`](../developers-guide.md), section "Compile-time
  contracts live in the build graph", states the practice for contributors.
