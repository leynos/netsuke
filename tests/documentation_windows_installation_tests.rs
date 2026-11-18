//! Executable contracts for the Windows install locations, the beta5 MSI
//! migration advice, and the Windows PATH and help setup examples.
//!
//! Split from `documentation_installation_tests.rs` to keep both files within
//! the repository's 400-line cap; the shared fixtures live in the
//! `documentation_examples` directory module, which every documentation test
//! binary declares independently.

pub mod documentation_examples;

use anyhow::{Context, Result, ensure};
use documentation_examples::documented_example;
use test_support::fs as test_fs;

/// Every README that repeats the Windows install location.
const READMES: [&str; 7] = [
    "README.md",
    "README.de.md",
    "README.es.md",
    "README.fr.md",
    "README.ja.md",
    "README.pt-BR.md",
    "README.zh-CN.md",
];

/// The native directory the x64 and Arm64 MSIs install to.
const WINDOWS_INSTALL_LOCATION: &str = "`C:\\Program Files\\netsuke`";

/// The 32-bit directory the v0.1.0-beta4 MSI uses, which each document may
/// name only as a statement about that release or as the old location to
/// clean up.
const LEGACY_WINDOWS_INSTALL_LOCATION: &str = "C:\\Program Files (x86)\\netsuke";

/// Verify the beta5 migration note covers the move and its manual steps.
#[test]
fn beta5_migration_note_covers_the_windows_msi_move() -> Result<()> {
    // The note is the only migration advice a beta4 Windows user receives, so
    // each manual step has to be named: the directory change, the PATH cleanup
    // the MSI cannot perform, and the beta2/beta3 uninstall that the upgrade
    // identity does not cover.
    let guide = collapse_whitespace(
        &test_fs::read_to_string("docs/v0-1-0-migration-guide.md")
            .context("read docs/v0-1-0-migration-guide.md")?,
    );
    let required_fragments = [
        "## Install Windows MSIs under the native program files directory",
        "Windows MSI installations",
        "ProgramFiles6432Folder",
        "Installing beta5 over a beta4 installation",
        LEGACY_WINDOWS_INSTALL_LOCATION,
        "C:\\Program Files\\netsuke",
        "The MSI does not edit `PATH`",
        "uninstall beta2 and beta3",
        "Installed apps",
    ];
    for fragment in required_fragments {
        ensure!(
            guide.contains(fragment),
            "the beta5 migration note should state: {fragment}"
        );
    }
    Ok(())
}

/// Verify that every README and the guide qualify both Windows install
/// locations.
#[test]
fn every_document_attributes_the_windows_install_locations() -> Result<()> {
    assert_every_document_names_the_windows_install_locations()
}

/// Verify that the users' guide qualifies MSI replacement and still tells a
/// beta2 or beta3 user to uninstall by hand.
#[test]
fn users_guide_distinguishes_msi_replacement_from_manual_uninstall() -> Result<()> {
    let users_guide = collapse_whitespace(
        &test_fs::read_to_string("docs/users-guide.md").context("read docs/users-guide.md")?,
    );
    // Replacement is automatic only within one upgrade identity. Without this
    // qualification the paragraph contradicts the beta2/beta3 advice below it.
    let expected_msi_replacement_fragments = [
        "installing a later beta or final MSI",
        "replaces the existing installation",
        "matching upgrade identity",
    ];
    ensure!(
        expected_msi_replacement_fragments
            .into_iter()
            .all(|fragment| users_guide.contains(fragment)),
        "users' guide should document MSI replacement within a matching upgrade identity"
    );
    let expected_upgrade_fragments = [
        LEGACY_WINDOWS_INSTALL_LOCATION,
        "The MSI does not edit `PATH`",
        "disappears only when nothing else remains",
        "uninstall those versions from Installed apps",
    ];
    ensure!(
        expected_upgrade_fragments
            .into_iter()
            .all(|fragment| users_guide.contains(fragment)),
        "users' guide should explain the move out of the legacy install directory"
    );
    Ok(())
}

/// Verify the documented Windows PATH and help setup contracts.
#[test]
fn windows_setup_examples_match_the_documented_install_locations() -> Result<()> {
    assert_windows_setup_examples()
}

/// Check that every README and the guide name both Windows install locations
/// and qualify each mention of the legacy 32-bit directory.
///
/// The linked package and the next release install to different directories,
/// so a document that names only one of them tells half the story: the native
/// path is a statement about MSIs built from the release authoring, and the
/// 32-bit path is a statement about the v0.1.0-beta4 MSI. Every paragraph
/// naming the 32-bit path must either attribute it to that release or present
/// it as the old location being cleaned up; checking only the first paragraph
/// would let a later one send beta5 users back to the 32-bit directory.
fn assert_every_document_names_the_windows_install_locations() -> Result<()> {
    for path in READMES.into_iter().chain(["docs/users-guide.md"]) {
        let document = test_fs::read_to_string(path).with_context(|| format!("read {path}"))?;
        ensure!(
            document.contains(WINDOWS_INSTALL_LOCATION),
            "{path} should state the native Windows MSI install location {WINDOWS_INSTALL_LOCATION}"
        );
        let legacy_paragraphs = paragraphs_naming(&document, LEGACY_WINDOWS_INSTALL_LOCATION);
        ensure!(
            !legacy_paragraphs.is_empty(),
            "{path} should state the v0.1.0-beta4 Windows MSI install location \
             {LEGACY_WINDOWS_INSTALL_LOCATION}"
        );
        for paragraph in legacy_paragraphs {
            ensure!(
                qualifies_legacy_install(&paragraph),
                "{path} mentions {LEGACY_WINDOWS_INSTALL_LOCATION} without attributing it \
                 to v0.1.0-beta4 or naming it as the old location to clean up: {paragraph}"
            );
        }
    }
    Ok(())
}

/// Return every Markdown paragraph holding a line that names `needle`.
///
/// The release attribution and the directory it explains can sit on different
/// lines of one wrapped paragraph, so a line-scoped search would reject prose
/// that reads correctly.
fn paragraphs_naming(document: &str, needle: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for line in document.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut paragraphs, &mut current, needle);
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    push_paragraph(&mut paragraphs, &mut current, needle);
    paragraphs
}

/// Push the accumulated paragraph when it names `needle`, then clear it.
fn push_paragraph(paragraphs: &mut Vec<String>, current: &mut String, needle: &str) {
    if current.is_empty() {
        return;
    }
    if current.contains(needle) {
        paragraphs.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

/// Whether a paragraph correctly qualifies the legacy 32-bit install location.
fn qualifies_legacy_install(paragraph: &str) -> bool {
    let lower = paragraph.to_lowercase();
    paragraph.contains("v0.1.0-beta4")
        || ["old", "no longer", "remove"]
            .iter()
            .any(|marker| lower.contains(marker))
}

/// Check the documented Windows help, PATH, and staging contracts.
fn assert_windows_setup_examples() -> Result<()> {
    assert_native_windows_path_example()?;
    assert_legacy_windows_path_example()?;
    assert_windows_help_examples()
}

/// Assert that `body` contains every fragment in `fragments`, naming
/// `expectation` when one is missing.
fn ensure_contains_all(body: &str, fragments: &[&str], expectation: &str) -> Result<()> {
    ensure!(
        fragments.iter().all(|fragment| body.contains(fragment)),
        "{expectation}"
    );
    Ok(())
}

/// Collapse runs of whitespace so prose matches ignore line wrapping.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Return the Complete Windows setup section with all whitespace collapsed.
///
/// Matching the collapsed section asserts the words a reader sees rather than
/// whichever line wrapping the formatter happens to choose.
fn collapsed_windows_setup_section(document: &str) -> Result<String> {
    let heading = "### Complete Windows setup";
    let start = document
        .find(heading)
        .context("users' guide should have a Complete Windows setup section")?;
    let section = &document[start..];
    let end = section[heading.len()..]
        .find("\n## ")
        .map_or(section.len(), |offset| heading.len() + offset);
    Ok(collapse_whitespace(&section[..end]))
}

/// Assert the documented PATH setup for an MSI that uses the native directory.
fn assert_native_windows_path_example() -> Result<()> {
    let windows_path = documented_example("guide-windows-path")?;
    let windows_path_fragments = [
        "$programFiles = if ($env:ProgramW6432) { $env:ProgramW6432 } else { $env:ProgramFiles }",
        "$netsukeDirectory = Join-Path $programFiles 'netsuke'",
        "$netsukeExecutable = Join-Path $netsukeDirectory 'netsuke.exe'",
        "Test-Path -LiteralPath $netsukeExecutable -PathType Leaf",
        "$netsukeDirectory",
        "$environmentKey = Get-Item -LiteralPath 'HKCU:\\Environment'",
        "GetValue('Path', '', 'DoNotExpandEnvironmentNames')",
        "GetValueKind('Path')",
        "$environmentKey.SetValue('Path', $newUserPath, $userPathKind)",
        "Get-Command netsuke -CommandType Application",
        "$resolved.Source -ne $netsukeExecutable",
    ];
    ensure_contains_all(
        &windows_path.body,
        &windows_path_fragments,
        "Windows PATH setup should resolve the native MSI directory and persist it",
    )?;
    // The x64 and Arm64 packages install into the native directory. A script
    // that read the 32-bit directory would add a path the MSI never populates,
    // and `netsuke --version` would then fail for a correctly installed user.
    let legacy_fragments = ["ProgramFilesX86", "Program Files (x86)"];
    ensure!(
        !legacy_fragments
            .iter()
            .any(|fragment| windows_path.body.contains(fragment)),
        "Windows PATH setup must not resolve the 32-bit program files directory"
    );
    assert_path_setup_checks_before_persisting(&windows_path.body, "the Windows PATH setup")
}

/// Assert the beta4 PATH setup that resolves the 32-bit program files directory.
fn assert_legacy_windows_path_example() -> Result<()> {
    // The linked beta4 MSI installs to the 32-bit directory, so the guide needs
    // a second example: the native script would refuse a correct beta4 install.
    let beta4_path = documented_example("guide-windows-path-beta4")?;
    let beta4_path_fragments = [
        "ProgramFiles(x86)",
        "GetFolderPath('ProgramFilesX86')",
        "$netsukeDirectory = Join-Path $legacyProgramFiles 'netsuke'",
        "$netsukeExecutable = Join-Path $netsukeDirectory 'netsuke.exe'",
        "Test-Path -LiteralPath $netsukeExecutable -PathType Leaf",
        "$environmentKey = Get-Item -LiteralPath 'HKCU:\\Environment'",
        "GetValue('Path', '', 'DoNotExpandEnvironmentNames')",
        "GetValueKind('Path')",
        "$environmentKey.SetValue('Path', $newUserPath, $userPathKind)",
        "Get-Command netsuke -CommandType Application",
        "$resolved.Source -ne $netsukeExecutable",
    ];
    ensure_contains_all(
        &beta4_path.body,
        &beta4_path_fragments,
        "the beta4 PATH setup should resolve the 32-bit MSI directory and persist it",
    )?;
    ensure!(
        !beta4_path.body.contains("ProgramW6432"),
        "the beta4 PATH setup must not resolve the native directory"
    );
    assert_path_setup_checks_before_persisting(&beta4_path.body, "the beta4 PATH setup")?;
    let users_guide =
        test_fs::read_to_string("docs/users-guide.md").context("read docs/users-guide.md")?;
    let setup = collapsed_windows_setup_section(&users_guide)?;
    let setup_prose_fragments = [
        "The v0.1.0-beta4 MSI",
        "32-bit program files directory",
        "The beta5 and later MSIs",
        "`C:\\Program Files\\netsuke`",
        "disappears only when nothing else remains",
        "Remove the beta4 `PATH` entry by hand",
    ];
    ensure_contains_all(
        &setup,
        &setup_prose_fragments,
        "the users' guide should distinguish beta4 PATH setup from beta5 and later",
    )
}

/// Assert that a PATH setup fence tests for the executable before changing
/// PATH and exits early when it is absent.
///
/// Checking first keeps a missing MSI an immediate, legible failure rather than
/// a stale entry discovered later. The early-exit guard is asserted
/// structurally because the Linux CI legs have no PowerShell and no Windows
/// user environment, so executing the fence is not portable. A structural
/// check still rejects deleting the `throw`, which an ordering comparison alone
/// would accept.
fn assert_path_setup_checks_before_persisting(body: &str, example: &str) -> Result<()> {
    let existence_check = body
        .find("Test-Path -LiteralPath $netsukeExecutable")
        .with_context(|| format!("{example} must test for netsuke.exe"))?;
    let path_persistence = body
        .find("SetValue('Path'")
        .with_context(|| format!("{example} must persist the directory"))?;
    ensure!(
        existence_check < path_persistence,
        "{example} must verify netsuke.exe before changing PATH"
    );
    let guard = &body[existence_check..path_persistence];
    ensure!(
        guard.contains("throw ") || guard.contains("return"),
        "{example} must exit early when netsuke.exe is absent, before changing PATH"
    );
    Ok(())
}

/// Assert the documented Windows help setup and release staging contracts.
fn assert_windows_help_examples() -> Result<()> {
    let windows = documented_example("guide-windows-help")?;
    ensure!(windows.body == "Get-Help Netsuke -Full\n", "help drifted");
    let windows_help_install = documented_example("guide-windows-help-install")?;
    // One release selection drives both the release tag and the installed
    // module directory, so a beta5 reader cannot download beta4 help into a
    // directory named after a different version. The default tracks the crate
    // version, which is the release this guide documents.
    let expected_release_selection = format!("$releaseVersion = '{}'", env!("CARGO_PKG_VERSION"));
    let windows_help_fragments = [
        "Import-Module",
        "tags/v$releaseVersion",
        "$moduleDirectory = Join-Path $moduleRoot \"Netsuke\\$releaseVersion\"",
        "Import-Module (Join-Path $moduleDirectory 'Netsuke.psd1')",
        "*windows-$architecture*",
    ];
    ensure_contains_all(
        &windows_help_install.body,
        &windows_help_fragments,
        "Windows help setup should import the downloaded sidecars",
    )?;
    ensure!(
        windows_help_install
            .body
            .contains(&expected_release_selection),
        "Windows help setup should default to the crate version: {expected_release_selection}"
    );
    ensure!(
        !windows_help_install.body.contains("/tags/v0.1.0-beta4"),
        "Windows help setup must derive the release tag from its release selection"
    );
    let staging = test_fs::read_to_string(".github/release-staging.toml")
        .context("read release staging configuration")?;
    ensure!(
        staging.contains("Netsuke-help.xml") && staging.contains("about_Netsuke.help.txt"),
        "Windows release should stage the help consumed by Get-Help"
    );
    Ok(())
}
