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

Amended on 2026-10-18 after review of PowerShell comment and stop-parsing edges.

Amended on 2026-10-19 for issue #846. The PowerShell script insertion-site
contract and scanner review corrections are recorded in the addenda below.

Amended on 2026-11-06 for issue #971. POSIX comment detection now uses the
token boundaries of POSIX shell word recognition rather than Unicode
whitespace, and a `)` that closes a substitution immediately before `#` fails
closed when the substitution source contains the `case` reserved word. The
POSIX comment and heredoc boundary amendment below states the contract.

Amended on 2026-11-18 for issue #1000. A closing backtick now restores its
saved quote context into the `$()` or script region that opened it, and nesting
that would leave that region closed is rejected. The addendum below states the
ownership rule.

Amended on 2026-11-19 for issue #993. A shared PowerShell `command:` list is
validated as one script, because PowerShell parses each entry where its
predecessor stopped. An entry that leaves a quoted string, here-string, block
comment, or `$()` substitution open, or that ends in a backtick escape, is now
rejected. The addendum below distinguishes this cross-entry boundary from the
same-recipe findings tracked in
[#846](https://github.com/leynos/netsuke/issues/846) and states the scope.

Amended on 2026-11-19 for issue #1145. Every interpolation rejection now names
the syntax it refused instead of quoting the rejected command, because a
rendered recipe may already contain a value that `env()` resolved while the
manifest was rendered. The amendment below states the contract.

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
being expanded; markers in heredoc delimiters are expanded. `script:` recipes
use the POSIX-aware scanner on POSIX and Bash routes. PowerShell `command:` and
`script:` recipes use the PowerShell-aware traversal; its unquoted insertion
contract and conservative rejection boundaries are recorded in the 2026-10-19
addendum below. Keep markers out of POSIX and Bash comments and heredoc bodies
because their internal tokens can remain in the generated recipe. Every
dollar-prefixed form — `$in`, `$out`, `$ins`, `$outs`, `$input`, `$output`,
`$PATH` — is a shell variable that Netsuke leaves for the selected shell to
interpret, in both recipe kinds. The Ninja backend doubles the dollar so the
shell receives the text unchanged.

Three terms are kept distinct throughout, following
[ADR-0034](adr-0034-preserve-script-in-and-out-as-shell-variables.md):

- a **marker** is the Netsuke-owned `{{ ins }}` or `{{ outs }}` placeholder;
- an **internal token** is the `INS_TOKEN` or `OUTS_TOKEN` sentinel created by
  manifest rendering and normally consumed during interpolation; inert comments
  and heredoc bodies can carry it into the generated recipe;
- a **shell variable** is handwritten text such as `$PATH` or `$in` that
  Netsuke does not rewrite.

The recognizer is `find_substitution`, which matches the two internal tokens
and nothing else. `find_script_substitution` delegates to it, so both recipe
kinds recognize the same set. POSIX-aware scanning leaves tokens in comments
and heredoc bodies unchanged, while heredoc delimiters remain eligible for
substitution.

Netsuke does not expose Ninja's own `$in` and `$out` rule variables. Resolved
paths are baked into each content-hashed rule, so Ninja never substitutes a
path into a recipe.

### Backtick handling is two mechanisms at two different strengths

The mechanisms are not the same kind of thing and are not promised equally.

**An invariant, promised.** A Netsuke placeholder is never substituted inside a
backtick region or a `$( … )` command substitution; such a recipe is rejected.
Substituting into a region the shell will re-evaluate cannot be made safe, so
the rejection is policy. It is implemented by `SubstitutionTraversal` for
commands and PowerShell scripts, and by `ScriptSubstitutionTraversal` for POSIX
and Bash scripts.

**A conservative check, not promised.** A `command:` recipe whose substituted
text contains an odd total number of backtick characters is *currently* rejected
(`has_unmatched_backticks` in `src/ir/cmd_interpolate/mod.rs`, reached from
`is_valid_command_for_shell` in the same module). The check is a whole-string
parity count, not a quoting-aware model: a backtick inside single quotes counts
toward the total, and two balanced but unrelated backticks do not. It may
therefore reject valid shell text. A future release may accept more, and such
widening is **not** treated as a breaking change.

**Not a guarantee at all.** Netsuke leaves author-written backticks untouched.
On POSIX routes, active backticks can trigger command substitution; backticks
inside single quotes remain literal. Path substitution separately protects
Netsuke-owned values: `quote_double_quoted_path` in
`src/ir/cmd_interpolate/mod.rs` backslash-escapes a backtick that falls inside
a *Netsuke-substituted path* landing in a double-quoted context.

**Route and recipe-kind scope.** PowerShell has no backtick
command-substitution region, but it rejects a marker immediately after a
backtick because that escape character would escape the path literal's opening
quote. PowerShell is outside the parity check entirely:
`is_valid_command_for_shell` returns `true` unconditionally for
`RecipeShell::PowerShell`. It is **inside** the `$( … )` half. The PowerShell
traversal treats a marker inside a command substitution, or inside any quoted
region, as protected and rejects it (`power_shell_marker_protection` in
`src/ir/cmd_interpolate/substitution/powershell/lexical/mod.rs`) — the same
outcome as POSIX, by a different rule, pinned by
`power_shell_commands_and_scripts_reject_shared_literal_boundaries` and
`power_shell_commands_and_scripts_reject_shared_ambiguous_syntax` in
`src/ir/cmd_interpolate/power_shell_tests.rs`. The parity check is also
`command:`-only: a `script:` recipe gets the marker invariant but no parity
check, because scripts may legitimately contain heredocs and other text that
`shlex` cannot model.

### The `shlex` guard is part of the acceptance contract, not a stability commitment

`is_valid_command_for_shell` calls `shlex::split(command).is_some()` on the
fully substituted text. Failure produces `IrGenError::InvalidCommand`, surfaced
through the Fluent message `ir.invalid_command` (in
`locales/en-GB/messages.ftl`), rendered as
`Invalid command interpolation: { $snippet }.` (The predicate and the message
placeholder were renamed by the 2026-11-19 amendment below; the contract this
paragraph states is unchanged.)

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
  `invalid_command_error` in `src/ir/cmd_interpolate/mod.rs` calls the
  localization layer and stores a pre-rendered, locale-resolved human string
  inside the domain error. That is presentation resolved within domain policy
  against ambient state. It does not affect the contract above, but this module
  is not an exemplar of a pure domain boundary and should not be cited as one.
- **The `shlex` guard is not an injection defence.** It is an approximation of
  word splitting, and a command that splits cleanly is not thereby safe.

## Addendum — 2026-10-19: PowerShell script placeholder security (#846)

The Decision section was amended on 2026-10-19 to supersede an earlier claim
that `script:` recipes used POSIX-aware scanning on PowerShell. PowerShell
`command:` and `script:` recipes both use PowerShell-aware marker scanning. In
both recipe kinds, markers are valid only at unquoted insertion sites. Markers
in single- or double-quoted strings, single- or double-quoted here-strings,
`$()` command substitutions, or immediately after a backtick are rejected.
Markers in the raw native-command region entered by an unquoted `--%` are also
rejected; scanning resumes after that region's newline or pipeline terminator.

The PowerShell quoted-context bindings are standalone single-quoted path
literals. They are not context-safe encoders for insertion inside an existing
PowerShell string. This is why those sites fail closed instead of receiving a
context-specific encoding. A marker immediately beside a PowerShell
single-quote character, including typographic forms, or another marker is also
rejected: standalone single-quoted literals could merge with neighbouring
strings or arguments. PowerShell scripts do not have inert comment regions:
markers in ordinary line-comment text remain eligible for interpolation, while
an apostrophe or other scanner-sensitive syntax in a comment can cause a later
marker to be rejected because the partial scanner cannot safely determine the
active context.

The scanner also fails closed for later markers after a typographic PowerShell
quote delimiter because it does not model delimiter pairing.

This addendum changes the PowerShell script-scanner claim and its
insertion-site contract above. ADR-034's separate contract that `$in` and
`$out` remain shell variables is unchanged.

The shell dispatch lives in `substitute_script` in
`src/ir/cmd_interpolate/mod.rs`. `SubstitutionTraversal` and its
`quote_context` are defined in `src/ir/cmd_interpolate/substitution/mod.rs`.
`PowerShellLexicalState` in
`src/ir/cmd_interpolate/substitution/powershell/lexical/mod.rs` owns
here-string, comment, and unmodelled-syntax state. Stop-parsing state is in
`src/ir/cmd_interpolate/substitution/powershell/stop_parsing.rs`. The
PowerShell script rejection cases are in
`src/ir/cmd_interpolate/powershell_script_tests/`.

## Addendum — 2026-10-19: PowerShell scanner review corrections

Follow-up review tightened comment-boundary handling for postfix `++` and `--`
hashes, and tracked scanner-sensitive text so later marker sites could fail
closed when necessary. The scanner also records unmodelled syntax before
opening a here-string and rejects markers within the raw region after `--%`,
while recognizing the region's newline and pipeline termination. The later
2026-10-20 amendment below broadens mid-token hash tracking; it supersedes the
earlier narrow hash classification. These conservative scanner rules do not
change the unquoted-only placeholder contract above.

## Amendment — 2026-10-20: Track ambiguous mid-token hashes

Review found that compact expressions such as `$x=1+#`, `$n=$s.Length#`, and
`$x=5#` could place a comment marker after an attached operator, member
expression, or numeric literal. The classifier in
`hash_follows_ambiguous_expression_boundary` recognizes hashes after assignment
and closing-bracket characters, unbraced variables, instance and static member
access, postfix `++`/`--`, redirection, arithmetic operators following a
variable assignment, and numeric literals at token or assignment boundaries. A
later marker fails closed if the comment text contains quotes, backticks,
`$()`, a here-string opener, a stop-parsing token, or a block-comment opener.
Generic argument hashes such as `foo*#` remain ordinary token text, so a later
balanced quoted argument does not trigger rejection. This keeps the partial
scanner fail-closed where PowerShell comment syntax can desynchronize it
without rejecting unrelated argument text.

The change fixes issue #846's PowerShell script placeholder injection finding:
the quoted-context path bindings are standalone literals, so any scanner
uncertainty must reject a later marker instead of changing its insertion-site
context. ADR-034's contract that `$in` and `$out` remain shell variables is
unchanged. The implementation is `hash_follows_ambiguous_expression_boundary` in
`src/ir/cmd_interpolate/substitution/powershell/tokens/mod.rs`; shared command
and script regressions are in `src/ir/cmd_interpolate/power_shell_tests.rs`,
with scanner-level token cases in
`src/ir/cmd_interpolate/substitution/powershell/tokens/tests.rs`.

## Addendum — 2026-11-01: Reject markers after an active POSIX escape

A marker that directly follows an unconsumed POSIX backslash is now rejected in
`command:`, `script:`, and heredoc-delimiter positions. The path binding
decides what that backslash escapes: between existing double quotes, a path
beginning with `"` is encoded as `\"`, so a retained backslash produces `\\"`,
which the shell reads as a literal backslash followed by a closing quote. The
rest of the path then reaches the shell as syntax. The insertion encoding, not
the template, determines the outcome, so no binding can be chosen safely at
these sites.

The rejection is deliberately broader than the demonstrated exploit: a marker
after any unpaired backslash fails closed even when the source backslash looks
inert, because the binding's first character may be escapable. Backslash pairs
are still consumed as one escaped backslash before the marker, and a
backslash-newline continuation does not count. Inside single quotes a backslash
is literal and escapes nothing, so a marker there remains eligible for
interpolation and is not rejected. Heredoc bodies remain inert.

Command, script, and delimiter traversal share the predicate
`backslash_escapes_marker` in `src/ir/cmd_interpolate/posix_lexical/mod.rs`.
The delimiter renderer reports a private `EscapeBeforeMarker` unit error, which
both traversals map to `IrGenError::InvalidCommand` carrying the original
template. PowerShell is unchanged: it has no unpaired backslash escape before a
marker and already rejects markers after its own escape character, the backtick.

## Amendment — 2026-11-06: POSIX comment and heredoc boundaries (#971)

A `#` begins an inert comment in POSIX and Bash `command:` and `script:`
recipes only outside quotes, at the beginning of input or after an unescaped
space, tab, newline, or one of the operator characters `;`, `&`, `|`, `(`, `)`,
`<`, and `>`. Those are the separators that delimit a POSIX token, so quote
tracking follows the text the interpreter executes. The set describes an
operator only where the shell treats it as one: the `)` that closes a command
substitution or arithmetic expansion stays in the surrounding word, so `#`
immediately after it is word data, whereas a `)` that closes a subshell is an
operator and `#` after it starts a comment. Every other character is word data:
a no-break space, another Unicode space, a carriage return, vertical tab, or
form feed before `#` does not start a comment, and a quote after that `#` still
opens or closes a quoted region. Heredoc delimiter words end at the same
separators, so a delimiter containing such a character remains one word and
only a line matching it terminates the body. The shared predicate therefore
prevents a false comment from skipping a real quote transition, which would
otherwise select an unquoted path encoding for a marker that the shell reads
inside quotes.

The two kinds of `)` are lexically identical without the `case` grammar: a `)`
that terminates a `case` pattern is an operator, while a `)` that closes a
substitution is word data. When a `)` is immediately followed by `#` and the
substitution's source contains the `case` reserved word, the scanner therefore
rejects the recipe instead of guessing. The tracker records the parenthesis
depth at which each `case` opened and treats a `)` as that construct's pattern
terminator only at the same depth, so a `case` inside a nested group keeps its
pattern terminator and the substitution still closes at the real one. The
enclosing quote context does not narrow the check: a substitution is parsed as
a separate script, so double quotes around it do not extend into its contents
and cannot make the boundary unambiguous. PowerShell, which has no `case`
patterns, keeps its previous unquoted-only rule because the ambiguity does not
arise there. The check deliberately over-approximates by scanning the raw
source, so a `case` keyword inside quotes or a comment also triggers the
rejection; re-lexing the substitution to narrow it would repeat the ambiguity
the check exists to avoid.

The tracker decides two things by the grammar rather than by spelling alone. A
reserved word counts only in command position, because `printf esac` passes
`esac` as an argument; command position follows a separator, a newline, or a
parenthesis and survives blanks, and a word that completes an argument list
clears it. An `esac` also closes only a construct open at its own depth, so a
pattern spelled `esac` cannot end an outer case. Comment boundaries are
reported by the traversal instead of inferred from `#`, because comment text is
copied verbatim and never reaches the tracker: the traversal announces the
comment's start and the newline that ends it, so a `case` on the following line
still opens its construct. Without both rules, losing or gaining a construct
would move the real closer and let the following `)#` read as a comment.

The boundary predicate is `is_word_terminator` in
`src/ir/cmd_interpolate/posix_lexical/mod.rs`. Comment detection and heredoc
delimiter parsing both call it, so the two cannot drift back to Unicode
whitespace independently. The regressions are in
`src/ir/cmd_interpolate/posix_lexical/tests/mod.rs`, with shared command and
script cases driving both POSIX interpolators, and the end-to-end case is in
`tests/ninja_dollar_escaping_tests/scripts/mod.rs`.

## Addendum — 2026-11-06: POSIX heredoc keyword, delimiter, and body boundaries (#998)

A heredoc tracker has to reproduce three shell rules, not one: where a keyword
or delimiter word ends, when a declaration is really a heredoc, and which line
ends a body. Recording them here, rather than inside `## Decision`, follows
this document's convention that a later finding amends the record through a
dated addendum. The rules below are the contract the implementation now
enforces, and the general marker rules in `## Decision` still apply on top of
them.

### Heredoc delimiters end only at POSIX word boundaries

The heredoc tracker must agree with the selected shell about where a delimiter
word ends. POSIX ends an unquoted word only at a blank (space or tab), a
newline, or one of the operator characters `;`, `|`, `&`, `(`, `)`, `<`, and
`>`. Every other character, including Unicode whitespace such as no-break
space, em space, vertical tab, form feed, and carriage return, is ordinary word
text. The predicate is `SourceChar::is_word_terminator`, built on
`SourceChar::is_blank`, on the `SourceChar` newtype in
`src/ir/cmd_interpolate/posix_lexical/mod.rs`. The newtype is what makes the
difference between a POSIX blank and Unicode whitespace a named operation
rather than a repeated comparison.

This is a security boundary, not a formatting preference. `<<EOF<nbsp>suffix`
declares the delimiter `EOF<nbsp>suffix`, so a line reading `EOF` does not end
the body. A tracker that treated the no-break space as a separator would end
the body at that line and resume marker lowering inside text the shell still
holds as heredoc data. An unquoted heredoc body undergoes parameter and command
expansion, so a path lowered there is re-evaluated by the shell, and backticks
inside a filename remain executable even when the recipe quotes the marker.
Word-boundary agreement prevents the premature exit; the general marker rules
in this document then keep the rest of the contract.

The same word boundary decides comment recognition. The #971 amendment above
records that contract in full; #998 relies on it and adds only what heredoc
declarations need, because the two rules must not drift apart again.

A `${ }` parameter expansion bounds the same rule from the other side. A `#`
inside one is expansion syntax, as in the pattern removal `${name# #}`, so
comment recognition is suspended until the expansion closes. Starting a comment
there would swallow the rest of the line, including a heredoc declared on it,
and marker lowering would then resume in body lines the shell still expands:
the premature-exit injection path reached from the other direction.

That suspension is counted per region rather than once for the whole line. A
brace inside a command substitution belongs to the substitution: the shell does
not match it against an enclosing `${`, and the expansion stays open across it.
Both spellings behave this way, `$( )` and the legacy backticks, because the
shell parses either body as its own command region. A single shared count would
be decremented by that brace, so the number sign after the substitution would
start a comment and lose the same heredoc declaration. The converse also
matters: a `#` inside the substitution is still a comment when the substitution
itself sits inside an open `${ }`, so each region starts its own count and the
enclosing count is restored when it closes.

Delimiter forms whose quote removal the tracker cannot reproduce, or whose text
it does not model, are rejected rather than approximated. Rejection is
fail-closed: the recipe fails at IR-lowering time with
`IrGenError::InvalidCommand` and the localized `ir.invalid_command` message,
whose snippet names the reason (for example,
`unsupported heredoc delimiter syntax: unclosed quote`). The rejected forms are
a missing delimiter, a leading unquoted `#` (which the shell reads as a
comment), an unclosed quote, a trailing backslash, a backslash-newline line
continuation whether unquoted or inside double quotes, and an unescaped `$` or
backtick outside single quotes. A backslash-escaped `\$` or backtick is
accepted, because the shell removes the escape and performs no expansion on the
result. Rendered-delimiter substitution is reparsed under the same rules, so a
lowered path that would introduce any of these forms is rejected too. A
rendered word must also stay exactly one word: a marker expands to every bound
path, joined by blanks, so a marker binding more than one path renders a
delimiter the shell would split, and that is rejected as
`unsupported heredoc delimiter syntax: rendered delimiter`. The quoting that
selects body matching is the rendered word's rather than the source word's,
because a path that needs quoting can turn an unquoted source delimiter into a
quoted one and so suppress expansion and line joining in the body. This is a
deliberate narrowing of what the tracker accepts. `<<$DELIM` and
`` <<`delim` `` are valid declarations: the shell applies quote removal to the
delimiter word and performs no parameter or command expansion on it, so it
looks for a line reading literally `$DELIM` or `` `delim` ``. The tracker does
not model those literal forms, and `<<#EOF` is a syntax error because the shell
reads `#` as the start of a comment. Those forms, and a delimiter with an
unclosed quote, now fail closed instead of being copied. The forms that remain
supported are `<<EOF`, `<<-EOF`, `<<'EOF'`, `<<"EOF"`, `<<E"O"F`, `<<\EOF`,
`<<''`, and non-ASCII delimiters that use no blank, newline, or operator
character. `<<<` is a here-string, not a heredoc, so its operator is copied and
no body is queued.

`<<` inside a `$(( ))` arithmetic expansion is the left-shift operator, not a
heredoc declaration, so heredoc tracking is suspended for the whole arithmetic
body. Both traversals record whether a `$(` opened an arithmetic expansion and
skip heredoc detection while the innermost active substitution is arithmetic.
Without that rule the tracker queues a delimiter from the rest of the
arithmetic expression and consumes the following line as body text, which
leaves the marker on that line unlowered. A nested `$( )` inside an arithmetic
expansion pushes a non-arithmetic frame, so heredocs declared by a genuine
nested command substitution are still tracked.

### Heredoc bodies end only at the shell's logical lines

The tracker must also agree with the shell about which line ends a body. An
unquoted delimiter leaves the body subject to parameter and command expansion,
and a shell removes backslash-newline throughout such a body before it looks
for the terminator. A physical line ending in an odd number of backslashes is
therefore only a fragment: the shell joins it to the next physical line, and
only the completed logical line can end the body. A line holding nothing but a
backslash leaves the logical line empty, so the next physical line still begins
it and `<<-` tab stripping still applies there. `HeredocBody` accumulates that
logical line, strips a leading tab for `<<-` only at the start of a logical
line, and compares the result with the delimiter.

This is the same security boundary as word termination. Matching physical lines
would end the body before the shell does — `prefix\` followed by an `EOF` line
forms `prefixEOF`, which is not the delimiter — and marker lowering would
resume inside text the shell still expands, where backticks in an inserted path
remain executable. The joining rule closes that path and keeps the general
marker rules in this document in force.

A quoted delimiter suppresses expansion, and with it backslash-newline joining,
so those bodies compare physical lines. An even run of trailing backslashes
leaves the newline unescaped and is likewise compared as a physical line. Both
controls, the joining cases, and their `<<-` tab-stripping variants are covered
by regressions through both traversals and the real-Ninja path.

Both recipe traversals share these decisions through the private
`posix_lexical` module. Its `mod.rs` owns the single-pass recognition and the
`SourceChar` vocabulary; `declaration.rs` parses and queues one declaration;
`delimiter.rs` owns the delimiter word's quoting grammar; and the arithmetic
classification lives beside `SourceChar` as `SubstitutionKind`, whose `at`
constructor classifies a `$(` delimiter so the two traversals cannot drift
apart on it. The split keeps each module within the workspace's 400-line limit.
Regressions run through both traversals in
`src/ir/cmd_interpolate/tests/heredoc/` and through real Ninja in
`tests/ninja_dollar_escaping_tests/heredoc.rs`.

## Addendum — 2026-11-18: Backtick quote state belongs to its opening region (#1000)

A legacy backtick command substitution saves the quote context of the shell
region that opened it. `ScriptSubstitutionTraversal` records that region's
`$()` frame depth beside the saved context and restores through the region the
depth selects, rather than writing the saved context into the script-level
field. A backtick at script level records depth zero and still restores the
script level; a backtick opened inside a `$( … )` restores that substitution's
own quote state, including when the backtick body contains nested quotes.

An open backtick body is its own shell region, so the traversal tracks the
body's own quoting there. A `)` inside a single- or double-quoted part of the
body is literal data, as the shell reads it; only an unquoted `)` may close a
`$()` frame. The body's quoting is discarded when the closing backtick restores
the region that opened it.

Nesting that would leave the saved context without its owner fails closed with
`IrGenError::InvalidCommand`; it is not approximated. Two shapes are rejected:
an unquoted `)` that would close the `$()` region owning an open backtick, and
a closing backtick seen while an inner `$()` frame is still open. Before this
amendment the first shape popped the frame, so the later backtick restored into
the script-level field. A marker after the substitution could then take the
double-quoted path encoding while the shell read it unquoted, leaving a
semicolon in a caller-supplied path executable.

This is distinct from the #629 marker invariant recorded under *Backtick
handling is two mechanisms at two different strengths*. #629 rejects a marker
inside a backtick or `$( … )`; this addendum fixes where the region state is
restored after that protected region closes. Supported syntax is unchanged: a
`$()` opened inside a backtick remains valid while it closes first, and
top-level backticks still restore the script level.

The Bash route decodes the same POSIX script and runs it under `/bin/sh`, so
the rule covers both routes. PowerShell does not use
`ScriptSubstitutionTraversal` and is outside this change. The execution
regressions are Unix-only, use `/bin/sh`, and require Ninja; Linux continuous
integration sets `NETSUKE_REQUIRE_NINJA=1`, so a missing Ninja fails the run
instead of skipping the case.

## Addendum — 2026-11-19: Shared PowerShell command-list boundaries (#993)

A `shared` PowerShell `command:` list is assembled into one script: the
interpreter parses entry `n + 1` where entry `n` stopped. The #846 fix made one
recipe's marker insertion site safe against an already-open quote in that same
recipe. It did not address a list in which one entry opens a region and the
next entry's marker is interpolated independently, because each entry was
scanned in isolation. A source path could therefore reach the script as a
standalone single-quoted literal while PowerShell was already inside a
single-quoted string, and filename text became executable syntax (CWE-78).

The scanner now walks a shared list in declaration order. Each entry is
interpolated with the previous entry's boundary, and an entry that ends inside
a single- or double-quoted string, a here-string, a block comment, or a `$()`
substitution, or that ends in a backtick escape, is rejected with an
`InvalidCommand` that names the entry and a language-neutral region code. The
rejection retains the entry's original template for inspection, never
interpolated path text, while the rendered diagnostic is the localized message
alone. The sticky fail-closed classification also follows the list, so later
markers stay protected after scanner-sensitive text in an earlier entry. Block
comments end at the first `#>`, as PowerShell does, so an apparent inner `<#`
neither deepens the region nor hides live syntax after the terminator.
PowerShell recognizes `<#` only where a token can begin, so an opener inside an
argument token — where the quotes around it still form an expandable string —
fails closed instead of opening a comment. A block opener after a mid-token
hash whose comment status is unknown fails closed for the same reason: a line
comment would make `<#` ordinary text. Escape state participates in that
boundary: a backtick keeps the character after it inside the current argument
token, so a `<#` after an escaped separator, a `<` that a backtick escapes, and
a `#` after an escaped separator are not comment syntax. A `<#` whose preceding
separator is escaped fails closed rather than hiding the quoted text that
follows. A `<` that a backtick escapes is copied as argument text and starts no
region, and a `#` after an escaped separator starts no comment, so the text
after either stays ordinary argument text and an open quote there is reported
by the unfinished-region rule.

Entries that end at a neutral boundary are unaffected, including a closed
here-string, a `#` line comment, a `--%` stop-parsing region, and a closed
block comment. Scalar commands, `script:` recipes,
`command_isolation: isolated` lists, and the POSIX and Bash routes keep their
existing scanning. Isolation is not a remedy for an entry this check rejects:
each isolated entry runs as its own script, so an entry that leaves a region
open is invalid on its own as well. The check is a boundary check over the
lexical regions this scanner models, not a complete PowerShell parser; RFC 0036
owns the longer-term harmonization under a version-matched grammar and states
the residual migration obligation. Native execution coverage runs in the
Windows recipe smoke suite; the regression and property suites run on every
platform.

## Addendum — 2026-11-19: Reject markers inside Bash ANSI-C strings (#992)

Bash reads `$'…'` as an ANSI-C string. Inside it, a backslash escapes the next
character, so `\'` is a literal apostrophe rather than a closing delimiter, and
only an unescaped apostrophe ends the string. The ordinary single-quoted path
encoding rewrites every apostrophe as `'"'"'`, which assumes those five
characters only close and reopen ordinary single quotes. A path that places a
backslash immediately before an apostrophe therefore escapes the first
rewritten apostrophe, closes the ANSI-C string on a later one, and leaves the
rest of the path to be parsed in a different quote state. The resulting line
can change the generated word, raise a syntax error, or expose path characters
as executable shell syntax (CWE-78), reported as finding
`b247e1c0-c546-4ac4-a8e1-ddb892fc9cee`.

Lowering cannot always know whether the child interpreter reads `$'…'` as
ANSI-C quoting. Both traversals reject a marker reached while the region is
open, for both `RecipeShell::Posix` and `RecipeShell::Bash`, rather than emit
the ordinary single-quoted binding. PowerShell is unchanged: it does not define
ANSI-C quoting, so its `$'` sequence is ordinary text. The rejection reuses
`IrGenError::InvalidCommand` and the localized `IR_INVALID_COMMAND` message, so
it travels through the existing diagnostic channel and reports the bounded
`MarkerInProtectedRegion` description. The rejected `command` is still retained
for callers and tests, but the rendering never quotes it.

### Opener and closing rules

Both traversals share the opener state machine in
`src/ir/cmd_interpolate/posix_lexical/ansi_c.rs`:

- Only an unquoted dollar can open the string, and the next significant
  character must be an apostrophe. A backslash-newline continuation between
  them is removed before Bash tokenizes, so it is skipped when looking ahead.
- An escaped dollar cannot open the string. In `$$`, the first dollar starts
  the PID expansion and the second is consumed by it, so neither opens a
  string; a third dollar may. Run parity, rather than a reverse scan, decides
  which dollar in a run is available.
- A `$'` sequence inside ordinary single or double quotes is literal text and
  does not open ANSI-C.
- Inside the string, the traversal consumes each backslash and the character
  after it as a pair, so an escaped apostrophe cannot close it. An unescaped
  apostrophe closes it and returns the traversal to the surrounding quote
  context. Command substitution and backtick syntax are literal inside the
  string.
- A comment, a heredoc declaration or body, and a command-substitution region
  all reset the look-behind, because a dollar before the boundary cannot pair
  with one after it.

Marker-free ANSI-C text is copied unchanged. A marker after the closing quote
is outside the region and is encoded for its own site, so supported recipes
keep working. The safe alternatives for a marker that must appear alongside
ANSI-C text are an unquoted site, an ordinary single- or double-quoted site, or
a site outside the `$'…'` region.

A string whose body holds a backslash is refused before any marker is
considered, by the escape-bearing ANSI-C guard above. Its extent therefore
never depends on which reading an interpreter takes, and the rejection names
that shape rather than the marker.

### Why the existing guards do not cover this

The post-substitution `shlex` guard admits the broken legacy encoding: `shlex`
models ordinary POSIX quotes and accepts the rewritten line as one word. Real
Bash then closes the ANSI-C string early. The regression suite pins both
observations, and the executable case passes the same path as a literal
argument to show that the failure belongs to the encoding rather than to Bash.
The `shlex` guard remains a command-shape check, not a proof that the chosen
path encoding is safe in every shell context.

This rejection is distinct from the active-escape rejection for #629. That rule
covers a marker directly after a backslash anywhere in unquoted text; this rule
covers a marker inside a region whose end the backslash changes.

### Look-behind resets and what the tests separate

A reset clears both the pending opener and the unpaired dollar. Removing a
reset was measured against the regression suite rather than assumed, and most
of the resets are boundary guards that no single case can separate. The
measurements below record what each removal does.

**Command-substitution boundaries.** `echo $$()$'{marker}'` leaves an unpaired
dollar at the first `$`, because the second `$` starts the region and never
reaches quote tracking, and the closer returns through the delimiter path.
Removing both the entry and the exit reset makes the case fail. Removing either
one alone leaves it green, because each clears what the other would. A case
whose marker sits *inside* the region cannot separate them at all: a marker in
a substitution is already rejected as a protected region, whatever the
look-behind says.

The remaining resets are guards rather than separately observable rules. Each
one was removed in turn and the suite stayed green:

- The reset taken when a heredoc declaration is copied. Two spellings were
  tried: `cat $<<EOF` and a variant that carries the dollar to the next line
  through a backslash-newline continuation. Neither separated it, so no case
  here claims to. The body case that follows is likewise unobservable, because
  the declaration reset has already run and nothing between the two reaches
  quote tracking.
- The resets at a comment boundary. For `#` to start a comment it must sit on
  a word boundary, and every word-boundary character that could separate a `$`
  from that `#` is ordinary text that already clears the dollar. The reset
  taken when a comment begins is in addition redundant with the one on its
  first copied character.
- The reset after lowering a placeholder cannot be observed while the internal
  tokens begin with `_`. That ordinary unquoted character clears both fields
  first, so the reset only keeps the invariant independent of token spelling.

## Amendment — 2026-11-19: Interpolation rejections name the failed syntax (#1145)

`is_valid_command_for_shell` is now `command_rejection`, which returns the
reason a command was refused rather than a Boolean; every earlier mention of
the old name in this record refers to it. The Fluent message
`ir.invalid_command` takes a `$reason` argument in place of `$snippet`, and
renders as `Invalid command interpolation: { $reason }.` The PowerShell and
heredoc traversals report through the same vocabulary: a private
`RejectionReason` for every traversal decision, and the existing
`DelimiterRejection` descriptions for heredoc delimiters.

The change is about disclosure, not style. `env()` is a Jinja helper, so it
runs while the manifest is rendered, before IR lowering. By the time a command
reaches this guard, any permitted environment value the recipe interpolated is
already part of the text. Quoting that text in the diagnostic — which the
`$snippet` form did, truncated to 160 characters — copies the value to standard
error in default human output and into the JSON diagnostic alike. The
truncation bounds the length of the leak without preventing it.

A rejection therefore names the syntax that failed: `unmatched backtick`,
`unparseable shell syntax`, `escape sequence before a recipe marker`,
`recipe marker inside a protected shell region`,
`recipe marker inside a PowerShell here-string`,
`recipe marker inside raw native command arguments`,
`recipe marker at an unsupported source position`, and, for delimiters, the
descriptions in `DelimiterRejection`. Each is fixed English text inserted into
a localized sentence, the same shape the heredoc rejections already used.

`IrGenError::InvalidCommand` retains the rejected `command` because callers and
tests inspect it, but its `Display` is the localized `message` alone. The field
that reaches the message is typed `&'static str`, so rendered text cannot be
placed in it. The doctest on the variant and
`permitted_environment_value_stays_out_of_the_interpolation_diagnostic` in
`tests/assert_cmd_tests/runner_diagnostics.rs` pin both properties.

The subsection on the `shlex` guard above records the contract before this
amendment; its predicate name and `$snippet` rendering are superseded here. Its
statements about which inputs are rejected, and that the accepted set is not a
stability commitment, still hold.

## Addendum — 2026-11-19: Heredoc delimiter escape regression coverage (#988)

A follow-up security finding (ID `e271961c-a57f-47ec-a2a9-85a95aa443c7`,
fingerprint `257158009af9723fe849205b8d68104d8b3d3533815b545f1c133b9f9724ed30`)
reported that heredoc delimiter rendering appends the ordinary path encoding
after a retained active backslash, so the backslash can escape the encoding's
opening quote and let path text become shell syntax. The 2026-11-01 addendum
above already rejects that shape at the delimiter site, and the renderer fails
before it extends the caller's output buffer, so no path text reaches the
script. This addendum records the verification and the decisions it reaffirms;
it changes no production behaviour.

Rejection, rather than a delimiter-specific encoder, remains the decision. An
encoder would have to reproduce the shell's quote removal and still render a
word the terminator line can match, and the binding decides whether the
backslash is active, so no encoding can be selected safely at that site. The
full-consumption reparse in `rendered_terminator` is a second, independent
check: a rendered word that leaves text outside the parsed word is rejected as
`unsupported heredoc delimiter syntax: rendered delimiter`, which also covers a
marker that renders as several words. An odd backslash count is rejected and an
even count decodes to literal backslashes before the marker; a single-quoted
backslash remains literal and leaves the marker eligible.

Coverage now pins both markers, both traversals, both POSIX-compatible shells,
unquoted, double-quoted, and mid-word delimiters, a backslash-parity table, and
a bounded property over shell-metacharacter paths. A CLI case proves the
rejection happens before Ninja starts, and a real-Ninja case proves paired
backslashes keep the body quoted. The general verified-transformation model
that would replace these case analyses remains deferred to
[RFC 0036](../rfcs/rfc-0036-shell-parsing-and-verified-transformations.md).

## Amendment — 2026-11-19: Heredoc bodies start at their command line (#991)

POSIX starts a queued heredoc body after the unquoted newline that ends the
command line declaring it. The tracker previously started the body at any
newline while a declaration was pending, including one inside a quoted
argument. That newline is argument text the shell keeps reading through, so
starting the body there consumed the closing quote as body content and left the
traversal inside a quote the shell had already closed. A marker after the real
terminator was then lowered with the double-quoted path encoding even though
the shell read it outside quotes: a path whose semicolon is escaped in the
unquoted encoding stayed bare in the double-quoted one, and the shell ran the
commands embedded in it.

Each queue entry now records the `FrameId` of the shell region that declared it.
`enter_command_substitution_region` allocates a fresh identifier for every
`$()` or backtick body, so a declaration inside that region is owned by it and
a declaration whose region has closed is owned by an identifier no longer on
the stack. Both traversals classify each newline through their own
`newline_context` and offer it to one entry point,
`PosixLexicalState::activate_pending_heredoc`. An unquoted newline in the
owning region starts the body; a quoted newline in the owning region leaves it
waiting for the closing quote's line; and any other combination fails closed.
That covers a newline in another region, inside backticks, inside `$(( ))`,
inside an open `${ }`, or after the declaring region has closed. Refusal is
reported as `IrGenError::InvalidCommand` through
`invalid_heredoc_boundary_error`, whose reason names the rejected boundary, on
the same channel and in the same form as the existing delimiter rejections. The
reason is fixed English text inside the localized sentence, not translated
prose, so a new rejection variant needs no locale entry in any catalogue.

Neither reading of the quoting rules is enough on its own for Bash's ANSI-C
strings. Bash reads `$'a\'b'` as one string, while the POSIX single-quote rule
ends it at the escaped quote and reopens it at the real closer, so a `<<`
between the two is never recognized and the following newline cannot be placed.
Rejecting only a declaration that is already pending leaves that earlier order
exploitable, because the body is then lowered as script text. Both traversals
therefore reject an escape-bearing ANSI-C string wherever they scan command
text, through `RejectionReason::AmbiguousAnsiCQuote`; the detector steps over
unquoted backslash-newline continuations, which Bash removes before reading the
string. Comments and heredoc bodies are copied before it runs, so an ANSI-C
string there stays inert data, and escape-free strings end at the same quote
under both readings and stay supported.

A newline that ends a comment is routed through the same entry point rather
than activating the queue inside the lexical state, so comment text stays
copied verbatim while its terminating newline takes the shared decision. The
decision runs before the traversal copies the character, so a refusal leaves
the traversal's quote state and accumulated output untouched. The end of the
script is not treated as a boundary: a body that never reaches its terminator
is copied literally and is not reported. PowerShell has no heredoc tracking and
is unaffected.

The region identifiers and the refusal reasons live in the new
`posix_lexical/diagnostics.rs` child, beside `declaration.rs`, `delimiter.rs`,
and `parameter_expansion.rs`, keeping each module within the workspace's
400-line limit. Regressions for both traversals are in
`src/ir/cmd_interpolate/tests/heredoc/boundary_tests.rs`; the quoted-newline
injection path runs through real Ninja and `/bin/sh` in
`tests/ninja_dollar_escaping_tests/heredoc.rs`.
