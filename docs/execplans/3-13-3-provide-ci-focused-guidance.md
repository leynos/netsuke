# 3.13.3. Provide CI-focused guidance

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (awaiting approval)

## Purpose / big picture

Roadmap item `3.13.3` asks for CI-focused guidance under the "Friendly"
polish phase. Netsuke already ships the core runtime behaviours that automation
needs:

- machine-readable diagnostics via `--diag-json` and
  `output_format = "json"`,
- layered configuration through OrthoConfig,
- low-noise execution by disabling progress output, and
- verbose timing summaries for troubleshooting.

What is missing is a cohesive, tested, user-facing guide that explains how to
combine those behaviours in Continuous Integration (CI) pipelines without
guesswork.

After this work is complete:

1. `docs/users-guide.md` will include a dedicated CI-and-automation subsection
   that explains:
   - how to consume Netsuke's JSON diagnostics safely,
   - how to configure a quiet baseline for automation using the shipped
     OrthoConfig surfaces, and
   - how to enable verbose mode for debugging while preserving the documented
     JSON-mode guarantees.
2. The CI guidance will explicitly show OrthoConfig layering through CLI
   flags, `NETSUKE_...` environment variables, and configuration-file keys,
   using the current localized help and orthographic naming conventions.
3. `rstest` coverage will prove the documented examples are accurate for happy
   paths, unhappy paths, and edge cases, including JSON parsing and
   quiet/verbose combinations.
4. `rstest-bdd` v0.5.0 scenarios will provide executable documentation for the
   user-visible CI workflows.
5. `docs/netsuke-design.md` will record any design decision needed to resolve
   terminology or behavioural ambiguity.
6. `docs/roadmap.md` will mark `3.13.3` done only after the implementation,
   documentation, and full validation gates all pass.

Observable success means a user can copy the documented CI examples, run them
against the real binary, and see the same stream separation and configuration
effects that the docs describe.

## Constraints

- Treat this roadmap item as documentation-backed validation of existing
  automation features unless the audit proves a genuine runtime gap. Do not
  invent a new CLI/config surface just because the roadmap uses the word
  "quiet".
- Reuse the existing OrthoConfig merge root and field vocabulary in
  `src/cli/mod.rs` and `src/cli/config.rs`. The plan must document the shipped
  knobs (`verbose`, `progress`, `spinner_mode`, `diag_json`,
  `output_format`) instead of bypassing them.
- Keep the CLI help/localization path authoritative. Any new user-visible
  wording about automation must remain consistent with the localized help
  surface described in `docs/ortho-config-users-guide.md` and implemented via
  Fluent.
- Add `rstest`-based tests for happy, unhappy, and edge paths.
- Add `rstest-bdd` v0.5.0 behavioural scenarios for the same user-facing
  workflows.
- Reuse existing BDD fixtures and steps where practical. Add a new step module
  only if the current library cannot express the required assertions cleanly.
- Keep files below the 400-line cap in `AGENTS.md`. If a feature file, step
  module, or Rust test file grows too large, split it.
- Use the environment-isolation guidance from `docs/developers-guide.md` and
  `docs/reliable-testing-in-rust-via-dependency-injection.md`. Tests must not
  mutate process-global environment state unsafely.
- Update `docs/users-guide.md` with any user-visible clarification.
- Record design decisions in `docs/netsuke-design.md`.
- Mark roadmap item `3.13.3` done in `docs/roadmap.md` only after all quality
  gates pass.
- Final validation must include logged runs of:
  - `make fmt`
  - `make check-fmt`
  - `make lint`
  - `make test`
  - `make markdownlint`
  - `make nixie`

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 12 changed files or roughly 700 net
  new lines, stop and reassess before proceeding.
- Behaviour gap: if the audit shows the current binary cannot actually express
  the roadmap's intended automation story without a new `--quiet`-style
  surface, stop and escalate with options rather than silently expanding
  scope.
- Documentation drift: if the current runtime behaviour materially disagrees
  with `docs/users-guide.md` and `docs/netsuke-design.md`, stop and decide
  which source is authoritative before pinning tests.
- Test brittleness: if BDD or integration assertions depend on terminal-width
  wrapping, animated progress rendering, or external tools such as `jq`, stop
  and reduce the assertions to stable stream/content checks.
- Environment isolation: if safe CI-style env injection cannot be achieved
  through the existing helpers after two focused attempts, stop and document
  the blocker.
- `.feature` recompilation: if updated feature files do not rebuild, touch
  `tests/bdd_tests.rs` before the final `make test` run.

## Risks

- Risk: roadmap wording says "quiet/verbose modes", but the current runtime
  does not expose a dedicated `--quiet` flag. Severity: high. Likelihood:
  high. Mitigation: start by auditing the shipped behaviour and document
  "quiet for automation" in terms of `spinner_mode = "disabled"` and
  `--progress false` unless the audit proves otherwise. Record that decision in
  `docs/netsuke-design.md`.

- Risk: documentation examples that use `jq` or shell snippets can drift from
  the actual JSON contract. Severity: medium. Likelihood: medium. Mitigation:
  docs may show `jq` for readability, but automated tests should parse the
  JSON with `serde_json` and assert only the stable documented fields.

- Risk: behaviour-driven tests that mutate `NETSUKE_...` variables can become
  flaky in parallel execution. Severity: medium. Likelihood: medium.
  Mitigation: reuse `TestWorld` environment tracking and the helpers described
  in `docs/developers-guide.md`.

- Risk: the guidance could duplicate or contradict the existing advanced usage
  chapter. Severity: low. Likelihood: high. Mitigation: place the new material
  as a focused CI subsection and cross-link rather than re-explaining every
  option in isolation.

- Risk: JSON mode plus verbose mode is already special-cased by the runtime.
  Documentation that explains verbose mode without mentioning that suppression
  would mislead users. Severity: medium. Likelihood: high. Mitigation: make
  the interaction a first-class documented rule and cover it in both BDD and
  `rstest` assertions.

## Progress

- [x] 2026-04-23: Reviewed roadmap item `3.13.3`, neighbouring ExecPlans, and
      the current user/design/configuration/testing documentation.
- [x] 2026-04-23: Audited the shipped automation-related surfaces in
      `docs/users-guide.md`, `docs/netsuke-design.md`, `src/cli/config.rs`,
      `src/runner/mod.rs`, `tests/features/json_diagnostics.feature`, and
      `tests/advanced_usage_tests.rs`.
- [x] 2026-04-23: Drafted this ExecPlan.
- [ ] Stage A: confirm the exact automation contract and final documentation
      placement.
- [ ] Stage B: write the CI-focused user-guide guidance.
- [ ] Stage C: add `rstest-bdd` scenarios for documented CI workflows.
- [ ] Stage D: add `rstest` coverage for JSON consumption and quiet/verbose
      automation combinations.
- [ ] Stage E: update design docs, roadmap, and run full validation.

## Surprises & Discoveries

- The repository already documents most of the raw ingredients for this task:
  `docs/users-guide.md` covers JSON diagnostics, progress suppression,
  verbose timing summaries, and OrthoConfig layering, but not as one cohesive
  CI story.
- The current runtime does **not** expose a dedicated `--quiet` flag.
  Automation-friendly low-noise output currently comes from disabling progress
  with `--progress false` or `spinner_mode = "disabled"`.
- `docs/users-guide.md` already includes one CI-oriented JSON example in
  Section 12.5, so roadmap `3.13.3` should refine and expand that material
  rather than duplicating it blindly.
- Existing tests already cover key automation primitives:
  `tests/features/json_diagnostics.feature`,
  `tests/features/progress_output.feature`, and
  `tests/advanced_usage_tests.rs` can all be extended or reused.
- The current runner behaviour already encodes the crucial automation rule:
  verbose timing output is suppressed when JSON diagnostics are active
  (`src/runner/mod.rs`).

## Decision Log

- Decision: unless Stage A uncovers a real runtime gap, interpret roadmap
  "quiet mode" for CI as the existing progress-suppression controls
  (`spinner_mode` / `progress`) rather than adding a new `--quiet` flag.
  Rationale: this roadmap item is guidance-focused, and the shipped binary
  already provides a low-noise automation path. Date/Author: 2026-04-23 /
  planning agent.

- Decision: document CI guidance in `docs/users-guide.md` as a dedicated
  subsection inside the advanced-usage chapter rather than scattering small
  notes across unrelated sections. Rationale: the user asked specifically for
  CI-focused guidance, and Section 12 already houses the automation-adjacent
  features. Date/Author: 2026-04-23 / planning agent.

- Decision: validate the documentation with both `rstest` and `rstest-bdd`.
  Rationale: `rstest` is the best fit for programmatically parsing JSON and
  asserting precedence combinations, while BDD scenarios keep the documented
  CI workflows readable and executable. Date/Author: 2026-04-23 /
  planning agent.

## Skills and references

This plan should signpost the documents and skills an implementation agent
needs before starting work.

### Skills

- `execplans`: requested for this task, but not installed in the current skill
  registry. Follow the established ExecPlan conventions in `docs/execplans/`
  and keep this document updated as implementation proceeds.
- `skill-installer`: relevant only if a future session needs to install the
  missing `execplans` skill before continuing.

### Primary documentation

- `docs/roadmap.md`: source of truth for roadmap item `3.13.3`.
- `docs/users-guide.md`: current user-facing documentation for configuration,
  progress, verbose timing, and JSON diagnostics.
- `docs/netsuke-design.md`: design-level contract for JSON diagnostics,
  progress reporting, and OrthoConfig-backed preferences.
- `docs/ortho-config-users-guide.md`: layered configuration, orthographic
  naming, merge helpers, and localized help support.
- `docs/rust-testing-with-rstest-fixtures.md`: parameterized `rstest`
  patterns for focused, readable automation tests.
- `docs/rstest-bdd-users-guide.md`: step/scenario authoring rules for
  behaviour-driven coverage.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`: safe
  dependency-injection and isolation guidance, especially for env-driven tests.
- `docs/rust-doctest-dry-guide.md`: guidance for keeping documentation
  examples aligned with executable behaviour.
- `docs/developers-guide.md`: commit gates, BDD harness layout, and safe
  environment mutation utilities.

## Context and orientation

The following repository areas matter for this task.

### Runtime/configuration surfaces

- `src/cli/config.rs`
  - Defines the user-facing OrthoConfig-backed preferences:
    `verbose`, `progress`, `spinner_mode`, `diag_json`, and `output_format`.
  - `CliConfig::resolved_diag_json()` and `resolved_progress()` are the key
    helpers the documentation must describe accurately.
- `src/runner/mod.rs`
  - Applies the effective runtime contract:
    `progress_enabled = cli.resolved_progress() && !cli.resolved_diag_json()`
    and `verbose: cli.verbose && !cli.resolved_diag_json()`.
  - This is the authoritative source for the quiet/verbose/JSON interaction.
- `src/cli/mod.rs` and `src/cli_l10n.rs`
  - Define the CLI surface and localized help mapping that should be cited in
    the docs, not reinterpreted manually.

### Existing documentation to refine

- `docs/users-guide.md`
  - Section 8 already documents progress suppression, timing summaries, and
    output-format layering.
  - Section 12.4 covers configuration layering.
  - Section 12.5 covers JSON diagnostics and already includes one CI example.
- `docs/netsuke-design.md`
  - Section 7.3 records the JSON diagnostics contract.
  - Section 8.4 records the OrthoConfig-backed CLI/configuration design and
    the current progress/verbose semantics.
- `docs/netsuke-cli-design-document.md`
  - Historical reference only. It still mentions a conceptual `quiet` field
    and should be treated as background, not as the acceptance contract.

### Existing tests to reuse or extend

- `tests/features/json_diagnostics.feature`
  - Already covers JSON-mode error/success paths and verbose suppression.
- `tests/features/progress_output.feature`
  - Already covers progress enable/disable behaviour and verbose timing.
- `tests/advanced_usage_tests.rs`
  - Already contains integration coverage for verbose precedence and JSON mode.
- `tests/bdd/steps/json_diagnostics.rs`
  - Reusable assertions for JSON validity and stream cleanliness.
- `tests/bdd/steps/manifest_command.rs`
  - Reusable workspace setup, command execution, and stdout/stderr assertions.

## Plan of work

### Stage A: audit the contract and settle wording

Before editing user-facing docs, confirm the exact behaviour that automation
guidance will promise.

1. Audit the current user guide, design doc, and runtime code for:
   - JSON diagnostics activation (`--diag-json`, `NETSUKE_DIAG_JSON`,
     `output_format = "json"`),
   - quiet automation behaviour (`--progress false`,
     `spinner_mode = "disabled"`),
   - verbose timing behaviour (`--verbose`, `NETSUKE_VERBOSE=true`), and
   - the JSON-plus-verbose interaction.
2. Decide where the new guidance belongs. The current expectation is a new
   subsection under Section 12, likely "CI and automation", with cross-links
   back to Sections 8, 12.4, and 12.5.
3. If the audit confirms that roadmap "quiet" maps to progress suppression
   rather than a dedicated flag, capture that design decision in
   `docs/netsuke-design.md`.

Stage A exit criteria:

- One clear acceptance contract exists for "quiet", "verbose", and JSON mode.
- The planned documentation placement is fixed.
- Any terminology mismatch is resolved in the design document before tests are
  written.

### Stage B: write the CI-focused user guidance

Add a dedicated CI subsection to `docs/users-guide.md` that turns the existing
behaviour into a coherent workflow guide.

The new content should include:

1. A quiet-baseline automation example:
   - `NETSUKE_SPINNER_MODE=disabled` or `NETSUKE_PROGRESS=false`
   - explanation that this suppresses progress noise while keeping real errors
     visible.
2. A verbose troubleshooting example:
   - `NETSUKE_VERBOSE=true`
   - explanation that successful runs gain timing summaries on `stderr`.
3. A structured-diagnostics example:
   - `NETSUKE_OUTPUT_FORMAT=json` or `netsuke --diag-json ...`
   - explicit stream contract: artefacts on `stdout`, diagnostics on `stderr`.
4. At least one example of consuming JSON diagnostics in automation.
   - The docs may use `jq` for readability.
   - The example should only depend on stable fields already documented:
     `schema_version`, `generator`, `diagnostics[*].code`,
     `diagnostics[*].message`.
5. A note on combining modes:
   - quiet + verbose => no progress noise, timing summary on success,
   - JSON + verbose => JSON contract wins; verbose timing/tracing is
     suppressed.
6. A short configuration-file example showing how OrthoConfig makes these
   preferences persistent in CI-oriented `.netsuke.toml` files.

Stage B exit criteria:

- The users guide contains a single, cohesive CI subsection.
- The examples align with the current runtime contract and OrthoConfig naming.
- The wording uses en-GB-oxendict spelling and matches the docs style guide.

### Stage C: add behavioural coverage with `rstest-bdd`

Add a focused behavioural feature that acts as executable documentation for the
CI workflows. Prefer a new `tests/features/ci_guidance.feature` file unless
extending an existing file is clearly simpler and keeps line counts sane.

Planned scenarios:

1. Environment-driven JSON diagnostics in CI:
   - set `NETSUKE_OUTPUT_FORMAT=json`,
   - run a failing command,
   - assert empty `stdout` and valid diagnostics JSON on `stderr`.
2. Quiet automation mode suppresses progress noise:
   - set `NETSUKE_SPINNER_MODE=disabled` or equivalent,
   - run a successful build with fake Ninja progress available,
   - assert progress markers are absent.
3. Verbose automation mode emits timing summaries:
   - set `NETSUKE_VERBOSE=true`,
   - run a successful command,
   - assert timing summary fragments appear on `stderr`.
4. JSON mode suppresses verbose output even when verbose is configured:
   - combine JSON mode with verbose,
   - assert `stderr` is valid JSON (or empty on success), with no timing noise.

Prefer reusing existing steps from:

- `tests/bdd/steps/manifest_command.rs`
- `tests/bdd/steps/json_diagnostics.rs`
- `tests/bdd/steps/progress_output.rs`

Only add a small new step module if the current library lacks:

- env-var setup steps with scenario-safe cleanup, or
- a concise assertion for "stderr omits progress/timing fragments".

Stage C exit criteria:

- Behavioural scenarios cover happy, unhappy, and edge paths.
- The feature text reads like user-facing CI documentation.
- No scenario relies on external tools such as `jq`.

### Stage D: add focused `rstest` coverage

Add or extend `rstest` integration coverage for the lower-level automation
claims that are awkward in BDD.

Recommended home: a new `tests/ci_guidance_tests.rs` module, unless the
existing `tests/advanced_usage_tests.rs` remains comfortably under the file
limit and the new cases fit naturally there.

Tests should cover:

1. JSON diagnostics example validation:
   - run Netsuke in JSON mode,
   - parse `stderr` with `serde_json`,
   - assert the fields used in the docs example exist and have sensible values.
2. OrthoConfig layering for automation:
   - config file enables quiet baseline,
   - environment variable overrides config,
   - CLI flag overrides environment variable.
3. Quiet plus verbose interaction:
   - progress remains suppressed,
   - timing summary still appears on successful human-readable runs.
4. Invalid automation configuration:
   - malformed `spinner_mode` or `output_format`,
   - assert failure is actionable and matches the documented shape.

Use `rstest` parameterized cases to keep the precedence permutations readable.
Use the existing fake-Ninja helpers and env-forwarding helpers rather than
mutating process-global state directly.

Stage D exit criteria:

- `rstest` tests cover the documentation-backed JSON fields and precedence
  claims.
- Happy, unhappy, and edge paths are present.
- The tests remain deterministic and do not rely on shell parsing tools.

### Stage E: design doc, roadmap, and validation

Finish the work by aligning the design record, updating the roadmap, and
running the full gate suite.

1. Update `docs/netsuke-design.md` with the final design decision for CI
   guidance, especially if "quiet" is formally documented as
   progress suppression rather than a dedicated flag.
2. Update `docs/roadmap.md` to mark `3.13.3` done only after all tests and
   documentation changes land successfully.
3. If any `.feature` file changed, `touch tests/bdd_tests.rs` before the final
   `make test`.
4. Run all required gates with `tee` and `set -o pipefail`:
   - `make fmt`
   - `make check-fmt`
   - `make lint`
   - `make test`
   - `make markdownlint`
   - `make nixie`

Stage E exit criteria:

- User guide, design doc, and roadmap all reflect the shipped behaviour.
- All required validation gates pass.
- The roadmap entry is updated only after success.

## Validation and acceptance

The feature is complete when all of the following are true:

1. `docs/users-guide.md` includes a dedicated CI-focused subsection with:
   - JSON-diagnostics consumption examples,
   - quiet/verbose automation guidance,
   - OrthoConfig layering examples for CLI, env, and config-file use.
2. `docs/netsuke-design.md` records the final terminology/behaviour decision.
3. `rstest-bdd` scenarios prove the documented CI workflows end to end.
4. `rstest` coverage proves the JSON fields and precedence combinations used in
   the docs.
5. `make fmt`, `make check-fmt`, `make lint`, `make test`,
   `make markdownlint`, and `make nixie` all pass.
6. `docs/roadmap.md` marks `3.13.3` done.

## Outcomes & Retrospective

Pending implementation approval.
