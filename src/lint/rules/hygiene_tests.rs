//! Tests for the hygiene rules.

use rstest::rstest;

#[test]
fn unused_var_reports_a_variable_no_template_mentions() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "vars:\n",
        "  used: hello\n",
        "  spare: world\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"echo {{ used }} > {{ outs }}\"\n",
    );
    crate::assert_lint_fires!(yaml, "unused-var", 1);
    assert_eq!(crate::lint_spans!(yaml, "unused-var"), vec!["spare"]);
}

/// A variable is used when any template mentions it, including a `foreach`
/// list, a `when` expression, or a macro body.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  items:\n",
    "    - a\n",
    "targets:\n",
    "  - foreach: items\n",
    "    name: \"{{ item }}\"\n",
    "    command: \"touch {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  enabled: true\n",
    "targets:\n",
    "  - name: out\n",
    "    when: enabled\n",
    "    command: \"touch {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  greeting: hi\n",
    "macros:\n",
    "  - signature: \"greet(name)\"\n",
    "    body: \"{{ greeting }}, {{ name }}\"\n",
    "targets:\n",
    "  - name: out\n",
    "    command: \"echo '{{ greet('world') }}' > {{ outs }}\"\n",
))]
fn unused_var_accepts_every_kind_of_reference(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "unused-var");
}

#[test]
fn unused_var_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "vars:\n",
        "  # netsuke-lint: allow unused-var -- consumed by a downstream include\n",
        "  spare: world\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "unused-var");
}

#[test]
fn unused_macro_reports_a_macro_nothing_calls() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "macros:\n",
        "  - signature: \"greet(name)\"\n",
        "    body: \"hello, {{ name }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_fires!(yaml, "unused-macro", 1);
}

#[test]
fn unused_macro_accepts_a_called_macro() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "macros:\n",
        "  - signature: \"greet(name)\"\n",
        "    body: \"hello, {{ name }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"echo '{{ greet('world') }}' > {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "unused-macro");
}

#[test]
fn unused_macro_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "macros:\n",
        "  # netsuke-lint: allow unused-macro -- kept for the next chapter\n",
        "  - signature: \"greet(name)\"\n",
        "    body: \"hello, {{ name }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "unused-macro");
}

#[test]
fn unused_rule_reports_a_rule_nothing_references() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: spare\n",
        "    command: \"work {{ outs }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_fires!(yaml, "unused-rule", 1);
}

/// A rule referenced only by a `foreach`-generated target counts as used,
/// which is why the rule binds to the expanded manifest.
#[test]
fn unused_rule_accepts_a_rule_a_generated_target_uses() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "vars:\n",
        "  items:\n",
        "    - a\n",
        "rules:\n",
        "  - name: touch\n",
        "    command: \"touch {{ outs }}\"\n",
        "targets:\n",
        "  - foreach: items\n",
        "    name: \"{{ item }}\"\n",
        "    rule: touch\n",
    );
    crate::assert_lint_silent!(yaml, "unused-rule");
}

/// A dead rule must not keep its delegate alive.
///
/// Neither rule contributes a build edge, so both are unused; crediting the
/// reference that `wrapper` makes would report only `wrapper` and hide the
/// real orphan.
#[test]
fn unused_rule_reports_a_rule_only_another_dead_rule_references() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: wrapper\n",
        "    rule: compile\n",
        "  - name: compile\n",
        "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_fires!(yaml, "unused-rule", 2);
}

/// A rule delegation cycle must terminate rather than hang.
#[test]
fn unused_rule_terminates_on_a_delegation_cycle() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: first\n",
        "    rule: second\n",
        "  - name: second\n",
        "    rule: first\n",
        "targets:\n",
        "  - name: out\n",
        "    rule: first\n",
    );
    crate::assert_lint_silent!(yaml, "unused-rule");
}

#[test]
fn unused_rule_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  # netsuke-lint: allow unused-rule -- referenced by a generated overlay\n",
        "  - name: spare\n",
        "    command: \"work {{ outs }}\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "unused-rule");
}
