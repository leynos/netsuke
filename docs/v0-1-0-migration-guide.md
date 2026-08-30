# Migrating to v0.1.0

This guide covers the released v0.1.0-beta3 additions: the injectable child
environment (`CommandEnv`), the named Ninja request types, narrow process
options (`NinjaProcessOptions`), target/action discovery through `description`
and `netsuke help targets`, and cached configuration discovery. Most existing
manifests remain compatible, and callers of the convenience wrappers retain
their child-process behaviour. Manifests using Jinja `glob()` must use
shell-inert matched paths. The cached configuration discovery API is a breaking
change for callers of the unstable Rust API; ordinary CLI users need no action.
The Ninja invocation chain now requires UTF-8 build-file and working-directory
paths; non-UTF-8 values are rejected at their input boundary.

Rust callers that construct `Target` with a struct literal must add the new
`description` field (set it to `None` or `Some(...)`); deserialized manifests
remain compatible. Callers constructing `NinjaBuildRequest` or
`NinjaToolRequest` must replace `cli: &cli` with `options: &options`; every
other addition is opt-in.

## Select the pinned Rust toolchain

Source builds from a checkout require the dated nightly pinned in
`rust-toolchain.toml`. Inside the checkout, `rustup` selects that toolchain
automatically, so no command-line argument is required. Registry installs run
outside the checkout and must select the same nightly explicitly:

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

## Netsuke is a build tool, not a library

Netsuke is intended to be used as a command-line build tool. The only surfaces
it commits to are the Netsukefile manifest format and the graph export.
Everything else — the Rust API described below included — is private in intent
and unstable in practice: it may change shape, or vanish, in any release of the
beta series without a deprecation period. Reliance on it is conditional on
tracking those changes.

## At-a-glance changes

<!-- markdownlint-disable-next-line MD013 -->
Table: documented v0.1.0 additions, including `netsuke help targets`, and their
impact

| Area                         | Impact                                                                                                                                                                                                                                                                                                                                                | Where to read more                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Convenience wrappers         | Unchanged. `run_ninja` and `run_ninja_tool` behave exactly as before, inheriting the process environment.                                                                                                                                                                                                                                             | [Users' guide](users-guide.md)                                                                   |
| Child environment            | New opt-in `netsuke::runner::CommandEnv` carries additive variable overrides and an injected `PATH` for Ninja child processes.                                                                                                                                                                                                                        | [Users' guide](users-guide.md)                                                                   |
| Request types                | New `netsuke::runner::NinjaBuildRequest` and `netsuke::runner::NinjaToolRequest` name the program, `NinjaProcessOptions`, build file, targets or tool, a child environment, and a required `stderr_mode: StderrMode` policy for the `*_with` run functions.                                                                                           | [Users' guide](users-guide.md)                                                                   |
| Cached CLI configuration API | Breaking for callers of the unstable Rust API: use the opt-in cached discovery flow with `ConfigEnvProvider`; `ConfigStdEnvProvider` supplies process-backed access.                                                                                                                                                                                  | [Users' guide](users-guide.md)                                                                   |
| Timing output                | Existing `VerboseTimingReporter::new` keeps its stderr sink; Rust callers can opt into an owned `Write + Send` sink with `with_writer`.                                                                                                                                                                                                               | [Users' guide](users-guide.md#capture-verbose-timing-output)                                     |
| Glob expansion               | Parent-relative patterns such as `glob('../shared/*.h')` now expand. The Jinja helper rejects matched paths that are not portable unquoted shell words. Metadata checks use a capability rooted at the pattern's longest literal directory prefix; missing or non-directory prefixes return no matches, and unresolvable symlink matches are skipped. | [Users' guide](users-guide.md) and [ADR-010](adr-010-scope-glob-capability-to-literal-prefix.md) |
| Command recipes              | On Windows, legacy scalar commands, lists, and scripts use Windows PowerShell by default; YAML command lists remain opt-in, ordered, and fail-fast.                                                                                                                                                                                                   | [Windows legacy recipe contract](users-guide.md#windows-legacy-recipe-contract)                  |
| Ninja text escaping          | Write shell dollars normally. `$ins` and `$outs` are shell variables, whereas `{{ ins }}` and `{{ outs }}` are Netsuke path markers. Spaces in build and default-target paths remain rejected. Paths containing `$`, colons, `\|`, or control characters remain rejected, as are newline, carriage-return, and NUL metadata values.                   | [Users' guide](users-guide.md#review-the-safety-boundary)                                        |
| Manifest discovery           | Optional target/action `description` values are shown by the new `netsuke help targets` command. Manifests without them and existing build output are unchanged.                                                                                                                                                                                      | [Users' guide](users-guide.md)                                                                   |
| Serial dependencies          | New opt-in `dependency_order: serial` runs an action or target's direct `deps` list in declaration order.                                                                                                                                                                                                                                             | [Serial dependency ordering](users-guide.md#run-direct-dependencies-serially)                    |

## Nothing to change for existing callers

The convenience wrappers keep their child-process behaviour: the child inherits
the calling process's environment, and Ninja is resolved exactly as before.
Their program and build-file path parameters now use `&Utf8Path`, so callers
using `std::path::Path` must convert those values. A caller that constructs
`NinjaBuildRequest`/`NinjaToolRequest` directly must pass `options: &options`
and supply the required `stderr_mode: StderrMode` field.

Existing callers that construct `VerboseTimingReporter::new` continue to
receive timing summaries on stderr. Callers that need to capture or redirect
those summaries can opt into `VerboseTimingReporter::with_writer`; the
[users' guide](users-guide.md#capture-verbose-timing-output) documents the
owned sink and completion-ordering contract.

## Use UTF-8 manifest and working-directory paths

The Ninja invocation chain accepts only UTF-8 paths. This includes the manifest
selected by `--file` or `NETSUKE_FILE`, the working directory selected by
`--directory`, and the paths passed to the unstable Rust Ninja APIs. The
`directory` setting is CLI-only; it has no configuration-file counterpart.

Command-line `--file` and `--directory` values are checked before manifest
discovery or runner setup. If the operating system supplies bytes that are not
valid UTF-8, parsing fails with a localized diagnostic naming the affected
option. A `file` value in a configuration file must decode as UTF-8, while a
`NETSUKE_FILE` value is checked while the environment layer extracts it; both
fail before the merged configuration reaches the runner.

Callers migrating from a non-UTF-8 path must rename or relocate the manifest or
working directory, so its path is representable as UTF-8, then pass the
supported path through `--file`, `--directory`, configuration, or
`NETSUKE_FILE` as appropriate. Environment-variable payloads used for the Ninja
child remain platform-native and are not subject to this path restriction.

## Check filenames used by manifest `glob()`

The planned beta3 source tightens the Jinja `glob()` helper. A manifest
that expands a matched filename containing whitespace, control characters, or
shell punctuation now fails during manifest loading. ASCII letters, digits, `/`,
`:`, comma, full stop, underscore, and hyphen remain accepted. This beta3 change
prevents a checkout filename from becoming executable shell syntax when a
`foreach` item is interpolated into a `command` or `script`.

Published beta2 does not reject these non-portable unquoted filenames.

Rename affected files to use the accepted character set, or change the manifest
so filesystem-derived paths do not cross the Jinja command-template boundary.
The Rust `manifest::glob_paths` query retains its previous contract and
continues to return any matching UTF-8 file path because its callers own their
downstream escaping boundary.

Relative manifest glob patterns, including parent-relative patterns, now
resolve from the manifest directory or workspace root rather than the process
working directory. Relative results retain their pattern-relative spelling
after the workspace base is stripped; absolute patterns remain absolute. The
manifest parse boundary supplies this base, so glob expansion does not read or
mutate process-global working-directory state. Callers of the Rust
`glob_paths(pattern, base)` API can supply the same base explicitly.

## Policy enum parsing

The public policy enums no longer implement `clap::ValueEnum`. This removes the
Clap parsing dependency from the domain configuration schema. Rust callers that
parse `ColourPolicy`, `ProgressPolicy`, `EmojiPolicy`, or `AccessibilityPolicy`
should use `str::parse` or `FromStr`; accepted spellings remain
case-insensitive and are listed in the users' guide's
[policy values and parsing](users-guide.md#policy-values-and-parsing) section.
Clap-specific validation and help metadata remain an internal CLI concern.

## Opting into ordered command lists

Existing scalar `command` recipes remain valid, so no migration is required. To
run a short sequence of commands in declaration order, change a recipe to a
non-empty YAML list. The entries run in one shell process and stop at the first
non-zero exit. See [Rules and recipes](users-guide.md#rules-and-recipes) for
the syntax, shell semantics, and examples.

## Windows legacy recipe interpreter

v0.1.x makes Windows legacy-recipe execution explicit. Netsuke starts
`powershell.exe` for every scalar command, ordered list, and script, regardless
of whether the CLI was launched by `pwsh`, `cmd.exe`, an IDE, or Git Bash. The
default is Windows PowerShell, not PowerShell Core. Existing Windows manifests
that contain POSIX-only syntax must either move to PowerShell syntax or opt
into the Bash compatibility route:

```powershell
choco install git --yes --no-progress
$env:PATH = "C:\Program Files\Git\bin;$env:PATH"
$env:NETSUKE_WINDOWS_SHELL = "bash"
netsuke build
```

MSYS2 is equally suitable when its `bash.exe` is on `PATH`. The executable is
checked before `build` and Ninja-tool commands, so an absent selected Bash
runtime produces an actionable Netsuke error instead of a Ninja command-not-
found failure. `generate` and `help targets` do not run recipes and therefore
do not require the optional runtime.

In the default route, write `$name` for a PowerShell variable and `$env:NAME`
for an environment variable. PowerShell parses the braced variable name in
`${VAR:-default}`, but does not perform POSIX default-value expansion. The
v0.1.0 dollar-escaping fix means these are ordinary, single dollars, not
Ninja-escaped `$$` forms. Ordered lists share one PowerShell process: Netsuke
checks `$LASTEXITCODE` immediately after each generated list entry, so a
non-zero status or terminating error stops the list before a later entry can
overwrite it. Multiple native commands inside one entry are not individually
instrumented. Variables, environment assignments, and current-directory changes
persist between entries. Each scalar, script, action, and target has a fresh
shell process. In PowerShell, `{{ ins }}` and `{{ outs }}` remain path-quoted.
Build and default-target paths containing spaces are escaped for Ninja, so
whitespace-containing outputs remain valid; quote any other path or argument
with the selected shell's syntax. Encoded PowerShell commands are retained
while they fit the Windows command-line limit. Larger recipes up to 1 MiB use
Ninja's `rspfile` and `rspfile_content` bindings, with a unique `$out`-derived
`.ps1` name per edge, created in that edge's working directory, containing an
ASCII PowerShell bootstrap and the Base64 UTF-16LE recipe payload. Recipes
above that limit are rejected before Netsuke allocates their encoded payload.
The command invokes it with `powershell.exe -File "$rspfile"`. The bootstrap
removes its own `$PSCommandPath` in a `finally` block after the recipe succeeds
or fails. Queries do not create response files. On POSIX and Bash routes, `$$`
means the process identifier; in PowerShell, `$$` is the automatic variable
containing the last token received by the session.

For reproducible Windows CI, use a `pwsh` step and let Netsuke select
PowerShell; do not use a workflow-level `shell: bash` setting as evidence of
recipe behaviour. If selecting Bash, install Git with Chocolatey as above,
prepend `C:\Program Files\Git\bin` to that step's `PATH`, and set
`NETSUKE_WINDOWS_SHELL=bash` explicitly.

This is deliberately a v0.1.x shell-string compatibility boundary. The
structured command blocks and argv templates in
[RFC: structured command blocks and argv templates #573](https://github.com/leynos/netsuke/pull/573)
are planned for v0.2.0 to remove shell-dependent quoting, paths, variable
expansion, and exit-status ambiguity. They are not backported through an
implicit change to legacy recipes.

## Opting into an explicit child environment

Construct a `CommandEnv`, name the variables to add, and pass it through the
request forms:

- `run_ninja_with` runs a build described by a `NinjaBuildRequest`;
- `run_ninja_tool_with` runs `ninja -t <tool>` described by a
  `NinjaToolRequest`.

Overrides are additive: variables not named are inherited. The injected `PATH`
governs what commands the Ninja child can see. Relative program names remain
valid and resolve through that child `PATH`; supply an absolute or otherwise
resolved `program` only when executable selection must stay isolated from the
injected `PATH`.

Both request types borrow their fields, so one `CommandEnv` and one
`NinjaProcessOptions` can serve several invocations. Direct request callers now
pass `options: &options` in place of `cli: &cli`.
`NinjaProcessOptions::working_dir` is `Option<Utf8PathBuf>`. Non-UTF-8 `--file`
and `--directory` values fail during CLI parsing. Configuration-file and
`NETSUKE_FILE` manifest values fail at their configuration or environment
boundary before runner setup. Worked examples live in the users' guide's "Drive
Ninja with an explicit environment" section.

## Cached CLI configuration API

Callers of the unstable Rust configuration API must update to the cached
configuration discovery flow. `ConfigEnvProvider` is the public environment
seam, and `ConfigStdEnvProvider` supplies process-backed access for production
callers. Deterministic tests and other adapters can implement
`ConfigEnvProvider` without mutating the process environment. This is a
breaking change without a deprecation period or stable compatibility guarantee.

Published beta2 already provides `merge_with_cached_file_layers`. The
observer-based `CachedMergeInput` flow described below is a beta3 change and is
not available in that release.

For the normal flow:

1. Call `resolve_json_and_layers_outcome_with_env` with a
   `ConfigEnvProvider`.
2. Call `emit_diagnostics()` after tracing is configured, then call
   `into_layers()` on the returned `DiscoveryOutcome`.
3. Pass the resulting `DiscoveredLayers` to
   `merge_with_cached_file_layers` with the same environment provider.

`DiscoveryOutcome` owns the deferred diagnostics until `emit_diagnostics()` and
the discovered layers until `into_layers()`.

Reusing the same discovered layers avoids a second configuration-file discovery
and loading pass. `merge_with_config` and `merge_with_config_and_env` remain
standalone alternatives: each discovers and merges configuration in one call,
so neither reuses an earlier discovery.

v0.1.0 also instruments configuration loading itself. The internal phase-level
series are `config_load_total`, labelled `phase=diag_mode|merge` and
`outcome=success|failure`, and `config_load_duration_seconds`, labelled only
`phase=diag_mode|merge`. The operator-facing startup-attempt series are
`netsuke_config_load_total`, labelled only `outcome=success|failure`, and
`netsuke_config_load_duration_seconds`, with no labels. Configuration-load
failures add bounded `operation` and `error_category` fields. Cached discovery
also records `netsuke_cli_config_discovery_total`, labelled
`outcome=success|error`, and `netsuke_cli_config_discovery_duration_seconds`
without labels. Neither exposes configuration paths. See the users' guide's
[bounded configuration metrics](users-guide.md#bounded-configuration-metrics)
and [interpret failures](users-guide.md#interpret-failures) sections.

## Opting into serial dependency ordering

Set `dependency_order: serial` on an action or target to run its direct `deps`
in declaration order. Omitting the field, or setting it to `parallel`, keeps
the existing parallel behaviour. Serial ordering stops later direct
dependencies after an earlier one fails, while shared work still executes at
most once in the enclosing Ninja invocation.

Serial lists containing two or more dependencies require Ninja 1.10 or newer.
The `generate` command materializes its supporting dyndep sidecars beneath
`.netsuke/dyndep` in the effective working directory while writing the
generated Ninja manifest. The `build` and `clean` commands materialize the
sidecars before invoking Ninja with the generated Ninja file. User targets must
not use `.netsuke/dyndep` or `.netsuke/serial`: both namespaces are reserved
for Netsuke's generated state. Sidecars are immutable and content-addressed.
Retention keeps the current bundle plus at most 32 obsolete `.dd` files and 1
MiB of obsolete `.dd` bytes; stale `.tmp` files are cleaned while the exclusive
sidecar-directory lease is held. `clean` prunes only after successful
`ninja -t clean`, not after a failure. An old arbitrary `generate --output`
manifest needs regeneration only if retention has removed any of its referenced
sidecars. See [ADR-012](adr-012-bound-dyndep-sidecar-retention.md). The
[users' guide](users-guide.md#run-direct-dependencies-serially) documents the
execution scope and the independent-reachability boundary.

## Discover targets and actions

Target and action `description` values are optional discovery metadata. Adding
them does not change manifest compatibility, Ninja progress text, or build
execution: Ninja progress continues to use the referenced rule's `description`.
Existing manifests without these fields remain valid.

Use the new command to inspect the selected manifest:

```sh
netsuke help targets
```

The command honours the usual manifest-selection options, including `--file` and
`-C/--directory`. It uses manifest-query rendering: discovery metadata and
structural rule selectors needed for graph validation are rendered, while
`command` and `script` recipe bodies are skipped. Consequently, build-only
helpers in those skipped bodies are not evaluated, and do not make discovery
fail. The command remains side-effect-free: it prints actions and targets
without running recipes or creating build outputs.

Queries allow only the lexical path filters `basename`, `dirname`,
`with_suffix`, and `relative_to`, the collection filters `uniq`, `flatten`, and
`group_by`, and the clock-independent `timedelta` function. Query mode rejects
direct evaluation of all query-disabled, host-dependent, or side-effecting
helpers in this surface, including `env()` and `glob()`, file tests, filesystem
metadata filters such as `size` and `linecount`, `hash`, `digest`, `contents`,
`realpath`, and `expanduser`, executable discovery through `which` and
`command_available`, network and command helpers (`fetch`, `shell`, and
`grep`), and the clock-dependent `now()` function. When a `when` expression
uses a query-disabled helper, its action or target is retained as a conditional
entry instead of failing discovery. A `when` expression that evaluates to
`false` still excludes its entry.

Human-readable output marks a conditional entry with the localized
`[◇ conditional]` marker when emoji output is enabled, or `[? conditional]` in
the ASCII theme. JSON output includes the boolean `conditional` JSON field.
Existing manifests need no changes, but integrations must not treat a
conditional catalogue entry as a confirmed selected entry. Full rendering for
`build`, `generate`, and normal manifest output retains the full standard
library and its existing semantics; these restrictions apply only to
manifest-query rendering. See the detailed [help targets documentation]
(users-guide.md#generate-and-inspect-artefacts). Add `--json` to receive the
versioned JSON result document; its `result.command` is `help-targets`. The
command and the new descriptions are beta-series additions and remain subject
to the stability caveat above.

## Diagnostics

Ninja subprocess spans and warn events carry two bounded fields,
`env_override_count` and `path_overridden`, so environment-caused failures are
diagnosable from logs. Variable names and values are never recorded;
`CommandEnv`'s `Debug` output is redacted to the same counts.
