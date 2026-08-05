//! Contract tests that keep integration-test module trees wired to Cargo targets.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;

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

/// How a generated module tree is declared by the generated test target.
///
/// `orphaned_module_trees` claims a universal property: a tree is excluded from
/// the orphan list exactly when some source declares it with an active `mod`
/// item or `#[path]` attribute. Enumerating the declaration forms lets the
/// property test below assert both halves of that biconditional.
#[derive(Debug, Clone, Copy)]
enum Declaration {
    Conventional,
    PathAttribute,
    CommentedConventional,
    CommentedPath,
    Absent,
}

impl Declaration {
    /// Whether this form should keep the tree out of the orphan list.
    const fn wires(self) -> bool {
        matches!(self, Self::Conventional | Self::PathAttribute)
    }

    /// Render this declaration for `name`, indented by `indent`.
    ///
    /// The indentation exercises the guard's line trimming; the `_tree` alias
    /// on the `#[path]` form mirrors how a real target names a relocated
    /// module. Generated names always end in `_<index>`, so that alias can
    /// never collide with another generated tree.
    fn render(self, name: &str, indent: &str) -> String {
        match self {
            Self::Conventional => format!("{indent}mod {name};\n"),
            Self::PathAttribute => {
                format!("{indent}#[path = \"{name}/mod.rs\"]\n{indent}mod {name}_tree;\n")
            }
            Self::CommentedConventional => format!("{indent}// mod {name};\n"),
            Self::CommentedPath => {
                format!("{indent}// #[path = \"{name}/mod.rs\"]\n{indent}// mod {name};\n")
            }
            Self::Absent => String::new(),
        }
    }
}

type ModuleTreeSpec = (String, Declaration, String);

fn module_tree_specs() -> impl Strategy<Value = Vec<ModuleTreeSpec>> {
    let declaration_strategy = prop_oneof![
        Just(Declaration::Conventional),
        Just(Declaration::PathAttribute),
        Just(Declaration::CommentedConventional),
        Just(Declaration::CommentedPath),
        Just(Declaration::Absent),
    ];
    proptest::collection::vec(
        ("[a-z][a-z0-9_]{0,6}", declaration_strategy, 0usize..4),
        1..6,
    )
    .prop_map(|specs| {
        specs
            .into_iter()
            .enumerate()
            .map(|(index, (name, declaration, indent))| {
                // Suffix the index so generated names stay distinct even
                // when the string strategy repeats a value.
                (format!("{name}_{index}"), declaration, " ".repeat(indent))
            })
            .collect()
    })
}

/// Materialize `specs` as a tests directory and return `(expected, actual)`.
fn run_wiring_scenario(specs: &[ModuleTreeSpec]) -> Result<(Vec<String>, Vec<String>)> {
    let temp = tempfile::tempdir().context("create generated wiring fixture")?;
    let tests_path = Utf8Path::from_path(temp.path()).context("fixture path is not valid UTF-8")?;
    let tests_dir =
        Dir::open_ambient_dir(tests_path, ambient_authority()).context("open generated fixture")?;

    let mut source = String::from("//! Generated wiring fixture.\n");
    let mut expected = Vec::new();
    for (name, declaration, indent) in specs {
        tests_dir
            .create_dir(name)
            .with_context(|| format!("create generated module tree {name}"))?;
        tests_dir
            .write(format!("{name}/mod.rs"), "//! Generated fixture.\n")
            .with_context(|| format!("write generated module root {name}"))?;
        source.push_str(&declaration.render(name, indent));
        if !declaration.wires() {
            expected.push(name.clone());
        }
    }
    tests_dir
        .write("generated_tests.rs", source)
        .context("write generated integration-test target")?;
    expected.sort_unstable();

    let sources = integration_test_sources(&tests_dir)?;
    let orphaned = orphaned_module_trees(&tests_dir, &sources)?;
    Ok((expected, orphaned))
}

proptest! {
    /// Only genuinely wired trees are excluded from the orphan list.
    ///
    /// The three handwritten fixtures above pin one example of each outcome;
    /// this covers arbitrary tree names against every declaration form, so a
    /// guard that matched substrings rather than whole trimmed lines, or that
    /// mistook a commented declaration for an active one, would be caught.
    #[test]
    fn only_unwired_module_trees_are_reported(specs in module_tree_specs()) {
        let (expected, orphaned) = run_wiring_scenario(&specs)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(orphaned, expected);
    }
}
