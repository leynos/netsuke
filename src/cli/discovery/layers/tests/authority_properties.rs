//! Property tests for definition-authority classification.
//!
//! Contract C1 has to hold for every arrangement of discovered occurrences, not
//! only the written cases: a file path alone must never elevate authority, an
//! ambiguous project identity must never authorize, the decision must not depend
//! on the order occurrences arrive in, and an unauthorized `shells` declaration
//! must always fail. Each property is paired with a witness, so none can pass
//! merely because the generator never reaches an interesting state.

use super::super::super::paths::FsPathNormalizer;
use super::super::super::project_policy::RootAuthority;
use super::super::{
    ProjectKey, ScopedFileLayer, retain_layers_and_resolve_json, scope_discovered_occurrence,
};
use camino::Utf8PathBuf;
use ortho_config::MergeLayer;
use proptest::prelude::*;
use serde_json::json;
use std::borrow::Cow;
use tempfile::tempdir;

/// Location family an occurrence is loaded from.
///
/// The families are the ones C1 distinguishes: the project file, an unrelated
/// path reached from a discovered root, and an occurrence the loader recorded
/// no path for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    /// The project configuration file itself.
    Project,
    /// An unrelated path, loaded through a discovered root.
    Discovered,
    /// The loader recorded no path, so provenance is unknown.
    Unknown,
}

/// Map a generated selector onto its location family.
fn location(selector: u8) -> Location {
    match selector % 3 {
        0 => Location::Project,
        1 => Location::Discovered,
        _ => Location::Unknown,
    }
}

/// State the C1 authority rule, independently of the classifier.
///
/// Only an occurrence that a trusted root loaded, and that is not the project
/// configuration file, may declare executable policy. Unknown provenance is not
/// evidence of operator ownership.
fn spec_authority(is_trusted_root: bool, location: Location) -> RootAuthority {
    match (is_trusted_root, location) {
        (true, Location::Discovered) => RootAuthority::Operator,
        (_, Location::Project | Location::Unknown) | (false, Location::Discovered) => {
            RootAuthority::Untrusted
        }
    }
}

/// Recorded path spellings the properties classify against.
struct Fixture {
    /// Classified identity of the project configuration file.
    key: ProjectKey,
    /// Canonical spelling of the project file, as discovery records it.
    project: String,
    /// An unrelated path no test writes to.
    unrelated: String,
}

/// Build the fixture, rooting the project file in a temporary directory.
fn fixture() -> (tempfile::TempDir, Fixture) {
    let temp = tempdir().expect("create temp dir");
    let project_file = temp.path().join(".netsuke.toml");
    test_support::fs::write(&project_file, "jobs = 3\n").expect("write project config");
    let canonical_root = dunce::canonicalize(temp.path()).expect("canonicalize temp dir");
    let project = canonical_root.join(".netsuke.toml");
    let unrelated = canonical_root.join("operator.toml");
    // The properties distinguish the project file from every other occurrence,
    // so a missing temporary directory must fail here rather than silently make
    // the two spellings compare unequal for the wrong reason.
    assert!(
        project.exists(),
        "the fixture must create the project configuration file"
    );
    let key = ProjectKey::classify(&FsPathNormalizer, &project_file);
    let text = |path: &std::path::Path| -> String {
        path.to_str()
            .expect("canonical fixture path is UTF-8")
            .to_owned()
    };
    (
        temp,
        Fixture {
            key,
            project: text(&project),
            unrelated: text(&unrelated),
        },
    )
}

/// Return the recorded path for `location`, if it has one.
fn recorded_path(location: Location, fixture: &Fixture) -> Option<&str> {
    match location {
        Location::Project => Some(&fixture.project),
        Location::Discovered => Some(&fixture.unrelated),
        Location::Unknown => None,
    }
}

/// Build one occurrence layer, optionally declaring a `shells` key.
fn layer(path: Option<&str>, declares_shells: bool) -> MergeLayer<'static> {
    let value = if declares_shells {
        json!({"shells": []})
    } else {
        json!({"jobs": 2})
    };
    MergeLayer::file(Cow::Owned(value), path.map(Utf8PathBuf::from))
}

/// Classify one generated occurrence exactly as discovery would.
///
/// Returns both the authority and the scoped occurrence, so the merge properties
/// can feed the same layers through the production pipeline.
fn classify(
    is_trusted_root: bool,
    selector: u8,
    declares_shells: bool,
    fixture: &Fixture,
) -> (RootAuthority, ScopedFileLayer) {
    let path = recorded_path(location(selector), fixture);
    let scoped = scope_discovered_occurrence(
        layer(path, declares_shells),
        is_trusted_root,
        Some(&fixture.key),
    );
    (scoped.authority(), scoped)
}

proptest! {
    /// The classifier agrees with the C1 rule for every generated occurrence.
    #[test]
    fn authority_matches_the_operator_rule(
        entries in prop::collection::vec((any::<bool>(), 0..3u8, any::<bool>()), 1..8),
    ) {
        let (_temp, fixture) = fixture();
        for (is_trusted_root, selector, declares_shells) in entries {
            let (authority, _) = classify(is_trusted_root, selector, declares_shells, &fixture);
            prop_assert_eq!(
                authority,
                spec_authority(is_trusted_root, location(selector)),
                "occurrence (trusted={}, location={:?}) misclassified",
                is_trusted_root,
                location(selector)
            );
        }
    }

    /// Reversing the occurrence order permutes authorities without changing them.
    #[test]
    fn reversing_occurrence_order_permutes_authority(
        entries in prop::collection::vec((any::<bool>(), 0..3u8, any::<bool>()), 0..8),
    ) {
        let (_temp, fixture) = fixture();
        let authorities = |items: &[(bool, u8, bool)]| -> Vec<RootAuthority> {
            items
                .iter()
                .map(|(is_trusted_root, selector, declares_shells)| {
                    classify(*is_trusted_root, *selector, *declares_shells, &fixture).0
                })
                .collect()
        };
        let mut reversed = entries.clone();
        reversed.reverse();
        let mut expected = authorities(&entries);
        expected.reverse();
        prop_assert_eq!(
            authorities(&reversed),
            expected,
            "an occurrence's authority must not depend on its position"
        );
    }

    /// An unauthorized `shells` declaration always fails the merge.
    #[test]
    fn any_shells_declaration_fails_closed(
        entries in prop::collection::vec((any::<bool>(), 0..3u8, any::<bool>()), 0..8),
    ) {
        let (_temp, fixture) = fixture();
        let declares_shells = entries.iter().any(|(_, _, shells)| *shells);
        let layers = entries
            .iter()
            .map(|(is_trusted_root, selector, shells)| {
                classify(*is_trusted_root, *selector, *shells, &fixture).1
            })
            .collect();
        let resolved = retain_layers_and_resolve_json(layers);
        prop_assert_eq!(
            !resolved.errors.is_empty(),
            declares_shells,
            "rejection must happen exactly when a declaration is present"
        );
    }
}

/// An unrelated occurrence from a trusted root is the only authoritative case.
///
/// This is the witness that keeps the properties above from passing vacuously:
/// the generator can reach an operator-authorized occurrence, and the spec then
/// requires authority rather than withholding it.
#[test]
fn trusted_unrelated_occurrence_is_the_witness() {
    let (_temp, fixture) = fixture();

    let (authority, _) = classify(true, 1, false, &fixture);

    assert_eq!(
        authority,
        RootAuthority::Operator,
        "a trusted root's unrelated occurrence must keep operator authority"
    );
}
