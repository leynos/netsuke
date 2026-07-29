//! Behavioural tests for the `dev-fast` installer and benchmark scripts.
//!
//! The installer's security-relevant behaviour is that it refuses to unpack an
//! artefact it cannot verify, so these tests serve a locally built tarball over
//! a `file://` URL and vary only the recorded checksum. No network is used.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::fs;
use std::process::Command;
use test_support::dev_fast::{Sandbox, combined, pinned_mold_version, pinned_toolchain};

/// A stand-in mold release laid out like the real tarball: a versioned root
/// directory the installer strips, containing `bin/mold`.
struct FakeRelease {
    dir: Utf8PathBuf,
    archive: Utf8PathBuf,
    name: String,
    sha256: String,
}

fn build_fake_release(sandbox: &Sandbox, version: &str) -> Result<FakeRelease> {
    let dir = sandbox.home().join("releases");
    let root = format!("mold-{version}-x86_64-linux");
    let staging = dir.join(&root);
    fs::create_dir_all(staging.join("bin").as_std_path()).context("stage fake release tree")?;
    fs::write(
        staging.join("bin/mold").as_std_path(),
        "#!/bin/sh\necho fake\n",
    )
    .context("write staged mold")?;

    let name = format!("{root}.tar.gz");
    let archive = dir.join(&name);
    let status = Command::new(sandbox.bin().join("tar").as_std_path())
        .current_dir(dir.as_std_path())
        .args(["--create", "--gzip", "--file"])
        .arg(&name)
        .arg(&root)
        .status()
        .context("create fake release tarball")?;
    ensure!(status.success(), "tar should build the fake release");

    let sha256 = sha256_of(sandbox, &archive)?;
    Ok(FakeRelease {
        dir,
        archive,
        name,
        sha256,
    })
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

/// Point the installer at a local release directory, with a caller-chosen
/// checksum recorded for the artefact.
fn install_env(
    sandbox: &Sandbox,
    release: &FakeRelease,
    version: &str,
    recorded: &str,
) -> Result<Vec<(&'static str, String)>> {
    let version_file = sandbox.home().join("MOLD_VERSION");
    let sums_file = sandbox.home().join("SHA256SUMS");
    fs::write(version_file.as_std_path(), format!("{version}\n"))
        .context("write test version pin")?;
    fs::write(
        sums_file.as_std_path(),
        format!("{recorded}  {}\n", release.name),
    )
    .context("write test checksum file")?;

    Ok(vec![
        ("MOLD_VERSION_FILE", version_file.to_string()),
        ("MOLD_SHA256SUMS_FILE", sums_file.to_string()),
        (
            "MOLD_RELEASE_BASE_URL",
            // The installer appends `/v<version>/<name>`, so expose the release
            // directory under a matching `v<version>` path.
            format!("file://{}", release.dir),
        ),
    ])
}

/// The installer builds `<base>/v<version>/<name>`; mirror the artefact there.
fn publish_under_version_path(release: &FakeRelease, version: &str) -> Result<()> {
    let versioned = release.dir.join(format!("v{version}"));
    fs::create_dir_all(versioned.as_std_path()).context("create versioned release path")?;
    fs::copy(
        release.archive.as_std_path(),
        versioned.join(&release.name).as_std_path(),
    )
    .context("publish fake release under its version path")?;
    Ok(())
}

fn prepared_installer(version: &str) -> Result<(Sandbox, FakeRelease)> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let release = build_fake_release(&sandbox, version)?;
    publish_under_version_path(&release, version)?;
    Ok((sandbox, release))
}

#[test]
fn installs_and_records_the_verification_when_the_checksum_matches() -> Result<()> {
    let version = "9.9.9";
    let (sandbox, release) = prepared_installer(version)?;
    let env = install_env(&sandbox, &release, version, &release.sha256)?;

    let output = sandbox.script("install-dev-fast.sh", &env)?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "install should succeed, got `{text}`"
    );
    ensure!(
        text.contains(&format!("verified {}", release.name)),
        "should report the verification, got `{text}`"
    );
    ensure!(
        sandbox.prefix().join("bin/mold").as_std_path().is_file(),
        "the tarball root should be stripped so bin/mold lands in the prefix"
    );
    ensure!(
        text.contains("rustc-codegen-cranelift-preview"),
        "should install the Cranelift component, got `{text}`"
    );
    Ok(())
}

/// Refusing an unverifiable artefact is the point of the checksum file, so both
/// a mismatch and an unlisted name must abort before anything is unpacked.
#[rstest]
#[case::mismatch(
    "0000000000000000000000000000000000000000000000000000000000000000",
    "checksum mismatch"
)]
#[case::unlisted("", "no checksum recorded")]
fn refuses_to_install_an_unverifiable_artefact(
    #[case] recorded: &str,
    #[case] expected: &str,
) -> Result<()> {
    let version = "9.9.9";
    let (sandbox, release) = prepared_installer(version)?;
    let mut env = install_env(&sandbox, &release, version, recorded)?;
    if recorded.is_empty() {
        // Record a checksum for a different artefact, so the file is valid but
        // this release is absent from it.
        let sums_file = sandbox.home().join("SHA256SUMS");
        fs::write(
            sums_file.as_std_path(),
            format!("{}  mold-0.0.0-x86_64-linux.tar.gz\n", release.sha256),
        )
        .context("write checksum file without this artefact")?;
        env.retain(|(key, _)| *key != "MOLD_SHA256SUMS_FILE");
        env.push(("MOLD_SHA256SUMS_FILE", sums_file.to_string()));
    }

    let output = sandbox.script("install-dev-fast.sh", &env)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "install should abort, got `{text}`"
    );
    ensure!(
        text.contains(expected),
        "should explain the refusal (`{expected}`), got `{text}`"
    );
    ensure!(
        !sandbox.prefix().join("bin/mold").as_std_path().exists(),
        "nothing should be unpacked when verification fails"
    );
    Ok(())
}

/// A cell holding a one-decimal duration, as `bench-build` formats them.
/// Timings are inherently unstable, so tests assert on shape, not value.
fn is_timing(cell: &str) -> bool {
    !cell.is_empty() && cell.contains('.') && cell.chars().all(|c| c.is_ascii_digit() || c == '.')
}

#[test]
fn benchmark_emits_a_markdown_table_for_both_paths() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let cargo = sandbox.write_fake(&sandbox.bin(), "cargo", "exit 0")?;
    let touch_file = sandbox.home().join("bench-touch");
    fs::write(touch_file.as_std_path(), "").context("create bench touch target")?;

    let output = sandbox.script(
        "bench-build.sh",
        &[
            ("CARGO", cargo.to_string()),
            ("BENCH_ROOT", sandbox.home().join("bench").to_string()),
            ("BENCH_TOUCH_FILE", touch_file.to_string()),
        ],
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "benchmark should succeed, got `{}`",
        combined(&output)
    );
    ensure!(
        stdout.contains("| Variant | Clean build (s) | Incremental build (s) |"),
        "should emit the table header, got `{stdout}`"
    );
    // Timings are inherently unstable, so assert on shape rather than values.
    let rows: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("| Default") || line.starts_with("| dev-fast"))
        .collect();
    ensure!(
        rows.len() == 2,
        "should report one row per variant, got `{stdout}`"
    );
    for row in rows {
        let measurements = row.split('|').map(str::trim).filter(|cell| is_timing(cell));
        ensure!(
            measurements.count() == 2,
            "row should carry two decimal timings, got `{row}`"
        );
    }
    Ok(())
}
