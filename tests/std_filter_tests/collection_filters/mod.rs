//! Behavioural coverage for the `MiniJinja` collection filters exposed by the
//! Netsuke stdlib.
//!
//! These tests exercise the filters end-to-end through a configured template
//! environment to ensure we keep parity between unit expectations and rendered
//! output, especially across error handling scenarios.
//!
//! This module owns the shared environment import and the `uniq` cases.
//! `flatten_tests`, `compact_tests`, and `group_by_tests` hold the remaining
//! behaviour, and `compact_property` holds the `OBL-COMPACT` property, so no
//! single file carries the whole surface.

use anyhow::{Context, Result, bail, ensure};
use minijinja::{ErrorKind, context};
use rstest::rstest;

pub(super) use super::support::fallible;

#[rstest]
fn uniq_removes_duplicate_strings() -> Result<()> {
    let mut env = fallible::stdlib_env()?;
    fallible::register_template(&mut env, "uniq", "{{ values | uniq | join(',') }}")?;
    let template = env.get_template("uniq").context("fetch template 'uniq'")?;
    let output = template
        .render(context!(values => vec!["a", "a", "b", "b", "c"]))
        .context("render template 'uniq'")?;
    ensure!(
        output == "a,b,c",
        "uniq should collapse duplicates, but rendered {output}"
    );
    Ok(())
}

#[rstest]
fn uniq_rejects_non_iterables() -> Result<()> {
    let env = fallible::stdlib_env()?;
    let err = match env.render_str("{{ value | uniq }}", context!(value => 1)) {
        Ok(output) => bail!("expected uniq to reject scalars but rendered {output}"),
        Err(err) => err,
    };
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "uniq should report InvalidOperation, but was {:?}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("is not iterable"),
        "error should mention non-iterable input: {err}"
    );
    Ok(())
}

mod compact_property;

mod compact_tests;

mod flatten_tests;

mod group_by_tests;
