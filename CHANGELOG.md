# Changelog

## Unreleased

### Added

- Ship 33 further locale catalogues, so `--locale` now selects any of `ar`,
  `cs`, `cy`, `da`, `de`, `el`, `en-GB`, `en-US`, `es-419`, `es-ES`, `fa`,
  `fi`, `fr`, `gd`, `he`, `hi`, `hu`, `id`, `it`, `ja`, `ko`, `nb`, `nl`,
  `pl`, `pt-BR`, `pt-PT`, `ro`, `ru`, `sv`, `th`, `tr`, `uk`, `vi`, `zh-Hans`
  or `zh-Hant`, with `en-US` remaining the source and fallback locale
  ([#466](https://github.com/leynos/netsuke/issues/466))

### Changed

- Select catalogues by exact locale tag with deliberate per-language fallback
  rules, so `es-419` and `es-ES`, `pt-BR` and `pt-PT`, and `zh-Hans` and
  `zh-Hant` stay distinct instead of collapsing onto one catalogue per
  language ([#466](https://github.com/leynos/netsuke/issues/466))
- Make `src/localization/locales.rs` the authoritative locale registry, read by
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
  (`RUSTFLAGS=-Zpolonius=next cargo +nightly-2026-06-25 install netsuke`)
  ([#465](https://github.com/leynos/netsuke/issues/465))
- Remove the `rust-version = "1.89.0"` minimum-supported-Rust-version
  declaration from `Cargo.toml`; `rust-toolchain.toml` is now the single source
  of truth for the compiler contract
  ([#465](https://github.com/leynos/netsuke/issues/465))

## [0.1.0] - 2026-07-28

_Initial release._

[0.1.0]: https://github.com/leynos/netsuke/releases/tag/v0.1.0
