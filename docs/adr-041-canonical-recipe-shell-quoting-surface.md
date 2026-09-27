# Architectural decision record (ADR) 041: Canonical recipe shell quoting surface

## Status

Accepted.

## Date

2026-09-27

## Context and problem statement

A manifest renders its string fields as MiniJinja templates, and a rendered
value frequently becomes part of a shell recipe. Before this decision the
template surface had no way to quote such a value, so an author who needed a
literal space, quote, or `$` in an argument had to hand-roll the escaping in
the manifest. Hand-rolled escaping is where command injection lives, and the
manifest author is the party least able to verify it.

The design document named the gap and the intended remedy. `DD-4.5` called a
quoting filter "a non-negotiable security feature to prevent command injection
vulnerabilities". `RFC-0006-8.9` proposed the name `shell_quote` with a
`dialect` argument, and `RM-6.8.3` repeated both. Three sources therefore
agreed on the shape of the surface before any of it existed; none of them
agreed on its scope, and the RFC's premise about which encoders are compiled in
was false.

Three questions had to be answered, and each is a decision below: what the
filter is called, which dialects it accepts and what it defaults to, and whether
`bash` is one of those dialect names.

### The RFC's stated premise is false in the code

`RFC-0006-8.9` says "`dialect` currently accepts only `sh`, matching the single
`shell-quote` feature Netsuke enables." The feature half is right —
`Cargo.toml` enables only `sh` — but the conclusion does not follow. Two facts
in the code contradict it.

`src/recipe_shell.rs` establishes that the default Windows recipe interpreter
is Windows PowerShell, and `RecipeShell::host_default()` returns
`RecipeShell::PowerShell` there. A manifest on Windows therefore executes under
an interpreter with entirely different quoting rules.

`src/ir/cmd_interpolate/` already implements a second, non-`shell-quote`
encoding for exactly that case. The capability the RFC described as absent was
already compiled in and already in use by the IR lowering path.

So an `sh`-only filter would emit POSIX quoting into a recipe that Windows
PowerShell then parses. Nothing would fail loudly; the quoting would simply be
read as data, and the argument the author believed was protected would arrive
corrupted or expanded. That is a silent injection-safety defect in the helper
whose stated purpose is to prevent injection.

### Nothing else in the ecosystem offers a dialect argument

Every comparable tool ships a one-argument function with no dialect selector:
`shlex.quote` and `shlex.join` in the standard library, Just's `quote()`,
bazel-skylib's `shell.quote`, Nix's `escapeShellArg`, and Ansible's `quote`.
The dialect argument is this surface's one point of divergence from the field,
and it is the reason the next decision is not free. It was considered for
removal and kept; the alternative is recorded below.

## Decision drivers

- Make the safe thing the easy thing. An author with a hard-to-quote argument
  should have a one-call answer, because the alternative is hand-rolled
  escaping.
- Never emit one dialect's quoting into another dialect's interpreter. A
  quoting helper that is wrong silently is worse than no helper, since it
  invites reliance.
- Keep exactly one implementation of the encoding. IR lowering and the
  template filters must not drift apart, because a recipe assembled from both
  would then carry two incompatible quotings.
- Do not put a name on the public template surface that a known-correct source
  says is already superseded.
- Defer no breaking rename to a later release. The template surface becomes
  documented and executed in the same milestone that ships it.

## Requirements

### Functional requirements

- A manifest can quote one string into one shell word, and quote a sequence
  into a command line of shell words, without hand-rolled escaping.
- The dialect used is selectable at the call site, and the spelling is stable
  enough to document.
- A call that cannot succeed — a non-string subject, an unknown dialect, a
  positional argument — fails with a diagnostic naming the filter and the
  received kind, rather than coercing the value.

### Technical requirements

- Exactly one implementation of recipe-shell word quoting is compiled in;
  every other call site delegates to it.
- The names shipped match what `RFC-0006-8.9`, `RFC-0006-13.3`, and `RM-6.8.3`
  say, or the deviation is recorded where a reader will find it.
- The decisions are discoverable from the documentation index and from the
  code they govern.

## Options considered

### Option A: `shell_escape`, single dialect (the roadmap's original name)

Ship the filter under the name the roadmap first used, with POSIX quoting only.

This is rejected on two independent grounds, either sufficient. The name is
known-superseded: `RFC-0006-13.3` states that the RFC "contributes the
canonical name `shell_quote` and the `dialect` argument; the roadmap task
should adopt them so the two do not diverge", and `RM-6.8.3` repeats it.
Shipping `shell_escape` would put a superseded name on the public surface and
force a breaking rename in a later release. `RM-3.14.8` permits "implement **or
remove**" the name; superseding it is a removal.

The single dialect is the more serious problem, for the reason above: on
Windows the filter would quote for the wrong interpreter and fail silently.

### Option B: `shell_quote`, `sh` only, error on a PowerShell host

Ship the canonical name but refuse to render when the active recipe shell is
PowerShell, so the filter cannot be used incorrectly.

This avoids the silent corruption but makes the filter unusable in the default
configuration on Windows, which is the platform where manifest authors most
need quoting, since it is the platform whose paths contain spaces and
backslashes. Refusing at render time is safe and useless.

### Option C: `shell_quote`, two dialects, host-dependent default (chosen)

Ship the canonical name with both dialects compiled in, defaulting to the
dialect implied by the active `RecipeShell`.

This is the only option that is both correct on every host and usable on every
host. Its cost is the dialect argument, which no comparable tool has, and the
instability of the omitted-dialect default, which is recorded as a decision in
its own right rather than left implicit.

### Option D: `shell_quote` with no dialect argument

Ship the canonical name and always follow the active recipe shell, dropping the
argument entirely.

This is the smallest surface and the closest to the ecosystem's convention. It
was considered seriously and not adopted: the requester chose the two-dialect
surface explicitly, and both `RFC-0006-8.9` and `RM-6.8.3` specify the
`dialect` argument by name, so dropping it would be a second deviation on top
of Option C's. It would also remove four of the six diagnostic keys and the
dialect-totality obligation, so it is a real reduction rather than a cosmetic
one.

The safety argument for Option D is adopted instead: every documented example
and every snapshot pins `dialect` explicitly, and the instability of the
omitted-dialect default is stated in the consequences below.

| Topic                         | A: `shell_escape`, `sh` | B: `sh` only, error | C: two dialects  | D: no argument   |
| ----------------------------- | ----------------------- | ------------------- | ---------------- | ---------------- |
| Name matches RFC and roadmap  | No                      | Yes                 | Yes              | Yes              |
| Correct on a PowerShell host  | No, silently            | N/A, refuses        | Yes              | Yes              |
| Usable on a PowerShell host   | No                      | No                  | Yes              | Yes              |
| Breaking rename deferred      | Yes                     | No                  | No               | No               |
| Call site states its encoding | No                      | No                  | Yes              | No               |
| Diagnostic keys introduced    | 6                       | 6                   | 6                | 2                |
| Diverges from ecosystem norm  | No                      | No                  | Yes              | No               |
| Reader can tell it was chosen | No, reads as legacy     | Yes, this record    | Yes, this record | Yes, this record |

*Table 1: Comparison of the options considered.*

## Decision outcome

Adopt **Option C**.

The helper ships as `shell_quote` and `shell_join`. Both accept a `dialect`
keyword argument taking `sh` or `powershell`, and both default to the dialect
implied by the active `RecipeShell` when the argument is omitted. The `sh`
dialect is documented as an *encoding* — the output is a lowest common
denominator for Bash, Z Shell, and `/bin/sh`-like shells — rather than as a
named interpreter.

`dialect='bash'` is **not** accepted. `RecipeShell::Bash` maps to the `sh`
dialect, because `sh` output is valid Bash. The name is refused because the
`shell-quote` crate's `Bash` encoder emits a different, `$'...'`-style form
that Netsuke does not compile in; accepting the name now would lock in a
meaning that a real `bash` dialect would later have to break.

Every `shell_escape` reference in the design and user guides is rewritten to
name `shell_quote`, and the filter is implemented once, in `src/shell_word.rs`,
with the template filters and the IR lowering path both delegating to it.

## Rationale

The decision turns on what a quoting helper owes its caller. Its value is
entirely in being right; a helper that is right on the author's machine and
wrong on a colleague's is worse than nothing, because it teaches reliance it
cannot support. Options A and B each break that promise in a different
direction — A silently, B loudly — and only Option C keeps it on every host.

The name follows from a different source with the same force. Three documents
agree on `shell_quote`, one of them an RFC that states the roadmap "should
adopt" it so the two do not diverge. Diverging would mean a public name, a
documented example, and a test suite that all have to change later, for no
benefit that any of the rejected options could name.

The dialect argument is the part that is genuinely a cost, and it is accepted
rather than waved away. It diverges from every comparable tool, it doubles the
diagnostic keys, and it introduces the one axis of this surface with no
versioning story: a manifest that omits `dialect` gets whatever the active
`RecipeShell` implies, which may change between releases. Option D would remove
that axis, and the argument for it is real. It was not adopted because two
normative documents specify the argument and the requester chose it explicitly,
which makes removing it a larger unilateral change than the cost justifies. The
mitigation is disclosure: the instability is stated here, every example pins
the dialect, and the diagnostic enumerates the accepted names so a reader
cannot guess wrong silently.

Option A is rejected on the name and on the Windows defect, and the two are
independent — either alone would sink it. Option B is rejected on usability
rather than on correctness; it is the safe version of the wrong answer. Option
D is rejected on authority rather than on merit, and the record says so, so
that a future reader who prefers it knows it was weighed rather than missed.

## Consequences

- `shell_quote` and `shell_join` are available to every manifest template, and
  the encoding is selected by `dialect`, defaulting to the active recipe shell.
- **The omitted-dialect default is not stable across releases or hosts.** A
  manifest whose generated text must be byte-stable must pin `dialect=`. This
  is the accepted cost of Option C, and the one behaviour here that a future
  release may change without a manifest edit.
- `RecipeShell::Bash` maps to the `sh` dialect today. If a real `bash` dialect
  is added, that mapping changes, which is a concrete instance of the point
  above.
- The name `shell_escape` no longer appears in the design or user guides.
- `src/shell_word.rs` is the single implementation of recipe-shell word
  quoting, and a constraint test holds the delegation to one call site per
  layer rather than to a convention.
- The `shell-quote` crate's `Sh` encoder is *suffix*-quoting rather than
  canonically enclosing: it leaves the longest safe prefix bare and quotes only
  the remainder, so `a b` becomes `a' b'` and not `'a b'`. The contract is
  therefore round-tripping, not any particular output shape.

## Known risks and limitations

- The surface is a *safe primitive*, not an enforced control. It quotes what it
  is given; it does not detect or prevent an author interpolating a filter
  inside shell double quotes, where the quoter's own quote characters are data
  and the argument is corrupted. That failure mode is pinned as a documented
  defect with a test rather than repaired, because repairing it would mean
  tracking shell context, which is a different and much larger mechanism.
- The only oracle for the PowerShell encoder on most hosts is an independently
  written decoder, not an interpreter. Where a real interpreter is present the
  two are compared, but the property is discharged by the model on hosts
  without one, and the residual gap is recorded rather than closed.
- The dialect set is a claim about which encoders are compiled in. It is
  correct for the current `Cargo.toml` feature selection and would need
  revisiting if a further encoder were enabled.
- Accepting the dialect name case-insensitively is a convenience that the
  design documents do not require; a reader who expects exact matching may be
  surprised.

## References

- [ADR-014](adr-014-backend-text-escaping-seam.md) defines the single-line
  recipe admissibility rule that the encoding is layered on top of.
- [ADR-019](adr-019-structured-command-shell-selection.md) owns the shell
  registry, which accepts `bash` where this decision rejects it. The two
  surfaces are deliberately not the same, and the divergence is recorded here.
- [`RFC 0006`](rfcs/0006-ansible-inspired-template-standard-library.md) §8.9
  proposed the `dialect` argument and the `shell_quote` name; §13.3 states that
  the roadmap task should adopt both.
- [`src/shell_word.rs`](../src/shell_word.rs) holds the single encoding.
  [`src/stdlib/recipe_text/`](../src/stdlib/recipe_text/) is the
  template-facing adapter that validates arguments and delegates to it.
- [`src/recipe_shell.rs`](../src/recipe_shell.rs) is the data-only interpreter
  vocabulary, including `host_default()`.
