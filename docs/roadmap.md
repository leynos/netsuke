# Netsuke implementation roadmap

This roadmap translates the [netsuke-design.md](netsuke-design.md) document
into a phased, actionable implementation plan. Each phase has a clear objective
and a checklist of tasks that must be completed to meet the success criteria.

## 1. The Static Core

Objective: To create a minimal, working build compiler capable of handling
static manifests without any templating. This phase validates the entire static
compilation pipeline from parsing to execution.

### 1.1. CLI and manifest parsing

- [x] 1.1.1. Implement initial clap CLI structure for build command and global
  options. See [netsuke-design.md §8.2](netsuke-design.md).
  - [x] Define --file, --directory, and --jobs options.
- [x] 1.1.2. Define core Abstract Syntax Tree data structures in `src/ast.rs`.
  - [x] Implement NetsukeManifest, Rule, Target, StringOrList, and Recipe
    structs.
- [x] 1.1.3. Annotate AST structs with serde attributes.
  - [x] Add `#[derive(Deserialize)]` and `#[serde(deny_unknown_fields)]` to
    enable serde_saphyr parsing.
- [x] 1.1.4. Implement netsuke_version field parsing with semver validation.
- [x] 1.1.5. Support `phony` and `always` boolean flags on targets.
- [x] 1.1.6. Parse actions list, treating each entry as a target with
  `phony: true`.
- [x] 1.1.7. Implement YAML parsing logic to deserialize static Netsukefile into
  NetsukeManifest AST.

### 1.2. Intermediate Representation and validation

- [x] 1.2.1. Define IR data structures in `src/ir.rs`. See
  [netsuke-design.md §5.2](netsuke-design.md).
  - [x] Implement BuildGraph, Action, and BuildEdge structs.
  - [x] Keep IR backend-agnostic per design.
- [x] 1.2.2. Implement ir::from_manifest transformation logic. See
  [netsuke-design.md §5.3](netsuke-design.md).
  - [x] Convert AST to BuildGraph IR.
- [x] 1.2.3. Consolidate and deduplicate rules into ir::Action structs based on
  property hash during transformation.
- [x] 1.2.4. Implement validation for rule, command, and script references.
  - [x] Ensure references are valid and mutually exclusive.
- [x] 1.2.5. Implement cycle detection algorithm using depth-first search. See
  [netsuke-design.md §5.3](netsuke-design.md).
  - [x] Fail compilation on circular dependency detection.

### 1.3. Code generation and execution

- [x] 1.3.1. Implement Ninja file synthesizer in `src/ninja_gen.rs`. See
  [netsuke-design.md §5.4](netsuke-design.md).
  - [x] Traverse BuildGraph IR.
- [x] 1.3.2. Generate Ninja rule and build statements.
  - [x] Write rule statements from ir::Action structs.
  - [x] Write build statements from ir::BuildEdge structs.
- [x] 1.3.3. Implement process management in `main.rs`.
  - [x] Invoke ninja executable as subprocess using `std::process::Command`.

**Success criterion:** Netsuke can successfully take a Netsukefile without
Jinja syntax, compile it to a `build.ninja` file, and execute it via the ninja
subprocess to produce the correct build artefacts. Validated via CI workflow.

## 2. The Dynamic Engine

Objective: To integrate the minijinja templating engine, enabling dynamic build
configurations with variables, control flow, and custom functions.

### 2.1. Jinja integration

- [x] 2.1.1. Integrate the `minijinja` crate into the build pipeline. See
  [netsuke-design.md §4.1](netsuke-design.md).
- [x] 2.1.2. Implement data-first parsing pipeline.
  - [x] Stage 2: Parse manifest into `serde_json::Value` (Initial YAML Parsing).
  - [x] Stage 3: Expand `foreach` and `when` entries with Jinja environment
    (Template Expansion).
  - [x] Stage 4: Deserialize expanded tree into typed AST and render remaining
    string fields (Deserialization & Final Rendering).
- [x] 2.1.3. Create minijinja::Environment and populate with global vars from
  manifest. See [netsuke-design.md §4.2](netsuke-design.md).

### 2.2. Dynamic features and custom functions

- [x] 2.2.1. Remove global first-pass Jinja parsing.
  - [x] Ensure manifests are valid YAML before any templating occurs.
- [x] 2.2.2. Restrict Jinja expressions to string values only.
  - [x] Forbid structural tags such as `{% if %}` and `{% for %}`.
- [x] 2.2.3. Implement `foreach` and `when` keys for target generation. See
  [netsuke-design.md §2.5](netsuke-design.md).
  - [x] Expose `item` and optional `index` variables.
  - [x] Layer per-iteration locals over `target.vars` and manifest globals for
    subsequent rendering phases.
- [x] 2.2.4. Implement `env(var_name)` custom Jinja function for reading system
  environment variables. See [netsuke-design.md §4.4](netsuke-design.md).
- [x] 2.2.5. Implement `glob(pattern)` custom function for file path globbing.
  See [netsuke-design.md §4.4](netsuke-design.md).
  - [x] Sort results lexicographically.
- [x] 2.2.6. Support user-defined Jinja macros declared in top-level macros
  list. See [netsuke-design.md §4.3](netsuke-design.md).
  - [x] Register macros with environment before rendering.

**Success criterion:** Netsuke can successfully build a manifest that uses
variables, conditional logic within string values, the `foreach` and `when`
keys, custom macros, and the `glob()` function to discover and operate on
source files.

### 2.3. YAML parser migration

- [x] 2.3.1. Draft ADR evaluating maintained replacements for `serde_yml`.
  - [x] Evaluate `serde_yaml_ng` and alternatives.
  - [x] Record migration decision.
- [x] 2.3.2. Migrate parser to `serde_saphyr`.
  - [x] Exercise manifest fixtures to capture compatibility notes.
  - [x] Document required mitigations.

## 3. The "Friendly" polish

Objective: To implement the advanced features that deliver a superior, secure,
and robust user experience, focusing on security, error reporting, the standard
library, and CLI ergonomics.

### 3.1. Security and shell escaping

- [x] 3.1.1. Integrate the `shell-quote` crate.
- [x] 3.1.2. Mandate shell-quote use for variable substitutions. See
  [netsuke-design.md §6.2](netsuke-design.md).
  - [x] Prevent command injection during IR generation.
  - [x] Validate final command string with shlex.
- [x] 3.1.3. Emit POSIX-sh-compatible quoting. See
  [netsuke-design.md §6.3](netsuke-design.md).
  - [x] Use portable single-quote style rather than Bash-only forms.
  - [x] Document and enforce bash execution if Bash-specific quoting is
    required.
- [x] 3.1.4. Validate final command string is parsable using shlex crate after
  interpolation.

### 3.2. Actionable error reporting

- [x] 3.2.1. Adopt `anyhow` and `thiserror` error handling strategy. See
  [netsuke-design.md §7.2](netsuke-design.md).
- [x] 3.2.2. Define structured error types using thiserror in library modules.
  See [netsuke-design.md §7.2](netsuke-design.md).
  - [x] Implement IrGenError::RuleNotFound, IrGenError::CircularDependency, and
    similar types.
- [x] 3.2.3. Use anyhow in application logic for human-readable context.
  - [x] Apply `.with_context()` for error propagation.
- [x] 3.2.4. Use `miette` to render diagnostics with source spans and helpful
  messages. See [netsuke-design.md §7.2](netsuke-design.md).
- [x] 3.2.5. Refactor all error-producing code to provide clear, contextual, and
  actionable error messages. See [netsuke-design.md §7](netsuke-design.md).

### 3.3. Template standard library

- [x] 3.3.1. Implement basic file-system tests. See
  [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Implement `dir`, `file`, `symlink`, `pipe`, `block_device`,
    `char_device`, and legacy `device` tests.
- [x] 3.3.2. Implement path and file filters. See
  [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Implement basename, dirname, with_suffix, realpath, contents, hash, and
    similar filters.
- [x] 3.3.3. Implement generic collection filters. See
  [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Implement `uniq`, `flatten`, and `group_by`.
- [x] 3.3.4. Implement network and command functions/filters. See
  [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Implement fetch, shell, and grep.
  - [x] Ensure shell marks templates as impure to disable caching.
- [x] 3.3.5. Implement time helpers. See
  [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Implement `now` and `timedelta`.

### 3.4. CLI and feature completeness

- [x] 3.4.1. Implement `clean` subcommand. See
  [netsuke-design.md §8.3](netsuke-design.md).
  - [x] Invoke `ninja -t clean`.
- [x] 3.4.2. Implement `graph` subcommand. See
  [netsuke-design.md §8.3](netsuke-design.md).
  - [x] Invoke `ninja -t graph` to output DOT representation of dependency
    graph.
- [x] 3.4.3. Refine all CLI output for clarity.
  - [x] Ensure help messages are descriptive.
  - [x] Ensure command feedback is intuitive.
- [x] 3.4.4. Implement `manifest` subcommand. See
  [netsuke-design.md §8.3](netsuke-design.md).
  - [x] Persist generated Ninja file without executing.
  - [x] Include integration tests for writing to disk and streaming to stdout.
- [ ] 3.4.5. Extend graph subcommand with optional `--html` renderer.
  - [ ] Produce browsable graph visualization.
  - [ ] Document text-only fallback workflow.
- [ ] 3.4.6. Evaluate `netsuke explain <code>` command for diagnostic codes.
  - [ ] Capture decision and rationale in architecture docs.

### 3.5. Executable discovery filter

- [x] 3.5.1. Implement cross-platform `which` MiniJinja filter and function
  alias. See [netsuke-design.md §4.7](netsuke-design.md).
  - [x] Expose `all`, `canonical`, `fresh`, and `cwd_mode` keyword arguments.
- [x] 3.5.2. Integrate finder with Stage 3/4 render cache.
  - [x] Include `PATH`, optional `PATHEXT`, current directory, and option flags
    in memoization key.
  - [x] Keep helper pure by default.
- [x] 3.5.3. Provide LRU cache with metadata self-healing.
  - [x] Avoid stale hits.
  - [x] Honour `fresh=true` without discarding cached entries.
- [x] 3.5.4. Emit actionable diagnostics.
  - [x] Implement `netsuke::jinja::which::not_found` and
    `netsuke::jinja::which::args` diagnostics.
  - [x] Include PATH previews and platform-appropriate hints.
- [x] 3.5.5. Cover POSIX and Windows behaviour with tests.
  - [x] Test canonicalization, list-all mode, and cache validation with unit
    tests.
  - [x] Add MiniJinja fixtures asserting deterministic renders across repeated
    invocations.

### 3.6. Onboarding and defaults

- [x] 3.6.1. Ensure default subcommand builds manifest defaults.
  - [x] Emit guided error and hint for missing-manifest scenarios. See CLI
    design.
  - [x] Guard with integration tests.
- [x] 3.6.2. Curate OrthoConfig-generated Clap help output.
  - [x] Ensure every subcommand and flag has plain-language, localizable
    description. See style guide.
- [x] 3.6.3. Publish "Hello World" quick-start walkthrough.
  - [x] Demonstrate running Netsuke end-to-end.
  - [x] Exercise via documentation test or example build fixture.

### 3.7. Localization with Fluent

- [x] 3.7.1. Externalize user-facing strings into Fluent `.ftl` bundles.
  - [x] Implement compile-time audit that fails CI on missing message keys.
- [x] 3.7.2. Implement locale resolution.
  - [x] Support `--locale`, `NETSUKE_LOCALE`, configuration files, and system
    defaults.
  - [x] Fall back to `en-US` when translations are absent.
- [x] 3.7.3. Translator tooling and documentation published.
  - [x] `docs/translators-guide.md` covers FTL syntax, key conventions, variable
    catalogue, plural forms, and adding new locales.
  - [x] Plural form examples (`example.files_processed`, `example.errors_found`)
    added to en-US and es-ES FTL files with corresponding key constants.
  - [x] Localization smoke tests verify en-US and es-ES message resolution.

### 3.8. Accessibility and Section 508 compliance

- [x] 3.8.1. Add accessible output mode.
  - [x] Auto-enable for `TERM=dumb`, `NO_COLOR`, or explicit config.
  - [x] Replace spinners with static status lines.
  - [x] Guarantee textual labels for every status.
- [x] 3.8.2. Respect accessibility preferences.
  - [x] Honour `NO_COLOR`, `NETSUKE_NO_EMOJI`, and ASCII-only preferences.
  - [x] Keep semantic prefixes (Error, Warning, Success) in all modes.
- [ ] 3.8.3. Conduct assistive technology verification.
  - [ ] Test with NVDA on Windows, VoiceOver on macOS, and a Linux screen
    reader.
  - [ ] Document results and corrective actions.

### 3.9. Real-time feedback and progress

- [x] 3.9.1. Integrate `indicatif::MultiProgress`.
  - [x] Surface the six pipeline stages with persistent summaries.
  - [x] Apply localization-aware labelling.
- [x] 3.9.2. Parse Ninja status lines to drive task progress.
  - [x] Emit fallback textual updates when stdout is not a TTY or accessible
    mode is active.
- [ ] 3.9.3. Capture per-stage timing metrics in verbose mode.
  - [ ] Include metrics in completion summary.
  - [ ] Avoid noise in default output.

### 3.10. Output channels and diagnostics

- [ ] 3.10.1. Guarantee status message and subprocess output ordering.
  - [ ] Stream Netsuke status messages to stderr.
  - [ ] Preserve subprocess output ordering on stdout.
  - [ ] Verify with end-to-end tests redirecting each stream.
- [ ] 3.10.2. Introduce consistent prefixes for log differentiation.
  - [ ] Use localizable prefixes or indentation rules.
  - [ ] Support ASCII and Unicode themes.
- [ ] 3.10.3. Deliver `--diag-json` machine-readable diagnostics mode.
  - [ ] Document schema.
  - [ ] Add snapshot tests to guard compatibility.

### 3.11. Configuration and preferences

- [ ] 3.11.1. Introduce `CliConfig` struct derived with `OrthoConfig`. See
  [ortho-config-users-guide.md](ortho-config-users-guide.md).
  - [ ] Share schema across Clap integration, configuration files, and
    environment variables.
  - [ ] Cover verbosity, colour policy, locale, spinner mode, output format,
    default targets, and theme.
- [ ] 3.11.2. Discover configuration files in project and user scopes.
  - [ ] Honour env overrides and CLI precedence.
  - [ ] Add integration tests for each precedence tier. See
    [ortho-config-users-guide.md](ortho-config-users-guide.md).
- [ ] 3.11.3. Expose `--config <path>` and `NETSUKE_CONFIG`.
  - [ ] Select alternative config files.
  - [ ] Ship annotated sample configs in documentation.
- [ ] 3.11.4. Add regression tests for OrthoConfig precedence ladder.
  - [ ] Test defaults < file < env < CLI precedence. See
    [ortho-config-users-guide.md](ortho-config-users-guide.md).

### 3.12. Visual design validation

- [ ] 3.12.1. Define design tokens for colours, symbols, and spacing.
  - [ ] Wire tokens through CLI theme system.
  - [ ] Ensure ASCII and Unicode modes remain consistent.
- [ ] 3.12.2. Snapshot progress and status output for themes.
  - [ ] Cover unicode and ascii themes.
  - [ ] Guard alignment and wrapping against regressions.
- [ ] 3.12.3. Test output renderings across common terminals.
  - [ ] Test Windows Console, PowerShell, and xterm-compatible shells.
  - [ ] Document any conditional handling.

### 3.13. User journey support

- [ ] 3.13.1. Add smoke tests for novice flows.
  - [ ] Test first run success, missing manifest, and help output.
  - [ ] Confirm UX matches documented journey.
- [ ] 3.13.2. Extend user documentation with advanced usage chapter.
  - [ ] Cover `clean`, `graph`, `manifest`, configuration layering, and JSON
    diagnostics.
- [ ] 3.13.3. Provide CI-focused guidance.
  - [ ] Include examples of consuming JSON diagnostics.
  - [ ] Document configuring quiet/verbose modes for automation.

**Success criterion:** Netsuke ships a localizable, accessible, and fully
configurable CLI that delivers real-time feedback, machine-readable
diagnostics, and the onboarding experience defined in the Netsuke CLI design
document.
