//! Fluent message identifiers used by `netsuke check`.
//!
//! Split from `keys.rs`, which sits at Whitaker's 400-line module cap. The
//! table keeps the same shape, so the build-time localization audit reads
//! it the same way, and `keys` re-exports every constant, so callers still
//! write `keys::CHECK_SUMMARY_TRUNCATED`.

define_keys! {
    CLI_SUBCOMMAND_CHECK_ABOUT => "cli.subcommand.check.about",
    CLI_SUBCOMMAND_CHECK_LONG_ABOUT => "cli.subcommand.check.long_about",
    CLI_SUBCOMMAND_CHECK_FLAG_RULE_HELP => "cli.subcommand.check.flag.rule.help",
    CLI_SUBCOMMAND_CHECK_FLAG_FAIL_ON_HELP => "cli.subcommand.check.flag.fail_on.help",
    CLI_SUBCOMMAND_CHECK_FLAG_LIMIT_HELP => "cli.subcommand.check.flag.limit.help",
    CLI_SUBCOMMAND_CHECK_FLAG_EXPLAIN_HELP => "cli.subcommand.check.flag.explain.help",
    CHECK_THRESHOLD_EXCEEDED => "check.threshold_exceeded",
    CHECK_THRESHOLD_EXCEEDED_HELP => "check.threshold_exceeded.help",
    CHECK_SUMMARY_COUNTS => "check.summary.counts",
    CHECK_SUMMARY_CLEAN => "check.summary.clean",
    CHECK_SUMMARY_TRUNCATED => "check.summary.truncated",
    CHECK_RULE_MALFORMED => "check.rule.malformed",
    CHECK_RULE_UNKNOWN => "check.rule.unknown",
    CHECK_RULE_SEVERITY => "check.rule.severity",
    CHECK_FAIL_ON_INVALID => "check.fail_on.invalid",
    CHECK_SOURCE_INDEX => "check.source_index",
    STATUS_TOOL_CHECK => "status.tool.check",
}
