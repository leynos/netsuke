//! Failure contracts for the documented-example fence loader.

mod documentation_examples;

use anyhow::{Result, ensure};
use documentation_examples::{manifest_workspace, parse_document};
use rstest::rstest;

#[rstest]
#[case("```yaml\ntargets: []\n```\n", "missing a tested-example marker")]
#[case(
    "<!-- tested-example: sample -->\n```yaml\ntargets: []\n",
    "fence is not terminated"
)]
#[case("<!-- tested-example: sample -->\n", "marker has no fence")]
#[case(
    "<!-- tested-example: sample -->\n```\ntargets: []\n```\n",
    "fence should declare a language"
)]
#[case(
    concat!(
        "<!-- tested-example: repeated -->\n```yaml\ntargets: []\n```\n",
        "<!-- tested-example: repeated -->\n```yaml\ntargets: []\n```\n"
    ),
    "duplicate tested-example identifier"
)]
fn malformed_documented_examples_are_rejected(
    #[case] contents: &str,
    #[case] expected_message: &str,
) -> Result<()> {
    let error = parse_document("fixture.md", contents)
        .expect_err("malformed documented example should be rejected");
    ensure!(
        error.to_string().contains(expected_message),
        "expected '{expected_message}' in '{error}'"
    );
    Ok(())
}

#[test]
fn non_yaml_example_cannot_be_used_as_a_manifest() -> Result<()> {
    let error = manifest_workspace("guide-cli-usage")
        .expect_err("non-YAML example should not create a manifest workspace");
    ensure!(
        error.to_string().contains("should be YAML"),
        "unexpected error: {error}"
    );
    Ok(())
}
