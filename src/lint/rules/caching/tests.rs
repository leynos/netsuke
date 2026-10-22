//! Tests for the caching rules.

use rstest::rstest;

#[rstest]
#[case("    deps: schema\n")]
#[case("    sources: schema\n")]
fn phony_dep_of_file_target_reports_an_always_dirty_dependency(#[case] dependency: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: schema\n",
            "    phony: true\n",
            "    command: \"generate-schema\"\n",
            "  - name: api.json\n",
            "{}",
            "    command: \"render > {{{{ outs }}}}\"\n",
        ),
        dependency
    );
    crate::assert_lint_fires!(&yaml, "phony-dep-of-file-target", 1);
}

/// An order-only edge sequences without tracking, and a phony target may
/// depend on another phony target freely.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: schema\n",
    "    phony: true\n",
    "    command: \"generate-schema\"\n",
    "  - name: api.json\n",
    "    order_only_deps: schema\n",
    "    command: \"render > {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: schema\n",
    "    phony: true\n",
    "    command: \"generate-schema\"\n",
    "  - name: all\n",
    "    phony: true\n",
    "    deps: schema\n",
    "    command: \":\"\n",
))]
fn phony_dep_of_file_target_accepts_correct_shapes(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "phony-dep-of-file-target");
}

/// A plain edge onto a directory target is a content dependency, whether the
/// directory's recipe is inline or comes from a shared `rule:` selector.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: build\n",
    "    command: \"mkdir -p {{ outs }}\"\n",
    "  - name: build/report.txt\n",
    "    deps: build\n",
    "    command: \"report > {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: make_dir\n",
    "    command: \"mkdir -p {{ outs }}\"\n",
    "targets:\n",
    "  - name: build\n",
    "    rule: make_dir\n",
    "  - name: build/report.txt\n",
    "    deps: build\n",
    "    command: \"report > {{ outs }}\"\n",
))]
fn directory_dep_not_order_only_reports_a_content_dependency(#[case] yaml: &str) {
    crate::assert_lint_fires!(yaml, "directory-dep-not-order-only", 1);
}

/// An order-only edge is the fix, and a recipe that writes a real output is
/// not a directory target even when it starts by creating one.
#[rstest]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: build\n",
    "    command: \"mkdir -p {{ outs }}\"\n",
    "  - name: build/report.txt\n",
    "    order_only_deps: build\n",
    "    command: \"report > {{ outs }}\"\n",
))]
#[case(concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: staged.txt\n",
    "    command: \"mkdir -p build && stage > {{ outs }}\"\n",
    "  - name: out.txt\n",
    "    deps: staged.txt\n",
    "    command: \"cp staged.txt {{ outs }}\"\n",
))]
fn directory_dep_not_order_only_accepts_correct_shapes(#[case] yaml: &str) {
    crate::assert_lint_silent!(yaml, "directory-dep-not-order-only");
}

/// A suppression directive above the dependent target silences each caching
/// rule for that target.
#[rstest]
#[case(
    concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: schema\n",
        "    phony: true\n",
        "    command: \"generate-schema\"\n",
        "  # netsuke-lint: allow phony-dep-of-file-target -- the schema really is an input\n",
        "  - name: api.json\n",
        "    deps: schema\n",
        "    command: \"render > {{ outs }}\"\n",
    ),
    "phony-dep-of-file-target"
)]
#[case(
    concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: build\n",
        "    command: \"mkdir -p {{ outs }}\"\n",
        "  # netsuke-lint: allow directory-dep-not-order-only -- the listing really is an input\n",
        "  - name: build/report.txt\n",
        "    deps: build\n",
        "    command: \"report > {{ outs }}\"\n",
    ),
    "directory-dep-not-order-only"
)]
fn a_directive_suppresses_each_caching_rule(#[case] yaml: &str, #[case] rule: &str) {
    crate::assert_lint_silent!(yaml, rule);
}
