//! Tests for file globbing via the `glob()` Jinja helper.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::{NetsukeManifest, StringOrList},
    manifest,
};
use rstest::{fixture, rstest};
use std::{fs, path::Path};
use test_support::{display_error_chain, manifest::manifest_yaml};

#[derive(Debug)]
struct TestFiles<'a> {
    pub files: &'a [(&'a str, &'a str)],
    pub dirs: &'a [&'a str],
}

#[derive(Debug)]
struct GlobTestCase<'a> {
    pub setup: TestFiles<'a>,
    pub pattern_suffix: &'a str,
    pub name_template: &'a str,
    pub expected_partial: &'a [&'a str],
    pub description: &'a str,
}

#[derive(Debug)]
struct BraceErrorTestCase<'a> {
    pub pattern: &'a str,
    pub expected: &'a str,
}

fn target_names(manifest: &NetsukeManifest) -> Result<Vec<String>> {
    manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            StringOrList::String(s) => Ok(s.clone()),
            other => Err(anyhow!("expected String, got {other:?}")),
        })
        .collect()
}

fn create_test_files(base: &Path, files: &[(&str, &str)]) -> Result<()> {
    for (rel, content) in files {
        let path = base.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dirs for {}", parent.display()))?;
        }
        fs::write(&path, content).with_context(|| format!("write file {}", path.display()))?;
    }
    Ok(())
}

fn create_test_dirs(base: &Path, dirs: &[&str]) -> Result<()> {
    for d in dirs {
        let dir_path = base.join(d);
        fs::create_dir_all(&dir_path)
            .with_context(|| format!("create dir {}", dir_path.display()))?;
    }
    Ok(())
}

#[fixture]
fn temp_dir() -> tempfile::TempDir {
    #[expect(
        clippy::expect_used,
        reason = "fixture should fail fast when temp directory creation fails"
    )]
    {
        tempfile::tempdir().expect("create temp dir")
    }
}

#[rstest]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("b.txt", "b"), ("a.txt", "a")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "{{ item | replace('{dir}/', '') | replace('{dir}\\\\', '') | replace('.txt', '.out') }}",
    expected_partial: &["a.out", "b.out"],
    description: "expands and sorts matches",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[], dirs: &[] },
    pattern_suffix: "*.nomatch",
    name_template: "none",
    expected_partial: &[],
    description: "no targets when pattern has no matches",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("sub/x.txt", "x")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "bad",
    expected_partial: &[],
    description: "wildcards must not cross '/'",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("sub/x.txt", "x")], dirs: &[] },
    pattern_suffix: "**/*.txt",
    name_template: "{{ item | replace('{dir}/', '') }}",
    expected_partial: &["sub/x.txt"],
    description: "** spans directories",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[(".hidden.txt", "h")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "{{ item }}",
    expected_partial: &[".hidden.txt"],
    description: "dotfiles should match",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("UPPER.TXT", "x")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "fail",
    expected_partial: &[],
    description: "glob should be case-sensitive",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("a.txt", "a")], dirs: &["sub"] },
    pattern_suffix: "*",
    name_template: "{{ item }}",
    expected_partial: &["a.txt"],
    description: "should filter directories",
})]
fn test_glob_behavior(temp_dir: tempfile::TempDir, #[case] case: GlobTestCase) -> Result<()> {
    create_test_files(temp_dir.path(), case.setup.files)?;
    create_test_dirs(temp_dir.path(), case.setup.dirs)?;

    let dir_str = temp_dir.path().display().to_string();
    let dir_fwd = dir_str.replace('\\', "/");
    let pattern = format!("{dir_str}/{}", case.pattern_suffix);
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{name_template}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        name_template = case
            .name_template
            .replace("{dir}/", &format!("{dir_fwd}/"))
            .replace("{dir}\\\\", &format!("{dir_str}\\\\\\\\")),
    ));

    let manifest = manifest::from_str(&yaml)?;

    if case.expected_partial.is_empty() {
        ensure!(
            manifest.targets.is_empty(),
            "expected no targets for {}",
            case.description
        );
    } else {
        let prefix_fwd = format!("{dir_fwd}/");
        let prefix_back = format!("{dir_str}\\");
        let names: Vec<_> = target_names(&manifest)?
            .into_iter()
            .map(|n| n.replace(&prefix_fwd, "").replace(&prefix_back, ""))
            .collect();
        ensure!(names == case.expected_partial, "{}", case.description);
    }
    Ok(())
}

#[test]
fn glob_unmatched_bracket_errors() -> Result<()> {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('[')\n    name: bad\n    command: echo hi\n");
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected invalid pattern error, but parsed manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = format!("{err:#}");
            ensure!(msg.contains("invalid glob pattern"), "{msg}");
            Ok(())
        }
    }
}

#[rstest]
#[case(BraceErrorTestCase { pattern: "{", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "foo{bar{baz.txt", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "{a,b{c,d}", expected: "unmatched '{'" })]
fn glob_unmatched_brace_errors(#[case] case: BraceErrorTestCase) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected invalid pattern error, but parsed manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = display_error_chain(err.as_ref());
            ensure!(msg.contains("invalid glob pattern"), "{msg}");
            ensure!(msg.contains(case.expected), "{msg}");
            Ok(())
        }
    }
}

#[cfg(unix)]
#[rstest]
#[case(BraceErrorTestCase { pattern: "\\\\{foo}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\{bar}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "{foo\\\\}", expected: "unmatched '{'" })]
fn glob_unmatched_brace_errors_with_escapes(#[case] case: BraceErrorTestCase) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected invalid pattern error, but parsed manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = display_error_chain(err.as_ref());
            ensure!(msg.contains("invalid glob pattern"), "{msg}");
            ensure!(msg.contains(case.expected), "{msg}");
            Ok(())
        }
    }
}

#[test]
fn glob_unmatched_opening_brace_reports_position() -> Result<()> {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('{')\n    name: bad\n    command: echo hi\n");
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected unmatched brace error, got manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = display_error_chain(err.as_ref());
            ensure!(msg.contains("unmatched '{' at position 0"), "{msg}");
            Ok(())
        }
    }
}

#[test]
fn glob_unmatched_closing_brace_reports_position() -> Result<()> {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('foo}')\n    name: bad\n    command: echo hi\n");
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected unmatched brace error, got manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = display_error_chain(err.as_ref());
            ensure!(msg.contains("unmatched '}' at position 3"), "{msg}");
            Ok(())
        }
    }
}

#[rstest]
#[case(BraceErrorTestCase { pattern: "\\\\{", expected: "" })]
#[case(BraceErrorTestCase { pattern: "\\\\}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo{bar\\\\}baz}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\{bar", expected: "" })]
#[cfg(unix)]
#[case(BraceErrorTestCase { pattern: "ends-with-backslash-\\\\", expected: "" })]
fn glob_escaped_braces_are_literals(#[case] case: BraceErrorTestCase) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: ok\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let manifest = manifest::from_str(&yaml)?;
    ensure!(manifest.targets.is_empty());
    Ok(())
}
#[cfg(windows)]
#[rstest]
#[case(BraceErrorTestCase { pattern: "\\{foo", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "foo\\}", expected: "unmatched '}'" })]
fn glob_windows_backslash_does_not_escape_braces(#[case] case: BraceErrorTestCase) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected invalid pattern error, but parsed manifest {manifest:?}"
        )),
        Err(err) => {
            let msg = display_error_chain(err.as_ref());
            ensure!(msg.contains("invalid glob pattern"), "{msg}");
            ensure!(msg.contains(case.expected), "{msg}");
            Ok(())
        }
    }
}
#[rstest]
#[case(BraceErrorTestCase { pattern: "[{}]", expected: "" })] // braces as literals inside a class
#[case(BraceErrorTestCase { pattern: "[{]", expected: "" })] // unmatched '{' inside class must NOT error
#[case(BraceErrorTestCase { pattern: "x{a,{b,c}}.txt", expected: "" })] // nested braces
#[case(BraceErrorTestCase { pattern: "{a,{b,{c,{d,e}}}}", expected: "" })] // deeply nested braces
fn glob_braces_in_classes_and_nested(#[case] case: BraceErrorTestCase) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: ok\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    manifest::from_str(&yaml).with_context(|| format!("pattern should parse: {}", case.pattern))?;
    Ok(())
}

#[rstest]
fn glob_accepts_windows_path_separators(temp_dir: tempfile::TempDir) -> Result<()> {
    fs::write(temp_dir.path().join("a.txt"), "a").context("write a")?;
    fs::write(temp_dir.path().join("b.txt"), "b").context("write b")?;
    let dir_fwd = temp_dir.path().display().to_string();
    let dir_win = dir_fwd.replace('/', "\\\\");
    let pattern = format!("{dir_win}\\\\*.txt");
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{{{{ item }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml)?;
    let prefix_fwd = format!("{dir_fwd}/");
    let names: Vec<_> = target_names(&manifest)?
        .into_iter()
        .map(|n| n.replace(&prefix_fwd, "").replace(".txt", ".out"))
        .collect();
    ensure!(names == ["a.out", "b.out"]);
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn glob_is_case_sensitive_on_windows(temp_dir: tempfile::TempDir) -> Result<()> {
    fs::write(temp_dir.path().join("UPPER.TXT"), "x").context("write file")?;
    let pattern = format!("{}/*.txt", temp_dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: fail\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml)?;
    ensure!(manifest.targets.is_empty());
    Ok(())
}
