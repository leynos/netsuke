//! Prepare and carry the optional capability to run real-Ninja checks.

use proptest::{
    prelude::ProptestConfig,
    test_runner::{TestCaseError, contextualize_config},
};
use std::ffi::{OsStr, OsString};
use tempfile::TempDir;
use test_support::ninja::{NinjaWorkspaceError, ninja_integration_workspace};

/// A verified command name that authorizes this oracle to invoke real Ninja.
pub(super) struct NinjaCapability {
    /// The executable name checked by the integration workspace probe.
    executable: OsString,
}

impl NinjaCapability {
    /// Return the executable name that passed the integration probe.
    pub(super) fn executable(&self) -> &OsStr {
        &self.executable
    }
}

/// Prepare the real-Ninja capability, skipping only when policy permits it.
///
/// `ninja_integration_workspace` owns the `NETSUKE_REQUIRE_NINJA` policy: it
/// panics for an unavailable required binary and returns probe failures only
/// when this test run is allowed to skip.
///
/// # Errors
///
/// Returns a test-case failure when creating the integration workspace fails
/// for a reason other than the Ninja availability probe.
pub(super) fn prepare_ninja_capability() -> Result<Option<NinjaCapability>, TestCaseError> {
    prepare_ninja_capability_with(ninja_integration_workspace)
}

/// Build the manual runner config with the `proptest!` macro's replay metadata.
///
/// The oracle is prepared before sampling, so these properties use a manual
/// runner instead of `proptest!`. Keep its contextualized settings, source file,
/// and full test name identical to the macro so persisted seeds still replay.
pub(super) fn ninja_property_config(
    source_file: &'static str,
    test_name: &'static str,
) -> ProptestConfig {
    let mut config = contextualize_config(ProptestConfig {
        cases: 64,
        max_shrink_time: super::super::support::HEAVY_SHRINK_TIME_MS,
        ..ProptestConfig::default()
    });
    config.source_file = Some(source_file);
    config.test_name = Some(test_name);
    config
}

/// Classify the integration helper's result and build an explicit capability.
///
/// This narrow seam keeps policy handling testable without changing the
/// harness process environment. A required missing binary panics inside the
/// integration helper before its probe error reaches this classifier.
///
/// # Errors
///
/// Returns a test-case failure for workspace creation failures and any other
/// non-probe integration error.
fn prepare_ninja_capability_with(
    create_workspace: impl FnOnce() -> Result<TempDir, NinjaWorkspaceError>,
) -> Result<Option<NinjaCapability>, TestCaseError> {
    match create_workspace() {
        Ok(workspace) => {
            drop(workspace);
            Ok(Some(NinjaCapability {
                executable: OsString::from("ninja"),
            }))
        }
        Err(
            error @ (NinjaWorkspaceError::ProbeSpawn(_)
            | NinjaWorkspaceError::ProbeFailed(_)
            | NinjaWorkspaceError::ProbeTimeout(_)),
        ) => {
            tracing::warn!(
                error = %error,
                "skipping OBL-NINJA: no usable `ninja` binary; set \
                 NETSUKE_REQUIRE_NINJA=1 to make this a failure"
            );
            Ok(None)
        }
        Err(error) => Err(TestCaseError::fail(format!(
            "create Ninja workspace: {error}"
        ))),
    }
}

#[cfg(test)]
mod preparation_tests {
    //! Focused tests for optional real-Ninja capability preparation.

    use super::{NinjaCapability, prepare_ninja_capability_with};
    use std::{io, process::ExitStatus, time::Duration};
    use tempfile::TempDir;
    use test_support::ninja::NinjaWorkspaceError;

    #[test]
    fn successful_probe_returns_a_capability() {
        let prepared = prepare_ninja_capability_with(|| {
            TempDir::new().map_err(NinjaWorkspaceError::Workspace)
        })
        .expect("successful Ninja preparation should not fail");
        let Some(NinjaCapability { executable }) = prepared else {
            panic!("successful probe should provide a Ninja capability");
        };
        assert_eq!(executable, "ninja");
    }

    #[test]
    fn permitted_probe_failures_return_no_capability() {
        let errors = [
            NinjaWorkspaceError::ProbeSpawn(io::Error::other("probe spawn failed")),
            NinjaWorkspaceError::ProbeFailed(ExitStatus::default()),
            NinjaWorkspaceError::ProbeTimeout(Duration::from_secs(2)),
        ];
        for error in errors {
            let prepared = prepare_ninja_capability_with(|| Err(error))
                .expect("optional Ninja absence should not fail the test");
            assert!(prepared.is_none());
        }
    }

    #[test]
    fn non_probe_workspace_failures_are_test_case_errors() {
        let result = prepare_ninja_capability_with(|| {
            Err(NinjaWorkspaceError::Workspace(io::Error::other(
                "temporary directory unavailable",
            )))
        });
        let Err(error) = result else {
            panic!("a workspace failure must remain a test-case failure");
        };
        assert!(format!("{error:?}").contains("create Ninja workspace"));
    }
}
