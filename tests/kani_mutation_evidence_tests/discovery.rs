//! Discovery of Kani harnesses and mutation-evidence patch files.
//!
//! Both sides of the contract this test target enforces are discovered from
//! the tree rather than declared: the harnesses by scanning `src/` for
//! `#[kani::proof]`, and the patches by listing the mutations directory. The
//! two resulting name sets are what the sibling contract modules compare, so
//! the naming rules live here beside the walk that produces them.

use std::{collections::BTreeSet, process::Command};

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Repository-relative directory holding the mutation evidence patches.
pub(crate) const MUTATIONS_DIR: &str = "docs/verification/mutations";

/// The workspace root, taken from the crate manifest directory.
pub(crate) fn manifest_dir() -> &'static Utf8Path {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Collect repository-relative paths of every `.rs` file under `prefix`.
pub(crate) fn collect_rust_sources(
    root: &Dir,
    prefix: &Utf8Path,
    sources: &mut Vec<Utf8PathBuf>,
) -> Result<()> {
    for entry_result in root
        .read_dir(prefix.as_str())
        .with_context(|| format!("read directory {prefix}"))?
    {
        let entry = entry_result.with_context(|| format!("read entry under {prefix}"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read entry name under {prefix}"))?;
        let child = if prefix.as_str() == "." {
            Utf8PathBuf::from(&name)
        } else {
            prefix.join(&name)
        };
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type of {child}"))?;
        if file_type.is_dir() {
            collect_rust_sources(root, &child, sources)?;
        } else if child.extension() == Some("rs") {
            sources.push(child);
        }
    }
    Ok(())
}

/// Extract the function names declared directly below `#[kani::proof]`.
pub(crate) fn harness_names_in(source: &str) -> Vec<String> {
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
pub(crate) fn declared_function_name(declaration: &str) -> Option<String> {
    let rest = declaration.trim_start().strip_prefix("fn ")?;
    let (name, _) = rest.split_once('(')?;
    Some(name.trim().to_owned())
}

/// Derive the harness module path for a verification source file.
///
/// The repository convention wires harness bodies as `mod verification`
/// declared by the sibling module they verify, so
/// `src/ir/cycle/verification.rs` maps to `ir::cycle::verification`, while
/// `src/ir/cmd_interpolate/verification.rs` maps to
/// `ir::cmd_interpolate::verification`.
pub(crate) fn module_path_for_source(relative: &Utf8Path) -> Result<String> {
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
pub(crate) fn discover_harnesses() -> Result<BTreeSet<String>> {
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
pub(crate) fn patch_stems() -> Result<BTreeSet<String>> {
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
pub(crate) fn patch_stem_for_harness(harness: &str) -> String {
    harness.replace("::", "__")
}

/// Return whether the source tree is a Git work tree.
///
/// `cargo-mutants` tests each mutant in a copied source tree without
/// `.git`, for which Git returns 128. Patch applicability must not run
/// there: a mutant overlapping a patch hunk would fail `git apply --check`
/// and be reported as killed without any behavioural or Kani assertion
/// detecting the fault, inflating mutation coverage.
pub(crate) fn is_git_work_tree(root: &Utf8Path) -> Result<bool> {
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
