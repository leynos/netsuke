//! Validate the `WiX` major-upgrade contract for Windows installer packages.

use anyhow::{Context, Result, ensure};
use roxmltree::Document;
use test_support::fs as test_fs;

/// Verify that `WiX` treats prerelease and final MSI packages as upgrades.
#[test]
fn package_declares_safe_major_upgrade_metadata() -> Result<()> {
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
