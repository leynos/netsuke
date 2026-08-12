//! Shell-safe rendering for ordered Ninja command-list entries.

use sha2::{Digest, Sha256};

/// Prefix used to carry bounded list-entry failure attribution through Ninja.
pub(crate) const COMMAND_LIST_FAILURE_PREFIX: &str = "netsuke command-list failure: action ";

/// Render one entry so it fails atomically without exposing command content.
pub(crate) fn command_list_entry(command: &str, action_id: &str, entry_index: usize) -> String {
    let identity = action_identity(action_id);
    let context = format!("{COMMAND_LIST_FAILURE_PREFIX}{identity}, entry {entry_index}");
    format!(
        concat!(
            "{{ _netsuke_background_before=$${{!:-}}; ",
            "trap '_netsuke_command_status=$$?; printf \"%s\\n\" \"{}\" >&2; ",
            "trap - EXIT; exit \"$$_netsuke_command_status\"' EXIT; ",
            "if eval {}; then _netsuke_command_status=0; ",
            "else _netsuke_command_status=$$?; fi; ",
            "_netsuke_background_after=$${{!:-}}; ",
            "if [ -n \"$$_netsuke_background_after\" ] && ",
            "[ \"$$_netsuke_background_after\" != \"$$_netsuke_background_before\" ]; then ",
            "wait \"$$_netsuke_background_after\"; _netsuke_command_status=$$?; fi; ",
            "if [ \"$$_netsuke_command_status\" -eq 0 ]; then trap - EXIT; :; ",
            "else trap - EXIT; printf '%s\\n' '{}' >&2; ",
            "exit \"$$_netsuke_command_status\"; fi; }}"
        ),
        context,
        shell_single_quote(command),
        context,
    )
}

/// Return a fixed-width fingerprint for an action identifier.
///
/// IR-generated identifiers are already hashes, but hashing again prevents a
/// programmatically supplied identifier from disclosing arbitrary content.
fn action_identity(action_id: &str) -> String {
    let digest = Sha256::digest(action_id.as_bytes());
    let mut identity = String::with_capacity(digest.len() * 2);
    for byte in digest {
        identity.push(hex_digit(byte >> 4));
        identity.push(hex_digit(byte & 0x0f));
    }
    identity
}

const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

/// Quote `value` as one literal POSIX shell argument.
///
/// The command-list renderer passes each entry to `eval` so an inline comment
/// or trailing control operator cannot consume the brace-group terminator.
fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', r"'\''");
    format!("'{escaped}'")
}
