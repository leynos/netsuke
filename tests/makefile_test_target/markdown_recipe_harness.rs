//! Fixture support for the Markdown recipe behavioural tests.
//!
//! Everything a test in `markdown_recipes` needs to run `make fmt` or
//! `make check-fmt` hermetically: fake executables for Cargo, Ruff,
//! `mdtablefix`, and `markdownlint-cli2` that append one tab-separated record
//! per call to a shared log and exit with a code the test chooses; throwaway
//! Git repositories holding tracked, untracked, and ignored Markdown, or no
//! Markdown at all; and the [`Harness`] that runs a Make target against the
//! Makefile's `CARGO`, `RUFF`, `MDTABLEFIX`, and `MDLINT` overrides and parses
//! the log back into [`Invocation`]s. Selection stays inside `mdtablefix`
//! `--git`, so nothing here passes a path to it or feeds it standard input.
//!
//! Split from `markdown_recipes` so each module stays under the workspace's
//! module-size limit; the tests own the assertions, this module owns the
//! world they run in.

use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::{TempDir, tempdir};
use test_support::{fs, write_exec_with_content};

/// The selection every Markdown recipe must delegate to `mdtablefix`.
pub(super) const SELECTION: [&str; 2] = ["--git", "--include-untracked"];
/// The complete shared rule set both recipes must pass.
pub(super) const RULES: [&str; 5] = ["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];
/// The Makefile assignment the rule set is read from, for cross-checking.
pub(super) const RULES_ASSIGNMENT: &str = "MDTABLEFIX_RULES = ";
/// The environment variable the fakes append their invocation records to.
const LOG_VARIABLE: &str = "MARKDOWN_RECIPE_INVOCATION_LOG";
/// The fake executables, in the names the Makefile variables expect.
const FAKES: [(&str, &str); 4] = [
    ("CARGO", "cargo"),
    ("RUFF", "ruff"),
    ("MDTABLEFIX", "mdtablefix"),
    ("MDLINT", "markdownlint-cli2"),
];

/// One recorded call: the fake's name and the arguments it received.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Invocation {
    pub(super) program: String,
    pub(super) arguments: Vec<String>,
    /// Bytes the fake could read from standard input; `mdtablefix` only.
    pub(super) stdin_bytes: Option<u64>,
}

/// The fakes, their log, and the Git workspace a recipe runs inside.
pub(super) struct Harness {
    _fakes_dir: TempDir,
    fakes: Vec<(&'static str, PathBuf)>,
    log: PathBuf,
    workspace: TempDir,
}

/// The exit code each fake reports, defaulting to success.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ExitCodes {
    pub(super) cargo: u8,
    pub(super) ruff: u8,
    pub(super) mdtablefix: u8,
    pub(super) markdownlint: u8,
}

/// Write a fake executable that logs its name and every argument.
///
/// Each record is one line: the name, then each argument, tab-separated. The
/// `mdtablefix` fake also counts the bytes on its standard input, which is how
/// the tests show Make feeds it nothing. The exit code comes from
/// `FAKE_<NAME>_EXIT`, so a test can model drift, an operational failure, or
/// an earlier formatter failing.
fn write_fake(directory: &Path, name: &str) -> Result<PathBuf> {
    let exit_variable = format!("FAKE_{}_EXIT", name.replace('-', "_").to_uppercase());
    let stdin_record = if name == "mdtablefix" {
        "printf '\\tstdin-bytes=%s' \"$(wc -c < /dev/stdin | tr -d ' ')\"\n"
    } else {
        ""
    };
    let script = format!(
        "#!/bin/sh\n\
         : \"${{{LOG_VARIABLE}:?}}\"\n\
         {{\n  printf '%s' '{name}'\n  for argument in \"$@\"; do printf '\\t%s' \"$argument\"; done\n  \
         {stdin_record}  printf '\\n'\n}} >> \"${LOG_VARIABLE}\"\n\
         exit \"${{{exit_variable}:-0}}\"\n"
    );
    write_exec_with_content(directory, name, &script)
        .with_context(|| format!("write fake {name} executable"))
}

/// Run `git` inside `repository` with a fixed identity and no host config.
fn git(repository: &Path, arguments: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repository)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .args(arguments)
        .output()
        .with_context(|| format!("run git {arguments:?}"))?;
    ensure!(
        output.status.success(),
        "git {arguments:?} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// The Markdown a populated workspace holds, by Git state.
pub(super) const TRACKED: &str = "tracked.md";
pub(super) const UNTRACKED: &str = "untracked.md";
pub(super) const IGNORED: &str = "ignored.md";
const MARKDOWN_BODY: &str = "# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";

/// Create a Git repository with tracked, untracked, and ignored Markdown.
pub(super) fn populated_workspace() -> Result<TempDir> {
    let workspace = tempdir().context("create Markdown workspace")?;
    let root = workspace.path();
    git(root, &["init", "--quiet"])?;
    fs::write(root.join(".gitignore"), format!("{IGNORED}\n")).context("write .gitignore")?;
    fs::write(root.join(TRACKED), MARKDOWN_BODY).context("write tracked Markdown")?;
    git(root, &["add", ".gitignore", TRACKED])?;
    git(root, &["commit", "--quiet", "--message", "seed"])?;
    fs::write(root.join(UNTRACKED), MARKDOWN_BODY).context("write untracked Markdown")?;
    fs::write(root.join(IGNORED), MARKDOWN_BODY).context("write ignored Markdown")?;
    Ok(workspace)
}

/// Create a Git repository that selects no Markdown at all.
pub(super) fn empty_selection_workspace() -> Result<TempDir> {
    let workspace = tempdir().context("create empty-selection workspace")?;
    let root = workspace.path();
    git(root, &["init", "--quiet"])?;
    fs::write(root.join("notes.txt"), "not markdown\n").context("write a non-Markdown file")?;
    git(root, &["add", "notes.txt"])?;
    git(root, &["commit", "--quiet", "--message", "seed"])?;
    Ok(workspace)
}

impl Harness {
    pub(super) fn new(workspace: TempDir) -> Result<Self> {
        let fakes_dir = tempdir().context("create fake executable directory")?;
        let fakes = FAKES
            .iter()
            .map(|(variable, name)| Ok((*variable, write_fake(fakes_dir.path(), name)?)))
            .collect::<Result<Vec<_>>>()?;
        let log = fakes_dir.path().join("invocations.log");
        Ok(Self {
            _fakes_dir: fakes_dir,
            fakes,
            log,
            workspace,
        })
    }

    /// Run one Make target inside the workspace with every tool faked.
    pub(super) fn run(&self, target: &str, exits: ExitCodes) -> Result<Output> {
        let mut make = Command::new("make");
        make.current_dir(self.workspace.path())
            .arg("--no-print-directory")
            .arg("-f")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Makefile"))
            .arg(target)
            .env(LOG_VARIABLE, &self.log)
            .env("FAKE_CARGO_EXIT", exits.cargo.to_string())
            .env("FAKE_RUFF_EXIT", exits.ruff.to_string())
            .env("FAKE_MDTABLEFIX_EXIT", exits.mdtablefix.to_string())
            .env(
                "FAKE_MARKDOWNLINT_CLI2_EXIT",
                exits.markdownlint.to_string(),
            )
            // The recipes get no standard input, as they do under CI.
            .stdin(Stdio::null());
        for (variable, path) in &self.fakes {
            make.arg(format!("{variable}={}", path.display()));
        }
        make.output()
            .with_context(|| format!("run make {target} against the fakes"))
    }

    /// Parse the invocation log into its recorded calls, in order.
    pub(super) fn invocations(&self) -> Result<Vec<Invocation>> {
        let log = fs::read_to_string(&self.log).context("read the fake invocation log")?;
        log.lines()
            .map(|line| {
                let mut fields = line.split('\t');
                let program = fields
                    .next()
                    .context("each invocation record names its program")?
                    .to_owned();
                let mut arguments: Vec<String> = fields.map(ToOwned::to_owned).collect();
                let stdin_bytes = match arguments
                    .last()
                    .and_then(|last| last.strip_prefix("stdin-bytes="))
                {
                    Some(count) => {
                        let value = count
                            .parse()
                            .context("stdin byte count should be numeric")?;
                        arguments.pop();
                        Some(value)
                    }
                    None => None,
                };
                Ok(Invocation {
                    program,
                    arguments,
                    stdin_bytes,
                })
            })
            .collect()
    }

    pub(super) fn markdown_bytes(&self) -> Result<Vec<(String, String)>> {
        [TRACKED, UNTRACKED, IGNORED]
            .iter()
            .map(|name| {
                let path = self.workspace.path().join(name);
                let body = fs::read_to_string(&path)
                    .with_context(|| format!("read {name} after the recipe"))?;
                Ok(((*name).to_owned(), body))
            })
            .collect()
    }
}
