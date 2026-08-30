//! Contract tests keeping Kani mutation evidence in lockstep with harnesses.
//!
//! Each `#[kani::proof]` harness demonstrates it can fail via a mutation
//! patch under `docs/verification/mutations/` that seeds one realistic fault
//! into the production code the harness drives. These tests keep that
//! evidence honest outside the Kani runner:
//!
//! - every patch must still apply to the current tree (`git apply --check`),
//!   catching silent rot when production code near a patched hunk moves;
//! - every harness must own a correspondingly named patch, or appear in an
//!   explicit exemption list with a stated reason; and
//! - every patch must correspond to a live harness, catching renames.
//!
//! Patch names use the harness path with `::` replaced by `__`, for example
//! `ir__cycle__verification__self_dependency_reports_cycle.patch`.

use std::{collections::BTreeSet, io::Write as _, process::Command};

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Repository-relative directory holding the mutation evidence patches.
const MUTATIONS_DIR: &str = "docs/verification/mutations";

/// Harnesses deliberately carrying no mutation patch, with the reason why.
///
/// Entries are full harness paths (`module::path::harness_name`). Keep this
/// list empty unless a harness genuinely cannot seed a distinct production
/// fault; record the rationale here and in `docs/developers-guide.md`.
const EXEMPT_HARNESSES: &[(&str, &str)] = &[];

/// Property-test mutation patches that deliberately have no Kani harness.
///
/// They exercise scanner and command-guard contracts that exceed the bounded
/// Kani resource cap. Keep this list narrow: each entry must name a live
/// Proptest property, and the patch still has to apply cleanly below.
const SUPPLEMENTAL_PROPERTY_PATCHES: &[&str] = &[
    "ir__cmd_interpolate__property_tests__guard_uses_the_substituted_command",
    "ir__cmd_interpolate__property_tests__scanner_agrees_with_independent_specification",
    "ir__cmd_interpolate__property_tests__substituted_odd_backticks_are_rejected",
];

/// The workspace root, taken from the crate manifest directory.
fn manifest_dir() -> &'static Utf8Path {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Collect repository-relative paths of every `.rs` file under `prefix`.
fn collect_rust_sources(
    root: &Dir,
    prefix: &Utf8Path,
    sources: &mut Vec<Utf8PathBuf>,
) -> Result<()> {
    for entry_result in root
        .read_dir(prefix.as_str())
        .with_context(|| format!("read directory {prefix}"))?
    {
        let entry = entry_result.with_context(|| format!("read directory entry in {prefix}"))?;
        let name = entry.file_name().context("read directory entry name")?;
        let path = prefix.join(&name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type of {path}"))?;
        if file_type.is_dir() {
            collect_rust_sources(root, &path, sources)?;
        } else if path.extension() == Some("rs") {
            sources.push(path);
        }
    }
    Ok(())
}

/// Extract the function names declared directly below `#[kani::proof]`.
fn harness_names_in(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut names = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "#[kani::proof]" {
            continue;
        }
        // Skip trailing attributes such as `#[kani::solver(...)]` and
        // `#[kani::unwind(...)]` between the proof marker and the function.
        let candidates = lines.get(index + 1..).unwrap_or_default();
        if let Some(declaration) = candidates
            .iter()
            .find(|candidate| !candidate.trim_start().starts_with("#["))
            && let Some(name) = declared_function_name(declaration)
        {
            names.push(name);
        }
    }
    names
}

/// Extract the function name from a `fn` declaration line, if present.
fn declared_function_name(declaration: &str) -> Option<String> {
    let rest = declaration.trim_start().strip_prefix("fn ")?;
    let (name, _) = rest.split_once('(')?;
    Some(name.trim().to_owned())
}

/// Derive the harness module path for a verification source file.
///
/// The repository convention wires harness bodies as `mod verification`
/// declared by the sibling module they verify, so
/// `src/ir/cycle_verification.rs` maps to `ir::cycle::verification`, while
/// `src/ir/cmd_interpolate/verification.rs` maps to
/// `ir::cmd_interpolate::verification`.
fn module_path_for_source(relative: &Utf8Path) -> Result<String> {
    let stem = relative
        .file_stem()
        .with_context(|| format!("source path {relative} should have a file stem"))?;
    let mut segments: Vec<&str> = relative
        .parent()
        .map(|parent| parent.components().map(|c| c.as_str()).collect())
        .unwrap_or_default();
    if let Some(parent_module) = stem.strip_suffix("_verification") {
        segments.push(parent_module);
        segments.push("verification");
    } else if stem == "verification" {
        segments.push("verification");
    } else {
        bail!(
            "{relative} declares a Kani harness outside a verification module; \
             use `*_verification.rs` or `verification.rs` below its module",
        );
    }
    Ok(segments.join("::"))
}

/// Discover every `#[kani::proof]` harness under `src/` as a full path.
fn discover_harnesses() -> Result<BTreeSet<String>> {
    let root = Dir::open_ambient_dir(manifest_dir().join("src"), ambient_authority())
        .context("open src")?;
    let mut sources = Vec::new();
    collect_rust_sources(&root, Utf8Path::new("."), &mut sources)?;

    let mut harnesses = BTreeSet::new();
    for source_path in sources {
        let source = root
            .read_to_string(source_path.as_str())
            .with_context(|| format!("read {source_path}"))?;
        let names = harness_names_in(&source);
        if names.is_empty() {
            continue;
        }
        let normalized = source_path
            .strip_prefix(".")
            .unwrap_or(source_path.as_path());
        let module_path = module_path_for_source(normalized)?;
        for name in names {
            harnesses.insert(format!("{module_path}::{name}"));
        }
    }
    ensure!(
        !harnesses.is_empty(),
        "no #[kani::proof] harnesses found under src/; the discovery logic \
         has gone stale",
    );
    Ok(harnesses)
}

/// List the mutation patch file names, without their `.patch` extension.
fn patch_stems() -> Result<BTreeSet<String>> {
    let mutations = Dir::open_ambient_dir(manifest_dir().join(MUTATIONS_DIR), ambient_authority())
        .with_context(|| format!("open {MUTATIONS_DIR}"))?;
    let mut stems = BTreeSet::new();
    for entry_result in mutations
        .read_dir(".")
        .context("read mutations directory")?
    {
        let entry = entry_result.context("read mutations directory entry")?;
        let name = entry.file_name().context("read mutation patch name")?;
        let path = Utf8Path::new(&name);
        ensure!(
            path.extension() == Some("patch"),
            "{MUTATIONS_DIR}/{name} is not a .patch file; keep the directory \
             restricted to mutation evidence",
        );
        stems.insert(
            path.file_stem()
                .with_context(|| format!("patch {name} should have a file stem"))?
                .to_owned(),
        );
    }
    Ok(stems)
}

/// Convert a full harness path into its expected patch file stem.
fn patch_stem_for_harness(harness: &str) -> String {
    harness.replace("::", "__")
}

/// Return whether the source tree is a Git work tree.
///
/// `cargo-mutants` tests each mutant in a copied source tree without
/// `.git`, for which Git returns 128. Patch applicability must not run
/// there: a mutant overlapping a patch hunk would fail `git apply --check`
/// and be reported as killed without any behavioural or Kani assertion
/// detecting the fault, inflating mutation coverage.
fn is_git_work_tree(root: &Utf8Path) -> Result<bool> {
    let probe = Command::new("git")
        .args(["-C", root.as_str(), "rev-parse", "--is-inside-work-tree"])
        .output()
        .context("probe whether the source tree is a Git work tree")?;
    if !probe.status.success() {
        if probe.status.code() == Some(128) {
            return Ok(false);
        }
        bail!(
            "git rev-parse --is-inside-work-tree failed ({})",
            probe.status
        );
    }
    match String::from_utf8(probe.stdout)
        .context("decode git work-tree probe output")?
        .trim()
    {
        "true" => Ok(true),
        "false" => Ok(false),
        output => bail!("unexpected git work-tree probe output: {output:?}"),
    }
}

/// Every harness carries a mutation patch or an explicit, live exemption.
#[test]
fn every_harness_has_mutation_evidence_or_exemption() -> Result<()> {
    let harnesses = discover_harnesses()?;
    let stems = patch_stems()?;

    for (exempt, reason) in EXEMPT_HARNESSES {
        ensure!(
            harnesses.contains(*exempt),
            "exemption for {exempt} ({reason}) names no live harness; remove \
             the stale entry",
        );
        ensure!(
            !stems.contains(&patch_stem_for_harness(exempt)),
            "exemption for {exempt} ({reason}) is stale: a mutation patch \
             exists, so drop the exemption",
        );
    }

    let exempt: BTreeSet<&str> = EXEMPT_HARNESSES.iter().map(|(name, _)| *name).collect();
    let missing: Vec<&String> = harnesses
        .iter()
        .filter(|harness| !exempt.contains(harness.as_str()))
        .filter(|harness| !stems.contains(&patch_stem_for_harness(harness)))
        .collect();
    ensure!(
        missing.is_empty(),
        "harnesses without mutation evidence under {MUTATIONS_DIR}: \
         {missing:?}; add a `<harness path with __>.patch` seeding one \
         realistic fault the harness rejects, or exempt the harness with a \
         reason",
    );
    Ok(())
}

/// Every mutation patch corresponds to a live harness or supplemental property.
#[test]
fn every_patch_matches_a_harness() -> Result<()> {
    let harnesses = discover_harnesses()?;
    let expected: BTreeSet<String> = harnesses
        .iter()
        .map(|harness| patch_stem_for_harness(harness.as_str()))
        .chain(
            SUPPLEMENTAL_PROPERTY_PATCHES
                .iter()
                .map(|patch| (*patch).to_owned()),
        )
        .collect();
    let orphans: Vec<String> = patch_stems()?
        .into_iter()
        .filter(|stem| !expected.contains(stem))
        .collect();
    ensure!(
        orphans.is_empty(),
        "mutation patches without a matching harness: {orphans:?}; rename \
         them alongside the harness or delete them if the harness is gone",
    );
    Ok(())
}

/// Every mutation patch still applies cleanly to the current tree.
#[test]
fn every_patch_applies_cleanly() -> Result<()> {
    if !is_git_work_tree(manifest_dir())? {
        // cargo-mutants copies omit `.git`; applicability there would
        // mis-report mutants overlapping a patch hunk as killed.
        writeln!(
            std::io::stderr().lock(),
            concat!(
                "skipping: source tree is not a git checkout; ",
                "operation=check mutation patch applicability; repository={}"
            ),
            manifest_dir()
        )?;
        return Ok(());
    }
    let mutations = Dir::open_ambient_dir(manifest_dir().join(MUTATIONS_DIR), ambient_authority())
        .with_context(|| format!("open {MUTATIONS_DIR}"))?;
    let mut rotted = Vec::new();
    for entry_result in mutations
        .read_dir(".")
        .context("read mutations directory")?
    {
        let entry = entry_result.context("read mutations directory entry")?;
        let name = entry.file_name().context("read mutation patch name")?;
        let patch = Utf8Path::new(MUTATIONS_DIR).join(&name);
        let output = Command::new("git")
            .args(["apply", "--check", patch.as_str()])
            .current_dir(manifest_dir())
            .output()
            .with_context(|| format!("run git apply --check {patch}"))?;
        if !output.status.success() {
            rotted.push(format!(
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
    }
    ensure!(
        rotted.is_empty(),
        "mutation patches no longer apply to the current tree, so their \
         evidence has rotted: {rotted:#?}; regenerate each patch against the \
         moved production code and re-validate it by running its harness \
         under the mutation",
    );
    Ok(())
}
