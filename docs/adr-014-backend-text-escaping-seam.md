# Architectural decision record (ADR) 014: Use a typed Ninja text-escaping seam

## Status

Accepted.

## Date

2026-08-24

## Context and problem statement

Netsuke compiles manifest recipes into a backend-neutral intermediate
representation (IR), then writes a `build.ninja` file. Ninja treats `$` as its
own escape and variable marker, whereas recipes use `$` for ordinary shell
variables such as `$PATH` and `${CARGO:-cargo}`. Emitting IR command text
verbatim caused Ninja either to erase a shell variable or reject the file.

Netsuke's `$in`, `$out`, `{{ ins }}`, and `{{ outs }}` placeholders have a
different meaning. They must be resolved while lowering the manifest into the
IR; the backend must never reinterpret them. Applying Ninja escaping earlier
would prevent that lowering, while applying it twice changes the shell text.

Paths are not shell text. Ninja's path grammar also gives special meaning to
dollars, spaces, colons, and control characters. Escaping only recipe text
while continuing to write such paths raw would produce a corrupt dependency
graph.

## Decision

Netsuke uses a private typed conversion at the Ninja writer boundary:

- IR commands and scripts remain plain `ShellText` after Netsuke has resolved
  its placeholders.
- The Ninja writer consumes that text once and produces an opaque
  `NinjaValue`. The conversion doubles each remaining dollar sign and rejects
  newline, carriage-return, and NUL control characters.
- Only the completed `NinjaValue` is written as a Ninja `command` binding;
  descriptions, `depfile`, `deps`, and `pool` are escaped at their Ninja
  emission boundary and reject newline, carriage-return, and NUL characters.
- Build-edge paths remain separate values and are rejected when they contain a
  dollar, space, colon, newline, carriage return, or NUL.
- A script uses substitution-only lowering, preserving script syntax such as
  heredocs. A Netsuke placeholder found inside backticks is rejected with a
  typed IR diagnostic rather than silently reaching the shell unlowered.

## Rationale

- **Backend ownership:** Ninja syntax belongs at the Ninja adapter, not in the
  IR, so a future backend receives ordinary shell text.
- **Ordering by construction:** the conversion accepts only completed shell
  text from the action writer, leaving no API that accepts pre-lowered manifest
  text or a `NinjaValue` for a second conversion.
- **Safe failure:** an unsupported path or control character is reported
  before an ambiguous or injectable `build.ninja` file is produced.
- **Author ergonomics:** manifest authors use ordinary shell syntax without
  learning Ninja's escaping rules.

## Consequences

- Existing manifests that wrote the former workaround `$$PATH` must change to
  `$PATH`; otherwise the shell receives `$$PATH`, whose first two dollars are
  its process identifier.
- Script actions that use `$in` or `$out` now lower those tokens before their
  action hash is calculated. Their generated rule IDs change once, so Ninja may
  rebuild them once.
- A path using a Ninja-special character is rejected rather than supported by
  partial escaping. Expanding the accepted path grammar is separate work.
- CI sets `NETSUKE_REQUIRE_NINJA=1`, so real-Ninja coverage fails rather than
  skipping when the executable is absent.
- Kani and Verus are not used for this change. The boundary is a finite string
  transformation covered by property tests and Ninja-as-lexer differential
  tests; neither tool would add proportionate assurance here.

## Alternatives considered

- **Escape dollars in the IR.** Rejected because it couples the IR to Ninja and
  would force every later backend to undo or tolerate Ninja syntax.
- **Escape only `$PATH`-shaped variables.** Rejected because shell syntax also
  includes braced expansions, substitutions, positional variables, and literal
  dollars; the backend must preserve all residual dollars uniformly.
- **Continue to use Ninja's `$in` and `$out` variables for scripts.** Rejected
  because backend escaping would turn them into literal shell text and diverge
  from command-recipe lowering.
- **Escape Ninja-special paths opportunistically.** Rejected because paths and
  shell text have different grammars; partial path escaping risks a Ninja graph
  that names different files than the command uses.

## Implementation references

- Backend conversion: [`src/ninja_gen_escape.rs`](../src/ninja_gen_escape.rs)
- Action and path emission: [`src/ninja_gen/mod.rs`](../src/ninja_gen/mod.rs)
  and [`src/ninja_gen_validation.rs`](../src/ninja_gen_validation.rs)
- Placeholder lowering:
  [`src/ir/cmd_interpolate.rs`](../src/ir/cmd_interpolate.rs) and
  [`src/ir/from_manifest_support.rs`](../src/ir/from_manifest_support.rs)
- Differential coverage:
  [`tests/ninja_dollar_escaping_tests.rs`](../tests/ninja_dollar_escaping_tests.rs)

## Revision history

2026-08-29 — The metadata decision was revised during implementation. The
initial scope discussion considered only command and script bindings, but the
Ninja writer also emits `description`, `depfile`, `deps`, and `pool` as binding
values. The accepted decision therefore escapes literal dollars and rejects
newline, carriage-return, and NUL in each of those fields at the Ninja emission
boundary, while keeping their IR representation backend-neutral.
