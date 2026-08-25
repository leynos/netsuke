//! Serialize successful command results into the shared JSON envelope.

use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};
use serde::Serialize;

/// Render a successful command result as Netsuke's versioned JSON document.
///
/// `content` carries a generated text artefact when the command would normally
/// write that artefact to standard output.
///
/// # Errors
///
/// Returns an error if the document cannot be serialized to JSON.
pub(crate) fn render_result_json(
    command: &str,
    content: Option<&str>,
) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&ResultDocument {
        schema_version: SCHEMA_VERSION,
        generator: GeneratorInfo::current(),
        result: CommandResult { command, content },
    })
}

/// The versioned JSON document wrapping a successful command result.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct ResultDocument<'a> {
    /// Schema version of the document envelope.
    schema_version: u32,
    /// Generator identity stamped from the build environment.
    generator: GeneratorInfo,
    /// The command text and optional generated content.
    result: CommandResult<'a>,
}

/// The command text and optional generated content of a successful run.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct CommandResult<'a> {
    /// The command line that produced the result.
    command: &'a str,
    /// Generated text artefact, when the command writes to standard output.
    content: Option<&'a str>,
}
