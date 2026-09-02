# RFC 0011: Allow-listed structured-command shells

## Preamble

- **RFC number:** 0011
- **Amends:** RFC 0001, Structured command blocks and argv templates
- **Governing decision:** ADR-019, Select allow-listed structured-command
  shells
- **Status:** Proposed
- **Created:** 2026-09-02
- **Target:** Structured command schema, shell registry, and execution IR

## 1. Summary

This amendment lets a structured command select a named shell without giving
the Netsukefile authority to choose an arbitrary executable or interpreter
argument list. The `shell` field becomes a `Boolean | ShellName` union:

- an absent field or `false` retains direct argument-vector (argv) execution;
- `true` retains the platform-default shell for compatibility; and
- a string selects a named shell from a finite built-in or trusted configured
  registry.

Netsuke configuration, not the Netsukefile, owns additional shell definitions.
The compiler resolves a selector into a complete shell invocation before the
execution intermediate representation (IR) crosses into the action runner.

The selector changes the interpreter, not the safety boundary. A selected shell
still receives rendered `invoke` text as source and therefore does not inherit
RFC 0001's direct-mode injection guarantee. Shebang-based `script` items remain
a separate execution contract.

[ADR-019](../adr-019-structured-command-shell-selection.md) is the accepted
architecture decision governing this amendment.

## 2. Problem and current state

[RFC 0001 section 10.1](0001-structured-command-blocks.md#101-selection-and-source-handling)
currently permits only `shell: true`. That spelling selects `/bin/sh -c` on
Unix-like platforms and non-interactive Windows PowerShell on Windows. RFC 0001
section 23 defers explicit dialect selection.

The Boolean contract cannot express that a command needs Bash, PowerShell 7, or
an operator-approved site shell. Letting each Netsukefile provide an executable
and arguments would solve the dialect problem by granting the manifest a much
broader interpreter-selection capability.

Existing `RecipeShell` configuration is recipe-wide legacy policy. A
shebang-based `script` item is a separate recipe item and stream boundary.
Neither is the right representation for a per-block structured-command shell.

## 3. Goals and non-goals

### 3.1 Goals

This amendment aims to:

- preserve the exact meanings of `false` and `true`;
- expose a finite, reviewable built-in shell vocabulary;
- let trusted Netsuke configuration add named shell definitions;
- keep executables and fixed interpreter arguments out of Netsukefiles;
- resolve shell availability through an injected host-environment seam;
- carry a complete resolved invocation through the execution IR;
- preserve structured working-directory, stream, sequence, and pipeline
  semantics;
- produce typed, actionable failures without guessing; and
- keep shell-source risk and the distinct `script` contract explicit.

### 3.2 Non-goals

This amendment does not:

- permit a command block to name an arbitrary executable or fixed argument;
- make shell source safe, quote interpolations automatically, or restrict the
  programmes that source may execute;
- turn the shell registry into a general interpreter or executable registry;
- change legacy scalar commands, legacy command lists, or `RecipeShell`;
- select a shebang or alter a `script` item;
- make a configured shell definition portable to a host on which it is not
  installed; or
- remediate the separate MiniJinja `shell()` and `grep()` command-execution
  capability finding.

## 4. Manifest schema

### 4.1 Selection values

The structured-command field accepts these forms:

```yaml
command:
  - invoke: cargo test --workspace
    shell: false
  - invoke: Write-Output $env:TARGET
    shell: true
  - invoke: set -o pipefail; build-project
    shell: bash
  - invoke: Get-ChildItem Env:
    shell: pwsh
```

The first block is direct mode. The second uses the platform default. The third
and fourth select named shells. An absent `shell` field is equivalent to
`false`.

`ShellName` is a validated name, not a path. Manifest deserialization must
distinguish Boolean values from strings and must reject every other YAML type.
A string is validated syntactically while loading the manifest and resolved
against the trusted registry during compilation.

### 4.2 Valid command positions

A named selector may occur on a singular structured command block, on a block
within a heterogeneous command sequence, or on any structured pipeline stage.
Each block or stage selects and resolves its shell independently.

A named selector may not occur on a legacy string, rule reference, or `script`
item. Those forms retain their existing execution boundaries.

## 5. Trusted shell registry

### 5.1 Built-in names

The initial built-in registry is finite:

| Name         | Supported hosts       | Executable       | Resolution                                        | Fixed arguments                                        |
| ------------ | --------------------- | ---------------- | ------------------------------------------------- | ------------------------------------------------------ |
| `sh`         | Unix-like             | `/bin/sh`        | Fixed absolute path                               | `-c`                                                   |
| `bash`       | Unix-like and Windows | `bash`           | Trusted host `PATH`; `PATHEXT` applies on Windows | `--noprofile`, `--norc`, `-c`                          |
| `pwsh`       | Unix-like and Windows | `pwsh`           | Trusted host `PATH`; `PATHEXT` applies on Windows | `-NoLogo`, `-NoProfile`, `-NonInteractive`, `-Command` |
| `powershell` | Windows               | `powershell.exe` | Trusted host `PATH` and `PATHEXT`                 | `-NoLogo`, `-NoProfile`, `-NonInteractive`, `-Command` |

_Table 1: Initial built-in structured-command shells._

The compiler appends the rendered `invoke` source after the fixed arguments.
`true` resolves to `sh` on Unix-like hosts and `powershell` on Windows.
Configured entries cannot replace these built-in names or change that mapping.

### 5.2 Configured names

`CliConfig` owns additional shell definitions through an append-merged array of
tables:

```toml
[[shells]]
name = "dash"
executable = "/usr/bin/dash"
args = ["-c"]
```

Each `ShellDefinition` contains exactly `name`, `executable`, and `args`.
Configuration loading enforces these rules:

- `name` matches `[a-z][a-z0-9_-]{0,62}`;
- built-in names are reserved and merged configured names are unique;
- `executable` is non-empty UTF-8 without NUL;
- `executable` is a bare executable name or an absolute path, never a relative
  path containing a separator;
- `args` contains between one and sixteen entries without NUL;
- the combined encoded argument size does not exceed 4 KiB; and
- `args` includes the shell's source-evaluation switch because Netsuke always
  appends source as the final argument.

Field-local validation occurs during deserialization. A `PostMergeHook`
validates reserved names and uniqueness after configuration layers compose. An
invalid definition prevents configuration loading.

Configured definitions have no platform field. They are eligible on every host
where the merged configuration is active, and executable resolution decides
whether they are available there. A missing configured executable is therefore
unavailable rather than an unsupported built-in.

This authority belongs exclusively to trusted Netsuke configuration. The
Netsukefile may select a validated name but cannot declare, override, or amend
its executable and arguments.

### 5.3 Resolution authority

Resolution reads the trusted host `PATH` and Windows `PATHEXT` through the
`mockable::Env` seam defined by
[ADR-008](../adr-008-environment-seam-taxonomy.md). It occurs before the
structured command's `env` overlay is applied.

The resolver may reuse the `which` subsystem's executable probing but must
disable current-directory and workspace fallback. A manifest-controlled `PATH`
and a repository-local executable therefore cannot replace an allow-listed
shell.

The registry is private to structured-command shell selection. MiniJinja
helpers, legacy recipes, and script interpreters do not consume it.

## 6. Lowering and execution semantics

### 6.1 AST and resolved IR

The Abstract Syntax Tree (AST) preserves manifest intent:

```rust
pub enum ShellSelection {
    Direct,
    PlatformDefault,
    Named(ShellName),
}
```

The compiler resolves either shell choice before constructing the execution IR.
The resolved value records the registry name, executable path, and fixed
arguments:

```rust
pub enum ProcessKind {
    Direct {
        program: String,
        args: Vec<String>,
    },
    Shell {
        source: String,
        shell: ResolvedShell,
    },
}
```

`false` lowers to `Direct`. `true` first maps to the host default and then
resolves exactly like a named built-in. A string resolves against the built-in
and configured registry. The IR contains neither an unresolved name nor a
configuration lookup.

The composition root supplies the registry and environment port. The compiler
produces the domain value, and the Ninja and action-runner adapters consume it
without consulting configuration or ambient process state again.

### 6.2 Working directories and streams

Named-shell blocks use the same `env`, `cwd`, `stdin`, `stdout`, `stderr`,
`tee`, `capture_stdout`, `temp_dir`, and `pipe` semantics as every other
structured block. Relative stream paths resolve against that stage's working
directory. Pipeline construction, relay ownership, teeing, capture bounds, and
failure collection remain Netsuke-managed.

The shell receives only the rendered `invoke` source. An inner pipe or
redirection written in that source belongs to the selected shell. An outer
structured `pipe`, standard-stream binding, or working directory belongs to
Netsuke. Shell state does not carry between sequence items or pipeline stages.

### 6.3 Script items remain distinct

A `script` item retains recipe-level shebang and interpreter handling. It
remains an execution and stream boundary under RFC 0001 sections 6.3 and 13.1.
A structured-command shell selector neither supplies a shebang nor changes the
script interpreter.

## 7. Diagnostics and observability

Shell resolution classifies failures internally as:

- **unknown name** when no built-in or configured entry matches;
- **unsupported shell** when a valid built-in does not support the host;
- **unavailable shell** when the executable cannot be resolved or executed
  safely; and
- **misconfigured shell** when a configured definition or persisted action
  plan violates its contract.

The compiler retains this typed classification and converts it to a localized,
user-facing diagnostic at one application boundary, following
[ADR-005](../adr-005-typed-which-resolve-error.md). Diagnostics identify the
action, target or rule, sequence item, and pipeline stage where available. They
suggest supported built-ins or the relevant configuration entry without
printing complete `PATH` values.

Unsupported and ambiguous cases fail instead of selecting another shell,
following
[ADR-014's safe-failure rule](../adr-014-backend-text-escaping-seam.md#decision).

Structured tracing may record the bounded registry name, selection class, host
support result, and resolution result. It must not record shell source,
complete environment values, or unbounded paths.

## 8. Safety model

The allow-list constrains the interpreter and fixed arguments that a manifest
may select. It does not constrain the meaning of `invoke`. The selected shell
receives rendered `invoke` as source, so an interpolated value may alter
quoting, expansion, redirection, operators, or control flow.

Named shell blocks therefore carry the same warning as `shell: true`: they do
not inherit direct mode's process-structure or argv-boundary injection
guarantees. Manifest authors should use direct mode unless shell-language
semantics are required.

This interpreter allow-list is only a partial hardening of shell execution. It
does not remediate unrestricted commands issued by MiniJinja `shell()` and
`grep()` helpers. The security audit must keep that capability finding open
until a separate command-level control or disable mechanism is implemented.

## 9. Compatibility and manifest version

The change is additive:

- absent and `false` retain direct argv execution;
- `true` retains the existing platform-shell behaviour;
- legacy strings, string lists, rule references, and scripts are unchanged;
  and
- a string is new syntax understood only by implementations of this amendment.

RFC 0001 already reserves an additive manifest-format minor version,
provisionally `1.1.0`, for structured command mappings. Named shell selection
ships as part of that not-yet-implemented mapping schema and does not require a
second version increment. If structured command mappings ship before this
amendment, the implementation must allocate the next additive minor version
instead.

Persisted action plans must version the resolved-shell representation and
reject unknown variants rather than falling back to the host default.

## 10. Implementation phases

### 10.1 Phase 1: Accept the governing decision

- Accept ADR-019 before changing the schema or implementing shell selection.
- Record the built-in registry, configuration authority, lowering, semantics,
  diagnostics, safety model, and script boundary in that decision.

### 10.2 Phase 2: Amend the structured-command contract

- Amend RFC 0001's field table, shell semantics, AST, IR, compatibility,
  implementation, test, and open-question sections.
- Cross-reference ADR-019 and this RFC from RFC 0001.
- Keep the manifest version compatible with the structured-command rollout.

### 10.3 Phase 3: Build shell-selection primitives

- Add the finite built-in `ShellName` domain type and the Boolean-or-name
  manifest selection type.
- Add configured `ShellDefinition` values and post-merge registry validation
  to `CliConfig`.
- Add trusted executable resolution, a typed internal error, and explicit
  selection-to-IR conversion.
- Do not wire these primitives into a structured-command AST until RFC 0001's
  implementation creates that AST.

### 10.4 Phase 4: Integrate, verify, and document

- Carry resolved shells through singular blocks, sequence items, and pipeline
  stages when structured commands are implemented.
- Add cross-platform unit, behavioural, and end-to-end coverage for built-ins,
  configured names, invalid definitions, unavailable and unsupported shells,
  and the `true` and `false` compatibility cases.
- Document configuration, execution, and security guidance for users and
  developers.
- Update the security audit as a partial-hardening note while keeping the
  unrestricted MiniJinja command-helper finding open.

## 11. Test strategy

Implementation must include:

- host-independent table tests for every built-in name, supported-host rule,
  executable, and fixed argument list;
- platform-specific tests for `true` selecting the existing host default;
- deserialization tests for `false`, `true`, valid names, invalid names, and
  non-Boolean, non-string values;
- configuration tests for valid definitions, reserved and duplicate names,
  invalid paths and arguments, and post-merge collision validation;
- resolver tests with `mockable::MockEnv` and injected executable probes for
  found, unavailable, unsupported, and ambiguous cases;
- tests proving command-level `env` and workspace executables cannot replace a
  trusted shell;
- conversion tests proving direct selection never enters shell resolution and
  every shell selection yields a complete `ResolvedShell`;
- sequence and pipeline tests proving per-stage shell, working-directory, and
  stream independence; and
- cross-platform end-to-end tests for direct, platform-default, built-in, and
  configured selection.

## 12. Alternatives considered

### 12.1 Per-command executable and arguments

Allowing each block to supply an executable and fixed arguments is flexible but
moves interpreter authority into the Netsukefile. It is rejected in favour of
finite names controlled by built-ins and trusted configuration.

### 12.2 Boolean selection only

Retaining only `true` and `false` avoids a schema extension but cannot express
a required shell dialect. It leaves portable PowerShell and Bash selection to
legacy or script workarounds and is rejected.

### 12.3 Configured overrides for built-ins

Letting configuration replace `sh`, `bash`, `pwsh`, or `powershell` makes a
built-in selector and the compatibility meaning of `true` depend on
configuration composition. Built-in names are therefore reserved.

### 12.4 Resolve through the command environment

Using the structured command's effective `PATH` would let a Netsukefile point
an allow-listed name at a repository-controlled executable. Resolution uses the
trusted host environment captured before the command overlay instead.

### 12.5 Fold selection into `script`

A script is a file-like recipe item with shebang and interpreter semantics. A
shell command block is rendered source within structured working-directory and
stream semantics. Combining them would erase a useful execution boundary and is
rejected.

## 13. Open questions and future extensions

The accepted decision settles the schema, initial built-ins, configuration
authority, lowering, supported positions, diagnostics, safety boundary, and
relationship to scripts. No open design question blocks implementation.

Future proposals may add a built-in shell, introduce a separately governed
interpreter registry, or constrain shell-source capabilities. Each extension
must preserve the trusted authority boundary and require explicit review.

## 14. Recommendation

Adopt the `Boolean | ShellName` schema and the allow-listed registry defined by
ADR-019. Preserve direct execution as the default, retain `true` exactly for
compatibility, and resolve every named shell into a complete IR value before
the runner boundary.

## 15. References

- [ADR-019: Select allow-listed structured-command
  shells](../adr-019-structured-command-shell-selection.md)
- [RFC 0001: Structured command blocks and argv
  templates](0001-structured-command-blocks.md)
- [Issue #593: Land structured recipes, Git-aware change detection, and
  OrthoConfig v0.10 integration](https://github.com/leynos/netsuke/issues/593)
- [Issue #598: Gate v0.1.0 with downstream migration
  canaries](https://github.com/leynos/netsuke/issues/598)
- [Pull request #600: Structured execution contexts and manifest
  composition](https://github.com/leynos/netsuke/pull/600)
- [Issue #638: Define structured-command shell-selection
  behaviour](https://github.com/leynos/netsuke/issues/638)
