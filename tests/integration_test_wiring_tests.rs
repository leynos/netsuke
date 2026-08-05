//! Contract tests that keep integration-test module trees wired to Cargo targets.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

fn integration_test_sources(tests_dir: &Dir) -> Result<Vec<String>> {
    let mut sources = Vec::new();
    for entry_result in tests_dir
        .read_dir(".")
        .context("read integration-test directory")?
    {
        let directory_entry = entry_result.context("read integration-test directory entry")?;
        let name = directory_entry
            .file_name()
            .context("read integration-test entry name")?;
        if Utf8Path::new(&name).extension() == Some("rs") {
            sources.push(
                tests_dir
                    .read_to_string(&name)
                    .with_context(|| format!("read integration-test source {name}"))?,
            );
        }
    }
    Ok(sources)
}

fn orphaned_module_trees(tests_dir: &Dir, sources: &[String]) -> Result<Vec<String>> {
    let mut orphaned = Vec::new();
    for entry_result in tests_dir
        .read_dir(".")
        .context("read integration-test directory")?
    {
        let directory_entry = entry_result.context("read integration-test directory entry")?;
        if !directory_entry
            .file_type()
            .context("read integration-test entry type")?
            .is_dir()
        {
            continue;
        }

        let name = directory_entry
            .file_name()
            .context("read integration-test directory name")?;
        if !tests_dir.try_exists(format!("{name}/mod.rs"))? {
            continue;
        }

        let conventional_declaration = format!("mod {name};");
        let explicit_path_attribute = format!("#[path = \"{name}/mod.rs\"]");
        let is_wired = sources.iter().any(|source| {
            source.lines().any(|line| {
                let trimmed_line = line.trim();
                trimmed_line == conventional_declaration || trimmed_line == explicit_path_attribute
            })
        });
        if !is_wired {
            orphaned.push(name);
        }
    }
    orphaned.sort_unstable();
    Ok(orphaned)
}

#[test]
fn module_trees_are_wired_to_cargo_test_targets() -> Result<()> {
    let tests_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let tests_dir = Dir::open_ambient_dir(&tests_path, ambient_authority())
        .context("open integration-test directory")?;
    let sources = integration_test_sources(&tests_dir)?;
    let orphaned = orphaned_module_trees(&tests_dir, &sources)?;

    ensure!(
        orphaned.is_empty(),
        "tests/*/mod.rs trees must be declared by a Cargo-discovered tests/*.rs target; orphaned: {}",
        orphaned.join(", ")
    );
    Ok(())
}

#[test]
fn orphaned_and_commented_module_trees_are_reported() -> Result<()> {
    let temp = tempfile::tempdir().context("create integration-test fixture")?;
    let tests_path = Utf8Path::from_path(temp.path()).context("fixture path is not valid UTF-8")?;
    let tests_dir = Dir::open_ambient_dir(tests_path, ambient_authority())
        .context("open integration-test fixture")?;
    tests_dir
        .create_dir("wired")
        .context("create wired module tree")?;
    tests_dir
        .write("wired/mod.rs", "//! Wired fixture.\n")
        .context("write wired module root")?;
    tests_dir
        .create_dir("orphaned")
        .context("create orphaned module tree")?;
    tests_dir
        .write("orphaned/mod.rs", "//! Orphaned fixture.\n")
        .context("write orphaned module root")?;
    tests_dir
        .create_dir("commented")
        .context("create commented module tree")?;
    tests_dir
        .write("commented/mod.rs", "//! Commented fixture.\n")
        .context("write commented module root")?;
    tests_dir
        .write("wired_tests.rs", "mod wired;\n")
        .context("write wired integration-test target")?;
    tests_dir
        .write(
            "commented_tests.rs",
            "// #[path = \"commented/mod.rs\"]\n// mod commented;\n",
        )
        .context("write commented integration-test target")?;

    let sources = integration_test_sources(&tests_dir)?;
    let orphaned = orphaned_module_trees(&tests_dir, &sources)?;

    ensure!(
        orphaned == ["commented", "orphaned"],
        "expected commented and orphaned fixtures to be reported, got {orphaned:?}"
    );
    Ok(())
}
