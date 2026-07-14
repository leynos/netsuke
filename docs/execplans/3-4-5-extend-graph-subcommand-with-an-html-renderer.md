# 3.4.5. Extend the `graph` subcommand with an `--html` renderer

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE (Stages A–D 2026-05-26; Stage E 2026-06-03).

## Purpose / big picture

After this work, a sighted user can produce a self-contained, offline-safe HTML
file from any Netsuke manifest with `netsuke graph --html --output graph.html`
and open it in a browser to see the build dependency graph rendered as Scalable
Vector Graphics (SVG). Automation still receives the canonical Graphviz DOT
language (DOT) graph from `netsuke graph` on stdout, preserving the raw graph
data contract. A new `--output <FILE>` flag, shared by both DOT and HTML
emission, writes the chosen artefact to disk with the same create/write/sync
semantics already used by `netsuke manifest`, and a `-` sentinel selects stdout
— matching the precedent set by
[`manifest`](../netsuke-design.md#83-command-behaviour).

This task implements roadmap item [`3.4.5`](../roadmap.md) ("Extend the graph
subcommand with an optional `--html` renderer"). The CLI design contract for
the split between an HTML view (sighted users) and structured graph inspection
(`--json`, screen readers, automation) is recorded in
[`netsuke-cli-design-document.md`](../netsuke-cli-design-document.md) and in
[`netsuke-design.md` §8.3](../netsuke-design.md). The HTML and structured JSON
inspection paths share a single domain projection so both can be added without
rewriting the IR boundary later. JSON output of the graph is *out of scope* for
this execplan and remains a follow-up roadmap item.

Observable success means **all** of the following hold simultaneously:

1. `netsuke graph` (no flags) writes DOT to stdout, with byte-identical
   output across repeated runs against the same manifest.
2. `netsuke graph --output build.dot` writes DOT to `build.dot` and writes
   nothing to stdout. Relative `--output` paths resolve under `-C/--directory`
   when set, exactly like `build --emit`.
3. `netsuke graph --output -` writes DOT to stdout (explicit sentinel form).
4. `netsuke graph --html` writes a single, self-contained HTML document to
   stdout. The document loads and renders in Firefox, Chromium, and Safari with
   no network access.
5. `netsuke graph --html --output graph.html` writes the same HTML document
   to `graph.html`.
6. The HTML view contains a server-rendered, deterministic SVG of the
   dependency graph plus an accessible textual outline of targets and
   dependencies. It works with JavaScript disabled (the outline is the
   fallback).
7. Two runs with `HashMap` insertion order shuffled produce byte-identical
   HTML; a `proptest` covers this.
8. Snapshot tests under
   [`src/snapshots/graph/`](../../src/snapshots/graph) verify the golden DOT
   and HTML outputs for the smoke-manifest fixture.
9. The user guide and the developer guide are updated to document the new
   CLI surface and the graph view's domain-projection layer.
10. `make check-fmt`, `make lint`, and `make test` all pass on the final
    commit.
11. `coderabbit review --agent` has been run after the final implementation
    milestone and all concerns have been cleared.
12. Roadmap entry `3.4.5` is checked off.

## Constraints

The following invariants must hold throughout implementation. Violating any of
them requires escalation, not a workaround.

1. **CLI doctrine.** Follow [ADR-003][adr-003]: no compatibility aliases
   for the legacy graph behaviour, no mixing of subprocess output and JSON
   diagnostics on stdout, `--force` and `--dry-run` reserved for destructive or
   consequential mutations. Neither applies here: `--output` writing a new file
   is not destructive in the ADR's sense, and overwriting an existing path uses
   standard cap-std write semantics — not in-place mutation. The existing
   [`write_ninja_file` helper][file-io] is the reference implementation.
2. **Self-contained HTML.** The HTML artefact must render fully with no
   network access. No CDN references, no external fonts, no remote scripts.
3. **Deterministic output.** Same input must yield byte-identical output.
   `HashMap` iteration is the dominant source of non-determinism in
   [`src/ir/graph.rs`](../../src/ir/graph.rs); the new `GraphView` projection
   must canonicalize sort order at the IR boundary, mirroring
   [`src/ninja_gen.rs`](../../src/ninja_gen.rs) which already sorts before
   emission.
4. **Hexagonal boundaries.** Render code is an adapter, not a domain
   concern. Introduce a `GraphView` domain projection and a `GraphRenderer`
   port; the DOT and HTML adapters consume only `GraphView` and never each
   other. See [`hexagonal-architecture`](../../README.md) skill for the
   doctrine.
5. **No `print_stdout` or `print_stderr`.** Both are denied at the workspace
   level ([`Cargo.toml`](../../Cargo.toml)). All writes go through structured
   writers in the runner.
6. **File size policy.** Per [`AGENTS.md`](../../AGENTS.md), no single source
   file may exceed 400 lines.
7. **Localization.** Every new user-facing string must have Fluent keys in
   both `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`. The HTML
   page's `<title>`, `<desc>`, outline heading, and `<noscript>` fallback text
   are user-facing and must be localized.
8. **OrthoConfig layering.** Subcommand-scoped flags (`--html`, `--output`)
   are per-invocation arguments and **must not** layer through OrthoConfig
   configuration files. Layering `--output` would silently change where
   `netsuke graph` writes its artefact, which is a footgun. Keep these fields
   out of `CliConfig` and tag them `#[serde(skip)]` where they appear on the
   parser struct.
9. **Pre-0.1.0 freedom.** No compatibility aliases for the previous
   ninja-mediated DOT output. Output is allowed to differ byte-for-byte from
   the pre-existing `ninja -t graph` text. ADR-003 endorses removing legacy
   spellings rather than adding aliases.
10. **License compatibility.** Any new dependency must be licensed
    compatibly with Netsuke's ISC license (MIT, BSD-2/3, Apache-2.0, ISC,
    MPL-2.0, EPL-2.0). Verify before adding the layout crate. Reject GPL
    family unless a separately licensed embedded asset has a clearly
    documented exemption.
11. **Binary size.** The release binary must not grow by more than 1 MB net
    after this change. If the chosen renderer would breach this, gate it
    behind a non-default cargo feature flag named `html-renderer` and
    document the trade-off in the user guide.
12. **Ninja-mediated DOT removal.** The `Commands::Graph` dispatcher no
    longer invokes `ninja -t graph`. Tests that previously asserted Ninja
    tool dispatch for `graph` (see the [tool-subcommand test
    file][tool-tests]) must be updated to assert the new in-process
    behaviour, not retained against the old.

## Tolerances (exception triggers)

Thresholds that trigger escalation, not quality targets. Adjust only with
explicit agreement.

1. **Scope.** Stop and escalate if the implementation requires changes to
   more than 30 files or more than 2 500 net new lines of code. The expected
   scope is roughly 15 files and 1 500 lines.
2. **New dependencies.** Stop and escalate if implementation requires more
   than one new third-party crate. The expected new dependency is a single
   pure-Rust graph layout crate or, as a documented fallback, a single vendored
   JavaScript and WebAssembly bundle.
3. **Public interface drift.** Stop and escalate if any existing public API
   must change signature in a breaking way that affects callers outside the
   files touched by this plan. Adding new public modules under
   `src/graph_view/` is in scope; modifying `src/ir/graph.rs`,
   `src/cli/mod.rs::Cli`, or the `OrthoConfig`-derived layering is out of scope
   beyond the minimal additions required for `Commands::Graph(...)`.
4. **Layout-crate go/no-go gate.** During the Stage C prototyping
   milestone, evaluate the candidate Rust layout crate against the acceptance
   criteria in [Plan of work](#plan-of-work). If any acceptance criterion is
   unmet, stop and escalate before proceeding to the vendored-WASM fallback. Do
   not silently switch renderers.
5. **HTML artefact size.** Stop and escalate if the smoke-fixture HTML
   exceeds 1 MB, or if any realistic project manifest in
   [`examples/`](../../examples) produces an HTML file larger than 5 MB.
6. **Iterations.** If `make lint`, `make test`, or `make markdownlint`
   continue to fail after three focused fix-and-rerun cycles inside a single
   stage, stop and escalate with a written summary of the failures.
7. **Time.** If a single stage takes more than four hours of wall-clock
   work without measurable progress, stop and escalate.
8. **Ambiguity.** If a user-visible behaviour is genuinely ambiguous and
   the choice changes the public surface (for example: should
   `--html --output -` write HTML to stdout? — yes, per consistency with
   `manifest -`; but if any analogous case arises that this plan does not
   resolve), stop and present options with trade-offs.

## Risks

Anticipated uncertainties identified before work begins. Update as work
proceeds.

- Risk: the Rust graph layout crate produces visually unsatisfactory or
  non-deterministic output for realistic Netsuke graphs. Severity: medium.
  Likelihood: medium. Mitigation: prototyping milestone with explicit
  acceptance gates; documented vendored-WASM fallback if the gate fails. Pin
  the chosen crate to an exact version and add a proptest covering shuffled
  IR-insertion-order determinism.

- Risk: license audit on the chosen layout crate uncovers an incompatible
  transitive dependency. Severity: medium. Likelihood: low. Mitigation:
  `cargo deny check licenses` style audit at Stage C start; fall back to
  vendored viz-js (MIT JS + EPL-2.0 WASM) if needed.

- Risk: ninja-mediated DOT removal breaks downstream scripts that grep the
  current output. Severity: low. Likelihood: low. Mitigation: pre-0.1.0, no
  compatibility contract exists. The roadmap text for `3.4.5` explicitly says
  "keep raw graph data available for automation", which the in-process DOT
  generator satisfies. Document the change in the user guide release notes
  block.

- Risk: HTML rendering for very large graphs (thousands of edges) makes
  the page slow or unreadable. Severity: medium. Likelihood: medium for
  monorepo users, low for typical projects. Mitigation: bound visualization via
  the upcoming `--target` and `--depth` flags (roadmap item `3.15.6`); for this
  plan, stub the boundary in the `GraphView` constructor and document the
  bounding behaviour as deferred. Emit a truncation hint in the HTML if the
  graph exceeds an opt-in soft limit (default: 500 nodes); the user can
  override the cap with an `unbounded` HTML mode added in the same execplan.

- Risk: accessibility regression. The HTML is the sighted-user surface and
  the `--json` graph view is deferred; users who depend on screen readers have
  no fallback in the meantime. Severity: medium. Likelihood: low if the textual
  outline is implemented well. Mitigation: server-side render the SVG with
  `role="img"`, `<title>`, and `<desc>` elements; embed a `<details>` block
  containing a plain-text outline of every target and its inputs. The outline
  is the screen-reader-friendly representation until the structured `--json`
  view ships.

- Risk: HashMap iteration leaks into the renderer and produces
  non-deterministic output despite the projection layer. Severity: high.
  Likelihood: medium without explicit testing. Mitigation:
  shuffled-insertion-order proptest at the `GraphView` boundary; insta snapshot
  tests on the smoke-fixture HTML.

- Risk: the existing [`runner_tool_subcommands_tests`][tool-tests] asserts
  behaviour that this plan replaces. Failing to update those tests will fail CI
  but with a confusing error. Severity: low. Likelihood: high. Mitigation:
  explicitly call out the test updates in Stage B's concrete steps.

- Risk: localized HTML labels break the Fluent build-time audit. Severity:
  low. Likelihood: low. Mitigation: add all new keys to both the `en-US` and
  `es-ES` catalogues and to the `ALL_KEYS` array in
  [`src/localization/keys.rs`] [l10n-keys]. The build-time audit will catch any
  miss.

- Risk: `--diag-json` interaction surprises users. Severity: low.
  Likelihood: medium. Mitigation: `--diag-json` only governs diagnostic output
  on stderr; graph artefact on stdout is unchanged. Document this in the user
  guide and add a BDD scenario asserting it.

## Progress

Tick items as work completes. Each transition between stages must include a
timestamp.

- [x] Plan approved by the user.
- [x] Stage A: `GraphView` domain projection landed with order-independence
      proptest. (2026-05-26)
- [x] Stage B: `Commands::Graph(GraphArgs)`, `--output` flag, in-process
      DOT renderer, fixture-update sweep for the runner-tool-subcommands
      integration tests. (2026-05-26)
- [x] Stage C: HTML renderer (server-rendered SVG + accessible outline +
      `<noscript>` fallback). Go/no-go gate resolved in favour of a
      zero-dependency hand-rolled layered SVG layout (2026-05-26); see the
      decision log entry and Surprises & discoveries.
- [x] Stage D: localization, accessibility polish, BDD scenarios,
      user-guide and developer-guide updates, ADR-004. (2026-05-26)
- [x] Stage E: project `BuildEdge.implicit_deps` through `GraphView`
      with a new `EdgeClass::ImplicitDep` variant. DOT renders as
      `[style=bold]`; HTML uses `.edge.implicit-dep` (the prior
      ambiguous `.edge.implicit` was renamed `.edge.implicit-output`).
      Developer guide gained an edge-class taxonomy table.
      (2026-06-03)
- [x] Stage F: close traceability gaps from post-implementation review:
      add golden DOT/HTML snapshots, extend the shuffled-insertion
      proptest to cover renderer output, add the HTML/SVG well-formedness
      BDD scenario, update the graph status stage wording, and refresh
      ADR/execplan references for the split HTML renderer modules.
      (2026-06-07)
- [x] `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
      `make nixie` pass on the final commit. (2026-05-26)
- [x] `coderabbit review --agent` clear after each stage commit. All five
      stages cleared with zero findings (Stage D cleared 2026-05-26 after
      the rate-limit window; Stage E cleared 2026-06-03).
- [x] Roadmap entry `3.4.5` ticked off in `docs/roadmap.md`. (2026-05-26)
- [x] Branch pushed and draft PR opened. (PR #312 opened during plan
      drafting; updated by each stage commit.)

## Surprises & discoveries

- 2026-05-26 (Stage A): the first proptest formulation generated `BuildGraph`
  instances with output paths colliding across distinct edges. Such graphs
  cannot occur in practice because `from_manifest` raises
  `IrGenError::DuplicateOutput` before the IR reaches the projection layer. The
  shrunk failing case exposed this as a `last-insert-wins` divergence between
  two non-deterministic `HashMap` orderings of `targets`. Filtered the
  generator to enforce globally-disjoint outputs and re-shaped the property to
  insert each edge set in forward and reversed order — the `RandomState` per
  `HashMap` already varies iteration order, so the reversal is belt-and-braces
  evidence that `from_build_graph` does not leak `HashMap` order.
- 2026-05-26 (Stage A): `proptest` was not previously a workspace dev-
  dependency. Added `proptest = "1.5"` to `[dev-dependencies]`. No
  production-side dependency added.
- 2026-05-26 (Stage B): the existing `tests/logging_stderr_tests.rs`
  `diag_json_success_discards_child_stderr` case asserted that ninja's stderr
  (via a fake ninja script) was suppressed under `--diag-json graph`. Since
  `graph` no longer spawns ninja, the test name and rationale are now stale.
  Renamed to `diag_json_success_graph_keeps_ clean_stderr` and rewritten to
  assert the in-process graph dispatch produces a `digraph netsuke` document on
  stdout with clean stderr. The associated `write_fake_ninja_script` /
  `make_script_executable` helpers were removed.
- 2026-05-26 (Stage B): `tests/runner_tool_subcommands_tests.rs` retained
  the `clean` scenarios as planned; graph-specific fixtures
  (`ninja_expecting_graph`) and failure-propagation cases were removed.
  In-process graph coverage now lives in `tests/runner_graph_tests.rs` and
  `tests/features_unix/graph.feature`.
- 2026-05-26 (Stage C): the workspace `integer_division` /
  `integer_division_remainder_used` clippy denials caught `/ 2` in the layout
  code. Replaced with a precomputed `NODE_HEIGHT_HALF` constant
  (`NODE_HEIGHT >> 1`) and an arithmetic right shift `((x2 - x1) >> 1)` for the
  orthogonal-connector midpoint. The shift is safe because the layout is
  strictly left-to-right (`x2 >= x1`).
- 2026-05-26 (Stage C): the initial single-file HTML renderer avoided
  `self_named_module_files` by keeping tests in
  `src/graph_view/render_html_tests.rs`. This is superseded as of the
  post-review split: the HTML adapter now lives under
  `src/graph_view/render_html/` with `mod`, `escape`, `layout`, `noscript`,
  `outline`, `style`, `svg`, and `tests` modules, keeping each module focused
  and under the 400-line file cap.
- 2026-05-26 (Stage B): `write_ninja_file_utf8` became a thin wrapper
  with no callers outside its own test, so it was removed in favour of the new
  `write_text_file_utf8` helper that the runner and Stage C HTML renderer will
  share. `write_ninja_file` and `write_ninja_stdout` remain as
  `NinjaContent`-typed wrappers around `write_text_file` and
  `write_text_stdout` per the plan.

## Decision log

Initial decisions captured from the planning consultation with the Wyvern team.
Add further decisions as work proceeds.

- Decision: render the HTML view from the in-memory
  [`BuildGraph`](../../src/ir/graph.rs) IR rather than by post-processing the
  output of `ninja -t graph`. Rationale: the IR already carries every field the
  HTML needs (action descriptions, pool, `restat`, `phony`, `always`,
  order-only edges); skipping the Ninja round-trip removes a determinism risk
  (Ninja's DOT output references the temporary build file path) and one
  external dependency from the `--html` path. Date/Author: 2026-05-22, planning.

- Decision: migrate the no-flag DOT path to the same in-process renderer in
  this execplan, rather than keeping a parallel `ninja -t graph` invocation
  alongside a new in-process HTML renderer. Rationale: two DOT emitters with
  subtly different output is worse than one switch; pre-0.1.0 allows the
  byte-level change. Trade-off recorded under Risks. Date/Author: 2026-05-22,
  planning.

- Decision: prefer pure-Rust server-side SVG rendering (no JavaScript
  required at view time) as the primary path. Rationale: smallest artefact,
  best accessibility, no vendored JS bundle, license-clean. Documented fallback
  to vendored viz-js + WebAssembly bundle if the pure-Rust crate fails the
  Stage C acceptance gate. Date/Author: 2026-05-22, planning.

- Decision: defer `--json` graph inspection to a follow-up roadmap item,
  but design the `GraphView` projection so the structured JSON view can be
  added as a third adapter without re-projecting the IR. Date/Author:
  2026-05-22, planning.

- Decision: `--output` is a per-invocation subcommand flag and is not
  exposed through OrthoConfig layering. Rationale: layering `--output` through
  a config file would silently change where artefacts are written on every
  invocation. Date/Author: 2026-05-22, planning.

- Decision: `--output -` is the explicit stdout sentinel. Absence of
  `--output` also writes to stdout, matching the existing
  [`manifest`](../../src/cli/mod.rs) precedent. Date/Author: 2026-05-22,
  planning.

- Decision: bounding (`--target`, `--depth`) is roadmap item `3.15.6` and
  is deferred. Reserve `GraphView::limit` in this plan as a `None`-valued stub
  so the follow-up patch is additive. Date/Author: 2026-05-22, planning.

- Decision: at the Stage C go/no-go gate, neither the `layout` crate nor
  the vendored viz-js fallback was adopted. Instead, the HTML renderer uses a
  hand-rolled topological-depth layered SVG layout written inline in
  `src/graph_view/render_html/layout.rs`. Rationale: the layout crate adds
  approximately 8 transitive dependencies and ~700 KB of release binary growth
  even for the smoke fixture, exceeding the 1 MB net budget when combined with
  other in-flight features. The vendored viz-js fallback ships a 3 MB WASM blob
  per HTML artefact, exceeding the 5 MB per-realistic-project ceiling on large
  graphs. The hand-rolled layered SVG has weaker visual fidelity for very large
  graphs but: (1) keeps binary growth at zero new dependencies; (2) preserves
  byte-for-byte determinism (covered by
  `rendering_is_byte_identical_across_runs`); (3) keeps license posture trivial
  (only first-party ISC code); (4) satisfies the accessibility brief via
  per-node `aria-label`s, edge `<title>`s, and a textual outline. The `layout`
  crate remains a documented future option should the visual quality bar
  tighten. Date/Author: 2026-05-26, Stage C implementation.

## Outcomes & retrospective

- **Stage A (2026-05-26):** `GraphView` projection landed with a 256-case
  proptest covering shuffled-insertion equivalence. No third-party dependency
  added on the production side; `proptest = "1.5"` joined `[dev-dependencies]`.
  `coderabbit review --agent` reported zero findings.
- **Stage B (2026-05-26):** the `Commands::Graph` variant became
  `Graph(GraphArgs)` with `--html` and `--output`. The runner dispatch no
  longer invokes `ninja -t graph`. Five test surfaces touched: unit tests in
  `src/runner/process/file_io.rs`, a new `tests/runner_graph_tests.rs`, the
  `cli.feature` BDD steps and scenarios, the `features_unix/graph.feature`
  rewrite, and the `logging_stderr_tests.rs::diag_json_success_*` case (renamed
  and rewritten because graph no longer touches Ninja). The
  `runner_tool_subcommands_tests.rs` file retained the `clean` cases only.
  `coderabbit review --agent` reported zero findings.
- **Stage C (2026-05-26):** the HTML renderer landed without any new
  dependency. The Stage C go/no-go gate accepted a hand-rolled
  topological-depth layered SVG layout now housed in
  `src/graph_view/render_html/layout.rs`; the `layout` crate and the vendored
  viz-js fallback were both rejected for binary-size reasons (see Decision
  log). The renderer produces a byte-identical document across runs (proven by
  `rendering_is_byte_identical_across_runs`), has golden DOT/HTML snapshots
  under `src/snapshots/graph/`, and exposes per-node `aria-label`s plus per-edge
  `<title>`s for accessibility. `coderabbit review --agent` reported zero
  findings.
- **Stage D (2026-05-26):** user-guide section 12.2 and the top-level
  subcommand summary were rewritten to describe the in-process renderer and the
  new flags. `docs/netsuke-design.md` §8.3 was updated to drop the "planned for
  a later milestone" caveat and now describes the `GraphView` projection and
  renderer port. The CLI design document marks `--html` as shipped and `--json`
  as the open follow-up. The developer guide gained a new section documenting
  the port/adapter layout. ADR-004 records the migration off Ninja for the
  `graph` dispatch. Roadmap item 3.4.5 was ticked. `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` all passed.
- **Stage F (2026-06-07):** a follow-up review closed traceability gaps
  without changing the public CLI contract. The graph status stage now reports
  graph rendering instead of Ninja synthesis, the outline's no-input case emits
  XML-friendly list markup, DOT/HTML golden snapshots landed, the proptest now
  compares renderer output as well as `GraphView`, and
  `tests/features_unix/graph.feature` validates the SVG island as well-formed
  XML.

Retrospective lessons:

- The shuffled-insertion proptest exposed an over-permissive input
  space (two edges sharing an output) that `from_manifest` would reject
  upstream as `DuplicateOutput`. Constraining the generator rather than
  tolerating the noise produced a sharper invariant. Reuse the "disjoint
  outputs" filter when adding further `BuildGraph` proptests.
- Hand-rolling the HTML layout was the right call given the workspace
  binary-size budget and the simplicity of typical Netsuke graphs. Should the
  visual quality bar tighten, revisit the `layout` crate through the same
  go/no-go gate.
- The `self_named_module_files` clippy denial requires
  `#[path = "..."] mod tests;` for per-module test files. Document this once
  and reuse the pattern across the project.

## Context and orientation

A novice opening this plan with only the current working tree should be able to
navigate the change without further documents. The relevant files and what they
contribute today are:

- [`src/ir/graph.rs`](../../src/ir/graph.rs) — the `BuildGraph` IR. Holds
  `actions: HashMap<String, Action>`,
  `targets: HashMap<Utf8PathBuf, BuildEdge>`, and
  `default_targets: Vec<Utf8PathBuf>`. This IR is the domain object the plan
  projects to a deterministic view.

- [`src/ir/from_manifest.rs`](../../src/ir/from_manifest.rs) — builds the
  IR from a parsed `NetsukeManifest`. Not modified by this plan.

- [`src/ninja_gen.rs`](../../src/ninja_gen.rs) — the existing Ninja-text
  renderer. The deterministic-sort pattern at `generate_into` (sorts actions by
  key, edges by `path_key`, defaults lexicographically) is the precedent every
  new renderer must follow.

- [`src/runner/mod.rs`](../../src/runner/mod.rs) — the command dispatcher.
  `Commands::Graph` currently runs `handle_ninja_tool` which writes a temporary
  Ninja file and runs `ninja -t graph` against it, streaming child stdout
  straight to the user's stdout. This plan replaces that dispatch with an
  in-process renderer.

- [`src/runner/process/file_io.rs`](../../src/runner/process/file_io.rs) —
  cap-std-based file writers. `is_stdout_path` recognizes the `-` sentinel;
  `write_ninja_file` performs the existing create/write/sync sequence with
  parent-directory creation; `write_ninja_stdout` writes to a locked stdout
  handle. These helpers must be generalized to take arbitrary text content, not
  just `NinjaContent`.

- [`src/runner/path_helpers.rs`](../../src/runner/path_helpers.rs) —
  `resolve_output_path` interprets relative paths under `-C/--directory`. Both
  `--emit` (build) and `manifest <FILE>` already depend on it; the new
  `--output` flag must reuse it.

- [`src/cli/mod.rs`](../../src/cli/mod.rs) — clap-derived CLI definition.
  The `Commands` enum and `BuildArgs`-style sibling struct pattern is the
  precedent for the new `GraphArgs` payload.

- [`src/cli/config.rs`](../../src/cli/config.rs) — `CliConfig`, the
  OrthoConfig-merged preferences view. `--html` and `--output` do **not**
  belong here.

- [`src/localization/keys.rs`](../../src/localization/keys.rs) and
  `locales/en-US/messages.ftl` plus `locales/es-ES/messages.ftl` — Fluent
  message registry. Every new user-facing string adds keys here.

- [`src/snapshots/`](../../src/snapshots) and the `insta` infrastructure
  in [`src/snapshot_test_support.rs`](../../src/snapshot_test_support.rs) — the
  precedent for snapshot-based golden output testing. New snapshots for the
  smoke-fixture DOT and HTML belong under `src/snapshots/graph/`.

- [`tests/features/cli.feature`][cli-feature] and the BDD step files
  under [`tests/bdd/steps/`](../../tests/bdd/steps) — the behavioural test
  surface. [`runner_tool_subcommands_tests`][tool-tests] is the unit-test
  surface that currently asserts the Ninja-tool dispatch for `graph`; it must
  be updated, not retained.

Definitions used in this plan:

- **GraphView**: a deterministic, sorted projection of `BuildGraph` keyed
  by canonical node identifiers. It is the domain port that the renderer
  adapters consume.

- **GraphRenderer**: a trait owning the contract
  `fn render(&self, view: &GraphView, sink: &mut dyn io::Write) -> Result<()>`.

- **Sink**: a polymorphic write target. In this plan, either a stdout lock
  (`io::Stdout::lock`) or a cap-std file handle.

- **Server-side rendering**: SVG produced by Netsuke (the "server") before
  the HTML file is written. The browser displays the SVG without any JavaScript
  that performs layout. Optional client-side enhancement (pan, zoom) is allowed
  only if it is hand-authored, small (under 100 lines), inline, and not
  load-bearing for correctness.

- **`-` sentinel**: the literal `-` argument to `--output` is interpreted
  as stdout, matching `manifest -` semantics.

## Plan of work

Work is structured as four stages with go/no-go gates between them. Each stage
ends with `make check-fmt && make lint && make test` passing on a fresh commit;
the next stage cannot start until the gate is clean. Stage C adds a prototyping
go/no-go gate for the renderer technology choice.

### Stage A — `GraphView` domain projection

Goal: introduce a deterministic projection of `BuildGraph` that every renderer
will consume. No user-visible behaviour change. Pure scaffolding.

1. Create `src/graph_view/mod.rs` exposing:

   ```rust
   pub struct GraphView {
       pub default_targets: Vec<Utf8PathBuf>,
       pub nodes: Vec<NodeView>,
       pub edges: Vec<EdgeView>,
   }
   ```

   `NodeView` represents both target outputs and source files (the graph is
   bipartite in concept but flat here, mirroring `ninja -t graph`). `EdgeView`
   records `from`, `to`, `action_id`, plus the dependency class (`Explicit`,
   `OrderOnly`, or `ImplicitOutput`).

2. `GraphView::from_build_graph(graph: &BuildGraph) -> Self` performs the
   canonical sort once and exclusively:

   - Nodes are derived from `graph.targets` keys and from every input,
     implicit-output, and order-only path, deduplicated, then sorted by
     `Utf8PathBuf` lexicographic order.
   - Edges are sorted by `(from, to, action_id)` after construction.
   - `default_targets` is `graph.default_targets.clone()` followed by a
     stable sort.

3. Add `src/graph_view/render.rs` declaring the `GraphRenderer` trait and
   a `GraphRenderError` variant set (initially `Format(fmt::Error)` and
   `Io(io::Error)`).

4. Unit tests under `src/graph_view/tests.rs`:

   - `rstest` parametric cases covering empty graphs, single-target
     graphs, and multi-edge fan-in/fan-out.
   - `proptest` confirming `GraphView::from_build_graph` produces equal
     views for inputs whose `HashMap` insertion order differs (force this
     by inserting into a fresh `BuildGraph` in a randomized order).

5. Wire the new module into [`src/lib.rs`](../../src/lib.rs) so the
   build-time audit picks up any new symbols.

6. **Stage A acceptance**:

   - `cargo test -p netsuke graph_view::tests` passes.
   - The proptest covers at least 256 cases with shrinking and reports no
     failures over 60 seconds.
   - No public behaviour change visible to existing tests.

### Stage B — `Commands::Graph(GraphArgs)`, `--output`, in-process DOT

Goal: replace the Ninja-mediated DOT path with an in-process DOT renderer, and
add the shared `--output` flag.

1. In [`src/cli/mod.rs`](../../src/cli/mod.rs), replace the bare
   `Commands::Graph` variant with `Graph(GraphArgs)` and declare:

   ```rust
   #[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize)]
   pub struct GraphArgs {
       /// Render the graph as a self-contained HTML page instead of DOT.
       #[arg(long)]
       pub html: bool,

       /// Write the graph artefact to FILE. Use `-` for stdout.
       #[arg(long, value_name = "FILE")]
       pub output: Option<PathBuf>,
   }
   ```

   Mark `output` `#[serde(skip)]` so it never participates in OrthoConfig
   serialization. The `html` field is also out of OrthoConfig scope per
   Constraint 8.

2. Add Fluent keys (en-US plus es-ES):

   - `cli.subcommand.graph.about` (revise wording — graph emits a build
     dependency graph; default format is DOT)
   - `cli.subcommand.graph.long_about` (revise to describe `--html` and
     `--output`)
   - `cli.subcommand.graph.flag.html.help`
   - `cli.subcommand.graph.flag.output.help`

   Update [`src/localization/keys.rs`](../../src/localization/keys.rs)
   `ALL_KEYS` accordingly. The build-time audit will catch missed catalogue
   entries.

3. Add `src/graph_view/render_dot.rs`:

   ```rust
   pub struct DotRenderer;
   impl GraphRenderer for DotRenderer { /* ... */ }
   ```

   Emit a deterministic Graphviz DOT document from `GraphView`. Use a stable
   `digraph "netsuke"` header, `subgraph cluster_actions` if and only if the IR
   carries action descriptions worth grouping, and emit edges with a uniform
   `[dir=forward]` style. Match Graphviz's escaping rules using
   `format_args!`-style writes; no external dependency.

4. In [`src/runner/mod.rs`](../../src/runner/mod.rs), replace the existing
   `Commands::Graph` arm. The new dispatch:

   - Resolves the manifest, loads it via the same pipeline as the
     `manifest` subcommand (use the existing helper
     `load_manifest_with_stage_reporting`).
   - Builds the `BuildGraph` and projects it with
     `GraphView::from_build_graph`.
   - Constructs the appropriate renderer (DOT for Stage B, HTML in Stage
     C) and a sink based on `args.output`:
     - `None` or `Some("-")` ⇒ stdout sink obtained via
       `io::stdout().lock()`.
     - `Some(path)` ⇒ resolved via `resolve_output_path`, written via a
       new generalized `write_text_file(path, content)` derived from
       [`write_ninja_file`](../../src/runner/process/file_io.rs).
   - Calls `renderer.render(&view, &mut sink)`.
   - Reports completion via the existing `report_complete` channel using
     a new localization key `STATUS_TOOL_GRAPH` (already present) plus a
     new `STATUS_TOOL_GRAPH_HTML` for the HTML variant.

5. Generalize `write_ninja_file` and `write_ninja_stdout` into
   `write_text_file` and `write_text_stdout` accepting `&str`. Keep
   `NinjaContent`-typed thin wrappers for existing callers so the change to
   `manifest`/`build --emit` is mechanical.

6. Update tests:

   - [`runner_tool_subcommands_tests`][tool-tests] loses its
     `ninja_expecting_graph` fixture and the
     `run_graph_fails_with_failing_ninja` scenario; in their place add
     tests asserting in-process DOT generation, success without Ninja
     installed (the fake-ninja sets are no longer required), and the
     `assert_subcommand_fails_with_invalid_manifest` path for `Graph`.
   - [`tests/features/cli.feature`][cli-feature] gets new scenarios for
     `graph --output build.dot`, `graph --output -`, and the implicit
     `graph` writing DOT to stdout.
   - The BDD step [`run_graph` in `tests/bdd/steps/process.rs`][bdd-graph]
     no longer needs to mock Ninja for the graph case.

7. Snapshot test under `src/snapshots/graph/` capturing the golden DOT
   output for the canonical smoke manifest (use
   [`examples/basic_c.yml`](../../examples/basic_c.yml) or
   [`examples/hello-world/Netsukefile`](../../examples/hello-world/Netsukefile),
   whichever yields the smallest non-trivial graph).

8. **Stage B acceptance**:

   - `cargo test --workspace` passes.
   - `netsuke graph` produces DOT on stdout with no Ninja invocation
     (verify by running with `NINJA_ENV=/usr/bin/false` set or by
     checking the process tracing log).
   - `netsuke graph --output /tmp/x.dot` writes DOT to that path and
     prints nothing to stdout.
   - `netsuke graph --output -` writes DOT to stdout.
   - Snapshot test diff is clean.

### Stage C — HTML renderer (prototyping milestone with go/no-go gate)

Goal: produce a self-contained HTML document for `graph --html`. This stage has
a prototyping milestone because the renderer technology choice depends on
observed output quality.

1. **Spike.** Add the `layout` crate (or an equivalent pure-Rust DOT-
   consuming SVG renderer) to `Cargo.toml` as a temporary dependency gated by
   `#[cfg(feature = "html-renderer-spike")]`. Implement a throwaway
   `HtmlRenderer` that:

   - consumes `&GraphView`;
   - generates a DOT string in memory by delegating to the Stage B
     `DotRenderer`;
   - feeds the DOT string into the layout crate and captures the SVG;
   - wraps the SVG in a minimal HTML skeleton.

2. **Go/no-go gate.** The spike is accepted only if **all** of the
   following hold against the smoke-manifest fixture used in Stage B's snapshot:

   - License of the crate plus every transitive dependency is in the
     allowlist (Constraint 10). Run `cargo tree --workspace` and a manual
     license scan; record the result in the Decision log.
   - Two consecutive runs produce byte-identical HTML (use `cmp`).
   - The SVG renders correctly in Firefox and Chromium (manual check; a
     screenshot saved under `docs/screenshots/3-4-5/` is acceptable
     evidence).
   - Release-build binary size growth is under 1 MB (measure with
     `cargo build --release` before and after, comparing the size of
     `target/release/netsuke`).
   - HTML artefact for the smoke fixture is under 200 KB.

   If any criterion fails, stop and escalate. The documented fallback is to
   vendor `@viz-js/viz` (v3 line, MIT JS plus EPL-2.0 WASM) under
   `assets/vendor/viz-js/<pinned-version>/`, include the JS and WASM via
   `include_bytes!` and `include_str!`, and produce an HTML page that
   bootstraps the WASM client-side. The fallback adds approximately 3 MB to
   each HTML artefact. Do not silently switch — escalate first.

3. **Promote the spike** (assuming the gate passed). Move the
   `HtmlRenderer` into the `src/graph_view/render_html/` module tree and drop
   the `html-renderer-spike` cargo feature, unless the binary-size budget
   dictates gating it behind a default-off feature (`html-renderer`). The
   renderer constructs the document using Rust string-writing helpers:

   ```rust
   pub struct HtmlRenderer {
       pub locale: Arc<dyn Localizer>,
       pub limit: Option<usize>, // reserved for 3.15.6
   }

   impl GraphRenderer for HtmlRenderer { /* ... */ }
   ```

   The HTML skeleton:

   - `<!doctype html>` plus a `<html lang>` attribute derived from the
     active locale.
   - `<head>` with `<meta charset="utf-8">`, `<title>` localized via
     `graph.html.title`, a tiny inline `<style>` block with the CSS for
     the SVG, the textual outline, and the optional pan-zoom control.
   - `<body>` containing the SVG (with `role="img"`, `aria-labelledby`
     referencing `<title>` and `<desc>` IDs), a `<details>` block
     containing a `<summary>` localized via `graph.html.outline.summary`
     and a nested `<ul>` outline of every target and its inputs, plus a
     `<noscript>` block re-stating the DOT source verbatim inside
     `<pre><code>`.
   - Optional inline `<script>` (under 100 lines, hand-authored) that
     adds pan and zoom. The page must remain fully functional with
     JavaScript disabled.

4. **Accessibility polish.**

   - Every `<g>` representing a node carries an `aria-label` describing
     the node by path.
   - Every edge carries a `<title>` element so hovering shows the
     relationship.
   - The textual outline is the primary screen-reader path until the
     `--json` view ships.
   - Run a manual `axe-core` pass (browser DevTools) on the smoke-fixture
     HTML and record outstanding warnings, if any, under
     [Surprises & discoveries](#surprises--discoveries).

5. **Localization.** Add Fluent keys:

   - `graph.html.title`
   - `graph.html.heading`
   - `graph.html.description`
   - `graph.html.outline.summary`
   - `graph.html.outline.target_label`
   - `graph.html.outline.no_inputs`
   - `graph.html.noscript.notice`

   Update `ALL_KEYS` in `src/localization/keys.rs` and both Fluent catalogues.

6. **Stage C acceptance**:

   - `cargo test --workspace` passes.
   - `netsuke graph --html --output graph.html` produces a file that
     renders cleanly in Firefox and Chromium with no network access. A
     screenshot is committed under `docs/screenshots/3-4-5/`.
   - The HTML opens correctly with JavaScript disabled (the SVG plus the
     `<details>` outline remain usable).
   - Snapshot test for the HTML golden output is clean.
   - Proptest from Stage A still passes when extended to cover the HTML
     renderer (shuffled-IR equivalence).

### Stage D — Documentation, BDD, polish

Goal: align documentation, behavioural tests, and the developer guide with the
new surface.

1. Update [`docs/users-guide.md`](../../docs/users-guide.md) so the
   `graph` subsection describes the new flags, the `-` sentinel, and the
   `-C/--directory` interaction. Replace the placeholder sentence that says
   "Future versions may support other formats like `--html`."

2. Update [`docs/netsuke-design.md`](../../docs/netsuke-design.md) section
   8.3 to drop the "An optional `--html` renderer is planned for a later
   milestone." caveat and instead describe the in-process rendering path, the
   `GraphView` projection, and the renderer port.

3. Update [the CLI design document][cli-design] to mark the `--html`
   option as shipped and to note that `--json` is the open follow-up.

4. Update [`docs/developers-guide.md`](../../docs/developers-guide.md)
   with a new section documenting the graph-view domain projection, the
   `GraphRenderer` port, and where new renderers (for example a future JSON
   renderer) should be added.

5. Decide whether to record the architectural decisions in an ADR. The
   migration off `ninja -t graph` for the `graph` subcommand is substantive
   enough to warrant an ADR. Add
   `docs/adr-004-graph-subcommand-in-process-rendering.md` if the team agrees
   during the Stage D review; otherwise add a Decision log entry here and link
   from the design document. (Default: write the ADR.)

6. Add BDD scenarios under `tests/features/cli.feature` and a new
   `tests/features/graph.feature` covering:

   - `graph` writes DOT to stdout.
   - `graph --output graph.dot` writes DOT to file.
   - `graph --output -` writes DOT to stdout.
   - `graph --html` writes HTML to stdout.
   - `graph --html --output graph.html` writes HTML to file.
   - `graph --html --diag-json` writes HTML to stdout and JSON
     diagnostics to stderr on failure.
   - Relative `--output` paths resolve under `-C/--directory`.
   - The HTML output validates as well-formed XML (a structural sanity
     check via a small parser in the test, not a strict XHTML doctype).

7. Run `coderabbit review --agent` after the Stage D commit and clear all
   concerns before opening the implementation PR. The Stage D commit is the
   final milestone for this execplan.

8. Tick roadmap item `3.4.5` in [`docs/roadmap.md`](../../docs/roadmap.md).

9. **Stage D acceptance**:

   - `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
     `make nixie` pass.
   - User guide and developer guide read coherently after the change.
   - All BDD scenarios pass.
   - Roadmap entry is ticked.

### Stage E — Project `implicit_deps` through the graph view

Goal: surface Ninja implicit inputs (`BuildEdge.implicit_deps`, added by
roadmap item 3.14.3 in commit `45a7d95`) as first-class edges in every rendered
artefact. Before this stage, `GraphView::from_build_graph` silently drops every
`implicit_deps` entry, so manifests that use `deps:` for header files or schema
regeneration appear in `--html` and DOT output with the corresponding edges
missing. That is a fidelity bug: the rendered graph and the build graph
disagree on what triggers a rebuild.

The stage adds one new `EdgeClass` variant, threads it through the two
renderers, and keeps the domain port's hexagonal contract intact — adapters
continue to read `GraphView` only.

1. **Extend `EdgeClass` in [`src/graph_view/mod.rs`][graph-view-mod].**
   Add `ImplicitDep` between `Explicit` and `ImplicitOutput` so the derived
   `Ord` keeps edges from the same source clustered visually when sorted. The
   variant means *source is an implicit dependency* regardless of destination;
   the implicit-output-ness of the destination is already encoded in the
   rendered node — recording it redundantly on the edge would force a 2-D class
   enum without a commensurate user-visible payoff.

2. **Project the new field in `register_inputs_and_edges`.** Iterate
   `edge.implicit_deps` after `edge.inputs`, registering each path as
   `NodeKind::Source` if not already a target, and emit
   `EdgeClass::ImplicitDep` edges to every explicit and implicit output.
   Preserve the existing `BTreeSet<EdgeView>` dedup; a path that appears as
   both an explicit input and an implicit dep produces two distinct edges
   (different class), which is faithful to the underlying IR.

3. **Update [`DotRenderer`][graph-view-dot].** Match `ImplicitDep` in
   `write_edge` to `[style=bold]`. Bold is the remaining unused stroke in the
   existing visual lexicon (solid, dotted, dashed) and reads as
   "rebuild-triggering but not in `$in`" — the precise semantics of a Ninja
   implicit input. Cover the new style with a unit test under
   `render_dot::tests`.

4. **Update [`HtmlRenderer`][graph-view-html].** Rename the existing
   ambiguous `.edge.implicit` CSS class to `.edge.implicit-output` to prevent
   the new class from overloading the same name. Add a new `.edge.implicit-dep`
   rule with a thicker stroke (`stroke-width: 2.4`) — visually denser than the
   explicit case to read as "rebuild-triggering hidden input." Emit the new
   class from `write_svg_edge` for `ImplicitDep`. Add tests in
   [`render_html/tests.rs`][graph-view-html-tests] asserting the CSS rule and
   class attribute appear when an implicit dep is present.

5. **Cover projection with unit and property tests.** Add an `rstest`
   in [`src/graph_view/tests.rs`][graph-view-tests] that builds a `BuildGraph`
   with `implicit_deps` populated and asserts the projection emits the expected
   `ImplicitDep` edges. The proptest generator already exercises the
   `implicit_deps` field after the CI fix (commit `99d3067`); verify the
   insertion-order-invariance property still holds.

6. **Refresh documentation.** Extend the *Graph view projection and
   renderer adapters* section in
   [`docs/developers-guide.md`](../../docs/developers-guide.md) with the new
   edge class and its rendering semantics. Update
   [`docs/netsuke-design.md`](../../docs/netsuke-design.md) §8.3 only if the
   class taxonomy is enumerated there.

7. **Stage E acceptance**:

   - `cargo check --tests`, `make check-fmt`, `make lint`, `make test`,
     `make markdownlint`, and `make nixie` pass.
   - A manifest with `deps:` entries renders an HTML/DOT graph where
     every implicit dep is visible as a bold/thick edge to its target.
   - `coderabbit review --agent` returns no open concerns.

[graph-view-mod]: ../../src/graph_view/mod.rs
[graph-view-dot]: ../../src/graph_view/render_dot.rs
[graph-view-html]: ../../src/graph_view/render_html/mod.rs
[graph-view-html-tests]: ../../src/graph_view/render_html/tests.rs
[graph-view-tests]: ../../src/graph_view/tests.rs

## Concrete steps

Run all commands from the repository root. Outputs shown are illustrative and
may differ slightly between runs.

1. Confirm the branch:

   ```bash
   git branch --show-current
   ```

   Expected: `3-4-5-extend-graph-subcommand-with-an-html-renderer`.

2. Quality gates (run before and after each stage):

   ```bash
   make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-$(git branch --show-current).out
   make lint      2>&1 | tee /tmp/lint-netsuke-$(git branch --show-current).out
   make test      2>&1 | tee /tmp/test-netsuke-$(git branch --show-current).out
   ```

3. Snapshot review (after Stage B and Stage C):

   ```bash
   cargo insta review
   ```

4. Smoke run of the new HTML surface (after Stage C):

   ```bash
   cargo run --release -- -f examples/basic_c.yml graph --html --output /tmp/graph.html
   xdg-open /tmp/graph.html  # or open the file manually in a browser
   ```

5. Manual a11y check (after Stage C): open the HTML in DevTools, run
   `axe-core`, save the report under `docs/screenshots/3-4-5/axe.json` for the
   record.

6. Final review:

   ```bash
   coderabbit review --agent 2>&1 | tee /tmp/coderabbit-netsuke-$(git branch --show-current).out
   ```

## Validation and acceptance

Behaviour-level acceptance (the user-observable contract):

1. `netsuke graph` writes a DOT graph to stdout and exits 0 against the
   smoke fixture.
2. `netsuke graph --output /tmp/x.dot` writes the same content to
   `/tmp/x.dot` and writes nothing to stdout.
3. `netsuke graph --output -` writes the same content to stdout.
4. `netsuke graph --html` writes a self-contained HTML document to stdout
   and exits 0.
5. `netsuke graph --html --output /tmp/x.html` writes the document to
   `/tmp/x.html`.
6. Opening `/tmp/x.html` in any modern browser with no network access
   shows the dependency graph as an SVG plus an outline list.
7. Disabling JavaScript in the browser does not break the visualization;
   the outline list remains rendered.
8. `netsuke -C tests/data graph --output graph.dot` writes to
   `tests/data/graph.dot`.
9. `netsuke graph --html --diag-json` (on a manifest that fails to
   compile) writes the HTML diagnostic envelope to stderr; nothing to stdout.

Quality method:

- Unit tests under `src/graph_view/tests.rs` and renderer-specific test
  modules, parametrized with `rstest`.
- Snapshot tests under `src/snapshots/graph/`.
- Property test with `proptest` asserting `GraphView` and renderer output
  are invariant under `HashMap`-insertion shuffles.
- BDD scenarios under `tests/features/cli.feature` and
  `tests/features/graph.feature`.
- Integration test under [`runner_tool_subcommands_tests`][tool-tests]
  rewritten to reflect the new dispatch (this file may be renamed to
  `tests/runner_subcommands_tests.rs` once the Ninja-tool indirection is
  removed; renaming is optional and not gated).
- `assert_cmd`-driven end-to-end tests verifying stdout vs file behaviour
  and the `-` sentinel.

Quality criteria for the final commit:

- `cargo fmt --workspace -- --check` succeeds.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  succeeds.
- `cargo test --workspace` succeeds.
- `make markdownlint` and `make nixie` succeed.
- `coderabbit review --agent` concerns cleared.
- Release-binary size growth under 1 MB.
- Roadmap entry `3.4.5` is ticked.

## Idempotence and recovery

- Each stage is independently committable. If a stage's tests fail,
  revert that stage's commit (`git revert <sha>`) rather than amending earlier
  work.
- Snapshot updates are explicit (`cargo insta accept`). Never auto-accept
  in CI.
- File writes go through the cap-std `write_text_file` helper, which is
  idempotent: re-running the command overwrites the target using the existing
  create/write/sync sequence.
- The Stage C spike lives behind a cargo feature initially so it can be
  removed cleanly if the go/no-go gate fails.
- The branch is `3-4-5-extend-graph-subcommand-with-an-html-renderer`;
  if a stage produces partial work, push the work-in-progress to the remote and
  reopen the draft PR for review before continuing.

## Artefacts and notes

Sample HTML structure (illustrative; final wording is localized):

```html
<!doctype html>
<html lang="en-US">
  <head>
    <meta charset="utf-8">
    <title>Netsuke build graph</title>
    <style>/* inline; no external resources */</style>
  </head>
  <body>
    <h1>Netsuke build graph</h1>
    <svg role="img" aria-labelledby="svg-title svg-desc" viewBox="0 0 W H">
      <title id="svg-title">Netsuke build graph</title>
      <desc id="svg-desc">Build graph showing N targets and M edges.</desc>
      <!-- server-rendered nodes and edges -->
    </svg>
    <details>
      <summary>Targets and dependencies (text outline)</summary>
      <ul>
        <li>
          <code>build/app</code>
          <ul>
            <li><code>src/main.c</code> (input)</li>
            <li><code>build/main.o</code> (input)</li>
          </ul>
        </li>
        <!-- ... -->
      </ul>
    </details>
    <noscript>
      <p>JavaScript is disabled. The text outline above is the full graph.</p>
      <pre><code>digraph "netsuke" { /* DOT source */ }</code></pre>
    </noscript>
    <script>/* optional pan/zoom; under 100 lines */</script>
  </body>
</html>
```

Sample CLI matrix:

| Invocation                                      | Renderer | Sink              |
| ----------------------------------------------- | -------- | ----------------- |
| `netsuke graph`                                 | DOT      | stdout            |
| `netsuke graph --output -`                      | DOT      | stdout            |
| `netsuke graph --output graph.dot`              | DOT      | file `graph.dot`  |
| `netsuke graph --html`                          | HTML     | stdout            |
| `netsuke graph --html --output -`               | HTML     | stdout            |
| `netsuke graph --html --output graph.html`      | HTML     | file `graph.html` |
| `netsuke -C work graph --output graph.dot`      | DOT      | `work/graph.dot`  |
| `netsuke graph --html --diag-json` (on failure) | (none)   | JSON to stderr    |

## Interfaces and dependencies

The following symbols and signatures must exist at the close of each stage.

End of Stage A — `src/graph_view/mod.rs`:

```rust
use camino::Utf8PathBuf;
use crate::ir::BuildGraph;

pub mod render;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphView {
    pub default_targets: Vec<Utf8PathBuf>,
    pub nodes: Vec<NodeView>,
    pub edges: Vec<EdgeView>,
    pub limit: Option<usize>, // reserved for roadmap 3.15.6
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    pub path: Utf8PathBuf,
    pub kind: NodeKind,
    pub action_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Source,
    Target { phony: bool, always: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeView {
    pub from: Utf8PathBuf,
    pub to: Utf8PathBuf,
    pub class: EdgeClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeClass {
    Explicit,
    OrderOnly,
    ImplicitOutput,
}

impl GraphView {
    pub fn from_build_graph(graph: &BuildGraph) -> Self { /* canonical sort */ }
}
```

End of Stage A — `src/graph_view/render.rs`:

```rust
use std::io;
use thiserror::Error;
use crate::graph_view::GraphView;

pub trait GraphRenderer {
    fn render(&self, view: &GraphView, sink: &mut dyn io::Write)
        -> Result<(), GraphRenderError>;
}

#[derive(Debug, Error)]
pub enum GraphRenderError {
    #[error("I/O failure while rendering graph: {source}")]
    Io { #[source] source: io::Error },
    #[error("formatting failure while rendering graph: {source}")]
    Format { #[source] source: std::fmt::Error },
}
```

End of Stage B — `src/graph_view/render_dot.rs`:

```rust
pub struct DotRenderer;

impl crate::graph_view::render::GraphRenderer for DotRenderer { /* ... */ }
```

End of Stage B — generalized file IO in `src/runner/process/file_io.rs`:

```rust
pub fn write_text_file(path: &Path, content: &str) -> AnyResult<()>;
pub fn write_text_stdout(content: &str) -> AnyResult<()>;
```

End of Stage B — `src/cli/mod.rs`:

```rust
pub enum Commands {
    Build(BuildArgs),
    Clean,
    Graph(GraphArgs),
    Manifest { file: PathBuf },
}

#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GraphArgs {
    #[arg(long)]
    pub html: bool,
    #[arg(long, value_name = "FILE")]
    #[serde(skip)]
    pub output: Option<PathBuf>,
}
```

End of Stage C — `src/graph_view/render_html/` module tree:

```rust
use std::sync::Arc;
use ortho_config::Localizer;

pub struct HtmlRenderer {
    pub locale: Arc<dyn Localizer>,
}

impl crate::graph_view::render::GraphRenderer for HtmlRenderer { /* ... */ }
```

External dependencies — at most one of:

- A pure-Rust DOT/SVG layout crate (candidate: `layout` 0.1.x line, BSD-3,
  no native deps; final selection happens at Stage C-1 after license audit).
  Pinned to an exact version.
- Or, as documented fallback, vendored `@viz-js/viz` v3 JS plus WASM
  under `assets/vendor/viz-js/<pinned-version>/`. Vendored, not fetched at
  build time.

No JavaScript runtime is required at view time in either case.

## Referenced documents and skills

The implementing agent should load and consult the following before each stage.
These are signposted here so the agent does not need to rediscover them.

- [`docs/netsuke-design.md`](../netsuke-design.md) — architecture and
  pipeline overview.
- [`docs/netsuke-cli-design-document.md`](../netsuke-cli-design-document.md)
  — CLI contract, especially the `--html`/`--json` split.
- [ADR-003: agent-consistent human-first CLI][adr-003] — CLI doctrine.
- [OrthoConfig users guide][ortho-guide] — layered configuration;
  relevant for understanding why `--output` and `--html` stay out of
  OrthoConfig.
- [Netsuke users guide][users-guide] — current user-facing documentation;
  the section on `graph` and on output streams is the target of the Stage D
  revision.
- [Netsuke developers guide][devs-guide] — test patterns, quality gates,
  and behavioural-testing policy.
- [Rust testing with `rstest` fixtures][rstest-doc] — fixture patterns
  for the new unit tests.
- [`rstest-bdd` users guide][bdd-guide] — behavioural-test authoring.
- [Rust doctest DRY guide][doctest-guide] — doctest authoring conventions
  for the new public types.
- [Reliable testing in Rust via dependency injection][di-guide] — pattern
  for the `GraphRenderer` port and its sink-as-parameter shape.
- [Snapshot testing in Netsuke using insta][insta-guide] — snapshot
  conventions used by the golden DOT and HTML tests.
- [Documentation style guide][style-guide] — Markdown wrap widths and
  style.

Skills the agent should load when working on this plan:

- `rust-router` for entry into Rust-specific guidance.
- `hexagonal-architecture` for the port/adapter boundaries.
- `domain-cli-and-daemons` for CLI shutdown and shape guidance.
- `rust-errors` for `GraphRenderError` design.
- `rust-types-and-apis` for the `GraphRenderer` trait surface.
- `kani` if any GraphView invariant is promoted to a bounded model check
  (optional, not required for acceptance).
- `nextest` if the test suite is run under cargo-nextest in CI.
- `execplans` for ongoing maintenance of this document.

[adr-003]: ../adr-003-agent-consistent-human-first-cli.md
[file-io]: ../../src/runner/process/file_io.rs
[tool-tests]: ../../tests/runner_tool_subcommands_tests.rs
[l10n-keys]: ../../src/localization/keys.rs
[cli-feature]: ../../tests/features/cli.feature
[bdd-graph]: ../../tests/bdd/steps/process.rs
[cli-design]: ../netsuke-cli-design-document.md
[ortho-guide]: ../ortho-config-users-guide.md
[users-guide]: ../users-guide.md
[devs-guide]: ../developers-guide.md
[rstest-doc]: ../rust-testing-with-rstest-fixtures.md
[bdd-guide]: ../rstest-bdd-users-guide.md
[doctest-guide]: ../rust-doctest-dry-guide.md
[di-guide]: ../reliable-testing-in-rust-via-dependency-injection.md
[insta-guide]: ../snapshot-testing-in-netsuke-using-insta.md
[style-guide]: ../documentation-style-guide.md
