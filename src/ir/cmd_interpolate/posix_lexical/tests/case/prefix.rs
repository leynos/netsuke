//! Unit tests for `case` keywords behind a compound-command prefix.
//!
//! Bash keeps a command position open after the `time` options and after a
//! `coproc` name, so a `case` written there is the command word of the
//! substitution even though it is not the first. These cases pin the forms the
//! tracker models and the fail-closed boundary for the forms it cannot read.

use super::super::*;

use rstest::rstest;

// Verify a `case` behind a compound-command prefix keeps the substitution open.
//
// Bash keeps a command position open after the `time` options and after a
// `coproc` name, so the `case` written there is the substitution's command word
// even though it is not the first. A tracker that completes the command at the
// prefix reads the pattern terminator as the substitution closer and lowers the
// arm's marker for the enclosing double-quote context. Each case names one
// prefix spelling: `time` with an option, `coproc` with a name, and a quoted
// `coproc` name or a keyword hidden behind a line continuation, which the word
// tracker cannot read and the fail-closed guard must reject instead.
#[rstest]
#[case::time_option(
    "time -p case x in x) echo {INS_TOKEN} ;; esac)",
    "a marker in a time-prefixed case arm must stay protected"
)]
#[case::coproc_name(
    "coproc jobs case x in x) echo {INS_TOKEN} ;; esac)",
    "a marker in a named coproc's case arm must stay protected"
)]
#[case::quoted_coproc_name(
    "coproc \"jobs\" case x in x) echo {INS_TOKEN} ;; esac)",
    "a case behind a quoted coproc name must fail closed"
)]
#[case::continued_case_keyword(
    "coproc \"jobs\" c\\\nase x in x) echo {INS_TOKEN} ;; esac)",
    "a line-continued case header behind a quoted coproc name must fail closed"
)]
#[case::continued_in_keyword(
    "coproc \"jobs\" case x i\\\nn x) echo {INS_TOKEN} ;; esac)",
    "a line-continued case header behind a quoted coproc name must fail closed"
)]
#[case::escaped_backslash(
    "coproc \"jobs\" case \\\\\nin x) echo {INS_TOKEN} ;; esac)",
    "an escaped backslash must not hide the header from the guard"
)]
fn case_behind_a_compound_command_prefix_keeps_the_substitution_open(
    #[case] substitution: &str,
    #[case] context: &str,
) {
    let template = format!("echo \"$({substitution})\"").replace("{INS_TOKEN}", INS_TOKEN);
    let bindings = hostile_bash_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings).expect_err(context);
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

// Verify only the first word after `coproc` fills the name slot.
//
// `coproc NAME command ARGS` names the coprocess once; the arguments of the
// command that follows are ordinary data. A tracker that re-armed the slot on
// every argument kept command position open, so the argument `esac` closed the
// case early and the following pattern terminator closed the substitution,
// lowering the next arm's marker for the enclosing double quotes.
#[rstest]
#[case::argument_spells_esac(
    "case y in x) coproc printf x esac ;; y) printf %s {INS_TOKEN} ;; esac)"
)]
#[case::group_after_coproc(
    "case y in x) coproc (echo case) esac ;; y) printf %s {INS_TOKEN} ;; esac)"
)]
fn coproc_arguments_do_not_keep_command_position_open(#[case] substitution: &str) {
    let template = format!("echo \"$({substitution})\"").replace("{INS_TOKEN}", INS_TOKEN);
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

// Verify pattern-text spellings of `esac` keep the substitution open.
//
// Bash recognizes a reserved word only when it is genuinely unquoted and
// unescaped, and only where a pattern would start. These cases put `esac` where
// the shell reads it as pattern text: as a quoted or escaped pattern, an
// alternative after `|`, an extglob group, and a quoted command word in an
// arm's list. A tracker that closed the construct at any of them would read the
// next pattern terminator as the substitution closer and lower the arm's
// marker for the enclosing double quotes. A backslash-newline pair is spliced
// before tokenizing, so only a quote or an escape of a real character makes the
// word pattern text; that continued spelling is pinned by the closer test
// below.
#[rstest]
#[case::quoted_suffix("case x in esac\"\") printf %s {INS_TOKEN} ;; esac)")]
#[case::leading_quote("case x in \"\"esac) printf %s {INS_TOKEN} ;; esac)")]
#[case::escaped_letter("case x in e\\sac) printf %s {INS_TOKEN} ;; esac)")]
#[case::alternative("case x in x|esac) printf %s {INS_TOKEN} ;; esac)")]
#[case::esac_subject_alternative("case esac in x|esac) printf %s {INS_TOKEN} ;; esac)")]
#[case::extglob_group("case x in @(case)) printf %s {INS_TOKEN} ;; esac)")]
#[case::quoted_esac_in_arm("case x in x) esac\"\" foo ;; y) printf %s {INS_TOKEN} ;; esac)")]
fn esac_as_pattern_text_keeps_the_substitution_open(#[case] substitution: &str) {
    let template = format!("echo \"$({substitution})\"").replace("{INS_TOKEN}", INS_TOKEN);
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

// Verify a line-continued `esac` still closes the case.
//
// Bash deletes a backslash-newline pair before tokenizing, so `es\<newline>ac`
// is the construct's terminator rather than pattern text. Treating every escape
// as non-literal would leave the construct open past the real closer and reject
// a marker that the shell reads as ordinary outer text.
#[test]
fn continued_esac_still_closes_the_case() {
    let template = format!("echo \"$(case x in x) printf y ;; es\\\nac)\" {OUTS_TOKEN}");
    let expected = "echo \"$(case x in x) printf y ;; es\\\nac)\" output";
    let bindings = bash_bindings();

    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings)
                .expect("Bash reads the continued esac as the case terminator"),
            expected,
            "the marker after the real closer must lower to its path"
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
