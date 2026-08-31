//! Tests for the redundant-declaration rules.

use rstest::rstest;

#[test]
fn builtin_clean_action_reports_a_hand_written_clean() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  - name: clean\n",
        "    command: \"rm -f *.o app\"\n",
        "targets: []\n",
    );
    crate::assert_lint_fires!(yaml, "builtin-clean-action", 1);
}

/// Only an action named `clean` duplicates the built-in; a rule or a target of
/// that name does not, and neither does a differently named action.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "actions:\n",
    "  - name: distclean\n",
    "    command: \"rm -rf build\"\n",
    "targets: []\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: clean\n",
    "    command: \"rm -rf {{ outs }}\"\n",
    "targets:\n",
    "  - name: out\n",
    "    rule: clean\n",
))]
fn builtin_clean_action_leaves_other_declarations_alone(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "builtin-clean-action");
}

#[test]
fn builtin_clean_action_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  # netsuke-lint: allow builtin-clean-action -- also clears the vendored cache\n",
        "  - name: clean\n",
        "    command: \"rm -rf build vendor/.cache\"\n",
        "targets: []\n",
    );
    crate::assert_lint_silent!(yaml, "builtin-clean-action");
}

#[rstest]
#[case("")]
#[case("    deps: only\n")]
fn serial_order_without_deps_reports_an_inert_declaration(#[case] deps: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "actions:\n",
            "  - name: only\n",
            "    command: \"one\"\n",
            "  - name: all\n",
            "    dependency_order: serial\n",
            "{}",
            "    command: \"aggregate\"\n",
            "targets: []\n",
        ),
        deps
    );
    crate::assert_lint_fires!(&yaml, "serial-order-without-deps", 1);
}

#[test]
fn serial_order_without_deps_accepts_an_ordered_list() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  - name: one\n",
        "    command: \"one\"\n",
        "  - name: two\n",
        "    command: \"two\"\n",
        "  - name: all\n",
        "    dependency_order: serial\n",
        "    deps:\n",
        "      - one\n",
        "      - two\n",
        "    command: \"aggregate\"\n",
        "targets: []\n",
    );
    crate::assert_lint_silent!(yaml, "serial-order-without-deps");
}

#[test]
fn serial_order_without_deps_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  # netsuke-lint: allow serial-order-without-deps -- dependencies arrive in the next change\n",
        "  - name: all\n",
        "    dependency_order: serial\n",
        "    command: \"aggregate\"\n",
        "targets: []\n",
    );
    crate::assert_lint_silent!(yaml, "serial-order-without-deps");
}

#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "actions:\n",
    "  - name: lint\n",
    "    always: true\n",
    "    command: \"lint\"\n",
    "targets: []\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: check\n",
    "    phony: true\n",
    "    always: true\n",
    "    command: \"check\"\n",
))]
fn redundant_always_reports_always_on_a_phony_target(#[case] yaml: &str) {
    crate::assert_lint_fires!(yaml, "redundant-always", 1);
}

/// The parser accepts YAML 1.1 Boolean spellings for `always` and `phony`.
#[rstest]
#[case("yes", "yes")]
#[case("on", "True")]
fn redundant_always_recognizes_every_boolean_spelling(#[case] always: &str, #[case] phony: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: check\n",
            "    phony: {}\n",
            "    always: {}\n",
            "    command: \"check\"\n",
        ),
        phony, always
    );
    crate::assert_lint_fires!(&yaml, "redundant-always", 1);
}

/// `always` on a file target is the feature working as documented.
#[test]
fn redundant_always_accepts_always_on_a_file_target() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: stamp\n",
        "    always: true\n",
        "    command: \"date > {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "redundant-always");
}

#[test]
fn redundant_always_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  # netsuke-lint: allow redundant-always -- kept while the phony flag is in flux\n",
        "  - name: lint\n",
        "    always: true\n",
        "    command: \"lint\"\n",
        "targets: []\n",
    );
    crate::assert_lint_silent!(yaml, "redundant-always");
}
