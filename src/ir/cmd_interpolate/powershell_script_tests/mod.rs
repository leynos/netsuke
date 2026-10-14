//! Share PowerShell script interpolation fixtures and organize its regression suites.

//! Test PowerShell script interpolation and comment-boundary rules.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

mod comments;
mod path_interpolation;
mod rejection;
mod stop_parsing;

const HOSTILE_SOURCE_PATH: &str = "; New-Item pwned; Write-Output x";
const OUTPUT_PATH: &str = "output file.txt";

enum ScriptExpectation {
    /// Require invalid templates to retain their original diagnostic text.
    Reject,
    /// Compare successful interpolation with an independent literal expectation.
    Exact(String),
}

/// Group arbitrary template text with the shell, bindings, and expected outcome.
struct ScriptScenario {
    template: String,
    shell: RecipeShell,
    inputs: Vec<Utf8PathBuf>,
    outputs: Vec<Utf8PathBuf>,
    expectation: ScriptExpectation,
}

impl ScriptScenario {
    /// Run one interpolation case and verify its declared outcome.
    fn verify(self) {
        let bindings = CommandBindings::new(&self.inputs, &self.outputs, self.shell);
        let actual = interpolate_script_with_bindings(&self.template, &bindings);
        match self.expectation {
            ScriptExpectation::Reject => {
                let error = actual.expect_err("the script scenario must reject its marker");
                assert!(
                    matches!(&error, IrGenError::InvalidCommand { command, .. } if command == &self.template),
                    "expected InvalidCommand to retain the original template {:?}, got {error:?}",
                    self.template
                );
            }
            ScriptExpectation::Exact(expected) => {
                let interpolated = actual.expect("the script scenario must interpolate");
                assert_eq!(
                    interpolated, expected,
                    "unexpected output for {:?}",
                    self.template
                );
            }
        }
    }
}

/// Hold the two delimiters used by a typographic-quote regression scenario.
struct QuotePair {
    opening: &'static str,
    closing: &'static str,
}

/// Preserve arbitrary text while comparing a literal path replacement result.
struct HereStringQuoteCase {
    template: String,
    marker: &'static str,
    encoded_path: &'static str,
}

/// Identify an argument fragment whose hash must remain inside its token.
struct ScriptArgumentCase {
    fragment: &'static str,
}

macro_rules! script_scenario {
    ($template:expr, $shell:expr, $inputs:expr, $outputs:expr, $expectation:expr) => {
        ScriptScenario {
            template: $template,
            shell: $shell,
            inputs: $inputs,
            outputs: $outputs,
            expectation: $expectation,
        }
    };
}

macro_rules! power_shell_rejection_case {
    ($template:expr) => {
        script_scenario!(
            $template,
            RecipeShell::PowerShell,
            vec![Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
            vec![Utf8PathBuf::from(OUTPUT_PATH)],
            ScriptExpectation::Reject
        )
    };
}

macro_rules! power_shell_output_case {
    ($template:expr, $expected:expr) => {
        script_scenario!(
            $template,
            RecipeShell::PowerShell,
            vec![Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
            vec![Utf8PathBuf::from(OUTPUT_PATH)],
            ScriptExpectation::Exact($expected.to_owned())
        )
    };
}

macro_rules! posix_output_case {
    ($template:expr, $expected:expr $(,)?) => {
        script_scenario!(
            $template,
            RecipeShell::Posix,
            vec![Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
            vec![Utf8PathBuf::from(OUTPUT_PATH)],
            ScriptExpectation::Exact($expected.to_owned())
        )
    };
}

pub(super) use posix_output_case;
pub(super) use power_shell_output_case;
pub(super) use power_shell_rejection_case;
pub(super) use script_scenario;

/// Prepare bindings with a Windows-valid path that can expose source injection.
fn script_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
        &[Utf8PathBuf::from(OUTPUT_PATH)],
        RecipeShell::PowerShell,
    )
}
