//! Property tests for POSIX heredoc body boundaries.
//!
//! The generator and the reference model below are written from the shell's
//! own rules rather than from the tracker, so a disagreement between them is
//! evidence about the tracker rather than a restatement of its implementation.

use super::super::*;
use super::support::{Lowering, Recipe, assert_both_traversals_lower};

use proptest::prelude::*;

/// One supported heredoc declaration and the rules its delimiter selects.
#[derive(Clone, Debug)]
struct DelimiterForm {
    /// The declaration text that follows the command word.
    declaration: String,
    /// The quote-removed text the shell compares against a body line.
    terminator: String,
    /// Whether the delimiter suppresses expansion and backslash-newline joining.
    is_quoted: bool,
    /// Whether `<<-` strips leading tabs before comparison.
    strips_leading_tabs: bool,
}

impl DelimiterForm {
    /// Build one supported declaration from its source text and its rules.
    fn new(
        declaration: &str,
        terminator: &str,
        is_quoted: bool,
        strips_leading_tabs: bool,
    ) -> Self {
        Self {
            declaration: declaration.to_owned(),
            terminator: terminator.to_owned(),
            is_quoted,
            strips_leading_tabs,
        }
    }
}

/// Generate a supported heredoc declaration and the rules it selects.
///
/// The forms are the ones the tracker models: plain, tab-stripping, quoted,
/// partially quoted, escaped, and generated words in each of those shapes.
fn supported_delimiter() -> impl Strategy<Value = DelimiterForm> {
    let word_pattern = "[a-z]{1,8}";
    prop_oneof![
        Just(DelimiterForm::new("<<EOF", "EOF", false, false)),
        Just(DelimiterForm::new("<<-EOF", "EOF", false, true)),
        Just(DelimiterForm::new("<<'EOF'", "EOF", true, false)),
        Just(DelimiterForm::new("<<\"EOF\"", "EOF", true, false)),
        Just(DelimiterForm::new("<<E\"O\"F", "EOF", true, false)),
        Just(DelimiterForm::new("<<\\EOF", "EOF", true, false)),
        word_pattern.prop_map(|word| DelimiterForm::new(&format!("<<{word}"), &word, false, false)),
        word_pattern.prop_map(|word| DelimiterForm::new(
            &format!("<<'{word}'"),
            &word,
            true,
            false
        )),
        word_pattern.prop_map(|word| DelimiterForm::new(&format!("<<-{word}"), &word, false, true)),
        word_pattern.prop_map(|word| DelimiterForm::new(
            &format!("<<-\"{word}\""),
            &word,
            true,
            true
        )),
    ]
}

/// Generate one heredoc body line that cannot be mistaken for a terminator.
///
/// Each line is an optional tab prefix followed by `-`, and no generated
/// delimiter contains `-`, so the reference model can assert that no logical
/// line ends the body early. The tab prefix lets `<<-` form a logical line that
/// begins with tabs, and the content covers Unicode and shell metacharacters.
/// The trailing backslash run exercises the shell's line joining, which in turn
/// decides whether the next line's tabs sit at a logical line start.
fn body_line() -> impl Strategy<Value = String> {
    let content = prop_oneof![
        Just('a'),
        Just(' '),
        Just('\t'),
        Just('\''),
        Just('"'),
        Just('\\'),
        Just('#'),
        Just('$'),
        Just('`'),
        Just('\u{00A0}'),
        Just('\u{2003}'),
        Just('é'),
        Just('-'),
        Just('.'),
        Just(':'),
    ];
    (
        prop::collection::vec(content, 0..12),
        0usize..=2,
        0usize..=3,
    )
        .prop_map(|(chars, tabs, backslashes)| {
            let mut line = "\t".repeat(tabs);
            line.push('-');
            line.extend(chars);
            line.extend(std::iter::repeat_n('\\', backslashes));
            line
        })
}

/// Return the logical lines the shell compares with the delimiter.
///
/// This states the shell's own rule independently of the tracker, checked
/// against GNU Bash and Debian `dash` rather than against one implementation:
/// `<<-` strips leading tabs only where a logical line begins, so a tab that
/// follows a backslash continuation survives; and an unquoted body removes a
/// backslash-newline pair before the comparison, so a line ending in an odd
/// backslash run continues into the next one. A quoted delimiter suppresses
/// the joining, leaving each physical line to be compared as it stands.
fn logical_lines(lines: &[String], form: &DelimiterForm) -> Vec<String> {
    let mut logical = Vec::new();
    let mut current = String::new();
    for line in lines {
        let at_logical_line_start = current.is_empty();
        let stripped = if form.strips_leading_tabs && at_logical_line_start {
            line.trim_start_matches('\t')
        } else {
            line.as_str()
        };
        current.push_str(stripped);
        if !form.is_quoted && has_odd_trailing_backslashes(&current) {
            current.pop();
            continue;
        }
        logical.push(std::mem::take(&mut current));
    }
    if !current.is_empty() {
        logical.push(current);
    }
    logical
}

/// Report whether `line` ends in an odd run of backslashes.
fn has_odd_trailing_backslashes(line: &str) -> bool {
    !line
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count()
        .is_multiple_of(2)
}

// Verify every supported delimiter keeps its body inert until the terminator.
//
// The reference model is the shell's own rule: a body line ends the heredoc
// only when it equals the delimiter, so every marker before that line must
// survive lowering byte-for-byte and the marker after it must be lowered. The
// model re-derives the logical lines, including tab stripping and backslash
// joining, and asserts that none of them matches the delimiter, so the marker
// line is provably inside the body before the lowering is compared.
proptest! {
    #[test]
    fn heredoc_bodies_stay_inert_for_supported_delimiters(
        form in supported_delimiter(),
        body_lines in prop::collection::vec(body_line(), 0..4),
        terminator_tabs in 0usize..=2,
    ) {
        let tabs = if form.strips_leading_tabs { terminator_tabs } else { 0 };
        let body = body_lines
            .iter()
            .fold(String::new(), |mut body, line| {
                body.push_str(line);
                body.push('\n');
                body
            });

        let mut compared = body_lines.clone();
        compared.push(INS_TOKEN.to_owned());
        prop_assert!(
            logical_lines(&compared, &form)
                .iter()
                .all(|line| line != &form.terminator),
            "the generated body must not end before its terminator"
        );

        let terminator_line = format!("{}{}", "\t".repeat(tabs), form.terminator);
        let template = format!(
            "cat {}\n{body}{INS_TOKEN}\n{terminator_line}\necho {OUTS_TOKEN}",
            form.declaration
        );
        let expected = format!(
            "cat {}\n{body}{INS_TOKEN}\n{terminator_line}\necho output",
            form.declaration
        );
        assert_both_traversals_lower(
            Recipe(&template),
            &Lowering {
                text: &expected,
                rule: "a supported delimiter must keep its body inert until the terminator",
            },
        );
    }
}

/// Prefix `line` with the tabs a `<<-` terminator line may carry.
///
/// Tab stripping applies only to a `<<-` body, so a delimiter that keeps its
/// tabs keeps the prefix empty and the tab stays ordinary word text.
fn indented(line: &str, tabs: usize, strips_leading_tabs: bool) -> String {
    let prefix = if strips_leading_tabs {
        "\t".repeat(tabs)
    } else {
        String::new()
    };
    format!("{prefix}{line}")
}

/// Move the text a closing-shape strategy must own out of `form`.
///
/// A strategy returned by an `impl Strategy` function captures its parameter's
/// lifetime under edition 2024, so a generator cannot borrow the form it was
/// given while the form is also moved into the generated tuple.
fn captured_text(form: DelimiterForm) -> (String, bool) {
    let DelimiterForm {
        terminator,
        strips_leading_tabs,
        ..
    } = form;
    (terminator, strips_leading_tabs)
}

/// A closing shape whose lines differ only in how they use the terminator line.
#[derive(Clone, Copy)]
enum TabbedShape {
    /// Close on the terminator line alone.
    WholeTerminator,
    /// Offer a line that only looks like the terminator, then the real one.
    ///
    /// The near miss must not close the body, which is what distinguishes this
    /// shape from [`TabbedShape::WholeTerminator`].
    NearMissThenTerminator,
    /// Offer a bare-backslash line, then a tab-prefixed terminator.
    ///
    /// `body_line` cannot build this shape: it always emits at least one
    /// character before any backslash run, so the logical line it continues is
    /// never empty. The shell removes the backslash-newline before comparing,
    /// leaving an empty logical line, so the terminator line still begins one
    /// and `<<-` still strips its tabs. That is the boundary this shape pins.
    EmptyContinuation,
}

/// Generate the closing lines `shape` produces, indenting with zero to two tabs.
fn tabbed_closing(form: DelimiterForm, shape: TabbedShape) -> impl Strategy<Value = Vec<String>> {
    let (terminator, strips_leading_tabs) = captured_text(form);
    (0usize..=2).prop_map(move |tabs| {
        let line = indented(&terminator, tabs, strips_leading_tabs);
        match shape {
            TabbedShape::WholeTerminator => vec![line],
            TabbedShape::NearMissThenTerminator => vec![format!("{line} "), line],
            TabbedShape::EmptyContinuation => vec!["\\".to_owned(), line],
        }
    })
}

/// Generate a terminator split across a backslash continuation.
///
/// The joined logical line is the terminator, so a body may end on a physical
/// line that only looks like a fragment of one. Every generated terminator is
/// ASCII, so the split index is a character boundary.
fn split_by_continuation(form: DelimiterForm) -> impl Strategy<Value = Vec<String>> {
    let (terminator, _) = captured_text(form);
    (1..terminator.len().max(2)).prop_map(move |at| {
        let (head, tail) = terminator.split_at(at);
        vec![format!("{head}\\"), tail.to_owned()]
    })
}

/// Generate a supported delimiter and physical lines that close its body.
///
/// Every shape is one the shell accepts, and the reference model re-derives
/// where the body ends from those physical lines alone. A quoted delimiter
/// suppresses backslash-newline joining, so the two continuation shapes are
/// offered only for the unquoted forms.
fn closing_case() -> impl Strategy<Value = (DelimiterForm, Vec<String>)> {
    supported_delimiter().prop_flat_map(|form| {
        let closing = if form.is_quoted {
            prop_oneof![
                tabbed_closing(form.clone(), TabbedShape::WholeTerminator),
                tabbed_closing(form.clone(), TabbedShape::NearMissThenTerminator),
            ]
            .boxed()
        } else {
            prop_oneof![
                tabbed_closing(form.clone(), TabbedShape::WholeTerminator),
                tabbed_closing(form.clone(), TabbedShape::NearMissThenTerminator),
                tabbed_closing(form.clone(), TabbedShape::EmptyContinuation),
                split_by_continuation(form.clone()),
            ]
            .boxed()
        };
        (Just(form), closing)
    })
}

// Verify a body ends where the reference model says it ends.
//
// The model owns the answer: it assembles the shell's logical lines from the
// generated physical lines, including tab stripping and backslash joining, and
// requires the terminator to complete the final logical line with no earlier
// match. The marker before that line must survive lowering byte-for-byte and
// the marker after it must be lowered, through both traversals. Candidate and
// split terminators are what separate this case from the physical-line rule it
// complements: a body may end on a line that only looks like a fragment, and
// it must not end on one that only looks like the terminator.
proptest! {
    #[test]
    fn heredoc_bodies_end_where_the_reference_model_ends_them(
        (form, closing) in closing_case(),
        body_lines in prop::collection::vec(body_line(), 0..4),
    ) {
        let mut physical = body_lines;
        physical.push(INS_TOKEN.to_owned());
        physical.extend(closing);

        let logical = logical_lines(&physical, &form);
        let (last, earlier) = logical
            .split_last()
            .expect("the closing lines must complete a logical line");
        prop_assert_eq!(last, &form.terminator, "the model ends the body here");
        prop_assert!(
            earlier.iter().all(|line| line != &form.terminator),
            "no earlier logical line may end the body"
        );

        let body = physical.join("\n");
        let template = format!("cat {}\n{body}\necho {OUTS_TOKEN}", form.declaration);
        let expected = format!("cat {}\n{body}\necho output", form.declaration);
        assert_both_traversals_lower(
            Recipe(&template),
            &Lowering {
                text: &expected,
                rule: "a heredoc body must end where the shell ends it",
            },
        );
    }
}
