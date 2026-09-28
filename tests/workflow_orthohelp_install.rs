//! Hold the release lane's `cargo-orthohelp` install to a prebuilt release.
//!
//! `ortho-config` published no binaries until 0.9.1
//! (leynos/ortho-config#479), so this lane once carried a documented
//! exception permitting a guarded source build. 0.9.1 ships checksum-verified
//! archives with working binstall metadata (leynos/ortho-config#480), so the
//! exception is retired and these contracts exist to keep it retired: a
//! retirement is only durable if something rejects its return.

mod common;

#[path = "support/workflow_steps.rs"]
mod workflow_steps;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use workflow_steps::workflow_step_body;

/// Keep the release workflow on the tested prebuilt installer.
///
/// The workflow passes the pinned version and scoped token to the Python
/// script, which owns the version probe and non-compiling binstall flags. A
/// source-install scan keeps the retired fallback out of the workflow.
///
/// `ortho-config` published no binaries until 0.9.1
/// (leynos/ortho-config#479), so this lane once carried a documented exception
/// permitting a guarded source build. 0.9.1 ships checksum-verified archives
/// with working binstall metadata (leynos/ortho-config#480), so the exception
/// is retired and a source build is now forbidden outright.
fn assert_orthohelp_comes_from_a_prebuilt_release(contents: &str) -> Result<()> {
    let install_body = workflow_step_body(contents, "Install cargo-orthohelp").join("\n");
    ensure!(
        install_body.contains("scripts/install_orthohelp.py"),
        "workflow should delegate installation to the tested Python script"
    );
    ensure!(
        install_body.lines().any(|line| line.trim()
            == "run: uv run --no-project --python 3.14 scripts/install_orthohelp.py"),
        "workflow should keep the installer invocation thin"
    );
    ensure!(
        install_body.contains("INPUT_VERSION: '0.9.1'"),
        "workflow should pin the installed version through INPUT_VERSION"
    );
    ensure!(
        install_body.contains("GITHUB_TOKEN: ${{ github.token }}"),
        "workflow should retain the scoped token for release asset resolution"
    );
    let source_install =
        regex::Regex::new(r#"cargo\s+install\s+(?:[-"'][^\s]*\s+)*"?cargo-orthohelp"#)
            .context("compile the cargo-orthohelp source-install pattern")?;
    ensure!(
        source_install.find_iter(contents).count() == 0,
        "cargo-orthohelp must never be installed from source; 0.9.1 publishes \
         prebuilt archives for every platform this lane targets"
    );
    ensure!(
        !contents.contains("orthohelp-build"),
        "the retired source build's dedicated target directory should stay gone"
    );

    let build_index = contents
        .find("- name: Build release binary")
        .context("workflow should build the release binary")?;
    let install_index = contents
        .find("- name: Install cargo-orthohelp")
        .context("workflow should install cargo-orthohelp")?;
    ensure!(
        build_index < install_index,
        "cargo-orthohelp must be installed after rust-build-release provisions cargo-binstall"
    );
    Ok(())
}

#[test]
fn behavioural_build_and_package_generates_release_help_with_orthohelp() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert_orthohelp_comes_from_a_prebuilt_release(&contents)
        .expect("cargo-orthohelp should come from a pinned prebuilt release");
    assert!(
        contents.contains("scripts/generate-release-help.sh"),
        "workflow should call the release help script"
    );
    let help_step = workflow_step_body(&contents, "Generate release help").join("\n");
    for expected in [
        "TARGET: ${{ inputs.target }}",
        "BIN_NAME: ${{ env.BIN_NAME }}",
        "HELP_MODULE_NAME: ${{ inputs.platform == 'windows' && 'Netsuke' || env.BIN_NAME }}",
        "\"target/orthohelp/$TARGET/release\"",
    ] {
        assert!(
            help_step.contains(expected),
            "release-help step should preserve argument wiring: {expected}"
        );
    }
    assert!(
        contents.contains("man-paths: ${{ steps.stage.outputs['man-path'] }}"),
        "Linux packaging should consume the staged man-path output directly"
    );
    assert!(
        !contents.contains("target/generated-man"),
        "workflow should not rely on build.rs generated man pages"
    );
}

#[test]
fn behavioural_build_and_package_validates_release_help_tooling() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(contents.contains("scripts/install_orthohelp.py"));
    assert!(contents.contains("cargo-orthohelp --version"));
    for step_name in ["Validate cargo-orthohelp version", "Generate release help"] {
        let step_body = workflow_step_body(&contents, step_name).join("\n");
        assert!(
            step_body.contains("shell: bash"),
            "{step_name} should use Bash explicitly"
        );
    }
}
