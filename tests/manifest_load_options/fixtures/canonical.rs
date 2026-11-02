//! Reducing a load outcome to the text a snapshot records.
//!
//! A characterization snapshot has to compare two things that are not the same
//! type: a successful load, which is a populated manifest, and a failed one,
//! which is an error chain. Both are reduced to a string so that a row
//! snapshots one shape whatever happens.
//!
//! Two decisions make the strings repeatable while staying reviewable:
//!
//! - Serialization goes through `serde_json_canonicalizer`, which fixes key
//!   order and number form. Two runs over equal values produce equal text even
//!   where the YAML mapping order differed, so a snapshot cannot fail for a
//!   reason unrelated to the loader.
//! - The canonical text is re-serialized with an indent before it is recorded.
//!   Canonicalization is what makes the comparison sound; the indent is what
//!   makes a reviewer able to read the baseline. Key order survives the round
//!   trip because `serde_json` preserves insertion order and the canonicalizer
//!   emitted the keys already sorted.
//!
//! The error path keeps `anyhow`'s alternate separator, which walks the chain
//! from the outermost context to the root cause. That is the part worth
//! pinning: a refactor that starts wrapping or dropping a context changes the
//! text, and that is exactly the kind of change the baseline exists to catch.
//!
//! Each block also leads with the stages the entry point's observer was
//! notified with. A seam a caller can pass is a seam a caller can observe, so
//! the notified sequence is part of what a row freezes.

use netsuke::ast::NetsukeManifest;
use netsuke::manifest::ManifestLoadStage;

/// Record which stage-observer seam one row's entry point offers.
///
/// The distinction is carried in the type rather than inferred from an empty
/// observation, because those two states mean different things: an entry point
/// that takes no observer cannot notify a stage however it is refactored, while
/// one that takes an observer and notifies nothing has lost behaviour it used
/// to have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageSeam {
    /// The entry point takes no observer parameter.
    Absent,
    /// The entry point takes an observer, which the row supplies.
    Accepted,
}

/// Render a load outcome as the text a snapshot records.
pub fn canonical(result: &anyhow::Result<NetsukeManifest>) -> String {
    match result {
        Ok(manifest) => canonical_manifest(manifest),
        Err(error) => format!("{error:#}"),
    }
}

/// Render one labelled block per fixture, in the order the cases were given.
///
/// A row snapshots one document rather than one file per fixture. Both record
/// the same results; the single document keeps an entry point's whole
/// characterization in one place, so a reviewer reads one file to see whether a
/// refactor changed *anything* about that entry, and the snapshot directory
/// stays small enough to hold in view.
///
/// `stages` is what the row's observer was notified with, and is empty for a
/// row whose entry point has no observer to notify.
pub fn report(
    entries: &[(String, anyhow::Result<NetsukeManifest>)],
    seam: StageSeam,
    stages: &[ManifestLoadStage],
) -> String {
    let observed = observer_line(seam, stages);
    entries
        .iter()
        .map(|(label, result)| format!("=== {label} ===\n{observed}{}", canonical(result)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render the stage-observer line that opens a fixture's block.
///
/// The three states are spelled out rather than collapsed into one, because a
/// missing observer and a silent one would otherwise read the same in a diff.
fn observer_line(seam: StageSeam, stages: &[ManifestLoadStage]) -> String {
    match (seam, stages) {
        (StageSeam::Absent, _) => "observer: none accepted by this entry point\n".to_owned(),
        (StageSeam::Accepted, []) => "observer: accepted, notified no stage\n".to_owned(),
        (StageSeam::Accepted, notified) => {
            let names = notified
                .iter()
                .map(|stage| format!("{stage:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("observer: {names}\n")
        }
    }
}

/// Render a manifest as indented canonical JSON.
fn canonical_manifest(manifest: &NetsukeManifest) -> String {
    let canonical = match serde_json_canonicalizer::to_string(manifest) {
        Ok(text) => text,
        Err(error) => return format!("<manifest not serializable: {error}>"),
    };
    match serde_json::from_str::<serde_json::Value>(&canonical) {
        Ok(value) => serde_json::to_string_pretty(&value)
            .unwrap_or_else(|error| format!("<manifest not re-indentable: {error}>")),
        Err(error) => format!("<canonical text not readable: {error}>"),
    }
}
