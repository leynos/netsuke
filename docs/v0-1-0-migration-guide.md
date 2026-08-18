# Migrating to v0.1.0

This guide signposts the v0.1.0 beta additions: the injectable child
environment (`CommandEnv`), the named Ninja request types, and target/action
discovery through `description` and `netsuke help targets`. Existing manifests
remain compatible, and callers of the unchanged convenience wrappers compile
unchanged.
Rust callers that construct `Target` with a struct literal must add the new
`description` field (set it to `None` or `Some(...)`); deserialized manifests
remain compatible, and every other addition is opt-in.

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
| Serial dependencies | New opt-in `dependency_order: serial` runs an action or target's direct `deps` list in declaration order. | [Serial dependency ordering](users-guide.md#run-direct-dependencies-serially) |

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
not use `.netsuke/dyndep` or `.netsuke/serial`: both namespaces are reserved for
Netsuke's generated state. Sidecars are immutable and content-addressed.
Retention keeps the current bundle plus at most 32 obsolete `.dd` files and
1 MiB of obsolete `.dd` bytes; stale `.tmp` files are cleaned while the
exclusive sidecar-directory lease is held. `clean` prunes only after successful
`ninja -t clean`, not after a failure. An old arbitrary `generate --output`
manifest needs regeneration only if retention has removed any of its referenced
sidecars. See [ADR-012](adr-012-bound-dyndep-sidecar-retention.md).
The [users' guide](users-guide.md#run-direct-dependencies-serially) documents
the execution scope and the independent-reachability boundary.

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
targets without running recipes or creating build outputs. Queries allow only
the lexical path filters `basename`, `dirname`, `with_suffix`, and
`relative_to`, the collection filters `uniq`, `flatten`, and `group_by`, and
the clock-independent `timedelta` function. Queries reject `env()` and
`glob()`, file tests, filesystem metadata filters such as `size` and
`linecount`, `hash`, `digest`, `contents`, `realpath`, and `expanduser`,
executable discovery through `which` and `command_available`, network and
command helpers (`fetch`, `shell`, and `grep`), and the clock-dependent `now()`
function. Normal build manifest rendering retains the full standard library;
this restriction applies only to query rendering. Add `--json` to receive the
versioned JSON result document; its
`result.command` is `help-targets`. The command and the new descriptions are
beta-series additions and remain subject to the stability caveat above.

## Diagnostics

Ninja subprocess spans and warn events carry two bounded fields,
`env_override_count` and `path_overridden`, so environment-caused
failures are diagnosable from logs. Variable names and values are never
recorded; `CommandEnv`'s `Debug` output is redacted to the same counts.
