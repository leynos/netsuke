# Architectural decision record (ADR) 0071: Bind Windows recipe interpreters to trusted absolute paths

## Status

Accepted.

## Date

2026-11-02.

## Context and problem statement

Generated Windows recipes invoked their PowerShell parent by the unqualified
name `powershell.exe`. Ninja launches recipe commands through `CreateProcess`
without an explicit application name (assumption A4), so Windows applies its
ordinary executable search order: the application directory, the current
directory, the system directories, then `PATH`. A workspace-controlled
`powershell.exe` in the build's current directory, or in an earlier `PATH`
entry, is therefore selected ahead of the real interpreter and runs with the
victim build process's authority. This is CWE-427, uncontrolled search path.

The same hazard applied to the Windows Bash compatibility route for `generate`:
a saved plan retained the literal `bash.exe` and resolved it from the
execution-time `PATH`. The runner's build path already bound the Bash
executable it had probed, but the library and `generate` surfaces did not
require any binding, and nothing bound PowerShell at all.

Two constraints shape the remedy. Rendering a command is a pure transformation
of its recipe text and binding, so it can neither perform I/O nor consult
ambient state; and query-only callers, such as target discovery, must render
without a resolvable runtime because they never execute the result.

## Decision drivers

- An unqualified interpreter name is unsafe whenever Ninja, not Netsuke,
  performs executable search.
- The renderer must remain a pure function of its inputs.
- A binding must be authenticated once and reused by preflight, rendering and
  execution, so no window exists for a different file to be selected.
- Existing manifests, including the Windows PowerShell legacy recipes and the
  explicit Bash compatibility route, must keep working on supported hosts.
- Query-only generation must not require an installed runtime and must never
  hand an inspection-only plan to an executing Ninja run.

## Options considered

### Option A: Quote the interpreter name

Rejected. Quoting affects argument parsing, not executable search. A quoted
bare name is still resolved through the same directories.

### Option B: Resolve `powershell.exe` through `PATH` at render time

Rejected. `PATH` is caller-controlled and the first match may be a
workspace-controlled decoy. Reading ambient environment state would also make
the renderer impure.

### Option C: Search the current directory and `SystemRoot`/`WINDIR` by hand

Rejected. Those variables are inherited from the launching process and can be
overridden; a hand-rolled search also duplicates Windows path rules and invites
the same race in a new form.

### Option D: Construct the interpreter from the Known Folder API and bind its absolute path

Chosen. The system directory comes from `KnownFolder::System`, which Windows
resolves independently of `PATH`, the current directory and environment
strings. The interpreter is always
`<System>\WindowsPowerShell\v1.0\powershell.exe`; `pwsh.exe` is not a
substitute, and no candidate is executed as a probe.

## Decision outcome

`src/recipe_shell/power_shell_location.rs` constructs the candidate path from
an injected `SystemDirectory` port, implemented for production by the
`known-folders` crate's `KnownFolder::System`. On non-Windows targets the
locator does not exist. The resolved path must be UTF-8, absolute, and a
regular file that is not a reparse point, following the existing reparse policy
in `src/stdlib/path/windows_reparse/`. Failure returns a typed error naming the
expected path.

The resolved path becomes an `InterpreterExecutable`, the renamed and now shared
`BashExecutable`, which validates that the path is absolute and contains no
NUL. Recipe rendering accepts one of three private binding modes: `Bound`,
which renders quoted absolute paths; `InspectionOnly`, which renders logical
names and is reserved for renderer tests and future query-only callers, since
no production path renders an executable plan through it; and `Unbound`, which
rejects PowerShell and Bash commands with
`NinjaGenError::InterpreterBindingRequired`. Dependency-only graphs render
successfully under `Unbound` because they have no command.

Default resolution sits above the renderer rather than inside it. The library
entry points `generate`, `generate_into`, `generate_with_shell` and
`generate_bundle` call `resolve_default_interpreter` in `src/ninja_gen/mod.rs`,
which invokes `locate_windows_power_shell` directly: that performs a Known
Folder query and a filesystem inspection, binds only `RecipeShell::PowerShell`
on Windows, and binds nothing elsewhere. `src/ninja_gen` cannot depend on the
runner, so this is a deliberate second call site for the same port rather than
a shared dispatcher. The `*_with_interpreter` entry points skip that
resolution, and the internal `generate_into_with_binding` path receives an
already-resolved binding, so rendering itself remains a pure function of the
graph, shell and binding once the binding exists. The resolved path is
therefore a host-dependent input to generation, of the kind ADR-0030's
determinism key covers through its environment and platform elements.

The two resolution policies are not interchangeable. Library callers that select
`RecipeShell::Bash` render under `Unbound`, because the Bash runtime lookup
belongs to the runner and needs an injected `PATH`; such a caller must supply an
`InterpreterExecutable` through `generate_with_interpreter` or
`generate_bundle_with_interpreter`, or accept
`NinjaGenError::InterpreterBindingRequired`. The runner binding below is what
binds Windows Bash for CLI operations.

`src/runner/recipe_shell/mod.rs` dispatches binding resolution. Windows
PowerShell uses the trusted locator; Windows Bash uses the unchanged
`resolve_bash_runtime` lookup and probe; other platforms need no binding.
Bindings are resolved for build operations, Ninja-tool operations including
clean, and `generate`. `generate` now writes the absolute Bash path, or fails
with a missing-runtime diagnostic and writes no plan.

The renderer counts the complete unescaped inline command in UTF-16 code units
before applying Ninja escaping, so the 32,766-character limit is measured in
the units `CreateProcess` uses. A response-file parent that would itself exceed
the limit is rejected rather than emitted.

Isolated PowerShell list entries continue to launch `"$PSHOME\powershell.exe"`
from the already-bound parent, which keeps the child tied to the same
installation as the trusted parent.

## Consequences

- A decoy `powershell.exe` in the current directory or first on `PATH` is no
  longer selected, on any recipe size, including response-file transport.
- The binding authenticates location only. No Authenticode signature check
  runs, and the resolution-to-launch race remains: a file swapped in at the
  bound path between resolution and launch is still executed.
- A 32-bit WOW64 build reads the 32-bit system directory, so it binds
  `SysWOW64\WindowsPowerShell\v1.0\powershell.exe`. CI builds and ships 64-bit
  binaries, and no native Arm64 runner currently covers this path; Arm64
  coverage depends on the emulated system directory the Known Folder API
  reports.
- The Windows Bash route still trusts the first `bash.exe` on the explicitly
  injected `PATH`; that is the documented compatibility contract, not a
  trusted-location policy.
- Bash recipes may still launch `/bin/sh` internally, and bare names in
  `src/shell_selection/built_in/` remain user-authored recipe content rather
  than interpreter selection.
- Library callers that render recipes must supply a binding through
  `generate_with_interpreter` or `generate_bundle_with_interpreter`, or accept
  `InterpreterBindingRequired`. This is a public API addition and a behaviour
  change for Bash `generate`.
- The renderer, the runner dispatcher and the test transport detector all
  reject the bare name `powershell.exe`, so a regression to unqualified
  invocation fails a deterministic gate.

## Architectural rationale

The defect is that executable search happens in a process the application does
not control. Resolving through a port and passing an absolute path removes that
search entirely; the three binding modes keep the pure renderer honest while
preserving a query-only path that never executes. The shared
`InterpreterExecutable` makes the authenticated path a value, so preflight,
rendering and launch cannot disagree about which file they mean, which is the
same identity principle ADR-0024 applies to `which` resolution and ADR-0045
applies to isolated command-list entries.

## References

- [Issue #980](https://github.com/leynos/netsuke/issues/980) reported the
  unqualified `powershell.exe` parent.
- [ADR-0024](adr-0024-require-explicit-recursive-which-search.md) requires
  explicit `which` search rather than an implicit recursive workspace fallback,
  which is the same trust-boundary principle for manifest names.
- [ADR-0045](adr-0045-isolate-command-list-entries.md) defines isolated
  PowerShell entries, whose `$PSHOME` launch is retained.
- [`src/recipe_shell/power_shell_location.rs`](../../src/recipe_shell/power_shell_location.rs)
  constructs the trusted candidate.
- [`src/runner/recipe_shell/mod.rs`](../../src/runner/recipe_shell/mod.rs)
  dispatches binding resolution.
- [`src/ninja_gen/recipe_shell/mod.rs`](../../src/ninja_gen/recipe_shell/mod.rs)
  applies the binding and the UTF-16 length policy.
- [`tests/windows_recipe_interpreter_binding_tests.rs`](../../tests/windows_recipe_interpreter_binding_tests.rs)
  exercises current-directory and `PATH` decoys on Windows.
- [`scripts/windows-recipe-smoke/powershell-paths.ps1`](../../scripts/windows-recipe-smoke/powershell-paths.ps1)
  runs the same decoy scenario through the native smoke workflow.
