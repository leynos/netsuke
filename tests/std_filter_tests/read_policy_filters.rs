//! File-reading policy tests for the bounded `contents`, `linecount`, `hash`,
//! and `digest` filters.
//!
//! Covers the byte budget (exact boundary, one byte over, per-call narrowing
//! that can only clamp), the default no-follow regular-file open (symlinks and
//! FIFOs rejected), and the `follow_symlinks` opt-in.
use anyhow::{Context, Result, bail, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{ErrorKind, context};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

#[cfg(unix)]
use rustix::fs::{Dev, FileType as RxFileType, Mode, mknodat};

use super::support::fallible;

/// Inputs for one bounded-read render against a policy workspace.
#[derive(Clone, Copy)]
struct PolicyRender<'a> {
    /// File-read budget configured on the stdlib environment.
    limit: u64,
    /// Template registration name.
    name: &'a str,
    /// Template source exercising a file-reading filter.
    template: &'a str,
    /// Workspace root the stdlib environment is bound to.
    root: &'a camino::Utf8Path,
    /// Path the template reads.
    path: &'a camino::Utf8Path,
}

/// Render a bounded-read template, returning the raw result for assertions.
fn render_with_file_read_limit(
    render: PolicyRender<'_>,
) -> Result<std::result::Result<String, minijinja::Error>> {
    let mut env = fallible::stdlib_env_with_root_and_file_read_limit(render.root, render.limit)?;
    fallible::register_template(&mut env, render.name, render.template)?;
    let registered = env
        .get_template(render.name)
        .context("fetch policy template")?;
    Ok(registered.render(context!(path => render.path.as_str())))
}

#[rstest]
fn contents_within_limit_renders_unchanged() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = root.join("file");
    let rendered = render_with_file_read_limit(PolicyRender {
        limit: 1024,
        name: "contents_within",
        template: "{{ path | contents }}",
        root: &root,
        path: &file,
    })?
    .context("render within limit")?;
    ensure!(
        rendered == "data",
        "expected contents 'data' within the limit but rendered {rendered}"
    );
    Ok(())
}

#[rstest]
fn contents_exactly_at_the_limit_renders() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    Dir::open_ambient_dir(&root, ambient_authority())?
        .write("exact.bin", b"12345")
        .context("write exact-limit fixture")?;
    let file = root.join("exact.bin");
    let rendered = render_with_file_read_limit(PolicyRender {
        limit: 5,
        name: "contents_exact",
        template: "{{ path | contents }}",
        root: &root,
        path: &file,
    })?
    .context("render at the limit")?;
    ensure!(
        rendered == "12345",
        "expected a file exactly at the limit to render but got {rendered}"
    );
    Ok(())
}

#[rstest]
fn contents_one_byte_over_the_limit_fails_with_the_limit() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    Dir::open_ambient_dir(&root, ambient_authority())?
        .write("over.bin", b"123456")
        .context("write over-limit fixture")?;
    let file = root.join("over.bin");
    let result = render_with_file_read_limit(PolicyRender {
        limit: 5,
        name: "contents_over",
        template: "{{ path | contents }}",
        root: &root,
        path: &file,
    })?;
    let err = match result {
        Ok(output) => bail!("expected an over-limit read to fail but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "over-limit reads should report InvalidOperation but was {:?}",
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains('5'),
        "error should interpolate the limit: {message}"
    );
    ensure!(
        !message.contains("123456"),
        "error must not disclose file contents: {message}"
    );
    Ok(())
}

#[rstest]
fn linecount_enforces_the_budget_incrementally() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let over = render_with_file_read_limit(PolicyRender {
        limit: 8,
        name: "linecount_over",
        template: "{{ path | linecount }}",
        root: &root,
        path: &root.join("lines.txt"),
    })?;
    ensure!(
        over.is_err(),
        "a 14-byte file must fail an 8-byte budget: {over:?}"
    );
    let message = normalize_fluent_isolates(&over.expect_err("over budget").to_string());
    ensure!(message.contains('8'), "limit should be quoted: {message}");
    Ok(())
}

#[rstest]
fn hash_and_digest_enforce_the_budget() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let over = render_with_file_read_limit(PolicyRender {
        limit: 2,
        name: "hash_over",
        template: "{{ path | hash('sha256') }}",
        root: &root,
        path: &root.join("file"),
    })?;
    ensure!(
        over.is_err(),
        "hashing a 4-byte file must fail a 2-byte budget"
    );
    let message = normalize_fluent_isolates(&over.expect_err("over budget").to_string());
    ensure!(message.contains('2'), "limit should be quoted: {message}");

    let within = render_with_file_read_limit(PolicyRender {
        limit: 4,
        name: "digest_within",
        template: "{{ path | digest(8, 'sha256') }}",
        root: &root,
        path: &root.join("file"),
    })?
    .context("digest within the budget")?;
    ensure!(
        within == "3a6eb079",
        "expected the known digest prefix but rendered {within}"
    );
    Ok(())
}

#[rstest]
fn reading_filters_reject_symlinks_by_default() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    if !root.join("link").exists() {
        return Ok(()); // Non-Unix workspaces ship a plain copy instead.
    }
    let link = root.join("link");
    for (name, template) in [
        ("contents_symlink", "{{ path | contents }}"),
        ("linecount_symlink", "{{ path | linecount }}"),
        ("hash_symlink", "{{ path | hash }}"),
    ] {
        let result = render_with_file_read_limit(PolicyRender {
            limit: 1024,
            name,
            template,
            root: &root,
            path: &link,
        })?;
        let err = match result {
            Ok(output) => bail!("expected {name} to reject a symlink but rendered {output}"),
            Err(err) => err,
        };
        ensure!(
            err.kind() == ErrorKind::InvalidOperation,
            "{name} should report InvalidOperation for a symlink but was {:?}",
            err.kind()
        );
        let message = normalize_fluent_isolates(&err.to_string());
        // O_NOFOLLOW surfaces either as our not-regular-file diagnostic or,
        // through the platform, as an ELOOP-style open failure; both reject
        // the link without reading it.
        ensure!(
            message.contains("not a regular file") || message.contains("symbolic links"),
            "{name} error should explain the rejection: {message}"
        );
    }
    Ok(())
}

#[rstest]
fn follow_symlinks_opt_in_reads_the_link_target() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    if !root.join("link").exists() {
        return Ok(());
    }
    let rendered = render_with_file_read_limit(PolicyRender {
        limit: 1024,
        name: "contents_follow",
        template: "{{ path | contents(follow_symlinks=true) }}",
        root: &root,
        path: &root.join("link"),
    })?
    .context("render with follow_symlinks")?;
    ensure!(
        rendered == "data",
        "expected the opt-in to follow the link but rendered {rendered}"
    );
    Ok(())
}

#[rstest]
fn per_call_max_bytes_narrows_the_budget() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let result = render_with_file_read_limit(PolicyRender {
        limit: 1024,
        name: "contents_narrow",
        template: "{{ path | contents(max_bytes=2) }}",
        root: &root,
        path: &root.join("file"),
    })?;
    ensure!(
        result.is_err(),
        "a per-call budget below the file size must fail: {result:?}"
    );
    let message = normalize_fluent_isolates(&result.expect_err("narrowed").to_string());
    ensure!(
        message.contains('2'),
        "narrowed limit should be quoted: {message}"
    );
    Ok(())
}

#[rstest]
fn per_call_max_bytes_cannot_raise_the_budget() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    Dir::open_ambient_dir(&root, ambient_authority())?
        .write("big.bin", [b'x'; 16])
        .context("write 16-byte fixture")?;
    let result = render_with_file_read_limit(PolicyRender {
        limit: 4,
        name: "contents_raise",
        template: "{{ path | contents(max_bytes=4096) }}",
        root: &root,
        path: &root.join("big.bin"),
    })?;
    ensure!(
        result.is_err(),
        "a per-call budget above the configured ceiling must be clamped"
    );
    let message = normalize_fluent_isolates(&result.expect_err("clamped").to_string());
    ensure!(
        message.contains('4'),
        "the configured ceiling should still apply: {message}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn reading_filters_reject_a_fifo() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
    mknodat(
        &dir,
        "pipe",
        RxFileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .map_err(|err| anyhow::anyhow!("create fifo fixture: {err}"))?;
    drop(dir);
    let pipe = root.join("pipe");
    let result = render_with_file_read_limit(PolicyRender {
        limit: 1024,
        name: "contents_fifo",
        template: "{{ path | contents }}",
        root: &root,
        path: &pipe,
    })?;
    let err = match result {
        Ok(output) => bail!("expected contents to reject a FIFO but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "contents should report InvalidOperation for a FIFO but was {:?}",
        err.kind()
    );
    Ok(())
}
