//! Contract tests for the project website URL across publishable surfaces.
//!
//! The README and crate metadata must point at the same website, and the
//! documentation link must remain a superset of the homepage URL so the two
//! cannot drift apart. The tests assert consistency between the files rather
//! than pinning any particular URL.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use toml::Value as TomlValue;

/// Read a repository file relative to the crate root.
fn repo_file(name: &str) -> Result<String> {
    let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .with_context(|| format!("open repository root {root}"))?;
    dir.read_to_string(name)
        .with_context(|| format!("read {name}"))
}

/// The website URL declared in the README's `Website:` line.
fn readme_website_url() -> Result<String> {
    let readme = repo_file("README.md")?;
    let line = readme
        .lines()
        .find(|line| line.starts_with("Website: <"))
        .context("README should carry a `Website: <url>` line")?;
    let url = line.trim_start_matches("Website: <").trim_end_matches('>');
    ensure!(!url.is_empty(), "README website URL should not be empty");
    Ok(url.to_owned())
}

/// The value of a `[package]` field in Cargo.toml.
fn cargo_package_field(field: &str) -> Result<String> {
    let manifest: TomlValue = repo_file("Cargo.toml")?
        .parse()
        .context("parse Cargo.toml")?;
    manifest
        .get("package")
        .and_then(|package| package.get(field))
        .and_then(TomlValue::as_str)
        .map(ToOwned::to_owned)
        .with_context(|| format!("Cargo.toml `package.{field}` should be a string"))
}

#[test]
fn website_url_is_consistent_across_publishable_surfaces() -> Result<()> {
    let website = readme_website_url()?;
    let cargo_homepage = cargo_package_field("homepage")?;
    ensure!(
        cargo_homepage == website,
        "Cargo.toml homepage {cargo_homepage} should match the README website {website}"
    );
    Ok(())
}

#[test]
fn documentation_url_is_a_superset_of_the_homepage() -> Result<()> {
    let homepage = cargo_package_field("homepage")?;
    let documentation = cargo_package_field("documentation")?;
    ensure!(
        documentation.starts_with(&homepage),
        "documentation {documentation} should be a superset of the homepage {homepage}"
    );
    Ok(())
}
