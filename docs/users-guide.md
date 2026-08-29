# Netsuke user's guide

This guide is for people evaluating or using Netsuke v0.1.0-beta2. It covers
the first build, the manifest format, templating, command-line usage,
configuration, diagnostics, accessibility, and the current safety boundary.

Netsuke v0.1.0-beta2 is an early-adopter release. The compiler pipeline is
useful, but command names, flags, diagnostic schemas, and some manifest details
may change before 1.0. Pin the Netsuke version in automated workflows.

## Install Netsuke

Netsuke requires [Ninja](https://ninja-build.org/) on `PATH`. A source build
also requires the dated Rust nightly toolchain pinned in `rust-toolchain.toml`
because Netsuke builds with the Polonius borrow checker, which nightly enables
by default.

Inside a checkout, `rustup` automatically selects the pinned toolchain from
`rust-toolchain.toml`; no command-line argument is required.

Netsuke v0.1.0-beta2 is available from crates.io. Where
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) is available,
prefer it: it fetches a prebuilt release binary and avoids the toolchain
requirement below.

<!-- tested-example: guide-binstall-install -->

```sh
cargo binstall netsuke-build
```

Building from the registry instead runs outside a repository checkout, so the
pinned toolchain is not picked up automatically; select it explicitly:

<!-- tested-example: guide-crates-io-install -->

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Pre-built installers are available from the
[v0.1.0-beta2 GitHub release](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta2):

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

The MSI installer supports pre-release SemVer versions such as `0.1.0-beta2`:
the pre-release suffix cannot be represented in an MSI product version, so the
installer carries the numeric release triple (`0.1.0`) while the full version
remains in the package and release names. Because successive pre-releases share
that numeric version, installing a later pre-release MSI replaces the existing
installation for that version series rather than installing alongside it.

SHA-256 checksum files accompany standalone binaries and staged help,
completion, and licence files. Installer packages do not have checksum sidecars
in v0.1.0-beta2. Windows PowerShell help files are published beside each MSI as
sidecar artefacts rather than embedded in the installer.

Each standalone release archive also contains generated shell completion
sidecars under `completions/<shell>/` for Bash, Elvish, Fish, PowerShell, and
Zsh. These files are portable and separate from the executable and installer
payloads. To use one, extract the matching archive and copy the file for the
chosen shell into that shell's normal completion directory, or load it through
the shell's documented completion mechanism. The package installation commands
above do not install completion files; completion directory names and
activation steps vary by shell and platform.

Install the current source checkout with Cargo. The clone supplies the pinned
nightly toolchain, so it is not given here — unlike the registry install above,
which runs outside a checkout:

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
$releaseUri = 'https://api.github.com/repos/leynos/netsuke/releases/tags/v0.1.0-beta2'
$release = Invoke-RestMethod -Uri $releaseUri

$documents = [Environment]::GetFolderPath('MyDocuments')
$editionDirectory = if ($PSVersionTable.PSEdition -eq 'Desktop') {
    'WindowsPowerShell'
} else {
    'PowerShell'
}
$moduleRoot = Join-Path $documents "$editionDirectory\Modules"
$moduleDirectory = Join-Path $moduleRoot 'Netsuke\0.1.0-beta2'
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
    body: "{{ greeting }}, {{ name }}!"

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

`defaults` entries are literal names in v0.1.0-beta2; Jinja expressions are not
rendered in this field.

`vars` keys named `env` or `glob` are rejected because those names identify
built-in template helpers (see
[Discover files with `glob`](#discover-files-with-glob) and
[Select optional tools](#select-optional-tools)). Rather than silently
shadowing the helper, the manifest fails to parse and the error names the
offending key.

### Rules and recipes

Rules must provide exactly one executable recipe. Actions and targets that
perform work must also provide exactly one recipe, but may omit it when a
non-empty `deps` list is their complete operation. This dependency-only
aggregate form is preferred over a no-op command such as `command: ":"`:

- `command`: one shell command, or an ordered list of commands.
- `script`: a multi-line script interpreted by the selected recipe shell.
- `rule`: the name of another rule to use.

Rules may also provide `description`, text used for Ninja's progress display.

Targets and actions may also provide `description`, but with a different
purpose: a target or action description is discovery metadata surfaced by
`netsuke help targets` (see
[Generate and inspect artefacts](#generate-and-inspect-artefacts)). It does not
affect Ninja progress output, which stays driven by the referenced rule's
`description`.

A `command` list runs its entries in declaration order and stops at the first
non-zero exit, so entries share the fail-fast behaviour of a handwritten `&&`
chain. The command field is a `StringOrList`: a scalar remains one shell
command, while a YAML sequence is rendered and lowered one entry at a time.
This applies equally to rules, direct targets, and actions. Each entry sees the
same Jinja context, including `{{ ins }}` and `{{ outs }}`; those two
placeholders are resolved later to the concrete target's shell-quoted input and
output paths. An empty command list is rejected when the manifest is parsed.


### Windows legacy recipe contract

On Windows, v0.1.x interprets every legacy `command` string, `command` list,
and `script` with **Windows PowerShell** (`powershell.exe`), not with the shell
that launched `netsuke`. Netsuke invokes it explicitly with an encoded,
non-interactive, no-profile command before Ninja executes a recipe. A build
started from PowerShell, `cmd.exe`, an IDE, or Git Bash therefore uses the same
recipe interpreter. This is a Windows PowerShell contract, not a PowerShell
Core (`pwsh`) contract.

Scalar commands and scripts each receive a fresh PowerShell process. A command
list receives one shared process: entries run in declaration order, and each
generated entry is followed immediately by a `$LASTEXITCODE` check. A non-zero
status stops the list before a later entry can overwrite it. An entry remains
opaque recipe text, so multiple native commands separated by semicolons inside
one entry are not checked individually; only the status left by that entry is
observed. PowerShell terminating errors also fail the recipe. Later entries see
PowerShell variables, `$env:` assignments, and locations left by earlier
entries, but state does not cross action or target boundaries.

Use PowerShell syntax in the default route. `$name` is a PowerShell variable
and `$env:NAME` reads an environment variable; `${VAR:-default}` is POSIX
syntax and is not valid PowerShell. Recipe text is protected from Ninja dollar
expansion, so write ordinary PowerShell dollars rather than `$$`. The rendered
`{{ ins }}` and `{{ outs }}` paths use single-quoted PowerShell arguments.
Build and default-target paths containing spaces are rejected before recipe
generation. Quote every other path and argument with PowerShell syntax;
arbitrary rendered Jinja text is not shell-quoted.

Netsuke rejects a PowerShell recipe when its encoded invocation would exceed
the 32,766-character Windows command-line safety limit. The diagnostic
instructs that the legacy recipe be split into smaller actions; v0.1.x does not
spill an oversized script to a temporary file or standard input.

Ninja turns a failed recipe into its own non-zero result, and `netsuke` returns
failure after forwarding Ninja's output. The CLI contract distinguishes success
from failure; it does not promise to return the recipe's exact child value.

To retain POSIX interpretation on Windows, explicitly select a Git
Bash-compatible runtime:

<!-- tested-example: guide-windows-bash-compatibility -->
```powershell
choco install git --yes --no-progress
$env:PATH = "C:\Program Files\Git\bin;$env:PATH"
$env:NETSUKE_WINDOWS_SHELL = "bash"
netsuke build
```

MSYS2 Bash is also supported when `bash.exe` is on `PATH`. Before `build` or
Ninja-tool execution, Netsuke checks this selection. If `bash.exe --version`
cannot run, it stops with instructions to install Git for Windows or MSYS2, add
Bash to `PATH`, or unset `NETSUKE_WINDOWS_SHELL`. `generate` and `help targets`
do not execute recipes, so they do not require Bash. In CI, install Git
explicitly, prepend its `bin` directory to `PATH`, set
`NETSUKE_WINDOWS_SHELL=bash`, and launch Netsuke normally from a `pwsh` step;
do not rely on a workflow-wide `shell: bash` setting.

For the Unix default and the explicit Bash route, each list entry is evaluated
inside its own brace group and the groups are joined with `&&`. The entry is
passed to `eval` as a shell-quoted payload, so an inline `#` comment or a
trailing control operator such as `&` cannot consume the generated group's
closing boundary. Brace groups run in the current shell rather than a
subshell: a changed working directory, environment assignment, or shell
variable can therefore be used by later entries. A failed entry stops the
chain, and the diagnostic identifies the generated action and one-based
list-entry positions, for example `netsuke command-list failure: action HASH,
entry 2`. These brace-group, `eval`, background-job, and `exec` restrictions
apply only to the Unix renderer and the explicit Windows Bash compatibility
route; the Windows PowerShell route uses the native-command and terminating-
error checks described above instead.

<!-- tested-example: guide-command-list -->

```yaml
netsuke_version: "1.0.0"

rules:
  - name: comprehensive-check
    description: Run the required checks sequentially
    command:
      - echo "check-fmt"
      - echo "lint"
      - echo "test"

targets:
  - name: done
    rule: comprehensive-check
```

The same list form can be attached directly to a target. Jinja rendering and
`{{ outs }}` interpolation apply independently to each entry:

<!-- tested-example: guide-direct-command-list -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: report.txt
    vars:
      heading: Report
    command:
      - "printf '{{ heading }}\\n' > {{ outs }}"
      - "printf 'complete\\n' >> {{ outs }}"
```

Prefer a `command` list for a short, ordered sequence of distinct commands.
Prefer `script` when the logic needs multi-line structure or shell constructs
such as loops, conditionals, or variable assignment.

Legacy recipes remain shell strings in v0.1.x. The structured command blocks
and argv templates proposed in
[RFC #573](https://github.com/leynos/netsuke/pull/573) for v0.2.0 are intended
to remove this shell-selection, quoting, path, variable, and exit-semantics
ambiguity. They do not change the v0.1.x contract described here.

### Targets, inputs, and dependencies

A target supports these fields:

- `name`: one output path or a list of output paths.
- `rule`, `command`, or `script`: exactly one recipe for work that has its own
  execution step. An action or target with a non-empty `deps` list may omit a
  recipe to form a dependency-only aggregate; this is preferred over a no-op
  `command: ":"`.
- `sources`: explicit inputs. They affect freshness and become `{{ ins }}`.
- `deps`: implicit dependencies. They affect freshness but do not become
  recipe arguments. Declare them on each target; reusable rules reject `deps`.
  The planned rule-level `deps_from` contract is not implemented in
  v0.1.0-beta2.
- `dependency_order`: scheduling policy for the `deps` list. `parallel` is the
  default; `serial` runs a list with more than one dependency in declaration
  order.
- `order_only_deps`: ordering dependencies. Their changes do not rebuild the
  dependent target.
- `vars`: values that override global variables for this target. The `env`
  and `glob` restriction above applies here too.
- `phony`: marks a logical target that does not represent a file.
- `always`: forces the recipe to run whenever the target is requested.
- `description`: an optional human-readable summary of the public operation
  the target performs. It is discovery metadata shown by
  `netsuke help targets`; it never replaces a referenced rule's `description`
  in Ninja progress output.

`name`, `sources`, `deps`, and `order_only_deps` accept either one string or a
list of strings.

Netsuke quotes paths inserted through `{{ ins }}` and `{{ outs }}`. Other Jinja
values render as ordinary command text and are not automatically shell-quoted.
The `shell_escape` filter described in older drafts is not implemented in
v0.1.0-beta2.

Cycle detection follows `sources` and `deps`. Order-only dependencies enforce
ordering but do not participate in cycle detection.

### Run direct dependencies serially

Actions and targets both accept `dependency_order`. Omit it, or set it to
`parallel`, to retain Ninja's ordinary concurrent scheduling. Set it to
`serial` when the direct `deps` list is an ordered workflow:

<!-- tested-example: guide-serial-dependency-order-manifest -->

```yaml
netsuke_version: "1.0.0"

actions:
  - name: check-fmt
    command: "echo checking format"
  - name: lint
    command: "echo linting"
  - name: test
    command: "echo testing"
  - name: all
    dependency_order: serial
    deps:
      - check-fmt
      - lint
      - test

targets:
  - name: release-notes
    command: "echo preparing release notes"
  - name: release
    command: "./package-release"
    dependency_order: serial
    deps:
      - check-fmt
      - test
      - release-notes
```

The `all` action has no recipe because its dependencies are the complete
workflow. Netsuke lowers it to a native Ninja `phony` node, so it has no shell
no-op of its own.

For a serial list, Netsuke starts each direct dependency only after the
preceding one succeeds. If an earlier dependency fails, later dependencies in
that list do not start through the serial path. Repeated or shared dependencies
are still owned by the one Ninja invocation and execute at most once.

Serial ordering applies only to the direct `deps` list. It does not serialize
`sources`, `order_only_deps`, or unrelated work. An independently requested or
otherwise reachable later dependency can still start through that separate
path; use a dedicated aggregate action when the whole workflow must share the
same ordered entry point.

Netsuke uses Ninja's `dyndep` support for serial lists with two or more
dependencies, and generated builds containing staged serial ordering require
Ninja 1.10 or newer. `netsuke generate`, `build`, and `clean` materialize the
generated sidecars under `.netsuke/dyndep` in the effective working directory
before writing or invoking the generated Ninja file. The sidecars are immutable
and content-addressed. Each sidecar-capable command retains the current bundle,
then at most 32 obsolete `.dd` files and 1 MiB of obsolete `.dd` bytes. Stale
`.tmp` files are removed while the exclusive sidecar-directory lease is held.
`build` and `generate` prune after materialization; `clean` prunes only after
successful `ninja -t clean`, and does not prune when clean fails.

An older arbitrary manifest written with `generate --output` may lose its
referenced sidecars after a later command. Regenerate that manifest before
using it if retention has removed any of its sidecars. The paths
`.netsuke/serial` and `.netsuke/dyndep` must not occur in any user graph path,
including outputs, inputs, implicit dependencies, and order-only dependencies;
they are reserved for Netsuke-generated gates and sidecars.

When migrating an existing manifest, see the
[v0.1.0 migration guide](v0-1-0-migration-guide.md#opting-into-serial-dependency-ordering)
for the opt-in syntax, Ninja version requirement, and generated-state
reservation.

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
are returned. Relative patterns resolve against the directory containing the
manifest — the workspace root — independent of the directory Netsuke is invoked
from, so `glob('src/*.c')` in a `Netsukefile` at the project root matches
`<project>/src/*.c`. The [quick-start guide](quickstart.md) shows a complete
runnable example.

Patterns may be absolute or relative to the manifest directory, including
parent-relative patterns such as `glob('../shared/*.h')`. Relative results
retain their pattern-relative spelling after Netsuke removes the workspace
base; absolute patterns remain absolute. Expansion is scoped to the pattern's
longest literal directory prefix — the text up to the first `*`, `?`, `[` or
`{`, trimmed back to the last separator, so `src/` for `src/**/*.c`. If that
prefix does not exist, or names something that is not a directory, the call
returns an empty list rather than failing. A symbolic-link literal prefix, such
as `src/link/*.c`, cannot establish the capability and causes expansion to
fail. A match is skipped rather than reported as an error when the metadata
lookup cannot resolve a symbolic link — the match itself or a directory reached
on the way to it — because it is unreadable within the prefix, dangling, or
resolves outside that prefix. A cyclic symbolic link is reported as an error
rather than skipped, since it describes a broken tree rather than a missing
file.

Patterns with unmatched braces are rejected during validation. When an opening
brace remains unclosed, the diagnostic points to the outermost unmatched
opening brace; an unmatched closing brace is reported at that closing brace.

The Jinja helper rejects a matched path unless it can be inserted as one
portable unquoted shell word. ASCII letters, digits, `/`, `:`, comma, full
stop, underscore, and hyphen are accepted; whitespace, control characters, and
shell punctuation are rejected. This prevents an untrusted checkout filename
from becoming shell syntax when `item` is interpolated into a `command` or
`script`. The Rust `manifest::glob_paths` query performs no shell-safety
validation; each caller must validate or escape matched paths before passing
them to a command sink.

Rust callers use `manifest::glob_paths(pattern, base)` with an optional base.
`Some(&Utf8Path)` anchors relative patterns and strips that base from results;
absolute patterns ignore the base, while `None` resolves relative patterns
against the process working directory.

### Define reusable macros

Macros return rendered text and can accept default arguments:

<!-- tested-example: guide-macro-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  greeting: Hello

macros:
  - signature: "say(name, punctuation='!')"
    body: "{{ greeting }}, {{ name }}{{ punctuation }}"

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
`PATHEXT`, the same list the shell uses — so `which('cargo')` finds `cargo.exe`
provided `.exe` is among those entries. A custom `PATHEXT` may legitimately
omit it, in which case it is not a candidate.

`PATHEXT` falls back to the built-in list only when it is unset or when no
entry survives normalization — that is, every entry is empty or whitespace. Any
other value is used as given, however unusual. The built-in list, in order:

`.com`, `.exe`, `.bat`, `.cmd`, `.vbs`, `.vbe`, `.js`, `.jse`, `.wsf`, `.wsh`,
`.msc`

The fallback exists because an empty effective list would match nothing and
report every command missing. Entries are matched case-insensitively and tried
in the order the list gives them. A name that already carries an extension is
used as written.

`command_available(name, **kwargs)` returns a boolean and is better for
complementary branches:

<!-- tested-example: guide-command-available-manifest -->

```yaml
netsuke_version: "1.0.0"

actions:
  - name: test-fast
    command: "cargo nextest run"
    deps:
      - config/test-profile.toml
    when: command_available("cargo-nextest")

  - name: test-fast
    command: "cargo test"
    deps:
      - config/test-profile.toml
    when: not command_available("cargo-nextest")

targets: []

defaults:
  - test-fast
```

Netsuke evaluates both guards while loading the manifest, without running
either recipe, so exactly one `test-fast` action enters the build graph. The
selected action's `deps` become Ninja implicit dependencies: changes to
`config/test-profile.toml` make the action stale, but the path is not appended
to `cargo nextest run` or `cargo test` as a recipe argument.

Both helpers accept:

- `all=true`: return all `which` matches. It does not change the boolean result
  from `command_available`.
- `canonical=true`: canonicalize matching paths.
- `fresh=true`: bypass the resolver cache for this lookup.
- `cwd_mode="auto"|"always"|"never"`: control bounded project-directory
  fallback searching.

The `env(name)` function reads one required environment variable. v0.1.0-beta2
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
section is private in intent and unstable, liable to change or disappear in any
beta release. It is documented here for the benefit of anyone who calls it
anyway, with that caveat understood.

A program calling Netsuke's Rust API can invoke Ninja without touching its own
process environment. `netsuke::runner::CommandEnv` carries child environment
overrides as data — `inherit()` changes nothing, `with_var` and `with_path` set
variables for the spawned command only — and the explicit request forms
`run_ninja_with` and `run_ninja_tool_with` accept a request naming the program,
build file, targets or tool, that environment, and a `stderr_mode: StderrMode`
policy routing the child's standard streams: `Suppress` drains both streams
(keeping JSON diagnostics machine-readable), while `Forward` relays them to the
caller. The convenience wrappers `run_ninja` and `run_ninja_tool` behave
identically with an inherited environment, deriving the policy from the CLI's
JSON setting. Overrides are additive: variables not named are inherited from
the calling process, and the injected `PATH` governs what commands Ninja
launches will see. Relative program names remain valid and resolve through that
child `PATH`; supply an absolute or otherwise resolved `program` only when
executable selection must stay isolated from the injected `PATH`.

The request itself is a named type: `netsuke::runner::NinjaBuildRequest` for a
build and `netsuke::runner::NinjaToolRequest` for `ninja -t <tool>`. Both
borrow their fields, so one `CommandEnv` and one `NinjaProcessOptions` can
serve several invocations. The
[v0.1.0 migration guide](v0-1-0-migration-guide.md) summarizes these additions
and confirms the wrappers are unchanged.

<!-- tested-example: guide-ninja-request-snippet -->

```rust
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest,
    StderrMode, run_ninja_tool_with, run_ninja_with,
};
use std::path::Path;

let options = NinjaProcessOptions::default();
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
    options: &options,
    build_file: Path::new("build.ninja"),
    targets: &targets,
    env: &env,
    // `Suppress` in JSON diagnostics mode keeps the child's output out of
    // the machine-readable streams; `Forward` relays it to the caller.
    stderr_mode: StderrMode::Forward,
};
let clean = NinjaToolRequest {
    program: Path::new("/usr/bin/ninja"),
    options: &options,
    build_file: Path::new("build.ninja"),
    tool: "clean",
    env: &env,
    stderr_mode: StderrMode::Forward,
};

if std::env::var_os("NETSUKE_GUIDE_RUN").is_some() {
    run_ninja_with(&build).expect("run ninja");
    run_ninja_tool_with(&clean).expect("run ninja -t clean");
}
```

The convenience wrappers `run_ninja` and `run_ninja_tool` keep their existing
signatures and behaviour, so a caller that uses them needs no change; the
request bundles use `options: &options` instead of `cli: &cli` and gained the
required `stderr_mode` field, so a caller that constructs `NinjaBuildRequest`/
`NinjaToolRequest` directly must supply both. Each release records such
additions in [`CHANGELOG.md`](../CHANGELOG.md), which is where Netsuke
signposts Rust API changes — with no stability promise attached to them ahead
of 1.0.

### Capture verbose timing output

Rust callers that wrap a `StatusReporter` can send verbose timing summaries to
an owned sink with `VerboseTimingReporter::with_writer`:

<!-- tested-example: guide-verbose-timing-reporter -->

```rust
use netsuke::output_prefs::resolve;
use netsuke::status::{SilentReporter, VerboseTimingReporter};

let reporter = VerboseTimingReporter::with_writer(
    Box::new(SilentReporter),
    resolve(None),
    Vec::<u8>::new(),
);
```

The generic writer must implement `Write + Send` and is owned by the timing
reporter. `VerboseTimingReporter::new` remains the default API and writes to
`io::Stderr`. On the first completion, the wrapped reporter receives its
completion event before the timing summary is written synchronously to the
sink. A blocking sink therefore blocks only that completion call; later stage,
progress, and completion events remain suppressed. Re-entrant calls observe the
completed state, and summary lines retain their rendered order. Write errors
are ignored, matching the existing accessible reporter contract; applications
can observe them through the bounded timing sink telemetry emitted by their
configured metrics and tracing backends.

## Use the template standard library

Netsuke registers focused path, collection, command, network, and time helpers
alongside MiniJinja's built-ins. The library covers path and collection
filters, file tests, clocks and durations, host commands, executable discovery,
environment variables, globbing, and policy-controlled network retrieval.

See the [template standard-library guide](stdlib-yaml-and-jinja-guide.md) for
every helper's signature, defaults, purity, platform caveats, and executable
examples. Host-observing helpers belong only in trusted manifests: Netsuke
bounds command and network output, but does not sandbox template evaluation.

When a Boolean is interpolated into a string field, Netsuke renders it as
lowercase `true` or `false`. For example, this writes `true` to `status.txt`:

<!-- tested-example: guide-boolean-string-interpolation -->

```yaml
netsuke_version: "1.0.0"

vars:
  enabled: true

targets:
  - name: status.txt
    command: "printf '%s\\n' '{{ enabled }}' > {{ outs }}"
```

The `now(offset=...)` helper accepts `Z` or `z` for UTC and signed ISO 8601
offsets whose absolute hour component is below 24. Offsets such as `+24:00`,
`-24:00`, and larger absolute hour values are rejected as invalid.

One helper deserves a note here because its result depends on the host's
environment. `path | expanduser` expands a leading `~` against the home
directory, resolved from `HOME` then `USERPROFILE` on POSIX hosts, and from
`HOME`, `USERPROFILE`, the `HOMEDRIVE`/`HOMEPATH` pair, then `HOMESHARE` on
Windows. The Windows pair counts only when both halves are non-empty; an
incomplete pair falls through to `HOMESHARE`. Named-user forms such as `~alice`
are unsupported, and when no home directory resolves at all, the filter fails
rather than passing the `~` through silently.

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
- `help [TOPIC]`: print the top-level help, or the help for a named topic.
  With no topic, it matches `--help`. `help targets` prints the target and
  action catalogue for the selected manifest (see
  [Generate and inspect artefacts](#generate-and-inspect-artefacts)).

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

`--directory` affects manifest lookup, automatic project-configuration
discovery, and relative output paths. It does not rebase an explicit `--config`
path or `NETSUKE_CONFIG` value: a relative selector resolves from the process
working directory, while an absolute selector remains unchanged. Pass an
absolute path when the selector must not depend on the invoking directory.

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
manifest to that file and leaves stdout empty. For a serial dependency list,
both forms also materialize the referenced sidecars under `.netsuke/dyndep` in
the effective Ninja working directory, so the emitted manifest is executable at
that point. Retention is bounded: a later Netsuke command may remove sidecars
referenced by an older arbitrary output file. Regenerate the file when that
happens.

`clean` removes file outputs tracked by Ninja. Phony targets and actions do not
represent files and are not removed.

`help targets` prints the target and action catalogue for the selected manifest
— actions first, then targets — with a localized default marker such as
`[★ default]` (or `[* default]` in accessible output) on manifest defaults and
an empty description column for entries without a `description`:

<!-- tested-example: guide-help-targets -->

```sh
netsuke help targets
```

The command loads, expands, renders, and validates the manifest through the
same structural stages as a build, but performs no recipes and creates no build
outputs. Rendering uses a restricted, side-effect-free Jinja surface. Queries
allow only the lexical path filters `basename`, `dirname`, `with_suffix`, and
`relative_to`, the collection filters `uniq`, `flatten`, and `group_by`, and
the clock-independent `timedelta` function. Query rendering skips command and
script recipe bodies, so build-only helpers in those recipes are not evaluated
and do not make discovery fail. Metadata such as `vars`, names, dependencies,
and descriptions is still rendered; structural rule selectors are rendered as
needed for graph validation.

Queries reject direct use of `env()` and `glob()`, file tests, filesystem
metadata filters such as `size` and `linecount`, `hash`, `digest`, `contents`,
`realpath`, and `expanduser`, executable discovery through `which` and
`command_available`, network and command helpers (`fetch`, `shell`, and
`grep`), and the clock-dependent `now()` function. A helper from this disabled
set in a `when` expression cannot be evaluated safely during discovery, so its
entry is retained and marked conditional, but that unresolved alternative is
excluded from graph validation. An ordinary false `when` expression still
filters its entry out. Normal build manifest rendering retains the full
standard library and its existing `when` semantics; these restrictions apply
only to query rendering.

In human-readable output, a conditional entry carries `[◇ conditional]` when
emoji output is allowed, or `[? conditional]` in the ASCII theme. JSON output
always includes a boolean `conditional` field: `true` means that discovery
could not resolve the entry's `when` expression, while `false` means no such
uncertainty was recorded. Integrations should therefore preserve conditional
entries rather than treating them as confirmed selections. The command honours
the usual manifest-selection options (`--file`, `-C/--directory`) and the
normal colour, accessibility, locale, and JSON-output conventions; with
`--json` the catalogue is emitted as a versioned JSON document whose
`result.command` is `help-targets`.

The standard-library reference describes the full helper set available while
rendering a normal build manifest. The query allowlist above is the deliberate
exception for `netsuke help targets`.

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

When automatic discovery finds no configuration file, Netsuke uses its built-in
defaults. When it finds a candidate that cannot be loaded, such as malformed
TOML or a file whose `extends` parent is missing, Netsuke reports the load
error. A broken discovered configuration is therefore not treated as absent.

On Windows, Netsuke normalizes alternate spellings of a configuration path,
including short and long path forms, before comparing discovered layers. A
project `.netsuke.toml` therefore contributes one layer even when two spellings
refer to the same physical file.

### Diagnose configuration selection

Pass `--verbose` to see how Netsuke selected its configuration. Structured
events report whether `--config`, `NETSUKE_CONFIG`, or automatic discovery won,
whether a path was present, and which environment lookups were attempted.
Events then identify whether Netsuke uses an explicit file or discovered layers.

During the merge, verbose tracing also reports the defaults, file, environment,
and CLI layers as they are applied. File-layer events include a bounded
`path_hash` so operators can correlate a layer with discovery events without
recording the raw path. CLI events record only the leaf keys in
`override_keys`; they do not record override values such as paths or host
lists. If validation rejects the merged configuration, the event includes the
rejected setting in `key` and a bounded explanation in `reason`. These events
make configuration precedence and rejection decisions auditable without
exposing user-supplied values.

If an explicit file cannot be loaded, the warning records `failure_kind` as
`Missing` or `LoadError`. Verbose tracing uses only `path_hash` and
`path_present`; it never exposes a file name or full path. The unkeyed
`path_hash` is only a correlation identifier: it does not protect a guessable
path from disclosure.

Configuration tracing is disabled in JSON mode, including when `json = true`
comes from a configuration file. This keeps stderr empty for successful JSON
commands and reserves it for the single diagnostic document on failure.

For a terminal human-mode failure in either the early diagnostic-mode
preference pass or the full `config_merge` phase, the
`configuration load failed` event includes bounded `operation` and
`error_category` fields. JSON mode instead emits the diagnostic document.
Passing `--verbose` additionally emits one final `metrics snapshot` debug event
before Netsuke exits. The snapshot is an in-process diagnostic record, not a
metrics listener or a Prometheus endpoint. After a successful configuration
merge, verbosity can also come from `NETSUKE_VERBOSE` or `verbose = true` in a
configuration file. A configuration failure before that merge uses only CLI
`--verbose`.

It includes the bounded configuration-load series:

- `netsuke_config_load_total`, with `outcome=success` or `outcome=failure`.
- `netsuke_config_load_duration_seconds`, with one sample for the startup
  configuration-load attempt.
- The phase-level `config_load_total` and
  `config_load_duration_seconds` entries, labelled with `phase=diag_mode` or
  `phase=merge`; the counter also carries the bounded outcome value.
- `netsuke_cli_config_discovery_total`, with `outcome=success` or
  `outcome=error`, and `netsuke_cli_config_discovery_duration_seconds`, which
  records the cached discovery pass duration.

For example, a missing explicit file reports the actionable error first, then
the bounded tracing fields and the snapshot (timestamps and metric values vary):

<!-- tested-example: guide-configuration-observability -->

```plaintext
Configuration file error in 'missing.toml': explicit configuration file not found
ERROR ... configuration load failed operation="diag_mode_resolution" error_category="io"
DEBUG ... metrics snapshot metrics=[...]
```

The snapshot is available for a configuration failure when `--verbose` was
supplied on the command line. A `verbose = true` setting in a file that cannot
be loaded cannot enable diagnostics because configuration merging has not
completed. JSON mode suppresses the tracing and snapshot so stderr remains one
machine-readable diagnostic document.

#### Cached merge API (unstable)

Programs using Netsuke's unstable Rust API can retain the layers from one
discovery pass and observe the subsequent merge. Construct
`CachedMergeInput::new(cli, matches, env, discovered)` with the parsed CLI
values, an injected `ConfigEnvProvider`, and `DiscoveryOutcome::into_layers()`;
then pass it to `cli::merge_with_cached_file_layers_with_observer(input)`. The
function returns the merge result alongside bounded events; replay those events
through `MergeObserver`, such as `TracingMergeObserver`. Another caller can
provide its own `MergeObserver` implementation. Observers receive bounded
`MergeEvent` values: layer application and failure states, file `path_hash` and
layer counts, CLI override leaf keys, and validation `key`/`reason` fields.
Configuration values and raw paths are never included. Ordinary
`merge_with_config*` and `merge_with_cached_file_layers` calls discard their
collected events and do not emit merge tracing.

#### Bounded configuration metrics

Configuration loading is recorded as two bounded metric series, both emitted in
the drained `metrics snapshot`:

- `config_load_total` — a counter with the `phase` and `outcome` labels that
  counts each configuration-loading phase. `phase` is `diag_mode` for the early
  diagnostic-mode resolution or `merge` for the full configuration merge;
  `outcome` is `success` or `failure`.
- `config_load_duration_seconds` — a histogram with the `phase` label only,
  recording each phase's duration in seconds.
- `netsuke_cli_config_discovery_total` — a counter with a bounded `outcome`
  label of `success` or `error` for the discovery pass reused by startup.
- `netsuke_cli_config_discovery_duration_seconds` — a histogram recording the
  discovery pass duration without labels.

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

### Diagnose Ninja execution

Pass `--verbose` to see Ninja subprocess diagnostics on stderr. The
`Executing Ninja subprocess` informational event contains stable fields:
`operation`, `ninja_program`, `arg_count`, `env_override_count`,
`path_overridden`, and `suppress_stderr`. A debug-level companion event
contains the redacted command as `Executing command: ...`, including the
arguments used for the invocation.

Ninja executable resolution also emits a debug event with `ninja_program` and
`source`. The source is `NETSUKE_NINJA` for a valid override and `fallback`
when the variable is unset, empty, or non-UTF-8. JSON mode suppresses these
tracing events so stderr remains parseable.

`NETSUKE_WHICH_WORKSPACE` switches off the `which()` workspace-tree fallback
search that runs when a command is not found on `PATH`. Set it to `0`, `false`,
or `off` (case-insensitively) to disable the fallback; any other value, or
leaving it unset, keeps the fallback enabled. A non-Unicode value also disables
the fallback and is treated as an explicit opt-out, emitting a warning.

### Policy values and parsing

The CLI and configuration use the same policy values, and policy names are
matched case-insensitively in both places. The accepted values are:

- `--color` and `color`: `auto`, `always`, or `never`.
- `--emoji` and `emoji`: `auto`, `always`, or `never`.
- `--progress` and `progress`: `auto`, `always`, or `never`.
- `--accessibility` and `accessibility`: `auto`, `on`, or `off`.

Lowercase spellings are used in help and examples. For instance,
`--color ALWAYS` and `color = "AlWaYs"` select the same explicit colour policy
as `--color always` and `color = "always"`. `auto` follows terminal and
environment detection. `always` or `never` makes colour, emoji, or progress
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
v0.1.0-beta2, so `color` currently affects mode selection but does not add
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
    "version": "0.1.0-beta2"
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

**Triage:** Treat schema version `1` as pre-stable for v0.1.0-beta2 and check
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

Exact and wildcard host matching ignores one terminal DNS dot: `example.com`
matches `example.com.`, and `*.example.com` matches `sub.example.com.`. The
wildcard does not match the apex, so `*.example.com` does not match
`example.com.`.

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

Terminal human-mode configuration-load events emitted by `config_err_to_exit`
include structured `operation` and `error_category` fields. `operation` is
`diag_mode_resolution` for the early diagnostic preference phase or
`config_merge` for the full configuration-merge phase; `error_category` is `io`,
`validation`, or `parse`. Paths and display text are never recorded. JSON mode
preserves the diagnostic document as the machine-readable failure output.

The `--verbose` flag enables diagnostic tracing, successful timing summaries,
and the final metrics snapshot described in
[Diagnose configuration selection](#diagnose-configuration-selection). It is
suppressed in JSON mode so stderr remains parseable.

## Review the safety boundary

Netsuke reduces some common quoting mistakes, but it is not a sandbox:

- `{{ ins }}` and `{{ outs }}` are quoted as path arguments.
- Arbitrary Jinja values in `command` and `script` are not automatically
  shell-quoted.
- On Windows, legacy recipes use the PowerShell contract above unless
  `NETSUKE_WINDOWS_SHELL=bash` selects the explicit Bash compatibility route.
  On Unix, scripts use `/bin/sh -e`.
- `shell`, `grep`, `fetch`, filesystem helpers, and ordinary recipes interact
  with the host.
- `glob` restricts its filesystem metadata access to a capability handle
  scoped to the pattern's literal directory prefix, so it cannot inspect
  anything outside the subtree the pattern can match; the pattern match walk
  itself still uses ambient filesystem access.
- Verbose glob tracing replaces every caller-controlled path field — patterns,
  prefixes, and sampled relative matches — with the stable `<redacted>` marker.
  Aggregate metrics retain only bounded status and reason data. Error messages
  may retain the original input so invalid patterns can be explained.
- `raw` template output and handwritten shell fragments remain the manifest
  author's responsibility.
- On Unix and in the explicit Windows Bash compatibility route, each
  `command` list entry is joined into a single shell chain; a later entry
  inherits the working directory, environment, and shell variables left by an
  earlier entry, and runs only when that earlier entry exits with status zero.
  A failed entry may still leave side effects behind before it halts the chain.
  The generated brace/eval boundary keeps comments and trailing control
  operators inside an entry from changing the chain's structure. An entry may
  start at most one background job; Netsuke waits for that job before moving to
  a later entry, and rejects an entry that starts more than one background job
  during Ninja generation. It also rejects an entry whose nested `eval` payload
  makes the background-job count dynamic because the wrapper cannot safely
  determine which jobs to wait for. A direct simple `exec`, optionally
  prefixed by shell assignments, is supervised so its success or failure
  retains the list's status semantics: a successful `exec` ends the remaining
  chain, while structured or nested `exec` forms are rejected during Ninja
  generation. Failure diagnostics include the action fingerprint and one-based
  entry position when Netsuke can attribute the failed list entry.
- On Windows in the default PowerShell route, each command list shares one
  PowerShell process. Netsuke checks `$LASTEXITCODE` immediately after every
  generated list entry and stops before a later entry can overwrite a failure.
  Multiple native commands inside one entry are not individually instrumented;
  terminating PowerShell errors also stop the list. The POSIX
  brace-group, `eval`, background-job, and `exec` restrictions do not apply to
  this route.
- Write shell dollar expressions normally. `$PATH`, `$RUSTFLAGS`, and
  `${CARGO:-cargo}` reach POSIX routes unchanged; PowerShell routes use `$name`
  or `$env:NAME`. Netsuke performs the required Ninja escaping after it lowers
  `$in`, `$out`, `{{ ins }}`, and `{{ outs }}`. On POSIX and Bash routes, a
  `$in` or `$out` token inside backticks is rejected because Netsuke cannot
  safely lower it there. PowerShell uses backticks as its native escape syntax,
  so they do not suppress placeholder interpolation.
- Build and default-target paths reject `$`, spaces, colons, `|`, and control
  characters because Ninja cannot represent them without ambiguity. Generation
  also rejects newline, carriage-return, and NUL characters in emitted metadata
  such as descriptions, `depfile`, `deps`, and `pool`.
- **Migration:** replace the historical manifest spelling `$$PATH` with
  `$PATH`. On POSIX and Bash routes, `$$` is the shell's process identifier;
  PowerShell interprets `$$` as its automatic variable containing the last
  token received by the session. Keeping the extra dollar can therefore change
  the command's result. Existing script actions that use `$in` or `$out` will
  receive the same paths after this release, but their generated action
  identifiers change once because lowering now happens before Ninja emission;
  Ninja may therefore rebuild those targets once.

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
