//! Directed witnesses that command text contributes to action identity.

use super::{
    Action, CommandIsolation, Recipe, StringOrList, assert_not_interned_away, scalar_action,
};

#[test]
fn scalar_command_text_difference_is_not_interned_away() {
    let base = scalar_action();
    let changed = Action {
        recipe: Recipe::Command {
            command: StringOrList::String("cc -o".to_owned()),
            isolation: CommandIsolation::Shared,
        },
        ..base.clone()
    };
    assert_not_interned_away(&base, &changed, "scalar command text");
}

#[test]
fn list_command_text_difference_is_not_interned_away() {
    let base = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec!["cd out".to_owned(), "cc -c".to_owned()]),
            isolation: CommandIsolation::Shared,
        },
        ..scalar_action()
    };
    let changed = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec!["cd out".to_owned(), "cc -o".to_owned()]),
            isolation: CommandIsolation::Shared,
        },
        ..base.clone()
    };
    assert_not_interned_away(&base, &changed, "list command text");
}
