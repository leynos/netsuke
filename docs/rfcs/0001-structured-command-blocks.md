# RFC 0001: Structured command blocks and argv templates

## Preamble

- **RFC number:** 0001
- **Amended by:** RFC 0009, RFC 0010, and RFC 0011
- **Status:** Proposed
- **Created:** 2026-08-20
- **Target:** Netsuke manifest format and execution intermediate representation

## 1. Summary

This RFC adds a structured command block to the `command` field while retaining
existing command strings and command-string lists without behavioural changes.
The new block provides environment overlays, file-backed standard streams,
teeing, and pipelines without requiring manifest authors to encode those
operations as shell source.

A structured command uses an ergonomic `invoke` string rather than a mandatory
YAML argument list. When `shell` is absent or `false`, Netsuke parses `invoke`
as an argument-vector (argv) template defined by Netsuke, not as POSIX shell or
PowerShell source. Netsuke records Jinja expressions as opaque template nodes,
evaluates them at manifest compilation time, and inserts their values directly
into argv elements. Rendered values are never reparsed as command syntax.

This ordering establishes the central safety property of the proposal:

> A scalar interpolation cannot change process structure or argv boundaries.

A list-valued interpolation may splice multiple arguments only when it occupies
an entire unquoted argv word. That typed splice is explicit in the template
structure and does not parse the resulting values.

When `shell: true` or a named `shell` selector is present, `invoke` is
deliberately treated as shell source. This keeps shell power available while
making the trust boundary visible. Shell mode does not inherit the direct mode
injection guarantee. [RFC 0011](0011-allow-listed-structured-command-shells.md)
defines the finite built-in and trusted configured shell registry.

## 2. Problem

Netsuke currently represents a command as either one shell string or a
non-empty list of shell strings. The list form improves readability and failure
reporting, but environment setup, redirection, pipelines, and command
composition still leak through shell text. The result is difficult to inspect,
less portable than the manifest schema, and easy to make unsafe when rendered
values contain whitespace or shell metacharacters.

The coverage action that motivated this RFC illustrates the problem:

```yaml
actions:
  - name: coverage
    command:
      - 'echo "coverage linker flags: -fuse-ld=lld"'
      - >-
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang
        RUSTFLAGS="-D warnings -C link-arg=-fuse-ld=lld"
        CFLAGS="-fuse-ld=lld"
        LDFLAGS="-fuse-ld=lld"
        cargo llvm-cov --lcov --output-path lcov.info
        --all-targets --all-features
```

The command contains three separate concerns in one shell-language string:

- child-process environment construction;
- argv construction and quoting;
- process invocation.

A conventional structured process API solves this by requiring an executable
and a YAML list of arguments. That representation is safe, but it is hostile to
ordinary command authoring. A six-word command becomes a tall stack of YAML
list items, and copying a documented command line requires manual tokenization.

Using a shell parser does not resolve the conflict. Tree-sitter can describe
shell or PowerShell syntax, but reconstructing the argv produced by a shell
requires implementing that shell's expansion rules. POSIX-like shells perform
quote removal, parameter expansion, command substitution, field splitting, and
pathname expansion. PowerShell has a different expression and native-command
argument model. A parser tree alone does not define the final argv.

Rendering Jinja first and then tokenizing the result creates a more direct
hazard. An interpolated semicolon, redirection operator, quote, or whitespace
sequence may become syntax or alter argument boundaries. Context-sensitive
quoting filters reduce that risk only when every interpolation context is known
and correctly classified.

Netsuke therefore needs a command notation that remains pleasant to write while
having semantics closer to `std::process::Command` than to a shell.

## 3. Current state

The current manifest recipe is a discriminated union containing exactly one of
`command`, `script`, or `rule`. A `command` is a scalar string or a non-empty
ordered list of strings. A scalar remains shell text. A list is lowered to
fail-fast shell groups joined by `&&`; its entries run in one shell process, so
shell variables, environment changes, and working-directory changes carry
between entries.

The current renderer delays `{{ ins }}` and `{{ outs }}` through opaque
internal markers, then lowers them to space-separated, shell-quoted paths.
Standalone `$in` and `$out` receive similar late substitution. This is already
a limited instance of the parse-or-classify-before-substitution principle
proposed here.

Netsuke compiles its intermediate representation (IR) to Ninja. Ninja remains
responsible for dependency scheduling and invokes the generated command text.
Direct process invocation, cross-platform stream wiring, and byte-preserving
teeing cannot be expressed portably by merely adding more shell quoting to that
text. Structured commands therefore require a small Netsuke-owned action runner
behind the generated Ninja edge.

The design document also contains a forward-looking `exec: { program, args }`
sketch. If accepted, this RFC supersedes that sketch with the more ergonomic
`command: { invoke: ... }` form.

## 4. Goals and non-goals

### 4.1 Goals

This RFC has the following goals:

- keep ordinary commands compact and easy to copy from tool documentation;
- make direct process invocation the default for structured command blocks;
- ensure scalar interpolation cannot introduce syntax or split arguments;
- support typed list interpolation for flags, inputs, and outputs;
- support exact per-process environment overlays;
- support per-block working directories and per-stage private temporary
  directories;
- model standard-input files, output files, teeing, and pipelines explicitly;
- support bounded stdout capture and selection of stdout or stderr as a pipe;
- preserve the existing behaviour of legacy command strings and all-string
  command lists;
- provide deterministic validation before Ninja starts work;
- produce precise diagnostics that identify the command item and pipeline stage;
- retain shell execution through an explicit Boolean or named `shell`
  boundary; and
- keep the generated build graph static and compatible with Ninja scheduling.

### 4.2 Non-goals

This RFC does not attempt to:

- emulate POSIX shell, `cmd.exe`, or PowerShell expansion in direct mode;
- infer build graph inputs or outputs from stream file paths;
- prevent a called program from interpreting an argument as an option,
  expression, query, or program fragment;
- sanitize shell source supplied under `shell: true` or a named selector;
- add runtime Jinja evaluation;
- add append-mode redirection or merged stdout and stderr;
- add rich environment operations such as `prepend`, `append`, `default`, or
  `unset`;
- replace Ninja's scheduler with a Netsuke scheduler; or
- require or standardize a Tree-sitter dependency.

## 5. Terminology

This RFC uses the following terms:

- **Legacy command:** An existing scalar command string or list item containing
  a string. Legacy commands retain shell semantics.
- **Structured command block:** A mapping with an `invoke` key and optional
  execution fields defined by this RFC.
- **Argv template:** The Netsuke-defined, shell-free notation accepted by
  `invoke` when `shell` is absent or `false`.
- **Shell name:** A validated registry-name identifier that selects one fixed
  shell executable and argument sequence. The initial built-in names are
  represented by the finite `BuiltInShell` enum; trusted configuration may add
  other names.
- **Word:** One syntactic argv position before typed expression expansion.
- **Scalar interpolation:** A Jinja expression that evaluates to a string,
  number, or Boolean value.
- **Sequence interpolation:** A Jinja expression that evaluates to a sequence
  of scalar values and occupies an entire unquoted word.
- **Execution unit:** A legacy shell group, structured process, structured
  pipeline, script, or referenced rule sequence that succeeds or fails as one
  step in the enclosing fail-fast sequence.
- **Pipeline stage:** One structured command block participating in a
  Netsuke-managed pipe chain.

## 6. Manifest syntax

### 6.1 `command` field forms

The `command` field accepts one of the following forms:

```yaml
# Existing scalar shell command.
command: cargo test

# Existing non-empty list of shell commands.
command:
  - export RUST_BACKTRACE=1
  - cargo test

# New structured command block.
command:
  invoke: cargo test
  env:
    RUST_BACKTRACE: "1"

# New heterogeneous command list.
command:
  - invoke: cargo build
  - rule: package
  - script: |
      printf '%s\n' 'package complete'
```

Formally, the user-facing union is:

```text
command       := legacy-string
               | command-block
               | non-empty-list<command-item>

command-item  := legacy-string
               | command-block
               | rule-item
               | script-item

rule-item     := { rule: string }
script-item   := { script: string }
```

A mapping used directly as the value of `command` must be a structured command
block. The `{ rule: ... }` and `{ script: ... }` mappings are valid only as
list items because recipe-level `rule:` and `script:` forms already cover the
single-item cases.

### 6.2 Structured command block fields

A structured command block has this shape:

```yaml
invoke: string
shell: false

env:
  NAME: value

stdin: path
stdout: path
stderr: path
tee: path
pipe: false
cwd: path
capture_stdout: 65536
temp_dir: true
```

| Field            | Type                         | Default             | Meaning                                                                                            |
| ---------------- | ---------------------------- | ------------------- | -------------------------------------------------------------------------------------------------- |
| `invoke`         | string                       | required            | Direct argv template, or shell source when `shell` is `true` or a name.                            |
| `shell`          | Boolean or shell name        | `false`             | Select direct, platform-default shell, or named allow-listed shell execution.                      |
| `env`            | mapping of string to string  | empty               | Exact child-environment overlay.                                                                   |
| `stdin`          | string path                  | inherited           | Read the child's standard input from a file.                                                       |
| `stdout`         | string path                  | inherited           | Write standard output to a truncated file.                                                         |
| `stderr`         | string path                  | inherited           | Write standard error to a truncated file.                                                          |
| `tee`            | string path                  | absent              | Copy standard output to inherited stdout and a truncated file.                                     |
| `pipe`           | Boolean or `stdout`/`stderr` | `false`             | Connect the selected stream to the next structured block's stdin. `true` is an alias for `stdout`. |
| `cwd`            | string path                  | effective directory | Run this block relative to the effective `-C` directory.                                           |
| `capture_stdout` | non-negative integer         | absent              | Retain at most this many stdout bytes for the action result.                                       |
| `temp_dir`       | Boolean                      | `false`             | Bind a private, per-stage runtime temporary directory.                                             |

Table 1: Structured command block fields.

Unknown keys are invalid. `invoke` must be present exactly once. The mapping
may not contain `rule` or `script`.

### 6.3 Reference and script items

A rule item contains exactly one key:

```yaml
- rule: package
```

The rendered value must name one existing rule. Rule-reference cycles remain
invalid. A rule item introduces an execution and stream boundary: pipelines do
not enter or leave the referenced rule. A referenced rule may define its own
structured pipeline internally.

A script item also contains exactly one key:

```yaml
- script: |
    printf '%s\n' 'generated by Netsuke'
```

It uses the existing recipe-level script semantics, including shebang and
interpreter handling. A script item also introduces an execution and stream
boundary in this RFC.

### 6.4 Coverage example

The motivating coverage command becomes:

```yaml
actions:
  - name: coverage
    command:
      - invoke: 'echo "coverage linker flags: -fuse-ld=lld"'
      - invoke: >-
          cargo llvm-cov --lcov --output-path lcov.info
          --all-targets --all-features
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -D warnings -C link-arg=-fuse-ld=lld
          CFLAGS: -fuse-ld=lld
          LDFLAGS: -fuse-ld=lld
```

Neither command requires shell quoting for the environment values. The quotes
around the message group literal whitespace into one argument; they are not
passed to `echo`.

## 7. Direct argv-template syntax

### 7.1 Selection

When `shell` is absent or `false`, `invoke` is parsed using the argv-template
language in this section. It is not parsed as shell, PowerShell, or Windows
command-line source.

The parser produces an ordered list of words. Each word contains literal and
Jinja-expression fragments. Quotes group fragments into one word and are
removed from the resulting argument. No shell expansion stage follows.

### 7.2 Conceptual grammar

The grammar below is descriptive. MiniJinja owns the grammar inside an
`expression` token, and its tokenizer must identify those tokens before the
argv lexer interprets surrounding quotes or whitespace.

```text
invocation       := whitespace* word (whitespace+ word)* whitespace*
word             := word-part+ | empty-single-quote | empty-double-quote
word-part        := unquoted-run | single-quoted | double-quoted | expression
empty-single-quote := "''"
empty-double-quote := '""'
unquoted-run     := unquoted-char+
unquoted-char    := any Unicode scalar except whitespace, U+0022, U+0027,
                    U+007C, U+0026, U+003B, U+003C, U+003E, or the start of
                    U+007B U+007B
single-quoted    := "'" (single-literal | expression)* "'"
single-literal   := single-char+
single-char      := any Unicode scalar except U+0027 or the start of
                    U+007B U+007B
double-quoted    := '"' (double-literal | escaped-quote
                    | escaped-backslash | expression)* '"'
double-literal   := double-char+
double-char      := any Unicode scalar except U+0022 or the start of
                    U+007B U+007B
escaped-quote    := U+005C U+0022
escaped-backslash := U+005C U+005C
expression       := MiniJinja expression token delimited by U+007B U+007B
                    and U+007D U+007D
whitespace       := U+0020 | U+0009 | U+000A | U+000D
```

The lexer checks for `expression` (the two-code-point start sequence `{{`)
before consuming any surrounding lexical production, including inside quotes.
Otherwise, `unquoted-char` excludes the four `whitespace` code points, U+0022
(`"`), U+0027 (`'`), and the direct-mode metacharacters U+007C (`|`), U+0026
(`&`), U+003B (`;`), U+003C (`<`), and U+003E (`>`). `single-char` excludes
only U+0027, and `double-char` excludes only U+0022; the `expression` check
therefore also applies within both literal productions. The escape alternatives
are checked before `double-char`, so only the two-code- point sequence `\"`
yields one literal `"` and only `\\` yields one literal `\`. Every other
backslash is literal, including a terminal backslash in an unquoted or
single-literal run; there is no general backslash escape and no dangling-escape
error. In double-quoted text, a backslash immediately before a quote is
therefore `escaped-quote`; a literal backslash before the closing quote must use
`\\`. A quote closes its corresponding quoted production unless it was
consumed by `escaped-quote`.

### 7.3 Quoting and escaping

Single and double quotes group literal text and expressions into one word. They
have no shell expansion semantics. In particular, single quotes do not suppress
Jinja evaluation.

Within double quotes:

- `\"` represents a literal double quote;
- `\\` represents one literal backslash; and
- a backslash before any other character remains a literal backslash.

Within single quotes, every character except the closing single quote is
literal, subject to Jinja-expression recognition. A value requiring a literal
single quote may use double quotes.

Outside quotes, a backslash is always literal. This rule keeps Windows paths
such as `C:\tools\bin` intact. Whitespace-containing literals must use quotes
rather than shell-style backslash escaping.

Adjacent literal, quoted, and expression fragments concatenate into one word:

```yaml
invoke: compiler --define=VERSION="{{ version }}" src/main.c
```

The second word is one argument whose fragments are the literal
`--define=VERSION=` and the rendered `version` value.

### 7.4 Reserved metacharacters

The characters `|`, `&`, `;`, `<`, and `>` are invalid outside quotes in direct
mode. This catches accidental pipelines, control operators, and redirections
rather than silently passing them as ordinary arguments.

A program that genuinely requires one of these characters may receive it by
quoting the containing word:

```yaml
invoke: matcher "alpha|beta" "x&y"
```

Other shell-looking text has no special meaning. Direct mode performs no
variable expansion, command substitution, globbing, tilde expansion, comment
recognition, or pathname expansion. For example, `$HOME`, `%TEMP%`, `*.rs`,
`$(date)`, backticks, and `#` are literal data unless Jinja itself produces a
value for an expression.

Standalone `$in` and `$out` are not supported in direct mode. Netsuke must emit
a targeted diagnostic recommending `{{ ins }}` or `{{ outs }}` instead.

### 7.5 Empty and malformed templates

An empty `invoke`, a template containing only whitespace, an unclosed quote, or
a malformed Jinja expression is rejected during manifest compilation. Empty
quoted words are valid and produce an empty argv element:

```yaml
invoke: tool "" final
```

This produces `tool`, an empty argument, and `final`.

NUL characters are invalid in every rendered program, argument, environment
name, environment value, and stream path.

## 8. Jinja interpolation semantics

### 8.1 Parse before evaluation

Direct invocation follows this fixed order:

1. Parse YAML and perform the existing manifest-time `foreach` and `when`
   expansion.
2. Deserialize the command-block shape.
3. Compile the `invoke` value into argv-template words containing literal and
   opaque expression nodes.
4. Evaluate each expression against the established compile-time Jinja context.
5. Convert scalar values and perform permitted sequence splices.
6. Validate the resulting program and argv.
7. Lower the result into the execution IR.

At no point is a rendered expression value fed back into the argv-template
parser. Quotation marks, whitespace, metacharacters, and Jinja delimiters in a
rendered value therefore remain data.

An implementation may use private sentinel values internally while compiling
the template, but the sentinels must never be representable by manifest text or
observable in diagnostics. A typed fragment representation is preferred over
round-tripping through a marker string.

### 8.2 Scalar values

A scalar expression contributes to its containing word without creating a new
word boundary. Spaces and metacharacters in its result remain part of the same
argv element.

The permitted scalar conversions are:

- strings are preserved exactly;
- integers use base-10 notation;
- finite floating-point values use Netsuke's stable manifest scalar rendering;
- Booleans become `true` or `false`; and
- undefined values, null values, mappings, and callable values are errors.

The following template is safe even when `destination` contains spaces, quotes,
or shell operators:

```yaml
invoke: frobnicate --output={{ destination }}
```

If `destination` is `release files; delete nothing`, the resulting argv is:

```text
["frobnicate", "--output=release files; delete nothing"]
```

### 8.3 Sequence values

A sequence expression may splice argv elements only when the expression is the
sole, unquoted content of a word:

```yaml
vars:
  coverage_args:
    - --all-targets
    - --all-features

command:
  invoke: cargo llvm-cov {{ coverage_args }}
```

The result is:

```text
["cargo", "llvm-cov", "--all-targets", "--all-features"]
```

Each sequence element must be a permitted scalar value. Nested sequences,
mappings, null values, undefined values, and callable values are invalid. An
empty sequence contributes zero argv elements.

A sequence is invalid when embedded in a composite or quoted word because that
word denotes exactly one argv position:

```yaml
# Invalid when flags is a sequence.
invoke: tool --flags={{ flags }}

# Also invalid when flags is a sequence.
invoke: tool "{{ flags }}"
```

The manifest author may use Jinja filters such as `join` to deliberately turn a
sequence into one scalar argument.

### 8.4 Inputs and outputs

In direct mode, `ins` and `outs` are sequences of path strings. They should
normally occupy whole unquoted words:

```yaml
invoke: clang {{ ins }} -o {{ outs }}
```

Every input and output path becomes one argv element, including paths
containing whitespace or metacharacters. An empty input list contributes no
arguments.

A command requiring one output may select it explicitly:

```yaml
invoke: clang {{ ins }} -o {{ outs[0] }}
```

In shell mode and legacy commands, existing shell-quoted, space-joined `ins`/
`outs` behaviour remains unchanged.

### 8.5 Examples

| Template                  | Expression value        | Result                            |
| ------------------------- | ----------------------- | --------------------------------- |
| `tool {{ value }}`        | `"a b; c"`              | `["tool", "a b; c"]`              |
| `tool --name={{ value }}` | `"a b"`                 | `["tool", "--name=a b"]`          |
| `tool {{ flags }}`        | `["-x", "a b"]`         | `["tool", "-x", "a b"]`           |
| `tool "{{ flags }}"`      | `["-x", "a b"]`         | validation error                  |
| `{{ prefix }} status`     | `["cargo", "+nightly"]` | `["cargo", "+nightly", "status"]` |
| `tool {{ empty }}`        | `[]`                    | `["tool"]`                        |

Table 2: Direct-mode interpolation examples.

## 9. Direct process semantics

After interpolation and sequence splicing, the first argv element names the
program and all remaining elements are arguments. The final argv must contain
at least one non-empty program element.

Netsuke passes each argument as a distinct value to the platform process API.
It does not add, remove, or interpret quotes after argv-template compilation.
It performs no shell word splitting, variable substitution, globbing, or
redirection.

Executable resolution uses the effective child environment after applying the
command block's `env` overlay. A bare program name is resolved through the
effective `PATH` according to Netsuke's platform resolver. A value containing a
path separator is treated as a path relative to the effective working directory
unless it is absolute.

The effective working directory starts with the directory selected by the
command-line `-C` option. A block without `cwd` runs there. When `cwd` is
present, Netsuke renders it as a path and resolves it relative to that
effective directory; an absolute path remains absolute. Each block resolves its
own `cwd`, so a pipeline stage does not inherit the preceding stage's working
directory. The resolved directory must exist and be a directory before the
stage starts. This block or stage directory is the base for all relative stream
paths and is the child's working directory.

On Windows, direct mode must reject a resolved `cmd.exe`, `.bat`, or `.cmd`
target. Rust preserves Windows batch-file launching through `cmd.exe`, whose
non-standard argument parser can reinterpret otherwise distinct arguments as
shell source. The diagnostic must recommend an explicit `shell: true` block or
a suitable named shell. A future RFC may define a narrower trusted opt-in.

This restriction does not make arbitrary programs safe interpreters. A program
may interpret one argument as SQL, a regular expression, a `find` predicate, a
PowerShell command, or another language. Netsuke preserves the argv boundary;
it cannot validate every callee's application grammar.

## 10. Shell mode

### 10.1 Selection and source handling

The `shell` field is a `Boolean | ShellName` union. Here `ShellName` is a
validated registry-name type; the finite initial built-in set is represented by
`BuiltInShell`, and trusted configuration may add names such as `dash`. An
absent field or `false` selects direct argv execution. `true` selects the
platform default shell. A string selects an allow-listed name from the built-in
or trusted configured registry.

In either shell case, Netsuke does not apply the argv-template grammar. It
renders `invoke` as one shell-source string using the existing command-template
context and launches the resolved shell through the structured action runner.

The initial platform defaults are:

- `/bin/sh -c` on Unix-like platforms; and
- `powershell.exe -NoLogo -NoProfile -NonInteractive -Command` on Windows.

The initial built-in names are `sh`, `bash`, `pwsh`, and `powershell`, with
platform support, executable resolution, and fixed arguments defined by
[ADR-019](../adr-019-structured-command-shell-selection.md). `CliConfig` may
add names, but a Netsukefile cannot supply an executable or interpreter
arguments. [RFC 0011](0011-allow-listed-structured-command-shells.md) defines
the schema, registry, diagnostics, and lowering amendment.

### 10.2 Interpolation boundary

Jinja evaluation remains compile-time, but rendered values become shell source
in this mode. An expression may therefore alter quoting, operators, expansion,
redirection, or control flow. `shell: true` and a named selector are explicit
opt-ins to that behaviour and carry no direct-mode injection guarantee.

Shell-specific escaping filters remain available where suitable. Netsuke must
not claim that Tree-sitter validation or quote balancing makes arbitrary shell
interpolation safe.

### 10.3 Structured facilities still apply

The `env`, `cwd`, `stdin`, `stdout`, `stderr`, `tee`, `capture_stdout`,
`temp_dir`, and `pipe` fields retain their Netsuke-managed meanings in shell
mode. A singular block, sequence item, or pipeline stage may select a name.
Each stage retains independent working-directory and stream semantics, so a
shell block may participate in a structured pipeline without placing the outer
pipe operator in shell source.

## 11. Environment semantics

### 11.1 Exact overlay

The initial `env` syntax is a mapping from rendered strings to rendered strings:

```yaml
env:
  RUSTFLAGS: -D warnings
  OUTPUT_DIR: "{{ output_dir }}"
```

The child inherits Netsuke's execution environment. Every block entry then sets
or replaces exactly one variable for that child process. The overlay is local
to the structured block and does not persist to later blocks.

Future enclosing rule, target, or action environment mappings should layer
before the command-block mapping. The reserved precedence is:

1. inherited process environment;
2. enclosing rule environment;
3. target or action environment; and
4. command-block environment.

Only level 4 is standardized by this RFC.

### 11.2 Rendering and validation

Environment keys and values are rendered at manifest compilation time. A key
must be non-empty and contain neither `=` nor NUL. A value must not contain
NUL. Sequence and mapping values are invalid in this initial form.

Duplicate keys produced after rendering are invalid. On Windows, duplicate
comparison is case-insensitive while preserving the spelling of the surviving
key for diagnostics.

The `env()` Jinja helper reads the manifest compiler's environment. It does not
observe the block overlay being defined because that overlay applies only when
the child process starts.

### 11.3 Runtime temporary directories

When `temp_dir: true`, the runner creates a private directory for that stage
before spawning it, binds the stage's `TMPDIR`, `TMP`, and `TEMP` variables to
that directory, and removes the directory after the stage and its associated
relays have finished. The directory is created outside the project tree with
owner-only permissions, or the closest platform equivalent. A stage receives
its own directory; sequential stages do not share one, and the directory is not
an action-plan sidecar.

The temporary-directory bindings are applied after the `env` overlay and take
precedence over entries with the same names. With `temp_dir: false`, `env` may
set those variables normally, but Netsuke does not create or remove a
directory. The runner never exposes the generated path to a later execution
unit or persists it in the action plan.

### 11.4 Deferred operations

This RFC deliberately defers object-valued operations such as `default`,
`prepend`, `append`, and `unset`. They require ordered merge semantics across
rule, target, action, and command levels. Exact string replacement provides a
small, deterministic first surface and covers the motivating use case.

## 12. Standard-stream semantics

### 12.1 Path resolution

Every stream path is rendered at manifest compilation time and resolved
relative to the block's resolved `cwd`. An absent `cwd` resolves to the
effective working directory after command-line `-C` processing. Netsuke opens
the path when the action executes, not while the manifest is compiled.

Validation occurs independently when each execution unit starts. For a maximal
structured pipeline, the unit's destination set is the union of `stdin`,
`stdout`, `stderr`, and `tee` from all its stages, resolved using each stage's
own `cwd`; all those destinations are checked together. A different sequential
execution unit starts a new validation set and does not compare its
destinations with a previous unit. Sequential output reuse is therefore
permitted: after one unit succeeds, a later unit may resolve and open that
unit's output as its `stdin`.

At unit start, the runner records the current identity (including an explicit
absent state) of every configured destination in that unit's set. It then opens
a provisional handle for each destination without truncation. Input handles are
read-only and never create files. For an output whose recorded identity is
absent, provisional creation must be atomic and exclusive (`O_CREAT | O_EXCL`,
or the closest platform-equivalent safe handle semantics). If another actor
creates that path first and exclusive creation reports `EEXIST`, the execution
unit is rejected as a race; the runner must not reopen or truncate that path.
Existing output handles use non-following flags and must not truncate during
provisional open.

For each provisional handle, the runner freshly resolves the current path and
compares that identity with the identity obtained from the handle. A path that
disappears, is replaced, changes symlink target, or cannot be resolved safely
is rejected. A replacement, disappearance, or unexpected creation by another
actor between the initial resolution and provisional open is also rejected. It
next compares every pair of opened-handle identities. Relative aliases,
symlinks, and hard links must therefore be detected, including `stdin` versus
every output destination, wherever the platform provides filesystem identity
information. All provisional handles are closed on rejection.

Only after every opened-handle identity and pairwise-distinctness check within
the execution unit succeeds may the runner truncate output handles and bind
them to child streams. The validation also applies when a destination is used
alongside a pipeline; the pipeline restrictions below remain in force. The
runner never reopens a destination by path after provisional open; truncation
and stream binding use only the validated handles.

Stream paths do not implicitly become target sources, dependencies, or outputs.
Manifest authors must continue to declare graph relationships explicitly.
Netsuke does not create missing parent directories.

### 12.2 Standard input

When `stdin` is absent, a non-pipeline process inherits the action runner's
standard input policy. When present, Netsuke opens the named file for reading
and supplies it as the child's standard input.

A block that receives standard input from a preceding structured pipe may not
also specify `stdin`.

### 12.3 Standard output

When none of `stdout`, `tee`, or `pipe` is selected, the child inherits the
action runner's standard output.

`stdout` uses its validated output handle, truncates the named file after all
destination checks succeed, and directs the child's standard output only to
that file.

`tee` uses its validated output handle, truncates the named file after all
destination checks succeed, and copies the child's standard output byte-for-
byte to both that file and the action runner's inherited standard output.
Netsuke treats a read or write failure in the tee path as an execution failure
even when the child exits successfully.

`capture_stdout` directs standard output to a bounded in-memory capture. Its
non-negative integer value is the maximum number of bytes retained. If the
child writes more than that limit, the runner fails the execution unit rather
than silently truncating the capture. Captured bytes remain available only in
the runner's bounded result and diagnostics until the unit completes; they are
then discarded and cannot be consumed by a later unit.

`pipe: true` or `pipe: "stdout"` directs the child's standard output to the
next structured command block's standard input. `true` is exactly an alias for
`"stdout"`.

`stdout`, `tee`, `capture_stdout`, and a stdout pipe are mutually exclusive.

### 12.4 Standard error

When `stderr` is absent, the child inherits the action runner's standard error.
When present, Netsuke uses its validated output handle, truncates the named
file after all destination checks succeed, and directs standard error to it.

Standard error is independent of the stdout selection. This RFC does not define
stderr teeing or `2>&1`-style stream merging. `pipe: "stderr"` directs the
child's standard error to the next structured command block's standard input.
It is mutually exclusive with `stderr`. A block may pipe stderr while selecting
an independent stdout behaviour, including `tee` or `capture_stdout`.

No two configured stream destinations in one execution unit may identify the
same file. Opening the same file through independent handles would give
ambiguous ordering and file-offset semantics. This pairwise check occurs before
any output handle is truncated; destinations in separate sequential units are
not part of this check.

### 12.5 File modes and byte handling

All output files use create-or-truncate semantics, implemented by truncating
the already-validated output handles only after every destination check
succeeds. Append mode is reserved for a future object-valued stream syntax.
Pipes and tee operations carry arbitrary bytes and do not require UTF-8 output.

Failure to open a stream path, write a tee output, or relay inherited output
fails the execution unit and stops the enclosing command sequence.

## 13. Pipeline semantics

### 13.1 Formation

A block with `pipe: true`, `pipe: "stdout"`, or `pipe: "stderr"` must be
followed immediately by another structured command block in the same source
command list. The next item may not be a legacy string, rule item, or script
item. The final item in a command list may not select a pipe.

A maximal chain of such adjacent blocks forms one structured pipeline:

```yaml
command:
  - invoke: producer --format=json
    pipe: true
  - invoke: jq ".items[]"
    pipe: true
  - invoke: consumer
    stdout: result.txt
```

Pipelines do not cross rule-reference or script boundaries. A referenced rule
may contain its own complete pipeline, but callers cannot pipe into its first
stage or out of its last stage in this RFC.

### 13.2 Execution

Netsuke creates operating-system pipes and starts the stages as one execution
unit. It starts every stage and every required drain or tee relay before
waiting for any stage. Each child inherits only its assigned standard handles:
the selected input pipe, the selected output or relay pipe, and its configured
standard handles. It never inherits an unrelated pipe end. After spawning each
stage, the runner closes its duplicate and otherwise unused pipe ends
immediately, retaining only ends needed by a later spawn or by a relay. Once
the final writer has been spawned, the runner closes its own duplicate writer
end so downstream readers can observe EOF.

Standard output or standard error selected by a non-final stage feeds standard
input of the next stage. The final stage applies its own inherited, `stdout`,
`tee`, or `capture_stdout` behaviour.

If a stage cannot be spawned, Netsuke closes every managed pipe end, terminates
and reaps any stages already started for that pipeline. If any managed pipe,
relay, or tee I/O fails, including a relay read failure or a tee write failure,
Netsuke closes the affected pipe ends, terminates every still-running stage,
and reaps every stage that was started. Otherwise, after all stages and relays
have started successfully, Netsuke waits for every stage and drains every
managed stream.

Before reporting either success or failure, the runner joins every started
drain or tee relay. No relay remains detached after stage statuses have been
collected.

The pipeline succeeds only when:

- every stage exits successfully;
- every required stream open succeeds; and
- every Netsuke-managed relay or tee operation succeeds.

This is equivalent to a strict `pipefail` policy, but Netsuke derives it from
the child statuses directly rather than from shell configuration. When multiple
stages fail, diagnostics report every failed stage in lexical order. The
pipeline's failure stops the enclosing sequence.

## 14. Command-list sequencing

### 14.1 Fail-fast order

Execution units run in declaration order. Netsuke starts the next unit only
when the previous unit succeeds. A failure stops the sequence.

Stream validation is repeated at the start of each unit, rather than across the
whole command list. Consequently, sequential output reuse is permitted: the
next unit may open a successful earlier unit's output as `stdin`. Stages within
one maximal pipeline remain one validation set and may not alias one another's
stream destinations.

### 14.2 Legacy string groups

To preserve current semantics, every maximal contiguous run of legacy string
items forms one legacy shell group. Entries within that group continue to run
in one shell process through the existing fail-fast lowering. Shell variables,
environment changes, and working-directory changes carry within the group.

A command list containing only strings therefore behaves exactly as it does
before this RFC.

A structured block, rule item, or script item ends the current legacy group.
Process state does not cross that boundary:

```yaml
command:
  - export MODE=fast
  - echo "$MODE"       # Same legacy shell group: prints fast.
  - invoke: tool
  - echo "$MODE"       # New legacy shell group: MODE is not retained.
```

Persisted configuration should use explicit fields such as `env`, not shell
state that crosses execution-model boundaries.

### 14.3 Rule and script units

A rule item executes the referenced rule as a nested fail-fast sequence. Its
success permits the enclosing sequence to continue; its failure stops the
enclosing sequence. Rule expansion must retain enough provenance for
diagnostics to identify both the outer list item and the referenced rule's
inner item.

A script item executes as one unit using existing script semantics. Neither
form shares shell process state with neighbouring units.

## 15. Validation rules

Netsuke must reject invalid structured commands before launching Ninja wherever
the required information is available at compile time.

### 15.1 Shape validation

- `command` must be a string, a structured command block, or a non-empty list.
- Every command-list mapping must contain exactly one of `invoke`, `rule`, or
  `script`.
- A mapping containing `invoke` may contain only the command-block fields.
- A mapping containing `rule` or `script` may contain no other field.
- `rule` and `script` mappings are invalid as the direct value of `command`.
- Unknown fields are invalid.
- Field values must have the documented YAML types.

### 15.2 Template validation

- Direct `invoke` values must satisfy the argv-template grammar.
- Structural Jinja blocks remain forbidden in string fields.
- Every Jinja expression must compile and evaluate at manifest compilation time.
- Composite-word expressions must produce permitted scalar values.
- Sequence splices must occupy one complete unquoted word and contain only
  permitted scalar elements.
- The rendered argv must contain a non-empty program.
- Environment and path templates must render to scalar strings.

### 15.3 Stream and pipeline validation

- At most one of `stdout`, `tee`, `capture_stdout`, or a stdout pipe may be
  selected. `pipe: true` is an alias for `pipe: "stdout"`.
- `pipe: "stderr"` is mutually exclusive with `stderr`; stderr piping does not
  prevent an independent stdout selection.
- A pipe selection is invalid on a singular command block or the final list
  item. The item after a pipe must be a structured command block.
- A pipeline recipient may not specify `stdin`.
- Pipelines may not cross rule, script, or legacy-string boundaries.
- All configured destinations among `stdin`, `stdout`, `stderr`, and `tee` in
  one execution unit must be provisionally opened without truncation using safe
  non-following handles, or the closest platform equivalent. Each handle must
  match a fresh path identity, and every pair of handles must have a distinct
  file identity. Validation must detect relative path aliases, symlinks, hard
  links, path replacement, disappearance, and unexpected creation before any
  output handle is truncated. Provisional handles are closed on rejection.
  Destinations in separate sequential units are checked independently.
- `capture_stdout` must be a non-negative byte limit. It is mutually exclusive
  with `stdout`, `tee`, and a stdout pipe; a limit violation fails the unit.
- `cwd` must resolve to an existing directory before its execution unit starts.
- `temp_dir: true` creates one private runtime directory for each stage and
  binds the conventional temporary-directory environment variables to it.

### 15.4 Reference validation

- Every rule item must resolve to an existing rule.
- Rule-reference cycles remain invalid, including cycles introduced through
  command-list rule items.
- A referenced rule's internal pipeline must be valid without relying on an
  outer caller.

### 15.5 Runtime validation

The following failures necessarily remain runtime errors:

- executable resolution or spawn failure;
- Windows batch-target rejection after executable resolution;
- stream file open or permission failure;
- missing runtime input files;
- child non-zero exit or signal termination; and
- pipe, relay, or tee I/O failure.

## 16. Diagnostics and observability

Diagnostics should identify:

- the enclosing action, target, or rule;
- the one-based command-list item;
- the referenced rule and inner item where applicable;
- the one-based pipeline stage;
- the program or shell mode involved; and
- the exit code, signal, spawn error, or stream error.

Diagnostics must not print complete environment mappings by default. Rendered
argv may also contain credentials, tokens, or private paths, so ordinary errors
should use bounded command summaries and existing redaction conventions.
Verbose or debug modes may expose more detail only under the project's
established telemetry and diagnostic policies.

Structured execution should emit stable categorical telemetry such as direct
versus shell mode, pipeline stage count, redirection kinds, and success or
failure category. It must not use program names, arguments, environment names,
environment values, or file paths as unbounded metric labels.

## 17. Compiler and runner architecture

### 17.1 AST representation

The AST should preserve user intent and source provenance. One illustrative
shape is:

```rust
pub enum CommandItem {
    Legacy(String),
    Invoke(CommandBlock),
    RuleRef(String),
    Script(String),
}

pub struct CommandBlock {
    pub invoke: String,
    pub shell: ShellSelection,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<Utf8PathBuf>,
    pub stdin: Option<Utf8PathBuf>,
    pub stdout: Option<Utf8PathBuf>,
    pub stderr: Option<Utf8PathBuf>,
    pub tee: Option<Utf8PathBuf>,
    pub pipe: PipeStream,
    pub capture_stdout: Option<usize>,
    pub temp_dir: bool,
}

pub enum ShellSelection {
    Direct,
    PlatformDefault,
    Named(ShellName),
}

pub enum BuiltInShell {
    Sh,
    Bash,
    Pwsh,
    PowerShell,
}

pub enum PipeStream {
    None,
    Stdout,
    Stderr,
}
```

The concrete implementation may use template wrapper types instead of rendered
strings. It must retain locations or item indexes for diagnostics.

### 17.2 Compiled direct template

A direct `invoke` should compile into typed words rather than an intermediate
shell string:

```rust
pub struct ArgTemplate {
    pub words: Vec<WordTemplate>,
}

pub struct WordTemplate {
    pub fragments: Vec<ArgFragment>,
    pub splice_eligible: bool,
}

pub enum ArgFragment {
    Literal(String),
    Expression(CompiledExpression),
}
```

The exact types are non-normative. The required invariant is that expression
results enter the rendered word or argv splice directly and never re-enter the
lexer.

### 17.3 Execution IR

After rendering and validation, the IR should distinguish execution semantics
explicitly:

```rust
pub enum ExecutionUnit {
    LegacyShellGroup(Vec<String>),
    Process(ProcessSpec),
    Pipeline(Vec<ProcessSpec>),
    Script(ScriptSpec),
    RuleSequence(RuleId),
}

pub enum ProcessKind {
    Direct { program: String, args: Vec<String> },
    Shell { source: String, shell: ResolvedShell },
}

pub struct ProcessSpec {
    pub kind: ProcessKind,
    pub cwd: Utf8PathBuf,
    pub env: BTreeMap<String, String>,
    pub streams: StreamBindings,
    pub capture_stdout: Option<usize>,
    pub temp_dir: bool,
}
```

`StreamBindings` is responsible for the validated file handles and selected
pipe ends. `cwd` is resolved per block before stream paths are resolved, and
`temp_dir` is materialized per stage at execution time; neither value is
inherited from a preceding sequential unit.

`ResolvedShell` contains the allow-listed registry name, resolved executable,
and fixed invocation arguments. The compiler resolves `PlatformDefault` and
`Named` before constructing the IR. The runner does not consult `CliConfig` or
ambient environment state again.

The IR must contain no Ninja-specific quoting. Backend escaping remains the
responsibility of Ninja synthesis.

### 17.4 Ninja action runner

Ninja should continue to schedule build edges. For a structured execution unit,
Netsuke should emit a generated command that invokes a hidden Netsuke action
runner with an opaque action identifier. The runner loads a versioned action
plan, constructs child processes with the platform process API, wires streams,
and returns one status to Ninja.

A conceptual generated edge is:

```text
<current-netsuke-executable> __run-action --plan <private-plan> --id <action-id>
```

The exact hidden subcommand and serialization format are implementation
details. The following requirements are normative:

- action plans are versioned and validated before execution;
- the generated command contains no rendered user arguments or environment
  values;
- the action identifier is opaque and cannot select data outside the plan; and
- diagnostics and telemetry do not dump the complete plan by default.

Action-plan lifetime depends on how the Ninja manifest was produced:

- During a build that Netsuke launches, action plans live outside the project
  tree in a private temporary directory. Netsuke leases each plan exclusively
  to the generated Ninja edge, and the lease remains held through Ninja and all
  action-runner children. Owner-only permissions, or the closest platform
  equivalent, protect the plan. Normal cleanup removes the plan after the lease
  ends, with bounded stale-file cleanup after abnormal termination.
- For `netsuke generate --output <manifest>`, generation does not launch Ninja,
  so a temporary plan cannot be removed when generation exits. Netsuke instead
  publishes each plan as a private sidecar associated with the generated output
  and records the association in a versioned manifest-local index. Publication
  is atomic: acquire an exclusive lease for the output, write and validate the
  new sidecar, then atomically replace the index that names the complete
  sidecar set. The sidecar remains available for later Ninja invocations and is
  protected by the same owner-only permissions. Replacing or deleting the
  generated output releases its lease and removes its associated sidecars;
  bounded stale-sidecar cleanup handles interrupted publication and abandoned
  leases without scanning unbounded paths.

The generated-output association must not allow a manifest or action identifier
to select a plan outside its private sidecar set. A later `netsuke generate`
must either publish a complete replacement or leave the previous usable
association intact.

A recipe containing only legacy command text may continue to lower directly to
Ninja command text. Once a recipe contains a structured block, rule item, or
script item, its complete normalized sequence runs through the action runner,
including any embedded legacy shell groups. Pure legacy and structured paths
may converge on the runner in a later RFC, but that is not required for initial
delivery.

## 18. Security properties and limitations

### 18.1 Properties provided by direct mode

Subject to the called program's platform argument decoder, direct mode provides
these properties:

- scalar expression values cannot create or remove argv boundaries;
- scalar expression values cannot introduce Netsuke pipeline or redirection
  syntax;
- quote characters and shell operators in values remain ordinary argument data;
- only a whole-word sequence expression can intentionally change argv length;
- environment values are passed through the child environment rather than
  shell assignments; and
- stream topology comes from validated fields rather than rendered source.

These properties are amenable to property-based testing because the output argv
can be compared directly with the typed template evaluation.

### 18.2 Properties not provided

Direct mode does not prevent:

- a rendered program value from selecting an unintended executable;
- a sequence value from introducing additional options;
- a value beginning with `-` from being interpreted as an option;
- an argument from being interpreted as code by the called program;
- unsafe use of `shell: true`, a named shell, or legacy command strings; or
- hostile behaviour by the executable itself.

Manifest authors remain responsible for program-specific boundaries such as
`--`, safe query parameterization, and restricted expression languages.

### 18.3 Tree-sitter and `shlex`

Tree-sitter may later support diagnostics or editor tooling for shell-mode
source, but it is not the execution semantics. Reproducing final argv from a
shell syntax tree would require reproducing the shell's expansion engine.

A POSIX `shlex` implementation may inspire or assist lexer tests, but the
Netsuke grammar deliberately differs in its treatment of backslashes, reserved
operators, Jinja nodes, and Windows paths. The RFC, rather than a third-party
library's current behaviour, defines the language.

## 19. Compatibility and migration

### 19.1 Existing manifests

The proposal is additive:

- scalar command strings retain their current shell semantics;
- all-string command lists retain their current one-shell, fail-fast semantics;
- recipe-level `script:` and `rule:` forms remain valid;
- existing `{{ ins }}`, `{{ outs }}`, `$in`, and `$out` behaviour remains for
  legacy and shell-mode commands; and
- no existing manifest is automatically converted to direct invocation.

The named selector is additive syntax. An absent field and both Boolean values
retain their existing meanings; a string selects a shell understood only by an
implementation of [RFC 0011](0011-allow-listed-structured-command-shells.md).

A heterogeneous list is new syntax. Its contiguous legacy-string grouping and
state boundaries are therefore not a compatibility change.

### 19.2 Manifest version

Implementation should allocate an additive manifest-format minor version,
provisionally `1.1.0`, so older Netsuke versions reject the new mapping syntax
cleanly rather than misinterpret it. Named shell selection ships within that
not-yet-implemented mapping schema and does not require a second increment. If
structured mappings ship first, named selection must use the next additive
minor version. The implementation pull request may select a different minor
version if intervening schema work consumes the provisional number.

### 19.3 Documentation migration

After acceptance and implementation:

- the user's guide should document structured command blocks as the preferred
  form for ordinary external tools;
- the design document's planned `exec: { program, args }` sketch should be
  replaced by this RFC's accepted form;
- the existing rich environment-operation sketch should remain explicitly
  deferred or move to a follow-up RFC;
- examples requiring shell state should retain legacy syntax or use
  `shell: true` or a named selector; and
- a migration guide should show mechanical conversions for environment
  assignments, redirections, and simple pipelines.

## 20. Implementation plan

### 20.1 Phase 1: Schema and template compiler

- Add the heterogeneous command-item AST and deny unknown fields.
  - Add Boolean-or-name shell selection, the finite `BuiltInShell` vocabulary,
    and the validated configured `ShellName` type defined by RFC 0011.
- Implement the argv-template lexer and typed fragment representation.
- Integrate MiniJinja expression compilation without render-then-reparse.
- Add shape, interpolation, and pipeline-topology validation.
- Preserve source provenance for diagnostics.

### 20.2 Phase 2: Execution IR

- Add direct process, shell process, stream, pipeline, and sequence IR types.
- Resolve platform-default and named shell selections through the trusted
  registry, then carry the complete resolved invocation in the shell IR.
- Normalize maximal legacy string runs into legacy shell groups.
- Resolve rule items while preserving execution boundaries and cycle checks.
- Hash the fully rendered execution plan for deterministic graph inspection.

### 20.3 Phase 3: Action runner

- Add the hidden, versioned action-runner entry point.
- Implement direct spawn, environment overlay, stream files, teeing, and strict
  pipeline status collection.
- Add private action-plan materialization with exclusive leases, bounded stale
  cleanup, and persistent generated-output sidecars.
- Add Windows batch-target rejection.

### 20.4 Phase 4: Ninja lowering

- Generate action-plan sidecars and runner commands for structured units.
- Keep legacy command lowering unchanged.
- Ensure build-time leases cover every Ninja child and generated-output plans
  remain available after `generate` exits.
- Extend graph and debug views to distinguish direct, shell, and pipeline units
  without exposing secrets.

### 20.5 Phase 5: Documentation and migration

- Update the design document, user's guide, migration guide, roadmap, and
  repository layout where required.
- Add end-to-end examples for Unix-like systems and Windows.
- Record any accepted implementation refinements in an Architecture Decision
  Record (ADR) when they represent stable architecture rather than RFC detail.

## 21. Test strategy

The implementation should include:

- lexer unit tests covering quotes, empty words, Windows paths, reserved
  metacharacters, and malformed templates;
- property tests showing that arbitrary scalar values remain within their
  original argv word;
- property tests showing that sequence values splice exactly their elements and
  nothing else;
- a helper executable that records argv, environment, stdin bytes, stdout bytes,
  stderr bytes, and exit status for cross-platform integration tests;
- per-block and per-stage `cwd` tests, including relative stream-path
  resolution and independent pipeline-stage directories;
- bounded stdout-capture tests for the limit, overflow failure, stream
  exclusivity, discarded completion lifetime, and captured-byte diagnostics;
- per-stage temporary-directory tests for `TMPDIR`, `TMP`, and `TEMP`
  precedence, isolation, cleanup, and distinction from action-plan sidecars;
- pipeline tests where the first, middle, and final stages fail independently;
- finite pipelines that verify unused pipe ends close, downstream stages
  receive EOF, all relays join, and successful stages terminate;
- spawn-failure tests that verify already-started stages are reaped;
- high-volume pipeline tests that verify all stages and drain or tee relays
  start before waiting, plus tee-write-failure and relay-read-failure tests
  that verify affected pipes close, remaining stages terminate, and every
  started stage is reaped;
- byte-oriented tee tests including non-UTF-8 output;
- truncate and path-resolution tests for every stream field and every pair of
  configured stream destinations, including relative aliases, symlinks, and
  hard links, verifying that collisions are rejected before any output
  truncation; a deterministic race test must use a test-controlled barrier to
  record an absent output identity, have a competing process create the path
  before provisional open, and verify that exclusive creation loses with
  `EEXIST`, rejects the execution unit, and does not truncate the competing
  process's file;
- action-plan lifecycle tests covering build-time cleanup after all Ninja and
  runner children finish, persistent `generate --output` sidecars, exclusive
  leases, atomic publication, replacement and deletion cleanup, and bounded
  stale-sidecar cleanup;
- duplicate environment-key tests, including Windows case folding;
- compatibility snapshots for existing scalar and all-string-list lowering;
- compatibility tests proving absent and `false` remain direct while `true`
  retains the platform default;
- cross-platform tests for every built-in name, configured names, invalid
  definitions, unsupported hosts, and unavailable executables;
- rule-cycle and pipeline-boundary tests;
- Windows `.bat`, `.cmd`, and `cmd.exe` rejection tests;
- behavioural tests for the motivating coverage example; and
- Kani or equivalent bounded checks for command-block mutual exclusions and
  pipeline-topology invariants where those checks remain tractable.

Fuzzing should treat Jinja expression values containing quotes, whitespace,
newlines, shell operators, Unicode, and shell-substitution text as ordinary
scalar payloads and assert that the resulting process structure remains
unchanged.

## 22. Alternatives considered

### 22.1 Require `program` and `args`

```yaml
command:
  program: cargo
  args:
    - llvm-cov
    - --lcov
    - --output-path
    - lcov.info
```

This form maps directly to a process API and remains a useful conceptual IR. It
is rejected as the primary user syntax because it turns compact commands into
vertical YAML and makes copied command lines laborious to adapt. An optional
explicit-argv escape hatch may be proposed later, but ordinary use should not
require it.

### 22.2 Render Jinja, then call `shlex`

A POSIX lexer can split simple commands without executing shell expansions. It
still gives rendered values the power to add words or alter quote structure. It
also imposes POSIX backslash rules on Windows manifests and loses the source
provenance needed to distinguish a quoted literal pipe from an accidental
pipeline. This approach is rejected.

### 22.3 Use Tree-sitter shell and PowerShell grammars

Tree-sitter can validate and classify source syntax, but a parse tree is not an
argv. Producing the final process arguments would require implementing each
shell's expansion and native-command translation semantics, including version
and platform differences. This would turn Netsuke into a partial shell
interpreter and still leave difficult interpolation contexts. This approach is
rejected for direct mode.

### 22.4 Insert rendered values into a shell AST

Replacing Jinja expressions with opaque nodes before parsing avoids syntax
injection during parsing, but inserting values afterwards still requires
context-specific reconstruction. A value in a command word, assignment,
arithmetic expression, here-document delimiter, or PowerShell expression has
different quoting requirements. Re-serializing the tree without changing
meaning is itself shell-specific. This approach is rejected as the direct-mode
execution model, though it may inform future shell-mode linting.

### 22.5 Automatically shell-escape every interpolation

Automatic quoting works only for interpolations that denote one shell word. It
cannot safely infer whether a value is intended as multiple arguments, an
operator, an assignment fragment, a path inside another word, or source in a
nested language. It also differs between POSIX shell and PowerShell. Explicit
filters remain useful in shell mode, but automatic escaping is rejected as the
foundation of structured commands.

### 22.6 Treat every string list item as a separate shell process

This would simplify heterogeneous sequencing but break existing lists that rely
on `cd`, shell variables, or exported environment changes carrying between
entries. Maximal contiguous legacy groups preserve compatibility while making
the boundary around structured units explicit.

### 22.7 Allow pipelines through rule references

Resolving pipeline adjacency after rule expansion would make a caller's stream
contract depend on the referenced rule's internal first and last steps. Small
rule refactors could then break distant callers. The initial design makes rule
references stream boundaries. A future RFC may introduce rules with explicit
stdin and stdout contracts.

## 23. Open questions and future extensions

The following questions do not block the initial syntax but should remain
visible during review. Named shell selection is no longer open:
[ADR-019](../adr-019-structured-command-shell-selection.md) and
[RFC 0011](0011-allow-listed-structured-command-shells.md) define the
`Boolean | ShellName` field and its trusted registry.

- Should direct Windows batch execution gain an explicit trusted opt-in, or
  should `.bat` and `.cmd` permanently require shell mode?
- Should a future explicit `args` field coexist with `invoke` as an escape hatch
  for generated manifests?
- What object syntax should add append mode, stderr teeing, or stdout/stderr
  merging without creating conflicting file handles?
- Which follow-up RFC should define ordered environment operations and merging
  across rule, target, action, and block levels?
- Should rule references eventually declare explicit stream contracts that
  permit safe pipeline composition across the reference boundary?

## 24. Recommendation

Adopt the structured command block and Netsuke-defined argv-template language.
The design keeps the common case compact, makes direct invocation genuinely
shell-free, gives typed list values a natural argv representation, and exposes
stream topology in the manifest rather than hiding it in command text.

The decisive rule is simple: parse syntax first, evaluate interpolation second,
and never parse an interpolated value again. That rule gives Netsuke a small,
portable execution language without asking manifest authors to build YAML
skyscrapers from argument lists.

## 25. References

<!-- markdownlint-disable MD013 -->

- [Netsuke design document](../netsuke-design.md)
- [ADR-019: Select allow-listed structured-command
  shells](../adr-019-structured-command-shell-selection.md)
- [RFC 0011: Allow-listed structured-command
  shells](0011-allow-listed-structured-command-shells.md)
- [Rust `std::process` module](https://doc.rust-lang.org/stable/std/process/)
- [Rust `std::process::Command`](https://doc.rust-lang.org/stable/std/process/struct.Command.html)
- [PowerShell parsing rules](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing)
- [Ninja manual](https://ninja-build.org/manual.html)
- [Rust `shlex` crate documentation](https://docs.rs/shlex/)

<!-- markdownlint-enable MD013 -->
