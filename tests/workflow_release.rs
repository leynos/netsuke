//! Validate release workflow wiring for shared actions.

use std::fs;
use std::path::PathBuf;

fn workflow_contents(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("workflows").join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "release workflow {} should be readable: {err}",
            path.display()
        )
    })
}

#[test]
fn behavioural_release_workflow_uses_shared_actions() {
    let contents = workflow_contents("release.yml");

    assert!(
        contents.contains("determine-release-modes@"),
        "release workflow should use shared determine-release-modes action"
    );
    assert!(
        contents.contains("ensure-cargo-version@"),
        "release workflow should use shared ensure-cargo-version action"
    );
    assert!(
        contents.contains("export-cargo-metadata@"),
        "release workflow should use shared export-cargo-metadata action"
    );
    assert!(
        contents.contains("upload-release-assets@"),
        "release workflow should use shared upload-release-assets action"
    );
}

#[test]
fn behavioural_release_workflow_exports_bin_name() {
    let contents = workflow_contents("release.yml");

    assert!(
        contents.contains("fields: bin-name"),
        "release workflow should export the bin-name field"
    );
    assert!(
        contents.contains("bin-name: ${{ needs.metadata.outputs.bin_name }}"),
        "release workflow should pass bin-name to upload-release-assets"
    );
}

#[test]
fn behavioural_release_workflow_wires_release_modes_outputs() {
    let contents = workflow_contents("release.yml");

    assert!(
        contents
            .contains("check-tag: ${{ fromJSON(steps.release_modes.outputs['should-publish']) }}"),
        "release workflow should gate tag checking on should-publish output"
    );
    assert!(
        contents.contains("should_publish: ${{ steps.release_modes.outputs['should-publish'] }}"),
        "release workflow should capture should-publish output"
    );
    assert!(
        contents.contains("dry_run: ${{ steps.release_modes.outputs['dry-run'] }}"),
        "release workflow should capture dry-run output"
    );
    assert!(
        contents.contains("should_upload_workflow_artifacts: ${{ steps.release_modes.outputs['should-upload-workflow-artifacts'] }}"),
        "release workflow should capture workflow artefact upload output"
    );
}
