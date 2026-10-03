# RFC 0017: Lexical path composition

## Preamble

- **RFC number:** 0017
- **Status:** Proposed
- **Created:** 2026-10-01
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.6
- **Originating issue:** [#597](https://github.com/leynos/netsuke/issues/597)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies Netsuke's lexical path composition: the `dialect` argument
and the three parsers behind it, the filters `path_join`, `normpath`,
`splitext`, `commonpath`, `relpath`, and `splitdrive`, the same argument added
to the existing `basename` and `dirname` filters, and the pure test `abs`.
Together they let a manifest compose and inspect Windows path text on a Unix
host, which is the concrete form of the cross-compilation problem this group
exists to solve. Section 5.1's registry lists nine members: **seven new pure
helpers** and two existing filters gaining an option. The seven are the six
filters `path_join`, `normpath`, `splitext`, `commonpath`, `relpath`, and
`splitdrive` plus the pure test `abs`, which is the only member of RFC 0006
section 8.7 that observes no filesystem; `basename` and `dirname` are
registered already and take `dialect` additively rather than being registered
anew. All of it is lexical: the group reads no filesystem metadata, takes no
capability handle, and resolves nothing.

The group is the pure half of RFC 0006 section 14.6's slice 5. The observing
half — `exists`, `link_exists`, `same_file`, `mount`, and `expandvars` — is RFC
0018's, and RFC 0006 section 14.13 says why the two groups are not one: the
pure/observing boundary is the stronger seam, so split at it rather than at the
section 8 boundary.

## 2. Problem

A manifest that composes a path for another platform today has no way to say
so. Netsuke's existing `basename` and `dirname` parse with host-native rules,
which is correct for local paths and silently wrong for cross-compilation: a
Unix host inspecting `C:\build\out` reads the whole string as one component and
returns a `dirname` of `.`. The manifest does not fail; it produces a wrong
path that a later step may or may not notice.

The alternative Ansible offers is a family of `win_*` filter names, which RFC
0006 section 8.6 rejects. A parallel name family doubles every helper, makes
"which one do I call" depend on a fact about the target rather than about the
text, and gives a manifest author no way to compose a path they have not yet
decided the platform of. One `dialect` argument on one helper answers the same
question with one name.

The second problem is the one that makes this group more than a port. Lexical
normalization is a capability question disguised as a string operation:
normalizing `../../etc/passwd` produces text, not access, and the group's whole
safety argument is that it stays text. A manifest author reaching for
`normpath` to "resolve" a path would be reaching for `realpath` and would not
learn that from the name.

## 3. Goals and non-goals

- Goals:
  - Compose and inspect Windows path text on a Unix host, and POSIX path text
    on a Windows host, with one uniform argument rather than two name families.
  - Add `dialect` to the existing `basename` and `dirname` **additively**, so a
    manifest that omits it keeps today's behaviour byte for byte.
  - Refuse the two compositions that would otherwise be silently wrong:
    `path_join` resetting at an absolute component, and `commonpath`
    comparing a path that can ascend.
  - Keep every helper lexical, so all seven new helpers are usable from
    manifest queries and none needs a capability handle.
- Non-goals:
  - `win_*` names and `win_splitdrive`, rejected in RFC 0006 section 10.2 and
    section 8.6.
  - Any filesystem access. `exists`, `link_exists`, `same_file`, `mount`, and
    `expandvars` are RFC 0018's.
  - Symbolic-link resolution, which is `realpath` and already exists.
  - `expanduser`, which already exists and reads the home directory rather than
    composing text.
  - Changing `relative_to`, which is unchanged and remains the stricter tool.

## 4. Capability set

Seven new pure helpers and two existing filters extended by an option.

- `path_join` — join a sequence of components under one dialect.
- `normpath` — lexically normalize separators and `.` and `..` components.
- `splitext` — split the final component at its last dot.
- `commonpath` — the longest common lexical path.
- `relpath` — a general lexical relative path from a start to a path.
- `splitdrive` — the drive or UNC root and the remainder.
- `abs` — true when the path is absolute under the selected dialect.
- `basename(dialect=...)` — an option on the existing filter.
- `dirname(dialect=...)` — an option on the existing filter.

This section does not restate any contract. Each helper's argument shape,
options, rejection conditions, and edge cases are specified in
[RFC 0006 section 8.6](0006-ansible-inspired-template-standard-library.md#86-lexical-path-composition)
and
[section 8.7](0006-ansible-inspired-template-standard-library.md#87-filesystem-predicates),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

### 5.1. Registry

| Helper       | Namespace | Registration | Purity class | Manifest query |
| ------------ | --------- | ------------ | ------------ | -------------- |
| `path_join`  | Filter    | New          | Pure         | Yes            |
| `normpath`   | Filter    | New          | Pure         | Yes            |
| `splitext`   | Filter    | New          | Pure         | Yes            |
| `commonpath` | Filter    | New          | Pure         | Yes            |
| `relpath`    | Filter    | New          | Pure         | Yes            |
| `splitdrive` | Filter    | New          | Pure         | Yes            |
| `abs`        | Test      | New          | Pure         | Yes            |
| `basename`   | Filter    | Option added | Pure         | Yes            |
| `dirname`    | Filter    | Option added | Pure         | Yes            |

Seven `New` rows and two `Option added` rows. The seven are the entire pure
contribution of RFC 0006 sections 8.6 and 8.7 other than `expandvars`, which is
environment-observing and belongs to RFC 0018; the roadmap's step 6.6 owns all
seven, and step 6.7 owns the observing group. RFC 0006 section 14.13 records
the optioned pair in this row's `Optioned` column because `basename` and
`dirname` already exist, so they add no helper to section 6.1's fifty-seven and
no pure helper to its fifty-two.

The registry takes the five written children to **41 of the 52 pure helpers**.
RFC 0013 contributes 5, RFC 0014 6, RFC 0015 15, RFC 0016 8, and this RFC 7, so
the five written children account for 41 and the three still to write — 0018,
0019, and 0020 — account for 11.

### 5.2. Manifest-query availability

No additional obligation beyond RFC 0006 section 6.2. All seven new helpers are
pure, so RFC 0006 section 6.2 clause 1 registers them in
`register_query_helpers`; none is stubbed, and the always-failing-stub path of
clause 2 is not reached by this group. The two optioned filters are already
registered there — `src/stdlib/path/filters.rs` wires `register_query_filters`
to `register_lexical_filters`, which is where `basename` and `dirname` live —
so extending them with `dialect` extends both the recipe and the query
registration at once and cannot leave the two disagreeing.

The clause's disposition test nevertheless has to cover the two optioned
filters as it covers the seven, because the inventory from section 14.1 lists
every helper the RFC touches rather than every helper it introduces.

### 5.3. Determinism

No additional obligation beyond RFC 0006 section 6.3. No helper in this group
returns a sequence whose order is not the input's: `path_join`, `normpath`,
`commonpath`, `relpath`, and `splitdrive` each return a string or, for
`splitext` and `splitdrive`, a two-element sequence whose order section 8.6
fixes as `[stem, extension]` and `[drive, rest]` respectively. Nothing here is
keyed on a hash, so no iteration order can leak into a generated graph.

The clause's determinism requirement reaches one thing this group might
otherwise get wrong, and it is worth naming rather than leaving to the
implementation: the `windows` dialect's comparison is
**ASCII-case-insensitive** while its output is not case-normalized. That means
`commonpath` and `relpath` compare components case-insensitively but return the
caller's own spelling, so `C:\SRC` and `C:\src\lib.rs` have a common path of
`C:\SRC` rather than of a folded or lowercased form. Two manifests that differ
only in the case of an input therefore produce different text for the same
relation, which is correct under this dialect — the relation is
case-insensitive, the value is not — and is the only place in the group where
the two can be confused.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. Every helper in the group
is pure and lexical: none opens a file, none stats one, none resolves a
symbolic link, and none takes a `Capability` handle. `normpath` is the one that
invites the mistake, because normalization *sounds* like resolution — RFC 0006
section 8.6 states the boundary as "normalizing `../../etc/passwd` produces
text, not access", and the group's obligation is to keep it that way. The
guide's `normpath` entry carries the classic caveat the clause exists for: in
the presence of symbolic links, lexical normalization can change which file a
path denotes, and `realpath` is the filter that consults the filesystem.

`abs` is the clause's second face. It is a lexical predicate over the text, so
it is available during manifest queries unlike the four filesystem-observing
tests in RFC 0006 section 8.7. Under the `windows` dialect a rooted path with
no drive such as `\foo` is **not** absolute, and neither is a drive-relative
path such as `C:foo`; `abs` answers a question about the text and never asks
whether the path exists.

### 5.5. Platform contract

This clause is where the group earns its place, so it carries more than a
statement of compliance. RFC 0006 section 6.5 requires each helper's platform
behaviour to be stated, and the group's answer is that **platform enters through
`dialect` and nowhere else**. `host` resolves to `windows` when compiled for
Windows and to `posix` otherwise; the two explicit values name the syntax
rather than the machine. A helper never reaches for host-native parsing once a
dialect has been selected, which is the property roadmap task 6.6.5's
combinatorial suite exists to falsify.

Three decisions follow from that:

- **`dialect` is one mechanism on every helper, not a per-helper option.**
  `path_join`, `normpath`, `splitext`, `commonpath`, `relpath`, `splitdrive`,
  and `abs` all take it, as do the extended `basename` and `dirname`. A helper
  that took it under a different name, or that defaulted differently, would
  make the suite's cross-product non-uniform and give an author one more thing
  to remember.
- **The `windows` dialect accepts both separators and emits `\`.** Accepting
  both is what makes a manifest pasted from a mixed source usable; emitting one
  is what makes the output reproducible. Emitting the caller's original
  separators would make a normalized path depend on its input spelling, which
  section 5.3 forbids.
- **Differing drives are an error rather than a fallback.** `path_join` with
  components on differing drives, `commonpath` with inputs on differing drives
  or UNC roots, and `relpath` with differing drives all fail under the
  `windows` dialect. There is no correct lexical answer to any of them, and a
  plausible-looking one is worse than an error.

### 5.6. Type and error contract

RFC 0006 section 8.6 specifies the argument shapes. What follows is when a
subject, a component list, or an option value is rejected, each carrying a code
from section 5.9.

| Helper       | Accepted subject                | Rejects                                                                                                                                                                                  |
| ------------ | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `path_join`  | a non-empty sequence of strings | `wrong_kind` for a non-sequence or a non-string component; `empty_input` for an empty sequence or component; `absolute_component` naming the index; `different_drive`; `unknown_dialect` |
| `normpath`   | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |
| `splitext`   | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |
| `commonpath` | a non-empty sequence of strings | `wrong_kind`; `empty_input`; `mixed_path_kind`; `parent_component`; `different_drive`; `unknown_dialect`                                                                                 |
| `relpath`    | a string                        | `wrong_kind`; `mixed_path_kind`; `different_drive`; `unknown_dialect`                                                                                                                    |
| `splitdrive` | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |
| `abs`        | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |
| `basename`   | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |
| `dirname`    | a string                        | `wrong_kind`; `unknown_dialect`                                                                                                                                                          |

Four decisions this group adds:

- **`path_join`'s absolute-component error names the index, not the value.**
  Section 8.6 requires the error "naming the index", and the index is the half
  a caller cannot recover: the offending component is in the message too, but a
  manifest joining eight components needs to know *which* one reset the path,
  and `['/safe/root', '/etc/passwd']` is a two-line reproduction while
  `index 4` is a pointer into the caller's own expression.
- **`commonpath` rejects `..` inputs with a code of their own, and `relpath`
  does not.** Section 8.6 says `commonpath`'s inputs containing `..` are
  rejected "because a purely lexical common prefix is not meaningful once a
  path can ascend". That is a different failure from mixing absolute and
  relative paths and has a different remedy — the author wants `realpath`, or
  wants to normalize first — so it gets `parent_component` rather than being
  folded into `mixed_path_kind`. `relpath` is the deliberate exception, and
  section 8.6 says so in its own words: it "may contain `..`" and "is for the
  cases that genuinely need to ascend", which the existing `relative_to` filter
  refuses. Adding `parent_component` to `relpath` would therefore remove the
  one helper the RFC keeps for ascent, so the rejection belongs to `commonpath`
  alone.
- **`different_drive` is one code across three helpers.** `path_join`,
  `commonpath`, and `relpath` all reach it under the `windows` dialect, and all
  three mean the same thing by it. One code keeps the diagnostic vocabulary
  small enough that a manifest author learns it once.
- **`unknown_dialect` is shared with the three-value enumeration in its
  message.** Every helper in the group can raise it, which is what makes it the
  group's most-reached error rather than an edge case. Section 8.6 requires an
  unknown value to be "an error enumerating the three", so the message carries
  `host`, `posix`, and `windows` rather than naming only the value it rejected.

### 5.7. Canonical value equality

No additional obligation beyond RFC 0006 section 6.7. Clause 6.7 opens by naming
`subset`, `superset`, `contains`, and duplicate detection, and adds the
canonical-JSON domain those relations are defined over. Nothing in this group
keys a collection on a value: the six filters return strings or two-element
sequences of strings, and `abs` returns a boolean. No mapping is traversed and
no deduplication occurs, so the canonical key is never computed.

There is one place where equality *is* decided, and it is deliberately not the
clause's: `commonpath` and `relpath` compare path components, and under the
`windows` dialect that comparison is ASCII-case-insensitive per section 8.6.
That is a domain relation defined by the dialect rather than the canonical-JSON
equality clause 6.7 establishes, and the two must not be conflated — a manifest
that wants canonical equality between two paths has `==`, and a manifest that
wants "the same path on Windows" has `commonpath`.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's. **This group reaches none of them**, and it
is the only child that reaches none.

That is not an omission to be tidied up. Every row of table 3 bounds an
allocation: an input length for a parser, a nesting depth for a recursive one,
an alias count and an output-tuple count for a combinatorial or materializing
one, a match count and a compiled-pattern size for the pattern family. This
group allocates nothing that grows with anything but its own input's length,
and its output is bounded by its input by construction: `normpath` is length-
decreasing or length-preserving, `commonpath` is a prefix of an input,
`splitext` and `splitdrive` partition an input rather than extending it. There
is no recursion, because lexical normalization is a single left-to-right pass
and `..` cancellation is a stack rather than a nested call.

The two composing helpers are bounded too, but by a **multiple** of their
operands rather than by the operands themselves. `path_join` inserts a
separator between every adjacent pair, so *n* components totalling *L* bytes
yield at most *L* + *n* − 1 bytes. `relpath`'s ascent contributes one `../`
segment per component it climbs, and its descent is the remainder of the
target, so a start of depth *d* and a target of *L* bytes yield at most *L* +
3*d* bytes. Both multipliers are bounded by the operand count, which is itself
bounded by the input-length row's 8 MiB, so neither can amplify an input into
an unbounded allocation: the ceiling on the result is at most a small multiple
of the ceiling on the input, not a function of the input's *content*. This is
the distinction the clause's materialization test turns on — a helper that
multiplied by the value of a component, or by a count the manifest supplied
separately, would reach an output the input length does not bound, and an
output ceiling would then be owed.

The one bound the clause does impose is the input-length row's *spirit* rather
than its letter — a manifest supplying a pathologically long path is bounded by
the same 8 MiB the parsers use, because it arrives through the same template
evaluation — and section 8.6 sets no separate ceiling for this group. Stating
that the group reaches no bound is the honest discharge; inventing a bound to
have one to cite would be the vacuity this clause's discharge exists to catch.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `PathDialectError`, and
exactly one `impl From<PathDialectError> for minijinja::Error`, per clause 6.9.
Every message is a Fluent key and every error carries a machine code, so the
codes are enumerated rather than described.

| Condition                              | Code                                       |
| -------------------------------------- | ------------------------------------------ |
| subject or component of the wrong kind | `netsuke::jinja::path::wrong_kind`         |
| empty sequence or empty component      | `netsuke::jinja::path::empty_input`        |
| absolute component after the first     | `netsuke::jinja::path::absolute_component` |
| absolute and relative paths mixed      | `netsuke::jinja::path::mixed_path_kind`    |
| `..` component where it is meaningless | `netsuke::jinja::path::parent_component`   |
| differing drives or UNC roots          | `netsuke::jinja::path::different_drive`    |
| unknown `dialect` value                | `netsuke::jinja::path::unknown_dialect`    |

Each code's Fluent key is its reason in upper snake case under `STDLIB_PATH_`,
per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>` form, so `unknown_dialect`
pairs with `STDLIB_PATH_UNKNOWN_DIALECT`.

Two decisions this group adds:

- **The module segment is `path`, and it is the existing module rather than a
  new one.** `src/stdlib/path/` exists, holds `basename` and `dirname`, and
  keys its messages `stdlib.path.*` under `keys::STDLIB_PATH_*` — the `io`,
  `action`, `hash`, and `expanduser` families among them. This group extends
  that namespace rather than opening a second one, which is the same rule RFC
  0015's section 5.9 states from the opposite side: the segment names the
  module the helpers live in, so `path` here and `collections` there are both
  correct and a singular form would be a typo waiting to happen. It also means
  `basename`'s new `unknown_dialect` sits beside its existing keys rather than
  in a parallel tree.
- **`unknown_dialect` enumerates its vocabulary in the message.** Section 8.6
  requires an unknown value to be an error enumerating the three values, so the
  `From` impl renders `host`, `posix`, and `windows` rather than only naming
  the rejected value. This is the pattern RFC 0015's section 5.9 established
  for codes that carry a list rather than a scalar, and `absolute_component` and
  `different_drive` follow it for their own payloads — an index and a pair of
  drive names respectively.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds seven, none an alias. The group carries
the RFC's two substantive naming decisions, and both are cross-namespace.

`abs` is registered as a **test** while MiniJinja registers `abs` as a
**filter** meaning numeric absolute value. RFC 0006 section 11.4 keeps the name
and records the resolution: Jinja keeps filters and tests in separate
namespaces, so `{{ n | abs }}` and `{% if p is abs %}` coexist without
grammatical ambiguity, and the collision is a fact about a human reader rather
than about the parser. The two mitigations section 11.4 requires are
obligations this group carries — the inventory lists both adjacent with their
namespaces marked, and the guide's path section states the distinction where
`abs` is introduced — and section 8 picks up the open question section 11.4
leaves.

`splitdrive` replaces Ansible's `win_splitdrive`, which section 10.2 rejects
along with the rest of the `win_*` family. The name drops the prefix because
the dialect argument carries what the prefix said, which is the whole point of
the mechanism.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. Each of the seven new
helpers and each of the two optioned filters requires all seven obligations
before its roadmap task is complete: a guide entry, a `tested-example` fence,
unit tests covering accepted kinds, rejected kinds, boundary values, and every
enumerated option value, a property test where an invariant exists, an
integration test through a manifest, an integration test for the manifest-query
disposition, and an inventory row.

The property tests this group owes are named by section 6.11 itself, which
lists "composition laws for `combine`, `path_join`, and `normpath`". Two laws
are worth stating because they are what makes the group checkable rather than
merely tested:

- **`normpath` is idempotent.** Normalizing an already-normalized path yields
  it unchanged, which is the property that makes the filter safe to apply twice
  and is falsifiable by a single counterexample.
- **`path_join` and `normpath` compose.** Normalizing a join is normalizing
  each component and rejoining, provided no component is absolute and none
  ascends — the proviso is the point, since `..` cancellation is not
  componentwise and the law fails exactly where `parent_component` would be
  raised elsewhere.

The combinatorial suite roadmap task 6.6.5 adds is the group's load-bearing
test rather than an extra: it crosses every lexical helper with all three
dialects and both host platforms, including drive-relative paths, UNC roots,
trailing separators, and leading `..` components, and it fails if any helper
reaches for host-native parsing when an explicit dialect was supplied.

### Clause discharge

| Clause | Discharge                                                                                                                                       |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Seven pure `New` helpers and two `Option added` rows; the seven are 7 of 52.                                                                    |
| `6.2`  | All seven pure, so all register in `register_query_helpers`; the optioned pair is already in `register_lexical_filters`.                        |
| `6.3`  | Every result is a string or a fixed-order pair; `commonpath` and `relpath` compare case-insensitively but never case-fold their output.         |
| `6.4`  | Nothing opens, stats, or resolves; `normpath` produces text rather than access and `abs` asks about the text rather than the filesystem.        |
| `6.5`  | Platform enters only through `dialect`; the `windows` dialect accepts both separators, emits `\`, and errors on differing drives.               |
| `6.6`  | Non-string subjects and components rejected rather than stringified; three enumerated dialects; seven rejected conditions with their own codes. |
| `6.7`  | No value is keyed or deduplicated; the dialect's component comparison is a domain relation, not the canonical key.                              |
| `6.8`  | The group reaches no table 3 bound: every output is bounded by its input and nothing recurses.                                                  |
| `6.9`  | One enum, one `From` impl, seven `netsuke::jinja::path::*` codes under the existing `stdlib.path.*` namespace.                                  |
| `6.10` | Seven new names, none an alias; `abs` keeps its name across namespaces per section 11.4, and `splitdrive` drops the rejected `win_` prefix.     |
| `6.11` | Seven guide entries, seven `tested-example` fences, the `normpath` and `path_join` laws, and the combinatorial dialect suite.                   |

## 6. Dependencies

**No new dependency.** The group is lexical string manipulation over UTF-8 path
text, which `camino`'s `Utf8Path` provides and which is a normal dependency at
`1.2.0`. Parsing is a handwritten pass over separators and components rather
than a call into a platform path library, because the whole point of the group
is that the platform is a parameter rather than a fact about the machine. A
dependency that parses Windows paths on Unix would be a second implementation
of rules RFC 0006 section 8.6 already states, and the dialect would then be
documented in two places.

Within the RFC set, the group requires the shared contract that RFC 0006
section 14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3
deliver. It requires no other child RFC. RFC 0018 requires **this** RFC, because
`expandvars`'s roadmap task 6.7.4 lists step 6.6.1 as a prerequisite and
because the environment-observing group's helper takes the same `dialect`
argument this group defines — so the mechanism lands here and is consumed there.

## 7. Delivery

Roadmap step 6.6, which implements this RFC in five tasks:

- 6.6.1. The `dialect` argument and its `host`, `posix`, and `windows` parsers,
  extending `basename` and `dirname` additively.
- 6.6.2. `path_join`, `normpath`, and `splitext`.
- 6.6.3. `commonpath`, `relpath`, and `splitdrive`.
- 6.6.4. The `abs` test as a pure lexical predicate.
- 6.6.5. The combinatorial path-dialect suite.

Each task carries the acceptance criteria that make this RFC checkable: a Unix
host parsing a Windows path identically to a Windows host with no host-native
fallback; `['/safe/root', '/etc/passwd'] | path_join` failing by naming the
index rather than yielding `/etc/passwd`; `abs` registered in the read-only
manifest-query environment unlike the filesystem predicates in step 6.7; and a
suite that fails if any helper reaches for host-native parsing when an explicit
dialect was supplied.

## 8. Open questions

RFC 0006 section 16 assigns **question 2** to this group: "Is `abs` the right
test name?" Section 11.4 keeps it despite the cross-namespace reuse with
MiniJinja's `abs` filter, and names `absolute` and `abs_path` as the
alternatives considered. Section 16 asks for this to be settled "before slice
5", which is this RFC's own slice, and roadmap task 6.6.4 repeats it: "Resolve
RFC 0006 §16 question 2 on the name before registering."

**This RFC carries it unresolved and records the three options and their
consequences.** It is the second consecutive child to carry its assigned
question rather than settle it, and the reason is the same as RFC 0016's: the
choice is manifest-visible and cheap to make late, and the argument on each
side is short enough to state in full.

- **Keep `abs`.** The name is Ansible's, the discoverability benefit for an
  Ansible-literate author is real, and the grammar disambiguates it completely
  — a filter call and a test call are different productions, so no manifest can
  mean the wrong one. The cost is a human reader who sees `abs` in the
  inventory twice and has to check which is which.
- **Rename to `absolute`.** No collision with a MiniJinja filter, so the
  inventory has one unambiguous entry and a reader never has to consult the
  namespace column. The cost is that the name is longer and that it is not the
  spelling an Ansible author will try first, which RFC 0006 section 10.1's
  rejection of the `is_abs` family already shows is a real cost to pay twice.
- **Rename to `abs_path`.** Also collision-free, and it says *what* is absolute
  — the path — rather than only asserting the property. The cost is the same
  discoverability cost as `absolute` plus a suffix that no Ansible author
  expects, which makes it the worst of the three on the criterion section 11.4
  used to decide the question in the first place.

Roadmap task 6.6.4 resolves it before the test registers. The recommendation of
this RFC, for the record rather than as a decision, is to keep `abs`: section
11.4's reasoning is sound, the mitigations it requires are cheap, and a
cross-namespace reuse that the grammar resolves without ambiguity is a
documentation problem rather than a design one.

## 9. Recommendation

This group should be implemented fifth, at v0.1.x or later. It is the smallest
of the three remaining groups by helper count after RFC 0018 and RFC 0019 are
counted, and it is the one that unblocks another child: RFC 0018's `expandvars`
takes this group's `dialect` argument, so landing the mechanism here is what
keeps the environment-observing helper from inventing a second one.

The case for the group is cross-compilation. A Unix host composing Windows path
text is not a hypothetical for a build tool, it is the ordinary state of a
containerized or CI build whose target is another platform, and today that
composition either happens in `shell()` or happens wrongly. The case for the
*dialect mechanism* over a `win_*` name family is RFC 0006 section 8.6's, and
this group is where it is tested: one argument, one helper set, one
cross-product suite, and no helper that silently falls back to the host when
the author has said which platform they mean.
