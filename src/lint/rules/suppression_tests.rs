//! Tests for the rules that police the lint directives themselves.

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

/// A file-scope suppression that silences a directive-stage finding.
///
/// The inner directive states no reason, so it produces a
/// `suppression-without-reason` finding, and the file-scope directive silences
/// that finding. Two tests read this manifest from opposite ends: that the
/// finding is silenced, and that silencing it counts as work.
fn directive_stage_suppression_manifest() -> &'static str {
    concat!(
        "netsuke_version: \"1.0.0\"\n",
        "# netsuke-lint-file: allow suppression-without-reason -- reasons live in the tracker\n",
        "targets:\n",
        "  # netsuke-lint: allow background-job\n",
        "  - name: out\n",
        "    command: \"feh preview &\"\n",
    )
}

#[test]
fn unknown_suppression_reports_a_misspelled_rule() {
    let yaml = manifest("# netsuke-lint: allow backgroundjob -- typo");
    crate::assert_lint_fires!(&yaml, "unknown-suppression", 1);
    let messages = crate::lint_messages!(&yaml, "unknown-suppression");
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
    crate::assert_lint_fires!(&yaml, "unknown-suppression", 1);
}

#[test]
fn unknown_suppression_accepts_a_registered_rule() {
    crate::assert_lint_silent!(
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
    crate::assert_lint_silent!(yaml, "unknown-suppression");
}

#[test]
fn suppression_without_reason_reports_a_bare_directive() {
    crate::assert_lint_fires!(
        &manifest("# netsuke-lint: allow background-job"),
        "suppression-without-reason",
        1,
    );
}

#[test]
fn suppression_without_reason_accepts_a_stated_reason() {
    crate::assert_lint_silent!(
        &manifest("# netsuke-lint: allow background-job -- deliberate"),
        "suppression-without-reason",
    );
}

/// A `--` with nothing after it states no reason, so the trailing separator
/// must not be mistaken for one.
#[test]
fn suppression_without_reason_rejects_an_empty_reason() {
    crate::assert_lint_fires!(
        &manifest("# netsuke-lint: allow background-job --"),
        "suppression-without-reason",
        1,
    );
}

/// A file-scope directive silencing a directive-stage finding counts as used.
///
/// The counts are taken across two passes so this holds: the directive here
/// silences the `suppression-without-reason` finding the bare directive below
/// it produces, which is work, and it must not then be reported as having
/// silenced nothing.
#[test]
fn a_directive_silencing_a_directive_finding_is_not_unused() {
    let yaml = directive_stage_suppression_manifest();
    crate::assert_lint_silent_by_default!(yaml, "suppression-without-reason");
    crate::assert_lint_silent_by_default!(yaml, "unused-suppression");
}

/// A file-scope directive silences the reasonless directive beneath it.
#[test]
fn suppression_without_reason_is_suppressed_by_a_directive() {
    let yaml = directive_stage_suppression_manifest();
    crate::assert_lint_silent!(yaml, "suppression-without-reason");
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
    crate::assert_lint_fires_by_default!(yaml, "unused-suppression");
}

#[test]
fn unused_suppression_accepts_a_directive_that_did_its_job() {
    crate::assert_lint_silent_by_default!(
        &manifest("# netsuke-lint: allow background-job -- deliberate"),
        "unused-suppression",
    );
}

/// A directive naming only unregistered rules is reported by
/// `unknown-suppression`; reporting it here as well would say the same thing
/// twice.
#[test]
fn unused_suppression_defers_to_unknown_suppression() {
    crate::assert_lint_silent_by_default!(
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
    crate::assert_lint_silent_by_default!(yaml, "unused-suppression");
}
