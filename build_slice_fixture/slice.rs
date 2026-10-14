// The fixture owns these path imports to compile the actual production sources.
// Keep the CLI declarations aligned with build.rs through the source contract.
#[path = "../src/cli"]
pub mod cli {
    //! Expose the production CLI schema without runtime configuration discovery.

    pub mod config;
    mod validation;
    mod help;
    mod command;

    pub use command::Cli;
    pub use config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
}

#[path = "../src/cli/localization/mod.rs"]
mod cli_localization;
#[path = "../src/host/pattern.rs"]
mod host_pattern;
#[path = "../src/locale/catalogues.rs"]
pub mod locale_catalogues;
#[path = "../src/localization/mod.rs"]
pub mod localization;
