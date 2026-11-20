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

// Verify a `coproc` simple command's `esac` still ends the case.
//
// When no compound command follows `coproc`, the next word is that coprocess's
// command rather than a name, but Bash still reads the following `esac` as the
// case terminator: `case x in x) coproc printf esac` is accepted, and the `)`
// that follows closes the substitution. Keeping command position open after the
// command word is therefore what matches the shell here, and the marker after
// the substitution must lower as ordinary outer text.
#[test]
fn coproc_simple_command_keeps_the_case_terminator_a_keyword() {
    let template = format!("echo \"$(case x in x) coproc printf esac)\" {OUTS_TOKEN}");
    let expected = "echo \"$(case x in x) coproc printf esac)\" output";

    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bash_bindings())
                .expect("Bash accepts this case and closes the substitution after it"),
            expected,
            "the marker after the real closer must lower to its path"
        );
    }
}

// Verify `esac` after `|` is pattern text, not the construct's terminator.
//
// Bash reads `esac` as the terminator only where a pattern would start, so a
// `|` makes it an alternative pattern instead: `case x in x|esac)` is valid and
// the `)` after it still terminates the pattern. A tracker that closes the
// construct at that `esac` reads the pattern terminator as the substitution
// closer and lowers the arm's marker for the enclosing double quotes, which is
// the reported defect in one more spelling. The subject may itself be `esac`,
// which must be read as header data rather than as a closer.
#[rstest]
#[case::subject_matches("case x in x|esac) printf %s")]
#[case::esac_subject("case esac in x|esac) printf %s")]
fn esac_after_a_pattern_alternative_keeps_the_substitution_open(#[case] prefix: &str) {
    let template = format!("echo \"$({prefix} {INS_TOKEN} ;; esac)\"");

    for interpolate in both_interpolators() {
        let hostile = interpolate(&template, &hostile_bash_bindings())
            .expect_err("a marker in the arm must stay encoded for the substitution");
        assert!(
            matches!(hostile, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {hostile:?}"
        );
        // A marker inside a substitution is refused even for a benign path, so
        // this fails too if the construct closed at the `esac` and the marker
        // was lowered as outer text.
        let benign = interpolate(&template, &bash_bindings())
            .expect_err("a marker inside the substitution must stay inside it");
        assert!(
            matches!(benign, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered as outer text: {benign:?}"
        );
    }
}

// Verify a pattern group does not hide the construct from the tracker.
//
// A pattern may be spelled as an extglob group, and Bash accepts `@(case)` as
// a pattern in its own right once `extglob` is enabled: the `)` that closes the
// group is followed by the pattern terminator. The word inside the group is
// pattern text, so it must not open a nested construct whose deeper depth hides
// the outer one. The group's `(` is consumed as a substitution delimiter rather
// than word data, which also leaves the keyword outside command position, so
// the tracker and the fail-closed guard refuse this shape together; the point
// pinned here is that the arm's marker is never lowered as outer text.
#[test]
fn extglob_pattern_keeps_the_substitution_open() {
    let template = format!("echo \"$(case x in @(case)) printf %s {INS_TOKEN} ;; esac)\"");
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("a marker in the arm must stay encoded for the substitution");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
        assert!(
            interpolate(&template, &bash_bindings()).is_err(),
            "a marker inside the substitution must stay inside it"
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

// Verify a line-continued keyword behind a quoted `coproc` name fails closed.
//
// Bash deletes a backslash-newline pair before parsing, so `c` + continuation +
// `ase` spells the keyword and `i` + continuation + `n` completes the header.
// The tracker reads both spellings; the raw-source check must read them the same
// way, or an unplaced `case` goes unrecognized and the arm's marker is lowered
// for the outer quote context.
#[rstest]
#[case::continued_case("coproc \"jobs\" c\\\nase x in x)")]
#[case::continued_in("coproc \"jobs\" case x i\\\nn x)")]
fn continued_keywords_behind_a_quoted_coproc_name_fail_closed(#[case] substitution: &str) {
    let template = format!("echo \"$({substitution} echo {INS_TOKEN} ;; esac)\"");
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings).expect_err(
            "a line-continued case header behind a quoted coproc name must fail closed",
        );
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify a continued `case` used as argument data is not rejected.
//
// The guard still needs a whole-word `in` after the keyword, so argument data
// that merely spells the keyword with a continuation must keep interpolating.
// Rejecting it would make the continuation-aware scan stricter than the
// construct it is guarding.
#[test]
fn continued_case_argument_with_a_later_marker_is_accepted() {
    let template = format!("echo \"$(echo c\\\nase)\" {OUTS_TOKEN}");
    let expected = "echo \"$(echo c\\\nase)\" output";
    let bindings = bash_bindings();

    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings)
                .expect("continued argument data must not fail closed"),
            expected,
            "the marker after the substitution must lower to its path"
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
