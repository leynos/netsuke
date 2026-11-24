# Select the pinned mold through Cargo's `linker` key (issue #875)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This plan delivers the second half of issue #875. The first half — provisioning
the pinned mold on the mutation worker and making the rustdoc-override tests
inject the preflight — landed as pull request #911 and is historical; see
[the completed plan](issue-875-mutation-baseline-reaches-real-build-tool-preflight-without-the-required-pinned-mold.md).
This plan does not modify that document.

## Purpose / big picture

The mutation worker installs the pinned mold, but its `rustc` invocations do
not select it. The shared workflow exports `RUSTFLAGS="-D warnings"`, and Cargo
lets an assigned `RUSTFLAGS` replace every `rustflags` table in
`.cargo/config.toml`, so the `-Clink-arg=-fuse-ld=mold` entry there is
displaced. The measured census on the worker is 569 invocations carrying
`-D warnings`, none carrying the mold flag. `make check-build-tools` still
passes, because mold is installed and executable; installation is not
selection. This is the issue's remaining acceptance clause and it is currently
recorded as unmet.

The measured remedy is the one setting an exported `RUSTFLAGS` does **not**
displace: the target `linker` key. The wrapper must also place its
`-fuse-ld=mold` *after* the forwarded arguments, because `rustc` forwards its
own `-fuse-ld=lld` and a compiler driver honours the last such flag; a
prepended flag loses, which a `cc` probe and a recorded `rustc` argv both show.
A three-configuration compile probe in the completed plan showed the
`rustflags` table disappearing under the export while a `linker` entry survives
it. The shared `setup-rust` action documents the same contract for its
`install-mold` input: it "sets no linker flag: select mold in the project's
Cargo configuration".

So the outcome is a repository-owned `linker` wrapper that selects mold for
every development build — including one whose caller exports its own
`RUSTFLAGS`, which is exactly the mutation worker — while release, packaging,
and coverage keep the platform linker through an explicit opt-out the wrapper
reads.

## Constraints

- `.cargo/config.toml` is auto-discovered, so the `linker` key reaches every
  Linux build in the repository, including release and coverage. Those shapes
  must keep the platform linker, and the exclusion must be enforced and
  contract-tested, not assumed.
- macOS and Windows are unchanged. mold ships for Linux only, so the key stays
  in the `cfg(target_os = "linux")` table.
- No caller-owned `RUSTFLAGS` replacement is introduced. The mutation caller
  continues to set no flag variable; the shared workflow's export is upstream
  flag ownership (leynos/shared-actions#544) and is not worked around.
- The pinned mold and its version remain owned by `tools/mold/VERSION` and
  `tools/mold/SHA256SUMS`; no second pin is created.
- The wrapper must not depend on the working directory. Cargo resolves a
  relative `linker` value as a config-relative path from the directory that
  contains `.cargo` — the repository root here — measured from a subdirectory
  with `--manifest-path`.
- cargo-mutants copies the tree (including `.cargo/`) into a temporary build
  directory and uses `fs::copy`, which preserves permission bits; the wrapper
  must therefore be a committed executable whose mode is part of the contract.

## Tolerances (exception triggers)

- If Cargo does not resolve the relative `linker` value from a subdirectory,
  stop and redesign rather than papering over it with an absolute path.
- If the wrapper changes the link for a shape the build standard does not
  cover, stop and record it before merging.
- If a Linux lane that compiles does not have mold installed and is not an
  exclusion, provision it or mark the lane; do not let it fail at link.
- If proving selection on the real worker needs a build-artefact inspection
  that costs a second full workspace build, prefer a cheaper observable
  (`cargo config get target`) plus the baseline's own success.

## Risks

- **A forgotten opt-out ships a mold-linked artefact.** Every excluded lane is
  named in a contract test: the release recipe and Kani composition through the
  Make expansion suite, the coverage and packaging lanes by workflow parsing.
- **The wrapper adds a shell to every link.** The cost is one `exec` per linked
  artefact and is negligible against a link; the wrapper is the only route that
  survives the export.
- **The wrapper could break cross-compilation.** A caller-supplied
  `CARGO_TARGET_<triple>_LINKER` outranks the configuration key, so the
  packaging cross-build keeps the linker the shared action configures. The
  packaging lane also opts out explicitly.
- **The `[env]` alternative was rejected** because it would put a marker in the
  environment of every build script and test in the repository. The wrapper
  defaults to mold, and only excluded lanes carry the opt-out.

## Progress

- [x] (2026-11-23) Confirm the first half is merged (`7d1e4c127`) and the
  remaining clause is linker selection.
- [x] (2026-11-23) Measure that Cargo resolves a relative `linker` as a
  config-relative path from the directory containing `.cargo`, from a
  subdirectory.
- [x] (2026-11-23) Confirm `[env]` reaches the linker process (mechanism
  available but rejected).
- [x] Add `.cargo/mold-linker` and the `linker` key.
- [x] Compose the `NETSUKE_PLATFORM_LINKER` opt-out into the release and Kani
  variables.
- [x] Set the opt-out in the coverage, packaging, and crate-packaging lanes.
- [x] Add the wrapper, configuration, Makefile, and workflow contract tests.
- [x] Update the developers' guide, ADR-0029, AGENTS.md, and the changelog.
- [x] (2026-11-23) `make check-fmt`, `make markdownlint`,
  `make github-actions-lint`, and the full `make test-workflow-contracts` pass
  (2606 passed, 3 skipped, re-run after the guard-test split).
- [x] (2026-11-23) Read the pinned shared `mutation-cargo.yml`: it sets no
  linker variable, its `install-mold` description directs callers to select
  mold in their Cargo configuration, and `setup-commands` runs under
  `set -euo pipefail` in the checkout root.
- [x] (2026-11-24) The full gate set passed through the gate runner:
  `check-fmt`, `lint`, `doc-coverage` (98.37%), `markdownlint`, `test` (6,586
  passed), `test-workflow-contracts` (2,606), `test-package-verify`, and
  `nixie`.
- [x] (2026-11-24) CodeRabbit review raised two findings; both were measured
  and fixed. The wrapper now *appends* `-fuse-ld=mold`, because `cc` honours
  the last `-fuse-ld` and `rustc` forwards `-fuse-ld=lld`; the path prose now
  says the value resolves from the directory containing `.cargo`. The Rust
  contract asserts the exact forwarded vector, including a forwarded
  `-fuse-ld=lld`.
- [x] (2026-11-24) A second review round found that the packaged Linux builds
  run inside `cross`, which forwards only `CARGO_*`, `CROSS_*`, and a short
  allowlist of host variables. `Cross.toml` now passes
  `NETSUKE_PLATFORM_LINKER` through, so the opt-out reaches the container where
  the wrapper runs; a Rust contract asserts it.
- [x] (2026-11-24) A fourth review round asked for the prescribed ADR
  amendment structure. ADR-0086 was allocated only after sweeping every remote
  and local ref (highest minted anywhere was 0085) and records the decision;
  ADR-0029 carries the `Amended by ADR-0086 (2026-11-24)` Status banner and an
  `## Amendments` section that preserves its decision text. The index is
  bijective: every ADR file is in `docs/contents.md` and every entry resolves.
- [x] (2026-11-24) CodeRabbit reported **0 findings** on the final candidate
  (25 files). Committed as `6c6d82857`, pushed over SSH, and opened as draft
  pull request [#1192](https://github.com/leynos/netsuke/pull/1192).
- [x] (2026-11-24) First CI signal found a real defect: both Linux packaging
  jobs failed at `Generate release help` with
  `collect2: fatal error: cannot find 'ld'`. The packaged artefact built fine;
  the nested `cargo-orthohelp` bridge build in the checkout fell through to
  `mold` on a runner that installs none, because the job-level opt-out did not
  reach it. The lane now runs `make install-build-tools` and extends `PATH`
  before that step, so it is correct whichever branch the nested build takes. A
  contract asserts the install, in a module split out to keep both under the
  file-length rule.
- [x] (2026-11-24) The fix passed every affected gate and was committed as
  `c0414c1f5` and pushed; a fresh release dry run (`45447812222`) and a
  CodeRabbit pass on that commit are in flight.
- [x] (2026-11-24) The mutation dispatch reached `Run setup commands` on all
  six shards and **every one succeeded**, so `make check-build-tools`, the
  `linker`-key resolution and the wrapper check all hold on the real worker.
- [ ] Record the shard baselines and the mutant outcomes when the dispatch
  finishes, then move the pull request to ready.

## Surprises & discoveries

- **Recording (2026-11-23): a relative `linker` is a config-relative path.**
  A probe crate ran from a subdirectory with `--manifest-path`; Cargo passed
  `-C linker=/tmp/…/.cargo/wrap.sh`, resolved from the directory *containing*
  `.cargo` — the crate root — not from `.cargo` itself and not from the working
  directory. The `..`-free `.cargo/mold-linker` value is therefore safe from
  any invocation directory.
- **Recording (2026-11-23): `[env]` reaches the linker.** A `[env]` entry in
  the same probe was visible inside the wrapper. It is not used: a repository-
  wide environment marker would leak into build scripts and tests, so the
  wrapper's default carries the choice instead.
- **Recording (2026-11-23): the upstream action already states this
  integration.** `setup-rust`'s `install-mold` description says it "sets no
  linker flag: select mold in the project's Cargo configuration", so the
  `linker` key is the documented consumer side rather than an invented
  workaround.
- **Recording (2026-11-23): the two shapes link as intended.** A probe crate
  using both the `rustflags` entry and the `linker` key linked with mold, and
  its executable carried the linker's `mold 2.41.0` version string. The same
  crate with `NETSUKE_PLATFORM_LINKER=1` and an assigned `RUSTFLAGS` (the
  release/coverage shape) carried no such string, so the opt-out keeps the
  platform linker once the table is displaced.

## Decision log

- **Decision:** select mold with a repository-owned wrapper in the `linker`
  key, defaulting to mold and opting out on `NETSUKE_PLATFORM_LINKER=1`.
  Rationale: `linker` survives the exported `RUSTFLAGS`; the wrapper is the
  only in-repository mechanism that can distinguish the standard's builds from
  the excluded shapes once displacement no longer can. Date/Author: 2026-11-23.
- **Decision:** the opt-out is positive and explicit (`=1`), matching the
  repository's other `NETSUKE_*_STRICT`-style switches, and it is read only by
  the wrapper. Rationale: each excluded lane states its intent at the place the
  exclusion applies. Date/Author: 2026-11-23.
- **Decision:** release and Kani compose the opt-out into their existing
  `RUSTFLAGS` variables; coverage and the two packaging jobs set it in the
  workflow. Rationale: those are the points at which each exclusion already
  becomes visible, and the contract suites already parse them. Date/Author:
  2026-11-23.
- **Decision:** pass `NETSUKE_PLATFORM_LINKER` through `Cross.toml`'s
  `[build.env] passthrough`. Rationale: the packaged Linux builds run under
  `cross`, which forwards only `CARGO_*`, `CROSS_*`, and a short allowlist, so
  a job-level opt-out would otherwise stop at the container boundary while the
  `linker` key inside would demand mold. Date/Author: 2026-11-24.
- **Decision:** the wrapper is `.cargo/mold-linker`, not a `scripts/` entry.
  Rationale: Cargo resolves the value from the directory containing `.cargo`,
  so this spelling needs no `..` and reads as configuration beside the file
  that selects it, rather than as a command contributors invoke. Date/Author:
  2026-11-23.

## Outcomes & retrospective

To be completed.

## Conformance basis

- ADR-0086 records the decision and amends ADR-0029, whose `## Amendments`
  section carries the correction and whose decision text stays as history.
- The developers' guide's *The build standard* and *Exclusions* sections are
  the working reference and are updated with the implementation.
- No new dependency, no new pin, and no formal-verification obligation.

## Verification plan

### Invariants

1. A Linux development build with no `NETSUKE_PLATFORM_LINKER` links through
   `cc -fuse-ld=mold`.
2. A build with `NETSUKE_PLATFORM_LINKER=1` links through `cc` alone.
3. Every excluded lane carries the opt-out: release and Kani in the Makefile
   composition, coverage in both workflows' steps, packaging in
   `build-and-package.yml` and `release.yml`.
4. The mutation caller sets neither the opt-out nor any `RUSTFLAGS`, so the
   worker selects mold.
5. macOS and Windows never invoke the wrapper.

### Acceptance commands

- `make check-fmt`
- `make lint`
- `make doc-coverage`
- `make markdownlint` (documentation touched)
- `make test`
- `make test-workflow-contracts`
- `make test-package-verify` (packaging lane touched)
- A manual `Mutation testing` dispatch on the change branch, recording
  `make check-build-tools` success, the resolved `linker` key, the named
  rustdoc tests, the complete unmodified baseline, and a nonempty mutant set.

### Evidence to capture

- The wrapper's argv under both marker values.
- `cargo config get target` in the repository showing the `linker` key.
- The gate logs under `/tmp`, named by action and branch.
- The mutation dispatch's job conclusions, uploaded artefacts, and any
  `-C linker=` census.
