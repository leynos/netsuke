//! Tests for the recipe-shape clarity rules.

use rstest::rstest;

#[test]
fn literal_recipe_path_reports_a_repeated_output() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    sources: in.txt\n",
        "    command: \"cp in.txt out.txt\"\n",
    );
    crate::assert_lint_fires!(yaml, "literal-recipe-path", 2);
    assert_eq!(
        crate::lint_spans!(yaml, "literal-recipe-path"),
        vec!["in.txt", "out.txt"]
    );
}

/// A placeholder is the fix, a substring is not a path, a templated name is
/// not literal, and a rule's `name` identifies the rule rather than a file.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    sources: in.txt\n",
    "    command: \"cp {{ ins }} {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"touch out.txt.bak && cp x {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  stem: out\n",
    "targets:\n",
    "  - name: \"{{ stem }}.txt\"\n",
    "    command: \"echo hi > {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: mkdir\n",
    "    command: \"mkdir -p {{ outs }}\"\n",
    "targets:\n",
    "  - name: build\n",
    "    rule: mkdir\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: check\n",
    "    phony: true\n",
    "    command: \"run check\"\n",
))]
fn literal_recipe_path_leaves_correct_manifests_alone(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "literal-recipe-path");
}

/// The parser accepts YAML 1.1 Boolean spellings, so a rule that only
/// recognized `true` would start reporting valid phony targets.
#[rstest]
#[case("yes")]
#[case("on")]
#[case("True")]
fn literal_recipe_path_accepts_every_phony_spelling(#[case] spelling: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: check\n",
            "    phony: {}\n",
            "    command: \"run check\"\n",
        ),
        spelling
    );
    crate::assert_lint_silent!(&yaml, "literal-recipe-path");
}

#[test]
fn literal_recipe_path_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow literal-recipe-path -- the tool insists on the bare name\n",
        "  - name: out.txt\n",
        "    command: \"generate --into out.txt\"\n",
    );
    crate::assert_lint_silent!(yaml, "literal-recipe-path");
}

#[test]
fn command_chain_not_list_reports_a_scalar_chain() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"build && test && package\"\n",
    );
    crate::assert_lint_fires!(yaml, "command-chain-not-list", 1);
    let messages = crate::lint_messages!(yaml, "command-chain-not-list");
    assert!(
        messages.iter().any(|message| message.contains("3 steps")),
        "the finding should count the steps, got {messages:?}"
    );
}

/// The list form is the fix, and an `&&` inside a shell quote is text.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out\n",
    "    command:\n",
    "      - \"build\"\n",
    "      - \"test\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out\n",
    "    command: \"echo 'a && b'\"\n",
))]
fn command_chain_not_list_leaves_lists_and_quoted_text_alone(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "command-chain-not-list");
}

#[test]
fn command_chain_not_list_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"build && test\"  # netsuke-lint: allow command-chain-not-list -- one logical step\n",
    );
    crate::assert_lint_silent!(yaml, "command-chain-not-list");
}
