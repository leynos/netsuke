//! A hermetic `PATH` and `HOME` for exercising the `dev-fast` targets.
//!
//! See the parent module for why the sandbox is built from nothing rather
//! than by prepending fakes to the ambient `PATH`.
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use mockable::{DefaultEnv, Env};
use std::io::ErrorKind;
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

use super::MakeInvocation;
use crate::exec::write_exec_with_content;
use crate::fs;

mod utilities;
use utilities::{SANDBOX_UTILITIES, which};
pub use utilities::{real_utility, real_utility_with_env};

/// Whether a script run supplies the pin-file environment overrides.
///
/// A closed choice rather than a bare `bool`, so a call site reads as the
/// scenario it is describing.
#[derive(Copy, Clone, Debug)]
pub enum PinOverrides {
    /// Point the script at the pin files explicitly, as the Makefile does.
    Supplied,
    /// Leave the variables unset, exercising the committed defaults.
    Omitted,
}

/// A `PATH` and `HOME` containing only what a test explicitly puts there.
pub struct Sandbox {
    /// Retained so the temporary tree outlives the sandbox; the usable path is
    /// `root`, validated as UTF-8 once during construction.
    _temp: TempDir,
    root: Utf8PathBuf,
    repo: Utf8PathBuf,
}

impl Sandbox {
    /// Build a sandbox and populate it with the utility allowlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the sandbox cannot be created or populated.
    pub fn new() -> Result<Self> {
        Self::with_env(&DefaultEnv)
    }

    /// Build a sandbox using `env` to resolve the utility allowlist.
    ///
    /// This seam is limited to locating the real tools copied into the
    /// sandbox; child processes still receive only the explicit environment
    /// assembled when commands are constructed.
    ///
    /// # Errors
    ///
    /// Returns an error if the sandbox cannot be created or its utilities cannot be resolved.
    pub fn with_env(env: &impl Env) -> Result<Self> {
        let temp = tempdir().context("create sandbox directory")?;
        let root = Utf8Path::from_path(temp.path())
            .context("sandbox path must be UTF-8")?
            .to_path_buf();
        let repo_path = std::env::current_dir().context("resolve repository root")?;
        let repo = Utf8PathBuf::try_from(repo_path).context("repository root must be UTF-8")?;
        let sandbox = Self {
            _temp: temp,
            root,
            repo,
        };
        fs::create_dir_all(sandbox.bin().as_std_path()).context("create sandbox bin")?;
        fs::create_dir_all(sandbox.home().as_std_path()).context("create sandbox home")?;
        sandbox.link_utilities(env)?;
        Ok(sandbox)
    }

    /// The only directory on the sandbox `PATH`.
    #[must_use]
    pub fn bin(&self) -> Utf8PathBuf {
        self.root.join("bin")
    }

    /// A `HOME` inside the sandbox, isolating the Makefile's `PATH` export.
    #[must_use]
    pub fn home(&self) -> Utf8PathBuf {
        self.root.join("home")
    }

    /// An install prefix that starts out empty; `DEV_FAST_PREFIX` points here.
    #[must_use]
    pub fn prefix(&self) -> Utf8PathBuf {
        self.root.join("prefix")
    }

    fn link_utilities(&self, env: &impl Env) -> Result<()> {
        for utility in SANDBOX_UTILITIES {
            let source = which(env, utility)
                .with_context(|| format!("locate `{utility}` for the sandbox"))?;
            fs::symlink(source.as_std_path(), self.bin().join(utility).as_std_path())
                .with_context(|| format!("link `{utility}` into the sandbox"))?;
        }
        Ok(())
    }

    /// Install a fake executable, creating `dir` if needed.
    ///
    /// `body` is a shell fragment without a shebang; this adds one, so call
    /// sites stay focused on the behaviour they are faking.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake executable cannot be written.
    pub fn write_fake(&self, dir: &Utf8Path, name: &str, body: &str) -> Result<Utf8PathBuf> {
        fs::create_dir_all(dir.as_std_path()).with_context(|| format!("create {dir}"))?;
        // Unlink first. The utility allowlist symlinks real binaries into this
        // directory, and writing to a symlink follows it — faking a utility
        // that is already linked would otherwise truncate the host's copy of
        // it. Only file permissions have stood between that and a broken
        // system.
        let target = dir.join(name);
        match fs::remove_file(target.as_std_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("replace {target}")),
        }
        let script = format!("#!/bin/sh\n{body}\n");
        let path = write_exec_with_content(dir.as_std_path(), name, &script)?;
        Utf8PathBuf::try_from(path).context("fake executable path must be UTF-8")
    }

    /// A `mold` reporting the given version, formatted as the real one does.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake Mold executable cannot be written.
    pub fn write_mold(&self, dir: &Utf8Path, version: &str) -> Result<Utf8PathBuf> {
        self.write_fake(
            dir,
            "mold",
            &format!("echo \"mold {version} (compatible with GNU ld)\""),
        )
    }

    /// A `rustup` reporting the given toolchain and, optionally, the Cranelift
    /// component as installed.
    ///
    /// Every invocation is appended to [`rustup_log`](Self::rustup_log), so a
    /// test can assert which toolchain commands the installer actually issued
    /// rather than inferring them from its diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake rustup executable cannot be written.
    pub fn write_rustup(&self, toolchain: &str, has_cranelift: bool) -> Result<Utf8PathBuf> {
        let component = if has_cranelift {
            "rustc-codegen-cranelift-x86_64-unknown-linux-gnu"
        } else {
            "rustfmt-x86_64-unknown-linux-gnu"
        };
        let body = format!(
            concat!(
                "printf '%s\\n' \"$*\" >> '{log}'\n",
                "case \"$1 $2\" in\n",
                "  'toolchain list') echo '{toolchain}-x86_64-unknown-linux-gnu' ;;\n",
                "  'component list') echo '{component}' ;;\n",
                "  *) exit 0 ;;\n",
                "esac"
            ),
            log = self.rustup_log(),
            toolchain = toolchain,
            component = component,
        );
        self.write_fake(&self.bin(), "rustup", &body)
    }

    /// Where [`write_rustup`](Self::write_rustup) records its invocations.
    #[must_use]
    pub fn rustup_log(&self) -> Utf8PathBuf {
        self.home().join("rustup-invocations.log")
    }

    /// The `rustup` command lines recorded so far, in order.
    ///
    /// A log that was never created means `rustup` was never called. Any other
    /// read failure is propagated rather than reported as "it did not run".
    ///
    /// # Errors
    ///
    /// Returns an error if the rustup invocation log cannot be read.
    pub fn rustup_invocations(&self) -> Result<Vec<String>> {
        let log = self.rustup_log();
        match fs::read_to_string(log.as_std_path()) {
            Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error).with_context(|| format!("read {log}")),
        }
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
    ///
    /// # Errors
    ///
    /// Returns an error if the requested script cannot be executed.
    pub fn script(&self, name: &str, env: &[(&str, String)]) -> Result<Output> {
        self.script_with(name, PinOverrides::Supplied, env)
    }

    /// As [`script`](Self::script), but choosing whether the pin-file
    /// environment variables are supplied at all.
    ///
    /// Omitting them is how a test proves the scripts fall back to the
    /// committed pins rather than depending on the caller to pass every path.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested script cannot be executed.
    pub fn script_with(
        &self,
        name: &str,
        pins: PinOverrides,
        env: &[(&str, String)],
    ) -> Result<Output> {
        let mut command = self.base_command(&self.bin().join("bash"));
        command
            .env("DEV_FAST_PREFIX", self.prefix().as_std_path())
            .env("DEV_FAST_CONFIG", "tools/dev-fast/config.toml");
        if matches!(pins, PinOverrides::Supplied) {
            command
                .env("MOLD_VERSION_FILE", "tools/mold/VERSION")
                .env("MOLD_SHA256SUMS_FILE", "tools/mold/SHA256SUMS")
                .env("RUST_TOOLCHAIN_FILE", "rust-toolchain.toml");
        }
        command.arg(format!("scripts/{name}"));
        for (key, value) in env {
            command.env(key, value);
        }
        command
            .output()
            .with_context(|| format!("run scripts/{name}"))
    }

    /// Run a Make target with the sandbox as the entire environment and no
    /// overrides beyond the sandbox's own install prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if Make cannot be executed for the target.
    pub fn make(&self, target: &str) -> Result<Output> {
        self.run_make(&MakeInvocation::new(target))
    }

    /// Run a described Make invocation with the sandbox as the entire
    /// environment. The sandbox's install prefix is applied first, so an
    /// invocation may still override it.
    ///
    /// # Errors
    ///
    /// Returns an error if Make cannot be executed for the invocation.
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

/// Combined stdout and stderr, for asserting on diagnostics regardless of the
/// stream a given message went to.
#[must_use]
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

/// The committed Cargo fragment's path, relative to the repository root.
pub const DEV_FAST_CONFIG_PATH: &str = "tools/dev-fast/config.toml";

/// The committed Cargo fragment's contents, so a test can assert on what the
/// `dev-*` recipes actually apply rather than only on the path they pass.
///
/// # Errors
///
/// Returns an error if the checked-in dev-fast configuration cannot be read.
pub fn dev_fast_config() -> Result<String> {
    fs::read_to_string(DEV_FAST_CONFIG_PATH).with_context(|| format!("read {DEV_FAST_CONFIG_PATH}"))
}

/// The repository's pinned mold release tag.
///
/// # Errors
///
/// Returns an error if the pinned Mold version cannot be read.
pub fn pinned_mold_version() -> Result<String> {
    read_pin("tools/mold/VERSION")
}

/// The repository's toolchain, read from `rust-toolchain.toml`.
///
/// dev-fast deliberately shares it rather than pinning a second nightly, so the
/// accelerated loop and the gates borrow-check identically under Polonius.
///
/// # Errors
///
/// Returns an error if the pinned Rust toolchain cannot be read.
pub fn pinned_toolchain() -> Result<String> {
    let contents = read_pin("rust-toolchain.toml")?;
    contents
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("channel")?;
            let value = rest.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_owned())
        })
        .context("rust-toolchain.toml should declare a channel")
}
