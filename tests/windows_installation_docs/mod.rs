//! Shared prose views for the documented Windows install locations.
//!
//! Two documentation test binaries check the Windows instructions: one owns the
//! install-location prose and the other owns the `PATH` and help examples. Both
//! read Markdown the same way, so the text view, the paragraph splitting, and
//! the rules that qualify the legacy directory live here rather than being
//! copied into each binary.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use test_support::fs as test_fs;

/// Every README that repeats the Windows install location.
pub const READMES: [&str; 7] = [
    "README.md",
    "README.de.md",
    "README.es.md",
    "README.fr.md",
    "README.ja.md",
    "README.pt-BR.md",
    "README.zh-CN.md",
];

/// The users' guide, which carries the detailed Windows instructions.
pub const USERS_GUIDE: &str = "docs/users-guide.md";

/// The native directory the x64 and Arm64 MSIs install to.
pub const WINDOWS_INSTALL_LOCATION: &str = "`C:\\Program Files\\netsuke`";

/// The 32-bit directory the v0.1.0-beta4 MSI uses, which each document may
/// name only as a statement about that release or as the old location to
/// clean up.
pub const LEGACY_WINDOWS_INSTALL_LOCATION: &str = "C:\\Program Files (x86)\\netsuke";

/// Verbs that turn a mention of the legacy directory into an instruction to
/// keep using it.
const LEGACY_PATH_INSTRUCTIONS: [&str; 4] = ["add", "append", "prepend", "include"];

/// Wording that presents the legacy directory as the location being retired.
const LEGACY_CLEANUP_MARKERS: [&str; 3] = ["old", "no longer", "remove"];

/// Characters that end a clause in the languages the documents use.
const CLAUSE_TERMINATORS: [char; 8] = ['.', ';', '。', '；', '!', '?', '！', '？'];

/// A group of literal fragments whose absence breaks one stated promise.
pub struct RequiredProse {
    /// What the document promises, reported when a fragment is missing.
    pub expectation: &'static str,
    /// Literal text the document must contain to keep that promise.
    pub fragments: &'static [&'static str],
}

/// Text read for the prose contracts.
///
/// The contracts match literal fragments, whole paragraphs, and
/// whitespace-collapsed prose rather than raw bytes. Owning the text lets the
/// type expose exactly those views, so a caller cannot compare a collapsed
/// section with raw prose by accident.
pub struct Document {
    /// The text under contract, in whichever form the caller asked for.
    text: String,
}

impl Document {
    /// Own `text`, normalising line endings to a single newline.
    ///
    /// Paragraphs are split on a blank line, which only exists when every line
    /// break is one byte. A checkout configured with `core.autocrlf` reads the
    /// same Markdown with `\r\n`, so normalising here keeps the paragraph
    /// contracts identical on every platform.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into().replace("\r\n", "\n"),
        }
    }

    /// Read the document at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read.
    pub fn read(path: &Path) -> Result<Self> {
        let text =
            test_fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Ok(Self::new(text))
    }

    /// Borrow the text under contract.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Return the text with runs of whitespace collapsed to single spaces.
    #[must_use]
    pub fn collapsed(&self) -> Self {
        Self {
            text: self.text.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    }

    /// Return the named section, with whitespace collapsed.
    ///
    /// A section runs to the next second-level heading, so a contract cannot be
    /// satisfied by prose from a neighbouring section.
    ///
    /// # Errors
    ///
    /// Returns an error when the document does not contain `heading`.
    pub fn collapsed_section(&self, heading: &str) -> Result<Self> {
        let start = self
            .text
            .find(heading)
            .with_context(|| format!("document should have a {heading} section"))?;
        let section = self
            .text
            .get(start..)
            .with_context(|| format!("{heading} should start a character boundary"))?;
        let rest = section
            .get(heading.len()..)
            .with_context(|| format!("{heading} should end on a character boundary"))?;
        let end = rest
            .find("\n## ")
            .map_or(section.len(), |offset| heading.len() + offset);
        let body = section
            .get(..end)
            .with_context(|| format!("the {heading} section should end on a boundary"))?;
        Ok(Self {
            text: body.to_owned(),
        }
        .collapsed())
    }

    /// Fail unless every fragment in every group appears in the text.
    ///
    /// The diagnostic names both the promise the group breaks and the fragment
    /// that is missing, so a failure does not send the reader hunting.
    ///
    /// # Errors
    ///
    /// Returns an error naming the broken promise and the missing fragment.
    pub fn ensure_all(&self, groups: &[RequiredProse]) -> Result<()> {
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
    #[must_use]
    pub fn contains(&self, fragment: &str) -> bool {
        self.text.contains(fragment)
    }

    /// Return every paragraph holding a line that names `needle`.
    ///
    /// A Markdown paragraph ends at a blank line. The release attribution and
    /// the directory it explains can sit on different lines of one wrapped
    /// paragraph, so a line-scoped search would reject prose that reads
    /// correctly.
    #[must_use]
    pub fn paragraphs_naming(&self, needle: &str) -> Vec<&str> {
        self.text
            .split("\n\n")
            .filter(|paragraph| paragraph.contains(needle))
            .collect()
    }
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
///
/// # Errors
///
/// Returns an error naming the document, the missing location, or the
/// paragraph that names the legacy directory without qualifying it.
pub fn assert_every_document_names_the_windows_install_locations() -> Result<()> {
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
///
/// Attributing the directory to v0.1.0-beta4 is not enough on its own: an
/// instruction to add it to `PATH` stays wrong however it is labelled, so a
/// paragraph that both names the directory and tells the reader to add it is
/// rejected even when the release attribution is present. Cleanup wording such
/// as "remove" qualifies a mention only when it sits next to the directory, so
/// "add the 32-bit directory and remove the native one" is rejected: the
/// removal is of the replacement, not of the location being retired.
#[must_use]
pub fn qualifies_legacy_install(paragraph: &str) -> bool {
    let nearby_clauses = legacy_install_neighbourhood(paragraph);
    let is_attributed = paragraph.contains("v0.1.0-beta4")
        || nearby_clauses.iter().any(|clause| {
            let lower = clause.to_lowercase();
            LEGACY_CLEANUP_MARKERS
                .iter()
                .any(|marker| lower.contains(marker))
        });
    let instructs_use = nearby_clauses.iter().any(|clause| {
        LEGACY_PATH_INSTRUCTIONS
            .iter()
            .any(|verb| contains_word(clause, verb))
    });
    is_attributed && !instructs_use
}

/// Return every clause that names the legacy directory, and one clause to
/// either side of each.
///
/// Both rules are clause-scoped rather than paragraph-scoped. A correct
/// paragraph may still say "add the manual page" about something else, and a
/// paragraph that says "remove the native path" is talking about the
/// replacement rather than the directory being retired; only wording next to
/// the legacy directory decides whether that mention is qualified. The window
/// reaches one clause to either side because an instruction or a cleanup
/// sentence may name the directory once and then refer to it as "that
/// directory".
fn legacy_install_neighbourhood(paragraph: &str) -> Vec<&str> {
    let clauses: Vec<&str> = paragraph.split(&CLAUSE_TERMINATORS[..]).collect();
    clauses
        .iter()
        .enumerate()
        .filter(|(_, clause)| clause.contains(LEGACY_WINDOWS_INSTALL_LOCATION))
        .flat_map(|(index, _)| {
            let start = index.saturating_sub(1);
            let end = (index + 2).min(clauses.len());
            clauses.get(start..end).unwrap_or_default().to_vec()
        })
        .collect()
}

/// Whether `paragraph` contains `word` as a whole word, ignoring case.
fn contains_word(paragraph: &str, word: &str) -> bool {
    paragraph
        .split(|character: char| !character.is_alphanumeric())
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}
