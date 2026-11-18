//! Executable contracts for the Windows install locations, the beta5 MSI
//! migration advice, and the Windows PATH and help setup examples.
//!
//! Split from `documentation_installation_tests.rs` to keep both files within
//! the repository's 400-line cap; the shared fixtures live in the
//! `documentation_examples` directory module, which every documentation test
//! binary declares independently.

pub mod documentation_examples;

use std::path::Path;

use anyhow::{Context, Result, ensure};
use documentation_examples::{DocumentedExample, documented_example};
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

/// The users' guide, which carries the detailed Windows instructions.
const USERS_GUIDE: &str = "docs/users-guide.md";

/// The native directory the x64 and Arm64 MSIs install to.
const WINDOWS_INSTALL_LOCATION: &str = "`C:\\Program Files\\netsuke`";

/// The 32-bit directory the v0.1.0-beta4 MSI uses, which each document may
/// name only as a statement about that release or as the old location to
/// clean up.
const LEGACY_WINDOWS_INSTALL_LOCATION: &str = "C:\\Program Files (x86)\\netsuke";

/// A group of literal fragments whose absence breaks one stated promise.
struct RequiredProse {
    /// What the document promises, reported when a fragment is missing.
    expectation: &'static str,
    /// Literal text the document must contain to keep that promise.
    fragments: &'static [&'static str],
}

/// Text read for the prose contracts below.
///
/// The contracts match literal fragments, whole paragraphs, and
/// whitespace-collapsed prose rather than raw bytes. Owning the text lets the
/// type expose exactly those views, so a caller cannot compare a collapsed
/// section with raw prose by accident.
struct Document {
    /// The text under contract, in whichever form the caller asked for.
    text: String,
}

impl Document {
    /// Read the document at `path`.
    fn read(path: &Path) -> Result<Self> {
        let text =
            test_fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Ok(Self { text })
    }

    /// Treat a documented example's body as the text under contract.
    fn from_example(example: &DocumentedExample) -> Self {
        Self {
            text: example.body.clone(),
        }
    }

    /// Return the text with runs of whitespace collapsed to single spaces.
    fn collapsed(&self) -> Self {
        Self {
            text: self.text.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    }

    /// Return the named section, with whitespace collapsed.
    ///
    /// A section runs to the next second-level heading, so a contract cannot be
    /// satisfied by prose from a neighbouring section.
    fn collapsed_section(&self, heading: &str) -> Result<Self> {
        let start = self
            .text
            .find(heading)
            .with_context(|| format!("document should have a {heading} section"))?;
        let section = &self.text[start..];
        let end = section[heading.len()..]
            .find("\n## ")
            .map_or(section.len(), |offset| heading.len() + offset);
        Ok(Self {
            text: section[..end].to_owned(),
        }
        .collapsed())
    }

    /// Fail unless every fragment in every group appears in the text.
    ///
    /// The diagnostic names both the promise the group breaks and the fragment
    /// that is missing, so a failure does not send the reader hunting.
    fn ensure_all(&self, groups: &[RequiredProse]) -> Result<()> {
        for group in groups {
            for fragment in group.fragments {
                ensure!(
                    self.text.contains(fragment),
                    "{}: expected the document to state {fragment:?}",
                    group.expectation
                );
            }
        }
        Ok(())
    }

    /// Report whether the text contains `fragment`.
    fn contains(&self, fragment: &str) -> bool {
        self.text.contains(fragment)
    }

    /// Return every paragraph holding a line that names `needle`.
    ///
    /// A Markdown paragraph ends at a blank line. The release attribution and
    /// the directory it explains can sit on different lines of one wrapped
    /// paragraph, so a line-scoped search would reject prose that reads
    /// correctly.
    fn paragraphs_naming(&self, needle: &str) -> Vec<&str> {
        self.text
            .split("\n\n")
            .filter(|paragraph| paragraph.contains(needle))
            .collect()
    }
}

/// Verify the beta5 migration note covers the move and its manual steps.
#[test]
fn beta5_migration_note_covers_the_windows_msi_move() -> Result<()> {
    // The note is the only migration advice a beta4 Windows user receives, so
    // each manual step has to be named: the directory change, the PATH cleanup
    // the MSI cannot perform, and the beta2/beta3 uninstall that the upgrade
    // identity does not cover.
    let guide = Document::read(Path::new("docs/v0-1-0-migration-guide.md"))?.collapsed();
    guide.ensure_all(&[RequiredProse {
        expectation: "the beta5 migration note should cover the move and its manual steps",
        fragments: &[
            "## Install Windows MSIs under the native program files directory",
            "Windows MSI installations",
            "ProgramFiles6432Folder",
            "Installing beta5 over a beta4 installation",
            LEGACY_WINDOWS_INSTALL_LOCATION,
            "C:\\Program Files\\netsuke",
            "The MSI does not edit `PATH`",
            "uninstall beta2 and beta3",
            "Installed apps",
        ],
    }])
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
    let users_guide = Document::read(Path::new(USERS_GUIDE))?.collapsed();
    // Replacement is automatic only within one upgrade identity. Without this
    // qualification the paragraph contradicts the beta2/beta3 advice below it.
    users_guide.ensure_all(&[
        RequiredProse {
            expectation: "users' guide should document MSI replacement within a matching \
                          upgrade identity",
            fragments: &[
                "installing a later beta or final MSI",
                "replaces the existing installation",
                "matching upgrade identity",
            ],
        },
        RequiredProse {
            expectation: "users' guide should explain the move out of the legacy install \
                          directory",
            fragments: &[
                LEGACY_WINDOWS_INSTALL_LOCATION,
                "The MSI does not edit `PATH`",
                "disappears only when nothing else remains",
                "uninstall those versions from Installed apps",
            ],
        },
    ])
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
    for path in READMES.into_iter().chain([USERS_GUIDE]) {
        let document = Document::read(Path::new(path))?;
        ensure!(
            document.contains(WINDOWS_INSTALL_LOCATION),
            "{path} should state the native Windows MSI install location {WINDOWS_INSTALL_LOCATION}"
        );
        let legacy_paragraphs = document.paragraphs_naming(LEGACY_WINDOWS_INSTALL_LOCATION);
        ensure!(
            !legacy_paragraphs.is_empty(),
            "{path} should state the v0.1.0-beta4 Windows MSI install location \
             {LEGACY_WINDOWS_INSTALL_LOCATION}"
        );
        for paragraph in legacy_paragraphs {
            ensure!(
                qualifies_legacy_install(paragraph),
                "{path} mentions {LEGACY_WINDOWS_INSTALL_LOCATION} without attributing it \
                 to v0.1.0-beta4 or naming it as the old location to clean up: {paragraph}"
            );
        }
    }
    Ok(())
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

/// Assert the documented PATH setup for an MSI that uses the native directory.
fn assert_native_windows_path_example() -> Result<()> {
    let example = documented_example("guide-windows-path")?;
    let windows_path = Document::from_example(&example);
    windows_path.ensure_all(&[RequiredProse {
        expectation: "Windows PATH setup should resolve the native MSI directory and persist it",
        fragments: &[
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
        ],
    }])?;
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
    let beta4_path = Document::from_example(&example);
    beta4_path.ensure_all(&[RequiredProse {
        expectation: "the beta4 PATH setup should resolve the 32-bit MSI directory and persist it",
        fragments: &[
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
        ],
    }])?;
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
/// structurally because the Linux CI legs have no PowerShell and no Windows
/// user environment, so executing the fence is not portable. A structural
/// check still rejects deleting the `throw`, which an ordering comparison alone
/// would accept.
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
    let guard = &body[existence_check..path_persistence];
    ensure!(
        guard.contains("throw ") || guard.contains("return"),
        "{name} must exit early when netsuke.exe is absent, before changing PATH"
    );
    Ok(())
}

/// Assert the documented Windows help setup and release staging contracts.
fn assert_windows_help_examples() -> Result<()> {
    let windows = Document::from_example(&documented_example("guide-windows-help")?);
    ensure!(windows.text == "Get-Help Netsuke -Full\n", "help drifted");
    let windows_help_install =
        Document::from_example(&documented_example("guide-windows-help-install")?);
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
