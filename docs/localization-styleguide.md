# Netsuke localization style guide

This document defines Netsuke's house style for user-facing text and explains
how that style carries into every localization. The base guidance applies to
all languages; the per-locale terminology and register notes live in the
[localization glossary](localization-glossary.md), and the Fluent mechanics,
locale registry, and fallback policy live in the
[translator guide](translators-guide.md).

The distinction that organizes this guide is the standard one between voice and
tone: voice is the product's constant personality, while tone varies with
context.[^1] Translators keep the voice fixed and adapt the tone to the target
culture's expectations.

## Voice

Netsuke's voice is clear, calm, capable, and respectful. It is human-first and
agent-consistent: messages help both people and automation understand what
happened and what to do next.

In practice this means:

- Messages state facts. They do not blame the user, dramatize failure, pad
  with filler, or make claims the source cannot support.
- Diagnostics are concise and actionable. They state the condition, retain
  the relevant path, field, command, or value, and give a safe correction when
  the source provides one.
- The product never jokes in diagnostics and never adopts marketing
  language anywhere.

This voice is invariant. A translation that reads as bossy, jokey, apologetic,
or ceremonial has changed the product, not just the language.

## Tone by content type

Tone shifts with context while the voice stays fixed. The source catalogue
contains these content types, each with its own tone expectation:

- **Help and about text** (`cli.about`, `cli.flag.*.help`,
  `cli.subcommand.*`): informative and neutral. Complete sentences or complete
  noun phrases, matching the source's pattern for that entry.
- **Validation errors** (`cli.validation.*`, `clap-error-*`): direct and
  specific. Name the offending value and the valid range or options exactly as
  the source does.
- **Runner and I/O errors** (`runner.*`, `stdlib.*`): factual condition
  statements. Most begin "Failed to …"; translate the pattern once, keep it
  parallel across the whole family, and preserve the diagnostic order:
  condition first, then the value, then any remediation.
- **Hints and remediation** (`*.hint.*`, `*.help`): constructive
  instructions. Imperative in English; use the target locale's natural
  instruction form.
- **Status and progress lines** (`status.*`): short, parallel labels that
  render in aligned, space-constrained output. Choose compact forms and keep
  the set consistent.
- **Rendered HTML strings** (`graph.html.*`): plain descriptive prose for a
  standalone page, including a notice that must make sense with JavaScript
  disabled.

When translating, identify the content type first and reuse the established
pattern for that family rather than translating each message in isolation.

## Formality and register

The base register is professional plain language with moderate formality.
Prefer short sentences and direct instructions. Avoid slang, jokes, idioms,
culturally specific metaphors, and needlessly ceremonial phrasing.

Register does not translate literally; each locale renders the same
professional distance using its own conventions.[^2] The rules are:

- Use the natural professional register of the target locale for developer
  tools, not a literal translation of English formality markers.
- Where the language distinguishes formal and informal address (the T–V
  distinction), address the reader with the form established software
  convention uses in that locale. The per-locale sections of the glossary
  record the choice; when in doubt, follow the register used by major
  developer-tool translations in that language, such as the Microsoft
  localization style guides.[^2]
- Never mix address forms within the catalogue. One locale, one form.
- Prefer constructions that avoid gendering the reader or third parties.
  Where the language forces grammatical gender, prefer the locale's accepted
  neutral or generic convention over invented forms, and never assume the
  gender of the user.
- Honorific systems (for example Japanese teineigo) follow the same rule:
  use the standard polite register of technical documentation, without humble
  or exalted forms.

## Usage rules

Do:

- Keep messages concise, concrete, and useful at the point of action.
- Translate explanatory prose naturally while preserving its meaning and
  diagnostic order.
- Preserve code, paths, identifiers, option names, command fragments,
  message keys, and Fluent placeables exactly. This includes `{ $path }`,
  `--locale`, and `command_available`.
- Keep the distinctions between manifests, targets, actions, rules,
  dependencies, graphs, generated Ninja manifests, and executed builds.
- Follow the [glossary](localization-glossary.md) for product names,
  technical terminology, and the per-locale term mappings.
- Keep message families parallel: translate a repeated source pattern
  ("Failed to …", "… must not be empty") the same way every time it occurs.

Don't:

- Do not translate product, library, API, Rust path, command, option,
  manifest key, or Jinja identifier names.
- Do not add, remove, rename, or change the type of a Fluent placeable.
- Do not turn an actionable diagnostic into a vague status message or
  invent a prompt, warning, remediation, or guarantee absent from the source.
- Do not replace a precise technical term with a near-synonym when Netsuke
  distinguishes the concepts.
- Do not add humour, idioms, gender assumptions, or culturally specific
  metaphors.
- Do not expand or contract meaning to fit the target language's rhythm;
  fidelity to the diagnostic content outranks elegance.

## Grammar and mechanics across locales

- **Word order and agreement.** Placeables move with the target language's
  syntax. A placeable is never locked to its English position, but its name and
  braces are literal.
- **Sentence shape.** The source favours one condition per sentence. Keep
  that structure; do not merge sentences even where the target language would
  tolerate longer periods.
- **Text expansion.** Translations may legitimately run longer than the
  source (German commonly expands about a third), but status labels and aligned
  output should stay as compact as the language allows.[^3]
- **Punctuation.** Apply the target locale's rules for quotation marks,
  spacing, and terminal punctuation — for example French spacing before
  two-part punctuation, or CJK full-width punctuation — except inside code,
  paths, identifiers, placeables, and literal commands, whose punctuation is
  frozen.
- **Numbers, dates, and units.** Use the locale's formats in prose. Literal
  values interpolated through placeables are formatted by the program and pass
  through untouched.
- **Capitalization.** Follow the target language's rules (German noun
  capitalization, French sentence case) rather than mirroring English casing.
  Identifiers keep their exact case in every script, including in locales such
  as Turkish whose casing rules would otherwise alter `i`/`I`.
- **Plurals.** Use the target language's required CLDR plural categories in
  select expressions, adding or removing variants as the language requires.
- **Scripts and transliteration.** Keep loan words in the script the
  locale's technical writing conventionally uses. Never transliterate
  identifiers, keys, or code.

## Locale integrity

- Treat `locales/en-US/messages.ftl` as the source catalogue. Preserve
  every message key and the names and number of its placeables.
- Preserve valid Fluent syntax, attributes, terms, select expressions, and
  plural variants.
- Preserve meaningful BCP 47 (Best Current Practice 47) locale variants. Do
  not collapse `en-GB` into `en-US`, `es-419` into `es-ES`, `pt-BR` into
  `pt-PT`, or `zh-Hans` into `zh-Hant`. Regional and script variants exist
  because their vocabulary, orthography, or register genuinely differ.
- For right-to-left languages, add the direction mark specified by the
  [translator guide](translators-guide.md) when the first strong character of
  the rendered message would otherwise have the wrong direction.
- Do not translate locale tags, message keys, or literal values that form
  part of a machine-readable contract. Do not localize paths, commands, or
  machine-readable JSON field names.

## Machine-readable output

Automation consumes Netsuke's JSON diagnostics and parses fragments of its CLI
output. Translations must never alter:

- JSON field names, structure, or literal values.
- Message keys and Fluent placeable names.
- Exit-status conventions, option names, or command syntax shown for the
  user to run.

Prose inside machine-readable envelopes (for example a human-readable `message`
field) translates normally under all the rules above.

## Quality checklist

Before submitting a locale, confirm:

1. Every message key from the source catalogue is present and untranslated.
2. Every placeable survives with its exact name, braces, and type.
3. Terminology matches the glossary, including the locale's own section.
4. One address form is used consistently throughout.
5. Message families remain parallel ("Failed to …" renders identically
   across the family).
6. Status labels are compact, parallel, and internally consistent.
7. No humour, idiom, metaphor, or invented content has crept in.
8. Punctuation, quotation marks, and spacing follow the locale, except
   inside frozen literals.
9. Plural variants match the language's CLDR categories.
10. Right-to-left messages begin with the correct direction mark where
    required.

When a source message is ambiguous, inspect its code context and record the
question for review instead of guessing or silently broadening its meaning.

[^1]: The voice/tone split is standard localization practice; see, for
    example, [POEditor's guidance on voice and tone guides for localization
    teams](https://poeditor.com/blog/voice-and-tone-guide/) and
    [Mailchimp's content style guide](https://styleguide.mailchimp.com/voice-and-tone/).

[^2]: Per-language register conventions for software are documented in the
    [Microsoft localization style guides](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides),
    which this project uses as its reference corpus for locale register.

[^3]: On expansion rates and layout impact, see
    [W3C's guidance on text size in translation](https://www.w3.org/International/articles/article-text-size).
