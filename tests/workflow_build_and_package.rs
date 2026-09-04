//! Validate build-and-package workflow wiring for shared actions.

mod common;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use rstest::rstest;
use serde_yaml::Value as YamlValue;
use std::{fs, path::PathBuf};
use toml::Value;

fn release_staging_contents() -> Result<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("release-staging.toml");
    fs::read_to_string(&path)
        .with_context(|| format!("read release staging contents from {}", path.display()))
}

fn goreleaser_contents() -> Result<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".goreleaser.yaml");
    fs::read_to_string(&path)
        .with_context(|| format!("read GoReleaser contents from {}", path.display()))
}

fn goreleaser_config() -> Result<YamlValue> {
    serde_yaml::from_str(&goreleaser_contents()?).context("parse GoReleaser YAML")
}

/// The fallback build whose `pre` hook maps GOOS/GOARCH to Rust target
/// triples. Identified by `skip_build: true` plus the `id`, because the
/// platform-defined build variables (`GOOS`/`GOARCH`) are only meaningful to
/// `GoReleaser` at packaging time — the very hook this contract pins.
fn fallback_build(config: &YamlValue) -> Result<&YamlValue> {
    config
        .get("builds")
        .and_then(YamlValue::as_sequence)
        .context("GoReleaser config should declare builds")?
        .iter()
        .find(|build| {
            build.get("skip_build").and_then(YamlValue::as_bool) == Some(true)
                && build.get("id").and_then(YamlValue::as_str) == Some("netsuke")
        })
        .context("GoReleaser config should declare the netsuke fallback build")
}

/// Count the builds declaring a non-empty build-scoped `pre` hook.
fn build_pre_hook_count(config: &YamlValue) -> usize {
    config
        .get("builds")
        .and_then(YamlValue::as_sequence)
        .iter()
        .flat_map(|builds| builds.iter())
        .filter(|build| {
            build
                .get("hooks")
                .and_then(|hooks| hooks.get("pre"))
                .and_then(YamlValue::as_sequence)
                .is_some_and(|hooks| !hooks.is_empty())
        })
        .count()
}

/// The fallback build is the only build allowed to carry a build-scoped `pre`
/// hook. A second build-level hook anywhere in the file would satisfy a
/// whole-file line scan without the fallback hook being reachable.
fn ensure_only_fallback_build_has_pre_hook(config: &YamlValue) -> Result<()> {
    let build_pre_hooks = build_pre_hook_count(config);
    ensure!(
        build_pre_hooks == 1,
        "the netsuke fallback build should be the only build declaring a pre hook, found {build_pre_hooks}"
    );
    Ok(())
}

fn staging_config() -> Result<Value> {
    release_staging_contents()?
        .parse::<Value>()
        .context("parse release staging TOML")
}

fn artefact_sources(config: &Value) -> Result<Vec<&str>> {
    let mut sources = Vec::new();
    let common_artefacts = config
        .get("common")
        .and_then(|common| common.get("artefacts"))
        .and_then(Value::as_array)
        .context("common artefacts should be an array")?;
    for artefact in common_artefacts {
        sources.push(
            artefact
                .get("source")
                .and_then(Value::as_str)
                .context("common artefact source should be a string")?,
        );
    }

    let targets = config
        .get("targets")
        .and_then(Value::as_table)
        .context("targets should be a table")?;
    for target in targets.values() {
        let Some(artefacts) = target.get("artefacts").and_then(Value::as_array) else {
            continue;
        };
        for artefact in artefacts {
            sources.push(
                artefact
                    .get("source")
                    .and_then(Value::as_str)
                    .context("target artefact source should be a string")?,
            );
        }
    }

    Ok(sources)
}

fn flush_block(current: &mut Vec<&str>, blocks: &mut Vec<String>) {
    if !current.is_empty() {
        let block = current.join("\n");
        if block.contains("rust-build-release@") {
            blocks.push(block);
        }
        current.clear();
    }
}

fn rust_build_release_step_blocks(contents: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for line in contents.lines() {
        if line.starts_with("      - ") {
            flush_block(&mut current_block, &mut blocks);
        }
        current_block.push(line);
    }

    flush_block(&mut current_block, &mut blocks);
    blocks
}

fn workflow_step_body<'a>(contents: &'a str, step_name: &str) -> Vec<&'a str> {
    let step = format!("- name: {step_name}");
    contents
        .lines()
        .skip_while(|line| !line.contains(&step))
        .take_while(|line| !line.contains("      - name: ") || line.contains(&step))
        .collect()
}

fn assert_shared_build_skips_man_page_discovery(contents: &str) {
    let rust_build_steps = rust_build_release_step_blocks(contents);
    assert!(
        !rust_build_steps.is_empty(),
        "workflow should call rust-build-release"
    );
    for step in rust_build_steps {
        assert!(
            step.contains("skip-man-page-discovery: 'true'"),
            "rust-build-release call should skip embedded man-page discovery"
        );
    }
}

#[test]
fn behavioural_build_and_package_wiring_matches_shared_actions() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains("stage-release-artefacts@"),
        "workflow should use shared stage-release-artefacts action"
    );
    assert!(
        contents.contains("normalize-windows-paths: ${{ inputs.platform == 'windows' }}"),
        "workflow should normalize Windows paths when staging on Windows"
    );
    assert!(
        contents.contains("application-path: ${{ steps.stage_paths.outputs.binary_path }}"),
        "windows-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("license-rtf-path: ${{ steps.stage_paths.outputs.license_path }}"),
        "windows-package should consume staged license_path output"
    );
    assert!(
        contents.contains("upload-artefact: ${{ inputs['should-upload-workflow-artifacts'] }}"),
        "windows-package should use the upload-artefact input spelling"
    );
    assert!(
        contents.contains("binary: ${{ steps.stage_paths.outputs.binary_path }}"),
        "macos-package should consume staged binary_path output"
    );
    assert!(
        contents.contains("manpage: ${{ steps.stage_paths.outputs.man_path }}"),
        "macos-package should consume staged man_path output"
    );
    assert!(
        contents.contains("${{ steps.stage_paths.outputs.artefact_dir }}"),
        "workflow should use the staged artefact_dir output for uploads"
    );
}

/// Reject every `cargo install` form that would compile `cargo-orthohelp`.
///
/// Matching the bare `cargo install cargo-orthohelp` prefix is not enough:
/// `cargo install --locked cargo-orthohelp@0.9.0` compiles the tool just the
/// same, and so does any other flag placed before the crate name. The pattern
/// therefore allows arbitrary flags and version selectors between the
/// subcommand and the crate.
fn assert_orthohelp_comes_from_a_prebuilt_release(contents: &str) -> Result<()> {
    let install_body = workflow_step_body(contents, "Install cargo-orthohelp").join("\n");
    ensure!(
        install_body.contains("cargo binstall --no-confirm --locked \\")
            && install_body.contains("--strategies crate-meta-data,quick-install"),
        "workflow should try binary-only cargo-binstall strategies first"
    );
    ensure!(
        install_body.contains("cargo-orthohelp@0.9.0"),
        "workflow should pin the cargo-orthohelp release version"
    );

    // `ortho-config` publishes no binaries for any platform
    // (leynos/ortho-config#479), so a source build is permitted, but only in
    // this exact guarded form: after the binary-only attempt has genuinely
    // failed, and into a dedicated target directory that never shares
    // compiler output with the product. Any other `cargo install` naming the
    // tool, in this step or elsewhere, is still rejected.
    let guarded_fallback = "CARGO_TARGET_DIR=\"${ORTHOHELP_BUILD_DIR}\" \\\n            \
        cargo install --locked cargo-orthohelp@0.9.0";
    ensure!(
        install_body.contains("if cargo binstall") && install_body.contains(guarded_fallback),
        "a cargo-orthohelp source build is permitted only as the guarded \
         fallback into a dedicated CARGO_TARGET_DIR"
    );
    // Flags, `--version`/`--index` selectors, and quoting all sit between the
    // subcommand and the crate name, so the pattern allows arbitrary tokens
    // that are not themselves the crate. Counting matches means a second
    // source install cannot hide behind the documented one.
    let source_install =
        regex::Regex::new(r#"cargo\s+install\s+(?:[-"'][^\s]*\s+)*"?cargo-orthohelp"#)
            .context("compile the cargo-orthohelp source-install pattern")?;
    ensure!(
        source_install.find_iter(contents).count() == 1,
        "the guarded fallback should be the only cargo-orthohelp source install \
         anywhere in the workflow"
    );

    let build_index = contents
        .find("- name: Build release binary")
        .context("workflow should build the release binary")?;
    let install_index = contents
        .find("- name: Install cargo-orthohelp")
        .context("workflow should install cargo-orthohelp")?;
    ensure!(
        build_index < install_index,
        "cargo-orthohelp must be installed after rust-build-release provisions cargo-binstall"
    );
    Ok(())
}

#[test]
fn behavioural_build_and_package_generates_release_help_with_orthohelp() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert_orthohelp_comes_from_a_prebuilt_release(&contents)
        .expect("cargo-orthohelp should come from a pinned prebuilt release");
    assert!(
        contents.contains("scripts/generate-release-help.sh"),
        "workflow should call the release help script"
    );
    assert!(
        contents.contains("\"target/orthohelp/${{ inputs.target }}/release\""),
        "workflow should generate help under target/orthohelp"
    );
    assert_shared_build_skips_man_page_discovery(&contents);
    assert!(
        contents.contains("man-paths: ${{ steps.stage_paths.outputs.man_path }}"),
        "Linux packaging should consume the staged man_path output"
    );
    assert!(
        !contents.contains("target/generated-man"),
        "workflow should not rely on build.rs generated man pages"
    );
}

#[rstest]
#[case("config-file: .github/release-staging.toml")]
#[case("man-paths: ${{ steps.stage_paths.outputs.man_path }}")]
fn build_and_package_wires_staged_release_outputs(#[case] expected: &str) {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains(expected),
        "build-and-package workflow should contain {expected}"
    );
}

#[test]
fn behavioural_build_and_package_validates_release_help_tooling() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");

    assert!(
        contents.contains(
            "cargo-orthohelp --version | grep -Eq '(^|[[:space:]])0\\.9\\.0([[:space:]]|$)'"
        ),
        "workflow should validate the installed cargo-orthohelp version"
    );
    assert!(
        contents.contains("\"${{ inputs.platform == 'windows' && 'Netsuke' || env.BIN_NAME }}\""),
        "workflow should pass the PowerShell module name explicitly"
    );
    for step_name in ["Validate cargo-orthohelp version", "Generate release help"] {
        let step_body = workflow_step_body(&contents, step_name).join("\n");
        assert!(
            step_body.contains("shell: bash"),
            "{step_name} should use Bash explicitly"
        );
    }
}

#[test]
fn goreleaser_fallback_uses_rust_target_triple_orthohelp_paths() -> Result<()> {
    let config = goreleaser_config()?;
    let fallback = fallback_build(&config)?;
    let pre_hook = fallback
        .get("hooks")
        .and_then(|hooks| hooks.get("pre"))
        .and_then(YamlValue::as_sequence)
        .and_then(|hooks| hooks.first())
        .and_then(YamlValue::as_str)
        .context("the netsuke fallback build should declare a pre hook")?;

    ensure!(
        !pre_hook.contains("target/orthohelp/${{GOOS}-${GOARCH}}")
            && !pre_hook.contains("target/orthohelp/${GOOS}-${GOARCH}"),
        "GoReleaser fallback must not use raw GOOS/GOARCH orthohelp paths"
    );
    ensure!(
        pre_hook.contains("x86_64-unknown-linux-gnu"),
        "GoReleaser fallback should map linux/amd64 to the Rust target triple"
    );
    ensure!(
        pre_hook.contains("target/orthohelp/${RUST_TARGET}/release/man/man1/netsuke.1"),
        "GoReleaser fallback should resolve orthohelp output through RUST_TARGET"
    );
    ensure!(
        pre_hook.contains("${GOOS}/${GOARCH}"),
        "GoReleaser fallback hook should branch on GOOS/GOARCH so it runs where those are defined"
    );

    ensure_only_fallback_build_has_pre_hook(&config)?;

    ensure!(
        !config
            .as_mapping()
            .is_some_and(|root| root.contains_key(YamlValue::String("before".to_owned()))),
        "GoReleaser config must not fall back to the deprecated global before hook"
    );
    Ok(())
}
#[test]
fn goreleaser_fallback_pre_hook_guards_an_unrelated_build_level_hook() -> Result<()> {
    // Reconstruct the file as parsed, with a second build that carries its own
    // build-level `pre` hook. A line-scanning assertion would see the second
    // `hooks: pre:` pair and pass even if the fallback hook were missing; the
    // structural contract must reject that configuration.
    let config = goreleaser_config()?;
    let mut builds = config
        .get("builds")
        .and_then(YamlValue::as_sequence)
        .cloned()
        .unwrap_or_default();
    let mut unrelated = serde_yaml::Mapping::new();
    unrelated.insert(
        YamlValue::String("id".to_owned()),
        YamlValue::String("unrelated-package".to_owned()),
    );
    let mut hooks = serde_yaml::Mapping::new();
    hooks.insert(
        YamlValue::String("pre".to_owned()),
        YamlValue::Sequence(vec![YamlValue::String("echo unrelated".to_owned())]),
    );
    unrelated.insert(
        YamlValue::String("hooks".to_owned()),
        YamlValue::Mapping(hooks),
    );
    builds.push(YamlValue::Mapping(unrelated));
    let mut edited = serde_yaml::Mapping::new();
    for (key, value) in config
        .as_mapping()
        .context("GoReleaser config should be a mapping")?
    {
        edited.insert(key.clone(), value.clone());
    }
    edited.insert(
        YamlValue::String("builds".to_owned()),
        YamlValue::Sequence(builds),
    );
    let edited_config = YamlValue::Mapping(edited);

    ensure!(
        build_pre_hook_count(&edited_config) == 2,
        "the regression fixture should contain exactly two build-level pre hooks"
    );
    let Err(error) = ensure_only_fallback_build_has_pre_hook(&edited_config) else {
        anyhow::bail!("the structural guard should reject a second unrelated build-level pre hook");
    };
    ensure!(
        error
            .to_string()
            .contains("only build declaring a pre hook"),
        "the guard should diagnose the unrelated build-level hook, got: {error}"
    );
    Ok(())
}

#[test]
fn windows_upload_includes_staged_artefact_dir() {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");
    let step_body = workflow_step_body(&contents, "Upload Windows artefacts").join("\n");

    assert!(
        step_body.contains("${{ steps.stage_paths.outputs.artefact_dir }}"),
        "Windows upload should include staged sidecar artefacts"
    );
}

#[rstest]
#[case("Stage artefacts")]
#[case("Capture staged paths")]
fn behavioural_staging_runs_for_every_platform(#[case] step_name: &str) {
    let contents = workflow_contents("build-and-package.yml")
        .expect("build-and-package workflow should be readable");
    let step = format!("- name: {step_name}");
    let step_body = workflow_step_body(&contents, step_name).join("\n");

    assert!(
        step_body.contains(&step),
        "{step_name} step should exist in the workflow"
    );
    assert!(
        !step_body.contains("if: inputs.platform != 'linux'"),
        "{step_name} should run for Linux as well as Windows and macOS"
    );
}

#[rstest]
#[case("target/orthohelp/{target}/release/man/man1/{bin_name}.1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psm1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/Netsuke.psd1")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/Netsuke-help.xml")]
#[case("target/orthohelp/{target}/release/powershell/Netsuke/en-US/about_Netsuke.help.txt")]
#[case("target/generated-completions/{target}/release/netsuke.bash")]
#[case("target/generated-completions/{target}/release/netsuke.elv")]
#[case("target/generated-completions/{target}/release/netsuke.fish")]
#[case("target/generated-completions/{target}/release/_netsuke.ps1")]
#[case("target/generated-completions/{target}/release/_netsuke")]
fn release_staging_declares_orthohelp_outputs(#[case] expected_source: &str) -> Result<()> {
    let config = staging_config()?;
    let sources = artefact_sources(&config)?;
    ensure!(
        sources.contains(&expected_source),
        "expected release staging source {expected_source}, got {sources:?}"
    );
    Ok(())
}

#[rstest]
#[case("target/generated-man")]
#[case("OUT_DIR")]
#[case("clap_mangen")]
fn release_staging_does_not_reference_build_script_help_paths(
    #[case] removed_fragment: &str,
) -> Result<()> {
    let contents = release_staging_contents()?;
    ensure!(
        !contents.contains(removed_fragment),
        "release staging should not reference {removed_fragment}"
    );
    Ok(())
}

#[test]
fn orthohelp_man_page_has_no_out_dir_alternative() -> Result<()> {
    let config = staging_config()?;
    let targets = config
        .get("targets")
        .and_then(Value::as_table)
        .context("targets should be a table")?;
    let man_page = targets
        .values()
        .filter_map(|target| target.get("artefacts").and_then(Value::as_array))
        .flatten()
        .find(|artefact| {
            artefact.get("source").and_then(Value::as_str)
                == Some("target/orthohelp/{target}/release/man/man1/{bin_name}.1")
        })
        .context("orthohelp man page artefact should be declared")?;

    ensure!(
        man_page.get("alternatives").is_none(),
        "orthohelp man page should not fall back to Cargo OUT_DIR"
    );
    Ok(())
}
