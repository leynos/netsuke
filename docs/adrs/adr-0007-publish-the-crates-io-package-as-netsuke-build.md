# Architectural decision record (ADR) 0007: Publish the crates.io package as netsuke-build

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
- Add a `[package.metadata.binstall]` template so
  `cargo binstall netsuke-build` resolves the release assets, which are named
  after the Cargo package rather than the binary. A single `pkg-url` template
  interpolating `{ name }-{ version }-{ target }.tar.gz` (`pkg-fmt = "tgz"`)
  covers every released target because `stage-release-artefacts` names each
  target's archive to the same shape. Without the template `cargo binstall`
  would probe its default asset-name patterns, which place the target before
  the version, fail to find any matching asset, and fall back to a source build
  that needs the pinned nightly — the very fallback the documented command
  exists to avoid.
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
  `pkg-url` template to `.github/release-staging.toml`'s `[common.binstall]`
  archive-naming contract and to the release workflow's target matrix, and
  `tests/workflow_contracts/hoist_binstall_archives_test.py` pins the hoist
  script that stages those archives to the release root.
- The `pkg-url` template encodes the staged archive name's shape. Changing
  `staging_dir_template`, `bin_name`, or the workflow artefact names without
  keeping the staged archive name in step with the template breaks
  `cargo binstall`; the contract tests fail first for the parts they can derive.
- Documentation and contract tests refer to `netsuke-build` only for registry
  installation. Everywhere else — prose, examples, help output, packaging — the
  project remains Netsuke.
- Should the `netsuke` name become available on crates.io, this decision can be
  reversed by changing `package.name` alone, because nothing else derives from
  it.

## References

- [ADR-0006](adr-0006-adopt-the-polonius-borrow-checker-on-a-pinned-nightly-toolchain.md):
  the pinned-nightly policy that makes the `cargo binstall` path worth
  preserving.
- [Repository layout](../repository-layout.md): the package-versus-target naming
  rule.
- [Developer guide](../developers-guide.md): the day-to-day naming guidance and
  the contract tests that enforce it, and the release-publication runbook that
  the second addendum's mechanism plugs into.
- [Trusted-packaging ExecPlan](../execplans/use-trusted-packaging.md): the
  plan of work that delivers the registry exchange named in the second addendum.

## Addendum (2026-08-28)

A source-build fallback from `cargo binstall` requires the repository's pinned
nightly toolchain. This requirement follows the repository toolchain policy;
the fallback must not restore the retired `RUSTFLAGS=-Zpolonius=next`
instruction.

## Addendum (2026-10-24): the trusted publisher

The decision above settles the *name* the registry sees. This addendum settles
how the bytes reach the registry, which the original decision left open.

**Decision.** Publish through crates.io Trusted Publishing, from the repository
itself, with no long-lived registry credential stored anywhere.

- The credential is exchanged, not stored. `rust-lang/crates-io-auth-action`
  takes a GitHub OpenID Connect (OIDC) token and yields a short-lived registry
  token, and the action revokes it in a post step. No `CARGO_REGISTRY_TOKEN`
  secret exists in the repository, and none is to be added.
- The exchange runs in a job declared in `release.yml` itself. crates.io
  registers a publisher by repository owner, repository name, workflow
  filename, and an optional environment, and it presents the OIDC
  `job_workflow_ref` of the file that declares the job. A reusable workflow
  would present its own filename, so moving the exchange into one silently
  breaks publication.
- The exchange job has `contents: read`, `id-token: write`, and the
  `crates-io` environment. It has no GitHub release-write authority; that
  permission remains in separate release jobs.
- The first publication is not reachable this way. Trusted Publishing can only
  publish a crate that already exists, so a brand-new package name still needs
  one manual token-authenticated upload. `netsuke-build` is already published,
  so this precondition is satisfied and no manual step is outstanding.

**Status of the decision.** Accepted as the target mechanism; not yet
implemented. `release.yml` currently packages the crate in `package-crate` and
publishes nothing to crates.io. The job that performs the exchange, the
inventory verifier that checks downloaded bytes against the pre-upload digest
record, the release finalizer, and the consumer smoke test are described in the
trusted-packaging ExecPlan and remain to be written. Until they land, a release
run publishes GitHub assets only, and this addendum records the shape the
registry path is to take rather than a mechanism in use.

The offline policy modules that support the decision are already in place and
are what the unwritten job will call. Policy modules compare in-memory facts;
the CLI and named adapters own file access, JSON/TOML serialization, registry
response schemas, HTTP status codes, and GitHub's digest spelling. In
particular, `release_reconcile_crates` receives typed registry observations,
`verify_crates_io_bytes` receives a typed download observation, and
`release_reconcile_github` receives bare digest strings. Those modules reach
neither the network nor filesystem or provider formats; workflow steps make the
network requests, and the adapters translate their results.
