//! Compile-fail fixture for a runtime import outside the `build.rs` slice.

#[path = "../../src/locale_catalogues.rs"]
pub mod locale_catalogues;
#[path = "../../src/cli_localization.rs"]
mod cli_localization;
#[path = "../../src/localization/mod.rs"]
pub mod localization;
#[path = "../../src/host_pattern.rs"]
mod host_pattern;

#[path = "../../src/cli"]
mod cli {
    //! The production CLI modules compiled by `build.rs`.

    #[path = "config.rs"]
    pub mod config;
    #[path = "validation.rs"]
    mod validation;
    #[path = "help.rs"]
    mod help;
    #[path = "command.rs"]
    mod command;

    pub use command::Cli;
    pub use config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
}

use cli::discovery;

fn main() {}
