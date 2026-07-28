# Changelog

## Unreleased

### Changed

- Build with the Polonius borrow checker (`-Zpolonius=next`) on the pinned
  `nightly-2026-06-25` toolchain; building from source now requires that
  nightly, which `rustup` installs automatically
  ([#465](https://github.com/leynos/netsuke/issues/465))
- Remove the `rust-version = "1.89.0"` minimum-supported-Rust-version
  declaration from `Cargo.toml`; `rust-toolchain.toml` is now the single
  source of truth for the compiler contract
  ([#465](https://github.com/leynos/netsuke/issues/465))

## [0.1.0] - 2026-07-28

_Initial release._

[0.1.0]: https://github.com/leynos/netsuke/releases/tag/v0.1.0
