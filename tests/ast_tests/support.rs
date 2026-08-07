//! Shared manifest-parsing helper for the AST test modules.
//!
//! Every sibling module in `tests/ast_tests/` parses YAML through the same
//! library entry point, so the wrapper lives here rather than being repeated.

use anyhow::Result;
use netsuke::{ast::NetsukeManifest, manifest};

/// Convenience wrapper around the library manifest parser for tests.
pub(super) fn parse_manifest(yaml: &str) -> Result<NetsukeManifest> {
    manifest::from_str(yaml)
}
