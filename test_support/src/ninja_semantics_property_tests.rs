//! Property tests for representation-aware generated-Ninja inspection.
//!
//! The cases in the parent module pin each transport with handwritten
//! samples; the properties below generalize them over arbitrary PowerShell
//! text, so a decoding fault that only appears for a particular mix of
//! quoting, separators, and non-ASCII characters cannot hide behind the
//! samples. Both alphabets used here are drawn from characters that cannot
//! spell a payload marker or a command binding prefix, so a generated document
//! only carries a payload where the property itself put one.

use super::{
    GeneratedNinja, PowerShellPayload, RecipeNeedle, RecipeTransport, ResponseFileContent,
    encode_power_shell,
};
use proptest::{prelude::*, test_runner::TestCaseError};

/// Convert a fallible inspection result into a property verdict.
///
/// A decode failure is a case failure rather than a rejection: these
/// properties assert that decoding succeeds, so declining the case would hide
/// exactly the fault they exist to find.
fn checked<T>(result: anyhow::Result<T>) -> Result<T, TestCaseError> {
    result.map_err(|error| TestCaseError::fail(format!("{error:#}")))
}

/// Build a non-empty text strategy over `alphabet`.
fn text_from(alphabet: Vec<char>) -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(alphabet), 1..24)
        .prop_map(|characters| characters.into_iter().collect())
}

/// PowerShell text that makes a UTF-16LE/Base64 round trip interesting.
///
/// The alphabet covers the characters PowerShell and Ninja both treat as
/// quoting or escaping, the line separators a rendered script is built from,
/// non-ASCII text, and a character outside the Basic Multilingual Plane, which
/// survives the round trip only as a UTF-16 surrogate pair.
fn power_shell_script() -> impl Strategy<Value = String> {
    text_from(vec![
        'a',
        'b',
        ' ',
        '\n',
        '\'',
        '"',
        '$',
        '`',
        ';',
        '=',
        'ü',
        '中',
        '\u{1f600}',
    ])
}

/// Plaintext recipe text: never able to spell a payload marker.
///
/// This is what the property searches for next to an encoded payload, so the
/// alphabet leaves out every character the two markers are built from
/// (`rspfile_content`, `netsukePayload`, `-EncodedCommand`, and
/// `powershell.exe`).
fn plaintext_recipe() -> impl Strategy<Value = String> {
    text_from(vec!['a', 'b', ' ', '\n', '\'', '"', '$', 'ü', '中'])
}

proptest! {
    /// Any PowerShell text survives a round trip through the binding that
    /// carries it, and the payload text itself never matches as plaintext.
    #[test]
    fn encoded_command_bindings_round_trip_arbitrary_power_shell_text(
        script in power_shell_script(),
    ) {
        let encoded = encode_power_shell(&script);
        let prefix = "  command = powershell.exe -NoProfile -EncodedCommand ";
        let generated = format!("{prefix}{encoded}\n");
        let document = GeneratedNinja::new(&generated);

        let decoded = checked(PowerShellPayload::new(&encoded).decode())?;
        prop_assert_eq!(
            decoded,
            script.clone(),
            "the decoder should recover the encoded script verbatim",
        );
        prop_assert_eq!(
            document.detected_recipe_transports(),
            vec![RecipeTransport::PowerShellEncodedCommand],
            "an encoded command binding should report the encoded-command transport",
        );
        let matched = checked(document.recipe_contains(RecipeNeedle::new(&script)))?;
        prop_assert!(
            matched,
            "the encoded binding should match the script it carries",
        );

        // The payload is reachable only by decoding. The guard keeps the
        // assertion about plaintext search, not about a generated script that
        // happens to contain its own encoding.
        prop_assume!(!prefix.contains(&encoded) && !script.contains(&encoded));
        let matched_encoded = checked(document.recipe_contains(RecipeNeedle::new(&encoded)))?;
        prop_assert!(
            !matched_encoded,
            "encoded payload text must not match as plaintext",
        );
    }

    /// A document carrying every transport reports each one once, in document
    /// order, and matches the text each encoding carries.
    #[test]
    fn mixed_documents_report_every_transport_and_match_each_payload(
        plaintext in plaintext_recipe(),
        response_script in power_shell_script(),
        command_script in power_shell_script(),
    ) {
        let response_binding = format!(
            "$$netsukePayload = '{}'; $$netsukeScript = 'bootstrap'",
            encode_power_shell(&response_script)
        );
        let generated = [
            format!("  command = {plaintext}"),
            format!("  rspfile_content = {response_binding}"),
            format!(
                "  command = powershell.exe -NoProfile -EncodedCommand {}",
                encode_power_shell(&command_script)
            ),
        ]
        .join("\n");
        let document = GeneratedNinja::new(&generated);

        prop_assert_eq!(
            document.detected_recipe_transports(),
            vec![
                RecipeTransport::PowerShellResponseFile,
                RecipeTransport::PowerShellEncodedCommand,
            ],
            "each transport should be reported once, in document order",
        );
        let matched_plaintext = checked(document.recipe_contains(RecipeNeedle::new(&plaintext)))?;
        prop_assert!(matched_plaintext, "plaintext recipe text should match");
        let matched_response = checked(document.recipe_contains(RecipeNeedle::new(&response_script)))?;
        prop_assert!(
            matched_response,
            "the response-file payload should decode to the script it carries",
        );
        let matched_command = checked(document.recipe_contains(RecipeNeedle::new(&command_script)))?;
        prop_assert!(
            matched_command,
            "the encoded-command payload should decode to the script it carries",
        );
        let decoded_response = checked(
            ResponseFileContent::new(&response_binding).decode_power_shell_payload(),
        )?;
        prop_assert_eq!(
            decoded_response,
            response_script,
            "the response-file binding should decode to the rendered script",
        );
    }
}
