//! Tests for the portability rules.

use rstest::rstest;

use crate::lint::test_support::{assert_fires, assert_silent, messages_for};

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
    assert_fires(&script(body), "bashism", 1);
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
    assert_silent(&script(body), "bashism");
}

#[test]
fn bashism_names_the_portable_alternative() {
    let messages = messages_for(&script("if [[ -f in ]]; then :; fi"), "bashism");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("POSIX operators")),
        "the finding should name the alternative, got {messages:?}"
    );
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
    assert_silent(yaml, "bashism");
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
    assert_fires(yaml, "bashism", 1);
}
