//! Exercise the grammar against an independent regex and audit its generators.
//!
//! Test-only helpers are shared with selector properties, never production.

use super::ShellName;
use crate::shell_selection::BuiltInShell;
use pretty_assertions::assert_eq;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use regex::Regex;
use rstest::rstest;

/// Keep the oracle independent of the production reserved-word constant.
const BOOLEAN_WORDS: [&str; 8] = ["true", "false", "yes", "no", "y", "n", "on", "off"];

/// Decide grammar membership independently of the handwritten parser.
pub(in crate::shell_selection) fn oracle(regex: &Regex, text: &str) -> bool {
    regex.is_match(text) && !BOOLEAN_WORDS.contains(&text)
}

/// Generate the full grammar, rejecting only its rare reserved spellings.
pub(in crate::shell_selection) fn valid_names() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,62}".prop_filter("exclude YAML Boolean names", |text| {
        !BOOLEAN_WORDS.contains(&text.as_str())
    })
}

/// Generate guaranteed changes across every forbidden-name class.
fn near_misses() -> impl Strategy<Value = String> {
    (
        valid_names(),
        0u8..5,
        any::<u8>(),
        prop::sample::select(vec!['/', '\\', '.', ' ', '\0', 'é', '{']),
    )
        .prop_map(|(name, operation, position, forbidden)| match operation {
            0 => name
                .chars()
                .enumerate()
                .map(|(index, character)| {
                    if index == 0 {
                        character.to_ascii_uppercase()
                    } else {
                        character
                    }
                })
                .collect(),
            1 => format!(
                "{}{name}",
                match position.checked_rem(3).unwrap_or_default() {
                    0 => '0',
                    1 => '_',
                    _ => '-',
                }
            ),
            2 => {
                let mut changed = name;
                // Grammar names are ASCII; the positive divisor includes the end boundary.
                let index = usize::from(position)
                    .checked_rem(changed.len() + 1)
                    .unwrap_or_default();
                changed.insert(index, forbidden);
                changed
            }
            3 => format!("{name}{}", "a".repeat(64 - name.len())),
            _ => BOOLEAN_WORDS
                .get(
                    usize::from(position)
                        .checked_rem(BOOLEAN_WORDS.len())
                        .unwrap_or_default(),
                )
                .copied()
                .unwrap_or("true")
                .to_owned(),
        })
}

/// Generate printable ASCII including empty and excessive-length spellings.
fn ascii_names() -> impl Strategy<Value = String> {
    "[ -~]{0,70}"
}

/// Mix valid names, guaranteed near misses, ASCII, and arbitrary Unicode.
pub(in crate::shell_selection) fn arbitrary_names() -> impl Strategy<Value = String> {
    prop_oneof![valid_names(), near_misses(), ascii_names(), any::<String>()]
}

proptest! {
    #[test]
    fn parse_agrees_with_regex_oracle(text in arbitrary_names()) {
        let regex = Regex::new(r"\A[a-z][a-z0-9_-]{0,62}\z").map_err(|error| TestCaseError::fail(error.to_string()))?;
        let parsed = ShellName::parse(&text);
        prop_assert_eq!(parsed.is_ok(), oracle(&regex, &text));
        if let Ok(name) = parsed { prop_assert_eq!(name.as_str(), text); }
    }

    #[test]
    fn lookup_is_none_for_non_built_in_names(text in valid_names().prop_filter("exclude built-ins", |name| !["sh", "bash", "pwsh", "powershell"].contains(&name.as_str()))) {
        let name = ShellName::parse(&text).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(BuiltInShell::lookup(&name), None);
    }

    #[test]
    fn name_round_trips(text in valid_names()) {
        let name = ShellName::parse(&text).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(ShellName::parse(name.as_str()), Ok(name.clone()));
        let json = serde_json::to_string(&name).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let yaml = serde_saphyr::to_string(&name).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(serde_json::from_str::<ShellName>(&json).ok(), Some(name.clone()));
        prop_assert_eq!(serde_saphyr::from_str::<ShellName>(&yaml).ok(), Some(name));
    }
}

#[rstest]
#[case("null")]
#[case("nan")]
#[case("inf")]
#[case("yess")]
#[case("no-op")]
fn scalar_like_names_round_trip(#[case] text: &str) {
    let name = ShellName::parse(text).expect("scalar-like valid name must parse");
    let json = serde_json::to_string(&name).expect("JSON name must serialize");
    let yaml = serde_saphyr::to_string(&name).expect("YAML name must serialize");
    assert_eq!(
        serde_json::from_str::<ShellName>(&json).expect("JSON name must decode"),
        name
    );
    assert_eq!(
        serde_saphyr::from_str::<ShellName>(&yaml).expect("YAML name must decode"),
        name
    );
}

#[test]
fn generators_reach_their_claimed_classes() -> Result<(), String> {
    let mut runner = TestRunner::deterministic();
    let strategies = [
        valid_names().boxed(),
        near_misses().boxed(),
        ascii_names().boxed(),
        any::<String>().boxed(),
    ];
    for (class, strategy) in strategies.into_iter().enumerate() {
        let mut accepted = 0;
        for _ in 0..1_000 {
            let text = strategy
                .new_tree(&mut runner)
                .map_err(|error| error.to_string())?
                .current();
            accepted += usize::from(ShellName::parse(&text).is_ok());
        }
        match class {
            0 if accepted != 1_000 => {
                return Err(format!(
                    "valid generator accepted {accepted} of 1000 samples"
                ));
            }
            1 if accepted != 0 => {
                return Err(format!("near-miss generator accepted {accepted} samples"));
            }
            // Both classes are seed-dependent because valid ASCII is rare.
            2 if !(1..1_000).contains(&accepted) => {
                return Err(format!(
                    "ASCII generator accepted {accepted} of 1000 samples"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
