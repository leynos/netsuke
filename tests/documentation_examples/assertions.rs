//! Assertions for executable-documentation integration tests.

use anyhow::{Result, ensure};
use test_support::netsuke::NetsukeRun;

/// Assert that a documented Netsuke invocation completed successfully.
///
/// # Errors
///
/// Returns an error containing the captured output when the invocation fails.
pub fn assert_success(run: &NetsukeRun, context: &str) -> Result<()> {
    ensure!(
        run.success,
        "{context} should succeed; stdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
    Ok(())
}
