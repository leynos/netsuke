# Migrating to v0.1.0

## Who should read this

Read this guide when upgrading within the v0.1.0 beta series. It records
changes through beta4 and migration notes for the next pre-release, beta5.
Manifest authors, source-build users, and Rust API callers should follow the
sections that match the features they use.

## Impact at a glance

| Priority          | Area                       | What to do                                                          |
| ----------------- | -------------------------- | ------------------------------------------------------------------- |
| Required          | Checkout source builds     | Use the pinned Rust toolchain before building from a checkout.      |
| Required          | Linux checkout builds      | Install the pinned `mold` linker before building from a checkout.   |
| Review if enabled | Manifest scheduling        | Opt in only where immediate console output is useful.               |
| Required          | `Target` literals          | Add `description` and `console` to every literal.                   |
| Required          | `BuildEdge` literals       | Add `output_visibility` to every literal.                           |
| Review if enabled | Rust `Action.pool` choices | Move console scheduling onto the edge; keep custom pools off it.    |
| Required          | `NinjaGenError` matches    | Add arms for the two new console variants.                          |
| Optional          | Existing manifests         | Omit `console` to retain ordinary scheduling; it defaults to false. |
| Optional          | Scripts parsing stdout     | Tolerate the forwarding-failure warning on stdout.                  |

_Table 1: migration obligations for the beta series and console-pool option._

This guide covers the changes released in the v0.1.0 beta series through
v0.1.0-beta4, plus migration notes for the next pre-release.

The v0.1.0-beta3 release added the injectable child environment (`CommandEnv`),
the named Ninja request types, narrow process options (`NinjaProcessOptions`),
target/action discovery through `description` and `netsuke help targets`, and
cached configuration discovery. It also introduced manifest-relative glob
expansion, UTF-8 CLI path boundaries, the Windows PowerShell-default legacy
recipe contract, and brace-only `{{ ins }}` and `{{ outs }}` path markers.

The v0.1.0-beta4 release adds manifest evaluation budgets, optional `env()`
allow and block lists, and bounded file-reading filters. It also contains
breaking changes: default executable discovery searches only `PATH`, `$in` and
`$out` in `script:` recipes are shell variables, the primary project
configuration can no longer widen the operator's fetch policy, every fetch
redirect is revalidated against that policy, mapping-root manifests must declare
`netsuke_version: "1.0.0"`, and checkout builds on Linux link with `mold` by
default.

The schema declaration is checked before any Jinja evaluation, so a missing,
malformed, or unsupported version fails the load rather than reaching a
template helper. See
[Declare the manifest schema version](#declare-the-manifest-schema-version).

Beta4 also adds the opt-in `netsuke --json help targets --edges` graph export.
`--edges` requires the root-level `--json` option before `help`. Omitting
`--edges` preserves the existing catalogue output, and existing manifests need
no changes. See the
[graph export reference](users-guide.md#export-dependency-edges-and-raw-recipes).

The next pre-release (`v0.1.0-beta5`) adds the opt-in `console` field for
actions and targets. It selects Ninja's special console pool for that build
edge. Ninja runs one console edge at a time; other edges keep running, but
Ninja buffers their output and progress while a console edge is active. Ninja
inherits rule-level bindings by edge, so a shared rule that sets
`pool = console` would otherwise widen console scheduling to every edge
generated from it; Netsuke clears that inherited binding on edges that did not
opt in, which is why only the edges a manifest marks with `console: true` run
in the console pool. Netsuke forwards Ninja's output as it reads it outside
JSON mode, but recipes receive a pipe rather than a PTY and may buffer their
own output. See the [console-pool guidance][console].

[console]: users-guide.md#stream-long-running-output-with-the-console-pool

Most existing manifests remain compatible, and callers of the convenience
wrappers retain their child-process behaviour. Manifests using Jinja `glob()`
must use shell-inert matched paths. The cached configuration discovery API is a
breaking change for callers of the unstable Rust API; ordinary CLI users need
no action. Non-UTF-8 build-file and working-directory paths are rejected at
their input boundary.

Rust callers that construct `Target` with a struct literal must add the new
`description` field (set it to `None` or `Some(...)`) and the `console` field
(set it to `false` or `true`). Callers that construct `BuildEdge` with a struct
literal must add `output_visibility` for the same reason: both types expose
public fields and neither is `#[non_exhaustive]`. Use
`EdgeOutputVisibility::Buffered` for ordinary edges and `Immediate` when the
manifest's `console: true` behaviour is required.

`NinjaGenError` gains two variants, `ConsoleDependencyOnly` and
`ConsolePoolConflict`, because generation now rejects a dependency-only edge
that asked for console scheduling and a console edge whose action selects a
different pool. The enum is public and is not `#[non_exhaustive]`, so a caller
that matches it exhaustively must add an arm for each variant, or a `_` arm.
Callers that only format or propagate the error need no change.

The edge, not the action, now selects the console pool. An `Action::pool` of
`Some("console")` no longer gives a `Buffered` edge console scheduling, because
Netsuke clears the inherited rule-level binding on edges that did not opt in. A
caller that relied on the action pool alone must set
`output_visibility = EdgeOutputVisibility::Immediate` to keep it. `Immediate`
and a custom action pool are mutually exclusive: generation rejects a pool
other than `console` on a console edge, so clear the pool or set it to
`console`. `Action::pool` has no manifest counterpart; a manifest expresses the
same choice with `console: true`.

Deserialized manifests remain compatible because `console` defaults to `false`.
Callers constructing `NinjaBuildRequest` or `NinjaToolRequest` must replace
`cli: &cli` with `options: &options`. Callers that read `BuildGraph::targets`
must use the graph accessors instead; see
[Resolve build-graph outputs through the accessors](#resolve-build-graph-outputs-through-the-accessors).
Every other addition is opt-in.

## Select the pinned Rust toolchain

Source builds from a checkout require the dated nightly pinned in
`rust-toolchain.toml`. Inside the checkout, `rustup` selects that toolchain
automatically, so no command-line argument is required. Registry installs run
outside the checkout and must select the same nightly explicitly:

```sh
rustup toolchain install nightly-2026-10-22
cargo +nightly-2026-10-22 install netsuke-build
```

## Install the linker the build standard uses

Builds that run inside a checkout now take a committed build standard, because
`.cargo/config.toml` is auto-discovered rather than opt-in. There is nothing to
enable for the compiler part: the parallel `rustc` front end applies on every
platform, and the pinned nightly above supplies the compiler it needs.

Linux is the exception. That configuration also passes `-fuse-ld=mold`, and
`mold` is a separate program the compiler must be able to find. On Linux,
install it before the first build:

```sh
make install-build-tools
export PATH="${BUILD_TOOLS_PREFIX:-$HOME/.local}/bin:$PATH"
```

The installer unpacks the pinned release into `$(BUILD_TOOLS_PREFIX)/bin`,
`~/.local/bin` by default, and does not edit any shell profile; the `PATH`
export above is therefore a manual step, and one to keep for any later shell.
The Make targets add that directory for the recipes they run, so a build driven
by `make` needs no export. A distribution `mold` on `PATH` also works for a
local install, though the development gates additionally check the version
against `tools/mold/VERSION`.

Without it the build fails at link time rather than falling back quietly: the
linker is named explicitly, so gcc reports that it cannot find `mold` and
stops. macOS and Windows name no linker and need no extra prerequisite; so does
any platform once `.cargo/config.toml` is removed.

Two build shapes are deliberately excluded and keep the platform linker: a
release or packaging build, whose output ships, and a coverage build, whose
output is a measurement. Both assign `RUSTFLAGS` at the point they run, which
displaces every table in the configuration file.

The opt-in `install-dev-fast` and `dev-fast-check` targets are gone. The
acceleration is the default now, so there is nothing to opt into:
`install-build-tools` installs the pinned linker, and `check-build-tools`
verifies it is present and is the pinned version. Every gate target depends on
that check, so a missing prerequisite is reported as such rather than surfacing
later as a linker error.

The [users' guide](users-guide.md#install-netsuke) covers the same ground in
more detail, and the
[developers' guide](developers-guide.md#the-build-standard) records the
standard's rationale and its exclusions.

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

| Area                         | Impact                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Where to read more                                                                                                       |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Convenience wrappers         | Unchanged. `run_ninja` and `run_ninja_tool` behave exactly as before, inheriting the process environment.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [Users' guide](users-guide.md)                                                                                           |
| Child environment            | New opt-in `netsuke::runner::CommandEnv` carries additive variable overrides and an injected `PATH` for Ninja child processes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | [Developers' guide](developers-guide.md#ninja-invocation-with-an-explicit-environment)                                   |
| Request types                | New `netsuke::runner::NinjaBuildRequest` and `netsuke::runner::NinjaToolRequest` name the program, `NinjaProcessOptions`, build file, targets or tool, a child environment, and a required `stderr_mode: StderrMode` policy for the `*_with` run functions.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [Developers' guide](developers-guide.md#ninja-invocation-with-an-explicit-environment)                                   |
| Cached CLI configuration API | Breaking for callers of the unstable Rust API: use the opt-in cached discovery flow with `ConfigEnvProvider`; `ConfigStdEnvProvider` supplies process-backed access.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [Developers' guide](developers-guide.md#cached-configuration-merge)                                                      |
| Timing output                | Existing `VerboseTimingReporter::new` keeps its stderr sink; Rust callers can opt into an owned `Write + Send` sink with `with_writer`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [Developers' guide](developers-guide.md#verbose-timing-sink)                                                             |
| Glob expansion               | Parent-relative patterns such as `glob('../shared/*.h')` now expand. The Jinja helper rejects matched paths that are not portable unquoted shell words. Metadata checks use a capability rooted at the pattern's longest literal directory prefix; missing or non-directory prefixes return no matches, and unresolvable symlink matches are skipped.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [Users' guide](users-guide.md) and [ADR-0010](adrs/adr-0010-scope-the-glob-metadata-capability-to-the-literal-prefix.md) |
| Executable discovery         | **Breaking:** default `which` and `command_available` no longer recursively discover workspace executables when `PATH` is empty or unset. Opt in with `cwd_mode='workspace-recursive'` only for trusted workspaces.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [Users' guide](users-guide.md#template-helpers)                                                                          |
| Command recipes              | On Windows, legacy scalar commands, lists, and scripts use Windows PowerShell by default; YAML command lists remain opt-in, ordered, and fail-fast.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [Windows legacy recipe contract](users-guide.md#windows-legacy-recipe-contract)                                          |
| Ninja text escaping          | Write shell dollars normally. In `command:` and `script:` recipes, `$in`, `$out`, `$ins`, and `$outs` are shell variables, so Ninja emits their dollars doubled and the selected shell receives their ordinary single-dollar spelling. `{{ ins }}` and `{{ outs }}` are the only Netsuke path markers. A manifest that used script `$in` or `$out` for Netsuke path substitution must use `{{ ins }}` or `{{ outs }}` instead. Spaces in build and default-target paths are escaped for Ninja as a `$` followed by a space, so whitespace-containing outputs remain valid. Paths containing `\|` or control characters remain rejected, as do paths containing `$` or colon, which Ninja can escape but Netsuke does not yet accept. Emitted metadata is escaped separately: a literal dollar in a description, `depfile`, `deps`, or `pool` is doubled, so `depfile: $out.d` is written as `$$out.d` and Ninja reads the literal name. Newline, carriage-return, and NUL metadata values are rejected. | [Users' guide](users-guide.md#review-the-safety-boundary)                                                                |
| Manifest discovery           | Optional target/action `description` values are shown by the new `netsuke help targets` command. Manifests without them and existing build output are unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [Users' guide](users-guide.md)                                                                                           |
| Manifest schema version      | **Breaking:** a mapping-root manifest must declare `netsuke_version: "1.0.0"`. Missing, non-string, malformed, and unsupported declarations fail during parsing, before any Jinja evaluation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | [Declare the manifest schema version](#declare-the-manifest-schema-version) and [Users' guide](users-guide.md#declare-the-schema-version) |
| Serial dependencies          | New opt-in `dependency_order: serial` runs an action or target's direct `deps` list in declaration order.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [Serial dependency ordering](users-guide.md#run-direct-dependencies-serially)                                            |
| Graph export                 | New opt-in `netsuke --json help targets --edges` output exposes a versioned graph beside the existing catalogue; root-level `--json` must precede `help`; omitting `--edges` preserves current output, and manifests need no changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [Graph export reference](users-guide.md#export-dependency-edges-and-raw-recipes)                                         |
| Fetch redirects              | Every redirect destination is now evaluated against the network policy before it is requested, so a redirect can no longer reach a host, scheme, or address the policy refuses. Chains stop after five redirects, a repeated destination is refused as a loop, and URL credentials are removed when the origin changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [Users' guide](users-guide.md#network-fetch-policy) and [ADR-0023](adrs/adr-0023-revalidate-every-fetch-redirect.md)     |
| Manifest environment access  | New optional exact-name `env()` allow and block lists. Existing manifests retain default-allow behaviour when neither list is configured; an active allowlist enables default-deny and a block always wins.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [Users' guide](users-guide.md#control-manifest-environment-access)                                                       |
| File-reading filters         | The `contents`, `linecount`, `hash`, and `digest` filters now read under one 8 MiB default byte budget; a symlink final component is rejected unless `follow_symlinks=true` opts in, while FIFOs and devices are rejected outright, and per-call `max_bytes` can only narrow the budget.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | [Configure file reading limits](users-guide.md#configure-file-reading-limits)                                            |
| Script shell variables       | **Breaking:** `$in` and `$out` in `script:` recipes are shell variables, as they already were in `command:` recipes. Use `{{ ins }}` and `{{ outs }}` for Netsuke path substitution.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [Script shell variables](#replace-script-in-and-out-path-substitution)                                                   |
| Build graph                  | **Breaking for callers of the unstable Rust API:** each logical build edge is stored once and indexed by output through `EdgeId`; `BuildGraph::targets` is no longer public.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | [Graph accessors](#resolve-build-graph-outputs-through-the-accessors)                                                    |
| Cycle detection              | **Breaking:** cycles through `sources`, `deps`, or `order_only_deps` are rejected during IR validation, even when Ninja would not be asked to build the affected target. Remove latent prerequisite cycles; the whole manifest is checked before Ninja starts.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | [Dependency and build-graph semantics](users-guide.md#dependency-and-build-graph-semantics)                              |
| Release packaging            | The release workflow validates the packaged candidate offline before upload: the identity, the archive digest, and the packaged manifest must agree, and a dry run must record no release tag.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | [Release packaging validation](#release-packaging-validates-its-candidate-offline)                                       |
| Release upload               | The publishing lane uploads only the assets this run declared: producer records are bound to the candidate commit and the workflow run, and a stale, missing, doubled, or undeclared release-shaped artefact is refused before any release asset is transferred. The job may already have checked for or created the draft release, which touches the release API; selection and declaration come after that and before the upload. An attached asset with the recorded digest is skipped; one with different bytes stops the run and is never replaced.                                                                                                                                                                                                                                                                                                                                                                                                                                                | [Release uploads](#release-uploads-only-the-assets-this-run-declared)                                                    |
| POSIX heredoc delimiters     | **Breaking:** a delimiter form whose quote removal Netsuke cannot reproduce now fails closed instead of being copied: a missing delimiter, a leading unquoted `#`, an unclosed quote, a trailing backslash, a line continuation, and an unescaped `$` or backtick outside single quotes. Quote the literal part instead, for example `<<'$DELIM'`; quoting any part of the delimiter also stops expansion in the body, so keep an unquoted literal delimiter such as `<<END` when the body must expand.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [Heredoc delimiters](#update-heredoc-delimiters-the-tracker-cannot-model)                                                |

## Remove order-only dependency cycles

Cycle validation now includes `order_only_deps` as well as `sources` and
`deps`. An order-only dependency does not trigger a rebuild when it changes,
but the dependent still must wait for it. Netsuke checks the whole manifest
before running Ninja, so a cycle in a target that is not currently requested
can still make `generate` or `build` fail with `Circular dependency detected`.

Remove the cycle by changing the dependency graph so that no target depends,
directly or transitively, on itself. A multi-output target is one build step:
referencing a different output path does not break the cycle. See the tested
[order-only cycle example](users-guide.md#dependency-and-build-graph-semantics)
and [ADR-0068](adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md).

## Bound manifest evaluation

The v0.1.0 release applies resource limits while Netsuke parses, expands, and
renders a manifest, including in `help targets` queries. The defaults are
1,000,000 instructions per evaluation, 100,000,000 instructions per manifest, 1
MiB per rendered value, 16 MiB of rendered output, 4 MiB of template source,
10,000 `foreach` values, and 50,000 expanded entries.

The limits use the existing configuration precedence. Operators can set or
narrow them through trusted configuration, environment variables, or CLI flags;
project configuration and its `extends` chain may narrow an established limit
but cannot widen it. Invalid or non-positive values now fail configuration
loading. See the [resource budgets](users-guide.md#resource-budgets) section
for the complete key, environment-variable, and flag reference, along with the
failure diagnostic.

## Declare the manifest schema version

A manifest whose root is a mapping must now declare the schema it is written
against. The current reader accepts exactly `1.0.0`:

```yaml
netsuke_version: "1.0.0"
targets:
  - name: hello
    command: echo hello
```

The reader rejects a missing declaration, a value that is not a string, a
string that is not a semantic version, and a version outside the supported
range. All four rejections happen immediately after the YAML is parsed and
before any variable, macro, `foreach`, `when`, or template helper runs, so an
unsupported manifest cannot evaluate `env()`, read a file, or run a shell
command on the way to its own rejection.

Only the literal schema release is admitted. A version inside the supported
range that is not `MAJOR.MINOR.0` — `1.0.1`, `1.0.0-rc.1`, or a value carrying
build metadata — is reported as non-canonical rather than as unsupported,
because no future reader could start accepting it. The diagnostic names the
declared and supported schemas, and reports the
`netsuke::manifest::schema_version` code.

In human output that code appears in the diagnostic header. In `--json` output
the top-level `code` is `netsuke::manifest::parse` and the schema code appears
inside an entry of the `causes` array. See
[Declare the schema version](users-guide.md#declare-the-schema-version) for the
full behaviour and the diagnostic shape.

Before changing `netsuke_version`, verify that the reader supports the schema
features used by the manifest. Changing the declaration alone does not migrate
unsupported syntax.

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
[developers' guide](developers-guide.md#verbose-timing-sink) documents the
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

The beta3 release tightens the Jinja `glob()` helper. A manifest that expands a
matched path containing whitespace, control characters, or shell punctuation
now fails during manifest loading. Beta3 accepted ASCII letters, digits, `/`,
`:`, full stop, underscore, hyphen, and comma. The
[beta5 subsection](#reject-commas-in-glob-matched-paths) drops the comma from
that set, so only ASCII letters, digits, `/`, `:`, full stop, underscore, and
hyphen remain accepted. This beta3 change prevents a checkout filename from
becoming executable shell syntax when a `foreach` item is interpolated into a
`command` or `script`.

Published beta2 does not reject these non-portable unquoted paths.

Rename affected files to use the accepted character set, or change the manifest
so filesystem-derived paths do not cross the Jinja command-template boundary.
The Rust `manifest::glob_paths` query retains its previous contract and
continues to return any matching UTF-8 file path because its callers own their
downstream escaping boundary.

### Reject commas in `glob()` matched paths

The next pre-release (`v0.1.0-beta5`) also rejects a matched path that contains
a comma. PowerShell cmdlet argument parsing treats a bare comma as an array
separator, so a pattern such as `foreach: glob('assets/*.txt')` could turn one
matched path into several arguments of a trusted recipe. Such a manifest now
fails during manifest loading with `characters that require shell quoting`.

Rename the file so the comma is gone, or remove the matching `glob()` call or
narrow its pattern so the path is excluded. The rejection happens while
`glob()` is evaluated, whatever the recipe or the selected shell, so passing
the path through the `shell_quote` or `shell_join` helper does not rescue it.
See [Discover files with `glob`](users-guide.md#discover-files-with-glob) for
the accepted characters.

Relative manifest glob patterns, including parent-relative patterns, now
resolve from the manifest directory or workspace root rather than the process
working directory. Relative results retain their pattern-relative spelling
after the workspace base is stripped; absolute patterns remain absolute. The
manifest parse boundary supplies this base, so glob expansion does not read or
mutate process-global working-directory state. Callers of the Rust
`glob_paths(pattern, base)` API can supply the same base explicitly.

## Opt into recursive workspace executable discovery

Published beta3 recursively searched the workspace after a default `which` or
`command_available` lookup missed with an empty or unset `PATH`. That allowed a
checkout-controlled executable to become the resolved command without the
manifest explicitly naming the workspace search.

The default `cwd_mode="auto"` now searches only `PATH` and reports the command
missing when `PATH` is empty or unset. `cwd_mode="always"` adds only the
workspace root/current directory, while `cwd_mode="never"` excludes it.

If a trusted manifest deliberately needs recursive discovery, change that call
to `which('tool', cwd_mode='workspace-recursive')` or
`command_available('tool', cwd_mode='workspace-recursive')`. Treat this as a
trust-boundary decision: the recursive mode may resolve an executable supplied
by the checkout. `NETSUKE_WHICH_WORKSPACE=0` remains a kill-switch for that
explicit mode.

## Policy enum parsing

The public policy enums no longer implement `clap::ValueEnum`. This removes the
Clap parsing dependency from the domain configuration schema. Rust callers that
parse `ColourPolicy`, `ProgressPolicy`, `EmojiPolicy`, or `AccessibilityPolicy`
should use `str::parse` or `FromStr`; accepted spellings remain
case-insensitive and are listed in the users' guide's
[policy values and parsing](users-guide.md#policy-values-and-parsing) section.
Clap-specific validation and help metadata remain an internal CLI concern.

## Colour, emoji and progress are resolved once

Netsuke now resolves colour, emoji, progress, accessibility and JSON into a
single display plan before any output is written. Existing command lines and
configuration files keep working; the change is visible in behaviour and in the
Rust API.

Behaviour users should know about:

- `--color always` styles Netsuke's own tracing messages, Miette diagnostics,
  and Clap help and version output even when the stream is a pipe. Generated
  artefacts and forwarded subprocess output stay unstyled.
- `--color always` overrides colour suppression from both `NO_COLOR` and
  `TERM=dumb`, so Netsuke's own output is still styled. Output mode is a
  separate decision: with `--accessibility auto` a dumb terminal still selects
  accessible mode, because it cannot render live redraws. Use
  `--accessibility off` to force standard mode against it; live redraws stay
  disabled.
- `--accessibility on` and `--accessibility off` name the output mode directly
  and take precedence over every inferred choice.
- An empty `NO_COLOR` no longer suppresses colour. The variable counts as set
  only when it has a non-empty value, which is what the `NO_COLOR` convention
  asks for; a bare `NO_COLOR=` in a shell or CI environment now leaves colour
  and accessible mode alone. Previously any presence of the variable was enough.
- Usage errors and the human-readable configuration-load error stay plain under
  every policy, because the configuration that would select a policy has not
  been read yet. The structured `tracing` events emitted beside them do follow
  `--color`.

See [Colour](users-guide.md#colour) and
[Accessible output](users-guide.md#accessible-output) for the full precedence
rules.

Rust callers of the output API must migrate:

- `netsuke::output_prefs::resolve_from_theme` is removed. Use
  `resolve_from_signals` with captured `EnvSignals`, or the existing `resolve`
  and `resolve_with` when an environment seam is what you hold.
  `resolve_from_signals` takes the reduced signals directly so captured facts
  are not turned back into environment strings and reduced a second time.
  `resolve_from_theme_with` remains available for callers that hold a theme
  preference and an environment reader rather than captured signals.
- `netsuke::display_plan` is new. `resolve_display_plan` is a `const fn` over
  `DisplayPolicies` and `TerminalFacts` that returns the whole `DisplayPlan`,
  so a caller can make every presentation decision once instead of re-deriving
  it per adapter.
- `netsuke::output_mode::resolve` is removed, because it read the environment
  itself and so could not take part in the single resolution above. Capture
  `EnvSignals::read` with the caller's own environment seam and pass it to
  `EnvSignals::resolve_output_mode`, or use `resolve_with`, which keeps its
  signature and now delegates to those two steps.
- `netsuke::runner::run` and `run_with_ninja_program` take a `DisplayPlan` in
  place of the `OutputPrefs` they used to accept. Pass the plan the caller has
  already resolved; the runner reads the reporter, progress, and colour
  decisions from it rather than re-deriving them.
- `netsuke::cli::parse_with_localizer_from` takes a third argument, the Clap
  `ColorChoice` for help and version output. `netsuke::cli::ColorChoice` is
  re-exported so a caller need not depend on Clap directly.
- `Cli::display_policies` is new, and `AccessibilityPolicy::explicit_override`,
  `EmojiPolicy::unicode_override` and `ProgressPolicy::is_enabled` expose the
  policy values a caller needs to build `DisplayPolicies` by hand.

## Opting into ordered command lists

Existing scalar `command` recipes remain valid, and command lists remain
opt-in, ordered and fail-fast. Their `command_isolation` mode defaults to
`shared`, so entries share one shell and its state. Set
`command_isolation: isolated` to start each entry from the action's initial
working directory and environment. POSIX and Bash use a subshell per entry;
Windows PowerShell uses a child process per entry. Isolated mode has a distinct
action identity; existing manifests need no migration. See
[Rules and recipes](users-guide.md#rules-and-recipes) and the
[Windows legacy recipe contract](users-guide.md#windows-legacy-recipe-contract)
for the syntax and shell-specific behaviour.

## Replace script `$in` and `$out` path substitution

Published beta3 lowered `$in` and `$out` in `script:` recipes to Ninja's own
rule variables, so they expanded to the edge's input and output paths. In beta4
they are ordinary shell variables in both `command:` and `script:` recipes:
Netsuke doubles their dollars for Ninja, and the selected shell receives `$in`
and `$out` unchanged. A script that relied on the old expansion now reads
whatever those shell variables hold, which is usually nothing.

Replace each such use with the Netsuke path markers:

```yaml
netsuke_version: '1.0.0'
targets:
  - name: output.txt
    sources: input.txt
    script: |
      cat {{ ins }} > {{ outs }}
defaults: [output.txt]
```

`{{ ins }}` and `{{ outs }}` are shell-quoted for the selected route, so no
further quoting is needed. See the
[users' guide safety boundary](users-guide.md#review-the-safety-boundary) for
the full marker contract.

## Separate a substitution closer and a following `#`

Netsuke now rejects a POSIX or Bash `command:` or `script:` recipe when a `)`
that closes a `$(...)` or `$((...))` substitution is immediately followed by
`#` and the substitution source contains the `case` reserved word. A `)` that
closes a substitution stays part of the surrounding word, so `#` after it is
word data; a `)` that terminates a `case` pattern is an operator, so `#` after
it starts a comment. The scanner cannot tell the two apart without the `case`
grammar, so it fails closed rather than encode a later path for the wrong shell
context.

The remedy depends on what the `#` means in your recipe.

- When `#` starts a comment, separate it from the closer with a space, a tab,
  or a newline. The shell then sees the comment it always did, and the text of
  that comment stops reaching the command's arguments.
- When `#` is literal word data, keep it inside a quoted region or escape it as
  `\#` in an unquoted word. Both forms leave the word the shell receives
  unchanged; only whitespace or an escape may separate it from the closer.

This recipe triggers the rejection:

```yaml
netsuke_version: '1.0.0'
targets:
  - name: output.txt
    sources: input.txt
    script: |
      echo "$(case x in x) :;; esac)# trailing comment"
      echo {{ ins }} > {{ outs }}
defaults: [output.txt]
```

The `#` there is word data: the double quote that opens before `$(` closes after
`comment`, so the literal text reaches `echo`. This rewrite clears the
rejection while keeping that output: it closes the double quote after the
closer and reopens a single-quoted region for the literal hash.

```yaml
      echo "$(case x in x) :;; esac)"'# trailing comment'
```

The argument is unchanged, so the recipe prints what it printed before. Placing
a space before the `#` also clears the rejection, as in
`... esac)" # trailing comment`, but it turns the text into a real comment:
that is the correct form when the recipe never meant to echo it, and the wrong
form when it did.

A separate unquoted recipe takes the other rewrite, escaping the `#`. Escaping
is not an option inside the quoted recipe above, because a backslash inside
double quotes does not escape `#`:

```yaml
      echo $(case x in x) :;; esac)\#literal
```

Without the escape this line is rejected, because `#literal` sits directly
after the closer. With it, the `case` still emits nothing and the word stays
`#literal`, so the line prints `#literal` rather than starting a comment.

The check scans raw recipe source, so it also rejects a `case` that appears
inside a string or a comment within the substitution. Anything that separates
the `#` from the closer, including the `\#` escape, lifts the rejection whether
or not the `case` was a real keyword. A rejection therefore means "the scanner
cannot prove which `)` this is", not "this recipe is unsafe".

Heredoc bodies follow the same inert-text rule. A marker in a heredoc body in a
`script:` recipe is copied literally, while a marker in the delimiter word is
interpolated. A recipe that needs a marker lowered on a body line must close
the heredoc first, so the marker stands in ordinary script text instead.

The delimiter word now follows the same token rule as the comment boundary
above. Only an ASCII space, tab, or newline, or one of `;`, `&`, `|`, `(`, `)`,
`<`, and `>`, ends it. A non-ASCII space, carriage return, vertical tab, or
form feed stays part of the delimiter, so the closing line must spell the
complete word. A recipe that wrote a delimiter such as `EOF` followed by a
no-break space and `X` used to close its body at a line spelling `EOF`; that
line is body text now, and the recipe must either spell the whole delimiter on
the closing line or change the delimiter.

Recipes that do not place `#` directly after a substitution closer are
unaffected by the ambiguity rejection. The heredoc delimiter rule above applies
independently of `)#`, so a recipe whose delimiter carries a non-ASCII space
changes behaviour whether or not it places `#` after a substitution closer. The
[users' guide safety boundary](users-guide.md#review-the-safety-boundary)
describes the full comment and substitution rules.

## Update heredoc delimiters the tracker cannot model

Netsuke tracks POSIX heredoc bodies so it never lowers a path marker into text
the shell still expands. The tracker now reproduces the shell's own word and
line rules and fails closed instead of copying a delimiter it cannot model. A
recipe that used one of the rejected forms below now fails at IR lowering with
an `ir.invalid_command` diagnostic naming the reason, for example
`unsupported heredoc delimiter syntax: unclosed quote`.

- **Missing delimiter**, or a leading unquoted `#` that the shell reads as a
  comment: `cat <<` and `cat <<#EOF`. Write the delimiter immediately after
  `<<` or `<<-`, and keep comments on their own line.
- **Unclosed quote**: `cat <<'EOF`. Close every quote in the word, as in
  `cat <<'EOF'`.
- **Trailing backslash**, or a backslash-newline continuation whether it is
  unquoted or inside double quotes: `cat <<EOF\`, an unquoted delimiter split
  across two lines, and `cat <<"EO\` followed by a newline and `F"`. Write the
  whole delimiter on one line.
- **Unescaped `$` or backtick outside single quotes**: `cat <<$DELIM`,
  `` cat <<`delim` ``, `cat <<"$DELIM"`, and `` cat <<"`delim`" ``. The shell
  performs quote removal on a delimiter word and no expansion, so these words
  are valid shell. Netsuke rejects them because its tracker does not model an
  expansion in a delimiter word, and double quotes do not change that. Use an
  unquoted literal delimiter, such as `cat <<END` with a terminating `END`
  line, when the body needs parameter expansion, command substitution, or
  arithmetic expansion. Quoting or escaping the delimiter, as in
  `cat <<'$DELIM'`, `` cat <<'`delim`' ``, or `cat <<\$DELIM`, suppresses those
  expansions throughout the body, because the shell treats a delimiter word
  with any quoted part as quoted. Use a quoted or escaped delimiter only when
  the body must stay literal.

The comment and unclosed-quote entries also apply to a rendered delimiter: if a
marker inside the delimiter lowers to a path that would introduce one of those
forms, Netsuke rejects the substitution too. A rendered delimiter must also
stay one word. A marker expands to every bound path, joined by blanks, so a
marker that binds more than one path renders a delimiter the shell would split;
Netsuke rejects that rather than guessing which word the shell keeps. Use a
literal delimiter when the word must stay unambiguous.

Three rules are unchanged in intent and now stated explicitly. A delimiter word
ends only at a POSIX blank, a newline, or a shell operator, so Unicode
whitespace such as a no-break space stays inside the delimiter and a line
reading only the visible prefix does not end the body. A `<<` inside a `$(( ))`
arithmetic expansion is a left shift, not a heredoc, so arithmetic expressions
pass through unchanged. A `#` inside a `${ }` parameter expansion is expansion
syntax rather than a comment — `${name# #}` is one such pattern removal — so a
heredoc declared later on that line is still tracked.

See the
[users' guide safety boundary](users-guide.md#review-the-safety-boundary) for
the full heredoc contract.

## Resolve build-graph outputs through the accessors

The intermediate build graph now owns each logical build edge once, however
many outputs it declares, and indexes every output alias, explicit and
implicit, to a stable `EdgeId`. The public `BuildGraph::targets` map, which
held one cloned edge per output, is gone. Rust callers of this unstable API
should use:

- `BuildGraph::edges` to iterate each logical edge exactly once;
- `BuildGraph::output_paths` to iterate every output alias, explicit and
  implicit;
- `BuildGraph::target_for_output` to fetch an output together with its edge;
- `BuildGraph::edge_id_for_output` to obtain the edge's stable identity.

Code should count logical edges with `edge_count()`. Note that `output_paths()`
and `output_count()` also include implicit outputs, which the old `targets` map
never held. Code that needs only the declared explicit outputs, as the old map
provided, should iterate `edges()` and read each edge's `explicit_outputs`.
Manifests, generated Ninja files, and graph exports are unaffected.

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
found failure. `generate` resolves the selected runtime before graph generation
and writes no plan when resolution fails; `help targets` does not run recipes
and does not require the optional runtime.

In the default route, write `$name` for a PowerShell variable and `$env:NAME`
for an environment variable. PowerShell parses the braced variable name in
`${VAR:-default}`, but does not perform POSIX default-value expansion. The
v0.1.0 dollar-escaping fix means these are ordinary, single dollars, not
Ninja-escaped `$$` forms. With `command_isolation: shared` (the default),
ordered lists share one PowerShell process. Netsuke checks `$LASTEXITCODE`
immediately after each generated list entry, so a non-zero status or
terminating error stops the list before a later entry can overwrite it.
Multiple native commands inside one entry are not individually instrumented.
Variables, environment assignments, and current-directory changes persist
between entries in this mode. With `command_isolation: isolated`, each ordered
entry runs in a child process starting from the action's initial working
directory and environment. Netsuke checks each child's exit status before
starting the next entry, preserving fail-fast behaviour. Each scalar, script,
action, and target has a fresh shell process.

In both PowerShell `command:` and `script:` recipes, `{{ ins }}` and
`{{ outs }}` are valid only at unquoted insertion sites. If a marker is inside
an existing quoted string, remove the surrounding quotes and move the marker to
an unquoted site; Netsuke supplies its own standalone single-quoted path
literal. A marker immediately beside any PowerShell single-quote character,
including typographic forms, or another marker is also rejected because
standalone single-quoted literals could merge with adjacent string or argument
text. Markers in single- or double-quoted strings, here-strings, `$()` command
substitutions, immediately after a backtick, or in the raw native command
region after `--%` are rejected. The script scanner has no inert PowerShell
comment regions: markers in ordinary line-comment text remain eligible for
interpolation, while scanner-sensitive syntax after a comment boundary or any
mid-token `#` can make a later marker fail closed. This includes an apostrophe,
quotes, backticks, or substitution syntax. The `$in` and `$out` names remain
PowerShell shell variables, not Netsuke path markers.

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

### Bind the interpreter through the Rust API

v0.1.x closes the same search-path hazard for embedders. The library entry
points `generate`, `generate_into`, `generate_with_shell`, and
`generate_bundle` resolve the platform default before rendering. On Windows
they bind `<System>\WindowsPowerShell\v1.0\powershell.exe` for PowerShell
recipes through the known-folder locator; for Bash, and on every other
platform, they bind nothing. Rendering a recipe that needs an unbound
interpreter returns `NinjaGenError::InterpreterBindingRequired`, while a
dependency-only graph still renders because it has no command.

Call `generate_with_interpreter` or `generate_bundle_with_interpreter` to
supply the binding yourself. Construct the `InterpreterExecutable` with
`InterpreterExecutable::from_absolute_path`, which rejects a relative path and
a path containing a NUL byte. Recipe rendering is a pure function of the graph,
shell, and binding, so the trust decision belongs to the embedder: no
Authenticode signature is checked, and a file replaced at the bound path
between resolution and launch is still executed. Ninja launches whatever file
occupies the bound path at execution time, so the embedder must choose a
location it controls.

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
boundary before runner setup. Worked examples live in the developers' guide's
[Ninja invocation with an explicit environment](developers-guide.md#ninja-invocation-with-an-explicit-environment)
section.

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

## Review the project fetch-policy boundary

The fetch-policy merge distinguishes operator policy from project requests.
System and user configuration, `NETSUKE_` environment variables, and explicit
CLI options are operator-controlled. Every occurrence in the project
`.netsuke.toml` `extends` chain is untrusted by default. Its
`fetch_allow_scheme` and `fetch_allow_host` entries do not widen the operator's
allowlists unless the operator explicitly trusts project policy. Discovery
folds grants from every project-chain occurrence in dependency-first order and
retains the last present `fetch_default_deny` value. Without operator trust,
the grants are ignored and the folded value tightens the operator policy only
when it is `true`. An earlier project `true` does not survive a later present
`false`, and the project value cannot weaken an operator restriction.

Only system or user configuration, `NETSUKE_TRUST_PROJECT_FETCH_POLICY`, or
`--trust-project-fetch-policy` can enable project grants. Trust settings in the
project chain cannot authorize themselves. Project `fetch_block_host` entries
remain cumulative, and a block wins over an allow. If one file is reached from
both project and operator roots, each occurrence retains the authority of its
own root.

To trust a checkout deliberately, set `trust_project_fetch_policy = true` in
system or user configuration, set `NETSUKE_TRUST_PROJECT_FETCH_POLICY=true`, or
pass `--trust-project-fetch-policy`. With that operator opt-in, project scheme
and host grants from every project-chain occurrence append to the operator
values in dependency-first order. The last present project `fetch_default_deny`
value applies directly; when none is present, the operator value remains in
effect. A project cannot grant itself this opt-in because its
`trust_project_fetch_policy` field is ignored in every project-chain occurrence.

### Revalidate every redirected fetch

Before v0.1.0, the HTTP client followed redirects itself, so only the
caller-supplied URL was checked against the network policy. Every redirect
destination is now resolved and evaluated before a connection is opened. A
manifest that previously fetched through a redirect to a host or scheme the
policy refuses now fails with a localized diagnostic instead of silently
reaching the destination.

The redirect behaviour most callers depend on is retained. Statuses `301`,
`302`, `303`, `307`, and `308` are followed, relative `Location` values resolve
against the current URL, GET remains the method at every hop, and the chain
stops after five redirects. A destination that was already visited is refused
as a loop. Credentials in the URL are removed when a redirect changes origin,
and no redirect diagnostic discloses userinfo.

Caching keeps the same identity: the cache entry is still keyed by the original
caller-supplied URL. A cache miss applies the same policy checks to every hop
before storing a body, while a cache hit opens no connection. See
[Network fetch policy](users-guide.md#network-fetch-policy) for the operator
surface. Most manifests need no change, but a manifest that relies on
redirecting to a host outside the allowlist must now grant that host explicitly.

## Control manifest environment access

The Jinja `env()` helper can read process environment variables while rendering
a manifest. The new policy is optional and keeps existing behaviour when no
list is configured. Use arrays in configuration files:

```toml
env_allow_var = ["CI", "PACKAGE_REGISTRY_TOKEN"]
env_block_var = ["AWS_SECRET_ACCESS_KEY", "GITHUB_TOKEN"]
```

The corresponding `NETSUKE_ENV_ALLOW_VAR` and `NETSUKE_ENV_BLOCK_VAR`
environment values are serialized JSON arrays. The command-line options take
one name and can be repeated:

```sh
export NETSUKE_ENV_ALLOW_VAR='["CI","PACKAGE_REGISTRY_TOKEN"]'
export NETSUKE_ENV_BLOCK_VAR='["AWS_SECRET_ACCESS_KEY","GITHUB_TOKEN"]'
netsuke --env-allow-var CI --env-block-var AWS_SECRET_ACCESS_KEY
```

An empty allowlist preserves default-allow behaviour. Once at least one
effective `env_allow_var` entry exists, names outside that list are denied.
`env_block_var` entries deny only their exact names when no allowlist is
active, and always override matching allow entries. The policy has no glob or
pattern matching. `env_allow_var` entries from the primary project file and
every file reached through `extends` in an automatically discovered project
chain are quarantined before policy composition. A checkout-controlled ancestor
therefore cannot widen the operator allowlist or activate default-deny.
`env_block_var` entries remain cumulative because they only restrict access.
Operator chains retain their grants, and ancestors in an explicitly selected
configuration chain retain operator authority; a selected primary project file
remains quarantined.

This policy reduces secret ingress into rendered manifest fields. It does not
make rendered manifest contents safe to log: allowed values still require the
existing output and logging discipline.

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
sidecars. See [ADR-0012](adrs/adr-0012-bound-dyndep-sidecar-retention.md). The
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

## Configure file reading limits

The `contents`, `linecount`, `hash`, and `digest` filters now read through a
single bounded stream. One read is limited to 8 MiB by default, and a final
symlink component is rejected unless the manifest opts in; the opened object
must always be a regular file, so FIFOs and devices are rejected outright.

Raise or lower the ceiling for the whole environment with
`StdlibConfig::with_file_max_read_bytes`. A per-call `max_bytes` argument can
only narrow that budget: a larger request is clamped to the configured ceiling
rather than rejected. Pass `follow_symlinks=true` to any of the four filters
when following a symlink is intended.

The `contents` and `linecount` filters require UTF-8 input and report an error
for other byte sequences; `hash` and `digest` stay byte-oriented and accept any
content. See the [users' guide](users-guide.md#configure-file-reading-limits)
for the full policy and its diagnostics.

## Release packaging validates its candidate offline

The release workflow no longer uploads a packaged crate before checking it. The
`package-crate` job's action validates the retained candidate offline: the
package identity, the single recorded `.crate`, its SHA-256 digest, and the
`Cargo.toml` at the archive's `<package>-<version>` root must all agree with
the run. A pull-request dry run is checked as a rehearsal and must record no
release tag; a publishing run supplies the tag, and the identity must record
exactly that tag. The step never contacts the registry.

Manifests, local builds, and the crate's published contents are unchanged. Only
the release workflow runs this check, and it needs no new input. The
[users' guide](users-guide.md) states the candidate contract and the recovery
path for an interrupted publication.

## Release uploads only the assets this run declared

The publishing lane now hands the downloaded tree to
`./.github/actions/upload-declared-assets` rather than uploading everything it
found. The action binds every producer record to the candidate commit and the
workflow run, assembles the declaration from the staging configuration and
those records, and refuses before any release asset is transferred when a
record belongs to another commit or run, a configured target is missing or
claimed twice, or a staged file that carries a release suffix is not one of the
declared names. The job does reach the release API earlier than that: the draft
release is checked for, and created when absent, before the download. A refusal
therefore stops a release asset from being sent, not the job's first request.

Release-shaped files are named by suffix, and a suffix describes a file rather
than its origin, so the count and suffix checks narrow what can reach the
upload; they do not prove where those bytes came from. Each package _is_ also
checked against where the download put it: the action records the target
subtree each published package was found in, and a record claiming a package a
sibling's subtree staged is refused. That is an observation of the tree the run
has, not proof of who built the file. The producer records are what bind the
bytes to a commit.

Two consequences matter to anyone re-running a release:

- An attached asset GitHub already reports with the recorded digest is skipped.
  An attached asset with different bytes stops the run, and the uploader never
  replaces an existing asset, so a re-run cannot swap a release's bytes for a
  second build's.
- The declaration and both digest records are retained as a
  `declared-assets-<job>-<attempt>` workflow artefact before the transfer, so a
  refusal later in the run does not cost the operator the evidence of what was
  declared. Two later jobs read them: the `verify-release-inventory` job
  downloads what the release holds and compares it with those records, so a
  release whose downloaded bytes differ from the staged ones stops the run, and
  the finalizer un-drafts the release only when every declared asset already
  holds its recorded digest and the release carries no attachment the record
  never declared. Comparing the downloads with the record witnesses the bytes
  the publisher staged; it does not prove those bytes came from the intended
  build, as the [developers' guide](developers-guide.md) explains. Those checks
  narrow the gap between the verified snapshot and the published release
  without closing it, because an asset can still change between the finalizer's
  own reconciliation and its edit. The finalizer's check governs the un-draft
  it performs: a release that is already public stays public whether or not
  those jobs run, so the guarantee is that the workflow never publishes an
  unverified draft, not that it verifies one it finds already published. Only a
  release the workflow creates is a draft; when the tag already has one, the
  job attaches to that release whether or not it is public. A missing or
  conflicting asset, or an attachment the record never declared, stops the run
  before it publishes the release; a rerun that finds one already public is
  refused rather than recorded as finalized.

Manifests, local builds, and the published contents are unchanged, and no new
release input was added: the dry run and the publishing lane keep using the same
`dry-run` output. The [users' guide](users-guide.md) describes the upload
policy and the [developers' guide](developers-guide.md#release-publication)
records the job graph and the runbook.

**Breaking for direct callers of the action:** the action's `commit` input now
carries a binding requirement it did not have before. It must be the full,
lowercase 40-character hexadecimal commit the release publishes, matching the
`commit` every producer recorded; an abbreviated SHA, an uppercase spelling, a
tag, or a branch name is refused before any upload. A workflow that calls
`./.github/actions/upload-declared-assets` directly should forward
`release.yml`'s `metadata.candidate_commit` output rather than deriving the
value from the triggering ref, because only the metadata job applies the
spelling rule and publishes the validated name. Pull requests that call
`release.yml` itself are unaffected.

**Breaking for direct callers of `build-and-package.yml`:** the reusable
workflow gained the same required `commit` input. A caller must supply the
full, lowercase 40-character hexadecimal commit the release publishes; the
producer checks out that value, refuses to build when the checkout's `HEAD`
differs, and records it in the producer record the upload action binds against.
A caller that omits it is refused when the workflow is invoked, and one that
passes an abbreviated SHA, an uppercase spelling, a tag, or a branch name is
refused at the checkout check rather than building bytes it cannot attribute to
a commit.

## Tolerate the forwarding-failure warning on stdout

Netsuke counts every child-output forwarding failure in
`netsuke_ninja_forward_failures_total`, labelled by `stream` and
`failure_kind`, and emits a warning beside the output it describes. Forwarding
loss does not change the build result: the child's exit status remains
authoritative, so a build whose output a closed pipe truncated is reported as
the build it was.

One case moves a line onto stdout. When stderr is the destination that rejected
a write or a flush — for example under `netsuke build 2>&-` — the closed pipe
cannot carry its own warning, so Netsuke writes
`netsuke: warning: failed to forward child stderr` to stdout instead. A script
that parses stdout must tolerate that line where it previously saw only build
output. JSON mode never reaches the fallback, because it drains both child
streams to a sink, so stdout stays one result document.

See
[Bounded output-forwarding metrics](users-guide.md#bounded-output-forwarding-metrics)
for the labels, the fallback condition, and what forwarding loss does and does
not change.

## What does not need to change

The convenience wrappers keep their child-process behaviour: the child inherits
the calling process's environment, and Ninja is resolved exactly as before.
Their program and build-file path parameters now use `&Utf8Path`, so callers
using `std::path::Path` must convert those values. A caller that constructs
`NinjaBuildRequest`/`NinjaToolRequest` directly must pass `options: &options`
and supply the required `stderr_mode: StderrMode` field.

Existing callers that construct `VerboseTimingReporter::new` continue to
receive timing summaries on stderr. Callers that need to capture or redirect
those summaries can opt into `VerboseTimingReporter::with_writer`; the
[developers' guide](developers-guide.md#verbose-timing-sink) documents the
owned sink and completion-ordering contract.

## Migration checklist

- [ ] Install the pinned Rust toolchain before building from a checkout.
- [ ] On Linux, install the pinned linker before the first source build.
- [ ] Review the migration sections for manifest or API features in use.
- [ ] Before opting into `console`, use Ninja 1.5 or later.
- [ ] Add `console: false` or `console: true` to Rust `Target` literals.
- [ ] Add `output_visibility: EdgeOutputVisibility::Buffered` or `Immediate`
  to Rust `BuildEdge` literals.
- [ ] Move console scheduling that relied on `Action.pool` onto
  `BuildEdge::output_visibility`.
- [ ] Add `NinjaGenError::ConsoleDependencyOnly` and
  `NinjaGenError::ConsolePoolConflict` arms to exhaustive matches on generation
  errors.
- [ ] If a script parses `netsuke` stdout, tolerate the
  `netsuke: warning: failed to forward child stderr` line.

## Common errors and fixes

- **Error:** `Dependency-only entries cannot select the console pool.`
  - **Fix:** Remove `console: true` from any dependency-only target.
- **Error:** `Build edge 'build/stamp' cannot select the console pool because its
  action is dependency-only.`
  - **Fix:** Enable console scheduling only on an executable action or target.
- **Rust API error (`Action.pool`):**
  `Build edge 'build/stamp' selects the console pool but its action requests
  custom pool 'compile'.`
  - **Fix:** For Rust callers, remove the conflicting `Action.pool` value or
    set it to `console` when that edge should use the console pool.

## Diagnostics

Ninja subprocess spans and warn events carry two bounded fields,
`env_override_count` and `path_overridden`, so environment-caused failures are
diagnosable from logs. Variable names and values are never recorded;
`CommandEnv`'s `Debug` output is redacted to the same counts.
