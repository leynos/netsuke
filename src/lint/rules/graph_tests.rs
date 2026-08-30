//! Tests for the graph rules.

use rstest::rstest;

#[test]
fn undeclared_target_input_reports_an_undeclared_producer() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: build/main.o\n",
        "    command: \"cc -c src/main.c -o {{ outs }}\"\n",
        "  - name: app\n",
        "    command: \"cc build/main.o -o {{ outs }}\"\n",
    );
    crate::assert_lint_fires!(yaml, "undeclared-target-input", 1);
}

/// A declared edge, a transitively reachable one, and a path that only appears
/// as a substring are all correct.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: build/main.o\n",
    "    command: \"cc -c src/main.c -o {{ outs }}\"\n",
    "  - name: app\n",
    "    sources: build/main.o\n",
    "    command: \"cc {{ ins }} -o {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: build\n",
    "    command: \"mkdir -p {{ outs }}\"\n",
    "  - name: build/main.o\n",
    "    order_only_deps: build\n",
    "    command: \"cc -c src/main.c -o {{ outs }}\"\n",
    "  - name: app\n",
    "    sources: build/main.o\n",
    "    command: \"cc {{ ins }} -o build/app && mv build/app {{ outs }}\"\n",
))]
fn undeclared_target_input_accepts_declared_and_reachable_paths(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "undeclared-target-input");
}

#[test]
fn undeclared_target_input_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: build/main.o\n",
        "    command: \"cc -c src/main.c -o {{ outs }}\"\n",
        "  # netsuke-lint: allow undeclared-target-input -- the linker resolves it lazily\n",
        "  - name: app\n",
        "    command: \"cc build/main.o -o {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "undeclared-target-input");
}

/// A manifest with one target reachable from the defaults and one not.
const UNREACHABLE: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: app\n",
    "    command: \"build > {{ outs }}\"\n",
    "  - name: scratch.txt\n",
    "    command: \"echo scratch > {{ outs }}\"\n",
    "defaults:\n",
    "  - app\n",
);

/// Building a target by name without declaring it a default is supported, so
/// the rule must stay silent until a project selects it.
#[test]
fn unreachable_target_is_off_by_default() {
    let reported: Vec<&str> = crate::lint_fixture!(UNREACHABLE)
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect();
    assert!(
        !reported.contains(&"unreachable-target"),
        "an opt-in rule should not run by default, got {reported:?}"
    );
}

#[test]
fn unreachable_target_reports_when_selected() {
    assert_eq!(crate::lint_count!(UNREACHABLE, "unreachable-target"), 1);
}

/// A manifest without defaults has no entry point to measure against, so every
/// target is an entry point and nothing is unreachable.
#[test]
fn unreachable_target_accepts_a_manifest_without_defaults() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: app\n",
        "    command: \"build > {{ outs }}\"\n",
        "  - name: scratch.txt\n",
        "    command: \"echo scratch > {{ outs }}\"\n",
    );
    crate::assert_lint_silent!(yaml, "unreachable-target");
}

#[test]
fn unreachable_target_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: app\n",
        "    command: \"build > {{ outs }}\"\n",
        "  # netsuke-lint: allow unreachable-target -- built by hand during releases\n",
        "  - name: scratch.txt\n",
        "    command: \"echo scratch > {{ outs }}\"\n",
        "defaults:\n",
        "  - app\n",
    );
    crate::assert_lint_silent!(yaml, "unreachable-target");
}
