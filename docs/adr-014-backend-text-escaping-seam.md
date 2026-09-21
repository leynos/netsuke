# Architectural decision record (ADR) 014: Use a typed Ninja text-escaping seam

## Status

Accepted.

[ADR-034](adr-034-preserve-script-in-out-as-shell-variables.md) partially
supersedes this record's script `$in` and `$out` decision. Its
backend-escaping, path, metadata, and marker decisions remain in force.

## Date

2026-08-24

## Context and problem statement

Netsuke compiles manifest recipes into a backend-neutral intermediate
representation (IR), then writes a `build.ninja` file. Ninja treats `$` as its
own escape and variable syntax, whereas recipes use `$` for ordinary shell
variables such as `$PATH` and `${CARGO:-cargo}`. Emitting IR command text
verbatim caused Ninja either to erase a shell variable or reject the file.

Netsuke markers `{{ ins }}` and `{{ outs }}` have a different meaning from
shell variables. Markers are lowered through internal tokens while generating
the IR; the backend must never reinterpret them. `$in` and `$out`, like
`$PATH`, are shell variables. Applying Ninja escaping earlier would prevent
marker lowering, while applying it twice changes the shell text.

Paths are not shell text. Ninja's path grammar also gives special meaning to
dollars, spaces, colons, pipes, and control characters. Escaping only recipe
text while continuing to write such paths raw would produce a corrupt
dependency graph.

## Decision

Netsuke uses a private typed conversion at the Ninja writer boundary:

- IR commands and scripts remain plain `ShellText` after Netsuke has resolved
  its placeholders.
- The Ninja writer consumes that text once and produces an opaque
  `NinjaValue`. The conversion doubles each remaining dollar sign and rejects
  newline, carriage-return, and NUL control characters.
- Only the completed `NinjaValue` is written as a Ninja `command` binding;
  descriptions, `depfile`, `deps`, and `pool` are escaped at their Ninja
  emission boundary, doubling a literal dollar in each, and reject newline,
  carriage-return, and NUL characters. A `depfile` of `$out.d` therefore emits
  `$$out.d` and names a literal file rather than a Ninja variable.
- Build-edge paths remain separate values. A literal space is escaped as a `$`
  followed by a space, Ninja's own path escape, so whitespace-containing
  outputs stay valid. A pipe, newline, carriage return, or NUL is rejected
  because Ninja's path grammar cannot represent it. A dollar or colon is also
  rejected, even though Ninja can escape both; Netsuke has not adopted that
  part of the path grammar, and supporting it is separate work.
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
- The former script-only `$in` and `$out` lowering decision is superseded by
  ADR-034. Scripts now retain those shell variables, whose dollars are doubled
  at the Ninja boundary.
- A path using a literal space remains valid because Ninja has a dedicated
  escape for it. Other Ninja-special characters are rejected rather than
  supported by partial escaping; expanding the accepted path grammar is
  separate work.
- A manifest description that contained a Ninja variable reference no longer
  expands. `description: CC $out` now emits `CC $$out`, so Ninja prints the
  literal text `CC $out` instead of the output path. The metadata decision
  provides no replacement for that expansion, so dynamic metadata is not
  currently supported; lowering `$in` and `$out` into descriptions remains
  follow-up work, as does populating `Action.depfile` for the `$out.d` idiom.
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
  by ADR-034. The backend escapes their dollars, so they are shell variables,
  not Ninja input or output markers.
- **Escape Ninja-special paths opportunistically.** Rejected because paths and
  shell text have different grammars; partial path escaping risks a Ninja graph
  that names different files than the command uses.

## Implementation references

- Backend conversion: [`src/ninja_gen_escape.rs`](../src/ninja_gen_escape.rs)
- Action and path emission: [`src/ninja_gen/mod.rs`](../src/ninja_gen/mod.rs)
  and [`src/ninja_gen_validation.rs`](../src/ninja_gen_validation.rs)
- Placeholder lowering:
  [`src/ir/cmd_interpolate/mod.rs`](../src/ir/cmd_interpolate/mod.rs) and
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

2026-09-16 — The path rule was corrected to match the shipped behaviour. A
literal space is escaped as Ninja's dollar-then-space path escape rather than
rejected, so whitespace-containing outputs remain buildable, and a pipe is
rejected alongside the dollar, colon, and control characters. The earlier
wording described a rejection guard that the Windows recipe-shell work
superseded. The same entry records why each rejected character is rejected:
Ninja's path grammar cannot represent a pipe, newline, carriage return, or NUL
at all, whereas it can escape a dollar and a colon, which Netsuke rejects as a
deliberate limit of the path grammar it accepts.

2026-09-20 — ADR-034 reverses this record's script-only `$in` and `$out`
lowering decision. That decision made shell-variable spelling depend on recipe
form and conflated Netsuke markers, internal tokens, and shell variables. The
alternative rejected here, preserving Ninja's `$in` and `$out` in scripts, is
also reversed: after the `ShellText` to `NinjaValue` conversion doubles their
dollars, those names correctly reach the shell as variables. The `ShellText` /
`NinjaValue` seam and the `{{ ins }}` / `{{ outs }}` marker contract remain
unchanged.
