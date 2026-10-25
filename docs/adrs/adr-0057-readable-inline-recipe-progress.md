# Architectural decision record (ADR) 0057: Readable progress for inline recipes

## Status

Accepted on 2026-10-24. Inline-recipe progress labels belong to canonical build
edges and are independent of action identity and target/action discovery
metadata.

## Date

2026-10-24

## Context and Problem Statement

Ninja uses a rule's `description` for progress output and otherwise prints the
command. Netsuke wraps inline command and script recipes in generated shell
text, so inline edges without a rule description expose that wrapper as their
progress text. Command lists repeat the wrapper for each entry. Roadmap item
3.14.11 separately establishes target/action `description` as discovery
metadata, leaving inline recipes without a readable progress label.

The progress label must remain attached to the edge: identical inline recipes
can share one deduplicated action while each target has its own display name.

## Decision Drivers

- Keep target/action `description` as discovery metadata under roadmap 3.14.11.
- Preserve rule descriptions as the authoritative progress text for named rules.
- Keep action hashes, deduplication, commands, and failure attribution stable.
- Make progress output safe for terminals and valid Ninja metadata.
- Bound output so a manifest-controlled name cannot dominate a captured log.

## Decision Outcome

Store the raw first declared output name as optional provenance on the one
canonical `BuildEdge` created for an inline command or script recipe. Leave the
field absent for empty names, dependency-only recipes, and referenced rules. Do
not include it in `Action`, action registration, hashing, or failure
fingerprints.

For an executable inline edge, sanitize the raw name with the shared terminal
text helper, then bound the sanitized name to 80 Unicode scalar values,
including a trailing `…` when truncation is needed. Localize a short scalar or
script label, or a command-list label that includes the recipe's entry count.
Use the resolved action recipe to choose the message and count; never use
command text or target/action discovery descriptions. Remove Fluent's
formatter-added directional isolates after rendering; raw bidi controls in the
name have already been made visible before interpolation.

Emit the localized label as an edge-level Ninja `description` binding beside
`restat`, in both ordinary and dyndep output. Escape it with the existing
metadata-value path, so `$` is doubled, and unsafe Ninja control characters
return the established generation error. Do not emit this binding for
dependency-only phony edges. Executable inline actions keep their label even
when their output is phony.

Named-rule edges have no edge-level label. Their rule `description` remains
authoritative. Target/action `description` remains discovery metadata, as
specified by 3.14.11.

Generated text is deterministic for a fixed graph and locale. Localization
changes the wording for the same edge; it does not change graph identity or
recipe execution.

## Consequences

- Different inline targets may share an action ID and command while emitting
  distinct edge descriptions.
- The shared terminal-text module owns terminal sanitization and bounded text
  operations. Terminal-facing callers may reuse it; metadata serialization,
  shell quoting, and general text normalization remain separate concerns.
- The Fluent catalogue owns scalar/script wording and list wording, including
  locale-specific plural selection.
- The first declared output is the stable display name for a multi-output
  edge, regardless of which output alias is requested.

## References

- [Netsuke design, §2.4 and §2.6](../netsuke-design.md)
- [Roadmap item 3.14.11](../roadmap.md#314-conditional-action-planning)
