pub(crate) const YAML_HINTS: [(&str, &str); 5] = [
    (
        "did not find expected '-'",
        "Start list items with '-' and ensure proper indentation.",
    ),
    (
        "expected ':'",
        "Ensure each key is followed by ':' separating key and value.",
    ),
    (
        "mapping values are not allowed",
        "Check for a stray ':' or add quotes around values where needed.",
    ),
    (
        "found character that cannot start any token",
        "Remove stray characters and ensure indentation uses spaces (no tabs).",
    ),
    (
        "unknown escape character",
        "Use valid YAML escape sequences or quote the string.",
    ),
];
