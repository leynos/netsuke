# Netsuke translator guide

This guide explains how to translate Netsuke's user-facing messages into new
languages or update existing translations.

## 1. Introduction

Netsuke uses [Project Fluent](https://projectfluent.org/) for localization.
Fluent is a modern localization system designed to handle the complexities of
natural language whilst keeping translations simple and readable.

**Current locales:**

- `en-US` - English (United States) - source locale
- `es-ES` - Spanish (Spain) - reference translation

**Locale precedence** (highest to lowest):

1. `--locale` command-line flag
2. `NETSUKE_LOCALE` environment variable
3. Configuration file `locale` setting
4. System default locale
5. Fallback to `en-US`

## 2. File structure

Translation files are located in the `locales/` directory:

```text
locales/
├── en-US/
│   └── messages.ftl
└── es-ES/
    └── messages.ftl
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

## 3. Message key conventions

Netsuke uses hierarchical dot-notation for message keys, organized by domain.

Table 1: Message key domains and their purposes

| Domain | Purpose | Example |
| ------ | ------- | ------- |
| `cli.*` | CLI help text and validation | `cli.flag.file.help` |
| `clap-error-*` | Command-line parser errors | `clap-error-missing-argument` |
| `runner.*` | Manifest loading and I/O | `runner.manifest.not_found` |
| `manifest.*` | YAML parse and template errors | `manifest.yaml.parse` |
| `ir.*` | Intermediate representation errors | `ir.rule_not_found` |
| `ninja_gen.*` | Ninja file generation | `ninja_gen.missing_action` |
| `stdlib.*` | Standard library helpers | `stdlib.fetch.url_invalid` |
| `host_pattern.*` | Network host validation | `host_pattern.empty` |
| `network_policy.*` | Network access control | `network_policy.host.blocked` |
| `example.*` | Translator examples | `example.files_processed` |

**Naming pattern:** `domain.subdomain.specific_message`

The corresponding Rust constants are defined in `src/localization/keys.rs`
using UPPER_SNAKE_CASE (e.g., `CLI_FLAG_FILE_HELP` maps to `cli.flag.file.help`).

## 4. Variable usage

Variables are placeholders replaced with dynamic values at runtime.

### Syntax

```ftl
# Basic variable substitution.
error-at-path = Error at { $path }: { $details }

# Variables can appear multiple times.
range-error = Value { $value } must be between { $min } and { $max }.
```

### Variable types

Table 2: Variable types used in Fluent messages

| Type | Description | Example |
| ---- | ----------- | ------- |
| String | Text values | `{ $path }`, `{ $name }` |
| Number | Numeric values (used with plurals) | `{ $count }`, `{ $limit }` |
| Path | File system paths | `{ $path }`, `{ $directory }` |

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

## 5. Plural forms

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

Table 3: CLDR plural categories for common languages

| Language | Categories |
| -------- | ---------- |
| English | `one`, `other` |
| Spanish | `one`, `other` |
| French | `one`, `other` |
| Russian | `one`, `few`, `many`, `other` |
| Arabic | `zero`, `one`, `two`, `few`, `many`, `other` |
| Japanese | `other` (no grammatical plural) |

Consult the [CLDR plural rules](https://cldr.unicode.org/index/cldr-spec/plural-rules)
for the target language.

### Current limitation

The netsuke localization API currently passes all arguments as strings rather
than preserving numeric types. This means CLDR plural selectors like `[one]`
will not match as expected because Fluent requires numeric `FluentValue` types
for CLDR category selection.

**Workaround:** Messages will resolve using the default `*[other]` variant.
The FTL files include plural form examples demonstrating correct Fluent syntax
for future compatibility when numeric argument support is added.

## 6. Adding a new locale

To add support for a new language (e.g., French `fr-FR`):

### Step 1: Create the locale directory

```sh
mkdir -p locales/fr-FR
```

### Step 2: Copy the English source file

```sh
cp locales/en-US/messages.ftl locales/fr-FR/messages.ftl
```

### Step 3: Translate messages

Edit `locales/fr-FR/messages.ftl` and translate each message. Keep the same
keys; only change the values.

```ftl
# Before (English):
cli.about = Netsuke compiles YAML + Jinja manifests into Ninja build plans.

# After (French):
cli.about = Netsuke compile les manifestes YAML + Jinja en plans Ninja.
```

### Step 4: Update the localizer builder

Edit `src/cli_localization.rs` to include the new locale:

1. Add an embedded resource constant:

   ```rust
   const NETSUKE_FR_FR: &str = include_str!("../locales/fr-FR/messages.ftl");
   ```

2. Update `build_localizer()` to handle the new locale tag.

### Step 5: Run the build

The compile-time audit will verify all keys are present:

```sh
cargo build
```

If any keys are missing or orphaned, the build will fail with a detailed error.

### Step 6: Test the locale

```sh
cargo run -- --locale fr-FR --help
```

Verify the output appears in French.

## 7. Quality checklist

Before submitting translations, verify:

- [ ] All message keys from `en-US/messages.ftl` are present
- [ ] No extra (orphaned) keys exist
- [ ] All variables match the English source (same names, same count)
- [ ] Plural forms use correct CLDR categories for the target language
- [ ] Comments are translated or preserved for context
- [ ] The build passes (`cargo build`)
- [ ] The locale renders correctly (`netsuke --locale <tag> --help`)

## 8. Compile-time validation

Netsuke validates translations at compile time via `build_l10n_audit.rs`:

- **Missing keys**: Keys in `keys.rs` but not in the FTL file
- **Orphaned keys**: Keys in the FTL file but not in `keys.rs`

Both conditions cause the build to fail with a clear error message listing
the problematic keys.

## 9. Testing translations

Localization is tested via:

- **Unit tests** (`tests/localization_tests.rs`): Verify message rendering
- **Smoke tests**: Confirm secondary locales resolve correctly
- **Fallback tests**: Verify unsupported locales fall back to English

Run tests with:

```sh
make test
```

## 10. Resources

- [Project Fluent](https://projectfluent.org/) - Fluent documentation
- [Fluent Syntax Guide](https://projectfluent.org/fluent/guide/) - FTL syntax
- [CLDR Plural Rules](https://cldr.unicode.org/index/cldr-spec/plural-rules) -
  Plural categories by language
- [Unicode CLDR](https://cldr.unicode.org/) - Locale data repository
