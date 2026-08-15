//! Unit tests for private command-list shell boundaries.

use super::{
    ActionId, CommandListEntry, CommandListEntryError, ExecBoundary, action_identity,
    background_operator_count, command_list_entry, command_list_entry_error, exec_boundary,
    shell_single_quote,
};
use rstest::rstest;

#[rstest]
#[case::direct_assignment_prefixed("FOO=1 exec false", ExecBoundary::Direct)]
#[case::conditional_body("if true; then exec false; fi", ExecBoundary::Unsupported)]
#[case::loop_body("while true; do exec false; done", ExecBoundary::Unsupported)]
#[case::case_body("case x in x) exec false;; esac", ExecBoundary::Unsupported)]
#[case::and_list("true && exec false", ExecBoundary::Unsupported)]
#[case::argument("echo exec", ExecBoundary::None)]
#[case::printf_argument("printf '%s' exec", ExecBoundary::None)]
#[case::command_wrapper("command exec false", ExecBoundary::Unsupported)]
fn classifies_direct_and_unsupported_exec_entries(
    #[case] command: &str,
    #[case] expected: ExecBoundary,
) {
    assert_eq!(exec_boundary(CommandListEntry(command)), expected);
}

#[rstest]
#[case::single_background("sleep 1 &", 1)]
#[case::multiple_backgrounds("sleep 1 & true &", 2)]
#[case::quoted_and_comment("echo '&' # &", 0)]
#[case::redirect_then_background("cmd 2>&1 &", 1)]
#[case::two_output_redirects("cmd 2>&1 1>&2", 0)]
#[case::output_redirect("cmd 1>&2", 0)]
fn counts_only_unquoted_background_operators_before_comments(
    #[case] command: &str,
    #[case] expected: usize,
) {
    assert_eq!(
        background_operator_count(CommandListEntry(command)),
        expected
    );
}

#[rstest]
#[case::single_static_eval_job("eval 'true &'", None)]
#[case::nested_multiple_jobs(
    "eval 'false & true &'",
    Some(CommandListEntryError::MultipleBackgroundJobs)
)]
#[case::nested_and_outer_job(
    "eval 'true &' &",
    Some(CommandListEntryError::MultipleBackgroundJobs)
)]
#[case::unsupported_exec(
    "if true; then exec false; fi",
    Some(CommandListEntryError::UnsupportedExec)
)]
#[case::dynamic_eval_source("eval '$jobs'", Some(CommandListEntryError::UnanalyzableEval))]
#[case::glob_eval_source("eval 'cp *.c build/'", Some(CommandListEntryError::UnanalyzableEval))]
#[case::variable_eval_source(
    "eval \"$CC -c main.c\"",
    Some(CommandListEntryError::UnanalyzableEval)
)]
fn rejects_unattributable_eval_background_jobs(
    #[case] command: &str,
    #[case] expected: Option<CommandListEntryError>,
) {
    assert_eq!(
        command_list_entry_error(CommandListEntry(command)),
        expected
    );
}

#[test]
fn shell_quotes_each_entry_as_one_literal_argument() {
    assert_eq!(
        shell_single_quote(CommandListEntry("echo 'quoted'")),
        "'echo '\\''quoted'\\'''"
    );
}

#[test]
fn rendered_entry_uses_a_hashed_action_identity_and_one_based_index() {
    let rendered = command_list_entry(CommandListEntry("false"), ActionId("example"), 3);
    let expected_identity = "50d858e0985ecc7f60418aaf0cc5ab587f42c2570a884095a9e8ccacd0f6545c";
    assert_eq!(action_identity(ActionId("example")), expected_identity);
    assert!(
        rendered.contains(&format!(
            "netsuke command-list failure: action {expected_identity}, entry 3"
        )),
        "entry must use the hashed identity and its one-based index: {rendered}"
    );
}
