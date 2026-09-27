//! Snapshot of the reserved-`vars` diagnostic as a user sees it.
//!
//! The rstest cases in `tests/ast_tests/parsing.rs` assert semantically — that
//! the chain carries the localized message for the offending name. This pins
//! the whole rendered chain instead, so a change to the wording, the
//! interpolation, or the surrounding structural wrapper shows up as a snapshot
//! diff rather than passing a substring check unnoticed.
//!
//! The locale is pinned to `en-US` because the localizer is process-global.

use crate::manifest;
use crate::snapshot_test_support::snapshot_settings;
use insta::assert_snapshot;
use rstest::rstest;
use test_support::localizer::{EnLocalizer, en_localizer};
use test_support::{display_error_chain, fluent::normalize_fluent_isolates};

/// Build a manifest whose `vars` shadows a built-in helper.
fn manifest_shadowing(reserved: &str) -> String {
    format!(
        r#"
        netsuke_version: "1.0.0"
        vars:
          {reserved}: shadowed
        targets:
          - name: hello
            command: "echo hi"
    "#
    )
}

#[rstest]
#[case::env("env", "reserved_vars_env")]
#[case::glob("glob", "reserved_vars_glob")]
fn reserved_vars_diagnostic_snapshot(
    en_localizer: EnLocalizer,
    #[case] reserved: &str,
    #[case] snapshot_name: &str,
) {
    let _localizer = en_localizer;

    let error = manifest::from_str(&manifest_shadowing(reserved))
        .expect_err("reserved vars key should be rejected");
    let rendered = normalize_fluent_isolates(&display_error_chain(error.as_ref()));

    snapshot_settings("manifest").bind(|| {
        assert_snapshot!(snapshot_name, rendered);
    });
}
