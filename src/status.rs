//! Pipeline status reporting for accessible and standard output modes.
//!
//! This module provides a [`StatusReporter`] trait and concrete
//! implementations that emit progress feedback during Netsuke's build
//! pipeline. [`AccessibleReporter`] writes static, labelled status lines
//! to stderr suitable for screen readers and dumb terminals.
//! [`SilentReporter`] emits nothing, serving as the default until future
//! animated progress indicators are added.

use crate::localization::{self, keys};
use std::io::{self, Write};

/// Report pipeline progress to the user.
///
/// Implementations decide how (or whether) to present stage transitions
/// and completion to the user. The trait is object-safe so callers can
/// dispatch dynamically based on the resolved output mode.
pub trait StatusReporter {
    /// Emit a status line for the given pipeline stage.
    fn report_stage(&self, current: u32, total: u32, description: &str);

    /// Emit a completion message after a successful pipeline run.
    fn report_complete(&self);
}

/// Accessible reporter: writes static, labelled lines to stderr.
///
/// Each line follows the pattern `Stage N/M: Description`, using
/// localised messages from the Fluent resource bundle.
pub struct AccessibleReporter;

impl StatusReporter for AccessibleReporter {
    fn report_stage(&self, current: u32, total: u32, description: &str) {
        let message = localization::message(keys::STATUS_STAGE_LABEL)
            .with_arg("current", current.to_string())
            .with_arg("total", total.to_string())
            .with_arg("description", description);
        drop(writeln!(io::stderr(), "{message}"));
    }

    fn report_complete(&self) {
        let message = localization::message(keys::STATUS_COMPLETE);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Silent reporter: emits nothing.
///
/// Used in standard output mode until future work (roadmap 3.9) adds
/// animated progress indicators via `indicatif`.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(&self, _current: u32, _total: u32, _description: &str) {}
    fn report_complete(&self) {}
}

/// The total number of pipeline stages reported during a build.
pub const PIPELINE_STAGE_COUNT: u32 = 5;

/// Localised description for "Loading manifest".
#[must_use]
pub fn stage_manifest_load() -> String {
    localization::message(keys::STATUS_STAGE_MANIFEST_LOAD).to_string()
}

/// Localised description for "Configuring network policy".
#[must_use]
pub fn stage_network_policy() -> String {
    localization::message(keys::STATUS_STAGE_NETWORK_POLICY).to_string()
}

/// Localised description for "Building dependency graph".
#[must_use]
pub fn stage_build_graph() -> String {
    localization::message(keys::STATUS_STAGE_BUILD_GRAPH).to_string()
}

/// Localised description for "Generating Ninja file".
#[must_use]
pub fn stage_generate_ninja() -> String {
    localization::message(keys::STATUS_STAGE_GENERATE_NINJA).to_string()
}

/// Localised description for "Executing build".
#[must_use]
pub fn stage_execute() -> String {
    localization::message(keys::STATUS_STAGE_EXECUTE).to_string()
}
