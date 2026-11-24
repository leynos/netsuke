# Architectural decision record (ADR) 0086: Select the pinned mold through Cargo's linker key

## Status

Accepted.

## Date

2026-11-24

## Context and problem statement

[ADR-0029](adr-0029-make-mold-and-the-parallel-frontend-the-default-build.md)
makes the `mold` linker and the parallel `rustc` frontend the default build
standard, committed to `.cargo/config.toml`, and excludes release and coverage
builds by assigning `RUSTFLAGS` at the point they run. Cargo selects a single
`rustflags` source, so an assigned value replaces every table in the
configuration; that displacement is the exclusion mechanism.

The mechanism fails in one direction. A caller that exports its own `RUSTFLAGS`
also displaces the standard, and the mutation worker does exactly that: the
shared workflow exports `RUSTFLAGS="-D warnings"` for the whole job. A census
of a `--verbose` baseline log found `-D warnings` on all 569 `rustc`
invocations and the mold flag on none. The worker therefore installed the
pinned mold — ADR-0029's tooling half — but did not select it. Installing a
linker is not selecting one.

Restating the flag in the caller is not available: the caller must not own a
value the project owns, and a caller-side replacement would not preserve
configuration-owned flags (leynos/shared-actions#544 and #628).

## Decision

Select the pinned mold through Cargo's target `linker` key, which an exported
`RUSTFLAGS` does not displace.

- The `cfg(target_os = "linux")` table in `.cargo/config.toml` names
  `linker = ".cargo/mold-linker"`, a repository-owned wrapper.
- The wrapper runs `cc "$@" -fuse-ld=mold` by default. The flag is appended,
  not prepended: `rustc` forwards its own `-fuse-ld=lld`, and a compiler driver
  honours the last `-fuse-ld` it receives, so a leading mold flag would be
  overridden and the link would use lld.
- The wrapper takes `cc` alone when `NETSUKE_PLATFORM_LINKER=1`. That explicit
  opt-out is what release, packaging, coverage, and the Kani lane set, because
  displacement no longer excludes this key.
- The packaged Linux builds run inside a `cross` container, which forwards
  only `CARGO_*`, `CROSS_*`, and a short allowlist of host variables.
  `Cross.toml` passes `NETSUKE_PLATFORM_LINKER` through, so the opt-out reaches
  the container the wrapper runs in.
- Cargo resolves the relative value as a config-relative path from the
  directory that contains `.cargo` — the repository root — not from the working
  directory. That is what makes the wrapper found from a subdirectory and from
  cargo-mutants' copied build tree, whose file copy preserves the execute bit.

## Rationale

The `linker` key is the only in-repository setting that survives an exported
`RUSTFLAGS`. The shared `setup-rust` action documents the same division of
labour: its `install-mold` input "sets no linker flag: select mold in the
project's Cargo configuration". A wrapper rather than a bare path is needed
because the excluded shapes must keep the platform linker, and the
configuration key reaches every Linux build.

Appending the mold flag rather than prepending it was measured. `cc` accepts a
repeated `-fuse-ld` and the last one wins, and a recorded `rustc` argv from
this workspace carries `-fuse-ld=lld`. A prepended flag selected lld in the
worker shape, which is the defect this record removes.

## Consequences

- The mutation worker selects mold even though its caller exports
  `RUSTFLAGS`. `-Zthreads=8` remains displaced there and is tracked upstream
  (leynos/shared-actions#544).
- Release, packaging, coverage, and Kani keep the platform linker through the
  explicit opt-out, asserted by the Makefile-expansion and workflow contract
  suites.
- A Linux contributor without the pinned mold still fails at link, never
  falling back silently.
- macOS and Windows are unchanged: the key sits inside the Linux `cfg` table.
- A caller-supplied `CARGO_TARGET_<triple>_LINKER` still outranks the
  configuration key, so cross-compilation keeps the linker its toolchain
  supplies.
- ADR-0029's exclusion rule is amended, not replaced: its `## Amendments`
  section records this change and its decision text stays as history.

## Implementation references

- Configuration: [`.cargo/config.toml`](../../.cargo/config.toml)
- Wrapper: [`.cargo/mold-linker`](../../.cargo/mold-linker)
- Container passthrough: [`Cross.toml`](../../Cross.toml)
- Composition and exclusion variables: [`Makefile`](../../Makefile)
- Contracts:
  [`tests/build_tools_linker_tests.rs`](../../tests/build_tools_linker_tests.rs),
  [`tests/makefile_test_target/rustflags.rs`](../../tests/makefile_test_target/rustflags.rs),
  and
  [`tests/workflow_contracts/build_standard_wiring_test.py`](../../tests/workflow_contracts/build_standard_wiring_test.py)
- Working reference:
  [developers' guide](../developers-guide.md#the-build-standard)

## Related decisions

- [ADR-0029: Make mold and the parallel frontend the default build](adr-0029-make-mold-and-the-parallel-frontend-the-default-build.md),
  amended by this record.
- [ADR-0006: Adopt the Polonius borrow checker on a pinned nightly toolchain](adr-0006-adopt-the-polonius-borrow-checker-on-a-pinned-nightly-toolchain.md)
  supplies the toolchain pin that the standard shares.
