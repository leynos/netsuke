# Architecture Decision Record (ADR): Render the `graph` subcommand in-process

## Status

Accepted

Accepted: the `graph` subcommand renders the build dependency graph
in-process from the parsed manifest's intermediate representation; it does
not invoke `ninja -t graph`.

## Date

2026-05-26

## Context and problem statement

Through milestone 3.4.4 the `netsuke graph` subcommand worked by:

1. Running the full manifest pipeline up to Ninja synthesis to produce a
   temporary `build.ninja`.
2. Invoking the Ninja executable with `ninja -t graph` against that
   temporary file.
3. Streaming Ninja's stdout (the DOT text) back to the user.

Milestone 3.4.5 added an `--html` renderer and an `--output <FILE>` flag.
Two implementation paths were available:

- Keep the existing `ninja -t graph` path for DOT and add a separate
  in-process HTML path.
- Migrate both DOT and HTML to a shared in-process renderer port that
  consumes a deterministic projection of the in-memory build graph.

The first path leaves two DOT emitters live with subtly different output
(Ninja's DOT references the temporary build-file path, which is a
determinism risk). It also keeps a Ninja runtime dependency on the
introspection path even though Ninja contributes nothing semantic that the
IR does not already carry.

## Decision

Migrate the entire `graph` dispatch off Ninja. Render DOT and HTML
in-process from a canonical `GraphView` projection of `BuildGraph`. Both
renderers consume `GraphView` through a single `GraphRenderer` trait. The
runner picks the appropriate adapter from `Commands::Graph(GraphArgs)`,
writes through the shared `write_text_file` / `write_text_stdout` sinks,
and honours the `-` sentinel and `-C/--directory` working directory in the
same way as `manifest`.

Specifically:

- A `GraphView` (in [`src/graph_view/mod.rs`](../src/graph_view/mod.rs))
  sorts every collection at construction time so the output is invariant
  under `HashMap` iteration order. A proptest confirms equivalence under
  reversed insertion order across freshly seeded `HashMap`s.
- A `GraphRenderer` port (in [`src/graph_view/render.rs`](../src/graph_view/render.rs))
  defines the contract `render(&self, view: &GraphView, sink: &mut dyn
  io::Write) -> Result<(), GraphRenderError>`.
- [`DotRenderer`](../src/graph_view/render_dot.rs) and
  [`HtmlRenderer`](../src/graph_view/render_html/mod.rs) implement that
  port. The HTML adapter is split across focused modules under
  [`src/graph_view/render_html/`](../src/graph_view/render_html/).
- The runner's [`Commands::Graph` dispatch](../src/runner/mod.rs) no longer
  spawns `ninja -t graph`. Tests that previously asserted the Ninja-tool
  dispatch have been updated.

## Rationale

- **Determinism.** The IR is the natural source of truth for the
  dependency graph. Running through Ninja introduces a temporary build
  file whose path leaks into Ninja's DOT output, which makes Ninja's
  output non-reproducible. Sorting at the projection boundary is much
  simpler than post-processing Ninja's stream.
- **Operational fitness.** `graph` is a lightweight introspection
  subcommand. It has no semantic reason to fail when Ninja is unavailable
  or misconfigured. Running in-process removes Ninja as a runtime
  dependency on the introspection path. Integration tests cover the
  Ninja-less path explicitly.
- **Architectural fit.** The hexagonal projection cleanly admits future
  renderers — for example the deferred `--json` view (roadmap item
  `3.15.6`) — as a third adapter without re-projecting the IR.
- **Pre-0.1.0 freedom.** ADR-003 endorses removing legacy spellings rather
  than adding compatibility aliases. The byte-level change in DOT output
  is acceptable because no stable contract exists yet.

## Consequences

- Downstream scripts that grep the previous `ninja -t graph` output may
  see syntactically different DOT. The semantic content (targets, edges,
  dependency classes) is preserved; the formatting and node identifiers
  follow Netsuke's own conventions. Documented in the user guide.
- The runner-tool-subcommand integration tests were rewritten to cover
  the new in-process behaviour for `graph` while retaining the
  Ninja-mediated behaviour for `clean`.
- `--html` and `--output` are intentionally kept out of `OrthoConfig`
  layering. Layering `--output` through a config file would silently
  change the artefact destination; this is a per-invocation flag only.

## Alternatives considered

- **Add an HTML renderer alongside the existing Ninja-mediated DOT
  path.** Rejected. Two DOT emitters with subtly different output is
  worse than one switch, and the parallel maintenance burden is
  permanent.
- **Adopt a third-party Rust layout crate (`layout` or similar) for
  HTML.** Evaluated and rejected during the Stage C go/no-go gate (see
  [`docs/execplans/3-4-5-extend-graph-subcommand-with-an-html-renderer.md`](execplans/3-4-5-extend-graph-subcommand-with-an-html-renderer.md)).
  The crate's transitive deps and binary-size impact exceeded the 1 MB
  release-binary budget.
- **Vendor `@viz-js/viz` (JS + WASM).** Documented fallback. Rejected
  because the 3 MB per-artefact WASM blob exceeds the 5 MB realistic-
  project HTML ceiling on large graphs.

## Implementation references

- Execplan: [`docs/execplans/3-4-5-extend-graph-subcommand-with-an-html-renderer.md`](execplans/3-4-5-extend-graph-subcommand-with-an-html-renderer.md)
- Production code: [`src/graph_view`](../src/graph_view), runner
  dispatch in [`src/runner/mod.rs`](../src/runner/mod.rs).
- Tests: [`src/graph_view/tests.rs`](../src/graph_view/tests.rs),
  [`src/graph_view/render_html/tests.rs`](../src/graph_view/render_html/tests.rs),
  [`tests/runner_graph_tests.rs`](../tests/runner_graph_tests.rs),
  [`tests/features_unix/graph.feature`](../tests/features_unix/graph.feature).
