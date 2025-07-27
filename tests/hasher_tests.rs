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
#[case(
    Action {
        recipe: Recipe::Command { command: String::new() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    "69f72afccc2aa5a709af1139a9c7ef5f4f72e57cf5376e6c043e575f68f2ef8d"
)]
#[case(
    Action {
        recipe: Recipe::Rule { rule: StringOrList::List(vec![]) },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    "c28b5c0b7f20bf1093cbab990976b904268f173413f54b7007166b2c02f498f3"
)]
#[case(
    Action {
        recipe: Recipe::Command { command: "特殊字符!@#$%^&*()".into() },
        description: Some("desc\nwith\nnewlines".into()),
        depfile: Some(String::new()),
        deps_format: None,
        pool: None,
        restat: false,
    },
    "28adc0857704aa0c54c3bc624cb2dc70c101c3936987b20ae520a20319f591c2"
)]
// Order of rule names influences the digest.
#[case(
    Action {
        recipe: Recipe::Rule { rule: StringOrList::List(vec!["b".into(), "a".into()]) },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: true,
    },
    "b93ff0102089f1f1a3fe9eec082b59d5aab58271a40724ccdfdaade6a68fe340"
)]
fn hash_action_is_stable(#[case] action: Action, #[case] expected: &str) {
    assert_eq!(ActionHasher::hash(&action), expected);
}
