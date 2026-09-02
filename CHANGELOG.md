# Changelog

## Unreleased

### Added

- Add `netsuke check`, a semantic linter for Netsukefiles. It analyses the
  compiler's own artefacts — the authored source with its spans, the expanded
  manifest, and the lowered build graph — rather than the YAML text, so a rule
  can tell an order-only directory dependency from a content dependency and
  recognize that a literal path in a recipe is another target's output.
  Twenty-four rules ship across nine categories, covering stale escaping
  workarounds, non-portable shell constructs, dependency shapes that defeat
  change detection, unused declarations, and the lint directives themselves.
  Findings are the command's result rather than its failure mode: `--fail-on`
  sets the severity at which they fail the command, `--rule NAME=SEVERITY`
  configures one rule or a whole category, and `--explain` prints the rule
  reference. Suppression is a comment that names the rules it silences and
  states a reason; there is no blanket disable
  ([#592](https://github.com/leynos/netsuke/issues/592))

## [0.1.0-beta2] - 2026-08-19

<!-- markdownlint-disable-next-line MD024 -->
### Added

- Add [docs/v0-1-0-migration-guide.md](docs/v0-1-0-migration-guide.md)
  signposting the child-environment API additions, and recording that every
  Rust API surface outside the Netsukefile format and the graph export is
  private in intent and unstable.
- Export `runner::CommandEnv` together with the `runner::NinjaBuildRequest` and
  `runner::NinjaToolRequest` bundles and the `runner::run_ninja_with` and
  `runner::run_ninja_tool_with` entry points, so an embedder can set the
  environment of the spawned Ninja process — `PATH` included — without mutating
  its own. Overrides are additive and `CommandEnv::inherit()` reproduces the
  existing behaviour, so `run_ninja` and `run_ninja_tool` keep their signatures
  and no embedder needs to change
  ([#490](https://github.com/leynos/netsuke/issues/490))
- Accept a non-empty ordered list of commands for a rule or target `command`
  recipe, executed as a single fail-fast `&&` shell chain, so the build stops
  at the first non-zero exit; an empty command list is rejected at parse time,
  and entries with multiple background jobs or unsupported `exec` structures
  are rejected during Ninja generation as `MultipleBackgroundJobs` or
  `UnsupportedCommandListExec`
  ([#550](https://github.com/leynos/netsuke/issues/550))

### Changed

- Require UTF-8 build-file and working-directory paths for Ninja invocation,
  reporting a clear diagnostic when either path is not valid UTF-8
  ([#525](https://github.com/leynos/netsuke/issues/525))
- Move the `StringOrList` conversion helpers out of
  `src/ir/from_manifest_support.rs` and onto the type itself as `map_each`,
  `to_string_vec` and `as_single`, so the behaviour lives with the data it
  converts; `to_paths` stays at the manifest-to-IR boundary, since only
  lowering treats these manifest strings as filesystem paths
  ([#73](https://github.com/leynos/netsuke/issues/73))
- Reject manifest `vars` keys named `env` or `glob` at parse time, since
  MiniJinja shares one namespace for template functions and global variables
  and such a key would otherwise silently shadow the built-in helper; manifests
  that previously used either name as a variable now fail to parse
  ([#79](https://github.com/leynos/netsuke/issues/79))
- Open the capability used for glob metadata checks at the pattern's longest
  literal directory prefix rather than at the filesystem root or the working
  directory, so glob expansion holds only the authority its pattern can reach;
  a pattern whose literal prefix is missing or names something other than a
  directory now expands to no matches, and a match reached through a symbolic
  link — the match itself or an intermediate directory — that resolves outside
  that prefix or dangles is skipped rather than failing the expansion, though a
  cyclic symbolic link still fails the expansion; opening a symbolic-link
  prefix now fails, and glob tracing redacts caller-controlled path fields as
  `<redacted>` ([#173](https://github.com/leynos/netsuke/issues/173))

### Removed

- Remove `runner::BuildTargets::is_empty`, which had no callers anywhere in
  the workspace. Library consumers who need the same answer can call
  `BuildTargets::as_slice().is_empty()`
  ([#75](https://github.com/leynos/netsuke/issues/75))

### Fixed

- Expand parent-relative glob patterns such as `glob('../shared/*.h')`; their
  matches previously reached the working-directory capability as `../…` and
  were rejected as sandbox escapes
  ([#173](https://github.com/leynos/netsuke/issues/173))

## [0.1.0-beta1] - 2026-08-05

<!-- markdownlint-disable-next-line MD024 -->
### Added

- Ship 33 further locale catalogues, so `--locale` now selects any of `ar`,
  `cs`, `cy`, `da`, `de`, `el`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fa`, `fi`,
  `fr`, `gd`, `he`, `hi`, `hu`, `id`, `it`, `ja`, `ko`, `nb`, `nl`, `pl`,
  `pt-BR`, `pt-PT`, `ro`, `ru`, `sv`, `th`, `tr`, `uk`, `vi`, `zh-Hans` or
  `zh-Hant`, with `en-US` remaining the source and fallback locale
  ([#466](https://github.com/leynos/netsuke/issues/466))

<!-- markdownlint-disable-next-line MD024 -->
### Changed

- Select catalogues by exact locale tag with deliberate per-language fallback
  rules, so `es-419` and `es-ES`, `pt-BR` and `pt-PT`, and `zh-Hans` and
  `zh-Hant` stay distinct instead of collapsing onto one catalogue per language
  ([#466](https://github.com/leynos/netsuke/issues/466))
- Make `src/locale_catalogues.rs` the authoritative locale registry, read by
  the embedded catalogues, the build-time audit, the `rerun-if-changed`
  directives, packaging, and the tests; the build now fails if `Cargo.toml`'s
  `ortho_config` locale metadata drifts from it
  ([#466](https://github.com/leynos/netsuke/issues/466))
- Extend the build-time localization audit to every declared locale and to
  interpolation variables, so a message that drops or invents a `{ $variable }`
  fails the build ([#466](https://github.com/leynos/netsuke/issues/466))
- Route graph-view node registration through a borrow-returning
  `NodePathRegistry` accessor that looks paths up once on hits and clones a
  path only on insertion ([#465](https://github.com/leynos/netsuke/issues/465))
- Build with the Polonius borrow checker (`-Zpolonius=next`) on the pinned
  `nightly-2026-06-25` toolchain; checkout builds pick this up automatically via
  `rustup`, while registry installs must pass the toolchain and flag explicitly
  (`RUSTFLAGS=-Zpolonius=next cargo +nightly-2026-06-25 install netsuke-build`)
  ([#465](https://github.com/leynos/netsuke/issues/465))
- Remove the `rust-version = "1.89.0"` minimum-supported-Rust-version
  declaration from `Cargo.toml`; `rust-toolchain.toml` is now the single source
  of truth for the compiler contract
  ([#465](https://github.com/leynos/netsuke/issues/465))

[0.1.0-beta2]: https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta2
[0.1.0-beta1]: https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta1
