//! Unit tests for the manifest schema-version admission predicate.

use super::{SUPPORTED, SchemaVersionRejection, admit, admit_document};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use rstest::rstest;
use semver::{BuildMetadata, Prerelease, Version, VersionReq};
use std::cell::Cell;

fn version_strategy() -> impl Strategy<Value = Version> {
    prop_oneof![
        1 => Just(Version::new(1, 0, 0)),
        3 => (
            0_u64..=3,
            0_u64..=3,
            0_u64..=3,
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(major, minor, patch, has_prerelease, has_build)| {
                let mut version = Version::new(major, minor, patch);
                if has_prerelease {
                    version.pre = Prerelease::new("rc.1").expect("static prerelease is valid");
                }
                if has_build {
                    version.build = BuildMetadata::new("build.5").expect("static build is valid");
                }
                version
            }),
    ]
}

fn oracle_admits(version: &Version) -> bool {
    let exact_release_match = (0..=SUPPORTED.max_minor).any(|minor| {
        VersionReq::parse(&format!("=1.{minor}.0"))
            .expect("generated exact version requirement is valid")
            .matches(version)
    });
    exact_release_match && version.build.is_empty()
}

#[rstest]
#[case("1.0.0", true)]
#[case("1.0.3", false)]
#[case("1.0.0+build.5", false)]
#[case("1.0.0-rc.1", false)]
#[case("1.1.0", false)]
#[case("1.1.0-alpha", false)]
#[case("2.0.0", false)]
#[case("0.1.0", false)]
#[case("0.0.0", false)]
fn admission_table_covers_schema_version_classes(#[case] raw: &str, #[case] is_admitted: bool) {
    let version = Version::parse(raw).expect("table version is valid SemVer");
    assert_eq!(
        admit(&version).is_ok(),
        is_admitted,
        "unexpected admission result for {raw}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn admission_matches_the_independent_exact_version_oracle(version in version_strategy()) {
        prop_assert_eq!(admit(&version).is_ok(), oracle_admits(&version));
    }
}

#[rstest]
fn generated_versions_cover_admitted_and_rejected_classes() {
    let config = ProptestConfig {
        cases: 1_000,
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new_with_rng(
        config.clone(),
        proptest::test_runner::TestRng::deterministic_rng(config.rng_algorithm),
    );
    let admitted = Cell::new(0_u32);
    let rejected = Cell::new(0_u32);
    runner
        .run(&version_strategy(), |version| {
            if admit(&version).is_ok() {
                admitted.set(admitted.get() + 1);
            } else {
                rejected.set(rejected.get() + 1);
            }
            Ok(())
        })
        .expect("deterministic samples should be generated");

    assert!(
        admitted.get() >= 100,
        "only {} generated versions admitted",
        admitted.get()
    );
    assert!(
        rejected.get() >= 100,
        "only {} generated versions rejected",
        rejected.get()
    );
}

#[rstest]
fn document_admission_reports_typed_rejections_and_preserves_non_mapping_roots() {
    let cases = [
        (serde_json::json!({}), SchemaVersionRejection::Missing),
        (
            serde_json::json!({"netsuke_version": 7}),
            SchemaVersionRejection::NotAString,
        ),
        (
            serde_json::json!({"netsuke_version": "not-semver"}),
            SchemaVersionRejection::Invalid,
        ),
        (
            serde_json::json!({"netsuke_version": "1.1.0"}),
            SchemaVersionRejection::Unsupported { major: 1, minor: 1 },
        ),
    ];
    for (document, expected) in cases {
        assert_eq!(admit_document(&document), Err(expected));
    }
    assert_eq!(
        admit_document(&serde_json::json!({"netsuke_version": "1.0.0"})),
        Ok(())
    );
    assert_eq!(admit_document(&serde_json::json!(null)), Ok(()));
}
