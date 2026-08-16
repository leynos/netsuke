//! Help topic data types for the `netsuke help` subcommand.
//!
//! Kept out of `parser.rs` so that module stays within the repository's
//! 400-line budget. The `Cli` command re-exports these types for clap.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

/// Arguments accepted by the `help` command.
///
/// The optional topic selects the help artefact to render. With no topic the
/// command prints the top-level long help, matching `--help`.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct HelpArgs {
    /// Help topic to print; omitting it prints the top-level help.
    #[command(subcommand)]
    pub topic: Option<HelpTopic>,
}

/// Help topics accepted by the `netsuke help` command.
#[derive(Debug, Subcommand, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HelpTopic {
    /// Print the target and action catalogue for the selected manifest.
    Targets,

    /// Print the help for the `build` command.
    Build,
    /// Print the help for the `clean` command.
    Clean,
    /// Print the help for the `graph` command.
    Graph,
    /// Print the help for the `generate` command.
    Generate,
}
