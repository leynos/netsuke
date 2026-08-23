# Polonius migration notes

Netsuke compiles with the Polonius alpha borrow-checking analysis, which the
dated nightly pinned in `rust-toolchain.toml` enables by default.
[ADR-006](adr-006-adopt-polonius-nightly-toolchain.md) records the toolchain
policy; this document records the audit that motivated it, the API evolutions
it enabled, and the refusals that bound it. Issue
[#465](https://github.com/leynos/netsuke/issues/465) tracked the migration.

The migration originally ran against an opt-in `-Zpolonius=next` directive.
Nightly toolchains dated 2026-08-04 and later enable Polonius by default, and
the directive is being retired, so the tree passes it nowhere. Historical
references to the flag below describe how a classification was made at the
time, not a build setting anything still applies.

## Method

The migration ran the `nll-to-polonius` two-pass audit with the compiler as the
oracle:

1. **Workaround scan** — mechanical sweep for local non-lexical-lifetimes
   (NLL) workaround shapes: double lookups, `entry()` with unconditionally
   cloned keys, re-lookup after insert, index-returning finders, borrow-killing
   `drop()` calls, and eager error context.
2. **Design-pressure scan** — structural sweep for owned lookup results,
   id/index indirection, clone-modify-writeback, snapshot-collect loops, and
   per-module clone hotspots.

Every change was compiled twice on `nightly-2026-06-25`, where the analysis was
still opt-in: once with `-Zpolonius=next` (must pass) and once without. The
no-flag compile existed only to classify the individual change: a failure
proves the design genuinely depends on Polonius and the site is tagged
`POLONIUS(...)`; success means the old form was habit rather than necessity and
the improvement carries no toolchain caveat. The complete behavioural test
suite runs under Polonius — the tree's only supported configuration — and was
required to pass unchanged after every change.

Classifying a new site the same way now means compiling it against a
pre-2026-08-04 nightly, which is the last configuration that still applies NLL.
`-Zpolonius=legacy` does not restore NLL. That comparison is a one-off
diagnostic, not a build setting: the tree itself is Polonius-only.

## Polonius-dependent sites

There are currently no tagged Polonius-dependent sites.

## Evolutions that compile under both checkers

These came out of the design-pressure scan. Each compiles under plain NLL as
well — the owned style was habit, so they carry no toolchain caveat:

- `src/graph_view/mod.rs` — `NodePathRegistry::ensure_node_mut` uses
  `hashbrown::HashMap::entry_ref` for a borrowed single lookup. It returns
  `&mut NodeKind` and allocates an owned path only for a vacant entry, while
  compiling under both borrow checkers.
- `src/stdlib/collections.rs` — `group_by_filter` consumed its resolved key
  in `entry(key_value)` instead of cloning it first.
- `src/ir/cycle.rs` — `detect_targets` snapshots borrowed
  `&'targets Utf8Path` keys for its deterministic sort instead of cloning every
  target path per analysis. The snapshot exists for sorting, not to end a
  borrow, so it stays.
- `src/stdlib/which/env.rs` — `EnvSnapshot::resolved_dirs` returns
  `Vec<&Utf8Path>` borrowed from the snapshot; the search loop reads borrowed
  directories and the paths are copied into the owned `ResolveError::NotFound`
  only at the error boundary.

## Refusals

Owned style retained deliberately. The constraint, not the borrow checker, is
load-bearing; each site carries the matching source tag:

| Site                                                     | Tag                               | Constraint                                                                                                                                                                                      |
| -------------------------------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/ir/from_manifest_support.rs` — `register_action`    | `POLONIUS-REFUSED(id-is-data)`    | The action hash is persistent IR identity: stored on every `BuildEdge` and named in the generated Ninja file. Remains owned unless callers demonstrate a need for the canonical interned value. |
| `src/stdlib/which/cache.rs` — `WhichResolver::try_cache` | `POLONIUS-REFUSED(lock-boundary)` | Cache hits are cloned out of the LRU because references cannot outlive the `MutexGuard`; the resolver is shared across evaluation sites.                                                        |
| `src/stdlib/collections.rs` — `GroupedValues::new`       | `POLONIUS-REFUSED(miss-dominant)` | First-wins string-key registration almost always inserts, so the owned-key `entry` form pays nothing on the rare hit.                                                                           |

## Non-candidates reviewed and cleared

Scanner suspects that turned out not to be NLL residue:

- `src/ir/from_manifest_support.rs` — the `contains_key`/`insert` guard in
  `register_action` keeps no reference, so it already compiles under NLL
  (write-only guarding); the refusal above covers the owned hash it returns.
- `src/ir/cycle.rs:238` — the doc comment on `visit_known_edge` blaming the
  borrow checker describes the `'targets` borrow discipline accurately and
  needs no Polonius caveat.
- `src/cli/merge.rs` — clones construct the resolved `Cli` from borrowed
  layers; owned construction of a new value, not clone-modify-writeback.
- `build_l10n_audit/` — the audit is split by input kind: `keys.rs` reads the
  `define_keys!` macro, `scanner.rs` holds the byte-level scanner it drives,
  `ftl.rs` parses catalogues, `metadata.rs` reads the Cargo metadata, and
  `compare.rs` holds the comparison rules. The scanner works in byte positions
  into borrowed source text — `find_matching_brace` in `keys.rs` and
  `find_raw_string_end` in `scanner.rs` both return byte offsets. The index is
  the result (a data id), not a borrow dodge.
- Test-suite `drop()` calls (environment guards, HTTP fixture teardown) are
  semantic Drop effects, not borrow appeasement.

The toolchain policy is contract-tested: `tests/polonius_toolchain_contract.rs`
requires the pinned channel to be a dated nightly at or after 2026-08-04, fails
if any build configuration reintroduces a `-Zpolonius` directive, and pins the
shared-action `with.rustflags` and toolchain inputs in the CI, Netsukefile,
coverage, and packaging workflows. The `RUSTFLAGS` shape of each Makefile
recipe is covered separately by `tests/makefile_test_target.rs`.

## Harness consequences

Because the analysis rides on the toolchain rather than on a flag, tooling only
has to use the pinned toolchain; nothing needs to propagate a build setting:

- **trybuild** discards ambient `RUSTFLAGS` and workspace `build.rustflags`,
  replacing them via `--config` on its scratch project, and it always builds
  the host crate as a fixture dependency. While Polonius was flag-gated that
  broke every fixture depending on `netsuke`, so the Kani cfg policy fixture is
  compiled and run directly with the workspace `rustc`
  (`tests/kani_cfg_ui_tests.rs`). That specific hazard is gone, but trybuild
  still needs a scratch project and a toolchain-sensitive `.stderr` snapshot,
  so the direct-compile harnesses stay.
- **Whitaker** runs its Dylint driver on a nightly of its own and needs no
  Polonius-specific handling.
- **Kani** manages a supporting nightly during `cargo kani setup`, and that
  nightly can be older than the repository's. Kani 0.67.0 uses
  `nightly-2025-11-21`, which predates the Polonius default, so `make
  kani-full` borrow-checks under NLL. With no tagged sites in the tree this
  costs nothing today; should a `POLONIUS(...)` site fail to verify, move Kani
  to a build whose nightly is 2026-08-04 or later rather than reinstating a
  `-Zpolonius` directive.
- **CI setup actions**: the shared `setup-rust` and `rust-build-release`
  actions export their own `RUSTFLAGS`, so anything a job needs travels through
  their `with.rustflags` inputs and workflows must not set a job-level
  `env.RUSTFLAGS`. CI and coverage pass `-D warnings`; Netsukefile tests and
  packaging pass no `rustflags` at all. The coverage action's `cargo-llvm-cov`
  invocation inherits the flags exported by `setup-rust` and appends its
  instrumentation flags. The per-workflow values, the `NETSUKE_RUST_TOOLCHAIN`
  policy and the reason the contract test pins each action's exact revision are
  set out in the developer guide under [Polonius CI shared-action
  contract](developers-guide.md#polonius-ci-shared-action-contract).
- **Registry installs**: the crates.io package excludes `rust-toolchain.toml`,
  and registry builds run outside the checkout, so `cargo install
  netsuke-build` must select the pinned nightly explicitly
  (`cargo +nightly-2026-08-23 install netsuke-build`). The README and users'
  guide document the command and `tests/documentation_installation_tests.rs`
  pins it.
- **cargo-mutants** (scheduled, informational) runs through the shared
  `mutation-cargo.yml` workflow, which controls its own environment; if those
  runs regress with E0499 at tagged sites, check that the shared workflow uses
  the pinned toolchain.

## Clone counts

Measured with `rg --count '\.clone\(\)'` over `src/` (tests included where they
live in `src/`):

| Scope                        | Before | After |
| ---------------------------- | ------ | ----- |
| `src/` total                 | 158    | 151   |
| `src/graph_view/mod.rs`      | 17     | 14    |
| `src/ir/cycle.rs` (non-test) | 1      | 0     |
| `src/stdlib/which/env.rs`    | 4      | 1     |
| `src/stdlib/collections.rs`  | 4      | 3     |

The scanner's clone-modify-writeback section was empty before and after the
migration. The remaining graph_view clones construct owned keys for the two
projection maps and owned metadata — data ownership, not workaround shapes.

## Stabilization

The flag plumbing is already gone; the analysis is the nightly default. When it
reaches stable Rust:

1. Move `rust-toolchain.toml` to the stabilizing release, and relax the
   dated-nightly lower bound in `tests/polonius_toolchain_contract.rs`.
2. Re-declare `rust-version` in `Cargo.toml` at that release.
3. Keep the `POLONIUS(...)` tags: they still explain why the shape exists;
   reword "nightly-only" phrasing in the ADR and guides.

## Anti-regression guidance

The contract for new code and reviews (also summarized in `AGENTS.md` and the
[developers' guide](developers-guide.md)):

- Do not rewrite `POLONIUS(...)` sites into double lookups,
  `entry(key.clone())`, or `contains_key` guards — the direct form is
  intentional and compiler-verified.
- Do not add defensive clones, id indirection, or eager error context to
  satisfy a borrow error without first checking whether the natural
  borrow-returning form compiles under the project toolchain.
- Respect `POLONIUS-REFUSED(...)` tags: the named constraint (identity,
  locks, aliasing, suspension points, thread boundaries) is permanent, and
  "simplifying" those sites into reference-returning forms will not compile or
  will break the design.
- Classify any new borrow-centric API by compiling it against a pre-2026-08-04
  nightly as well as the pinned one, then record it here.
