# Architectural decision record (ADR) 018: Terminate Ninja options before build targets

## Status

Accepted.

## Date

2026-09-02

## Context and problem statement

Netsuke builds a Ninja command from trusted process options and target operands
drawn from either explicit CLI arguments or `default_targets` in configuration.
Before this decision, those operands followed Netsuke's `-j` and `-f` options
directly. Ninja therefore continued parsing a leading operand such as `-C` or
`-f` as an option, allowing input that was meant to name a target to alter the
child process's control plane.

`std::process::Command` keeps argument values out of a shell, but it does not
tell Ninja where its options end. The build path needs the same explicit
option-separation discipline as the bounded Git vectors in
[ADR-015](adr-015-use-bounded-git-cli-for-change-detection.md).

## Decision

For every Ninja build invocation with one or more selected targets, append a
literal `--` after all Netsuke-owned Ninja options and before the target
operands. The resulting shape is:

```plaintext
ninja ... -f <generated-build-file> -- <targets...>
```

The command omits the terminator when no targets are selected. Both explicit
CLI targets and configured `default_targets` use the same `BuildTargets` path
and therefore receive the same boundary. Ninja tool invocations remain
unchanged because they append only Netsuke-owned fixed operands.

## Rationale

- **Ninja owns its operand grammar.** `--` is Ninja's native, unambiguous
  option terminator, so it preserves the child program's representation of
  unusual literal target names.
- **One convergence point protects both sources.** Applying the boundary in
  the build command configurator prevents drift between CLI and configuration
  paths.
- **The existing no-target shape remains stable.** Omitting `--` for an empty
  list avoids depending on version-specific empty-operand behaviour while
  retaining the previous invocation shape.

## Consequences

- A configured list such as `default_targets = ["-f", "evil.ninja"]` is
  passed as target operands and cannot replace Netsuke's generated build file.
- Explicit inputs such as `-C ../evil default` are target operands and cannot
  change Ninja's working directory.
- Tests must retain exact argv assertions for empty, single-target, and
  multi-target build requests, plus integration coverage using real Ninja.
- Clean and graph modes are unaffected because they do not forward a
  user-controlled target list through the build command configurator.

## Alternatives considered

- **Reject leading hyphens in targets.** Rejected because Ninja can represent
  unusual literal target names, and the child program already provides the
  correct operand boundary. Rejection would reduce legitimate target support
  while leaving Netsuke responsible for an incomplete approximation of Ninja's
  grammar.
- **Rely on `std::process::Command` argument separation.** Rejected because it
  prevents shell metacharacter interpretation, not option parsing by the child
  program.
- **Add a separate configured-target command path.** Rejected because the
  shared `BuildTargets` path is the narrowest reliable place to enforce the
  rule for both input sources.

## Implementation references

- [Ninja build command configurator](../src/runner/process/configure.rs)
- [Runner build-target selection](../src/runner/mod.rs)
- [Default-target runner coverage](../tests/runner_cases/default_targets.rs)
- [Netsuke design, section 6.1](netsuke-design.md#61-invoking-ninja)
- [ADR-015: bounded Git CLI queries](adr-015-use-bounded-git-cli-for-change-detection.md)
