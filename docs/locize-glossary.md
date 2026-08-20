# Netsuke Locize glossary

This document is the local source of truth for Netsuke terminology in Locize.
It follows the preferred, allowed, and forbidden term model described in the
[Locize glossary documentation][^1] and complements the
[translator guide](translators-guide.md).

## Schema

Each row in Table 1 represents one Locize term for one language. The first five
columns are the complete publishable record. A semicolon separates multiple
values in `allowed` or `forbidden`; an em dash means that the set is empty.

Netsuke's source catalogue is `en-US`, but the Locize project currently uses
`en` as its reference language. The glossary therefore uses `en` until that
project setting is corrected. The terms themselves follow the `en-US` source
catalogue and repository documentation.

Table 1: Locize glossary records for Netsuke

| title               | language | preferred           | allowed                                            | forbidden                                |
| ------------------- | -------- | ------------------- | -------------------------------------------------- | ---------------------------------------- |
| Netsuke             | en       | `Netsuke`           | `netsuke`                                          | —                                        |
| Netsukefile         | en       | `Netsukefile`       | `Netsuke manifest`; `manifest file`                | `NetsukeFile`; `Netsuke file`            |
| Ninja               | en       | `Ninja`             | `ninja`                                            | —                                        |
| Fluent              | en       | `Fluent`            | `Project Fluent`; `Fluent Translation List`; `FTL` | —                                        |
| manifest            | en       | `manifest`          | `Netsuke manifest`; `manifest file`                | `configuration file`                     |
| target              | en       | `target`            | `build target`; `default target`                   | `rule`                                   |
| action              | en       | `action`            | `build action`; `action catalogue`                 | `rule`                                   |
| dependency          | en       | `dependency`        | `direct dependency`; `order-only dependency`       | `dependancy`                             |
| dependencies        | en       | `dependencies`      | `direct dependencies`; `dependency graph`          | `dependancies`                           |
| standard library    | en       | `standard library`  | `template standard library`; `stdlib`              | —                                        |
| build graph         | en       | `build graph`       | `dependency graph`; `graph`                        | —                                        |
| `command_available` | en       | `command_available` | `command_available(name, **kwargs)`                | `command-available`; `command available` |

## Usage notes

- Use `Netsuke` for the product. Use lowercase `netsuke` only for the
  executable, package, crate, command, or code path.
- Use `Netsukefile` for the conventional manifest filename. Use `manifest` for
  the parsed YAML document or for manifests with another filename.
- Use `Ninja` for the build system and lowercase `ninja` only for its executable
  or a literal command.
- A target is a build output or named build entry. An action is an implicitly
  phony target. Neither is interchangeable with a Ninja rule.
- Preserve `command_available` exactly because it is a Jinja helper identifier.
- Keep forbidden values conservative. Add a value only for an observed
  misspelling, an identifier variant that would break a literal interface, or a
  collision between distinct Netsuke concepts.

[^1]: [Locize glossary documentation](https://www.locize.com/docs/glossary)
