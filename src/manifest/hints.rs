//! YAML error hint mappings for manifest diagnostics.

use crate::localization::keys;

pub(crate) const YAML_HINTS: [(&str, &str); 5] = [
    (
        "did not find expected '-'",
        keys::MANIFEST_YAML_HINT_LIST_ITEM,
    ),
    ("expected ':'", keys::MANIFEST_YAML_HINT_EXPECTED_COLON),
    (
        "mapping values are not allowed",
        keys::MANIFEST_YAML_HINT_MAPPING_VALUES,
    ),
    (
        "found character that cannot start any token",
        keys::MANIFEST_YAML_HINT_INVALID_TOKEN,
    ),
    ("unknown escape character", keys::MANIFEST_YAML_HINT_ESCAPE),
];
