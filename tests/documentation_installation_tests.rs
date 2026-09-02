//! Executable contracts for the installation and release examples in the
//! public user documentation.
//!
//! Split from `documentation_examples_tests.rs` to keep that file within the
//! repository's 400-line cap; the shared fixtures live in the
//! `documentation_examples` directory module, which every documentation test
//! binary declares independently.

pub mod documentation_examples;

use anyhow::{Context, Result, ensure};
use documentation_examples::{documented_example, load_documented_examples};
use test_support::fs as test_fs;

#[test]
fn installation_examples_match_source_and_release_contracts() -> Result<()> {
    assert_release_installation_contract()?;
    let readme = documented_example("readme-source-install")?;
    let guide = documented_example("guide-source-install")?;
    let expected = concat!(
        "git clone https://github.com/leynos/netsuke.git\n",
        "cd netsuke\n",
        "cargo install --path .\n"
    );
    ensure!(readme.body == expected, "README source install drifted");
    ensure!(guide.body == expected, "user guide source install drifted");
    assert_windows_setup_examples()
}

#[test]
fn registry_install_examples_pin_the_toolchain() -> Result<()> {
    // Registry installs build outside a checkout, where rust-toolchain.toml
    // does not apply, so every tagged example installing from crates.io must
    // select the pinned nightly itself; that nightly is what enables Polonius.
    // `cargo binstall` fetches a prebuilt binary and `cargo install --path .`
    // runs inside a checkout, so both are exempt.
    let mut registry_install_ids = Vec::new();
    for example in load_documented_examples()? {
        let mut example_matches = false;
        for line in example.body.lines() {
            if !line.contains("install netsuke-build") || line.contains("binstall") {
                continue;
            }
            ensure!(
                line.contains("cargo +nightly-2026-08-23 install netsuke-build"),
                "{id} must install with the pinned nightly toolchain: {line}",
                id = example.id
            );
            example_matches = true;
        }
        if example_matches {
            registry_install_ids.push(example.id);
        }
    }
    ensure!(
        registry_install_ids.len() >= 2,
        "expected registry-install examples in the README and users' guide, found {registry_install_ids:?}"
    );
    // The quickstart carries no tested-example fences, so guard its prose
    // against reintroducing an unsupported bare registry install.
    let quickstart =
        test_fs::read_to_string("docs/quickstart.md").context("read docs/quickstart.md")?;
    ensure!(
        !quickstart.contains("cargo install netsuke-build"),
        "docs/quickstart.md must defer to the users' guide install command"
    );
    Ok(())
}

/// Check the documented crates.io install command and release details.
fn assert_release_installation_contract() -> Result<()> {
    let readme_binstall = documented_example("readme-binstall-install")?;
    let guide_binstall = documented_example("guide-binstall-install")?;
    let expected_binstall = "cargo binstall netsuke-build\n";
    ensure!(
        readme_binstall.body == expected_binstall,
        "README binstall drifted"
    );
    ensure!(
        guide_binstall.body == expected_binstall,
        "user guide binstall drifted"
    );
    let readme_release = documented_example("readme-crates-io-install")?;
    let guide_release = documented_example("guide-crates-io-install")?;
    // Registry installs run outside a checkout, so the packaged source does
    // not see rust-toolchain.toml; the documented command must select the
    // pinned nightly itself, which is what enables Polonius.
    let expected_release = concat!(
        "rustup toolchain install nightly-2026-08-23\n",
        "cargo +nightly-2026-08-23 install netsuke-build\n"
    );
    ensure!(readme_release.body == expected_release, "README drifted");
    ensure!(guide_release.body == expected_release, "user guide drifted");
    // Derive version literals from the crate version so docs contracts track
    // release bumps instead of drifting behind them.
    let expected_release_details = [
        concat!(
            "https://github.com/leynos/netsuke/releases/tag/v",
            env!("CARGO_PKG_VERSION")
        ),
        "Debian (`.deb`) and RPM (`.rpm`)",
        "Installer package (`.pkg`)",
        "Windows Installer (`.msi`)",
        "x86-64 (`amd64`) and Arm64 (`arm64`)",
        "Installer packages do not have checksum",
        "The Windows MSI installs to `C:\\Program Files\\netsuke`",
    ];
    for path in ["README.md", "docs/users-guide.md"] {
        let document = test_fs::read_to_string(path).with_context(|| format!("read {path}"))?;
        for expected in expected_release_details {
            ensure!(
                document.contains(expected),
                "{path} should document v{} release detail: {expected}",
                env!("CARGO_PKG_VERSION")
            );
        }
    }
    let users_guide =
        test_fs::read_to_string("docs/users-guide.md").context("read docs/users-guide.md")?;
    ensure!(
        users_guide.contains(concat!(
            "installing a later beta or final MSI\n",
            "replaces the existing installation"
        )),
        "users' guide should document MSI replacement within a version series"
    );
    Ok(())
}

/// Check the documented Windows help, PATH, and staging contracts.
fn assert_windows_setup_examples() -> Result<()> {
    let windows = documented_example("guide-windows-help")?;
    ensure!(windows.body == "Get-Help Netsuke -Full\n", "help drifted");
    let windows_path = documented_example("guide-windows-path")?;
    let windows_path_fragments = [
        "SetEnvironmentVariable",
        "$netsukeDirectory",
        "SetEnvironmentVariable('Path', $newUserPath, 'User')",
    ];
    ensure!(
        windows_path_fragments
            .into_iter()
            .all(|fragment| windows_path.body.contains(fragment)),
        "Windows PATH setup should persist the MSI installation directory"
    );
    let windows_help_install = documented_example("guide-windows-help-install")?;
    let windows_help_fragments = [
        "Import-Module",
        concat!(
            "$moduleDirectory = Join-Path $moduleRoot 'Netsuke\\",
            env!("CARGO_PKG_VERSION"),
            "'"
        ),
        "Import-Module (Join-Path $moduleDirectory 'Netsuke.psd1')",
        "*windows-$architecture*",
    ];
    ensure!(
        windows_help_fragments
            .into_iter()
            .all(|fragment| windows_help_install.body.contains(fragment)),
        "Windows help setup should import the downloaded sidecars"
    );
    let staging = test_fs::read_to_string(".github/release-staging.toml")
        .context("read release staging configuration")?;
    ensure!(
        staging.contains("Netsuke-help.xml") && staging.contains("about_Netsuke.help.txt"),
        "Windows release should stage the help consumed by Get-Help"
    );
    Ok(())
}
