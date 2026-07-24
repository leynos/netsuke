//! Failure contracts for the documented-example fence loader.

mod documentation_examples;

use anyhow::{Result, ensure};
use documentation_examples::{manifest_workspace, parse_document};
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case(
    "<!-- tested-example: -->\n",
    "tested-example identifier must not be empty"
)]
#[case(
    "<!-- tested-example:  -->\n",
    "tested-example identifier must not be empty"
)]
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

proptest! {
    #[test]
    fn marked_fence_round_trips(
        id in "[a-z][a-z0-9_-]{0,15}",
        language in "[a-z]{1,8}",
        body_lines in prop::collection::vec("[A-Za-z0-9 .,/_-]{0,40}", 0..8),
    ) {
        let body = if body_lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", body_lines.join("\n"))
        };
        let document = format!(
            "<!-- tested-example: {id} -->\n```{language}\n{body}```\n"
        );

        match parse_document("property.md", &document) {
            Ok(examples) => match examples.as_slice() {
                [example] => {
                    prop_assert_eq!(example.id.as_str(), id.as_str());
                    prop_assert_eq!(example.language.as_str(), language.as_str());
                    prop_assert_eq!(example.body.as_str(), body.as_str());
                }
                _ => prop_assert!(false, "expected one example, got {examples:?}"),
            },
            Err(error) => prop_assert!(false, "valid marked fence failed: {error}"),
        }
    }

    #[test]
    fn duplicate_identifiers_are_rejected(
        id in "[a-z][a-z0-9_-]{0,15}",
        first_body in "[A-Za-z0-9 .,/_-]{0,40}",
        second_body in "[A-Za-z0-9 .,/_-]{0,40}",
    ) {
        let document = format!(
            concat!(
                "<!-- tested-example: {} -->\n```yaml\n{}\n```\n",
                "<!-- tested-example: {} -->\n```sh\n{}\n```\n",
            ),
            id, first_body, id, second_body,
        );
        let result = parse_document("property.md", &document);

        prop_assert!(result.is_err());
        if let Err(error) = result {
            prop_assert!(
                error.to_string().contains("duplicate tested-example identifier"),
                "unexpected duplicate error: {error}",
            );
        }
    }

    #[test]
    fn unterminated_marked_fences_are_rejected(
        id in "[a-z][a-z0-9_-]{0,15}",
        language in "[a-z]{1,8}",
        body_lines in prop::collection::vec("[A-Za-z0-9 .,/_-]{0,40}", 0..8),
    ) {
        let body = body_lines.join("\n");
        let document = format!(
            "<!-- tested-example: {id} -->\n```{language}\n{body}\n"
        );
        let result = parse_document("property.md", &document);

        prop_assert!(result.is_err());
        if let Err(error) = result {
            prop_assert!(
                error.to_string().contains("fence is not terminated"),
                "unexpected unterminated-fence error: {error}",
            );
        }
    }
}
