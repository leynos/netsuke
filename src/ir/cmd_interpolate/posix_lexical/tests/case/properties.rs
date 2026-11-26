//! Property tests for the `case` tracker's security invariant.
//!
//! The fixed regressions beside this module pin the shapes each review round
//! found. This module states the invariant those shapes are instances of: a
//! marker written inside an open command substitution is never lowered for the
//! enclosing quote context, because the shell still reads it as substitution
//! source. The generated `case` grammar exercises that invariant across the
//! subjects, optional openers, patterns, and command prefixes the tracker
//! models.
//!
//! The property is deliberately an accept-or-refuse assertion rather than an
//! exact-output oracle. The scanner may over-approximate and refuse a recipe it
//! could have accepted; it may not accept one whose marker escapes the
//! substitution. Because that permits refusing everything, the sweep below also
//! counts preserved marker-free recipes, so a vacuous pass fails.

use proptest::prelude::*;

use super::super::*;

/// Subjects the header must read as data rather than as its `in` keyword.
const SUBJECTS: [&str; 5] = ["x", "in", "\"x\"", "x\"y\"in", "\"in\""];

/// Pattern lists, each paired with the optional opener Bash permits before it.
///
/// A bare `esac` is only a pattern when that opener precedes it: written
/// without one it terminates the construct, and the `)` after it is a syntax
/// error rather than a pattern terminator. Pairing the two keeps every entry
/// inside the shell grammar the invariant claims to cover.
const PATTERNS: [(&str, &str); 7] = [
    ("", "x"),
    ("", "\"esac\""),
    ("", "x|esac"),
    ("", "in"),
    ("(", "x"),
    ("(", "esac"),
    ("(", "in"),
];

/// Command lists for one arm, with `@MARK@` standing in for a marker.
///
/// The prefixes cover the positions where the tracker must keep command
/// position open or read a keyword as data: a reserved-word prefix, a subshell,
/// a brace group, and a marker glued to quoted text. A `coproc` whose command
/// word is `printf` cannot appear here because its argument `esac` terminates
/// the construct, so nothing may follow it; that shape is pinned by a fixed
/// regression in the prefix tests instead.
const ARMS: [&str; 6] = [
    "printf %s @MARK@",
    ": @MARK@",
    "(printf %s @MARK@)",
    "time printf %s @MARK@",
    "{ printf %s @MARK@; }",
    "printf %s \"x\"@MARK@",
];

/// Separators the tracker reads as ending an arm.
const SEPARATORS: [&str; 2] = [";;", ";;&"];

/// Placeholder the tables leave for the text under test.
const MARKER: &str = "@MARK@";

/// Spellings of `esac` the shell reads as pattern text rather than a closer.
///
/// A quote or an escape of a real character makes the word non-literal, so the
/// shell keeps it as data and the construct survives it. A backslash-newline
/// pair is deliberately absent: that pair is spliced before tokenizing, so the
/// continued spelling still closes the construct, and the closer test below
/// pins it.
const QUOTED_ESAC_SPELLINGS: [&str; 4] = ["esac\"\"", "\"\"esac", "'esac'", "e\\sac"];

/// One point in the generated `case` grammar.
#[derive(Clone, Debug)]
struct CaseShape {
    /// The word between `case` and `in`.
    subject: &'static str,
    /// The optional `(` Bash permits before a pattern list, or the empty string.
    opener: &'static str,
    /// The pattern list up to, but not including, its terminator.
    pattern: &'static str,
    /// The command list of the arm the marker sits in.
    arm: &'static str,
    /// The operator that ends the arm.
    separator: &'static str,
}

impl CaseShape {
    /// Spell the `case` construct this shape describes.
    fn body(&self) -> String {
        let Self {
            subject,
            opener,
            pattern,
            arm,
            separator,
        } = self;
        format!("case {subject} in {opener}{pattern}) {arm} {separator} esac")
    }
}

/// Generate a `case` body with a marker inside its arm.
fn case_body_strategy() -> impl Strategy<Value = CaseShape> {
    (
        prop::sample::select(&SUBJECTS[..]),
        prop::sample::select(&PATTERNS[..]),
        prop::sample::select(&ARMS[..]),
        prop::sample::select(&SEPARATORS[..]),
    )
        .prop_map(|(subject, (opener, pattern), arm, separator)| CaseShape {
            subject,
            opener,
            pattern,
            arm,
            separator,
        })
}

/// Build every shape in the generated grammar.
fn generated_shapes() -> Vec<CaseShape> {
    let capacity = SUBJECTS.len() * PATTERNS.len() * ARMS.len() * SEPARATORS.len();
    let mut shapes = Vec::with_capacity(capacity);
    for subject in SUBJECTS {
        for (opener, pattern) in PATTERNS {
            for arm in ARMS {
                shapes.extend(SEPARATORS.iter().map(|separator| CaseShape {
                    subject,
                    opener,
                    pattern,
                    arm,
                    separator,
                }));
            }
        }
    }
    shapes
}

/// One generated recipe and the marker-free twin it is compared against.
struct TemplatePair {
    /// The hostile spelling, whose marker must never reach the outer context.
    with_marker: String,
    /// The same recipe with benign text where the marker stood.
    without_marker: String,
}

impl TemplatePair {
    /// Wrap a generated body in the double-quoted substitution under test.
    ///
    /// The host's hostile binding encodes its path for the *outer* quote
    /// context, so only a marker the shell reads inside the substitution can be
    /// lowered to that encoding.
    fn new(body: &str) -> Self {
        let render = |replacement: &str| format!("echo \"$({body})\"").replace(MARKER, replacement);
        Self {
            with_marker: render(INS_TOKEN),
            without_marker: render("y"),
        }
    }
}

/// Check one template pair under one set of recipe bindings.
///
/// Fails when an interpolator lowers the marker, since that lowering is the
/// defect the invariant forbids. Otherwise reports whether the marker-free twin
/// was preserved verbatim, which is the evidence that any refusal came from the
/// marker rather than from the surrounding shape.
fn check_pair_with(pair: &TemplatePair, bindings: &CommandBindings) -> Result<bool, String> {
    let mut is_preserved = false;
    for interpolate in both_interpolators() {
        if let Ok(output) = interpolate(&pair.with_marker, bindings) {
            return Err(format!(
                "a marker inside a substitution body must be refused, not lowered \
                 ({:?}): {output:?}",
                pair.with_marker
            ));
        }
        is_preserved |= check_twin(pair, bindings, interpolate)?;
    }
    Ok(is_preserved)
}

/// Report whether one interpolator preserves the marker-free twin verbatim.
///
/// A refusal is reported as "not preserved" rather than as a failure: the
/// scanner may decline a shape it cannot classify, and the sweep only needs to
/// know that some shape survived.
fn check_twin(
    pair: &TemplatePair,
    bindings: &CommandBindings,
    interpolate: Interpolator,
) -> Result<bool, String> {
    match interpolate(&pair.without_marker, bindings) {
        Ok(output) if output == pair.without_marker => Ok(true),
        Ok(output) => Err(format!(
            "a marker-free substitution must keep its text ({:?}): {output:?}",
            pair.without_marker
        )),
        Err(_) => Ok(false),
    }
}

/// Check one template pair under both recipe shells.
///
/// The marker-free twin counts as preserved when either shell accepts it, so a
/// shell-specific refusal does not make the sweep look vacuous.
fn check_pair(pair: &TemplatePair) -> Result<bool, String> {
    let shells = [hostile_posix_bindings(), hostile_bash_bindings()];
    let mut is_preserved = false;
    for bindings in &shells {
        let accepts_pair = check_pair_with(pair, bindings)?;
        is_preserved |= accepts_pair;
    }
    Ok(is_preserved)
}

/// Report whether the word closes the `case` construct.
///
/// The observation is a marker placed after the substitution's `)`, still
/// inside the enclosing double quotes. When the word closed the construct, that
/// `)` is the substitution's closer, so the marker is ordinary outer text and
/// must lower to its path. When the word was pattern text, the construct was
/// still open, so the `)` was read as a pattern terminator and the substitution
/// never closed: the marker is substitution source and the tracker refuses it.
///
/// A marker-free probe cannot tell the two apart, because the shorthand arm
/// `) : ;;` already contains the `)` that ends the pattern list.
fn closes_construct(spelling: &str) -> Result<bool, String> {
    let template = format!("echo \"$(case x in x) : ;; {spelling}){{MARK}}\"");
    let with_marker = template.replace("{MARK}", INS_TOKEN);
    let expected = template.replace("{MARK}", "input");
    let bindings = bindings();
    let mut accepted = 0_usize;
    let mut refused = 0_usize;
    for interpolate in both_interpolators() {
        match interpolate(&with_marker, &bindings) {
            Ok(output) if output == expected => accepted += 1,
            Ok(output) => {
                return Err(format!(
                    "a marker after the closer must lower to its path ({with_marker:?}): \
                     {output:?}"
                ));
            }
            Err(_) => refused += 1,
        }
    }
    if accepted > 0 && refused > 0 {
        return Err(format!(
            "the interpolators disagree about `{spelling}`: {accepted} accept and {refused} refuse"
        ));
    }
    Ok(accepted > 0)
}

proptest! {
    /// Never lower a marker that the shell reads inside a `$()` body.
    ///
    /// Refusal is always acceptable here; the property fails only when a
    /// lowering succeeds while the marker sits inside the substitution.
    #[test]
    fn markers_inside_a_case_substitution_are_never_lowered_for_the_outer_context(
        shape in case_body_strategy()
    ) {
        if let Err(reason) = check_pair(&TemplatePair::new(&shape.body())) {
            return Err(TestCaseError::fail(reason));
        }
    }

    /// Keep the outcome when an unquoted continuation joins a keyword.
    ///
    /// Bash deletes the pair before parsing, so `c\<newline>ase` is the same
    /// keyword as `case`. `prop_assume` skips the pair where it would land
    /// inside a generated quote, because only the unquoted form splices.
    #[test]
    fn a_continued_keyword_does_not_change_the_outcome(
        head in "[ -~]{0,12}",
        tail in "[ -~]{0,12}",
    ) {
        prop_assume!(!head.contains(['"', '\\', '\'', '#', '<', '>']));
        prop_assume!(!tail.contains(['"', '\\', '\'', '#', '<', '>']));
        let literal = format!("echo \"$({head}c{tail}ase x in x) :;; esac)\"");
        let continued = format!("echo \"$({head}c\\\n{tail}ase x in x) :;; esac)\"");
        let bindings = bindings();
        for interpolate in both_interpolators() {
            let literal_outcome = interpolate(&literal, &bindings);
            let continued_outcome = interpolate(&continued, &bindings);
            prop_assert_eq!(
                literal_outcome.is_ok(),
                continued_outcome.is_ok(),
                "splicing a continuation must not change acceptance: {:?} vs {:?}",
                literal,
                continued
            );
        }
    }
}

/// Close a `case` construct only when `esac` is spelled bare.
///
/// This is the rule the tracker's `esac` classification turns on, asserted
/// directly rather than through a marker: the bare word ends the construct, so
/// the `)` that follows closes the substitution and the construct is no longer
/// open; every quoted or escaped spelling is pattern text, so the construct
/// survives the terminator that follows it.
#[test]
fn only_a_bare_esac_closes_a_case_construct() {
    assert!(
        closes_construct("esac").expect("the bare spelling must be classifiable"),
        "a bare `esac` must close the construct"
    );
    for spelling in QUOTED_ESAC_SPELLINGS {
        assert!(
            !closes_construct(spelling).expect("a quoted spelling must be classifiable"),
            "`{spelling}` is pattern text and must not close the construct"
        );
    }
}

/// Sweep the whole finite grammar rather than sampling it.
///
/// `proptest` may revisit the same entry of a finite table many times, so the
/// invariant is checked exhaustively here. The preserved count is the
/// liveliness guard: a tracker that refused every `case` construct would
/// satisfy the accept-or-refuse property vacuously, and this fails instead.
#[test]
fn every_generated_case_skeleton_refuses_a_marker_inside_the_substitution() {
    let shapes = generated_shapes();
    let mut preserved = 0_usize;
    for shape in &shapes {
        match check_pair(&TemplatePair::new(&shape.body())) {
            Ok(is_preserved) => preserved += usize::from(is_preserved),
            Err(reason) => panic!("{reason}"),
        }
    }
    assert!(
        preserved > 0,
        concat!(
            "no marker-free skeleton was preserved, so the sweep proves ",
            "nothing about {} generated templates",
        ),
        shapes.len()
    );
}

/// Validate every generated skeleton with the shell before trusting the oracle.
///
/// The property above only means something if its inputs are shell grammar the
/// tracker claims to support. `bash` is the authority on that, so this checks
/// the whole generated table with `bash -n` and reports every offending script.
/// It is not part of the default run because the suite must not depend on a
/// `bash` on `PATH`; run it explicitly with
/// `cargo nextest run -p netsuke-build --run-ignored -E 'test(~generated_)'`.
#[test]
#[ignore = "requires bash on PATH; run explicitly to validate the generated grammar"]
fn generated_case_skeletons_parse_as_bash() {
    let shells: [&str; 2] = ["bash", "/bin/bash"];
    let Some(shell) = shells.iter().find(|shell| {
        std::process::Command::new(shell)
            .arg("-c")
            .arg("exit 0")
            .status()
            .is_ok_and(|status| status.success())
    }) else {
        panic!("no bash found on PATH or at /bin/bash, so the grammar cannot be validated");
    };

    let mut rejected = Vec::new();
    for shape in generated_shapes() {
        let script = format!("echo \"$({})\"", shape.body());
        let status = std::process::Command::new(shell)
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .status()
            .expect("bash must be runnable once it has answered the probe");
        if !status.success() {
            rejected.push(script);
        }
    }
    let sample: Vec<&String> = rejected.iter().take(5).collect();
    assert!(
        rejected.is_empty(),
        "the generated grammar must be valid shell; {} of {} scripts were rejected, \
         first {}: {sample:?}",
        rejected.len(),
        SUBJECTS.len() * PATTERNS.len() * ARMS.len() * SEPARATORS.len(),
        sample.len()
    );
}
