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

### Release help artefacts

Release archives include platform help files generated from the same command
metadata that powers `netsuke --help`. Unix-like release artefacts include the
`netsuke.1` manual page. Windows release artefacts include PowerShell external
help in a `Netsuke` module layout, so installed or unpacked artefacts can be
inspected with:

```powershell
Get-Help Netsuke -Full
```

This change affects release packaging only. The Netsuke command-line flags,
subcommands, output, and exit statuses are unchanged.

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

If no `Netsukefile` is found, Netsuke will provide a helpful error message:

```text
Error: Manifest 'Netsukefile' not found in the current directory.
help: Ensure the manifest exists or pass `--file` with the correct path.
```

A different manifest path can be specified using the `-f` or `--file` option:

```sh
netsuke -f path/to/manifest.yml
```

For a step-by-step introduction, see the [Quick Start guide](quickstart.md).

## 3\. The Netsukefile Manifest

The `Netsukefile` is a YAML file describing the build process.

Netsuke targets YAML 1.2 and forbids duplicate keys in manifests. If the same
mapping key appears more than once (even if a YAML parser would normally accept
it with “last key wins” behaviour), Netsuke treats this as an error.

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
  be strings, numbers, booleans, or lists, accessible within Jinja expressions.

- `macros` (Optional): A list of Jinja macro definitions. Each item has a
  `signature` (e.g., `"my_macro(arg1, arg2='default')"` ) and a multi-line
  `body`.

- `rules` (Optional): A list of named, reusable command templates.

- `targets` (**Required**): A list defining the primary build outputs (files
  or logical targets).

- `actions` (Optional): Similar to `targets`, but entries here are implicitly
  treated as `phony: true` (meaning they don't necessarily correspond to a file
  and should run when requested). Useful for tasks like `clean` or `test`.

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

```

- `name`: Unique string identifier.

- `recipe`: The action to perform. Defined by one of:

  - `command`: A single shell command string. May contain `{{ ins }}`
    (space-separated inputs) and `{{ outs }}` (space-separated outputs). These
    specific placeholders are substituted *after* Jinja rendering but *before*
    hashing the action. All other Jinja interpolations happen first. The final
    command must be parseable by `shlex` (POSIX mode).

  - `script`: A multi-line script (using YAML `|`). If it starts with `#!`,
    it's executed directly. Otherwise, it's run via `/bin/sh -e` (or PowerShell
    on Windows) by default. Interpolated variables are automatically
    shell-escaped unless `| raw` is used.

  - `rule`: References another rule by name (less common within a rule
    definition).

- `description` (Optional): A user-friendly message printed by Ninja when
  this rule runs. Can contain `{{ ins }}` / `{{ outs }}`.

## 5\. Defining Targets and Actions

Targets define *what* to build or *what action* to perform.

```yaml
targets:
  # Example 1: Building an object file using a rule
  - name: build/utils.o         # Output file(s). Can be a string or list.
    rule: compile               # Rule to use (mutually exclusive with command/script)
    sources: src/utils.c        # Input file(s). String or list.
    deps:                       # Implicit dependencies: trigger rebuilds but
      # not passed to $in/{{ ins }}
      - build/utils.h
    vars:                       # Target-local variables, override globals
      cflags: "-O0 -g"

  # Example 2: Linking an executable using an inline command
  - name: my_app
    command: "{{ cc }} build/main.o build/utils.o -o my_app"
    sources:                    # Explicit recipe inputs: passed to
      # $in/{{ ins }} and trigger rebuilds
      - build/main.o
      - build/utils.o
    order_only_deps:            # Dependencies built before, but changes
      # do not trigger rebuild
      - build_directory         # e.g., Ensure 'build/' exists

  # Example 3: A phony action (can also be in top-level 'actions:')
  - name: package
    phony: true                 # Doesn't represent a file, always considered out-of-date
    command: "tar czf package.tar.gz my_app"
    deps: my_app                # Depends on the 'my_app' target

  # Example 4: A target that always runs
  - name: timestamp
    always: true                # Runs every time, regardless of inputs/outputs
    command: "date > build/timestamp.txt"

```

- `name`: Output file path(s). Can be a string or a list (`StringOrList`).

- `recipe`: How to build the target. Defined by one of `rule`, `command`, or
  `script` (mutually exclusive).

- `sources`: Input file path(s) (`StringOrList`). Sources are explicit recipe
  inputs: they are passed to `$in` and `{{ ins }}` and trigger rebuilds when
  changed.

- `deps` (Optional): Implicit target dependencies (`StringOrList`). Changes
  trigger rebuilds, but these paths are not passed to `$in` or `{{ ins }}`.
  Maps to Ninja `|`.

- `order_only_deps` (Optional): Dependencies that must run first but whose
  changes don't trigger rebuilds (`StringOrList`). Maps to Ninja `||`.

Cycle detection traverses `sources` and `deps`, because both classes affect the
build graph and rebuild freshness. `order_only_deps` only enforce build
ordering and do not participate in Netsuke's cycle detection.

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

- Control Structures (within specific keys like `foreach`, `when`, or inside
  `macros`): `{% if enable %}…{% endif %}`,
  `{% for item in list %}…{% endfor %}`

**Important:** Structural Jinja (`{% %}`) is generally **not** allowed directly
within the YAML structure outside of `macros`. Logic should primarily be within
string values or dedicated keys like `foreach` and `when`.

### Processing Order

Netsuke processes the manifest in stages:

1. Initial YAML Parse: Load the raw file into an intermediate structure (like
   `serde_json::Value`).

2. Template Expansion (`foreach`, `when`): Evaluate `foreach` expressions to
   generate multiple target or action definitions. The `item` (and optional
   `index`) become available in the context. Evaluate `when` expressions to
   conditionally include/exclude entries (`when` can exclude both targets and
   actions). This is a manifest-time selection step; skipped entries are
   removed before Netsuke builds the typed manifest AST, creates its IR, emits
   `build.ninja`, or runs Ninja.

3. Deserialization to AST: Convert the expanded intermediate structure into
   Netsuke's typed Rust structs (`NetsukeManifest`, `Target`, etc.).

4. Final Rendering: Render Jinja expressions **only within string fields**
   (like `command`, `description`, `name`, `sources` etc.) using the combined
   context (globals + target vars + iteration vars).

### `foreach` and `when`

These keys enable generating multiple similar targets or actions
programmatically.

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

```yaml
actions:
  # Generate named test actions, skipping disabled suites.
  - foreach:
      - unit
      - integration
      - disabled
    when: item != 'disabled'
    name: "test-{{ item }}"
    command: "cargo test --test {{ item }}"
```

- `foreach`: A Jinja expression evaluating to a list (or any iterable). A
  target or action definition will be generated for each item.

- `when` (Optional): A Jinja expression evaluating to a boolean. If false,
  the target or action generated for the current `item` is skipped.

`foreach` and `when` do not create build-time branches. They decide which
manifest entries exist while Netsuke loads the manifest. The generated Ninja
file contains only the selected targets and actions, so a skipped entry cannot
contribute rules, outputs, dependencies, defaults, or command text later in the
build pipeline.

When a decision must happen while a target is being built, put that branching
inside the recipe command or script:

```yaml
targets:
  - name: report
    script: |
      if test -f report.in; then
        ./render report.in > report
      else
        ./render-empty > report
      fi
```

A future runtime-condition feature may model build-time branching directly, but
current manifests should treat recipe commands and scripts as the build-time
decision point.

### User-defined Macros

Define reusable Jinja logic in the top-level `macros` section.

```yaml
macros:
  - signature: "cc_cmd(src, obj, flags='')" # Jinja macro signature
    body: |                                # Multi-line body
      {{ cc }} {{ flags }} -c {{ src | shell_escape }} \
        -o {{ obj | shell_escape }}

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
  caches the result in `.netsuke/fetch` within the workspace based on URL hash.
  Enforces a configurable maximum response size (default 8 MiB); requests abort
  with an error quoting the configured threshold when the limit is exceeded.
  Cached downloads stream directly to disk and remove partial files on error.
  Configure the limit with `StdlibConfig::with_fetch_max_response_bytes`. Marks
  template as impure.

- `now(offset=None)`: Returns the current time as a timezone-aware object
  (defaults to UTC). `offset` can be '+HH:MM' or 'Z'. Exposes `.iso8601`,
  `.unix_timestamp`, `.offset`.

- `timedelta(...)`: Creates a duration object (e.g., for age comparisons).
  Accepts `weeks`, `days`, `hours`, `minutes`, `seconds`, `milliseconds`,
  `microseconds`, `nanoseconds`. Exposes `.iso8601`, `.seconds`, `.nanoseconds`.

### Key Filters

Apply filters using the pipe `|` operator: `{{ value | filter_name(args...) }}`

**Path & File Filters:**

- `basename`: `{{ 'path/to/file.txt' | basename }}` -> `"file.txt"`

- `dirname`: `{{ 'path/to/file.txt' | dirname }}` -> `"path/to"`

- `with_suffix(new_suffix, count=1, sep='.')`: Replaces the last `count`
  dot-separated extensions. `{{ 'archive.tar.gz' | with_suffix('.zip', 2) }}` ->
  `"archive.zip"`

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
  a single shell argument. *Use this whenever interpolating paths or variables
  into commands unless you are certain they are safe.*

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
  `{{ user_list | shell('grep admin') }}`. The captured stdout is limited to 1
  MiB by default; configure a different budget with
  `StdlibConfig::with_command_max_output_bytes`. Exceeding the limit raises an
  `InvalidOperation` error that quotes the configured threshold. Templates can
  pass an options mapping such as `{'mode': 'tempfile'}` to stream stdout into
  a temporary file instead. The file path is returned to the template and
  remains bounded by `StdlibConfig::with_command_max_stream_bytes` (default 64
  MiB).

- `grep(pattern, flags=None)`: Filters input lines matching `pattern`.
  `flags` can be a string (e.g., `'-i'`) or list of strings. Implemented via
  `shell`. Marks template as impure. The same output and streaming limits apply
  when `grep` emits large result sets.

#### Executable discovery

The `which` filter and function resolve executables using the current `PATH`
without marking the template as impure. `{{ 'clang++' | which }}` returns the
first matching binary; `{{ which('clang++') }}` is available when a function
call is clearer than piping. Missing executables raise
`netsuke::jinja::which::not_found` with a preview of the searched directories.

Use `command_available(name, **kwargs)` when absence should select another
manifest-time branch instead of failing the render. It uses the same resolver
and cache as `which`, returns `true` when at least one executable is found, and
returns `false` for absent commands. Invalid arguments still raise
`netsuke::jinja::which::args`.

| Kwarg       | Default | Effect on `command_available`                                                |
| ----------- | ------- | ---------------------------------------------------------------------------- |
| `all`       | `false` | Accepted for kwarg symmetry with `which`; does not change the bool return.   |
| `canonical` | `false` | Canonicalises discovered paths before deciding whether any match exists.     |
| `fresh`     | `false` | Bypasses the resolver cache for this lookup and refreshes the cached result. |
| `cwd_mode`  | `auto`  | Controls current-directory search: `auto`, `always`, or `never`.             |

When `PATH` is empty and `cwd_mode="auto"`, Netsuke can still discover a
project-local executable through the bounded workspace fallback:

```yaml
actions:
  - name: lint
    command: ./tools/project-lint
    when: command_available("project-lint", cwd_mode="auto")
  - name: lint
    command: cargo clippy --workspace --all-targets --all-features -- -D warnings
    when: not command_available("project-lint", cwd_mode="auto")
```

The `which` filter remains the right choice when a missing tool should stop the
manifest render. The predicate is for optional toolchains and complementary
branches.

Use `command_available` in manifest-time `when` clauses when optional tooling
selects between actions:

```yaml
actions:
  - name: test-fast
    command: cargo nextest run
    when: command_available("cargo-nextest")
  - name: test-fast
    command: cargo test
    when: not command_available("cargo-nextest")
```

Only the selected action reaches the typed manifest and generated Ninja file.
Top-level actions selected this way still keep the normal implicit
`phony: true` behaviour.

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

- `-v, --verbose`: Enable verbose diagnostic logging and completion timing
  summaries.

- `--locale <LOCALE>`: Localize CLI help and error messages (for example
  `en-US` or `es-ES`).

### Network Policy Options

Netsuke's `fetch` helper is guarded by a configurable network policy. The
policy is configured by these global options:

- `--fetch-allow-scheme <SCHEME>`: Allow additional URL schemes beyond the
  defaults.

- `--fetch-allow-host <HOST>`: Allow the provided hostnames when default deny
  is enabled (wildcards such as `*.example.com` are supported).

- `--fetch-block-host <HOST>`: Always block the provided hostnames (wildcards
  supported), even if they are allowlisted.

- `--fetch-default-deny`: Deny all hosts by default and only permit the
  allowlist.

### Commands

- `build` (default): Compiles the manifest and runs Ninja to build the
  specified `TARGETS` (or the `defaults` if none are given).

  - `--emit <FILE>`: Write the generated `build.ninja` file to `<FILE>` and
    keep it, instead of using a temporary file. When `-C/--directory` is set,
    relative `--emit` paths are resolved under `<DIR>`.

- `manifest <FILE>`: Generates the `build.ninja` file and writes it to
  `<FILE>` without executing Ninja. Use `-` to stream the generated Ninja file
  to stdout (for example `netsuke manifest - | sed ...`). When `-C/--directory`
  is set, relative manifest output paths are resolved under `<DIR>`.

- `clean`: Removes build artefacts by running `ninja -t clean`. Requires
  rules/targets to be properly configured for cleaning in Ninja (often via
  `phony` targets).

- `graph`: Renders the build dependency graph in-process. Writes a Graphviz
  DOT document to stdout by default; `--html` selects a self-contained HTML
  page instead, and `--output <FILE>` (with `-` for stdout) chooses the
  destination. Ninja is not invoked. See section 12.2 for details.

### Configuration and Localization

Netsuke layers configuration in this order, with later entries overriding
earlier ones: defaults, configuration files, environment variables, and
command-line flags.

#### Configuration file discovery

Configuration files are discovered using OrthoConfig unless you select an
explicit file. Netsuke honours these file-selection inputs in precedence order:

- `--config <PATH>`
- `NETSUKE_CONFIG=<PATH>`
- `NETSUKE_CONFIG_PATH=<PATH>` (legacy alias)
- Automatic discovery

When one of the explicit selectors is set, Netsuke loads only that file and
skips automatic discovery. If that file is missing, cannot be loaded, or cannot
be parsed, Netsuke returns an error and stops, without falling back to
discovery. Otherwise, Netsuke searches for configuration in three scopes:

- **Project scope** — `.netsuke.toml` in the current working directory
  (or the directory specified by `-C` / `--directory`).
- **User scope** — user-specific locations:
  - `$HOME/.netsuke.toml`
  - `$XDG_CONFIG_HOME/netsuke/config.toml` (XDG (X Desktop Group); Unix;
    defaults to `$HOME/.config`)
  - `%APPDATA%\netsuke\config.toml` (Windows)
  - `%LOCALAPPDATA%\netsuke\config.toml` (Windows)
  - `$HOME/.config/netsuke/config.toml` (Unix fallback)
- **System scope** — system-wide locations:
  - Each entry in `$XDG_CONFIG_DIRS/netsuke/config.toml` (Unix; falls back
    to `/etc/xdg/netsuke/config.toml`)

Configuration precedence follows **project > user > system**: any field set in
the project file overrides the same field from user or system files, user-scope
settings override system-scope, and fields set only in lower-precedence files
are still applied.

The explicit config path is resolved against the shell's current working
directory, not the `-C` / `--directory` project anchor. The directory flag only
changes where project-scope discovery and manifest lookup begin.

#### Directory flag and project anchoring

The `-C <DIR>` / `--directory <DIR>` flag re-anchors project-scope discovery to
the specified directory instead of the current working directory. This is
useful for build scripts and CI pipelines that invoke Netsuke from a different
location:

```sh
netsuke -C /path/to/project build
```

With the flag, only `/path/to/project/.netsuke.toml` is considered for
project-scope discovery; user-scope discovery is unaffected.

For a documented starting point, see
[`docs/sample-netsuke.toml`](sample-netsuke.toml), which annotates every
supported config-file key.

#### Environment variables

Environment variables use the `NETSUKE_` prefix (for example,
`NETSUKE_JOBS=8`). Use `__` to separate nested keys when matching structured
configuration.

The layered schema is rooted in `CliConfig`. Netsuke currently accepts these
top-level configuration keys:

- `file = "Netsukefile"`
- `jobs = 8`
- `verbose = true|false`
- `locale = "en-US"`
- `fetch_allow_scheme = ["https"]`
- `fetch_allow_host = ["example.com"]`
- `fetch_block_host = ["blocked.example.com"]`
- `fetch_default_deny = true|false`
- `accessible = true|false`
- `progress = true|false`
- `theme = "auto"|"unicode"|"ascii"`
- `no_emoji = true|false`
- `spinner_mode = "auto"|"enabled"|"disabled"`
- `colour_policy = "auto"|"always"|"never"`
- `output_format = "human"`

Build-only defaults live under `[cmds.build]`:

- `emit = "out.ninja"`
- `targets = ["hello"]`

Example:

```toml
verbose = true
locale = "es-ES"
colour_policy = "auto"
spinner_mode = "auto"
output_format = "human"
theme = "ascii"
progress = true
accessible = false

[cmds.build]
targets = ["hello"]
```

`[cmds.build].targets` is used only when the user does not pass explicit build
targets on the command line. Explicit CLI targets always win.

`theme` is the canonical presentation setting. `no_emoji = true` remains as a
compatibility alias and resolves to the ASCII theme. Conflicting settings such
as `theme = "unicode"` with `no_emoji = true` are rejected during configuration
merge.

`spinner_mode = "disabled"` is equivalent to disabling progress output unless
the user explicitly sets `progress = true`, which is treated as a conflict.
Likewise, `spinner_mode = "enabled"` conflicts with `progress = false`.

JSON diagnostics are implemented through `--diag-json` and the layered
`diag_json` preference. The `--output-format json` flag and
`NETSUKE_OUTPUT_FORMAT=json` environment variable are accepted for diagnostic
mode, but configuration files intentionally reject `output_format = "json"`. Use
`output_format = "human"` in configuration files.

`colour_policy` is accepted and layered today, so users can standardize their
preferred setting, but Netsuke does not yet emit coloured terminal output, so
this value currently has no visible effect.

`NETSUKE_NINJA` overrides the Ninja executable used for `build`, `clean`, and
`graph` commands. Leave it unset to use the default `ninja` command on `PATH`,
or set it to another executable name such as `ninja-build` or to an absolute
path such as `/opt/ninja/bin/ninja` when the binary is installed outside the
default search path.

Use `--locale <LOCALE>`, `NETSUKE_LOCALE`, or a `locale = "..."` entry in a
configuration file to select localized CLI copy and error messages. Locale
precedence is: command-line flag, environment variable, configuration file,
then the system default. The same locale applies to user-facing runtime
diagnostics, including manifest parse failures, stdlib template errors, and
runner failures. Spanish (`es-ES`) is included as a reference translation;
unsupported locales fall back to English (`en-US`).

For information on contributing translations, see the
[Translator Guide](translators-guide.md).

JSON diagnostics follow the same OrthoConfig layering:

- CLI flag: `--diag-json`
- Environment variable: `NETSUKE_DIAG_JSON=true|false`
- Configuration file: `diag_json = true|false`

The newer typed preference aliases use the same precedence chain:

- `--colour-policy auto|always|never`
- `NETSUKE_COLOUR_POLICY=auto|always|never`
- `colour_policy = "auto" | "always" | "never"`
- `--spinner-mode enabled|disabled`
- `NETSUKE_SPINNER_MODE=enabled|disabled`
- `spinner_mode = "enabled" | "disabled"`
- `--output-format human|json`
- `NETSUKE_OUTPUT_FORMAT=human|json`
- `output_format = "human"` (config file; `"json"` is rejected — use
  `--diag-json` instead)
- `--default-target <TARGET>` (repeatable)
- `NETSUKE_DEFAULT_TARGETS__0=<TARGET>` style environment entries
- `default_targets = ["lint", "test"]`

For startup failures that happen before configuration files can be loaded,
Netsuke honours the CLI flag and environment variable immediately. A
configuration file cannot request JSON for errors raised while that same file
is being discovered or parsed.

### Output streams

Netsuke separates its output into two streams for scriptability:

- **stderr**: Status messages, progress indicators, stage summaries, and
  diagnostics
- **stdout**: Subprocess output (for example, `ninja -t graph` produces DOT
  graphs on stdout)

This separation allows reliable piping and redirection:

```bash
# Capture build graph without status noise
netsuke graph > build.dot

# Capture progress log without build output
netsuke build 2> progress.log

# Suppress status messages entirely
netsuke --progress false build
```

When using the `manifest` subcommand with `-` as the output path, the generated
Ninja file streams to stdout while status messages remain on stderr:

```bash
# Pipe generated Ninja file for inspection
netsuke manifest - | grep 'rule '
```

### JSON diagnostics mode

Use `--diag-json` when a caller needs machine-readable diagnostics on `stderr`
instead of the human-oriented text renderer.

When JSON diagnostics are enabled:

- Failing commands write exactly one JSON document to `stderr`.
- Successful commands write nothing to `stderr`.
- Progress lines, timing summaries, emoji prefixes, and tracing logs are
  suppressed so `stderr` remains machine-readable.
- `stdout` behaviour is unchanged. For example, `netsuke --diag-json manifest -`
  still streams the generated Ninja file to `stdout`.

The current schema version is `1`. The document shape is:

- `schema_version`: Integer schema version for compatibility checks.
- `generator`: Object containing `name` and `version`.
- `diagnostics`: Array of diagnostic objects.

Each diagnostic object contains:

- `message`: Localized summary line.
- `code`: Stable diagnostic code, or `null` when unavailable.
- `severity`: One of `error`, `warning`, or `advice`.
- `help`: Localized remediation hint, or `null`.
- `url`: Optional documentation URL.
- `causes`: Ordered error-cause chain.
- `source`: Optional source descriptor currently containing `name`.
- `primary_span`: The first labelled span, or `null`.
- `labels`: All labelled spans with `label`, `offset`, `length`, `line`,
  `column`, `end_line`, `end_column`, and `snippet`.
- `related`: Nested diagnostics rendered using the same schema.

Example:

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
      "help": "YAML does not permit tabs; use spaces for indentation.",
      "url": null,
      "causes": [
        "YAML parse error at line 2, column 2: tabs disallowed within this context",
        "tabs disallowed within this context (block indentation)",
        "at line 2, column 2"
      ],
      "source": {
        "name": "Netsukefile"
      },
      "primary_span": {
        "label": "invalid YAML",
        "offset": 10,
        "length": 1,
        "line": 2,
        "column": 2,
        "end_line": 2,
        "end_column": 2,
        "snippet": "-"
      },
      "labels": [
        {
          "label": "invalid YAML",
          "offset": 10,
          "length": 1,
          "line": 2,
          "column": 2,
          "end_line": 2,
          "end_column": 2,
          "snippet": "-"
        }
      ],
      "related": []
    }
  ]
}
```

### Accessible output mode

Netsuke supports an accessible output mode that uses static, labelled status
lines suitable for screen readers and dumb terminals.

Accessible mode is auto-enabled when:

- `TERM` is set to `dumb`
- `NO_COLOR` is set (any value)

Accessible mode can be forced on or off:

- CLI flag: `--accessible true` or `--accessible false`
- Environment variable: `NETSUKE_ACCESSIBLE=true`
- Configuration file: `accessible = true`

Explicit configuration always takes precedence over auto-detection, in either
direction (`--accessible false` disables accessible mode even when `NO_COLOR`
is set).

When accessible mode is active, each pipeline stage produces a labelled status
line on stderr:

```text
Stage 1/6: Reading manifest file
Stage 2/6: Parsing YAML document
Stage 3/6: Expanding template directives
Stage 4/6: Deserializing and rendering manifest values
Stage 5/6: Building and validating dependency graph
Stage 6/6: Synthesizing Ninja plan and executing Build
Task 1/2: cc -c src/a.c
Task 2/2: cc -c src/b.c
Build complete.
```

In standard mode, Netsuke uses `indicatif` stage summaries when progress is
enabled. During Stage 6, Netsuke parses Ninja status lines (`[current/total]`)
and emits task progress updates. When stdout is not a teletype terminal (TTY),
task progress automatically falls back to textual updates, so continuous
integration (CI) and redirected logs remain readable.

When verbose mode is enabled, Netsuke appends a completion timing summary on
stderr after a successful run:

```text
Build complete.
Stage timing summary:
- Stage 1/6: Reading manifest file: 12ms
- Stage 2/6: Parsing YAML document: 4ms
- Stage 3/6: Expanding template directives: 7ms
- Stage 4/6: Deserializing and rendering manifest values: 6ms
- Stage 5/6: Building and validating dependency graph: 3ms
- Stage 6/6: Synthesizing Ninja plan and executing Build: 18ms
Total pipeline time: 50ms
```

Without `--verbose`, timing lines are not emitted. Failed runs also suppress
timing summary output, even when verbose mode is enabled.

Progress output can be controlled via OrthoConfig layering:

- CLI flag: `--progress true` or `--progress false`
- Environment variable: `NETSUKE_PROGRESS=true|false`
- Configuration file: `progress = true|false`
- Typed alias: `--spinner-mode enabled|disabled`,
  `NETSUKE_SPINNER_MODE=enabled|disabled`, or
  `spinner_mode = "enabled" | "disabled"`

When progress is disabled, Netsuke suppresses stage and task progress output in
both standard and accessible modes. If verbose mode is enabled at the same
time, only the completion timing summary remains visible on successful runs.
When both the legacy and typed forms are present, `spinner_mode` takes
precedence.

### Theme and accessibility preferences

Netsuke resolves a CLI theme through the same layered configuration model as
its other user-facing preferences:

- CLI flag: `--theme auto|unicode|ascii`
- Environment variable: `NETSUKE_THEME=auto|unicode|ascii`
- Configuration file: `theme = "auto" | "unicode" | "ascii"`

Theme precedence is:

1. Explicit `theme`
2. Legacy `no_emoji = true`
3. `NETSUKE_NO_EMOJI` present
4. `NO_COLOR` present
5. Output mode default (`unicode` for standard output, `ascii` for
   accessible output)

`auto` keeps the mode-sensitive default. `unicode` forces Unicode symbols even
in accessible mode, while `ascii` forces ASCII-safe symbols everywhere.

Colour policy is configured separately from theme selection:

- CLI flag: `--colour-policy auto|always|never`
- Environment variable: `NETSUKE_COLOUR_POLICY=auto|always|never`
- Configuration file: `colour_policy = "auto" | "always" | "never"`

`auto` preserves the current `NO_COLOR`-aware behaviour. `always` ignores
`NO_COLOR` when deciding between standard and accessible presentation. `never`
forces `NO_COLOR` behaviour internally, which also makes `auto` theme
resolution choose ASCII-safe symbols.

Netsuke still supports the legacy no-emoji compatibility flag for users who
already rely on it:

- CLI flag: `--no-emoji true`
- Environment variable: `NETSUKE_NO_EMOJI` (any value, including empty)
- Configuration file: `no_emoji = true`

Only `--no-emoji true` acts as a hard override; `--no-emoji false` and omitting
the flag both defer to environment variable detection. `NETSUKE_NO_EMOJI` uses
presence-based semantics, so setting it to any value (including `"false"` or
`"0"`) still selects the ASCII theme unless an explicit `theme` overrides it.

In all output modes, Netsuke uses semantic text prefixes, so meaning is never
conveyed solely by colour. The active theme swaps only the glyph set:

- Unicode theme: `✖ Error:`, `⚠ Warning:`, `✔ Success:`, `ℹ Info:`,
  `⏱ Timing:`
- ASCII theme: `X Error:`, `! Warning:`, `+ Success:`, `i Info:`,
  `T Timing:`

### Default targets from configuration

The manifest's `defaults:` list remains the final fallback when a build command
does not name targets explicitly. Netsuke now supports a higher-precedence CLI
configuration layer for that same decision:

- CLI flag: `--default-target <TARGET>` (repeatable)
- Environment variable: indexed `NETSUKE_DEFAULT_TARGETS__N`
- Configuration file: `default_targets = ["hello", "test"]`

Configured `default_targets` are appended across defaults, files, environment
variables, and CLI flags. When `netsuke` or `netsuke build` is invoked without
explicit positional targets, Netsuke uses `default_targets` first and falls
back to the manifest's `defaults:` list only when the configured list is empty.

These theme-specific stage, task, completion, and timing renderings are also
guarded by snapshot regression tests so alignment and prefix stability stay
pinned for both Unicode and ASCII output.

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
  Shows `glob`, `foreach`, and path manipulation filters (`basename`,
  `with_suffix`).

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
  location info (line, column). Netsuke may add hints for common issues (e.g.,
  tabs vs spaces).

- **Schema Errors:** If the YAML structure doesn't match the expected schema
  (e.g., missing `targets`, unknown keys), errors indicate the structural
  problem.

- **Template Errors:** Jinja errors during `foreach`, `when`, or string
  rendering include context (e.g., undefined variable, invalid filter usage)
  and potentially location information.

- **IR Validation Errors:** Errors found during graph construction (e.g.,
  missing rules, duplicate outputs, circular dependencies) are reported before
  execution.

- **Build Failures:** If a command run by Ninja fails, Netsuke reports the
  failure, typically showing the command's output/error streams captured by
  Ninja.

Use the `-v` flag for more detailed error context or internal logging.

## 11\. Security Considerations

- **Command Injection:** Netsuke automatically shell-escapes variables
  interpolated into `command:` strings *unless* the `| raw` Jinja filter is
  explicitly used. Avoid `| raw` unless you fully trust the variable's content.
  File paths from `glob` or placeholders like `{{ ins }}` / `{{ outs }}` are
  quoted safely.

- **`script:` Execution:** Scripts run via the specified interpreter
  (defaulting to `sh -e`). Ensure scripts handle inputs safely.

- **Impure Functions/Filters:**
  `env()`, `glob()`, `fetch()`, `shell()`, `grep()` interact with the
  environment or network. Be mindful when using them with untrusted manifest
  parts. Future versions might offer sandboxing options.

Always review `Netsukefile` manifests, especially those from untrusted sources,
before building.

## 12\. Advanced usage

This chapter covers features aimed at users who have mastered the basics and
want to integrate Netsuke into more sophisticated workflows. These include
utility subcommands (`clean`, `graph`, `manifest`), configuration file
discovery and layering, and JSON diagnostics mode for programmatic consumption
of Netsuke's output.

### 12.1 The `clean` subcommand

The `clean` subcommand removes build artefacts that Ninja tracked in its
internal database. It delegates directly to `ninja -t clean`:

```sh
netsuke clean
```

This removes all output files declared in target `name:` fields that Ninja has
built. Only file-producing targets (those with output files declared in their
`name:` fields) are removed. Entries under the `actions` section are implicitly
phony (treated as `{ phony: true, always: false }` by default) and are
therefore ignored by `clean` since their names are not considered build
artefacts.

**Interaction with phony targets:** If a target outside the `actions` section
is declared as `phony: true`, Ninja does not track it as a file, so `clean`
ignores it as well.

**Example workflow:**

```sh
# Build the project
netsuke build

# Remove all build outputs
netsuke clean

# Rebuild from scratch
netsuke build
```

**Note:** Running `clean` in a workspace that has never been built (no
`.ninja_log` exists) will either succeed as a no-op or report that no build
state is available, depending on Ninja's behaviour.

### 12.2 The `graph` subcommand

The `graph` subcommand renders the build dependency graph in-process. It does
not invoke Ninja; the projection is computed directly from the parsed manifest
so the output is reproducible and does not depend on Ninja being installed.

By default `graph` writes a Graphviz DOT document to stdout:

```sh
netsuke graph > build.dot
dot -Tpng build.dot -o build.png
```

Use `--output <FILE>` to write the artefact to disk instead. The literal `-`
selects stdout (matching the existing `manifest -` convention):

```sh
netsuke graph --output build.dot
netsuke graph --output -        # equivalent to omitting --output
```

Relative `--output` paths resolve under the `-C/--directory` working directory
when one is configured, mirroring `build --emit`.

Use `--html` to render a self-contained HTML page instead of DOT. The page
contains a server-rendered SVG, a screen-reader-friendly textual outline of
every target and its inputs, and a `<noscript>` block restating the graph in
DOT for fully JavaScript-free viewing. No network access is required and no
external assets are referenced:

```sh
netsuke graph --html --output graph.html
```

Open `graph.html` in any modern browser. The SVG layout is hierarchical: source
files appear on the left, intermediate targets in the middle, and the
right-most column contains the leaves of the dependency graph. Order-only
dependencies are rendered as dashed edges; implicit outputs use a dotted style.

**Interpreting the DOT output:**

- Each node is labelled with the target or action name.
- Directed edges (`->`) point from prerequisites to dependants.
- Order-only dependencies are shown as dashed edges.
- Implicit outputs are shown as dotted edges.

**Example workflow:** trace a specific target's prerequisites:

```sh
netsuke graph | grep -A5 -B5 my_target
```

**Requirements:** the `graph` command requires a valid manifest. If the
manifest is syntactically invalid or contains structural errors, `graph` will
fail before writing any output.

**Note on output streams:** `--diag-json` governs diagnostic output on stderr;
the graph artefact on stdout (or in the chosen `--output` file) is unchanged
when `--diag-json` is set.

### 12.3 The `manifest` subcommand

The `manifest` subcommand writes the generated Ninja build file to a specified
location without invoking Ninja:

```sh
netsuke manifest out.ninja
```

This is useful for:

- Inspecting the exact Ninja rules and build statements Netsuke generates.
- Debugging template expansion or rule generation issues.
- Integrating with tools that consume Ninja files directly.

**Streaming to stdout:**

Passing `-` as the path streams the manifest to stdout:

```sh
netsuke manifest - | less
```

This avoids creating a temporary file and is convenient for quick inspection.

**Comparison with `--emit`:**

The build command also supports `--emit <path>`, which writes the manifest and
then invokes Ninja on it:

```sh
netsuke build --emit build.ninja
```

Use `manifest` to obtain the Ninja file *without* running the build, and
`--emit` to obtain both the file and the build execution.

### 12.4 Configuration layering

Netsuke uses a layered precedence model where configuration sources are merged,
with later sources overriding earlier ones. The precedence order (from lowest
to highest) is:

1. **Built-in defaults** — hard-coded fallback values.
2. **Configuration files** (merged from multiple scopes, lowest first):
   - **System** — `$XDG_CONFIG_DIRS/netsuke/config.toml` on Unix (defaults to
     `/etc/xdg/netsuke/config.toml`).
   - **User** — `$XDG_CONFIG_HOME/netsuke/config.toml` on Unix
     (`~/.config/netsuke/config.toml` by default), `%APPDATA%\netsuke` and
     `%LOCALAPPDATA%\netsuke` on Windows, or `~/.netsuke.toml`.
   - **Project** — `.netsuke.toml` in the current working directory (or the
     directory specified by `-C`).
3. **Environment variables** — any variable with the `NETSUKE_` prefix
   (e.g., `NETSUKE_VERBOSE`, `NETSUKE_COLOUR_POLICY`).
4. **CLI flags** — explicit command-line options (e.g., `--verbose`,
   `--colour-policy`).

Multiple configuration files are merged (not a single-winner search), and each
successive layer overrides values from earlier layers. Setting
`NETSUKE_CONFIG_PATH` bypasses automatic file discovery and uses only the
specified file. See Section 8 ("Configuration and Localization") for the full
discovery model.

**Configuration file format:**

Configuration files are TOML. Supported keys match the CLI option names:

```toml
# .netsuke.toml
verbose = true
colour_policy = "always"
spinner_mode = "enabled"
theme = "unicode"
default_targets = ["hello", "test"]
```

**Environment variables:**

Environment variables use the `NETSUKE_` prefix and convert kebab-case option
names to screaming snake case:

- `--verbose` → `NETSUKE_VERBOSE=true`
- `--colour-policy` → `NETSUKE_COLOUR_POLICY=always`
- `--spinner-mode` → `NETSUKE_SPINNER_MODE=disabled`
- Ninja executable override → `NETSUKE_NINJA=/opt/ninja/bin/ninja`

For nested fields or indexed lists, use double underscore separators:

- `NETSUKE_DEFAULT_TARGETS__0=hello`
- `NETSUKE_DEFAULT_TARGETS__1=test`

**Example layering workflow:**

Given a project configuration file:

```toml
# .netsuke.toml (project scope)
verbose = true
colour_policy = "auto"
```

Override `colour_policy` for a single invocation using an environment variable:

```sh
NETSUKE_COLOUR_POLICY=never netsuke build
```

Override both settings for a single invocation using CLI flags:

```sh
netsuke build --colour-policy always
```

**Precedence verification:**

To verify which configuration values take effect, run your command with
`--verbose`. The timing summary and diagnostic output confirm that verbose mode
is active, for example:

```sh
netsuke --verbose build
```

If the timing summary appears, the verbose setting reached the binary — useful
for confirming that a config file, environment variable, or CLI flag is being
picked up as expected.

### 12.5 JSON diagnostics

Netsuke supports a JSON diagnostics mode for programmatic consumption of error
messages and structured output. Enable it with:

```sh
netsuke --diag-json build
```

`--output-format json` is equivalent to `--diag-json` and enables JSON
diagnostics mode. `--output-format human` explicitly disables JSON diagnostics,
including when a config file sets `diag_json = true`. When both
`--output-format` and `--diag-json` are supplied, `--output-format` takes
priority.

```sh
netsuke --output-format json build
netsuke --output-format human build
```

Or via the environment variable:

```sh
NETSUKE_OUTPUT_FORMAT=json netsuke build
```

**JSON diagnostics on error:**

When a command fails, stderr contains a JSON envelope with structured error
information:

```json
{
  "schema_version": 1,
  "generator": {
    "name": "netsuke",
    "version": "0.1.0"
  },
  "diagnostics": [
    {
      "severity": "error",
      "code": "netsuke::runner::manifest_not_found",
      "message": "Manifest 'Netsukefile' not found in the current directory.",
      "help": "Ensure the manifest exists or pass `--file` with the correct path."
    }
  ]
}
```

**Interaction with stdout:**

When JSON diagnostics are enabled, stdout remains clean for machine-readable
output (e.g., from `manifest -`). All diagnostic messages go to stderr in JSON
format.

**Interaction with `--verbose`:**

Verbose logging is suppressed in JSON mode. The `--verbose` flag does not add
tracing output when `--diag-json` is active, preventing pollution of the
structured JSON stream.

**Example workflow - CI integration:**

In a continuous integration pipeline, parse Netsuke's JSON diagnostics to
report structured failures:

```sh
netsuke --diag-json build 2>diagnostics.json
if [ $? -ne 0 ]; then
  jq '
    .schema_version,
    .generator.name,
    .generator.version,
    .diagnostics[0].code,
    .diagnostics[0].message
  ' diagnostics.json
fi
```

This allows the CI system to categorize errors by code and present actionable
messages to developers.
