# Migrating to v0.1.0

This guide signposts the child-environment additions arriving in the v0.1.0
beta series: the injectable child environment (`CommandEnv`) and the named
Ninja request types. Existing callers compile unchanged; every addition is
opt-in.

## Netsuke is a build tool, not a library

Netsuke is intended to be used as a command-line build tool. The only
surfaces it commits to are the Netsukefile manifest format and the graph
export. Everything else — the Rust API described below included — is
private in intent and unstable in practice: it may change shape, or vanish,
in any release of the beta series without a deprecation period. Reliance
on it is conditional on tracking those changes.

## At-a-glance changes

Table: v0.1.0 child-environment API additions and their impact

| Area | Impact | Where to read more |
| --- | --- | --- |
| Convenience wrappers | Unchanged. `run_ninja` and `run_ninja_tool` behave exactly as before, inheriting the process environment. | [Users' guide](users-guide.md) |
| Child environment | New opt-in `netsuke::runner::CommandEnv` carries additive variable overrides and an injected `PATH` for Ninja child processes. | [Users' guide](users-guide.md) |
| Request types | New `netsuke::runner::NinjaBuildRequest` and `netsuke::runner::NinjaToolRequest` name the program, build file, and targets or tool for the `*_with` run functions. | [Users' guide](users-guide.md) |
| Glob expansion | Parent-relative patterns such as `glob('../shared/*.h')` now expand. Metadata checks use a capability rooted at the pattern's longest literal directory prefix; missing or non-directory prefixes return no matches, and unresolvable symlink matches are skipped. | [Users' guide](users-guide.md) and [ADR-010](adr-010-scope-glob-capability-to-literal-prefix.md) |
| Command recipes | Existing scalar `command` recipes are unchanged. New YAML command lists are opt-in and run in declaration order with fail-fast semantics. | [Rules and recipes](users-guide.md#rules-and-recipes) |

## Nothing to change for existing callers

The convenience wrappers keep their signatures and their behaviour: the
child inherits the calling process's environment, and Ninja is resolved
exactly as before. No caller needs to change to adopt this release.
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

## Diagnostics

Ninja subprocess spans and warn events carry two bounded fields,
`env_override_count` and `path_overridden`, so environment-caused
failures are diagnosable from logs. Variable names and values are never
recorded; `CommandEnv`'s `Debug` output is redacted to the same counts.
