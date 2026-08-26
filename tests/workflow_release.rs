//! Validate release workflow wiring for shared actions.

mod common;

use common::workflow_contents;
use rstest::rstest;

#[test]
fn behavioural_release_workflow_uses_shared_actions() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

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
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

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
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

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

#[test]
fn behavioural_release_workflow_requires_pinned_canaries() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains("release-admission-canaries:"),
        "release workflow should define the downstream canary admission job"
    );
    assert!(
        contents.contains("leynos/repovec-appliance 6be365b4b30ef48537add5719a9b387ccc41777f")
            && contents.contains("leynos/mxd 8146278cc82506c222bb78d4f3fc05c12ed95b41")
            && contents.contains("leynos/ortho-config b42b5d0adfacd79456d2a2f9edbf9f561aac943b"),
        "release workflow should keep every v0.1.0 canary revision pinned"
    );
    assert!(
        contents.contains("- release-admission-canaries"),
        "release publication should require successful downstream canaries"
    );
}

#[rstest]
#[case("linux-x86_64")]
#[case("linux-aarch64")]
fn behavioural_release_workflow_passes_linux_stage_targets(#[case] target_key: &str) {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains(&format!("target_key: {target_key}")),
        "release workflow should declare Linux stage target {target_key}"
    );
    assert!(
        contents.contains("stage-target: ${{ matrix.target_key }}"),
        "release workflow should pass matrix stage targets to build-and-package"
    );
}
