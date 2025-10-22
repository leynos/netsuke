#![allow(
    clippy::expect_used,
    reason = "hasher tests use expect for descriptive failures"
)]

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
    "0fe3670f0746dcec34768df158d814ac099e416b6045e7e213d0aabd6aa761cb"
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
    "9b0289f92ea0e374eecdaf50c8c9080547635aaff38d07fe2a278af6894c3207"
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
    "9733343b512253e636fbacfea40ef4f5771d49409fcda026aec7c7ce2f5405ec"
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
    "9b53c477668394e59eca5b34416ef7ad7fb5799ca96dd283e81d7acda6c56006"
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
    "57023b1c00f7daf410d3d2077346e38014d3612c278aadef73a8484c94bdcb77"
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
    "d5f1a262a95b75db3a7a79a5855eb27b6b430833e7ba93538502a16ebd03f50b"
)]
fn hash_action_is_stable(#[case] action: Action, #[case] expected: &str) {
    let digest = ActionHasher::hash(&action).expect("hash action");
    assert_eq!(digest, expected);
}
