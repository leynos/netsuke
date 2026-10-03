//! Tests for the duplication rules.

use rstest::rstest;

#[test]
fn duplicate_rule_recipe_reports_the_second_of_two_identical_rules() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: compile\n",
        "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
        "  - name: compile_test\n",
        "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
        "targets:\n",
        "  - name: a.o\n",
        "    rule: compile\n",
        "  - name: b.o\n",
        "    rule: compile_test\n",
    );
    crate::assert_lint_fires!(yaml, "duplicate-rule-recipe", 1);
    let messages = crate::lint_messages!(yaml, "duplicate-rule-recipe");
    assert!(
        messages.iter().any(|message| message.contains("`compile`")),
        "the finding should name the rule it duplicates, got {messages:?}"
    );
}

/// Different recipes, and the same text under different recipe keys, are not
/// duplicates.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: compile\n",
    "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
    "  - name: link\n",
    "    command: \"cc {{ ins }} -o {{ outs }}\"\n",
    "targets:\n",
    "  - name: a.o\n",
    "    rule: compile\n",
    "  - name: app\n",
    "    rule: link\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: as_command\n",
    "    command: \"work\"\n",
    "  - name: as_script\n",
    "    script: \"work\"\n",
    "targets:\n",
    "  - name: a\n",
    "    rule: as_command\n",
    "  - name: b\n",
    "    rule: as_script\n",
))]
fn duplicate_rule_recipe_leaves_distinct_rules_alone(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "duplicate-rule-recipe");
}

#[test]
fn duplicate_rule_recipe_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: compile\n",
        "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
        "  # netsuke-lint: allow duplicate-rule-recipe -- the two diverge in the next release\n",
        "  - name: compile_test\n",
        "    command: \"cc -c {{ ins }} -o {{ outs }}\"\n",
        "targets:\n",
        "  - name: a.o\n",
        "    rule: compile\n",
        "  - name: b.o\n",
        "    rule: compile_test\n",
    );
    crate::assert_lint_silent!(yaml, "duplicate-rule-recipe");
}

#[rstest]
#[case("    deps: in.txt\n")]
#[case("    order_only_deps: in.txt\n")]
fn redundant_dependency_reports_a_path_declared_twice(#[case] extra: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: in.txt\n",
            "    command: \"touch {{{{ outs }}}}\"\n",
            "  - name: out.txt\n",
            "    sources: in.txt\n",
            "{}",
            "    command: \"cp {{{{ ins }}}} {{{{ outs }}}}\"\n",
        ),
        extra
    );
    crate::assert_lint_fires!(&yaml, "redundant-dependency", 1);
}

#[test]
fn redundant_dependency_accepts_distinct_dependency_keys() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: in.txt\n",
        "    command: \"touch {{ outs }}\"\n",
        "  - name: build\n",
        "    command: \"mkdir -p {{ outs }}\"\n",
        "  - name: out.txt\n",
        "    sources: in.txt\n",
        "    order_only_deps: build\n",
        "    command: \"cp {{ ins }} {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "redundant-dependency");
}

#[test]
fn redundant_dependency_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: in.txt\n",
        "    command: \"touch {{ outs }}\"\n",
        "  # netsuke-lint: allow redundant-dependency -- kept explicit while the graph settles\n",
        "  - name: out.txt\n",
        "    sources: in.txt\n",
        "    deps: in.txt\n",
        "    command: \"cp {{ ins }} {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "redundant-dependency");
}
