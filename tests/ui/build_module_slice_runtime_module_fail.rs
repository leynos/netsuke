//! Compile-fail fixture for a runtime import outside the `build.rs` slice.

#[path = "../../src/locale/catalogues.rs"]
pub mod locale_catalogues;
#[path = "../../src/cli/localization/mod.rs"]
mod cli_localization;
#[path = "../../src/localization/mod.rs"]
pub mod localization;
#[path = "../../src/host/pattern.rs"]
mod host_pattern;

#[path = "../../src/cli"]
mod cli {
    //! The production CLI modules compiled by `build.rs`.

    pub mod config;
    mod validation;
    mod help;
    mod command;

    pub use command::Cli;
    pub use config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
}

use cli::discovery;

fn main() {}
