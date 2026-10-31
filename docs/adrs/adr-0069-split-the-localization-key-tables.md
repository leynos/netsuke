# Architectural decision record (ADR) 0069: Split the localization key tables

## Status

Accepted. The Fluent key set is declared across three tables —
`src/localization/keys.rs`, `src/localization/check_keys.rs`, and
`src/localization/clap_error_keys.rs` — which the build audit reads as one key
set. The public `keys::ALL_KEYS` aggregate keeps its `&'static [&'static str]`
type and is assembled at compile time.

## Date

2026-10-31.

## Context and problem statement

Every Fluent message key was declared by one `define_keys!` invocation in
`src/localization/keys.rs`, and that file also published `ALL_KEYS`, the slice
listing every declared key. Localizing clap's `TooManyValues` diagnostic
([issue #957]) added seven `clap-error-*` keys and pushed the file from 400 to
401 raw lines.

Whitaker's `module-max-lines` lint caps a module at 400 raw lines. The cap is a
readability boundary rather than an arbitrary number: the file had reached it
because one table was serving three audiences — general CLI and diagnostic
messages, the `netsuke check` rule prose, and clap's own error rendering.

The key set also has a second reader. `build_l10n_audit` parses every
`define_keys!` invocation to compare declared keys against all 35 catalogues,
and it must see the whole set whichever file declares a key. A split therefore
has to move keys without moving the boundary the audit reads.

## Decision drivers

- The line cap is a real constraint, so a change that exceeds it must shrink a
  module rather than silence the lint.
- The public key namespace must not move: every call site spells keys
  `keys::NAME`, and `keys::ALL_KEYS` is public API.
- The audit must keep rejecting a catalogue that is missing a declared key, an
  orphaned key, or a mismatched interpolation variable, no matter which table
  declares the key.
- A key declared in two tables is a mistake that has to fail the build, not
  silently produce a duplicate.

## Options considered

### Option A: Suppress the line cap for `keys.rs`

Rejected. The file had already reached the boundary for the reason the boundary
exists, and a suppression buys one release before the next key family arrives.

### Option B: Move the clap keys into the existing sibling file

Rejected. There is one sibling, `check_keys.rs`, which belongs to
`netsuke check`; mixing clap's error rendering into it would repeat the
original mistake at a smaller scale.

### Option C: Generate the key tables from the catalogues at build time

Rejected. The constants are referenced as `keys::NAME` in code that the build
script compiles before the audit runs, so the tables have to exist as source.

### Option D: Split the tables by audience and assemble the aggregate at compile time

Chosen. Each table keeps its own `define_keys!` invocation, and the union is
derived from them rather than written out by hand.

## Decision outcome

Each table keeps its own `define_keys!` invocation, and the macro publishes a
`TABLE_KEYS` slice beside the constants it declares. The union lives in
`src/localization/mod.rs`, where a `const fn` copies the three slices into an
array whose length is their summed length, and `ALL_KEYS` is that array as a
`&'static [&'static str]`. `keys.rs` re-exports its two siblings and the
aggregate, so `keys::NAME` and `keys::ALL_KEYS` remain the names callers use.

`build_l10n_audit::KEY_SOURCES` lists all three files, and the audit parses
them as one set: a key declared twice is an error regardless of which pair of
files declares it.

## Consequences

- Adding a key is unchanged: declare it in the table that owns its audience and
  add it to every catalogue.
- The union is derived rather than written out, so a key cannot be in a table
  and missing from the aggregate.
- `ALL_KEYS` is assembled at compile time rather than on first use. The
  alternative, a `LazyLock<Vec<_>>`, would have changed the published type from
  the slice the single table used to publish; a `const _: &[&str] = ALL_KEYS;`
  item now fails the build if the type drifts.
- `slice::get` and `slice::get_mut` are not yet `const`, so the copy loop
  indexes. That one loop carries a narrowly scoped
  `#[expect(clippy::indexing_slicing)]`; the destination length is the exact
  sum of the tables copied into it, so every index is in bounds.
- The audit, the `rerun-if-changed` directives in `build.rs`, the packaging
  smoke test's required-file list, and the documentation enumerations all name
  three key sources where they named one. A future table split has to extend
  each of them.
- `src/localization/keys.rs` sits close to the cap once more, so the next key
  family that is large enough should get its own table rather than being
  appended to the general one.

## Architectural rationale

The cap and the audit pull in opposite directions: the cap wants files broken
up, while the audit wants one key set. Publishing a per-table slice from the
macro, and deriving the union from those slices, satisfies both without a
hand-maintained list that could drift. Deriving the union at compile time
rather than at first use keeps the split invisible to callers, which is what
makes it a refactor rather than an API change.

## References

- [Issue #957](https://github.com/leynos/netsuke/issues/957) added the
  `clap-error-*` keys that crossed the cap.
- [`src/localization/mod.rs`](../../src/localization/mod.rs) assembles
  `ALL_KEYS` and holds the module-cap guidance for the tables.
- [`src/localization/keys.rs`](../../src/localization/keys.rs) re-exports the
  sibling tables and the aggregate.
- [`build_l10n_audit/mod.rs`](../../build_l10n_audit/mod.rs) lists the three
  key sources the audit reads.
- [`tests/build_l10n_audit_tests.rs`](../../tests/build_l10n_audit_tests.rs)
  mutates each table in turn and asserts the audit rejects the result.
- [`tests/build_l10n_keys_tests.rs`](../../tests/build_l10n_keys_tests.rs)
  runs the key-extraction parser over each of the three real tables.
- [`tests/packaging_smoke_tests.rs`](../../tests/packaging_smoke_tests.rs)
  requires all three tables to ship in the published package, because the build
  script reads each of them at build time downstream.
- [`docs/developers-guide.md`](../developers-guide.md), section "Adding or
  changing messages", states the contributor practice.

[issue #957]: https://github.com/leynos/netsuke/issues/957
