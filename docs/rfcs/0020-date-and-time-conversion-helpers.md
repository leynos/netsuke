# RFC 0020: Date and time conversion helpers

## Preamble

- **RFC number:** 0020
- **Status:** Proposed
- **Created:** 2026-10-03
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.9
- **Originating issue:** [#597](https://github.com/leynos/netsuke/issues/597)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies Netsuke's date and time conversion helpers: the two pure
filters `to_datetime` and `strftime`, which share one closed set of
`strftime`-style conversion specifiers. Together they let a manifest parse a
timestamp out of text and render a timestamp back to text, using the timestamp
value `now()` already returns. Section 5.1's registry lists two members, **both
new pure filters**, and both are usable from a manifest query.

The group is RFC 0006 section 8.10 entire, and it is the smallest child in the
set at two helpers. It is also the last: with its two pure helpers the written
children account for **52 of the 52 pure helpers** section 6.1 names, so
whatever else the RFC may be, it is the one whose landing closes the purity
arithmetic rather than advancing it.

What makes the group interesting is not its size but the boundary it draws.
Netsuke already reads the clock, in `now(offset=...)`, and already does
duration arithmetic, in `timedelta(...)`; both exceed Ansible's equivalents.
The missing capability is *conversion*, and the group's whole purpose is to add
it **without adding a second clock read**. Section 3.3 of RFC 0006 records the
seam, [ADR-008](../adr-008-environment-seam-taxonomy.md)'s 2026-09-11 addendum
supplies it, and the two filters here are on the pure side of it: given text
and a format they return a value, and given a value and a format they return
text, and neither ever asks what time it is.

## 2. Problem

Three concrete things a manifest cannot do today, each of which currently sends
an author out of the templating layer.

- **A timestamp that arrives as text cannot be compared.** A manifest reading a
  version or build stamp out of a generated file, or a date out of `from_json`,
  holds a string. It cannot ask whether that stamp is older than a file it also
  knows about, because `now()` and `timedelta()` operate on the timestamp value
  and nothing turns a string into one.
- **A timestamp cannot be rendered in a chosen shape.** The reverse gap. A
  manifest holding a timestamp — from `now()`, or soon from `to_datetime` — has
  no way to print it as `2026-10-03` rather than in whatever form the value
  happens to carry. Generated headers, lock files, and provenance stamps all
  want a chosen shape.
- **Epoch seconds cannot be recovered.** A manifest that obtains an integer
  count of seconds from a command's output, or that wants to *emit* one, has no
  bridge between that integer and the timestamp value. `strftime`'s `%s`
  supplies one direction and its integer input supplies the other.

Two traps sit inside those gaps and are the reason the group carries a table
rather than a free-form format string, which section 4 states and section 5.3
elaborates.

## 3. Goals and non-goals

- Goals:
  - Add `to_datetime` and `strftime` as pure filters over the timestamp value
    `now()` already returns, so the three are interchangeable downstream.
  - Pin the invariant C locale for every name-producing specifier, so identical
    manifests cannot acquire machine-dependent graph text.
  - Keep clock reading in `now` alone, with no second read and no second seam.
  - Reject every specifier outside the accepted set, enumerating the set, so an
    unsupported conversion is a diagnostic rather than a silent wrong string.
  - Round-trip `text | to_datetime(fmt) | strftime(fmt)` for every lossless
    format, which is the group's cheapest falsifiable invariant.
- Non-goals:
  - A `now(...).format(...)` method, refused by section 6.10's one-spelling
    rule.
  - Ansible's `strftime(format, second, utc)` argument order, in which the
    format is the subject; section 8.10 records the reshape.
  - IANA time-zone names and a time-zone database, refused in section 8.10 and
    again here for the reproducibility reason it gives.
  - `%Z` and every other zone-naming, locale-varying, or week-numbering
    specifier, excluded by table 12.
  - An injected clock seam. Section 16's **question 7** records that this
    question is already answered, by ADR-008's addendum and roadmap item 7.1.1,
    and that this group neither needs the seam nor depends on it. Section 8
    records why that is a discharge rather than a deferral.

## 4. Capability set

Two new pure filters sharing one closed conversion-specifier set.

- `text | to_datetime(format='%Y-%m-%d %H:%M:%S', timezone='UTC')` — parse
  text into the timestamp value `now()` returns.
- `timestamp | strftime(format)` — render a timestamp, or an integer count of
  Unix epoch seconds, to text.

The shared set is RFC 0006 **table 12**: `%Y`, `%y`, `%m`, `%d`, `%H`, `%I`,
`%M`, `%S`, `%f`, `%j`, `%z`, `%s`, `%a`, `%A`, `%b`, `%B`, `%p`, and `%%`.
This section does not restate the table or either helper's contract. Each
helper's argument shape, options, rejection conditions, and edge cases are
specified in
[RFC 0006 section 8.10](0006-ansible-inspired-template-standard-library.md#810-date-and-time-conversion),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

One thing the table's shape *is* worth stating here, because it drives section
5.9 rather than belonging there: the set is closed and small, and the two
filters share it rather than each defining its own. A single parser and a
single formatter over one vocabulary is what makes the round-trip property
meaningful — a format the parser accepts and the formatter rejects would be a
format table 12 has not actually specified.

## 5. Cross-cutting contract conformance

### 5.1. Registry

| Helper        | Namespace | Registration | Purity class | Manifest query |
| ------------- | --------- | ------------ | ------------ | -------------- |
| `to_datetime` | Filter    | New          | Pure         | Yes            |
| `strftime`    | Filter    | New          | Pure         | Yes            |

Two `New` rows and no `Option added` row, both in the Filter namespace. The two
are the whole of RFC 0006 section 8.10, which the coverage map gives this row
and no other; the roadmap's step 6.9 owns both, and no other child contributes
to this group.

**Both are Filter because both are accepted in the 7.1 core-filters table.**
The namespace column is not a judgement here: `to_datetime` and `strftime` are
section 7.1 rows accepted in place, neither renamed and neither optioned, so
the namespace the contract derives for them is the table's and there is nothing
to reconcile. That makes this child the only one in the set with no rename, no
option, and no namespace question, which is a consequence of its size rather
than of a decision.

**`strftime` is accepted with a reshaped argument order, and the reshape is
already in the survey.** RFC 0006 section 7.1's row records it as `Accept` with
the resolution "`§8.10, reshaped`"; Ansible's shape takes the format as the
subject and the timestamp as an argument, and section 8.10 reverses that so the
timestamp pipes. The reshape does not make `strftime` a rename — the registered
name is the surveyed name — so `RENAMES` in the contract's inventory does not
carry it, and the registry records `New` for the ordinary reason.

The two helpers take the written children to **52 of the 52 pure helpers**
section 6.1 names. RFC 0013 contributes 5, RFC 0014 6, RFC 0015 15, RFC 0016 8,
RFC 0017 7, RFC 0018 0, RFC 0019 9, and this RFC 2, and the eight sum to
**52**. With all eight children written, the coverage contract's purity classes
are compared for exact equality rather than bounded above, and this group is
what makes the comparison close. The two non-pure classes are unaffected by the
group: its two helpers are pure, so the four filesystem-observing and one
environment-observing helpers remain RFC 0018's six rows minus `glob`, which is
`Option added` and already counted in the survey.

### 5.2. Manifest-query availability

No additional obligation beyond RFC 0006 section 6.2. Both helpers are pure, so
clause 1 registers them in `register_query_helpers` and neither is stubbed; the
always-failing-stub path of clause 2 is not reached by this group. This is the
second consecutive child with that uniform disposition, and the reason is the
same: a pure helper is available in a manifest query by definition.

**The clause's contrast with `now` is the group's most instructive fact and is
worth stating explicitly.** `now` is clock-observing, so manifest-query
registration receives no clock and keeps its refusing `now` stub — ADR-008's
addendum records that in the same sentence that supplies the seam. This group
is the other half of that arrangement: it converts between text and the
timestamp value and therefore needs no clock at all, which is why the two
filters register in `register_query_helpers` while `now` registers a stub. A
reader who sees `now` stubbed and `to_datetime` registered might read an
inconsistency; the difference is exactly the purity class, and section 5.3
states why conversion does not reintroduce the observation `now` performs.

### 5.3. Determinism

No additional obligation beyond RFC 0006 section 6.3. Neither helper reads a
clock, the environment, the filesystem, or the network: `to_datetime` is a
function of its subject, its format, and its `timezone` argument, and
`strftime` is a function of its subject and its format. There is no sequence
whose order could vary and no iteration whose order could leak into a generated
graph.

Two determinism obligations are sharper than "same input, same output", and
both are the reason section 4 says the specifier set is closed.

- **The five name-producing specifiers are locale-pinned, not
  locale-detected.** `%a`, `%A`, `%b`, `%B`, and `%p` always render the
  invariant C locale's English forms. This is the clause's sharpest instance in
  the whole RFC set, because it is the one place where the *platform's* own
  formatter would silently do the wrong thing: a `strftime` that delegated to
  the C library's `strftime` would produce `Okt` on a German host and `Oct` on
  an English one, so one manifest would generate two different files. Section
  8.10 makes the pin a requirement in table 12's own caption, and RFC 0006's
  section 14.14 summary row names the trap — "`strftime` output is
  locale-sensitive" against the resolution "the invariant C locale is pinned
  and locale-varying specifiers are rejected" — so a future implementer meets
  the constraint before the code rather than after a bug report.
- **The conversion specifier set is closed, which is what makes the pin
  enforceable.** A free-form format string is one the implementation must
  delegate to the platform, and delegation is what reintroduces locale
  variation. Rejecting every specifier outside table 12 — `%c`, `%x`, `%X`,
  `%U`, `%W`, `%G`, and `%Z` among them — is therefore a determinism measure
  rather than a strictness preference, and `%U`, `%W`, and `%G` are excluded
  for the adjacent reason that week numbering depends on a week-start
  convention no manifest should have to know.

The `timezone` argument adds no non-determinism, and the reason is worth
stating because it looks like an ambient input. It accepts `UTC` and fixed
offsets in `+HH:MM` form — a *literal* the caller supplies, not a lookup — and
it is consulted only when the format carries no `%z`. So the output is a
function of the arguments in every case, and a host's configured zone cannot
reach it. Rejecting IANA zone names is what keeps that true: supporting them
would mean consulting a time-zone database whose contents vary by platform and
release, which section 8.10 records as a separate decision with its own
reproducibility consequences.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. Neither helper takes a
capability handle: neither opens a file, resolves a path, or reads a directory,
so there is no path outside a capability to reject and no `cap_std` handle to
thread. The group is pure over values, exactly as RFC 0019 is, and for the same
structural reason — its inputs are values the template already holds.

The clause's one near miss is worth recording because the temptation is
specific. `to_datetime` parses a timestamp *out of text*, and the natural next
thought is that a manifest would rather read one out of a *file* — a stamp a
previous step wrote. That would turn the helper into a file-reading filter and
remove it from every manifest query, which is precisely the trade RFC 0018's
`expandvars` had to make and this group does not. The resolution is the same
shape as RFC 0019's `comment`: the helper operates on text, and the reading is
a separate capability the manifest already has. Section 8.10 says nothing about
a file for this reason, and the guide should state the composition —
`read_text` or `from_json` when a file is involved — where `to_datetime` is
documented.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. Both helpers are
platform-uniform: `time::OffsetDateTime` is the same value on every host, table
12's specifiers are defined over that value's fields, and the name-producing
specifiers are pinned to the invariant C locale. There is no `st_dev` to
compare, no volume root to read, and no offset to infer from the host.

That last point is the one a reader might expect to be platform-dependent and
is worth stating. A naive timestamp formatter might render `%z` from the host's
current offset, which differs between a machine in London and one in Tokyo, and
a naive parser might interpret an offset-less input in the host's local zone.
Neither happens here: `%z` renders the *value's* stored offset, which
`to_datetime` set from `timezone` or from the input's own `%z`, so the rendered
text depends on the value rather than on the reader. The group therefore has no
platform divergence at all, which makes it the second child — after RFC 0019 —
with a uniform platform contract and no both-platform suite to owe. The suite
that *would* have been owed, a locale-varying one, is instead a
locale-*independence* test: the same manifest must render identical text with
`LC_ALL` set to two different values, which is the clause's property expressed
as a test rather than as a platform pair.

### 5.6. Type and error contract

The group defines one private domain error enum, `TimeError`, and exactly one
`impl From<TimeError> for minijinja::Error`, per clause 6.9. Two helpers
sharing one specifier vocabulary is the case the clause's enumeration rule is
written for: a format error raised by `to_datetime` and the same format error
raised by `strftime` must be the *same* diagnostic, or an author who fixes a
format on one call site and not the other sees two different messages for one
mistake.

The conditions divide into four families.

- **Subject conditions.** `to_datetime` requires a string and `strftime`
  requires a timestamp value or an integer; a float is rejected with the reason
  section 8.10 gives, and any other kind is a wrong-kind error.
- **Format conditions.** A specifier outside table 12, and an unterminated
  trailing `%`. These are the shared family, and both helpers reach them
  through the same parser.
- **Parse conditions.** `to_datetime` reads its subject against the format: an
  input that does not match, and a value that matches the shape but is not a
  real date-time.
- **Argument conditions.** `strftime`'s missing or wrong-kind format, and
  `to_datetime`'s malformed `timezone`, plus the range conditions a
  fixed-offset value can violate.

Two decisions in the enum are substantive and are recorded here rather than in
the code.

**The enum is `TimeError` and the codes extend the existing `stdlib.time.*`
family.** `src/stdlib/time/mod.rs` already registers `now` and `timedelta` and
already keys `STDLIB_TIME_OFFSET_INVALID`, `STDLIB_TIME_OVERFLOW`, and the eight
`STDLIB_TIME_LABEL_*` keys under `stdlib.time.*`. So the group's codes extend
a family rather than founding one, which is the same disposition RFC 0019
reached for `shell_quote` and for the same reason: clause 6.9 requires a stdlib
helper to key under `stdlib.<module>.<condition>`, the module segment is
`time`, and the existing keys already satisfy it. The module also already owns
the timestamp value — `TimestampValue` lives in `src/stdlib/time/format.rs` —
so the two filters are added to the module that defines their subject rather
than to a new one.

**Parse failures name the position and the specifier, which is two payloads
rather than one.** Section 8.10 requires `to_datetime`'s parse error to name
"the offending position in the input and the specifier that failed", and the
reason is the one RFC 0019's section 5.9 gives for its offset: a template's
subject is usually a computed value rather than a literal, so "the input did
not match the format" is a message an author cannot act on. `2026-13-01` fails
at byte 5 for `%m`; naming both turns a rejection into a repair. This is the
richest single code in the RFC set and it carries two values, of which one is
often a position and both are often short — which is why it is one code with
two payloads rather than two codes.

The float rejection deserves its own sentence because it is a decision rather
than a kind check. Section 8.10 rejects float epoch seconds because "sub-second
epoch values raise a rounding question the filter should not answer silently",
and the enum implements that as a *named* condition rather than as a generic
wrong-kind error: a manifest author who passes `1696377600.5` should be told to
convert to an integer, not told that a float is not an integer. The condition
is therefore `float_epoch` with that guidance in its Fluent message, and the
guide repeats it where `strftime`'s integer input is documented.

### 5.7. Canonical value equality

No additional obligation beyond RFC 0006 section 6.7. Neither helper keys a
value, deduplicates a sequence, or performs set membership over a canonical
form. Both consume and produce scalars — a timestamp value and a string — and
no identity relation is consumed or produced.

The one thing worth stating is that the timestamp value is **not** an identity
relation either, though it is the group's most identity-looking type. Two
timestamps that render identically are not thereby the same instant, because
the format may have discarded the offset or the fractional second; and two
timestamps that are the same instant may render differently under two formats.
The value is a value, and comparing two of them is `now`'s and `timedelta`'s
existing business rather than a canonical key. A manifest that wants a stable
identity from a timestamp should render it with a lossless format and key on
the resulting text, which is a composition rather than a property of these two
filters.

### 5.8. Resource bounds

No additional obligation beyond RFC 0006 section 6.8, and the reasoning is
short for both members because neither materializes more than its input.

`to_datetime` is bounded by its input: it consumes a subject of `n` bytes and
produces one timestamp value, which is fixed-width. There is no expansion, no
traversal, and no second buffer.

`strftime` is bounded by its format rather than its subject, which is the
distinction table 3's output row is written for: the subject is a fixed-width
timestamp and the output is proportional to the format string, whose name-
producing specifiers emit at most nine characters and whose `%%` emits one. So
the bound is linear in the format with a small constant, and a manifest cannot
amplify a small value into a large one. The format is author-supplied text
rather than a computed value in every realistic case, and even a computed one
is bounded by table 3's input row for the string that carries it.

The clause's one caveat is arithmetic rather than textual, and it belongs here
because no other clause claims it. `%s` over an out-of-range timestamp and the
`timezone` offset arithmetic can both overflow `OffsetDateTime`'s range, and
section 8.10's `timedelta` already keys `STDLIB_TIME_OVERFLOW` for exactly that
class of failure. The group reuses that key rather than adding a second
spelling, so an overflow is an error naming the same condition `timedelta`
names, and it is a rejection rather than a wrapped value. Checked arithmetic is
the rule, which is the same rule RFC 0019's `human_to_bytes` follows for the
same reason.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `TimeError`, and exactly one
`impl From<TimeError> for minijinja::Error`, per clause 6.9. Every message is a
Fluent key and every error carries a machine code. The codes are this group's
contribution to clause 6.9's policy, and all of them extend the existing
`stdlib.time.*` family.

| Condition             | Code                                        |
| --------------------- | ------------------------------------------- |
| wrong kind            | `netsuke::jinja::time::wrong_kind`          |
| format not a string   | `netsuke::jinja::time::format_not_string`   |
| unsupported specifier | `netsuke::jinja::time::unknown_specifier`   |
| trailing `%`          | `netsuke::jinja::time::trailing_percent`    |
| input does not match  | `netsuke::jinja::time::parse_failed`        |
| value out of range    | `netsuke::jinja::time::invalid_value`       |
| timezone not a string | `netsuke::jinja::time::timezone_not_string` |
| timezone malformed    | `netsuke::jinja::time::timezone_invalid`    |
| IANA zone name        | `netsuke::jinja::time::timezone_named`      |
| float epoch seconds   | `netsuke::jinja::time::float_epoch`         |
| arithmetic overflow   | `netsuke::jinja::time::overflow`            |

Each code's Fluent key is the code's reason in upper snake case under
`STDLIB_TIME_`, so `unknown_specifier` pairs with
`STDLIB_TIME_UNKNOWN_SPECIFIER` and `float_epoch` with
`STDLIB_TIME_FLOAT_EPOCH`, per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>`
form. Two of the eleven are already written: `overflow` is the existing
`STDLIB_TIME_OVERFLOW` that `timedelta` keys, reused rather than duplicated;
the other ten are new.

Three further decisions belong here rather than in the code.

**`unknown_specifier` enumerates the supported set rather than describing it.**
Section 8.10 requires any specifier outside table 12 to be "an error
enumerating the supported set", and the clause's enumeration rule is what makes
that possible: the message renders all eighteen specifiers, so an author who
wrote `%C` learns which eighteen are accepted without opening the guide. This
is the group's longest message and the one whose length is deliberate — a
partial enumeration would send an author to the documentation anyway, which is
the outcome the requirement exists to prevent.

**`timezone_named` is separate from `timezone_invalid` because the repairs
differ.** A malformed offset such as `+25:00` or `Europe/London`'s missing
digits is a syntax error; an IANA zone name such as `Europe/London` is
*well-formed* and is refused for a policy reason — no time-zone database is
shipped. Folding the two together would tell an author that `Europe/London` is
malformed, which is false, and would hide the reason the helper cannot do what
they asked. The split is the same shape RFC 0019 uses to separate its three
Base64 conditions, and for the same reason: three repairs, three codes.

**`parse_failed` carries the position and the specifier, and the guide states
the composition that avoids it.** The code renders both payloads per section
5.6. The guide's job is adjacent and is recorded here so the obligation is not
lost: where `to_datetime` is documented, the entry should show how a manifest
*obtains* the text it parses, since the common failure is a subject that is not
in the expected shape at all rather than one that is subtly malformed.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds two, neither an alias and neither
renamed. Both registered names are the surveyed names, which makes this child
the only one in the set with nothing to reconcile against `RENAMES`.

**`strftime` keeps Ansible's name and reshapes its arguments, and the
distinction matters.** Section 7.1's row is `Accept` with the resolution
"`§8.10, reshaped`", so the name is surveyed and the *signature* is Netsuke's:
the timestamp is the subject and the format is the argument, so
`published | strftime('%Y-%m-%d')` reads as a pipeline. Renaming the filter —
`format_time`, say — would have made the reshape visible, but at the cost of a
name no Ansible author recognizes and a third rename exception the section 7.8
prose does not state. The survey chose recognition over tidiness and this RFC
implements that.

**A `now(...).format(...)` method is refused, and the refusal is section 6.10's
one-spelling rule rather than a preference.** RFC 0006 section 3.3 and section
8.10 both record it: `now()` returns a value and `strftime` renders one, and a
method on the value would be a second spelling of the same capability. The
practical consequence for this RFC is that `strftime` must accept everything
`now()` returns — the timestamp value itself, not a wrapper — so the two are
interchangeable and the pipeline is the only spelling.

**`to_datetime`'s name is Ansible's and its default format is too.** The default
`'%Y-%m-%d %H:%M:%S'` is the surveyed default, kept because it is the shape
`now()`'s own rendering uses and because a default an author can predict from
Ansible's documentation is worth more than a Netsuke-specific one. The
registered signature differs from the surveyed one in `timezone` only, which
section 8.10 adds and Ansible's signature does not carry.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. Each of the two helpers
requires all seven obligations before its roadmap task is complete: a guide
entry, a `tested-example` fence, unit tests covering accepted kinds, rejected
kinds, boundary values, and every enumerated option value, a property test
where an invariant exists, an integration test through a manifest, an
integration test for the manifest-query disposition, and an inventory row.

The group's unit tests are dominated by the specifier table, and that is the
right shape rather than a burden: eighteen accepted specifiers and the rejected
ones are a closed enumerated set, so each deserves its own case and the count
is the specification's. Four property tests carry the invariants that matter.

- **The round trip over the lossless subset.** For a format that discards
  nothing, `text | to_datetime(fmt) | strftime(fmt)` equals `text`. Section
  8.10 names this as the group's property, and "lossless" is what keeps it
  honest: `%y` discards the century and `%p` discards the hour, so the property
  holds over the formats that preserve their inputs and the guide states the
  exclusions where the pair is documented.
- **Locale independence is the determinism property, expressed as a test.** The
  same manifest must render identical text under two different `LC_ALL` values,
  which is the direct encoding of clause 6.3's requirement and the test that
  would have caught a delegated-to-the-platform implementation. Section 5.5
  records why this replaces the both-platform suite the group would otherwise
  owe.
- **Every rejected specifier enumerates the accepted set.** The property is on
  the *message* rather than the result: for each of a corpus of unsupported
  specifiers, the error names all eighteen accepted ones. A test that only
  asserted the rejection would pass against an implementation that described
  the set instead of enumerating it, which is the failure section 8.10 names.
- **The `timezone` precedence rule holds in both directions.** When the format
  carries `%z`, that offset wins and `timezone` is ignored; when it does not,
  `timezone` supplies the offset. Supplying both is not an error, and the
  second half is the case that catches an implementation which treats the two
  as mutually exclusive.

The disposition suite is the group's simplest and is stated for uniformity:
both helpers must be reachable from a manifest query, so the test asserts a
registration for both rather than a stub for either. The suite's contrast with
`now` — registered as a refusing stub because it observes the clock — is the
assertion that makes the group's purity load-bearing rather than incidental,
and it is worth an explicit case so a future change that routed these filters
through the clock would fail rather than silently pass.

### Clause discharge

| Clause | Discharge                                                                                                                                                           |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Two `New` pure filters; the group contributes 2 of 52 and its landing takes the eight written children to 52 of 52.                                                 |
| `6.2`  | Both register in `register_query_helpers`; neither observes the clock, which is why only `now` keeps its refusing stub.                                             |
| `6.3`  | Both are pure; the five name-producing specifiers are pinned to the invariant C locale and the specifier set is closed so the pin is enforceable.                   |
| `6.4`  | No capability handle: neither opens, resolves, or reads; a file-borne timestamp is read by the manifest and passed in as text.                                      |
| `6.5`  | Both are platform-uniform; `%z` renders the value's stored offset rather than the host's, and locale independence is tested instead of a platform pair.             |
| `6.6`  | Eleven conditions with their own codes; the parse error names the position and the specifier, and the unsupported-specifier error enumerates the accepted set.      |
| `6.7`  | No value is keyed or deduplicated; the timestamp value is explicitly not an identity relation.                                                                      |
| `6.8`  | Table 3's input row bounds `to_datetime`; `strftime` is bounded by its format; both use checked arithmetic and reuse the existing overflow key.                     |
| `6.9`  | One enum, one `From` impl, ten new `netsuke::jinja::time::*` codes extending the existing `stdlib.time.*` family, with `overflow` reused from `timedelta`.          |
| `6.10` | Two new names, neither an alias and neither renamed; `strftime`'s argument order is reshaped while its name is kept, and no `now(...).format(...)` method exists.   |
| `6.11` | Two guide entries, two `tested-example` fences, the lossless round trip, the locale-independence property, the enumeration property, and the both-registered suite. |

## 6. Dependencies

**No new dependency, and that is the group's least obvious virtue.** The
timestamp value is `time::OffsetDateTime` and the `time` crate is already a
dependency — `src/stdlib/time/mod.rs` imports it, and `TimestampValue` in
`src/stdlib/time/format.rs` already wraps it. So the group needs no date-time
library, no time-zone database, and no locale data: table 12's specifiers are
implemented over fields `OffsetDateTime` already exposes, and the invariant C
locale is the one whose forms Netsuke writes as constants rather than looks up.
`strftime`'s format-description syntax is Netsuke's own rather than the `time`
crate's, which section 8.10 records and which is why the `time` crate's
descriptor syntax is not a public contract. The one thing the group must not do
is adopt the C library's `strftime`, and section 5.3 records why: delegation is
what reintroduces locale variation.

Within the RFC set, the group requires the shared contract that RFC 0006
section 14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3
deliver. It requires **no other child RFC**, making it the third independent
leaf after RFC 0013 and RFC 0019: nothing in date and time conversion composes
with a path, a collection, an encoding, or a digest, so no sibling's mechanism
has to land first. It is in turn required by nothing, so the group is terminal,
and with it the whole child set is terminal — every one of the eight is a leaf
or a near-leaf, which is a property of ADR-040's partition rather than of this
RFC.

The one dependency worth stating is a **negative** one, and RFC 0006 section 16
states it in question 7: "RFC 0020 — which owns `to_datetime` and `strftime` —
neither needs it nor depends on 7.1.1." The clock seam and this group are
independent, and the reasoning is section 5.2's contrast. `now` observes the
clock and needs the injected `ClockProvider`; these two filters convert between
text and a value the caller already holds, so they need no clock at all and
would be unaffected if the seam were removed. A reader who assumes a shared
module implies a shared seam will look for that dependency; there is none.

## 7. Delivery

Roadmap step 6.9, which implements this RFC in two tasks:

- 6.9.1. The shared conversion-specifier set and `strftime`, pinning the
  invariant C locale for the name-producing specifiers, rejecting every
  specifier outside the set with the supported set enumerated, and accepting the
  `now()` timestamp value and integer epoch seconds while rejecting floats.
- 6.9.2. `to_datetime`, accepting `UTC` and fixed offsets for `timezone` and
  rejecting IANA zone names so no time-zone database is required.

The acceptance criteria make the RFC checkable. Identical manifests must
produce identical text on machines with different locales, which is clause
6.3's requirement in testable form. The round-trip property must hold for every
lossless format, which is section 5.11's first property. And `strftime` must
accept exactly what `now()` returns, so the two are interchangeable and the
pipeline is the only spelling.

The second task's dependency on the first is real rather than procedural: the
round-trip property cannot be tested until both ends exist, and the shared
specifier set is the thing both ends are defined over. So 6.9.1 lands a
formatter that can only render the value `now()` produces, and 6.9.2 completes
the pair and enables the property.

## 8. Open questions

RFC 0006 section 16 assigns **question 7** to this group: "Does `now` need an
injected clock seam?" Unlike the questions assigned to RFCs 0017 through 0019 —
each of which this set carries unresolved — this one is **already resolved**,
and section 16 says so in its own text: "Resolved. Section 3.3 records the gap,
and nothing in this RFC required the seam, because `to_datetime` and `strftime`
are pure. Roadmap item 7.1.1 supplied it: `now()` reads through a
`ClockProvider` held by `StdlibConfig`, classified in the ADR-008 addendum for
2026-09-11. The seam is answered there rather than here, and RFC 0020 — which
owns `to_datetime` and `strftime` — neither needs it nor depends on 7.1.1."

**This RFC discharges the question rather than carrying it, and recording the
difference is the point of this section.**
[ADR-040](../adr-040-focused-child-rfcs-for-survey-rfcs.md) requires a child's
section 8 to record its assigned section 16 question; it does not require the
question to be open. Where the survey has already settled one and named the
artefact that settles it, the child's obligation is to say so and to point at
that artefact, which is what this section and section 6 do. Three things follow.

- **The resolution is not this RFC's to restate.** ADR-008's addendum is the
  decision of record — it names the shape (`ClockProvider`, an
  `Arc<dyn Fn() -> OffsetDateTime + Send + Sync>` held by `StdlibConfig`), the
  production supplier (`system_clock()`, the sole caller of
  `OffsetDateTime::now_utc` for `now()`), the manifest-query arrangement (no
  clock, refusing stub), and the alternatives rejected (`mockable::Clock`,
  typed in `chrono`; `monotony`, which abstracts only monotonic elapsed time).
  A child RFC that paraphrased all of that would create a second record that
  can drift.
- **The question's answer explains this group's purity, which is why it is
  recorded here at all.** The reason `to_datetime` and `strftime` need no seam
  is that conversion is a pure function of values, and the reason `now` needs
  one is that reading the clock is not. That is the same distinction section
  5.2 draws between the registered filters and the stubbed `now`, and section
  5.3 draws between a deterministic function and a clock read. So the question
  is answered *by* the group's shape rather than adjacent to it.
- **The single-part ADR-008 addendum is the pattern a future clock-adjacent
  decision should follow, but that is a process observation rather than an open
  question.** It is noted for the record and needs no amendment here.

**It cannot become an ADR**, because it already has one: ADR-008's addendum is
the ADR, and the clause here forbids a child RFC from settling a section 16
question that the survey has already answered elsewhere. What this RFC
contributes is the confirmation that the resolution covers this group — that
neither filter observes the clock, needs the seam, or depends on roadmap item
7.1.1 — which is a claim about these two helpers and could not have been made
before they were specified.

**Two readings of the group's scope are recorded and left open, both belonging
to roadmap task 6.9.1 rather than here.** First, whether `strftime`'s accepted
input should extend beyond the timestamp value and integer epoch seconds to
`now()`'s offset-carrying variants; section 8.10 admits the value and the
integer, and the task can test whether a manifest has a use for anything else.
Second, whether the specifier set should ever grow — `%Z` and the
week-numbering specifiers are excluded by table 12 for reasons section 5.3
records, but a future manifest might need one, and the table's amendment is a
survey question rather than a child RFC's. Neither reading changes the group's
contract, and both are recorded so a future reader knows the questions were
seen rather than missed.

## 9. Recommendation

This group should be implemented last, at v0.1.x or later, after RFC 0019.

The case for the group is that it closes the survey. Two pure helpers over an
existing value, with no new dependency, no capability handle, no platform
divergence, no cross-child dependency, and no seam to inject: its landing takes
the written children to 52 of the 52 pure helpers and the coverage contract
from a bounded comparison to an exact one. A group that finishes an arithmetic
is worth landing for that reason alone, and this one finishes it while adding a
capability rather than a caveat.

The case for the group being *last* rather than earlier is that it is the one
that can be. It depends on nothing, so nothing is blocked behind it, and it is
small enough that its two tasks fit in a single review pass. Ordering it before
RFC 0019 would have left the larger group's nine helpers and their shipped-code
reconciliation to be squeezed into whatever time remained; ordering it after
means the child with the most to reconcile landed while it could still be
reconciled carefully. That is the set's ordering principle throughout: the
group with the least external coupling goes last, so the coupling-heavy work
gets the attention.

The case for the section 8.10 boundary itself needs the least defence. Table 12
is one table, two helpers share it, and there is no seam inside the group: the
formatter and the parser are defined over the same vocabulary, and the
round-trip property spans both. A child at this size is a child the partition
produced rather than one a designer chose, and section 14.13's partition is
right here for the reason it is right elsewhere — the seam is between
capability groups, and "convert between text and a timestamp" is one group,
however few members it has. With this group written, every capability RFC 0006
proposed has a child RFC, and the survey's expansion is fully specified.
