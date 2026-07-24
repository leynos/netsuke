# 🧵 Netsuke

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/netsuke)

*A friendly build-system compiler: YAML and Jinja in, Ninja out.*

Netsuke turns a readable `Netsukefile` into a validated, static Ninja build
graph. It keeps the dynamic work in a higher-level manifest and leaves fast,
incremental execution to [Ninja](https://ninja-build.org/).

______________________________________________________________________

## Why Netsuke?

- **Readable manifests**: Describe rules, targets, dependencies, and defaults
  in YAML instead of a tab-sensitive language.
- **Dynamic planning**: Use Jinja variables, macros, `foreach`, `when`, and
  globbing before Netsuke creates the build graph.
- **Static execution**: Inspect the generated Ninja file or render the graph
  before running any build command.
- **Useful diagnostics**: Get source-aware errors, localized output, progress
  reporting, and canonical `--json` machine-readable command output.
- **No blessed toolchain**: Use the same manifest model for Rust, C, Python,
  web projects, or anything else a command can build.

______________________________________________________________________

## Quick start

### Prerequisites

Netsuke currently requires:

- [Ninja](https://ninja-build.org/) on `PATH`;
- Rust 1.89 or later when installing from source.

### Installation

Until the v0.1.0 release is published, install the current source checkout with
Cargo:

<!-- tested-example: readme-source-install -->

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Your first build

Create a new directory and add a file named `Netsukefile`:

<!-- tested-example: readme-first-build-manifest -->

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Run Netsuke, then inspect the result:

<!-- tested-example: readme-first-build-commands -->

```sh
netsuke
cat hello.txt
```

The second command prints `Hello from Netsuke!`. See the
[quick-start guide](docs/quickstart.md) for variables, templates, and `foreach`.

______________________________________________________________________

## What works today

The core build-system compiler is implemented:

- YAML 1.2 manifest parsing with duplicate-key and schema validation;
- Jinja variables, macros, `foreach`, `when`, globbing, environment helpers,
  executable discovery, and opt-in network helpers;
- reusable rules, targets, actions, defaults, and explicit, implicit, and
  order-only dependencies;
- a deterministic intermediate build graph with duplicate-output, missing-rule,
  and cycle checks;
- Ninja generation and execution, plus `clean` and standalone manifest
  generation;
- reproducible dependency graphs as Graphviz DOT or self-contained,
  accessible HTML;
- layered configuration, localized output, accessibility preferences,
  progress reporting, stage timings, and versioned JSON results or diagnostics;
- unit, behavioural, integration, property, snapshot, and initial Kani
  verification coverage.

Release automation is configured to build packages for Linux, macOS, and
Windows, including platform help artefacts. v0.1.0 will be the first public
release of this work.

______________________________________________________________________

## v0.1.0 status

v0.1.0 is a useful preview for early adopters, not a declaration that Netsuke
is finished or that every interface is stable. The compiler pipeline and
ordinary local-build workflow are substantial; the command-line interface,
configuration vocabulary, and advanced recipe model are still evolving.

Pin the Netsuke version in automation and expect some command names, flags,
diagnostic schemas, and manifest details to change before 1.0.

Known limitations include:

- recipes are shell strings; structured executable arguments and recipe
  environment mappings are not implemented yet;
- literal shell dollar expressions still need Ninja-aware escaping in
  manifests;
- compiler-generated dependency imports such as GCC depfiles are planned but
  not yet part of the manifest model;
- `--json` emits exactly one versioned result or diagnostic document for each
  command, but the schema may still change before 1.0;
- accessibility, terminal rendering, configuration precedence, and
  cross-platform compiler invariants need broader verification.

A `Netsukefile` can execute commands and use impure template helpers. Treat it
with the same care as a `Makefile`: review untrusted manifests before running
them. Netsuke quotes supported path substitutions, but it is not a sandbox.

______________________________________________________________________

## The road ahead

Work after the first release is organized around three priorities:

1. **Stabilize the command-line contract**: harden the canonical command and
   flag names, non-interactive safeguards, stable exit codes, bounded output,
   and versioned `--json` documents.
2. **Make recipes safer and clearer**: add structured executable arguments,
   environment mappings, compiler dependency imports, backend dollar escaping,
   and better conditional-action feedback.
3. **Strengthen confidence**: expand Kani and property-test coverage, verify
   accessibility with assistive technology, and add regression coverage for
   configuration precedence and terminal rendering.

Longer-term work explores machine-readable context, profiles, run history,
artefact delivery, and local-first feedback for human and agent workflows. The
[roadmap](docs/roadmap.md) tracks the detailed sequence and current progress.

______________________________________________________________________

## Learn more

- [Quick-start guide](docs/quickstart.md) — build something in five minutes.
- [Users' guide](docs/users-guide.md) — manifest and command reference.
- [Design document](docs/netsuke-design.md) — architecture and design
  rationale.
- [Developers' guide](docs/developers-guide.md) — development workflow and
  quality gates.
- [Roadmap](docs/roadmap.md) — completed foundations and planned work.

______________________________________________________________________

## Licence

ISC — see [LICENSE](LICENSE) for details.

______________________________________________________________________

## Contributing

Contributions are welcome. Start with the
[developers' guide](docs/developers-guide.md); automated contributors should
also follow [AGENTS.md](AGENTS.md).
