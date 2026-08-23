# Netsuke localization glossary

This document is the source of truth for Netsuke terminology across every
locale. It records the preferred, allowed, and forbidden forms of each
source-language term, explains which terms resist translation and why, and sets
out per-locale terminology so translators and reviewers apply the same
vocabulary consistently. It complements the
[translator guide](translators-guide.md), which owns the locale registry,
Fluent mechanics, and fallback policy, and the
[localization style guide](localization-styleguide.md), which owns voice, tone,
and mechanics.

## Schema

Each row in a terminology table represents one term record. The columns are:

- `title`: the canonical term as it appears in the source catalogue or
  repository documentation.
- `preferred`: the form translators must use.
- `allowed`: acceptable alternatives, separated by semicolons.
- `forbidden`: forms that must not appear, separated by semicolons.

An em dash (—) means the set is empty. Netsuke's source catalogue is
`locales/en-US/messages.ftl`; the `en-US` terms below are authoritative for
message content. Per-locale sections map the difficult subset of these terms
into each shipped locale.

Keep forbidden values conservative. Add a value only for an observed
misspelling, an identifier variant that would break a literal interface, or a
collision between distinct Netsuke concepts.

## Source terminology (en-US)

### Product and ecosystem names

These names are invariant in every locale. They are never translated,
transliterated, inflected with affixes that alter the written form, or respelt,
although surrounding text follows the target language's grammar.

Table 1: product and ecosystem names

| title       | preferred     | allowed                                            | forbidden                     |
| ----------- | ------------- | -------------------------------------------------- | ----------------------------- |
| Netsuke     | `Netsuke`     | `netsuke` (executable, crate, command, code path)  | —                             |
| Netsukefile | `Netsukefile` | `Netsuke manifest`; `manifest file`                | `NetsukeFile`; `Netsuke file` |
| Ninja       | `Ninja`       | `ninja` (executable or literal command)            | —                             |
| Fluent      | `Fluent`      | `Project Fluent`; `Fluent Translation List`; `FTL` | —                             |
| Jinja       | `Jinja`       | `MiniJinja` (the Rust implementation)              | `Jinja2` (for the dialect)    |
| YAML        | `YAML`        | —                                                  | `Yaml`; `yaml` (in prose)     |
| Graphviz    | `Graphviz`    | —                                                  | —                             |
| DOT         | `DOT`         | `Graphviz DOT`                                     | `dot` (for the language)      |
| Rust        | `Rust`        | —                                                  | —                             |
| CLDR        | `CLDR`        | `Unicode CLDR`                                     | —                             |

### Manifest and build-model concepts

These terms carry precise, mutually exclusive meanings in Netsuke's build
model. Substituting a near-synonym in any language blurs a distinction the
product depends on.

Table 2: manifest and build-model terms

| title                 | preferred               | allowed                                      | forbidden                     |
| --------------------- | ----------------------- | -------------------------------------------- | ----------------------------- |
| manifest              | `manifest`              | `Netsuke manifest`; `manifest file`          | `configuration file`          |
| target                | `target`                | `build target`; `default target`             | `rule`; `goal`; `objective`   |
| action                | `action`                | `build action`; `action catalogue`           | `rule`; `task`                |
| rule                  | `rule`                  | `Ninja rule`                                 | `target`; `action`            |
| dependency            | `dependency`            | `direct dependency`; `order-only dependency` | `dependancy`                  |
| dependencies          | `dependencies`          | `direct dependencies`; `dependency graph`    | `dependancies`                |
| order-only dependency | `order-only dependency` | —                                            | `weak dependency`             |
| build graph           | `build graph`           | `dependency graph`; `graph`                  | —                             |
| build edge            | `build edge`            | `edge`                                       | —                             |
| phony target          | `phony target`          | `implicitly phony target`                    | `fake target`; `false target` |
| default target        | `default target`        | `manifest default`                           | —                             |
| build                 | `build` (noun and verb) | `build run`                                  | `compilation` (generic use)   |
| clean                 | `clean` (subcommand)    | —                                            | `purge`; `wipe`               |
| generate              | `generate` (subcommand) | —                                            | —                             |
| artefact              | `artefact`              | `build artefact`; `graph artefact`           | —                             |
| workspace root        | `workspace root`        | —                                            | `project root` (in messages)  |
| working directory     | `working directory`     | `current directory`                          | `folder`                      |
| dyndep                | `dyndep`                | `dyndep file`; `generated dyndep file`       | `dynamic dependency file`     |
| serial dependency     | `serial dependency`     | `serial dependency ordering`                 | —                             |

### Template and expression language

Netsuke manifests embed Jinja templates and expressions. Several of these terms
are identifiers or near-identifiers that must survive translation byte-for-byte.

Table 3: template and expression terms

| title               | preferred           | allowed                                 | forbidden                                |
| ------------------- | ------------------- | --------------------------------------- | ---------------------------------------- |
| template            | `template`          | `Jinja template`                        | `boilerplate`                            |
| standard library    | `standard library`  | `template standard library`; `stdlib`   | —                                        |
| macro               | `macro`             | `manifest macro`                        | `function` (for macros)                  |
| expression          | `expression`        | `when expression`; `foreach expression` | `formula`                                |
| `foreach`           | `foreach`           | —                                       | `for each`; `for-each`                   |
| `when`              | `when`              | —                                       | —                                        |
| `vars`              | `vars`              | —                                       | `variables` (for the key)                |
| `command_available` | `command_available` | `command_available(name, **kwargs)`     | `command-available`; `command available` |
| glob                | `glob`              | `glob pattern`                          | `wildcard search`                        |
| helper              | `helper`            | `template helper`; `built-in helper`    | —                                        |
| filter              | `filter`            | `Jinja filter`                          | —                                        |
| fetch               | `fetch` (helper)    | `fetch helper`                          | `download` (for the helper)              |
| which               | `which` (helper)    | `which helper`                          | —                                        |

### Localization and message concepts

Table 4: localization terms

| title             | preferred           | allowed                             | forbidden                 |
| ----------------- | ------------------- | ----------------------------------- | ------------------------- |
| locale            | `locale`            | `locale tag`; `BCP 47 tag`          | `language` (for the tag)  |
| placeable         | `placeable`         | `Fluent placeable`; `variable`      | `placeholder text`        |
| message key       | `message key`       | `key`                               | `message ID`              |
| plural category   | `plural category`   | `CLDR plural category`              | `plural form` (imprecise) |
| select expression | `select expression` | —                                   | `switch`                  |
| fallback          | `fallback`          | `fallback chain`                    | —                         |
| source catalogue  | `source catalogue`  | `source catalog`; `en-US catalogue` | —                         |

### Command-line and diagnostic vocabulary

Table 5: CLI and diagnostic terms

| title                | preferred              | allowed                      | forbidden                    |
| -------------------- | ---------------------- | ---------------------------- | ---------------------------- |
| allowlist            | `allowlist`            | `host allowlist`             | `whitelist`                  |
| blocklist            | `blocklist`            | `blocked hosts`              | `blacklist`                  |
| host pattern         | `host pattern`         | —                            | —                            |
| scheme               | `scheme`               | `URL scheme`                 | `protocol` (for URL schemes) |
| cache                | `cache`                | `fetch cache`; `which cache` | —                            |
| stdout               | `stdout`               | `standard output` (prose)    | `STDOUT`                     |
| stderr               | `stderr`               | `standard error` (prose)     | `STDERR`                     |
| exit status          | `exit status`          | `status`                     | `error level`                |
| signal               | `signal`               | —                            | —                            |
| broken pipe          | `broken pipe`          | —                            | —                            |
| timeout              | `timeout`              | `timed out`                  | `time-out`                   |
| capture              | `capture` (mode)       | `capture mode`               | —                            |
| streaming            | `streaming` (mode)     | `streaming mode`             | —                            |
| verbose              | `verbose`              | `verbose logging`            | `noisy`                      |
| pipeline             | `pipeline`             | `build pipeline`             | —                            |
| stage                | `stage`                | `pipeline stage`             | `phase`; `step` (in status)  |
| environment variable | `environment variable` | `env var` (informal)         | —                            |
| path                 | `path`                 | `file path`                  | `route`                      |
| UTF-8                | `UTF-8`                | —                            | `UTF8`; `utf-8` (in prose)   |
| canonicalize         | `canonicalize`         | —                            | `normalise` (for paths)      |

## Localizability notes

The terms below are the ones most likely to go wrong in translation. Each note
explains the hazard; the per-locale sections apply these notes to each shipped
locale.

- **`target` versus `action` versus `rule`.** Netsuke separates the three
  concepts, while many languages use one everyday word for all of them (compare
  French *cible*, *action*, *règle*). A target is a build output or named build
  entry; an action is an implicitly phony target; a rule is the Ninja construct
  a target references. Translations must keep three distinct words and use them
  consistently.
- **`build`.** English uses one word as noun and verb; many languages must
  split the pair (German *Build* as a loan noun beside the verb *erstellen*).
  Pick one noun and one verb per locale and keep them stable.
- **`dependency` (direct versus order-only).** "Order-only dependency" is
  Ninja jargon with no natural equivalent in most languages. Translate the head
  noun, keep the qualifier literal, and gloss it on first use if the target
  language would otherwise read it as "a dependency that is only an order".
- **`dyndep`, `glob`, `stdlib`, `phony`.** These are loan words even in
  English: they name Ninja and Unix constructs, not ideas. Keep them as loan
  words in every locale; translating them severs the link to the underlying
  tool and its documentation.
- **`manifest`.** In several languages the everyday cognate means a shipping
  document or a political manifesto. Prefer the local software sense where one
  is established; otherwise keep the loan word rather than fall back to
  "configuration file", which Netsuke forbids because a manifest is not
  configuration.
- **`allowlist` and `blocklist`.** Prefer local equivalents of allow/block
  lists over legacy white/black metaphors, matching the source's own choice.
- **`cache`.** Widely borrowed, but the local form varies (French *cache*,
  German *Cache*, Spanish *caché*, Russian *кэш*). Use the established local
  form; do not invent a descriptive phrase.
- **`placeable`.** Fluent-specific jargon. Most locales should keep the
  Fluent term untranslated on first use with a local gloss, because it names a
  syntax object translators must not alter.
- **`stdout`, `stderr`, `UTF-8`, identifiers.** Never translated,
  transliterated, or respaced in any locale, including locales that
  transliterate freely elsewhere.
- **`stage` and status vocabulary.** Status lines such as `pending`,
  `in progress`, `done`, and `failed` render in aligned, space-constrained
  output. Locales should choose short, parallel forms and keep the set
  internally consistent rather than translating each state in isolation.
- **`artefact`.** The source catalogue uses the spelling `artefact`. Locales
  translate the concept (a produced file), not the spelling.
- **`fetch` and `which`.** As helper names they are identifiers and stay
  literal; as prose ("the fetch helper") the surrounding words translate.

## Locale terminology

Each section below records how the difficult subset of the source terminology
is rendered in one shipped locale, together with register notes and the loan
words that locale conventionally keeps. `en-US` is the source and has no
section. Terms listed as invariant in Table 1 stay invariant in every locale
and are not repeated per locale.

<!-- Per-locale sections are inserted below in locale-tag order. -->
