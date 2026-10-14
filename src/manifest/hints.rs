//! YAML error hint mappings for manifest diagnostics.
//!
//! # Maintenance
//!
//! The needles match substrings of the parser's own error prose, so they are a
//! deliberate, narrow coupling to the underlying YAML backend. That backend is
//! not part of Netsuke's API: `serde-saphyr` 0.0.6 used `saphyr-parser`, whose
//! messages the original needles were written against, and `serde-saphyr`
//! 1.2.0 replaced it with `granit-parser`.
//!
//! Where a hint has an equivalent in both vocabularies both spellings are
//! listed. The needles are matched with `contains`, so an inactive alias never
//! fires; add an alias rather than replacing one spelling with the other. The
//! aliases are not free, so verify one is actually needed before adding it:
//! granit renders `simple key expected ':'`, which the existing `expected ':'`
//! needle already matches, and listing the granit spelling separately would
//! have been redundant.
//!
//! Dropping a needle silently removes a user-visible hint without failing any
//! test, so prefer widening to deleting.

use crate::localization::keys;

/// Error-substring to localization-key hints for common YAML mistakes.
pub(crate) const YAML_HINTS: [(&str, &str); 6] = [
    (
        "did not find expected '-'",
        keys::MANIFEST_YAML_HINT_LIST_ITEM,
    ),
    // Unchanged across both backends: granit still renders this as
    // `simple key expected ':'`, which contains this needle.
    ("expected ':'", keys::MANIFEST_YAML_HINT_EXPECTED_COLON),
    (
        "mapping values are not allowed",
        keys::MANIFEST_YAML_HINT_MAPPING_VALUES,
    ),
    // Saphyr's spelling: `found character that cannot start any token`.
    (
        "found character that cannot start any token",
        keys::MANIFEST_YAML_HINT_INVALID_TOKEN,
    ),
    // Granit's spelling of the same scanner failure, which the saphyr needle
    // does not match: `unexpected character: `@'`.
    (
        "unexpected character",
        keys::MANIFEST_YAML_HINT_INVALID_TOKEN,
    ),
    ("unknown escape character", keys::MANIFEST_YAML_HINT_ESCAPE),
];
