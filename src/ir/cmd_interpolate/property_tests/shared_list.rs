//! Property tests for shared PowerShell command-list interpolation.
//!
//! A shared list is parsed as one script, so an entry may not leave a lexical
//! region open for its successor. These properties generate both the hostile
//! prefixes and the neutral ones, and check the marker still lowers as a
//! standalone literal after a neutral entry.

use proptest::prelude::*;

use super::super::{
    CommandBindings, INS_TOKEN, IrGenError, PowerShellListCarry, RecipeShell,
    interpolate_shared_power_shell_entry,
};
use super::support::power_shell_path_strategy;

/// Classify one entry in a generated shared command-list sequence.
#[derive(Clone, Copy, Debug)]
enum SequenceEntry {
    /// A completed ordinary command that leaves the scanner unquoted.
    Neutral,
    /// A here-string that closes on its own terminator line.
    ClosedHereString,
    /// An entry that leaves a single-quoted string open.
    OpenQuote,
    /// An entry that leaves a block comment open.
    OpenBlockComment,
    /// An entry whose escaped separator and quote hide a live string.
    EscapedBoundary,
    /// A mid-token opener that makes later markers fail closed.
    MidTokenOpener,
    /// An ordinary entry whose input marker must lower as one literal.
    Marker,
}

impl SequenceEntry {
    /// Render the entry's recipe text.
    fn text(self) -> String {
        match self {
            Self::Neutral => "Write-Output neutral".to_owned(),
            Self::ClosedHereString => "@'\nbody\n'@".to_owned(),
            Self::OpenQuote => "Write-Output 'open".to_owned(),
            Self::OpenBlockComment => "Write-Output <# note".to_owned(),
            Self::EscapedBoundary => "Write-Output prefix` <#\" #>".to_owned(),
            Self::MidTokenOpener => "Write-Output prefix<#".to_owned(),
            Self::Marker => format!("Copy-Item {INS_TOKEN}"),
        }
    }

    /// Report whether PowerShell is left inside a region after this entry.
    const fn leaves_region_open(self) -> bool {
        matches!(
            self,
            Self::OpenQuote | Self::OpenBlockComment | Self::EscapedBoundary
        )
    }

    /// Report whether this entry makes later markers fail closed.
    const fn fails_closed(self) -> bool {
        matches!(self, Self::MidTokenOpener)
    }

    /// Report whether this entry carries the interpolated input marker.
    const fn is_marker(self) -> bool {
        matches!(self, Self::Marker)
    }

    /// Report whether the carry rejects this entry at its own position.
    ///
    /// An unfinished region is rejected immediately; a marker is rejected once
    /// an earlier entry has made the carry fail closed.
    const fn is_rejected_with(self, is_fail_closed: bool) -> bool {
        self.leaves_region_open() || (is_fail_closed && self.is_marker())
    }
}

/// Predict the first entry a generated sequence must reject.
///
/// The model is independent of the product. It walks the entries in order: an
/// entry that leaves a region open is rejected at that entry, and a marker is
/// rejected at its own entry once any earlier entry has made the carry fail
/// closed. The walk stops at the first rejection, so a later unfinished entry
/// behind an earlier rejection is never reached.
fn expected_rejection_index(sequence: &[SequenceEntry]) -> Option<usize> {
    let mut is_fail_closed = false;
    for (index, entry) in sequence.iter().enumerate() {
        if entry.is_rejected_with(is_fail_closed) {
            return Some(index);
        }
        is_fail_closed |= entry.fails_closed();
    }
    None
}

proptest! {
    /// Reject every shared-list entry that leaves a generated region open.
    ///
    /// Each fragment is a real unfinished region the PowerShell scanner can
    /// recognise. The rejection must name the entry and carry its original
    /// template, so the diagnostic never depends on interpolated path text.
    #[test]
    fn shared_power_shell_lists_reject_generated_unfinished_regions(
        region in prop::sample::select(vec![
            "Write-Output 'open",
            "Write-Output \"open",
            "@'\nbody",
            "Write-Output <# note",
            "Write-Output $(Get-Date",
            "Write-Output open`",
        ]),
        input in power_shell_path_strategy(),
    ) {
        // The rendered diagnostic is localized, and the localizer is
        // process-wide, so pin English rather than inheriting whichever locale
        // another test last installed.
        let _en_localizer = test_support::en_localizer();
        let inputs = [input];
        let outputs = [camino::Utf8PathBuf::from("out.txt")];
        let bindings = CommandBindings::new(&inputs, &outputs, RecipeShell::PowerShell);
        let error = interpolate_shared_power_shell_entry(
            region,
            &bindings,
            PowerShellListCarry::new(),
            1,
        )
        .expect_err("an unfinished shared-list entry must be rejected");

        prop_assert!(
            matches!(&error, IrGenError::InvalidCommand { command, .. } if command == region),
            "the rejection must carry the original entry template: {error:?}"
        );
        let rendered = test_support::fluent::normalize_fluent_isolates(&error.to_string());
        prop_assert!(rendered.contains("entry 1"), "entry index missing: {rendered}");
        prop_assert!(
            !rendered.contains("NETSUKE"),
            "the diagnostic must not carry interpolated markers: {rendered}"
        );
    }

    /// Interpolate a marker after a neutral shared-list entry as one literal.
    ///
    /// These entries all close the region they open, so the next entry starts
    /// unquoted and its path is lowered to a single standalone literal.
    #[test]
    fn shared_power_shell_lists_interpolate_after_neutral_entries(
        neutral in prop::sample::select(vec![
            "Write-Output 'closed'",
            "@'\nbody\n'@",
            "# ordinary note",
            "cmd.exe /c --% literal",
            "Write-Output <# note #> body",
        ]),
        input in power_shell_path_strategy(),
    ) {
        let inputs = [input];
        let outputs = [camino::Utf8PathBuf::from("out.txt")];
        let bindings = CommandBindings::new(&inputs, &outputs, RecipeShell::PowerShell);
        let (_, carry) =
            interpolate_shared_power_shell_entry(neutral, &bindings, PowerShellListCarry::new(), 1)
                .expect("a neutral entry should interpolate");
        let (interpolated, _) = interpolate_shared_power_shell_entry(
            &format!("Copy-Item {INS_TOKEN}"),
            &bindings,
            carry,
            2,
        )
        .expect("a marker after a neutral entry should interpolate");

        let literal = format!("'{}'", inputs[0].as_str().replace('\'', "''"));
        prop_assert!(
            interpolated.contains(&literal),
            "the path must stay one literal after {neutral:?}: {interpolated}"
        );
    }

    /// Follow the shared carry through a generated sequence of entries.
    ///
    /// The expected outcome comes from the fragment classes, not from the
    /// product, so a carry that resets early, a rejection that names the wrong
    /// entry, or a marker that leaks into the diagnostic fails here even when
    /// the fixed examples still pass.
    #[test]
    fn shared_power_shell_list_sequences_follow_their_carry_state(
        sequence in prop::collection::vec(
            prop::sample::select(vec![
                SequenceEntry::Neutral,
                SequenceEntry::ClosedHereString,
                SequenceEntry::OpenQuote,
                SequenceEntry::OpenBlockComment,
                SequenceEntry::EscapedBoundary,
                SequenceEntry::MidTokenOpener,
                SequenceEntry::Marker,
            ]),
            1..5,
        ),
        input in power_shell_path_strategy(),
    ) {
        let _en_localizer = test_support::en_localizer();
        let inputs = [input.clone()];
        let outputs = [camino::Utf8PathBuf::from("out.txt")];
        let bindings = CommandBindings::new(&inputs, &outputs, RecipeShell::PowerShell);
        let expected_rejection = expected_rejection_index(&sequence);
        let literal = format!("'{}'", input.as_str().replace('\'', "''"));
        let mut carry = PowerShellListCarry::new();
        let mut accepted = 0;
        let mut rejection = None;
        for (index, entry) in sequence.iter().enumerate() {
            match interpolate_shared_power_shell_entry(&entry.text(), &bindings, carry, index + 1) {
                Ok((interpolated, next)) => {
                    carry = next;
                    accepted += 1;
                    if entry.is_marker() {
                        prop_assert!(
                            interpolated.contains(&literal),
                            "entry {} must keep the path as one literal: {interpolated}",
                            index + 1
                        );
                    }
                }
                Err(error) => {
                    rejection = Some((index, error));
                    break;
                }
            }
        }
        match (expected_rejection, rejection) {
            (Some(expected), Some((actual, error))) => {
                prop_assert_eq!(actual, expected, "the first affected entry must be rejected");
                let expected_text = sequence.get(expected).map(|entry| entry.text());
                match &error {
                    IrGenError::InvalidCommand { command, .. } => prop_assert_eq!(
                        Some(command.as_str()),
                        expected_text.as_deref(),
                        "the rejection must carry the offending entry template"
                    ),
                    other => {
                        prop_assert!(false, "expected an InvalidCommand rejection: {other:?}");
                    }
                }
                let rendered =
                    test_support::fluent::normalize_fluent_isolates(&error.to_string());
                if sequence
                    .get(expected)
                    .is_some_and(|entry| entry.leaves_region_open())
                {
                    prop_assert!(
                        rendered.contains(&format!("entry {}", expected + 1)),
                        "the diagnostic must name entry {}: {rendered}",
                        expected + 1
                    );
                } else {
                    prop_assert!(
                        rendered.contains("protected shell region"),
                        "a marker after a fail-closed entry must name the protected region: \
                         {rendered}"
                    );
                }
                prop_assert!(
                    !rendered.contains(input.as_str()),
                    "the diagnostic must not carry the interpolated path: {rendered}"
                );
            }
            (None, None) => prop_assert_eq!(
                accepted,
                sequence.len(),
                "every entry in a neutral sequence must lower"
            ),
            (expected, actual) => prop_assert!(
                false,
                "expected rejection at {expected:?}, observed {actual:?} after {accepted} entries"
            ),
        }
    }
}
