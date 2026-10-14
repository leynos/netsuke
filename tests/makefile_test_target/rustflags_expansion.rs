//! What each contracted `RUSTFLAGS` variable expands to.
//!
//! Split from `rustflags.rs` to keep that module within the Whitaker
//! `module_max_lines` cap, and declared `#[cfg(unix)]` there because everything
//! here needs a real shell. The parent owns the model — the table of contracted
//! variables and the reading that pulls an assignment out of a line — and this
//! submodule owns the expansion of those variables.
//!
//! Make expands the variables, because one of them uses a Make function the
//! parent has no business reimplementing; the resulting shell expression is then
//! expanded in a real shell, without running the recipe's command, so the
//! assertions are about what Cargo would actually receive. The expected flags
//! are read from `.cargo/config.toml` rather than restated here, which is what
//! keeps the Makefile and the configuration from drifting apart.

use super::{RUSTFLAGS_VARIABLES, rustflags_assignment};
use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use test_support::build_tools::standard_flags;
use test_support::fs;

/// A value a caller might already have exported before invoking `make`.
const CALLER_RUSTFLAGS: &str = "-C target-cpu=native";

const DENY_WARNINGS: &str = "-D warnings";

/// Asks Make to expand `name`, returning the assignment it yields.
///
/// Make is the authority rather than a substitution written here: one of these
/// variables selects the linker flag with a Make conditional, and a
/// reimplementation of that would be a second thing to keep in step with the
/// Makefile — which is the very failure this module exists to catch.
fn expanded_variable(name: &str) -> Result<String> {
    let printer = tempfile::Builder::new()
        .suffix(".mk")
        .tempfile()
        .context("create the Make printer fragment")?;
    fs::write(
        printer.path(),
        // Single-quoted: the value itself contains double quotes, so wrapping
        // it in them would hand the shell a broken word rather than the
        // assignment. Nothing here needs the shell to expand anything.
        "print-variable:\n\t@printf '%s' '$($(VARIABLE))'\n",
    )
    .context("write the Make printer fragment")?;

    let output = Command::new("make")
        .args(["--no-print-directory", "-f", "Makefile", "-f"])
        .arg(printer.path())
        .arg(format!("VARIABLE={name}"))
        .arg("print-variable")
        .output()
        .with_context(|| format!("ask make to expand {name}"))?;
    ensure!(
        output.status.success(),
        "make should expand {name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expanded = String::from_utf8(output.stdout).context("expanded value should be UTF-8")?;
    ensure!(
        !expanded.trim().is_empty(),
        "{name} should expand to an assignment, got an empty value"
    );
    Ok(expanded)
}

/// Returns `name`'s assignment as a shell expression Make has already expanded.
fn shell_expression(name: &str) -> Result<String> {
    let expanded = expanded_variable(name)?;
    let assignment = rustflags_assignment(&expanded)
        .with_context(|| format!("{name} should assign a double-quoted RUSTFLAGS value"))?;
    ensure!(
        !assignment.contains("$("),
        "{name}: assignment {assignment:?} still names a Make variable after expansion"
    );
    Ok(assignment.to_owned())
}

/// Expands `expression` in a shell, exporting `inherited` as `RUSTFLAGS`.
///
/// Only the assignment is expanded; the command a recipe would run is never
/// executed, so no test here invokes Cargo, Kani, nextest, or Dylint.
fn expand(expression: &str, inherited: Option<&str>) -> Result<String> {
    ensure!(
        !expression.contains('"') && !expression.contains('`'),
        "the expansion helper cannot safely embed {expression:?}"
    );
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!("printf '%s' \"{expression}\""))
        .env_remove("RUSTFLAGS");
    if let Some(value) = inherited {
        command.env("RUSTFLAGS", value);
    }

    let output = command
        .output()
        .with_context(|| format!("expand {expression:?} with sh"))?;
    ensure!(
        output.status.success(),
        "sh should expand {expression:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("expanded RUSTFLAGS should be UTF-8")
}

#[test]
fn behavioural_every_variable_preserves_inherited_flags() -> Result<()> {
    for variable in RUSTFLAGS_VARIABLES {
        let expanded = expand(&shell_expression(variable.name)?, Some(CALLER_RUSTFLAGS))?;
        ensure!(
            expanded.contains(CALLER_RUSTFLAGS),
            "{} should preserve an inherited value, expanded to {expanded:?}",
            variable.name
        );
    }
    Ok(())
}

#[test]
fn behavioural_every_variable_is_well_formed_without_inherited_flags() -> Result<()> {
    for variable in RUSTFLAGS_VARIABLES {
        let expanded = expand(&shell_expression(variable.name)?, None)?;
        ensure!(
            !expanded.contains(CALLER_RUSTFLAGS),
            "{} should not invent flags the caller never set, expanded to {expanded:?}",
            variable.name
        );
        // `${VAR:+VAR }` contributes its separator only alongside a value, so
        // an unset RUSTFLAGS must not leave a leading space.
        ensure!(
            !expanded.starts_with(' '),
            "{} should not emit a leading separator when RUSTFLAGS is unset, \
             expanded to {expanded:?}",
            variable.name
        );
    }
    Ok(())
}

#[test]
fn behavioural_each_variable_carries_its_own_policy() -> Result<()> {
    let standard = standard_flags()?;
    ensure!(
        !standard.is_empty(),
        "the configuration should name at least one standard flag"
    );

    for variable in RUSTFLAGS_VARIABLES {
        let expanded = expand(&shell_expression(variable.name)?, None)?;
        ensure!(
            expanded.contains(DENY_WARNINGS) == variable.denies_warnings,
            "{} warning policy ({}) not met, expanded to {expanded:?}",
            variable.name,
            variable.denies_warnings
        );
        for flag in &standard {
            ensure!(
                expanded.contains(flag.as_str()) == variable.carries_standard,
                "{} should {} `{flag}`, expanded to {expanded:?}",
                variable.name,
                if variable.carries_standard {
                    "carry"
                } else {
                    "exclude"
                }
            );
        }
    }
    Ok(())
}

/// The contracted variables' shell expressions, expanded by Make once.
///
/// Each of these costs a full `make` run — the fragment is loaded on top of
/// the real Makefile — and none of them depends on the generated value, so a
/// property test that asked per case would spend its whole budget re-parsing
/// the Makefile. Cached rather than hoisted into a `const`, because Make is
/// the authority on what they expand to and only a run can say.
fn contracted_expressions() -> &'static [(&'static str, String)] {
    static EXPRESSIONS: std::sync::OnceLock<Vec<(&'static str, String)>> =
        std::sync::OnceLock::new();
    // A `OnceLock` initializer cannot return `Result`, and a Makefile that will
    // not expand is not a bad property input but a broken repository, which the
    // suite must abort on rather than skip past. Same shape as the `#[once]`
    // fixture in tests/ninja_semantics_ui_tests.rs, and the message names the
    // variable so the abort says which contract broke.
    EXPRESSIONS.get_or_init(|| {
        RUSTFLAGS_VARIABLES
            .iter()
            .map(|variable| {
                #[expect(
                    clippy::expect_used,
                    reason = "a OnceLock initializer cannot return Result; a broken Makefile must abort the suite"
                )]
                let expanded = shell_expression(variable.name)
                    .expect("every contracted variable should expand in Make");
                (variable.name, expanded)
            })
            .collect()
    })
}

/// Generate a `RUSTFLAGS` value a caller might have exported.
///
/// The cases that matter are the ones a shell treats specially, because the
/// composition hands the caller's value to `${RUSTFLAGS:+$RUSTFLAGS }` inside
/// a double-quoted assignment. An unquoted expansion would word-split on the
/// spaces and tabs and run a command substitution on the backticks; a value
/// carrying a quote would break the assignment outright. `any::<String>()` is
/// the wrong generator here: random bytes are overwhelmingly one unbroken
/// word, which is the case that needs no handling at all.
fn caller_rustflags_strategy() -> impl Strategy<Value = String> {
    let pieces = prop_oneof![
        // Ordinary flags, the common case.
        "[A-Za-z0-9_./+=-]{0,12}",
        // Whitespace, which a shell splits on if the expansion is unquoted.
        Just(" ".to_owned()),
        Just("\t".to_owned()),
        // Shell metacharacters and separators.
        Just(";".to_owned()),
        Just("&&".to_owned()),
        Just("|".to_owned()),
        Just("$".to_owned()),
        Just("`".to_owned()),
        Just("\\".to_owned()),
        Just("\"".to_owned()),
        Just("'".to_owned()),
        Just("*".to_owned()),
        Just("~".to_owned()),
        Just("{a,b}".to_owned()),
        Just("$(id)".to_owned()),
        Just("$RUSTFLAGS".to_owned()),
        Just("$(".to_owned()),
    ];
    prop::collection::vec(pieces, 0..5).prop_map(|parts| parts.join(""))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        // Name the file explicitly. The default `SourceParallel` policy looks
        // for a `lib.rs` or `main.rs` beside the source, and this module is
        // pulled in with `#[path]` from `tests/makefile_test_target.rs`, so the
        // walk up from `tests/makefile_test_target/` finds neither. proptest
        // says so on stderr and falls back to `WithSource`, which resolves
        // against the crate root instead — so a recorded seed was written to a
        // path nobody reads and replayed against nothing.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/makefile_test_target/rustflags_expansion.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// A hostile caller value is preserved exactly, and cannot alter the flags.
    ///
    /// The relation is between what the caller exported and what Cargo would
    /// receive, and it has two halves that fail to different bugs. The caller's
    /// value must survive byte-for-byte — an unquoted expansion would
    /// word-split on the spaces, run `$(…)` and backticks as commands, and
    /// truncate at a quote or separator. And the contribution each variable
    /// then appends must be its own policy exactly, unaffected by whatever the
    /// caller put in front of it: a value ending in a space or a quote must not
    /// be able to swallow or forge one of the standard's flags.
    ///
    /// A generated value reaches the shell through the environment rather than
    /// through the command string, so the quotes and backticks it may contain
    /// are data, not syntax — which is how Cargo receives a real `RUSTFLAGS`
    /// too, and is why this can generate them at all.
    #[test]
    fn property_every_variable_preserves_the_callers_value_and_its_own_flags(
        caller in caller_rustflags_strategy(),
    ) {
        let standard = standard_flags().expect("read the standard's flags");
        for (name, expression) in contracted_expressions() {
            let variable = RUSTFLAGS_VARIABLES
                .iter()
                .find(|variable| variable.name == *name)
                .expect("every cached expression names a contracted variable");
            let expanded = expand(expression, Some(&caller))
                .expect("expand the variable in a shell");

            prop_assert!(
                expanded.starts_with(&caller),
                "{} mangled the caller's `{caller}` into `{expanded}`",
                variable.name
            );

            let mut contributions = Vec::new();
            if variable.denies_warnings {
                contributions.push(DENY_WARNINGS);
            }
            if variable.carries_standard {
                contributions.extend(standard.iter().map(String::as_str));
            }
            let appended = contributions.join(" ");
            // Exact, not trimmed. The separator exists to separate the
            // caller's flags from ours, so it appears only when there is
            // something on both sides of it: `${VAR:+$VAR }` supplies one
            // alongside a value, and `${VAR-}` — which is how the release
            // exclusion passes the caller through without taking anything —
            // never supplies one. Modelling this instead of appending a space
            // whenever the caller set a value is what makes the release
            // exclusion's emptiness a thing this test checks rather than a
            // thing it assumes. Losing the space where a contribution *does*
            // follow is not cosmetic: it concatenates the caller's last flag
            // with the first of ours, which is a different flag and still
            // builds.
            let expected = match (caller.is_empty(), appended.is_empty()) {
                (true, _) => appended,
                (false, true) => caller.clone(),
                (false, false) => format!("{caller} {appended}"),
            };
            prop_assert!(
                expanded == expected,
                "{} should expand to `{expected}` given `{caller}`, got `{expanded}`",
                variable.name
            );
        }
    }
}
