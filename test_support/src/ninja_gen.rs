//! Shared fixtures and case types for Ninja generation tests.
//!
//! This module keeps Ninja-backed integration test data out of individual test
//! files. It exports:
//!
//! - [`AssertionType`], which describes how a scenario should validate Ninja's
//!   result.
//! - [`NinjaIntegrationCase`], which bundles the action, edge, target,
//!   command-line arguments, and expected assertion for one scenario.
//! - [`ninja_integration_setup`], which creates a temporary Ninja workspace
//!   only when [`crate::ninja::ninja_integration_workspace`] can confirm that
//!   the `ninja` binary is available.
//!
//! Test modules use these items with `rstest` `#[case::...]` parameterisation:
//! each case constructs a [`NinjaIntegrationCase`], the
//! `ninja_integration_setup` fixture provides an optional workspace, and the
//! test writes a generated `build.ninja`, runs `ninja`, then checks the result
//! through the case's [`AssertionType`].
//!
//! Typical usage:
//!
//! ```rust,ignore
//! #[rstest]
//! #[case::phony_action(NinjaIntegrationCase {
//!     action,
//!     edge,
//!     target_name,
//!     ninja_args: vec!["target"],
//!     assertion: AssertionType::FileExists,
//! })]
//! fn ninja_integration_tests(
//!     ninja_integration_setup: Option<TempDir>,
//!     #[case] case: NinjaIntegrationCase,
//! ) -> anyhow::Result<()> {
//!     let Some(workspace) = ninja_integration_setup else {
//!         return Ok(());
//!     };
//!
//!     // Generate build.ninja inside workspace, run ninja, then match on
//!     // case.assertion to validate the generated output.
//!     Ok(())
//! }
//! ```

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
