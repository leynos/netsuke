# Architectural decision record (ADR) 0051: Compile the build-script module slice in the workspace graph

## Status

Accepted.

## Date

2026-10-22

## Context and problem statement

`build.rs` compiles a slice of the production CLI: the Clap schema modules, the
locale registry, pattern syntax, and the localization surface. Before this
decision, `tests/build_module_slice_ui_tests.rs` proved that the slice compiled
by running a package-scoped `cargo check` inside a Nextest test body, resolving
the compiler artefacts from Cargo's JSON stream, and then invoking `rustc`
twice, once for a positive fixture and once for a negative one.

That placed Cargo's package-cache lock, dependency rebuilds, and
compiler-admission queueing inside a single test's allowance. The test timed
out at 900 s while its nested requests waited behind healthy admission, and
raising the allowance twice changed the symptom rather than the shape: a test
that prepares its own compiler work is charged for whatever else the host is
doing at the time.

Three further facts constrained the repair. The workspace has no
`default-members`, so a bare `cargo build` selects only the current package and
would never compile a fixture member. `Cargo.toml` excludes the production
slice from the shipped crate's build script only in the sense that the slice is
a module of `build.rs`; nothing else compiled it. And the timed test's
compilation had to keep proving the same boundary, or the timeout would be
removed at the cost of the coverage it existed to protect.

## Decision

Add `build_slice_fixture/`, a non-published workspace library that compiles the
production module slice during Cargo's ordinary build phase.

- **Cargo graph.** `Cargo.toml`'s `[workspace] members` lists
  `build_slice_fixture` beside `test_support`. The fixture's manifest declares
  `publish = false`, `[lib] path = "lib.rs"` with `test = false`, and inherits
  the workspace lint configuration.
- **Production sources, not a copy.** `build_slice_fixture/slice.rs` composes
  the same five path imports as `build.rs`, written with `../src/` prefixes:
  the inline `cli` module at `../src/cli`, then `cli_localization`,
  `host_pattern`, `locale_catalogues`, and `localization`. `lib.rs` wraps the
  composition in `#[cfg(not(any(test, doctest)))] include!("slice.rs");` so
  unrelated production unit tests stay out of the fixture and rustdoc does not
  collect their examples a second time.
- **Compiler controls.** The two module-level doctests in `lib.rs` are the
  positive and negative compile controls: a positive example names the supported
  `cli::Cli` surface, and a `compile_fail,E0432` example names
  `cli::discovery`, which must stay outside the slice.
- **Dependency-equality contract.**
  `tests/workflow_contracts/build_slice_fixture_test.py` requires the fixture's
  dependency table to equal the root `build-dependencies` entries for `camino`,
  `clap`, `ortho_config`, `serde`, `thiserror`, and `tracing` exactly, so the
  fixture cannot compile a slice the real build script could not.
- **Source-only runtime test.** `tests/build_module_slice_ui_tests.rs` keeps
  the source-boundary assertions. It compares `build.rs` with
  `build_slice_fixture/slice.rs` across the whole slice — the four inline `cli`
  declarations, the two `pub use` re-exports, and every sibling path/module
  pair — in order, after normalizing the fixture's `../src/` prefix. Order is
  part of the comparison because a path attribute applies to the declaration
  beneath it, so a reordered pair would compile a different file while still
  naming the same modules. `build_l10n_audit` is declared only by `build.rs`,
  and the test asserts both its presence there and its absence from `slice.rs`
  rather than leaving the exclusion implicit. The same workflow contract
  refuses a reintroduced child process or the retired artefact resolver in that
  test.
- **Removed override.** The test loses its dedicated fifteen-period 900 s
  Nextest allowance and its membership of the serialized `nested-cargo-builds`
  group, because it no longer starts Cargo.
- **Environment-suppression coverage.** `COMPILED_SOURCE_ROOTS` in
  `tests/env_access_suppressions/roots.rs` lists `build_slice_fixture`, so the
  fixture's sources are scanned like every other workspace source.
- **Ordinary builds select the workspace.** The Makefile's `build`, `release`,
  and `typecheck` recipes, and the two workflow steps that invoke Cargo
  directly, select `--workspace`. Without that selection the fixture would be
  built only by test commands, and the slice would stop being compiled by the
  ordinary build the fixture exists to guard.
- **Manifest tracking.** `.github/dependabot.yml` lists the fixture directory,
  which keeps the repository's manifest-coverage contract satisfied.

## Rationale

Compilation belongs to the build phase. Moving it there makes the cost visible
where Cargo already schedules it, so a cold dependency graph, a shared cache
lock, or a saturated admission pool delays the build rather than consuming a
test's budget. The test that remains is a pure query over two source files: it
is deterministic, instantaneous, and cannot be delayed by any other job on the
host.

Keeping the fixture a workspace library rather than a test target is what makes
the ordinary build compile the slice. A library member is part of `cargo build`,
`cargo check`, and every gate that selects the workspace, whereas an
integration test target is compiled only by test commands. The `test = false`
and `cfg(not(any(test, doctest)))` settings then keep the fixture to exactly
the compilation it was created for.

The dependency-equality contract is the part that keeps the fixture honest. A
fixture that drifted to a different dependency set, or a different feature set,
could compile a slice the build script cannot; comparing against the root
`build-dependencies` table makes that drift a contract failure rather than a
latent difference discovered during a release.

## Consequences

The production slice now compiles on every ordinary build, including on
platforms and feature selections the timed test never covered. A change that
breaks the slice fails the build instead of a test, which is the earlier and
more accurate signal.

The fixture adds a workspace member to every workspace-wide command.
`make build` and `cargo build --workspace` compile `test_support` and the
fixture as well as the binary, so a local debug build does more work than
before. That cost buys the guarantee above and is bounded by the same compiler
cache as the rest of the workspace.

The timeout documentation, the Nextest budget table, and the test-execution
coverage contract all lose their production-module entry. Any future change to
the slice must update `slice.rs`, the fixture manifest, and the workflow
contract together; the source test names the mismatch rather than silently
passing.

## Alternatives considered

- **Raise the allowance again.** Rejected. Two previous raises did not change
  the outcome, because the delay was admission and cache latency rather than
  the test's own work.
- **Keep the runtime test and share the build cache.** Rejected. The test would
  still wait for Cargo's package-cache lock and compiler admission inside its
  own allowance, which is the shape being removed.
- **Compile the slice in a `#[cfg(test)]` module of the library.** Rejected.
  That compiles the slice only for test builds, so a broken slice would still
  pass an ordinary build, and the module would sit in the shipped crate.
- **Use an integration test target instead of a library member.** Rejected. An
  integration target is built only by test commands, and any assertion it makes
  at run time is back inside a timed test body.
- **Keep the fixture but leave builds package-scoped.** Rejected. With no
  `default-members`, a package-scoped build never compiles the fixture, so the
  slice would return to being compiled by nothing but the tests.
- **Assert the slice boundary with a generated source file.** Rejected. A copy
  of the declarations proves nothing about the production sources; the fixture
  imports them by path precisely so there is only one definition.

## Implementation references

- [`build_slice_fixture/Cargo.toml`](../../build_slice_fixture/Cargo.toml)
  declares the non-published workspace library.
- [`build_slice_fixture/lib.rs`](../../build_slice_fixture/lib.rs) holds the
  `cfg(not(any(test, doctest)))` gate and the two compiler controls.
- [`build_slice_fixture/slice.rs`](../../build_slice_fixture/slice.rs)
  composes the production module by path.
- [`build.rs`](../../build.rs) declares the slice the fixture mirrors.
- [`tests/build_module_slice_ui_tests.rs`](../../tests/build_module_slice_ui_tests.rs)
  holds the source-only boundary contract.
- [`tests/workflow_contracts/build_slice_fixture_test.py`](../../tests/workflow_contracts/build_slice_fixture_test.py)
  holds the dependency-equality and no-compiler contracts.
- [`tests/env_access_suppressions/roots.rs`](../../tests/env_access_suppressions/roots.rs)
  lists the fixture in `COMPILED_SOURCE_ROOTS`.
- [`.config/nextest.toml`](../../.config/nextest.toml) no longer carries a
  production-module allowance or group membership.
- [`docs/developers-guide.md`](../developers-guide.md) records the module-slice
  boundary and the Nextest budget it now shares.

## References

- [Issue #913](https://github.com/leynos/netsuke/pull/913) repaired nested
  Cargo fixtures by making their preparation cheaper; this record goes further
  for this boundary by removing the compilation from test execution entirely.
- [ADR-0007](adr-0007-publish-the-crates-io-package-as-netsuke-build.md)
  governs the `netsuke-build` package this slice is compiled from.
