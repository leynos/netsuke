//! A locally built stand-in for a pinned mold release.
//!
//! The installer downloads `<base>/v<version>/<name>`, verifies the artefact
//! against a checksum file, and unpacks it with the tarball's versioned root
//! stripped. Serving that from a `file://` URL exercises the whole path — URL
//! layout, verification, and strip depth — with no network.
//!
//! Each release owns its version, so no caller threads a version string
//! through: ask the release for what you need instead.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;

use super::Sandbox;
use crate::fs;

/// A published fake release, ready for the installer to fetch.
pub struct FakeRelease {
    version: String,
    directory: Utf8PathBuf,
    name: String,
    sha256: String,
}

impl FakeRelease {
    /// Build a release for `version` and publish it under the `v<version>` path
    /// the installer will request.
    ///
    /// # Errors
    ///
    /// Returns an error if the release fixture cannot be published into the sandbox.
    pub fn publish(sandbox: &Sandbox, version: &str) -> Result<Self> {
        let directory = sandbox.home().join("releases");
        let root = format!("mold-{version}-x86_64-linux");
        let name = format!("{root}.tar.gz");

        stage_release_tree(&directory.join(&root))?;
        let archive = build_archive(sandbox, &directory, &name, &root)?;
        let sha256 = sha256_of(sandbox, &archive)?;
        publish_under_version_path(&directory, &archive, &name, version)?;

        Ok(Self {
            version: version.to_owned(),
            directory,
            name,
            sha256,
        })
    }

    /// The artefact's file name, as it appears in a checksum file.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The artefact's real SHA-256 digest.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// The base URL to point the installer at. It appends `/v<version>/<name>`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("file://{}", self.directory)
    }

    /// Write a version pin naming this release, and return its path.
    ///
    /// # Errors
    ///
    /// Returns an error if the version pin cannot be written.
    pub fn write_version_pin(&self, sandbox: &Sandbox) -> Result<Utf8PathBuf> {
        let path = sandbox.home().join("MOLD_VERSION");
        fs::write(&path, format!("{}\n", self.version)).context("write test version pin")?;
        Ok(path)
    }

    /// Write a checksum file recording `digest` for this artefact, and return
    /// its path. Pass the real [`sha256`](Self::sha256) for the success path, or
    /// any other value to exercise a mismatch.
    ///
    /// # Errors
    ///
    /// Returns an error if the checksum fixture cannot be written.
    pub fn write_checksums(&self, sandbox: &Sandbox, digest: &str) -> Result<Utf8PathBuf> {
        Self::write_checksum_file(sandbox, &format!("{digest}  {}\n", self.name))
    }

    /// Write a checksum file that is well-formed but does not mention this
    /// artefact, exercising the unlisted-artefact refusal.
    ///
    /// # Errors
    ///
    /// Returns an error if the checksum fixture cannot be written.
    pub fn write_checksums_omitting_this_artefact(&self, sandbox: &Sandbox) -> Result<Utf8PathBuf> {
        let other = format!("{}  mold-0.0.0-x86_64-linux.tar.gz\n", self.sha256);
        Self::write_checksum_file(sandbox, &other)
    }

    fn write_checksum_file(sandbox: &Sandbox, contents: &str) -> Result<Utf8PathBuf> {
        let path = sandbox.home().join("SHA256SUMS");
        fs::write(&path, contents).context("write test checksum file")?;
        Ok(path)
    }
}

/// Lay out the tarball's versioned root containing `bin/mold`, so a correct
/// `--strip-components` lands the binary directly in the install prefix.
fn stage_release_tree(root: &Utf8Path) -> Result<()> {
    fs::create_dir_all(root.join("bin")).context("stage fake release tree")?;
    fs::write(root.join("bin/mold"), "#!/bin/sh\necho fake\n").context("write staged mold")
}

fn build_archive(
    sandbox: &Sandbox,
    directory: &Utf8Path,
    name: &str,
    root: &str,
) -> Result<Utf8PathBuf> {
    let status = Command::new(sandbox.bin().join("tar").as_std_path())
        .current_dir(directory.as_std_path())
        .args(["--create", "--gzip", "--file"])
        .arg(name)
        .arg(root)
        .status()
        .context("create fake release tarball")?;
    ensure!(status.success(), "tar should build the fake release");
    Ok(directory.join(name))
}

fn sha256_of(sandbox: &Sandbox, path: &Utf8Path) -> Result<String> {
    let output = Command::new(sandbox.bin().join("sha256sum").as_std_path())
        .arg(path.as_std_path())
        .output()
        .context("hash fake release tarball")?;
    ensure!(output.status.success(), "sha256sum should succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .split_whitespace()
        .next()
        .context("parse sha256sum output")?
        .to_owned())
}

fn publish_under_version_path(
    directory: &Utf8Path,
    archive: &Utf8Path,
    name: &str,
    version: &str,
) -> Result<()> {
    let versioned = directory.join(format!("v{version}"));
    fs::create_dir_all(&versioned).context("create versioned release path")?;
    fs::copy(archive, versioned.join(name))
        .context("publish fake release under its version path")?;
    Ok(())
}
