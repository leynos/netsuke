//! Executable contracts for the documented Windows `PATH` and help setup.
//!
//! The install-location prose lives in the sibling
//! `documentation_windows_installation_tests.rs`, and the shared Markdown views
//! live in the `windows_installation_docs` directory module. The examples are
//! checked as source rather than executed: the Linux CI legs have no PowerShell
//! and no Windows user environment, so the fences are pinned by fragment and by
//! the structure that makes them safe.

pub mod documentation_examples;
pub mod windows_installation_docs;

use std::path::Path;

use anyhow::{Context, Result, ensure};
use documentation_examples::{DocumentedExample, documented_example};
use windows_installation_docs::{Document, RequiredProse, USERS_GUIDE};

/// Fragments every documented Windows PATH setup shares.
///
/// The two scripts differ only in which program files directory they resolve,
/// so the persistence contract is stated once: stop when the payload is
/// missing, open the user environment key for writing, read the raw value and
/// its type, write the value back with that type, and release the handle. The
/// provider key returned by `Get-Item` is read-only, so appending through it
/// throws instead of saving the directory.
const PATH_PERSISTENCE_FRAGMENTS: [&str; 7] = [
    "if (-not (Test-Path -LiteralPath $netsukeExecutable -PathType Leaf))",
    "$environmentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)",
    "if ($null -eq $environmentKey)",
    "GetValue('Path', '', 'DoNotExpandEnvironmentNames')",
    "GetValueKind('Path')",
    "$environmentKey.SetValue('Path', $newUserPath, $userPathKind)",
    "$environmentKey.Dispose()",
];

/// Verify the documented Windows PATH and help setup contracts.
#[test]
fn windows_setup_examples_match_the_documented_install_locations() -> Result<()> {
    assert_native_windows_path_example()?;
    assert_legacy_windows_path_example()?;
    assert_windows_help_examples()
}

/// Assert the documented PATH setup for an MSI that uses the native directory.
fn assert_native_windows_path_example() -> Result<()> {
    let example = documented_example("guide-windows-path")?;
    let windows_path = Document::new(example.body.clone());
    windows_path.ensure_all(&[
        RequiredProse {
            expectation: "Windows PATH setup should persist the directory in the user PATH",
            fragments: &PATH_PERSISTENCE_FRAGMENTS,
        },
        RequiredProse {
            expectation: "Windows PATH setup should resolve the native MSI directory",
            fragments: &[
                "$programFiles = if ($env:ProgramW6432) { $env:ProgramW6432 } else { $env:ProgramFiles }",
                "$netsukeDirectory = Join-Path $programFiles 'netsuke'",
                "$netsukeExecutable = Join-Path $netsukeDirectory 'netsuke.exe'",
                "Get-Command netsuke -CommandType Application",
                "$resolved.Source -ne $netsukeExecutable",
            ],
        },
    ])?;
    // The x64 and Arm64 packages install into the native directory. A script
    // that read the 32-bit directory would add a path the MSI never populates,
    // and `netsuke --version` would then fail for a correctly installed user.
    let legacy_fragments = ["ProgramFilesX86", "Program Files (x86)"];
    ensure!(
        !legacy_fragments
            .iter()
            .any(|fragment| windows_path.contains(fragment)),
        "Windows PATH setup must not resolve the 32-bit program files directory"
    );
    assert_path_setup_checks_before_persisting(&example)
}

/// Assert the beta4 PATH setup that resolves the 32-bit program files directory.
fn assert_legacy_windows_path_example() -> Result<()> {
    // The linked beta4 MSI installs to the 32-bit directory, so the guide needs
    // a second example: the native script would refuse a correct beta4 install.
    let example = documented_example("guide-windows-path-beta4")?;
    let beta4_path = Document::new(example.body.clone());
    beta4_path.ensure_all(&[
        RequiredProse {
            expectation: "the beta4 PATH setup should persist the directory in the user PATH",
            fragments: &PATH_PERSISTENCE_FRAGMENTS,
        },
        RequiredProse {
            expectation: "the beta4 PATH setup should resolve the 32-bit MSI directory",
            fragments: &[
                "ProgramFiles(x86)",
                "GetFolderPath('ProgramFilesX86')",
                "$netsukeDirectory = Join-Path $legacyProgramFiles 'netsuke'",
                "$netsukeExecutable = Join-Path $netsukeDirectory 'netsuke.exe'",
                "Get-Command netsuke -CommandType Application",
                "$resolved.Source -ne $netsukeExecutable",
            ],
        },
    ])?;
    ensure!(
        !beta4_path.contains("ProgramW6432"),
        "the beta4 PATH setup must not resolve the native directory"
    );
    assert_path_setup_checks_before_persisting(&example)?;
    let setup =
        Document::read(Path::new(USERS_GUIDE))?.collapsed_section("### Complete Windows setup")?;
    setup.ensure_all(&[RequiredProse {
        expectation: "the users' guide should distinguish beta4 PATH setup from beta5 and later",
        fragments: &[
            "The v0.1.0-beta4 MSI",
            "32-bit program files directory",
            "The beta5 and later MSIs",
            "`C:\\Program Files\\netsuke`",
            "disappears only when nothing else remains",
            "Remove the beta4 `PATH` entry by hand",
        ],
    }])
}

/// Assert that a PATH setup fence tests for the executable before changing
/// PATH and exits early when it is absent.
///
/// Checking first keeps a missing MSI an immediate, legible failure rather than
/// a stale entry discovered later. The early-exit guard is asserted
/// structurally rather than by running the fence, because the Linux CI legs
/// have no PowerShell and no Windows user environment. A structural check still
/// rejects deleting the `throw`, which an ordering comparison alone would
/// accept.
fn assert_path_setup_checks_before_persisting(example: &DocumentedExample) -> Result<()> {
    let body = example.body.as_str();
    let name = example.id.as_str();
    let existence_check = body
        .find("Test-Path -LiteralPath $netsukeExecutable")
        .with_context(|| format!("{name} must test for netsuke.exe"))?;
    let path_persistence = body
        .find("SetValue('Path'")
        .with_context(|| format!("{name} must persist the directory"))?;
    ensure!(
        existence_check < path_persistence,
        "{name} must verify netsuke.exe before changing PATH"
    );
    // The window starts at the beginning of the line holding the check, so it
    // includes the `if (-not (` that decides whether the check stops the
    // script rather than beginning at the call it guards.
    let guard_start = body
        .get(..existence_check)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |offset| offset + 1);
    let guard = body
        .get(guard_start..path_persistence)
        .with_context(|| format!("{name} must check for netsuke.exe before changing PATH"))?;
    ensure!(
        guard.contains("throw ") || guard.contains("return"),
        "{name} must exit early when netsuke.exe is absent, before changing PATH"
    );
    // Dropping the negation inverts the guard: the script would append the
    // directory exactly when the executable is missing, and every string
    // fragment above would still match.
    ensure!(
        guard.contains("if (-not (Test-Path -LiteralPath $netsukeExecutable"),
        "{name} must negate the existence check so a missing netsuke.exe stops it"
    );
    Ok(())
}

/// Assert the documented Windows help setup and release staging contracts.
fn assert_windows_help_examples() -> Result<()> {
    let windows = Document::new(documented_example("guide-windows-help")?.body);
    ensure!(
        windows.as_str() == "Get-Help Netsuke -Full\n",
        "help drifted: {}",
        windows.as_str()
    );
    let windows_help_install =
        Document::new(documented_example("guide-windows-help-install")?.body);
    // One release selection drives both the release tag and the installed
    // module directory, so a beta5 reader cannot download beta4 help into a
    // directory named after a different version. The default tracks the crate
    // version, which is the release this guide documents.
    let expected_release_selection = format!("$releaseVersion = '{}'", env!("CARGO_PKG_VERSION"));
    windows_help_install.ensure_all(&[RequiredProse {
        expectation: "Windows help setup should import the downloaded sidecars",
        fragments: &[
            "Import-Module",
            "tags/v$releaseVersion",
            "$moduleDirectory = Join-Path $moduleRoot \"Netsuke\\$releaseVersion\"",
            "Import-Module (Join-Path $moduleDirectory 'Netsuke.psd1')",
            "*windows-$architecture*",
        ],
    }])?;
    ensure!(
        windows_help_install.contains(&expected_release_selection),
        "Windows help setup should default to the crate version: {expected_release_selection}"
    );
    ensure!(
        !windows_help_install.contains("/tags/v0.1.0-beta4"),
        "Windows help setup must derive the release tag from its release selection"
    );
    Document::read(Path::new(".github/release-staging.toml"))?.ensure_all(&[RequiredProse {
        expectation: "Windows release should stage the help consumed by Get-Help",
        fragments: &["Netsuke-help.xml", "about_Netsuke.help.txt"],
    }])
}
