//! Tests for typed CLI configuration preferences and compatibility helpers.

use super::*;
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case::colour_auto(
    ColourPolicy::parse_raw("auto").map(|value| value.to_string()),
    "auto"
)]
#[case::colour_always(
    ColourPolicy::parse_raw("always").map(|value| value.to_string()),
    "always"
)]
#[case::spinner_enabled(
    SpinnerMode::parse_raw("enabled").map(|value| value.to_string()),
    "enabled"
)]
#[case::spinner_disabled(
    SpinnerMode::parse_raw("disabled").map(|value| value.to_string()),
    "disabled"
)]
#[case::output_human(
    OutputFormat::parse_raw("human").map(|value| value.to_string()),
    "human"
)]
#[case::output_json(
    OutputFormat::parse_raw("json").map(|value| value.to_string()),
    "json"
)]
fn config_enums_round_trip(
    #[case] parsed: Result<String, &'static [&'static str]>,
    #[case] expected: &'static str,
) {
    assert_eq!(parsed.expect("enum value should parse"), expected);
}

#[rstest]
#[case::colour(
    ColourPolicy::from_str("loud").expect_err("invalid colour policy should fail"),
    &["auto", "always", "never"]
)]
#[case::spinner(
    SpinnerMode::from_str("paused").expect_err("invalid spinner mode should fail"),
    &["enabled", "disabled"]
)]
#[case::output(
    OutputFormat::from_str("tap").expect_err("invalid output format should fail"),
    &["human", "json"]
)]
fn config_enums_reject_invalid_values(
    #[case] error: ParseConfigEnumError,
    #[case] expected_options: &'static [&'static str],
) {
    assert_eq!(error.valid_options, expected_options);
}

#[test]
fn cli_config_alias_resolution_prefers_new_fields() {
    let config = CliConfig {
        progress: Some(false),
        spinner_mode: Some(SpinnerMode::Enabled),
        diag_json: false,
        output_format: Some(OutputFormat::Json),
        ..CliConfig::default()
    };

    assert!(config.resolved_progress());
    assert!(config.resolved_diag_json());
}

#[rstest]
#[case::colour("Always", "always", "colour")]
#[case::colour_upper("ALWAYS", "always", "colour")]
#[case::colour_trimmed(" always ", "always", "colour")]
#[case::spinner("Enabled", "enabled", "spinner")]
#[case::spinner_upper("DISABLED", "disabled", "spinner")]
#[case::spinner_trimmed(" disabled ", "disabled", "spinner")]
#[case::output("Human", "human", "output")]
#[case::output_upper("JSON", "json", "output")]
#[case::output_trimmed(" json ", "json", "output")]
fn config_enums_deserialize_case_insensitively(
    #[case] raw: &str,
    #[case] expected_display: &str,
    #[case] family: &str,
) {
    match family {
        "colour" => {
            let parsed: ColourPolicy =
                serde_json::from_value(json!(raw)).expect("colour policy should deserialize");
            let expected = ColourPolicy::from_str(raw)
                .expect("colour policy should parse through FromStr")
                .to_string();
            assert_eq!(parsed.to_string(), expected);
            assert_eq!(parsed.to_string(), expected_display);
        }
        "spinner" => {
            let parsed: SpinnerMode =
                serde_json::from_value(json!(raw)).expect("spinner mode should deserialize");
            let expected = SpinnerMode::from_str(raw)
                .expect("spinner mode should parse through FromStr")
                .to_string();
            assert_eq!(parsed.to_string(), expected);
            assert_eq!(parsed.to_string(), expected_display);
        }
        "output" => {
            let parsed: OutputFormat =
                serde_json::from_value(json!(raw)).expect("output format should deserialize");
            let expected = OutputFormat::from_str(raw)
                .expect("output format should parse through FromStr")
                .to_string();
            assert_eq!(parsed.to_string(), expected);
            assert_eq!(parsed.to_string(), expected_display);
        }
        other => panic!("unexpected enum family {other}"),
    }
}
