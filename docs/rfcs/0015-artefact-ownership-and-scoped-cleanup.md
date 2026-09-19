# RFC 0015: Artefact ownership and scoped cleanup

## Preamble

- **RFC number:** 0015
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Optional owned-output declarations and bounded deletion
- **Implementation:** [Progressive-enhancement roadmap, phase 24][roadmap]

## 1. Summary

Add optional named artefacts and one standardized scoped-cleanup operation. An
author who declares disposable output ownership gains bounded enumeration, a
useful preview, explicit confirmation, and capability-scoped deletion. Plain
commands, existing targets, and existing `clean` behaviour remain supported.

Ownership is an assertion by the manifest author, not proof that a directory
contains no valuable files. The implementation must make the scope inspectable
and enforce its boundaries without claiming to sandbox arbitrary recipes.
[RFC 0017][maturity] makes this an opt-in improvement rather than an onboarding
prerequisite.

## 2. Problem and existing boundaries

Repositories repeat cleanup lists and broad deletion commands. The lists drift
away from producers, and shell fragments obscure path, symlink, and error
semantics. A benchmark report, shared tool cache, temporary directory, and
prepared environment also need different retention decisions.

Netsuke already has target outputs and delegates ordinary output cleaning to
Ninja. Its runtime owns dyndep sidecars under a separate bounded-retention
contract. RFC 0010 owns command-local temporary directories. This RFC must not
replace those mechanisms or infer ownership from arbitrary command text.

## 3. Progressive authoring

The following proposed fragment adds one explicit disposable directory and a
cleanup action without a state, typed input, context, or package provider:

```yaml
artefacts:
  build-output:
    path: build
    kind: directory

actions:
  - name: clean-build
    command:
      clean_owned: [build-output]
```

`path` names exactly one workspace-relative object. `kind` is `file` by default;
recursive ownership requires explicit `directory`. A directory declaration
claims the entire subtree, including existing and subsequently created children.
The preview must say this plainly. It is unsuitable for a directory shared with
source files or another owner's mutable state.

A producer can identify a report without changing whether the action runs:

```yaml
artefacts:
  benchmark-report:
    path: dist/benchmarks/microbenchmarks.json
    role: report
    create_parent: true

actions:
  - name: benchmark
    produces: [benchmark-report]
    command:
      invoke: python benchmarks/run.py --output dist/benchmarks/microbenchmarks.json
```

The command is illustrative. `produces` records ownership and expected outputs;
it does not make a benchmark incremental or authorize replay of an old result.
Directory creation occurs only when that producer executes, through the same
path boundary used for cleanup.

## 4. Declaration, ownership, and output contracts

An artefact contains required `path`, optional `kind`, optional `role`, and
optional `create_parent`. Initial roles are `generated`, `report`, `cache`, and
`environment`, with `generated` the default. A role describes intent, not an
automatic deletion or caching rule. No role disappears merely because another
action failed. Exact paths, rather than arbitrary glob expressions, keep the
first implementation bounded and reviewable.

Use existing namespace, duplicate, and source-provenance rules. Each normalized
path has one owning declaration. Reject duplicate or overlapping ownership
roots, including case-equivalent names on the relevant filesystem. Parent
creation is not an ownership claim over the parent or its other contents. A
directory and a separately declared child are overlapping owners in this first
version; declare one or use disjoint exact files.

A `produces` reference identifies one producer. Reject multiple producers for
the same artefact unless an existing explicit target contract already supplies
one shared producer. An explicit artefact may refer to an existing target output
only when both identify that same producer and compatible object type. It
augments metadata; it must not create a second Ninja producer edge.

Verify declared required outputs after successful production. Missing outputs
are producer failures, not success. Failure may leave partial owned outputs;
record the failure but retain those outputs for inspection and explicit cleanup.
Do not automatically remove them or reuse them as successful build evidence.

Declarations without a producer remain useful for externally generated,
explicitly disposable paths. Inspection must distinguish declared ownership from
observed successful production. Neither status attests that the author chose a
safe directory. No producer receipt is required merely to remove a declared
legacy build directory, but declaration, bounded preview, and authorization are.

This does not introduce remote artefact delivery, a content-addressed store, or
a provenance attestation system. Roadmap phase 5 retains its delivery boundary.

## 5. Scoped-cleanup operation and public command integration

Add `clean_owned: [NAME, ...]` as an explicit execution unit in the structured
command union. The list is nonempty, order-insensitive after name resolution,
and duplicate references normalize to one selection. Unknown references fail.
The operation may name artefacts only; it never accepts raw shell paths.

Extend the existing `clean` command with repeatable `--artefact NAME` selection.
Register the extension in canonical CLI metadata before implementing it. Without
that option, preserve existing Ninja-output cleanup. With explicit artefact
selection, clean only the selected ownership roots; do not implicitly add every
Ninja output, environment, cache, or runtime directory.

These proposed commands demonstrate preview and explicit noninteractive consent:

```bash
netsuke clean --artefact build-output --dry-run
netsuke clean --artefact build-output --force --no-input
```

Both command and recipe forms use the same planner, validator, deleter, and
structured results. Use the existing mutation metadata for `--dry-run`,
`--force`, and `--no-input`; do not invent a cleanup-specific confirmation
framework. An interactive run can request confirmation after displaying scope. A
noninteractive destructive run without explicit consent fails before deletion.
`--force` skips confirmation, not validation, ownership conflicts, or bounds.

A dry-run is a read-only plan, not a reusable authorization token. Execution
resolves and validates scope again, and must not consume a stale user-supplied
list as trusted filesystem authority. The preview lists normalized roots,
recursive ownership, existing objects, missing objects, retained siblings, and
any rejected scope. It must not truncate away selected objects and then claim to
show a complete destructive plan: exceeding bounds fails the plan.

## 6. Filesystem safety contract

Anchor paths at the effective workspace capability, not the caller's ambient
working directory or the location of an included fragment. Reject absolute
paths, empty paths, the workspace root, parent traversal, incompatible
encodings, and platform-specific escape forms before touching the filesystem.

Protect the effective manifest, loaded configuration, local runtime scripts
known to the compiled plan, declared source paths, VCS metadata such as `.git`,
and Netsuke's reserved runtime directories. A directory claim containing a
protected object is invalid. Without source-control metadata Netsuke cannot
identify every source file; the contract must disclose this limitation rather
than infer arbitrary source ownership.

Never follow a symlink or Windows reparse point during recursive traversal. A
selected final symlink may be unlinked as an object within the declared root,
but never traversed to its destination. Revalidate its identity and parent
capability at deletion. Reject unsupported mount, junction, or filesystem cases
rather than fall back to lexical `starts_with` checks or ambient `rm -rf`.

Use handle-relative traversal and deletion with platform-specific identity
checks. A path checked before enumeration is not automatically safe at deletion.
Detect replacement of selected roots and parents, and stop affected work. No
claim of race freedom is acceptable until adversarial replacement tests pass on
each supported platform; unavailable guarantees must produce explicit refusal.
Hard-linked file removal unlinks the selected directory entry, never truncates
the shared underlying file.

Enforce operator-capped entry, depth, byte, and elapsed-time limits. Perform the
bounded admission pass before the first deletion; exceeding a bound there causes
zero deletions. Recheck while deleting because the tree may change. Delete files
before directories in a stable order. Already-missing objects are successful
no-ops. Permission, replacement, or interruption errors may follow partial
progress; report exact removed, retained, and failed objects. Cleanup is not an
atomic transaction and must never report rollback that it did not perform.

## 7. Interaction with states, concurrency, and replay

A separately declared environment artefact may name the same normalized path as
a [managed state][states]; the resource registry identifies that association.
Ownership remains optional for state use. When an environment is
selected for cleanup, acquire its integrity lease and invalidate its readiness
records before any deletion, including failure paths. Probe success from before
cleanup cannot establish subsequent readiness.

Reject a selected build closure that both cleans and produces or consumes the
same declared resource. A Ninja pool would serialize access but would not
establish the user's intended order. Separate invocations are the initial
remedy. Coordinated state leases protect cooperating invocations; unrelated
external programs remain outside the guarantee.

Pure artefact cleanup also needs an exclusive lease over each selected root,
ordered canonically. Producers participating in ownership use the same lease. It
must share the state-resource identity boundary rather than introduce a second
incompatible lock system. The lightweight artefact-only path cannot require
authoring a state declaration.

Persist ownership definitions and provenance in the versioned action plan, not
captured directory listings. Replay obtains fresh capabilities and enumerates
again. Existing dyndep retention and command-private temporary cleanup retain
their original owners; `clean_owned` cannot target their reserved locations. A
later explicit `cargo clean` recipe still has Cargo's own semantics and is not
covered by Netsuke's scoped-deletion guarantee.

## 8. Acceptance and migration

Keep the existing hello-world and legacy `clean` fixtures unchanged. Add a
one-directory example whose complete ownership declaration needs only `path` and
`kind`. Test explicit file production, parent creation, stale reports, missing
outputs, and unchanged always-run benchmark behaviour.

Property tests must cover canonical-path uniqueness, scope normalization,
protected roots, and duplicate selection. End-to-end tests must exercise
symlinks, reparse points, root replacement, hard links, permission errors, case
collisions, interrupted partial deletion, entry-budget overflow, and missing
paths. External sentinel files must remain unchanged. Assert zero filesystem
writes in dry-run, including no preparation or probe execution.

Migrate Cuprum incrementally: explicitly declare disjoint output roots first;
keep unmatched wildcard cleanup as an ordinary command until a separate safe
file-selection contract exists. Do not add unrestricted glob deletion merely to
reproduce a long shell command in the initial release. Deleting legacy output
trees remains an explicit author decision shown in preview.

## 9. Alternatives and outstanding decisions

A mandatory out-of-tree store would change onboarding and project layout.
Inferring ownership from redirections or tool names would be unreliable.
Wrapping `rm -rf` would provide neither platform consistency nor a capability
boundary. Requiring an artefact declaration for every existing target would make
an optional benefit contagious.

Before implementation, ratify the supported-platform deletion primitives,
resource-lease identity, ownership/source conflict rules, and concrete operator
limits. Atomic trash-and-rename, adoption receipts, wildcard collections, role
selectors, and remote delivery remain separate possible extensions, not
prerequisites for a useful exact-path cleanup operation.

## 10. Recommendation

Start with exact files and explicitly owned directory trees, one bounded cleanup
implementation, and transparent scope. Preserve ordinary recipes and existing
cleaning while giving annotated outputs stronger, testable guarantees.

[roadmap]: ../roadmap-progressive-enhancement.md#24-owned-artefacts-and-bounded-cleanup
[maturity]: 0017-progressive-enhancement-and-maturity-policies.md
[states]: 0013-managed-states-and-probes.md
