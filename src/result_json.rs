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

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ResultDocument<'a> {
    schema_version: u32,
    generator: GeneratorInfo,
    result: CommandResult<'a>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct CommandResult<'a> {
    command: &'a str,
    content: Option<&'a str>,
}
