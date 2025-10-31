# Netsuke User Guide

## 1\. Introduction: What is Netsuke?

Netsuke is a modern, declarative build system designed to be intuitive, fast,
and safe. Think of it not just as a `make` replacement, but as a **build system
compiler**. You describe your build process in a human-readable YAML manifest
(`Netsukefile`), leveraging the power of Jinja templating for dynamic logic.
Netsuke then compiles this high-level description into an optimized build plan
executed by the high-performance [Ninja](https://ninja-build.org/ "null") build
system.

**Core Philosophy:**

- **Declarative:** Define *what* you want to build, not *how* step-by-step.

- **Dynamic where needed:** Use Jinja for variables, loops (`foreach`),
  conditionals (`when`), file globbing (`glob`), and more.

- **Static Execution Plan:** All dynamic logic is resolved *before*
  execution, resulting in a static Ninja file for fast, reproducible builds.

- **Safety First:** Automatic shell escaping prevents command injection
  vulnerabilities.

- **Fast Execution:** Leverages Ninja for efficient dependency tracking and
  parallel execution.

## 2\. Getting Started

### Installation

Netsuke is typically built from source using Cargo:

```sh
cargo build --release
# The executable will be in target/release/netsuke

```

Refer to the project's `README.md` or release pages for pre-compiled binaries
if available. Ensure the `ninja` executable is also installed and available in
your system's `PATH`.

### Basic Usage

The primary way to use Netsuke is through its command-line interface (CLI). The
default command is `build`.

1. **Create a `Netsukefile`:** Define your build rules and targets in a YAML
   file named `Netsukefile` in your project root.

2. **Run Netsuke:** Execute the `netsuke` command.

```sh
netsuke # Builds default targets defined in Netsukefile
netsuke build target_name another_target # Builds specific targets

```

If no `Netsukefile` is found, Netsuke will provide a helpful error message
guiding you.

## 3\. The Netsukefile Manifest

The `Netsukefile` is a YAML file describing your build process.

### Top-Level Structure

```yaml
# Mandatory: Specifies the manifest format version
netsuke_version: "1.0.0"

# Optional: Global variables accessible throughout the manifest
vars:
  cc: gcc
  src_dir: src

# Optional: Reusable Jinja macros
macros:
  - signature: "compile_cmd(input, output)"
    body: |
      {{ cc }} -c {{ input }} -o {{ output }}

# Optional: Reusable command templates
rules:
  - name: link
    command: "{{ cc }} {{ ins }} -o {{ outs }}"

# Optional: Phony targets often used for setup or meta-tasks
# Implicitly phony: true, always: false
actions:
  - name: clean
    command: "rm -rf build *.o"

# Required: Defines the build targets (artefacts to produce)
targets:
  - name: build/main.o
    # A target can define its command inline...
    command: "{{ compile_cmd('src/main.c', 'build/main.o') }}"
    sources: src/main.c

  - name: my_app
    # ...or reference a rule
    rule: link
    sources: build/main.o

# Optional: Targets to build if none are specified on the command line
defaults:
  - my_app

```

### Key Sections Explained

- `netsuke_version` (**Required**): Semantic version string (e.g., `"1.0.0"`)
  indicating the manifest schema version. Parsed using `semver`.

- `vars` (Optional): A mapping (dictionary) of global variables. Values can
  be strings, numbers, booleans, or lists, accessible within Jinja
  expressions.

- `macros` (Optional): A list of Jinja macro definitions. Each item has a
  `signature` (e.g., `"my_macro(arg1, arg2='default')"` ) and a multi-line
  `body`.

- `rules` (Optional): A list of named, reusable command templates.

- `targets` (**Required**): A list defining the primary build outputs (files
  or logical targets).

- `actions` (Optional): Similar to `targets`, but entries here are implicitly
  treated as `phony: true` (meaning they don't necessarily correspond to a
  file and should run when requested). Useful for tasks like `clean` or
  `test`.

- `defaults` (Optional): A list of target names (from `targets` or `actions`)
  to build when `netsuke` is run without specific targets.

## 4\. Defining Rules

Rules encapsulate reusable build commands or scripts.

```yaml
rules:
  - name: compile # Unique identifier for the rule
    # Recipe: Exactly one of 'command', 'script', or 'rule'
    command: "{{ cc }} {{ cflags }} -c {{ ins }} -o {{ outs }}"
    # Optional: Displayed during the build
    description: "Compiling {{ outs }}"
    # Optional: Ninja dependency file info (gcc or msvc format)
    deps: gcc

```

- `name`: Unique string identifier.

- `recipe`: The action to perform. Defined by one of:

  - `command`: A single shell command string. May contain `{{ ins }}`
    (space-separated inputs) and `{{ outs }}` (space-separated outputs).
    These specific placeholders are substituted *after* Jinja rendering but
    *before* hashing the action. All other Jinja interpolations happen
    first. The final command must be parseable by `shlex` (POSIX mode).

  - `script`: A multi-line script (using YAML `|`). If it starts with `#!`,
    it's executed directly. Otherwise, it's run via `/bin/sh -e` (or
    PowerShell on Windows) by default. Interpolated variables are
    automatically shell-escaped unless `| raw` is used.

  - `rule`: References another rule by name (less common within a rule
    definition).

- `description` (Optional): A user-friendly message printed by Ninja when
  this rule runs. Can contain `{{ ins }}` / `{{ outs }}`.

- `deps` (Optional): Specifies dependency file format (`gcc` or `msvc`) for
  C/C++ header dependencies, generating Ninja's `depfile` and `deps`
  attributes.

## 5\. Defining Targets and Actions

Targets define *what* to build or *what action* to perform.

```yaml
targets:
  # Example 1: Building an object file using a rule
  - name: build/utils.o         # Output file(s).
    # Can be a string or list.
    rule: compile               # Rule to use.
    # Mutually exclusive with command/script.
    sources: src/utils.c        # Input file(s). String or list.
    deps:                       # Explicit dependencies.
    # Targets built before this one.
      - build/utils.h
    vars:                       # Target-local variables, override globals
      cflags: "-O0 -g"

  # Example 2: Linking an executable using an inline command
  - name: my_app
    command: "{{ cc }} build/main.o build/utils.o -o my_app"
    sources:                    # Implicit dependencies derived from
    # command/rule usage.
      - build/main.o
      - build/utils.o
    order_only_deps:            # Dependencies built before.
    # Changes do not trigger rebuild.
      - build_directory         # For example, ensure 'build/' exists.

  # Example 3: A phony action (can also be in top-level 'actions:')
  - name: package
    phony: true                 # Doesn't represent a file.
    # Always considered out-of-date.
    command: "tar czf package.tar.gz my_app"
    deps: my_app                # Depends on the 'my_app' target

  # Example 4: A target that always runs
  - name: timestamp
    always: true                # Runs every time.
    # Regardless of inputs/outputs.
    command: "date > build/timestamp.txt"

```

- `name`: Output file path(s). Can be a string or a list (`StringOrList`).

- `recipe`: How to build the target. Defined by one of `rule`, `command`, or
  `script` (mutually exclusive).

- `sources`: Input file path(s) (`StringOrList`). If a source matches another
  target's `name`, an implicit dependency is created.

- `deps` (Optional): Explicit target dependencies (`StringOrList`). Changes
  trigger rebuilds.

- `order_only_deps` (Optional): Dependencies that must run first but whose
  changes don't trigger rebuilds (`StringOrList`). Maps to Ninja `||`.

- `vars` (Optional): Target-specific variables that override global `vars`.

- `phony` (Optional, default `false`): Treat target as logical, not a file.
  Always out-of-date if requested.

- `always` (Optional, default `false`): Re-run the command every time
  `netsuke` is invoked, regardless of dependency changes.

`StringOrList`: Fields like `name`, `sources`, `deps`, and `order_only_deps`
accept either a single string or a YAML list of strings for convenience.

## 6\. Jinja Templating in Netsuke

Netsuke uses the [MiniJinja](https://docs.rs/minijinja "null") engine to add
dynamic capabilities to your manifest.

### Basic Syntax

- Variables: `{{ my_variable }}`

- Expressions: `{{ 1 + 1 }}`, `{{ sources | map('basename') }}`

- Control Structures (within specific keys like `foreach`, `when`, or
  inside `macros`): `{% if enable %}...{% endif %}`,
  `{% for item in list %}...{% endfor %}`

**Important:** Structural Jinja (`{% %}`) is generally **not** allowed directly
within the YAML structure outside of `macros`. Logic should primarily be within
string values or dedicated keys like `foreach` and `when`.

### Processing Order

Netsuke processes the manifest in stages:

1. Initial YAML Parse: Load the raw file into an intermediate structure (like
   `serde_json::Value`).

2. Template Expansion (`foreach`, `when`): Evaluate `foreach` expressions to
   generate multiple target definitions. The `item` (and optional `index`)
   become available in the context. Evaluate `when` expressions to
   conditionally include/exclude targets.

3. Deserialisation to AST: Convert the expanded intermediate structure into
   Netsuke's typed Rust structs (`NetsukeManifest`, `Target`, etc.).

4. Final Rendering: Render Jinja expressions **only within string fields**
   (like `command`, `description`, `name`, `sources` etc.) using the combined
   context (globals + target vars + iteration vars).

### `foreach` and `when`

These keys enable generating multiple similar targets programmatically.

```yaml
targets:
  # Generate a target for each .c file in src/
  - foreach: glob('src/*.c')        # Jinja expression returning an iterable
    # Only include if the filename isn't 'skip.c'
    when: item | basename != 'skip.c'
    # 'item' and 'index' (0-based) are available in the context
    name: "build/{{ item | basename | with_suffix('.o') }}"
    rule: compile
    sources: "{{ item }}"
    vars:
      compile_flags: "-O{{ index + 1 }}" # Example using index

```

- `foreach`: A Jinja expression evaluating to a list (or any iterable). A
  target definition will be generated for each item.

- `when` (Optional): A Jinja expression evaluating to a boolean. If false,
  the target generated for the current `item` is skipped.

### User-defined Macros

Define reusable Jinja logic in the top-level `macros` section.

```yaml
macros:
    - signature: "cc_cmd(src, obj, flags='')" # Jinja macro signature
      body: |                                # Multi-line body
        {{ cc }} {{ flags }} -c {{ src|shell_escape }} -o {{ obj|shell_escape }}

targets:
  - name: build/main.o
    command: "{{ cc_cmd('src/main.c', 'build/main.o', flags=cflags) }}"
    sources: src/main.c

```

## 7\. Netsuke Standard Library (Stdlib)

Netsuke provides a rich set of built-in Jinja functions, filters, and tests to
simplify common build tasks. These are automatically available in your manifest
templates.

### Key Functions

- `env(name, default=None)`: Reads an environment variable. Fails if `name`
  is unset and no `default` is provided. Example: `{{ env('CC', 'gcc') }}`

- `glob(pattern)`: Expands a filesystem glob pattern into a sorted list of
  *files* (directories are excluded). Handles `*`, `**`, `?`, `[]`.
  Case-sensitive. Example: `{{ glob('src/**/*.c') }}`

- `fetch(url, cache=False)`: Downloads content from a URL. If `cache=True`,
  caches the result in `.netsuke/fetch` within the workspace based on URL
  hash. Marks template as impure.

- `now(offset=None)`: Returns the current time as a timezone-aware object
  (defaults to UTC). `offset` can be '+HH:MM' or 'Z'. Exposes
  `.iso8601`, `.unix_timestamp`, `.offset`.

- `timedelta(...)`: Creates a duration object (e.g., for age comparisons).
  Accepts `weeks`, `days`, `hours`, `minutes`, `seconds`,
  `milliseconds`, `microseconds`, `nanoseconds`. Exposes `.iso8601`,
  `.seconds`, `.nanoseconds`.

### Key Filters

Apply filters using the pipe `|` operator: `{{ value | filter_name(args...) }}`

**Path & File Filters:**

- `basename`: `{{ 'path/to/file.txt' | basename }}` -> `"file.txt"`

- `dirname`: `{{ 'path/to/file.txt' | dirname }}` -> `"path/to"`

- `with_suffix(new_suffix, count=1, sep='.')`: Replaces the last `count`
  dot-separated extensions. `{{ 'archive.tar.gz' | with_suffix('.zip', 2) }}`
  -> `"archive.zip"`

- `relative_to(base_path)`: Makes a path relative.
  `{{ '/a/b/c' | relative_to('/a/b') }}` -> `"c"`

- `realpath`: Canonicalizes path, resolving symlinks.

- `expanduser`: Expands `~` to the home directory.

- `contents(encoding='utf-8')`: Reads file content as a string.

- `size`: File size in bytes.

- `linecount`: Number of lines in a text file.

- `hash(alg='sha256')`: Full hex digest of file content (supports
  `sha256`, `sha512`; `md5`, `sha1` if `legacy-digests` feature enabled).

- `digest(len=8, alg='sha256')`: Truncated hex digest.

- `shell_escape`: **Crucial for security.** Safely quotes a string for use as
  a single shell argument. *Use this whenever interpolating paths or
  variables into commands unless you are certain they are safe.*

**Collection Filters:**

- `uniq`: Removes duplicate items from a list, preserving order.

- `flatten`: Flattens a nested list. `{{ [[1], [2, 3]] | flatten }}` ->
  `[1, 2, 3]`

- `group_by(attribute)`: Groups items in a list of dicts/objects by an
  attribute's value.

- `map(attribute='...')` / `map('filter', ...)`: Applies attribute access or
  another filter to each item in a list.

- `filter(attribute='...')` / `filter('test', ...)`: Selects items based on
  attribute value or a test function.

- `join(sep)`: Joins list items into a string.

- `sort`: Sorts list items.

**Command Filters (Impure):**

- `shell(command_string)`: Pipes the input value (string or bytes) as stdin
  to `command_string` executed via the system shell (`sh -c` or `cmd /C`).
  Returns stdout. **Marks the template as impure.** Example:
  `{{ user_list | shell('grep admin') }}`

- `grep(pattern, flags=None)`: Filters input lines matching `pattern`.
  `flags` can be a string (e.g., `'-i'`) or list of strings. Implemented via
  `shell`. Marks template as impure.

**Impurity:** Filters like `shell` and functions like `fetch` interact with the
outside world. Netsuke tracks this "impurity". Impure templates might affect
caching or reproducibility analysis in future versions. Use impure helpers
judiciously.

### Key Tests

Use tests with the `is` keyword: `{% if path is file %}`

- `file`, `dir`, `symlink`: Checks filesystem object type (without following
  links).

- `readable`, `writable`, `executable`: Checks permissions for the current
  user.

- `absolute`, `relative`: Checks path type.

## 8\. Command-Line Interface (CLI)

Netsuke's CLI provides commands to manage your build.

```text
netsuke [OPTIONS] [COMMAND] [TARGETS...]

```

### Global Options

- `-f, --file <FILE>`: Path to the `Netsukefile` (default: `Netsukefile`).

- `-C, --directory <DIR>`: Change to directory `DIR` before doing anything.

- `-j, --jobs <N>`: Set the number of parallel jobs Ninja should run
  (default: Ninja's default).

- `-v, --verbose`: Enable verbose logging output.

### Commands

- `build` (default): Compiles the manifest and runs Ninja to build the
  specified `TARGETS` (or the `defaults` if none are given).

  - `--emit <FILE>`: Write the generated `build.ninja` file to `<FILE>` and
    keep it, instead of using a temporary file.

- `manifest <FILE>`: Generates the `build.ninja` file and writes it to
  `<FILE>` without executing Ninja. Useful for debugging or integration with
  other tools.

- `clean`: Removes build artefacts by running `ninja -t clean`. Requires
  rules/targets to be properly configured for cleaning in Ninja (often via
  `phony` targets).

- `graph`: Generates the build dependency graph and outputs it in DOT format
  (suitable for Graphviz). Future versions may support other formats like
  `--html`.

### Exit Codes

- `0`: Success.

- Non-zero: Failure (e.g., manifest parsing error, template error, build
  command failure).

## 9\. Examples in Practice

Refer to the `examples/` directory in the Netsuke repository for practical
manifest files:

- `basic_c.yml`: A simple C project compilation. Demonstrates
  `vars`, `rules`, `targets`, and `defaults`.

- `website.yml`: Builds a static website from Markdown files using Pandoc.
  Shows `glob`, `foreach`, and path manipulation filters
  (`basename`, `with_suffix`).

- `photo_edit.yml`: Processes RAW photos with `darktable-cli` and creates a
  gallery. Uses `glob`, `foreach`, and `actions`.

- `visual_design.yml`: Rasterizes SVG files using Inkscape. Uses `env`
  function with a default.

- `writing.yml`: Compiles a multi-chapter book from Markdown to PDF via
  LaTeX. Shows more complex dependencies and sorting of glob results.

These examples illustrate how to combine Netsuke's features to manage different
kinds of build workflows effectively.

## 10\. Error Handling

Netsuke aims for clear and actionable error messages.

- **YAML Errors:** If `Netsukefile` is invalid YAML, `serde-saphyr` provides
  location info (line, column). Netsuke may add hints for common issues
  (e.g., tabs vs spaces).

- **Schema Errors:** If the YAML structure doesn't match the expected schema
  (e.g., missing `targets`, unknown keys), errors indicate the structural
  problem.

- **Template Errors:** Jinja errors during `foreach`, `when`, or string
  rendering include context (e.g., undefined variable, invalid filter usage)
  and potentially location information.

- **IR Validation Errors:** Errors found during graph construction (e.g.,
  missing rules, duplicate outputs, circular dependencies) are reported
  before execution.

- **Build Failures:** If a command run by Ninja fails, Netsuke reports the
  failure, typically showing the command's output/error streams captured by
  Ninja.

Use the `-v` flag for more detailed error context or internal logging.

## 11\. Security Considerations

- **Command Injection:** Netsuke automatically shell-escapes variables
  interpolated into `command:` strings *unless* the `| raw` Jinja filter is
  explicitly used. Avoid `| raw` unless you fully trust the variable's
  content. File paths from `glob` or placeholders like `{{ ins }}` /
  `{{ outs }}` are quoted safely.

- **`script:` Execution:** Scripts run via the specified interpreter
  (defaulting to `sh -e`). Ensure scripts handle inputs safely.

- **Impure Functions/Filters:**
  `env()`, `glob()`, `fetch()`, `shell()`, `grep()` interact with the
  environment or network. Be mindful when using them with untrusted manifest
  parts. Future versions might offer sandboxing options.

Always review `Netsukefile` manifests, especially those from untrusted sources,
before building.
