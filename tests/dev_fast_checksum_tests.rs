//! Property coverage for the installer's checksum verification.
//!
//! Verification is the installer's whole reason for existing, so its decision
//! is checked against a model rather than against a list of examples. The
//! strategy ranges over the structural relationships a recorded row can hold to
//! the artefact — right digest, wrong, truncated, re-cased, another artefact's,
//! duplicated, whitespace-padded — because random digests would explore only
//! one equivalence class: they never match.
//!
//! This is where the order-dependence defect surfaced, on the first run: two
//! rows for one artefact made the shell's `expected` multi-line, and only the
//! last one was actually checked.

#![cfg(all(unix, target_os = "linux"))]

use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::FileFailurePersistence;
use test_support::dev_fast::{InstallerScenario, WRONG_SHA256, combined};

/// How one recorded digest relates to the artefact's real one.
#[derive(Copy, Clone, Debug)]
enum Digest {
    /// The artefact's own digest, verbatim.
    Real,
    /// The same digest upper-cased. `sha256sum` compares hex case-insensitively,
    /// so this still verifies — established by probing the tool rather than
    /// assumed.
    RealUpperCased,
    /// A well-formed digest belonging to nothing.
    Wrong,
    /// The real digest with its tail removed.
    Truncated,
}

/// Which artefact a checksum row names.
#[derive(Copy, Clone, Debug)]
enum Named {
    ThisArtefact,
    Other,
}

/// One row of a checksum file.
#[derive(Copy, Clone, Debug)]
struct Row {
    digest: Digest,
    named: Named,
    /// Surrounding whitespace, which the installer's `awk` field split ignores.
    padded: bool,
}

impl Row {
    fn render(self, real: &str, artefact: &str) -> String {
        let digest = match self.digest {
            Digest::Real => real.to_owned(),
            Digest::RealUpperCased => real.to_uppercase(),
            Digest::Wrong => WRONG_SHA256.to_owned(),
            // `get` rather than a bare range: the digest is ASCII hex, but a
            // panicking slice has no place in a model oracle.
            Digest::Truncated => real
                .get(..real.len().saturating_sub(4))
                .unwrap_or_default()
                .to_owned(),
        };
        let name = match self.named {
            Named::ThisArtefact => artefact,
            Named::Other => "mold-0.0.0-x86_64-linux.tar.gz",
        };
        if self.padded {
            format!("   {digest}   {name}   \n")
        } else {
            format!("{digest}  {name}\n")
        }
    }
}

/// The installer must unpack exactly when the file records this artefact
/// exactly once, under the artefact's real digest.
///
/// This is a model, not a lookup table: it predicts the outcome for row lists
/// nobody enumerated. Requiring a single row is deliberate — several rows for
/// one artefact are ambiguous, and the installer refuses rather than picking
/// one. Case is not significant, because `sha256sum` compares hex
/// case-insensitively.
fn model_installs(rows: &[Row]) -> bool {
    let mut selected = rows
        .iter()
        .filter(|row| matches!(row.named, Named::ThisArtefact));
    let Some(only) = selected.next() else {
        return false;
    };
    selected.next().is_none() && matches!(only.digest, Digest::Real | Digest::RealUpperCased)
}

fn digest_strategy() -> impl Strategy<Value = Digest> {
    prop_oneof![
        Just(Digest::Real),
        Just(Digest::RealUpperCased),
        Just(Digest::Wrong),
        Just(Digest::Truncated),
    ]
}

fn row_strategy() -> impl Strategy<Value = Row> {
    (
        digest_strategy(),
        prop_oneof![Just(Named::ThisArtefact), Just(Named::Other)],
        any::<bool>(),
    )
        .prop_map(|(digest, named, padded)| Row {
            digest,
            named,
            padded,
        })
}

proptest! {
    // Each case runs the installer, so keep the corpus small enough to stay a
    // second or two while still ranging over the row space.
    #![proptest_config(ProptestConfig {
        cases: 24,
        // Name the file explicitly. The default `SourceParallel` policy
        // looks for a `lib.rs` or `main.rs` beside the source and gives up
        // in an integration-test crate, so recorded seeds were neither
        // written nor replayed — the file on disk was inert.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/dev_fast_checksum_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Whatever rows a checksum file carries, the installer's decision must
    /// match the model, and a refusal must leave nothing unpacked.
    ///
    /// Random digests alone would be uninformative — they never match — so the
    /// strategy ranges over the structural relationships instead: right digest,
    /// wrong digest, truncated, re-cased, another artefact's, duplicated, and
    /// whitespace-padded.
    #[test]
    fn the_installer_verifies_exactly_when_the_model_says_it_should(
        rows in prop::collection::vec(row_strategy(), 0..4)
    ) {
        let scenario = InstallerScenario::prepare()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let real = scenario.release().sha256().to_owned();
        let artefact = scenario.release().name().to_owned();
        let contents: String = rows.iter().map(|row| row.render(&real, &artefact)).collect();

        let checksums = scenario.sandbox().home().join("SHA256SUMS");
        scenario
            .sandbox()
            .write_file(&checksums, &contents)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let fixture = scenario
            .fixture(checksums)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let output = scenario
            .sandbox()
            .script("install-dev-fast.sh", &fixture.script_env())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let expected = model_installs(&rows);
        prop_assert_eq!(
            output.status.success(),
            expected,
            "rows {:?} should {} install, got `{}`",
            rows,
            if expected { "" } else { "not" },
            combined(&output)
        );
        prop_assert_eq!(
            scenario.installed_mold().as_std_path().exists(),
            expected,
            "unpacked state should follow the verification decision for {:?}",
            rows
        );
    }
}
