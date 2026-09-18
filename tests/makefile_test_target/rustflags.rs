//! Contract model for the Makefile variables that assign `RUSTFLAGS`.
//!
//! No recipe spells the value out any more: each composes one of three Make
//! variables, so the variables are what this module contracts. That indirection
//! is exactly the hazard a text-walking contract can be blind to, so the
//! completeness test has two halves — every `RUSTFLAGS="` in the file must be
//! one of the contracted variables, and every recipe that sets `RUSTFLAGS` must
//! do so through one of them. Neither half alone would notice a recipe that
//! quietly went back to composing its own.
//!
//! Make expands the variables, because one of them uses a Make function this
//! module has no business reimplementing; the resulting shell expression is
//! then expanded in a real shell, without running the recipe's command, so the
//! assertions are about what Cargo would actually receive. The expected flags
//! are read from `.cargo/config.toml` rather than restated here, which is what
//! keeps the Makefile and the configuration from drifting apart.
//!
//! The parent `makefile_test_target` module supplies the repository-file and
//! recipe-lookup helpers.

use super::read_repo_file;
// The recipe lookup and the shell expansion it feeds are Unix-only, so the
// import is too: an unconditional one is an unused-import error on Windows.
#[cfg(unix)]
use super::target_recipe;
use anyhow::{Result, ensure};
// Every `context` call sits in a Unix-only helper, so the trait import is
// gated with them.
#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use assert_cmd::Command;
use camino::Utf8Path;
use std::collections::BTreeSet;
#[cfg(unix)]
use test_support::fs;

/// The prefix introducing a quoted `RUSTFLAGS` assignment.
const RUSTFLAGS_PREFIX: &str = "RUSTFLAGS=\"";

/// A value a caller might already have exported before invoking `make`.
#[cfg(unix)]
const CALLER_RUSTFLAGS: &str = "-C target-cpu=native";

#[cfg(unix)]
const DENY_WARNINGS: &str = "-D warnings";

/// A Make variable holding a `RUSTFLAGS` assignment, and the policy it carries.
#[derive(Clone, Copy, Debug)]
struct RustflagsVariable {
    /// The Make variable's name.
    name: &'static str,
    /// Whether the composed value denies warnings. The gate targets do; a
    /// plain build must not, or `make build` silently becomes a gate.
    ///
    /// Read only by the shell-expansion test, which needs a real shell and so
    /// runs on Unix alone. The field stays on every platform because the table
    /// below is one list, not one per platform.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by the Unix-only expansion test")
    )]
    denies_warnings: bool,
    /// Whether the composed value carries the build standard's flags. The
    /// release exclusion turns on this being false.
    ///
    /// Read only by the shell-expansion test, as above.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by the Unix-only expansion test")
    )]
    carries_standard: bool,
}

/// Every Make variable that assigns `RUSTFLAGS`.
const RUSTFLAGS_VARIABLES: [RustflagsVariable; 3] = [
    RustflagsVariable {
        name: "GATE_RUSTFLAGS",
        denies_warnings: true,
        carries_standard: true,
    },
    RustflagsVariable {
        name: "DEBUG_RUSTFLAGS",
        denies_warnings: false,
        carries_standard: true,
    },
    RustflagsVariable {
        name: "RELEASE_RUSTFLAGS",
        denies_warnings: false,
        carries_standard: false,
    },
];

/// Extracts the double-quoted `RUSTFLAGS` assignment from a line.
///
/// `RUSTDOCFLAGS="…"` does not contain `RUSTFLAGS="`, so a line setting both
/// still yields the `RUSTFLAGS` value.
fn rustflags_assignment(line: &str) -> Option<&str> {
    let start = line.find(RUSTFLAGS_PREFIX)? + RUSTFLAGS_PREFIX.len();
    let rest = line.get(start..)?;
    let end = rest.rfind('"')?;
    rest.get(..end)
}

/// Asks Make to expand `name`, returning the assignment it yields.
///
/// Make is the authority rather than a substitution written here: one of these
/// variables selects the linker flag with a Make conditional, and a
/// reimplementation of that would be a second thing to keep in step with the
/// Makefile — which is the very failure this module exists to catch.
#[cfg(unix)]
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
#[cfg(unix)]
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

/// The flags the build standard applies on this platform.
///
/// Read from the committed configuration, not restated: a flag the Makefile
/// composes must be one the configuration also names, or a bare `cargo build`
/// would not get it.
///
/// Only the shell-expansion tests read this, and those are Unix-only.
#[cfg(unix)]
fn standard_flags() -> Result<Vec<String>> {
    let config: toml::Value =
        toml::from_str(&read_repo_file(Utf8Path::new(".cargo/config.toml"))?)?;
    // The Makefile appends the linker flag on Linux only, matching the `cfg`
    // gate in the configuration, so the two must be compared on the same terms.
    let table = if cfg!(target_os = "linux") {
        config
            .get("target")
            .and_then(|value| value.get(r#"cfg(target_os = "linux")"#))
    } else {
        config.get("build")
    };
    let flags = table
        .and_then(|value| value.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("the configuration should carry a rustflags list for this platform")?;
    Ok(flags
        .iter()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// Expands `expression` in a shell, exporting `inherited` as `RUSTFLAGS`.
///
/// Only the assignment is expanded; the command a recipe would run is never
/// executed, so no test here invokes Cargo, Kani, nextest, or Dylint.
#[cfg(unix)]
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
fn unit_extracts_the_rustflags_assignment_from_a_line() {
    assert_eq!(
        rustflags_assignment(r#"RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings""#),
        Some(r"${RUSTFLAGS:+$RUSTFLAGS }-D warnings")
    );
    // A line setting RUSTDOCFLAGS first still yields the RUSTFLAGS value.
    assert_eq!(
        rustflags_assignment(r#"RUSTDOCFLAGS="-D warnings" RUSTFLAGS="${RUSTFLAGS-} -Z""#),
        Some(r"${RUSTFLAGS-} -Z")
    );
    assert_eq!(rustflags_assignment("cargo build"), None);
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

#[test]
fn behavioural_every_rustflags_assignment_is_a_contracted_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let declared: BTreeSet<String> = makefile
        .lines()
        .filter(|line| line.contains(RUSTFLAGS_PREFIX))
        .map(|line| line.trim().to_owned())
        .collect();
    let names: Vec<&str> = RUSTFLAGS_VARIABLES
        .iter()
        .map(|variable| variable.name)
        .collect();

    for line in &declared {
        ensure!(
            names.iter().any(|name| line.starts_with(name)),
            "every RUSTFLAGS assignment must be one of {names:?}; found {line:?}"
        );
    }
    ensure!(
        declared.len() == RUSTFLAGS_VARIABLES.len(),
        "expected one assignment per contracted variable, found {declared:#?}"
    );
    Ok(())
}

#[test]
fn behavioural_every_recipe_sets_rustflags_through_a_contracted_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    // The two halves catch different edits. The assignment test above would
    // still pass if a recipe stopped referencing any variable and simply
    // dropped the flags; this one would still pass if a variable's definition
    // were rewritten. Only together do they hold the composition.
    let offenders: Vec<&str> = makefile
        .lines()
        .filter(|line| line.starts_with('\t') && line.contains("RUSTFLAGS"))
        .filter(|line| {
            !RUSTFLAGS_VARIABLES
                .iter()
                .any(|variable| line.contains(&format!("$({})", variable.name)))
        })
        .collect();
    ensure!(
        offenders.is_empty(),
        "every recipe setting RUSTFLAGS must compose a contracted variable; found {offenders:#?}"
    );

    for variable in RUSTFLAGS_VARIABLES {
        let reference = format!("$({})", variable.name);
        ensure!(
            makefile
                .lines()
                .any(|line| line.starts_with('\t') && line.contains(&reference)),
            "{} is defined but no recipe composes it",
            variable.name
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn behavioural_the_gate_variable_reaches_every_gate_recipe() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for target in [
        "test-nextest",
        "doctest",
        "lint-clippy",
        "lint-whitaker",
        "typecheck",
    ] {
        let recipe = target_recipe(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        ensure!(
            recipe.contains("$(GATE_RUSTFLAGS)"),
            "{target} should compose GATE_RUSTFLAGS, found {recipe:?}"
        );
    }
    Ok(())
}
