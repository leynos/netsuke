//! Tests for the portability rules.

use rstest::rstest;

/// Build a one-target manifest whose script body is `body`.
fn script(body: &str) -> String {
    format!("netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    script: |\n      {body}\n")
}

#[rstest]
#[case("if [[ -f in ]]; then cp in out; fi")]
#[case("function build { cp in out; }")]
#[case("source ./env.sh")]
#[case("local value=1")]
#[case("cat <<< \"text\"")]
#[case("cmd &> log")]
#[case("cmd |& tee log")]
#[case("set -o pipefail")]
#[case("echo -e 'a\\tb'")]
fn bashism_reports_constructs_sh_does_not_promise(#[case] body: &str) {
    crate::assert_lint_fires!(&script(body), "bashism", 1);
}

/// Word-bounded tokens must not match inside a longer word, and a construct
/// inside a shell quote is text rather than syntax.
#[rstest]
#[case("if [ -f in ]; then cp in out; fi")]
#[case("./sourcemap --check")]
#[case("localise --input in")]
#[case("printf '%s' 'function build'")]
#[case("echo 'set -o pipefail'")]
fn bashism_leaves_portable_and_quoted_text_alone(#[case] body: &str) {
    crate::assert_lint_silent!(&script(body), "bashism");
}

/// A shell keyword is only a keyword in command position.
///
/// `function`, `source`, and `local` are ordinary arguments elsewhere, so
/// matching them anywhere reported search patterns and file names as
/// non-portable syntax.
#[rstest]
#[case("grep function main.c")]
#[case("find . -name local")]
#[case("grep -l source *.c")]
#[case("cc -o source main.c")]
fn bashism_ignores_keywords_outside_command_position(#[case] body: &str) {
    crate::assert_lint_silent!(&script(body), "bashism");
}

/// The same words in command position are still reported.
#[rstest]
#[case("source ./env.sh")]
#[case("local value=1")]
#[case("function build { cp in out; }")]
fn bashism_still_reports_keywords_in_command_position(#[case] body: &str) {
    crate::assert_lint_fires!(&script(body), "bashism", 1);
}

#[test]
fn bashism_names_the_portable_alternative() {
    let messages = crate::lint_messages!(&script("if [[ -f in ]]; then :; fi"), "bashism");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("POSIX operators")),
        "the finding should name the alternative, got {messages:?}"
    );
}

/// A rule must see every line of a multi-line script, not just the first.
#[test]
fn bashism_reports_a_construct_below_the_first_script_line() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    script: |\n",
        "      echo start\n",
        "      if [[ -f in ]]; then cp in out; fi\n",
    );
    crate::assert_lint_fires!(yaml, "bashism", 1);
}

#[test]
fn bashism_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow bashism -- this project only builds on bash hosts\n",
        "  - name: out\n",
        "    script: |\n",
        "      if [[ -f in ]]; then cp in out; fi\n",
    );
    crate::assert_lint_silent!(yaml, "bashism");
}

/// A directive must not reach past the declaration it governs.
#[test]
fn a_directive_does_not_silence_the_next_declaration() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow bashism -- only this one is bash-only\n",
        "  - name: first\n",
        "    script: |\n",
        "      [[ -f in ]]\n",
        "  - name: second\n",
        "    script: |\n",
        "      [[ -f in ]]\n",
    );
    crate::assert_lint_fires!(yaml, "bashism", 1);
}
