# RFC 0016: Pattern and version predicates

## Preamble

- **RFC number:** 0016
- **Status:** Proposed
- **Created:** 2026-10-01
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.5
- **Originating issue:** [#596](https://github.com/leynos/netsuke/issues/596)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies the regular-expression family and the version predicate: the
filters `regex_replace`, `regex_search`, `regex_findall`, and `regex_escape`,
and the tests `match`, `search`, `regex`, and `version`. Together they let a
manifest inspect a `--version` string, select a compiler flag from a version
comparison, and filter a path list by pattern, without `shell()` and without a
subprocess whose output format the manifest does not control. All eight are
pure, so all eight are available to manifest queries as well as to recipes, and
none needs a capability handle. The group is RFC 0006 section 14.5's slice 4,
and section 14.11 puts "the regular-expression family and `version`" in the
recommended first wave. The version predicate has no dependency on the
regular-expression work, as section 14.5 itself notes, but the two share a
roadmap step, a purity assignment, and a diagnostic shape, so one document owns
the whole of step 6.5.

## 2. Problem

A manifest that needs to know a tool's version today either runs it and parses
the output with `shell()` and `cut`, or hard-codes the answer and drifts. RFC
0006 section 2 records the cost: a pure, cacheable, capability-free planning
expression becomes a subprocess with ambient authority, and the manifest's
behaviour now depends on a program's output format rather than on its own
inputs. The pattern half is the same contortion one step narrower — filtering a
source list, testing a filename, or extracting a version component becomes
another process rather than one expression. The version half is worse in one
respect: a `version` comparison written as string equality is *silently* wrong
for every pair such as `1.10.0` and `1.9.0`, so the manifest does not merely
lose elegance, it reports the wrong branch.

RFC 0006 section 8.4 also records the constraint that shapes this group rather
than merely describing it: Rust has no Python `re` implementation, so a surface
that looked like Ansible's would fail unpredictably on patterns an author had
every reason to expect to work. The dialect is therefore named and its
unsupported constructs are refused explicitly, which is a decision about error
behaviour rather than about syntax.

## 3. Goals and non-goals

- Goals:
  - Express a version condition — `>= 1.82.0`, `< 2.0.0` — strictly, so the
    branch a manifest takes matches the branch a human would read.
  - Inspect a `--version` string, a filename, or a path list with a bounded,
    documented pattern dialect whose failure modes are typed rather than
    incidental.
  - Refuse, loudly, the two migrations that would otherwise succeed silently: a
    Python-style `\1` replacement emitting literal text, and a pattern using
    look-around that a linear-time engine cannot honour.
  - Keep every helper pure, so all eight are usable from manifest queries.
- Non-goals:
  - `version_compare`, rejected in RFC 0006 section 7.6 as an alias of
    `version`, and `version_type`, replaced by `scheme` per section 8.5.
  - Ansible's `strict` argument: Netsuke's only behaviour is strict parsing.
  - PEP 440, Debian, and RPM version schemes, which become new `scheme` values
    only when a consumer requires one.
  - POSIX basic regular expressions, which `regex_escape` does not offer
    because nothing in Netsuke consumes them.
  - The look-around, back-reference, recursion, atomic-group, possessive
    quantifier, and `\G` constructs section 8.4 lists as unsupported. These are
    consequences of linear-time matching, not gaps to be filled later.
  - `regex_replace`'s Python-style replacement forms, which are rejected rather
    than translated.

## 4. Capability set

Eight helpers: four filters and four tests, all pure, all newly registered.

- `regex_replace` — replace matches, with `$1`-form replacements and an
  optional exact-count requirement.
- `regex_search` — the matched text or a selected group, or `none`.
- `regex_findall` — every whole match or one group per match, as a sequence of
  strings.
- `regex_escape` — a pattern matching the subject literally.
- `match` — true when the pattern matches at the start of the subject.
- `search` — true when the pattern matches anywhere in the subject.
- `regex` — explicit `search` / `match` / `fullmatch` selection.
- `version` — a strict Semantic Versioning comparison under a required
  operator.

This section does not restate any contract. Each helper's argument shape,
options, rejection conditions, and bounds are specified in
[RFC 0006 section 8.4](0006-ansible-inspired-template-standard-library.md#84-pattern-matching)
and
[section 8.5](0006-ansible-inspired-template-standard-library.md#85-version-predicates),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

### 5.1. Registry

| Helper          | Namespace | Registration | Purity class | Manifest query |
| --------------- | --------- | ------------ | ------------ | -------------- |
| `regex_replace` | Filter    | New          | Pure         | Yes            |
| `regex_search`  | Filter    | New          | Pure         | Yes            |
| `regex_findall` | Filter    | New          | Pure         | Yes            |
| `regex_escape`  | Filter    | New          | Pure         | Yes            |
| `match`         | Test      | New          | Pure         | Yes            |
| `search`        | Test      | New          | Pure         | Yes            |
| `regex`         | Test      | New          | Pure         | Yes            |
| `version`       | Test      | New          | Pure         | Yes            |

Eight rows, taking the child registries to 34 of the 52 pure helpers. RFC 0013
contributes 5, RFC 0014 contributes 6, and RFC 0015 contributes 15, so the four
written children account for 34 and the four still to write account for 18.

### 5.2. Manifest-query availability

All eight are pure, so all eight register in the manifest-query environment and
none is stubbed. The clause's interesting case in this group is `version`: a
manifest query that reads a tool's version has to get that version from the
manifest rather than from the tool, because a query may not run a subprocess.
Nothing here makes that possible or prevents it — `version` compares two
strings it is given, and where the first came from is the manifest's business.
Section 8.5 says the same thing from the other direction when it names
`regex_search` as the sanctioned tool for normalizing `--version` output: the
extraction and the comparison are separate steps, and both are pure.

### 5.3. Determinism

No helper in this group exposes an iteration order, and none depends on one.
`regex_findall` returns matches in input order. The clause's force here is
directed at *pattern* determinism rather than collection ordering, and it has
two parts this group decides.

- **The compiled-pattern cache is an optimizing cache, never a semantic one.**
  Table 3's cache is 64 entries, least-recently-used, so two identical calls
  can compile the pattern once or twice depending on what else ran between
  them. That must not be observable: a cache miss recompiles the same pattern
  with the same flags to the same automaton, so the match set is identical
  whether the entry was resident or not. A property test over a pattern
  sequence longer than the cache is what makes that checkable, and roadmap task
  6.5.1 carries it.
- **The leftmost-first guarantee is what makes the result a contract.** The
  dialect's linear-time guarantee constrains *which* match the engine reports,
  not merely how long it takes: alternation is leftmost-first rather than
  leftmost-longest, so `a|ab` against `ab` matches `a`. A manifest that reads
  `regex_search`'s result is therefore reading a specified value, not an
  engine-dependent one, and the guide has to say so for the same reason section
  8.3's first-appearance ordering is stated.

`regex_replace`'s `count` and `mandatory_count` interact with this: a bounded
replacement count is applied from the left, so the first N matches in the
leftmost-first order are the ones replaced.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. No helper takes a
`cap_std` handle, reads a path, or touches the environment, so the clause's
trapdoor rule about a filesystem predicate reporting `false` for an
out-of-scope path has nothing to bind here. The group is worth one explicit
sentence all the same, because a regular-expression engine is exactly the kind
of dependency an author would expect to be a capability hole: the `regex`
crate's linear-time guarantee is the property that keeps a pattern supplied by
a manifest from becoming one. A pattern that could backtrack catastrophically
would turn an 8 MiB input into unbounded work, which is a denial of service on
the build rather than a correctness bug, and section 8.4 chose the engine for
that reason.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. No helper takes a
`dialect` argument, none parses a path, and none renders one. The clause's
per-platform obligation still applies to each of the eight entries, and the
answer is the same for all of them: the helpers are defined over Unicode
strings, which have no platform variant. `regex_escape`'s `dialect` argument is
not a platform dialect — it names a regular-expression syntax, and `netsuke` is
the only accepted value.

One consequence is worth stating rather than leaving to the reader, because it
is the opposite of what a platform-contract section usually concludes: the
patterns are Unicode-aware by default, and line endings are *not* normalized. A
pattern containing `$` under `multiline` matches before a bare `\n` on every
platform, so a manifest that has read a Windows-authored file with `\r\n`
endings will see the `\r` as part of the preceding match. That is a property of
the input rather than of the host, which is why it belongs here and not in the
capability clause: two hosts given the same bytes produce the same result.

### 5.6. Type and error contract

RFC 0006 section 8.4 specifies the argument shapes. What follows is when a
subject, a pattern, a replacement, or an option value is rejected, each
carrying a code from section 5.9.

| Helper          | Accepted subject    | Rejects                                                                                                                                                                |
| --------------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `regex_replace` | a string            | `wrong_kind` for a non-string subject, pattern, or replacement; `bad_pattern`; `unsupported_construct`; `bad_replacement`; `mandatory_count_unmet`; `output_too_large` |
| `regex_search`  | a string            | `wrong_kind`; `bad_pattern`; `unsupported_construct`; `no_such_group`                                                                                                  |
| `regex_findall` | a string            | `wrong_kind`; `bad_pattern`; `unsupported_construct`; `no_such_group`; `match_limit`                                                                                   |
| `regex_escape`  | a string            | `wrong_kind`; `unknown_dialect`                                                                                                                                        |
| `match`         | a string            | `wrong_kind` for subject or pattern; `bad_pattern`; `unsupported_construct`                                                                                            |
| `search`        | a string            | as `match`                                                                                                                                                             |
| `regex`         | a string            | as `match`; `unknown_match_type`                                                                                                                                       |
| `version`       | two version strings | `wrong_kind` for either operand; `bad_version` naming the offending operand and text; `unknown_operator`; `unknown_scheme`                                             |

`unsupported_construct` reaches the six helpers that **compile** an
author-supplied pattern — `regex_replace`, `regex_search`, `regex_findall`,
`match`, `search`, and `regex` — because it is a property of the compiler
rather than of a call. It does not reach `regex_escape`, which escapes text
*into* a pattern and never parses one, nor `version`, which has no pattern at
all.

Four decisions this group adds:

- **`bad_pattern` carries the offset, not only the pattern.** Section 8.4
  requires an invalid pattern to produce "a typed, localized diagnostic
  carrying the pattern, the offset, and the parser's explanation". The offset
  is what makes the message actionable for a long pattern, and the parser's
  explanation is the third field rather than a restatement of the first two.
- **`unsupported_construct` is a distinct code from `bad_pattern`.** Section
  8.4 requires that an unsupported construct produce "a typed diagnostic naming
  the construct and the offset, not a generic parse failure". The two are
  different failures with different remedies: `bad_pattern` is a typo, and
  `unsupported_construct` is a construct that works in Python and cannot work
  here. Folding the second into the first would report the engine's limitation
  as the author's mistake, which is the diagnostic equivalent of a wrong answer.
- **`bad_replacement` is what a Python-style replacement gets.** Section 8.4
  rejects `\1` and `\g<name>` "with a diagnostic pointing at the `$1` form", so
  the code names the migration rather than reporting the replacement as
  malformed text. The message carries both spellings, because a manifest pasted
  from Ansible is the case this exists for and the author needs to see what to
  write instead.
- **`wrong_kind` covers numbers and booleans rather than stringifying them.**
  Section 8.4 is explicit that all seven pattern helpers "accept a string
  subject only", and section 8.5 that non-string version operands are errors.
  This is the clause's strict-undefined discipline reaching types: `1.82` as a
  YAML float is already a different value from the string `"1.82"`, and
  accepting it would make the version predicate's strictness vanish at the call
  site.

### 5.7. Canonical value equality

No additional obligation beyond RFC 0006 section 6.7. Clause 6.7 opens by naming
`subset`, `superset`, `contains`, and duplicate detection, and adds the
canonical-JSON domain those relations are defined over. Nothing in this group
keys a collection on a value: `regex_findall` returns strings, the four tests
return booleans, and `regex_search` returns a string or `none`. No mapping is
traversed and no deduplication occurs, so the canonical key is never computed
and the clause's rule about a hash-set iteration order has nothing to reach.

The one relation the clause's *spirit* does touch is `regex_search`'s `none`,
and the distinction is worth stating because it is easy to conflate. Section
8.4 guarantees that a non-match returns `none` and "never returns undefined, so
`| default(...)` and `is none` compose". That is the clause's strict-undefined
rule applied to a return value rather than to an argument, and it is the
opposite of a canonical-equality question: `none` here is a matched-absence
sentinel, not a value participating in a relation.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation, plus one output ceiling this group applies. This group reaches
three of table 3's rows, and it is the only child that reaches the cache row.

| Bound                  | Value                           | Where it applies                                       |
| ---------------------- | ------------------------------- | ------------------------------------------------------ |
| Match count            | 100000                          | `regex_findall`                                        |
| Compiled pattern size  | 1 MiB                           | the six helpers below that compile an author's pattern |
| Compiled pattern cache | 64 entries, least-recently-used | the six helpers below that compile an author's pattern |
| Output length          | 8 MiB                           | `regex_replace`                                        |

The six are `regex_replace`, `regex_search`, `regex_findall`, `match`,
`search`, and `regex`. The two bound rows scoped to "every regular-expression
helper" in RFC 0006 table 3 therefore reach all six and neither of the group's
other two members: `regex_escape` escapes text *into* a pattern and never
parses one, and `version` has no pattern to compile.

Four consequences this group decides:

- **The match ceiling can be reached before the input is exhausted.** 100000
  matches from a small pattern is easy to hit — a pattern matching every
  character of a 1 MiB string produces a million — so `regex_findall` must
  check the count as it grows and fail with `match_limit` rather than
  materializing the sequence and then measuring it. The rejection happens
  before the result is built, per clause 6.8.
- **The compiled-pattern ceiling is checked at compile time, not at match
  time.** A 1 MiB compiled automaton is a large but finite object, and the
  check belongs where the object is made: a pattern whose compiled form exceeds
  the ceiling is rejected on first use and never enters the cache, so a
  manifest cannot spend the whole cache on one pathological entry.
- **The cache is per-call-site state rather than a global, since it is bounded
  and LRU.** Netsuke already carries `lru`, and already runs an LRU cache for
  `which` at `src/stdlib/which/cache.rs`, which is the shape to follow: the
  cache and its capacity travel with the configured environment rather than in a
  `static`. Clause 6.2 makes this load-bearing rather than tidy — a global
  cache is state a manifest query would share with a build, and the query
  environment's whole purpose is to be free of ambient state.
- **`regex_replace` bounds its output, because a match count is not an output
  size.** The first row above bounds how many matches `regex_findall` may
  return, and it is scoped to that helper: it says nothing about how much text
  `regex_replace` may emit. Those are different quantities, and a pattern
  decides the second independently of the first. A subject of 1 MiB and a
  pattern matching one character per position is 1,048,576 matches, and a
  replacement naming `$0` twice emits 2 MiB from a template shorter than this
  paragraph — the count is under the ceiling while the output is arbitrary,
  because the replacement's length is the author's to choose and multiplies the
  match count. Matching that subject still costs time linear in its length, so
  section 8.4's guarantee holds unchanged; what the guarantee does not cover is
  the materialization the *replacement* asks for. Clause 6.8 names
  "materialized output" for exactly this case and requires the rejection before
  allocating, so `regex_replace` counts its output with checked arithmetic as
  the matches are walked, abandoning the walk the moment the running total
  passes 8 MiB, and fails with `output_too_large` at the ceiling RFC 0013's
  serializers and RFC 0014's amplifying transforms already apply. The count is
  taken before the result string is built rather than measured after.

The group enforces no other bound, and the absent subject ceiling is a
consequence of the dialect rather than an omission. Section 8.4 names the
syntax of the Rust `regex` crate and with it the property that decides this:
matching is "guaranteed linear-time in the length of the input". The engine is
a finite automaton, and the constructs that would break that guarantee —
look-around, back-references, recursion, atomic groups — are rejected by the
dialect rather than left to a runtime budget. A subject is therefore not an
expansion hazard the way a YAML alias graph or a `product` is: matching it
costs time linear in its own length and allocates in proportion to it, so table
3's 8 MiB input-length row, whose purpose is to stop a small input expanding
into a large one, has nothing to bound here. The rows that do reach this group
bound the compiled pattern, the match count, and `regex_replace`'s output: the
quantities a *pattern*, rather than a subject, can make grow. The output row is
the one that keeps the distinction honest — it bounds text rather than matches,
and it exists because a pattern can drive the former past the ceiling without
approaching the latter.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `PatternError`, and exactly one
`impl From<PatternError> for minijinja::Error`, per clause 6.9. Every message
is a Fluent key and every error carries a machine code, so the codes are
enumerated rather than described.

| Condition                             | Code                                             |
| ------------------------------------- | ------------------------------------------------ |
| subject or argument of the wrong kind | `netsuke::jinja::pattern::wrong_kind`            |
| pattern does not parse                | `netsuke::jinja::pattern::bad_pattern`           |
| pattern uses an unsupported construct | `netsuke::jinja::pattern::unsupported_construct` |
| Python-style replacement supplied     | `netsuke::jinja::pattern::bad_replacement`       |
| named or indexed group not in pattern | `netsuke::jinja::pattern::no_such_group`         |
| `mandatory_count` not achieved        | `netsuke::jinja::pattern::mandatory_count_unmet` |
| unknown `match_type`                  | `netsuke::jinja::pattern::unknown_match_type`    |
| unknown `regex_escape` dialect        | `netsuke::jinja::pattern::unknown_dialect`       |
| `regex_findall` match count exceeded  | `netsuke::jinja::pattern::match_limit`           |
| `regex_replace` output too large      | `netsuke::jinja::pattern::output_too_large`      |
| version operand does not parse        | `netsuke::jinja::pattern::bad_version`           |
| unknown version operator              | `netsuke::jinja::pattern::unknown_operator`      |
| unknown version scheme                | `netsuke::jinja::pattern::unknown_scheme`        |

Each code's Fluent key is its reason in upper snake case under
`STDLIB_PATTERN_`, per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>` form, so
`bad_pattern` pairs with `STDLIB_PATTERN_BAD_PATTERN`.

Three decisions this group adds:

- **One enum serves both halves of the group, and its module segment is
  `pattern`.** The version predicate shares no code with the regular-expression
  filters — `bad_version` and `unknown_operator` are its own — so a reviewer
  may ask why it does not get its own module. The shared codes are the ones an
  author meets first: `wrong_kind` and the option-enumeration codes behave
  identically in both halves, and two enums would mean two `From` impls and two
  Fluent key spaces expressing one contract. The name is `pattern` rather than
  `regex` because `version` is not a regular-expression helper and a module
  called `regex` that owns `bad_version` would mislead. It is singular to match
  clause 6.9's `stdlib.<module>.<condition>` and
  `keys::STDLIB_<MODULE>_<CONDITION>`, which both take the module name that
  `src/stdlib/` uses; the existing tree already has both spellings (`shell`,
  `which`, and `register` beside `collections` and `path`), so the rule that
  matters is that the key segment, the machine code's segment, and the file
  agree, which they do here.
- **The enumeration codes name their vocabulary in the message, not only in
  the code.** Section 8.4 requires `regex`'s unknown `match_type` to be "an
  error enumerating the three", `regex_escape`'s unknown dialect to enumerate
  the valid ones, and section 8.5 requires `version`'s unknown operator to
  enumerate all twelve and its unknown scheme to enumerate the accepted ones.
  Four codes therefore carry a list rather than a scalar, and the `From` impl
  is what renders it — the pattern RFC 0015's section 5.9 established for codes
  that carry numbers rather than sentences.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds eight, none an alias. The group carries
two of RFC 0006's renamed-or-rejected entries, and both are cases where the
accepted name is a deliberate departure from Ansible's.

`regex_search` and `regex_findall` are registered under Ansible's names with
**reshaped** signatures, which RFC 0006 section 7.6 records in place of a
rename. The reshaping is the group's substantive naming decision: Ansible's
`regex_search(value, regex, *args, **kwargs)` takes positional and keyword
arguments that mean different things depending on how many capture groups the
pattern has, and section 8.4 replaces that with an explicit `group` argument.
The name survives because the capability is the same and only the spelling of
its arguments changes; a rename would break every manifest for no gain, and the
reshaped argument list is what makes the return shape depend on the arguments
rather than on the pattern.

`version_compare` is rejected outright, in section 7.6, as an alias of
`version`. Ansible exposes both — `version` as the test and `version_compare`
as the same callable — and clause 6.10 registers one name per capability. The
same table rejects `regex_escape`'s Ansible `re_type='python'` spelling in
favour of a `dialect` argument with a single accepted value, which is the
clause's one-name-per-capability rule reaching an option rather than a helper.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. The clause's seven
obligations apply unmodified. Three group-specific notes rather than a
restatement.

Clause 6.11.4 requires property tests, and this group's properties are about
the dialect rather than about a data structure: `regex_escape`'s output matches
its input literally for every input, which is the one property that makes the
helper's name a promise; `regex_findall`'s return shape depends only on its
arguments, over patterns with zero, one, and several capture groups, which is
section 8.4's explicit rejection of Ansible's polymorphism; and the
compiled-pattern cache is not observable, over a pattern sequence longer than
the cache's 64 entries.

The remaining two are documentation obligations that the parent states and this
group is the first to discharge. Section 8.4 requires the supported and
unsupported constructs to be "named … in the standard-library guide", which
makes the dialect's limits a documented artefact rather than an emergent
property of the engine. Section 8.5 requires the guide to state that
pre-release identifiers order below the corresponding release and that build
metadata is ignored for comparison — two facts that are easy to get wrong by
assuming, and that a reader will otherwise infer from the wrong version of the
SemVer specification.

### Clause discharge

| Clause | Discharge                                                                                                                                                                |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `6.1`  | Eight pure `New` helpers; the eight section 5.1 rows are 8 of 52.                                                                                                        |
| `6.2`  | All eight pure, so all register in `register_query_helpers`, none stubbed.                                                                                               |
| `6.3`  | `regex_findall` returns leftmost-first input order; the cache is an optimization whose hit or miss is unobservable.                                                      |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken, and the linear-time engine is what keeps a pattern from becoming one.                                 |
| `6.5`  | No platform `dialect` argument; `regex_escape`'s dialect names a syntax, and the helpers are defined over Unicode strings.                                               |
| `6.6`  | Non-string subjects and operands rejected rather than stringified; four enumerated option sets; `bad_pattern` separate from `unsupported_construct`.                     |
| `6.7`  | No value is keyed or deduplicated; `regex_search`'s `none` is a matched-absence sentinel, not a relation over the canonical key.                                         |
| `6.8`  | Table 3's 100000-match ceiling for `regex_findall`, 1 MiB compiled pattern, the 64-entry least-recently-used pattern cache, and an 8 MiB `regex_replace` output ceiling. |
| `6.9`  | One enum, one `From` impl, thirteen `netsuke::jinja::pattern::*` codes.                                                                                                  |
| `6.10` | Eight new names, none an alias; `version_compare` rejected and the two reshaped names documented in section 5.10.                                                        |
| `6.11` | The clause's seven obligations, with the `regex_escape` and cache-invisibility properties the parent states.                                                             |

## 6. Dependencies

One new dependency, and it is a **promotion rather than an addition**. RFC 0006
section 13.4 lists `regex` as a new dependency needed by §8.4, and the crate is
already in `Cargo.toml` at `1.12.2` — but as a **dev-dependency**, used by
`src/snapshot_test_support.rs`. This group is what moves it to the
`[dependencies]` table, and the distinction matters for review: the
supply-chain gate section 13.4 requires re-runs against the same version rather
than a new one, and the version is not expected to move. `semver` is already a
normal dependency at `1` with the `serde` feature, so `version` adds nothing.

`lru` is likewise already a normal dependency at `0.18`, which is what lets
section 5.8 specify the pattern cache without a new crate.

Within the RFC set, it requires the shared contract that RFC 0006 section
14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3 deliver: the
bounded-materialization helper, needed by `regex_findall`'s match ceiling and by
`regex_replace`'s output ceiling. It requires no other child RFC, and none
requires it — section 14.5 notes that the version predicate has no dependency
on the regular-expression work, and this RFC keeps both in one document without
making either depend on the other.

## 7. Delivery

Roadmap step 6.5, which implements this RFC in five tasks:

- 6.5.1. The `netsuke-regex-v1` dialect and the bounded pattern cache,
  including the compiled-pattern size limit and the 64-entry
  least-recently-used cache.
- 6.5.2. `regex_replace`, with `count`, `mandatory_count`, the rejection of
  Python-style replacements, and the 8 MiB output ceiling.
- 6.5.3. `regex_search`, `regex_findall`, and `regex_escape`.
- 6.5.4. The `match`, `search`, and `regex` tests.
- 6.5.5. The `version` test over the existing `semver` dependency.

Each task carries the acceptance criteria that make this RFC checkable: an
unsupported construct producing a typed diagnostic naming the construct and the
offset; a Python-style replacement failing loudly rather than emitting `\1`;
`regex_findall` returning a sequence of strings whether the pattern has zero,
one, or several capture groups; a `regex_replace` whose replacement doubles
every match failing with `output_too_large` on a subject whose match count is
under the ceiling; and a version parse failure naming which operand failed and
the offending text.

## 8. Open questions

RFC 0006 section 16 assigns **question 3** to this group, and it is the first
question a child RFC has had to do more than record: "Should `version` tolerate
a `v` prefix?" Section 8.5 rejects it and points at `regex_search`, while
noting that "tags and `--version` output carry the prefix constantly", so an
explicit `strip_prefix=true` argument may be worth more than the purity.
Section 16 asks for this to be settled before slice 4, which is this RFC's own
slice.

**This RFC carries it unresolved and records the two options and their
consequences rather than choosing.** The case for `strip_prefix=true` is
frequency: the prefix is common enough that nearly every call site will
otherwise be `version | regex_search('^v?(.*)$') | first | version(...)`. The
case against is that the argument invites a third state — what happens to
`vv1.2.3`? — and that `regex_search` already discharges the need without
widening the version predicate's contract, which section 8.5 chose to keep
strict precisely so that `1.82` is rejected rather than silently treated as
`1.82.0`. A `strip_prefix` argument is the thin end of that wedge: once one
normalization is offered, the next manifest asks why the vendor suffix is not.

Roadmap task 6.5.5 resolves it before the test registers.

## 9. Recommendation

This group should be implemented fourth, at v0.1.x or later, and RFC 0006
section 14.11's recommended first wave says so: it takes "the
regular-expression family and `version`" from slice 4. The version predicate is
the single helper in the RFC set whose absence produces a *wrong answer* rather
than an awkwardness — string comparison of `1.10.0` against `1.9.0` is not
cumbersome, it is incorrect — and that makes it the easiest to justify of the
eight. The pattern family is the one that retires the most `shell()` calls per
helper, because inspecting a `--version` string, filtering a source list, and
testing a filename are all subprocess-shaped today. The group is also the only
child that promotes an existing dev-dependency, so it is where the supply-chain
gate is re-run against a crate the repository already carries.
