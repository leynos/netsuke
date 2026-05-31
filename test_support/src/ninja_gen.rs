//! Shared fixtures and case types for Ninja generation tests.

use camino::Utf8PathBuf;
use netsuke::ir::{Action, BuildEdge};
use tempfile::TempDir;

use crate::ninja;

/// Define how an integration test should assert Ninja's behaviour.
#[derive(Debug)]
pub enum AssertionType {
    /// Assert that a generated file contains the expected trimmed content.
    FileContent(String),
    /// Assert that a generated file exists after Ninja runs.
    FileExists,
    /// Assert that the Ninja invocation exits successfully.
    StatusSuccess,
}

/// Full input and assertion data for one Ninja-backed integration scenario.
pub struct NinjaIntegrationCase {
    /// Action registered in the generated build graph.
    pub action: Action,
    /// Build edge registered in the generated build graph.
    pub edge: BuildEdge,
    /// Target path used by the test assertion.
    pub target_name: Utf8PathBuf,
    /// Arguments passed to the `ninja` binary.
    pub ninja_args: Vec<&'static str>,
    /// Assertion applied after the Ninja invocation.
    pub assertion: AssertionType,
}

/// Provide a temporary directory when Ninja is available, skipping otherwise.
pub fn ninja_integration_setup() -> Option<TempDir> {
    ninja::ninja_integration_workspace().ok()
}
