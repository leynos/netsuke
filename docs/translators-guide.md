# Netsuke translator guide

This guide explains how to translate Netsuke's user-facing messages into new
languages or update existing translations.

## 1. Introduction

Netsuke uses [Project Fluent](https://projectfluent.org/) for localization.
Fluent is a modern localization system designed to handle the complexities of
natural language whilst keeping translations simple and readable.

**Locale precedence** (highest to lowest):

1. `--locale` command-line flag
2. `NETSUKE_LOCALE` environment variable
3. Configuration file `locale` setting
4. System default locale
5. Fallback to `en-US`

`en-US` is the source locale: it defines the key set every other catalogue must
match, and it renders any message a translation has not yet covered.

### Shipped locales

Table 1: Locales Netsuke ships, by script family

| Script family | Tags                                                                                                                                                        |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Latin         | `cs`, `cy`, `da`, `de`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fi`, `fr`, `gd`, `hu`, `id`, `it`, `nb`, `nl`, `pl`, `pt-BR`, `pt-PT`, `ro`, `sv`, `tr`, `vi` |
| Cyrillic      | `ru`, `uk`                                                                                                                                                  |
| Greek         | `el`                                                                                                                                                        |
| Right-to-left | `ar`, `fa`, `he`                                                                                                                                            |
| Indic         | `hi`                                                                                                                                                        |
| Thai          | `th`                                                                                                                                                        |
| CJK           | `ja`, `ko`, `zh-Hans`, `zh-Hant`                                                                                                                            |

## 2. The locale registry

`src/localization/locales.rs` owns the list of locales. Its `define_locales!`
macro both declares the supported tags and embeds each catalogue, so a tag
without a catalogue on disk fails to compile. Everything downstream reads the
registry rather than keeping its own list: the build-time audit, the
`cargo:rerun-if-changed` directives, the packaging smoke test, and the tests.

The one necessary duplicate is `package.metadata.ortho_config.locales` in
`Cargo.toml`, because Cargo metadata cannot call into Rust. The build audit
compares the two and fails the build if they drift, so adding a locale means
editing exactly two lists and nothing else.

### Fallback policy

Selection matches the exact BCP 47 tag first. A tag with no catalogue of its
own resolves in this order:

1. A script or region rule for that language, where the registry declares one.
2. The only catalogue for that language, so `fr-CA` uses `fr`.
3. `en-US`.

Table 2: Deliberate per-language fallback rules

| Language | Rule                                                                                |
| -------- | ----------------------------------------------------------------------------------- |
| `en`     | `en-US` for the bare tag and the United States; `en-GB` for every other region      |
| `es`     | `es-ES` for the bare tag and Spain; `es-419` for every other region                 |
| `pt`     | `pt-BR` for Brazil; `pt-PT` for the bare tag and every other region                 |
| `zh`     | Script wins; otherwise `zh-Hans` for CN, SG and MY, and `zh-Hant` for TW, HK and MO |
| `no`     | `nb`, the Bokmål catalogue                                                          |

These rules exist so that variants which differ in substance stay distinct.
`es-419` is not a synonym for `es-ES`, `pt-BR` is not a synonym for `pt-PT`, and
`zh-Hans` is not a synonym for `zh-Hant`; collapsing any of these onto a
generic language catalogue would ship the wrong copy. When adding a locale
whose language already ships a catalogue, add a rule to the registry rather
than relying on the unique-language step.

## 3. File structure

Translation files are located in the `locales/` directory, one directory per
tag:

```text
locales/
├── en-US/
│   └── messages.ftl
├── es-419/
│   └── messages.ftl
├── es-ES/
│   └── messages.ftl
└── …
```

Each locale has a single `messages.ftl` file containing all translations.

### FTL file format

Fluent Translation List (FTL) files use a simple key-value format:

```ftl
# Comment explaining the message context.
message-key = The translated message text.

# Message with a variable.
greeting = Hello, { $name }!
```

**Key rules:**

- Message keys use lowercase with hyphens or dots as separators
- Comments start with `#` and describe context for translators
- Blank lines separate logical sections
- Lines starting with `.` are attributes (sub-messages)
- Lines starting with `-` are terms (reusable fragments, not referenced in code)

## 4. Message key conventions

Netsuke uses hierarchical dot-notation for message keys, organized by domain.

Table 3: Message key domains and their purposes

| Domain             | Purpose                            | Example                       |
| ------------------ | ---------------------------------- | ----------------------------- |
| `cli.*`            | CLI help text and validation       | `cli.flag.file.help`          |
| `clap-error-*`     | Command-line parser errors         | `clap-error-missing-argument` |
| `runner.*`         | Manifest loading and I/O           | `runner.manifest.not_found`   |
| `manifest.*`       | YAML parse and template errors     | `manifest.yaml.parse`         |
| `ir.*`             | Intermediate representation errors | `ir.rule_not_found`           |
| `ninja_gen.*`      | Ninja file generation              | `ninja_gen.missing_action`    |
| `stdlib.*`         | Standard library helpers           | `stdlib.fetch.url_invalid`    |
| `host_pattern.*`   | Network host validation            | `host_pattern.empty`          |
| `network_policy.*` | Network access control             | `network_policy.host.blocked` |
| `example.*`        | Translator examples                | `example.files_processed`     |

**Naming pattern:** `domain.subdomain.specific_message`

The corresponding Rust constants are defined in `src/localization/keys.rs`
using UPPER_SNAKE_CASE (e.g., `CLI_FLAG_FILE_HELP` maps to
`cli.flag.file.help`).

## 5. Variable usage

Variables are placeholders replaced with dynamic values at runtime.

### Syntax

```ftl
# Basic variable substitution.
error-at-path = Error at { $path }: { $details }

# Variables can appear multiple times.
range-error = Value { $value } must be between { $min } and { $max }.
```

### Variable types

Table 4: Variable types used in Fluent messages

| Type   | Description                        | Example                       |
| ------ | ---------------------------------- | ----------------------------- |
| String | Text values                        | `{ $path }`, `{ $name }`      |
| Number | Numeric values (used with plurals) | `{ $count }`, `{ $limit }`    |
| Path   | File system paths                  | `{ $path }`, `{ $directory }` |

### Variable catalogue by domain

**CLI validation (`cli.validation.*`):**

- `$value` - User-provided value
- `$min`, `$max` - Range boundaries
- `$scheme` - URL scheme
- `$locale` - Locale identifier

**Runner errors (`runner.*`):**

- `$path` - File path
- `$directory` - Directory path
- `$manifest_name` - Manifest file name

**Manifest diagnostics (`manifest.*`):**

- `$name` - Field or macro name
- `$details` - Error details
- `$line`, `$column` - Source location
- `$pattern`, `$position`, `$character` - Glob pattern info
- `$expr` - Expression text
- `$value` - Parsed value

**Standard library (`stdlib.*`):**

- `$url` - URL being fetched
- `$details` - Error details
- `$path` - File path
- `$action` - Action being performed
- `$command` - Command name
- `$count` - Numeric count (for plurals)
- `$limit` - Size limit in bytes
- `$mode`, `$stream` - Output configuration

## 6. Plural forms

Fluent uses Common Locale Data Repository (CLDR) plural rules to handle
grammatical number. Different languages have different plural categories.

### English plural categories

English uses two categories: `one` (singular) and `other` (plural).

```ftl
example.files_processed = { $count ->
    [one] Processed { $count } file.
   *[other] Processed { $count } files.
}
```

The `*` marks the default variant (required).

### Spanish plural categories

Spanish also uses `one` and `other`, but verb conjugation often differs:

```ftl
example.files_processed = { $count ->
    [one] Se procesó { $count } archivo.
   *[other] Se procesaron { $count } archivos.
}
```

### Special cases

Use explicit numeric matches for special cases like zero:

```ftl
example.errors_found = { $count ->
    [0] No errors found.
    [one] { $count } error found.
   *[other] { $count } errors found.
}
```

### CLDR plural categories by language

Table 5: CLDR plural categories by shipped locale

| Categories                                   | Locales                                                                                                                       |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `other`                                      | `hu`, `id`, `ja`, `ko`, `th`, `vi`, `zh-Hans`, `zh-Hant`                                                                      |
| `one`, `other`                               | `da`, `de`, `el`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fa`, `fi`, `fr`, `hi`, `it`, `nb`, `nl`, `pt-BR`, `pt-PT`, `sv`, `tr` |
| `one`, `few`, `other`                        | `ro`                                                                                                                          |
| `one`, `few`, `many`, `other`                | `cs`, `pl`, `ru`, `uk`                                                                                                        |
| `one`, `two`, `few`, `other`                 | `gd`                                                                                                                          |
| `one`, `two`, `many`, `other`                | `he`                                                                                                                          |
| `zero`, `one`, `two`, `few`, `many`, `other` | `ar`, `cy`                                                                                                                    |

A locale that lists a category must spell out its own wording for that variant.
A test in `tests/locale_catalogue_tests.rs` asserts these category sets, so
dropping Polish `few` or Welsh `two` fails the suite rather than quietly losing
a form.

Note that `one` does not always mean "exactly one": in French it also covers
zero, and in Hindi likewise. Where a language prefers a distinct phrase for
none at all, use an explicit `[0]` variant, as the shipped catalogues do for
`example.errors_found`.

Consult the
[CLDR plural rules](https://cldr.unicode.org/index/cldr-spec/plural-rules) for
the target language.

### Current limitation

The Netsuke localization API currently passes all arguments as strings rather
than preserving numeric types. This means CLDR plural selectors like `[one]`
will not match as expected because Fluent requires numeric `FluentValue` types
for CLDR category selection.

**Workaround:** Messages will resolve using the default `*[other]` variant. The
FTL files include plural form examples demonstrating correct Fluent syntax for
future compatibility when numeric argument support is added.

## 7. Adding a new locale

To add support for a new language (for example Icelandic, `is`):

### Step 1: Start from the source catalogue

```sh
mkdir -p locales/is
cp locales/en-US/messages.ftl locales/is/messages.ftl
```

The copy is a starting scaffold, not a deliverable. A catalogue that still
carries English values is not a translation, and a test rejects one.

### Step 2: Translate the messages

Edit `locales/is/messages.ftl` and translate each value. Keep every key, keep
each message's `{ $variables }` exactly as the English source has them, and
translate the section comments so the next translator has the same context.

```ftl
# Before (English):
cli.about = Netsuke compiles YAML + Jinja manifests into Ninja build plans.

# After (Icelandic):
cli.about = Netsuke þýðir YAML + Jinja lýsingarskrár í Ninja-byggingaráætlanir.
```

Leave Netsuke's own identifiers untranslated — users type them. That covers
`foreach`, `when`, `vars`, `cwd_mode`, `with_suffix`, `group_by`, the
`netsuke::jinja::*` diagnostic tags, the literal option values (`auto`,
`always`, `never`, `on`, `off`) and the shell fragment `ninja -t clean`.

### Step 3: Register the locale

Add the tag to the two lists that name it:

1. `define_locales!` in `src/localization/locales.rs`, in tag order.
2. `package.metadata.ortho_config.locales` in `Cargo.toml`, in the same order.

If the language already ships a catalogue — a new Spanish or Chinese variant,
say — also add or extend its entry in `LANGUAGE_FALLBACKS` so requests route to
the right variant rather than falling through to the first match.

### Step 4: Build

```sh
cargo build
```

The compile-time audit verifies the tag lists agree and that the catalogue's
keys and interpolation variables match the source.

### Step 5: Test the locale

```sh
make test
cargo run -- --locale is --help
```

Verify the output appears in Icelandic.

## 8. Right-to-left locales

Arabic, Hebrew and Persian ship right-to-left catalogues. Fluent already wraps
interpolated values in bidi isolation controls, so `{ $path }` needs no special
handling. What does need care is the *first* character of a message: a value
that opens with a Latin word, a bracket, or a placeable lets that token decide
the paragraph direction, which flips the whole line in a terminal.

Prefix such values with U+200F RIGHT-TO-LEFT MARK:

```ftl
# The leading Latin word would otherwise set the direction.
manifest.yaml.label = ‏YAML غير صالح
```

A test in `tests/locale_catalogue_tests.rs` enforces this: in a right-to-left
catalogue, any message containing right-to-left text must begin with either a
right-to-left character or U+200F. Messages that are entirely Latin — `stdout`,
`stderr`, the `netsuke::jinja::which::args` diagnostic — are left unmarked.

## 9. Quality checklist

Before submitting translations, verify:

- [ ] All message keys from `en-US/messages.ftl` are present
- [ ] No extra (orphaned) keys exist
- [ ] All variables match the English source (same names, same count)
- [ ] Plural forms use the CLDR categories in Table 5 for the target language
- [ ] Netsuke identifiers and literal option values are untranslated
- [ ] Right-to-left catalogues carry the direction marks described in §8
- [ ] Comments are translated or preserved for context
- [ ] The build passes (`cargo build`)
- [ ] The tests pass (`make test`)
- [ ] The locale renders correctly (`netsuke --locale <tag> --help`)

## 10. Compile-time validation

Netsuke validates every registered locale at compile time via
`build_l10n_audit/`:

- **Metadata drift**: `Cargo.toml`'s locale list disagreeing with the registry
- **Missing keys**: Keys in `keys.rs` but not in the FTL file
- **Orphaned keys**: Keys in the FTL file but not in `keys.rs`
- **Variable mismatches**: A message interpolating different variables from the
  English source — a dropped `{ $path }` or a stray `{ $name }`

Each condition fails the build with an error naming the locale and the keys
concerned.

## 11. Testing translations

Localization is tested via:

- **Rendering tests** (`tests/localization_tests.rs`): every registered locale
  renders and interpolates its arguments; non-Latin scripts and right-to-left
  direction marks survive to the rendered string
- **Registry tests** (`tests/locale_registry_tests.rs`): each shipped tag
  resolves to its own catalogue, each documented fallback rule holds, and
  unsupported or unparsable tags fall back to `en-US`
- **Catalogue tests** (`tests/locale_catalogue_tests.rs`): CLDR plural
  categories per language, the right-to-left direction policy, untranslated
  Netsuke identifiers, and the rule that a translation is not a copy of the
  English source

Run tests with:

```sh
make test
```

## 12. Resources

- [Project Fluent](https://projectfluent.org/) - Fluent documentation
- [Fluent Syntax Guide](https://projectfluent.org/fluent/guide/) - FTL syntax
- [CLDR Plural Rules](https://cldr.unicode.org/index/cldr-spec/plural-rules) -
  Plural categories by language
- [Unicode CLDR](https://cldr.unicode.org/) - Locale data repository
