# Architecture decision record (ADR): Adopt the Polonius borrow checker on a pinned nightly toolchain

## Status

Accepted.

## Date

2026-08-23

**Historical note:** This ADR was originally accepted on 2026-07-29.

## Context and problem statement

Netsuke's internal APIs carry the shape that the non-lexical-lifetimes (NLL)
borrow checker imposed on the whole Rust ecosystem: lookups that clone keys
unconditionally, registries that hand back owned values, and error paths that
compute context eagerly. The Polonius alpha analysis accepts a strict superset
of NLL and removes the lifetime limitation behind several of these shapes, so
the natural borrow-returning form of an accessor can compile where NLL rejected
it. When this ADR was first accepted the analysis was opt-in behind
`-Zpolonius=next`; nightly toolchains dated 2026-08-04 and later enable it by
default, and the directive is on its way out.

Adopting those borrow-centric designs binds the source tree to a
Polonius-enabled compiler, which is nightly-only until the analysis stabilizes.
That conflicts with three standing policies:

- `rust-toolchain.toml` pinned stable `1.89.0`;
- `Cargo.toml` declared `rust-version = "1.89.0"` as a minimum supported Rust
  version (MSRV);
- continuous integration (CI) built a `stable`/`1.89.0` matrix with nightly as
  an experimental leg.

Netsuke is a pre-1.0 application whose only API consumers are its own crates,
so internal API quality was judged to outweigh a stable-toolchain guarantee
(issue #465).

## Decision

Adopt Polonius now, as a nightly-only source tree:

- Pin a dated nightly in `rust-toolchain.toml` so builds stay reproducible. The
  pin is currently `nightly-2026-08-23`, which is at or after 2026-08-04 and so
  enables Polonius by default; a contract test enforces that lower bound.
- Treat the pin as carrying the compiler's *front-end dialect*, not just
  Polonius. It also supplies the next-generation trait solver, which Netsuke
  assumes and which subsequent work may rely on. The same no-directive rule
  applies: pass no `-Znext-solver` flag, because a build that restates a
  default is a build that can silently drop it.
- Pass no `-Zpolonius` directive anywhere. The pinned toolchain carries the
  requirement on its own, so plain Cargo invocations, rust-analyzer, Clippy,
  Whitaker, and Kani all borrow-check with the same analysis without any
  build-configuration cooperation. A contract test fails if the directive
  reappears in the Makefile, a Cargo configuration fragment, or a workflow.
- Collapse the CI matrices in `ci.yml` and `netsukefile-test.yml` to the
  pinned nightly, and align `coverage-main.yml`. Stable and MSRV legs are
  removed because the tree no longer compiles without Polonius.
- Remove the `rust-version` field from `Cargo.toml`. Cargo cannot express a
  nightly requirement there, and advertising `1.89.0` would misstate the
  contract; `rust-toolchain.toml` is now the single source of truth.

Every borrow-centric rewrite that depends on the analysis is recorded in
[polonius migration notes](polonius.md), including refusals where owned style
remains correct.

An earlier revision of this ADR enabled the analysis explicitly, through
`[build] rustflags` in `.cargo/config.toml`, a `POLONIUS_FLAGS` Make variable
restated by every recipe that set `RUSTFLAGS`, and a `with.rustflags` input on
each CI shared action. That plumbing existed only because the flag was
overridden by any `RUSTFLAGS` a wrapper exported. It became redundant when the
pin moved past 2026-08-04, and has been removed entirely; `.cargo/config.toml`
no longer exists, because carrying the flag was its only purpose.

## Rationale

- **Design over deployment breadth.** Netsuke ships binaries, not a library
  API. Consumers install packaged artefacts or build from source; the toolchain
  pin costs contributors one `rustup` fetch, whereas NLL-era double lookups and
  key clones cost every call site, forever.
- **Reproducibility.** A dated nightly behaves like a release: the same
  compiler bits build the tree everywhere. `rustup` provisions it automatically
  from `rust-toolchain.toml`.
- **Coherent tooling.** Carrying the requirement in the toolchain pin alone
  keeps rust-analyzer, Clippy, Whitaker (whose Dylint driver is nightly-based),
  and Kani borrow-checking the same dialect, avoiding phantom editor errors on
  correct code. There is no flag for a wrapper to drop.
- **Stabilization path.** Polonius is a Rust project goal for stabilization,
  and is already the nightly default. When it reaches stable, the pin can be
  dropped without touching the migrated code, and an MSRV can be re-declared at
  that release.

## Consequences

- Publishing to crates.io remains possible, but the packaged source excludes
  `rust-toolchain.toml` (and Cargo would not apply it to a registry build
  anyway), so a bare `cargo install netsuke-build` of a Polonius-dependent
  release fails borrow checking on the user's default toolchain. Registry
  installs must select the pinned nightly explicitly
  (`cargo +nightly-2026-08-23 install netsuke-build`); the README and users'
  guide document this command and a contract test pins it. Source installs from
  a checkout are unaffected because the pinned toolchain applies there.
- Release packaging builds from the pinned nightly. Binary artefacts are
  unaffected: the borrow checker changes what compiles, not what is generated.
- Dependabot-style toolchain drift is impossible; moving the pin is a
  deliberate act. Move it forward periodically (and especially once Polonius
  stabilizes), re-running the full gate suite, and update this ADR's references
  when doing so. Because the pin now carries the trait solver as well, expect a
  pin move to surface toolchain events beyond borrow checking — new lints, and
  build-layout or metadata changes in the accompanying Cargo. Record what a
  move required rather than treating the fallout as unrelated breakage.
- Sites that genuinely require Polonius are tagged `POLONIUS(...)` in source
  and must not be rewritten into NLL-era defensive forms; `AGENTS.md` and
  [polonius migration notes](polonius.md) carry the anti-regression guidance.
- `cargo +stable` invocations fail to borrow-check the `POLONIUS(...)` sites.
  Under the retired flag the failure was loud and immediate — stable rejects the
  `-Z` directive outright — whereas the requirement now surfaces as a
  borrow-check error. The pinned toolchain applies automatically inside a
  checkout, so reaching that error takes a deliberate `+stable` override.
