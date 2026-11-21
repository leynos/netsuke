//! Process and filesystem scaffolding for the real-Ninja phony scenarios.
//!
//! The scenario files drive the `netsuke` CLI against an actual `ninja` binary
//! and assert observable effects. They share this module so each scenario reads
//! as the behaviour it checks rather than as workspace setup. The child process
//! receives its own `PATH` and `NETSUKE_NINJA`, so no in-process environment
//! mutation is involved even though the scenarios run in parallel.
//!
//! When Ninja is unavailable the scenarios skip, unless the injected
//! `NETSUKE_REQUIRE_NINJA=1` setting has made a missing installation fatal.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::fixture;
use tempfile::TempDir;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};
use test_support::ninja::{
    NinjaWorkspaceError, ninja_integration_workspace, resolve_ninja_program,
};

/// The `PATH` recipes see: a host utility directory, not the test runner's.
pub(crate) const RECIPE_PATH: &str = "/usr/bin:/bin";

/// An action, which Netsuke treats as an implicitly phony operation.
///
/// `targets` is required even when the manifest declares none.
pub(crate) const ACTION_MANIFEST: &str = include_str!("../data/phony_action.yml");

/// A target that declares itself phony in the manifest.
pub(crate) const PHONY_TARGET_MANIFEST: &str = include_str!("../data/phony_target.yml");

/// An always target whose output keeps its modification time, plus a consumer.
pub(crate) const ALWAYS_MANIFEST: &str = include_str!("../data/phony_always.yml");

/// A phony action named after a user directory, plus a real always output.
pub(crate) const CLEAN_MANIFEST: &str = include_str!("../data/phony_clean.yml");

/// A phony target whose output names a directory that does not exist yet.
pub(crate) const NESTED_OUTPUT_MANIFEST: &str = include_str!("../data/phony_nested_output.yml");

/// A nested phony output beside an unrelated phony target that is the default.
pub(crate) const UNSELECTED_OUTPUT_MANIFEST: &str =
    include_str!("../data/phony_unselected_output.yml");

/// A prerequisite whose recipe creates the directory a phony output needs.
pub(crate) const PREREQUISITE_DIRECTORY_MANIFEST: &str =
    include_str!("../data/phony_prerequisite_directory.yml");

/// A nested phony output whose serial prerequisites must run in order.
pub(crate) const SERIAL_OUTPUT_MANIFEST: &str = include_str!("../data/phony_serial_output.yml");

/// The force node, relative to the workspace.
pub(crate) const FORCE_NODE: &str = ".netsuke/phony/force";

/// Build a manifest whose always target declares the force node as its output.
///
/// The spelling is absolute, so only the runner's working-directory resolution
/// can tell that it names generated state.
pub(crate) fn alias_manifest(force_node: impl AsRef<Utf8Path>) -> String {
    include_str!("../data/phony_alias.yml").replace("{force_node}", force_node.as_ref().as_str())
}

/// Provide a temporary workspace, or skip when Ninja is unavailable.
///
/// Only a failed Ninja probe is a skip: a probe answer means the scenario was
/// expected to run, so failing to create its workspace is a real error rather
/// than an environment gap.
#[fixture]
pub(crate) fn temp_workspace() -> Result<Option<TempDir>> {
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
pub(crate) enum Shadow {
    /// A directory at the target's name, holding one user file.
    Directory,
    /// A regular file at the target's name.
    File,
}

impl Shadow {
    /// Create the shadowing path inside `workspace`.
    ///
    /// The shadow's location is a workspace-relative path, so it is typed as
    /// one rather than as free text that happens to look like a path.
    pub(crate) fn create(self, workspace: &Workspace, path: impl AsRef<Utf8Path>) -> Result<()> {
        let name = path.as_ref();
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
    pub(crate) fn survivor(self, path: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        let name = path.as_ref();
        match self {
            Self::Directory => name.join("keep.txt"),
            Self::File => name.to_path_buf(),
        }
    }
}

/// One temporary workspace holding a manifest and a pinned Ninja program.
pub(crate) struct Workspace {
    /// The temporary directory the child process runs in.
    directory: TempDir,
    /// The same directory as a capability-oriented handle.
    pub(crate) root: Dir,
    /// The Ninja program the child is told to run.
    ninja: Utf8PathBuf,
}

impl Workspace {
    /// Open `directory` as a workspace and resolve the Ninja program.
    pub(crate) fn new(directory: TempDir) -> Result<Self> {
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
    pub(crate) fn write_manifest(&self, manifest: &str) -> Result<()> {
        self.root
            .write("Netsukefile", manifest)
            .context("write the test manifest")
    }

    /// Run `netsuke` in the workspace with the supplied arguments.
    pub(crate) fn run(&self, arguments: &[&str]) -> Result<NetsukeRun> {
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
    pub(crate) fn build(&self) -> Result<NetsukeRun> {
        let run = self.run(&["build"])?;
        anyhow::ensure!(run.success, "netsuke build failed: {}", run.stderr);
        Ok(run)
    }

    /// Read `name` as lines, treating a missing file as no lines.
    pub(crate) fn lines(&self, path: impl AsRef<Utf8Path>) -> Result<Vec<String>> {
        let name = path.as_ref();
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

    /// Report whether `name` exists in the workspace.
    pub(crate) fn exists(&self, path: impl AsRef<Utf8Path>) -> Result<bool> {
        let name = path.as_ref();
        self.root
            .try_exists(name)
            .with_context(|| format!("check whether {name} exists"))
    }

    /// Create `name` and its parents inside the workspace.
    pub(crate) fn create_dir_all(&self, path: impl AsRef<Utf8Path>) -> Result<()> {
        let name = path.as_ref();
        self.root
            .create_dir_all(name)
            .with_context(|| format!("create {name}"))
    }

    /// Remove `name` and everything beneath it.
    pub(crate) fn remove_dir_all(&self, path: impl AsRef<Utf8Path>) -> Result<()> {
        let name = path.as_ref();
        self.root
            .remove_dir_all(name)
            .with_context(|| format!("remove {name}"))
    }

    /// Write `contents` to `name`.
    pub(crate) fn write(&self, path: impl AsRef<Utf8Path>, contents: &[u8]) -> Result<()> {
        let name = path.as_ref();
        self.root
            .write(name, contents)
            .with_context(|| format!("write {name}"))
    }

    /// Read `name` as text.
    pub(crate) fn read_to_string(&self, path: impl AsRef<Utf8Path>) -> Result<String> {
        let name = path.as_ref();
        self.root
            .read_to_string(name)
            .with_context(|| format!("read {name}"))
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
