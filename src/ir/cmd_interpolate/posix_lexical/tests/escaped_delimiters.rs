//! Regression tests for escapes immediately before a heredoc delimiter marker.
//!
//! The delimiter renderer appends the ordinary path encoding at the marker, so
//! an unconsumed backslash would escape the encoding's opening quote and turn
//! path text into shell syntax. These tests pin that both markers fail closed
//! for every supported path shape, that an escaped pair still renders one
//! literal backslash, and that a rendered word must be consumed entirely.

use super::super::super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell,
    interpolate_command_with_bindings, interpolate_script_with_bindings,
};
use super::super::{
    EscapeBeforeMarker, delimiter::render_delimiter, delimiter::rendered_terminator,
};

use camino::Utf8PathBuf;
use proptest::prelude::*;
use rstest::rstest;

/// Path shapes whose unquoted encoding either quotes or escapes shell syntax.
const HOSTILE_PATHS: [&str; 4] = [
    "input",
    "input path",
    "a`touch injected`b$HOME",
    "x\"; touch injected; echo \"y",
];

/// A path of shell metacharacters that must stay delimiter data.
///
/// Naming the corpus entry keeps the helper signatures about the scenario
/// rather than about a bare string, and stops the module's argument list from
/// reading as string plumbing.
struct HostilePath<'a>(&'a str);

impl<'a> HostilePath<'a> {
    /// Borrow the path text bound into the test command.
    fn as_str(&self) -> &'a str {
        self.0
    }
}

/// Build bindings that bind the same path to both markers.
fn bindings_for(shell: RecipeShell, path: &HostilePath<'_>) -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(path.as_str())],
        &[Utf8PathBuf::from(path.as_str())],
        shell,
    )
}

/// Render one complete delimiter word from its source characters.
fn render_word(source: &str, bindings: &CommandBindings) -> Result<String, EscapeBeforeMarker> {
    let chars: Vec<char> = source.chars().collect();
    render_delimiter(&chars, 0, chars.len(), bindings)
}

/// Assert one interpolator rejects `template` with the original command text.
///
/// The heredoc renderer is a pure transformation, so a rejected declaration can
/// only report the source it refused to lower. Both the command and its bounded
/// snippet must therefore match the unlowered template.
fn assert_invalid_command(error: &IrGenError, template: &str) {
    let IrGenError::InvalidCommand {
        command, snippet, ..
    } = error
    else {
        panic!("expected an invalid-command error, got {error:?}");
    };
    assert_eq!(
        command, template,
        "the rejection must report the original template"
    );
    let expected_snippet: String = template.chars().take(160).collect();
    assert_eq!(
        snippet, &expected_snippet,
        "the snippet must be the first 160 characters of the template"
    );
}

/// Reject a marker after one unpaired backslash for every hostile path shape.
///
/// The marker site, not the template, decides what the backslash escapes, so
/// both markers must be rejected before any binding is inserted.
#[rstest]
#[case::simple(HOSTILE_PATHS[0])]
#[case::spaces(HOSTILE_PATHS[1])]
#[case::backtick_and_dollar(HOSTILE_PATHS[2])]
#[case::quote_and_semicolon(HOSTILE_PATHS[3])]
fn unquoted_delimiter_marker_after_an_active_escape_is_rejected(#[case] path: &str) {
    let bindings = bindings_for(RecipeShell::Posix, &HostilePath(path));
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        let source = format!("\\{marker}");
        assert!(
            matches!(render_word(&source, &bindings), Err(EscapeBeforeMarker)),
            "a delimiter marker after one backslash must be rejected for {path:?}"
        );
    }
}

/// Build the unquoted, double-quoted, and mid-word delimiter templates.
///
/// Each form places the marker after one backslash at a different site, so a
/// guard that only inspects the start of the delimiter word cannot pass the
/// matrix by accident.
fn escaped_delimiter_templates(marker: &str, path: &HostilePath<'_>) -> [String; 3] {
    let path_text = path.as_str();
    [
        format!("cat <<\\{marker}\nbody\n\\{path_text}"),
        format!("cat <<\"\\{marker}\"\nbody\n\\{path_text}"),
        format!("cat <<pre\\{marker}\nbody\n\\pre{path_text}"),
    ]
}

/// Assert both POSIX traversals reject one template with its original text.
///
/// The script traversal does not apply the command traversal's `shlex` and
/// backtick validation, so each traversal must reach the shared delimiter
/// guard rather than relying on a later syntax check.
fn assert_both_traversals_reject(template: &str, bindings: &CommandBindings) {
    for interpolate in [
        interpolate_command_with_bindings
            as fn(&str, &CommandBindings) -> Result<String, IrGenError>,
        interpolate_script_with_bindings,
    ] {
        let error =
            interpolate(template, bindings).expect_err("an escaped delimiter marker must fail");
        assert_invalid_command(&error, template);
    }
}

/// Assert one shell rejects every marker and delimiter template for a path.
fn assert_shell_rejects_every_escaped_marker(shell: RecipeShell, path: &HostilePath<'_>) {
    let bindings = bindings_for(shell, path);
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        for template in escaped_delimiter_templates(marker, path) {
            assert_both_traversals_reject(&template, &bindings);
        }
    }
}

/// Reject the same trigger through both POSIX traversals and both shells.
///
/// The matrix keeps every hostile path shape, both markers, and all three
/// delimiter sites while each helper stays shallow enough to read.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn both_traversals_reject_a_delimiter_marker_after_an_unpaired_backslash(
    #[case] shell: RecipeShell,
) {
    for path in HOSTILE_PATHS {
        assert_shell_rejects_every_escaped_marker(shell, &HostilePath(path));
    }
}

/// Keep a single-quoted backslash literal because it escapes nothing there.
///
/// Single quotes are the supported way to place a marker beside a literal
/// backslash in a delimiter word, so this is the positive control for the
/// rejection above.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn a_single_quoted_backslash_leaves_a_delimiter_marker_eligible(#[case] shell: RecipeShell) {
    for path in HOSTILE_PATHS {
        let hostile = HostilePath(path);
        let bindings = bindings_for(shell, &hostile);
        for marker in [INS_TOKEN, OUTS_TOKEN] {
            let source = format!("'\\{marker}'");
            let rendered =
                render_word(&source, &bindings).expect("a single-quoted backslash must not reject");
            let terminator = rendered_terminator(&rendered)
                .expect("the rendered single-quoted word must parse as one delimiter");
            assert_eq!(
                terminator.text,
                format!("\\{path}"),
                "the literal backslash must precede the quote-removed path"
            );
            assert!(
                terminator.is_quoted,
                "a delimiter containing quotes must suppress body expansion"
            );
        }
    }
}

/// Report the terminator a supported rendered delimiter selects.
///
/// Odd backslash counts leave the last backslash active before the marker and
/// are rejected. An even count is consumed as escaped pairs first, and each
/// pair renders one literal backslash in the quote-removed terminator, so the
/// rendered word is quoted and must be consumed entirely by the reparse.
#[rstest]
#[case::one(1)]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn backslash_parity_before_a_delimiter_marker(#[case] backslashes: usize) {
    let path = HostilePath(HOSTILE_PATHS[3]);
    let bindings = bindings_for(RecipeShell::Posix, &path);
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        let source = format!("{}{marker}", "\\".repeat(backslashes));
        let rendered = render_word(&source, &bindings);
        if !backslashes.is_multiple_of(2) {
            assert!(
                matches!(&rendered, Err(EscapeBeforeMarker)),
                "an odd backslash count must leave the last escape active"
            );
            continue;
        }
        let rendered_word = rendered.expect("an even backslash count must render");
        let terminator = rendered_terminator(&rendered_word)
            .expect("an even backslash count must render one delimiter word");
        assert_eq!(
            terminator.text,
            format!(
                "{}{}",
                "\\".repeat(backslashes.div_euclid(2)),
                path.as_str()
            ),
            "each escaped pair must render one literal backslash before the path"
        );
        assert!(
            terminator.is_quoted,
            "a rendered path that needs quoting must suppress body expansion"
        );
        assert_eq!(
            terminator.end,
            rendered_word.chars().count(),
            "the reparse must consume the whole rendered word"
        );
    }
}

/// Generate one path made only of characters a shell treats specially.
fn hostile_path() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just('"'),
            Just('\''),
            Just('$'),
            Just('`'),
            Just('\\'),
            Just(';'),
            Just('&'),
            Just('|'),
            Just('#'),
            Just(' '),
            Just('\t'),
        ],
        1..=12,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    /// Check escape parity for generated hostile paths and bounded backslashes.
    ///
    /// An odd count leaves an active escape and must be rejected. An even count
    /// must render one quoted delimiter word whose quote-removed text is the
    /// literal backslashes followed by the exact path, and the reparse must
    /// consume all of it.
    #[test]
    fn delimiter_escape_parity_holds_for_hostile_paths(
        path in hostile_path(),
        backslashes in 0usize..=4,
    ) {
        let hostile = HostilePath(&path);
        let bindings = bindings_for(RecipeShell::Posix, &hostile);
        for marker in [INS_TOKEN, OUTS_TOKEN] {
            let source = format!("{}{marker}", "\\".repeat(backslashes));
            let rendered = render_word(&source, &bindings);
            if !backslashes.is_multiple_of(2) {
                prop_assert!(
                    matches!(&rendered, Err(EscapeBeforeMarker)),
                    "an odd backslash count must be rejected for path {:?}",
                    path
                );
                continue;
            }
            let rendered_word = rendered.expect("an even backslash count must render");
            let terminator = rendered_terminator(&rendered_word)
                .expect("the rendered delimiter must parse as one word");
            prop_assert_eq!(
                &terminator.text,
                &format!("{}{path}", "\\".repeat(backslashes.div_euclid(2)))
            );
            prop_assert!(
                terminator.is_quoted,
                "a hostile path must render a quoted delimiter"
            );
            prop_assert_eq!(terminator.end, rendered_word.chars().count());
        }
    }
}
