//! Tests covering manifest workspace resolution and filesystem helpers.
use super::super::{
    EnvReadError, EnvReader, from_path_for_manifest_query, from_path_with_policy_and_env,
    open_manifest_workspace,
};
use crate::ast::Recipe;
use crate::stdlib::NetworkPolicy;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use camino::Utf8Path;
use metrics_util::debugging::DebuggingRecorder;
use rstest::rstest;
use std::{path::Path, sync::Arc};
use tempfile::tempdir;
use test_support::fs as test_fs;
use test_support::{hash, http};
use tracing::level_filters::LevelFilter;
use url::Url;

#[rstest]
#[case(true)]
#[case(false)]
fn open_manifest_workspace_resolves_workspace_root(#[case] use_relative: bool) -> AnyResult<()> {
    let temp = tempdir().context("create temp workspace")?;
    let manifest_path = if use_relative {
        Path::new("Netsukefile").to_path_buf()
    } else {
        temp.path().join("Netsukefile")
    };
    let base = use_relative.then(|| temp.path());
    let workspace = open_manifest_workspace(&manifest_path, base)?;
    let expected =
        Utf8Path::from_path(temp.path()).context("temp workspace path should be valid UTF-8")?;
    ensure!(
        workspace.root == expected,
        "expected workspace root {expected}, got {root}",
        root = workspace.root
    );
    ensure!(
        workspace.manifest_file == "Netsukefile",
        "expected manifest file name Netsukefile, got {file}",
        file = workspace.manifest_file
    );
    Ok(())
}

/// A relative base is anchored at the process working directory, so the
/// reported root stays absolute even though the manifest path is relative.
#[rstest]
fn open_manifest_workspace_anchors_relative_base_at_the_process_directory() -> AnyResult<()> {
    // Both the manifest parent and the base are relative, which is the branch
    // that must be anchored at the process working directory. The workspace is
    // opened on the ambient directory; no file needs to exist for this check.
    let workspace = open_manifest_workspace(Path::new("Netsukefile"), Some(Path::new(".")))?;
    ensure!(
        workspace.root.is_absolute(),
        "a relative base must be anchored at the working directory, got {root}",
        root = workspace.root
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_manifest_workspace_rejects_non_utf_workspace_root() -> AnyResult<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp workspace")?;
    let invalid_component = OsString::from_vec(vec![0xFF]); // invalid standalone byte
    let manifest_dir = temp.path().join(&invalid_component);
    test_fs::create_dir_all(&manifest_dir)
        .context("create manifest directory with invalid UTF-8 component")?;
    let manifest_path = manifest_dir.join("manifest.yml");
    let err = open_manifest_workspace(&manifest_path, None)
        .expect_err("workspace should fail when its root contains non-UTF-8 components");
    ensure!(
        err.to_string().contains("path is not valid UTF-8"),
        "error should mention non-UTF-8 components but was {err}"
    );
    Ok(())
}

#[rstest]
fn open_manifest_workspace_reports_missing_file_name() -> AnyResult<()> {
    // The filesystem root has no file-name component, so extraction fails with a
    // missing-name error, distinct from the non-UTF-8 case.
    let err = open_manifest_workspace(Path::new("/"), None)
        .expect_err("workspace should fail when the path has no file name");
    ensure!(
        err.to_string().contains("has no file name"),
        "error should report the missing file name but was {err}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_manifest_workspace_rejects_non_utf_file_name() -> AnyResult<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().context("create temp workspace")?;
    let invalid_name = OsString::from_vec(vec![b'm', 0xFF]); // invalid trailing byte
    let manifest_path = temp.path().join(&invalid_name);
    let err = open_manifest_workspace(&manifest_path, None)
        .expect_err("workspace should fail when the file name is not valid UTF-8");
    ensure!(
        err.to_string().contains("path is not valid UTF-8"),
        "error should mention the non-UTF-8 file name but was {err}"
    );
    Ok(())
}

#[rstest]
fn open_manifest_workspace_reports_open_failure() -> AnyResult<()> {
    // The parent directory does not exist, so `Dir::open_ambient_dir` fails and
    // the error is wrapped as a workspace open failure.
    let temp = tempdir().context("create temp workspace")?;
    let manifest_path = temp.path().join("missing-subdir").join("Netsukefile");
    let err = open_manifest_workspace(&manifest_path, None)
        .expect_err("workspace open should fail when the parent directory is absent");
    ensure!(
        err.to_string().contains("Failed to open workspace"),
        "error should mention the workspace open failure but was {err}"
    );
    Ok(())
}

#[rstest]
fn from_path_uses_manifest_directory_for_caches() -> AnyResult<()> {
    let temp = tempdir()?;
    let workspace = temp.path().join("workspace");
    test_fs::create_dir_all(&workspace)?;
    let outside = temp.path().join("outside");
    test_fs::create_dir_all(&outside)?;
    let manifest_path = workspace.join("Netsukefile");

    let (url, server) = match http::spawn_http_server("workspace-body") {
        Ok(pair) => pair,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping from_path_uses_manifest_directory_for_caches: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let manifest_yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: fetch\n",
        "    vars:\n",
        "      url: \"{{ env('NETSUKE_MANIFEST_URL') }}\"\n",
        "    command: \"{{ fetch(url, cache=true) }}\"\n",
    );
    test_fs::write(&manifest_path, manifest_yaml)?;

    let manifest_url = url.clone();
    let env_reader: EnvReader = Arc::new(move |key| {
        if key == "NETSUKE_MANIFEST_URL" {
            Ok(manifest_url.clone())
        } else {
            Err(EnvReadError::NotPresent)
        }
    });

    let manifest = from_path_with_policy_and_env(
        &manifest_path,
        NetworkPolicy::default()
            .deny_all_hosts()
            .allow_hosts(["127.0.0.1", "localhost"])?
            .allow_scheme("http")?,
        &env_reader,
        None,
    )?;
    if let Err(err) = server.join() {
        return Err(anyhow!("join server thread panicked: {err:?}"));
    }

    let first_target = manifest.targets.first().context("target missing")?;
    match &first_target.recipe {
        Recipe::Command { command } => anyhow::ensure!(
            command.as_single() == Some("workspace-body"),
            "unexpected recipe output: {command:?}"
        ),
        other => anyhow::bail!("expected command recipe, got {other:?}"),
    }

    let parsed_url = Url::parse(&url).context("parse manifest URL")?;
    let cache_key = hash::sha256_hex(parsed_url.as_str().as_bytes());
    let cache_path = workspace.join(".netsuke").join("fetch").join(cache_key);
    anyhow::ensure!(
        cache_path.exists(),
        "cache file should be created inside the manifest workspace"
    );
    anyhow::ensure!(
        !outside.join(".netsuke").exists(),
        "outside working directory must not receive cache data"
    );

    Ok(())
}

/// Discovery queries must reject helpers that could cause side effects or
/// disclose host data before a catalogue is rendered.
#[rstest]
#[case::fetch("{{ fetch('https://example.invalid', cache=true) }}", "fetch")]
#[case::shell("{{ 'ignored' | shell('printf side-effect') }}", "shell")]
#[case::grep("{{ 'ignored' | grep('ignored') }}", "grep")]
#[case::env("{{ env('PATH') }}", "env")]
#[case::glob("{{ glob('*') }}", "glob")]
#[case::expanduser("{{ '~' | expanduser }}", "expanduser")]
#[case::contents("{{ 'secret.txt' | contents }}", "contents")]
#[case::realpath("{{ 'secret.txt' | realpath }}", "realpath")]
#[case::size("{{ 'secret.txt' | size }}", "size")]
#[case::linecount("{{ 'secret.txt' | linecount }}", "linecount")]
#[case::hash("{{ 'secret.txt' | hash }}", "hash")]
#[case::digest("{{ 'secret.txt' | digest }}", "digest")]
#[case::file_test("{{ 'secret.txt' is file }}", "file")]
#[case::which("{{ which('sh') }}", "which")]
#[case::command_available("{{ command_available('sh') }}", "command_available")]
fn manifest_query_rejects_restricted_template_helpers(
    #[case] expression: &str,
    #[case] helper: &str,
) -> AnyResult<()> {
    let temp = tempdir().context("create manifest-query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(temp.path().join("secret.txt"), QUERY_SECRET)?;
    let manifest = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: discovery\n",
            "    description: >-\n",
            "      {}\n",
            "    command: echo discovery\n",
        ),
        expression,
    );
    test_fs::write(&manifest_path, manifest)?;

    let error = from_path_for_manifest_query(&manifest_path, None)
        .expect_err("manifest query should reject restricted template helpers");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(helper)),
        "query should name its rejected helper: {error:?}"
    );
    ensure!(
        !error
            .chain()
            .any(|cause| cause.to_string().contains(QUERY_SECRET)),
        "a query error must not disclose local file contents: {error:?}"
    );
    ensure!(
        !temp.path().join(".netsuke").exists(),
        "a rejected query must not create a fetch cache"
    );
    Ok(())
}
#[test]
fn manifest_query_rejects_clock_dependent_template_helpers() -> AnyResult<()> {
    let temp = tempdir().context("create clock-free manifest-query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: discovery\n",
            "    description: \"{{ now() }}\"\n",
            "    command: echo discovery\n",
        ),
    )?;

    let error = from_path_for_manifest_query(&manifest_path, None)
        .expect_err("manifest query should reject the clock-dependent now helper");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("now is disabled")),
        "query should name the disabled now helper: {error:?}"
    );
    Ok(())
}

#[rstest]
fn manifest_query_keeps_inline_build_helpers_in_recipes() -> AnyResult<()> {
    let temp = tempdir().context("create recipe-only helper query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(
        &manifest_path,
        r#"netsuke_version: "1.0.0"
actions:
  - name: test
    description: Run tests with cargo-nextest or Cargo
    command: >-
      RUSTFLAGS='-D warnings'
      cargo {% if command_available("cargo-nextest") %}nextest run{% else %}test{% endif %}
      --all-targets --all-features
targets: []
"#,
    )?;

    let manifest = from_path_for_manifest_query(&manifest_path, None)?;
    let Some(action) = manifest.actions.first() else {
        anyhow::bail!("query fixture should retain its action");
    };
    let Recipe::Command { command } = &action.recipe else {
        anyhow::bail!("query fixture action should retain its command recipe");
    };
    ensure!(
        command
            .as_single()
            .is_some_and(|recipe| recipe.contains("command_available")),
        "query should leave the recipe helper unrendered: {command:?}"
    );
    ensure!(
        !temp.path().join(".netsuke").exists(),
        "query should not create a build-output directory"
    );
    Ok(())
}

#[test]
fn manifest_query_does_not_emit_expansion_telemetry() -> AnyResult<()> {
    let temp = tempdir().context("create telemetry-free query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: skipped-query-target\n",
            "    command: echo skipped\n",
            "    when: 'false'\n",
        ),
    )?;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let events = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            from_path_for_manifest_query(&manifest_path, None)?;
            Ok::<_, anyhow::Error>(captured.snapshot())
        })
    })?;

    ensure!(
        events.iter().all(
            |event| !event.contains("filtered manifest entry by when expression")
                && !event.contains("expanded manifest foreach and when directives")
        ),
        "manifest queries must not emit expansion telemetry: {events:?}"
    );
    let snapshot = snapshotter.snapshot().into_vec();
    for metric_name in [
        "netsuke_manifest_filtered_targets_total",
        "netsuke_manifest_filtered_actions_total",
        "netsuke_manifest_omitted_filtered_entries_total",
    ] {
        ensure!(
            snapshot
                .iter()
                .all(|(key, _, _, _)| key.key().name() != metric_name),
            "manifest queries must not emit {metric_name}: {snapshot:?}"
        );
    }
    Ok(())
}
const QUERY_SECRET: &str = "help-query-secret";
