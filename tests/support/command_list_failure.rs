//! Shared assertions for command-list failure diagnostics in CLI tests.

/// Prefix emitted for command-list failure markers.
pub const FAILURE_PREFIX: &str = "netsuke command-list failure: action ";

/// Return the first command-list marker suffix present in `message`.
pub fn failure_marker_suffix(message: &str) -> Option<&str> {
    message
        .split_once(FAILURE_PREFIX)
        .map(|(_, suffix)| suffix.lines().next().unwrap_or_default())
}

/// Check that `message` contains a marker for the requested entry number.
pub fn identifies_entry(message: &str, expected_entry: usize) -> bool {
    failure_marker_suffix(message)
        .and_then(|suffix| suffix.split_once(", entry ").map(|(_, entry)| entry))
        .and_then(|entry| entry.split_whitespace().next())
        .and_then(|entry| entry.parse::<usize>().ok())
        == Some(expected_entry)
}

/// Verify that `message` retains a full fixed-width hexadecimal fingerprint.
pub fn contains_full_fingerprint(message: &str) -> bool {
    failure_marker_suffix(message)
        .and_then(|suffix| {
            suffix
                .split_once(", entry ")
                .map(|(fingerprint, _)| fingerprint)
        })
        .is_some_and(|fingerprint| {
            fingerprint.len() == 64 && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// Match a quoted target clause in plain or escaped diagnostic output.
pub fn contains_target_clause(message: &str, clause: &str) -> bool {
    message.contains(clause) || message.contains(&clause.replace('"', "\\\""))
}
