//! Exercises standard library I/O filters to ensure they render file contents,
//! line counts, and error paths correctly in end-to-end scenarios.
use anyhow::{bail, ensure, Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{context, ErrorKind};
use rstest::rstest;

use super::support::fallible;

#[rstest]
fn contents_and_linecount_filters() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    let file = root.join("file");
    let text = fallible::render(&mut env, "contents", "{{ path | contents }}", &file)
        .context("render contents filter")?;
    ensure!(text == "data", "expected file contents 'data' but rendered {text}");
    let lines_output = fallible::render(
        &mut env,
        "linecount",
        "{{ path | linecount }}",
        &root.join("lines.txt"),
    )
    .context("render linecount filter")?;
    let linecount: usize = lines_output
        .parse()
        .context("parse linecount result")?;
    ensure!(linecount == 3, "expected 3 lines but counted {linecount}");

    Dir::open_ambient_dir(&root, ambient_authority())
        .context("open workspace root for empty file")?
        .write("empty.txt", b"")
        .context("write empty file")?;
    let empty_file = root.join("empty.txt");
    let empty_lines = fallible::render(
        &mut env,
        "empty_linecount",
        "{{ path | linecount }}",
        &empty_file,
    )
    .context("render linecount for empty file")?;
    let empty_count: usize = empty_lines
        .parse()
        .context("parse empty linecount result")?;
    ensure!(empty_count == 0, "expected zero lines but counted {empty_count}");
    Ok(())
}

#[rstest]
fn contents_filter_unsupported_encoding() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    fallible::register_template(
        &mut env,
        "contents_bad_encoding",
        "{{ path | contents('latin-1') }}",
    )?;
    let template = env
        .get_template("contents_bad_encoding")
        .context("fetch template 'contents_bad_encoding'")?;
    let file = root.join("file");
    let err = match template.render(context!(path => file.as_str())) {
        Ok(output) => bail!(
            "expected contents to reject unsupported encoding but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "contents should report InvalidOperation for unsupported encoding but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("unsupported encoding"),
        "error should mention unsupported encoding"
    );
    Ok(())
}

#[rstest]
fn size_filter() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    let file = root.join("file");
    let size = fallible::render(&mut env, "size", "{{ path | size }}", &file)
        .context("render size filter")?;
    let bytes: u64 = size.parse().context("parse size output")?;
    ensure!(bytes == 4, "expected size 4 bytes but measured {bytes}");
    Ok(())
}

#[rstest]
fn size_filter_missing_file() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let mut env = fallible::stdlib_env()?;
    fallible::register_template(&mut env, "size_missing", "{{ path | size }}")?;
    let template = env
        .get_template("size_missing")
        .context("fetch template 'size_missing'")?;
    let missing = root.join("does_not_exist");
    let err = match template.render(context!(path => missing.as_str())) {
        Ok(output) => bail!(
            "expected size to error for missing file but rendered {output}"
        ),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "size should report InvalidOperation for missing files but was {:?}",
        err.kind()
    );
    let message = err.to_string();
    ensure!(
        message.contains("does_not_exist") || message.contains("not found"),
        "error should mention missing file: {message}"
    );
    Ok(())
}
