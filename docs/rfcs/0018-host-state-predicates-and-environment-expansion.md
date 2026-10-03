# RFC 0018: Host-state predicates and environment expansion

## Preamble

- **RFC number:** 0018
- **Status:** Proposed
- **Created:** 2026-10-03
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.7
- **Originating issue:** [#597](https://github.com/leynos/netsuke/issues/597)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies Netsuke's filesystem predicates and its one
environment-observing helper: the tests `exists`, `link_exists`, `same_file`,
and `mount`, the `files_only` option added to the existing `glob` function, and
the filter `expandvars`. Section 5.1's registry lists six members: **five new
helpers** and one existing function gaining an option. The five are the four
tests plus `expandvars`. `glob` is the sixth, and it is registered already: it
takes `files_only` additively rather than being registered anew.

The group takes the observing members of RFC 0006 section 14.6's slice 5 —
`exists`, `link_exists`, `same_file`, `mount`, and `glob(files_only=...)` — and
joins them to `expandvars` from section 14.7's slice 6. The pure members of
slice 5 — the `dialect` parsers, the six lexical filters, and the `abs` test —
are [RFC 0017](0017-lexical-path-composition.md)'s. RFC 0006 section 14.13
gives the reason: section 8.6 and section 8.7 "sit on opposite sides of the
pure/observing boundary", so rows `0017` and `0018` divide those two groups
between them. This group is therefore the one child that is uniformly **not**
pure; every member observes something outside the manifest, and every member is
consequently unavailable during manifest queries.

The group is the one place the section 8.7 and section 8.6 boundaries do not
coincide with the row boundary, and it is deliberate on both sides. `abs` is
specified in section 8.7 beside the four tests it resembles, but it is pure, so
it travels with the lexical group; `expandvars` is specified in section 8.6
beside the lexical filters it resembles, but it is environment-observing, so it
travels with this one. RFC 0006 section 14.7 separates it from the lexical
helpers for exactly that reason: it is "the only environment-observing helper
in the RFC and therefore the only one that needs an injected reader, a
manifest-query stub, and its own capability review".

## 2. Problem

Netsuke can test a path for lexical properties but not for facts. A manifest
that wants to know whether a tool was configured, whether a link is dangling,
or whether two paths name the same file has no way to ask, and the workarounds
are all worse than the question: `shell()` for a `test -e`, an `env` fallback
for a variable that may be unset, or a build step that fails later with a
message about a missing file rather than about the manifest's assumption.

The existing `glob` makes this concrete. It filters every match to regular
files, so a manifest that wants directories too writes
`glob('build/*') | select('dir')` — except that it cannot, because the
directory never reaches the template. The filter is already there and already
capability-scoped; what is missing is the ability to turn it off. The option
exists because the excluded case deserves a spelling rather than a second glob.

The environment is the sharper half of the problem. `env()` already reads the
environment, and RFC 0006 section 8.6 adds `expandvars` for interpolating
`$NAME` into a longer string. Both must read through the injected reader that
[ADR-008](../adr-008-environment-seam-taxonomy.md) establishes, because a
helper reaching for `std::env::var` directly would give a manifest query and a
build two different environments to disagree about — the ambient-authority path
the roadmap step's own summary names as the thing to avoid.

## 3. Goals and non-goals

- Goals:
  - Answer questions about the filesystem through the injected capability
    handle, so every path is scoped to the workspace the manifest was given.
  - Distinguish "the link is missing" from "the target is missing", which no
    existing Netsuke test can express.
  - Compare file identity rather than path spelling, and error rather than
    guess when the platform cannot decide.
  - Add `files_only` to the existing `glob` **additively**, leaving its
    capability scoping, ordering, and observability contracts unchanged. The
    current filter is `files_only=true`, so the shipped behaviour is preserved
    under that setting. **The default is not settled here:** RFC 0006 section
    8.7's signature reads `files_only=false` while its section 12 promises
    behaviour-preserving defaults, and the shipped `glob` filters directories
    unconditionally, so the two cannot both hold. Section 8 records the conflict
    and roadmap task 6.7.3 decides it. What this RFC fixes is that the question
    is manifest-visible rather than discovered by a user.
  - Expand `$NAME` and `${NAME}` through the injected environment reader, with
    strict-by-default handling of unset variables.
- Non-goals:
  - Any lexical path operation. The `dialect` parsers, `normpath`,
    `commonpath`, and the `abs` test are [RFC 0017](0017-lexical-path-composition.md)'s.
  - `expanduser`, which already exists and reads the home directory rather than
    the environment.
  - Resolving symbolic links, which is `realpath` and already exists.
  - A second glob implementation. Ansible's `fileglob` is rejected in RFC 0006
    section 8.7 precisely so that one does not appear.
  - Any new environment seam. `expandvars` consumes the reader ADR-008 already
    defines rather than introducing a second one.

## 4. Capability set

Five new helpers, one existing function extended by an option, and one existing
helper whose registration is unchanged.

- `exists` — true when the path resolves to an existing object, following
  symbolic links.
- `link_exists` — true when link metadata resolves, so a dangling link is true.
- `same_file` — true when both paths denote the same filesystem object.
- `mount` — true when the path is a mount point.
- `expandvars` — expand `$NAME` and `${NAME}` through the injected reader.
- `glob(pattern, files_only=...)` — an option on the existing function. RFC
  0006 section 8.7 spells the default `false`; the shipped filter corresponds to
  `true`, and the two disagree. The default is decided by roadmap task 6.7.3,
  not here; see section 8.

This section does not restate any contract. Each helper's argument shape,
options, rejection conditions, and edge cases are specified in
[RFC 0006 section 8.7](0006-ansible-inspired-template-standard-library.md#87-filesystem-predicates)
and
[section 8.6](0006-ansible-inspired-template-standard-library.md#86-lexical-path-composition),
and what follows in section 5 is how this group meets the cross-cutting
clauses rather than what the helpers do.

## 5. Cross-cutting contract conformance

### 5.1. Registry

| Helper        | Namespace | Registration | Purity class          | Manifest query |
| ------------- | --------- | ------------ | --------------------- | -------------- |
| `exists`      | Test      | New          | Filesystem-observing  | No             |
| `link_exists` | Test      | New          | Filesystem-observing  | No             |
| `same_file`   | Test      | New          | Filesystem-observing  | No             |
| `mount`       | Test      | New          | Filesystem-observing  | No             |
| `expandvars`  | Filter    | New          | Environment-observing | No             |
| `glob`        | Function  | Option added | Filesystem-observing  | No             |

Five `New` rows and one `Option added` row. RFC 0006 section 14.13 records
`glob` in this row's `Optioned` column because it already exists, so it adds no
helper to section 6.1's fifty-seven and no pure helper to its fifty-two. This
group contributes **no** pure helper: it is the one child whose rows all fall
outside section 6.1's fifty-two, and the coverage contract asserts that as a
hard zero rather than as a budget.

The registry takes the six written children to **41 of the 52 pure helpers** —
unchanged from RFC 0017, because none of this group's five new helpers is pure.
The group's contribution to the accepted set is 5, taking the accepted helpers
covered by the six written children to 46 of 57. The two remaining children,
0019 and 0020, account for the last 11 pure helpers.

### 5.2. Manifest-query availability

RFC 0006 section 6.2's clause 1 registers pure helpers in
`register_query_helpers`, and this group has none, so the disposition is clause
2 for every row: each registers a **stub** in `register_disabled_query_helpers`
that fails with the explicit restriction diagnostic.

Two of the five are not new to that treatment. `glob` is already a
filesystem-observing function and already has the disposition it needs — adding
`files_only` extends the recipe without touching the registration, so the
option cannot make the two disagree. `expandvars` is deliberately excluded from
the read-only registration **exactly as `env` is excluded**, which roadmap task
6.7.4 names as an acceptance criterion: the exclusion must cover every
read-only generation caller, not only `netsuke help targets`.

The group's obligation beyond RFC 0006 section 6.2 is that the stub's message
must be actionable in each case. "Unavailable during manifest queries" is the
right answer for all five, and section 5.9 gives each its own code so a
manifest author can tell which helper they reached for rather than inferring it
from the surrounding text.

### 5.3. Determinism

No additional obligation beyond RFC 0006 section 6.3 for four of the five
tests: each returns a boolean, and a boolean has no ordering to get wrong.
`expandvars` returns the input's text with substitutions applied in a single
left-to-right pass, so its output is a function of its arguments and the
injected environment alone.

`glob` is the member the clause reaches, and it reaches it because the group
*changes* what is filtered rather than how it is ordered. The existing ordering
contract is part of what ADR-010 scopes and what roadmap task 6.7.3 requires be
left unchanged: `glob_paths` returns matches "in the order the `glob` crate
yields them", and that order must survive either option value. The metadata
check that decides each match already runs per entry — `walk.rs`'s
`names_a_file` classifies each candidate as a path, an unreachable symlink, or
`NotAFile` — so `files_only` selects which of those classifications yield a
path, not how the surviving paths are sequenced. Under either value two runs
over the same tree yield byte-identical sequences, because the traversal order
is the `glob` crate's and the option does not touch it. A `false` implemented
by re-globbing without the metadata pass, or by concatenating the two sets,
would be a determinism defect in a change that adds no iteration order of its
own.

### 5.4. Capability boundary

This clause is where the group earns its place, so it carries more than a
statement of compliance. RFC 0006 section 6.4 requires every filesystem touch
to go through the injected capability handle, and every test in this group
obeys it: `exists`, `link_exists`, `same_file`, `mount`, and `glob`'s
`files_only` traversal all resolve paths through the `cap_std` workspace handle
the manifest was given, and none reaches for an ambient `std::env::var` or a
host-native `stat`.

Three decisions follow from that:

- **A path outside the capability boundary is an error, not `false`.** This is
  roadmap task 6.7.1's acceptance criterion and it is the distinction the whole
  clause turns on: "the file is not there" and "you may not look there" are
  different manifest bugs with different remedies. Collapsing the second into
  the first would make a capability violation indistinguishable from a missing
  file, which is precisely the silent failure the boundary exists to prevent.
- **`same_file` errors on a missing operand rather than reporting `false`.**
  Section 8.7 states it directly: "these are not the same file" and "one of
  them is not there" are different bugs. A `false` here would let a manifest
  compare a path against a typo and conclude the two differ, which is true and
  useless.
- **`mount` errors on a platform where it cannot decide, rather than returning
  `false`.** It is the most platform-divergent helper in RFC 0006, and section
  8.7 records the acceptable fallback — a Unix-only implementation with an
  explicit unsupported-platform error on Windows, documented in the guide.
  Silently returning `false` is explicitly not acceptable, because a mount test
  that answers "no" on a platform it does not understand is worse than one that
  declines to answer.

`expandvars` is the clause's environment face. It reads through the injected
reader described in RFC 0006 section 6.4 and ADR-008, never through an ambient
`std::env::var` call in the leaf helper, and it inherits `env`'s existing
treatment rather than inventing a new one: a non-UTF-8 environment value is an
error, matching the existing `env` function. The whole point of the seam is
that a manifest query and a build cannot see different environments, and a
helper that read the process environment directly would break that while still
passing every test that supplies an environment the ordinary way.

### 5.5. Platform contract

RFC 0006 section 6.5 requires each helper's platform behaviour to be stated,
and this group's answer is that **platform enters through the filesystem and
through two explicitly named divergences** — nothing else.

- `exists`, `link_exists`, `same_file`, and `glob(files_only=true)` are
  platform-uniform in *specification* and platform-specific only in
  *implementation*. The pair `exists`/`link_exists` exists to make the
  symbolic-link behaviour explicit rather than incidental, and it behaves
  identically wherever symbolic links exist.
- `same_file` compares by **file identity**, which is device and inode on Unix
  and volume serial number plus file index on Windows. On a platform where
  identity cannot be determined it errors with a diagnostic naming the
  platform; it never falls back to comparing normalized path text, because that
  would answer a different question and answer it wrongly in exactly the
  hard-link case the helper exists for.
- `mount` is the divergent one. Unix decides by comparing the path's device
  identifier with its parent's, or by the path being its own parent; Windows
  decides by volume root or mounted folder. Section 8.7 sequences it last
  within its slice for this reason and permits the Unix-only fallback with an
  explicit unsupported-platform error.
- `expandvars` is dialect-sensitive in *input* rather than in platform
  behaviour: it expands `$NAME` and `${NAME}` in every dialect and additionally
  `%NAME%` under the `windows` dialect, which is the `dialect` argument
  [RFC 0017](0017-lexical-path-composition.md) defines. That argument is the
  mechanism this group consumes rather than redefines, and roadmap task 6.7.4
  lists step 6.6.1 as a prerequisite for exactly that reason.

Roadmap task 6.7.2's acceptance criterion is that the platform contract for
`same_file` and `mount` is stated in the guide and **exercised on Unix and
Windows continuous integration**. That is a real obligation rather than a
formality: a platform-divergent helper tested on one platform is a helper whose
divergence is untested, and the fallback this section permits is only
acceptable because the Windows path is exercised or explicitly refused.

### 5.6. Type and error contract

RFC 0006 sections 8.6 and 8.7 specify the argument shapes. What follows is when
a subject or an option value is rejected, each carrying a code from section 5.9.

| Helper        | Accepted subject | Rejects                                                                                                               |
| ------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------- |
| `exists`      | a string         | `wrong_kind`; `outside_capability`                                                                                    |
| `link_exists` | a string         | `wrong_kind`; `outside_capability`                                                                                    |
| `same_file`   | two strings      | `wrong_kind`; `outside_capability`; `missing_operand`; `identity_unavailable`                                         |
| `mount`       | a string         | `wrong_kind`; `outside_capability`; `unsupported_platform`                                                            |
| `expandvars`  | a string         | `wrong_kind`; `malformed_reference`; `non_utf8_value`; `unknown_missing_value`; `unknown_dialect`; `output_too_large` |
| `glob`        | a string         | `wrong_kind`; `outside_capability`; `match_limit`                                                                     |

Four decisions this group adds:

- **`outside_capability` is one code across five helpers, and it is not
  `missing_operand`.** `exists`, `link_exists`, `same_file`, `mount`, and
  `glob` all reach it, and all five mean the same thing by it: the path is not
  within the workspace the manifest was given. Keeping it distinct from "the
  thing is not there" is section 5.4's first decision expressed as a code, and
  it is why the group carries one capability code rather than folding it into a
  not-found path.
- **`identity_unavailable` and `unsupported_platform` are separate codes
  because they have separate remedies.** The first means the platform has a
  filesystem but no way to identify a file within it; the second means the
  platform's mount semantics are not implemented. A caller can act on the first
  by comparing paths deliberately and accepting the weaker guarantee; the
  second requires a different platform or a manifest change. Section 8.7
  requires both messages to name the platform, so both carry it as payload.
- **`expandvars`'s rejections are five, and `malformed_reference` is not a
  passthrough.** Section 8.6 states that "an unterminated `${` is an error, not
  a passthrough", which is the decision worth recording: a lenient expander
  that emitted the text unchanged would make a typo in a variable reference
  produce a literal `$` in a build command, and the failure would surface as a
  shell error some steps later rather than as a template error here.
- **`unknown_missing_value` enumerates its three values, and `wrong_kind`
  covers a non-string but not a non-boolean option.** `missing` accepts `error`,
  `empty`, or `preserve` and section 8.6 requires an unknown value to be "an
  error enumerating the three", so the message carries all three. The
  `files_only` option takes a boolean, and a truthy non-boolean is MiniJinja's
  ordinary coercion rather than this group's error — inventing a stricter rule
  for one option would make `glob` differ from every other optioned helper in
  the RFC set.

### 5.7. Canonical value equality

No additional obligation beyond RFC 0006 section 6.7. Clause 6.7 opens by naming
`subset`, `superset`, `contains`, and duplicate detection, and adds the
canonical-JSON domain those relations are defined over. Nothing in this group
keys a collection on a value: the four new tests return booleans, `expandvars`
returns a string, and `glob` returns a sequence whose order is already fixed by
section 5.3. No mapping is traversed and no deduplication occurs, so the
canonical key is never computed.

There is one place where equality is *decided*, and it is deliberately not the
clause's: `same_file` decides whether two paths denote the same object. That is
a filesystem relation rather than the canonical-JSON equality clause 6.7
establishes, and the two must not be conflated — a manifest that wants to know
whether two paths are textually equal has `==`, and one that wants to know
whether they are the same file has `same_file`, which is the answer `==` cannot
give and, in the hard-link case, the one that is actually wanted.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, and this group reaches **two** of its rows —
the input-length row, which every path argument meets, and the match-count row
by adoption — **plus two ceilings of its own** on the two members that
materialize: 100000 matches for `glob(files_only=false)` and 8 MiB of output for
`expandvars`. Each of the four is stated where it applies rather than folded
into one sentence, because three of them bound different things and the fourth
is the status quo.

The input-length row bounds a subject at 8 MiB, and a path supplied to `exists`,
`link_exists`, `same_file`, or `mount` arrives through the same template
evaluation as any other string, so it is bounded by the same ceiling. That is
the input row's ordinary operation rather than a bound this group adds: none of
the four allocates anything that grows with its input, each returns a boolean,
and the path is consumed by one capability-handled lookup.

`glob(files_only=...)` is the member the clause genuinely reaches, and it
reaches it because it *materializes*. The existing `glob` already collects its
matches into a `Vec<String>` before returning them, and section 8.7 leaves its
ordering and observability contracts unchanged. What the option changes is
which entries reach that collection, and the resource obligation runs in the
direction the new value opens.

`files_only=true` is the status quo, and its bound is **unchanged**: it adds no
traversal, because the check it relies on already runs per candidate, and it
returns the regular-file subset of the same match set. It carries no
cardinality ceiling today, and this group does not add one — section 12's "no
existing contract changes" promise covers a manifest that matches a very large
tree, so a ceiling introduced on the shipped spelling would break it.

`files_only=false` is the newly reachable path, and it is the one whose result
is larger, because it admits `NotAFile` entries as well as `Path` ones. **A
larger result is not by itself an unbounded one, but neither is it a bound**:
the result is limited only by the number of entries the pattern matches, and
that count is a property of the workspace rather than of the argument, so no
length of `pattern` limits it. The clause asks a materialized output to reject
unreasonable expansion *before* allocating, so the group applies a
**match-count ceiling** of 100000 to this path alone: the count is checked as
the collection grows, and exceeding it fails with `match_limit` before the
sequence is materialized, which is the discipline RFC 0016's section 5.8
requires of `regex_findall` for the same reason. The ceiling is the one row
this group *adds* rather than inherits, and it is confined to the value that
opens the larger result because only that value can be widened by an argument.
The existing skipped-entry accounting in `GlobSkippedEntries` is unchanged and
remains where an operator sees the rejections either way.

The clause imposes a second bound here, on *implementation* rather than on
size: the `false` path must reuse the same per-entry metadata check and the
same capability root, not a second traversal. A `false` implemented as an
additional `stat` pass, or as a second glob expression, would double the
syscall cost of an expansion and would be a resource regression in a change
whose entire argument is that it avoids a second glob. Roadmap task 6.7.3's "do
not introduce a second glob implementation" is the same constraint stated from
the design side.

`expandvars` is the second member the clause reaches, and it is the one member
of the group whose output is not bounded by its input. A variable reference is
replaced by the value the injected reader returns, so the output is bounded by
the input plus the total length of the environment values referenced; that
total is bounded by the process environment, which the operating system caps
and which **no manifest input can drive**. A multiplier bounded by the operand
count rather than by an argument is the shape RFC 0017's composing helpers also
carry, and it is not by itself an expansion this clause refuses. It is
nevertheless not a bound on the output, because the injected reader is under
the *host's* control rather than the manifest's and can return a value far
larger than the subject being expanded — so the group adds an **output
ceiling** of 8 MiB, checked before the expanded string is materialized, and
rejects an expansion that would exceed it with `output_too_large`. Naming both
the multiplier and the ceiling is the honest discharge: the first is why the
multiplier is not manifest-driven, and the second is why the result is bounded
anyway.

The four tests reach no row of table 3. Each consumes a path bounded by the
input row, allocates nothing that grows with it, and returns a boolean.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `HostStateError`, and exactly
one `impl From<HostStateError> for minijinja::Error`, per clause 6.9. Every
message is a Fluent key and every error carries a machine code, so the codes
are enumerated rather than described.

| Condition                            | Code                                          |
| ------------------------------------ | --------------------------------------------- |
| subject of the wrong kind            | `netsuke::jinja::path::wrong_kind`            |
| path outside the capability boundary | `netsuke::jinja::path::outside_capability`    |
| `same_file` operand does not exist   | `netsuke::jinja::path::missing_operand`       |
| file identity cannot be determined   | `netsuke::jinja::path::identity_unavailable`  |
| mount semantics unimplemented        | `netsuke::jinja::path::unsupported_platform`  |
| malformed `$` reference              | `netsuke::jinja::path::malformed_reference`   |
| non-UTF-8 environment value          | `netsuke::jinja::path::non_utf8_value`        |
| unknown `missing` value              | `netsuke::jinja::path::unknown_missing_value` |
| unknown `dialect` value              | `netsuke::jinja::path::unknown_dialect`       |
| `glob` match count exceeded          | `netsuke::jinja::path::match_limit`           |
| `expandvars` output too large        | `netsuke::jinja::path::output_too_large`      |

Each code's Fluent key is its reason in upper snake case under `STDLIB_PATH_`,
per clause 6.9's `keys::STDLIB_<MODULE>_<CONDITION>` form, so
`outside_capability` pairs with `STDLIB_PATH_OUTSIDE_CAPABILITY`.

Three decisions this group adds:

- **The module segment is `path`, and it is the existing module.** RFC 0017's
  section 5.9 states the rule: the segment names the module the helpers live in.
  `src/stdlib/path/` already holds the `is <kind>` file tests —
  `file_type_matches` in `fs_utils.rs` is what `register.rs`'s `FILE_TESTS`
  loop calls — and it already keys filesystem failures `stdlib.path.*` under
  `keys::STDLIB_PATH_*`, including the `io` family carrying `not_found`/
  `permission_denied`/`invalid_input`. `exists`, `link_exists`, `same_file`, and
  `mount` are the same kind of operation as `is file` and `is symlink`: a
  metadata lookup on a path, through the capability handle. Putting them
  anywhere else would separate the new tests from the seven that already do
  exactly this, and would give `file_type_matches`'s existing `io_to_error`
  path a second, parallel spelling for the same failures. The purity boundary
  is recorded in the registry's own column, where the contract reads it, rather
  than in the diagnostic namespace, where it would have to be inferred.
- **`expandvars`'s non-UTF-8 error matches `env` in behaviour but not in
  key, and clause 6.9 is why.** Section 8.6 requires it to match "the existing
  `env` function", and that helper is not an RFC 0006 helper: it is registered
  by `register_env_function` in `src/manifest/registration.rs` and keys its
  rejection as `keys::MANIFEST_ENV_INVALID_UTF8` → `manifest.env.invalid_utf8`.
  Clause 6.9 is scoped to this RFC's own helpers — "every user-facing message
  is a Fluent key under `stdlib.<module>.<condition>`" — so a `stdlib` helper
  reusing a `manifest.*` key would break the clause it is meant to discharge.
  The match §8.6 asks for is that both reject rather than substitute silently,
  which `stdlib.path.non_utf8_value` delivers; the two keys name the same
  condition in the two namespaces each helper's clause requires.
- **`unknown_dialect` is genuinely shared with RFC 0017 rather than
  coincidentally spelled the same.** `expandvars` takes the `dialect` argument
  RFC 0017 defines, so the rejection is the same failure reached through the
  same parser: one code, `netsuke::jinja::path::unknown_dialect`, and one
  `STDLIB_PATH_UNKNOWN_DIALECT` key, extending that helper's message rather
  than duplicating it. This is the reuse section 5.10's naming clause permits —
  it is one capability with one name, not an alias — and it is why the two
  children can share a key without either owning the other.
- **Both platform-naming codes carry the platform as payload.** Section 8.7
  requires `identity_unavailable`'s message to name the platform and
  `unsupported_platform`'s to name "the platform and the capability", so both
  render it rather than asserting only that they could not decide. This follows
  the pattern RFC 0017's section 5.9 established for codes carrying a payload
  rather than a scalar.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds five, none an alias. The group carries
one substantive naming decision, and it is a collision-avoidance one.

`mount` is registered as a test, and its name is a noun where every other
member of the test family is a predicate. RFC 0006 section 8.7 keeps it:
Ansible has no equivalent, the RFC's own `path is mount` grammar reads as the
question it answers, and a `is_mount` spelling would break the pattern the rest
of the group follows where the test name is the property being asserted. The
alternative that matters is not a longer name but a *different* name —
`is_mount` or `mount_point` — and both were declined because the test namespace
already supplies the "is" and the argument already supplies the "what".

`glob`'s `files_only` is named for what it selects rather than for what it
excludes. `regular` and `no_dirs` describe the same filter from the other side;
`files_only` reads as the option a caller wants when the result is being used
as a file list, and section 8.7's own text uses it. The rejected alternative is
worth naming because it is the one an Ansible author will try: `fileglob`,
which section 8.7 rejects along with the rest of the second-glob family.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. Each of the five new
helpers and the one optioned function requires all seven obligations before its
roadmap task is complete: a guide entry, a `tested-example` fence, unit tests
covering accepted kinds, rejected kinds, boundary values, and every enumerated
option value, a property test where an invariant exists, an integration test
through a manifest, an integration test for the manifest-query disposition, and
an inventory row.

The property tests this group owes are the ones its invariants support, and
they are worth naming because the group's invariants are relational rather than
algebraic:

- **`exists` and `link_exists` disagree exactly on dangling links.** For every
  path, `link_exists` is true whenever `exists` is false *and* the path is a
  dangling symbolic link, and the two agree on every path that is not one. This
  is the pair's whole purpose and it is falsifiable by a single fixture.
- **`same_file` is reflexive on every existing path.** A path compared with
  itself is true wherever identity is determinable, which catches an
  implementation that special-cases the two-operands-differ branch.
- **`glob(files_only=true)` is a subsequence of `glob(...)`.** The filtered
  result preserves the unfiltered result's order and contains no entry the
  unfiltered call did not return. This is the property that makes roadmap task
  6.7.3's "leave the ordering contract unchanged" checkable rather than
  asserted.

The platform suite roadmap task 6.7.2 requires is the group's load-bearing test
rather than an extra: `same_file` and `mount` are exercised on Unix and Windows
continuous integration, and the guide states each one's platform contract. A
helper whose divergence is untested on one of its two platforms is a helper
whose fallback is unverified.

The disposition suite is this group's second obligation, and it is the one the
RFC set has not yet had to write: **every helper in this group must fail during
a manifest query**, so the test asserts a stub for all six rows rather than
asserting a registration for some. The inventory row for each names the
namespace, the registration, and the exclusion, so the exclusion cannot be
removed without the inventory disagreeing.

### Clause discharge

| Clause | Discharge                                                                                                                                                                                       |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `6.1`  | Five `New` helpers and one `Option added`; none is pure, so the group contributes 0 of 52 and the six written children remain at 41.                                                            |
| `6.2`  | All six register stubs in `register_disabled_query_helpers`; `expandvars` is excluded exactly as `env` is, and `glob`'s registration is unchanged.                                              |
| `6.3`  | Four tests return booleans; `expandvars` substitutes in one left-to-right pass; `files_only` filters the existing result rather than reordering it.                                             |
| `6.4`  | Every path resolves through the injected `cap_std` handle; a path outside it errors; `expandvars` reads the injected environment reader.                                                        |
| `6.5`  | Platform is uniform for four helpers, identity-based for `same_file`, divergent only in `mount`, and dialect-sensitive in `expandvars`'s input.                                                 |
| `6.6`  | Eleven rejected conditions with their own codes; `outside_capability` is distinct from not-found; both platform codes name the platform.                                                        |
| `6.7`  | No value is keyed or deduplicated; `same_file`'s identity relation is a filesystem fact, not the canonical key.                                                                                 |
| `6.8`  | Table 3's input row bounds every path; `files_only=false` adds a 100000-match ceiling checked before materialization; `expandvars` adds an 8 MiB output ceiling checked before materialization. |
| `6.9`  | One enum, one `From` impl, eleven `netsuke::jinja::path::*` codes extending the existing module namespace, with `unknown_dialect` shared with RFC 0017 and `match_limit` shared with RFC 0016.  |
| `6.10` | Five new names, none an alias; `mount` keeps the noun spelling the test namespace supplies the "is" for; `fileglob` stays rejected.                                                             |
| `6.11` | Five guide entries, five `tested-example` fences, the dangling-link and subsequence laws, the two-platform suite, and the all-stubs disposition suite.                                          |

## 6. Dependencies

**No new dependency.** The filesystem work uses `cap_std`, which is already a
normal dependency and is the capability handle ADR-008's taxonomy and RFC 0006
section 6.4 require; the path text is `camino`'s `Utf8Path`, as in RFC 0017.
Mount-point detection needs no crate: the Unix form compares `st_dev` with the
parent's, which `cap_std`'s metadata already exposes, and the Windows form
reads the volume root, which the platform API provides. A crate that answered
"is this a mount point" would be a second implementation of rules RFC 0006
section 8.7 already states, and it would have to be trusted on two platforms
where the RFC specifies both.

Within the RFC set, the group requires the shared contract that RFC 0006
section 14.1's "slice 0" describes, which roadmap steps 6.1.2 and 6.1.3
deliver. It requires no other child RFC for the filesystem tests, and it
requires [RFC 0017](0017-lexical-path-composition.md) for `expandvars`, because
the environment-observing helper takes the same `dialect` argument that RFC
defines — so the mechanism lands there and is consumed here. Roadmap task 6.7.4
lists step 6.6.1 as a prerequisite for exactly this reason. The group is
otherwise the terminal child of the pure/observing seam: nothing requires it,
because it is where the observing half ends.

## 7. Delivery

Roadmap step 6.7, which implements this RFC in four tasks:

- 6.7.1. The `exists` and `link_exists` tests, routed through the capability
  handle, with a path outside it erroring rather than reporting `false`.
- 6.7.2. The `same_file` and `mount` tests, comparing file identity and
  qualifying `mount` explicitly per platform.
- 6.7.3. The `files_only` option on the existing `glob`, leaving the capability
  scoping, ordering, and observability contracts unchanged.
- 6.7.4. `expandvars` through the injected environment reader, with `missing`
  defaulting to `error` and malformed references rejected.

Each task carries the acceptance criteria that make this RFC checkable: a
dangling symbolic link is `false` for `exists` and `true` for `link_exists`;
the platform contract for `same_file` and `mount` is stated in the guide and
exercised on Unix and Windows continuous integration; an unsupported platform
errors rather than returning a plausible `false`; and no leaf helper reads the
environment ambiently, with the manifest-query stub explaining the restriction.

## 8. Open questions

RFC 0006 section 16 assigns **no question to this group**, and that is a
consequence of the partition rather than an omission. Questions 1 and 4 belong
to RFC 0013, question 3 to RFC 0016, question 2 to RFC 0017, and question 6 to
RFC 0019; question 5 concerns the shared bounds of roadmap step 6.1, and
question 7 is resolved by roadmap task 7.1.1. This group is the first child
with no allocation at all, for the same reason it is the first with no pure
helper: the questions attach to specific helpers, and none of the seven section
16 names is in this group.

The decision a reader would expect here is not a section 16 question but a
**section 8.7 clause fallback**, and it is worth stating in those terms: the
RFC permits a Unix-only `mount` with an explicit unsupported-platform error on
Windows, "if the Windows semantics cannot be specified crisply during
implementation". Section 16 does not ask this because section 8.7 already
answers it — the fallback is prescribed, with the condition attached — so what
remains is an implementation-time judgement rather than an open question.

**This RFC carries that judgement unresolved and records the three options and
their consequences.** The choice is manifest-visible and cheap to make late,
which is why it is recorded here rather than settled in advance.

- **Implement both platforms.** `mount` answers on Unix by device-identifier
  comparison and on Windows by volume root or mounted folder. The cost is that
  the Windows semantics must be specified crisply enough to implement and test
  in continuous integration, which is the condition section 8.7 attaches to
  this option.
- **Unix-only with an explicit unsupported-platform error.** Section 8.7's own
  permitted fallback, recorded in the guide. The cost is a helper that is
  absent on one platform, which is a smaller cost than a helper that answers
  wrongly — but it means the group's "platform enters through two explicitly
  named divergences" claim is one divergence plus one refusal.
- **Defer `mount` to a later RFC.** Keep the four other helpers and move
  `mount` out of this group. The cost is that the partition in RFC 0006 section
  14.13's coverage map would need amending, and the map's rows are asserted to
  partition the accepted set, so this is the only option that changes a
  contract rather than a document.

Roadmap task 6.7.2 resolves it before the test registers. The recommendation of
this RFC, for the record rather than as a decision, is the second option if the
Windows semantics resist specification during implementation and the first
otherwise: section 8.7 sequenced `mount` last within its slice precisely so
that this decision could be made with the other four already landed, and an
explicit refusal is a better failure than a plausible `false`.

**A second decision sits outside section 16 for a different reason** — it is
not a question RFC 0006 asks but a conflict between what RFC 0006 states and
what the shipped code does. It is recorded rather than discharged here because
it is between the survey and the implementation, not within this RFC.

RFC 0006 specifies `glob(pattern, files_only=false)` — `false` is the default —
and its section 12 states that the three optioned helpers gain "optional
arguments with behaviour-preserving defaults". For `basename` and `dirname`
that holds. For `glob` it does not: `glob_paths` in `src/manifest/glob/mod.rs`
filters directories out unconditionally, through the capability-scoped metadata
check in `walk.rs`'s `names_a_file`, and `GlobEntry::NotAFile` is recorded as a
skipped entry rather than returned. A directory has never reached a template.
`files_only=false` is therefore **not** the shipped behaviour; it is behaviour
that does not exist yet, and shipping it as the default would add directories
to every existing `glob()` result.

The specification and the code cannot both be right, and this RFC does not
silently pick one. Three readings are available, and the second is this RFC's
recommendation for the reason given:

- **Follow RFC 0006 literally.** `false` is the default, so `glob()` begins
  returning directories and every manifest relying on the current filtering
  changes meaning. RFC 0006 section 12's own "no existing contract changes"
  sentence is then false, and the guide needs a migration note.
- **Keep the shipped default and read `false` in section 8.7's signature as an
  error.** Register `files_only=true` as the default and the only
  behaviour-preserving choice; `false` is opt-in and newly available. This
  contradicts section 8.7's signature text while satisfying section 12's stated
  intent — that no shipped manifest changes meaning — and it is the reading
  that keeps `glob` consistent with the other two optioned helpers.
- **Amend RFC 0006 section 8.7** to spell the default `true` and correct
  section 12's claim. This is the only option that leaves the survey and the
  children consistent, and it is a documentation change to an accepted RFC
  rather than a code change.

Roadmap task 6.7.3 decides, before the option registers. What this RFC fixes is
that the decision is **manifest-visible**: under the first reading, a manifest
that globs a directory and passes the result to a command begins passing
directories to it, which is a silent behaviour change of exactly the kind RFC
0006 section 12 promises not to make. Recording it here means it is settled by
the task that can test it, rather than discovered by a user. Sections 5.3 and
5.8 are written so that both readings hold: the resource bound of `false` is
stated separately from the unchanged bound of `true`.

The choice also decides whether section 5.8's new match-count ceiling can ever
fire under the shipped default. If 6.7.3 adopts the first reading, `false`
becomes what an existing `glob()` call gets, and the ceiling applies to those
calls; if it adopts the second or third, the ceiling is reached only by a
manifest that opts in. The ceiling itself is owed either way, because the
*wider* result is what the clause's materialization test turns on, but the task
should record which reading it chose so a user meeting `match_limit` knows
whether it was reachable from a call they did not change.

## 9. Recommendation

This group should be implemented sixth, at v0.1.x or later, after RFC 0017.

The case for the group is that it is where the capability contract stops being
a claim and becomes testable. RFC 0006 section 6.4 says every filesystem touch
goes through the injected handle; a set of helpers that only reads filesystem
metadata is the smallest surface on which that can be exercised, and the
`outside_capability` error is the clause's first genuinely falsifiable
consequence. The step's own summary names this as its outcome: existence
probing and environment expansion added "without a second ambient-authority
path and without any helper silently disappearing from a manifest query".

The case for the split at the purity seam, rather than at the section 8
boundary, is RFC 0006 section 14.13's and RFC 0017's, and this RFC is where
that choice pays: it is the one child that is uniformly non-pure, so its
registry's purity column is a constant, its manifest-query disposition is a
stub for every row, and the coverage contract can assert its purity
contribution as a hard zero rather than as a budget. A child that mixed the two
classes would have neither property.
