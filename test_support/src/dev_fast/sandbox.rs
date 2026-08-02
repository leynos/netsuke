//! A hermetic `PATH` and `HOME` for exercising the `dev-fast` targets.
//!
//! See the parent module for why the sandbox is built from nothing rather
//! than by prepending fakes to the ambient `PATH`.
use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

use super::MakeInvocation;
use crate::exec::write_exec_with_content;

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
    "stat",
    "tar",
    "touch",
    "tr",
    "uname",
];

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
    pub fn new() -> Result<Self> {
        let temp = tempdir().context("create sandbox directory")?;
        let root = Utf8Path::from_path(temp.path())
            .context("sandbox path must be UTF-8")?
            .to_path_buf();
        let repo = std::env::current_dir().context("resolve repository root")?;
        let repo = Utf8PathBuf::try_from(repo).context("repository root must be UTF-8")?;
        let sandbox = Self {
            _temp: temp,
            root,
            repo,
        };
        fs::create_dir_all(sandbox.bin().as_std_path()).context("create sandbox bin")?;
        fs::create_dir_all(sandbox.home().as_std_path()).context("create sandbox home")?;
        sandbox.link_utilities()?;
        Ok(sandbox)
    }

    /// The only directory on the sandbox `PATH`.
    pub fn bin(&self) -> Utf8PathBuf {
        self.root.join("bin")
    }

    /// A `HOME` inside the sandbox, isolating the Makefile's `PATH` export.
    pub fn home(&self) -> Utf8PathBuf {
        self.root.join("home")
    }

    /// An install prefix that starts out empty; `DEV_FAST_PREFIX` points here.
    pub fn prefix(&self) -> Utf8PathBuf {
        self.root.join("prefix")
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
    pub fn rustup_log(&self) -> Utf8PathBuf {
        self.home().join("rustup-invocations.log")
    }

    /// The `rustup` command lines recorded so far, in order.
    ///
    /// A log that was never created means `rustup` was never called. Any other
    /// read failure is propagated rather than reported as "it did not run".
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
    pub fn script(&self, name: &str, env: &[(&str, String)]) -> Result<Output> {
        self.script_with(name, PinOverrides::Supplied, env)
    }

    /// As [`script`](Self::script), but choosing whether the pin-file
    /// environment variables are supplied at all.
    ///
    /// Omitting them is how a test proves the scripts fall back to the
    /// committed pins rather than depending on the caller to pass every path.
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

/// The committed Cargo fragment's path, relative to the repository root.
pub const DEV_FAST_CONFIG_PATH: &str = "tools/dev-fast/config.toml";

/// The committed Cargo fragment's contents, so a test can assert on what the
/// `dev-*` recipes actually apply rather than only on the path they pass.
pub fn dev_fast_config() -> Result<String> {
    fs::read_to_string(DEV_FAST_CONFIG_PATH).with_context(|| format!("read {DEV_FAST_CONFIG_PATH}"))
}

/// The repository's pinned mold release tag.
pub fn pinned_mold_version() -> Result<String> {
    read_pin("tools/mold/VERSION")
}

/// The repository's toolchain, read from `rust-toolchain.toml`.
///
/// dev-fast deliberately shares it rather than pinning a second nightly, so the
/// accelerated loop and the gates borrow-check identically under Polonius.
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
