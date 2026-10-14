# Architectural decision record (ADR) 043: Isolate command-list entries

## Status

Accepted.

## Date

2026-10-12

## Context and problem statement

An ordered `command` list is one shell process on every current route. POSIX
and Windows Bash render entries as brace groups joined with `&&`, so changes to
the working directory, environment, and shell variables reach later entries.
The default Windows PowerShell route also uses one process and shares location,
`$env:` values, and PowerShell variables. A static reader therefore cannot
decide each entry's effective directory or environment from the manifest alone.
Recipe syntax also depends on the route: POSIX `${VAR:-default}` is not
PowerShell parameter expansion.

Static consumers such as concordat's RFC 0004 QG-001 need to identify each
quality gate and its working directory without executing the manifest. A
manifest author can currently make an apparently workspace-wide gate run from a
subdirectory by placing `cd` in an earlier entry. The same text may also
resolve differently on Windows. Requiring each consumer to return
`indeterminate` moves the ambiguity into every consumer instead of resolving it
in Netsuke.

## Decision drivers

- Give static readers a literal, typed declaration of entry isolation.
- Preserve the behaviour, rendered text, and action identity of existing
  manifests.
- Apply the same isolation guarantee on POSIX, Windows Bash, and Windows
  PowerShell.
- Keep rules, direct targets, and actions on one recipe-level contract.

## Requirements

### Functional requirements

- A command-list recipe may opt into `command_isolation: isolated`; omission
  and `shared` retain shared state.
- Isolation starts every entry from the action's working directory and
  environment. State changes made inside an entry do not reach the next entry.
- Entries remain ordered and fail-fast. A successful `exit` or direct `exec`
  ends only its isolated entry; a non-zero exit prevents later entries.
- A mode is invalid beside `script` or `rule`, without a recipe, or as
  `isolated` on a scalar command.

### Technical requirements

- `CommandIsolation` belongs to `Recipe::Command`; rule resolution and
  interpolation preserve it without additional precedence rules.
- `shared` is omitted from serialization so legacy action hashes remain
  unchanged. `isolated` participates in the action hash.
- POSIX and Bash use a subshell around each existing entry wrapper. PowerShell
  starts a fresh `powershell.exe` for each entry, using the existing UTF-16LE
  Base64 transport and response-file path for an oversized parent recipe.
- Reject statically oversized PowerShell child command lines during
  generation, and check the expanded `$PSHOME` executable path before each
  child launch; keep the 1 MiB limit on parent recipe text, including payloads.
- The manifest format version remains unchanged. Adding a public field to
  `Recipe::Command` is an accepted Rust source break for the unstable v0.1 API.

## Options considered

### Option A: Add typed, opt-in entry isolation (chosen)

Add `command_isolation` to command recipes with `shared` as the default and
`isolated` as the opt-in value. Shared recipes keep their current rendering.
Isolated entries receive a process boundary on every route and retain the
existing ordered, fail-fast contract.

### Option B: Ship structured commands with literal `cwd` fields

RFC 0009 and roadmap items 12.1.1 and 12.2.3 plan structured command working
directories. That model is a useful future capability, but it is not shipped
and depends on separate command-model work. It does not resolve the immediate
ambiguity for legacy command lists.

### Option C: Diagnose stateful or dialect-specific command lists

A loader diagnostic or lint could reject selected uses of `cd`, assignments, or
POSIX parameter expansion. Such a scan would need to analyse shell syntax and
still could not describe each entry's baseline. It would also leave all other
static consumers without a reliable contract.

## Decision outcome

In the context of static `Netsukefile` consumers and host-specific command-list
interpreters, facing shared shell state and route-dependent recipe meaning, we
decided for an opt-in `command_isolation` mode on command-list recipes, and
against immediate structured `cwd` work or a lint-only warning, to let later
entries start from the action baseline while retaining legacy behaviour by
default, accepting an additional child process per isolated entry and a
PowerShell command-line bound.

`shared` remains the default for scalar and list commands. `isolated` is valid
only on command lists. A rule carries its own mode and each target that selects
it inherits that mode when the rule is cloned into an action. The mode is a
literal schema value; Jinja does not template it. Interpolation changes command
text only.

Each POSIX or Bash list entry is wrapped in a subshell around the existing
brace/eval failure-attribution wrapper. Its `&&` parent chain continues after a
successful `exit 0` or direct `exec`, because those terminate the entry's
subshell. A non-zero status still stops the chain, and failure markers are
emitted only for failed isolated entries. The shared wrapper and its output
remain unchanged.

Each PowerShell entry runs in a new child process from the parent action's
initial directory and environment. The child sets `$ErrorActionPreference` to
`Stop`, resets `$LASTEXITCODE`, executes one entry, and propagates a non-zero
native exit. A successful `exit` ends only that child and the parent proceeds
to the next entry. Native stderr remains connected to Ninja's stderr.

This runtime baseline is distinct from manifest-load `env()` access, which
remains governed by [ADR-026](adr-026-manifest-environment-access-policy.md);
command isolation does not grant or change manifest environment access.

Generation rejects statically oversized child command lines, and the parent
checks the expanded executable path before launch so the final child command
line stays within 32,766 characters. Larger parent recipes continue through
Ninja's response-file transport, up to the existing 1 MiB recipe-text ceiling.

The default `shared` mode is omitted from serialized actions, preserving legacy
hashes. The `isolated` value is serialized and therefore produces a distinct
action identity. No manifest version bump is required: the new key is optional
and the old interpretation remains the default.

## Consequences

- Static readers may rely on the declared isolation mode and action baseline;
  they must still interpret recipe text according to its selected shell.
- Gate authors can request entry isolation immediately, while portable recipe
  authors must avoid shell-specific defaults such as `${VAR:-default}`.
- Structured per-unit `cwd` remains planned work under RFC 0009 and roadmap
  items 12.1.1 and 12.2.3; this decision does not ship it.
