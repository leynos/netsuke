//! Tests for action hashing utilities.

use netsuke::ast::{Recipe, StringOrList};
use netsuke::hasher::ActionHasher;
use netsuke::ir::Action;
use rstest::rstest;

#[rstest]
#[case(
    Action {
        recipe: Recipe::Command { command: "echo".into() },
        description: Some("desc".into()),
        depfile: Some("$out.d".into()),
        deps_format: Some("gcc".into()),
        pool: None,
        restat: false,
    },
    "a0f6e2cd3b9b3cee0bf94a7d53bce56cf4178dfe907bb1cb7c832f47846baf38"
)]
#[case(
    Action {
        recipe: Recipe::Rule { rule: StringOrList::List(vec!["a".into(), "b".into()]) },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: true,
    },
    "cf8e97357820acf6f66037dcf977ee36c88c2811d60342db30c99507d24a0d60"
)]
fn hash_action_is_stable(#[case] action: Action, #[case] expected: &str) {
    assert_eq!(ActionHasher::hash(&action), expected);
}
