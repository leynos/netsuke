//! Tests for the determinism rules.

use rstest::rstest;

use crate::lint::test_support::{assert_fires, assert_silent, spans_for};

/// Build a one-target manifest whose command is `command`.
fn manifest(command: &str) -> String {
    format!("netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    command: {command}\n")
}

#[rstest]
#[case("\"feh preview &\"")]
#[case("\"server --port 8080 & \"")]
fn background_job_reports_a_detached_process(#[case] command: &str) {
    assert_fires(&manifest(command), "background-job", 1);
}

/// A `&` that joins commands or redirects a stream is not backgrounding.
#[rstest]
#[case("\"build && test\"")]
#[case("\"cmd > log 2>&1\"")]
#[case("\"echo 'a & b'\"")]
#[case("\"cmd 2>&1 | tee log\"")]
fn background_job_leaves_joining_and_redirecting_alone(#[case] command: &str) {
    assert_silent(&manifest(command), "background-job");
}

#[test]
fn background_job_points_at_the_ampersand() {
    assert_eq!(
        spans_for(&manifest("\"feh preview &\""), "background-job"),
        vec!["&"]
    );
}

#[test]
fn background_job_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow background-job -- the viewer must outlive the build\n",
        "  - name: out\n",
        "    command: \"feh preview &\"\n",
    );
    assert_silent(yaml, "background-job");
}

#[rstest]
#[case("\"make -C vendor all\"")]
#[case("\"ninja -C build\"")]
#[case("\"netsuke build\"")]
#[case("\"cd vendor && make all\"")]
#[case("\"/usr/bin/make all\"")]
fn recursive_build_invocation_reports_a_nested_build(#[case] command: &str) {
    assert_fires(&manifest(command), "recursive-build-invocation", 1);
}

/// A longer program name only starts with a build tool's name, and a tool
/// named as an argument is not being invoked.
#[rstest]
#[case("\"makeinfo manual.texi\"")]
#[case("\"cmake --build build\"")]
#[case("\"echo 'run make first'\"")]
#[case("\"cc -c src/main.c\"")]
fn recursive_build_invocation_leaves_other_programs_alone(#[case] command: &str) {
    assert_silent(&manifest(command), "recursive-build-invocation");
}

#[test]
fn recursive_build_invocation_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    command: \"make -C vendor all\"  # netsuke-lint: allow recursive-build-invocation -- vendored third-party build\n",
    );
    assert_silent(yaml, "recursive-build-invocation");
}
