# Architectural decision record (ADR) 049: Gate command-schema extraction

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

The current [build script][build-script] compiles a deliberately selected slice
of CLI schema, host-pattern syntax, and localisation code to generate manual
pages and completions and audit catalogue keys. Its explicit file selection
excludes runtime discovery and other application behaviour.

A `netsuke-cli-schema` crate could replace that source-inclusion arrangement,
but a tidier import graph is not by itself evidence of lower build cost.
Host build dependencies and target runtime dependencies can still occupy
separate compilation contexts. Extracting a crate does not promise one
compilation of the schema for every workflow.

## Decision drivers

- Preserve one authoritative public command tree and metadata contract.
- Keep runtime configuration discovery out of build-time generation.
- Avoid an application dependency cycle through `build.rs`.
- Measure host and target compilation separately under unchanged toolchain,
  feature, and resource settings.

## Proposed direction

Retain the selected source slice initially. Consider `netsuke-cli-schema` only
when the benchmark identifies material build-script/schema cost, an actual
independent schema consumer needs a stable library boundary, or repeated drift
between consumers demonstrates a concrete ownership defect.

A proposed extraction must contain declarative command/schema information and
only the validation/metadata required to construct it. It may depend on the
necessary Clap and configuration-derive interfaces, but must not perform
configuration discovery, environment probing, process execution, template
loading, or application dispatch. Inventory unavoidable dependencies explicitly
rather than calling a configuration-heavy schema library lightweight.

Both runtime parsing and build-time generation must consume the same command
contract. Preserve defaults, selector handling, localized help, completion
content, manual generation, and the release-help metadata source of truth in
[ADR-016][metadata]. Coordinate generated identifiers and documentation IR
adoption with [RFC 0032][localisation]; do not copy their rules into this crate.

The application and its build script may depend on the schema crate. The schema
crate must never depend on `netsuke-build` or import application source through
a back door. Keep product catalogue data and explicit localizer construction at
the boundary established by RFC 0032; do not initialize global locale state.

A new schema package in the build dependency closure needs the same registry
publication preparation as a runtime component under [RFC 0035][release].

## Alternatives considered

Immediate extraction could introduce another host/target package without
removing work. Copying separate build-time and runtime schema definitions would
create drift. Importing the entire CLI subtree into the build script would
undo the current exclusion of runtime facilities.

## Consequences

The existing source-inclusion arrangement remains until evidence justifies a
replacement. Maintainers must continue to respect its explicit small slice.
An eventual extraction can improve ownership even when duplicate host/target
compilation remains, but its measured trade-off must be stated honestly.

## Verification and revisit criteria

Use [RFC 0030][benchmark] to compare no-op builds, command-schema edits,
catalogue edits, and representative CLI implementation edits. Retain host and
target timings, rebuilt units, memory, and package graph evidence.

Before accepting extraction, demonstrate equivalent command-tree and metadata
snapshots, generated manuals/completions, default-command behaviour, and catalogue
audits. Include a negative dependency fixture proving that the schema cannot
acquire application/runtime discovery. Test clean package consumers and relevant
cross-compilation contexts instead of inferring them from a native build.

Reopen after a concrete consumer, measured bottleneck, or documented drift case.
Otherwise retain the current arrangement and record the decision in the
[decomposition roadmap][roadmap], without creating an empty package.

## Outstanding decisions

The implementation review decides the exact schema surface and whether its
benefit is compilation, ownership, or both. Registry naming and public API
stability remain subject to the component release policy.

[build-script]: ../build.rs
[metadata]: adr-016-public-cli-metadata-source-of-truth.md
[localisation]: rfcs/0032-localisation-crate-boundary.md
[release]: rfcs/0035-lading-backed-workspace-publication.md
[benchmark]: rfcs/0030-build-performance-benchmark-extension.md
[roadmap]: roadmap-crate-decomposition.md
