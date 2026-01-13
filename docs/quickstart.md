# Quick Start: Your First Netsuke Build

This guide walks you through creating and running your first Netsuke build in
under five minutes.

## Prerequisites

Before you begin, ensure you have:

- **Netsuke** installed (build from source with `cargo build --release` or
  install via `cargo install netsuke`)
- **Ninja** build tool in your PATH (install via your package manager, e.g.,
  `apt install ninja-build` or `brew install ninja`)

## Step 1: Create a Project Directory

Open a terminal and create a new directory for your project:

```sh
mkdir hello-netsuke
cd hello-netsuke
```

## Step 2: Create Your First Manifest

Create a file named `Netsukefile` with the following content:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

This manifest defines:

- A target called `hello.txt` that creates a file with a greeting
- A default target so running `netsuke` without arguments builds `hello.txt`

## Step 3: Run Netsuke

Run Netsuke to build your project:

```sh
netsuke
```

You should see output similar to:

```text
[1/1] echo 'Hello from Netsuke!' > hello.txt
```

Check the result:

```sh
cat hello.txt
```

Output:

```text
Hello from Netsuke!
```

Congratulations! You've just run your first Netsuke build.

## Step 4: Add Variables and Templates

Netsuke supports Jinja templating for dynamic manifests. Update your
`Netsukefile`:

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

Run `netsuke` again:

```sh
netsuke
```

Check the output:

```sh
cat greeting.txt
```

Output:

```text
Hello, World!
```

## Step 5: Use Globbing and Foreach

For more complex builds, Netsuke can process multiple files. Create some input
files:

```sh
echo "Content A" > input_a.txt
echo "Content B" > input_b.txt
```

Update your `Netsukefile` to process all `.txt` files:

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

Run `netsuke`:

```sh
netsuke
```

Check the outputs:

```sh
cat output_input_a.out
cat output_input_b.out
```

The input files have been transformed to uppercase.

## Next Steps

- Read the full [User Guide](users-guide.md) for comprehensive documentation
- Explore the `examples/` directory for real-world manifest examples:
  - `basic_c.yml` — C compilation with rules and variables
  - `website.yml` — Static site generation from Markdown
  - `photo_edit.yml` — Photo processing with glob patterns
- Run `netsuke --help` to see all available options
- Try `netsuke graph` to visualize your build dependency graph

## Troubleshooting

### "No `Netsukefile` found in the current directory"

Ensure you're in the correct directory and that a file named `Netsukefile`
exists. You can also specify a different manifest path with `-f`:

```sh
netsuke -f path/to/your/manifest.yml
```

### "ninja: command not found"

Install Ninja using your system's package manager:

- **Ubuntu/Debian:** `sudo apt install ninja-build`
- **macOS (Homebrew):** `brew install ninja`
- **Windows (Chocolatey):** `choco install ninja`
