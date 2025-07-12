# Netsuke Implementation Roadmap

## Phase 1: The Static Core 🏗️

**Objective:** To create a minimal, working build compiler capable of handling
static manifests without any templating. This phase validates the entire static
compilation pipeline from parsing to execution.

- [ ] **CLI and Manifest Parsing:**

  - [ ] Implement the initial command-line interface structure using `clap` with
    the `derive` feature, as defined in `src/main.rs`. Focus on the default
    `build` command and global options like `--file`, `--directory`, and
    `--jobs`.

  - [ ] Define the core Abstract Syntax Tree (AST) data structures
    (`NetsukeManifest`, `Rule`, `Target`, `StringOrList`) in `src/ast.rs`.

  - [ ] Annotate the AST structs with `#[derive(Deserialize)]` and
    `#[serde(deny_unknown_fields)]` to enable parsing.

  - [ ] Implement the YAML parsing logic using `serde_yaml` to deserialize a
    static `Netsuke.yml` file into the `NetsukeManifest` AST.

- [ ] **Intermediate Representation (IR) and Validation:**

  - [ ] Define the Intermediate Representation (IR) data structures
    (`BuildGraph`, `Action`, `BuildEdge`) in `src/ir.rs` to mirror Ninja's
    conceptual model.

  - [ ] Implement the transformation logic (`ir::from_manifest`) to convert the
    `NetsukeManifest` AST into the `BuildGraph` IR.

  - [ ] During transformation, consolidate and deduplicate rules into
    `ir::Action` structs.

  - [ ] Implement basic validation within the AST-to-IR process, specifically
    ensuring that every rule referenced by a target actually exists.

  - [ ] Implement a cycle detection algorithm (e.g., depth-first search) to fail
    compilation if a circular dependency is found in the target graph.

- [ ] **Code Generation and Execution:**

  - [ ] Implement the Ninja file synthesizer in `src/ninja_gen.rs` to traverse
    the `BuildGraph` IR.

  - [ ] Write logic to generate Ninja `rule` statements from `ir::Action`
    structs and `build` statements from `ir::BuildEdge` structs.

  - [ ] Ensure correct translation of placeholders like `{ins}` and `{outs}` to
    Ninja's `$in` and `$out`.

  - [ ] Implement the process management logic in `main.rs` to invoke the
    `ninja` executable as a subprocess using `std::process::Command`.

- **Success Criterion:**

  - [ ] Netsuke can successfully take a `Netsuke.yml` file **without any Jinja
    syntax**, compile it to a `build.ninja` file, and execute it via the `ninja`
    subprocess to produce the correct build artifacts.

______________________________________________________________________

## Phase 2: The Dynamic Engine ✨

**Objective:** To integrate the `minijinja` templating engine, enabling dynamic
build configurations with variables, control flow, and custom functions.

- [ ] **Jinja Integration:**

  - [ ] Integrate the `minijinja` crate into the build pipeline.

  - [ ] Implement the two-pass parsing mechanism: the first pass renders the
    manifest as a Jinja template, and the second pass parses the resulting pure
    YAML string with `serde_yaml`.

  - [ ] Create a `minijinja::Environment` and populate its initial context with
    the global `vars` defined in the manifest.

- [ ] **Dynamic Features and Custom Functions:**

  - [ ] Implement support for basic Jinja control structures (`{% if %}`,
    `{% for %}`) and variable substitution.

  - [ ] Implement the essential custom Jinja function `env(var_name)` to read
    system environment variables.

  - [ ] Implement the critical `glob(pattern)` custom function to perform file
    path globbing, bridging a key feature gap not supported by Ninja.

  - [ ] Support user-defined Jinja macros declared in a top-level `macros` list.

- **Success Criterion:**

  - [ ] Netsuke can successfully build a manifest that uses variables,
    conditional logic (e.g., different flags based on a variable), custom
    macros, and the `glob()` function to discover source files.

______________________________________________________________________

## Phase 3: The "Friendly" Polish 🛡️

**Objective:** To implement the advanced features that deliver a superior,
secure, and robust user experience, focusing on security, error reporting, and
CLI ergonomics.

- [ ] **Security and Shell Escaping:**

  - [ ] Integrate the `shell-quote` crate.

  - [ ] **Mandate** its use for all variable substitutions within the `command`
    strings during Ninja file synthesis to prevent command injection
    vulnerabilities. This is a critical security feature.

  - [ ] Implement custom Jinja filters for shell safety and path manipulation,
    such as `| shell_escape`, `| to_path`, and `| parent`.

- [ ] **Actionable Error Reporting:**

  - [ ] Adopt the `anyhow` and `thiserror` error handling strategy.

  - [ ] Use `thiserror` to define specific, structured error types within
    library modules (e.g., `IrGenError::RuleNotFound`).

  - [ ] Use `anyhow` in the application logic to add human-readable context to
    errors as they propagate up the call stack (e.g., using `.with_context()`).

  - [ ] Refactor all error-producing code to provide the clear, contextual, and
    actionable error messages specified in the design document's error reporting
    table.

- [ ] **CLI and Feature Completeness:**

  - [ ] Implement the `clean` subcommand by invoking `ninja -t clean`.

  - [ ] Implement the `graph` subcommand by invoking `ninja -t graph` to output
    a DOT representation of the dependency graph.

  - [ ] Refine all CLI output for clarity, ensuring help messages are
    descriptive and command feedback is intuitive.

- **Success Criterion:**

  - [ ] Netsuke is a feature-complete, secure, and user-friendly build tool that
    meets all initial design goals, with robust error handling and a polished
    command-line interface.
