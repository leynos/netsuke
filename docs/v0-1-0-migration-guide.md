# Migrating to v0.1.0

This guide signposts the v0.1.0 beta additions: the injectable child
environment (`CommandEnv`), the named Ninja request types, and target/action
discovery through `description` and `netsuke help targets`. Existing callers
compile unchanged; every addition is opt-in.

## Netsuke is a build tool, not a library

Netsuke is intended to be used as a command-line build tool. The only
surfaces it commits to are the Netsukefile manifest format and the graph
export. Everything else — the Rust API described below included — is
private in intent and unstable in practice: it may change shape, or vanish,
in any release of the beta series without a deprecation period. Reliance
on it is conditional on tracking those changes.

## At-a-glance changes

<!-- markdownlint-disable-next-line MD013 -->
Table: documented v0.1.0 additions, including `netsuke help targets`, and their impact

| Area | Impact | Where to read more |
| --- | --- | --- |
| Convenience wrappers | Unchanged. `run_ninja` and `run_ninja_tool` behave exactly as before, inheriting the process environment. | [Users' guide](users-guide.md) |
| Child environment | New opt-in `netsuke::runner::CommandEnv` carries additive variable overrides and an injected `PATH` for Ninja child processes. | [Users' guide](users-guide.md) |
| Request types | New `netsuke::runner::NinjaBuildRequest` and `netsuke::runner::NinjaToolRequest` name the program, build file, targets or tool, a child environment, and a required `stderr_mode: StderrMode` policy for the `*_with` run functions. | [Users' guide](users-guide.md) |
| Glob expansion | Parent-relative patterns such as `glob('../shared/*.h')` now expand. Metadata checks use a capability rooted at the pattern's longest literal directory prefix; missing or non-directory prefixes return no matches, and unresolvable symlink matches are skipped. | [Users' guide](users-guide.md) and [ADR-010](adr-010-scope-glob-capability-to-literal-prefix.md) |
| Command recipes | Existing scalar `command` recipes are unchanged. New YAML command lists are opt-in and run in declaration order with fail-fast semantics. | [Rules and recipes](users-guide.md#rules-and-recipes) |
| Manifest discovery | Optional target/action `description` values are shown by the new `netsuke help targets` command. Manifests without them and existing build output are unchanged. | [Users' guide](users-guide.md) |

## Nothing to change for existing callers

The convenience wrappers keep their signatures and their behaviour: the
child inherits the calling process's environment, and Ninja is resolved
exactly as before. Callers of `run_ninja` or `run_ninja_tool` need no change;
a caller that constructs `NinjaBuildRequest`/`NinjaToolRequest` directly must
now supply the required `stderr_mode: StderrMode` field, derived from the CLI
with `StderrMode::from_json_enabled(cli.json)`.

## Opting into ordered command lists

Existing scalar `command` recipes remain valid, so no migration is required.
To run a short sequence of commands in declaration order, change a recipe to a
non-empty YAML list. The entries run in one shell process and stop at the first
non-zero exit. See [Rules and recipes](users-guide.md#rules-and-recipes) for
the syntax, shell semantics, and examples.

## Opting into an explicit child environment

Construct a `CommandEnv`, name the variables to add, and pass it through
the request forms:

- `run_ninja_with` runs a build described by a `NinjaBuildRequest`;
- `run_ninja_tool_with` runs `ninja -t <tool>` described by a
  `NinjaToolRequest`.

Overrides are additive: variables not named are inherited. The injected
`PATH` governs what commands the Ninja child can see. Relative program
names remain valid and resolve through that child `PATH`; supply an
absolute or otherwise resolved `program` only when executable selection
must stay isolated from the injected `PATH`.

Both request types borrow their fields, so one `CommandEnv` and one `Cli`
can serve several invocations. Worked examples live in the users' guide's
"Drive Ninja with an explicit environment" section.


## Discover targets and actions

Target and action `description` values are optional discovery metadata. Adding
them does not change manifest compatibility, Ninja progress text, or build
execution: Ninja progress continues to use the referenced rule's
`description`. Existing manifests without these fields remain valid.

Use the new command to inspect the selected manifest:

```sh
netsuke help targets
```

The command honours the usual manifest-selection options, including `--file`
and `-C/--directory`. It loads, expands, renders, and validates the manifest
through a restricted, side-effect-free Jinja surface, then prints actions and
targets without running recipes or creating build outputs. Query expressions
invoking `env()`, the `contents` filter, `fetch`, `shell`, or `grep` are
rejected rather than executed by this command. This restriction applies only
to query rendering; normal build manifest rendering remains unchanged. Add
`--json` to receive the versioned JSON result document; its
`result.command` is `help-targets`. The command and the new descriptions are
beta-series additions and remain subject to the stability caveat above.

## Diagnostics

Ninja subprocess spans and warn events carry two bounded fields,
`env_override_count` and `path_overridden`, so environment-caused
failures are diagnosable from logs. Variable names and values are never
recorded; `CommandEnv`'s `Debug` output is redacted to the same counts.
