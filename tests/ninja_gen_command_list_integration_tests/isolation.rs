//! Retain POSIX validation rules across command-list isolation modes.

use super::*;
use rstest::rstest;

#[rstest]
#[case::multiple_background_jobs(
    "true & sh -c 'sleep 0.1; exit 1' &",
    "echo unexpected > continued-after-multiple-background-jobs.txt",
    CommandIsolation::Shared,
    "multiple background jobs should be rejected before Ninja runs"
)]
#[case::isolated_multiple_background_jobs(
    "true & sh -c 'sleep 0.1; exit 1' &",
    "echo unexpected > continued-after-multiple-background-jobs.txt",
    CommandIsolation::Isolated,
    "isolated mode must retain multiple-background rejection"
)]
#[case::nested_eval_background_jobs(
    "eval 'false & true &'",
    "echo unexpected > continued-after-nested-eval.txt",
    CommandIsolation::Shared,
    "nested eval background jobs should be rejected before Ninja runs"
)]
#[case::isolated_nested_eval_background_jobs(
    "eval 'false & true &'",
    "echo unexpected > continued-after-nested-eval.txt",
    CommandIsolation::Isolated,
    "isolated mode must retain nested-eval rejection"
)]
fn command_list_rejects_unattributable_background_jobs(
    #[case] entry: &str,
    #[case] later_entry: &str,
    #[case] isolation: CommandIsolation,
    #[case] expectation: &str,
) -> Result<()> {
    assert_multiple_background_jobs_are_rejected(
        vec![entry.into(), later_entry.into()],
        isolation,
        expectation,
    )
}

#[rstest]
#[case::unsupported_exec(
    "true && exec false",
    NinjaGenError::UnsupportedCommandListExec {
        action_index: 1,
        entry_index: 1,
    }
)]
#[case::dynamic_eval(
    "eval '$jobs'",
    NinjaGenError::UnanalyzableCommandListEval {
        action_index: 1,
        entry_index: 1,
    }
)]
#[case::control_character(
    "echo safe\ninjected",
    NinjaGenError::NinjaControlCharacter {
        action_index: 1,
        entry_index: 1,
    }
)]
fn isolated_mode_keeps_other_command_list_rejections(
    #[case] entry: &str,
    #[case] expected: NinjaGenError,
) -> Result<()> {
    let error = command_list_command_line(
        vec![entry.into(), "echo later".into()],
        CommandIsolation::Isolated,
    )
    .expect_err("isolated mode must retain command-list validation");
    let actual = error
        .downcast_ref::<NinjaGenError>()
        .context("command-list rejection should retain its typed error")?;
    ensure!(
        matches!(
            (actual, &expected),
            (
                NinjaGenError::UnsupportedCommandListExec { .. },
                NinjaGenError::UnsupportedCommandListExec { .. }
            ) | (
                NinjaGenError::UnanalyzableCommandListEval { .. },
                NinjaGenError::UnanalyzableCommandListEval { .. }
            ) | (
                NinjaGenError::NinjaControlCharacter { .. },
                NinjaGenError::NinjaControlCharacter { .. }
            )
        ),
        "isolated command-list entry returned the wrong error: {actual:?}"
    );
    Ok(())
}
