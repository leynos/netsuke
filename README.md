# 🧵 Netsuke

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/netsuke)

*A friendly build-system compiler: YAML and Jinja in, Ninja out.*

Netsuke turns a readable `Netsukefile` into a validated, static Ninja build
graph. It keeps the dynamic work in a higher-level manifest and leaves fast,
incremental execution to [Ninja](https://ninja-build.org/).

Website: <https://df12.studio/netsuke>

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
- when installing from source, the dated Rust nightly toolchain pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml) (`rustup` installs it
  automatically in a checkout). Netsuke builds with the Polonius borrow
  checker, which nightly enables by default and which stays nightly-only until
  it stabilizes; see
  [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md).

### Installation

The latest published prerelease is Netsuke v0.1.0-beta3 (v0.1.0-beta2 preceded
it), available from crates.io. Where
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) is available,
prefer it: it fetches a prebuilt release binary and avoids the toolchain
requirement below.

<!-- tested-example: readme-binstall-install -->

```sh
cargo binstall netsuke-build
```

Building from the registry instead runs outside a repository checkout, so the
pinned toolchain is not picked up automatically; select it explicitly:

<!-- tested-example: readme-crates-io-install -->

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Pre-built installers are available from the
[v0.1.0-beta3 GitHub release](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3):

| Platform | Architectures                        | Packages                         |
| -------- | ------------------------------------ | -------------------------------- |
| Linux    | x86-64 (`amd64`) and Arm64 (`arm64`) | Debian (`.deb`) and RPM (`.rpm`) |
| macOS    | Intel x86-64 and Apple silicon Arm64 | Installer package (`.pkg`)       |
| Windows  | x64 and Arm64                        | Windows Installer (`.msi`)       |

The Linux packages install the `netsuke` manual page and declare `ninja-build`
as a dependency. Ninja must be installed separately when using the macOS or
Windows installer. The Windows MSI installs to `C:\Program Files\netsuke` and
does not update `PATH`. SHA-256 checksum files accompany standalone binaries
and staged help and licence files. Installer packages do not have checksum
sidecars in v0.1.0-beta3. See the
[user's guide](docs/users-guide.md#install-netsuke) for platform-specific
commands and Windows setup.

To install the current source checkout with Cargo:

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
[quick-start guide](docs/quickstart.md) for variables, templates, and
`foreach`, then use the
[template standard-library guide](docs/stdlib-yaml-and-jinja-guide.md) for
every path, collection, filesystem, time, command, environment, glob, and
network helper.

______________________________________________________________________

## What works today

Netsuke v0.1.0-beta3's core build-system compiler provides:

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

The beta3 release also supports dependency-only action and target aggregates:
nodes with a non-empty `deps` list may omit a recipe.

______________________________________________________________________

## Release and development status

The v0.1.0-beta3 release is a useful preview for early adopters, not a
declaration that Netsuke is finished or that every interface is stable. The
compiler pipeline and ordinary local-build workflow are substantial; the
command-line interface, configuration vocabulary, and advanced recipe model
remain pre-stable.

Pin the Netsuke version in automation and expect some command names, flags,
diagnostic schemas, and manifest details to change before 1.0.

The following limitations apply to beta3.

Known limitations include:

- recipes are shell strings; structured executable arguments and recipe
  environment mappings are not implemented yet;
- compiler-generated dependency imports such as GCC depfiles are planned but
  not yet part of the manifest model;
- `--json` emits exactly one versioned result or diagnostic document for each
  command, but the schema may still change before 1.0;
- `script` recipes invoke `/bin/sh -e`; there is no portable PowerShell
  abstraction;
- beta3 does not enforce PowerShell as the Windows recipe interpreter, and
  native Windows recipe execution is not yet release-validated;
- colour rendering is not implemented;
- accessibility and cross-platform compiler invariants need broader
  verification.

The beta3 release fixes beta2's shell-dollar limitation with Ninja-aware
escaping, so ordinary shell expressions can be written normally. Beta2
manifests that use literal shell dollar expressions require migration; see the
[users' guide safety boundary](docs/users-guide.md#review-the-safety-boundary).

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
   environment mappings, compiler dependency imports, and better
   conditional-action feedback.
3. **Strengthen confidence**: expand Kani and property-test coverage, verify
   accessibility with assistive technology and add regression coverage for
   terminal rendering.

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
