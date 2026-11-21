//! Real-Ninja ordering for a serial phony edge's directory preparation.
//!
//! The preparation edge that creates a nested phony output's directory is an
//! extra consumer of the edge's prerequisites. Naming the raw serial list
//! there would let Ninja start a later stage while an earlier one still runs,
//! even though the recipe statement itself is staged through the gates. The
//! scenario makes that failure visible: `check` tests for the marker `setup`
//! writes only after its delay, so a parallel start fails the build.

use super::phony_runtime::{SERIAL_OUTPUT_MANIFEST, Workspace, temp_workspace};
use anyhow::{Result, ensure};
use rstest::rstest;
use tempfile::TempDir;

/// Run a serial prerequisite list in order despite the preparation edge.
#[rstest]
fn a_serial_phony_output_orders_its_preparation(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(SERIAL_OUTPUT_MANIFEST)?;
    workspace.build()?;
    let entries = workspace.lines("serial.log")?;
    ensure!(
        entries == ["setup", "check"],
        "the serial plan must run check only after setup: {entries:?}",
    );
    ensure!(
        workspace.exists("build/report")?,
        "the phony recipe must still write its nested output",
    );
    Ok(())
}
