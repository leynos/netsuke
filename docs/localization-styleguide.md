# Netsuke Locize style guide

This document is the local source of truth for Netsuke's base Locize style
guide. Its six sections correspond exactly to the fields defined by the
[Locize styleguide documentation][^1]. The base applies to every language;
language-specific overrides should contain only rules that differ from it.

## Tone of voice

Clear, calm, capable, and respectful. Netsuke is human-first and
agent-consistent: messages help both people and automation understand what
happened and what to do next. Diagnostics are concise and actionable. They
state the condition, retain the relevant path, field, command, or value, and
give a safe correction when the source provides one. Avoid blame, drama,
filler, marketing language, and unsupported claims.

## Level of formality

Professional plain language with moderate formality. Prefer short sentences and
direct instructions. Avoid slang, jokes, idioms, culturally specific metaphors,
and needlessly ceremonial phrasing. Use the natural professional register of
the target locale rather than translating English formality markers literally.

## Target audience

Developers who author and run `Netsukefile` manifests, operators who run builds
locally or in Continuous Integration (CI), and automation agents that consume
the command-line interface or JSON diagnostics. Assume technical competence,
but do not assume knowledge of Netsuke internals that the source message does
not explain.

## Usage rules (do's and don'ts)

Do:

- Keep messages concise, concrete, and useful at the point of action.
- Translate explanatory prose naturally while preserving its meaning and
  diagnostic order.
- Preserve code, paths, identifiers, option names, command fragments, message
  keys, and Fluent placeables exactly. This includes `{ $path }`, `--locale`,
  and `command_available`.
- Keep the distinctions between manifests, targets, actions, rules,
  dependencies, graphs, generated Ninja manifests, and executed builds.
- Follow the project glossary for product names and technical terminology.

Don't:

- Do not translate product, library, API, Rust path, command, option, manifest
  key, or Jinja identifier names.
- Do not add, remove, rename, or change the type of a Fluent placeable.
- Do not turn an actionable diagnostic into a vague status message or invent a
  prompt, warning, remediation, or guarantee absent from the source.
- Do not replace a precise technical term with a near-synonym when Netsuke
  distinguishes the concepts.
- Do not add humour, idioms, gender assumptions, or culturally specific
  metaphors.

## Localization rules

- Treat `locales/en-US/messages.ftl` as the source catalogue. Preserve every
  message key and the names and number of its placeables.
- Preserve valid Fluent syntax, attributes, terms, select expressions, and
  plural variants. Use the target language's required plural categories.
- Apply the target locale's grammar, punctuation, quotation marks, spacing,
  word order, number formats, and conventions for addressing the reader.
- Preserve meaningful BCP 47 variants. In particular, do not collapse
  `en-GB` into `en-US`, `es-419` into `es-ES`, `pt-BR` into `pt-PT`, or
  `zh-Hans` into `zh-Hant`.
- Preserve punctuation that belongs to code, paths, identifiers, placeables,
  or literal commands.
- For right-to-left languages, add the direction mark specified by the
  [translator guide](translators-guide.md) when the first strong character of
  the rendered message would otherwise have the wrong direction.
- Do not translate locale tags, message keys, or literal values. Do not
  localize paths, commands, or machine-readable JSON field names.

## Misc

Consult the [Netsuke glossary](localization-glossary.md) for terminology and
the [translator guide](translators-guide.md) for Fluent structure, fallback
rules, variables, plurals, bidirectional text, and catalogue validation. When
a source message is ambiguous, inspect its code context and record the question
for review instead of guessing or silently broadening its meaning.

The Locize project currently labels `en` as its reference language, whereas
Netsuke's source catalogue is `en-US`. Treat `en-US` as authoritative for
message content until the Locize project setting is corrected.

[^1]: [Locize styleguide documentation](https://www.locize.com/docs/styleguide)
