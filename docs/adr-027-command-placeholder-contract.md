# Architecture decision record (ADR) 027: Command placeholder contract

## Status

Accepted. The command placeholder set, the backtick handling boundary, and the
scope of the `shlex` guard are fixed as stated below. Roadmap item 4.4.1 and
the same change set carry them to users in a new `README.md` section, *Security
and command interpolation*, in every translated README.

## Date

2026-09-19

## Context and problem statement

Netsuke compiles a YAML manifest into a Ninja build file. Part of a recipe is
Netsuke's to rewrite; the rest is opaque shell text passed to `/bin/sh`,
`bash.exe`, or Windows PowerShell. That split is a security boundary: it is the
last point at which Netsuke can reject a command whose shell syntax the
substitution damaged, and the only place Netsuke promises to quote a path.

`docs/formal-verification-methods-in-netsuke.md` records the boundary as a
contract to settle before further proof work, and names a README section as its
eventual home. It poses three questions:

- whether the internal marker tokens are the only supported placeholders;
- whether POSIX backtick rejection and PowerShell escape handling are the full
  contract or a temporary subset of shell command-substitution handling; and
- whether `shlex::split` is part of the semantic acceptance contract or only a
  guard against obviously malformed commands.

Each question has at least two defensible answers, and the existing prose across
`docs/developers-guide.md`, `docs/users-guide.md`, and the formal-verification
document does not agree with the implementation on the first. Reading the code
settles what the behaviour *is*; it does not settle which parts of that
behaviour are promises. This record settles both.

## Decision drivers

- **The documented contract must be the implemented one.** A boundary a reader
  cannot rely on is worse than an irregular boundary stated plainly, because
  the reader will act on the reassuring reading.
- **Promises must be separated from implementation detail.** The backtick
  handling contains both a genuine invariant and a conservative approximation
  of it. Freezing the approximation at the strength of the invariant makes a
  future correctness fix a breaking change.
- **The accepted command set is not wholly Netsuke's to define.** It is fixed
  by the `shlex` version resolved at build time, and `Cargo.toml` carries a
  caret requirement rather than a pin.
- **Proof claims must not outrun the proofs.** Only the marker recognizer is
  covered by Kani; the backtick and `shlex` behaviour is covered by Proptest.

## Decision

### The supported placeholder set differs by recipe kind

`{{ ins }}` and `{{ outs }}` are the supported placeholders in both `command:`
and `script:` recipes. In `script:` recipes Netsuke *additionally* rewrites bare
`$in` and `$out` at identifier boundaries — a boundary being a position not
adjacent to an alphanumeric character or an underscore. Every other
dollar-prefixed form, including `$ins`, `$outs`, `$input`, `$output`, and
`$PATH`, is literal text for the shell in both recipe kinds.

The recognizer implementing the two halves of this rule is `find_substitution`
for `command:` recipes and `find_script_substitution` for `script:` recipes
(`src/ir/cmd_interpolate/mod.rs:261-297`). Only the latter consults
`try_match_dollar_placeholder`.

The `script:`-only forms are **retained legacy behaviour, not a blessed
feature**. New manifests should use `{{ ins }}` and `{{ outs }}`, and the
README section this record establishes steers authors accordingly. Two
consequences are stated wherever the contract is documented, because neither is
predictable from the rule:

- A script that writes its own shell variable named `in` or `out`, such as
  `in=foo; echo $in`, has that variable rewritten to input paths with no
  diagnostic.
- Moving the same text from a `script:` to a `command:` silently changes its
  meaning: `$in` there degrades to an ordinary, usually empty, shell variable.

Netsuke does not expose Ninja's own `$in` and `$out` rule variables. Resolved
paths are baked into each content-hashed rule, so Ninja never substitutes a
path into a recipe.

### Backtick handling is two mechanisms at two different strengths

The mechanisms are not the same kind of thing and are not promised equally.

**An invariant, promised.** A Netsuke placeholder is never substituted inside a
backtick region or a `$( … )` command substitution; such a recipe is rejected.
Substituting into a region the shell will re-evaluate cannot be made safe, so
the rejection is policy. It is implemented in the command traversal
(`SubstitutionTraversal::append_protected_character`,
`src/ir/cmd_interpolate/substitution.rs:364-375`) and in the script traversal
(`append_substitution_or_character`,
`src/ir/cmd_interpolate/script_substitution.rs:222-243`).

**A conservative check, not promised.** A `command:` recipe whose substituted
text contains an odd total number of backtick characters is *currently* rejected
(`has_unmatched_backticks`, `src/ir/cmd_interpolate/mod.rs:172-174`, reached
from `is_valid_command_for_shell` at `:226-231`). The check is a whole-string
parity count, not a quoting-aware model: a backtick inside single quotes counts
toward the total, and two balanced but unrelated backticks do not. It may
therefore reject valid shell text. A future release may accept more, and such
widening is **not** treated as a breaking change.

**Not a guarantee at all.** Netsuke does not inspect backticks the author
wrote. A balanced pair is passed through to the shell and executed as command
substitution. The single exception is that `quote_double_quoted_path`
(`src/ir/cmd_interpolate/mod.rs:154-163`) backslash-escapes a backtick that
falls inside a *Netsuke-substituted path* landing in a double-quoted context.

**Route and recipe-kind scope.** PowerShell is outside both mechanisms, because
a backtick is its escape character rather than a command-substitution delimiter;
`is_valid_command_for_shell` returns `true` unconditionally for
`RecipeShell::PowerShell` (`src/ir/cmd_interpolate/mod.rs:227-228`). The parity
check is also `command:`-only: a `script:` recipe gets the marker invariant but
no parity check, because scripts may legitimately contain heredocs and other
text that `shlex` cannot model.

### The `shlex` guard is part of the acceptance contract, not a stability commitment

`is_valid_command_for_shell` calls `shlex::split(command).is_some()` on the
fully substituted text (`src/ir/cmd_interpolate/mod.rs:230`). Failure produces
`IrGenError::InvalidCommand`, surfaced through the Fluent message
`ir.invalid_command` (`locales/en-GB/messages.ftl:188`), rendered as
`Invalid command interpolation: { $snippet }.`.

It **is** part of the observable acceptance contract: a `command:` recipe on
the POSIX or Bash route whose substituted text cannot be split is rejected with
a stable, localized diagnostic at IR-lowering time. It is **not** a stability
commitment about the precise accepted set, because that set is `shlex` 2.0.1's
approximation of POSIX and `Cargo.toml:139` is a caret requirement rather than
a pin. `Cargo.lock` records one resolution; `cargo install --path .` ignores
the lockfile, so two people building the same source can get different
acceptance sets.

The two drift directions are named separately because they are not symmetric:

- A `shlex` release that accepts text rejected today means previously rejected
  manifests begin to build. Not treated as a breaking change.
- A `shlex` release that rejects text accepted today means a working manifest
  stops building. That is a defect, not a policy change, and is recorded in the
  changelog.

The guard does not apply to `script:` recipes, and Netsuke never executes the
token vector `shlex::split` returns; it only asks whether a split was possible.
A second, `debug_assert!`-only use exists in `assert_shell_command`
(`src/ninja_gen/mod.rs:283-290`), reached only for `Recipe::Command` because
`script_shell_text` (`src/ninja_gen/mod.rs:343-356`) is explicitly exempt.

## Options considered

### The placeholder set

1. **Document the set the implementation actually has** — `{{ ins }}` and
   `{{ outs }}` everywhere, plus `$in` and `$out` in scripts. Selected.
2. **Document a uniform set** — declare the two marker forms universal and
   describe `$in` and `$out` as literal shell variables in both recipe kinds.
   Rejected: it is false for `script:` recipes, which three documents currently
   assert. Documenting a contract Netsuke does not honour is worse than
   documenting an irregular one.
3. **Widen the implementation to match** — make `command:` recipes rewrite
   `$in` and `$out` too, then document the uniform set. Rejected: it is a
   production behaviour change on a security-sensitive path, and it would
   silently change the meaning of existing commands that use those names as
   shell variables.

### The backtick boundary

1. **Scope the promise to markers, and state the rest as status** — a promised
   invariant, a conservative check explicitly not promised, and an explicit
   statement that author-written command substitution is executed. Selected.
2. **Promise both mechanisms as guarantees.** Rejected: it freezes a
   whole-string parity approximation alongside a genuine invariant, so a future
   quoting-aware fix would be a breaking change.
3. **Call the behaviour a temporary subset of a command-substitution model.**
   Rejected: nothing in the roadmap commits to widening it, and the phrasing
   implies a defence against author-written command substitution that does not
   and is not intended to exist.

### The `shlex` guard

1. **Split the answer along the rejection/acceptance axis** — the rejection is
   contractual, the accepted set is not. Selected.
2. **Declare it fully part of the acceptance contract.** Rejected: it would
   commit the project to `shlex` 2.0.1's approximation of POSIX across future
   crate versions, which nobody has agreed to, and the caret requirement makes
   that commitment unstable in a way a version pin would not.
3. **Declare it only a malformed-command guard.** Rejected: users observe and
   rely on the rejection, which has a stable localized diagnostic. Denying that
   it is part of the contract would be false.

## Rationale

- **The contract is stated where it can be falsified.** The existing tests and
  property tests under `src/ir/cmd_interpolate/` already pin this behaviour,
  and the README section this record establishes carries a marked, executable
  example for each of the three decisions. A later change that breaks the
  documented behaviour therefore fails a test rather than silently invalidating
  prose.
- **Legacy behaviour is documented as legacy.** The `script:`-only `$in` and
  `$out` forms are load-bearing for existing manifests, so removing them is not
  this decision's to make; describing them as retained legacy with a warning
  and a migration steer keeps the documentation honest under either future
  answer.
- **The approximation is labelled as one.** Splitting the backtick behaviour
  into a promised invariant and an unpromised check leaves room to make the
  check quoting-aware later without a breaking change.
- **Stability follows the observable surface.** The rejection has a stable,
  localized diagnostic and is therefore contractual; the accepted set varies
  with a resolved dependency and is therefore not.

## Consequences

Manifest authors who use `script:` recipes and write their own `in` or `out`
shell variables will have those variables rewritten with no diagnostic. This
becomes documented rather than silent, and the README will recommend
`{{ ins }}` and `{{ outs }}`; a diagnostic for the shadowing case is follow-up
work if the legacy forms are retained indefinitely.

Authors can rely on the marker invariant and on the rejection diagnostic.
Authors cannot rely on the accepted set across `shlex` versions, and cannot
rely on Netsuke to sanitize shell text they or their templates wrote. Passing
the `shlex` guard says nothing about whether a shell would run a command
substitution in the same text: `shlex` performs no expansion and treats a
backtick as an ordinary character, so downstream tooling must not reuse
`shlex::split(x).is_some()` as an injection check.

Widening the parity check to accept text it currently rejects is not a breaking
change; narrowing the accepted set is a defect. Renaming the diagnostic message
`ir.invalid_command` or altering its meaning is a breaking change to a
published interface.

## Implementation references

- Placeholder recognition:
  [`src/ir/cmd_interpolate/mod.rs`](../src/ir/cmd_interpolate/mod.rs)
  (`find_substitution`, `find_script_substitution`,
  `try_match_dollar_placeholder`).
- Marker invariant and path quoting: the `substitution` and
  `script_substitution` modules under
  [`src/ir/cmd_interpolate/`](../src/ir/cmd_interpolate/).
- Marker recognizer proofs:
  [`src/ir/cmd_interpolate/verification.rs`](../src/ir/cmd_interpolate/verification.rs).
- Backtick and guard properties:
  [`src/ir/cmd_interpolate_property_tests.rs`](../src/ir/cmd_interpolate_property_tests.rs).
- Diagnostic text: [`locales/en-GB/messages.ftl`](../locales/en-GB/messages.ftl)
  (`ir.invalid_command`).
- User-facing statement of this contract: the `README.md` section *Security and
  command interpolation* added under roadmap item 4.4.1; detailed shell-route
  mechanics remain in
  [`users-guide.md`](users-guide.md#review-the-safety-boundary).
- Upstream requirement: `docs/formal-verification-methods-in-netsuke.md`,
  sections *Kani for command interpolation* and *Command placeholder contract*.

## Known risks and limitations

- **Proof coverage is narrower than the contract.** Per
  `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md`, Kani proves the marker
  recognizer only. The backtick state machine and the `shlex` guard are covered
  by Proptest properties, not by proof, and are described here as
  property-tested rather than proved.
- **The domain boundary in this module is not clean.**
  `invalid_command_error` (`src/ir/cmd_interpolate/mod.rs:215-222`) calls the
  localization layer and stores a pre-rendered, locale-resolved human string
  inside the domain error. That is presentation resolved within domain policy
  against ambient state. It does not affect the contract above, but this module
  is not an exemplar of a pure domain boundary and should not be cited as one.
- **The `shlex` guard is not an injection defence.** It is an approximation of
  word splitting, and a command that splits cleanly is not thereby safe.
