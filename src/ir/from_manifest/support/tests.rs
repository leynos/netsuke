//! Regression tests for command-list manifest-to-IR lowering.

use super::*;
use crate::ir::cmd_interpolate::{binding_preparations, reset_binding_preparations};
use crate::ir::{INS_TOKEN, OUTS_TOKEN};
use crate::manifest;
use crate::recipe_shell::RecipeShell;
use proptest::prelude::*;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::{EnLocalizer, en_localizer};

#[path = "tests/fixtures.rs"]
mod fixtures;

use fixtures::{
    Entries, RejectionCase, assert_unfinished_region_rejected, lower_shared_power_shell_list,
    single_command_entries,
};

#[test]
fn large_command_list_prepares_path_bindings_once() {
    reset_binding_preparations();
    let entries = (0..64)
        .map(|index| format!("printf {index} {INS_TOKEN} {OUTS_TOKEN}"))
        .collect();
    let mut actions = IrHashMap::default();
    register_action(
        &mut actions,
        Recipe::Command {
            command: StringOrList::List(entries),
            isolation: crate::ast::CommandIsolation::Shared,
        },
        None,
        ActionBindings {
            inputs: &[Utf8PathBuf::from("input")],
            outputs: &[Utf8PathBuf::from("output")],
            shell: RecipeShell::Posix,
        },
    )
    .expect("shell-safe command list should lower");
    assert_eq!(
        binding_preparations(),
        1,
        "all entries in one recipe must reuse one prepared input/output binding set"
    );
}

/// Name the recipe form a quoted-marker case uses.
#[derive(Clone, Copy, Debug)]
enum RecipeKind {
    /// A scalar or list `command` recipe.
    Command,
    /// A `script` recipe.
    Script,
}

impl RecipeKind {
    /// Return the manifest key that selects this recipe form.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Script => "script",
        }
    }
}

#[rstest]
#[case::script(RecipeKind::Script)]
#[case::command(RecipeKind::Command)]
fn power_shell_manifest_rejects_quoted_input_marker(#[case] recipe_kind: RecipeKind) {
    let recipe_key = recipe_kind.as_str();
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: output\n    sources: \"; New-Item pwned; Write-Output x\"\n    {recipe_key}: \"Write-Output '{{{{ ins }}}}' > {{{{ outs }}}}\"\n"
    );
    let manifest = manifest::render_manifest(
        manifest::from_str(&source).expect("the manifest should parse"),
        &minijinja::Environment::new(),
    )
    .expect("the manifest template should render");
    let error = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)
        .expect_err("quoted PowerShell recipe markers must fail during IR lowering");

    assert!(
        matches!(
            &error,
            crate::ir::IrGenError::InvalidCommand { command, .. }
                if command.contains(crate::ir::INS_TOKEN)
        ),
        "expected the quoted-marker rejection, got {error:?}"
    );
}

/// Name which shared-list marker an example interpolates.
#[derive(Clone, Copy, Debug)]
enum Marker {
    /// The first input path.
    Inputs,
    /// The first output path.
    Outputs,
}

impl Marker {
    /// Return the internal token the manifest carries for this marker.
    const fn token(self) -> &'static str {
        match self {
            Self::Inputs => INS_TOKEN,
            Self::Outputs => OUTS_TOKEN,
        }
    }
}

/// Reject a shared PowerShell entry that leaves a quote open for its successor.
///
/// A shared list is assembled into one script, so an entry that ends inside a
/// string reclassifies the next entry's marker: path text that was lowered as a
/// standalone literal becomes executable syntax instead.
#[rstest]
#[case::direct_inputs(false, Marker::Inputs)]
#[case::direct_outputs(false, Marker::Outputs)]
#[case::rule_inputs(true, Marker::Inputs)]
#[case::rule_outputs(true, Marker::Outputs)]
fn shared_power_shell_lists_reject_an_entry_that_leaves_a_quote_open(
    #[case] is_rule_backed: bool,
    #[case] marker: Marker,
    en_localizer: EnLocalizer,
) {
    let _localizer = en_localizer;
    let unfinished = "Write-Output 'prefix";
    let marker_entry = format!("Copy-Item {}", marker.token());
    let error = lower_shared_power_shell_list(
        Entries::from(&[unfinished, marker_entry.as_str()][..]),
        is_rule_backed,
    )
    .expect_err("an unfinished single-quoted string must fail during lowering");
    assert_unfinished_region_rejected(
        &error,
        RejectionCase {
            template: unfinished,
            entry_index: 1,
            region: "single_quoted_string",
        },
    );
}

/// Reject every shared PowerShell region the scanner can leave unfinished.
#[rstest]
#[case::double_quote("Write-Output \"prefix", "double_quoted_string")]
#[case::here_string("@'\nbody", "here_string")]
#[case::block_comment("Write-Output <# note", "block_comment")]
#[case::command_substitution("Write-Output $(Get-Date", "command_substitution")]
#[case::trailing_escape("Write-Output prefix`", "trailing_backtick_escape")]
// PowerShell ends a block comment at the first `#>`, so the quoted text after
// the inner-looking opener is live syntax and can hide a later entry's marker.
#[case::block_comment_does_not_nest("<# <# #>\nWrite-Output \" #>", "double_quoted_string")]
// A mid-token hash may begin a line comment, which would make `<#` ordinary
// comment text; the scanner must not model it as a block comment.
#[case::ambiguous_hash_before_block_comment(
    "$x=$null# <#\nWrite-Output \" #>",
    "double_quoted_string"
)]
// A backtick-escaped space stays inside the argument token, so the following
// `<#` is literal text and the quote after it opens an expandable string.
#[case::escaped_separator_before_block_comment(
    "Write-Output prefix` <#\" #>",
    "double_quoted_string"
)]
// A backtick that escapes the `<` also keeps the opener inside the token.
#[case::escaped_block_comment_opener("Write-Output prefix`<# \" #>", "double_quoted_string")]
// An escaped space before a hash keeps the hash mid-token, so the quote after
// it opens an expandable string rather than starting a line comment.
#[case::escaped_separator_before_line_comment("Write-Output prefix` #\"", "double_quoted_string")]
fn shared_power_shell_lists_reject_other_unfinished_regions(
    #[case] unfinished: &str,
    #[case] region: &str,
    en_localizer: EnLocalizer,
) {
    let _localizer = en_localizer;
    let error = lower_shared_power_shell_list(
        Entries::from(&[unfinished, "Write-Output {{ ins }}"][..]),
        false,
    )
    .expect_err("an unfinished lexical region must fail during lowering");
    assert_unfinished_region_rejected(
        &error,
        RejectionCase {
            template: unfinished,
            entry_index: 1,
            region,
        },
    );
}

/// Reject markers that a mid-token `<#` would otherwise hide.
///
/// PowerShell opens a block comment only where a token can begin, so the `<#`
/// in `prefix<#" #>` stays inside the argument token and the quotes around the
/// marker still form an expandable string. Modelling it as a comment would hide
/// that string and accept the marker as a standalone literal.
#[rstest]
#[case::inputs(Marker::Inputs)]
#[case::outputs(Marker::Outputs)]
fn shared_power_shell_lists_reject_a_mid_token_block_comment_opener(
    #[case] marker: Marker,
    en_localizer: EnLocalizer,
) {
    let _localizer = en_localizer;
    let entry = format!("Write-Output prefix<#\" #> {} <# \" #>", marker.token());
    let error = lower_shared_power_shell_list(Entries::from(&[entry.as_str()][..]), false)
        .expect_err("a marker inside a mid-token quote sequence must fail");
    let IrGenError::InvalidCommand { command, .. } = &error else {
        panic!("expected an InvalidCommand rejection, got {error:?}");
    };
    assert_eq!(
        command, &entry,
        "the diagnostic must carry the entry's original template"
    );
    let rendered = normalize_fluent_isolates(&error.to_string());
    assert!(
        !rendered.contains("New-Item"),
        "the diagnostic must not carry interpolated path text: {rendered}"
    );
}

/// Keep an unmodelled opener's fail-closed state for the rest of the list.
///
/// A mid-token `<#` and a compact comment after a static member access both
/// leave the scanner unable to model the following text, so the carried state
/// must reject the next entry's marker.
#[rstest]
#[case::mid_token_inputs("Write-Output prefix<#", Marker::Inputs)]
#[case::mid_token_outputs("Write-Output prefix<#", Marker::Outputs)]
#[case::static_member_inputs("[string]::Empty# <# note #>", Marker::Inputs)]
#[case::static_member_outputs("[string]::Empty# <# note #>", Marker::Outputs)]
fn shared_power_shell_lists_keep_fail_closed_after_an_unmodelled_opener(
    #[case] opener: &str,
    #[case] marker: Marker,
    en_localizer: EnLocalizer,
) {
    let _localizer = en_localizer;
    let marker_entry = format!("Copy-Item {}", marker.token());
    let error =
        lower_shared_power_shell_list(Entries::from(&[opener, marker_entry.as_str()][..]), false)
            .expect_err("a later marker must stay protected after an unmodelled opener");
    let IrGenError::InvalidCommand { command, .. } = &error else {
        panic!("expected an InvalidCommand rejection, got {error:?}");
    };
    assert_eq!(
        command, &marker_entry,
        "the carried fail-closed state must reject the later marker entry"
    );
}

/// Accept a marker-free entry whose hash comment follows a block comment.
///
/// PowerShell treats `#>` as whitespace, so the apostrophe in `don't` stays
/// inside the line comment. The comment text still makes later markers fail
/// closed, but the entry itself has no marker and must lower.
#[test]
fn shared_power_shell_lists_accept_a_hash_comment_after_a_block_comment() {
    let entry = "Write-Output ok <# note #>#  don't";
    let graph = lower_shared_power_shell_list(Entries::from(&[entry][..]), false)
        .expect("a hash after a completed block comment starts a valid line comment");
    let resolved = single_command_entries(&graph);
    assert_eq!(
        resolved,
        [entry],
        "the completed block comment must not turn the hash into argument text"
    );
}

/// Report the index of the entry that left its lexical region open.
#[rstest]
fn shared_power_shell_lists_name_the_unfinished_entry_index(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let unfinished = "Write-Output 'prefix";
    let error = lower_shared_power_shell_list(
        Entries::from(&["Write-Output first", "Write-Output second", unfinished][..]),
        false,
    )
    .expect_err("a later unfinished entry must still fail during lowering");
    assert_unfinished_region_rejected(
        &error,
        RejectionCase {
            template: unfinished,
            entry_index: 3,
            region: "single_quoted_string",
        },
    );
}

/// Keep entries whose lexical regions are finished accepted in a shared list.
#[rstest]
#[case::ordinary(
    &["Write-Output first", "Copy-Item {{ ins }} {{ outs }}"][..],
    "Copy-Item '; New-Item pwned; Write-Output x' 'out.txt'"
)]
#[case::closed_here_string(
    &["@'\nbody\n'@", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
#[case::line_comment(
    &["# ordinary note", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
#[case::stop_parsing(
    &["cmd.exe /c --% literal", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
#[case::closed_block_comment(
    &["Write-Output <# note #> body", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
#[case::boundary_block_comment(
    &["Write-Output 1;<# note #> 2", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
#[case::block_comment_does_not_nest(
    &["<# outer <# inner #> body", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
// An escaped `<` is copied as one unit, so the following `#` is ordinary token
// text and the entry ends neutral for its successor's marker.
#[case::escaped_block_comment_opener(
    &["Write-Output prefix`<#", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
// A hash after an escaped separator starts no comment, so it stays argument
// text and does not set the fail-closed flag.
#[case::escaped_separator_before_hash(
    &["Write-Output prefix` #", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
// PowerShell treats a completed block comment as whitespace, so the hash that
// follows `#>` starts a line comment. A comment with no scanner-sensitive text
// leaves the carry neutral.
#[case::block_comment_end_before_hash(
    &["Write-Output ok <# note #># note", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
// Two adjacent block comments are both comments, so the entry ends neutral.
#[case::adjacent_block_comments(
    &["Write-Output ok <# a #><# b #> tail", "Write-Output {{ ins }}"][..],
    "Write-Output '; New-Item pwned; Write-Output x'"
)]
fn shared_power_shell_lists_accept_entries_that_end_neutral(
    #[case] entries: &[&str],
    #[case] expected_marker_entry: &str,
) {
    let graph = lower_shared_power_shell_list(Entries::from(entries), false)
        .expect("entries that end at a neutral boundary should lower");
    let resolved = single_command_entries(&graph);
    assert_eq!(
        resolved.len(),
        entries.len(),
        "lowering must preserve one entry per command-list item: {resolved:?}"
    );
    assert_eq!(
        resolved.get(1).map(String::as_str),
        Some(expected_marker_entry),
        "the later entry must be interpolated at its own unquoted marker site"
    );
}

proptest! {
    #[test]
    fn command_list_placeholder_interpolation_preserves_entry_order(
        labels in prop::collection::vec("[a-z]{1,10}", 1..9),
    ) {
        let entries: Vec<String> = labels
            .iter()
            .map(|label| format!("echo {label} {INS_TOKEN} {OUTS_TOKEN}"))
            .collect();
        let mut actions = IrHashMap::default();
        let action_id = register_action(
            &mut actions,
            Recipe::Command { command: StringOrList::List(entries), isolation: crate::ast::CommandIsolation::Shared },
            None,
            ActionBindings {
                inputs: &[Utf8PathBuf::from("input")],
                outputs: &[Utf8PathBuf::from("output")],
                shell: RecipeShell::Posix,
            },
        ).expect("shell-safe generated entries should interpolate");
        let action = actions.get(&action_id).expect("registered action should be available");
        let Recipe::Command { command, .. } = &action.recipe else {
            prop_assert!(false, "registered command list should remain a command recipe");
            return Ok(());
        };
        let expected: Vec<String> = labels
            .iter()
            .map(|label| format!("echo {label} input output"))
            .collect();
        prop_assert_eq!(command.to_string_vec(), expected);
    }
}
