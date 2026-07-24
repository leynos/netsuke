# Netsuke user's guide

This guide is for people evaluating or using Netsuke v0.1.0. It covers the
first build, the manifest format, templating, command-line usage,
configuration, diagnostics, accessibility, and the current safety boundary.

Netsuke v0.1.0 is an early-adopter release. The compiler pipeline is useful,
but command names, flags, diagnostic schemas, and some manifest details may
change before 1.0. Pin the Netsuke version in automated workflows.

## Install Netsuke

Netsuke requires [Ninja](https://ninja-build.org/) on `PATH`. A source build
also requires Rust 1.89 or later.

Until v0.1.0 is published, the current checkout can be installed with Cargo:

<!-- tested-example: guide-source-install -->

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

Release archives contain platform-specific packages and help artefacts. Unix
archives include a `netsuke.1` manual page. Windows archives include PowerShell
external help:

<!-- tested-example: guide-windows-help -->

```powershell
Get-Help Netsuke -Full
```

## Run the first build

Create an empty project directory and add a file named `Netsukefile` with the
following complete manifest:

<!-- tested-example: guide-first-build-manifest -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Run Netsuke without a subcommand to build the manifest's `defaults`, then read
the generated file:

<!-- tested-example: guide-first-build-commands -->

```sh
netsuke
cat hello.txt
```

The second command prints `Hello from Netsuke!`.

If Netsuke cannot find `Netsukefile`, it reports the missing file and suggests
`--file`. A different path can be selected with
`netsuke --file path/to/manifest.yml build`.

The [quick-start guide](quickstart.md) provides a longer walkthrough.

## Understand the build model

Netsuke is a build-system compiler. A build moves through six stages:

1. Read the manifest.
2. Parse YAML 1.2.
3. Expand manifest-time `foreach` and `when` expressions.
4. Deserialize the typed manifest and render string fields.
5. Build and validate a static intermediate representation (IR).
6. Generate Ninja and, for `build` or `clean`, run Ninja.

All manifest-time decisions finish before Ninja starts. The generated graph is
therefore static and inspectable.

A `Netsukefile` is executable build configuration, not passive data. Commands,
scripts, and impure template helpers can access the host. Review an untrusted
manifest with the same care as an untrusted `Makefile`.

## Author a manifest

`Netsukefile` is a YAML mapping. Unknown fields and duplicate mapping keys are
errors. `netsuke_version` and `targets` are required; all other top-level
collections are optional.

The following complete example shows every top-level section:

<!-- tested-example: guide-complete-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  greeting: Hello

macros:
  - signature: "message(name)"
    body: |
      {{ greeting }}, {{ name }}!

rules:
  - name: write_message
    command: "echo '{{ message('Netsuke') }}' > {{ outs }}"
    description: "Writing greeting"

actions:
  - name: greet
    command: "echo '{{ message('builder') }}'"

targets:
  - name: greeting.txt
    rule: write_message

defaults:
  - greeting.txt
```

The top-level fields are:

- `netsuke_version`: required semantic version for the manifest schema.
- `vars`: global strings, numbers, booleans, or lists available to Jinja.
- `macros`: named Jinja macro definitions registered before other fields
  render.
- `rules`: reusable recipes referenced by targets or actions.
- `actions`: implicitly phony operations, such as `test` or `lint`.
- `targets`: required list of file-producing or logical build nodes. An empty
  list is valid for an action-only manifest.
- `defaults`: target or action names used when `build` receives no explicit
  targets.

`defaults` entries are literal names in v0.1.0; Jinja expressions are not
rendered in this field.

### Rules and recipes

A rule or target must provide exactly one recipe:

- `command`: one shell command.
- `script`: a multi-line POSIX shell script.
- `rule`: the name of another rule to use.

Rules may also provide `description`, text used for Ninja's progress display.

The v0.1.0 `script` implementation invokes `/bin/sh -e`; it is not currently a
portable PowerShell abstraction. Prefer `command` or platform-selected actions
when a manifest must work on Windows.

### Targets, inputs, and dependencies

A target supports these fields:

- `name`: one output path or a list of output paths.
- `rule`, `command`, or `script`: exactly one recipe.
- `sources`: explicit inputs. They affect freshness and become `{{ ins }}`.
- `deps`: implicit dependencies. They affect freshness but do not become
  recipe arguments. Declare them on each target; reusable rules reject `deps`.
  The planned rule-level `deps_from` contract is not implemented in v0.1.0.
- `order_only_deps`: ordering dependencies. Their changes do not rebuild the
  dependent target.
- `vars`: values that override global variables for this target.
- `phony`: marks a logical target that does not represent a file.
- `always`: forces the recipe to run whenever the target is requested.

`name`, `sources`, `deps`, and `order_only_deps` accept either one string or a
list of strings.

Netsuke quotes paths inserted through `{{ ins }}` and `{{ outs }}`. Other Jinja
values render as ordinary command text and are not automatically shell-quoted.
The `shell_escape` filter described in older drafts is not implemented in
v0.1.0.

Cycle detection follows `sources` and `deps`. Order-only dependencies enforce
ordering but do not participate in cycle detection.

## Use Jinja safely

Jinja expressions are allowed in renderable string fields, including variables,
target fields, and rule recipes. Structural Jinja blocks cannot reshape the
YAML document. Use the dedicated `foreach` and `when` keys for manifest-time
expansion.

### Generate targets with `foreach` and `when`

The next complete manifest creates two targets and excludes the disabled item:

<!-- tested-example: guide-foreach-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  reports:
    - daily
    - weekly
    - disabled

targets:
  - foreach: reports
    when: item != 'disabled'
    name: "{{ item }}.txt"
    command: "echo {{ index }} > {{ outs }}"

defaults:
  - daily.txt
  - weekly.txt
```

Each expansion receives `item` and a zero-based `index`. Target-local variables
take precedence over global variables, and iteration values take precedence
over both.

`when` is evaluated while Netsuke loads the manifest. It does not create a
runtime branch. Runtime decisions belong in a command or script.

Top-level actions support the same `foreach` and `when` keys.

### Define reusable macros

Macros return rendered text and can accept default arguments:

<!-- tested-example: guide-macro-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  greeting: Hello

macros:
  - signature: "say(name, punctuation='!')"
    body: |
      {{ greeting }}, {{ name }}{{ punctuation }}

targets:
  - name: greeting.txt
    command: "echo '{{ say('Netsuke') }}' > {{ outs }}"

defaults:
  - greeting.txt
```

### Select optional tools

`which(name, **kwargs)` returns an executable path and fails when the command
is absent. The same helper is also available as a filter.

`command_available(name, **kwargs)` returns a boolean and is better for
complementary branches:

<!-- tested-example: guide-command-available-manifest -->

```yaml
netsuke_version: "1.0.0"

actions:
  - name: test-fast
    command: "cargo nextest run"
    when: command_available("cargo-nextest")

  - name: test-fast
    command: "cargo test"
    when: not command_available("cargo-nextest")

targets: []

defaults:
  - test-fast
```

Both helpers accept:

- `all=true`: return all `which` matches. It does not change the boolean result
  from `command_available`.
- `canonical=true`: canonicalize matching paths.
- `fresh=true`: bypass the resolver cache for this lookup.
- `cwd_mode="auto"|"always"|"never"`: control bounded project-directory
  fallback searching.

The `env(name)` function reads one required environment variable. v0.1.0 does
not accept a default argument; an absent or non-Unicode value is an error.

## Use the template standard library

Netsuke registers focused path, collection, command, network, and time helpers
alongside MiniJinja's built-ins.

### Path filters

The path filters are:

- `basename`
- `dirname`
- `with_suffix(suffix[, count[, separator]])`
- `relative_to(root)`
- `realpath`
- `expanduser`
- `contents([encoding])`
- `size`
- `linecount`
- `hash([algorithm])`
- `digest([length[, algorithm]])`

`with_suffix` defaults to replacing one dot-separated suffix. `contents`
defaults to UTF-8, `hash` defaults to SHA-256, and `digest` defaults to the
first eight characters of a SHA-256 digest. Hashing supports SHA-256 and
SHA-512. MD5 and SHA-1 require the `legacy-digests` Cargo feature.

### Collection filters

Netsuke adds `uniq`, `flatten`, and `group_by(attribute)`. MiniJinja also
provides general filters such as `join`, `map`, `select`, and `sort`.

This complete manifest exercises string-only helpers without depending on
external files:

<!-- tested-example: guide-stdlib-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  names:
    - alpha
    - alpha
    - beta

targets:
  - name: "{{ 'report.tmp' | with_suffix('.txt') }}"
    command: "echo {{ names | uniq | join(',') }} > {{ outs }}"

defaults:
  - report.txt
```

### File tests

Jinja `is` expressions can test `file`, `dir`, `symlink`, `pipe`,
`block_device`, `char_device`, and `device`. Filesystem tests inspect the host,
so results depend on the current workspace and platform.

### Time helpers

`now(offset=...)` returns the current timestamp, optionally at an offset such as
`"+01:00"`. `timedelta(...)` constructs a duration from keyword components:
weeks, days, hours, minutes, seconds, milliseconds, microseconds, and
nanoseconds. These helpers read the clock or perform duration arithmetic; they
do not schedule work.

### Impure helpers

The following helpers can observe or modify the outside world:

- `fetch(url, cache=false)` retrieves a URL. HTTPS is the only allowed scheme
  by default.
- `value | shell(command, options)` sends a value to a host shell command.
- `value | grep(pattern, flags, options)` filters lines through `grep`.
- `now(offset=...)` reads the clock.
- File-reading and path-canonicalization filters inspect the filesystem.

`fetch`, `shell`, and `grep` enforce bounded output. These internal limits are
not currently user-configurable from `Netsukefile`.

Use impure helpers only in trusted manifests. Netsuke does not sandbox them.

## Use the command-line interface

The top-level command shape is:

<!-- tested-example: guide-cli-usage -->

```plaintext
netsuke [OPTIONS] [COMMAND]
netsuke [OPTIONS] build [TARGETS]...
```

Global options must appear before the subcommand. For example,
`netsuke --color always build` is valid; `netsuke build --color always` is not.

The commands are:

- `build [TARGETS]...`: generate Ninja and build the named targets. With no
  targets, use configured defaults and then manifest defaults.
- `clean`: generate a temporary Ninja file and run `ninja -t clean`.
- `graph`: render the build graph as DOT or self-contained HTML without
  invoking Ninja.
- `generate`: write Ninja without invoking it. By default, the generated Ninja
  manifest is the only content written to stdout; use `--output <FILE>` to
  write it to a file instead.

Running `netsuke` without a subcommand is the same as `netsuke build` with no
explicit targets. A bare target such as `netsuke hello` is not accepted; use
`netsuke build hello`.

Important global options include:

- `-f, --file <FILE>`
- `-C, --directory <DIR>`
- `--config <FILE>`
- `-j, --jobs <1..64>`
- `-v, --verbose`
- `--locale <LOCALE>`
- `--no-input`
- `--json`
- `--color <auto|always|never>`
- `--emoji <auto|always|never>`
- `--progress <auto|always|never>`
- `--accessibility <auto|on|off>`
- `--default-target <TARGET>`

Run `netsuke --help` or `netsuke <command> --help` for the complete current
surface.

### Anchor a project with `--directory`

`--directory` changes manifest lookup, project configuration discovery and
relative output paths:

<!-- tested-example: guide-project-anchor -->

```sh
netsuke --directory /path/to/project build
```

An explicit `--config` path remains relative to the shell's original working
directory.

### Generate and inspect artefacts

These commands cover the non-default utility workflows:

<!-- tested-example: guide-utility-commands -->

```sh
netsuke clean
netsuke graph --output build.dot
netsuke graph --html --output graph.html
netsuke generate
netsuke generate --output build.ninja
```

`graph` is rendered in-process and does not require Ninja. DOT goes to stdout
unless `--output` is supplied. HTML output contains a server-rendered SVG, a
textual outline and a `<noscript>` DOT representation.

`generate` writes Ninja without running it. With no `--output`, stdout contains
only the generated Ninja manifest. With `--output <FILE>`, Netsuke writes the
manifest to that file and leaves stdout empty.

`clean` removes file outputs tracked by Ninja. Phony targets and actions do not
represent files and are not removed.

## Configure Netsuke

Configuration precedence, from lowest to highest, is:

1. Built-in defaults.
2. System configuration.
3. User configuration.
4. Project `.netsuke.toml`.
5. `NETSUKE_` environment variables.
6. Explicit command-line options.

An explicit selector bypasses automatic discovery. Selectors are checked in
this order:

1. `--config <PATH>`
2. `NETSUKE_CONFIG=<PATH>`

An explicit file that is missing or invalid causes an error; Netsuke does not
fall back to discovery.

The annotated [sample configuration](sample-netsuke.toml) lists every key. A
small project configuration looks like this:

<!-- tested-example: guide-project-config -->

```toml
jobs = 4
verbose = true
locale = "en-US"
json = false
no_input = true
color = "never"
emoji = "never"
progress = "never"
accessibility = "on"
default_targets = ["hello.txt"]
```

Common environment equivalents include:

- `NETSUKE_JOBS=4`
- `NETSUKE_VERBOSE=true`
- `NETSUKE_JSON=false`
- `NETSUKE_NO_INPUT=true`
- `NETSUKE_COLOR=never`
- `NETSUKE_EMOJI=never`
- `NETSUKE_PROGRESS=never`
- `NETSUKE_ACCESSIBILITY=on`
- `NETSUKE_DEFAULT_TARGETS__0=hello.txt`
- `NETSUKE_NINJA=/opt/ninja/bin/ninja`

`NETSUKE_NINJA` overrides the Ninja executable used by `build` and `clean`.
Leave it unset to use `ninja` from `PATH`, or set another executable name or an
absolute path. Empty and non-UTF-8 values fall back to the default.

The CLI and configuration use the same policy values. `auto` follows terminal
and environment detection. `always` or `never` makes colour, emoji, or progress
behaviour explicit. Accessibility uses `on` and `off` for its explicit values.

Netsuke has no interactive mode. It never prompts, and `no_input = false` is
rejected. Pass root `--no-input` in automation to state that requirement
explicitly and make the invocation self-documenting.

## Control output and accessibility

Netsuke separates machine-consumable output from status information:

- stdout contains generated artefacts and subprocess stdout.
- stderr contains status, progress, timing, and diagnostics.

This makes redirection predictable:

<!-- tested-example: guide-output-streams -->

```sh
netsuke graph > build.dot
netsuke --progress never build
netsuke generate > build.ninja
```

### Accessible output

Accessible mode uses static, labelled status lines instead of animated
progress. It is enabled automatically when `TERM=dumb` or `NO_COLOR` is
present. Select it explicitly with `--accessibility on`, or force standard
output with `--accessibility off`.

A typical accessible build reports:

<!-- tested-example: guide-accessible-output -->

```plaintext
Stage 1/6: Reading manifest file
Stage 2/6: Parsing YAML document
Stage 3/6: Expanding template directives
Stage 4/6: Deserializing and rendering manifest values
Stage 5/6: Building and validating dependency graph
Stage 6/6: Synthesizing Ninja plan and executing Build
Build complete.
```

When stdout is redirected or connected to Continuous Integration (CI), task
progress falls back to text so logs remain readable.

Netsuke uses semantic text labels as well as glyphs; meaning is not conveyed by
colour alone. Emoji policy values are:

- `always`: Unicode status symbols.
- `never`: ASCII-safe prefixes.
- `auto`: Unicode in standard output and ASCII in accessible output.

The colour policy is separate. Colour rendering is not implemented in v0.1.0, so
`color` currently affects mode selection but does not add coloured terminal
text.

Verbose mode adds per-stage timing after a successful command. Failed commands
do not print a timing summary.

### JSON output

Use `--json` when a caller needs machine-readable command output. Every
invocation emits exactly one versioned JSON document: a result document on
success or a diagnostic document on failure. Generated stdout artefacts, such
as the Ninja text from `generate`, are carried inside the successful result
document rather than written as unstructured text.

The following command deliberately selects a missing manifest:

<!-- tested-example: guide-json-command -->

```sh
netsuke --json --no-input --file missing.yml build
```

The exact localized message can vary, but the envelope has this shape:

<!-- tested-example: guide-json-output -->

```json
{
  "schema_version": 1,
  "generator": {
    "name": "netsuke",
    "version": "0.1.0"
  },
  "diagnostics": [
    {
      "message": "Manifest 'missing.yml' not found in the current directory.",
      "code": "netsuke::runner::manifest_not_found",
      "severity": "error",
      "help": "Ensure the manifest exists or pass `--file` with the correct path.",
      "url": null,
      "causes": [],
      "source": null,
      "primary_span": null,
      "labels": [],
      "related": []
    }
  ]
}
```

The schema fields are:

- `schema_version`: diagnostic envelope version.
- `generator`: Netsuke name and version.
- `diagnostics`: ordered diagnostic objects.
- `message`, `code`, `severity`, `help`, and `url`: primary details.
- `causes`: ordered error-cause chain.
- `source`, `primary_span`, and `labels`: optional source locations.
- `related`: nested diagnostics using the same shape.

Treat schema version `1` as pre-stable for v0.1.0 and check `schema_version`
before parsing other fields.

## Configure network access

`fetch()` allows HTTPS by default. Network policy can be tightened or extended
with global flags or their configuration equivalents:

- `--fetch-allow-scheme <SCHEME>`
- `--fetch-allow-host <HOST>`
- `--fetch-block-host <HOST>`
- `--fetch-default-deny`

Host patterns may contain wildcards such as `*.example.com`. A block rule wins
over an allow rule. `--fetch-default-deny` permits only explicitly allowed
hosts.

Avoid placing secrets in URLs. Netsuke logs hosts and cache keys rather than
complete URLs, but downloaded content and commands still run within the host
trust boundary.

## Interpret failures

Netsuke reports failures at the earliest stage that can identify them:

- YAML failures include locations when the parser provides them.
- Schema failures identify unknown or malformed fields.
- Jinja failures identify missing variables or invalid helpers.
- IR failures report missing rules, duplicate outputs, and cycles before Ninja
  starts.
- Ninja failures retain the subprocess exit status and output.

Human diagnostics include remediation hints where one is available. JSON mode
exposes the same information as fields.

The `--verbose` flag enables diagnostic tracing and successful timing
summaries. It is suppressed in JSON mode so stderr remains parseable.

## Review the safety boundary

Netsuke reduces some common quoting mistakes, but it is not a sandbox:

- `{{ ins }}` and `{{ outs }}` are quoted as path arguments.
- Arbitrary Jinja values in `command` and `script` are not automatically
  shell-quoted.
- `script` uses `/bin/sh -e` in v0.1.0.
- `shell`, `grep`, `fetch`, filesystem helpers, and ordinary recipes interact
  with the host.
- `raw` template output and handwritten shell fragments remain the manifest
  author's responsibility.
- Literal shell dollar expressions currently require Ninja-aware escaping,
  such as `$$PATH`.

Do not run an untrusted `Netsukefile`. Prefer explicit inputs, avoid embedding
secrets in commands or URLs, and pin dependencies used by recipes.

## Explore complete examples

The repository contains complete manifests for several domains:

- [`examples/basic_c.yml`](../examples/basic_c.yml): rules and compilation.
- [`examples/website.yml`](../examples/website.yml): `foreach` and a combined
  landing page.
- [`examples/photo_edit.yml`](../examples/photo_edit.yml): generated image
  targets and an action.
- [`examples/visual_design.yml`](../examples/visual_design.yml): SVG
  rasterization.
- [`examples/writing.yml`](../examples/writing.yml): ordered document inputs.
- [`examples/hello-world/`](../examples/hello-world/): a minimal runnable
  project.

These manifests are compiled by the documentation-example test suite. External
programs such as C compilers, Pandoc, Darktable, and Inkscape are still
required to execute their recipes.

## Find more information

- [Quick-start guide](quickstart.md): a five-minute tutorial.
- [Sample configuration](sample-netsuke.toml): annotated configuration keys.
- [Design document](netsuke-design.md): architecture and rationale.
- [Roadmap](roadmap.md): current completion and planned work.
- [Translator guide](translators-guide.md): localization contributions.
