//! Regression tests for rendering command-list entries.

use super::*;

#[test]
fn large_command_list_prepares_the_jinja_context_once() {
    reset_recipe_context_preparations();
    let mut command = StringOrList::List(
        (0..64)
            .map(|index| format!("echo {{{{ label }}}} {index} {{{{ ins }}}}"))
            .collect(),
    );
    let mut vars = Vars::new();
    vars.insert("label".into(), ManifestValue::String("rendered".into()));

    render_recipe_string_or_list(&mut command, &Environment::new(), &vars, || {
        "render command list".into()
    })
    .expect("shell-safe command list should render");

    assert_eq!(
        recipe_context_preparations(),
        1,
        "one recipe must prepare its Jinja context once regardless of entry count"
    );
    let rendered_entries = command.to_string_vec();
    assert_eq!(
        rendered_entries.first().map(String::as_str),
        Some("echo rendered 0 __NETSUKE_INS_PLACEHOLDER__")
    );
    assert_eq!(
        rendered_entries.last().map(String::as_str),
        Some("echo rendered 63 __NETSUKE_INS_PLACEHOLDER__")
    );
}
