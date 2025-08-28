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
    to enable serde_yml parsing. *(done)*

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
    `serde_yml::Value` (Stage 2: Initial YAML Parsing), expand `foreach` and
    `when` entries with a Jinja environment (Stage 3: Template Expansion), then
    deserialise the expanded tree into the typed AST and render remaining
    string fields (Stage 4: Deserialisation & Final Rendering).

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

  - [ ] Support user-defined Jinja macros declared in a top-level macros list,
    registering them with the environment before rendering.

- **Success Criterion:**

  - [ ] Netsuke can successfully build a manifest that uses variables,
    conditional logic within string values, the `foreach` and `when` keys,
    custom macros, and the `glob()` function to discover and operate on source
    files.

## Phase 3: The "Friendly" Polish 🛡️

Objective: To implement the advanced features that deliver a superior, secure,
and robust user experience, focusing on security, error reporting, the standard
library, and CLI ergonomics.

- [ ] **Security and Shell Escaping:**

  - [x] Integrate the `shell-quote` crate.

  - [x] Mandate its use for $in/$out substitutions within command strings
    during Ninja file synthesis to prevent command injection. Other
    interpolations are validated with shlex but are not shell-quoted.

  - [x] Emit POSIX-sh-compatible quoting (portable single-quote style) rather
    than Bash-only forms. If Bash-specific quoting is required, document and
    enforce execution under bash.

  - [x] After interpolation, validate the final command string is parsable using
    the shlex crate.

- [ ] **Actionable Error Reporting:**

  - [ ] Adopt the `anyhow` and `thiserror` error handling strategy.

  - [ ] Use thiserror to define specific, structured error types within library

    modules (e.g., IrGenError::RuleNotFound, IrGenError::CircularDependency).

  - [ ] Use anyhow in the application logic to add human-readable context to
    errors as they propagate (e.g., using .with_context()).

  - [ ] Refactor all error-producing code to provide the clear, contextual, and
    actionable error messages specified in the design document.

- [ ] **Template Standard Library:**

  - [ ] Implement the file-system tests (is dir, is file, is readable,
    etc.).

  - [ ] Implement the path and file filters (basename, dirname, with_suffix,
    realpath, contents, hash, etc.).

  - [ ] Implement the generic collection filters (`uniq`, `flatten`,
    `group_by`).

  - [ ] Implement the network and command functions/filters (fetch, shell,
    grep), ensuring shell marks templates as impure to disable caching.

  - [ ] Implement the time helpers (`now`, `timedelta`).

- [ ] **CLI and Feature Completeness:**

  - [ ] Implement the `clean` subcommand by invoking `ninja -t clean`.

  - [ ] Implement the graph subcommand by invoking ninja -t graph to output
    a DOT representation of the dependency graph.

  - [ ] Refine all CLI output for clarity, ensuring help messages are

    descriptive and command feedback is intuitive.

- **Success Criterion:**

  - [ ] Netsuke is a feature-complete, secure, and user-friendly build tool that
    meets all initial design goals, with a comprehensive template standard
    library, robust error handling, and a polished command-line interface.
