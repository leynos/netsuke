# Migration guide: v0.1.x to v0.2.0

## Who should read this

Read this guide when upgrading a manifest from v0.1.x to v0.2.0. Two changes
are observable: `netsuke check` becomes standard, and a `{{ ins }}` or
`{{ outs }}` marker directly after an unpaired backslash is rejected in POSIX
and Bash recipes. Readers upgrading from a pre-beta5 build should also note the
two changes carried over from the v0.1.0 beta series.

## Impact at a glance

| Priority | Area                   | What to do                                                |
| -------- | ---------------------- | --------------------------------------------------------- |
| Required | POSIX and Bash recipes | Rewrite a marker that directly follows a single backslash |
| Required | Pre-beta5 Rust matches | Bind `reason` instead of the removed `snippet` field.     |
| Optional | `netsuke check`        | Start using the linter now that every build includes it   |
| Optional | Scripts parsing stderr | Tolerate the additional cause lines in human output.      |

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

## 3. Note changes carried over from the v0.1.0 beta series

Two changes recorded in the v0.1.0 migration guide arrived in `v0.1.0-beta5`,
before the v0.1.x baseline this guide starts from. They are repeated here for
readers upgrading from an earlier beta; a v0.1.x to v0.2.0 upgrade needs no
action for them unless a script parses human stderr.

### Human-mode failure causes

The `v0.1.0-beta5` pre-release prints the ordinary cause chain beneath the
context line in default human output for graph, interpolation, and
Ninja-generation failures. A missing rule therefore reports both the enclosing
build step and the failing reference. A cause that repeats the context line or
the cause above it is omitted, so each message appears once.

An interpolation rejection names the refused syntax, such as
`unmatched backtick`, instead of quoting the recipe. A rendered recipe can
contain a value resolved from the environment, so repeating it would disclose
that value. JSON output already listed the same chain in its `causes` array, so
the two modes agree at the same depth, except that human output omits a message
repeated by the layer above it.

No action is needed unless a script parses human-readable standard error; such
a script must tolerate the additional cause lines. See
[Interpret failures](users-guide.md#interpret-failures).

### `InvalidCommand` carries a fixed rejection reason

The `v0.1.0-beta5` pre-release changes `IrGenError::InvalidCommand`: its
`snippet: String` field is replaced by `reason: &'static str`. A rejection now
names the syntax it refused instead of quoting the rejected command, because a
rendered recipe can contain a value resolved from the environment. A pattern
match that binds or destructures `snippet` must change to bind the new field or
ignore it.

Before:

```rust,no_run
use netsuke::ir::IrGenError;

fn describe(err: IrGenError) -> String {
    match err {
        IrGenError::InvalidCommand { snippet, .. } => snippet,
        other => other.to_string(),
    }
}
```

After:

```rust,no_run
use netsuke::ir::IrGenError;

fn describe(err: IrGenError) -> String {
    match err {
        IrGenError::InvalidCommand { reason, .. } => reason.to_owned(),
        other => other.to_string(),
    }
}
```

The variant still carries the rejected command in its `command` field for
callers that inspect it; its `Display` renders the localized message alone.

## What does not need to change

- A backslash pair still decodes to one escaped backslash before the marker.
- A backslash inside single quotes stays literal.
- A backslash-newline continuation is unaffected.
- Heredoc bodies remain inert; only the delimiter word is scanned.
- A marker in a shell comment is inert; the comment is copied unchanged.
- PowerShell `command:` and `script:` recipes are unaffected.

## Migration checklist

- [ ] In POSIX and Bash `command:` and `script:` text, and in heredoc
      delimiter words, find the `{{ ins }}` and `{{ outs }}` markers that are
      actually interpolated. Markers in a heredoc body or in a shell comment
      are inert, so skip them.
- [ ] Among the rest, rewrite only a marker whose preceding run of backslashes
      has odd length, and likewise for `outs`. Adding one backslash makes the
      run even and turns the formerly active escape into a pair, as in
      `\\{{ ins }}`. Placing the marker in single quotes, as `'{{ ins }}'`,
      leaves the run untouched. A marker inside single quotes needs no change,
      because a backslash there escapes nothing, and an even run is already
      valid.
- [ ] Run `netsuke check` and resolve any findings worth adopting.

## Common errors and fixes

- **Error:** `Invalid command interpolation: escape sequence before a recipe
  marker`
  - **Fix:** the template contains a marker directly after a single backslash.
    Write `\\{{ outs }}` for a literal backslash, or place the marker inside
    single quotes.
