//! OBL-SH-ROUNDTRIP and OBL-PS-ROUNDTRIP: an oracle reads back what was quoted.
//!
//! The POSIX obligation is discharged by a real shell: the encoder's output is
//! handed to `sh` in argument position and `printf %s` returns the word. The
//! PowerShell obligation has no interpreter to hand on most hosts, so it is
//! discharged by a *decoder* written independently of the encoder.
//!
//! Both are round trips, and a round trip is only evidence when the oracle can
//! fail: [`the_posix_harness_rejects_a_naive_quoter`] and
//! [`the_power_shell_decoder_rejects_posix_quoting`] supply that, and
//! [`the_generated_corpus_spans_the_quoting_boundary`] shows the generator
//! reaches the inputs where a wrong answer would show.
use super::property_support::{
    POWERSHELL, decode_power_shell_literal, decode_through_posix_shell, encoded_sh, posix_shell,
    quote_value, word,
};
use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
// The only `#[rstest]` case here is the `#[cfg(unix)]` control, so an
// unconditional import is an unused-import error on Windows, where
// `-D warnings` is a merge gate.
#[cfg(unix)]
use rstest::rstest;
use std::cell::Cell;
use std::process::Command;

// ---------------------------------------------------------------------------
// OBL-SH-ROUNDTRIP
// ---------------------------------------------------------------------------

proptest! {
    // 64 cases, not 128: every case forks a shell, and the whole file must
    // finish inside the 10s budget the milestones set for it. The corpus is
    // still wide enough to reach the boundary — the companion test below
    // asserts that it does rather than assuming it.
    #![proptest_config(ProptestConfig {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// A real shell reads back exactly what was quoted.
    #[test]
    fn sh_quoting_round_trips_through_a_real_shell(value in word()) {
        let Some(shell) = posix_shell() else {
            // No POSIX shell on this host: the obligation is undischarged here
            // and the `#[cfg(unix)]` companion below says so explicitly. Failing
            // rather than passing keeps the gap visible — a silent return would
            // report green for a property that never ran.
            return Err(TestCaseError::fail("no POSIX shell available"));
        };
        let encoded = encoded_sh(&value)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let decoded = decode_through_posix_shell(&shell, &encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&decoded, &value, "quoted {:?} as {:?}", value, encoded);
    }
}

/// The subprocess harness rejects a quoter that is wrong.
///
/// The witness is the plan's own worst case: `$HOME 'x'` contains an expansion,
/// a space, and a single quote. Wrapping it in double quotes — the naive fix a
/// manifest author reaches for — leaves the expansion live and the quote
/// unbalanced, so a real shell does not return the input. If this ever passes,
/// [`decode_through_posix_shell`] is not measuring anything and every green case
/// above is void.
#[cfg(unix)]
#[rstest]
fn the_posix_harness_rejects_a_naive_quoter() -> Result<()> {
    let shell = posix_shell().context("a POSIX shell is required for this control")?;
    let witness = "$HOME 'x'";
    let naive = format!("\"{witness}\"");
    let decoded = decode_through_posix_shell(&shell, &naive)?;
    // `ensure!` rather than `assert_ne!` throughout this file: the workspace
    // denies `clippy::panic_in_result_fn`, so a `Result`-returning test reports
    // a failure by returning it.
    ensure!(
        decoded != witness,
        "the harness accepted a naive double-quoting; it is not measuring quoting"
    );
    Ok(())
}

/// The generated corpus reaches the quoting boundary, not just alphanumerics.
///
/// The property above can only fail on inputs that need quoting. Without this,
/// a generator that happened to produce nothing but inert characters would let
/// the property pass while testing nothing. The tallies live in a `RefCell`
/// because `TestRunner::run` takes an `Fn`.
#[test]
fn the_generated_corpus_spans_the_quoting_boundary() {
    let tallies: Cell<Corpus> = Cell::new(Corpus::default());
    TestRunner::new(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })
    .run(&word(), |value| {
        tallies.set(tallies.get().extended(&value));
        Ok(())
    })
    .expect("the word corpus should be generatable");

    let observed = tallies.into_inner();
    assert!(
        observed.with_quote > 0
            && observed.with_dollar > 0
            && observed.with_space > 0
            && observed.with_control > 0
            && observed.empty > 0
            && observed.quoted > 0,
        "the corpus must reach every quoting boundary: {observed:?}"
    );
}

/// Tallies of the quoting boundaries a generated corpus reached.
#[derive(Clone, Copy, Debug, Default)]
struct Corpus {
    /// Words containing a single quote, the escaping case.
    with_quote: usize,
    /// Words containing a dollar sign, the expansion case.
    with_dollar: usize,
    /// Words containing a space, the splitting case.
    with_space: usize,
    /// Words containing a C0 control character other than the three rejected.
    with_control: usize,
    /// The empty word, which the encoder must render as `''`.
    empty: usize,
    /// Words the encoder did not leave bare, i.e. cases that exercised quoting.
    quoted: usize,
}

impl Corpus {
    /// Fold one generated word into the tallies, returning the result.
    ///
    /// `Cell::get` needs `Copy` and `Cell::set` needs a whole value, so the
    /// tally is a fold rather than an in-place mutation.
    fn extended(self, value: &str) -> Self {
        // "Did this need quoting" is asked of the encoder, through the same
        // filter the property exercises, rather than of a re-derived character
        // class that could disagree with it.
        let needed_quoting = encoded_sh(value).is_ok_and(|encoded| encoded != value);
        Self {
            with_quote: self.with_quote + usize::from(value.contains('\'')),
            with_dollar: self.with_dollar + usize::from(value.contains('$')),
            with_space: self.with_space + usize::from(value.contains(' ')),
            with_control: self.with_control
                + usize::from(
                    value.chars().any(|character| {
                        character.is_control() && !matches!(character, '\n' | '\r')
                    }),
                ),
            empty: self.empty + usize::from(value.is_empty()),
            quoted: self.quoted + usize::from(needed_quoting),
        }
    }
}

// ---------------------------------------------------------------------------
// OBL-PS-ROUNDTRIP
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// The PowerShell model decodes back to the input for every word.
    #[test]
    fn power_shell_quoting_round_trips_through_the_model(value in word()) {
        let encoded = quote_value(&value, POWERSHELL)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let decoded = decode_power_shell_literal(&encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&decoded, &value, "quoted {:?} as {:?}", value, encoded);
    }
}

/// The PowerShell decoder rejects POSIX quoting.
///
/// Without this, a decoder that returned its input unchanged — or one that
/// merely stripped the first and last byte — would satisfy the property above
/// for every word the POSIX encoder already leaves bare. The witness is chosen
/// so the two encoders genuinely differ: POSIX suffix-quoting emits `a' b'`,
/// which has no enclosing quotes to strip.
#[test]
fn the_power_shell_decoder_rejects_posix_quoting() -> Result<()> {
    let posix = encoded_sh("a b")?;
    ensure!(
        posix == "a' b'",
        "POSIX quoting changed shape: {posix:?}, so the witness is no longer a bare word"
    );
    let decoded = decode_power_shell_literal(&posix);
    ensure!(
        decoded.is_err(),
        "the decoder accepted POSIX output {posix:?} as a PowerShell literal"
    );
    Ok(())
}

/// The name of an interpreter to try, in the order a host is likely to have it.
///
/// `powershell.exe` is the Windows PowerShell the recipes actually run under;
/// `pwsh` is PowerShell Core, which parses single-quoted strings identically
/// and so can discharge the same axiom on a non-Windows CI host.
const POWER_SHELL_CANDIDATES: &[&str] = &["powershell.exe", "pwsh", "powershell"];

/// Run `script` under the first PowerShell interpreter this host provides.
///
/// # Errors
///
/// Returns `Ok(None)` when no interpreter is installed, or an error when one is
/// installed and fails to run.
fn run_power_shell(script: &str) -> Result<Option<String>> {
    for candidate in POWER_SHELL_CANDIDATES {
        let output = match Command::new(candidate)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("run {candidate}"));
            }
        };
        ensure!(
            output.status.success(),
            "{candidate} exited with {} for {script}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout)
            .with_context(|| format!("{candidate} stdout should be valid UTF-8"))?;
        // PowerShell terminates its output with a newline the writer did not
        // ask for; the round trip is about the bytes, so it is stripped here
        // rather than papered over in the assertion.
        return Ok(Some(stdout.trim_end_matches(['\r', '\n']).to_owned()));
    }
    Ok(None)
}

/// The model agrees with the real interpreter where one is present.
///
/// On Linux the model is the only available oracle, which is why AXIOM-2 rests
/// on it there; this case discharges the gap wherever an interpreter exists
/// (Windows CI, or a host with PowerShell Core installed).
#[test]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable interpreter must be visible in the captured test output instead of the case passing silently"
)]
fn the_power_shell_model_matches_the_real_interpreter() -> Result<()> {
    for value in ["a b", "it's", "$HOME", "", "a\"b", "中", "x\ty"] {
        let encoded = quote_value(value, POWERSHELL)?;
        let script = format!("[Console]::Out.Write({encoded})");
        let Some(output) = run_power_shell(&script)? else {
            eprintln!("skipped: no PowerShell interpreter on this host");
            return Ok(());
        };
        ensure!(
            output == value,
            "the interpreter read {output:?} back from {encoded} for {value:?}"
        );
    }
    Ok(())
}
