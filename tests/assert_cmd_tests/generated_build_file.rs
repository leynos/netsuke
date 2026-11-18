//! Detect the generated Ninja build file in rendered diagnostics.
//!
//! Both CLI diagnostic suites assert that a summary does not name the
//! temporary `netsuke.<random>.ninja` file, and both need the same detector.
//! It runs over rendered sentences, so it must see a path that punctuation
//! abuts while still treating a `.ninja` file beneath an innocuous directory
//! as something the diagnostic may name.

use std::path::Path;

/// Whether text names the generated `netsuke.<random>.ninja` build file.
///
/// The `netsuke.` prefix alone is not enough: a temporary directory may
/// legitimately be called `netsuke.review`, and the diagnostic has to name
/// that path. The generated file is a path whose final component starts with
/// `netsuke.` and ends with `.ninja`, possibly followed by sentence
/// punctuation.
pub(super) fn names_generated_build_file(text: &str) -> bool {
    text.split(|character: char| character.is_whitespace() || character == '`')
        .map(trim_prose_punctuation)
        .any(|token| {
            Path::new(token)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("netsuke.")
                        && Path::new(name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("ninja"))
                })
        })
}

/// Strip the punctuation a rendered sentence can place beside a path.
fn trim_prose_punctuation(token: &str) -> &str {
    token.trim_matches(|character: char| {
        matches!(
            character,
            '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '.' | '!' | '?'
        )
    })
}

/// A sentence that ends with the generated file still names it.
#[test]
fn trailing_punctuation_does_not_hide_the_generated_file() {
    assert!(
        names_generated_build_file("Generated file: /tmp/netsuke.abc.ninja."),
        "a final period must not hide the generated build file"
    );
    assert!(
        names_generated_build_file("(see `/tmp/netsuke.abc.ninja`),"),
        "surrounding punctuation must not hide the generated build file"
    );
}

/// A `.ninja` path beneath a directory named `netsuke.review` is not it.
#[test]
fn a_directory_named_netsuke_review_is_not_the_generated_file() {
    assert!(
        !names_generated_build_file("/tmp/netsuke.review/foo.ninja"),
        "a .ninja file beneath that directory may be named"
    );
    assert!(
        !names_generated_build_file("/tmp/netsuke.review/ninja-not-executable"),
        "a program beneath that directory may be named"
    );
    assert!(
        names_generated_build_file("/tmp/netsuke.review/netsuke.abc.ninja"),
        "the generated basename is still detected inside that directory"
    );
}
