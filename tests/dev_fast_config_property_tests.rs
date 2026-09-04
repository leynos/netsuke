//! Property tests for `DEV_FAST_CONFIG` argument preservation.
//!
//! The dev-fast Make recipes receive this caller-controlled value and pass it
//! to Cargo through a shell. These tests use a raw-byte capture fake so empty
//! values and whitespace stay observable, rather than relying on the ordinary
//! recording fake's whitespace-split log format.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use test_support::{
    dev_fast::{BuildScenario, MakeInvocation, combined},
    fs,
};

/// Run `target` and return the raw `--config` argument captured by fake Cargo.
fn captured_config_argument(target: &str, config: &str) -> Result<Vec<u8>> {
    let scenario = BuildScenario::prepare()?;
    let capture = scenario.sandbox().home().join("config-argument.bin");
    let cargo = scenario.sandbox().write_fake(
        &scenario.sandbox().bin(),
        "capture-config",
        &format!(
            concat!(
                "while [ \"$#\" -gt 0 ]; do\n",
                "  if [ \"$1\" = --config ]; then\n",
                "    [ \"$#\" -ge 2 ] || exit 64\n",
                "    printf '%s' \"$2\" > '{capture}'\n",
                "    exit 0\n",
                "  fi\n",
                "  shift\n",
                "done\n",
                "exit 65"
            ),
            capture = capture
        ),
    )?;
    let invocation = MakeInvocation::new(target)
        .variable("CARGO", cargo)
        .variable("DEV_FAST_CONFIG", config);
    let output = scenario.sandbox().run_make(&invocation)?;
    ensure!(
        output.status.success(),
        "make {target} should capture the --config argument, got `{}`",
        combined(&output)
    );
    fs::read(&capture).with_context(|| format!("read captured config argument from {capture}"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Preserve generated shell-sensitive overrides as one Cargo argument.
    #[test]
    fn dev_fast_config_overrides_reach_both_targets_unchanged(
        prefix in "[A-Za-z0-9._/-]{0,12}",
        quote in prop_oneof![Just('\''), Just('"')],
        whitespace in prop_oneof![Just(' '), Just('\t')],
        separator in prop_oneof![Just(';'), Just('&'), Just('|')],
        metacharacter in prop_oneof![
            Just('$'), Just('`'), Just('('), Just(')'), Just('<'), Just('>'), Just('\\'),
        ],
        suffix in "[A-Za-z0-9._/-]{0,12}",
    ) {
        let shell_sensitive = format!("{prefix}{quote}{whitespace}{separator}{metacharacter}{suffix}");
        for config in [String::new(), shell_sensitive] {
            for target in ["dev-build", "dev-test"] {
                let captured = captured_config_argument(target, &config)
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert_eq!(
                    captured.as_slice(),
                    config.as_bytes(),
                    "{} should preserve {:?} as one --config argument, got {:?}",
                    target,
                    config,
                    captured
                );
            }
        }
    }
}
