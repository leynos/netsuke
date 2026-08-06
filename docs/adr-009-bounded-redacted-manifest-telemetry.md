# Architecture decision record (ADR): Bounded, redacted telemetry for manifest evaluation

## Status

Accepted.

## Date

2026-08-06.

## Context and problem statement

Manifest evaluation renders Jinja templates and invokes manifest-defined
macros. Both are queries: they compute a value and were expected to stay free
of ambient concerns such as timing and metric emission. Once observability was
needed for these paths, two risks had to be weighed against each other:

- Manifest content — template text, macro names, macro arguments, and context
  values — is caller-controlled and unbounded. Recording it directly in a
  metric label produces unbounded cardinality in the metric series, and
  recording it in a trace risks leaking secrets, because environment variable
  names routinely identify credentials (`src/manifest/env_reader.rs` already
  applies this rule to `env()` lookup failures).
- Interleaving spans and metric emission with the evaluation logic in
  `render_template` and the macro-invocation callback would make those
  functions read as instrumentation rather than plain evaluation, and would
  scatter the redaction contract across every call site instead of collecting
  it in one place a reviewer can audit.

`AGENTS.md` also constrains the shape of the answer: libraries may emit
`metrics` and `tracing` instrumentation but must not install global recorders
or subscribers; only the application initializes those once, at startup.

## Decision

Collect manifest telemetry in `src/manifest/jinja_macros/telemetry.rs`, kept
separate from evaluation, with two distinct instrumentation boundaries:

- **Template render** — `manifest::jinja_macros::render_template` composes
  `telemetry::instrument_template_render`, which wraps the render in the
  `manifest.template.render` span, increments the
  `netsuke_manifest_template_renders_total` counter, and records the
  `netsuke_manifest_template_render_duration_seconds` histogram.
- **Macro invocation** — the compiled-expression fallback built by
  `make_macro_fn` (`src/manifest/jinja_macros/invocation.rs`) composes
  `telemetry::instrument_macro_invocation`, which wraps the invocation in the
  `manifest.macro.invoke` span, increments the
  `netsuke_manifest_macro_invocations_total` counter, and records the
  `netsuke_manifest_macro_invocation_duration_seconds` histogram.

Macros reached through a template import are metered only at the render
boundary; the macro-invocation counter covers the compiled-expression fallback
only, because imports evaluate inside the render call and never reach
`make_macro_fn`. `macro_invocation_telemetry.rs`'s
`imported_macro_render_does_not_emit_invocation_metrics` test pins this split.

The label and field vocabulary is bounded by construction:

- `outcome` is always `"success"` or `"error"`.
- The render boundary adds `has_macro_imports`, `"true"` or `"false"`.
- On failure, both boundaries add `error_category`, the `Debug` form of
  `minijinja::ErrorKind`, never the error's `Display` text, which can embed
  manifest content.

Template text, macro names, macro arguments, context values, and environment
variable names never reach a span field or a metric label.

## Rationale

- **Queries stay pure.** Keeping instrumentation in one module lets
  `render_template` and the macro-invocation callback compose an
  instrumentation boundary explicitly rather than interleave timing and metric
  code with evaluation.
- **One place to audit the privacy contract.** Every field emitted by
  `telemetry.rs` is enumerated in that module, so confirming that manifest
  content cannot reach a subscriber is a single, small review rather than an
  audit of every call site.
- **Two boundaries, not one.** Template rendering and macro invocation are
  different operations with different failure shapes and different call
  patterns (one render can invoke many macros); merging them into a single
  counter would hide the render/invocation split that
  `imported_macro_render_does_not_emit_invocation_metrics` depends on.
- **Bounded labels prevent cardinality blowups.** `outcome`,
  `has_macro_imports`, and `error_category` are drawn from small, fixed
  vocabularies, so the metric series stays bounded regardless of how many
  distinct manifests, macros, or templates are evaluated.
- **`error_category` uses `Debug`, not `Display`.** `minijinja::ErrorKind`'s
  `Debug` form is a fixed enum variant name; the `Display` text of a
  `minijinja::Error` can embed manifest content such as variable names.
- **Matches the existing environment-name redaction rule.** `env_var_with` in
  `src/manifest/env_reader.rs` already omits the variable name from both
  tracing and the returned Jinja error, for the same reason: manifest-supplied
  names routinely identify credentials.

## Consequences

- New telemetry fields for these two boundaries must be added to
  `telemetry.rs` and reviewed against the redaction contract before merging;
  they must not be added ad hoc at the render or invocation call sites.
- `src/manifest/tests/macros_telemetry.rs` and
  `src/manifest/tests/macro_invocation_telemetry.rs` pin the counter and
  histogram names, the bounded label vocabulary, and the render/invocation
  boundary split using a local `metrics_util::debugging::DebuggingRecorder` and
  the workspace's tracing capture helper, so neither the global recorder nor
  the global subscriber is touched by the test suite.
  `macro_invocation_telemetry.rs` also runs a proptest,
  `macro_telemetry_stays_bounded_for_arbitrary_macros`, asserting the redaction
  contract for arbitrary generated macro names, arguments, and
  undefined-variable names, not only the fixed sentinel cases.
- The counter and histogram names
  (`netsuke_manifest_template_renders_total`,
  `netsuke_manifest_template_render_duration_seconds`,
  `netsuke_manifest_macro_invocations_total`,
  `netsuke_manifest_macro_invocation_duration_seconds`) are a published
  contract for anything scraping these metrics; renaming them is a breaking
  change.
- Because the module only calls `metrics` and `tracing` macros and never
  installs a recorder or subscriber, the application remains free to choose or
  omit an exporter at startup without any change to manifest evaluation.

## Alternatives considered

- **Instrument inline in the evaluation path.** Rejected. Interleaving spans
  and metric emission directly inside `render_template` and the macro-callback
  closure would make the redaction contract implicit at each call site instead
  of reviewable in one module, and would make `render_template` read as
  instrumentation-plus-evaluation rather than plain evaluation composed with an
  explicit instrumentation boundary.
- **Inject a telemetry-sink trait.** Rejected. The `metrics` crate already
  provides that abstraction: `counter!`/`histogram!`/`describe_*!` route
  through whatever recorder the application installs, and `tracing` provides
  the equivalent for spans and events through the subscriber. A bespoke sink
  trait would duplicate that abstraction while still needing the same redaction
  discipline, and `AGENTS.md` already forbids this module from installing a
  recorder or subscriber itself.
- **One shared counter and histogram for both boundaries.** Rejected. Merging
  render and invocation telemetry would erase the render/invocation split that
  distinguishes the import path from the compiled-expression fallback, making
  it impossible to answer "did this render's macro imports run, or fall back to
  compiled-expression invocation?" from the metrics alone.

## Implementation references

- Telemetry module:
  [`src/manifest/jinja_macros/telemetry.rs`](../src/manifest/jinja_macros/telemetry.rs)
- Render boundary composition:
  [`src/manifest/jinja_macros/mod.rs`](../src/manifest/jinja_macros/mod.rs)
- Macro-invocation boundary composition:
  [`src/manifest/jinja_macros/invocation.rs`](../src/manifest/jinja_macros/invocation.rs)
- Matching environment-name redaction rule:
  [`src/manifest/env_reader.rs`](../src/manifest/env_reader.rs)
- Tests:
  [`src/manifest/tests/macros_telemetry.rs`](../src/manifest/tests/macros_telemetry.rs),
  [`src/manifest/tests/macro_invocation_telemetry.rs`](../src/manifest/tests/macro_invocation_telemetry.rs)
- Developer guide:
  [`docs/developers-guide.md`](developers-guide.md#manifest-telemetry-template-render-and-macro-invocation)
