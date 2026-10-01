//! Exercise ownership, namespace, registration and denied-name validators.

use super::*;
use crate::rfc_stdlib_coverage::validation_fixtures as fixture;
use rstest::rstest;

#[test]
fn accepts_matching_ownership_and_registry() -> Result<()> {
    check_ownership(&fixture::world())?;
    check_forbidden(&fixture::world())
}

#[rstest]
#[case::unowned("unowned", "claims no owner for [\"helper\"]")]
#[case::extra("extra", "which RFC 0006 does not accept")]
#[case::missing_map("missing_map", "coverage map has no row for it")]
#[case::registry_names("registry_names", "missing [\"helper\"]; unexpected [\"other\"]")]
#[case::namespace(
    "namespace",
    "namespace test, but RFC 0006 section 7 places it in filter"
)]
#[case::registration("registration", "marks helper `option added`")]
#[case::two_owners("two_owners", "claimed by both RFC 0013 and RFC 0014")]
fn rejects_partition_mismatches(#[case] mutation: &str, #[case] diagnostic: &str) {
    let mut world = fixture::world();
    match mutation {
        "unowned" => world
            .map
            .rows
            .first_mut()
            .expect("fixture owning map row")
            .owns
            .clear(),
        "extra" => world
            .map
            .rows
            .get_mut(1)
            .expect("fixture second map row")
            .owns
            .push("extra".into()),
        "missing_map" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .number = "0021".into();
        }
        "registry_names" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .helper
                .name = "other".into();
        }
        "namespace" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .helper
                .namespace = super::super::Namespace::Test;
        }
        "registration" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .registration = Registration::OptionAdded;
        }
        "two_owners" => world
            .map
            .rows
            .get_mut(1)
            .expect("fixture second map row")
            .owns
            .push("helper".into()),
        _ => {}
    }
    let error = check_ownership(&world).expect_err("one partition condition changed");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}

#[test]
fn rejects_unaccepted_rows_before_namespace_comparison() {
    let mut registry = fixture::registry();
    registry.rows.first_mut().expect("fixture row").helper.name = "unknown".into();
    let error =
        check_rows_agree_with_survey(&registry, &fixture::survey()).expect_err("unknown helper");
    assert!(
        error
            .to_string()
            .contains("docs/rfcs/0013-child.md registers unknown")
    );
}

#[test]
fn requires_optioned_helpers_to_be_marked_option_added() {
    let mut survey = fixture::survey();
    survey.optioned.push("helper".into());
    let error =
        check_rows_agree_with_survey(&fixture::registry(), &survey).expect_err("wrong kind");
    assert!(
        error
            .to_string()
            .contains("RFC 0006 lists it `option added`")
    );
}

#[test]
fn rejects_denied_registrations_with_the_child_filename() {
    let mut world = fixture::world();
    world
        .registries
        .first_mut()
        .expect("fixture registry")
        .rows
        .first_mut()
        .expect("fixture row")
        .helper
        .name = "is_file".into();
    let error = check_forbidden(&world).expect_err("denied helper");
    assert!(
        error
            .to_string()
            .contains("docs/rfcs/0013-child.md registers is_file")
    );
    assert!(error.to_string().contains("section 7 or section 9 forbids"));
}
