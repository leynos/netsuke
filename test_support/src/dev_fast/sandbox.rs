//! A hermetic `PATH` and `HOME` for exercising the `dev-fast` targets.
//!
//! See the parent module for why the sandbox is built from nothing rather
//! than by prepending fakes to the ambient `PATH`.
use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

use crate::exec::write_exec_with_content;
use super::MakeInvocation;

/// Utilities the scripts and `make` legitimately need. Kept explicit so a new
/// dependency surfaces as a test failure rather than silently resolving to
/// whatever the developer happens to have installed.
const SANDBOX_UTILITIES: &[&str] = &[
    "awk",
    "bash",
    "cat",
    "chmod",
    "cp",
    "curl",
    "dirname",
    "env",
    "grep",
    "gzip",
    "ln",
    "make",
    "mkdir",
    "mktemp",
    "rm",
    "sed",
    "sh",
    "sha256sum",
    "tar",
    "touch",
    "tr",
    "uname",
];

/// A `PATH` and `HOME` containing only what a test explicitly puts there.
pub struct Sandbox {
    temp: TempDir,
    repo: Utf8PathBuf,
}

impl Sandbox {
    /// Build a sandbox and populate it with the utility allowlist.
    pub fn new() -> Result<Self> {
        let temp = tempdir().context("create sandbox directory")?;
        let repo = std::env::current_dir().context("resolve repository root")?;
        let repo = Utf8PathBuf::try_from(repo).context("repository root must be UTF-8")?;
        let sandbox = Self { temp, repo };
        fs::create_dir_all(sandbox.bin().as_std_path()).context("create sandbox bin")?;
        fs::create_dir_all(sandbox.home().as_std_path()).context("create sandbox home")?;
        sandbox.link_utilities()?;
        Ok(sandbox)
    }

    fn root(&self) -> Utf8PathBuf {
        Utf8Path::from_path(self.temp.path())
            .expect("tempdir path must be UTF-8")
            .to_path_buf()
    }

    /// The only directory on the sandbox `PATH`.
    pub fn bin(&self) -> Utf8PathBuf {
        self.root().join("bin")
    }

    /// A `HOME` inside the sandbox, isolating the Makefile's `PATH` export.
    pub fn home(&self) -> Utf8PathBuf {
        self.root().join("home")
    }

    /// An install prefix that starts out empty; `DEV_FAST_PREFIX` points here.
    pub fn prefix(&self) -> Utf8PathBuf {
        self.root().join("prefix")
    }

    fn link_utilities(&self) -> Result<()> {
        for utility in SANDBOX_UTILITIES {
            let source =
                which(utility).with_context(|| format!("locate `{utility}` for the sandbox"))?;
            std::os::unix::fs::symlink(
                source.as_std_path(),
                self.bin().join(utility).as_std_path(),
            )
            .with_context(|| format!("link `{utility}` into the sandbox"))?;
        }
        Ok(())
    }

    /// Install a fake executable, creating `dir` if needed.
    ///
    /// `body` is a shell fragment without a shebang; this adds one, so call
    /// sites stay focused on the behaviour they are faking.
    pub fn write_fake(&self, dir: &Utf8Path, name: &str, body: &str) -> Result<Utf8PathBuf> {
        fs::create_dir_all(dir.as_std_path()).with_context(|| format!("create {dir}"))?;
        let script = format!("#!/bin/sh\n{body}\n");
        let path = write_exec_with_content(dir.as_std_path(), name, &script)?;
        Utf8PathBuf::try_from(path).context("fake executable path must be UTF-8")
    }

    /// A `mold` reporting the given version, formatted as the real one does.
    pub fn write_mold(&self, dir: &Utf8Path, version: &str) -> Result<Utf8PathBuf> {
        self.write_fake(
            dir,
            "mold",
            &format!("echo \"mold {version} (compatible with GNU ld)\""),
        )
    }

    /// A `rustup` reporting the given toolchain and, optionally, the Cranelift
    /// component as installed.
    pub fn write_rustup(&self, toolchain: &str, has_cranelift: bool) -> Result<Utf8PathBuf> {
        let component = if has_cranelift {
            "rustc-codegen-cranelift-x86_64-unknown-linux-gnu"
        } else {
            "rustfmt-x86_64-unknown-linux-gnu"
        };
        let body = format!(
            concat!(
                "case \"$1 $2\" in\n",
                "  'toolchain list') echo '{toolchain}-x86_64-unknown-linux-gnu' ;;\n",
                "  'component list') echo '{component}' ;;\n",
                "  *) exit 0 ;;\n",
                "esac"
            ),
            toolchain = toolchain,
            component = component,
        );
        self.write_fake(&self.bin(), "rustup", &body)
    }

    fn base_command(&self, program: &Utf8Path) -> Command {
        let mut command = Command::new(program.as_std_path());
        command
            .current_dir(self.repo.as_std_path())
            .env_clear()
            .env("PATH", self.bin().as_std_path())
            .env("HOME", self.home().as_std_path());
        command
    }

    /// Run a `scripts/` entry point directly, with the sandbox as the entire
    /// environment plus the given overrides. Use where a test must vary inputs
    /// the Makefile does not expose as variables.
    pub fn script(&self, name: &str, env: &[(&str, String)]) -> Result<Output> {
        let mut command = self.base_command(&self.bin().join("bash"));
        command
            .env("DEV_FAST_PREFIX", self.prefix().as_std_path())
            .env("MOLD_VERSION_FILE", "tools/mold/VERSION")
            .env("MOLD_SHA256SUMS_FILE", "tools/mold/SHA256SUMS")
            .env("CRANELIFT_TOOLCHAIN_FILE", "tools/cranelift/VERSION")
            .env("DEV_FAST_CONFIG", "tools/dev-fast/config.toml")
            .arg(format!("scripts/{name}"));
        for (key, value) in env {
            command.env(key, value);
        }
        command
            .output()
            .with_context(|| format!("run scripts/{name}"))
    }

    /// Run a Make target with the sandbox as the entire environment and no
    /// overrides beyond the sandbox's own install prefix.
    pub fn make(&self, target: &str) -> Result<Output> {
        self.run_make(&MakeInvocation::new(target))
    }

    /// Run a described Make invocation with the sandbox as the entire
    /// environment. The sandbox's install prefix is applied first, so an
    /// invocation may still override it.
    pub fn run_make(&self, invocation: &MakeInvocation) -> Result<Output> {
        let mut command = self.base_command(&self.bin().join("make"));
        command
            .arg("--no-print-directory")
            .arg("-f")
            .arg("Makefile")
            .arg(format!("DEV_FAST_PREFIX={}", self.prefix()));
        for (name, value) in invocation.environment_entries() {
            command.env(name, value);
        }
        for argument in invocation.variable_arguments() {
            command.arg(argument);
        }
        command.arg(invocation.target());
        command
            .output()
            .with_context(|| format!("run make {}", invocation.target()))
    }
}

/// Resolve a utility against the ambient `PATH`, before it is replaced.
///
/// Executability is part of the match, not an afterthought: `uv` installs a
/// sourceable `env` shell fragment into `~/.local/bin`, so a plain file probe
/// selects a non-executable file and every sandboxed `env` invocation then
/// fails with a permission error.
fn which(utility: &str) -> Result<Utf8PathBuf> {
    let path = std::env::var_os("PATH").context("read PATH")?;
    for dir in std::env::split_paths(&path) {
        let Ok(dir) = Utf8PathBuf::try_from(dir) else {
            continue;
        };
        let candidate = dir.join(utility);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    bail!("`{utility}` not found on PATH")
}

fn is_executable_file(path: &Utf8Path) -> bool {
    fs::metadata(path.as_std_path())
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Combined stdout and stderr, for asserting on diagnostics regardless of the
/// stream a given message went to.
pub fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Read a `tools/` version pin, so tests agree with the repository rather than
/// hard-coding a value that drifts on the next bump.
fn read_pin(path: &str) -> Result<String> {
    Ok(fs::read_to_string(path)
        .with_context(|| format!("read {path}"))?
        .trim()
        .to_owned())
}

/// The repository's pinned mold release tag.
pub fn pinned_mold_version() -> Result<String> {
    read_pin("tools/mold/VERSION")
}

/// The repository's pinned Cranelift-bearing nightly.
pub fn pinned_toolchain() -> Result<String> {
    read_pin("tools/cranelift/VERSION")
}
