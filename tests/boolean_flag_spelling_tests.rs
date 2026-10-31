//! Process-level contracts for the spelling of boolean command-line flags.
//!
//! `ortho_config` 0.10.0 taught its *generated* parsers to accept
//! `--flag=<BOOL>` with an optional value. Netsuke does not use a generated
//! parser: `src/cli/command.rs` declares every boolean as a plain
//! `#[arg(long)]` switch. These tests therefore record preservation, not new
//! behaviour. They exist because the migration changed the metadata that
//! *describes* those flags, and a future move to the generated loader would
//! otherwise be able to change what the binary accepts without any test
//! noticing.
//!
//! Exit status and stream routing are process-level facts, so every case runs
//! the real binary through `assert_cmd`. The child's environment is cleared and
//! pointed at a sandbox `HOME` so host configuration cannot change the parsed
//! values, and the locale is pinned to `en-US` so the wording these cases
//! assert is the wording the host's preferred language cannot replace.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use std::path::Path;
use std::process::Output;
use tempfile::TempDir;
use test_support::fluent::normalize_fluent_isolates;
use test_support::fs as test_fs;

/// Create a workspace holding the minimal manifest and an empty sandboxed home.
fn sandbox() -> Result<(TempDir, TempDir)> {
    let workspace = tempfile::tempdir().context("create boolean-spelling workspace")?;
    let home = tempfile::tempdir().context("create empty boolean-spelling home")?;
    test_fs::copy(
        "tests/data/minimal.yml",
        workspace.path().join("Netsukefile"),
    )?;
    Ok((workspace, home))
}

/// Run `netsuke` in the sandbox with the supplied arguments.
///
/// The locale is pinned to `en-US` ahead of the case's arguments: clearing the
/// environment removes the `LC_*` and `LANG` variables the locale detector
/// reads, but that leaves the choice to the host's fallback rather than making
/// it explicit. Every assertion in this file names English wording, so the
/// locale it was rendered in has to be part of the fixture.
fn run_netsuke(workspace: &Path, home: &Path, arguments: &[&str]) -> Result<Output> {
    run_netsuke_in_locale(workspace, home, Catalogue::English, arguments)
}

/// Run `netsuke` in the sandbox with an explicit catalogue.
///
/// The locale is passed as a flag rather than through the environment so it
/// cannot be overridden by anything the child inherits, and so a case can name
/// the catalogue its assertions are written against.
fn run_netsuke_in_locale(
    workspace: &Path,
    home: &Path,
    catalogue: Catalogue,
    arguments: &[&str],
) -> Result<Output> {
    let mut command = cargo_bin_cmd!("netsuke");
    command
        .current_dir(workspace)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_CONFIG_DIRS", home.join("empty-system-config"))
        .args(["--locale", catalogue.locale()])
        .args(arguments);
    command
        .output()
        .context("run netsuke with boolean arguments")
}

/// Run a rejection case in a fresh sandbox and return its normalized standard
/// error.
///
/// Every rejection case needs the same three facts: an isolated workspace, the
/// exact exit status, and a message with Fluent isolates normalized. Each
/// `#[case]` therefore supplies only the argument vector and the status it
/// should produce, and stays free to assert the message its own contract names.
fn rejected_stderr(arguments: &[&str], expected_status: i32) -> Result<String> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), arguments)?;
    let case = arguments.join(" ");
    ensure!(
        output.status.code() == Some(expected_status),
        "{case} should exit {expected_status}, got {:?}",
        output.status.code(),
    );
    ensure!(
        output.stdout.is_empty(),
        "{case} should write nothing to stdout, got {} bytes",
        output.stdout.len(),
    );
    Ok(normalize_fluent_isolates(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

/// Assert a case succeeded and return its standard output.
fn assert_succeeded(output: &Output, case: &str) -> Result<String> {
    ensure!(
        output.status.code() == Some(0),
        "{case} should exit 0, got {:?}; stderr: {}",
        output.status.code(),
        normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr)),
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A bare root-level switch is accepted and reaches the merged configuration.
///
/// `--verbose` is the vehicle because it has an observable effect on its own:
/// verbose mode emits `TRACE`-level events that a non-verbose run suppresses.
/// Rendering the empty manifest therefore proves the flag was parsed *and*
/// applied, rather than merely tolerated by clap.
#[rstest]
fn a_bare_root_switch_is_accepted_and_applied() -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(workspace.path(), home.path(), &["--verbose", "graph"])?;
    assert_succeeded(&output, "a bare --verbose switch")?;

    let stderr = normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr));
    ensure!(
        stderr.contains("TRACE"),
        "--verbose should raise the event filter to TRACE, got: {stderr}"
    );
    Ok(())
}

/// A bare switch on a subcommand is accepted and changes the artefact.
///
/// `graph --html` switches the rendered document from DOT to a self-contained
/// HTML page, so the produced bytes prove the flag was read rather than
/// defaulted.
#[rstest]
fn a_bare_subcommand_switch_is_accepted_and_applied() -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke(
        workspace.path(),
        home.path(),
        &["graph", "--html", "--output", "-"],
    )?;
    let stdout = assert_succeeded(&output, "a bare --html switch")?;
    ensure!(
        stdout.starts_with("<!doctype html>"),
        "--html should render an HTML document, got: {stdout:.60}"
    );
    Ok(())
}

/// A catalogue an unexpected-value diagnostic can be drawn from.
///
/// Naming the catalogue rather than passing a locale tag and its wording
/// separately keeps a case to one extra argument and makes it impossible to
/// pair the German wording with the English catalogue by hand.
#[derive(Clone, Copy)]
enum Catalogue {
    /// The source catalogue, and the language every English case asserts.
    English,
    /// German, chosen because its wording shares no word with the English one.
    German,
}

impl Catalogue {
    /// Return the locale tag that selects this catalogue.
    fn locale(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::German => "de",
        }
    }

    /// Return every run of text the diagnostic must contain for this catalogue.
    fn required(self) -> &'static [&'static str] {
        match self {
            Self::English => &["Unexpected value for"],
            Self::German => &["Unerwarteter Wert", "für"],
        }
    }
}

/// A case that clap must reject as an unexpected value.
struct Rejected<'a> {
    /// Arguments passed to `netsuke`.
    arguments: &'a [&'a str],
    /// Catalogue the diagnostic must be rendered from.
    catalogue: Catalogue,
}

/// The `--flag=value` and `--flag=` spellings are rejected as unexpected
/// values.
///
/// Clap reports too many values, so the process exits 2 and names the switch
/// and the value it saw. `--flag=` carries no value at all, so `ortho_config`
/// 0.10.0 drops the empty `InvalidValue` from the Fluent arguments and the
/// message cannot resolve `$value`; without the restoration in
/// `src/cli/localization/mod.rs` those cases log a missing-translation warning
/// and print clap's untranslated text. The German cases would also pass if the
/// message were never drawn from a translation, because an English fallback
/// reads the same as an English catalogue, so rendering the same rejection in
/// German proves the catalogue is consulted and both interpolations survive.
#[rstest]
#[case::equals_verbose(&["--verbose=false", "graph"], Catalogue::English)]
#[case::equals_json(&["--json=false", "graph"], Catalogue::English)]
#[case::equals_fetch_default_deny(&["--fetch-default-deny=false", "graph"], Catalogue::English)]
#[case::equals_trust_project_fetch_policy(
    &["--trust-project-fetch-policy=false", "graph"],
    Catalogue::English
)]
#[case::equals_no_input(&["--no-input=false", "graph"], Catalogue::English)]
#[case::equals_html(&["graph", "--html=false"], Catalogue::English)]
#[case::empty_verbose(&["--verbose=", "graph"], Catalogue::English)]
#[case::empty_html(&["graph", "--html="], Catalogue::English)]
#[case::german_equals(&["--verbose=false", "graph"], Catalogue::German)]
#[case::german_empty(&["--verbose=", "graph"], Catalogue::German)]
fn an_unexpected_value_is_rejected_in_the_selected_catalogue(
    #[case] arguments: &[&str],
    #[case] catalogue: Catalogue,
) -> Result<()> {
    assert_unexpected_value(&Rejected {
        arguments,
        catalogue,
    })
}

/// Assert a case is rejected as an unexpected value.
///
/// Every spelling that reaches clap as `TooManyValues` has to report the same
/// facts: the wording of the selected catalogue, the switch it applies to, and
/// the value clap saw. Asserting them in one place keeps the supplied-value,
/// empty-value, and non-English cases from drifting apart, and pins the absence
/// of a missing-translation warning for all of them.
fn assert_unexpected_value(shows: &Rejected<'_>) -> Result<()> {
    let (workspace, home) = sandbox()?;
    let output = run_netsuke_in_locale(
        workspace.path(),
        home.path(),
        shows.catalogue,
        shows.arguments,
    )?;
    let case = shows.arguments.join(" ");
    ensure!(
        output.status.code() == Some(2),
        "{case} should exit 2, got {:?}",
        output.status.code(),
    );
    ensure!(
        output.stdout.is_empty(),
        "{case} should write nothing to stdout, got {} bytes",
        output.stdout.len(),
    );
    let stderr = normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr));
    for wording in shows.catalogue.required() {
        ensure!(
            stderr.contains(wording),
            "{case} should render {wording:?}, got: {stderr}",
        );
    }
    let attached = attached_value(shows.arguments)?;
    ensure!(
        stderr.contains(attached.flag) && stderr.contains(attached.value),
        "{case} should name {} and {}, got: {stderr}",
        attached.flag,
        attached.value,
    );
    ensure!(
        !stderr.contains("missing translation"),
        "{case} should not log a missing translation warning: {stderr}",
    );
    Ok(())
}

/// The switch and value a case attaches to it.
struct Attached<'a> {
    /// Switch the diagnostic must name.
    flag: &'a str,
    /// Value the diagnostic must interpolate.
    value: &'a str,
}

/// Derive the switch and value a case attaches.
///
/// Each case spells its attachment inline, so both are read from the argument
/// that carries an `=`. Deriving them keeps the assertions free of a
/// hand-copied switch name or value that could disagree with what the case
/// actually passes. `--flag=` attaches nothing, and the adapter renders that
/// absence as `''`, so the empty attachment is normalized the same way here.
fn attached_value<'a>(arguments: &[&'a str]) -> Result<Attached<'a>> {
    let (flag, value) = arguments
        .iter()
        .find_map(|arg| arg.split_once('='))
        .context("case should contain a flag with an attached value")?;
    let rendered = if value.is_empty() { "''" } else { value };
    Ok(Attached {
        flag,
        value: rendered,
    })
}

/// The `--flag false` spelling is rejected too, but as a stray subcommand.
///
/// Because the switch takes no value, clap consumes only the flag and then
/// reads `false` as the next positional word. At the root that word is a
/// subcommand, and inside `graph` it is a stray argument, so the two tiers
/// report different errors. Both exit 2 and neither writes to standard output.
#[rstest]
#[case::root_switch(&["--verbose", "false", "graph"], StrayWord::AtRoot)]
#[case::subcommand_switch(&["graph", "--html", "false"], StrayWord::InSubcommand)]
fn a_space_separated_value_is_rejected_as_a_stray_word(
    #[case] arguments: &[&str],
    #[case] stray: StrayWord,
) -> Result<()> {
    let stderr = rejected_stderr(arguments, 2)?;
    let expected = stray.diagnostic();
    ensure!(
        stderr.contains(expected) && stderr.contains("false"),
        "{} should report {expected:?} naming `false`, got: {stderr}",
        arguments.join(" "),
    );
    Ok(())
}

/// The diagnostic clap reports for a word that has no positional to fill.
///
/// The tier decides it: at the root the word could only be a subcommand, while
/// inside `graph` it is simply an argument the subcommand does not accept.
#[derive(Clone, Copy)]
enum StrayWord {
    /// The word was read where a subcommand was expected.
    AtRoot,
    /// The word was read where a subcommand argument was expected.
    InSubcommand,
}

impl StrayWord {
    /// Return the wording clap reports for this tier.
    fn diagnostic(self) -> &'static str {
        match self {
            Self::AtRoot => "Unknown subcommand",
            Self::InSubcommand => "Unknown argument",
        }
    }
}

/// `--json false` is a different case: the stray word is accepted first.
///
/// The pre-clap JSON hint scanner (`src/cli/l10n/mod.rs::json_hint_from_args`)
/// only recognises a bare `--json`, and it runs before clap. With `--json`
/// present the scanner selects JSON diagnostics, so the *subcommand* error is
/// rendered as a JSON document on standard error. This is the one space
/// separated case that exits 1 rather than 2, and the reason is routing, not
/// acceptance: the `false` word is still rejected.
#[rstest]
fn json_with_a_space_separated_value_keeps_its_json_diagnostics() -> Result<()> {
    let stderr = rejected_stderr(&["--json", "false", "graph"], 1)?;

    let document: serde_json::Value =
        serde_json::from_str(&stderr).context("--json false should route a JSON document")?;
    let message = document
        .pointer("/diagnostics/0/message")
        .and_then(serde_json::Value::as_str)
        .context("JSON diagnostics should carry a message")?;
    ensure!(
        message.contains("Unknown subcommand") && message.contains("false"),
        "--json false should still reject `false`: {message}"
    );
    Ok(())
}
