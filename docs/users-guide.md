# Netsuke user's guide

This guide is for people evaluating or using Netsuke v0.1.0-beta1. It covers
the first build, the manifest format, templating, command-line usage,
configuration, diagnostics, accessibility, and the current safety boundary.

Netsuke v0.1.0-beta1 is an early-adopter release. The compiler pipeline is
useful, but command names, flags, diagnostic schemas, and some manifest details
may change before 1.0. Pin the Netsuke version in automated workflows.

## Install Netsuke

Netsuke requires [Ninja](https://ninja-build.org/) on `PATH`. A source build
also requires the dated Rust nightly toolchain pinned in `rust-toolchain.toml`
because Netsuke builds with the Polonius borrow checker (`-Zpolonius=next`).

Inside a checkout both settings are inherited automatically: `rustup` installs
the pinned toolchain, and the repository's `.cargo/config.toml` supplies
`RUSTFLAGS=-Zpolonius=next`. Neither has to be passed on the command line.

Netsuke v0.1.0-beta1 is available from crates.io. Where
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) is available,
prefer it: it fetches a prebuilt release binary and avoids the toolchain
requirement below.

<!-- tested-example: guide-binstall-install -->

```sh
cargo binstall netsuke-build
```

Building from the registry instead runs outside a repository checkout, so
neither the pinned toolchain nor the Polonius flag is picked up automatically;
supply both explicitly:

<!-- tested-example: guide-crates-io-install -->

```sh
rustup toolchain install nightly-2026-06-25
RUSTFLAGS=-Zpolonius=next cargo +nightly-2026-06-25 install netsuke-build
```

Pre-built installers are available from the
[v0.1.0-beta1 GitHub release](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta1):

| Platform | Architectures                        | Packages                         |
| -------- | ------------------------------------ | -------------------------------- |
| Linux    | x86-64 (`amd64`) and Arm64 (`arm64`) | Debian (`.deb`) and RPM (`.rpm`) |
| macOS    | Intel x86-64 and Apple silicon Arm64 | Installer package (`.pkg`)       |
| Windows  | x64 and Arm64                        | Windows Installer (`.msi`)       |

Download the package for the host architecture, then install it with the
platform tool. Replace `PACKAGE` with the downloaded filename:

- Debian or Ubuntu: `sudo apt install ./PACKAGE.deb`
- Fedora, Rocky Linux, or another RPM-based distribution:
  `sudo dnf install ./PACKAGE.rpm`
- macOS: `sudo installer -pkg ./PACKAGE.pkg -target /`
- Windows: `msiexec.exe /i PACKAGE.msi`

The Linux packages install the binary under `/usr/bin`, add the `netsuke.1`
manual page and declare `ninja-build` as a dependency. The macOS packages
install the binary under `/usr/local/bin`, along with the manual page and
licence. Ninja must be installed separately when using the macOS or Windows
installer. The Windows MSI installs to `C:\Program Files\netsuke` and does not
update `PATH`.

The MSI installer supports pre-release SemVer versions such as
`0.1.0-beta1`: the pre-release suffix cannot be represented in an MSI
product version, so the installer carries the numeric release triple
(`0.1.0`) while the full version remains in the package and release names.
Because successive pre-releases share that numeric version, installing a
later pre-release MSI replaces the existing installation for that version
series rather than installing alongside it.

SHA-256 checksum files accompany standalone binaries and staged help and
licence files. Installer packages do not have checksum sidecars in
v0.1.0-beta1. Windows PowerShell help files are published beside each MSI as
sidecar artefacts rather than embedded in the installer.

Install the current source checkout with Cargo. The clone supplies both the
pinned nightly toolchain and `RUSTFLAGS=-Zpolonius=next`, so neither is given
here — unlike the registry install above, which runs outside a checkout:

<!-- tested-example: guide-source-install -->

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Complete Windows setup

The MSI does not add its installation directory to `PATH`. Add it to the
current user's persistent `PATH`, update the current PowerShell session, then
verify that the command resolves:

<!-- tested-example: guide-windows-path -->

```powershell
$netsukeDirectory = Join-Path $env:ProgramFiles 'netsuke'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$userEntries = @($userPath -split ';' | Where-Object { $_ })
if ($userEntries -notcontains $netsukeDirectory) {
    $newUserPath = (($userEntries + $netsukeDirectory) -join ';')
    [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
}
if (($env:Path -split ';') -notcontains $netsukeDirectory) {
    $env:Path = "$env:Path;$netsukeDirectory"
}
netsuke --version
```

The release publishes PowerShell help separately for each Windows architecture.
The following script downloads the matching module script, manifest, Microsoft
Assistance Markup Language (MAML) help, and about-help file. It then restores
the versioned module layout under the current user's standard PowerShell module
directory and imports the module. Set `$architecture` to match the downloaded
MSI:

<!-- tested-example: guide-windows-help-install -->

```powershell
$architecture = 'amd64' # Use 'arm64' for the Arm64 MSI.
$releaseUri = 'https://api.github.com/repos/leynos/netsuke/releases/tags/v0.1.0-beta1'
$release = Invoke-RestMethod -Uri $releaseUri

$documents = [Environment]::GetFolderPath('MyDocuments')
$editionDirectory = if ($PSVersionTable.PSEdition -eq 'Desktop') {
    'WindowsPowerShell'
} else {
    'PowerShell'
}
$moduleRoot = Join-Path $documents "$editionDirectory\Modules"
$moduleDirectory = Join-Path $moduleRoot 'Netsuke\0.1.0-beta1'
$helpDirectory = Join-Path $moduleDirectory 'en-US'
New-Item -ItemType Directory -Path $helpDirectory -Force | Out-Null

$patterns = @{
    'Netsuke.psm1' = '*Netsuke.psm1'
    'Netsuke.psd1' = '*Netsuke.psd1'
    'Netsuke-help.xml' = '*en-US-Netsuke-help.xml'
    'about_Netsuke.help.txt' = '*en-US-about_Netsuke.help.txt'
}
$localizedFiles = @('Netsuke-help.xml', 'about_Netsuke.help.txt')
foreach ($fileName in $patterns.Keys) {
    $pattern = "*windows-$architecture*$($patterns[$fileName])"
    $asset = $release.assets | Where-Object { $_.name -like $pattern } | Select-Object -First 1
    if ($null -eq $asset) {
        throw "Release asset not found: $pattern"
    }
    $destinationDirectory = if ($localizedFiles -contains $fileName) {
        $helpDirectory
    } else {
        $moduleDirectory
    }
    $destination = Join-Path $destinationDirectory $fileName
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $destination -UseBasicParsing
}

Import-Module (Join-Path $moduleDirectory 'Netsuke.psd1') -Force
```

The versioned directory is discoverable in later PowerShell sessions. After the
import, inspect the external help with:

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

`defaults` entries are literal names in v0.1.0-beta1; Jinja expressions are not
rendered in this field.

`vars` keys named `env` or `glob` are rejected because those names identify
built-in template helpers (see
[Discover files with `glob`](#discover-files-with-glob) and
[Select optional tools](#select-optional-tools)). Rather than silently
shadowing the helper, the manifest fails to parse and the error names the
offending key.

### Rules and recipes

A rule or target must provide exactly one recipe:

- `command`: one shell command.
- `script`: a multi-line POSIX shell script.
- `rule`: the name of another rule to use.

Rules may also provide `description`, text used for Ninja's progress display.

The v0.1.0-beta1 `script` implementation invokes `/bin/sh -e`; it is not
currently a portable PowerShell abstraction. Prefer `command` or
platform-selected actions when a manifest must work on Windows.

### Targets, inputs, and dependencies

A target supports these fields:

- `name`: one output path or a list of output paths.
- `rule`, `command`, or `script`: exactly one recipe.
- `sources`: explicit inputs. They affect freshness and become `{{ ins }}`.
- `deps`: implicit dependencies. They affect freshness but do not become
  recipe arguments. Declare them on each target; reusable rules reject `deps`.
  The planned rule-level `deps_from` contract is not implemented in
  v0.1.0-beta1.
- `order_only_deps`: ordering dependencies. Their changes do not rebuild the
  dependent target.
- `vars`: values that override global variables for this target. The `env`
  and `glob` restriction above applies here too.
- `phony`: marks a logical target that does not represent a file.
- `always`: forces the recipe to run whenever the target is requested.

`name`, `sources`, `deps`, and `order_only_deps` accept either one string or a
list of strings.

Netsuke quotes paths inserted through `{{ ins }}` and `{{ outs }}`. Other Jinja
values render as ordinary command text and are not automatically shell-quoted.
The `shell_escape` filter described in older drafts is not implemented in
v0.1.0-beta1.

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

### Discover files with `glob`

`glob(pattern)` expands a shell-style pattern to the sorted list of matching
files while Netsuke loads the manifest, so the results become part of the
static build graph. It pairs naturally with `foreach` to generate one target
per matched file; a target list such as `foreach: glob('src/*.c')` produces one
expansion per matching source.

Matching is case-sensitive. `*` and `?` do not cross directory separators; use
`**` to descend into subdirectories. Directories are excluded, so only files
are returned. The [quick-start guide](quickstart.md) shows a complete runnable
example.

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

On Windows, a name without an extension is matched against the effective
`PATHEXT`, the same list the shell uses — so `which('cargo')` finds
`cargo.exe` provided `.exe` is among those entries. A custom `PATHEXT` may
legitimately omit it, in which case it is not a candidate.

`PATHEXT` falls back to the built-in list only when it is unset or when no
entry survives normalization — that is, every entry is empty or whitespace.
Any other value is used as given, however unusual. The built-in list, in
order:

`.com`, `.exe`, `.bat`, `.cmd`, `.vbs`, `.vbe`, `.js`, `.jse`, `.wsf`,
`.wsh`, `.msc`

The fallback exists because an empty effective list would match nothing and
report every command missing. Entries are matched case-insensitively and
tried in the order the list gives them. A name that already carries an
extension is used as written.

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

The `env(name)` function reads one required environment variable. v0.1.0-beta1
does not accept a default argument; an absent or non-Unicode value is an error.

### Inject the environment reader for tests

`env()` does not read `std::env::var` directly. Manifest parsing goes through
an injectable `EnvReader` seam, so callers that need deterministic `env()`
results — test suites, and any program driving Netsuke's unstable Rust API —
can supply their own reader instead of mutating the process environment.

- `netsuke::manifest::from_str` parses a manifest using the live process
  environment.
- `netsuke::manifest::from_str_with_env` takes an explicit `EnvReader`,
  letting the caller control every value `env()` returns.
- `netsuke::manifest::process_env_reader` builds the process-backed reader
  that `from_str` uses by default.

A missing variable still fails the parse with a Jinja "undefined" error, and a
non-Unicode value still fails with an "invalid operation" error; only the
source of the values changes.

<!-- tested-example: guide-env-reader-snippet -->

```rust
use netsuke::manifest::{EnvReader, from_str_with_env};
use std::sync::Arc;

let reader: EnvReader = Arc::new(|_| Ok(String::from("release")));
let yaml = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: \"{{ env('PROFILE') }}\"\n",
    "    command: echo hi\n",
);
let manifest = from_str_with_env(yaml, &reader).expect("parse");
assert!(format!("{:?}", manifest.targets[0].name).contains("release"));
```

This snippet mirrors the executable doctest on `from_str_with_env` in the API
documentation, rather than the YAML-only examples elsewhere in this guide.

### Drive Ninja with an explicit environment

Netsuke is a build tool, not a library: the Netsukefile format and the graph
export are the only surfaces it commits to, and every Rust API named in this
section is private in intent and unstable, liable to change or disappear in
any beta release. It is documented here for the benefit of anyone who calls
it anyway, with that caveat understood.

A program calling Netsuke's Rust API can invoke Ninja without touching
its own process environment. `netsuke::runner::CommandEnv` carries child
environment overrides as data — `inherit()` changes nothing, `with_var` and
`with_path` set variables for the spawned command only — and the explicit
request forms `run_ninja_with` and `run_ninja_tool_with` accept a request
naming the program, build file, targets or tool, and that environment. The
convenience wrappers `run_ninja` and `run_ninja_tool` behave identically
with an inherited environment. Overrides are additive: variables not named
are inherited from the calling process, and the injected `PATH` governs
what commands Ninja launches will see. Relative program names remain valid
and resolve through that child `PATH`; supply an absolute or otherwise
resolved `program` only when executable selection must stay isolated from
the injected `PATH`.

The request itself is a named type: `netsuke::runner::NinjaBuildRequest` for a
build and `netsuke::runner::NinjaToolRequest` for `ninja -t <tool>`. Both
borrow their fields, so one `CommandEnv` and one `Cli` can serve several
invocations. The [v0.1.0 migration guide](v0-1-0-migration-guide.md)
summarizes these additions and confirms the wrappers are unchanged.

<!-- tested-example: guide-ninja-request-snippet -->

```rust
use netsuke::cli::Cli;
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaToolRequest, run_ninja_tool_with,
    run_ninja_with,
};
use std::path::Path;

let cli = Cli::default();
let targets = BuildTargets::default();
// `with_path` replaces the child's `PATH` outright, so compose the whole
// value first. The calling process is never modified.
let path = std::env::join_paths(["/opt/toolchain/bin", "/usr/bin"])
    .expect("separator-free entries always join");
let env = CommandEnv::inherit()
    .with_var("NINJA_STATUS", "[%f/%t] ")
    .with_path(&path);

let build = NinjaBuildRequest {
    program: Path::new("/usr/bin/ninja"),
    cli: &cli,
    build_file: Path::new("build.ninja"),
    targets: &targets,
    env: &env,
};
let clean = NinjaToolRequest {
    program: Path::new("/usr/bin/ninja"),
    cli: &cli,
    build_file: Path::new("build.ninja"),
    tool: "clean",
    env: &env,
};

if std::env::var_os("NETSUKE_GUIDE_RUN").is_some() {
    run_ninja_with(&build).expect("run ninja");
    run_ninja_tool_with(&clean).expect("run ninja -t clean");
}
```

These types are additive: `run_ninja` and `run_ninja_tool` keep their existing
signatures and behaviour, so an existing caller needs no change. Each release
records such additions in [`CHANGELOG.md`](../CHANGELOG.md), which is where
Netsuke signposts Rust API changes — with no stability promise attached to
them ahead of 1.0.

## Use the template standard library

Netsuke registers focused path, collection, command, network, and time helpers
alongside MiniJinja's built-ins. The library covers path and collection
filters, file tests, clocks and durations, host commands, executable discovery,
environment variables, globbing, and policy-controlled network retrieval.

See the [template standard-library guide](stdlib-yaml-and-jinja-guide.md) for
every helper's signature, defaults, purity, platform caveats, and executable
examples. Host-observing helpers belong only in trusted manifests: Netsuke
bounds command and network output, but does not sandbox template evaluation.

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
- `generate`: write Ninja without invoking it. Outside JSON mode, the generated
  Ninja manifest is the only content written to stdout; use `--output <FILE>`
  to write it to a file instead. In JSON mode (`--json`) the manifest is
  carried in the result document's `result.content` field instead.

Running `netsuke` without a subcommand is the same as `netsuke build` with no
explicit targets. A bare target such as `netsuke hello` is not accepted; use
`netsuke build hello`.

Important global options include:

- `-f, --file <FILE>`
- `-C, --directory <DIR>`
- `--config <FILE>`
- `-j, --jobs <N>` (accepts 1 to 64)
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

### Choose a language with `--locale`

Netsuke's help text, validation errors, progress labels, and runtime
diagnostics are translated. The locale is chosen by the first source that
yields a valid BCP 47 tag, and which sources are available depends on when the
message is rendered.

Help, usage, and command-line validation errors are produced before any
configuration file is read, so they use the `--locale` flag, then
`NETSUKE_LOCALE`, then the system default, then `en-US`. Diagnostics, progress,
and status output are rendered after the configuration merge, so they consult
the configuration file's `locale` setting as well, between `NETSUKE_LOCALE` and
the system default.

System values are normalized first, so `en_GB.UTF-8` is understood as `en-GB`.

Table 1: Locales Netsuke ships

| Tag      | Language                 | Tag       | Language              |
| -------- | ------------------------ | --------- | --------------------- |
| `ar`     | Arabic                   | `it`      | Italian               |
| `cs`     | Czech                    | `ja`      | Japanese              |
| `cy`     | Welsh                    | `ko`      | Korean                |
| `da`     | Danish                   | `nb`      | Norwegian Bokmål      |
| `de`     | German                   | `nl`      | Dutch                 |
| `el`     | Greek                    | `pl`      | Polish                |
| `en-GB`  | English (United Kingdom) | `pt-BR`   | Portuguese (Brazil)   |
| `en-US`  | English (United States)  | `pt-PT`   | Portuguese (Portugal) |
| `es-419` | Spanish (Latin America)  | `ro`      | Romanian              |
| `es-ES`  | Spanish (Spain)          | `ru`      | Russian               |
| `fa`     | Persian                  | `sv`      | Swedish               |
| `fi`     | Finnish                  | `th`      | Thai                  |
| `fr`     | French                   | `tr`      | Turkish               |
| `gd`     | Scottish Gaelic          | `uk`      | Ukrainian             |
| `he`     | Hebrew                   | `vi`      | Vietnamese            |
| `hi`     | Hindi                    | `zh-Hans` | Chinese (Simplified)  |
| `hu`     | Hungarian                | `zh-Hant` | Chinese (Traditional) |
| `id`     | Indonesian               |           |                       |

`en-US` is the source locale. Any message a translation has not yet covered
falls back to the English text rather than disappearing.

A requested tag resolves by these rules, in order:

1. The exact tag, if a catalogue carries it.
2. A script or region rule for that language. Bare `es` and `es-ES` use
   `es-ES`, and every other Spanish region uses `es-419`; bare `pt` and every
   Portuguese region except Brazil use `pt-PT`; Chinese resolves by script, with
   `zh-CN`, `zh-SG`, and `zh-MY` taking Simplified and `zh-TW`, `zh-HK`, and
   `zh-MO` taking Traditional; English outside the United States uses `en-GB`;
   and `no` resolves to `nb`.
3. The only catalogue for that language, so `fr-CA` uses `fr` and `de-AT`
   uses `de`.
4. `en-US`, for anything still unmatched.

Regional and script variants that differ in substance are never merged: asking
for `pt-BR` never yields European Portuguese, and asking for `zh-TW` never
yields Simplified Chinese.

Manual pages and PowerShell help shipped in releases are generated in `en-US`
only. Translated copy reaches users through the running binary, which embeds
every catalogue.

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

System and user configuration are discovered from platform conventions rather
than two separately named Netsuke layers. On Unix this means the XDG base
directories and the home directory; on Windows it means the application-data
directories, such as `%APPDATA%\netsuke\config.toml`. Their relative order
follows those platform conventions.

An explicit selector bypasses automatic discovery. Selectors are checked in
this order:

1. `--config <PATH>`
2. `NETSUKE_CONFIG=<PATH>`

An explicit file that is missing or invalid causes an error; Netsuke does not
fall back to discovery.

### Diagnose configuration selection

Pass `--verbose` to see how Netsuke selected its configuration. Structured
events report whether `--config`, `NETSUKE_CONFIG`, or automatic discovery won,
whether a path was present, and which environment lookups were attempted.
Events then identify whether Netsuke uses an explicit file or discovered layers.

If an explicit file cannot be loaded, the warning records `failure_kind` as
`Missing` or `LoadError`. Path fields are bounded to `path_hash` and
`path_file_name`; full paths and formatted parser errors are not tracing
fields. The file name is visible, and the unkeyed hash is only a correlation
identifier: it does not confidentially conceal a guessable path.

Configuration tracing is disabled in JSON mode, including when `json = true`
comes from a configuration file. This keeps stderr empty for successful JSON
commands and reserves it for the single diagnostic document on failure.

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
- `NETSUKE_LOCALE=en-US`
- `NETSUKE_DEFAULT_TARGETS__0=hello.txt`
- `NETSUKE_NINJA=/opt/ninja/bin/ninja`
- `NETSUKE_WHICH_WORKSPACE=0`

`NETSUKE_LOCALE` selects the interface language; see
[Choose a language with `--locale`](#choose-a-language-with---locale) for how
it combines with the flag and the system default.

`NETSUKE_NINJA` overrides the Ninja executable used by `build` and `clean`.
Leave it unset to use `ninja` from `PATH`, or set another executable name or an
absolute path. Empty and non-UTF-8 values fall back to the default.

`NETSUKE_WHICH_WORKSPACE` switches off the `which()` workspace-tree fallback
search that runs when a command is not found on `PATH`. Set it to `0`,
`false`, or `off` (case-insensitively) to disable the fallback; any other
value, or leaving it unset, keeps the fallback enabled. A non-Unicode value
also disables the fallback and is treated as an explicit opt-out, emitting a
warning.

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

In JSON mode (`--json`), a successful command writes exactly one versioned
result document to stdout, with generated content embedded in `result.content`.
On failure, stdout is left empty and the single versioned diagnostic document
is written to stderr instead. Diagnostics therefore stay on stderr in both
modes, so a caller parsing errors reads stderr regardless of `--json`.

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
progress falls back to text, so logs remain readable.

Netsuke uses semantic text labels as well as glyphs; meaning is not conveyed by
colour alone. Emoji policy values are:

- `always`: Unicode status symbols.
- `never`: ASCII-safe prefixes.
- `auto`: Unicode in standard output and ASCII in accessible output.

The colour policy is separate. Colour rendering is not implemented in
v0.1.0-beta1, so `color` currently affects mode selection but does not add
coloured terminal text.

Verbose mode adds per-stage timing after a successful command. Failed commands
do not print a timing summary.

### JSON output

Use `--json` when a caller needs machine-readable command output. Every
invocation emits exactly one versioned JSON document: a result document on
success, written to stdout, or a diagnostic document on failure, written to
stderr while stdout stays empty. Generated stdout artefacts, such as the Ninja
text from `generate`, are carried inside the successful result document rather
than written as unstructured text.

JSON selection follows the normal configuration precedence: `--json`, then
`NETSUKE_JSON`, then `json = true|false` in a configuration file. Set
`NETSUKE_JSON` to `true` or `1` to enable JSON output, or to `false` or `0` to
disable it. Any other value, including malformed or non-Boolean text, produces
a configuration validation error rather than silently falling back. An explicit
`--json` flag takes precedence and bypasses parsing a lower-priority
environment value. For example, `NETSUKE_JSON=1 netsuke …` enables JSON output.

The following command deliberately selects a missing manifest:

<!-- tested-example: guide-json-command -->

```sh
netsuke --json --no-input --file missing.yml build
```

The exact localized message can vary, but the diagnostic document written to
stderr has this shape:

<!-- tested-example: guide-json-output -->

```json
{
  "schema_version": 1,
  "generator": {
    "name": "netsuke",
    "version": "0.1.0-beta1"
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

The common envelope fields are:

- `schema_version`: JSON envelope version.
- `generator`: Netsuke name and version.

Exactly one outcome branch is present:

- `result`: present only on success.
  - `command`: the command that completed, such as `build`, `clean`, `generate`,
    or `graph`.
  - `content`: the generated text artefact when the command would otherwise
    write it to standard output. In particular, `generate` embeds its Ninja
    manifest here when `--output` is not supplied. This field is `null` when
    the command produces no text artefact or writes it to a file.
- `diagnostics`: present only on failure and contains ordered diagnostic
  objects.
  - `message`, `code`, `severity`, `help`, and `url`: primary details.
  - `causes`: ordered error-cause chain.
  - `source`, `primary_span`, and `labels`: optional source locations.
  - `related`: nested diagnostics using the same shape.

**Triage:** Treat schema version `1` as pre-stable for v0.1.0-beta1 and check
`schema_version` before parsing other fields.

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
- `script` uses `/bin/sh -e` in v0.1.0-beta1.
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
