# Architecture Decision Record (ADR): Use a typed `which` resolver error

## Status

Accepted.

## Date

2026-06-12.

## Context and problem statement

Roadmap item 3.14.4 promotes `command_available(name, **kwargs)` into a
non-throwing executable probe for manifest-time `when` clauses. The original
implementation reused the `which` resolver but detected absence by converting
the resolver failure into a `minijinja::Error` and then string-matching the
rendered diagnostic for `netsuke::jinja::which::not_found`.

That coupling made the predicate depend on user-visible message text. It also
put an error-classification decision at the MiniJinja registration boundary,
even though absence is a resolver-domain result.

## Decision

Keep executable lookup classification inside `src/stdlib/which/` with an
internal typed resolver error. The resolver returns
`Result<Vec<Utf8PathBuf>, ResolveError>`. The MiniJinja boundary converts
`ResolveError` into the existing user-facing diagnostics for `which`, while
`command_available` pattern-matches the typed absence variants and returns
`false` for those cases only.

`command_available` therefore treats command absence as data and still treats
argument misuse, canonicalisation failures, workspace encoding failures, and
current-directory failures as errors.

## Rationale

- **Message stability.** The resolver no longer classifies absence by matching
  localized diagnostic text, so copy edits cannot change predicate semantics.
- **Hexagonal boundary.** Lookup classification belongs to the resolver port.
  Manifest, AST, IR, Ninja, and CLI layers receive only selected manifest
  entries and must not inspect resolver diagnostics.
- **Shared resolver.** `which` and `command_available` use the same search,
  cache, `cwd_mode`, workspace fallback, and `PATHEXT` behaviour.
- **Preserved diagnostics.** `From<ResolveError> for minijinja::Error`
  preserves the existing diagnostic codes and rendered messages for the throwing
  `which` filter/function.
- **Non-throwing absence.** Treating the no-place-to-search case as absence
  aligns the predicate with comparable build-system probes such as `-NOTFOUND`,
  `None`, or `.found() == false`.

## Consequences

- Every resolver callsite inside `src/stdlib/which/` consumes a typed result.
  New resolver error cases must be explicitly handled by both the MiniJinja
  conversion and the predicate absence helper.
- The typed error remains internal. No public Rust API outside the `which`
  module exposes it.
- Future stdlib helpers should copy this pattern when they need both a
  throwing filter/function and a non-throwing predicate over the same resolver.
- Diagnostic snapshots are the guardrail for accidental user-visible error
  drift.

## Alternatives considered

- **Keep string-matching `minijinja::Error`.** Rejected because it makes
  predicate behaviour depend on localized message text and diagnostic-code
  formatting.
- **Expose the typed error publicly.** Rejected because 3.14.4 needs an
  internal resolver contract, not a new crate API surface.
- **Create a separate predicate resolver.** Rejected because duplicating lookup
  logic would split cache behaviour, workspace fallback handling, and
  platform-specific path semantics between two implementations.

## Implementation references

- Execplan:
  [`docs/execplans/3-14-4-command-available-non-throwing-executable-probe.md`](execplans/3-14-4-command-available-non-throwing-executable-probe.md)
- Resolver error:
  [`src/stdlib/which/resolve_error.rs`](../src/stdlib/which/resolve_error.rs)
- MiniJinja registration:
  [`src/stdlib/which/mod.rs`](../src/stdlib/which/mod.rs)
- User-facing contract: [`docs/users-guide.md`](users-guide.md)
