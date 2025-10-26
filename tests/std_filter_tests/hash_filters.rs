//! Tests for hash and digest filters in the standard library.
//!
//! Validates SHA-256, SHA-512, and optionally SHA-1 and MD5 (under the
//! `legacy-digests` feature) for both the `hash` and `digest` filters.
use anyhow::{bail, ensure, Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, ErrorKind};
use rstest::rstest;

use super::support::fallible;

#[rstest]
#[case(
    "sha256",
    "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7",
    "3a6eb079"
)]
#[case(
    "sha512",
    "77c7ce9a5d86bb386d443bb96390faa120633158699c8844c30b13ab0bf92760b7e4416aea397db91b4ac0e5dd56b8ef7e4b066162ab1fdc088319ce6defc876",
    "77c7ce9a"
)]
#[case(
    "sha256-empty",
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "e3b0c442"
)]
#[case(
    "sha512-empty",
    "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
    "cf83e135"
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("sha1", "a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd", "a17c9aaa",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("sha1-empty", "da39a3ee5e6b4b0d3255bfef95601890afd80709", "da39a3ee",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("md5", "8d777f385d3dfec8815d20f7496026dc", "8d777f38",)
)]
#[cfg_attr(
    feature = "legacy-digests",
    case("md5-empty", "d41d8cd98f00b204e9800998ecf8427e", "d41d8cd9",)
)]
fn hash_and_digest_filters(
    #[case] alg: &str,
    #[case] expected_hash: &str,
    #[case] expected_digest: &str,

) -> Result<()> {
    let ( _temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open workspace root for hashing tests")?;

    let (file, algorithm) = if let Some(stripped) = alg.strip_suffix("-empty") {
        let relative = format!("{stripped}_empty");
        dir.write(relative.as_str(), b"")
            .with_context(|| format!("create empty fixture {relative}"))?;
        (root.join(relative.as_str()), stripped)
    } else {
        (root.join("file"), alg)
    };

    let hash_template_name = format!("hash_{alg}");
    let hash_template = format!("{{{{ path | hash('{algorithm}') }}}}");
    fallible::register_template(&mut env, hash_template_name.as_str(), hash_template)?;
    let hash_result = env
        .get_template(hash_template_name.as_str())
        .with_context(|| format!("fetch template '{hash_template_name}'"))?
        .render(context!(path => file.as_str()))
        .context("render hash template")?;
    ensure!(
        hash_result == expected_hash,
        "expected hash {expected_hash} but rendered {hash_result}"
    );

    let digest_template_name = format!("digest_{alg}");
    let digest_template = format!("{{{{ path | digest(8, '{algorithm}') }}}}");
    fallible::register_template(&mut env, digest_template_name.as_str(), digest_template)?;
    let digest_result = env
        .get_template(digest_template_name.as_str())
        .with_context(|| format!("fetch template '{digest_template_name}'"))?
        .render(context!(path => file.as_str()))
        .context("render digest template")?;
    ensure!(
        digest_result == expected_digest,
        "expected digest {expected_digest} but rendered {digest_result}"
    );
    Ok(())
}

#[cfg(not(feature = "legacy-digests"))]
#[rstest]
fn hash_filter_legacy_algorithms_disabled() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;

    fallible::register_template(&mut env, "hash_sha1", "{{ path | hash('sha1') }}")?;
    let template = env
        .get_template("hash_sha1")
        .context("fetch template 'hash_sha1'")?;
    let result = template.render(context!(path => root.join("file").as_str()));
    let err = match result {
        Ok(output) => bail!(
            "expected hash to require legacy digests for sha1 but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "hash should report InvalidOperation without legacy digests but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("enable feature 'legacy-digests'"),
        "error should mention legacy feature: {err}"
    );
    Ok(())
}

#[rstest]
fn hash_filter_rejects_unknown_algorithm() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    let file = root.join("file");

    fallible::register_template(&mut env, "hash_unknown", "{{ path | hash('whirlpool') }}")?;
    let hash_template = env
        .get_template("hash_unknown")
        .context("fetch template 'hash_unknown'")?;
    let hash_result = hash_template.render(context!(path => file.as_str()));
    let hash_err = match hash_result {
        Ok(output) => bail!(
            "expected hash to reject unsupported algorithm but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        hash_err.kind() == ErrorKind::InvalidOperation,
        "hash should report InvalidOperation for unsupported algorithms but was {:?}",
        hash_err.kind()
    );
    ensure!(
        hash_err
            .to_string()
            .contains("unsupported hash algorithm 'whirlpool'"),
        "error should mention unsupported algorithm: {hash_err}"
    );

    fallible::register_template(
        &mut env,
        "digest_unknown",
        "{{ path | digest(8, 'whirlpool') }}",
    )?;
    let digest_template = env
        .get_template("digest_unknown")
        .context("fetch template 'digest_unknown'")?;
    let digest_result = digest_template.render(context!(path => file.as_str()));
    let digest_err = match digest_result {
        Ok(output) => bail!(
            "expected digest to reject unsupported algorithms but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        digest_err.kind() == ErrorKind::InvalidOperation,
        "digest should report InvalidOperation for unsupported algorithms but was {:?}",
        digest_err.kind()
    );
    ensure!(
        digest_err
            .to_string()
            .contains("unsupported hash algorithm 'whirlpool'"),
        "error should mention unsupported algorithm: {digest_err}"
    );
    Ok(())
}
