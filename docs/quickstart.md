# Quick start: a Netsuke build

This guide walks through creating and running a Netsuke build in under five
minutes.

## Prerequisites

Before beginning, ensure the following are available:

- **Netsuke** installed (build from source with `cargo build --release` or
  install via `cargo install netsuke`)
- **Ninja** build tool in your PATH (install via your package manager, e.g.,
  `apt install ninja-build` or `brew install ninja`)

## Step 1: Create a project directory

In a terminal, create a new directory for the project:

```sh
mkdir hello-netsuke
cd hello-netsuke
```

## Step 2: Create the first manifest

A file named `Netsukefile` should be created with the following content:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

This manifest defines:

- A target called `hello.txt` that creates a file with a greeting.
- A default target, so running `netsuke` without arguments builds `hello.txt`.

## Step 3: Run netsuke

To build the project, run Netsuke:

```sh
netsuke
```

The output should be similar to:

```text
[1/1] echo 'Hello from Netsuke!' > hello.txt
```

The result can be verified:

```sh
cat hello.txt
```

Output:

```text
Hello from Netsuke!
```

This completes a first Netsuke build.

## Step 4: Add variables and templates

Netsuke supports Jinja templating for dynamic manifests. The `Netsukefile`
can be updated as follows:

```yaml
netsuke_version: "1.0.0"

vars:
  greeting: "Hello"
  name: "World"

targets:
  - name: greeting.txt
    command: "echo '{{ greeting }}, {{ name }}!' > greeting.txt"

defaults:
  - greeting.txt
```

Running `netsuke` again:

```sh
netsuke
```

The output can be checked:

```sh
cat greeting.txt
```

Output:

```text
Hello, World!
```

## Step 5: Use globbing and foreach

For more complex builds, Netsuke can process multiple files. Some input files
can be created:

```sh
echo "Content A" > input_a.txt
echo "Content B" > input_b.txt
```

The `Netsukefile` can be updated to process all `.txt` files:

```yaml
netsuke_version: "1.0.0"

targets:
  - foreach: glob('input_*.txt')
    name: "output_{{ item | basename | with_suffix('.out') }}"
    command: "cat {{ item }} | tr 'a-z' 'A-Z' > {{ outs }}"
    sources: "{{ item }}"

defaults:
  - output_input_a.out
  - output_input_b.out
```

Running `netsuke`:

```sh
netsuke
```

The outputs can be checked:

```sh
cat output_input_a.out
cat output_input_b.out
```

The input files have been transformed to uppercase.

## Next steps

- Read the full [User Guide](users-guide.md) for comprehensive documentation
- Explore the `examples/` directory for real-world manifest examples:
  - `basic_c.yml` — C compilation with rules and variables
  - `website.yml` — Static site generation from Markdown
  - `photo_edit.yml` — Photo processing with glob patterns
- Run `netsuke --help` to see all available options
- Try `netsuke graph` to visualize your build dependency graph

## Troubleshooting

### "No `Netsukefile` found in the current directory"

Ensure the current directory is correct and that a file named `Netsukefile`
exists. A different manifest path can be specified with `-f`:

```sh
netsuke -f path/to/your/manifest.yml
```

### "ninja: command not found"

Install Ninja using the system's package manager:

- **Ubuntu/Debian:** `sudo apt install ninja-build`
- **macOS (Homebrew):** `brew install ninja`
- **Windows (Chocolatey):** `choco install ninja`
