# Architectural decision record (ADR) 034: Preserve script `$in` and `$out` as shell variables

## Status

Accepted.

## Date

2026-09-20

## Context and problem statement

ADR-014 introduced a backend seam that turns completed `ShellText` into a
Ninja-safe `NinjaValue`. It correctly doubles residual dollars at that seam so
the child shell receives variables such as `$PATH` unchanged. Its script
interpolation decision treated `$in` and `$out` differently: script recipes
lowered them to input and output paths before reaching the seam.

That decision conflated three separate concepts. A **marker** is the
Netsuke-owned `{{ ins }}` or `{{ outs }}` placeholder. An **internal token** is
the `INS_TOKEN` or `OUTS_TOKEN` sentinel that survives only between rendering
and interpolation. A **shell variable** is handwritten text such as `$PATH`,
`$in`, `$out`, `$ins`, or `$outs` that Netsuke leaves to the selected shell.

Treating `$in` and `$out` as script markers made their meaning depend on recipe
form and caused generated Ninja to retain its own input and output markers. The
documented marker contract already provides `{{ ins }}` and `{{ outs }}` for
Netsuke-owned path substitution without overloading shell-variable syntax.

## Decision drivers

- Keep Netsuke's marker contract independent of Ninja syntax.
- Preserve ordinary shell variables uniformly in `command:` and `script:`
  recipes.
- Retain the existing `ShellText` to `NinjaValue` boundary and its exact-once
  escaping guarantee.
- Preserve contextual path quoting and marker rejection inside protected shell
  syntax.

## Decision outcome

Scripts lower only `{{ ins }}` and `{{ outs }}` markers, exactly as command
recipes do. The first interpolation stage represents those markers as the
private `INS_TOKEN` and `OUTS_TOKEN` internal tokens; the later stage lowers
the tokens to quoted paths.

`$in` and `$out` are shell variables in every recipe. They are not Netsuke
markers or internal tokens, so script interpolation leaves them unchanged. The
Ninja backend then doubles their dollars with every other residual shell
dollar, preventing Ninja from expanding them as its built-in input or output
variables.

ADR-014's `ShellText` / `NinjaValue` seam stays unchanged. So do the
`{{ ins }}` / `{{ outs }}` marker contract, contextual path quoting, and
backtick or command-substitution rejection for markers.

## Consequences

- A script that needs Netsuke input or output paths uses `{{ ins }}` or
  `{{ outs }}`.
- A script that writes `$in`, `$out`, `$ins`, or `$outs` receives those names
  from its selected shell, subject to that shell's normal environment rules.
- Generated Ninja no longer interprets script `$in` or `$out` as its own
  built-in variables.
- Tests and documentation distinguish markers, internal tokens, and shell
  variables explicitly.

## Alternatives considered

- **Keep script `$in` and `$out` lowering.** Rejected because it overloads
  shell-variable syntax and contradicts the marker contract.
- **Leave script `$in` and `$out` for Ninja to expand.** Rejected because the
  backend must double residual dollars to preserve shell text, and allowing
  these two names through would couple the IR to Ninja.
- **Change the `ShellText` / `NinjaValue` seam.** Rejected because that seam
  already correctly owns backend escaping; changing it is unnecessary for this
  interpolation correction.

## Implementation references

- Script token matching:
  [`src/ir/cmd_interpolate/mod.rs`](../src/ir/cmd_interpolate/mod.rs)
- Script traversal:
  [`src/ir/cmd_interpolate/script_substitution.rs`](../src/ir/cmd_interpolate/script_substitution.rs)
- Backend escaping: [`src/ninja_gen_escape.rs`](../src/ninja_gen_escape.rs)
- Regression coverage:
  [`tests/ninja_dollar_escaping_tests.rs`](../tests/ninja_dollar_escaping_tests.rs)

## Supersedes

This record supersedes only ADR-014's decision to lower script `$in` and
`$out`. Its backend-escaping, path, metadata, and marker decisions remain in
force.
