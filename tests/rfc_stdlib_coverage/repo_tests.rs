//! Exercise the capability-scoped document adapter and shared name readers.

use super::*;
use anyhow::ensure;
use rstest::rstest;

#[rstest]
#[case(" `name` ", "name")]
#[case("name", "name")]
#[case("`name", "`name")]
#[case("name`", "name`")]
#[case("``name``", "`name`")]
fn strips_only_a_surrounding_backtick_pair(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(strip_backticks(text), expected);
}

#[rstest]
#[case("—", &[])]
#[case(" - ", &[])]
#[case("`alpha` / `beta`", &["alpha", "beta"])]
#[case("`alpha` / ``", &["alpha"])]
#[case("`alpha/beta`", &["alpha/beta"])]
fn reads_alias_groups_without_substring_splitting(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(names_in(text), expected);
}

#[test]
fn shared_name_readers_preserve_order_and_set_identity() {
    assert_eq!(
        backticked("`beta` then `alpha` then `beta`"),
        ["beta", "alpha", "beta"]
    );
    assert_eq!(
        name_set(["beta", "alpha", "beta"]),
        name_set(["alpha", "beta"])
    );
    assert_eq!(backticked("plain text"), Vec::<String>::new());
    assert_eq!(backticked("`unterminated"), ["unterminated"]);
    assert_eq!(Namespace::Function.label(), "function");
    assert_eq!(Registration::OptionAdded.label(), "option added");
}

#[test]
fn repository_reads_and_lists_only_its_isolated_fixture() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = Utf8Path::from_path(temporary.path()).context("UTF-8 fixture path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/nested")?;
    repo.dir.write("docs/z.md", "last")?;
    repo.dir.write("docs/a.md", "first")?;
    repo.dir.write("docs/ignored.txt", "ignored")?;
    repo.dir.write("docs/nested/hidden.md", "nested")?;
    ensure!(repo.read("docs/a.md")? == "first", "fixture read differs");
    ensure!(repo.exists("docs/a.md")?, "fixture document should exist");
    ensure!(
        !repo.exists("docs/missing.md")?,
        "missing document should not exist"
    );
    let listed = repo.markdown_files("docs")?;
    ensure!(
        listed == ["docs/a.md", "docs/z.md"],
        "unexpected listed files: {listed:?}"
    );
    let read_error = repo.read("missing.md").expect_err("missing read must fail");
    ensure!(
        format!("{read_error:#}").contains("read missing.md"),
        "{read_error:#}"
    );
    let list_error = repo
        .markdown_files("missing")
        .expect_err("missing directory must fail");
    ensure!(
        format!("{list_error:#}").contains("list missing"),
        "{list_error:#}"
    );
    repo.dir.write("invalid.md", [0xff])?;
    let encoding_error = repo
        .read("invalid.md")
        .expect_err("non-UTF-8 document must fail");
    ensure!(
        format!("{encoding_error:#}").contains("read invalid.md"),
        "{encoding_error:#}"
    );
    Ok(())
}

#[test]
fn fixture_open_failure_keeps_the_root_context() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = Utf8Path::from_path(temporary.path()).context("UTF-8 fixture path")?;
    let missing = root.join("missing");
    let error = Repo::fixture(&missing)
        .err()
        .context("missing root must fail")?;
    ensure!(
        format!("{error:#}").contains(&format!("open fixture root {missing}")),
        "{error:#}"
    );
    Ok(())
}
