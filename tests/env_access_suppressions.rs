//! Contract test: no compiled source suppresses the environment-access policy.
//!
//! `clippy.toml` disallows the process-environment entry points, and the
//! workspace denies `clippy::disallowed_methods`, but the two together do not
//! close the door. An inner attribute — an `allow` of the policy lint, at the
//! top of a file — switches the lint off for everything below it and passes
//! `make lint` with every other contract green: `clippy.toml` still lists the
//! methods, the workspace still denies the lint, and the lint target still runs
//! across the workspace. None of them observes that a source opted out, because
//! each asserts a true statement about something else. A green tick means the
//! gate ran, not that it was allowed to see anything.
//!
//! Clippy cannot close this itself: `clippy::allow_attributes` does not fire on
//! inner attributes, so the seam taxonomy's "an `#[expect]` carrying a reason,
//! never an `allow`" rule has no mechanical enforcement there. The assertion is
//! therefore about source text, which is normally the wrong shape for a
//! contract — but an attribute *is* source text, there is no execution to
//! model, and "this file does not opt out" is exactly a statement about what
//! the file says.
//!
//! [`scanner`] finds the attributes, [`mask`] keeps quoted text out of its way,
//! and [`policy`] decides which names they may carry; [`roots`] decides which
//! sources are read at all, and this crate holds the assertions over both.
//!
//! See `docs/adr-008-environment-seam-taxonomy.md` for the taxonomy, and the
//! developers' guide for the sanctioned forms and the scoped exemption.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};

#[path = "env_access_suppressions/mask.rs"]
mod mask;
#[path = "env_access_suppressions/policy.rs"]
mod policy;
#[path = "env_access_suppressions/roots.rs"]
mod roots;
#[path = "env_access_suppressions/scanner.rs"]
mod scanner;

use roots::{MINIMUM_WORKSPACE_SOURCES, collect_all_sources, compiled_sources, is_scanned};
use scanner::scan_source;

/// Render every finding as one failure message, so one run names them all.
///
/// Collecting before asserting means a contributor sees every offending file in
/// one run rather than fixing them one gate at a time. The message is assembled
/// from a joined list rather than pushed into a growing `String`, because the
/// helper sits outside a `#[test]` body, where neither `expect` nor the
/// `std::fmt::Write` result may be discarded.
fn build_error_message(findings: &[(String, String)]) -> String {
    let listed = findings
        .iter()
        .map(|(path, lint)| format!("{path}: {lint}"))
        .collect::<Vec<_>>()
        .join("\n- ");
    format!(
        "the environment-access policy is suppressed in compiled sources; \
         remove the `allow` and state the site as `#[expect(..., reason = \"...\")]`, \
         or route the access through a seam:\n- {listed}"
    )
}

/// Render the `(path, lint)` pairs a table case expects the scan to report.
///
/// Every finding for one source carries that source's path, so a case states
/// the lints and the path once and the pairs are assembled here. That keeps a
/// row down to the three things that distinguish it — the source, where it
/// sits, and what must be found — rather than repeating the pair shape at every
/// row.
fn expected_findings(path: &str, lints: &[&str]) -> Vec<(String, String)> {
    lints
        .iter()
        .map(|lint| (path.to_owned(), (*lint).to_owned()))
        .collect()
}

/// Fail if any compiled source suppresses the environment-access policy.
#[test]
fn compiled_sources_never_suppress_the_environment_policy() -> Result<()> {
    let sources = compiled_sources()?;
    // A sweep that silently matched nothing would make the assertion vacuous.
    ensure!(
        !sources.is_empty(),
        "the compiled source roots should contain at least one Rust source"
    );
    let findings: Vec<(String, String)> = sources
        .iter()
        .flat_map(|(path, contents)| scan_source(path, contents))
        .collect();

    ensure!(findings.is_empty(), "{}", build_error_message(&findings));
    Ok(())
}

/// Fail if any Rust source in the workspace falls outside the scanned roots.
///
/// The scan above can only be as good as the roots it lists, and a root list
/// is exactly the kind of thing that ages badly: Cargo discovers targets in
/// `src/bin`, `examples`, `tests`, and `benches`, a contributor can add a
/// fourth location, and a root that is renamed or misspelled silently excuses
/// its sources rather than reporting itself. So the roots are not trusted on
/// their own. This walk enumerates every Rust source the workspace holds and
/// fails when one of them is not scanned, which turns the silent failure into a
/// named one and makes the root list safe to extend rather than something a
/// reviewer has to keep re-deriving.
///
/// Caches and `target` are skipped, not because their sources do not matter,
/// but because they are generated or vendored rather than written here, and a
/// gate that read them would depend on what a cache happened to hold. Writes
/// under `target/` are the compiler's, and the `.uv-cache` and friends are
/// tooling state; neither is a place a contributor edits.
#[test]
fn every_rust_source_in_the_workspace_is_scanned() -> Result<()> {
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the workspace root")?;
    let mut present = Vec::new();
    collect_all_sources(&crate_root, ".", &mut present)?;

    // A walk that silently found nothing would pass while inspecting nothing.
    ensure!(
        !present.is_empty(),
        "the workspace walk should find at least one Rust source"
    );
    ensure!(
        present.len() >= MINIMUM_WORKSPACE_SOURCES,
        "the workspace walk found {} Rust sources, fewer than the {} the workspace \
         holds; the walk is probably not descending",
        present.len(),
        MINIMUM_WORKSPACE_SOURCES
    );

    let mut unscanned: Vec<&String> = present.iter().filter(|path| !is_scanned(path)).collect();
    unscanned.sort();
    ensure!(
        unscanned.is_empty(),
        "these Rust sources would not be scanned for policy suppressions; add each \
         source's root to `COMPILED_SOURCE_ROOTS` (or `STANDALONE_COMPILED_SOURCES` \
         if it is a file), so that the suppression contract covers it:\n- {}",
        unscanned
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join("\n- ")
    );
    Ok(())
}

#[path = "env_access_suppressions/scanner_tests.rs"]
mod scanner_tests;

#[path = "env_access_suppressions/spelling_tests.rs"]
mod spelling_tests;

#[path = "env_access_suppressions/walk_tests.rs"]
mod walk_tests;
