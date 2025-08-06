# 🧵 Netsuke

A modern, declarative build system compiler. YAML + Jinja in, Ninja out.
Nothing more. Nothing less.

## What is Netsuke?

**Netsuke** is a friendly build system that compiles structured manifests into
a Ninja build graph. It’s not a shell-script runner, a meta-task framework, or
a domain-specific CI layer. It’s `make`, if `make` hadn’t been invented in 1977.

### Key properties

- **Declarative**: Targets, rules, and dependencies described explicitly.
- **Dynamic when needed**: Jinja templating for loops, macros, conditionals,
  file globbing.
- **Static where required**: Always compiles to a reproducible, fully static
  dependency graph.
- **Unopinionated**: No magic for C, Rust, Python, JavaScript, or any other
  blessed language.
- **Safe**: All variable interpolation is securely shell-escaped by default.
- **Fast**: Builds executed by [Ninja](https://ninja-build.org/), the fastest
  graph executor we know of.

## Quick Example

```yaml
netsuke_version: "1.0"

vars:
  cc: clang
  cflags: -Wall -Werror

rules:
  - name: compile
    command: "{{ cc }} {{ cflags }} -c {{ ins }} -o {{ outs }}"

  - name: link
    command: "{{ cc }} {{ cflags }} {{ ins }} -o {{ outs }}"

targets:
  - foreach: glob('src/*.c')
    name: "build/{{ item | basename | with_suffix('.o') }}"
    rule: compile
    sources: "{{ item }}"

  - name: app
    rule: link
    sources: "{{ glob('src/*.c') | map('basename') | map('with_suffix', '.o') }}"
```

Yes, it’s just YAML. Yes, that’s a Jinja `foreach`. No, you don’t need to
define `.PHONY` or remember what `$@` means. This is 2025. You deserve better.

## Key Concepts

### 🔨 Rules

Rules are reusable command templates. Each one has exactly one of:

- `command:` — a single shell string
- `script:` — a multi-line block
- (or) can be declared inline on a target

```yaml
rules:
  - name: rasterise
    script: |
      inkscape --export-png={{ ins }} {{ outs }}
```

### 🎯 Targets

Targets are things you want to build.

```yaml
- name: build/logo.png
  rule: rasterise
  sources: assets/logo.svg
```

Targets can also define:

- `deps`: explicit dependencies
- `order_only_deps`: e.g. `mkdir -p build`
- `vars`: per-target variables

You may also use `command:` or `script:` instead of referencing a `rule`.

## 🧪 Phony Targets and Actions

Phony targets behave like Make’s `.PHONY`:

```yaml
- name: clean
  phony: true
  always: true
  command: rm -rf build
```

For cleaner structure, you may also define phony targets under an `actions:`
block:

```yaml
actions:
  - name: test
    command: pytest
```

All `actions` are treated as `{ phony: true, always: false }` by default.

## 🧠 Templating

Netsuke uses [MiniJinja](https://docs.rs/minijinja) to render your manifest
before parsing.

You can:

- Glob files: `{{ glob('src/**/*.c') }}`
- Read environment vars: `{{ env('CC') }}`
- Use filters: `{{ path | basename | with_suffix('.o') }}`
- Define reusable macros:

  ```yaml
  macros:
    - signature: "shout(msg)"
      body: |
        echo "{{ msg | upper }}"
  ```

Templating happens **before** parsing, so any valid output must be valid YAML.

## 🔐 Safety

Shell commands are automatically escaped. Interpolation into `command:` or
`script:` will never yield a command injection vulnerability unless you
explicitly ask for `| raw`.

```yaml
command: "echo {{ dangerous_value }}"      # Safe
command: "echo {{ dangerous_value | raw }}" # Unsafe (your problem now)
```

## 🔧 CLI

```shell
netsuke [build] [target1 target2 ...]
netsuke clean
netsuke graph
 netsuke manifest FILE
```

- `netsuke` alone builds the `defaults:` targets from your manifest
- `netsuke graph` emits a Graphviz `.dot` of the build DAG
- `netsuke clean` runs `ninja -t clean`
- `netsuke manifest FILE` writes the Ninja manifest to `FILE` without invoking
  Ninja

You can also pass:

- `--file` to use an alternate manifest
- `--directory` to run in a different working dir
- `-j N` to control parallelism (passed through to Ninja)
- `-v`, `--verbose` to enable verbose logging

## 🚧 Status

Netsuke is **under active development**. It’s not finished, but it’s buildable,
usable, and increasingly delightful.

Coming soon:

- `graph --html` for interactive DAGs
- Extensible plugin system for filters/functions
- Toolchain presets (`cargo`, `node`, etc.)

## Why “Netsuke”?

A **netsuke** is a small carved object used to fasten things securely to a
belt. It’s not the sword. It’s not the pouch. It’s the thing that connects them.

That’s what this is: a tidy connector between your intent and the tool that
gets it done.

## License

[ISC](https://opensource.org/licenses/ISC) — because you don't need a legal
thesis to use a build tool.
