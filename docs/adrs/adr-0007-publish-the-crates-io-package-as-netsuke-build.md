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

## Addendum (2026-11-02): first publisher stage implemented

The 2026-10-24 decision is implemented for its first publisher stage.
`release.yml` declares `publish-crates-io`, which consumes the retained
`package-crate` archive, probes the registry before mutation, publishes only an
absent version, skips an identical version, and verifies downloaded bytes
against the candidate digest. It uses the `crates-io` environment and a
short-lived Trusted Publishing exchange; no long-lived registry secret is
required. The publisher also records bounded phase outcomes and timings, plus
download retry count, status class, and elapsed time, in an always-run job
summary and JSONL artefact. These diagnostics do not constitute a live
publishing exercise.

No live exchange or release has exercised this workflow. The inventory
verifier, GitHub release finalizer, and consumer smoke test remain open in the
trusted-packaging ExecPlan, and `publish-release` is not yet gated on the
crates.io job. Until those stages land, this addendum records a partially
implemented mechanism rather than an end-to-end release path.

The candidate boundary is also enforced before the artefact hand-off.
`package-crate` runs `crates_io_candidate_cli.py check` against the local
identity record and archive, in a required and mutually exclusive release mode:
`--rehearsal` for a dry run, which accepts only a candidate that records no
tag, and `--tag` for a publishing run, which requires that exact tag. The
publisher repeats the check after downloading the artefact, so an interrupted
publication can be rerun: the second probe observes `identical`, skips the
upload, and verifies the public bytes. Both the resume sequence and the
rehearsal boundary are held by tests, the latter as a workflow-contract
assertion that the offline step's shell text names no credential flag, fetch
tool, or registry URL.

## Addendum (2026-11-09): the GitHub asset publisher

This addendum records the mechanism the GitHub half of a release uses to attach
assets, which the original decision left open: it settles how the bytes *reach
the Release*, where the 2026-10-24 addendum settles how they reach the
registry. It names the publisher, as the trusted-packaging ExecPlan's
documentation list requires.

**Decision.** `release.yml`'s `publish-release` job calls
`./.github/actions/upload-declared-assets`, a composite action in this
repository, rather than the shared `upload-release-assets` action the rehearsal
still uses. The action composes three CLIs already in `scripts/`:
`select_release_payload_cli.py` copies only the payload the release declares,
`release_declaration_cli.py` writes the declaration and the trusted digest
record from the staged bytes, and `upload_declared_assets_cli.py` reconciles
the release against that record and uploads only the gaps.

- Every producer binds its record to the candidate commit and the workflow
  run. `publish-release` checks out the candidate rather than the triggering
  ref, `build-and-package` verifies its own checkout against the same commit,
  and selection refuses a record from another commit or run before any name it
  claims is trusted.
- Selection matches staged files to the declaration by published name.
  Non-release-shaped material -- the binary, licences, completions, manual
  pages that ship inside the packages -- is excluded; a file carrying a release
  suffix that is not a declared name is refused.
- Each package claim is checked against where the download put it. Every
  producer uploads one workflow artefact and the release downloads each into
  its own subtree of `dist`, so `scripts/release_package_origins.py` can report
  the target key whose subtree staged each published package. A record claiming
  a package a sibling's subtree staged is refused, which is the one-for-one
  exchange no per-format count can see.
- The declaration and both digest records are written before the first
  network call and retained as a `declared-assets-<job>-<attempt>` workflow
  artefact, so a run that is later refused still leaves the account of what it
  declared.
- An attached asset whose digest matches the record is skipped. An attached
  asset with different bytes stops the run, as does one whose bytes could not
  be established at all, and the two are reported as distinct diagnoses. The
  uploader never replaces an attached asset.

**What this decision does not claim.** A package name cannot be derived: three
external shared actions own those names, so its format suffix and the
per-target count are all the name itself says. The subtree position is a
stronger observation but still an observation -- it reports where the download
put a file, not who built it. Suffix, count, and position checks narrow what
can reach the upload; none of them proves origin, and the records -- not those
checks -- are what bind the bytes to a commit. This is stated in the users' and
developers' guides rather than implied.

**Status.** Implemented and exercised by an executed-shell harness in
`tests/workflow_contracts/`; not yet exercised by a live release. The inventory
verifier, the GitHub release finalizer, and the consumer smoke test remain open
in the trusted-packaging ExecPlan, so the retained records have no consumer in
the workflow yet and the release stays a draft a person publishes.
