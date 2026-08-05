//! Unit tests for nested environment-layer construction and conflicts.

use super::*;
use proptest::prelude::*;

fn layer(entries: &[(&str, &str)]) -> EnvironmentLayer {
    EnvironmentLayer::new(
        entries
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect(),
    )
}

#[test]
fn provider_filters_prefixes_and_builds_nested_values() {
    let data = layer(&[
        ("IGNORED", "value"),
        ("netsuke_cmds__build__targets", "all"),
    ])
    .data()
    .expect("valid nested environment should produce provider data");
    let defaults = data
        .get(&Profile::Default)
        .expect("provider should emit the default profile");
    let Value::Dict(_, commands) = defaults.get("cmds").expect("cmds dictionary") else {
        panic!("cmds should be a dictionary");
    };
    let Value::Dict(_, build) = commands.get("build").expect("build dictionary") else {
        panic!("build should be a dictionary");
    };
    assert_eq!(build.get("targets").and_then(Value::as_str), Some("all"));
    assert!(!defaults.contains_key("ignored"));
}

#[test]
fn provider_rejects_parent_scalar_before_nested_key() {
    let error = layer(&[
        ("NETSUKE_CMDS", "build"),
        ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
    ])
    .data()
    .expect_err("a scalar parent must conflict with a nested key");
    assert!(error.to_string().contains("scalar configuration key"));
}

#[test]
fn provider_rejects_nested_key_before_parent_scalar() {
    let error = layer(&[
        ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
        ("NETSUKE_CMDS", "build"),
    ])
    .data()
    .expect_err("a nested key must conflict with a scalar parent");
    assert!(error.to_string().contains("nested configuration key"));
}

#[test]
fn provider_rejects_aliases_of_an_existing_scalar_key() {
    let aliases = [
        ("NETSUKE_CMDS__BUILD", "NETSUKE_CMDS____BUILD"),
        ("netsuke_cmds__build", " NETSUKE_CMDS__ BUILD "),
    ];

    for (first, alias) in aliases {
        let error = layer(&[(first, "first"), (alias, "second")])
            .data()
            .expect_err("normalized scalar aliases must conflict");
        assert!(
            error
                .to_string()
                .contains("existing scalar configuration key"),
            "normalized alias conflict should identify an existing scalar: {error}"
        );
    }
}

mod entry {
    //! Direct tests for per-entry interpretation.
    //!
    //! The provider-level tests above cover the integration boundary; these
    //! pin what `parse_entry` decides before anything is inserted.

    use super::super::*;
    // rstest expands each case into a nested module, so `super::` would
    // shift a level inside a `#[case]` expression; import the helper here.
    #[cfg(unix)]
    use super::non_utf8::invalid_bytes;

    fn parse(key: &str, value: &str) -> Result<Option<(Vec<String>, Value)>, Box<Error>> {
        parse_entry(OsStr::new(key), OsStr::new(value))
    }

    #[rstest::rstest]
    #[case::unrelated("IGNORED")]
    #[case::non_matching_prefix("NETSUK_CMDS")]
    #[case::empty_effective_key("NETSUKE_")]
    #[case::separator_only_key("NETSUKE___")]
    #[case::whitespace_only_key("NETSUKE_  ")]
    fn keys_outside_the_configuration_namespace_yield_nothing(#[case] key: &str) {
        let parsed = parse(key, "value")
            .unwrap_or_else(|error| panic!("{key:?} should not be an error: {error}"));

        assert!(
            parsed.is_none(),
            "{key:?} should contribute no configuration, got {parsed:?}"
        );
    }

    #[test]
    fn valid_key_yields_normalized_components_and_parsed_value() {
        let Some((components, value)) = parse(" netsuke_CMDS__ Build __targets ", "all")
            .expect("a valid Netsuke key should parse")
        else {
            panic!("a valid Netsuke key should yield components");
        };

        assert_eq!(components, ["cmds", "build", "targets"]);
        assert_eq!(value.as_str(), Some("all"));
    }

    /// Invalid UTF-8 only exists as `OsString` on Unix, where
    /// `OsStringExt::from_vec` accepts arbitrary bytes.
    #[cfg(unix)]
    #[rstest::rstest]
    #[case::key(invalid_bytes(b"NETSUKE_"), OsString::from("value"), NON_UTF8_KEY)]
    #[case::value(
        OsString::from("NETSUKE_CMDS__BUILD"),
        invalid_bytes(b""),
        NON_UTF8_VALUE
    )]
    fn invalid_utf8_entries_report_their_fixed_message(
        #[case] key: OsString,
        #[case] value: OsString,
        #[case] expected_message: &str,
    ) {
        let Err(error) = parse_entry(&key, &value) else {
            panic!("invalid UTF-8 configuration must be rejected");
        };

        assert!(
            error.to_string().contains(expected_message),
            "expected the fixed rejection text, got {error}"
        );
    }
}

#[cfg(unix)]
mod non_utf8 {
    //! Strict-decode tests for injected configuration.
    //!
    //! Invalid UTF-8 keys and values only exist as `OsString` on Unix,
    //! where `OsStringExt::from_vec` accepts arbitrary bytes.

    use super::super::*;
    use crate::test_tracing_capture::with_test_subscriber;
    use std::os::unix::ffi::OsStringExt;
    use tracing_subscriber::filter::LevelFilter;

    /// Stands in for a secret smuggled through the environment; it must
    /// never reach an error message or a log line.
    const SENTINEL: &[u8] = b"s3cr3t-sentinel";

    /// Build a non-UTF-8 `OsString` that carries [`SENTINEL`] after
    /// `prefix`, so a leak into an error or log is recognisable.
    pub(super) fn invalid_bytes(prefix: &[u8]) -> OsString {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(SENTINEL);
        bytes.push(0xFF);
        OsString::from_vec(bytes)
    }

    fn layer(key: OsString, value: OsString) -> EnvironmentLayer {
        EnvironmentLayer::new(vec![(key, value)])
    }

    #[rstest::rstest]
    #[case::key(
        invalid_bytes(b"NETSUKE_"),
        OsString::from("value"),
        NON_UTF8_KEY,
        "a non-UTF-8 Netsuke key"
    )]
    #[case::value(
        OsString::from("NETSUKE_CMDS__BUILD"),
        invalid_bytes(b""),
        NON_UTF8_VALUE,
        "a non-UTF-8 Netsuke value"
    )]
    fn non_utf8_entries_are_rejected_with_fixed_text(
        #[case] key: OsString,
        #[case] value: OsString,
        #[case] expected_message: &str,
        #[case] case_description: &str,
    ) {
        let Err(error) = layer(key, value).data() else {
            panic!("{case_description} must be rejected");
        };

        assert!(
            error.to_string().contains(expected_message),
            "expected the fixed rejection text for {case_description}, got {error}"
        );
        assert!(
            !error.to_string().contains("s3cr3t-sentinel"),
            "the rejected input must not appear in the error for {case_description}: {error}"
        );
    }

    #[test]
    fn unrelated_non_utf8_entries_are_skipped() {
        let data = layer(invalid_bytes(b"UNRELATED_"), invalid_bytes(b""))
            .data()
            .expect("an unrelated non-UTF-8 entry must not fail the load");

        assert!(
            data.get(&Profile::Default).is_some_and(Dict::is_empty),
            "an unrelated entry should contribute no configuration"
        );
    }

    #[rstest::rstest]
    #[case::key(invalid_bytes(b"NETSUKE_"), OsString::from("value"), "non_utf8_key")]
    #[case::value(
        OsString::from("NETSUKE_CMDS__BUILD"),
        invalid_bytes(b""),
        "non_utf8_value"
    )]
    fn rejection_warns_with_only_a_bounded_failure_kind(
        #[case] key: OsString,
        #[case] value: OsString,
        #[case] failure_kind: &str,
    ) {
        let events = with_test_subscriber(LevelFilter::WARN, |captured| {
            layer(key, value)
                .data()
                .expect_err("the invalid entry must be rejected");
            captured.snapshot()
        });

        assert!(
            events.iter().any(
                |event| event.contains("rejected non-UTF-8 injected configuration")
                    && event.contains(&format!("failure_kind=\"{failure_kind}\""))
            ),
            "expected a bounded rejection warning in {events:?}"
        );
        assert!(
            !events.iter().any(|event| event.contains("s3cr3t-sentinel")),
            "the rejected input must not be logged: {events:?}"
        );
    }
}

fn component(name: &str, uppercase: bool, whitespace: &str) -> String {
    let normalized_case = if uppercase {
        name.to_ascii_uppercase()
    } else {
        name.to_ascii_lowercase()
    };
    format!("{whitespace}{normalized_case}{whitespace}")
}

proptest! {
    #[test]
    fn normalized_scalar_aliases_never_replace_existing_values(
        prefix_uppercase in any::<bool>(),
        cmds_uppercase in any::<bool>(),
        build_uppercase in any::<bool>(),
        whitespace in "[ \\t]{0,2}",
        separator in prop::sample::select(vec!["__", "____", "______"]),
        reverse_order in any::<bool>(),
    ) {
        let prefix = component("netsuke_", prefix_uppercase, "");
        let cmds = component("cmds", cmds_uppercase, &whitespace);
        let build = component("build", build_uppercase, &whitespace);
        let canonical = String::from("NETSUKE_CMDS__BUILD");
        let alias = format!("{whitespace}{prefix}{cmds}{separator}{build}{whitespace}");
        let entries = if reverse_order {
            [(alias.as_str(), "alias"), (canonical.as_str(), "canonical")]
        } else {
            [(canonical.as_str(), "canonical"), (alias.as_str(), "alias")]
        };

        prop_assert!(layer(&entries).data().is_err());
    }

    #[test]
    fn scalar_and_nested_keys_conflict_in_either_order(reverse_order in any::<bool>()) {
        let entries = if reverse_order {
            [
                ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
                ("NETSUKE_CMDS__BUILD", "scalar"),
            ]
        } else {
            [
                ("NETSUKE_CMDS__BUILD", "scalar"),
                ("NETSUKE_CMDS__BUILD__TARGETS", "all"),
            ]
        };

        prop_assert!(layer(&entries).data().is_err());
    }
}
