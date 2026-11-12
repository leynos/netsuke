# Architectural decision record (ADR) 0072: Display policy contract and terminal verification

## Status

Accepted on 2026-11-05. Netsuke resolves terminal policies and captured facts
once, then applies one display plan across its output adapters.

## Date

2026-11-05

## Context and Problem Statement

Netsuke's output policy spans colour, status glyphs, progress, accessibility,
and JSON mode. Before this decision, separate resolvers and output adapters
could disagree about terminal status or environment values. In particular,
`--color never` did not consistently keep diagnostics and tracing plain,
`--color always` did not style all Netsuke-controlled output on pipes, and an
empty `NO_COLOR` value was treated as active despite the variable's documented
non-empty convention.

Roadmap item 3.12.3 requires terminal-rendering regression tests. The policies
have a finite input domain, while the externally visible output also depends on
stream attachment, environment signals, and whether an adapter uses Clap,
Miette, tracing, or the status reporter.

## Decision Drivers

- Resolve every policy from one set of captured facts.
- Keep policy resolution pure and exhaustive over its finite input domain.
- Make each output adapter follow the same resolved decisions.
- Verify both stable policy projections and bytes emitted by the real binary.
- Keep redirected and subprocess output predictable.
- Preserve readable text labels when glyphs or colour are unavailable.

## Decision Outcome

Add `src/output/plan/mod.rs` as the pure display-policy core. The application
entry point captures `NO_COLOR`, `TERM`, stdout terminal status, and stderr
terminal status once. It resolves `DisplayPolicies` and `TerminalFacts` into one
`DisplayPlan`, which is passed to the status reporter, tracing formatter,
Miette diagnostic hook, and Clap help renderer. `EnvSignals::read` reduces
environment strings to booleans; downstream adapters do not inspect the
environment or call `is_terminal()`.

The user-visible contract is:

| Policy                       | Resolution                                                      | Observable behaviour                                                                                                |
| ---------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `--color auto`               | Uses the target stream's terminal fact.                         | Styles only a terminal when `TERM` is not `dumb` and `NO_COLOR` is absent or empty.                                 |
| `--color always`             | Forces colour for human-readable Netsuke-controlled output.     | Styles tracing and diagnostics on stderr and help/version on stdout, including pipes; JSON output remains unstyled. |
| `--color never`              | Disables ANSI styling.                                          | Also selects accessible mode when accessibility is `auto`.                                                          |
| `--emoji auto`               | Follows the resolved output mode.                               | Uses Unicode in standard mode and ASCII in accessible mode.                                                         |
| `--emoji always` / `never`   | Selects the corresponding glyph set.                            | Unicode or ASCII status symbols are used, while every status retains a text label.                                  |
| `--progress auto` / `always` | These policies are equivalent.                                  | Both request stage reporting; neither forces animation into a pipe, dumb terminal, or accessible mode.              |
| `--progress never`           | Selects the silent reporter.                                    | No stage or task progress is emitted.                                                                               |
| `--accessibility auto`       | Uses colour, environment, and terminal mode.                    | Selects accessible mode for `--color never`, non-empty `NO_COLOR`, or `TERM=dumb`.                                  |
| `--accessibility on` / `off` | Overrides automatic mode selection.                             | `on` uses static labelled output; `off` keeps standard mode, while colour `auto` still respects `NO_COLOR`.         |
| `--json`                     | Selects machine-readable output.                                | Progress and tracing are suppressed; diagnostics use the JSON document path.                                        |
| Help and version             | Uses the last CLI `--color`, then `NETSUKE_COLOR`, then `auto`. | Configuration files are not read; ordinary usage errors remain plain.                                               |

Colour applies only to Netsuke-controlled tracing, diagnostics, and Clap
help/version output. Generated artefacts and subprocess streams are forwarded
unchanged. `TERM=dumb` prevents live redraws even when standard output is
forced with `--accessibility off`.

A configuration-load failure is emitted before any configuration value has been
read, so it cannot follow a configured colour policy. It is therefore split:
the human-readable error message uses the plain fallback style under every
policy, while the structured `tracing` events recorded alongside it follow the
startup colour hint, as `D11` requires of every tracing event Netsuke controls.
The two halves are asserted separately, because collapsing them either drops
colour from the diagnostic stream or breaks the plain-message guarantee.

The verification strategy follows one oracle per behaviour:

- `resolve_display_plan` is compared against an independent truth model over
  all 5,184 policy/fact combinations. Witness tables and property tests cover
  the string-to-environment-signal reduction.
- Four `insta` YAML snapshots record deterministic projections for colour,
  emoji, progress, and accessibility policies on representative facts.
- Pipe and Unix PTY tests run the actual `netsuke` binary and compare its
  emitted bytes with a plan resolved from the same arguments and facts.
- Behavioural scenarios pin externally visible progress and glyph behaviour.

The principal decisions behind the implementation are:

- D2: use a functional core and imperative shell, with one plan handed to four
  outbound adapters rather than a terminal-probe trait.
- D3: keep `auto` and `always` progress equivalent; `always` requests progress
  but does not override output-mode constraints.
- D4: treat `NO_COLOR` as active only when its value is non-empty.
- D6 and D7: exhaust the finite resolver domain and keep plan snapshots as the
  contract oracle while end-to-end tests derive expectations from the plan.
- D8: map diagnostic colour, Unicode, and link capabilities from
  `DiagnosticStyle`; do not force Miette's graphical handler.
- D9: pass Clap an explicit `ColorChoice::Always` or `Never` based on the
  startup hint so its own environment heuristics cannot disagree.
- D11: `--color always` forces styling on pipes for every emitter Netsuke
  controls.
- D12: capture terminal and environment facts at the process boundary, with
  lint enforcing the single terminal query.

## Consequences

The display-plan core is an internal interface used only by the application
composition root, runner, output adapters, and tests. The established
`OutputPrefs` and `ThemeContext` surfaces remain the glyph and theme adapters;
they do not make separate terminal decisions. The single-plan boundary makes
policy changes reviewable in snapshots and allows behaviour tests to test the
binary without copying a second policy implementation.

An empty `NO_COLOR` is now equivalent to an unset variable. `--progress always`
does not mean animated progress in every environment. Help styling is decided
before configuration discovery, so help follows command-line and environment
hints only. The Unix PTY harness reuses the `nix` crate already in the
dependency graph rather than adding a crate: `test_support` takes `nix` as a
Unix-target dependency, and the `term`, `fs`, and `signal` features are enabled
on both it and the root dev-dependency.

## Known Limitations

- Netsuke does not read `FORCE_COLOR` or `CLICOLOR`; `NETSUKE_COLOR` is the
  supported environment policy for startup help and runtime configuration.
- Native Windows ConPTY tests are not included. Windows uses pipe-level
  behaviour tests and the platform-neutral display-plan snapshots.
- Colour is not added to subprocess output; child bytes remain the child's
  responsibility.
- Accessible diagnostics use Miette's configured Unicode and link settings,
  but do not force Miette's narratable handler.

## Addendum (2026-11-12): display-policy clarifications

Two clarifications were made while implementing roadmap task 3.12.3. Both
refine what the accepted decision already said; neither changes its resolution
rules, and the accepted text above is left as it was written.

**The accessible-mode triggers are not equally overridable.** `NO_COLOR` is a
colour hint, so `--color always` suppresses it: `no_color_active` returns
`false` for `ColourPolicy::Always`, and automatic accessibility selection then
sees no `NO_COLOR` trigger. `TERM=dumb` is not a colour hint — it is a
statement that the terminal cannot redraw — so no colour policy suppresses it,
and `--accessibility off` is the only override. `--color never` is itself the
third trigger, since it acts like an internal `NO_COLOR`.

For the same reason the `--accessibility auto` row above is not narrowed by
colour policy: `--color always` removes the `NO_COLOR` trigger it would
otherwise act on, but the row's remaining triggers still apply, so the row's
wording stands unchanged.

## References

- [Roadmap task 3.12.3](../roadmap.md#312-terminal-rendering-verification)
- [Approved ExecPlan](../execplans/3-12-3-terminal-rendering-regression-tests.md)
- [Netsuke design](../netsuke-design.md)
- [Users' guide: control output and accessibility](../users-guide.md#control-output-and-accessibility)
