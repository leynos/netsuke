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
}
