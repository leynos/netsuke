# Architectural decision record (ADR) 0082: Accept environment Boolean digits and case-insensitive policy values

## Status

Accepted on 2026-11-18. Environment values for the five Boolean configuration
keys accept `1` and `0` as well as `true` and `false`; the four display-policy
enums deserialize case-insensitively by delegating to `FromStr`; indexed `__N`
list variables remain rejected, and configuration files keep strict Boolean
typing.

## Date

2026-11-18.

## Context and problem statement

Issue #1147 records three forms that the users' guide documents but the full
configuration merge rejects:

1. `NETSUKE_JSON=1` and `NETSUKE_VERBOSE=1`, because Figment parses `1` as a
   number and Serde refuses to deserialize a Boolean from a number.
2. Policy values such as `color = "AlWaYs"` and `NETSUKE_COLOR=ALWAYS`, because
   `#[serde(rename_all = "kebab-case")]` matches spellings exactly while the
   command-line `FromStr` implementation is case-insensitive.
3. `NETSUKE_DEFAULT_TARGETS__0=hello.txt`, because the environment layer turns
   the double-underscore segment into a nested map rather than a list.

The first two forms also split behaviour across passes: the early
`NETSUKE_JSON` resolver used before the full merge and the merge itself must
accept the same values, or a malformed startup reports a different outcome from
a successful one. The third form has no equivalent JSON shape for a list, so
accepting it would require a new path model rather than a value coercion.

## Decision drivers

- The guide and the runtime must agree on the accepted value forms.
- The early diagnostic resolver and the full merge must accept exactly the same
  `NETSUKE_JSON` values.
- Configuration files should remain strongly typed rather than accepting
  numbers where a Boolean is declared.
- The accepted sets should stay small and predictable instead of adopting
  general "truthy" parsing.

## Requirements

### Functional requirements

- Accept `true`, `false`, `1`, and `0`, and nothing else, for the `verbose`,
  `json`, `fetch_default_deny`, `trust_project_fetch_policy`, and `no_input`
  environment variables.
- Reject case variants (`True`), word variants (`yes`, `on`), numbers other than
  `1` and `0`, empty values, and values with surrounding whitespace for those
  Boolean variables in both passes.
- Accept every canonical policy spelling in any ASCII casing from TOML, the
  environment, and the command line.
- Keep accepting the scalar and serialized JSON-array forms of list variables.
- Keep rejecting indexed `__N` list variables.

### Technical requirements

- One shared strict Boolean parser owns the environment's accepted spellings.
- Policy `Deserialize` implementations delegate to the existing `FromStr`, whose
  `Err` stays `String`.
- Policy serialization keeps the canonical lowercase spelling.
- Configuration-file Booleans stay strict.

## Options considered

### Option A: Accept Boolean digits and case-insensitive policy values

Environment Booleans accept the four conventional spellings through a shared
strict parser, and policy deserialization reuses the case-insensitive `FromStr`
already used by the command line. Indexed list variables stay rejected.

### Option B: Correct the guide to match the rejection

The guide would stop documenting `1` and mixed-case policies. This was rejected
because it preserves a difference between the command line and configuration
files, and because the early and full `NETSUKE_JSON` resolvers would keep
disagreeing on `1`.

### Option C: Adopt a general lenient Boolean parser

A parser accepting `yes`, `on`, `no`, and `off` would match some third-party
conventions. This was rejected because the early resolver and the merge would
both have to grow the same open-ended vocabulary, and because it silently
accepts spellings the guide does not document.

## Decision outcome / proposed direction

`src/cli/environment/mod.rs` exposes one strict parser for the four accepted
spellings. The environment layer applies it only to the exact five Boolean
configuration paths and otherwise keeps Figment's ordinary scalar parsing, so
`jobs=1` stays a number. A spelling outside the four is kept as raw text, which
makes the full merge fail deserialization rather than coercing the value.

`src/cli/config/mod.rs` implements `Deserialize` for `ColourPolicy`,
`EmojiPolicy`, `ProgressPolicy`, and `AccessibilityPolicy` by reading a string
and delegating to `FromStr`. The error names the accepted spellings, and
serialization continues to emit the canonical lowercase spelling. The
deserializer does not trim, matching the command-line contract: a quoted file
value such as `color = " always "` is rejected. The environment value parser
trims unquoted scalars before this point, so `NETSUKE_COLOR=' always '` is
accepted; that trim is Figment's ordinary scalar parsing, not part of the
policy contract, and applies to every string-valued environment variable.

The merge composes layers and deserializes the masked JSON once. A valid value
in a higher layer therefore masks an invalid value in a lower layer; the
explicit `--json` short circuit is consistent with that contract rather than a
special case. Indexed `__N` variables still nest into a map and fail list
deserialization with an invalid-type error.

## Goals and non-goals

- Goals:
  - Make the documented value forms work through the full merge.
  - Keep the two `NETSUKE_JSON` passes in agreement.
  - Keep configuration files strongly typed.
- Non-goals:
  - Introduce a general truthy parser or accept `yes`, `on`, `no`, or `off`.
  - Accept Unicode case folding for policy spellings; the existing contract is
    ASCII case-insensitive.
  - Make indexed `__N` environment variables produce lists.
  - Change the `no_input = false` policy rejection.

## Migration plan

- Users of `NETSUKE_*__0` list variables switch to the scalar form
  (`NETSUKE_DEFAULT_TARGETS=hello.txt`) or a serialized JSON array.
- No migration is needed for `1` and `0` or for mixed-case policy values; they
  become accepted without a configuration change.
- Surrounding whitespace in a quoted policy value remains invalid and must be
  removed. The environment parser trims unquoted values, so an environment
  policy needs no change unless it is quoted by the caller in a way that the
  shell preserves as literal spaces.

## Known risks and limitations

- The early `NETSUKE_JSON` resolver reads the exact variable name, while the
  environment layer normalizes ASCII case and surrounding whitespace in keys.
  An alias such as `netsuke_json=1` therefore reaches the full merge but not
  the early resolver. This difference is recorded here and does not affect the
  canonical spelling.
- `src/cli/l10n/mod.rs::parse_bool_hint` remains a third, lenient reader that
  accepts `yes`, `on`, `no`, and `off` before full parsing. It only selects a
  startup message language and never decides whether configuration is accepted.
- ASCII case-insensitivity does not fold non-ASCII characters, matching
  `FromStr`.
- Because higher layers mask lower layers before deserialization, an invalid
  value in a lower layer can be hidden by a valid value in a higher layer. This
  is the established layering contract, not a property of these value forms.
