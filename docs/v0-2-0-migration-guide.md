# Migration guide: v0.1.x to v0.2.0

## Who should read this

Read this guide when upgrading a manifest from v0.1.x to v0.2.0. Two changes
are observable: `netsuke check` becomes standard, and a `{{ ins }}` or
`{{ outs }}` marker directly after an unpaired backslash is rejected in POSIX
and Bash recipes.

## Impact at a glance

| Priority | Area                   | What to do                                                |
| -------- | ---------------------- | --------------------------------------------------------- |
| Required | POSIX and Bash recipes | Rewrite a marker that directly follows a single backslash |
| Optional | `netsuke check`        | Start using the linter now that every build includes it   |

_Table 1: Obligations when moving from v0.1.x to v0.2.0._

## 1. Rewrite markers that follow a backslash

Netsuke previously copied a backslash that immediately preceded a marker into
the recipe. On POSIX and Bash routes the substituted path then decided what
that backslash escaped: in a double-quoted site a path beginning with `"` is
encoded as `\"`, so the retained backslash produced `\\"`. The shell read a
literal backslash followed by a closing quote, and the rest of the path reached
it as shell syntax.

Version 0.2.0 rejects the marker in `command:`, `script:`, and
heredoc-delimiter text before inserting any binding. PowerShell is unchanged,
because its escape character is the backtick. The decision is recorded in
[ADR-0027](adrs/adr-0027-command-placeholder-contract.md).

### Before

```yaml
netsuke_version: "1.0.0"

targets:
  - name: greeting.txt
    command: printf 'Hello from %s\n' \{{ outs }} > greeting.txt
```

### After

Write a pair when the recipe needs a literal backslash:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: greeting.txt
    command: printf 'Hello from %s\n' \\{{ outs }} > greeting.txt
```

Or place the marker inside single quotes, where backslashes are literal:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: greeting.txt
    command: printf 'Hello from %s\n' '{{ outs }}' > greeting.txt
```

## 2. Adopt `netsuke check`

In v0.1.x, `netsuke check` exists only in builds compiled with the `lint` Cargo
feature; release binaries built with the default feature set do not include it.
Version 0.2.0 removes that feature gate, so no feature flag is needed after
upgrading. Existing manifests need no changes: checking is read-only, and the
command analyses the same compiler artefacts as a build without running recipes
or creating build output.

Run the command against the selected manifest:

```sh
netsuke check
```

The linter reports likely defects, portability problems, and caching issues.
Each rule has a severity, and `--fail-on` selects the severity that makes the
command fail. Use `--rule` to change a rule or category, or to turn one off.
Suppression comments name the rules they silence and must include a reason;
there is no blanket disable. The complete rule list, policy options, and JSON
output contract are in the
[users' guide](users-guide.md#lint-a-manifest-with-netsuke-check).

## What does not need to change

- A backslash pair still decodes to one escaped backslash before the marker.
- A backslash inside single quotes stays literal.
- A backslash-newline continuation is unaffected.
- Heredoc bodies remain inert; only the delimiter word is scanned.
- PowerShell `command:` and `script:` recipes are unaffected.

## Migration checklist

- [ ] In POSIX and Bash `command:` and `script:` text, and in heredoc
      delimiter words, inspect the backslashes immediately before every
      `{{ ins }}` and `{{ outs }}` marker.
- [ ] Rewrite only a marker whose preceding run of backslashes has odd length,
      and likewise for `outs`. Adding one backslash makes the run even and turns
      the formerly active escape into a pair, as in `\\{{ ins }}`. Placing the
      marker in single quotes, as `'{{ ins }}'`, leaves the run untouched. An
      even run is already valid, so leave it unchanged; a marker inside single
      quotes needs no change either, because a backslash there escapes nothing.
      Heredoc bodies are inert, so nothing in them needs rewriting.
- [ ] Run `netsuke check` and resolve any findings worth adopting.

## Common errors and fixes

- **Error:** `Invalid command interpolation: <template>`
  - **Fix:** the template contains a marker directly after a single backslash.
    Write `\\{{ outs }}` for a literal backslash, or place the marker inside
    single quotes.
