//! Validate build-and-package workflow wiring for shared actions.

use std::fs;
use std::path::PathBuf;

fn workflow_contents(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("workflows").join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "build-and-package workflow {} should be readable: {err}",
            path.display()
        )
    })
}

#[test]
fn behavioural_build_and_package_wiring_matches_shared_actions() {
    let contents = workflow_contents("build-and-package.yml");

    assert!(
        contents.contains("stage-release-artefacts@"),
        "workflow should use shared stage-release-artefacts action"
    );
    assert!(
        contents.contains("normalize-windows-paths: ${{ inputs.platform == 'windows' }}"),
        "workflow should normalize Windows paths when staging on Windows"
    );
    assert!(
        contents.contains("application-path: ${{ steps.stage.outputs['binary-path'] }}"),
        "windows-package should consume staged binary-path output"
    );
    assert!(
        contents.contains("license-rtf-path: ${{ steps.stage.outputs['license-path'] }}"),
        "windows-package should consume staged license-path output"
    );
    assert!(
        contents.contains("upload-artefact: ${{ inputs['should-upload-workflow-artifacts'] }}"),
        "windows-package should use the upload-artefact input spelling"
    );
    assert!(
        contents.contains("binary: ${{ steps.stage.outputs['binary-path'] }}"),
        "macos-package should consume staged binary-path output"
    );
    assert!(
        contents.contains("manpage: ${{ steps.stage.outputs['man-path'] }}"),
        "macos-package should consume staged man-path output"
    );
    assert!(
        contents.contains("${{ steps.stage.outputs['artifact-dir'] }}"),
        "workflow should use the staged artifact-dir output for uploads"
    );
}
