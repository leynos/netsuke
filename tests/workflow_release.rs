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
fn behavioural_release_workflow_authorizes_before_reading_release_modes() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains("id: release_authorization"),
        "release workflow should decide release writes in a step of its own"
    );
    assert!(
        contents.contains("uv run --no-project --python 3.14 scripts/authorize_release_writes.py"),
        "release workflow should run the release-write authorization policy"
    );
    assert!(
        contents.contains("INPUT_WORKFLOW_REF: ${{ github.workflow_ref }}"),
        "release workflow should pass the workflow ref so a direct push is distinguishable"
    );
    assert!(
        contents.contains(
            "check-tag: ${{ fromJSON(steps.release_authorization.outputs['should-publish']) }}"
        ),
        "release workflow should gate tag checking on the authorized should-publish output"
    );
    assert!(
        contents.contains(
            "should_publish: ${{ steps.release_authorization.outputs['should-publish'] }}"
        ),
        "release workflow should capture the authorized should-publish output"
    );
    assert!(
        contents.contains("dry_run: ${{ steps.release_authorization.outputs['dry-run'] }}"),
        "release workflow should capture the authorized dry-run output"
    );
    assert!(
        contents.contains("should_upload_workflow_artifacts: ${{ steps.release_authorization.outputs['should-upload-workflow-artifacts'] }}"),
        "release workflow should capture the authorized workflow artefact upload output"
    );
    assert!(
        contents.contains("should_upload_package_artifacts: >-"),
        "release metadata should expose a separate package artefact upload output"
    );
    assert_eq!(
        contents
            .matches("fromJSON(needs.metadata.outputs.should_upload_package_artifacts)")
            .count(),
        3,
        "every platform build should receive the package artefact upload output"
    );
    assert!(
        contents.contains("dry-run: ${{ needs.metadata.outputs.should_publish != 'true' }}"),
        "release asset upload should validate its plan on non-publishing runs"
    );
    assert!(
        contents.contains("needs.metadata.outputs.dry_run != 'true'"),
        "publish-release should refuse a release write while the run is a dry run"
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
