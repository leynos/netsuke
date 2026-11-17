//! Validate the `WiX` major-upgrade contract for Windows installer packages.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use roxmltree::{Document, Node};
use test_support::fs as test_fs;

/// The component GUID v0.1.0-beta4 shipped for the installed executable.
const LEGACY_COMPONENT_GUID: &str = "{8A7AA941-A8C3-4314-8A7D-3195B4806511}";
/// The component GUID the release uses after moving the executable.
const NATIVE_COMPONENT_GUID: &str = "{C3257D3D-FC15-40C7-A36F-ED6F37025326}";
/// The frozen v0.1.0-beta4 authoring the MSI gate builds its beta1 fixture from.
const LEGACY_FIXTURE_PATH: &str =
    ".github/actions/windows-msi-upgrade-validation/fixtures/legacy-program-files-x86.wxs";

/// Find the first descendant element with the given tag name and attribute.
fn find_element<'a, 'input>(
    document: &'a Document<'input>,
    tag_name: &str,
    attribute: &str,
    value: &str,
) -> Result<Node<'a, 'input>> {
    document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == tag_name
                && node.attribute(attribute) == Some(value)
        })
        .with_context(|| format!("expected {tag_name} with {attribute}={value}"))
}

/// Find the first child element with the given tag name.
fn find_child<'a, 'input>(parent: Node<'a, 'input>, tag_name: &str) -> Result<Node<'a, 'input>> {
    parent
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == tag_name)
        .with_context(|| format!("expected a {tag_name} child"))
}

/// Read an element's attributes as a sorted map, omitting one allowed difference.
fn comparable_attributes(node: Node<'_, '_>, omitted: Option<&str>) -> BTreeMap<String, String> {
    node.attributes()
        .filter(|attribute| Some(attribute.name()) != omitted)
        .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
        .collect()
}

/// Compare two authoring trees, allowing only the moved layout to differ.
///
/// The `StandardDirectory` `Id` names the program files directory the payload
/// is rooted at, and the `BinaryComponent` `Guid` changes with the executable's
/// target location. Every other element, attribute, and child order must match,
/// so the legacy fixture cannot drift from the release authoring it stands in
/// for.
fn assert_semantically_equal_except_layout(
    release: Node<'_, '_>,
    legacy: Node<'_, '_>,
) -> Result<()> {
    let tag_name = release.tag_name().name();
    ensure!(
        tag_name == legacy.tag_name().name(),
        "release authoring has {tag_name} where the legacy fixture has {}",
        legacy.tag_name().name()
    );
    let omitted = match tag_name {
        "StandardDirectory" => Some("Id"),
        "Component" if release.attribute("Id") == Some("BinaryComponent") => Some("Guid"),
        _ => None,
    };
    ensure!(
        comparable_attributes(release, omitted) == comparable_attributes(legacy, omitted),
        "release {tag_name} attributes differ from the legacy fixture beyond {omitted:?}: \
         release {:?}, legacy {:?}",
        comparable_attributes(release, omitted),
        comparable_attributes(legacy, omitted)
    );
    let release_children: Vec<Node<'_, '_>> = release.children().filter(Node::is_element).collect();
    let legacy_children: Vec<Node<'_, '_>> = legacy.children().filter(Node::is_element).collect();
    ensure!(
        release_children.len() == legacy_children.len(),
        "release {tag_name} has {} child elements where the legacy fixture has {}",
        release_children.len(),
        legacy_children.len()
    );
    for (release_child, legacy_child) in release_children.into_iter().zip(legacy_children) {
        assert_semantically_equal_except_layout(release_child, legacy_child)?;
    }
    Ok(())
}

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

/// Verify that the release installs under the native program files directory.
#[test]
fn package_roots_install_folder_at_the_6432_program_files_directory() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let install_folder = find_element(&document, "Directory", "Id", "INSTALLFOLDER")?;
    ensure!(
        install_folder.attribute("Name") == Some("$(env.PRODUCT_NAME)"),
        "INSTALLFOLDER should continue to take its name from the product name"
    );
    let standard_directory = install_folder
        .parent_element()
        .context("INSTALLFOLDER should be a child of a standard directory")?;
    ensure!(
        standard_directory.tag_name().name() == "StandardDirectory",
        "INSTALLFOLDER should be a child of a standard directory, got {}",
        standard_directory.tag_name().name()
    );
    ensure!(
        standard_directory.attribute("Id") == Some("ProgramFiles6432Folder"),
        "INSTALLFOLDER should follow the package architecture through \
         ProgramFiles6432Folder, got {:?}",
        standard_directory.attribute("Id")
    );
    Ok(())
}

/// Verify that the moved executable carries a fresh component identity.
#[test]
fn package_gives_the_moved_binary_component_a_new_identity() -> Result<()> {
    let contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let document = Document::parse(&contents).context("parse installer/Package.wxs")?;
    let component = find_element(&document, "Component", "Id", "BinaryComponent")?;
    ensure!(
        component.attribute("Guid") == Some(NATIVE_COMPONENT_GUID),
        "BinaryComponent should carry the new GUID for its changed target location, got {:?}",
        component.attribute("Guid")
    );
    ensure!(
        !contents.contains(LEGACY_COMPONENT_GUID),
        "installer/Package.wxs must not reuse the beta4 component GUID after moving its key path"
    );
    ensure!(
        component.attribute("Directory") == Some("INSTALLFOLDER"),
        "BinaryComponent should install into INSTALLFOLDER"
    );
    let executable = find_child(component, "File")?;
    ensure!(
        executable.attribute("KeyPath") == Some("yes"),
        "the executable should remain the component's key path"
    );
    let release_rank_marker = find_child(component, "RegistryValue")?;
    ensure!(
        release_rank_marker.attribute("Key") == Some("Software\\Leynos\\Netsuke\\$(var.Version)"),
        "the release-rank marker should keep its shipped registry location"
    );
    ensure!(
        release_rank_marker.attribute("Value") == Some("$(env.NETSUKE_RELEASE_RANK)"),
        "the release-rank marker should keep recording the candidate rank"
    );
    Ok(())
}

/// Verify that the frozen beta4 fixture differs only where the migration does.
#[test]
fn legacy_fixture_matches_the_release_authoring_except_its_layout() -> Result<()> {
    let release_contents =
        test_fs::read_to_string("installer/Package.wxs").context("read installer/Package.wxs")?;
    let legacy_contents =
        test_fs::read_to_string(LEGACY_FIXTURE_PATH).context("read the legacy MSI fixture")?;
    let release = Document::parse(&release_contents).context("parse installer/Package.wxs")?;
    let legacy = Document::parse(&legacy_contents).context("parse the legacy MSI fixture")?;

    assert_semantically_equal_except_layout(release.root_element(), legacy.root_element())?;

    let release_standard = find_element(
        &release,
        "StandardDirectory",
        "Id",
        "ProgramFiles6432Folder",
    )?;
    let legacy_standard = find_element(&legacy, "StandardDirectory", "Id", "ProgramFilesFolder")?;
    ensure!(
        release_standard.attribute("Id") != legacy_standard.attribute("Id"),
        "the fixture should freeze the beta4 ProgramFilesFolder root"
    );
    let release_component = find_element(&release, "Component", "Id", "BinaryComponent")?;
    let legacy_component = find_element(&legacy, "Component", "Id", "BinaryComponent")?;
    ensure!(
        legacy_component.attribute("Guid") == Some(LEGACY_COMPONENT_GUID),
        "the fixture should freeze the beta4 BinaryComponent GUID, got {:?}",
        legacy_component.attribute("Guid")
    );
    ensure!(
        release_component.attribute("Guid") == Some(NATIVE_COMPONENT_GUID),
        "the release should use the new BinaryComponent GUID, got {:?}",
        release_component.attribute("Guid")
    );

    let release_package = find_child(release.root_element(), "Package")?;
    let legacy_package = find_child(legacy.root_element(), "Package")?;
    ensure!(
        legacy_package.attribute("UpgradeCode") == release_package.attribute("UpgradeCode"),
        "the fixture should keep the stable UpgradeCode so upgrades remain related products"
    );
    let release_major_upgrade = find_child(release_package, "MajorUpgrade")?;
    let legacy_major_upgrade = find_child(legacy_package, "MajorUpgrade")?;
    ensure!(
        comparable_attributes(release_major_upgrade, None)
            == comparable_attributes(legacy_major_upgrade, None),
        "the fixture should keep the release's MajorUpgrade attributes"
    );
    let release_rank_search = rank_search(&release)?;
    let legacy_rank_search = rank_search(&legacy)?;
    ensure!(
        comparable_attributes(release_rank_search, None)
            == comparable_attributes(legacy_rank_search, None),
        "the fixture should keep the release's release-rank search"
    );
    Ok(())
}

/// Return the registry search that reads the installed release rank.
fn rank_search<'a, 'input>(document: &'a Document<'input>) -> Result<Node<'a, 'input>> {
    let property = find_element(document, "Property", "Id", "PREVIOUS_NETSUKE_RELEASE_RANK")?;
    find_child(property, "RegistrySearch")
}
