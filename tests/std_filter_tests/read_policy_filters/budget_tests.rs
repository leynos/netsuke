//! Byte-budget tests for the file-reading filters.
//!
//! Each case is driven through the shared filter table, so the exact
//! boundary, the one-byte-over rejection, the per-call narrowing that can
//! only clamp, and the UTF-8 requirement for counted text are asserted for
//! every entry point rather than for whichever filter a test happened to pick.
use anyhow::{Context, Result, ensure};
use minijinja::ErrorKind;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

use super::fallible;
use super::{
    CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, PolicyRender, ReadTarget, rejection,
    render_case, render_with_file_read_limit, write_fixture,
};

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
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn filters_render_a_file_exactly_at_the_limit(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = write_fixture(&root, "exact.bin", b"12345")?;
    let rendered = render_case(
        case,
        "exact",
        ReadTarget::new(&root, &file, 5),
        &case.template(),
    )?
    .with_context(|| format!("{}: render at the limit", case.name))?;
    ensure!(
        rendered == case.exact,
        "{}: expected {} for a file exactly at the limit but rendered {rendered}",
        case.name,
        case.exact
    );
    Ok(())
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn filters_reject_a_file_one_byte_over_the_limit(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = write_fixture(&root, "over.bin", b"123456")?;
    let err = rejection(
        case,
        render_case(
            case,
            "over",
            ReadTarget::new(&root, &file, 5),
            &case.template(),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: over-limit reads should report InvalidOperation but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("5 bytes"),
        "{}: error should interpolate the limit: {message}",
        case.name
    );
    ensure!(
        !message.contains("123456"),
        "{}: error must not disclose file contents: {message}",
        case.name
    );
    Ok(())
}

#[rstest]
fn linecount_enforces_the_budget_incrementally() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let over = render_case(
        LINECOUNT,
        "linecount_over",
        ReadTarget::new(&root, &root.join("lines.txt"), 8),
        &LINECOUNT.template(),
    )?;
    ensure!(
        over.is_err(),
        "a 14-byte file must fail an 8-byte budget: {over:?}"
    );
    let message = normalize_fluent_isolates(&over.expect_err("over budget").to_string());
    ensure!(
        message.contains("8 bytes"),
        "limit should be quoted: {message}"
    );
    Ok(())
}

/// An unterminated line longer than the budget must fail, not be counted.
///
/// The line-counting loop reads fixed-size chunks, so a file with no
/// terminator is rejected by the running total rather than by materializing
/// the line first. This pins the diagnostic for that path.
#[rstest]
fn linecount_rejects_a_line_longer_than_the_budget() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = write_fixture(&root, "long-line.bin", &[b'x'; 32])?;
    let err = rejection(
        LINECOUNT,
        render_case(
            LINECOUNT,
            "long_line",
            ReadTarget::new(&root, &file, 8),
            &LINECOUNT.template(),
        )?,
    )?;
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("8 bytes"),
        "limit should be quoted: {message}"
    );
    Ok(())
}

/// Counted text must be UTF-8: binary input is rejected, not counted.
#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
fn text_filters_reject_invalid_utf8(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = write_fixture(&root, "binary.bin", b"one\n\xff\xfe\ntwo\n")?;
    let err = rejection(
        case,
        render_case(
            case,
            "utf8",
            ReadTarget::new(&root, &file, 1024),
            &case.template(),
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "{}: undecodable bytes should report InvalidOperation but was {:?}",
        case.name,
        err.kind()
    );
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("invalid"),
        "{}: error should report invalid data: {message}",
        case.name
    );
    Ok(())
}

#[rstest]
fn hash_and_digest_enforce_the_budget() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let over = render_case(
        HASH,
        "hash_over",
        ReadTarget::new(&root, &root.join("file"), 2),
        &HASH.template(),
    )?;
    ensure!(
        over.is_err(),
        "hashing a 4-byte file must fail a 2-byte budget"
    );
    let message = normalize_fluent_isolates(&over.expect_err("over budget").to_string());
    ensure!(
        message.contains("2 bytes"),
        "limit should be quoted: {message}"
    );

    let within = render_case(
        DIGEST,
        "digest_within",
        ReadTarget::new(&root, &root.join("file"), 4),
        &DIGEST.template(),
    )?
    .context("digest within the budget")?;
    ensure!(
        within == "3a6eb079",
        "expected the known digest prefix but rendered {within}"
    );
    Ok(())
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn per_call_max_bytes_narrows_the_budget(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let err = rejection(
        case,
        render_case(
            case,
            "narrow",
            ReadTarget::new(&root, &root.join("file"), 1024),
            &case.template_with("max_bytes=2"),
        )?,
    )?;
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("2 bytes"),
        "{}: narrowed limit should be quoted: {message}",
        case.name
    );
    Ok(())
}

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn per_call_max_bytes_cannot_raise_the_budget(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = write_fixture(&root, "big.bin", &[b'x'; 16])?;
    let err = rejection(
        case,
        render_case(
            case,
            "raise",
            ReadTarget::new(&root, &file, 4),
            &case.template_with("max_bytes=4096"),
        )?,
    )?;
    let message = normalize_fluent_isolates(&err.to_string());
    ensure!(
        message.contains("4 bytes"),
        "{}: the configured ceiling should still apply: {message}",
        case.name
    );
    Ok(())
}
