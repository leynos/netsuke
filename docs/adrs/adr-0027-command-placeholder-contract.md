# Architectural decision record (ADR) 0027: Command placeholder contract

## Status

Accepted. The command placeholder set, the backtick handling boundary, and the
scope of the `shlex` guard are fixed as stated below. Roadmap item 4.4.1
carries them to users in the
[README section](../../README.md#security-and-command-interpolation) *Security
and command interpolation* and in every translated README. This record explains
the decisions behind that user-facing contract.

Revised on 2026-09-23. The first draft of this record described the placeholder
set as differing by recipe kind, because `script:` recipes then lowered bare
`$in` and `$out` to paths.
[ADR-0034](adr-0034-preserve-script-in-and-out-as-shell-variables.md)
subsequently removed that lowering, so the set is now uniform. ADR-0034 owns
that decision; this record states the resulting contract and is revised to
agree with it. The backtick and `shlex` decisions below are unaffected and were
re-verified against the implementation on the revision date.

Amended on 2026-10-12 for issue #846. A security finding showed that
PowerShell's standalone single-quoted path literal is unsafe when inserted
inside an existing string. Both PowerShell recipe kinds now use the same
PowerShell-aware traversal and accept markers only at unquoted insertion sites.

Amended on 2026-10-17 after review of the #846 fix. PowerShell treats
typographic quote characters as string delimiters. Because the traversal does
not model their pairing, it fails closed for later markers after encountering
one. Path literals double every PowerShell single-quote delimiter, including
typographic forms. A here-string terminator is accepted only at the start of a
line when followed by a newline or the end of the template. Plain line-comment
text does not poison later marker sites, but an unquoted block-comment opener
or scanner-sensitive syntax inside a line comment makes later markers fail
closed because the traversal does not parse comment text. In particular, an
apostrophe in a comment can cause rejection.

Amended on 2026-10-17 to bound PowerShell `--%` marker rejection to its raw
native-command region. PowerShell passes the rest of that line to a native
command as literal text, so Netsuke's standalone path literal is not a safe
encoder there. The scanner resumes after the raw region ends at its newline or
pipeline terminator; a semicolon does not end it.

Amended on 2026-10-18 to document the quoted PowerShell script placeholder
security fix for issue #846.

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

### The placeholder set is uniform across recipe kinds

`{{ ins }}` and `{{ outs }}` are the only Netsuke markers, and both recipe
kinds recognize that same set in active recipe text. On POSIX and Bash routes,
markers in comments and heredoc bodies are copied as internal tokens instead of
being expanded; markers in heredoc delimiters are expanded. PowerShell
`command:` and `script:` recipes use the same PowerShell-aware traversal and
reject markers in single-quoted strings, double-quoted strings, here-strings,
`$( … )`, immediately after a backtick, and within the raw native-command
region after an unquoted `--%` token. PowerShell passes the rest of that line
after `--%` to a native command as literal text, so Netsuke's standalone quoted
path encoding is not protective there. The scanner rejects markers in that
region and resumes after its newline or pipeline terminator; a semicolon does
not end it. See Microsoft's documentation of the
[stop-parsing token](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-7.6#the-stop-parsing-token).
Keep markers out of POSIX and Bash comments and heredoc bodies because their
internal tokens can remain in the generated recipe. PowerShell scripts have no
inert comment regions: markers in plain line-comment text remain eligible for
interpolation, while a block-comment opener or scanner-sensitive quote,
backtick, or substitution syntax in a line-comment makes later markers fail
closed. In particular, an apostrophe in a comment can make a later marker fail
closed. Every dollar-prefixed form — `$in`, `$out`, `$ins`, `$outs`, `$input`,
`$output`, `$PATH` — is a shell variable that Netsuke leaves for the selected
shell to interpret, in both recipe kinds. The Ninja backend doubles the dollar
so the shell receives the text unchanged.

The traversal also fails closed for later markers after encountering a
typographic PowerShell quote delimiter because it does not model delimiter
pairing.

Three terms are kept distinct throughout, following
[ADR-0034](adr-0034-preserve-script-in-and-out-as-shell-variables.md):

- a **marker** is the Netsuke-owned `{{ ins }}` or `{{ outs }}` placeholder;
- an **internal token** is the `INS_TOKEN` or `OUTS_TOKEN` sentinel created by
  manifest rendering and normally consumed during interpolation; POSIX and Bash
  comments and heredoc bodies can carry it into the generated recipe;
- a **shell variable** is handwritten text such as `$PATH` or `$in` that
  Netsuke does not rewrite.

The recognizer is `find_substitution`, which matches the two internal tokens
and nothing else. `find_script_substitution` delegates to it, so both recipe
kinds recognize the same set. POSIX-aware scanning leaves tokens in POSIX and
Bash comments and heredoc bodies unchanged, while heredoc delimiters remain
eligible for substitution. PowerShell scripts use `SubstitutionTraversal`, like
PowerShell commands, so the PowerShell quote and escape rules apply to both.

Netsuke does not expose Ninja's own `$in` and `$out` rule variables. Resolved
paths are baked into each content-hashed rule, so Ninja never substitutes a
path into a recipe.

### Backtick handling is two mechanisms at two different strengths

The mechanisms are not the same kind of thing and are not promised equally.

**An invariant, promised.** A Netsuke placeholder is never substituted inside a
POSIX or Bash backtick region or any shell's `$( … )` command substitution;
such a recipe is rejected. PowerShell also rejects a marker immediately after a
backtick because it escapes the following character. Substituting into a region
the shell will re-evaluate cannot be made safe, so the rejection is policy. It
is implemented by `SubstitutionTraversal::append_protected_character`
(`src/ir/cmd_interpolate/substitution/mod.rs`) and the POSIX/Bash script
traversal (`append_substitution_or_character` in
`src/ir/cmd_interpolate/script_substitution.rs`).

**A conservative check, not promised.** A `command:` recipe whose substituted
text contains an odd total number of backtick characters is *currently* rejected
(`has_unmatched_backticks` in `src/ir/cmd_interpolate/mod.rs`, reached from
`is_valid_command_for_shell`). The check is a whole-string parity count, not a
quoting-aware model: a backtick inside single quotes counts toward the total,
and two balanced but unrelated backticks do not. It may therefore reject valid
shell text. A future release may accept more, and such widening is **not**
treated as a breaking change.

**Not a guarantee at all.** Netsuke leaves author-written backticks untouched.
On POSIX routes, active backticks can trigger command substitution; backticks
inside single quotes remain literal. Path substitution separately protects
Netsuke-owned values: `quote_double_quoted_path`
(`src/ir/cmd_interpolate/mod.rs`) backslash-escapes a backtick that falls
inside a *Netsuke-substituted path* landing in a double-quoted context.

**Route and recipe-kind scope.** PowerShell sits outside the POSIX backtick
command-substitution and parity checks because a PowerShell backtick escapes
the following character. A marker immediately following a backtick is still
rejected, as are markers in single-quoted or double-quoted strings,
here-strings, and `$( … )`. The PowerShell-aware traversal applies these rules
to both `command:` and `script:` recipes; the standalone path literal is not a
context-safe encoder inside an existing PowerShell string. PowerShell scripts
do not have inert comment regions in this scanner: plain line-comment text
leaves later markers eligible for interpolation, while a block-comment opener
or scanner-sensitive syntax such as an apostrophe can make later markers fail
closed. The `shlex` guard remains `command:`-only: a `script:` recipe bypasses
it because scripts may contain heredocs and other text that `shlex` cannot
model.

### The `shlex` guard is part of the acceptance contract, not a stability commitment

`is_valid_command_for_shell` calls `shlex::split(command).is_some()` on the
fully substituted text (`src/ir/cmd_interpolate/mod.rs`). Failure produces
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
(`src/ninja_gen/mod.rs`), reached only for `Recipe::Command` because
`script_shell_text` is explicitly exempt.

## Options considered

### The placeholder set

These options were weighed while `script:` recipes still lowered `$in` and
`$out`. They are retained because the outcome was decided elsewhere, and the
reasoning explains why.

1. **Document the recipe-kind asymmetry as retained legacy** — `{{ ins }}` and
   `{{ outs }}` everywhere, plus `$in` and `$out` in scripts, with a shadowing
   warning and a migration steer. Selected at first draft, on the ground that
   documenting a contract Netsuke does not honour is worse than documenting an
   irregular one.
2. **Document a uniform set without changing the code** — Rejected: it was
   false for `script:` recipes at the time.
3. **Widen the implementation to match** — make `command:` recipes rewrite
   `$in` and `$out` too. Rejected: a production behaviour change on a
   security-sensitive path that would silently change the meaning of existing
   commands using those names as shell variables.
4. **Narrow the implementation instead** — stop lowering `$in` and `$out` in
   `script:` recipes, making the uniform set true. Not considered here;
   [ADR-0034](adr-0034-preserve-script-in-and-out-as-shell-variables.md)
   selected it on 2026-09-20 and it is now the implemented behaviour. It
   carries the same migration risk as option 3 in the opposite direction, which
   ADR-0034 accepted and documented in the migration guide.

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
- **One decision, one owner.** The placeholder set is ADR-0034's decision, not
  this record's; this record states the contract that follows from it. Keeping
  the superseded reasoning visible under *Options considered* shows why the
  asymmetry was tolerable to document before it was removed.
- **The approximation is labelled as one.** Splitting the backtick behaviour
  into a promised invariant and an unpromised check leaves room to make the
  check quoting-aware later without a breaking change.
- **Stability follows the observable surface.** The rejection has a stable,
  localized diagnostic and is therefore contractual; the accepted set varies
  with a resolved dependency and is therefore not.

## Consequences

Manifest authors who relied on bare `$in` or `$out` resolving inside a
`script:` recipe must migrate to `{{ ins }}` and `{{ outs }}`. ADR-0034 made
that change and the migration guide records it; the README section this record
establishes states the resulting uniform rule rather than repeating the
migration. The shadowing hazard the first draft warned about — a script
assigning its own `in` or `out` variable — no longer exists, because those
names are never rewritten.

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
  [`src/ir/cmd_interpolate/mod.rs`](../../src/ir/cmd_interpolate/mod.rs)
  (`find_substitution`, and `find_script_substitution`, which delegates to it).
- Marker invariant and path quoting: the `substitution` and
  `script_substitution` modules under
  [`src/ir/cmd_interpolate/`](../../src/ir/cmd_interpolate/).
- Marker recognizer proofs:
  [`src/ir/cmd_interpolate/verification.rs`](../../src/ir/cmd_interpolate/verification.rs).
- Backtick and guard properties:
  [property test module](../../src/ir/cmd_interpolate/property_tests/mod.rs).
- Diagnostic text:
  [`locales/en-GB/messages.ftl`](../../locales/en-GB/messages.ftl)
  (`ir.invalid_command`).
- User-facing statement of this contract: the `README.md` section *Security and
  command interpolation* added under roadmap item 4.4.1; detailed shell-route
  mechanics remain in
  [`users-guide.md`](../users-guide.md#review-the-safety-boundary).
- Upstream requirement: `docs/formal-verification-methods-in-netsuke.md`,
  sections *Kani for command interpolation* and *Command placeholder contract*.

## Known risks and limitations

- **Proof coverage is narrower than the contract.** Per
  `docs/adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md`,
  Kani proves the marker recognizer only. The backtick state machine and the
  `shlex` guard are covered by Proptest properties, not by proof, and are
  described here as property-tested rather than proved.
- **The domain boundary in this module is not clean.**
  `invalid_command_error` (`src/ir/cmd_interpolate/mod.rs`) calls the
  localization layer and stores a pre-rendered, locale-resolved human string
  inside the domain error. That is presentation resolved within domain policy
  against ambient state. It does not affect the contract above, but this module
  is not an exemplar of a pure domain boundary and should not be cited as one.
- **The `shlex` guard is not an injection defence.** It is an approximation of
  word splitting, and a command that splits cleanly is not thereby safe.
