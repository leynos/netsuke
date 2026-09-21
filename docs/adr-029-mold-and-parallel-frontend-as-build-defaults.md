# Architecture decision record (ADR): Make `mold` and the parallel frontend the default build

## Status

Accepted.

## Date

2026-09-19

## Context and problem statement

Netsuke's development loop spent most of its wall-clock time in two places that
are not the code: linking, and `rustc`'s single-threaded front-end. Both have
supported, low-risk accelerations for years, and both were reachable only by
opting in — an environment variable exported by whoever remembered, or a local
edit to a file that is git-ignored. The result was that a contributor's build
speed depended on whether they had read a particular section of the developers'
guide, and that continuous integration and local runs compiled under different
flags without either side recording that they did.

Two candidate accelerations are available to the whole workspace:

- the `mold` linker (Linux only), which replaces GNU `ld` for the final link;
- the parallel `rustc` front-end, `-Zthreads=N`, which on nightly parallelizes
  front-end work that has been serial since the compiler was written.

A third looks obvious and is not: the Cranelift codegen backend, which promises
a much larger win because it replaces LLVM entirely, and which is unusable here
for a reason that a shallow probe does not reveal.

Choosing a default also means choosing what the default must _not_ reach. Two
build shapes are not development builds: a release or packaging build, whose
output is shipped, and a coverage build, whose output is a measurement. An
acceleration reaching either would change what is shipped or compared.

Finally, the mechanism matters as much as the flags. A default that lives in a
target, or in an exported variable, is not a default: it is an opt-in that some
paths take and others miss, and the misses are silent.

## Decision

Commit the build standard to `.cargo/config.toml`, the file Cargo
auto-discovers, so that every build in the repository takes it — including a
bare `cargo build`, and including builds that no Make target drives:

- `-Zthreads=8` in `[build] rustflags`, taking effect on every platform;
- the same flag plus `-Clink-arg=-fuse-ld=mold` in a
  `cfg(target_os = "linux")` table. `mold` ships for Linux only, so macOS and
  Windows fall through to the `[build]` table and use their platform linker.

The flags are restated in the Makefile as `STANDARD_THREADS_FLAG` and
`STANDARD_MOLD_FLAG` and composed into the `RUSTFLAGS` each gate builds. Cargo
selects a single `rustflags` source rather than merging them, so an assigned
`RUSTFLAGS` — which every gate sets, to deny warnings — replaces every table in
the configuration file. Restating is therefore required rather than redundant,
and contract tests hold the two sources equal in both directions.

**Exclusions.** Release and coverage builds are held out by assigning
`RUSTFLAGS` at the point they run, which is the same mechanism that makes the
restatement necessary. Release assigns it because the artefact ships; coverage
assigns it at the step, not on the job, so the exclusion is visible where it
applies and a contract has something to read. Continuous integration's release
lanes are already covered because the toolchain action exports `RUSTFLAGS` for
the whole job.

**Cranelift is refused.** No `codegen-backend` key may appear in the
configuration, under any profile, and a contract refuses one along three
routes: a direct profile key, a key beneath a profile's `package` table, and
`-Zcodegen-backend=` inside rustflags.

**The toolchain and the tools are separate.** `rust-toolchain.toml` supplies
the compiler and is provisioned by `rustup` automatically. `mold` is a binary
the contributor installs: `make install-build-tools` fetches the pinned release
and verifies it against `tools/mold/SHA256SUMS`, and `make check-build-tools`
verifies it is present and is the pinned version. Every gate target depends on
the check, so a missing prerequisite reports an installation hint rather than
surfacing later as a linker error.

## Rationale

**Why the configuration file, not a target.** A Make target can only accelerate
the builds that go through it. `cargo build`, `cargo test`, an editor's
`cargo check`, and rust-analyzer all bypass the Makefile, and a contributor who
uses them would get the slow path while the gates got the fast one — with
nothing to indicate the difference. Auto-discovery is what makes this a default
rather than an opt-in, and it is why the file must be treated as applying to
everything, including the shapes that must be excluded.

**Why the flags are restated.** Documented under _Decision_. This is the single
most surprising property of the mechanism: the composition that makes the gates
work is what would silently disable the standard if the flags were not
repeated. Keeping the two sources equal is therefore a testable contract rather
than a convention, and the guide asks explicitly that they not be consolidated.

**Why Cranelift is excluded, stated narrowly.** A Cranelift-compiled panic does
not find the unwind handler it should. Measured on `nightly-2026-08-23`
(`librustc_codegen_cranelift-1.100.0-nightly.so`) in a crate with no
dependencies, with `[profile.dev] codegen-backend = "cranelift"` and the
standard's flags:

| Case                      | Cranelift                  | LLVM control |
| ------------------------- | -------------------------- | ------------ |
| `#[should_panic]`         | passes                     | passes       |
| `catch_unwind`            | does not catch; test fails | passes       |
| Panic on a spawned thread | aborts the process         | passes       |

_Table 1: Panic behaviour under the Cranelift backend and under LLVM._

The `#[should_panic]` row is why the refusal has to be justified on this
specific evidence rather than on a general claim that "panics do not unwind".
That case passes, because libtest's outermost handler is the panic's own
handler and nothing between them has to work. `catch_unwind` sits in between
and the unwinder walks past it; a spawned thread has no handler above it, so
the unwinder reaches the end of the stack and the process leaves on SIGABRT with
`failed to initiate panic, error 5` — error 5 being exactly that.

What was ruled out, each by its own run: not the linker (it aborts with the
platform linker too), not the parallel front-end (LLVM with `-Zthreads=8`
passes), not a compiler-cache wrapper (it aborts with `RUSTC_WRAPPER` unset),
and not a missing flag (`-Cforce-unwind-tables=yes` and an explicit
`-Cpanic=unwind` both still abort). Scoping the backend to `[profile.test]`
alone does not rescue it either: a clean `cargo test` under that setting builds
every crate on LLVM, dependencies included, so the only beneficiary would be
`make build` — producing an artefact that aborts on a panic, differing from the
binary the tests exercise.

**Why the thread count is a variable.** No tuning sweep was done, and none is
claimed: `8` is the value the standard carries, chosen to exceed the core count
of the machines contributors use so that the front end keeps work in flight
through I/O waits. It is deliberately reached through one variable in two
places, `STANDARD_THREADS_FLAG` and the configuration file, held equal by
contract, so that raising it is a one-line change; a higher or lower value
should be justified by the benchmark rather than by this record.

## Consequences

- A bare `cargo build` in a checkout is accelerated, with no opt-in and no
  slower alternative to select. There is no `install-dev-fast` or
  `dev-fast-check` target any more: `install-build-tools` and
  `check-build-tools` name what they do, and the build targets are the build
  targets.
- A Linux contributor who has not run `make install-build-tools` gets a
  _failure_ at link time, not a silent fallback: gcc is passed `-fuse-ld=mold`
  explicitly, and with no `mold` reachable it reports that it cannot find the
  linker and stops. The gate targets report this earlier and more clearly, as a
  missing prerequisite.
- A `cargo install --path .` inside the checkout inherits the standard, because
  it reads the same configuration file. The users' guide and README therefore
  document the prerequisite for source installs and name the platform
  difference; a registry install builds from packaged source, where neither the
  configuration file nor `rust-toolchain.toml` applies, and needs neither.
- The release and coverage exclusions are asserted by contract, not assumed.
  Adding anything new to `.cargo/config.toml` reaches those shapes too, so a
  setting that is only safe for the development loop belongs in the Makefile's
  composed `RUSTFLAGS` instead.
- The standard is currently justified by what it does rather than by a recorded
  figure. The one measurement attempt, on 2026-09-17, ran on a shared host
  whose load average went from 0.7 to 117 and reversed its verdict twice. No
  table is recorded here until a run on an otherwise-idle host produces samples
  that agree; the developers' guide states the conditions such a run needs.
- Re-testing Cranelift on a toolchain bump is a deliberate act, and the
  developers' guide carries the probe. `CARGO_PROFILE_DEV_CODEGEN_BACKEND`
  remains available for a single scoped experiment.

## Alternatives considered

- **Export the flags from the Makefile's shell environment instead.** Rejected
  because it accelerates only Make-driven builds, leaves `cargo build` and
  rust-analyzer slow, and is harder to make a contract read than a committed
  file.
- **Keep two targets, one standard and one accelerated.** Rejected as the shape
  that let the acceleration go unused: an "opt-in fast target" is a target
  nobody runs, and it doubles the CI matrix to prove something the default
  could prove on its own.
- **Adopt the Cranelift backend.** Rejected on the evidence above. The
  performance case is real and the correctness case is disqualifying: a debug
  binary that aborts at 134 where it should exit 101 is a different program
  from the one the tests exercise.
- **Scope Cranelift to `make build` only.** Rejected because a profile override
  does not confine the backend to the artefacts the developer intends, and the
  one artefact it does reach would then behave differently from every test.
- **Let `mold` fall back to the platform linker when absent.** Rejected as the
  worst of both worlds: the build would succeed, so nothing would prompt the
  contributor to install it, and the numbers they compare against a colleague's
  would be measuring different links.
- **Pin `mold` through the toolchain file.** Rejected because `rustup` manages
  compilers, not linkers; the version pin and its checksums live in
  `tools/mold/` and are verified by the installer.

## Addendum, 2026-09-21: the suite under Cranelift

The decision above is unchanged. This records the measurement that was missing
from it.

The Cranelift exclusion rested on the three-case probe in _Rationale_: a crate
with no dependencies, written to isolate the unwind behaviour. That probe
explains a mechanism. It does not answer the question the exclusion turns on,
which is whether this repository's own suite runs under the backend — and the
estate prefers Cranelift wherever a repository's suite passes under it.

So the suite was run, on `main` at `00f48f77`, on the pinned
`nightly-2026-08-23`, with `[profile.dev] codegen-backend = "cranelift"` and
`[unstable] codegen-backend = true` added to `.cargo/config.toml` and nothing
else changed. The control is the same commit and the same command with that
fragment removed.

| Arm                     | Result | Counts                                               |
| ----------------------- | ------ | ---------------------------------------------------- |
| LLVM control            | passes | 3309 run, 3309 passed, 5 skipped; 39 doctests passed |
| Cranelift               | fails  | stops at 1372 of 3309 on the first abort             |
| Cranelift, no fail-fast | fails  | 3309 run, 3302 passed, 6 failed, 1 timed out         |

_Table 2: `make test` on 2026-09-21, by codegen backend._

Five of the six failures are the unwind behaviour Table 1 already describes, in
both of its shapes. The sixth is a test that runs the built binary repeatedly
and crosses its per-test allowance because the Cranelift-built binary is
slower; run alone it passes under both backends. The developers' guide names
each failing test and attributes it.

Two things follow, and neither changes the decision:

- The re-test on a toolchain bump is the suite, not the probe. The probe is
  kept because it explains what fails; only the suite answers whether the
  backend is usable here.
- The exclusion is about this repository. Five of the six failures are tests
  whose subject is a panic crossing a boundary, so a repository without such
  tests would meet none of them. Other repositories on this estate do use
  Cranelift, and this record does not argue against that.

## Implementation references

- The standard itself: [`.cargo/config.toml`](../.cargo/config.toml)
- Composed flags and gate prerequisites: [`Makefile`](../Makefile)
- Capability check and installer:
  [`scripts/check-build-tools.sh`](../scripts/check-build-tools.sh) and
  [`scripts/install-build-tools.sh`](../scripts/install-build-tools.sh)
- Version and checksum pins: [`tools/mold/`](../tools/mold)
- Configuration contract:
  [`tests/build_tools_cargo_config_tests.rs`](../tests/build_tools_cargo_config_tests.rs)
- Makefile and configuration held equal:
  [`tests/build_tools_make_target_tests.rs`](../tests/build_tools_make_target_tests.rs)
  and
  [`tests/makefile_test_target/rustflags.rs`](../tests/makefile_test_target/rustflags.rs),
  whose model is composed through real Make and shell expansion in
  [`tests/makefile_test_target/rustflags_expansion.rs`](../tests/makefile_test_target/rustflags_expansion.rs)
- Exclusion contracts:
  [`tests/workflow_contracts/build_standard_wiring_test.py`](../tests/workflow_contracts/build_standard_wiring_test.py)
- Benchmark: [`scripts/bench-build.sh`](../scripts/bench-build.sh)
- Contributor guidance:
  [`developers-guide.md`](developers-guide.md#the-build-standard)

## Related decisions

- [ADR-006: Adopt the Polonius borrow checker on a pinned nightly toolchain][adr-006]
- [ADR-025: Local pull-request coverage ratcheting with persistent coverage data owned by `main`][adr-025]

[adr-006]: adr-006-adopt-polonius-nightly-toolchain.md
[adr-025]: adr-025-main-owned-coverage-publication.md
