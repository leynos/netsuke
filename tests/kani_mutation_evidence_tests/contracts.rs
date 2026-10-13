//! The three lockstep contracts between harnesses and mutation evidence.
//!
//! Each `#[kani::proof]` harness demonstrates it can fail via a patch that
//! seeds one realistic fault into the production code the harness drives.
//! These checks keep that evidence honest outside the Kani runner: every patch
//! must still apply, every harness must own a patch or a stated exemption, and
//! every patch must name a live harness or a live Proptest property.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::discovery::{
    MUTATIONS_DIR, declared_function_name, discover_harnesses, is_git_work_tree, manifest_dir,
    patch_stem_for_harness, patch_stems,
};

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

/// Resolve the source file that declares the property named by a patch stem.
///
/// A stem encodes the property's module path with `__` standing in for `::`,
/// so `ir__cmd_interpolate__property_tests__scanner_…` names the property
/// `scanner_…` in the module `ir::cmd_interpolate::property_tests`. The module
/// path is resolved against the tree rather than by assuming a layout: its
/// leading segments name directory modules, and the final segment is either a
/// directory module's `mod.rs` or a single-file module, both of which this
/// repository uses. The root segment is taken from the stem rather than
/// checked against a list, so a new root needs no change here.
///
/// # Errors
///
/// Returns an error when the stem carries no module path, or when neither
/// candidate declares the named property. The message names every candidate
/// searched, so a module that moves is diagnosable from the failure alone.
fn supplemental_property_location(root: &Dir, patch_stem: &str) -> Result<Utf8PathBuf> {
    let mut segments: Vec<&str> = patch_stem.split("__").collect();
    let property_name = segments
        .pop()
        .with_context(|| format!("supplemental patch {patch_stem} has no property name"))?;
    let Some((leaf, parents)) = segments.split_last() else {
        bail!("supplemental patch {patch_stem} has no module path");
    };

    let mut parent_path = Utf8PathBuf::from("src");
    for segment in parents {
        parent_path.push(segment);
    }
    let candidates = [
        parent_path.join(leaf).join("mod.rs"),
        parent_path.join(format!("{leaf}.rs")),
    ];

    for candidate in &candidates {
        if !root.exists(candidate.as_str()) {
            continue;
        }
        let source = root
            .read_to_string(candidate.as_str())
            .with_context(|| format!("read supplemental property source {candidate}"))?;
        if source
            .lines()
            .filter_map(declared_function_name)
            .any(|name| name == property_name)
        {
            return Ok(candidate.clone());
        }
    }
    bail!(
        "supplemental patch {patch_stem} names property {property_name}, which \
         none of the candidate sources declare; searched: {candidates:?}",
    )
}

/// Validate and return supplemental patch stems backed by live properties.
fn supplemental_property_patches() -> Result<BTreeSet<String>> {
    let root = Dir::open_ambient_dir(manifest_dir(), ambient_authority())
        .context("open workspace root")?;
    let available_patch_stems = patch_stems()?;
    let mut patches = BTreeSet::new();
    for patch_stem in SUPPLEMENTAL_PROPERTY_PATCHES {
        supplemental_property_location(&root, patch_stem)?;
        ensure!(
            available_patch_stems.contains(*patch_stem),
            "supplemental patch {patch_stem} is missing {MUTATIONS_DIR}/{patch_stem}.patch",
        );
        patches.insert((*patch_stem).to_owned());
    }
    Ok(patches)
}

/// A supplemental patch rooted anywhere resolves, and a missing one is named.
///
/// The resolver is exercised against a synthetic tree rather than the live
/// repository so that the two module shapes it must accept — a directory
/// module and a single-file module — are both pinned here, and so that the
/// failure path is pinned without depending on a property that happens to be
/// absent from this revision.
#[test]
fn supplemental_property_location_resolves_directory_and_file_modules() -> Result<()> {
    let staged = tempfile::tempdir().context("create synthetic source tree")?;
    let staged_path = Utf8PathBuf::from_path_buf(staged.path().to_path_buf()).map_err(|invalid| {
        anyhow::anyhow!("synthetic source tree {} is not UTF-8", invalid.display())
    })?;
    let root = Dir::open_ambient_dir(&staged_path, ambient_authority())
        .context("open synthetic source tree")?;
    root.create_dir_all("src/ninja_gen/property_tests")
        .context("create directory module")?;
    root.create_dir_all("src/hex")
        .context("create single-file module parent")?;
    root.write(
        "src/ninja_gen/property_tests/mod.rs",
        "fn emission_is_deterministic() {}\n",
    )
    .context("write directory module")?;
    root.write(
        "src/hex/property_tests.rs",
        "fn encoding_round_trips() {}\n",
    )
    .context("write single-file module")?;

    let resolved =
        supplemental_property_location(&root, "ninja_gen__property_tests__emission_is_deterministic")?;
    ensure!(
        resolved == Utf8Path::new("src/ninja_gen/property_tests/mod.rs"),
        "a ninja_gen-rooted patch must resolve to its directory module, got {resolved}",
    );

    let resolved = supplemental_property_location(&root, "hex__property_tests__encoding_round_trips")?;
    ensure!(
        resolved == Utf8Path::new("src/hex/property_tests.rs"),
        "a single-file property module must resolve to its own source, got {resolved}",
    );

    let missing = supplemental_property_location(&root, "ninja_gen__property_tests__absent")
        .expect_err("a property no candidate declares must be rejected");
    let message = missing.to_string();
    ensure!(
        message.contains("src/ninja_gen/property_tests/mod.rs")
            && message.contains("src/ninja_gen/property_tests.rs"),
        "the failure must name every candidate searched, got: {message}",
    );
    Ok(())
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
    let supplemental_patches = supplemental_property_patches()?;
    let expected: BTreeSet<String> = harnesses
        .iter()
        .map(|harness| patch_stem_for_harness(harness.as_str()))
        .chain(supplemental_patches)
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
