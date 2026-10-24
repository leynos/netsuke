# Architectural decision record (ADR) 0065: Artefact ownership schema

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFC 0023 proposes explicit ownership declarations and one bounded cleanup
operation. Ownership must be an auditable assertion by a manifest author, not
an inference from shell text or an automatic claim that a directory contains no
valuable data. The contract must also preserve existing Ninja output cleaning
and keep caches, environments, and runtime directories out of an explicit
cleanup selection unless the author names them.

## Decision Drivers

- Make deletion scope explicit, reviewable, and bounded.
- Keep one path owner and one producer relationship.
- Preserve ordinary recipes and existing `clean` behaviour.
- Use capability-scoped paths and existing mutation-consent metadata.

## Requirements

### Functional requirements

- Add optional root `artefacts` declarations with exact workspace-relative
  paths.
- Allow producers to declare `produces` metadata without changing whether an
  action is incremental.
- Add `clean_owned` to the shared command operation union.
- Canonically spell the feature `artefacts` and the CLI selector `--artefact`.

### Technical requirements

- Reject overlapping ownership roots and conflicting producers.
- Protect declared sources and reserved runtime paths from directory ownership.
- Revalidate and bound the selected scope at execution time.
- Reuse the shared cleanup planner, path capabilities, and mutation metadata.

## Options considered

### Option A: Exact owned paths with a shared bounded cleanup operation

Authors name files or directory trees explicitly. Preview, confirmation,
validation, and deletion use one planner and one capability boundary.

### Option B: Infer ownership from command redirections or tool names

Inference would depend on shell and program-specific behaviour, miss indirect
writes, and provide no reliable deletion boundary.

### Option C: Require every output to be an artefact

Making all existing targets declare ownership would turn an optional cleanup
benefit into a migration prerequisite.

## Decision outcome / proposed direction

An artefact declaration contains required `path`, optional `kind`, optional
`role`, and optional `create_parent`. `kind` defaults to `file`; directory
ownership must be explicit. `role` defaults to `generated`; the closed initial
role vocabulary is `generated`, `report`, `cache`, and `environment`.
`create_parent` defaults to `false`. Roles describe intent and do not grant
automatic caching or deletion behaviour.

The canonical root key is `artefacts`. Do not accept `artifacts` as an alias;
24.1.1 may suggest the canonical spelling when it is mistyped. Paths name one
workspace-relative object, not a glob. Normalized ownership roots must not be
equal, nested, overlapping, or case-equivalent on a case-insensitive
filesystem. Parent creation does not claim ownership of the parent.

A directory artefact may not contain the effective manifest, loaded
configuration, `.git`, Netsuke runtime directories, local runtime scripts, or a
target `sources` path not produced by any target in the graph. An artefact may
equal a target output only when both name the same producer and compatible
object kinds. A `produces` reference identifies one producer; duplicate
producers fail unless an existing target contract already supplies one shared
producer. Missing required outputs are producer failures, not success.

`clean_owned` accepts artefact names, never raw paths. The repeatable CLI
selector is `--artefact NAME`. Without explicit selection, preserve ordinary
Ninja output cleaning. An explicit selection cleans only its named ownership
roots. Preview is read-only and not a reusable authorization token; execution
validates the scope again. Exceeding a bound fails the plan rather than
truncating it. `--force` skips confirmation but not validation or bounds.

Use operator ceilings `clean_entries_ceiling`, `clean_depth_ceiling`,
`clean_bytes_ceiling`, and `clean_elapsed_ceiling_seconds` under ADR-0062.

## Goals and non-goals

- Goals:
  - Make ownership and destructive scope explicit before deletion.
  - Reuse one planner and deleter for recipe and CLI cleanup.
  - Preserve partial outputs for inspection after producer failure.
- Non-goals:
  - Infer ownership or safety from arbitrary commands.
  - Introduce remote delivery, a content-addressed store, or wildcard cleanup.
  - Replace dyndep, command-private temporary, or ordinary Ninja cleanup.

## Migration plan

1. Roadmap item 24.1.1 adds exact declarations and producer references.
2. Item 24.1.2 adds a bounded read-only preview; 24.2.1 owns deletion and
   leases.
3. Item 24.2.2 wires `clean_owned` and `clean --artefact` through existing
   mutation metadata.
4. Item 24.3.1 integrates state-record invalidation only after the state owner
   provides that contract.

## Known risks and limitations

- Ownership is an author assertion; a directory may contain valuable files.
- Without version-control metadata, Netsuke cannot identify every source file.
- A preview cannot guarantee the filesystem remains unchanged before
  execution, so the implementation must validate the live scope again.

## Outstanding decisions

- Roadmap item 24.2.1 owns supported-platform deletion primitives; merge an
  ADR amendment before its code merges if the safety boundary changes.
- Roadmap item 20.2.1 owns resource-lease identity and 24.1.2 owns initial
  numeric bounds; merge an ADR amendment before either implementation merges if
  those contracts change.
- Trash-and-rename, adoption receipts, wildcard collections, role selectors,
  and remote delivery are outside this feature and need a separate RFC before
  they have an implementation owner.
