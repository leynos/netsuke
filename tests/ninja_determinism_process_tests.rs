//! `OBL-PROCESS`: one manifest, two processes, one bundle.
//!
//! The published determinism guarantee is a statement about *processes*, not
//! about calls: running the built binary twice over one manifest must produce a
//! byte-identical bundle — the build-file text, every dyndep sidecar's relative
//! path, and every sidecar's content. Every other obligation in this plan
//! compares two emissions inside one process, so a fault that varies with a
//! per-process `RandomState` would pass all of them. This test is the one that
//! starts a second process.
//!
//! **What it detects.** `graph.actions` is an `IrHashMap`, whose iteration order
//! is reseeded per process. `write_action_rules` reads it through
//! `ordered_actions`, and the sort there is the only thing that re-establishes
//! determinism. `MUT-ACTIONSORT` (deleting `actions.sort_by_key`) must therefore
//! make the two runs disagree. The two runs write to separate workspaces, so
//! neither can satisfy the other's sidecar reference.
//!
//! **What it cannot see.** A fault whose output is fixed for a fixed manifest is
//! invisible to a two-run comparison, because both processes make the same
//! mistake. `MUT-DEFSORT` is exactly that: the unsorted `default_targets` order
//! is a property of the declaration, not of the process, so both runs emit the
//! same unsorted `default` line and agree. `MUT-DEFSORT` is `OBL-E2E`'s
//! validator; this obligation is not a substitute for the single-run properties
//! or for the snapshots, and they are not a substitute for it.
//!
//! **The precondition.** Deleting the action sort is observable only when the
//! map has an alternative iteration order to reach, which `EP-M0` question 1
//! measured as roughly five keys. The dedicated fixture therefore carries six
//! distinct commands and a serial edge over three dependencies, and
//! `the_process_fixture_reaches_the_required_classes` fails if either property
//! drifts. Satisfying the obligation with the one-to-three-action snapshot
//! fixtures would leave the mutation unobservable and the test passing without
//! having tested anything.
//!
//! The environment perturbation uses `Command::env`, which mutates only the
//! child's environment; nothing here touches this process's own.

use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::io::ErrorKind;
use tempfile::TempDir;

/// The process fixture's name, for diagnostics.
const PROCESS_FIXTURE_NAME: &str = "ninja_determinism_process.yml";
/// The process fixture: six distinct actions and a serial edge over three deps.
const PROCESS_FIXTURE: &str = include_str!("data/ninja_determinism_process.yml");
/// Fewest distinct actions that make `MUT-ACTIONSORT` observable.
const MIN_DISTINCT_ACTIONS: usize = 5;
/// The fixtures compared across processes, each declaring a different shape.
///
/// The dedicated fixture supplies the action count and the sidecar; the three
/// existing ones supply an implicit default list, a rule-backed command list,
/// and implicit and action-level dependencies. `implicit_deps.yml` names
/// sources that need not exist, which `generate` never reads, so a fixture can
/// be copied into an empty workspace without a file tree around it.
const FIXTURES: &[(&str, &str)] = &[
    (PROCESS_FIXTURE_NAME, PROCESS_FIXTURE),
    (
        "target_defaults.yml",
        include_str!("data/target_defaults.yml"),
    ),
    ("multi_command.yml", include_str!("data/multi_command.yml")),
    ("implicit_deps.yml", include_str!("data/implicit_deps.yml")),
];

/// One emitted bundle: the build file and every dyndep sidecar beside it.
#[derive(Debug)]
struct Bundle {
    /// The text written to `build.ninja`.
    build_file: String,
    /// The staged sidecars, sorted by relative path.
    sidecars: Vec<Sidecar>,
}

/// One dyndep sidecar, identified by its content-addressed path.
#[derive(Debug)]
struct Sidecar {
    /// Path relative to the workspace root.
    path: String,
    /// The sidecar's body.
    content: String,
}

#[test]
fn two_runs_emit_identical_bytes() -> Result<()> {
    for &(fixture, manifest) in FIXTURES {
        let first = emit_once(fixture, manifest, &[])?;
        let second = emit_once(fixture, manifest, &[])?;
        assert_identical(fixture, &first, &second)?;
    }
    Ok(())
}

#[test]
fn the_process_fixture_reaches_the_required_classes() -> Result<()> {
    let bundle = emit_once(PROCESS_FIXTURE_NAME, PROCESS_FIXTURE, &[])?;
    let actions = distinct_action_count(&bundle.build_file);
    ensure!(
        actions >= MIN_DISTINCT_ACTIONS,
        "{PROCESS_FIXTURE_NAME} must emit at least {MIN_DISTINCT_ACTIONS} distinct actions so \
         that deleting the action sort has an alternative iteration order to reach; it emitted \
         {actions}, which leaves MUT-ACTIONSORT unobservable"
    );
    ensure!(
        !bundle.sidecars.is_empty(),
        "{PROCESS_FIXTURE_NAME} must stage at least one dyndep sidecar, otherwise the bundle \
         comparison never reaches a sidecar and is a build-file comparison under another name"
    );
    Ok(())
}

#[test]
fn a_changed_environment_does_not_change_the_bundle() -> Result<()> {
    let baseline = emit_once(PROCESS_FIXTURE_NAME, PROCESS_FIXTURE, &[])?;
    // `TMPDIR` is where the second process may look for scratch space, and the
    // locale changes the copy its diagnostics would use. Neither may reach the
    // emitted bytes.
    let scratch = tempfile::tempdir().context("create a scratch directory for the second run")?;
    let scratch_path = Utf8PathBuf::from_path_buf(scratch.path().to_path_buf())
        .map_err(|path| anyhow!("scratch directory is not UTF-8: {}", path.display()))?;
    let perturbed = emit_once(
        PROCESS_FIXTURE_NAME,
        PROCESS_FIXTURE,
        &[
            ("TMPDIR", scratch_path.as_str()),
            ("NETSUKE_LOCALE", "fr"),
            ("LANG", "fr_FR.UTF-8"),
            ("LC_ALL", "fr_FR.UTF-8"),
        ],
    )?;

    assert_identical(PROCESS_FIXTURE_NAME, &baseline, &perturbed)
}

/// Emit `manifest` once, in a fresh workspace, and read the resulting bundle.
///
/// Each call gets its own temporary directory, so the two runs of a comparison
/// cannot share output files, staged sidecars, or the dyndep directory that
/// `generate` prunes. `environment` is applied to the child only.
fn emit_once(fixture: &str, manifest: &str, environment: &[(&str, &str)]) -> Result<Bundle> {
    let directory =
        tempfile::tempdir().with_context(|| format!("create a workspace for {fixture}"))?;
    let root = open_workspace(&directory)?;
    root.write("Netsukefile", manifest)
        .with_context(|| format!("write the {fixture} manifest"))?;

    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command
        .current_dir(directory.path())
        .args(["-j", "3", "generate", "--output", "build.ninja"]);
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command
        .output()
        .with_context(|| format!("run netsuke generate for {fixture}"))?;
    ensure!(
        output.status.success(),
        "{fixture}: netsuke generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let build_file = root
        .read_to_string("build.ninja")
        .with_context(|| format!("read the {fixture} build file"))?;
    Ok(Bundle {
        build_file,
        sidecars: read_sidecars(&root)?,
    })
}

/// Open a temporary workspace as a capability directory.
fn open_workspace(directory: &TempDir) -> Result<Dir> {
    let path = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow!("temporary workspace is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(&path, ambient_authority()).context("open the temporary workspace")
}

/// Read every `.dd` sidecar under `.netsuke/dyndep`, sorted by path.
///
/// The directory is absent for a manifest whose graph needs no serial gates, so
/// a missing directory is an empty sidecar set rather than a failure.
fn read_sidecars(root: &Dir) -> Result<Vec<Sidecar>> {
    match root.open_dir(".netsuke/dyndep") {
        Ok(dyndep) => read_sidecar_entries(&dyndep),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).context("open the dyndep directory"),
    }
}

/// Read every `.dd` entry of an open dyndep directory.
fn read_sidecar_entries(dyndep: &Dir) -> Result<Vec<Sidecar>> {
    let mut names = dyndep
        .entries()
        .context("enumerate the dyndep directory")?
        .map(|result| result.and_then(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()
        .context("read the dyndep entry names")?;
    names.retain(|name| {
        Utf8Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dd"))
    });
    names.sort_unstable();
    names
        .into_iter()
        .map(|name| {
            let content = dyndep
                .read_to_string(&name)
                .with_context(|| format!("read the sidecar {name}"))?;
            Ok(Sidecar {
                path: Utf8Path::new(".netsuke/dyndep").join(name).into_string(),
                content,
            })
        })
        .collect()
}

/// How many `rule` blocks the build file declares.
fn distinct_action_count(build_file: &str) -> usize {
    build_file
        .lines()
        .filter(|line| line.starts_with("rule "))
        .count()
}

/// Assert that two runs produced the same bundle.
///
/// A sidecar is identified by its content-addressed path as well as its body:
/// two bundles differing only in a sidecar name are still different artefacts,
/// and comparing bodies alone would miss it.
fn assert_identical(fixture: &str, first: &Bundle, second: &Bundle) -> Result<()> {
    if first.build_file != second.build_file {
        bail!(
            "{fixture}: two processes emitted different build files\n{}",
            describe_difference(&first.build_file, &second.build_file)
        );
    }
    if first.sidecars.len() != second.sidecars.len() {
        bail!(
            "{fixture}: two processes staged {} and {} dyndep sidecars: {:?} vs {:?}",
            first.sidecars.len(),
            second.sidecars.len(),
            sidecar_paths(first),
            sidecar_paths(second),
        );
    }
    for (left, right) in first.sidecars.iter().zip(&second.sidecars) {
        if left.path != right.path {
            bail!(
                "{fixture}: two processes staged sidecars at different paths: {} vs {}",
                left.path,
                right.path
            );
        }
        if left.content != right.content {
            bail!(
                "{fixture}: two processes staged different content for {}\n{}",
                left.path,
                describe_difference(&left.content, &right.content)
            );
        }
    }
    Ok(())
}

/// The sidecar paths of one bundle, for a diagnostic.
fn sidecar_paths(bundle: &Bundle) -> Vec<&str> {
    bundle
        .sidecars
        .iter()
        .map(|sidecar| sidecar.path.as_str())
        .collect()
}

/// Describe the first line on which two texts differ.
///
/// Lines are truncated because an emitted command can run to hundreds of
/// characters, and the reader needs the location and a recognizable fragment
/// rather than the whole wrapper.
fn describe_difference(first: &str, second: &str) -> String {
    for (index, (left, right)) in first.lines().zip(second.lines()).enumerate() {
        if left != right {
            return format!(
                "first difference at line {}:\n  first:  {}\n  second: {}",
                index + 1,
                truncate(left),
                truncate(right),
            );
        }
    }
    format!(
        "one text is a prefix of the other: {} and {} lines",
        first.lines().count(),
        second.lines().count(),
    )
}

/// Shorten one line to a diagnostic-friendly length.
///
/// The prefix is rebuilt character by character rather than sliced at a byte
/// offset: the line may contain non-ASCII text, and a byte slice would panic
/// if its end fell inside a multi-byte character.
fn truncate(line: &str) -> String {
    const LIMIT: usize = 200;
    if line.len() <= LIMIT {
        return line.to_owned();
    }
    let mut prefix = String::with_capacity(LIMIT + '…'.len_utf8());
    for character in line.chars() {
        if prefix.len() + character.len_utf8() > LIMIT {
            break;
        }
        prefix.push(character);
    }
    prefix.push('…');
    prefix
}
