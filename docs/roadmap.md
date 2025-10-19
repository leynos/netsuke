# Netsuke Implementation Roadmap

This roadmap translates the [netsuke-design.md](netsuke-design.md) document
into a phased, actionable implementation plan. Each phase has a clear objective
and a checklist of tasks that must be completed to meet the success criteria.

## Phase 1: The Static Core 🏗️

Objective: To create a minimal, working build compiler capable of handling
static manifests without any templating. This phase validates the entire static
compilation pipeline from parsing to execution.

- [ ] **CLI and Manifest Parsing:**

  - [x] Implement the initial clap CLI structure for the build command and
    global options (--file, --directory, --jobs), as defined in the design
    document. *(done)*

  - [x] Define the core Abstract Syntax Tree (AST) data structures
    (NetsukeManifest, Rule, Target, StringOrList, Recipe) in `src/ast.rs`.
    *(done)*

  - [x] Annotate AST structs with #[derive(Deserialize)] and
    #[serde(deny_unknown_fields)]
    to enable serde_saphyr parsing. *(done)*

  - [x] Implement parsing for the netsuke_version field and validate it using
    the semver crate. *(done)*

  - [x] Support `phony` and `always` boolean flags on targets. *(done)*

  - [x] Parse the actions list, treating each entry as a target with
    phony: true. *(done)*

  - [x] Implement the YAML parsing logic to deserialize a static Netsukefile
    into the NetsukeManifest AST. *(done)*

- [ ] **Intermediate Representation (IR) and Validation:**

  - [x] Define the IR data structures (BuildGraph, Action, BuildEdge) in
    `src/ir.rs`, keeping it backend-agnostic as per the design. *(done)*

  - [x] Implement the ir::from_manifest transformation logic to convert the
    AST into the BuildGraph IR. *(done)*

  - [x] During transformation, consolidate and deduplicate rules into ir::Action
    structs based on a hash of their properties. *(done)*

  - [x] Implement validation to ensure that every rule, command, or script
    referenced by a target is valid and that they are mutually exclusive.
    *(done)*

  - [x] Implement a cycle detection algorithm (e.g., depth-first search) to fail
    compilation if a circular dependency is found in the target graph. *(done)*

- [ ] **Code Generation and Execution:**

  - [x] Implement the Ninja file synthesizer in
    [src/ninja_gen.rs](src/ninja_gen.rs) to traverse the BuildGraph IR. *(done)*

  - [x] Write logic to generate Ninja rule statements from ir::Action structs
    and build statements from ir::BuildEdge structs. *(done)*

  - [x] Implement the process management logic in `main.rs` to invoke the ninja
    executable as a subprocess using `std::process::Command`. *(done)*

- **Success Criterion:**

  - [x] Netsuke can successfully take a Netsukefile without any Jinja syntax,
    compile it to a `build.ninja` file, and execute it via the ninja subprocess
    to produce the correct build artefacts. *(validated via CI workflow)*

## Phase 2: The Dynamic Engine ✨

Objective: To integrate the minijinja templating engine, enabling dynamic build
configurations with variables, control flow, and custom functions.

- [x] **Jinja Integration:**

  - [x] Integrate the `minijinja` crate into the build pipeline.

  - [x] Implement data-first parsing: parse the manifest into a
    `serde_json::Value` (Stage 2: Initial YAML Parsing), expand `foreach` and
    `when` entries with a Jinja environment (Stage 3: Template Expansion), then
    deserialise the expanded tree into the typed AST and render remaining
    string fields (Stage 4: Deserialization & Final Rendering).

  - [x] Create a minijinja::Environment and populate its initial context with
    the global vars defined in the manifest.

- [ ] **Dynamic Features and Custom Functions:**

  - [x] Remove the global first-pass Jinja parsing, so that manifests are
        valid YAML before any templating occurs.

  - [x] Evaluate Jinja expressions only within string values, forbidding
        structural tags such as `{% if %}` and `{% for %}`.

  - [x] Implement the `foreach` and `when` keys for target generation,
        exposing `item` and optional `index` variables and layering
        per-iteration locals over `target.vars` and manifest globals for
        subsequent rendering phases.

  - [x] Implement the essential custom Jinja function env(var_name) to read
    system environment variables.

  - [x] Implement the critical glob(pattern) custom function to perform file
     path globbing, with results sorted lexicographically.

  - [x] Support user-defined Jinja macros declared in a top-level macros list,
    registering them with the environment before rendering.

- **Success Criterion:**

  - [ ] Netsuke can successfully build a manifest that uses variables,
    conditional logic within string values, the `foreach` and `when` keys,
    custom macros, and the `glob()` function to discover and operate on source
    files.

- [ ] **YAML Parser Migration:**

  - [x] Draft an ADR evaluating maintained replacements for `serde_yml`
        (for example `serde_yaml_ng`) and record the migration decision.
  - [x] Migrate the parser to `serde_saphyr`, exercising the manifest fixtures
        to capture compatibility notes and required mitigations.

## Phase 3: The "Friendly" Polish 🛡️

Objective: To implement the advanced features that deliver a superior, secure,
and robust user experience, focusing on security, error reporting, the standard
library, and CLI ergonomics.

- [ ] **Security and Shell Escaping:**

  - [x] Integrate the `shell-quote` crate.

  - [x] Mandate its use for variable substitutions within command strings
    during IR generation to prevent command injection, and validate the final
    command string with shlex.

  - [x] Emit POSIX-sh-compatible quoting (portable single-quote style)
    rather than Bash-only forms. If Bash-specific quoting is required, document
    and enforce execution under bash.

  - [x] After interpolation, validate the final command string is parsable using
    the shlex crate.

- [x] **Actionable Error Reporting:**

  - [x] Adopt the `anyhow` and `thiserror` error handling strategy.

  - [x] Use thiserror to define specific, structured error types within library

    modules (e.g., IrGenError::RuleNotFound, IrGenError::CircularDependency).

  - [x] Use anyhow in the application logic to add human-readable context to
    errors as they propagate (e.g., using .with_context()).
  - [x] Use `miette` to render diagnostics with source spans and helpful
    messages.

  - [x] Refactor all error-producing code to provide the clear, contextual, and
    actionable error messages specified in the design document.

- [x] **Template Standard Library:**

  - [x] Implement the basic file-system tests (`dir`, `file`, `symlink`,
    `pipe`, `block_device`, `char_device`, legacy `device`). *(done)*

  - [x] Implement the path and file filters (basename, dirname, with_suffix,
    realpath, contents, hash, etc.).

  - [x] Implement the generic collection filters (`uniq`, `flatten`,
    `group_by`). *(done)*

  - [x] Implement the network and command functions/filters (fetch, shell,
    grep), ensuring shell marks templates as impure to disable caching.

  - [x] Implement the time helpers (`now`, `timedelta`).

- [ ] **CLI and Feature Completeness:**

  - [ ] Implement the `clean` subcommand by invoking `ninja -t clean`.

  - [ ] Implement the graph subcommand by invoking ninja -t graph to output
    a DOT representation of the dependency graph.

  - [ ] Refine all CLI output for clarity, ensuring help messages are
    descriptive and command feedback is intuitive.

  - [ ] Implement the `manifest` subcommand to persist the generated Ninja file
    without executing it, including integration tests that cover writing to
    disk and streaming to stdout.

  - [ ] Extend the graph subcommand with an optional `--html` renderer that
    produces a browsable graph visualization while documenting a text-only
    fallback workflow.

  - [ ] Evaluate and document whether to ship a `netsuke explain <code>`
    command for diagnostic codes, capturing the decision and rationale in the
    architecture docs.

- [ ] **Welcoming Onboarding and Defaults:**

  - [ ] Ensure running `netsuke` with no subcommand builds manifest defaults,
    and missing-manifest scenarios emit the guided error and hint specified in
    the CLI design, guarded by integration tests.

  - [ ] Curate OrthoConfig-generated Clap help output so every subcommand and
    flag has a plain-language, localizable description aligned with the style
    guide.

  - [ ] Publish a “Hello World” quick-start walkthrough that demonstrates
    running Netsuke end-to-end, exercised by a documentation test or example
    build fixture.

- [ ] **Localization with Fluent:**

  - [ ] Externalize all user-facing strings into Fluent `.ftl` bundles with a
    compile-time audit that fails CI when a message key is missing.

  - [ ] Implement locale resolution via `--locale`, `NETSUKE_LOCALE`,
    configuration files, and system defaults, falling back to `en-US` when
    translations are absent.

  - [ ] Provide translator tooling and documentation covering message keys,
    plural forms, and variable usage, and ensure localization smoke tests cover
    at least one secondary locale.

- [ ] **Accessibility and Section 508 Compliance:**

  - [ ] Add an accessible output mode (auto-enabled for `TERM=dumb`,
    `NO_COLOR`, or explicit config) that replaces spinners with static status
    lines and guarantees textual labels for every status.

  - [ ] Respect `NO_COLOR`, `NETSUKE_NO_EMOJI`, and ASCII-only preferences
    while keeping semantic prefixes (Error, Warning, Success) in all modes.

  - [ ] Conduct assistive technology verification (NVDA on Windows,
    VoiceOver on macOS, and a Linux screen reader), documenting results and
    corrective actions.

- [ ] **Real-Time Feedback and Progress:**

  - [ ] Integrate `indicatif::MultiProgress` to surface the six pipeline stages
    with persistent summaries and localization-aware labelling.

  - [ ] Parse Ninja status lines to drive task progress, ensuring fallback
    textual updates are emitted when stdout is not a TTY or accessible mode is
    active.

  - [ ] Capture per-stage timing metrics in verbose mode and include them in
    the completion summary while avoiding noise in default output.

- [ ] **Output Channels and Diagnostics:**

  - [ ] Guarantee Netsuke status messages always stream to stderr and
    subprocess output preserves ordering on stdout, verified by end-to-end
    tests that redirect each stream.

  - [ ] Introduce consistent, localizable prefixes or indentation rules that
    differentiate Netsuke logs from child process output, with ASCII and
    Unicode themes.

  - [ ] Deliver a `--diag-json` machine-readable diagnostics mode with a
    documented schema, plus snapshot tests to guard compatibility.

- [ ] **Configuration and Preferences:**

  - [ ] Introduce a `CliConfig` struct derived with `OrthoConfig` so Clap
    integration, configuration files, and environment variables share one
    schema covering verbosity, colour policy, locale, spinner mode, output
    format, default targets, and theme.

  - [ ] Discover configuration files in project and user scopes, honouring env
    overrides and CLI precedence, with integration tests for each precedence
    tier.

  - [ ] Expose `--config <path>` (and `NETSUKE_CONFIG`) to select alternative
    config files, and ship annotated sample configs in the documentation.

  - [ ] Add regression tests that exercise OrthoConfig’s precedence ladder
    (defaults < file < env < CLI) to ensure the Clap facade remains aligned
    with the design document.

- [ ] **Visual Design Validation:**

  - [ ] Define design tokens for colours, symbols, and spacing, and wire them
    through the CLI theme system so ASCII and Unicode modes remain consistent.

  - [ ] Snapshot progress and status output for unicode and ascii themes to
    guard alignment and wrapping against regressions.

  - [ ] Test output renderings across common terminals (Windows Console,
    PowerShell, xterm-compatible shells) and document any conditional handling.

- [ ] **User Journey Support:**

  - [ ] Add smoke tests that exercise novice flows (first run success, missing
    manifest, help output) and confirm UX matches the documented journey.

  - [ ] Extend user documentation with an advanced usage chapter covering
    `clean`, `graph`, `manifest`, configuration layering, and JSON diagnostics.

  - [ ] Provide CI-focused guidance, including examples of consuming JSON
    diagnostics and configuring quiet/verbose modes for automation.

- **Success Criterion:**

  - [ ] Netsuke ships a localizable, accessible, and fully configurable CLI
    that delivers real-time feedback, machine-readable diagnostics, and the
    onboarding experience defined in the Netsuke CLI design document.
