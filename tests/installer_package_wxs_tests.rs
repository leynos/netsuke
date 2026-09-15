//! Validate the `WiX` major-upgrade contract for Windows installer packages.

use anyhow::{Context, Result, ensure};
use roxmltree::Document;
use test_support::fs as test_fs;

/// Verify that `WiX` declares major-upgrade metadata for prerelease packages.
#[test]
fn package_declares_major_upgrade_metadata() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let package = document
        .root_element()
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "Package")
        .context("Package.wxs should contain a Package element")?;

    ensure!(
        package.attribute("UpgradeCode") == Some("{870359C0-A975-4DCB-992A-AD67D97292DD}"),
        "Package should retain its stable UpgradeCode"
    );
    ensure!(
        package.attribute("Id").is_none(),
        "Package should let WiX generate a fresh ProductCode for each build"
    );

    let major_upgrade = package
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "MajorUpgrade")
        .context("Package should declare a MajorUpgrade element")?;
    ensure!(
        major_upgrade.attribute("AllowSameVersionUpgrades") == Some("yes"),
        "MajorUpgrade should replace prerelease MSI packages with the same numeric version"
    );
    ensure!(
        major_upgrade.attribute("AllowDowngrades").is_none(),
        "MajorUpgrade should continue to block numeric release downgrades"
    );
    ensure!(
        major_upgrade
            .attribute("DowngradeErrorMessage")
            .is_some_and(|message| !message.is_empty()),
        "MajorUpgrade should explain why a numeric downgrade is blocked"
    );
    ensure!(
        major_upgrade.attribute("Schedule") == Some("afterInstallInitialize"),
        "MajorUpgrade should remove the prior product early enough to roll back safely"
    );
    Ok(())
}

/// Verify that `WiX` v4 package metadata uses the supported schema locations.
#[test]
fn package_declares_wix_v4_metadata() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let package = document
        .root_element()
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "Package")
        .context("Package.wxs should contain a Package element")?;

    ensure!(
        package.attribute("Scope") == Some("perMachine"),
        "Package should explicitly retain its per-machine installation scope"
    );
    ensure!(
        package.attribute("InstallScope").is_none(),
        "Package should not use the WiX v3 InstallScope attribute"
    );
    ensure!(
        package.attribute("Description").is_none(),
        "Package should not use the WiX v3 Description attribute"
    );
    let summary_information = package
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "SummaryInformation")
        .context("Package should declare summary information")?;
    ensure!(
        summary_information.attribute("Description")
            == Some("$(env.PRODUCT_NAME) command line interface"),
        "SummaryInformation should retain the installer product description"
    );
    Ok(())
}

/// Verify that `WiX` blocks an older prerelease within one numeric release line.
#[test]
fn package_declares_same_version_release_ordering() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let package = document
        .root_element()
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "Package")
        .context("Package.wxs should contain a Package element")?;
    let release_rank = package
        .children()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "Property"
                && node.attribute("Id") == Some("NETSUKE_RELEASE_RANK")
        })
        .context("Package should declare the candidate release rank")?;
    ensure!(
        release_rank.attribute("Value") == Some("$(env.NETSUKE_RELEASE_RANK)"),
        "Package should receive the candidate release rank from packaging"
    );
    let previous_release_rank = package
        .children()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "Property"
                && node.attribute("Id") == Some("PREVIOUS_NETSUKE_RELEASE_RANK")
        })
        .context("Package should search for the installed release rank")?;
    let rank_search = previous_release_rank
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "RegistrySearch")
        .context("Package should search the installed release-rank registry value")?;
    ensure!(
        rank_search.attribute("Key") == Some("Software\\Leynos\\Netsuke\\$(var.Version)"),
        "Package should compare release ranks only within the numeric MSI version"
    );
    let release_rank_condition = package
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "Launch")
        .context("Package should block older same-version prereleases")?;
    ensure!(
        release_rank_condition.attribute("Condition")
            == Some(
                "Installed OR NOT PREVIOUS_NETSUKE_RELEASE_RANK OR NETSUKE_RELEASE_RANK >= PREVIOUS_NETSUKE_RELEASE_RANK"
            ),
        "Package should allow only equal-or-newer release ranks within a version line"
    );
    Ok(())
}

/// Verify that `WiX` matches the shared action's custom-authoring environment.
#[test]
fn package_uses_shared_action_environment_contract() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let executable = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "File")
        .context("Package should contain the executable file")?;
    ensure!(
        executable.attribute("Source") == Some("$(env.APPLICATION_SPEC)"),
        "Package should use the application-path environment contract of windows-package"
    );
    let release_rank_marker = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "RegistryValue"
                && node.attribute("Name") == Some("ReleaseRank")
        })
        .context("Package should persist the installed release rank")?;
    ensure!(
        release_rank_marker.attribute("Key") == Some("Software\\Leynos\\Netsuke\\$(var.Version)"),
        "Package should persist release ranks within the numeric MSI version"
    );
    ensure!(
        release_rank_marker.attribute("Value") == Some("$(env.NETSUKE_RELEASE_RANK)"),
        "Package should persist the candidate release rank for later comparisons"
    );
    Ok(())
}
