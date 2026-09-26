//! Contract tests for the canonical `make test` entry point and for every
//! Makefile recipe that overrides `RUSTFLAGS`.
//!
//! `make test` is the single command local development and continuous
//! integration (CI) both run. These tests pin the runner contract it encodes:
//! non-doctest tests go through cargo-nextest and doctests run separately
//! because nextest cannot execute them. Every recipe that invokes
//! `cargo nextest run` shares one worker-bound contract, so no future
//! accelerated loop can disagree with the gate about which bounds a caller set.
//!
//! They also pin the `RUSTFLAGS` contract shared by every recipe that sets the
//! variable. Each such recipe adds `-D warnings` and prepends any value the
//! caller already exported rather than discarding it. `bench-config-load` and
//! the binary-build recipe deliberately set nothing because neither is a lint
//! gate.
//!
//! The `RUSTFLAGS` tests extract each assignment from the Makefile and expand
//! it in a shell. They assert on the flags that expansion yields rather than
//! on recipe text, so they stay valid when the command a recipe runs changes.
//! A guard test fails if a recipe starts setting `RUSTFLAGS` without joining
//! the covered set.

#[path = "support/makefile.rs"]
mod makefile;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{parse_rule, phony_targets, read_repo_file, target_prerequisites, target_recipe};
use std::collections::BTreeSet;
use toml::Value;

/// Every Make target that invokes `cargo nextest run`, and so shares the
/// worker-bound contract.
///
/// `test-nextest` is the gate `make test` composes, and `test-kani-mutations`
/// runs the same runner over the `#[ignore]`-gated mutation compile gate, which
/// is too expensive for the default profile. A contributor sets the bounds once
/// and expects them honoured wherever nextest runs, so the list is a contract
/// rather than a note of what happens to be true today.
///
/// The list is not trusted on its own.
/// [`behavioural_nextest_targets_forward_both_worker_bounds`] discovers the
/// targets that actually invoke the runner and fails when the two disagree, so
/// a new recipe joins the contract or breaks the build.
const NEXTEST_TARGETS: [&str; 2] = ["test-nextest", "test-kani-mutations"];

/// True when `line` is a tab-indented recipe line that invokes the nextest
/// runner.
///
/// Factored out of [`nextest_invoking_targets`] so its `if` keeps two predicate
/// branches: the `let` in a let-chain counts towards the
/// `conditional_max_n_branches` limit, so an inline three-clause condition
/// trades a clippy `collapsible_if` failure for a Whitaker one.
fn invokes_nextest(line: &str) -> bool {
    line.starts_with('\t') && line.contains("nextest run")
}

/// Every Make target whose recipe invokes `cargo nextest run`.
///
/// Walks the file once, remembering the most recent rule header, so a recipe is
/// attributed to the target that declares it. Only tab-indented lines count,
/// which keeps a variable assignment mentioning the runner from being mistaken
/// for a recipe.
fn nextest_invoking_targets(makefile: &str) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    let mut current_rule: Option<&str> = None;
    for line in makefile.lines() {
        if let Some((name, _)) = parse_rule(line) {
            current_rule = Some(name);
            continue;
        }
        if invokes_nextest(line)
            && let Some(name) = current_rule
        {
            targets.insert(name.to_owned());
        }
    }
    targets
}

/// Verify both nextest worker bounds are arguments to the `nextest run` call.
///
/// Two separate bounds, not one. `NEXTEST_BUILD_JOBS` limits the compile that
/// precedes the run and `NEXTEST_TEST_JOBS` limits the test processes, so a
/// lane on a small runner can bound each without oversubscribing the other.
/// Dropping either silently returns that half to nextest's default of one
/// worker per core, which is what exhausted a two-vCPU runner.
///
/// Asserted against the invoking line rather than the whole recipe.
/// [`target_recipe`] returns the recipe's lines joined together, so a check
/// over that string would accept a bound sitting in an unrelated later
/// command, reading as configured while bounding nothing.
///
/// `test-nextest` forwards the bounds only when a caller set them, so the
/// contract covers the empty default too: the passed-in recipe is a real one,
/// where an unset variable expands to nothing and nextest sees no bound at all.
fn ensure_worker_bounds_reach_nextest(target: &str, recipe: &str) -> Result<()> {
    let run_command = recipe
        .lines()
        .find(|line| line.contains("nextest run"))
        .with_context(|| format!("{target} should invoke cargo nextest run"))?;
    ensure!(
        run_command.contains("$(NEXTEST_BUILD_JOBS)"),
        "{target} should pass NEXTEST_BUILD_JOBS to nextest run, found {run_command:?}"
    );
    ensure!(
        run_command.contains("$(NEXTEST_TEST_JOBS)"),
        "{target} should pass NEXTEST_TEST_JOBS to nextest run, found {run_command:?}"
    );
    Ok(())
}

/// Verify every nextest-invoking target forwards both worker bounds.
///
/// The discovered set is the point: every recipe that runs the runner is held
/// to the same rule, so a future accelerated loop cannot honour a bound the
/// gate drops, or vice versa, and leave a local run silently divergent from CI.
///
/// [`NEXTEST_TARGETS`] is checked against the targets the file actually
/// invokes, in both directions, before the bounds are asserted. Checking only
/// the declared list would let a new `nextest run` recipe omit both bounds and
/// still pass; checking only the discovered set would let a stale entry linger
/// after its recipe disappears. The two must name exactly the same targets.
#[test]
fn behavioural_nextest_targets_forward_both_worker_bounds() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;

    let discovered = nextest_invoking_targets(&makefile);
    let declared: BTreeSet<String> = NEXTEST_TARGETS.iter().map(ToString::to_string).collect();
    ensure!(
        discovered == declared,
        "NEXTEST_TARGETS must name every target invoking `nextest run`; \
         invoking but undeclared: {:?}; declared but not invoking: {:?}",
        discovered.difference(&declared).collect::<Vec<_>>(),
        declared.difference(&discovered).collect::<Vec<_>>()
    );

    for target in NEXTEST_TARGETS {
        let recipe = target_recipe(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        ensure_worker_bounds_reach_nextest(target, &recipe)?;
    }
    Ok(())
}

/// The discovery helper sees each target's own recipe, not its neighbours'.
///
/// A guard that attributed a recipe to the wrong target could still return the
/// right *count* on the real Makefile, so the failure mode it must rule out is
/// checked directly: a target whose recipe omits the runner stays out of the
/// set even when the surrounding recipes invoke it, and a rule name is never
/// taken from a variable assignment that merely mentions the runner.
#[test]
fn unit_nextest_discovery_attributes_recipes_to_their_own_target() {
    let makefile = concat!(
        "alpha:\n\tcargo nextest run --workspace\n",
        "beta:\n\tcargo build\n",
        "gamma: alpha\n\tcargo nextest run --workspace\n",
        "delta:\n\tcargo build\n",
        "VAR := mentions nextest run but is not a recipe\n",
        ".PHONY: alpha beta gamma delta\n",
    );
    let discovered = nextest_invoking_targets(makefile);
    assert_eq!(
        discovered,
        BTreeSet::from(["alpha".to_owned(), "gamma".to_owned()]),
        "only alpha and gamma invoke the runner from a recipe line"
    );
    assert!(
        !discovered.contains("VAR"),
        "a variable assignment mentioning the runner is not a recipe"
    );
}

/// Verify that `make test` orders the nextest pass before the doctest pass.
#[test]
fn behavioural_make_test_composes_the_nextest_and_doctest_passes() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;

    let prerequisites =
        target_prerequisites(&makefile, "test").context("Makefile should declare a test target")?;
    ensure!(
        prerequisites == ["test-nextest", "doctest"],
        "make test must depend on nextest and doctests, found {prerequisites:?}"
    );

    let nextest_recipe = target_recipe(&makefile, "test-nextest")
        .context("Makefile should declare a test-nextest target")?;
    ensure!(
        nextest_recipe.contains("nextest run"),
        "test-nextest should route through cargo nextest run, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains("--all-targets"),
        "test-nextest should cover every test target, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains("--all-features"),
        "test-nextest should enable all features, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains("--workspace"),
        "test-nextest should cover the workspace, found {nextest_recipe:?}"
    );
    // What that value expands to is contracted in the `rustflags` module; here
    // the point is only that the pass composes it rather than rolling its own.
    ensure!(
        nextest_recipe.contains("$(GATE_RUSTFLAGS)"),
        "test-nextest should compose GATE_RUSTFLAGS, found {nextest_recipe:?}"
    );

    ensure_worker_bounds_reach_nextest("test-nextest", &nextest_recipe)?;

    let doctest_recipe =
        target_recipe(&makefile, "doctest").context("Makefile should declare a doctest target")?;
    ensure!(
        doctest_recipe.contains("--doc"),
        "doctest should invoke the doctest harness, found {doctest_recipe:?}"
    );
    ensure!(
        !doctest_recipe.contains("nextest"),
        "doctests cannot run under nextest, found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains("$(GATE_RUSTFLAGS)"),
        "doctest should compose GATE_RUSTFLAGS, found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains("--workspace"),
        "doctest should cover the workspace, found {doctest_recipe:?}"
    );
    Ok(())
}

/// Verify `test-kani-mutations` selects the ignored mutation compile gate.
///
/// The gate it runs is `#[ignore]`d, so the recipe is correct only if it names
/// the owning test binary *and* asks nextest to run ignored tests. Drop either
/// half and the target still exits zero while compiling nothing: nextest skips
/// the gate and reports success. That is the exact failure this whole contract
/// exists to prevent — a check that looks green and proves nothing — so the
/// selection is pinned here rather than left to the recipe's good behaviour.
///
/// Asserted against the `nextest run` line, for the reason
/// [`ensure_worker_bounds_reach_nextest`] gives: a flag sitting in an unrelated
/// later command would read as configured while selecting nothing.
#[test]
fn behavioural_kani_mutation_target_selects_the_ignored_compile_gate() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "test-kani-mutations")
        .context("Makefile should declare a test-kani-mutations target")?;
    let run_command = recipe
        .lines()
        .find(|line| line.contains("nextest run"))
        .context("test-kani-mutations should invoke cargo nextest run")?;

    ensure!(
        run_command.contains("--test kani_mutation_evidence_tests"),
        "test-kani-mutations should select the mutation evidence tests, found {run_command:?}"
    );
    ensure!(
        run_command.contains("--run-ignored ignored-only"),
        "test-kani-mutations should run ignored tests or the gated compile check silently \
         skips, found {run_command:?}"
    );
    ensure!(
        run_command.contains("$(GATE_RUSTFLAGS)"),
        "test-kani-mutations should compose GATE_RUSTFLAGS, found {recipe:?}"
    );
    Ok(())
}

/// Keep the glob-expansion benchmark available through the Makefile.
#[test]
fn benchmark_glob_expansion_target_is_phony_and_runs_the_expected_bench() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let phony = phony_targets(&makefile);
    ensure!(
        phony.contains(&"bench-glob-expansion"),
        ".PHONY must include bench-glob-expansion, found {phony:?}"
    );
    let recipe = target_recipe(&makefile, "bench-glob-expansion")
        .context("Makefile should declare a bench-glob-expansion recipe")?;
    ensure!(
        recipe.contains("$(CARGO) bench --bench glob_expansion"),
        "bench-glob-expansion must invoke the glob_expansion bench, found {recipe:?}"
    );
    Ok(())
}

#[path = "makefile_test_target/rustflags.rs"]
mod rustflags;

#[path = "makefile_test_target/rustdocflags.rs"]
mod rustdocflags;

#[cfg(unix)]
#[path = "makefile_test_target/markdown_recipe_harness.rs"]
mod markdown_recipe_harness;

#[cfg(unix)]
#[path = "makefile_test_target/markdown_recipes.rs"]
mod markdown_recipes;

/// Returns every nextest profile override.
fn all_profile_overrides(config: &Value) -> impl Iterator<Item = &Value> {
    config
        .get("profile")
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .filter_map(|profile| profile.get("overrides").and_then(Value::as_array))
        .flatten()
}

#[test]
fn behavioural_nextest_config_does_not_serialize_environment_tests() -> Result<()> {
    let config: Value = read_repo_file(&Utf8Path::new(".config").join("nextest.toml"))?
        .parse()
        .context("nextest configuration should be valid TOML")?;

    ensure!(
        config
            .get("test-groups")
            .and_then(|groups| groups.get("serial-env"))
            .is_none(),
        "environment tests use injected state and should not declare a serial-env test group"
    );

    let has_serial_override = all_profile_overrides(&config)
        .any(|entry| entry.get("test-group").and_then(Value::as_str) == Some("serial-env"));
    ensure!(
        !has_serial_override,
        "environment tests use injected state and should not have a serial-env override"
    );
    Ok(())
}
