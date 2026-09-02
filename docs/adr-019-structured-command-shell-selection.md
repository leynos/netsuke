# Architecture decision record (ADR) 019: Select allow-listed structured-command shells

## Status

Accepted. Netsuke will retain Boolean structured-command shell selection and
add named selection through a finite built-in registry plus trusted
configuration entries.

## Date

2026-09-02.

## Context and problem statement

[RFC 0001 section 10.1](rfcs/0001-structured-command-blocks.md#101-selection-and-source-handling)
defines `shell: true` for a structured command block. It selects the platform
shell and treats `invoke` as shell source. The RFC deliberately defers a named
selector in section 23, leaving manifests unable to request a known shell
dialect without falling back to legacy recipe-wide behaviour or a `script` item.

The selector must preserve the compatibility meaning of both Boolean values,
keep arbitrary executable paths and interpreter arguments out of the
Netsukefile, and establish one trusted authority for adding shells. It must
also keep shell source distinct from RFC 0001's direct argument-vector (argv)
safety guarantee and from the existing shebang-based `script` contract.

## Decision

### Selection contract and scope

The structured command `shell` field is a `Boolean | ShellName` union:

- an absent field or `false` selects direct argv execution;
- `true` selects the platform default shell; and
- a string selects a built-in or configured shell by its allow-listed name.

A named selector is valid on any structured command block: a singular block, a
block in a heterogeneous sequence, or any stage of a structured pipeline. It is
not valid on a legacy command string, a rule reference, or a `script` item.
Rule and script items remain execution and stream boundaries.

Named shell blocks retain all Netsuke-managed environment, working-directory,
standard-stream, capture, temporary-directory, sequencing, and pipeline
semantics from RFC 0001 sections 10.3 to 14. Each stage resolves its own
working directory and stream bindings. Shell process state never carries to a
neighbouring structured stage.

### Built-in registry

The initial built-in set is the following finite enumeration. Netsuke appends
the rendered `invoke` source as the final process argument after the fixed
arguments shown in the table.

| Name         | Supported hosts       | Executable       | Resolution                                        | Fixed arguments                                        |
| ------------ | --------------------- | ---------------- | ------------------------------------------------- | ------------------------------------------------------ |
| `sh`         | Unix-like             | `/bin/sh`        | Fixed absolute path                               | `-c`                                                   |
| `bash`       | Unix-like and Windows | `bash`           | Trusted host `PATH`; `PATHEXT` applies on Windows | `--noprofile`, `--norc`, `-c`                          |
| `pwsh`       | Unix-like and Windows | `pwsh`           | Trusted host `PATH`; `PATHEXT` applies on Windows | `-NoLogo`, `-NoProfile`, `-NonInteractive`, `-Command` |
| `powershell` | Windows               | `powershell.exe` | Trusted host `PATH` and `PATHEXT`                 | `-NoLogo`, `-NoProfile`, `-NonInteractive`, `-Command` |

_Table 1: Initial built-in structured-command shells._

`true` maps to `sh` on Unix-like hosts and the `powershell` invocation on
Windows. This preserves RFC 0001 section 10.1 even when a similarly named
configured entry exists. Built-in names are reserved and cannot be overridden.

Shell resolution captures the host `PATH` and, on Windows, `PATHEXT` through the
[`mockable::Env` seam](adr-008-environment-seam-taxonomy.md). It may reuse the
executable lookup machinery under `src/stdlib/which/`, but it disables
current-directory and workspace fallback. It also resolves before applying a
structured command's `env` overlay. These rules keep executable authority with
the Netsuke operator: manifest-controlled `PATH` values and repository-local
files cannot replace an allow-listed shell.

### Configured shell registry

`CliConfig` owns additional shell definitions. They are configuration data, not
Netsukefile data, and they are unavailable as command-line flags. The
configuration schema is an append-merged array of tables:

```toml
[[shells]]
name = "dash"
executable = "/usr/bin/dash"
args = ["-c"]
```

Each `ShellDefinition` contains exactly `name`, `executable`, and `args`:

- `name` must match `[a-z][a-z0-9_-]{0,62}`;
- `name` must not equal a built-in name, and merged names must be unique;
- `executable` must be non-empty, valid UTF-8, and contain no NUL;
- `executable` must be either a bare executable name or an absolute path;
  relative paths containing a path separator are invalid;
- `args` must contain between one and sixteen fixed arguments;
- every fixed argument must contain no NUL, and their combined encoded size
  must not exceed 4 KiB; and
- Netsuke always appends the rendered shell source after `args`; configuration
  must therefore include the shell's source-evaluation switch.

Field-local rules run while each entry is deserialized. `PostMergeHook` checks
reserved-name collisions and duplicate configured names after all configuration
layers have composed. An invalid definition fails configuration loading before
a manifest is compiled.

Configured definitions have no platform declaration. Each is eligible on every
host where the merged configuration is active, and executable resolution
determines its availability on that host. A missing configured executable is
therefore unavailable, not an unsupported built-in.

The configuration file is the authority boundary. A configured executable may
be an absolute host path because the operator who controls `CliConfig` already
controls Netsuke's execution policy. A Netsukefile may only select the
validated name; it cannot provide or alter the executable or fixed arguments.

The shell registry is feature-private. The composition root constructs it from
`CliConfig`, and structured-command validation and lowering consume it. It is
not a general interpreter registry and must not be reused by the MiniJinja
`shell()` or `grep()` helpers, legacy `RecipeShell` selection, shebang
handling, or arbitrary executable discovery.

### Lowering and execution intermediate representation

The Abstract Syntax Tree (AST) preserves the three user choices as `Direct`,
`PlatformDefault`, and `Named(ShellName)`. Resolution converts either shell
choice into a `ResolvedShell` containing a registry name, a resolved executable
path, and fixed arguments. The execution intermediate representation (IR)
contains no Boolean selector and no unresolved configuration lookup:

```rust,no_run
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

The manifest-facing shell name and configured-definition types belong outside
the execution IR. The IR receives only the resolved domain value, preserving
the existing AST-to-IR boundary exemplified by `DependencyOrder`. The Ninja
adapter and action runner consume that value without consulting `CliConfig` or
the process environment again.

### Diagnostics

Resolution uses a crate-internal typed error with at least these categories:

- **unknown name:** the selector is neither built in nor configured;
- **unsupported shell:** the built-in name is valid but does not support the
  current platform;
- **unavailable shell:** the definition supports the platform, but its
  executable is absent, not executable, or cannot be resolved safely; and
- **misconfigured shell:** a configured definition violates its field or
  collision rules, or a persisted action plan contains an invalid shell
  definition.

Configuration errors identify the configuration key and definition name.
Manifest diagnostics identify the action, target, or rule, command-list item,
and pipeline stage, then suggest the supported built-ins or the relevant
`CliConfig` entry. They use bounded path previews and do not print complete
`PATH` values. Domain classification remains internal and is converted to a
localized, user-facing diagnostic at one application boundary, following
[ADR-005](adr-005-typed-which-resolve-error.md). Unsupported and ambiguous
cases fail rather than guessing, following the safe-failure rule in
[ADR-014](adr-014-backend-text-escaping-seam.md).

### Safety and the `script` boundary

Any selected shell receives rendered `invoke` text as shell source. A Jinja
value can therefore alter quoting, operators, expansion, redirection, or
control flow. Named selection does not inherit direct mode's injection
guarantee. The allow-list constrains which interpreter and fixed arguments a
manifest may select; it does not make the source safe or restrict which
programmes that source may execute.

This distinction means the registry does not remediate the separate audit
finding that the MiniJinja `shell()` and `grep()` helpers permit arbitrary
command execution. That finding still requires a capability-disable control or
a command-level allow-list. Recording the interpreter registry as a complete
remediation would overstate its security effect.

`script` remains a separate recipe item. It retains recipe-level shebang and
interpreter handling and remains an execution and stream boundary under RFC
0001 sections 6.3 and 13.1. A named structured-command shell neither supplies a
shebang nor changes a script item's interpreter.

## Rationale

- **Compatibility by construction.** The Boolean variants preserve RFC 0001's
  direct and platform-default behaviour without making existing manifests pick
  a name.
- **Finite manifest authority.** A manifest selects a reviewed name rather
  than an executable path or interpreter argument list.
- **Trusted composition.** `CliConfig` owns host policy, while the AST and IR
  remain free of configuration-adapter types.
- **Resolver integrity.** Ignoring manifest `PATH`, current-directory lookup,
  and workspace fallback prevents repository content from replacing a trusted
  shell name.
- **Hexagonal boundary.** Configuration and environment adapters compose a
  resolved domain value before it enters the execution IR. The runner does not
  reach back into those adapters.
- **One diagnostic conversion.** Typed internal outcomes remain useful to
  tests and policy code without coupling behaviour to localized message text.
- **Explicit danger.** A named selector communicates dialect intent while
  retaining the visible shell-source trust boundary.

## Consequences

- RFC 0001's field schema, AST sketch, IR sketch, compatibility text, test
  strategy, and open questions must be amended before implementation.
- The structured-command implementation must introduce separate
  manifest-facing selection, registry, resolution, and resolved-IR types.
- `CliConfig` gains an append-merged structured field and post-merge validation;
  sample configuration and configuration documentation must describe it.
- Built-in shell support is platform-specific. A portable manifest must either
  use `true`, select a cross-platform installed shell such as `pwsh`, or
  arrange matching trusted configuration on every target host.
- Generated action plans must version the resolved-shell representation and
  reject unknown variants rather than silently choosing a default.
- Cross-platform tests must cover each built-in, configured definitions,
  duplicate and reserved names, unavailable executables, unsupported hosts, and
  the `false` and `true` compatibility cases.
- User and developer documentation must distinguish interpreter allow-listing
  from shell-source safety and from the unresolved MiniJinja command-helper
  capability finding.

## Alternatives considered

- **Allow a per-command executable and argument list.** Rejected because it
  moves trusted execution policy into the Netsukefile and lets every command
  define a new interpreter surface.
- **Accept only Boolean selection.** Rejected because it cannot express a
  required shell dialect and keeps the RFC 0001 section 23 question unresolved.
- **Replace `true` with a required name.** Rejected because it breaks the
  compatibility spelling and forces existing manifests to know the host default.
- **Let configured names override built-ins.** Rejected because `true` and a
  built-in selector would then vary with configuration layering.
- **Resolve through the command's effective `PATH`.** Rejected because a
  Netsukefile could redirect an allow-listed name to a repository-controlled
  executable.
- **Reuse the workspace fallback from `which`.** Rejected for the same reason;
  command discovery and trusted interpreter resolution have different authority
  boundaries.
- **Treat a shebang as a shell selector.** Rejected because shebang parsing,
  script files, and script stream boundaries belong to the distinct `script`
  item contract.

## Implementation references

- Legacy recipe shell value: [`src/recipe_shell.rs`](../src/recipe_shell.rs)
- CLI configuration: [`src/cli/config.rs`](../src/cli/config.rs)
- Executable lookup: [`src/stdlib/which/`](../src/stdlib/which/)
- Structured command contract:
  [RFC 0001](rfcs/0001-structured-command-blocks.md)
