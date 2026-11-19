//! Executable contracts for the Windows install locations and the beta5 MSI
//! migration advice.
//!
//! The Windows `PATH` and help examples live in the sibling
//! `documentation_windows_path_setup_tests.rs`, and the shared Markdown views
//! live in the `windows_installation_docs` directory module, so each file
//! stays inside the repository's 400-line cap.

pub mod windows_installation_docs;

use std::path::Path;

use anyhow::Result;
use windows_installation_docs::{
    Document, LEGACY_WINDOWS_INSTALL_LOCATION, Paragraph, RequiredProse, USERS_GUIDE,
    assert_every_document_names_the_windows_install_locations,
};

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

/// Verify that a correct paragraph naming the legacy directory is accepted.
#[test]
fn attributed_legacy_mention_is_accepted() {
    let paragraph = "The Linux packages add the `netsuke.1` manual page. The \
                     v0.1.0-beta4 MSI installs to \
                     `C:\\Program Files (x86)\\netsuke`.";
    assert!(
        Paragraph::new(paragraph).qualifies_legacy_install(),
        "an attributed mention without a PATH instruction must be accepted: {paragraph}"
    );
}

/// Verify that an instruction to add the legacy directory is rejected even
/// when the paragraph names the release.
#[test]
fn legacy_path_instruction_is_rejected_despite_attribution() {
    // A reader who is told to add the 32-bit directory keeps a `PATH` entry the
    // MSI no longer populates, whichever release the sentence credits.
    let paragraph = "The v0.1.0-beta4 MSI installs to \
                     `C:\\Program Files (x86)\\netsuke`; add that directory to `PATH`.";
    assert!(
        !Paragraph::new(paragraph).qualifies_legacy_install(),
        "a paragraph that adds the legacy directory to PATH must be rejected: {paragraph}"
    );
}

/// Verify that the clause-scoped check still sees an instruction split off
/// from the directory by a sentence boundary.
#[test]
fn legacy_path_instruction_spanning_a_sentence_boundary_is_rejected() {
    let paragraph = "The v0.1.0-beta4 MSI installs to \
                     `C:\\Program Files (x86)\\netsuke`. Add it to `PATH`.";
    assert!(
        !Paragraph::new(paragraph).qualifies_legacy_install(),
        "an instruction in the next sentence must still be rejected: {paragraph}"
    );
}

/// Verify that cleanup wording away from the legacy directory does not
/// qualify a mention of it.
#[test]
fn cleanup_wording_away_from_the_legacy_path_does_not_qualify_it() {
    // The removal is of the replacement directory, and the legacy directory is
    // only named as where an older release installed. Crediting the cleanup to
    // the legacy mention would accept a paragraph that sends the reader back to
    // the directory the MSI no longer populates.
    let paragraph = "Remove `C:\\Program Files\\netsuke` from `PATH`. \
                     That directory replaced the beta2 location. \
                     The beta2 MSI installed to \
                     `C:\\Program Files (x86)\\netsuke`.";
    assert!(
        !Paragraph::new(paragraph).qualifies_legacy_install(),
        "cleanup of the native directory must not qualify the legacy one: {paragraph}"
    );
}

/// Verify that cleanup wording next to the legacy directory qualifies it.
#[test]
fn cleanup_wording_beside_the_legacy_path_qualifies_it() {
    let paragraph = "Remove the stale `C:\\Program Files (x86)\\netsuke` \
                     directory after upgrading.";
    assert!(
        Paragraph::new(paragraph).qualifies_legacy_install(),
        "cleanup of the legacy directory must qualify it: {paragraph}"
    );
}

/// Verify that a CRLF checkout still yields whole paragraphs.
#[test]
fn crlf_line_endings_keep_paragraphs_intact() {
    let document = Document::new(
        "Intro paragraph.\r\n\r\nThe v0.1.0-beta4 MSI installs to \
         `C:\\Program Files (x86)\\netsuke`\r\nand a later MSI moves it.\r\n",
    );
    let paragraphs = document.paragraphs_naming(LEGACY_WINDOWS_INSTALL_LOCATION);
    assert_eq!(
        paragraphs.len(),
        1,
        "CRLF text should split into whole paragraphs: {paragraphs:?}"
    );
    assert!(
        paragraphs
            .first()
            .is_some_and(|paragraph| { paragraph.as_str().contains("and a later MSI moves it.") }),
        "the paragraph should keep its wrapped continuation: {paragraphs:?}"
    );
}
