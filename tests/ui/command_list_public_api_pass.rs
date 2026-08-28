//! Compile-pass fixture for the public command-list AST surface.

use netsuke::ast::{Recipe, Recipe::Command, StringOrList};

fn command(recipe: Recipe) -> StringOrList {
    let Recipe::Command { command } = recipe else {
        unreachable!("fixture constructs only command recipes");
    };
    command
}

fn recipe_kind(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::Command { .. } => "command",
        Recipe::Script { .. } => "script",
        Recipe::Rule { .. } => "rule",
    }
}

fn main() {
    let borrowed = StringOrList::from("borrowed");
    let owned = StringOrList::from(String::from("owned"));
    let listed = StringOrList::from(vec![String::from("first"), String::from("second")]);

    assert!(matches!(borrowed, StringOrList::String(value) if value == "borrowed"));
    assert!(matches!(owned, StringOrList::String(value) if value == "owned"));
    assert!(matches!(listed, StringOrList::List(values) if values == ["first", "second"]));

    let constructed: StringOrList = command(Command {
        command: StringOrList::from("recipe"),
    });
    assert!(matches!(constructed, StringOrList::String(value) if value == "recipe"));
    assert_eq!(
        recipe_kind(Command {
            command: StringOrList::from("recipe"),
        }),
        "command"
    );
}
