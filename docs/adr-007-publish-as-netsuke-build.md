# Architecture decision record (ADR): Publish the crates.io package as `netsuke-build`

## Status

Accepted.

## Date

2026-08-05.

## Context and problem statement

Netsuke ships a single application crate whose Cargo package, library target,
and binary target were all named `netsuke`. The `netsuke` name is already taken
on crates.io by an unrelated package, so the registry cannot accept a release
under it.

Renaming the package is the only way to publish, but the name is load-bearing
in several places that have nothing to do with the registry:

- the command users type, and the `Usage:` line clap renders for it;
- the manual page, which packaging installs as `netsuke.1` and users read with
  `man netsuke`;
- the Debian, RPM, macOS, and Windows package names, and the release assets
  they are built from;
- the library target that the integration tests, behavioural tests, and build
  script all import as `netsuke`.

Cargo lets the package name and the target names diverge, but nothing enforces
the divergence: the build script previously derived the manual page name from
`CARGO_BIN_NAME`/`CARGO_PKG_NAME` and rejected a mismatch with the command-line
interface (CLI) name, which would have renamed the manual page to follow the
package.

## Decision

Publish as `netsuke-build`, and keep every user-facing name as `netsuke`.

- Set `package.name = "netsuke-build"` in `Cargo.toml`, with
  `[lib] name = "netsuke"` and `[[bin]] name = "netsuke"`.
- Derive the manual page name and its `.TH` source from the CLI name that
  `clap` reports, not from Cargo's package or binary environment variables.
  `build.rs` no longer reads `CARGO_PKG_NAME` or `CARGO_BIN_NAME`, and no
  longer fails the build when they differ from the CLI name; that check
  enforced exactly the coupling this decision removes.
- Keep `.github/release-staging.toml`, the `linux-packages`, `windows-package`,
  and `macos-package` steps, and the release help tooling driven by the
  `bin-name` Cargo metadata field, which resolves to `netsuke`.
- Add `[package.metadata.binstall]` overrides so `cargo binstall netsuke-build`
  resolves the release assets, which are named after the binary. Without them
  `cargo binstall` would look for `netsuke-build`-prefixed assets, fail to find
  any, and fall back to a source build that needs the pinned nightly and the
  Polonius flag — the very fallback the documented command exists to avoid.
- Update the crates.io installation guidance in the README, the users' guide,
  and the quickstart to install `netsuke-build`.

## Rationale

- **The registry name is an implementation detail.** Users invoke `netsuke`,
  read `man netsuke`, and install a `netsuke` operating-system package. Only
  the two `cargo install` and `cargo binstall` commands mention the package
  name, and both are documented and pinned by contract tests.
- **Renaming the targets would be far more invasive.** The library target name
  is the crate path every test, the build script, and the
  `[package.metadata.ortho_config]` `root_type` setting use; renaming it would
  churn the whole tree to work around a registry collision.
- **`netsuke-build` reads as a description, not a substitute.** It names what
  the package is — the Netsuke build system — so a reader who finds it on
  crates.io is not left guessing whether it is the same project.

## Consequences

- The package name and target names diverge permanently. Anything deriving a
  user-facing name from Cargo package metadata is a defect; derive from the CLI
  name or from the `bin-name` metadata field instead.
- `tests/man_page_contract_tests.rs` pins the manual page's name, staging
  location, and `.TH` source against the CLI name, and asserts the package name
  never reaches the title. `tests/binstall_metadata_tests.rs` pins the
  `binstall` overrides to `.github/release-staging.toml` and to the release
  workflow's target matrix.
- The `binstall` overrides encode release asset names. Changing
  `staging_dir_template`, `bin_name`, or the workflow artefact names without
  updating the overrides breaks `cargo binstall`; the contract test fails first
  for the parts it can derive.
- Documentation and contract tests refer to `netsuke-build` only for registry
  installation. Everywhere else — prose, examples, help output, packaging — the
  project remains Netsuke.
- Should the `netsuke` name become available on crates.io, this decision can be
  reversed by changing `package.name` alone, because nothing else derives from
  it.

## References

- [ADR-006](adr-006-adopt-polonius-nightly-toolchain.md): the pinned-nightly
  policy that makes the `cargo binstall` path worth preserving.
- [Repository layout](repository-layout.md): the package-versus-target naming
  rule.
- [Developer guide](developers-guide.md): the day-to-day naming guidance and
  the contract tests that enforce it.
