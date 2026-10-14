//! Pin exact names, rejection boundaries, precedence, and conversion surfaces.

use super::{ShellName, ShellNameError};
use pretty_assertions::assert_eq;
use rstest::rstest;

#[rstest]
#[case("a")]
#[case("dash")]
#[case("a0_-9")]
#[case("yess")]
#[case("no-op")]
#[case("on1")]
#[case("nn")]
#[case("offline")]
#[case("truex")]
#[case("null")]
fn accepts_exact_valid_names(#[case] text: &str) {
    let name = ShellName::parse(text).expect("valid name must parse");
    assert_eq!(name.as_str(), text);
    assert_eq!(name.as_ref(), text);
    assert_eq!(name.to_string(), text);
    assert_eq!(name.clone().into_inner(), text);
    assert_eq!(ShellName::try_from(text), Ok(name.clone()));
    assert_eq!(ShellName::try_from(text.to_owned()), Ok(name.clone()));
    assert_eq!(text.parse::<ShellName>(), Ok(name));
}

#[rstest]
#[case(1)]
#[case(63)]
fn accepts_boundary_lengths(#[case] length: usize) {
    assert_eq!(
        ShellName::parse(&"a".repeat(length))
            .expect("boundary must parse")
            .as_str()
            .len(),
        length
    );
    assert_eq!(ShellName::MAX_LEN, 63);
}

#[rstest]
#[case("")]
#[case("/bin/bash")]
#[case("bash.exe")]
#[case(r"C:\Windows\System32\bash.exe")]
#[case("{{ shell }}")]
#[case(" bash")]
#[case("Bash")]
#[case("1bash")]
#[case("_bash")]
#[case("-bash")]
#[case("a\0")]
#[case("aé")]
fn rejects_invalid_names(#[case] text: &str) {
    assert!(ShellName::parse(text).is_err(), "accepted {text:?}");
    assert!(ShellName::try_from(text).is_err());
    assert!(ShellName::try_from(text.to_owned()).is_err());
    assert!(text.parse::<ShellName>().is_err());
}

#[rstest]
#[case("true")]
#[case("false")]
#[case("yes")]
#[case("no")]
#[case("y")]
#[case("n")]
#[case("on")]
#[case("off")]
fn rejects_reserved_words(#[case] word: &'static str) {
    assert_eq!(
        ShellName::parse(word),
        Err(ShellNameError::ReservedWord { word })
    );
}

#[rstest]
#[case(String::new(), ShellNameError::Empty)]
#[case("aaB".repeat(23) + "a", ShellNameError::TooLong { length: 70 })]
#[case("é".repeat(70), ShellNameError::TooLong { length: 140 })]
#[case("a".repeat(64), ShellNameError::TooLong { length: 64 })]
#[case("Bash".into(), ShellNameError::InvalidFirstCharacter { character: 'B' })]
#[case("é".into(), ShellNameError::InvalidFirstCharacter { character: 'é' })]
#[case("ba/sh".into(), ShellNameError::InvalidCharacter { character: '/', char_index: 2 })]
#[case("abé/".into(), ShellNameError::InvalidCharacter { character: 'é', char_index: 2 })]
#[case("true".into(), ShellNameError::ReservedWord { word: "true" })]
#[case("True".into(), ShellNameError::InvalidFirstCharacter { character: 'T' })]
fn error_precedence(#[case] text: String, #[case] expected: ShellNameError) {
    assert_eq!(ShellName::parse(&text), Err(expected));
}

#[rstest]
#[case(serde_json::json!(false))]
#[case(serde_json::json!(1))]
#[case(serde_json::Value::Null)]
#[case(serde_json::json!([]))]
#[case(serde_json::json!({}))]
#[case(serde_json::json!("true"))]
fn rejects_non_name_serialized_values(#[case] value: serde_json::Value) {
    assert!(serde_json::from_value::<ShellName>(value).is_err());
}
