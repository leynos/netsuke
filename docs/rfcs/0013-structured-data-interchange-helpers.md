# RFC 0013: Structured data interchange helpers

## Preamble

- **RFC number:** 0013
- **Status:** Proposed
- **Created:** 2026-09-24
- **Parent RFC:** [RFC 0006, Ansible-inspired template standard-library
  expansion](0006-ansible-inspired-template-standard-library.md)
- **Roadmap step:** 6.2
- **Originating issue:** [#596](https://github.com/leynos/netsuke/issues/596)
  (closed)
- **Release target:** v0.1.x or later; must not widen the v0.1.0 hardening
  release defined by [#594](https://github.com/leynos/netsuke/issues/594)

## 1. Summary

This RFC specifies the structured data interchange group: `from_json`,
`from_yaml`, `from_yaml_all`, `to_yaml`, and `to_nice_json`. Together they let
a manifest read the metadata `cargo metadata` emits, a compiler's JSON output,
a YAML package manifest, or a generated configuration fragment, and write a
fragment back, without invoking `jq`, `yq`, or a scripting runtime. All five
are pure, so all five are available to manifest queries as well as to target
recipes, and none needs a capability handle. The group is the first slice RFC
0006 section 14.2 sequences, because the remaining groups read their inputs in
the forms it produces.

## 2. Problem

A manifest that needs one field from a compiler's JSON metadata has, today, one
route: `shell()` out to `jq`, `yq`, Python, or Ruby and capture the output. RFC
0006 section 2 records the cost. Each such call converts a pure, cacheable,
capability-free planning expression into a subprocess with ambient authority,
imports a host dependency that the build description never declared, and adds
an escaping surface between the tool's output and the template that consumes
it. The same applies in reverse: a manifest that generates a configuration
fragment has no way to emit it but to interpolate text with no guarantee about
quoting, so a value reading `no` can silently become a YAML boolean.

The contortion is not incidental to build manifests; it is the ordinary case.
Compiler metadata is JSON, package manifests and generated configuration are
YAML in most of the ecosystems Netsuke targets, and Rust's own `cargo metadata`
is JSON. Section 15.2 considers and rejects adding nothing here, on the grounds
that it leaves `netsuke help targets` unable to answer questions it should be
able to answer purely.

This group addresses the JSON and YAML halves. A TOML package manifest is
outside it: RFC 0006 accepts no TOML parser, and Cargo metadata is available as
JSON without one, so a manifest that needs it reads
`cargo metadata --format-version 1` through `fetch` rather than parsing
`Cargo.toml` directly.

## 3. Goals and non-goals

- Goals:
  - Read one JSON document into native MiniJinja values, preserving object
    order, and reject a duplicate key rather than let the last value win.
  - Read one YAML document, or a multi-document stream materialized in full,
    over the `serde-saphyr` stack [ADR-001](../adr-001-replace-serde-yml-with-serde-saphyr.md)
    already adopts.
  - Serialize deterministically to block-style YAML and to pretty-printed JSON,
    with quoting that cannot be read back as another type.
  - Make both round trips hold under RFC 0006 section 6.7 canonical equality, as
    property tests rather than examples.
- Non-goals:
  - `to_json`: rejected in RFC 0006 section 7 as redundant with MiniJinja's
    `tojson`, which remains the compact serializer.
  - `to_nice_yaml`: rejected in RFC 0006 section 10.2 as differing from
    `to_yaml` only in a default indent. Whether that rejection is expressed as
    outright absence or as a diagnostic-raising registration is carried as open
    question 1.
  - `from_ini`, `from_csv`, and every other parser for a format no consumer has
    named. RFC 0006 section 9 defers by evidence bar, and none of these has one.
  - Ansible's loader behaviour: unsafe strings, vault tags, and the order-
    dependent duplicate-key tolerance RFC 0006 section 13.3 lists.
  - Merge keys (`<<`), which are ambiguous with `combine`'s explicit list policy
    and recursion flag, and are rejected rather than merged.
  - A lazy iterator over a YAML stream. `from_yaml_all` materializes.

## 4. Capability set

Five helpers, all filters, all pure, all newly registered.

- `from_json` — parse one JSON document into native values. Contract in
  [RFC 0006 section 8.1](0006-ansible-inspired-template-standard-library.md#81-structured-data-interchange).
- `from_yaml` — parse exactly one YAML document into native values. Contract in
  the same subsection.
- `from_yaml_all` — parse a multi-document YAML stream into a materialized
  sequence. Contract in the same subsection.
- `to_yaml` — serialize a value as deterministic block-style YAML. Contract in
  the same subsection.
- `to_nice_json` — pretty-print a value as JSON. Contract in the same
  subsection.

This section does not restate any contract. Each helper's kinds, options,
rejection conditions, and bounds are specified in RFC 0006 section 8.1, and
what follows in section 5 is how this group meets the cross-cutting clauses
rather than what the helpers do.

## 5. Cross-cutting contract conformance

This section discharges RFC 0006 section 6 for the five helpers section 4
lists. Where a clause's consequence follows from RFC 0006 alone and is the same
for every group, the subsection says so in one line; where this group forces a
decision, the subsection states the decision.

### 5.1. Registry

| Helper          | Namespace | Registration | Purity class | Manifest query |
| --------------- | --------- | ------------ | ------------ | -------------- |
| `from_json`     | Filter    | New          | Pure         | Yes            |
| `from_yaml`     | Filter    | New          | Pure         | Yes            |
| `from_yaml_all` | Filter    | New          | Pure         | Yes            |
| `to_yaml`       | Filter    | New          | Pure         | Yes            |
| `to_nice_json`  | Filter    | New          | Pure         | Yes            |

All five rows are `New`: this group introduces five helpers and extends none.
All five are pure, so all five fall inside RFC 0006 section 6.1's 52, and this
RFC accounts for 5 of them.

### 5.2. Manifest-query availability

All five are pure, so `from_json`, `from_yaml`, `from_yaml_all`, `to_yaml`, and
`to_nice_json` all register in `register_query_helpers`, and none registers in
`register_disabled_query_helpers` as a stub. The consequence is that
`netsuke help targets` gains five working helpers and no new always-failing
stub. One caveat, carried rather than resolved here: if RFC 0006 section 16
question 1 is answered by registering `to_nice_yaml` solely to raise a
diagnostic naming `to_yaml(indent=4)`, that registration introduces no accepted
helper and so is not a section 5.1 row; it is decided in section 8.

### 5.3. Determinism

Two group-specific obligations follow from clause 6.3, and both are testable
without reading prose.

- **Key order is an output, not a side effect.** `from_json` preserves object
  order because `serde_json` is built with `preserve_order`; `from_yaml`
  preserves mapping order; `to_yaml` and `to_nice_json` emit insertion order
  unless `sort_keys=true`. A round trip therefore returns a mapping in the
  order it went in, and the tests assert order, not merely equality.
- **Trailing-newline behaviour is a contract, not a convention.** `to_yaml`
  ends with exactly one trailing newline and `to_nice_json` with none, so one
  composes as a whole document and the other composes inside a larger one. The
  asymmetry is deliberate and is pinned by a test at each end.

### 5.4. Capability boundary

No additional obligation beyond RFC 0006 section 6.4. The clause's substantive
rules all address helpers that reach outside their arguments, and no helper in
this group does: none takes a `cap_std` handle, none takes an injected reader,
and none can violate the clause's trapdoor rule about a filesystem predicate
reporting `false` for an out-of-scope path.

### 5.5. Platform contract

No additional obligation beyond RFC 0006 section 6.5. The clause's obligations
attach to helpers whose behaviour varies by platform or that parse another
platform's syntax: no helper here takes a `dialect` argument, none can fail
with a platform diagnostic, and all five emit LF on every platform.

### 5.6. Type and error contract

RFC 0006 section 8.1 specifies the kinds each helper accepts. What follows is
the conditions under which a kind, a key, or an option value is rejected, each
carrying a code from section 5.9. Per helper, because the three parsing helpers
and the two serializers do not share a rejection set.

- `from_json` accepts a string. It rejects `wrong_kind`, `syntax`,
  `duplicate_key`, `depth_exceeded`, and `length_exceeded`.
- `from_yaml` accepts a string. It rejects every `from_json` condition, and adds
  `unsupported_key` for a sequence or mapping key, `special_tag`, `merge_key`,
  `alias_budget`, and `document_count` for a stream that is not exactly one
  document.
- `from_yaml_all` accepts a string and applies every per-document condition from
  `from_yaml` to each document. `document_count` is the one condition it does
  **not** inherit: RFC 0006 section 8.1 makes a stream of zero documents an
  empty sequence, not an error, and multi-document input is the whole point of
  the helper. The input-length and node budgets apply to the whole stream
  rather than to each document.
- `to_yaml` accepts any value except undefined. It rejects `undefined_input`
  and `indent_out_of_range` outright, plus `unsupported_key` when
  `sort_keys=true` meets a mapping key with no canonical JSON form, and
  `unsupported_kind` for a value that has none.
- `to_nice_json` accepts any value except undefined, and rejects the same four
  conditions as `to_yaml`, with the difference that section 8.1 states its key
  rule directly: integer and boolean keys are rendered in canonical string form
  and every other key kind is rejected rather than coerced.

Three decisions this group adds:

- **`none` is accepted; undefined is not.** Clause 6.6 makes undefined an error
  and makes `none` a value. Every helper here follows that split, and the line
  between them is where a manifest author is most likely to be surprised, so
  each of the five documents it explicitly rather than leaving the clause to
  imply it.
- **A duplicate key is rejected with a position.** `from_json` and `from_yaml`
  name the duplicated key and the offset of its second occurrence. This is the
  one place the group adds detection the underlying parser does not perform,
  and the reason is that last-key-wins is a silent data-loss trapdoor in a
  build manifest rather than a tolerable convenience.
- **The two `indent` ranges differ on purpose.** `to_yaml` accepts 1 to 8 and
  `to_nice_json` accepts 0 to 8. Zero is meaningful for JSON, where it means
  compact output, and meaningless for block YAML, where it would mean no
  indentation at all. Both reject an unknown value with an error enumerating
  the accepted range, satisfying clause 6.6's enumerated-option rule and
  roadmap task 3.15.5.

### 5.7. Canonical value equality

Clause 6.7 governs `to_yaml`'s and `to_nice_json`'s `sort_keys=true` and
nothing else in this group. The obligation here is to say which of the clause's
exclusions this group can meet, and to fix how the round trips are stated.

- `sort_keys=true` sorts mapping keys by their RFC 8785 canonical key, so two
  mappings that differ only in insertion order serialize identically. That is
  clause 6.7's relation applied to a key-ordering decision, and it is why the
  option can promise deterministic output at all.
- The clause excludes undefined, callables, and the `now()` timestamp object,
  because none has a canonical JSON form. Undefined is already rejected on
  input by clause 6.6; the other two are rejected on output as
  `unsupported_kind`, because a manifest can hold a callable or the result of
  `now()` and pass it to a serializer, and clause 6.7 makes that a typed error
  naming the value kind rather than a silent rendering.
- The group defines **no second equality relation** for round-trip testing.
  `value | to_yaml | from_yaml` and `value | to_nice_json | from_json` are
  asserted equal under clause 6.7's relation. A looser relation for tests would
  make the property tests agree with an implementation the clause does not
  describe.

### 5.8. Resource bounds

The bounds are RFC 0006 table 3's, applied through checked comparison before
allocation. What this group adds is where each one is enforced, because the
JSON and YAML parsers do not share a code path.

| Helper          | Bounds enforced                                      |
| --------------- | ---------------------------------------------------- |
| `from_json`     | input 8 MiB; nesting depth 128                       |
| `from_yaml`     | input 8 MiB; depth 128; alias expansion 100000 nodes |
| `from_yaml_all` | input 8 MiB; depth 128; alias expansion 100000 nodes |
|                 | — the same three, applied to the stream as a whole   |
| `to_yaml`       | none; output is a function of a bounded input        |
| `to_nice_json`  | none; output is a function of a bounded input        |

Three consequences this group decides:

- **`from_yaml_all`'s budget is stream-wide.** A stream of many small documents
  is bounded in total, so a caller cannot evade the input ceiling by splitting
  an expansion bomb across document boundaries. The diagnostic reports the
  stream total rather than a per-document figure, which is what makes the bound
  auditable after the fact.
- **The alias budget is the one bound that can fail the implementation slice
  rather than the input.** Clause 6.8 requires the expansion to be rejected
  before allocating, and if `serde-saphyr` cannot bound alias expansion then
  this RFC rejects aliases outright and records that in the guide, per RFC 0006
  section 8.1 and roadmap task 6.2.2. That choice is carried as section 8's
  open question 4 and is not resolved here.
- **The serializers enforce nothing, and that is a decision rather than an
  omission.** Neither allocates proportionally to anything but its input, so a
  bound would reject documents a parser had already accepted. The row reads
  "none" instead of being left blank so that a reviewer sees the absence was
  chosen.

### 5.9. Diagnostics and localization

The group defines one private domain error enum, `InterchangeError`, and
exactly one `impl From<InterchangeError> for minijinja::Error`, per clause 6.9.
Every message is a Fluent key and every error carries a machine code. The codes
are this group's contribution to clause 6.9's policy, so they are enumerated
rather than described.

| Condition      | Code                                               |
| -------------- | -------------------------------------------------- |
| not a string   | `netsuke::jinja::interchange::wrong_kind`          |
| syntax         | `netsuke::jinja::interchange::syntax`              |
| duplicate key  | `netsuke::jinja::interchange::duplicate_key`       |
| key kind       | `netsuke::jinja::interchange::unsupported_key`     |
| special tag    | `netsuke::jinja::interchange::special_tag`         |
| merge key      | `netsuke::jinja::interchange::merge_key`           |
| alias budget   | `netsuke::jinja::interchange::alias_budget`        |
| document count | `netsuke::jinja::interchange::document_count`      |
| depth          | `netsuke::jinja::interchange::depth_exceeded`      |
| length         | `netsuke::jinja::interchange::length_exceeded`     |
| undefined      | `netsuke::jinja::interchange::undefined_input`     |
| indent         | `netsuke::jinja::interchange::indent_out_of_range` |
| value kind     | `netsuke::jinja::interchange::unsupported_kind`    |

Each code's Fluent key is the code's reason in upper snake case under
`STDLIB_INTERCHANGE_`, so `wrong_kind` pairs with
`STDLIB_INTERCHANGE_WRONG_KIND` and `indent_out_of_range` with
`STDLIB_INTERCHANGE_INDENT_OUT_OF_RANGE`, per clause 6.9's
`keys::STDLIB_<MODULE>_<CONDITION>` form.

The module segment is `interchange` rather than `json` or `yaml`, because one
enum serves both parsers and both serializers and the code names the capability
group, not the syntax. All five helpers — `from_json`, `from_yaml`,
`from_yaml_all`, `to_yaml`, and `to_nice_json` — reach their errors through
this enum: every `Error::new` call in the group's leaf functions is replaced by
a variant of it, so a caller can tell an interchange failure from a manifest
diagnostic by the code alone. Clause 6.9 rejects ad hoc construction at this
scale, and a group with thirteen conditions is the case it names.

### 5.10. Naming and alias policy

No additional obligation beyond RFC 0006 section 6.10. The clause registers one
name per capability and this group adds five, none an alias. The group does,
however, put the clause under real pressure in a way no other group does, and a
reviewer should see why the outcome is still five names rather than six or four.

`to_json` is **rejected** by RFC 0006 section 10.2 as an alias of MiniJinja's
existing `tojson`, while `to_nice_json` is **accepted** in section 8.1. The
obvious reading — that `to_nice_json` is the pretty-printer for a helper this
set does not add — is wrong, and the reason is worth stating once. `tojson` and
`to_nice_json` differ in *kind*, not in degree: `tojson` is compact and
whitespace-free by construction, and `to_nice_json(indent=…)` is the same
serialization with a layout parameter `tojson` does not accept. A parameterless
`to_json` would therefore be a true alias and is correctly rejected, whereas
`to_nice_json` carries an argument surface that makes it a distinct capability.
The near-miss is that `indent=0` *does* reproduce `tojson` exactly, so the two
overlap at one point of that parameter space. That overlap is deliberate and
harmless — it is what makes `indent=0` a usable "compact, but reachable through
the Netsuke helper" path — but it is the reason clause 6.10's one-name-per-
capability line cannot be read as one-*output*-per-capability.

The clause's other live case is unsettled rather than resolved: `to_nice_yaml`
is rejected by RFC 0006 section 10.2 as redundant with `to_yaml(indent=...)`,
and whether that rejection is expressed as outright absence or as a
diagnostic-raising registration is RFC 0006 section 16 question 1, carried
unresolved to section 8 and decided at roadmap task 6.2.3. Note that the
asymmetry with JSON is intended and not an inconsistency: `to_nice_yaml`'s
redundancy is with `to_yaml`, a helper this standard library *already* adds, so
the rejection costs a caller nothing it cannot already reach; `to_json`'s
redundancy is with a MiniJinja builtin that is likewise already reachable.

### 5.11. Documentation and testing obligations

No additional obligation beyond RFC 0006 section 6.11. The clause's seven
obligations apply unmodified. One group-specific note rather than a
restatement: the round-trip property tests clause 6.11.4 requires are the two
named in section 5.7 under clause 6.7 canonical equality, and the
serialization-determinism property it names is the same proposition as section
5.3's key-order requirement, tested from the other side.

### Clause discharge

| Clause | Discharge                                                                                               |
| ------ | ------------------------------------------------------------------------------------------------------- |
| `6.1`  | Five pure `New` helpers; the five section 5.1 rows are 5 of 52.                                         |
| `6.2`  | All five pure, so all register in `register_query_helpers`, none stubbed.                               |
| `6.3`  | Mapping order in and out; one trailing newline for `to_yaml`, none for `to_nice_json`.                  |
| `6.4`  | No filesystem, environment, or subprocess access; no handle taken.                                      |
| `6.5`  | No `dialect` argument; all five emit LF everywhere.                                                     |
| `6.6`  | Undefined rejected, `none` accepted; duplicates rejected positionally; both `indent` ranges enumerated. |
| `6.7`  | `sort_keys` sorts by canonical key; both round trips under canonical equality.                          |
| `6.8`  | Table 3's input and depth bounds, stream-wide for `from_yaml_all`, plus the alias budget.               |
| `6.9`  | One enum, one `From` impl, thirteen `netsuke::jinja::interchange::*` codes.                             |
| `6.10` | Five new names, no alias family, none reused across namespaces.                                         |
| `6.11` | The clause's seven obligations, plus the two round trips and the determinism property.                  |

## 6. Dependencies

No new crate. This group is the one RFC 0006 section 13.4 adds nothing for: it
uses `serde_json` with `preserve_order` for the JSON half and the existing
`serde-saphyr` stack for the YAML half, both of which Netsuke already carries,
plus `serde_json_canonicalizer` for `sort_keys=true`'s canonical key. Adding no
dependency is why RFC 0006 section 14.2 sequences this slice first.

Within the RFC set it requires the shared contract RFC 0006 section 14.2's
"slice 0" describes, which roadmap steps 6.1.3 and 6.1.4 deliver: the
bounded-parser helper the two parsers share. It requires no other child RFC,
and no other child RFC requires it, which is why it is the group `EP-M3`'s hard
go/no-go is spent on.

## 7. Delivery

Roadmap step 6.2, which implements this RFC in three tasks:

- 6.2.1. `from_json`, with duplicate-key rejection and source offsets.
- 6.2.2. `from_yaml` and `from_yaml_all`, over the existing safe YAML stack.
- 6.2.3. `to_yaml` and `to_nice_json`, with pinned output.

Each task carries the acceptance criteria that make this RFC checkable: the
duplicate-key diagnostic naming the key and its second offset, the
alias-expansion bomb failing with a bounded-resource diagnostic rather than
exhausting memory, and the two round-trip property tests together with the YAML
1.1 `yes`, `no`, `on`, and `off` spellings being unable to reach a generated
file unquoted.

## 8. Open questions

Two of RFC 0006 section 16's questions are assigned to this group and are
carried unresolved rather than answered here.

- **Question 1: is rejecting `to_nice_yaml` correct?** RFC 0006 section 10.2
  rejects it as redundant with `to_yaml(indent=4)`. The counter-argument is
  discoverability: an Ansible-literate author will reach for the name and
  MiniJinja's "unknown filter" error will not help them. The intermediate
  option is to register it solely to raise a typed diagnostic naming
  `to_yaml(indent=4)`. This RFC does not settle it; roadmap task 6.2.3 does,
  before the serializer registers. Section 5.2 records the consequence either
  way: the diagnostic-raising registration introduces no accepted helper, so it
  is not a section 5.1 row.
- **Question 4: can `serde-saphyr` bound alias expansion?** RFC 0006 section
  8.1 requires either a bounded expansion budget or outright rejection of
  aliases. Which applies is a fact about the dependency and must be established
  during the implementation slice, not assumed here. Section 5.8 records both
  outcomes as obligations; roadmap task 6.2.2 establishes which holds and
  records it in the standard-library guide.

Both questions were open when RFC 0006 was written and neither is a defect in
it: each names work the implementing slice must do, and each is answered by a
roadmap task rather than by a child RFC.

## 9. Recommendation

This group should be implemented first, at v0.1.x or later. It is the only
group RFC 0006 section 14.2 sequences with no prerequisite besides the shared
contract, it adds no crate, and it removes the most common reason a manifest
reaches for `shell()` at all. The group's five helpers are also the ones whose
absence is least defensible in a build tool: reading a JSON field is not a
template language feature request, it is the minimum needed to consume the
metadata that compilers and package managers already emit. Implementing it
first also exercises the whole contract at its smallest: five pure filters with
no capability handle, no platform variation, and no dialect, which is exactly
the shape that shows whether the cross-cutting clauses in section 5 carry
content or merely restate RFC 0006.
