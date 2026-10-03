# RFC 0009: Structured-command working directories

## Preamble

- **RFC number:** 0009
- **Amends:** RFC 0001, Structured command blocks and argv templates
- **Status:** Proposed
- **Created:** 2026-08-26
- **Target:** Structured command block schema and execution IR

## 1. Summary

This amendment adds a first-class `cwd` field to RFC 0001 structured command
blocks.

The field selects the child process working directory without embedding `cd`,
`pushd`, platform shell syntax, or directory state in a legacy command group.
It applies equally to direct and explicit-shell structured commands and may
vary between stages of one structured pipeline.

Upon acceptance, RFC 0001 must be read as if:

- per-command working directories were removed from its non-goals;
- the `cwd` field appeared in the structured command schema and execution IR;
- executable resolution, spawning, validation, diagnostics, security, and tests
  included the semantics below; and
- the open question asking whether `cwd` should exist were resolved in favour
  of this amendment.

The implementation PR should fold this amendment into the main RFC text before
RFC 0001 moves from Proposed to Accepted.

## 2. Motivation

Several downstream Makefiles operate across more than one project root. Common
examples include:

- running Cargo commands in `rust_extension`;
- running backend checks below `backend/`;
- invoking tooling from a generated fixture directory;
- building one workspace member with a tool whose configuration is relative to
  that member; and
- executing a pipeline whose producer and consumer belong to different local
  subprojects.

Without `cwd`, a structured command must either:

- fall back to `shell: true` and write `cd directory && command`;
- wrap the command in a helper script; or
- require every called tool to expose an equivalent directory option.

That undermines the shell-free path RFC 0001 is intended to provide. Working
directory is a primitive of every process API and belongs alongside argv,
environment, and standard streams.

## 3. Goals and non-goals

### 3.1 Goals

This amendment aims to:

- set the child process working directory explicitly;
- preserve direct-mode argv safety;
- use the same field in direct and shell modes;
- support a distinct directory for each pipeline stage;
- keep path resolution deterministic and capability-scoped;
- retain the existing stream-path and graph-path contract unless stated
  explicitly;
- expose useful source provenance and diagnostics; and
- preserve all legacy command semantics.

### 3.2 Non-goals

This amendment does not:

- make `cwd` persist to a later structured block;
- alter the current one-shell state sharing inside a contiguous legacy command
  group;
- infer target inputs or outputs from the working directory;
- permit arbitrary directory escape outside the effective workspace;
- create a missing directory;
- search upward for a workspace, manifest, or tool configuration;
- make a resolving stream path, `PATH` entry, or executable lookup depend on a
  directory other than the effective `-C` directory, its resolved `cwd`, or the
  effective child `PATH` as already defined;
- define a directory stack; or
- add per-rule, per-target, or per-action enclosing working directories.

Those enclosing scopes may be proposed later with explicit precedence rules.

## 4. Schema amendment

RFC 0001 section 6.2 gains `cwd`:

```yaml
invoke: cargo clippy --all-targets
shell: false
cwd: rust_extension

env:
  RUSTFLAGS: -D warnings

stdin: input.txt
stdout: output.txt
stderr: errors.txt
tee: trace.log
pipe: false
```

The field table gains:

| Field | Type        | Default                             | Meaning                          |
| ----- | ----------- | ----------------------------------- | -------------------------------- |
| `cwd` | string path | Netsuke effective working directory | Child process working directory. |

Table 1: Structured-command fields.

Unknown-key rejection remains unchanged.

`cwd` is valid on every structured command block, whether singular, in a
heterogeneous sequence, or in a structured pipeline.

## 5. Rendering and path resolution

`cwd` is a scalar Jinja template rendered at manifest compilation time. It must
produce one non-empty string containing no NUL.

Sequence, mapping, null, undefined, and callable values are errors.

A relative `cwd` is resolved against Netsuke's effective working directory
after CLI `-C` processing. It is not resolved relative to:

- the source file containing the command;
- a bundle directory;
- the previous command's `cwd`;
- the current process directory after another command; or
- the directory containing an executable.

The initial structured-command surface accepts only working directories that
resolve within the effective workspace capability. Absolute paths and lexical
or symlink escapes are rejected.

RFC 0001 section 9 previously said that an absolute `cwd` "remains absolute".
That sentence was inconsistent with the confinement rule here, and
[ADR-043](../adr-043-consolidated-structured-command-execution-contract.md)
resolves the inconsistency in favour of rejection. A rendered string must not
expand the process's ambient filesystem authority; an explicit external
directory handle remains the designed extension in section 5 above. RFC 0001
section 9 has been amended to match.

This confinement is deliberate. A later capability RFC may allow an explicit
external directory handle, but a rendered string must not silently expand the
process's ambient filesystem authority.

## 6. Existence and type checks

Netsuke performs all safe lexical and capability-boundary validation during
manifest compilation.

Directory existence is checked by the action runner immediately before spawning
the execution unit. The path must identify a directory at that time.

Runtime errors distinguish:

- path not found;
- path exists but is not a directory;
- permission denied;
- symlink or capability escape;
- path changed between validation and spawn where the platform exposes that
  distinction; and
- platform process API rejection.

Netsuke does not create the directory automatically.

## 7. Direct-mode semantics

For a direct structured command, the action runner configures the process using
the platform process API equivalent of `std::process::Command::current_dir`. It
does not prefix argv with a shell command.

Executable resolution follows RFC 0001 section 9 with this clarification:

- a bare program name is resolved through the effective child `PATH`;
- a relative program containing a path separator is resolved against the
  normalized `cwd` to an absolute, capability-checked executable path before
  `current_dir` is set; and
- an absolute executable remains subject to existing capability and platform
  policy.

Arguments are unaffected by `cwd`; Netsuke does not rewrite relative path
arguments because only the called program knows their grammar.

Example:

```yaml
command:
  invoke: cargo test --all-targets
  cwd: rust_extension
```

This launches `cargo` with `rust_extension` as its process working directory
and passes the remaining arguments unchanged.

## 8. Shell-mode semantics

For `shell: true`, Netsuke sets the shell process working directory through the
process API before executing the rendered shell source.

Example:

```yaml
command:
  invoke: printf '%s\n' "$PWD"
  shell: true
  cwd: backend
```

Netsuke must not lower this to `cd backend && ...`; doing so would reintroduce
shell-dialect quoting and error-propagation differences.

The shell sees its ordinary working-directory variables and built-ins after
startup. Any `cd` performed inside that shell process remains local to the
execution unit.

Shell mode selects only how `invoke` is interpreted. It does not change the
stream-path base, the child working directory, or the confinement rules; the
named shells of [RFC 0011](0011-allow-listed-structured-command-shells.md)
follow the same contract as `shell: true`.

## 9. Command-list boundaries

Each structured block receives its own `cwd`. The value does not carry into a
later structured block, rule reference, script item, or separate legacy shell
group.

```yaml
command:
  - invoke: cargo build
    cwd: workspace-a
  - invoke: cargo test
    cwd: workspace-b
```

The second command starts directly in `workspace-b`, regardless of changes made
by the first process.

RFC 0001 legacy compatibility remains unchanged:

```yaml
command:
  - cd workspace-a
  - cargo build
```

This all-string list remains one legacy shell group, so the shell `cd` carries
between its entries. Netsuke does not reinterpret or migrate legacy groups
automatically.

## 10. Pipeline semantics

Every structured pipeline stage may specify a distinct `cwd`:

```yaml
command:
  - invoke: generator --format json
    cwd: producer
    pipe: true
  - invoke: validator --schema schema.json
    cwd: consumer
    stdout: validated.json
```

Netsuke establishes pipes before spawning the stages and applies each stage's
working directory independently.

A stage's `cwd` does not affect pipe bytes or the next stage's path resolution.
Spawn failure caused by one invalid `cwd` follows RFC 0001 pipeline cleanup:
already-started stages are terminated and reaped, and every failure identifies
the affected stage.

## 11. Stream-path relationship

RFC 0001 section 12.1 resolves `stdin`, `stdout`, `stderr`, and `tee` paths
relative to the block's resolved `cwd`, falling back to Netsuke's effective
working directory after CLI `-C` processing when `cwd` is absent. Section 9 of
that RFC states the same rule for the process working directory: the block or
stage directory is the base for all relative stream paths. This amendment
confirms that rule for the field it adds. Stream paths **are** relative to
`cwd`, and each stage resolves its own paths against its own resolved `cwd`.

An earlier revision of this section asserted the opposite and described RFC
0001 section 12.1 as resolving stream paths against the effective workspace
root. That description was wrong. Issue
[#803](https://github.com/leynos/netsuke/issues/803) recorded the
contradiction, and roadmap task 12.1.1 consolidated the contract:
[ADR-043](../adr-043-consolidated-structured-command-execution-contract.md)
records the accepted rule.

Example:

```yaml
command:
  invoke: cargo test
  cwd: rust_extension
  stdout: artefacts/rust-extension-test.log
```

The child runs in `rust_extension`, and the output path resolves from
`rust_extension` as well. The declaration renders at manifest compilation time,
so the normalized result is
`<effective -C directory>/rust_extension/artefacts/rust-extension-test.log`.

The base composes exactly once. A relative `cwd` resolves against the effective
`-C` directory, and a relative stream path then resolves against that normalized
`cwd`. A relative `cwd` always resolves inside the effective workspace
capability, and the confinement rules in section 5 apply to `cwd` as inscribed
there. [RFC 0010](0010-runtime-bindings-and-secure-tempdirs.md) section 6.4
extends `cwd` with a secure-temporary form that deliberately resolves outside
the workspace, and section 9.5 of that RFC states how a relative stream path
behaves under it.

One consequence is deliberate. Stream destinations are workspace artefacts, and
they now follow the directory a stage was placed in. A command moved to a
different subdirectory writes its output to a different path unless it names
the artefact from the effective `-C` directory. That is the same behaviour as a
shell redirect, and it is what makes `cwd` reusable as a single declaration:

```yaml
command:
  - invoke: generator --format json
    cwd: producer
    stdout: generated/items.json
  - invoke: validator ../producer/generated/items.json
    cwd: consumer
```

The first stage writes under `producer`. The second runs under `consumer`, so
its own relative base is `consumer`; it reaches the artefact only because it
names the producer's directory explicitly. Each stage is self-consistent, and
neither inherits the other's directory.

A consumer's argument is not rewritten. Section 7 states that Netsuke passes
relative path arguments unchanged, because only the called program knows their
grammar, so the argument is interpreted from the consumer's working directory
and must be spelled to match. Declaring the artefact from the effective `-C`
directory instead would also work and would spare the consumer any knowledge of
where the producer was placed, at the cost of pinning the pipeline to one
layout.

Keeping the bases distinct was considered and rejected by ADR-043. It would let
a stage write outside the directory it was placed in while the child still ran
inside it, which makes a relocated command silently produce a different graph
artefact than the one an author reading the block expects.

## 12. Bundle and include relationship

Commands originating from included fragments or bundles still resolve `cwd`
against the importing build's effective workspace root.

A bundle must not gain ambient access to its own source directory merely
because its manifest file lives there. A reusable bundle that needs a
subproject path should accept it as a typed parameter and use that value in
`cwd`.

The provenance record retains both:

- the source fragment or bundle that declared `cwd`; and
- the rendered workspace-relative working directory.

This keeps bundle execution portable across repository layouts.

## 13. Execution IR amendment

RFC 0001's illustrative `CommandBlock` gains:

```rust
pub struct CommandBlock {
    pub invoke: String,
    pub shell: bool,
    pub cwd: Option<Utf8PathBuf>,
    pub env: BTreeMap<String, String>,
    pub stdin: Option<Utf8PathBuf>,
    pub stdout: Option<Utf8PathBuf>,
    pub stderr: Option<Utf8PathBuf>,
    pub tee: Option<Utf8PathBuf>,
    pub pipe: PipeStream,
}
```

`PipeStream` is the RFC 0001 enum with `None`, `Stdout`, and `Stderr` variants.
The `pipe: true` compatibility spelling normalizes to `Stdout`; RFC 0010
refines the `Stderr` selection with its raw-byte pipeline semantics.

The rendered `ProcessSpec` also carries a normalized, capability-relative
working-directory value or directory handle.

The action plan must not contain an unvalidated absolute host path when a
workspace-relative identity or leased directory capability is sufficient.

## 14. Action runner and generated sidecars

The action runner receives the effective workspace root through the same leased
execution context that owns action-plan sidecars.

Before spawn it:

1. resolves the normalized `cwd` through the workspace directory capability;
2. verifies it is still a directory;
3. configures the child process directory;
4. applies the environment overlay;
5. configures streams and pipeline handles; and
6. spawns the process.

The order above is conceptual. Implementations may prepare handles differently,
but a failed directory resolution must occur before the child executes.

Generated plans include enough schema versioning that a runner which does not
understand `cwd` rejects the plan rather than ignoring the field.

## 15. Diagnostics and observability

A `cwd` failure identifies:

- enclosing action, target, or rule;
- command-list item and pipeline stage;
- original source file and span;
- rendered workspace-relative directory;
- failure category; and
- bundle/include provenance where applicable.

Absolute host paths should be omitted or redacted when the relative path is
sufficient.

Telemetry may record bounded categories such as:

- default versus explicit working directory;
- direct versus shell mode; and
- failure category.

Directory values must not become metric labels.

## 16. Security properties

The amendment provides these properties:

- `cwd` cannot alter argv boundaries or shell syntax in direct mode;
- a rendered relative path cannot escape the workspace capability;
- one command's directory cannot mutate another command's process state;
- bundle source location does not confer filesystem authority; and
- generated plans retain a validated directory identity rather than arbitrary
  shell source.

It does not prevent the child from traversing from its working directory using
its own ambient filesystem permissions. Broader child sandboxing remains a
separate execution policy.

## 17. Compatibility and migration

Existing structured command proposals without `cwd` retain the effective
workspace root as their process directory.

Legacy scalar and all-string-list commands retain their existing shell
semantics.

Mechanical migration examples are:

```yaml
# Before
command: cd rust_extension && cargo clippy --all-targets

# After
command:
  invoke: cargo clippy --all-targets
  cwd: rust_extension
```

and:

```yaml
# Before
command:
  - cd backend
  - cargo test

# After
command:
  invoke: cargo test
  cwd: backend
```

Only migrate a legacy list when no other shell state must persist between its
entries.

## 18. Validation additions

RFC 0001 section 15 gains:

- `cwd` must render to one non-empty scalar string;
- `cwd` must contain no NUL;
- relative path normalization must remain inside the effective workspace;
- absolute paths are invalid in the initial surface;
- bundle and fragment origin does not change the resolution base;
- each pipeline stage validates its own directory; and
- an action plan containing `cwd` requires a runner schema that supports it.

Directory existence and type remain runtime validation as described in section
6.

## 19. Test additions

RFC 0001's test strategy gains:

- direct and shell commands observing the requested directory;
- default-directory compatibility tests;
- relative executable resolution from `cwd`, including normalization of
  separator-containing paths to absolute capability-checked paths before
  `current_dir` is set;
- paths containing spaces and Unicode;
- lexical `..`, absolute, and symlink escape rejection;
- missing, non-directory, and permission failures;
- independent directories across adjacent structured blocks;
- distinct directories across pipeline stages;
- pipeline cleanup when a later stage has an invalid directory;
- stream paths resolving against the stage's `cwd` rather than the effective
  workspace root, including adjacent stages with different `cwd` values and a
  relative `cwd` composed once with a relative stream path;
- fragment and bundle declarations using parameterized directories;
- Windows drive, separator, and case behaviour; and
- action-plan schema compatibility and provenance snapshots.

Property tests should generate bounded relative paths and assert that
normalized accepted paths remain descendants of the effective workspace root.

## 20. Alternatives considered

### 20.1 Require `cd` under `shell: true`

This keeps the schema smaller but defeats direct mode, differs between shells,
and mixes directory selection with command source. Rejected.

### 20.2 Add only rule-level working directories

Some command sequences need different directories per stage, and a rule-level
field creates inheritance and precedence questions before the primitive exists.
Rejected as the initial surface.

### 20.3 Keep stream paths relative to the effective workspace root

This keeps graph artefact locations independent of process placement, at the
cost of making a relocated stage read and write beside its previous location
while its child runs somewhere else. It also contradicts RFC 0001 sections 9
and 12.1, which already resolve relative stream paths against the block's
resolved `cwd`. Rejected by
[ADR-043](../adr-043-consolidated-structured-command-execution-contract.md), so
that `cwd` is one declaration covering both the child's directory and the
stage's relative artefacts.

### 20.4 Resolve `cwd` relative to the declaring fragment

That makes extracted fragments and bundles location-dependent. Rejected in
favour of the effective workspace root and typed bundle parameters.

### 20.5 Allow unrestricted absolute paths

This grants ambient filesystem authority through a rendered string and weakens
bundle portability. Rejected until an explicit external-directory capability is
designed.

## 21. Recommendation

Amend RFC 0001 to include `cwd` as a first-class structured process field.

Working directory is part of process construction, not shell syntax. Adding it
now closes a major Makefile-migration gap while preserving RFC 0001's central
safety property: rendered values remain typed process data and are never
reparsed as command structure.
