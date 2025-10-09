# Netsuke: A Mid-Level Design for a Modern Build System in Rust

## Section 1: Core Architecture and Data Flow

This document presents a mid-level engineering design for Netsuke, a modern
build automation tool implemented in Rust. Netsuke is designed to provide the
power and dependency resolution capabilities of traditional `make` while
offering a significantly more intuitive, readable, and secure user experience.
This is achieved by leveraging a user-friendly YAML-based manifest, a powerful
Jinja templating engine for dynamic configuration, and the high-performance
Ninja build system as its execution backend.

### 1.1 Introduction: Netsuke as a Build System Compiler

At its core, Netsuke should not be conceptualized as a direct, imperative
replacement for `make`. Instead, it is architected as a high-level **build
system compiler**. This architectural paradigm is central to its design. Ninja,
the chosen execution backend, describes itself as a low-level "assembler" for
build systems.[^1] It is intentionally constrained, lacking features like
string manipulation or conditional logic, to ensure its primary goal: running
builds as fast as possible.[^2]

This design choice by Ninja's authors necessitates the existence of a higher-

level generator tool. Netsuke fulfills this role. It provides a rich,
user-friendly language (YAML with Jinja) for describing the *what* and *why* of
a build--the project's structure, its logical rules, and its configurable
parameters. Netsuke's primary responsibility is to compile this high-level
description into a low-level, highly optimized execution plan that Ninja can
understand and execute. This separation of concerns--Netsuke managing build
logic and Ninja managing execution--is the foundational principle of the entire
architecture.

### 1.2 The Six Stages of a Netsuke Build

The process of transforming a user's `Netsukefile` manifest into a completed
build artefact now follows a six-stage pipeline. This data flow validates the
manifest as YAML first, then resolves all dynamic logic into a static plan
before execution, a critical requirement for compatibility with Ninja.

1. Stage 1: Manifest Ingestion

   The process begins by locating and reading the user's project manifest file
   (e.g., Netsukefile) from the filesystem into memory as a raw string.

2. Stage 2: Initial YAML Parsing

   The raw string is parsed into an untyped `serde_yml::Value`. This step
   ensures the manifest is valid YAML before any templating takes place.

3. Stage 3: Template Expansion

   Netsuke walks the YAML `Value`, evaluating Jinja macros, variables, and the
   `foreach` and `when` keys. Each mapping containing these keys is expanded
   with an iteration context providing `item` and optional `index`. Variable
   lookups respect the precedence `globals` < `target.vars` < per-iteration
   locals, and this context is preserved for later rendering. At this stage
   Jinja must not modify the YAML structure directly; control constructs live
   only within these explicit keys. Structural Jinja blocks (`{% ... %}`) are
   not permitted to reshape mappings or sequences.

4. Stage 4: Deserialisation & Final Rendering

   The expanded `Value` is deserialised into strongly typed Rust structs. Jinja
   expressions are then rendered, but only within string fields. Structural
   templating using `{% %}` blocks is forbidden; all control flow must appear
   in YAML values.

5. Stage 5: IR Generation & Validation

   The AST is traversed to construct a canonical, fully resolved Intermediate
   Representation (IR) of the build. This IR represents the build as a static
   dependency graph with all file paths, commands, and dependencies explicitly
   defined. During this transformation, Netsuke performs critical validation
   checks. It verifies the existence of referenced rules, ensures each rule has
   exactly one of `command` or `script`, and ensures every target specifies
   exactly one of `rule`, `command`, or `script`. Circular dependencies and
   missing inputs are also detected at this stage.

6. Stage 6: Ninja Synthesis & Execution

   The final, validated IR is traversed by a code generator. This generator
   synthesizes the content of a `build.ninja` file, translating the IR's nodes
   and edges into corresponding Ninja rule and build statements. Once the file
   is written, Netsuke invokes the `ninja` executable as a subprocess, passing
   control to it for the final dependency checking and command-execution phase.

   Netsuke's pipeline is **deterministic**. Given the same `Netsukefile` and
   environment variables, the generated `build.ninja` will be byte-for-byte
   identical. This property is essential for reproducible builds and makes the
   output suitable for caching or source control.

```mermaid
flowchart TD
    A[Stage 1:\nManifest Ingestion] --> B[Stage 2:\nInitial YAML Parsing]
    B --> C[Stage 3:\nTemplate Expansion]
    C --> D[Stage 4:\nDeserialisation & Final Rendering]
    D --> E[Stage 5:\nIR Generation & Validation]
    E --> F[Stage 6:\nNinja Synthesis & Execution]
```

### 1.3 The Static Graph Mandate

The architecture's multi-stage pipeline is a direct consequence of a
fundamental design constraint imposed by the choice of Ninja as the backend.
Ninja's remarkable speed in incremental builds stems from its simplicity; it
operates on a pre-computed, static dependency graph and avoids costly runtime
operations like filesystem queries (e.g., glob expansion) or string
manipulation.[^2]

At the same time, a "friendlier" build system must offer dynamic capabilities.
Users will expect to define builds that can adapt to their environment, such as
using different compiler flags on Linux versus Windows, or automatically
discovering source files in a directory. These features are provided in Netsuke
by the Jinja templating engine.

This creates a necessary architectural division. All the dynamic logic,
templating, and configuration must be fully evaluated by Netsuke *before* Ninja
is ever invoked. The point of this transition is the Intermediate
Representation (IR) generated in Stage 4. The IR serves as a static snapshot of
the build plan after all Jinja logic has been resolved. It is the "object code"
that the Netsuke "compiler" produces, which can then be handed off to the Ninja
"assembler" for execution. This mandate for a pre-computed static graph
dictates the entire six-stage pipeline and establishes a clean boundary between
the user-facing logic layer and the machine-facing execution layer.

## Section 2: The Netsuke Manifest: A User-Centric YAML Schema

The primary interface for the user is the Netsuke manifest file, `Netsukefile`.
The design of its YAML schema is paramount to achieving the goal of being
"friendlier" than `make`. The schema is guided by a set of core principles
aimed at maximizing readability, reducing cognitive overhead, and promoting
best practices.

### 2.1 Schema Design Principles

- **Readability:** The schema prioritizes human-readability. It uses clear,
  descriptive keys and a structured format to make build configurations self-
  documenting. This stands in contrast to the often-cryptic special variables
  and implicit rules of Makefiles.

- **Declarative Style:** Users should declare the desired state of their
  project--the targets they want to build and the rules to build them--rather
  than writing imperative scripts. Netsuke is responsible for determining the
  necessary steps to achieve that state.

- **Reusability:** The schema is designed to encourage the creation of reusable
  components. Variables and rules are defined once and can be referenced
  throughout the manifest, reducing duplication and improving maintainability.

- **Discoverability:** The structure is intended to be intuitive. A developer
  familiar with YAML should be able to understand the intent of a simple
  `Netsukefile` file with minimal reference to documentation.

### 2.2 Top-Level Schema Structure

A `Netsukefile` file is a YAML mapping containing a set of well-defined top-
level keys.

- `netsuke_version`: A mandatory string that specifies the version of the
  Netsuke schema the manifest conforms to (e.g., `"1.0.0"`). This allows for
  future evolution of the schema while maintaining backward compatibility. This
  version string should be parsed and validated using the `semver` crate.[^4]

- `vars`: A mapping of global key-value pairs. Keys must be strings. Values may
  be strings, numbers, booleans, or sequences. These variables seed the Jinja
  templating context and drive control flow within the manifest.

- `macros`: An optional list of Jinja macro definitions. Each item provides a
  `signature` string using standard Jinja syntax and a `body` declared with the
  YAML `|` block style. Netsuke registers these macros in the template
  environment before rendering other sections.

- `rules`: A list of rule definitions. Each rule is a reusable template for a
  command, analogous to a Ninja `rule` block.[^2]

- `targets`: The primary list of build targets. Each target defines an output,
  the sources it depends on, and the rule used to produce it. This corresponds
  to a Ninja `build` statement.[^3]

- `actions`: A secondary list of build targets. Any target placed here is
  treated as `{ phony: true, always: false }` by default.

- `defaults`: An optional list of target names to be built when Netsuke is
  invoked without any specific targets on the command line. This maps directly
  to Ninja's `default` target statement.[^3]

The E-R diagram below summarizes the structure of a `Netsukefile` and the
relationships between its components.

```mermaid
erDiagram
    NETSUKE_MANIFEST {
        string netsuke_version
        map vars
        list rules
        list actions
        list targets
        list defaults
    }
    RULE {
        string name
        Recipe recipe
        string description
        StringOrList deps
    }
    TARGET {
        StringOrList name
        Recipe recipe
        StringOrList sources
        StringOrList deps
        StringOrList order_only_deps
        map vars
        bool phony
        bool always
    }
    RECIPE {
        string command
        string script
        StringOrList rule
    }
    STRING_OR_LIST {
        enum value
    }
    NETSUKE_MANIFEST ||--o{ RULE : contains
    NETSUKE_MANIFEST ||--o{ TARGET : has_actions
    NETSUKE_MANIFEST ||--o{ TARGET : has_targets
    RULE }o--|| RECIPE : uses
    TARGET }o--|| RECIPE : uses
    TARGET }o--|| STRING_OR_LIST : uses
    RECIPE }o--|| STRING_OR_LIST : uses
```

### 2.3 Defining `rules`

Each entry in the `rules` list is a mapping that defines a reusable action.

- `name`: A unique string identifier for the rule.

- `command`: A single command string to be executed. It may include the
  placeholders `{{ ins }}` and `{{ outs }}` to represent input and output
  files. Netsuke expands these placeholders to space-separated lists of file
  paths quoted for POSIX `/bin/sh` using the
  [`shell-quote`](https://docs.rs/shell-quote/latest/shell_quote/) crate (Sh
  mode) before hashing the action. The IR stores the fully expanded command;
  Ninja executes this text verbatim. After interpolation, the command must be
  parsable by [shlex](https://docs.rs/shlex/latest/shlex/) (POSIX mode). Any
  interpolation other than `ins` or `outs` is automatically shell-escaped.

- `script`: A multi-line script declared with the YAML `|` block style. The
  entire block is passed to an interpreter. If the first line begins with `#!`
  Netsuke executes the script verbatim, respecting the shebang. Otherwise, the
  block is wrapped in the interpreter specified by the optional `interpreter`
  field (defaulting to `/bin/sh -e`). For `/bin/sh` scripts, each interpolation
  is automatically passed through the `shell_escape` filter unless a `| raw`
  filter is applied. Future versions will allow configurable script languages
  with their own escaping rules. On Windows, scripts default to
  `powershell -Command` unless the manifest's `interpreter` field overrides the
  setting. Exactly one of `command`, `script`, or `rule` must be provided. The
  manifest parser enforces this rule to prevent invalid states.

  Internally, these options deserialise into a shared `Recipe` enum. Presence
  of exactly one of `command`, `script`, or `rule` determines the variant.

- `description`: An optional, user-friendly string that is printed to the
  console when the rule is executed. This maps to Ninja's `description` field
  and improves the user's visibility into the build process.[^2]

- `deps`: An optional field to configure support for C/C++-style header
  dependency generation. Its value specifies the format (e.g., `gcc` or
  `msvc`), which instructs Netsuke to generate the appropriate `depfile` or
  `deps` attribute in the corresponding Ninja rule.[^3]

### 2.4 Defining `targets`

Each entry in `targets` defines a build edge; placing a target in the optional
`actions` list instead marks it as `phony: true` with `always` left `false`.

- `name`: The primary output file or files for this build step. This can be a
  single string or a list of strings.

- `rule`: The name of the rule (from the `rules` section) to use for building
  this target. A YAML list may be supplied to run multiple rules sequentially.

```yaml
rule:
  - build
  - clean-up
```

- `command`: A single command string to run directly for this target.

- `script`: A multi-line script passed to the interpreter. When present, it is
  defined using the YAML `|` block style.

Only one of `rule`, `command`, or `script` may be specified. The parser
validates this exclusivity during deserialisation. When multiple fields are
present, Netsuke emits a `RecipeConflict` error with the message "rule, command
and script are mutually exclusive".

This union deserialises into the same `Recipe` enum used for rules. The parser
enforces that only one variant is present and errors if multiple recipe fields
are specified.

- `sources`: The input files required by the command. This can be a single
  string or a list of strings. If any source entry matches the `name` of
  another target, that target is built first, before the current target's
  explicit `deps`.

- `deps`: An optional list of other target names. These targets are explicit
  dependencies and must be successfully built before this target can be. A
  change in any of these dependencies will trigger a rebuild of the current
  target.

- `order_only_deps`: An optional list of other target names that must be built
  before this target, but whose modification does not trigger a rebuild of this
  target. This maps directly to Ninja's order-only dependencies, specified with
  the `||` operator.[^7]

- `vars`: An optional mapping of local variables. These variables override any
  global variables defined in the top-level `vars` section for the scope of
  this target only. This provides the same functionality as Ninja's build-local
  variables.[^3]

- `macros`: An optional list of Jinja macro definitions. Each item provides a
  `signature` string using standard Jinja syntax and a `body` declared with the
  YAML `|` block style. Netsuke registers these macros in the template
  environment before rendering other sections.

- `phony`: When set to `true`, the target runs when explicitly requested even if
  a file with the same name exists. The default value is `false`.

- `always`: When set to `true`, the target runs on every invocation regardless
  of timestamps or dependencies. The default value is `false`.

### 2.5 Generated Targets with `foreach`

Large sets of similar outputs can clutter a manifest when written individually.
Netsuke supports a `foreach` entry within `targets` to generate multiple
outputs succinctly. The `foreach` and optional `when` keys accept bare Jinja
expressions evaluated after the initial YAML pass. Each resulting value becomes
`item` in the target context, and the per-iteration environment is carried
forward to later rendering.

```yaml
- foreach: glob('assets/svg/*.svg')
  when: item | basename != 'logo.svg'
  name: "{{ outdir }}/{{ item | basename | replace('.svg', '.png') }}"
  rule: rasterise
  sources: "{{ item }}"
```

The expansion flow is:

```mermaid
flowchart TD
    A[Iterate over targets in YAML] --> B{Has foreach?}
    B -- Yes --> C[Evaluate foreach expression]
    C --> D[For each item:]
    D --> E{Has when?}
    E -- Yes --> F[Evaluate when expression]
    F -- True --> G[Expand target with item/index]
    F -- False --> H[Skip target]
    E -- No --> G
    B -- No --> I[Keep target as is]
```

Each element in the sequence produces a separate target. The iteration context:

- `item`: current element
- `index`: 0-based index (optional)
- Variables resolve with precedence `globals` < `target.vars` < iteration locals

Jinja control structures cannot shape the YAML; all templating must occur
within the string values. The resulting build graph is still fully static and
behaves the same as if every target were declared explicitly.

### 2.6 Table: Netsuke Manifest vs. Makefile

To illustrate the ergonomic advantages of the Netsuke schema, the following
table compares a simple C compilation project defined in both a traditional
`Makefile` and a `Netsukefile` file. The comparison highlights Netsuke's
explicit, structured, and self-documenting nature.

| Feature         | Makefile Example                                                                   | Netsukefile Example                                                                                               |
| --------------- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Variables       | CC=gcc                                                                             | { vars: { cc: gcc } }                                                                                             |
| Macros          | define greet\\t@echo Hello $$1endef                                                | { macros: { signature: "greet(name)", body: "Hello {{ name }}" } }                                                |
| Rule Definition | %.o: %.c\\n\\t$(CC) -c $< -o $@                                                    | { rules: { name: compile, command: "{{ cc }} -c {{ ins }} -o {{ outs }}", description: "Compiling {{ outs }}" } } |
| Target Build    | my_program: main.o utils.o\\t$(CC) $^ -o $@                                        | { targets: { name: my_program, rule: link, sources: [main.o, utils.o] }                                           |
| Readability     | Relies on cryptic automatic variables ($@, $\<, $^) and implicit pattern matching. | Uses explicit, descriptive keys (name, rule, sources) and standard YAML list/map syntax.                          |

## Section 3: Parsing and Deserialisation Strategy

Once the Jinja evaluation stage has produced a pure YAML string, the next
critical step is to parse this string and deserialise it into a structured, in-
memory representation. The choice of libraries and the definition of the target
data structures are crucial for the robustness and maintainability of Netsuke.

### 3.1 Crate Selection: `serde_yml`

For YAML parsing and deserialisation, the recommended crate is `serde_yml`.
This choice is based on its deep and direct integration with the `serde`
framework, the de-facto standard for serialisation and deserialisation in the
Rust ecosystem. Using `serde_yml` allows `serde`'s powerful derive macros to
automatically generate the deserialisation logic for Rust structs. This
approach is idiomatic, highly efficient, and significantly reduces boilerplate.
Add `#[derive(Deserialize)]` (optionally also `Debug`) to make a struct a
deserialisation target.

While other promising YAML libraries like `saphyr` exist, their `serde`
integration (`saphyr-serde`) is currently described as "soon-to-be" or is at a
highly experimental stage (version 0.0.0)[^11]. Building a core component of
Netsuke on a nascent or unreleased library would introduce significant and
unnecessary project risk.

`serde_yml` is mature, widely adopted, and battle-tested, making it the prudent
choice for production-quality software.

### 3.2 Core Data Structures (`ast.rs`)

The Rust structs that `serde_yml` will deserialise into form the Abstract
Syntax Tree (AST) of the build manifest. These structs must precisely mirror
the YAML schema defined in Section 2. They will be defined in a dedicated
module, `src/ast.rs`, and annotated with `#[derive(Deserialize)]` (and `Debug`)
to enable automatic deserialisation and easy debugging.

Rust

```rust
// In src/ast.rs

use serde::Deserialize;
use std::collections::HashMap;

/// Represents the top-level structure of a Netsukefile file.
#[serde(deny_unknown_fields)]
pub struct NetsukeManifest {
    pub netsuke_version: Version,

    #[serde(default)]
    pub vars: HashMap<String, serde_yml::Value>,

    #[serde(default)]
    pub rules: Vec<Rule>,

    #[serde(default)]
    pub actions: Vec<Target>,

    pub targets: Vec<Target>,

    #[serde(default)]
    pub defaults: Vec<String>,
}

/// Represents a reusable command template.
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    #[serde(flatten)]
    pub recipe: Recipe,
    pub description: Option<String>,
    #[serde(default)]
    pub deps: StringOrList,
    // Additional fields like 'pool' or 'restat' can be added here
    // to map to more advanced Ninja features.
}

/// A union of execution styles for both rules and targets.
#[serde(untagged)]
pub enum Recipe {
    Command { command: String },
    Script { script: String },
    Rule { rule: StringOrList },
}

/// Represents a single build target or edge in the dependency graph.
#[serde(deny_unknown_fields)]
pub struct Target {
    pub name: StringOrList,
    #[serde(flatten)]
    pub recipe: Recipe,

    #[serde(default)]
    pub sources: StringOrList,

    #[serde(default)]
    pub deps: StringOrList,

    #[serde(default)]
    pub order_only_deps: StringOrList,

    #[serde(default)]
    pub vars: HashMap<String, serde_yml::Value>,

    /// Run this target when requested even if a file with the same name exists.
    #[serde(default)]
    pub phony: bool,

    /// Run this target on every invocation regardless of timestamps.
    #[serde(default)]
    pub always: bool,
}

/// An enum to handle fields that can be either a single string or a list of strings.
#[serde(untagged)]
pub enum StringOrList {
    #[default]
    Empty,
    String(String),
    List(Vec<String>),
}
```

*Note: The* `StringOrList` *enum with* `#[serde(untagged)]` *provides the
flexibility for users to specify single sources, dependencies, and rule names
as a simple string and multiple as a list, enhancing user-friendliness.*

#### Example Manifest and AST

The following minimal Netsukefile shows how the derived structures behave when
unknown fields are denied.

YAML

```yaml
netsuke_version: "1.0.0"
targets:
  - name: hello
    command: echo hi
```

Rust

```rust
use std::collections::HashMap;
use netsuke::ast::*;

let ast = NetsukeManifest {
    netsuke_version: Version::parse("1.0.0").unwrap(),
    vars: HashMap::new(),
    rules: vec![],
    actions: vec![],
    targets: vec![Target {
        name: StringOrList::String("hello".into()),
        recipe: Recipe::Command {
            command: "echo hi".into(),
        },
        sources: StringOrList::Empty,
        deps: StringOrList::Empty,
        order_only_deps: StringOrList::Empty,
        vars: HashMap::new(),
        phony: false,
        always: false,
    }],
    defaults: vec![],
};
```

### 3.3 YAML-First Multi-Stage Ingestion

The integration of a templating engine like Jinja fundamentally shapes the
parsing pipeline, mandating a two-pass approach. It is impossible to parse the
user's `Netsukefile` file with `serde_yml` in a single step.

Consider a manifest containing Jinja syntax:

YAML

```yaml
targets:
  - name: my_app
    sources: "{{ glob('src/*.c') }}"
    rule: compile
```

The value of `sources`, `{{ glob('src/*.c') }}`, is a plain YAML string. The
manifest must be valid YAML before any templating occurs, so the parser can
first load it into a `serde_yml::Value` tree.

Once parsed, Netsuke performs a series of transformation stages:

1. **Template Expansion:** The `foreach` and optional `when` keys in the raw
   YAML are evaluated to generate additional targets. Each iteration layers the
   `item` and `index` variables over the manifest's globals and any target
   locals.
2. **Deserialisation:** The expanded document is deserialised into the typed
   [`NetsukeManifest`] AST.
3. **Final Rendering:** Remaining string fields are rendered using Jinja,
   resolving expressions such as `{{ glob('src/*.c') }}`.

This data-first approach avoids a lossy text-rendering pre-pass and keeps YAML
parsing and template evaluation cleanly separated.

### 3.4 Design Decisions

The AST structures are implemented in `src/ast.rs` and derive `Deserialize`.
Unknown fields are rejected to surface user errors early. `StringOrList`
provides a default `Empty` variant, so optional lists are trivial to represent.
The manifest version is parsed using the `semver` crate to validate that it
follows semantic versioning rules. Global and target variable maps now share
the `HashMap<String, serde_yml::Value>` type so booleans and sequences are
preserved for Jinja control flow. Targets also accept optional `phony` and
`always` booleans. They default to `false`, making it explicit when an action
should run regardless of file timestamps. Targets listed in the `actions`
section are deserialised using a custom helper so they are always treated as
`phony` tasks. This ensures preparation actions never generate build artefacts.
Convenience functions in `src/manifest.rs` load a manifest from a string or a
file path, returning `anyhow::Result` for straightforward error handling.

The ingestion pipeline now parses the manifest as YAML before any Jinja
evaluation. A dedicated expansion pass handles `foreach` and `when`, and string
fields are rendered only after deserialisation, keeping data and templating
concerns clearly separated.

### 3.5 Testing

Unit tests in `tests/ast_tests.rs` and behavioural scenarios in
`tests/features/manifest.feature` exercise the deserialisation logic. They
assert that manifests fail to parse when unknown fields are present, and that a
minimal manifest round-trips correctly. A collection of sample manifests under
`tests/data` cover both valid and invalid permutations of the schema. These
fixtures are loaded by the tests to ensure real-world YAML files behave as
expected. This suite guards against regressions as the schema evolves.

## Section 4: Dynamic Builds with the Jinja Templating Engine

To provide the dynamic capabilities and logical expressiveness that make a
build system powerful and "friendly," Netsuke will integrate a Jinja templating
engine. This engine acts as the user's primary tool for scripting and
configuration within the YAML manifest.

### 4.1 Crate Selection: `minijinja`

The recommended templating engine is `minijinja`.

This crate is the ideal choice for several reasons. It is explicitly designed
as a Rust implementation of the Jinja2 template engine, aiming for close
compatibility with its syntax and behaviour.[^15] This is advantageous as
Jinja2 is a mature, well-documented, and widely understood language, reducing
the learning curve for new Netsuke users. Furthermore,

`minijinja` is designed with minimal dependencies, which is beneficial for
keeping Netsuke's compile times and binary size reasonable.[^17] Its API is
well-documented and provides first-class support for adding custom functions
and filters, which is essential for extending its capabilities to suit the
needs of a build system.[^16]

Alternative template engines like Askama are less suitable for this use case.
Askama is a type-safe engine that compiles templates into Rust code at build
time.[^18] This model is incompatible with Netsuke's requirement to load and
parse user-defined manifest files at runtime.

`minijinja`, with its dynamic environment and runtime rendering, is perfectly
aligned with Netsuke's architecture.

### 4.2 The Jinja Environment and Context

Netsuke will construct a single `minijinja::Environment` instance at startup.
This environment will be configured with a set of custom functions and filters
that provide build-specific functionality.

When rendering a user's `Netsukefile` file, the initial context provided to the
template will be constructed from the `vars` section of the manifest. This
allows users to define variables in their YAML and immediately reference them
within Jinja expressions. For example:

YAML

```yaml
vars:
  compiler: gcc
  src_dir: src

targets:
  - name: "{{ compiler }}_output"
    sources: "{{ glob(src_dir ~ '/*.c') }}"
    #...
```

The `vars` mapping is read directly from the raw YAML before any Jinja is
evaluated. This avoids a lenient rendering pass for undefined placeholders and
keeps evaluation deterministic. The values are injected into the environment
prior to rendering.

The parser copies `vars` values into the environment using
`Value::from_serializable`. This preserves native YAML types so Jinja's
`{% if %}` and `{% for %}` constructs can branch on booleans or iterate over
sequences. Keys must be strings; any non-string key causes manifest parsing to
fail. Attempting to iterate over a non-sequence results in a render error
surfaced during manifest loading.

### 4.3 User-Defined Macros

Netsuke allows users to declare reusable Jinja macros directly in the manifest.
These are provided in a top-level `macros` list where each entry defines a
`signature` and a `body` string. The body must use YAML's `|` block syntax so
multi-line macro definitions remain readable. All macros are registered with
the template environment before any other section is rendered.

YAML

```yaml
macros:
  - signature: "greet(name)"
    body: |
      Hello {{ name }}
```

Macros can be invoked in any templated field using normal Jinja call syntax.
For example:

```yaml
rules:
  - name: hello
    command: "echo {{ greet('world') }}"
```

If a macro name matches a built-in function or filter, the macro overrides the
built-in definition. This mirrors Jinja's behaviour and follows `minijinja`
semantics where later definitions shadow earlier ones.

### 4.4 Essential Custom Functions

To transform `minijinja` from a general-purpose templating engine into a
powerful build tool, Netsuke must expose a curated set of custom functions to
the template environment. These functions will be implemented in safe Rust,
providing a secure bridge to the underlying system.

- `env(var_name: &str) -> Result<String, Error>`: A function that reads an
  environment variable from the system. This allows build configurations to be
  influenced by the external environment (e.g., `PATH`, `CC`). It returns an
  error if the variable is undefined or contains invalid UTF-8 to ensure
  manifests fail fast on missing inputs.

- `glob(pattern: &str) -> Result<Vec<String>, Error>`: Expand filesystem
  patterns (e.g., `src/**/*.c`) into a list of matched paths. Results are
  yielded in lexicographic order by the iterator and returned unchanged.
  Symlinks are followed by the `glob` crate by default. Matching is case-
  sensitive on all platforms. `glob_with` enforces
  `require_literal_separator = true` internally, so wildcards do not cross path
  separators unless `**` is used. Callers may use `/` or `\` in patterns; these
  are normalized to the host platform before matching. Results contain only
  files (directories are ignored) and path separators are normalized to `/`.
  Leading-dot entries are matched by wildcards. Empty results are represented
  as `[]`. Invalid patterns surface as `SyntaxError`; filesystem iteration
  errors surface as `InvalidOperation`, matching minijinja error semantics. On
  Unix, backslash escapes for glob metacharacters (`[`, `]`, `{`, `}`, `*`,
  `?`) are preserved during separator normalization. A backslash before `*` or
  `?` is kept only when the wildcard is trailing or followed by an
  alphanumeric, `_`, or `-`; otherwise it becomes a path separator so
  `config\*.yml` maps to `config/*.yml`. On Windows, backslash escapes are not
  supported. This provides globbing support not available in Ninja itself,
  which does not support globbing.[^3]
- `python_version(requirement: &str) -> Result<bool, Error>`: An example of a
  domain-specific helper function that demonstrates the extensibility of this
  architecture. This function would execute `python --version` or
  `python3 --version` using `std::process::Command`,[^24] parse the output
  using the `semver` crate,[^4] and compare it against a user-provided SemVer
  requirement string (e.g., `">=3.8"`). This allows for conditional logic in
  the build based on toolchain versions.

### 4.5 Essential Custom Filters

In addition to functions, custom filters provide a concise, pipe-based syntax
for transforming data within templates.

- `| shell_escape`: A filter that takes a string or list and escapes it for
  safe inclusion as a single argument in a shell command. This is a
  non-negotiable security feature to prevent command injection vulnerabilities.
  The implementation will use the `shell-quote` crate for robust, shell-aware
  quoting.[^22]

- `| to_path`: A filter that converts a string into a platform-native path
  representation, handling `/` and `\` separators correctly.

- `| parent`: A filter that takes a path string and returns its parent
  directory.

### 4.6 Jinja as the "Logic Layer"

The integration of Jinja is more than a simple convenience for string
substitution. It effectively serves as the **logic layer** for the entire build
system. Traditional `make` provides powerful but often opaque functions like
`$(shell...)` and `$(wildcard...)`. Netsuke achieves and surpasses this
functionality in a much friendlier and safer way.

By implementing complex or potentially unsafe operations (like filesystem
access or command execution) as custom functions in Rust and exposing them as
simple, declarative primitives in the Jinja environment, Netsuke provides a
powerful yet controlled scripting environment. The user can write a clean,
readable template like `sources: {{ glob("src/*.c") }}`, and the complex,
error-prone logic of traversing the filesystem is handled by secure,
well-tested Rust code. This design pattern is the key to providing both power
and safety, fulfilling the core requirement of a system that is friendlier and
more robust than its predecessors.

### 4.7 Template Standard Library

Netsuke bundles a small "standard library" of Jinja helpers. These tests,
filters, and functions are available to every template and give concise access
to common filesystem queries, path manipulations, collection utilities, and
network operations.

#### File-system tests

| Test                                                  | True when the operand…                                           |
| ----------------------------------------------------- | ---------------------------------------------------------------- |
| `dir` / `file` / `symlink`                            | …is that object type                                             |
| `pipe` / `block_device` / `char_device` *(Unix-only)* | …is that object type                                             |
| `device` (legacy, Unix-only)                          | …is a block or character device                                  |
| `present`                                             | …exists (any type)                                               |
| `owned`                                               | …is owned by the current UID                                     |
| `readable` / `writable` / `executable`                | …has the corresponding permission bit for current user           |
| `empty`                                               | …has size 0 bytes                                                |
| `older_than(value)`                                   | …has `mtime` < given value (seconds, `timedelta`, or file)       |
| `newer_than(value)`                                   | …has `mtime` > given value                                       |
| `contains(substr)`                                    | …file’s text contains **substr**                                 |
| `matches(regex)`                                      | …file’s text matches **regex**                                   |
| `type(kind)`                                          | …is of the file-type string supplied (`"file"`, `"dir"`, etc.)   |

The `dir`, `file`, and `symlink` tests use `cap_std`'s UTF-8-capable
[`Dir::symlink_metadata`][cap-symlink] with `camino` paths to inspect the
operand's [`FileType`][filetype]. Because this lookup does not follow links,
`symlink` tests never report a file or directory for the same path. On Unix the
`pipe`, `block_device`, `char_device`, and legacy `device` tests also probe the
metadata. On non-Unix targets these predicates are stubbed to always return
`false` so templates remain portable. Missing paths evaluate to `false`, while
I/O errors raise a template error.

[cap-symlink]:
https://docs.rs/cap-std/latest/cap_std/fs_utf8/struct.Dir.html#method.symlink_metadata

[filetype]: https://doc.rust-lang.org/std/fs/struct.FileType.html

#### Path & file filters

| Filter                                     | Purpose                                                              |
| ------------------------------------------ | -------------------------------------------------------------------- |
| `basename`                                 | Return last path component                                           |
| `dirname`                                  | Return parent directory                                              |
| `with_suffix(suffix, n=1, sep='.')`        | Replace last `n` dotted suffix components (`foo.tar.gz → foo.zip`)   |
| `relative_to(root)`                        | Make path relative to **root**                                       |
| `realpath`                                 | Resolve symlinks to canonical path                                   |
| `commonpath(other)`                        | Longest common prefix with **other**                                 |
| `expanduser`                               | Expand leading `~`                                                   |
| `size`                                     | File size in bytes                                                   |
| `contents(encoding='utf-8')`               | File content as text                                                 |
| `linecount`                                | Number of text lines                                                 |
| `head(n=10)` / `tail(n=10)`                | First / last *n* lines                                               |
| `mtime` / `ctime`                          | Return timestamp (`datetime`)                                        |
| `age(unit='s')`                            | Seconds (or `m`, `h`, `d`) since `mtime`                             |
| `date(fmt='%Y-%m-%d')`                     | Format `mtime`/`ctime`                                               |
| `owner` / `group`                          | User / group name                                                    |
| `stat`                                     | Full `os.stat()` result as dict                                      |
| `hash(alg='sha256')`                       | Hex digest of file (`md5`, `sha1`, …)                                |
| `digest(n=8, alg='sha256')`                | Truncated digest (e.g. build ID)                                     |
| `base64` / `hex`                           | Encode bytes or string                                               |
| `slugify`                                  | Make filename-safe slug                                              |
| `snake_case` / `camel_case` / `kebab-case` | Rename helpers                                                       |

All built-in filters use `snake_case`. The `camel_case` helper is provided in
place of `camelCase` so naming remains consistent with `snake_case` and
`kebab-case`.

Implementation notes:

- Filters rely on `cap-std` directories opened with ambient authority for
  file-system work. Callers must ensure that templates granted access to the
  stdlib are trusted to read from the process' working tree.
- `realpath` canonicalises the parent directory before joining the resolved
  entry so results are absolute and symlink-free.
- `contents` and `linecount` currently support UTF-8 input; other encodings are
  rejected with an explicit error. `contents` streams data from the ambient
  file-system, so consumers should guard access carefully when evaluating
  untrusted templates.
- `hash` and `digest` accept `sha256` (default) and `sha512`. Legacy
  algorithms `sha1` and `md5` are cryptographically broken and are disabled by
  default; enabling them requires the `legacy-digests` Cargo feature and should
  only be done for compatibility with existing ecosystems.
- `expanduser` mirrors shell semantics by inspecting `HOME`, `USERPROFILE`,
  and on Windows the `HOMEDRIVE`/`HOMEPATH` or `HOMESHARE` fallbacks.
  Platform-specific forms such as `~user` remain unsupported.
- `with_suffix` removes dotted suffix segments (default `n = 1`) before
  appending the provided suffix.

#### Generic collection filters

| Filter                            | Purpose                                      |
| --------------------------------- | -------------------------------------------- |
| `uniq`                            | De-duplicate list (preserve order)           |
| `flatten`                         | Deep flatten of arbitrarily nested lists     |
| `group_by(attr)`                  | Dict keyed on `attr` of list items           |
| `zip(other)`                      | Pairwise tuples of two lists                 |
| `version_compare(other, op='>=')` | SemVer comparison (`'<'`, `'<=', '==', …`)   |

Implementation notes for collection filters:

- `uniq` stores values in an `IndexSet` so duplicates are removed with `O(n)`
  complexity while preserving the original order according to MiniJinja's
  equality semantics.
- `flatten` recurses through nested sequences and iterables; scalars raise an
  `InvalidOperation` error to avoid silently iterating over strings or other
  unintended inputs.
- `group_by` returns an insertion-order-preserving mapping keyed by the
  original value so lookups via attribute names and bracket syntax remain in
  sync. Empty attribute names and items without the attribute surface an
  `InvalidOperation` error so templates fail loudly rather than mis-grouping
  data.

#### Network & command functions / filters

| Name                                                  | Kind         | Purpose                                                          |
| ----------------------------------------------------- | ------------ | ---------------------------------------------------------------- |
| `fetch(url, cache=False, cache_dir='.netsuke/fetch')` | **function** | Retrieve URL, return content (str/bytes)                         |
| `http_head(url)`                                      | function     | Return headers dict                                              |
| `download(url, dest)`                                 | function     | Idempotent file download (returns **dest**)                      |
| `shell(cmd)`                                          | **filter**   | Pipe value to arbitrary shell command; marks template **impure** |
| `grep`, `sed`, `awk`, `cut`, `wc`, `tr`               | filters      | Canonical wrappers implemented via `shell()` for convenience     |

Using `shell()` marks the template as *impure* and disables caching of the
rendered YAML between Stage 2 and Stage 3. This avoids accidental reuse of
results that depend on external commands.

Implementation details:

- `fetch` issues HTTP requests through the `ureq` client. When caching is
  enabled a SHA-256 digest of the URL becomes the cache key and responses are
  written beneath `.netsuke/fetch` (or a user-provided directory) using
  capability-restricted file handles. Any remote fetch or cache write marks the
  stdlib state as impure so callers can discard memoised renders, and absolute
  cache directories open with ambient authority so Windows drive prefixes work
  correctly.
- `shell` and `grep` spawn the platform shell (`sh` or `cmd.exe`) with POSIX
  single-quoted arguments emitted via `shell-quote`. The stdlib registers a
  shared `StdlibState` that flips an `impure` flag whenever these helpers
  execute so callers can detect templates that interacted with the outside
  world.

Custom external commands can be registered as additional filters. Those should
be marked `pure` if safe for caching or `impure` otherwise.

#### Time helpers

| Name                  | Kind     | Purpose                                   |
| --------------------- | -------- | ----------------------------------------- |
| `now()`               | function | Current `datetime` (UTC by default)       |
| `timedelta(**kwargs)` | function | Convenience creator for `age` comparisons |

The `now()` helper produces an object that renders as an ISO&nbsp;8601
timestamp and exposes `iso8601`, `unix_timestamp`, and `offset` accessors so
templates can serialize or compare values without string parsing. It defaults
to UTC but accepts an `offset="+HH:MM"` keyword argument that re-bases the
captured time on another fixed offset. Time is captured lazily when the helper
executes so behaviour remains deterministic during a render.

`timedelta(**kwargs)` constructs a duration object that renders using the
ISO&nbsp;8601 duration grammar (for example, `P1DT2H30M5.75025S`). The helper
accepts integer keyword arguments `weeks`, `days`, `hours`, `minutes`,
`seconds`, `milliseconds`, `microseconds`, and `nanoseconds`, allowing callers
to describe durations at nanosecond precision. Arguments may be negative, but
overflow or non-integer inputs raise `InvalidOperation` errors so templates
cannot silently wrap. The resulting object exposes `.iso8601`, `.seconds`, and
`.nanoseconds` attributes for downstream predicates.

##### Example usage

```jinja
{% if "config.yaml" is file and "config.yaml" is readable %}
  {{ "config.yaml" | contents | grep("version") }}
{% endif %}

{{ "src/app.c" | basename | with_suffix(".o") }}
{{ fetch('https://example.com/data.csv', cache=True) | head(5) }}
```

## Section 5: The Bridge to Ninja: Intermediate Representation and Code Generation

After the user's manifest has been deserialized into the AST and remaining
string fields have been rendered by Jinja, the next phase is to transform this
high-level representation into a format suitable for the Ninja backend. This is
accomplished via a two-step process: converting the AST into a canonical
Intermediate Representation (IR), and then synthesizing the final `build.ninja`
file from that IR.

### 5.1 The Role of the Intermediate Representation (IR)

The Intermediate Representation is a critical architectural component that
serves as the static, fully resolved, and validated representation of the
entire build graph. It is the bridge between the user-facing front-end (the
YAML schema and its corresponding AST) and the machine-facing back-end (the
Ninja file format).

The primary purpose of the IR is to create a decoupling layer. This abstraction
barrier allows the front-end and back-end to evolve independently. For example,
the YAML schema could be significantly redesigned in a future version of
Netsuke, but as long as the transformation logic is updated to produce the same
stable IR, the Ninja generation back-end would require no changes. Conversely,
if the decision were made to support an alternative execution back-end (e.g., a
distributed build system), only a new generator module (`IR -> NewBackend`)
would need to be written, leaving the entire front-end parsing and validation
logic untouched.

Importantly, the IR contains **no Ninja-isms**. Placeholders such as `$in` and
`$out` are resolved to plain lists of file paths, and command strings are
expanded before hashing. This deliberate absence of Ninja-specific syntax makes
the IR a stable contract that future back-ends--distributed builders, remote
executors, or otherwise--can consume without modification.

Furthermore, the IR is the ideal stage at which to perform graph-level analysis
and optimizations, such as detecting circular dependencies, pruning unused
build targets, or identifying duplicate build actions.

### 5.2 IR Data Structures (`ir.rs`)

The IR data structures are designed to closely mirror the conceptual model of
the Ninja build system, which consists of "Action" nodes (commands) and
"Target" nodes (files).[^7] This close mapping simplifies the final code
generation step.

Rust

```rust
// In src/ir.rs

use std::collections::HashMap;
use camino::Utf8PathBuf;

/// The complete, static build graph.
pub struct BuildGraph {
    /// A map of all unique actions (rules) in the build.
    /// The key is a hash of a canonical JSON serialisation of the action's
    /// properties to enable deduplication.
    pub actions: HashMap<String, Action>,

    /// A map of all target files to be built. The key is the output path.
    pub targets: HashMap<Utf8PathBuf, BuildEdge>,

    /// A list of targets to build by default.
    pub default_targets: Vec<Utf8PathBuf>,
}

/// Represents a reusable command, analogous to a Ninja 'rule'.
pub struct Action {
    pub recipe: Recipe,
    pub description: Option<String>,
    pub depfile: Option<String>, // Template for the.d file path, e.g., "$out.d"
    pub deps_format: Option<String>, // "gcc" or "msvc"
    pub pool: Option<String>,
    pub restat: bool,
}

/// Represents a single build statement, analogous to a Ninja 'build' edge.
/// It connects a set of inputs to a set of outputs via an Action. The `phony`
/// and `always` flags control execution when outputs already exist or when
/// timestamps would normally skip the step.
pub struct BuildEdge {
    /// The unique identifier of the Action used for this edge.
    pub action_id: String,

    /// Explicit inputs that, when changed, trigger a rebuild.
    pub inputs: Vec<Utf8PathBuf>,

    /// Outputs explicitly generated by the command.
    pub explicit_outputs: Vec<Utf8PathBuf>,

    /// Outputs implicitly generated by the command. Maps to Ninja's '|' syntax.
    pub implicit_outputs: Vec<Utf8PathBuf>,

    /// Dependencies that must be built first but do not trigger a rebuild on change.
    /// Maps to Ninja's '||' syntax.
    pub order_only_deps: Vec<Utf8PathBuf>,

    /// Run this edge when requested even if the output file already exists.
    pub phony: bool,

    /// Run this edge on every invocation regardless of timestamps.
    pub always: bool,
}
```

```mermaid
classDiagram
    class BuildGraph {
        +HashMap<String, Action> actions
        +HashMap<Utf8PathBuf, BuildEdge> targets
        +Vec<Utf8PathBuf> default_targets
    }
    class Action {
        +Recipe recipe
        +Option<String> description
        +Option<String> depfile
        +Option<String> deps_format
        +Option<String> pool
        +bool restat
    }
    class BuildEdge {
        +String action_id
        +Vec<Utf8PathBuf> inputs
        +Vec<Utf8PathBuf> explicit_outputs
        +Vec<Utf8PathBuf> implicit_outputs
        +Vec<Utf8PathBuf> order_only_deps
        +bool phony
        +bool always
    }
    class Recipe {
        <<enum>>
        Command
        Script
        Rule
    }
    class ninja_gen {
        +generate(graph: &BuildGraph) String
    }
    BuildGraph "1" o-- "many" Action : actions
    BuildGraph "1" o-- "many" BuildEdge : targets
    Action "1" o-- "1" Recipe
    BuildEdge "1" --> "1" Action : action_id
    ninja_gen ..> BuildGraph : uses
    ninja_gen ..> Action : uses
    ninja_gen ..> BuildEdge : uses
    ninja_gen ..> Recipe : uses
```

### 5.3 The Transformation Process: AST to IR

The core logic of the validation stage is a function, `ir::from_manifest`, that
consumes a `NetsukeManifest` (the AST) and produces a `BuildGraph` (the IR).
This transformation involves several steps:

1. **Rule Collection:** Insert each entry in `manifest.rules` into a
   `HashMap` keyed by its name. Rules are stored as templates and are not
   deduplicated at this stage.

2. **Target Expansion:** Iterate through the `manifest.targets` and the optional
   `manifest.actions`. Entries in `actions` are treated identically to targets
   but with `phony` defaulting to `true`. For each item, resolve all strings
   into `Utf8PathBuf`s and resolve all dependency names against other targets.

3. **Action Registration and Edge Creation:** For each expanded target,
   resolve the referenced rule template, interpolate its command with the
   target's input and output paths, and register the resulting `ir::Action` in
   the `actions` map. Actions are hashed on the fully resolved command and file
   set, so identical rule templates yield distinct actions when their paths
   differ. Create a corresponding `ir::BuildEdge` linking the target to the
   action identifier and transfer the `phony` and `always` flags.

4. **Graph Validation:** As the graph is constructed, perform validation checks.
   This includes ensuring that every rule referenced by a target exists in the
   `actions` map and running a cycle detection algorithm (e.g., a depth-first
   search maintaining a visitation state) on the dependency graph to fail early
   on circular dependencies.

   The implemented algorithm performs a depth-first traversal of the target
   graph and maintains a recursion stack. Order-only dependencies are ignored
   during this search. Self-edges are rejected immediately, and encountering an
   already visiting node indicates a cycle. The stack slice from the first
   occurrence of that node forms the cycle and is returned in
   `IrGenError::CircularDependency` for improved debugging. The cycle list is
   rotated so the lexicographically smallest node appears first, ensuring
   deterministic error messages.

   Traversal state is implemented in the dedicated `ir::cycle` module. Its
   `CycleDetector` helper owns the recursion stack and visitation map. Keys are
   cloned from the `targets` map so traversal leaves the input graph untouched.
   Missing dependencies encountered during traversal are logged, collected, and
   returned alongside any cycle to aid diagnostics.

### 5.4 Ninja File Synthesis (`ninja_gen.rs`)

The final step is to synthesize the `build.ninja` file from the `BuildGraph`
IR. This process is a straightforward, mechanical translation from the IR data
structures to the Ninja file syntax.

1. **Write Variables:** Any global variables that need to be passed to Ninja can
   be written at the top of the file (e.g., `msvc_deps_prefix` for Windows

2. **Write Rules:** Iterate through the `graph.actions` map. For each
   `ir::Action`, write a corresponding Ninja `rule` statement. The input and
   output lists stored in the action replace the `ins` and `outs` placeholders.
   These lists are then rewritten as Ninja's `$in` and `$out`.

   When an action's `recipe` is a script, the generated rule wraps the script
   in an invocation of `/bin/sh -e -c` so that multi-line scripts execute
   consistently across platforms.

   Code snippet

   ```ninja
   # Generated from an ir::Action
   rule cc
     command = gcc -c -o $out $in
     description = CC $out
     depfile = $out.d
     deps = gcc
   ```

3. **Write Build Edges:** Iterate through the `graph.targets` map. For each
   `ir::BuildEdge`, write a corresponding Ninja `build` statement. This
   involves formatting the lists of explicit outputs, implicit outputs, inputs,
   and order-only dependencies using the correct Ninja syntax (`:`, `|`, and
   `||`).[^7] Use Ninja's built-in `phony` rule when `phony` is `true`. For an
   `always` edge, either generate a `phony` build with no outputs or emit a
   dummy output marked `restat = 1` and depend on a permanently dirty target so
   the command runs on each invocation.

   Code snippet

   ```ninja
   # Generated from an ir::BuildEdge
   build foo.o: cc foo.c
   build bar.o: cc bar.c
   build my_app: link foo.o bar.o | lib_dependency.a
   ```

4\. **Write Defaults:** Finally, write the `default` statement, listing all
paths from `graph.default_targets`.

```ninja
default my_app
```

### 5.5 Design Decisions

The IR structures defined in `src/ir.rs` are minimal containers that mirror
Ninja's conceptual model while remaining backend-agnostic. `BuildGraph`
collects all `Action`s and `BuildEdge`s in hash maps keyed by stable strings
and `Utf8PathBuf`s so the graph can be deterministically traversed for snapshot
tests. Actions hold the parsed `Recipe` and optional execution metadata.
`BuildEdge` connects inputs to outputs using an action identifier and carries
the `phony` and `always` flags verbatim from the manifest. No Ninja specific
placeholders are stored in the IR to keep the representation portable.

- Actions are deduplicated using a SHA-256 hash of a canonical JSON
  serialisation of their recipe, inputs, and outputs. Because commands embed
  shell-quoted file paths, two targets share an identifier only when both the
  command text and file sets match exactly.
- Multiple rule references in a single target are not yet supported. The IR
  generator reports `IrGenError::MultipleRules` when encountered.
- Duplicate output files are rejected. Attempting to define the same output
  path twice results in `IrGenError::DuplicateOutput`.
- The Ninja generator sorts actions and edges before output and deduplicates
  edges based on their full set of explicit outputs. Sorting uses the joined
  path strings to keep ordering stable across platforms, ensuring deterministic
  `build.ninja` files. Small macros reduce formatting boilerplate when writing
  optional key-value pairs or flags, keeping the generator easy to scan.
- Integration tests snapshot the generated Ninja file with `insta` and
  execute the Ninja binary to validate structure and no-op behaviour.

## Section 6: Process Management and Secure Execution

The final stage of a Netsuke build involves executing commands. While Netsuke
delegates the core task scheduling and execution to the Ninja binary, it
remains responsible for invoking Ninja correctly and, most importantly, for
ensuring that the commands it generates for Ninja to run are constructed
securely.

### 6.1 Invoking Ninja

Netsuke will use Rust's standard library `std::process::Command` API to
configure and spawn the `ninja` process.[^24] This provides fine-grained
control over the child process's execution environment.

The command construction will follow this pattern:

1. A new `Command` is created via `Command::new("ninja")`. Netsuke will assume
   `ninja` is available in the system's `PATH`.

2. Arguments passed to Netsuke's own CLI will be translated and forwarded to
   Ninja. For example, a `Netsuke build my_target` command would result in
   `Command::new("ninja").arg("my_target")`. Flags like `-j` for parallelism
   will also be passed through.[^8]

3. The working directory for the Ninja process will be set using
   `.current_dir()`. When the user supplies a `-C` flag, Netsuke canonicalises
   the path and applies it via `current_dir` rather than forwarding the flag to
   Ninja.

4. Standard I/O streams (`stdin`, `stdout`, `stderr`) will be configured using
   `.stdout(Stdio::piped())` and `.stderr(Stdio::piped())`.[^24] This allows
   Netsuke to capture the real-time output from Ninja, which can then be
   streamed to the user's console, potentially with additional formatting or
   status updates from Netsuke itself.

In the initial implementation a small helper wraps `Command::new` to forward
the `-j` and `-C` flags and any explicit build targets. Standard output and
error are piped and written back to Netsuke's own streams so users see Ninja's
messages in order. A non-zero exit status or failure to spawn the process is
reported as an `io::Error` for the CLI to surface.

### 6.2 The Criticality of Shell Escaping

A primary security responsibility for Netsuke is the prevention of command
injection attacks. The `command` strings defined in a user's `Netsukefile` are
templates. When Netsuke substitutes variables like file paths into these
templates, it is imperative that these substituted values are treated as
single, literal arguments by the shell that Ninja ultimately uses to execute
the command.

Without proper escaping, a malicious or even accidental filename like
`"my file; rm -rf /;.c"` could be interpreted as multiple commands, leading to
catastrophic consequences.

For this critical task, the recommended crate is `shell-quote`.

While other crates like `shlex` exist, `shell-quote` offers a more robust and
flexible API specifically designed for this purpose.[^22] It supports quoting
for multiple shell flavours (e.g., Bash, sh, Fish), which is vital for a
cross-platform build tool. It also correctly handles a wide variety of input
types, including byte strings and OS-native strings, which is essential for
dealing with non-UTF8 file paths. The

`QuoteExt` trait provided by the crate offers an ergonomic and safe method for
building command strings by pushing quoted components into a buffer:
`script.push_quoted(Bash, "foo bar")`.

### 6.3 Implementation Strategy

The command generation logic within the `ninja_gen.rs` module must not use
simple string formatting (like `format!`) to construct the final command
strings. Instead, parse the Netsuke command template (e.g.,
`{{ cc }} -c {{ ins }} -o` `{{ outs }}`) and build the final command string
step by step. The placeholders `{{ ins }}` and `{{ outs }}` are expanded to
space-separated lists of file paths within Netsuke itself, each path being
shell-escaped using the `shell-quote` API. Netsuke uses the `Sh` quoting mode
to emit POSIX-compliant single-quoted strings and scans the template for
standalone `$in` and `$out` tokens to avoid rewriting unrelated variables.
Substitution happens during IR generation and the fully expanded command is
emitted to `build.ninja` unchanged. After substitution, the command is
validated with \[`shlex`\](<https://docs.rs/shlex/latest/shlex/>) to ensure it
parses correctly. This approach guarantees that every dynamic part of the
command is securely quoted, albeit at the cost of deduplicating only actions
with identical file sets.

### 6.4 Automatic Security as a "Friendliness" Feature

The concept of being "friendlier" than `make` extends beyond syntactic sugar to
encompass safety and reliability. A tool that is easy to use but exposes the
user to trivial security vulnerabilities is fundamentally unfriendly. In many
build systems, the burden of correct shell quoting falls on the user, an
error-prone task that requires specialised knowledge.

Netsuke's design elevates security to a core feature by making it automatic and
transparent. The user writes a simple, unquoted command template, and Netsuke
performs the complex and critical task of making it secure behind the scenes.
By integrating `shell-quote` directly into the Ninja file synthesis stage,
Netsuke protects users from a common and dangerous class of errors by default.
This approach embodies a deeper form of user-friendliness: one that anticipates
and mitigates risks on the user's behalf.

## Section 7: A Framework for Friendly and Actionable Error Reporting

A key differentiator for a "friendly" tool is how it communicates failure.
Cryptic, unhelpful error messages are a major source of frustration for
developers. Netsuke's error handling framework is designed to provide messages
that are clear, contextual, and actionable.

### 7.1 Error Handling Philosophy

Errors are not exceptional events; they are an expected part of the development
workflow. Every error reported by Netsuke to the user must strive to answer
three fundamental questions:

1. **What** went wrong? A concise summary of the failure (e.g., "YAML parsing
   failed," "Build configuration is invalid").

2. **Where** did it go wrong? Precise location information, including the file,
   line number, and column where applicable (e.g., "in `Netsukefile` at line
   15, column 3").

3. **Why** did it go wrong, and what can be done about it? The underlying cause
   of the error and a concrete suggestion for how to fix it (e.g., "Cause:
   Found a tab character, which is not allowed. Hint: Use spaces for
   indentation instead.").

### 7.2 Crate Selection and Strategy: `anyhow`, `thiserror`, and `miette`

Netsuke uses a two-tier error architecture:

1. `anyhow` captures internal context as errors propagate through the
   application.
2. `miette` renders user-facing diagnostics and **is not optional**. All
   surface errors must implement `miette::Diagnostic` so the CLI can present
   spans, annotated source, and helpful suggestions.

This hybrid strategy is common in the Rust ecosystem and provides both rich
context and polished user output.[^27]

- `thiserror`: This crate is used *within* Netsuke's internal library modules
  (e.g., `parser`, `ir`, `ninja_gen`) to define specific, structured error
  types. The `#[derive(Error)]` macro reduces boilerplate and allows for the
  creation of rich, semantic errors.[^29]

Rust

```rust
// In src/ir.rs use thiserror::Error; use camino::Utf8PathBuf;

#[derive(Debug, Error)]
pub enum IrGenError {
    #[error("rule '{rule_name}' referenced by target '{target_name}' was not found")]
    RuleNotFound { target_name: String, rule_name: String },

    #[error("multiple rules for target '{target_name}': {rules:?}")]
    MultipleRules { target_name: String, rules: Vec<String> },

    #[error("No rules specified for target {target_name}")]
    EmptyRule { target_name: String },

    #[error("duplicate target outputs: {outputs:?}")]
    DuplicateOutput { outputs: Vec<String> },

    #[error("circular dependency detected: {cycle:?}")]
    CircularDependency {
        cycle: Vec<Utf8PathBuf>,
        missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
    },

    #[error("failed to serialise action: {0}")]
    ActionSerialisation(#[from] serde_json::Error), }
```

- `anyhow`: Used in the main application logic (`main.rs`) and at the
  boundaries between modules. `anyhow::Result` wraps any error implementing
  `std::error::Error`.[^30] The `?` operator provides clean propagation, while
  `.context()` and `.with_context()` attach high-level explanations as errors
  bubble up.[^31]

- `miette`: Presents human-friendly diagnostics, highlighting exact error
  locations with computed spans. Every diagnostic must retain `miette`'s
  `Diagnostic` implementation as it travels through `anyhow`.

#### Canonical pattern: `YamlDiagnostic`

`YamlDiagnostic` is the reference implementation of a Netsuke diagnostic. It
wraps `yaml-rust` errors with annotated source, spans, and optional help text:

```rust
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(netsuke::yaml::parse))]
pub struct YamlDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("parse error here")]
    span: Option<SourceSpan>,
    #[help]
    help: Option<String>,
    #[source]
    source: YamlError,
    message: String,
}

#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    #[error("manifest parse error")]
    #[diagnostic(code(netsuke::manifest::parse))]
    Parse {
        #[source]
        #[diagnostic_source]
        source: Box<dyn Diagnostic + Send + Sync + 'static>,
    },
}
```

`ManifestError::Parse` boxes the diagnostic to preserve the rich error so
`miette` can show the offending YAML snippet. All new user-facing errors with
source context must follow this model.

Common use cases requiring `miette` diagnostics include:

- YAML parsing errors.
- Jinja template rendering failures with line numbers and context.
- Any scenario where highlighting spans or providing structured help benefits
  the user.

Although `src/diagnostics.rs` is currently unused, it contains prototypes for
`miette` patterns and remains a valuable reference. Future diagnostics should
mirror the `YamlDiagnostic` approach by implementing `Diagnostic`, providing a
`NamedSource`, a `SourceSpan`, and actionable help text.

### 7.3 Error Handling Flow

The flow of an error from its origin to the user follows a clear path of
enrichment:

1. A specific, low-level error occurs within a module. For instance, the IR
   generator detects a missing rule and creates an `IrGenError::RuleNotFound`.
   Likewise, the Ninja generator returns `NinjaGenError::MissingAction` when a
   build edge references an undefined action, preventing panics during file
   generation.

2. The function where the error occurred returns
   `Err(IrGenError::RuleNotFound {... }.into())`. The `.into()` call converts
   the specific `thiserror` enum variant into a generic `anyhow::Error` object,
   preserving the original error as its source.

3. A higher-level function in the call stack, which called the failing function,
   receives this `Err` value. It uses the `.with_context()` method to wrap the
   error with more application-level context. For example:
   `ir::from_manifest(ast)`
   `.with_context(|| "Failed to build the internal build graph from the manifest")?`
    .

4. This process of propagation and contextualisation repeats as the error
   bubbles up towards `main`. Use `anyhow::Context` to add detail, but never
   convert a `miette::Diagnostic` into a plain `anyhow::Error`--doing so would
   discard spans and help text.

5. Finally, the `main` function receives the `Err` result. It prints the entire
   error chain provided by `anyhow`, which displays the highest-level context
   first, followed by a list of underlying "Caused by:" messages. This provides
   the user with a rich, layered explanation of the failure, from the general
   to the specific.

For automation use cases, Netsuke will support a `--diag-json` flag. When
enabled, the entire error chain is serialised to JSON, allowing editors and CI
tools to annotate failures inline.

### 7.4 Table: Transforming Errors into User-Friendly Messages

This table provides a specification for the desired output of Netsuke's error
reporting system, contrasting raw, unhelpful messages with the friendly,
actionable output that the implementation should produce.

| Error Type | Poor Message (Default)                                                               | Netsuke's Friendly Message (Goal)                                                                                                                                               |
| ---------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| YAML Parse | (line 15, column 3): Found a tab character where indentation is expected             | Error: Failed to parse 'Netsukefile'. Caused by: Found a tab character. Hint: Use spaces for indentation instead of tabs.                                                       |
| Validation | thread 'main' panicked at 'Rule not found'                                           | Error: Build configuration is invalid. Caused by: Target 'my_program' uses a rule named 'link-program' which is not defined in the 'rules' section.                             |
| Execution  | ninja: error: 'main.o', needed by 'my_program', missing and no known rule to make it | Error: Build failed during execution. Caused by: Ninja could not build target 'my_program' because its dependency 'main.o' is missing. Hint: Ensure a target produces 'main.o'. |

## Section 8: Command-Line Interface (CLI) Design

The command-line interface is the user's entry point to Netsuke. A
well-designed CLI is essential for a good user experience. It should be
intuitive, self-documenting, and consistent with the conventions of modern
command-line tools.

### 8.1 Crate Selection: `clap`

The CLI for Netsuke will be built using the `clap` (Command Line Argument
Parser) crate, specifically leveraging its `derive` feature. `clap` is the de-
facto standard for building rich, professional CLIs in Rust. It automatically
generates parsing logic, help messages, version information, and shell
completions from simple struct definitions. Its integration with error handling
frameworks like `anyhow` is seamless, making it the ideal choice.[^32]

### 8.2 CLI Structure and Commands

The CLI's structure will be defined using a set of structs annotated with
`clap`'s derive macros. This provides a single, clear source of truth for the
entire CLI specification.

Rust

```rust
use clap::{Args, Parser, Subcommand}; use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli { /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
    file: PathBuf,

    /// Change to this directory before doing anything.
    #[arg(short = 'C', long, value_name = "DIR")]
    directory: Option<PathBuf>,

    /// Set the number of parallel build jobs.
    #[arg(short, long, value_name = "N")]
    jobs: Option<usize>,

    /// Enable verbose logging output.
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>, }

#[derive(Subcommand)]
enum Commands { /// Build specified targets (or default targets if none are
given). /// This is the default subcommand. Build(BuildArgs),

    /// Remove build artefacts and intermediate files. Clean,

    /// Display the build dependency graph in DOT format for visualisation.
    Graph,

    /// Write the Ninja manifest to `FILE` without invoking Ninja. Manifest {
    /// Output path for the generated Ninja file.
        #[arg(value_name = "FILE")]
        file: PathBuf, }, }

#[derive(Args)]
struct BuildArgs { /// Write the generated Ninja manifest to this path and
retain it.
    #[arg(long, value_name = "FILE")]
    emit: Option<PathBuf>,

    /// A list of specific targets to build. targets: Vec<String>, }
```

*Note: The* `Build` *command is wrapped in an* `Option<Commands>` *and will be
treated as the default subcommand if none is provided, allowing for the common*
`Netsuke [targets...]` *invocation.*

### 8.3 Command Behaviour

The behaviour of each subcommand is clearly defined:

- `Netsuke build [--emit FILE] [targets...]`: This is the primary and default
command. It executes the full six-stage pipeline: Manifest Ingestion, Initial
YAML Parsing, Template Expansion, Deserialisation & Final Rendering, IR
Generation & Validation, and Ninja Synthesis & Execution. By default the
generated Ninja file is written to a securely created temporary location and
removed after the build completes. Supplying `--emit FILE` writes the Ninja
file to `FILE` and retains it. If no targets are provided on the command line,
the targets listed in the `defaults` section of the manifest are built.

- `Netsuke clean`: This command provides a convenient way to clean the build
  directory. It will invoke the Ninja backend with the appropriate flags, such
  as `ninja -t clean`, to remove the outputs of the build rules.

- `Netsuke graph`: This command is an introspection and debugging tool. It will
  run the Netsuke pipeline up to Stage 4 (IR Generation) and then invoke Ninja
  with the graph tool, `ninja -t graph`. This outputs the complete build
  dependency graph in the DOT language. The result can be piped through
  `dot -Tsvg` or displayed via `netsuke graph --html` using an embedded
  Dagre.js viewer. Visualizing the graph is invaluable for understanding and
  debugging complex projects.

- `Netsuke manifest FILE`: This command performs the pipeline up to Ninja
  synthesis and writes the resulting Ninja file to `FILE` without invoking
  Ninja.

### 8.4 Design Decisions

The CLI is implemented using clap's derive API in `src/cli.rs`. Clap's
`default_value_t` attribute marks `Build` as the default subcommand, so
invoking `netsuke` with no explicit command still triggers a build. CLI
execution and dispatch live in `src/runner.rs`, keeping `main.rs` focused on
parsing. Process management, Ninja invocation, argument redaction, and the
temporary file helpers reside in `src/runner/process.rs`, allowing the runner
entry point to delegate low-level concerns. The working directory flag mirrors
Ninja's `-C` option but is resolved internally; Netsuke changes directory
before spawning Ninja rather than forwarding the flag. Error scenarios are
validated using clap's `ErrorKind` enumeration in unit tests and via Cucumber
steps for behavioural coverage.

The Ninja executable may be overridden via the `NINJA_ENV` environment
variable. For example, `NINJA_ENV=/opt/ninja/bin/ninja netsuke build` forces
Netsuke to execute the specified binary while preserving the default when the
variable is unset or invalid.

### 8.5 Manual Pages

The CLI definition doubles as the source for user documentation. A build script
uses `clap_mangen` to emit a `netsuke.1` manual page in
`target/generated-man/<target>/<profile>` and mirrors the page into Cargo's
`OUT_DIR` so release automation can discover it without additional tooling. The
staging helper always prefers the deterministic `generated-man` copy and falls
back to the most recent `OUT_DIR` candidate only when necessary, avoiding false
positives when several historical build directories remain on disk. Release
artefacts include this platform-agnostic man page; the published crate remains
code-only. The build script honours `SOURCE_DATE_EPOCH` to produce reproducible
dates, emitting a warning and falling back to `1970-01-01` when the environment
value is invalid.

### 8.6 Release Automation

Release engineering is delegated to GitHub Actions workflows built on the
`leynos/shared-actions` toolchain. The actions are pinned to
`61340852250fe0c3cf1a06a16443629fccce746e` so release automation remains
reproducible. The tagging workflow first verifies that the Git ref matches
`Cargo.toml` and records the crate's binary name once so all subsequent jobs
operate on consistent metadata. Linux builds invoke the `rust-build-release`
composite action to cross-compile for `x86_64` and `aarch64`, generate the
staged binary + man page directory, and then call the shared `linux-packages`
composite a second time with explicit metadata so the resulting `.deb` and
`.rpm` archives both declare a runtime dependency on `ninja-build`. Windows
builds reuse the same action for compilation and now invoke the generic staging
composite defined in `.github/actions/stage`. The composite shells out to a
Cyclopts-driven script that reads `.github/release-staging.toml`, merges the
`[common]` configuration with the target-specific overrides, and copies the
configured artefacts into a fresh `dist/{bin}_{platform}_{arch}` directory. The
helper writes SHA-256 sums for every staged file and exports a JSON map of the
artefact outputs, allowing the workflow to hydrate downstream steps without
hard-coded path logic. Figure 8.1 summarises the configuration entities,
including optional keys reserved for templated directories and explicit
artefact destinations that the helper can adopt without breaking compatibility.

Figure 8.1: Entity relationship for the staging configuration schema.

```mermaid
%% Figure 8.1: Entity relationship for the staging configuration schema.
erDiagram
  COMMON {
    string bin_name
    string dist_dir
    string checksum_algorithm
    string staging_dir_template
    ArtefactConfig[] artefacts
  }
  TARGETS {
    string platform
    string arch
    string target
    string bin_ext
    string staging_dir_template
    ArtefactConfig[] artefacts
  }
  ArtefactConfig {
    string source
    boolean required
    string output
    string destination
    string[] alternatives
  }
  COMMON ||--o{ TARGETS : "has targets"
  COMMON ||--o{ ArtefactConfig : "has artefacts"
  TARGETS ||--o{ ArtefactConfig : "has artefacts"
```

The staged artefacts feed a WiX v4 authoring template stored in
`installer/Package.wxs`; the workflow invokes the shared
`windows-package@61340852250fe0c3cf1a06a16443629fccce746e` composite to convert
the repository licence into RTF, embed the binary, and output a signed MSI
installer alongside the staged directory. The composite pins the
`WixToolset.UI.wixext` extension to v6 to match the WiX v6 CLI and avoid the
`WIX6101` incompatibility seen with the legacy v4 bundle. The installer uses
WiX v4 syntax, installs per-machine, and presents the minimal UI appropriate
for a CLI tool. Windows does not modify the PATH, so users must add the
installation directory manually if they want global command resolution. The
Unix manual page remains in the staged artefacts for parity with the other
platforms but is not bundled into the installer to avoid shipping an
inaccessible help format.

macOS releases execute the shared action twice: once on an Intel runner and
again on Apple Silicon. The same composite action interprets the TOML
configuration, emits checksums, and exposes artefact metadata via JSON outputs
before feeding the resulting paths into the `macos-package` action. Embedding
the PEP 723 metadata keeps Cyclopts discoverable without a repository-level
`pyproject.toml`, maintaining the existing approach where uv resolves
dependencies on demand. Python linting still lives in the top-level
`ruff.toml`, so the dedicated staging scripts remain self-contained whilst the
broader helper suite stays consistently linted.

Each job uploads its products as workflow artefacts, and the final release job
downloads every file, filters out unrelated downloads, and prefixes asset names
with their staging directories to avoid collisions before attaching them to the
GitHub release draft. This automated pipeline guarantees parity across Windows,
Linux, and macOS without custom GoReleaser logic.

## Section 9: Implementation Roadmap and Strategic Recommendations

This final section outlines a strategic plan for implementing Netsuke, along
with a summary of key technological choices and potential avenues for future
development. This roadmap is designed to manage complexity, mitigate risk, and
ensure that a functional and robust tool is delivered efficiently.

### 9.1 Phased Implementation Plan

A phased implementation approach is recommended to tackle the project in
manageable stages. Each phase builds upon the last and has a clear, verifiable
goal.

- **Phase 1: The Static Core**

  - **Objective:** To create a minimal, working build compiler for static
    manifests.

  - **Tasks:**

    1. Implement the initial `clap` CLI structure for the `build` command.

    2. Implement the YAML parser using `serde_yml` and the AST data structures
       (`ast.rs`).

    3. Implement the AST-to-IR transformation logic, including basic validation
       like checking for rule existence.

    4. Implement the IR-to-Ninja file generator (`ninja_gen.rs`).

    5. Implement the `std::process::Command` logic to invoke `ninja`.

  - **Success Criterion:** Netsuke can successfully take a `Netsukefile` file
    *without any Jinja syntax* and compile it to a `build.ninja` file, then
    execute it to produce the correct artefacts. This phase validates the
    entire static compilation pipeline.

- **Phase 2: The Dynamic Engine**

  - **Objective:** To integrate the templating engine and support dynamic
    manifests.

  - **Tasks:**

    1. Integrate the `minijinja` crate into the build pipeline.

    2. Implement the two-pass parsing mechanism: first render the manifest with
       `minijinja`, then parse the result with `serde_yml`.

    3. Populate the initial Jinja context with the global `vars` from the
       manifest.

    4. Implement basic Jinja control flow (`{% if... %}`, `{% for... %}`) and
       variable substitution.

  - **Success Criterion:** Netsuke can successfully build a manifest that uses
    variables and conditional logic (e.g., different compiler flags based on a
    variable).

- **Phase 3: The "Friendly" Polish**

  - **Objective:** To implement the advanced features that deliver a superior
    user experience.

  - **Tasks:**

    1. Implement the full suite of custom Jinja functions (`glob`, `env`, etc.)
       and filters (`shell_escape`).

    2. Mandate the use of `shell-quote` for all command variable substitutions.

    3. Refactor the error handling to fully adopt the `anyhow`/`thiserror`
       strategy, ensuring all user-facing errors are contextual and actionable
       as specified in Section 7.

    4. Implement the `clean` and `graph` subcommands.

    5. Refine the CLI output for clarity and readability.

  - **Success Criterion:** Netsuke is a feature-complete, secure, and
    user-friendly build tool that meets all the initial design goals.

### 9.2 Key Technology Summary

This table serves as a quick-reference guide to the core third-party crates
selected for this project and the rationale for their inclusion.

| Component      | Recommended Crate           | Rationale                                                                                                                       |
| -------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| CLI Parsing    | clap                        | The Rust standard for powerful, derive-based CLI development.                                                                   |
| YAML Parsing   | serde_yml                   | Mature, stable, and provides seamless integration with the serde framework.                                                     |
| Templating     | minijinja                   | High compatibility with Jinja2, minimal dependencies, and supports runtime template loading.                                    |
| Shell Quoting  | shell-quote                 | A critical security component; provides robust, shell-specific escaping for command arguments.                                  |
| Error Handling | anyhow + thiserror + miette | An idiomatic and powerful combination for creating rich, contextual, and user-friendly error reports with precise source spans. |
| Logging        | tracing                     | Structured, levelled diagnostic output for debugging and insight.                                                               |
| Versioning     | semver                      | The standard library for parsing and evaluating Semantic Versioning strings, essential for the `netsuke_version` field.         |

### 9.3 Future Enhancements

The architecture described in this document provides a solid foundation for a
powerful build tool. The use of a decoupled IR, in particular, opens up many
possibilities for future enhancements beyond the initial scope.

- **Advanced Caching:** While Ninja provides excellent file-based incremental
  build caching, Netsuke could implement a higher-level artefact caching layer.
  This could involve caching build outputs in a shared network location (e.g.,
  S3) or a local content-addressed store, allowing for cache hits across
  different machines or clean checkouts.

- **Plugin Architecture:** A system could be designed to allow users to load
  custom Jinja functions and filters from external Rust crates at runtime. This
  would enable the community to extend Netsuke's capabilities without requiring
  changes to the core application.

- **Language-Specific Toolchains:** Netsuke could offer pre-packaged "toolchain"
  modules. For example, a `Netsuke-rust-toolchain` could provide a standard set
  of rules and variables for compiling Rust projects, abstracting away the
  details of invoking `cargo`.

- **Distributed Builds:** The IR is backend-agnostic. A future version of
  Netsuke could include an alternative generator that targets a distributed
  build system, allowing for massively parallel builds across a cluster of
  machines. The user's `Netsukefile` manifest would remain unchanged.

## Section 10: Example Manifests

The repository includes several complete Netsuke manifests in the `examples/`
directory. They demonstrate how the YAML schema can be applied to real-world
projects.

- [`basic_c.yml`](../examples/basic_c.yml): a minimal C project compiling two
  object files and linking them into a small application.
- [`photo_edit.yml`](../examples/photo_edit.yml): converts RAW photographs and
  generates a simple HTML gallery for previewing the results.
- [`visual_design.yml`](../examples/visual_design.yml): rasterises a set of SVG
  design assets into PNG images using Inkscape.
- [`website.yml`](../examples/website.yml): builds a static web site from
  Markdown pages with Pandoc and assembles an index page.
- [`writing.yml`](../examples/writing.yml): produces a multi-chapter PDF book
  by combining chapters rendered from Markdown via LaTeX.

### **Works cited**

[^1]: Ninja, a small build system with a focus on speed. Accessed on 12 July
      2025\. <https://ninja-build.org/>

[^2]: "Ninja (build system)." Wikipedia. Accessed on 12 July 2025\.
      <https://en.wikipedia.org/wiki/Ninja_(build_system)>

[^3]: "A Complete Guide To The Ninja Build System." Spectra - Mathpix. Accessed
      on 12 July 2025\.
      <https://spectra.mathpix.com/article/2024.01.00364/a-complete-guide-to-the-ninja-build-system>

[^4]: "semver - Rust." Accessed on 12 July 2025\.
      <https://creative-coding-the-hard-way.github.io/Agents/semver/index.html>

[^7]: "How Ninja works." Fuchsia. Accessed on 12 July 2025\.
      <https://fuchsia.dev/fuchsia-src/development/build/ninja_how>

[^8]: "The Ninja build system." Ninja. Accessed on 12 July 2025\.
      <https://ninja-build.org/manual.html>

[^11]: "Saphyr libraries." crates.io. Accessed on 12 July 2025\.
       <https://crates.io/crates/saphyr>

[^15]: "minijinja." crates.io. Accessed on 12 July 2025\.
       <https://crates.io/crates/minijinja>

[^16]: "minijinja." Docs.rs. Accessed on 12 July 2025\.
       <https://docs.rs/minijinja/>

[^17]: "minijinja." wasmer-pack API docs. Accessed on 12 July 2025\.
       <https://wasmerio.github.io/wasmer-pack/api-docs/minijinja/index.html>

[^18]: "Template engine - list of Rust libraries/crates." Lib.rs. Accessed on
       12 July 2025\. <https://lib.rs/template-engine>

[^22]: "shell_quote." Docs.rs. Accessed on 12 July 2025\.
       <https://docs.rs/shell-quote/latest/shell_quote/>

[^24]: "std::process." Rust. Accessed on 12 July 2025\.
       <https://doc.rust-lang.org/std/process/index.html>

[^27]: "Rust Error Handling Compared: anyhow vs thiserror vs snafu." dev.to.
       Accessed on 12 July 2025\.
       <https://dev.to/leapcell/rust-error-handling-compared-anyhow-vs-thiserror-vs-snafu-2003>

[^29]: "Practical guide to Error Handling in Rust." Dev State. Accessed on 12
       July 2025. <https://dev-state.com/posts/error_handling/>

[^30]: "thiserror and anyhow." Comprehensive Rust. Accessed on 12 July 2025\.
       <https://comprehensive-rust.mo8it.com/error-handling/thiserror-and-anyhow.html>

[^31]: "Simple error handling for precondition/argument checking in Rust."
       Stack Overflow. Accessed on 12 July 2025\.
       <https://stackoverflow.com/questions/78217448/simple-error-handling-for-precondition-argument-checking-in-rust>

[^32]: "Nicer error reporting." Command Line Applications in Rust. Accessed on
       12 July 2025\. <https://rust-cli.github.io/book/tutorial/errors.html>
