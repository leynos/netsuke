# Architecture decision record (ADR): Adopt the Polonius borrow checker on a pinned nightly toolchain

## Status

Accepted.

## Date

2026-07-29.

## Context and problem statement

Netsuke's internal APIs carry the shape that the non-lexical-lifetimes (NLL)
borrow checker imposed on the whole Rust ecosystem: lookups that clone keys
unconditionally, registries that hand back owned values, and error paths that
compute context eagerly. The Polonius alpha analysis (`-Zpolonius=next`)
accepts a strict superset of NLL and removes the lifetime limitation behind
several of these shapes, so the natural borrow-returning form of an accessor
can compile where NLL rejected it.

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

- Pin the dated toolchain `nightly-2026-06-25` in `rust-toolchain.toml` so
  builds stay reproducible.
- Enable `-Zpolonius=next` in `.cargo/config.toml` under `[build] rustflags`,
  so plain Cargo invocations and rust-analyzer borrow-check with the same
  analysis. Makefile recipes that set `RUSTFLAGS` (which overrides that table)
  re-state the flag via the `POLONIUS_FLAGS` variable. `cargo kani` sets
  `CARGO_ENCODED_RUSTFLAGS` itself, which also bypasses the table, so the
  `kani-full` recipe passes the flag through the `RUSTFLAGS` environment
  variable, which Kani appends to its own flags.
- Collapse the CI matrices in `ci.yml` and `netsukefile-test.yml` to the
  pinned nightly, and align `coverage-main.yml`. Stable and MSRV legs are
  removed because the tree no longer compiles without Polonius.
- Remove the `rust-version` field from `Cargo.toml`. Cargo cannot express a
  nightly requirement there, and advertising `1.89.0` would misstate the
  contract; `rust-toolchain.toml` is now the single source of truth.

Every borrow-centric rewrite that depends on the flag is verified both with and
without `-Zpolonius=next` and recorded in
[polonius migration notes](polonius.md), including refusals where owned style
remains correct.

## Rationale

- **Design over deployment breadth.** Netsuke ships binaries, not a library
  API. Consumers install packaged artefacts or build from source; the toolchain
  pin costs contributors one `rustup` fetch, whereas NLL-era double lookups and
  key clones cost every call site, forever.
- **Reproducibility.** A dated nightly behaves like a release: the same
  compiler bits build the tree everywhere. `rustup` provisions it automatically
  from `rust-toolchain.toml`.
- **Coherent tooling.** Putting the flag in `.cargo/config.toml` keeps
  rust-analyzer, Clippy, Whitaker (whose Dylint driver is nightly-based), and
  Kani borrow-checking the same dialect, avoiding phantom editor errors on
  correct code.
- **Stabilization path.** Polonius is a Rust project goal for stabilization.
  When `-Zpolonius=next` becomes default behaviour on stable, the pin and the
  flag can be dropped without touching the migrated code, and an MSRV can be
  re-declared at that release.

## Consequences

- Publishing to crates.io remains possible, but the packaged source excludes
  `rust-toolchain.toml` and `.cargo/config.toml` (and Cargo would not apply
  them to a registry build anyway), so a bare `cargo install netsuke-build` of
  a Polonius-dependent release fails borrow checking on the user's default
  toolchain. Registry installs must select the pinned nightly and pass the flag
  explicitly
  (`RUSTFLAGS=-Zpolonius=next cargo +nightly-2026-06-25 install netsuke-build`);
  the README and users' guide document this command and a contract test pins
  it. Source installs from a checkout are unaffected because the pinned
  toolchain and workspace configuration apply there.
- Release packaging builds from the pinned nightly. Binary artefacts are
  unaffected: the borrow checker changes what compiles, not what is generated.
- Dependabot-style toolchain drift is impossible; moving the pin is a
  deliberate act. Move it forward periodically (and especially once Polonius
  stabilizes), re-running the full gate suite, and update this ADR's references
  when doing so.
- Sites that genuinely require Polonius are tagged `POLONIUS(...)` in source
  and must not be rewritten into NLL-era defensive forms; `AGENTS.md` and
  [polonius migration notes](polonius.md) carry the anti-regression guidance.
- `cargo +stable` invocations fail on `-Zpolonius=next`. This is intentional:
  the failure is loud and immediate rather than a confusing borrowck error
  later.

## Addendum — 2026-08-27: nightly-default Polonius and toolchain boundaries

The repository pin has since moved to `nightly-2026-08-23`. Nightlies dated
2026-08-04 and later enable the Polonius alpha analysis by default, so the pin
now carries the borrow-checker requirement without an explicit directive.

The explicit `-Zpolonius` plumbing from the original decision has been retired:
the `.cargo/config.toml` rustflags entry, the `POLONIUS_FLAGS` Makefile
variable, and the CI `with.rustflags` inputs were removed. The pin is now the
sole repository mechanism for this compiler behaviour.

Kani is outside that boundary. Kani 0.67.0 installs and uses its own bundled
`nightly-2025-11-21` toolchain through `cargo kani setup`. That toolchain
predates nightly-default Polonius, so Kani currently uses NLL. Moving the
repository Rust pin does not upgrade Kani. Do not claim that Kani verifies
`POLONIUS(...)` APIs with Polonius until a Kani release bundles a sufficiently
recent nightly, or Kani is rebuilt from source against that nightly.

Registry installs likewise do not inherit the checkout's toolchain file. They
must select the repository's pinned nightly explicitly, for example:

```sh
cargo +nightly-2026-08-23 install netsuke-build
```

## Addendum — 2026-09-04: Dependabot-owned toolchain updates

Dependabot now owns routine updates to the checked-in `rust-toolchain.toml`
declaration through the `rust-toolchain` ecosystem block. The repository
remains pinned to a dated nightly, and each Dependabot pull request still
requires normal human review and the repository quality gates before the pin
changes. Kani's separately managed toolchain remains outside this policy.
