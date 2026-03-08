# Deliver `--diag-json` machine-readable diagnostics mode

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item `3.10.3` adds a machine-readable diagnostics mode for automation,
editor integrations, and continuous integration pipelines. After this change, a
user will be able to run `netsuke --diag-json ...` and rely on a stable JSON
document instead of the current human-oriented `miette` text rendering.

Observable success means:

1. A failing command such as `netsuke --diag-json build` writes exactly
   one valid JSON document to `stderr`, with no progress lines, emoji prefixes,
   or tracing noise mixed in.
2. A successful command such as `netsuke --diag-json manifest -` keeps
   writing command artefacts to `stdout` and writes nothing to `stderr`.
3. The JSON schema is documented as a supported interface in
   [docs/users-guide.md](/home/user/project/docs/users-guide.md), design
   decisions are recorded in
   [docs/netsuke-design.md](/home/user/project/docs/netsuke-design.md), and
   compatibility is guarded by snapshot tests.
4. `make check-fmt`, `make lint`, and `make test` pass after the change.

Illustrative failure output:

```json
{
  "schema_version": 1,
  "generator": {
    "name": "netsuke",
    "version": "0.1.0"
  },
  "diagnostics": [
    {
      "message": "Manifest parse failed.",
      "code": "netsuke::manifest::parse",
      "severity": "error",
      "help": "Use spaces for indentation instead of tabs.",
      "causes": [
        "YAML parse error at line 2, column 2: tabs disallowed within this context"
      ],
      "source": {
        "name": "Netsukefile"
      },
      "primary_span": {
        "label": "problem here",
        "offset": 9,
        "length": 1,
        "line": 2,
        "column": 2,
        "end_line": 2,
        "end_column": 3,
        "snippet": "\t- name: test"
      },
      "labels": [
        {
          "label": "problem here",
          "offset": 9,
          "length": 1,
          "line": 2,
          "column": 2,
          "end_line": 2,
          "end_column": 3,
          "snippet": "\t- name: test"
        }
      ],
      "related": []
    }
  ]
}
```

## Constraints

Hard invariants that must hold throughout implementation:

- Implement roadmap item `3.10.3` only: machine-readable diagnostics via
  `--diag-json`, documented schema, and compatibility snapshots.
- Reuse the existing `Cli` struct, which already derives
  `OrthoConfig`. Do not introduce a second top-level configuration type for
  this feature.
- The new preference must participate in Netsuke's layered
  configuration model through OrthoConfig: defaults < config file < environment
  < CLI.
- Add a localized help surface for the flag via the existing CLI
  localization pipeline in
  [src/cli_l10n.rs](/home/user/project/src/cli_l10n.rs) and both Fluent bundles.
- Preserve current human-readable diagnostics when JSON mode is not
  enabled.
- When JSON mode is enabled, `stderr` must remain machine-readable. No
  progress messages, timing summaries, emoji prefixes, or tracing logs may be
  interleaved with the JSON document.
- `stdout` semantics must remain unchanged. Subcommands such as
  `manifest -` and `graph` must keep writing their normal artefacts to `stdout`.
- The JSON schema must be explicit, versioned, and stable enough to be
  treated as a user-facing contract.
- Add unit coverage with `rstest` for happy paths, unhappy paths, and
  edge cases.
- Add behavioural coverage with `rstest-bdd` v0.5.0 for happy paths,
  unhappy paths, and edge cases.
- Add snapshot coverage for the JSON contract using the existing
  `insta` dependency.
- Record design decisions in
  [docs/netsuke-design.md](/home/user/project/docs/netsuke-design.md).
- Update
  [docs/users-guide.md](/home/user/project/docs/users-guide.md) for the new
  flag, configuration knobs, stream behaviour, and schema.
- Mark roadmap item `3.10.3` done in
  [docs/roadmap.md](/home/user/project/docs/roadmap.md) only after all quality
  gates pass.
- Before finishing implementation, run all relevant gates with logged
  output:
  - `make check-fmt`
  - `make lint`
  - `make test`
  - `make markdownlint`
  - `make nixie`

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 16 files or adds
  more than 900 net new lines, stop and escalate.
- Interface: if this requires changing an existing public diagnostic code
  or user-visible text-mode error format outside JSON mode, stop and escalate.
- Schema: if a required field cannot be populated consistently across the
  current diagnostic types without inventing misleading placeholder values,
  stop and escalate with a schema revision proposal.
- Early-startup behaviour: if configuration-file-sourced `diag_json`
  cannot be honoured for configuration-load failures without a large bootstrap
  refactor, do not improvize. Document the limitation and escalate for
  confirmation.
- File size: if any edited Rust file would exceed 400 lines, split the
  work into focused helper modules before continuing.
- Test determinism: if JSON snapshots or BDD assertions remain flaky
  after two attempts, stop and escalate with the specific unstable field or
  ordering behaviour.

## Risks

- Risk: raw `miette::JSONReportHandler` output is controlled upstream and
  does not include an envelope or expanded line/column coordinates. Mitigation:
  define a Netsuke-owned schema and serializer, rather than exposing upstream
  `miette` JSON as the public contract.

- Risk: status output and tracing currently write to `stderr`, which
  would corrupt JSON mode. Mitigation: resolve JSON mode early, suppress status
  reporters and tracing when enabled, and add behavioural tests asserting empty
  `stderr` on successful JSON-mode runs.

- Risk: startup failures happen before the merged `Cli` exists.
  Mitigation: add a lightweight raw-args and environment hint resolver for
  `diag_json`, similar in spirit to locale startup resolution, so clap and
  configuration-load failures can still be rendered as JSON when explicitly
  requested.

- Risk: localized strings make snapshots noisy across locales.
  Mitigation: pin snapshot tests to `en-US`, parse JSON into
  `serde_json::Value`, and snapshot the structured value rather than raw
  whitespace.

- Risk: `rstest-bdd` feature edits may not rebuild automatically.
  Mitigation: touch [tests/bdd_tests.rs](/home/user/project/tests/bdd_tests.rs)
  before the final `make test` run if any `.feature` file changes.

## Progress

- [x] (2026-03-07 00:00Z) Reviewed roadmap item `3.10.3`, the existing
      diagnostics pipeline, OrthoConfig usage, and neighbouring
      execplans.
- [x] (2026-03-07 00:00Z) Confirmed the current `Cli` already derives
      `OrthoConfig`, so this feature should extend the existing layered
      configuration surface.
- [x] (2026-03-07 00:00Z) Confirmed `miette` 7.6.0 ships a
      `JSONReportHandler`, but its raw output is not sufficient as
      Netsuke's documented schema.
- [x] (2026-03-07 00:00Z) Drafted this ExecPlan in
      [docs/execplans/3-10-3-json-diagnostics-mode.md](/home/user/project/docs/execplans/3-10-3-json-diagnostics-mode.md).
- [ ] Stage A: add CLI/config plumbing and localized help for
      `diag_json`.
- [ ] Stage B: implement a versioned JSON diagnostic document model and
      serializer.
- [ ] Stage C: integrate JSON mode into startup, error rendering, and
      output-channel selection.
- [ ] Stage D: add `rstest` unit coverage, behavioural coverage, and
      snapshots.
- [ ] Stage E: update user/design documentation, mark the roadmap item
      done, and pass all quality gates.

## Surprises & Discoveries

- The roadmap's configuration section is partly out of date relative to
  the codebase: the existing
  [src/cli/mod.rs](/home/user/project/src/cli/mod.rs) already derives
  `OrthoConfig` and already participates in file, environment, and CLI merging.

- `miette` 7.6.0 includes
  `miette::JSONReportHandler`, but its shape is a flat recursive JSON tree
  without a Netsuke-owned envelope, schema version, or expanded line/column
  fields. That makes it useful as implementation reference, but not as the
  supported wire contract.

- The current hook point for runtime error rendering is centralised in
  [src/main.rs](/home/user/project/src/main.rs), which keeps the implementation
  additive if JSON formatting is introduced there.

- Behaviour tests autodiscover every feature file under
  [tests/features](/home/user/project/tests/features), so a dedicated
  diagnostics feature file can be added without updating a manual test list.

## Decision Log

- Decision: expose a Netsuke-owned JSON document rather than the raw
  `miette` JSON format. Rationale: the user asked for a documented schema and
  compatibility snapshots. That contract should be owned by Netsuke, not by an
  upstream formatter whose field set and ordering may change between dependency
  releases. Date/Author: 2026-03-07 / Codex

- Decision: JSON mode suppresses all status/progress/timing/tracing
  output and reserves `stderr` for machine-readable diagnostics only.
  Rationale: mixed `stderr` output is not machine-readable in practice. This
  also keeps `stdout` free for normal artefacts on successful runs.
  Date/Author: 2026-03-07 / Codex

- Decision: the `diag_json` preference will be layered through the
  existing `Cli` OrthoConfig surface as `diag_json: bool`, with `--diag-json`,
  `NETSUKE_DIAG_JSON`, and `diag_json = true`. Rationale: this satisfies the
  user's requirement to use `ortho_config` for ergonomic layered configuration
  and localized help. Date/Author: 2026-03-07 / Codex

- Decision: snapshot tests will parse JSON into `serde_json::Value` and
  snapshot the structured value with `insta`. Rationale: this guards the schema
  while avoiding churn from insignificant whitespace. Date/Author: 2026-03-07 /
  Codex

- Decision: early-startup failures will honour raw CLI and environment
  hints for `diag_json`; configuration-file-sourced preference is only
  guaranteed after configuration has loaded successfully. Rationale: a file
  cannot reliably request JSON for errors raised while that same file is being
  parsed or validated, and forcing that would require a larger bootstrap
  refactor. Date/Author: 2026-03-07 / Codex

## Outcomes & Retrospective

Not started yet. This section must be rewritten during implementation with
shipped behaviour, validation evidence, and lessons learned.

## Context and orientation

The current diagnostics path is split across a small number of files:

- [src/main.rs](/home/user/project/src/main.rs) parses CLI arguments,
  merges OrthoConfig layers, initializes tracing, calls `runner::run(...)`, and
  renders runtime failures.
- [src/cli/mod.rs](/home/user/project/src/cli/mod.rs) defines the
  existing `Cli` struct and already derives `OrthoConfig`.
- [src/cli_l10n.rs](/home/user/project/src/cli_l10n.rs) maps clap
  argument identifiers to Fluent help keys.
- [src/runner/mod.rs](/home/user/project/src/runner/mod.rs) constructs
  status reporters and executes commands.
- [src/runner/error.rs](/home/user/project/src/runner/error.rs) contains
  `RunnerError`, which already implements `miette::Diagnostic`.
- [src/manifest/diagnostics/mod.rs](/home/user/project/src/manifest/diagnostics/mod.rs)
  and
  [src/manifest/diagnostics/yaml.rs](/home/user/project/src/manifest/diagnostics/yaml.rs)
   create `miette` diagnostics for manifest and YAML failures.

The current output-channel contract is already strong enough to build on:

- `stdout` carries command artefacts such as `manifest -` output and
  graph DOT output.
- `stderr` carries status lines, completion summaries, and diagnostics.

JSON mode must preserve that separation while making `stderr` machine-readable.

The existing test surfaces that should be extended are:

- [tests/cli_tests/parsing.rs](/home/user/project/tests/cli_tests/parsing.rs)
  for flag parsing.
- [tests/cli_tests/merge.rs](/home/user/project/tests/cli_tests/merge.rs)
  for OrthoConfig layer precedence.
- [tests/yaml_error_tests.rs](/home/user/project/tests/yaml_error_tests.rs)
  for span and hint-bearing diagnostics.
- [tests/logging_stderr_tests.rs](/home/user/project/tests/logging_stderr_tests.rs)
  for stream placement.
- [tests/ninja_snapshot_tests.rs](/home/user/project/tests/ninja_snapshot_tests.rs)
  as the local snapshot-testing precedent.
- [tests/features/missing_manifest.feature](/home/user/project/tests/features/missing_manifest.feature)
  and
  [tests/features/progress_output.feature](/home/user/project/tests/features/progress_output.feature)
   as likely behavioural coverage anchors.

## Supported JSON schema

This plan proposes the following stable document schema for v1.

Top-level document:

- `schema_version: u32`
  - Initial value: `1`.
  - This changes only for breaking schema changes.
- `generator: { name: string, version: string }`
  - `name` is always `"netsuke"`.
  - `version` comes from the crate version.
- `diagnostics: Diagnostic[]`
  - Always present.
  - Empty on success is not emitted; successful runs simply produce no
    JSON diagnostics.

Each `Diagnostic` object contains:

- `message: string`
  - The localized top-level diagnostic message.
- `code: string | null`
  - Stable diagnostic code such as `netsuke::manifest::parse`.
- `severity: "error" | "warning" | "advice"`
- `help: string | null`
  - Localized help text, when available.
- `url: string | null`
  - Diagnostic reference URL, when available.
- `causes: string[]`
  - Text chain beneath the top-level message.
- `source: { name: string } | null`
  - Present when a diagnostic has source-backed file or manifest context.
- `primary_span: Span | null`
  - The first label when labels are present, else `null`.
- `labels: Span[]`
  - Every available label, in emitted order.
- `related: Diagnostic[]`
  - Recursive nested diagnostics.

Each `Span` object contains:

- `label: string | null`
- `offset: u64`
- `length: u64`
- `line: u32`
- `column: u32`
- `end_line: u32`
- `end_column: u32`
- `snippet: string | null`
  - The single-line excerpt that contains the span when it can be read
    safely from the backing source.

Notes for implementers:

- The JSON contract is allowed to add optional fields in future minor
  releases, but field removals, renames, or semantic changes require a
  schema-version bump.
- Field order should remain the struct declaration order so snapshots and
  human inspection stay readable.
- `primary_span` is intentionally redundant with `labels[0]`; it makes
  common editor integrations simpler without forcing consumers to infer a
  primary label.

## Plan of work

### Stage A: Add layered CLI/config support and localized help

Extend the existing `Cli` struct rather than inventing a new config wrapper.

Changes:

1. Add `diag_json: bool` to
   [src/cli/mod.rs](/home/user/project/src/cli/mod.rs) with:
   - `#[arg(long)]`
   - `#[ortho_config(default = false)]`
   - a help string describing machine-readable diagnostics
2. Update `Default for Cli`.
3. Wire a new help-key mapping in
   [src/cli_l10n.rs](/home/user/project/src/cli_l10n.rs).
4. Add new Fluent keys in
   [src/localization/keys.rs](/home/user/project/src/localization/keys.rs) and
   translated strings in both locale bundles.
5. Add raw startup hint resolution for `diag_json` from:
   - raw CLI args
   - `NETSUKE_DIAG_JSON`
   This helper exists only for early parse/configuration failures.

Acceptance for Stage A:

- `Cli::try_parse_from(["netsuke", "--diag-json"])` sets
  `diag_json == true`.
- OrthoConfig merge tests prove that `diag_json` follows the normal
  precedence ladder.
- Help text for the new flag localizes through the existing Fluent
  surface.

### Stage B: Implement a versioned diagnostic JSON model and serializer

Introduce a focused module, for example
[src/diagnostic_json.rs](/home/user/project/src/diagnostic_json.rs), to own the
schema and conversion logic.

Changes:

1. Define serializable structs for:
   - `DiagnosticDocument`
   - `GeneratorInfo`
   - `DiagnosticEntry`
   - `DiagnosticSource`
   - `DiagnosticSpan`
2. Add conversion helpers from `&dyn miette::Diagnostic` into the schema.
3. Reuse current source-backed diagnostics to derive:
   - code
   - help
   - labels
   - filename / source name
   - line and column coordinates
   - optional snippets
4. Keep the serializer independent of `JSONReportHandler`, though that
   implementation can be used as a field-discovery reference while building
   tests.

Acceptance for Stage B:

- A YAML parse diagnostic with a span becomes a document containing a
  non-null `primary_span` and a populated `labels` array.
- A runner error without source spans serializes with `source: null` and
  `labels: []`.
- Serialization is deterministic across repeated runs with the same
  locale and input.

### Stage C: Integrate JSON mode into startup and output-channel control

Hook the new serializer into the real execution path without regressing text
mode.

Changes:

1. In [src/main.rs](/home/user/project/src/main.rs):
   - resolve an early `diag_json` hint before clap parsing exits
   - intercept clap parse failures when JSON mode was explicitly
     requested
   - intercept configuration-load failures when JSON mode was requested
     through CLI or environment hints
   - render runtime failures as JSON when the merged CLI enables it
2. Suppress `tracing_subscriber` stderr logging when JSON mode is active.
3. In [src/runner/mod.rs](/home/user/project/src/runner/mod.rs), force
   `SilentReporter` when JSON mode is active so no status lines or timing
   summaries reach `stderr`.
4. Ensure success-path subcommand output remains unchanged on `stdout`.

Acceptance for Stage C:

- `netsuke --diag-json` in an empty directory fails with JSON on
  `stderr`, no text prefixes, and empty `stdout`.
- `netsuke --diag-json manifest -` succeeds with a Ninja manifest on
  `stdout` and empty `stderr`.
- `netsuke --diag-json --verbose ...` still emits clean JSON or empty
  `stderr`; verbose logging does not leak.

### Stage D: Add unit, behavioural, and snapshot coverage

Cover happy paths, unhappy paths, and edge cases in both fast unit tests and
end-to-end behavioural tests.

Unit and integration coverage:

1. Extend
   [tests/cli_tests/parsing.rs](/home/user/project/tests/cli_tests/parsing.rs)
   for `--diag-json`.
2. Extend
   [tests/cli_tests/merge.rs](/home/user/project/tests/cli_tests/merge.rs) for
   defaults, config file, environment, and CLI precedence.
3. Add focused `rstest` cases for serializer behaviour:
   - YAML diagnostic with span and help
   - runner missing-manifest diagnostic without spans
   - nested cause chain
   - JSON mode combined with `verbose`
4. Add snapshot tests using `insta` under a new path such as
   `tests/snapshots/diagnostics/`. Recommended approach:
   - run the real serializer
   - parse output into `serde_json::Value`
   - snapshot the structured value

Behavioural coverage:

1. Add or extend a feature file so these scenarios are observable through
   the compiled binary:
   - failing run emits valid JSON diagnostics
   - successful JSON-mode manifest run keeps `stderr` empty
   - JSON mode suppresses progress/status output
   - JSON mode with `--verbose` still keeps `stderr` machine-readable
2. Add step definitions to parse `stderr` as JSON and assert selected
   fields instead of doing fragile substring-only checks.
3. Touch
   [tests/bdd_tests.rs](/home/user/project/tests/bdd_tests.rs) before the final
   test run if Cargo misses feature-file updates.

Acceptance for Stage D:

- Snapshot files show the supported schema for at least:
  - manifest YAML parse failure
  - missing-manifest runner failure
- Behaviour tests prove stream separation and JSON validity end to end.

### Stage E: Document the interface and close the roadmap item

Finish the contract documentation only after implementation and tests are
stable.

Changes:

1. Update
   [docs/users-guide.md](/home/user/project/docs/users-guide.md) with:
   - the `--diag-json` flag
   - `NETSUKE_DIAG_JSON`
   - config file usage (`diag_json = true`)
   - stream semantics in JSON mode
   - the supported schema, with a short example
2. Update
   [docs/netsuke-design.md](/home/user/project/docs/netsuke-design.md) with the
   implementation decision to use a Netsuke-owned versioned JSON document, plus
   the startup/streaming constraints.
3. Mark `3.10.3` done in
   [docs/roadmap.md](/home/user/project/docs/roadmap.md).

Acceptance for Stage E:

- The user guide is sufficient for a CI integrator to consume JSON
  diagnostics without reading source code.
- The roadmap reflects completion only after the gates pass.

## Validation and acceptance

Before writing implementation code, establish a red test for the missing
behaviour. During implementation, keep the loop explicit: red, green, refactor.

Recommended final validation commands:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/3-10-3-check-fmt.log
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/3-10-3-lint.log
```

```bash
touch tests/bdd_tests.rs
set -o pipefail
make test 2>&1 | tee /tmp/3-10-3-test.log
```

```bash
set -o pipefail
PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/3-10-3-markdownlint.log
```

```bash
set -o pipefail
make nixie 2>&1 | tee /tmp/3-10-3-nixie.log
```

The feature is complete when all of the following are true:

1. Text mode remains unchanged unless `diag_json` is enabled.
2. JSON-mode failures write one valid document to `stderr` and nothing
   else.
3. JSON-mode successes keep `stderr` empty while preserving normal
   `stdout` artefacts.
4. The schema is documented and guarded by snapshots.
5. The roadmap entry is checked off only after the full gate set passes.

## Idempotence and recovery

The implementation steps above are rerunnable. If a step fails:

1. Keep the current stage marked incomplete in `Progress`.
2. Record the unexpected behaviour in `Surprises & Discoveries`.
3. If the failure breaches a tolerance, stop and escalate instead of
   widening scope informally.
4. If the failure is a BDD refresh issue after editing `.feature` files,
   touch [tests/bdd_tests.rs](/home/user/project/tests/bdd_tests.rs) and rerun
   `make test`.

## Approval gate

This document is the draft phase only. Do not begin implementation until the
user explicitly approves this ExecPlan or asks for revisions.
