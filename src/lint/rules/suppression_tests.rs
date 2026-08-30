//! Tests for the rules that police the lint directives themselves.

use crate::lint::test_support::{
    assert_fires, assert_fires_by_default, assert_silent, assert_silent_by_default, messages_for,
};

/// A manifest whose only finding is silenced by `directive`.
fn manifest(directive: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  {}\n",
            "  - name: out\n",
            "    command: \"feh preview &\"\n",
        ),
        directive
    )
}

#[test]
fn unknown_suppression_reports_a_misspelled_rule() {
    let yaml = manifest("# netsuke-lint: allow backgroundjob -- typo");
    assert_fires(&yaml, "unknown-suppression", 1);
    let messages = messages_for(&yaml, "unknown-suppression");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`backgroundjob`")),
        "the finding should name the unknown rule, got {messages:?}"
    );
}

#[test]
fn unknown_suppression_reports_a_directive_naming_nothing() {
    let yaml = manifest("# netsuke-lint: allow -- nothing named");
    assert_fires(&yaml, "unknown-suppression", 1);
}

#[test]
fn unknown_suppression_accepts_a_registered_rule() {
    assert_silent(
        &manifest("# netsuke-lint: allow background-job -- deliberate"),
        "unknown-suppression",
    );
}

#[test]
fn unknown_suppression_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint-file: allow unknown-suppression -- the rule ships in the next release\n",
        "  # netsuke-lint: allow future-rule -- forward compatibility\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    assert_silent(yaml, "unknown-suppression");
}

#[test]
fn suppression_without_reason_reports_a_bare_directive() {
    assert_fires(
        &manifest("# netsuke-lint: allow background-job"),
        "suppression-without-reason",
        1,
    );
}

#[test]
fn suppression_without_reason_accepts_a_stated_reason() {
    assert_silent(
        &manifest("# netsuke-lint: allow background-job -- deliberate"),
        "suppression-without-reason",
    );
}

/// A `--` with nothing after it states no reason, so the trailing separator
/// must not be mistaken for one.
#[test]
fn suppression_without_reason_rejects_an_empty_reason() {
    assert_fires(
        &manifest("# netsuke-lint: allow background-job --"),
        "suppression-without-reason",
        1,
    );
}

#[test]
fn suppression_without_reason_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "# netsuke-lint-file: allow suppression-without-reason -- reasons live in the tracker\n",
        "targets:\n",
        "  # netsuke-lint: allow background-job\n",
        "  - name: out\n",
        "    command: \"feh preview &\"\n",
    );
    assert_silent(yaml, "suppression-without-reason");
}

/// Usage is measured against the rules that actually ran, so these cases use
/// the registry defaults rather than isolating one rule.
#[test]
fn unused_suppression_reports_a_directive_that_silenced_nothing() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow background-job -- the process used to be detached\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    assert_fires_by_default(yaml, "unused-suppression");
}

#[test]
fn unused_suppression_accepts_a_directive_that_did_its_job() {
    assert_silent_by_default(
        &manifest("# netsuke-lint: allow background-job -- deliberate"),
        "unused-suppression",
    );
}

/// A directive naming only unregistered rules is reported by
/// `unknown-suppression`; reporting it here as well would say the same thing
/// twice.
#[test]
fn unused_suppression_defers_to_unknown_suppression() {
    assert_silent_by_default(
        &manifest("# netsuke-lint: allow no-such-rule -- typo"),
        "unused-suppression",
    );
}

#[test]
fn unused_suppression_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "# netsuke-lint-file: allow unused-suppression -- inert on this platform\n",
        "targets:\n",
        "  # netsuke-lint: allow bashism -- only fires on the Windows manifest\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    assert_silent_by_default(yaml, "unused-suppression");
}
