//! Lowering and execution helpers for the script recipe integration cases.
//!
//! Every helper here is Unix-only because the generated wrapper is a POSIX
//! shell command. Keeping them in one place lets the cases state only their
//! manifest and the assertions that distinguish them.

// Every item in this module is Unix-only, so the parent imports are too.
// Leaving them ungated makes the imports unused on Windows, where `-D warnings`
// turns that into a build failure.
#[cfg(unix)]
use super::*;
#[cfg(unix)]
use netsuke::ninja_gen::{InterpreterExecutable, generate_with_interpreter};
#[cfg(unix)]
use test_support::shell_transport::INTERPRETER_PATH;

/// Generate Ninja text for `shell`, binding an interpreter for the Bash route.
///
/// The renderer refuses an unbound Bash interpreter, so this case binds the
/// absolute fixture path the composition suite also uses. The path is never
/// executed: only the rendered script is decoded and run under the host shell.
#[cfg(unix)]
pub(super) fn generate_for(shell: RecipeShell, graph: &BuildGraph) -> Result<String> {
    match shell {
        RecipeShell::Bash => {
            let executable =
                InterpreterExecutable::from_absolute_path(Utf8PathBuf::from(INTERPRETER_PATH))?;
            Ok(generate_with_interpreter(graph, shell, &executable)?)
        }
        RecipeShell::Posix => Ok(generate_with_shell(graph, shell)?),
        RecipeShell::PowerShell => bail!("this suite covers only the POSIX lexis"),
    }
}

/// One generated script command and the workspace it ran in.
#[cfg(unix)]
pub(super) struct ScriptRun {
    /// Holds the working directory so a case can inspect files it created.
    pub(super) workspace: tempfile::TempDir,
    /// Captures the exit status and the captured streams.
    pub(super) output: std::process::Output,
}

/// A script manifest and the target whose command it generates.
#[cfg(unix)]
pub(super) struct ScriptCase {
    /// Target name exactly as the manifest declares it.
    pub(super) target: String,
    /// Full Netsukefile text.
    pub(super) manifest: String,
}

/// One lowered script command, ready to run under a POSIX shell.
#[cfg(unix)]
pub(super) struct GeneratedCommand(String);

#[cfg(unix)]
impl GeneratedCommand {
    /// Wrap command text a case decoded from the Bash transport.
    pub(super) const fn new(text: String) -> Self {
        Self(text)
    }

    /// Borrow the command text.
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lower `case` for POSIX and return the Ninja command for its target.
///
/// The target name is the untrusted value under test, so it is looked up by
/// the exact text the manifest declared rather than by its lowered encoding.
#[cfg(unix)]
pub(super) fn lower_script(case: &ScriptCase) -> Result<GeneratedCommand> {
    let manifest = manifest::from_str(&case.manifest)?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;
    ninja_commands(&ninja, &case.target).map(GeneratedCommand)
}

/// Run one generated script command in a fresh workspace under `/bin/sh`.
///
/// A case that reaches the shell has already passed lowering, so a non-zero
/// exit is a failure of the generated command and reported with its stderr.
/// The child environment is cleared and given a fixed `PATH`: a
/// non-interactive shell sources `BASH_ENV` whenever `/bin/sh` is Bash, so an
/// inherited hook could run before the generated command and change what the
/// case observes.
#[cfg(unix)]
pub(super) fn run_script(command: &GeneratedCommand) -> Result<ScriptRun> {
    let workspace = tempfile::tempdir()?;
    let output = Command::new("/bin/sh")
        .args(["-ec", command.as_str()])
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .current_dir(workspace.path())
        .output()
        .context("run the generated script command")?;
    ensure!(
        output.status.success(),
        "generated script command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(ScriptRun { workspace, output })
}
