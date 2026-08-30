//! Tests for the migration rules.

use rstest::rstest;

use crate::lint::test_support::{assert_fires, assert_silent, messages_for, spans_for};

/// Build a one-target manifest whose command is `command`.
fn manifest(command: &str) -> String {
    format!("netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    command: {command}\n")
}

#[rstest]
#[case("\"cp $$SRC {{ outs }}\"")]
#[case("\"printf %s $${HOME} > {{ outs }}\"")]
#[case("\"echo $$_private > {{ outs }}\"")]
fn manual_ninja_escape_reports_a_doubled_dollar(#[case] command: &str) {
    assert_fires(&manifest(command), "manual-ninja-escape", 1);
}

/// A bare `$$` is the shell's own process identifier, not the retired
/// escaping workaround, so it must not be reported.
#[rstest]
#[case("\"echo $$ > {{ outs }}\"")]
#[case("\"echo $PATH > {{ outs }}\"")]
#[case("\"echo 'a $$ b' > {{ outs }}\"")]
fn manual_ninja_escape_leaves_ordinary_shell_text_alone(#[case] command: &str) {
    assert_silent(&manifest(command), "manual-ninja-escape");
}

#[test]
fn manual_ninja_escape_points_at_the_doubled_dollar() {
    assert_eq!(
        spans_for(&manifest("\"cp $$SRC {{ outs }}\""), "manual-ninja-escape"),
        vec!["$$"]
    );
}

#[test]
fn manual_ninja_escape_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow manual-ninja-escape -- kept for the legacy runner\n",
        "  - name: out\n",
        "    command: \"cp $$SRC {{ outs }}\"\n",
    );
    assert_silent(yaml, "manual-ninja-escape");
}

#[rstest]
#[case("\"cp $in {{ outs }}\"", 1)]
#[case("\"cp {{ ins }} $out\"", 1)]
#[case("\"cp $in $out\"", 2)]
fn legacy_placeholder_reports_undocumented_spellings(
    #[case] command: &str,
    #[case] expected: usize,
) {
    assert_fires(&manifest(command), "legacy-placeholder", expected);
}

/// A longer variable name only starts with the placeholder; a doubled dollar
/// belongs to `manual-ninja-escape`.
#[rstest]
#[case("\"cp $input {{ outs }}\"")]
#[case("\"cp $output {{ outs }}\"")]
#[case("\"cp $$out {{ outs }}\"")]
#[case("\"cp {{ ins }} {{ outs }}\"")]
fn legacy_placeholder_leaves_other_variables_alone(#[case] command: &str) {
    assert_silent(&manifest(command), "legacy-placeholder");
}

#[test]
fn legacy_placeholder_names_the_documented_replacement() {
    let messages = messages_for(&manifest("\"cp $in $out\""), "legacy-placeholder");
    assert!(
        messages.iter().any(|message| message.contains("{{ ins }}")),
        "the finding should name the replacement, got {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("{{ outs }}")),
        "the finding should name the replacement, got {messages:?}"
    );
}

#[test]
fn legacy_placeholder_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"cp $in $out\"  # netsuke-lint: allow legacy-placeholder -- migrating\n",
    );
    assert_silent(yaml, "legacy-placeholder");
}
