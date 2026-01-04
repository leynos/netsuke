//! Validate build-and-package workflow wiring for shared actions.

mod common;

use common::workflow_contents;

#[test]
fn behavioural_build_and_package_wiring_matches_shared_actions() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains("stage-release-artefacts@"),
        "workflow should use shared stage-release-artefacts action"
    );
    assert!(
        contents.contains("normalize-windows-paths: ${{ inputs.platform == 'windows' }}"),
        "workflow should normalize Windows paths when staging on Windows"
    );
    assert!(
        contents.contains("application-path: ${{ steps.stage_paths.outputs.binary_path }}"),
        "windows-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("license-rtf-path: ${{ steps.stage_paths.outputs.license_path }}"),
        "windows-package should consume staged license_path output"
    );
    assert!(
        contents.contains("upload-artefact: ${{ inputs['should-upload-workflow-artifacts'] }}"),
        "windows-package should use the upload-artefact input spelling"
    );
    assert!(
        contents.contains("binary: ${{ steps.stage_paths.outputs.binary_path }}"),
        "macos-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("manpage: ${{ steps.stage_paths.outputs.man_path }}"),
        "macos-package should consume staged man_path output"
    );
    assert!(
        contents.contains("${{ steps.stage_paths.outputs.artifact_dir }}"),
        "workflow should use the staged artifact_dir output for uploads"
    );
}
