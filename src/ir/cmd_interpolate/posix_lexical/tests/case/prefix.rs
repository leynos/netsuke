//! Unit tests for `case` keywords behind a compound-command prefix.
//!
//! Bash keeps a command position open after the `time` options and after a
//! `coproc` name, so a `case` written there is the command word of the
//! substitution even though it is not the first. These cases pin the forms the
//! tracker models and the fail-closed boundary for the forms it cannot read.

use super::super::*;

use rstest::rstest;

// Verify a `case` behind a `time` option keeps the substitution open.
//
// Bash's `time` takes `-p` before its command, so the `case` there is a
// reserved word even though it is not the first word. A tracker that completes
// the command at `-p` reads the pattern terminator as the substitution closer
// and lowers the arm's marker for the enclosing double-quote context, which is
// the reported defect in one more spelling.
#[test]
fn case_behind_a_time_option_keeps_the_substitution_open() {
    let template = format!("echo \"$(time -p case x in x) echo {INS_TOKEN} ;; esac)\"");
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("a marker in a time-prefixed case arm must stay protected");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify a `case` behind a `coproc` name keeps the substitution open.
//
// `coproc NAME command` names the coprocess before the command word, so the
// `case` after the name is again that command. Missing the name slot has the
// same consequence as missing the `time` option.
#[test]
fn case_behind_a_coproc_name_keeps_the_substitution_open() {
    let template = format!("echo \"$(coproc jobs case x in x) echo {INS_TOKEN} ;; esac)\"");
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("a marker in a named coproc's case arm must stay protected");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify a quoted `coproc` name cannot hide a `case` from the guard.
//
// Bash accepts a quoted name, and a quoted word is not word data, so the
// tracker cannot tell whether the next word is the name or the command. Rather
// than guess, the boundary check fails closed: the `case` is a whole-word
// keyword with its `in` before the closer, and a marker follows it.
#[test]
fn quoted_coproc_name_before_a_case_fails_closed() {
    let template = format!("echo \"$(coproc \"jobs\" case x in x) echo {INS_TOKEN} ;; esac)\"");
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("a case behind a quoted coproc name must fail closed");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify `case` passed as an argument is not mistaken for a construct.
//
// The fail-closed check needs a whole-word `in` after the keyword, because the
// shell requires one before the first pattern terminator. Without that
// requirement, ordinary argument data such as `echo case` would be rejected
// even though no substitution boundary was ever in question.
#[test]
fn case_argument_without_a_construct_is_accepted() {
    let template = "echo \"$(echo case)\"";
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(template, &bindings())
                .expect("a 'case' argument with no construct must be accepted"),
            template,
            "argument data must not fail closed: {template}"
        );
    }
}

// Verify a `time`-prefixed case without markers is still interpolated.
//
// Crossing a `time` option or a `coproc` name must only extend command
// position. A substitution with no marker keeps its text, so recognizing the
// prefix cannot reject a supported script.
#[rstest]
#[case::time_option("echo \"$(time -p case x in x) printf y ;; esac)\"")]
#[case::coproc_name("echo \"$(coproc jobs case x in x) printf y ;; esac)\"")]
fn prefixed_case_substitution_without_markers_is_accepted(#[case] template: &str) {
    let bindings = hostile_bash_bindings();
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(template, &bindings)
                .expect("a time- or coproc-prefixed case is supported Bash syntax"),
            template,
            "the prefixed case substitution must be left unchanged: {template}"
        );
    }
}

// Verify a marker after a prefixed case substitution is lowered normally.
//
// Recognizing the `time` option and the `coproc` name must end at the real
// closer, so the marker that follows is ordinary outer text. Falling back to
// the fail-closed guard here would reject a supported recipe: the guard exists
// for keywords the tracker cannot place, not for the forms it models.
#[rstest]
#[case::time_option("time -p case x in x) printf y ;; esac")]
#[case::coproc_name("coproc jobs case x in x) printf y ;; esac")]
fn marker_after_a_prefixed_case_substitution_is_lowered(#[case] substitution: &str) {
    let template = format!("echo \"$({substitution})\" {OUTS_TOKEN}");
    let expected = format!("echo \"$({substitution})\" output");
    let bindings = bash_bindings();

    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings)
                .expect("a marker after a prefixed case substitution must be lowered"),
            expected,
            "the marker must lower to its path after the real closer"
        );
    }
}
