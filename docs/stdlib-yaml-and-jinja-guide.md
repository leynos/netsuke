# Template standard library for YAML and Jinja

Netsuke extends MiniJinja with helpers for paths, collections, host files,
time, commands, executable discovery, environment variables, and globbing.
These helpers run while Netsuke expands a `Netsukefile`, before Ninja runs the
generated build graph.

## Read YAML in a `Netsukefile`

A `Netsukefile` is a YAML mapping. Write `key: value` for a field, indent
nested content with spaces rather than tabs, and introduce each list item with
`-`. Netsuke rejects duplicate keys, unknown fields, and missing required
fields. The top-level `netsuke_version` and `targets` fields are required.

Quote version numbers and strings containing Jinja. In particular, quote a
value beginning with `{{` because an unquoted `{` has meaning in YAML. Use `|`
for a multi-line string that preserves line breaks, or `>-` to fold lines into
one string and remove the final newline. This example shows mappings, a list,
quoted template text, and a folded command:

<!-- tested-example: stdlib-yaml-syntax-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  greeting: Hello
  recipients:
    - Netsuke
    - Ninja

targets:
  - name: greeting.txt
    command: >-
      printf '%s\n'
      "{{ greeting }}, {{ recipients | first }}!"
      > {{ outs }}

defaults:
  - greeting.txt
```

See the [yaml.info YAML tutorial](https://www.yaml.info/learn/index.html) for a
deeper introduction to mappings, sequences, scalars, quoting, and block style.

## Read Jinja inside YAML

Netsuke uses MiniJinja to render string fields. Put an expression inside
`{{ ... }}`, read values declared under `vars` by name, pass a value through a
filter with `|`, call a function with parentheses, and apply a test with `is`.
Undefined values are errors. Apart from `{{ ins }}` and `{{ outs }}`, rendered
values are not automatically quoted for the host shell.

The manifest control keys are deliberately different: `foreach` and `when` take
direct Jinja expressions without `{{ ... }}`. Structural Jinja statements such
as `{% for ... %}` cannot reshape the YAML document; use those control keys or
Netsuke's declared macros instead.

<!-- tested-example: stdlib-jinja-syntax-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  artifact: report.tmp
  labels:
    - alpha
    - alpha
    - beta

targets:
  - name: "{{ artifact | with_suffix('.txt') }}"
    when: labels | length > 0
    command: "printf '%s\n' '{{ labels | uniq | join(',') }}' > {{ outs }}"

defaults:
  - report.txt
```

The [Jinja introduction and variable-substitution tutorial][jinja-tutorial]
provides a more detailed introduction. It describes Jinja2, so remember that
Netsuke exposes MiniJinja plus Netsuke-specific manifest control keys.

[jinja-tutorial]: https://ttl255.com/jinja2-tutorial-part-1-introduction-and-variable-substitution/

Use host-observing helpers only in trusted manifests. Netsuke bounds output
from network and command helpers, but it does not sandbox template evaluation.

## Read the signatures

Signatures below use `value | filter(arguments)` for filters,
`function(arguments)` for functions, and `value is test` for Jinja tests.
Arguments in square brackets are optional. “Pure” means the result depends only
on the supplied value and arguments. “Host-observing” helpers read the clock,
environment, filesystem, network, or subprocess state; `fetch` with a cache may
also write beneath Netsuke's cache directory.

## Transform paths

Path filters accept UTF-8 strings and return UTF-8 strings. Path parsing uses
the host platform's separator rules, so do not assume that Windows and Unix
split the same input identically.

- `path | basename` is pure and returns the final path component. For example,
  `{{ 'reports/daily.csv' | basename }}` produces `daily.csv`.
- `path | dirname` is pure and returns the parent, using `.` when the path has
  no explicit parent. For example, `{{ 'reports/daily.csv' | dirname }}`
  produces `reports`.
- `path | with_suffix(suffix[, count[, separator]])` is pure. `count` defaults
  to `1`, and `separator` defaults to `.`. For example,
  `{{ 'archive.tar.gz' | with_suffix('.zip', 2) }}` produces `archive.zip`.
- `path | relative_to(root)` is pure and requires `path` to be beneath `root`.
  For example, `{{ 'reports/daily.csv' | relative_to('reports') }}` produces
  `daily.csv`.
- `path | realpath` is host-observing. It canonicalizes an existing path,
  resolving links, and fails when the path cannot be resolved. The result may
  be relative or absolute: `.` produces the canonical absolute workspace path,
  and a relative link may resolve to an absolute target. Do not rely on the
  input's relativity being preserved. For example,
  `{{ 'input-link' | realpath }}`.
- `path | expanduser` is host-observing because it reads the home-directory
  environment. It expands `~` and `~/...`; named-user forms such as `~alice`
  are unsupported. For example, `{{ '~/cache' | expanduser }}`.

## Read and identify files

These filters inspect the host filesystem and fail when their input cannot be
read. Relative paths are resolved from the workspace in which Netsuke runs.

- `path | contents([encoding])` returns file text. The default is `utf-8`;
  `utf8` is accepted as an alias, and no other encoding is currently supported.
  Example: `{{ 'fixtures/message.txt' | contents }}`.
- `path | size` returns the file length in bytes. Example:
  `{{ 'fixtures/message.txt' | size }}`.
- `path | linecount` counts text lines using Rust's line semantics. Example:
  `{{ 'fixtures/message.txt' | linecount }}`.
- `path | hash([algorithm])` returns the full hexadecimal digest. The default
  is `sha256`; `sha512` is also available. Example:
  `{{ 'fixtures/message.txt' | hash('sha512') }}`.
- `path | digest([length[, algorithm]])` returns a prefix of the hash. Length
  defaults to `8` and the algorithm defaults to `sha256`. Example:
  `{{ 'fixtures/message.txt' | digest(12, 'sha512') }}`.

MD5 and SHA-1 are available only in builds compiled with Cargo feature
`legacy-digests`. Without that feature, `hash('md5')`, `hash('sha1')`, and their
`digest` equivalents fail with a feature-specific diagnostic. New manifests
should use SHA-256 or SHA-512.

## Transform collections

Collection filters are pure and preserve input order.

- `values | uniq` removes later duplicate values. Example:
  `{{ ['alpha', 'alpha', 'beta'] | uniq | join(',') }}` produces `alpha,beta`.
- `values | flatten` recursively flattens nested sequences and rejects scalar
  members. Example: `{{ [[1, 2], [3]] | flatten | join(',') }}` produces
  `1,2,3`.
- `values | group_by(attribute)` groups objects by an attribute while
  preserving first-seen key order. Missing attributes are errors. Example:
  `{{ ([{'kind': 'tool'}, {'kind': 'tool'}] | group_by('kind')).tool | length }}`
  produces `2`.

The following complete manifest exercises every path and collection filter. Its
filesystem inputs are created by the documentation test before Netsuke is run.

<!-- tested-example: stdlib-path-and-collection-manifest -->

```yaml
netsuke_version: "1.0.0"

vars:
  records:
    - kind: tool
    - kind: tool
    - kind: input

targets:
  - name: stdlib-paths.txt
    command: >-
      printf '%s\n'
      "{{ 'fixtures/report.tmp' | basename }}"
      "{{ 'fixtures/report.tmp' | dirname }}"
      "{{ 'fixtures/report.tmp' | with_suffix('.txt') }}"
      "{{ 'fixtures/report.tmp' | relative_to('fixtures') }}"
      "{{ 'fixtures/message-link' | realpath }}"
      "{{ '~/notes.txt' | expanduser }}"
      "{{ 'fixtures/message.txt' | contents }}"
      "{{ 'fixtures/message.txt' | size }}"
      "{{ 'fixtures/message.txt' | linecount }}"
      "{{ 'fixtures/message.txt' | hash }}"
      "{{ 'fixtures/message.txt' | digest }}"
      "{{ ['alpha', 'alpha', 'beta'] | uniq | join(',') }}"
      "{{ [[1, 2], [3]] | flatten | join(',') }}"
      "{{ (records | group_by('kind')).tool | length }}"
      > {{ outs }}

defaults:
  - stdlib-paths.txt
```

## Test file types

File tests are host-observing and have signature `path is test`. A missing path
or a non-string value yields `false`; an inspection error is reported.

- `path is file` identifies regular files: `{{ 'input.txt' is file }}`.
- `path is dir` identifies directories: `{{ 'fixtures' is dir }}`.
- `path is symlink` identifies symbolic links without following them:
  `{{ 'input-link' is symlink }}`.
- `path is pipe` identifies named pipes on Unix:
  `{{ 'events.pipe' is pipe }}`.
- `path is block_device` identifies Unix block devices:
  `{{ '/dev/loop0' is block_device }}`.
- `path is char_device` identifies Unix character devices:
  `{{ '/dev/null' is char_device }}`.
- `path is device` identifies either block or character devices on Unix:
  `{{ '/dev/null' is device }}`.

The four special-file tests (`pipe`, `block_device`, `char_device`, and
`device`) always return `false` on non-Unix platforms. The runnable example
therefore demonstrates the portable tests and the Unix behaviour separately.

<!-- tested-example: stdlib-file-tests-manifest -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: stdlib-file-tests.txt
    command: >-
      printf '%s\n'
      "{{ 'fixtures/message.txt' is file }}"
      "{{ 'fixtures' is dir }}"
      "{{ 'fixtures/message-link' is symlink }}"
      "{{ 'fixtures/events.pipe' is pipe }}"
      "{{ 'fixtures/message.txt' is block_device }}"
      "{{ '/dev/null' is char_device }}"
      "{{ '/dev/null' is device }}"
      > {{ outs }}

defaults:
  - stdlib-file-tests.txt
```

## Work with time

- `now([offset=...])` is host-observing and returns the current timestamp.
  With no offset it uses UTC; `offset` accepts `Z` or a signed offset such as
  `+02:00`. The value exposes `iso8601`, `unix_timestamp`, and `offset`.
  Example: `{{ now(offset='+02:00').iso8601 }}`.
- `timedelta(**components)` is pure. It accepts `weeks`, `days`, `hours`,
  `minutes`, `seconds`, `milliseconds`, `microseconds`, and `nanoseconds`.
  Every component defaults to zero and may be negative. The result exposes
  `iso8601`, whole `seconds`, and subsecond `nanoseconds`. Example:
  `{{ timedelta(days=1, minutes=30).iso8601 }}`.

These helpers represent values during template expansion; they do not delay or
schedule build work.

<!-- tested-example: stdlib-time-manifest -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: stdlib-time.txt
    command: >-
      printf '%s\n'
      "{{ now(offset='+02:00').iso8601 }}"
      "{{ timedelta(days=1, hours=2, minutes=30).iso8601 }}"
      > {{ outs }}

defaults:
  - stdlib-time.txt
```

## Run commands and inspect the host

These helpers observe the host and should appear only in trusted manifests.

- `value | shell(command[, options])` sends `value` to a host shell command's
  standard input. Capture mode is the default and returns the command's
  standard output. Pass `{'mode': 'tempfile'}` (also spelled `stream` or
  `streaming`) to write bounded output to a persisted temporary file; these
  modes return the temporary file's path, not its contents. Example:
  `{{ 'hello' | shell('uppercase') | trim }}`.
- `value | grep(pattern[, flags[, options]])` runs the host `grep` executable
  over `value`. `flags` is a sequence such as `['-i']`; options have the same
  modes and return values as `shell`, including a persisted temporary-file path
  in `tempfile`, `stream`, or `streaming` mode. Availability and flag spelling
  are platform-dependent. Example:
  `{{ 'alpha\nbeta\n' | grep('beta') | trim }}`.
- `which(name, **options)` and `name | which(**options)` search for an
  executable and fail when none is found. Boolean options `all`, `canonical`,
  and `fresh` default to `false`; `cwd_mode` defaults to `auto` and also accepts
  `always` or `never`. With `all=true`, the result is a list. Example:
  `{{ which('guide-tool', canonical=true) }}`.
- `command_available(name, **options)` accepts the same options as `which` but
  returns `true` or `false` for ordinary misses. Example:
  `{{ command_available('guide-tool', cwd_mode='never') }}`.
- `env(name)` returns one required Unicode environment variable. There is no
  default-value argument; a missing or non-Unicode value is an error. Example:
  `{{ env('NETSUKE_STDLIB_TOKEN') }}`.
- `glob(pattern)` returns matching workspace paths. It is host-observing;
  matches and separator syntax depend on workspace contents and platform.
  Example: `{{ glob('fixtures/*.txt') | join(',') }}`.

The executable example uses stub commands and a controlled `PATH`, environment
variable, and fixture directory, so it never depends on developer-installed
tools.

<!-- tested-example: stdlib-host-context-manifest -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: stdlib-host-context.txt
    command: >-
      printf '%s\n'
      "{{ 'hello' | shell('uppercase') | trim }}"
      "{{ 'alpha\nbeta\n' | grep('beta') | trim }}"
      "{{ which('guide-tool') }}"
      "{{ command_available('guide-tool', cwd_mode='never') }}"
      "{{ env('NETSUKE_STDLIB_TOKEN') }}"
      "{{ glob('fixtures/*.txt') | join(',') }}"
      > {{ outs }}

defaults:
  - stdlib-host-context.txt
```

## Fetch network content

`fetch(url[, cache=false])` is host-observing and retrieves a URL, returning
text for UTF-8 responses and bytes otherwise. HTTPS is the only allowed scheme
by default. `cache=true` enables the on-disk response cache. Network policy can
be narrowed or extended with Netsuke's `--fetch-*` options; see
[Configure network access](users-guide.md#configure-network-access).

The expression below is registry-tested for documentation drift but
deliberately not executed by the test suite: documentation tests must not make
network requests. Use a stable, policy-approved URL in a trusted manifest.

<!-- tested-example: stdlib-fetch-expression -->

```jinja
{{ fetch('https://example.com/toolchain.json', cache=true) }}
```

## Choose pure planning where practical

Pure helpers make generated build graphs reproducible. Host-observing helpers
are useful for discovery and conditional planning, but their results can vary
between machines or invocations. Prefer explicit manifest inputs when a value
is part of the build's reproducibility contract, and keep network or command
execution out of untrusted `Netsukefile` content.
