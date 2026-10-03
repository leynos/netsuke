# RFC 0019: Encoding, identity, and formatting helpers

## Preamble

- **RFC number:** 0019
- **Status:** Proposed
- **Created:** 2026-10-03
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.8
- **Originating issue:** [#597](https://github.com/leynos/netsuke/issues/597)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies Netsuke's encoding, identity, and formatting helpers: the
nine pure filters `b64encode`, `b64decode`, `urldecode`, `to_uuid`,
`shell_quote`, `comment`, `human_readable`, `human_to_bytes`, and `text_hash`.
Together they let a manifest produce comment banners, encoded payloads, quoted
shell words, human-readable sizes, and content-derived identifiers without a
subprocess and without a second quoting implementation. Section 5.1's registry
lists nine members, **all nine new pure filters**, and all nine are usable from
a manifest query.

The group is RFC 0006 section 8.9 entire, which is why it is a child rather
than part of a neighbour: section 14.13's partition splits at the capability
seam, and "encode, identify, and format a value" is one seam rather than four.
Two members carry a debt the rest of the set does not, and both are named here
rather than discovered in section 5. `shell_quote` was **already delivered** by
roadmap task 3.14.8, so this RFC adopts it rather than specifying it from
scratch, and what remains is the dialect set. `text_hash` is a **rename**, not
an introduction, so section 5.10 and section 11.1's policy applies to it.

## 2. Problem

A manifest that writes a generated file today can compose its text but not
decorate or encode it. Three concrete gaps, all of which send an author out of
the templating layer and into a subprocess or a hand-rolled escape:

- **A comment banner has no safe spelling.** An author writing `# {notice}`
  into a generated shell fragment has no way to know whether `notice` contains
  a newline, in which case the second line escapes the comment and becomes live
  syntax in the generated file.
- **A shell argument has no safe spelling either.** The same problem one level
  down: an argument containing a space, quote, or metacharacter must be quoted
  or the recipe splits it. RFC 0006 section 8.9 names "a second quoting
  implementation" as the thing to avoid, and Netsuke already carries exactly
  one in `src/shell_word.rs`, used by IR lowering and Ninja rendering.
- **A size or a digest has no canonical text.** `human_readable` must produce
  one deterministic string per input, and `text_hash` must not be confused with
  the existing `hash`, which treats its subject as a *path*.

The last is a trapdoor rather than a gap, and it is the reason `text_hash` is a
new name instead of an overload. If `hash` accepted both a path and a string, a
manifest bug that turned a path into arbitrary text would stop erroring and
start returning a plausible wrong digest. Section 11.1 of RFC 0006 records the
resolution; this RFC implements it.

## 3. Goals and non-goals

- Goals:
  - Add the nine section 8.9 filters, all pure, all available in manifest
    queries, with no subprocess and no second quoting implementation.
  - Keep `text_hash` distinct from `hash`, so the path/string distinction stays
    a type error rather than a silent wrong answer.
  - Reuse the existing `legacy-digests` gating for `sha1` and `md5` rather than
    inventing a second policy for the same algorithms.
  - Make the encoders round-trip with the decoders Netsuke actually ships,
    which is why `urldecode` defaults `plus=false` and `b64decode` rejects
    non-canonical input by default.
  - Make every error name the offset or the accepted set, so a rejection is
    actionable without reading the source.
- Non-goals:
  - Byte-string values. Netsuke has no such value kind, so binary input is out
    of scope and a decode that yields invalid UTF-8 errors rather than
    returning bytes.
  - `to_nice_yaml` and Ansible's banner-drawing `plain` comment style, rejected
    in RFC 0006 sections 10.2 and 8.9.
  - `checksum`, `md5`, and `sha1` as filter names, rejected in section 10.4.
  - The `bash` dialect of `shell_quote`, which
    [ADR-041](../adr-041-canonical-recipe-shell-quoting-surface.md) refuses and
    3.14.8 implements. Section 8.9 records the refusal; this RFC does not
    reopen it.
  - Widening the dialect set beyond `sh` and `powershell`, which is roadmap
    task 6.8.3's remaining scope and not this RFC's.

## 4. Capability set

Nine new pure filters.

- `b64encode(urlsafe=false, padding=true)` — encode UTF-8 text as Base64.
- `b64decode(urlsafe=false, strict=true)` — decode Base64 to UTF-8 text.
- `urldecode(plus=false)` — percent-decode to UTF-8 text.
- `to_uuid(namespace=...)` — deterministic UUID version 5 over the subject.
- `shell_quote(dialect='sh')` — quote one value for a named shell dialect.
- `comment(style='hash', prefix=none)` — decorate text as comments.
- `human_readable(unit_system='binary', unit=none, precision=2, bits=false)` —
  format a byte or bit count for display.
- `human_to_bytes(default_unit=none, bits=false)` — parse a display size
  strictly.
- `text_hash(algorithm='sha256')` — hash a string's UTF-8 bytes.

This section does not restate any contract. Each helper's argument shape,
options, rejection conditions, and edge cases are specified in
[RFC 0006 section 8.9](0006-ansible-inspired-template-standard-library.md#89-encoding-identity-and-formatting),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

### 5.1. Registry

| Helper           | Namespace | Registration | Purity class | Manifest query |
| ---------------- | --------- | ------------ | ------------ | -------------- |
| `b64encode`      | Filter    | New          | Pure         | Yes            |
| `b64decode`      | Filter    | New          | Pure         | Yes            |
| `urldecode`      | Filter    | New          | Pure         | Yes            |
| `to_uuid`        | Filter    | New          | Pure         | Yes            |
| `shell_quote`    | Filter    | New          | Pure         | Yes            |
| `comment`        | Filter    | New          | Pure         | Yes            |
| `human_readable` | Filter    | New          | Pure         | Yes            |
| `human_to_bytes` | Filter    | New          | Pure         | Yes            |
| `text_hash`      | Filter    | New          | Pure         | Yes            |

Nine `New` rows and no `Option added` row. The nine are the whole of RFC 0006
section 8.9, which the coverage map gives this row and no other; the roadmap's
step 6.8 owns all nine.

**All nine are Filter, and two of them are worth stating.** RFC 0006 section
14.13's coverage map gives this row `8.9` and nothing else, and every section
8.9 helper is a filter, so the namespace column has one value here — the
opposite of RFC 0018's row, where the section mixes filters, tests, and an
option. `shell_quote` is a filter because the section 7 row it is renamed from
is `quote`, which sits in the 7.1 core-filters table; the rename carries the
namespace with it, which is what makes the contract's namespace comparison
load-bearing rather than decorative.

`shell_quote` is registered `New` even though roadmap task 3.14.8 **already
delivered it**. The contract derives its expectation from the document, not the
code: `quote` is one of section 7.8's three rename exceptions, so `shell_quote`
enters the accepted set through it and is classified `New` by
`check_rows_agree_with_survey`. Marking it `Option added` would fail the
contract, which is correct — it is not an existing helper gaining an option, it
is a renamed capability — and the registry states that rather than misreporting
the shipped state. What the column records is the survey's disposition of the
capability, and the survey's disposition is a new name.

The registry takes the written children to **50 of the 52 pure helpers**. RFC
0013 contributes 5, RFC 0014 6, RFC 0015 15, RFC 0016 8, RFC 0017 7, RFC 0018
0, and this RFC 9. RFC 0018's zero is the group's defining property rather than
an omission: it is uniformly non-pure, so its six registry rows contribute
nothing to the pure count. The remaining two pure helpers are RFC 0020's, which
is the only child that can still bring the aggregate to 52.

### 5.2. Manifest-query availability

No additional obligation beyond RFC 0006 section 6.2. All nine helpers are
pure, so clause 1 registers them in `register_query_helpers` and none is
stubbed; the always-failing-stub path of clause 2 is not reached by this group.
That is the clause's first fully uniform disposition since RFC 0017, and it
follows from the section 6.1 purity class rather than from a decision here: a
pure helper is available in a manifest query by definition, and section 6.2
admits nothing else.

The one helper whose availability is worth stating explicitly is `shell_quote`,
because it is already registered and its registration is what must not change.
`src/stdlib/recipe_text/mod.rs` is wired through the same query registration as
the seven helpers around it, so the filter is already reachable from a manifest
query and this RFC neither adds nor removes a registration for it. What it
changes is the documented dialect surface, and section 5.10 records where that
lands.

### 5.3. Determinism

No additional obligation beyond RFC 0006 section 6.3. Every helper in this
group is a pure function of its subject and its arguments: none reads a clock,
the environment, the filesystem, or the network, and none keys a result on a
hash. There is no sequence whose order could vary and no iteration whose order
could leak into a generated graph.

Three members have a determinism obligation sharper than "same input, same
output", and each is stated rather than left to the implementation:

- **`human_readable` output is a locale-independent string.** Section 8.9
  pins ASCII digits, `.` as the decimal separator, no digit grouping, and one
  space before the unit. A formatting routine that reached for a locale — the
  platform's decimal comma being the obvious trap — would produce two different
  generated files from one manifest on two machines, which is the failure
  section 6.3 exists to prevent. Rounding is half away from zero with trailing
  zeros retained, so the output is a function of the input and not of a
  formatter's defaults.
- **`to_uuid` is deterministic by construction.** UUID version 5 derives the
  result from the subject and the namespace, so the same subject and namespace
  always produce the same identifier. Section 8.9 freezes both the derivation
  of the default namespace and its literal value, so the default is
  reproducible rather than inherited from Ansible.
- **`text_hash` output is lowercase hexadecimal.** The algorithm is fixed by
  the argument and the encoding is fixed by the clause, so no platform's casing
  convention can reach the generated text.

The one place a caller could mistake non-determinism for determinism is
`shell_quote`'s *default* dialect, which is host-dependent: `sh` on Unix and
PowerShell on Windows. That is precisely the situation clause 6.3's disclosure
rule covers, and section 5.10 records how 3.14.8 already handles it — the
default is resolved once at registration and disclosed by D6, not read per
call. The helper is therefore deterministic given a dialect, and the omission
of the argument is the disclosed host-dependence.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. This group is the child
that needs the clause least and satisfies it most easily: **it takes no
capability handle at all.** Nothing here opens a file, resolves a path, or
reads a directory; every helper operates on a value the template already holds.
There is therefore no path outside a capability to reject, no metadata lookup
to route, and no `cap_std` handle to thread — which is a property of the group
rather than a gap in it, and it is why section 5.1's registry has no filesystem
class.

The clause still reaches two members, and each is worth naming because the
temptation to cross the boundary is real:

- **`comment` does not read the file it decorates.** It decorates text. The
  helper is used on the way *into* a generated file, so the natural next
  thought — "check whether the target already contains the closing marker" —
  would turn a pure filter into a filesystem-observing one and remove it from
  every manifest query. Section 8.9 puts the check on the *input text* instead,
  which is the only form that keeps the helper pure, and that is the whole
  reason the guard is specified over the subject rather than over the target.
- **`shell_quote` does not consult the host to choose its dialect.** It takes
  the dialect as an argument and resolves the default once at registration,
  which is what keeps a host-dependent fact from becoming a per-call read.
  Section 5.10 records the disclosure.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. Eight of the nine helpers
are platform-uniform: Base64, percent-decoding, UUID version 5, comment
markers, the size formatters, and the digest are defined over the subject's
bytes and arguments, and produce byte-identical output on every platform. There
is no `st_dev` to compare, no volume root to read, and no symlink to follow, so
the group is the opposite of RFC 0018 in this respect: where that group's
platform contract was its load-bearing test, this group's is its simplest
clause.

`shell_quote` is the one divergence, and it diverges in the way ADR-041 settled
rather than in a way this RFC invents. Two things are true at once and are
worth separating:

- **The output for a named dialect is platform-uniform.** `shell_quote('sh')`
  produces the same text on Windows as on Unix, because the encoding is
  selected by the argument and not by the host.
- **The default dialect is host-dependent.** `RecipeShell::host_default()`
  returns PowerShell on Windows and `sh` elsewhere, which section 8.9 records
  as the reason the filter is not `sh`-only: an `sh`-only filter would emit
  POSIX quoting into a recipe Windows PowerShell then parses, silently
  corrupting the argument the author believed was protected.

So the platform contract the guide must state is a two-part sentence: the
dialect argument makes the output uniform, and omitting it makes the output
follow the host. Continuous integration must exercise the second half on both
platforms, because a default that is only tested on Unix is a default whose
Windows branch is unverified. `powershell` output is generated on Unix and `sh`
output on Windows in the same suite, so the encoders are exercised on both
hosts without needing a Windows runner for the pass and a Unix one for the fail.

### 5.6. Type and error contract

The group defines one private domain error enum, `TextError`, and exactly one
`impl From<TextError> for minijinja::Error`, per clause 6.9. It is modelled on
RFC 0013's `InterchangeError` because this group has the same shape of problem
— one enum serving several related parsers — and the argument that justified it
there applies here unchanged: a group with more than a dozen conditions should
not construct its errors ad hoc at each site.

The conditions divide into five families, and naming them is what shows the
enum is a specification rather than a bag.

- **Encoding conditions**, from the two decoders. `b64decode` and `urldecode`
  each decode text to bytes and must then admit that Netsuke has no byte-string
  value to return, so a decode that does not yield valid UTF-8 is an error.
- **Parse conditions**, from `human_to_bytes` and `to_uuid`. Both read a
  grammar out of the subject: a size with an optional sign, number, and unit; a
  UUID in canonical hyphenated form.
- **Format conditions**, from `human_readable`. The subject must be an integer
  or a finite float; a non-finite float has no display form.
- **Dialect conditions**, from `shell_quote`. The dialect is a closed set and
  an unknown value enumerates the accepted names.
- **Digest conditions**, from `text_hash`, which are the two the existing
  `hash` already emits.

Two decisions in the enum are substantive and are recorded here rather than in
the code.

**The enum is shared, so its codes name the group and not the syntax.** RFC
0013 argued this for `interchange` over `json` or `yaml`; the argument here is
the same and the temptation is different. The obvious module name for this
group is `encoding`, which is wrong for two reasons: it describes only the
first two families, and `text_hash` is not an encoding at all. The module
segment is therefore `text`, matching the filter names that the group's own
members already carry — `text_hash` is the survey's own rename, and the section
8.9 title is "Encoding, identity, and formatting". The codes are
`netsuke::jinja::text::*` under `STDLIB_TEXT_*`, and the three families that
are not encoding have a code namespace that does not pretend otherwise.

**`legacy-digests` reuses the existing gate rather than adding one.**
`text_hash` accepts `sha1` and `md5` only when the existing Cargo feature is
enabled, and emits the same feature-gated diagnostic `hash` already emits.
Section 8.9 states the reuse as a requirement rather than a preference, and the
reason is that a second gate for the same algorithms would be a second place
for the policy to drift. The clause 6.9 obligation this creates is that
`text_hash`'s digest conditions must route through the *same* key family the
existing helper uses, not a parallel one, so the diagnostic a manifest author
sees does not depend on which of the two helpers they happened to call.

The `to_uuid` SHA-1 question is adjacent but distinct, and gets its own
paragraph because a reader will look for it. UUID version 5 is defined over
SHA-1, which is a `legacy-digests` algorithm; but UUID version 5 uses it as a
namespacing primitive rather than as a security digest, so it does not fall
under the feature policy and `to_uuid` is available with the feature disabled.
Section 8.9 records the distinction at the helper, and this RFC restates it
because a future reader who "fixes" the apparent inconsistency would break the
default namespace every manifest depends on.

### 5.7. Canonical value equality

No additional obligation beyond RFC 0006 section 6.7. Nothing in this group
keys a value, deduplicates a sequence, or performs set membership over a
canonical form. Every helper returns a string, except `human_readable` and
`human_to_bytes`, which are an inverse pair over a number, and no identity
relation is consumed or produced.

Two members produce text that *could* be mistaken for a canonical key, and both
are deliberately not one:

- **`text_hash` is not an identity relation.** Two subjects with the same
  digest are not thereby the same value, and Netsuke does not use a digest as a
  key anywhere. The helper's output is a string a manifest may interpolate;
  section 6.7's canonical key is a different mechanism and this group does not
  touch it. The distinction matters because the obvious next thought — "use
  `text_hash` to deduplicate a list" — would introduce a collision-dependent
  equality that section 6.7 forbids.
- **`to_uuid` is not an identity relation either.** It is deterministic, so
  the same subject always yields the same UUID, but the converse does not hold:
  the UUID does not recover the subject, and two different subjects under two
  different namespaces are unrelated. Determinism is what section 5.3 owed;
  identity is not something this group claims.

### 5.8. Resource bounds

The clause reaches this group through one member that materializes a value
larger than its input, which is the shape RFC 0006 table 3 bounds, and through
several that do not.

`comment` is the member. Its output is the input plus a marker per line, so for
an input of `n` lines and `m` bytes the output is bounded by `m + n * k` where
`k` is the longest marker. That is linear in the input with a small constant,
and it is the same shape as the bounds RFC 0017's string helpers carry. It adds
no traversal and reads no file, so there is no amplification beyond the
per-line marker and no second bound to state.

The remaining eight are bounded by their input or output rather than by a
materialized expansion, and each is stated because the reasoning is not uniform:

- **The two decoders shrink.** `b64decode` and `urldecode` produce at most
  their input's length, so table 3's input row bounds them and no output row is
  needed.
- **The two size formatters are constant-bounded.** `human_readable` produces a
  short fixed-width string for any input, and `human_to_bytes` produces one
  integer. Neither can amplify.
- **`text_hash` is constant-bounded.** The digest is fixed-width by the
  algorithm. Its cost is linear in the subject, which is the same cost the
  existing `hash` pays over a file's contents, and section 8.9 assigns it no
  output bound for the reason RFC 0006 table 3 gives: a fixed-width result
  cannot exceed an output ceiling.
- **`to_uuid` is constant-bounded**, producing the 36-character canonical form
  for any subject.
- **`shell_quote` is the one worth a sentence more.** Its output can be longer
  than its input — quoting a string adds escapes — but the growth is bounded by
  a constant factor per character rather than by a per-line marker, and the
  encoder in `src/shell_word.rs` is the same one IR lowering already runs over
  recipe arguments. If that encoder's bound were insufficient, the gap would
  already be reachable from a recipe and not from this filter; the filter does
  not widen it. Section 8.9 assigns it no separate bound and this RFC agrees.

`human_to_bytes` deserves the group's inverse caveat recorded rather than
implied: it computes with **checked** arithmetic and errors on overflow, so the
bound is enforced as a rejection rather than as a truncation. A non-integral
result such as `0.1B` is an error rather than a silent truncation, which is the
same choice clause 6.6 prefers and the reason the two formatters are an inverse
pair only over the values section 8.9 names.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `TextError`, and exactly one
`impl From<TextError> for minijinja::Error`, per clause 6.9. Every message is a
Fluent key and every error carries a machine code. The codes are this group's
contribution to clause 6.9's policy, so they are enumerated rather than
described.

| Condition             | Code                                         |
| --------------------- | -------------------------------------------- |
| not a string          | `netsuke::jinja::text::wrong_kind`           |
| Base64 alphabet       | `netsuke::jinja::text::bad_alphabet`         |
| Base64 character      | `netsuke::jinja::text::invalid_base64`       |
| Base64 padding        | `netsuke::jinja::text::bad_padding`          |
| UTF-8                 | `netsuke::jinja::text::invalid_utf8`         |
| percent escape        | `netsuke::jinja::text::invalid_escape`       |
| namespace parse       | `netsuke::jinja::text::invalid_namespace`    |
| size parse            | `netsuke::jinja::text::invalid_size`         |
| unknown size unit     | `netsuke::jinja::text::unknown_unit`         |
| non-finite number     | `netsuke::jinja::text::not_finite`           |
| precision range       | `netsuke::jinja::text::precision_range`      |
| unknown unit system   | `netsuke::jinja::text::unknown_unit_system`  |
| unknown comment style | `netsuke::jinja::text::unknown_style`        |
| closing marker        | `netsuke::jinja::text::closing_marker`       |
| unknown dialect       | `netsuke::jinja::text::unknown_dialect`      |
| embedded NUL          | `netsuke::jinja::text::embedded_nul`         |
| unknown algorithm     | `netsuke::jinja::text::unknown_algorithm`    |
| digest gated          | `netsuke::jinja::text::digest_feature_gated` |

Each code's Fluent key is the code's reason in upper snake case under
`STDLIB_TEXT_`, so `invalid_base64` pairs with `STDLIB_TEXT_INVALID_BASE64` and
`unknown_unit_system` with `STDLIB_TEXT_UNKNOWN_UNIT_SYSTEM`, per clause 6.9's
`keys::STDLIB_<MODULE>_<CONDITION>` form.

The module segment is `text` rather than `encoding`, for the reason section 5.6
gives: one enum serves a decoder, a parser, a formatter, a quoter, and a
digest, and only two of those are encoding. `text` matches the names the
group's own members carry and the section 8.9 title, so a reader who sees
`STDLIB_TEXT_INVALID_SIZE` can find the ring the key belongs to without
consulting the registry. All nine helpers reach their errors through this enum:
every `Error::new` call in the group's leaf functions is replaced by a variant
of it, so a caller can tell a text failure from a manifest diagnostic by the
code alone.

Three further decisions belong here rather than in the code.

**`shell_quote` keeps its existing key family, and that is the opposite of a
new namespace.** 3.14.8 shipped the filter with `stdlib.shell.*` keys under
`STDLIB_SHELL_*` — `stdlib.shell.args_error` renders
`[netsuke::jinja::shell::args]` — and `src/stdlib/recipe_text/mod.rs` is where
it lives. Clause 6.9 requires a stdlib helper to key under
`stdlib.<module>.<condition>`, which `shell` already satisfies; moving the
filter to `text` to match this RFC's new enum would either rename a shipped key
or leave two namespaces for one helper. This RFC therefore **extends the
existing family rather than forking it**, and the dialect conditions it adds are
`STDLIB_SHELL_DIALECT_NOT_STRING` and `STDLIB_SHELL_DIALECT_INVALID`, which
already exist. The group's new codes are the other sixteen; `shell_quote`
contributes none, because 3.14.8 wrote them.

**The two payload-carrying encoders name the offset.** Section 8.9 requires
`b64decode`'s invalid-UTF-8 error to name the byte offset and `urldecode`'s
invalid or truncated percent escape to name the offset as well. Both codes
render it rather than asserting only that the input was rejected, following the
pattern RFC 0017's section 5.9 established for codes carrying a payload rather
than a scalar. An offset is what makes a rejection actionable in a template
where the subject is a computed value rather than a literal.

**The alphabet, padding, and character conditions are three codes because they
are three repairs.** A Base64 rejection could name the alphabet (`urlsafe`
selected wrongly), an illegal character (the input is not Base64), or the
padding (the input is truncated or over-padded). A single `invalid_base64`
would send every author to the same place, and the three places are different:
the first is the caller's argument, the second is the subject, and the third is
usually a truncated subject. The strictness split is what separates them —
`strict=false` tolerates embedded whitespace only, not a wrong alphabet or
non-canonical padding — and enumerating the three is how the clause's
"enumeration rather than description" is discharged for the group's most
condition-rich helper.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds nine, none an alias. Three carry a
naming decision worth recording, and one of them is not the RFC's to make.

**`text_hash` is a rename, and the rename is the whole point.** RFC 0006
section 7.8 records it as one of three exceptions where a *capability* is
accepted under a new name: Ansible's `hash` row is a reject row whose
disposition cell names `text_hash`, so the capability is in and the name is
Netsuke's. The reason is section 2's trapdoor: Netsuke's existing `hash` treats
its subject as a path, and one name with type-dependent behaviour would let a
manifest bug change from an error into a plausible wrong answer. Neither
`hash_text` nor `checksum` is registered, per roadmap task 6.8.6 and section
10.4. The guide states the distinction at both inventory entries, which is what
makes the pair discoverable rather than merely correct.

**`shell_quote`'s name was settled by 3.14.8, and `bash` is refused.** Section
8.9 originally read "`dialect` currently accepts only `sh`" and was amended on
2026-09-27; the amendment is recorded in the parent and not repeated here
beyond its conclusion. The canonical name and the `dialect` argument were
adopted as the parent proposed; `bash` is refused by the implementation because
`sh` output is valid Bash and the `shell-quote` crate's `Bash` encoder emits a
different form Netsuke does not compile in. This RFC does not reopen either
decision — roadmap task 6.8.3 owns the remaining dialect question, and it is
recorded as a task rather than as an open question here because ADR-041 already
answers the principle.

**`comment`'s `style` values are named for the syntax family, not the
character.** `hash`, `slashes`, `semicolon`, `c_block`, and `xml` name what the
comment is *for*, so an author writing a generated Sass file reaches for
`slashes` and an author writing a generated plist reaches for `xml`. The
rejected alternative is naming them for the marker — `pound`, `double_slash` —
which is unambiguous but requires knowing the target syntax's punctuation
before you can look up the style for it. The `plain` preset Ansible has is not
reproduced: it draws a decorative banner, and section 8.9's non-goal is that a
generated file does not need a border.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. Each of the nine helpers
requires all seven obligations before its roadmap task is complete: a guide
entry, a `tested-example` fence, unit tests covering accepted kinds, rejected
kinds, boundary values, and every enumerated option value, a property test
where an invariant exists, an integration test through a manifest, an
integration test for the manifest-query disposition, and an inventory row.

This group carries more property tests than any child so far, because six of
its nine helpers are inverses or near-inverses and an inverse is the cheapest
falsifiable invariant in the set:

- **Base64 round-trips over both alphabets.** `text | b64encode | b64decode`
  equals `text`, with and without `urlsafe` and with `padding` on and off. The
  four combinations are the cases, and the padding-off case is the one that
  catches a decoder which assumes padding.
- **Percent-decoding round-trips with the encoder Netsuke ships.**
  `text | urlencode | urldecode` equals `text`, using MiniJinja's `urlencode`.
  This is the invariant that *decides* `plus=false`: a `plus=true` default
  would break the round trip against the encoder in the same template engine,
  which is why section 8.9 makes the default an argument rather than a
  preference.
- **The size pair is an inverse over the values it can represent.**
  `n | human_readable(precision=0) | human_to_bytes` equals `n` when `n` is a
  whole multiple of the selected unit. The named exception — values that are
  not — is what keeps the property honest rather than vacuous, and the guide
  states the rounding caveat where the pair is documented.
- **`to_uuid` is deterministic and namespace-sensitive.** The same subject and
  namespace always produce the same UUID, and two different namespaces over one
  subject produce two different UUIDs. The second half is the case that catches
  an implementation which ignores its `namespace` argument and returns the
  fifth version of an empty namespace.
- **`comment` output contains no trailing whitespace.** For every input,
  including one with empty lines and one that is entirely empty, no output line
  ends in a space. Section 8.9 makes this a hard requirement rather than a
  stylistic one, and it is exactly the property a per-line prefixing loop gets
  wrong on the empty-line case.
- **`shell_quote` output re-parses to the input.** For a corpus of values
  containing spaces, quotes, metacharacters, and the empty string, each dialect
  encoder's output parses back to the original. This is the property that makes
  "no second quoting implementation" testable, and it is the one 3.14.8's
  shipped implementation already carries.

The platform suite this group owes is narrower than RFC 0018's but not empty:
`shell_quote`'s host-dependent default is exercised on both platforms, since a
default resolved once at registration is precisely the kind of value that is
correct on the host it was written on. The `powershell` dialect is generated on
Unix and the `sh` dialect on Windows in the same suite, so both encoders are
exercised on both runners without either needing the other's host.

The disposition suite is the group's simplest and is stated for uniformity: all
nine helpers must be reachable from a manifest query, so the test asserts a
registration for all nine rather than a stub for any. The inventory row for
each names the namespace, the registration, and the purity class, so a helper
quietly moved out of the query registration is caught by the inventory
disagreeing rather than by a manifest failing at runtime.

### Clause discharge

| Clause | Discharge                                                                                                                                                               |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Nine `New` pure filters; the group contributes 9 of 52 and the six written children reach 50 of 52, leaving 2 for RFC 0020.                                             |
| `6.2`  | All nine register in `register_query_helpers`; `shell_quote` is already registered there and its registration is unchanged.                                             |
| `6.3`  | Every helper is pure; `human_readable` is locale-independent, `to_uuid` is deterministic, `text_hash` is lowercase, and `shell_quote`'s default is disclosed.           |
| `6.4`  | No capability handle: nothing opens, resolves, or reads; `comment` guards its input text rather than the target file.                                                   |
| `6.5`  | Eight helpers are platform-uniform; `shell_quote` diverges only in its default dialect, with each named dialect's output uniform.                                       |
| `6.6`  | Eighteen conditions with their own codes; the decoders name the offset, the dialect and algorithm conditions enumerate the accepted set.                                |
| `6.7`  | No value is keyed or deduplicated; `text_hash` and `to_uuid` are explicitly not identity relations.                                                                     |
| `6.8`  | Table 3's input row bounds the decoders; `comment` adds one marker per line; the rest are constant- or input-bounded, and `human_to_bytes` uses checked arithmetic.     |
| `6.9`  | One enum, one `From` impl, sixteen new `netsuke::jinja::text::*` codes, with `shell_quote` extending the shipped `STDLIB_SHELL_*` family rather than forking it.        |
| `6.10` | Nine new names, none an alias; `text_hash` is a section 7.8 rename, `shell_quote`'s name is 3.14.8's, and `comment`'s styles name the syntax family.                    |
| `6.11` | Nine guide entries, nine `tested-example` fences, the Base64 and percent round trips, the size inverse, the UUID and comment laws, and the both-platform quoting suite. |

## 6. Dependencies

**Two dependency questions, and both are answered by the parent rather than
here.** Base64 needs an encoder, and the roadmap's task 6.8.1 names adding the
dependency as part of its own scope; the choice belongs to that task rather
than to this RFC, which specifies the behaviour the crate must provide — both
alphabets, configurable padding, and a strictness split — and does not name a
crate. `to_uuid` needs SHA-1, and the same applies: section 8.9 fixes the
version 5 derivation and the frozen namespace, so any SHA-1 provider satisfies
it, and the choice is the task's. Everything else in the group is standard
library or already present: percent-decoding is string work, `human_readable`
and `human_to_bytes` are arithmetic, `comment` is text, and `text_hash` reuses
whatever the existing `hash` filter already uses, including its
`legacy-digests` gate.

Within the RFC set, the group requires the shared contract that RFC 0006
section 14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3
deliver. It requires **no other child RFC**, which makes it the second
independent leaf after RFC 0013: nothing in encoding, identity, or formatting
composes with a path, a collection, or a date, so no sibling's mechanism has to
land first. It is in turn required by nothing — RFC 0020's date filters do not
consume a digest or a quoted word — so the group is genuinely terminal.

The one cross-child relationship worth stating is a **negative** one, because a
reader who expects a dependency will look for it. `shell_quote` and RFC 0017's
`dialect` argument are different arguments on different helpers: RFC 0017's
selects a *path* dialect (`posix` or `windows`) for parsing path text, while
this group's selects a *shell* dialect (`sh` or `powershell`) for quoting a
word. The names collide in English and nowhere else — different accepted sets,
different parsers, different modules — and this RFC deliberately does **not**
extract a shared `dialect` type, because the two enumerations share no member
and a common abstraction over disjoint sets is a name pretending to be a
concept.

## 7. Delivery

Roadmap step 6.8, which implements this RFC in six tasks:

- 6.8.1. `b64encode`, `b64decode`, and `urldecode`, with the Base64 dependency
  and `urldecode` defaulting `plus=false`.
- 6.8.2. `to_uuid` over the frozen Netsuke namespace, with the version 5
  policy recorded at the helper.
- 6.8.3. `shell_quote` over the existing quoting machinery, adopting the
  canonical name 3.14.8 shipped and recording what remains of the dialect set.
- 6.8.4. `comment` with the closing-marker guard.
- 6.8.5. `human_readable` and `human_to_bytes`, with checked arithmetic and
  locale-independent output.
- 6.8.6. `text_hash` without disturbing the existing `hash` contract.

Each task carries the acceptance criteria that make this RFC checkable: both
Base64 alphabets and the URL codec round-trip with invalid input naming the
offset; the default namespace is Netsuke's rather than Ansible's, with its
derivation and literal recorded in the guide; the user guide and the registered
surface agree on `shell_quote` and no second quoting implementation is
introduced; a block style whose input contains the closing marker fails; the
size formatters reject overflow and non-integral results rather than
truncating; and `hash` still hashes the file at its path while the inventory
states the distinction at both entries.

## 8. Open questions

RFC 0006 section 16 assigns **question 6** to this group: "Should `text_hash`
gain a truncating sibling?" The existing `digest` filter is `hash` plus a
length, so `text_digest` is the obvious follow-on, and section 16 records that
it is "not proposed here for want of a use case".

**This RFC carries it unresolved and records the options and their
consequences.** It is the third consecutive child to carry its assigned
question rather than settle it, and the reasons are the pattern's now plus one
of its own: the decision is cheap to make late, and it is genuinely gated on
evidence no RFC can supply.

- **Do not add `text_digest`.** The symmetry with `hash`/`digest` is real but it
  is symmetry, not demand: no manifest has asked for a truncated text digest,
  and every name the standard library carries is a name it must document, test,
  keep available in queries, and support forever. The cost is that an author
  who wants the first sixteen hex characters writes
  `text_hash | truncate(16, end='')`, which is one composition and no new
  surface.
- **Add `text_digest(length)` mirroring `digest`.** The symmetry argument is
  that an author who knows `digest` will reach for `text_digest`, and that the
  pair is more discoverable than a composition. The cost is a second truncation
  helper whose *purpose* is unproven, and section 11.1's whole reason for
  separating the two names is that the hash family already carries a trapdoor;
  adding a member before a use case exists risks repeating the mistake with a
  shorter digest.
- **Add it gated on this group landing first.** Land the nine helpers, then
  revisit with whatever manifests actually reach for. The cost is that the
  question stays open across a release, which is what section 16 already
  accepts by phrasing it as "if `text_hash` proves useful".

The recommendation of this RFC, for the record rather than as a decision, is
the third option in substance and the first in practice: add nothing now, and
re-open the question if a manifest asks. Section 16's own wording is
conditional — "if `text_hash` proves useful" — and a conditional question
cannot be answered before its condition is observable. The second option's
discoverability benefit is real but it is the same benefit `text_hash` itself
already provides over the existing `hash`, and paying for it twice before the
first payment is amortized would be premature.

**It cannot become an ADR**, because
[ADR-040](../adr-040-focused-child-rfcs-for-survey-rfcs.md) requires a child
RFC's section 8 to record its section 16 question without settling it, and the
settlement belongs to the roadmap task that can test it.

A second question sits outside section 16 and is recorded here for the same
reason RFC 0018 recorded its `glob` conflict: it is between the survey and the
shipped code rather than within this RFC.

**`shell_quote` is in the registry as `New` while the code already carries
it.** Section 5.1 explains that the contract derives its expectation from the
document rather than the code, and that `New` is the correct classification of
a renamed capability. What is left for a reader is the question of what roadmap
task 6.8.3 actually implements, and the answer is in the roadmap rather than
here: 3.14.8 delivered the canonical name, the `dialect` argument, and the
single implementation, and what 6.8.3 owes is the wider dialect set beyond `sh`
and `powershell` — which ADR-041's refusal of `bash` currently leaves empty.
The task therefore has a live acceptance criterion ("the user guide and the
registered surface agree") and a scope that may be satisfied by documentation
alone. This RFC does not pre-empt that reading, because whether `bash` should
be reconsidered is an ADR-041 question and not an RFC 0006 one.

## 9. Recommendation

This group should be implemented seventh, at v0.1.x or later, after RFC 0018.

The case for the group is that it is the largest remaining pure contribution
and the one with the least new mechanism. Nine pure filters take the written
children from 41 to 50 of the 52 pure helpers with no capability handle, no
platform divergence in eight of nine, and no cross-child dependency at all; the
one shipped implementation it touches, `src/shell_word.rs`, is reused rather
than duplicated, which is constraint 4's "exactly one implementation" satisfied
by construction rather than by discipline. A sibling that composes with nothing
is a sibling that cannot block or be blocked, and this is the only child in the
set that is both large and fully independent.

The case for the split at the section 8 boundary, rather than at some seam
within section 8.9, is that the section *has* no seam. Its nine helpers divide
into encoders, an identifier, a quoter, a commenter, three size formatters, and
a digest; any finer cut would produce a child with two helpers and a full
overhead, which the ExecPlan's `Surprises` section measured as the move that
*raises* the aggregate rather than relieving it. The three checks that would
have justified a seam — a purity difference, a capability difference, a
dependency difference — are all absent: every member is pure, none takes a
handle, and none composes with a sibling. The group is therefore a child
because section 14.13's partition makes it one, and the partition is right here
for the reason it is right elsewhere: the seam is between capability groups,
not within them.
