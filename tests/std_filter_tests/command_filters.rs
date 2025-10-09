use minijinja::{ErrorKind, context};
use rstest::rstest;

use super::support::stdlib_env_with_state;

#[rstest]
fn shell_filter_marks_templates_impure() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell", "{{ 'hello' | shell('tr a-z A-Z') | trim }}")
        .expect("template");
    let template = env.get_template("shell").expect("get template");
    let rendered = template.render(context! {}).expect("render");
    assert_eq!(rendered, "HELLO");
    assert!(
        state.is_impure(),
        "shell filter should mark template impure"
    );
}

#[rstest]
fn shell_filter_surfaces_command_failures() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("shell_fail", "{{ 'data' | shell('false') }}")
        .expect("template");
    let template = env.get_template("shell_fail").expect("get template");
    let result = template.render(context! {});
    let err = result.expect_err("shell should propagate failures");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        state.is_impure(),
        "failure should still mark template impure"
    );
    assert!(
        err.to_string().contains("shell command")
            || err.to_string().contains("failed")
            || err.to_string().contains("error"),
        "error should indicate command failure: {err}",
    );
}

#[rstest]
fn grep_filter_filters_lines() {
    let (mut env, state) = stdlib_env_with_state();
    state.reset_impure();
    env.add_template("grep", "{{ 'alpha\\nbeta\\n' | grep('beta') | trim }}")
        .expect("template");
    let template = env.get_template("grep").expect("get template");
    let rendered = template.render(context! {}).expect("render");
    assert_eq!(rendered, "beta");
    assert!(state.is_impure(), "grep should mark template impure");
}

#[rstest]
fn grep_filter_rejects_invalid_flags() {
    let (mut env, _state) = stdlib_env_with_state();
    env.add_template("grep_invalid", "{{ 'alpha' | grep('a', [1, 2, 3]) }}")
        .expect("template");
    let template = env.get_template("grep_invalid").expect("get template");
    let err = template
        .render(context! {})
        .expect_err("non-string flags should be rejected");
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        err.to_string().contains("grep flags must be strings"),
        "error should explain invalid flags: {err}",
    );
}
