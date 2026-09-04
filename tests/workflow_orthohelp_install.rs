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

/// Reject every `cargo install` form that would compile `cargo-orthohelp`.
///
/// Matching the bare `cargo install cargo-orthohelp` prefix is not enough:
/// `cargo install --locked cargo-orthohelp@0.9.1` compiles the tool just the
/// same, and so does any other flag placed before the crate name. The pattern
/// therefore allows arbitrary flags and version selectors between the
/// subcommand and the crate.
///
/// `ortho-config` published no binaries until 0.9.1
/// (leynos/ortho-config#479), so this lane once carried a documented exception
/// permitting a guarded source build. 0.9.1 ships checksum-verified archives
/// with working binstall metadata (leynos/ortho-config#480), so the exception
/// is retired and a source build is now forbidden outright.
fn assert_orthohelp_comes_from_a_prebuilt_release(contents: &str) -> Result<()> {
    let install_body = workflow_step_body(contents, "Install cargo-orthohelp").join("\n");
    ensure!(
        install_body.contains("cargo binstall --no-confirm --locked \\"),
        "workflow should install cargo-orthohelp with cargo-binstall"
    );
    // Structural rather than hopeful. The retired form named the binary-only
    // strategies it preferred and fell through to a compile when they missed.
    // Disabling the compile strategy means a release that stops publishing
    // assets fails the lane instead of quietly building the tool from source.
    ensure!(
        install_body.contains("--disable-strategies compile"),
        "cargo-binstall must be unable to fall back to compiling the tool"
    );
    ensure!(
        !install_body.contains("--strategies crate-meta-data,quick-install"),
        "the retired strategy list permitted a compile fallback and must not return"
    );
    // Only 0.9.1 and later carry release assets, so pinning below that would
    // reintroduce the compile this contract exists to forbid.
    ensure!(
        install_body.contains("cargo-orthohelp@0.9.1"),
        "workflow should pin a cargo-orthohelp release that publishes assets"
    );

    // Flags, `--version`/`--index` selectors, and quoting all sit between the
    // subcommand and the crate name, so the pattern allows arbitrary tokens
    // that are not themselves the crate. Zero matches: unlike the retired
    // exception, no source install of this tool is permitted anywhere.
    let source_install =
        regex::Regex::new(r#"cargo\s+install\s+(?:[-"'][^\s]*\s+)*"?cargo-orthohelp"#)
            .context("compile the cargo-orthohelp source-install pattern")?;
    ensure!(
        source_install.find_iter(contents).count() == 0,
        "cargo-orthohelp must never be installed from source; 0.9.1 publishes \
         prebuilt archives for every platform this lane targets"
    );
    // The dedicated build directory existed only to keep that source build's
    // compiler output away from the product's tree. With no source build it
    // has no purpose, and leaving it in the cache entry would archive an empty
    // path on every packaging run.
    ensure!(
        !contents.contains("orthohelp-build"),
        "the source build's dedicated target directory should be gone"
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
    assert!(
        contents.contains("\"target/orthohelp/${{ inputs.target }}/release\""),
        "workflow should generate help under target/orthohelp"
    );
    assert!(
        contents.contains("man-paths: ${{ steps.stage_paths.outputs.man_path }}"),
        "Linux packaging should consume the staged man_path output"
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

    assert!(
        contents.contains(
            "cargo-orthohelp --version | grep -Eq '(^|[[:space:]])0\\.9\\.1([[:space:]]|$)'"
        ),
        "workflow should validate the installed cargo-orthohelp version"
    );
    assert!(
        contents.contains("\"${{ inputs.platform == 'windows' && 'Netsuke' || env.BIN_NAME }}\""),
        "workflow should pass the PowerShell module name explicitly"
    );
    for step_name in ["Validate cargo-orthohelp version", "Generate release help"] {
        let step_body = workflow_step_body(&contents, step_name).join("\n");
        assert!(
            step_body.contains("shell: bash"),
            "{step_name} should use Bash explicitly"
        );
    }
}
