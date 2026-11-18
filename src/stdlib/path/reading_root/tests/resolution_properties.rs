//! Property tests for the lexical rule resolution applies to a request.
//!
//! The parent module's tables name spellings a reviewer can read. This module
//! covers the space between them: whatever sequence of `.`, `..` and ordinary
//! names a template composes, resolution must keep exactly the components the
//! filesystem has to interpret, in order, and must refuse a request only when
//! it begins by climbing above the root. A regression that folds `..` away
//! before the open, which is the defect this resolver exists to prevent, fails
//! here on inputs no table row spells.

use camino::Utf8Path;
use minijinja::ErrorKind;
use proptest::prelude::*;

use super::scratch;

/// One component a template can spell in a request.
///
/// `...` and `..hidden` are ordinary names that merely resemble a climb, so a
/// case using them shows they are kept rather than read as `..`.
fn segment() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("."),
        Just(".."),
        Just("file"),
        Just("sub"),
        Just("..."),
        Just("..hidden"),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Resolution keeps every component after a root-side start, and refuses a
    /// request that climbs before naming anything.
    #[test]
    fn resolution_keeps_each_component_and_refuses_only_a_leading_climb(
        segments in prop::collection::vec(segment(), 1..=6),
    ) {
        // Every generated request is relative, so the scratch tree supplies a
        // capability without constraining the answer.
        let (_temp, _path, root) = scratch().expect("the scratch tree must be creatable");
        let request = segments.join("/");

        // `.` names nothing, so it is the one component resolution may drop.
        let expected: Vec<&str> = segments
            .iter()
            .copied()
            .filter(|part| *part != ".")
            .collect();
        let outcome = root.resolve(Utf8Path::new(&request));

        if expected.first() == Some(&"..") {
            let err = outcome
                .expect_err("a request that climbs before naming anything must be refused");
            prop_assert_eq!(
                err.kind(),
                ErrorKind::InvalidOperation,
                "request {:?} should be reported as outside the workspace",
                request
            );
        } else {
            let literal = outcome.expect("a request naming a root-side component must resolve");
            let kept: Vec<&str> = literal.components().map(|part| part.as_str()).collect();
            prop_assert_eq!(kept, expected, "request {:?}", request);
            prop_assert!(
                !literal.is_absolute(),
                "request {:?} resolved to the absolute path {:?}",
                request,
                literal
            );
        }
    }
}
