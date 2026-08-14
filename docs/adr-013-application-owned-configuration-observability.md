# Architecture decision record (ADR): Application-owned configuration observability

## Status

Accepted.

## Date

2026-08-14

## Context and problem statement

Netsuke needs configuration-load latency and outcome measurements for startup
diagnostics. Configuration resolution is otherwise a query: it resolves the
effective JSON mode and merges the configuration layers. Adding recorder
installation or metric emission inside those query functions would couple
configuration behaviour to process-wide observability and make the boundaries
harder to reuse and test.

The application also needs to preserve two output guarantees. Human users can
request verbose startup diagnostics, while JSON users must receive exactly one
machine-readable diagnostic document on stderr. Configuration paths, values,
and formatted source errors are unbounded or sensitive and must not become
metric labels.

## Decision

Compose configuration observability at the CLI composition root. The query
functions `cli::resolve_merged_json` and `cli::merge_with_config` do not install
a recorder or own configuration-load metrics. `run_with_args` wraps those
queries, and `src/observability.rs` owns the phase-level vocabulary and
classification helpers.

Both aggregate and phase-level configuration-load timing receive the same
`&impl monotony::MonotonicClock` seam. Production supplies
`monotony::StdMonotonicClock`; tests use deterministic clocks from
`monotony::test_util`. No boundary defines a local
`ConfigurationLoadClock` or `SystemConfigurationLoadClock`, and no boundary
calls `Instant::now` directly. Netsuke selects `monotony = "0.1.0"`; its
public contract keeps the production clock abstraction dependency-free while
its optional `test-util` feature provides deterministic test clocks.

The application installs one in-process
`metrics_util::debugging::DebuggingRecorder` after tracing starts. Netsuke does
not open a metrics listener or install a network exporter as a side effect of a
command invocation. A verbose run emits the recorder's aggregate
`metrics snapshot` debug event after command completion. A configuration-load
failure reached with CLI `--verbose` emits it before exiting. JSON mode
suppresses tracing and the snapshot so stderr remains the single diagnostic
document.

The metric contract has two layers:

- Phase-level `config_load_total` uses `phase=diag_mode|merge` and
  `outcome=success|failure`; `config_load_duration_seconds` uses only the
  bounded `phase` label.
- Startup-attempt `netsuke_config_load_total` uses only
  `outcome=success|failure`; `netsuke_config_load_duration_seconds` has no
  labels.

No metric label may contain a path, configuration value, formatted error, or
other caller-controlled text. Human configuration errors retain an
`operation` from the fixed set `diag_mode_resolution|config_merge` and an
`error_category` from `io|parse|validation` in structured tracing; detailed
source errors remain in the user-facing diagnostic path.

## Rationale

- **Plain queries remain reusable.** The composition root can measure startup
  without making configuration resolution depend on a global recorder.
- **Application ownership is explicit.** The binary controls recorder lifetime
  and output policy; libraries and query modules do not install process-wide
  state.
- **Cardinality stays bounded.** All metric labels and structured error fields
  come from fixed vocabularies, regardless of the selected path or error text.
- **Output streams remain parseable.** Buffered startup diagnostics and JSON
  mode suppress tracing before the diagnostic document is written.

## Consequences

Contributors adding configuration-load measurements must compose them at the
startup boundary and extend the bounded vocabulary deliberately. Tests must
use a local recorder through `metrics::with_local_recorder`; they must not
depend on the application's process-wide recorder.

The verbose snapshot is a debugging aid, not a stable scrape protocol. The
metric names and label sets are the stable contract; the in-process snapshot
may include both phase-level and startup-attempt entries.

## Alternatives considered

- **Instrument the query functions directly.** Rejected because it couples
  configuration resolution to ambient process observability and distributes
  the label-redaction contract across query call sites.
- **Install a Prometheus listener by default or through configuration.**
  Rejected because a short-lived CLI command cannot guarantee that a scraper
  observes it, and opening a network listener is outside startup configuration
  loading's responsibility.
- **Use paths or source errors as labels.** Rejected because caller-controlled
  values create unbounded cardinality and can expose sensitive information.

## Implementation references

- Composition root and startup metrics:
  [`src/main.rs`](../src/main.rs)
- Phase-level observability:
  [`src/observability.rs`](../src/observability.rs)
- Configuration query orchestration:
  [`src/cli/diag.rs`](../src/cli/diag.rs) and [`src/cli/merge.rs`](../src/cli/merge.rs)
- Design narrative:
  [`docs/netsuke-design.md`](netsuke-design.md)
- Metric names, phase boundaries, and local-recorder testing:
  [`docs/developers-guide.md`](developers-guide.md#configuration-observability)
- User-visible verbose output:
  [`docs/users-guide.md`](users-guide.md#diagnose-configuration-selection)
