//! Real-Ninja runtime tests for the `phony` and `always` lowerings.
//!
//! Every scenario here drives the `netsuke` CLI against an actual `ninja`
//! binary and asserts observable effects: lines appended to a log, files that
//! survive `netsuke clean`, and files that do not. The generated text is pinned
//! separately by `tests/ninja_phony_lowering_tests.rs`; this file exists
//! because correct-looking text is not evidence that Ninja schedules it the way
//! the manifest author asked.
//!
//! The child process receives its own `PATH` and `NETSUKE_NINJA`, so no
//! in-process environment mutation is involved even though the tests run in
//! parallel.
//!
//! When Ninja is unavailable the scenarios skip, unless the injected
//! `NETSUKE_REQUIRE_NINJA=1` setting has made a missing installation fatal.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};
use test_support::ninja::{
    NinjaWorkspaceError, ninja_integration_workspace, resolve_ninja_program,
};

/// The `PATH` recipes see: a host utility directory, not the test runner's.
const RECIPE_PATH: &str = "/usr/bin:/bin";

/// An action, which Netsuke treats as an implicitly phony operation.
///
/// `targets` is required even when the manifest declares none.
const ACTION_MANIFEST: &str = include_str!("data/phony_action.yml");

/// A target that declares itself phony in the manifest.
const PHONY_TARGET_MANIFEST: &str = include_str!("data/phony_target.yml");

/// An always target whose output keeps its modification time, plus a consumer.
const ALWAYS_MANIFEST: &str = include_str!("data/phony_always.yml");

/// A phony action named after a user directory, plus a real always output.
const CLEAN_MANIFEST: &str = include_str!("data/phony_clean.yml");

/// The force node, relative to the workspace.
const FORCE_NODE: &str = ".netsuke/phony/force";

/// Build a manifest whose always target declares the force node as its output.
///
/// The spelling is absolute, so only the runner's working-directory resolution
/// can tell that it names generated state.
fn alias_manifest(force_node: &str) -> String {
    include_str!("data/phony_alias.yml").replace("{force_node}", force_node)
}

/// Provide a temporary workspace, or skip when Ninja is unavailable.
///
/// Only a failed Ninja probe is a skip: a probe answer means the scenario was
/// expected to run, so failing to create its workspace is a real error rather
/// than an environment gap.
#[fixture]
fn temp_workspace() -> Result<Option<TempDir>> {
    match ninja_integration_workspace() {
        Ok(directory) => Ok(Some(directory)),
        Err(
            NinjaWorkspaceError::ProbeSpawn(_)
            | NinjaWorkspaceError::ProbeFailed(_)
            | NinjaWorkspaceError::ProbeTimeout(_),
        ) => Ok(None),
        Err(error @ NinjaWorkspaceError::Workspace(_)) => {
            Err(error).context("create the Ninja integration workspace")
        }
    }
}

/// How a test shadows a logical target's name with a real user path.
#[derive(Clone, Copy)]
enum Shadow {
    /// A directory at the target's name, holding one user file.
    Directory,
    /// A regular file at the target's name.
    File,
}

impl Shadow {
    /// Create the shadowing path inside `workspace`.
    fn create(self, workspace: &Workspace, name: &str) -> Result<()> {
        match self {
            Self::Directory => {
                workspace
                    .root
                    .create_dir(name)
                    .with_context(|| format!("create user directory {name}"))?;
                workspace
                    .root
                    .write(self.survivor(name), b"user content")
                    .with_context(|| format!("write {name}/keep.txt"))?;
            }
            Self::File => workspace
                .root
                .write(name, b"user content")
                .with_context(|| format!("write user file {name}"))?,
        }
        Ok(())
    }

    /// Return the path that must still exist after the build and clean.
    fn survivor(self, name: &str) -> Utf8PathBuf {
        match self {
            Self::Directory => Utf8Path::new(name).join("keep.txt"),
            Self::File => Utf8PathBuf::from(name),
        }
    }
}

/// One temporary workspace holding a manifest and a pinned Ninja program.
struct Workspace {
    /// The temporary directory the child process runs in.
    directory: TempDir,
    /// The same directory as a capability-oriented handle.
    root: Dir,
    /// The Ninja program the child is told to run.
    ninja: Utf8PathBuf,
}

impl Workspace {
    /// Open `directory` as a workspace and resolve the Ninja program.
    fn new(directory: TempDir) -> Result<Self> {
        let path = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).map_err(|path| {
            anyhow::anyhow!("test workspace path is not UTF-8: {}", path.display())
        })?;
        let root =
            Dir::open_ambient_dir(&path, ambient_authority()).context("open the test workspace")?;
        Ok(Self {
            directory,
            root,
            ninja: resolve_ninja()?,
        })
    }

    /// Write `manifest` as the workspace's `Netsukefile`.
    fn write_manifest(&self, manifest: &str) -> Result<()> {
        self.root
            .write("Netsukefile", manifest)
            .context("write the test manifest")
    }

    /// Run `netsuke` in the workspace with the supplied arguments.
    fn run(&self, arguments: &[&str]) -> Result<NetsukeRun> {
        run_netsuke_in_with_env(
            self.directory.path(),
            arguments,
            &[
                ("PATH", RECIPE_PATH),
                ("NETSUKE_NINJA", self.ninja.as_str()),
            ],
        )
        .with_context(|| format!("run netsuke {}", arguments.join(" ")))
    }

    /// Run `netsuke build`, requiring it to succeed.
    fn build(&self) -> Result<NetsukeRun> {
        let run = self.run(&["build"])?;
        ensure!(run.success, "netsuke build failed: {}", run.stderr);
        Ok(run)
    }

    /// Read `name` as lines, treating a missing file as no lines.
    fn lines(&self, name: &str) -> Result<Vec<String>> {
        let exists = self
            .root
            .try_exists(name)
            .with_context(|| format!("check whether {name} exists"))?;
        if !exists {
            return Ok(Vec::new());
        }
        let content = self
            .root
            .read_to_string(name)
            .with_context(|| format!("read {name}"))?;
        Ok(content.lines().map(str::to_owned).collect())
    }
}

/// Resolve `ninja` to the absolute executable the probe already ran.
///
/// Sharing the fixture crate's resolver is what keeps the probe and the child
/// launch on one executable: both walk `PATH` with the same executability rule
/// and the same resolution of relative entries. The child receives a minimal
/// `PATH`, so the program is named to it explicitly instead of being searched
/// for.
fn resolve_ninja() -> Result<Utf8PathBuf> {
    let program = resolve_ninja_program()
        .context("ninja must be on PATH for the phony and always runtime tests")?;
    Utf8PathBuf::from_path_buf(program)
        .map_err(|path| anyhow::anyhow!("ninja path is not UTF-8: {}", path.display()))
}

#[rstest]
#[case::action_over_a_directory(ACTION_MANIFEST, Shadow::Directory)]
#[case::action_over_a_file(ACTION_MANIFEST, Shadow::File)]
#[case::phony_target_over_a_directory(PHONY_TARGET_MANIFEST, Shadow::Directory)]
#[case::phony_target_over_a_file(PHONY_TARGET_MANIFEST, Shadow::File)]
fn a_logical_target_runs_despite_a_same_named_user_path(
    temp_workspace: Result<Option<TempDir>>,
    #[case] manifest: &str,
    #[case] shadow: Shadow,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(manifest)?;
    shadow.create(&workspace, "check")?;

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 1,
        "a same-named user path must not suppress the recipe: {:?}",
        workspace.lines("runs.log")?,
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "a logical target must run on every build: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace
            .root
            .try_exists(shadow.survivor("check"))
            .context("check the surviving user path")?,
        "the build must not consume the user path that shares the target's name",
    );
    Ok(())
}

#[rstest]
fn always_reruns_and_restat_prunes_its_downstream(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(ALWAYS_MANIFEST)?;

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 1,
        "the forced target must run on the first build: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace.lines("consumer.log")?.len() == 1,
        "the downstream target must run on the first build: {:?}",
        workspace.lines("consumer.log")?,
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "an always target must re-run while its inputs are unchanged: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace.lines("consumer.log")?.len() == 1,
        "restat must prune the downstream target when the forced output is unchanged: {:?}",
        workspace.lines("consumer.log")?,
    );
    Ok(())
}

#[rstest]
fn a_stray_force_node_is_cleared_before_the_build(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(ALWAYS_MANIFEST)?;
    workspace
        .root
        .create_dir_all(".netsuke/phony")
        .context("create the reserved phony namespace")?;
    workspace
        .root
        .write(".netsuke/phony/force", b"stray")
        .context("write a stray force node")?;

    workspace.build()?;
    ensure!(
        !workspace
            .root
            .try_exists(".netsuke/phony/force")
            .context("check the force node after the build")?,
        "a stray path at the force node must not survive the build",
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "a stray force node must not suppress the always target: {:?}",
        workspace.lines("runs.log")?,
    );
    Ok(())
}

#[rstest]
fn a_declared_alias_of_the_force_node_stops_the_build(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let force_node = Utf8PathBuf::from_path_buf(directory.path().join(FORCE_NODE))
        .map_err(|path| anyhow::anyhow!("force node path is not UTF-8: {}", path.display()))?;
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(&alias_manifest(force_node.as_str()))?;
    workspace
        .root
        .create_dir_all(".netsuke/phony")
        .context("create the reserved phony namespace")?;
    workspace
        .root
        .write(FORCE_NODE, b"declared output")
        .context("write the path the manifest declares")?;

    let run = workspace.run(&["build"])?;
    ensure!(
        !run.success,
        "a manifest declaring the force node under an absolute spelling must be refused",
    );
    ensure!(
        run.stderr.contains("reserved"),
        "the refusal must report the reservation, not a Ninja failure: {}",
        run.stderr,
    );
    ensure!(
        workspace
            .root
            .try_exists(FORCE_NODE)
            .context("check the declared output")?,
        "the refused reset must leave the declared output in place",
    );
    Ok(())
}

#[rstest]
fn clean_preserves_user_paths_and_removes_real_outputs(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(CLEAN_MANIFEST)?;
    Shadow::Directory.create(&workspace, "check")?;

    workspace.build()?;
    ensure!(
        workspace
            .root
            .try_exists("stamp")
            .context("check the always output")?,
        "the always target must materialise its declared output",
    );
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "both the action and the always target must run: {:?}",
        workspace.lines("runs.log")?,
    );

    let clean = workspace.run(&["clean"])?;
    ensure!(clean.success, "netsuke clean failed: {}", clean.stderr);

    ensure!(
        workspace
            .root
            .try_exists("check/keep.txt")
            .context("check the user path after clean")?,
        "clean must not remove a user path that shares a phony name",
    );
    ensure!(
        !workspace
            .root
            .try_exists("stamp")
            .context("check the always output after clean")?,
        "clean must remove the output of a real always edge",
    );
    Ok(())
}
