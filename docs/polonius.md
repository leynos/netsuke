# Polonius migration notes

Netsuke compiles with the Polonius alpha borrow-checking analysis
(`-Zpolonius=next`) on the dated nightly pinned in `rust-toolchain.toml`.
[ADR-006](adr-006-adopt-polonius-nightly-toolchain.md) records the toolchain
policy; this document records the audit that motivated it, the API
evolutions it enabled, and the refusals that bound it. Issue
[#465](https://github.com/leynos/netsuke/issues/465) tracked the migration.

## Method

The migration ran the `nll-to-polonius` two-pass audit with the compiler as
the oracle:

1. **Workaround scan** — mechanical sweep for local non-lexical-lifetimes
   (NLL) workaround shapes: double lookups, `entry()` with unconditionally
   cloned keys, re-lookup after insert, index-returning finders,
   borrow-killing `drop()` calls, and eager error context.
2. **Design-pressure scan** — structural sweep for owned lookup results,
   id/index indirection, clone-modify-writeback, snapshot-collect loops, and
   per-module clone hotspots.

Every change was compiled twice on `nightly-2026-06-25`: once with
`-Zpolonius=next` (must pass) and once without. The no-flag compile exists
only to classify the individual change: a failure proves the design
genuinely depends on Polonius and the site is tagged `POLONIUS(...)`;
success means the old form was habit rather than necessity and the
improvement carries no toolchain caveat. The complete behavioural test
suite runs under `-Zpolonius=next` — the tree's only supported
configuration — and was required to pass unchanged after every change.

## Polonius-dependent sites

| Site | Tag | Verification |
| --- | --- | --- |
| `src/graph_view/mod.rs` — `NodePathRegistry::ensure_node_mut` | `POLONIUS(case-3)` | Passes with `-Zpolonius=next`; rejected by NLL with E0499 on nightly-2026-06-25 |

`ensure_node_mut` is the get-or-insert accessor behind graph projection: it
returns `&mut NodeKind`, performs a single lookup on the hit path, and
clones the path only on insertion. It replaced three
`entry(path.clone()).or_insert(NodeKind::Source)` sites that cloned every
input, implicit-dependency, and order-only path on every registration. The
`get_mut` loan escapes only via the early return, which is the canonical
Polonius problem-case-3 shape (conditional early return of a borrow).

## Evolutions that compile under both checkers

These came out of the design-pressure scan. Each compiles under plain NLL as
well — the owned style was habit, so they carry no toolchain caveat:

- `src/stdlib/collections.rs` — `group_by_filter` consumed its resolved key
  in `entry(key_value)` instead of cloning it first.
- `src/ir/cycle.rs` — `detect_targets` snapshots borrowed
  `&'targets Utf8Path` keys for its deterministic sort instead of cloning
  every target path per analysis. The snapshot exists for sorting, not to
  end a borrow, so it stays.
- `src/stdlib/which/env.rs` — `EnvSnapshot::resolved_dirs` returns
  `Vec<&Utf8Path>` borrowed from the snapshot; the search loop reads
  borrowed directories and the paths are copied into the owned
  `ResolveError::NotFound` only at the error boundary.

## Refusals

Owned style retained deliberately. The constraint, not the borrow checker,
is load-bearing; each site carries the matching source tag:

| Site | Tag | Constraint |
| --- | --- | --- |
| `src/ir/from_manifest_support.rs` — `register_action` | `POLONIUS-REFUSED(id-is-data)` | The action hash is persistent IR identity: stored on every `BuildEdge` and named in the generated Ninja file. Remains owned unless callers demonstrate a need for the canonical interned value. |
| `src/stdlib/which/cache.rs` — `WhichResolver::try_cache` | `POLONIUS-REFUSED(lock-boundary)` | Cache hits are cloned out of the LRU because references cannot outlive the `MutexGuard`; the resolver is shared across evaluation sites. |
| `src/stdlib/collections.rs` — `GroupedValues::new` | `POLONIUS-REFUSED(miss-dominant)` | First-wins string-key registration almost always inserts, so the owned-key `entry` form pays nothing on the rare hit. |

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
- `build_l10n_audit.rs` — `find_matching_brace` and `find_raw_string_end`
  return byte offsets into source text; the index is the result (a data id),
  not a borrow dodge.
- Test-suite `drop()` calls (environment guards, HTTP fixture teardown) are
  semantic Drop effects, not borrow appeasement.

## Harness consequences

Tooling that rebuilds the crate with its own flags must propagate the
Polonius flag or avoid compiling the crate:

- **trybuild** discards ambient `RUSTFLAGS` and workspace `build.rustflags`,
  replacing them via `--config` on its scratch project, and it always builds
  the host crate as a fixture dependency. The Kani cfg policy fixture is
  therefore compiled and run directly with the workspace `rustc`
  (`tests/kani_cfg_ui_tests.rs`); do not reintroduce trybuild cases that
  depend on the `netsuke` crate while the tree is Polonius-only.
- **Kani** and **Whitaker** run under their own toolchains but read the
  workspace `.cargo/config.toml` or the Makefile `RUSTFLAGS`, so they
  borrow-check with `-Zpolonius=next` and need no special handling.
- **CI setup actions**: `actions-rust-lang/setup-rust-toolchain` exports
  `RUSTFLAGS="-D warnings"` into the job environment when the variable is
  unset, which shadows `.cargo/config.toml` for every later step. The
  workflows therefore pre-set `RUSTFLAGS` (including `-Zpolonius=next`) at
  job level — the action defers to an existing value — and the Makefile
  recipes append `POLONIUS_FLAGS` to any ambient `RUSTFLAGS` as a second
  line of defence. `cargo-llvm-cov` appends its instrumentation flags to
  the ambient value, so coverage inherits the flag from the job
  environment.
- **cargo-mutants** (scheduled, informational) runs through the shared
  `mutation-cargo.yml` workflow, which controls its own environment; if its
  runs regress with E0499 at tagged sites, the shared workflow needs the
  same `RUSTFLAGS` treatment.

## Clone counts

Measured with `rg --count '\.clone\(\)'` over `src/` (tests included where
they live in `src/`):

| Scope | Before | After |
| --- | --- | --- |
| `src/` total | 158 | 151 |
| `src/graph_view/mod.rs` | 17 | 14 |
| `src/ir/cycle.rs` (non-test) | 1 | 0 |
| `src/stdlib/which/env.rs` | 4 | 1 |
| `src/stdlib/collections.rs` | 4 | 3 |

The scanner's clone-modify-writeback section was empty before and after the
migration. The remaining graph_view clones construct owned keys for the two
projection maps and owned metadata — data ownership, not workaround shapes.

## Stabilization

When `-Zpolonius=next` (or its successor) reaches stable Rust:

1. Move `rust-toolchain.toml` to the stabilizing release and delete the
   `[build] rustflags` entry in `.cargo/config.toml` plus the Makefile
   `POLONIUS_FLAGS` variable.
2. Re-declare `rust-version` in `Cargo.toml` at that release.
3. Keep the `POLONIUS(...)` tags: they still explain why the shape exists;
   reword "nightly-only" phrasing in the ADR and guides.

## Anti-regression guidance

The contract for new code and reviews (also summarized in `AGENTS.md` and
the [developers' guide](developers-guide.md)):

- Do not rewrite `POLONIUS(...)` sites into double lookups,
  `entry(key.clone())`, or `contains_key` guards — the direct form is
  intentional and compiler-verified.
- Do not add defensive clones, id indirection, or eager error context to
  satisfy a borrow error without first checking whether the natural
  borrow-returning form compiles under the project toolchain.
- Respect `POLONIUS-REFUSED(...)` tags: the named constraint (identity,
  locks, aliasing, suspension points, thread boundaries) is permanent, and
  "simplifying" those sites into reference-returning forms will not compile
  or will break the design.
- Classify any new borrow-centric API by compiling with and without the
  flag, then record it here.
