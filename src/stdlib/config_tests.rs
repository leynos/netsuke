//! Unit tests for [`StdlibConfig`] builders and validation.
use super::{
    DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES,
    DEFAULT_WHICH_CACHE_CAPACITY, StdlibConfig,
};
use crate::localization::{self, keys};
use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use std::env;

#[fixture]
fn workspace() -> Result<(Dir, Utf8PathBuf)> {
    let dir =
        Dir::open_ambient_dir(".", ambient_authority()).context("open workspace root fixture")?;
    let path = Utf8PathBuf::from_path_buf(
        env::current_dir().context("resolve cwd for workspace fixture")?,
    )
    .map_err(|path| anyhow!("cwd should be valid UTF-8: {path:?}"))?;
    Ok((dir, path))
}

#[fixture]
fn base_config(#[from(workspace)] workspace: Result<(Dir, Utf8PathBuf)>) -> Result<StdlibConfig> {
    let (dir, path) = workspace?;
    StdlibConfig::new(dir)
        .context("construct stdlib config")?
        .with_workspace_root_path(path)
        .context("record workspace root")
}

#[rstest]
#[case(Utf8Path::new(""), keys::STDLIB_FETCH_CACHE_EMPTY)]
#[case(Utf8Path::new("/cache"), keys::STDLIB_FETCH_CACHE_NOT_RELATIVE)]
#[case(Utf8Path::new("../escape"), keys::STDLIB_FETCH_CACHE_ESCAPES)]
fn validate_cache_relative_rejects_invalid_inputs(
    #[case] path: &Utf8Path,
    #[case] message_key: &'static str,
) {
    let err = StdlibConfig::validate_cache_relative(path).expect_err("invalid paths should fail");
    let expected = match message_key {
        keys::STDLIB_FETCH_CACHE_EMPTY => localization::message(message_key).to_string(),
        keys::STDLIB_FETCH_CACHE_NOT_RELATIVE | keys::STDLIB_FETCH_CACHE_ESCAPES => {
            localization::message(message_key)
                .with_arg("path", path.as_str())
                .to_string()
        }
        _ => panic!("unexpected message key {message_key}"),
    };
    assert_eq!(err.to_string(), expected);
}

#[rstest]
fn validate_cache_relative_accepts_workspace_relative_paths() {
    StdlibConfig::validate_cache_relative(Utf8Path::new("nested/cache"))
        .expect("relative path should be accepted");
}

#[rstest]
#[case::output(CommandLimitCase {
    builder: StdlibConfig::with_command_max_output_bytes,
    accessor: |cfg: &StdlibConfig| cfg.command_max_output_bytes,
    default_value: DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
    updated: 2_048,
    zero_err_key: keys::STDLIB_COMMAND_OUTPUT_LIMIT_POSITIVE,
})]
#[case::stream(CommandLimitCase {
    builder: StdlibConfig::with_command_max_stream_bytes,
    accessor: |cfg: &StdlibConfig| cfg.command_max_stream_bytes,
    default_value: DEFAULT_COMMAND_MAX_STREAM_BYTES,
    updated: 65_536,
    zero_err_key: keys::STDLIB_COMMAND_STREAM_LIMIT_POSITIVE,
})]
fn command_limit_builders_validate_and_update(
    #[from(base_config)] base_config_res: Result<StdlibConfig>,
    #[case] case: CommandLimitCase,
) -> Result<()> {
    let base_config = base_config_res?;
    let default_value = (case.accessor)(&base_config);
    ensure!(
        default_value == case.default_value,
        "default limit {default_value} did not match {}",
        case.default_value
    );

    let updated_config =
        (case.builder)(base_config.clone(), case.updated).context("positive limit")?;
    let updated_value = (case.accessor)(&updated_config);
    ensure!(
        updated_value == case.updated,
        "updated limit {updated_value} did not match {}",
        case.updated
    );

    let err = (case.builder)(base_config, 0).expect_err("zero-byte limits must be rejected");
    let expected = localization::message(case.zero_err_key).to_string();
    ensure!(
        err.to_string() == expected,
        "error '{err}' did not match '{expected}'"
    );
    Ok(())
}

struct CommandLimitCase {
    builder: fn(StdlibConfig, u64) -> anyhow::Result<StdlibConfig>,
    accessor: fn(&StdlibConfig) -> u64,
    default_value: u64,
    updated: u64,
    zero_err_key: &'static str,
}

#[rstest]
fn command_limits_propagate_into_components(base_config: Result<StdlibConfig>) -> Result<()> {
    let config = base_config?
        .with_command_max_output_bytes(4_096)
        .context("set capture limit")?
        .with_command_max_stream_bytes(131_072)
        .context("set streaming limit")?;
    let (_network, command) = config.into_components();
    ensure!(
        command.max_capture_bytes == 4_096,
        "capture limit {} did not match 4096",
        command.max_capture_bytes
    );
    ensure!(
        command.max_stream_bytes == 131_072,
        "stream limit {} did not match 131072",
        command.max_stream_bytes
    );
    Ok(())
}

#[rstest]
fn which_cache_capacity_validates_and_updates(
    #[from(base_config)] base_config_res: Result<StdlibConfig>,
) -> Result<()> {
    let base_config = base_config_res?;
    let default_capacity = base_config.which_cache_capacity().get();
    ensure!(
        default_capacity == DEFAULT_WHICH_CACHE_CAPACITY,
        "default capacity {default_capacity} did not match {DEFAULT_WHICH_CACHE_CAPACITY}"
    );

    let updated = base_config
        .clone()
        .with_which_cache_capacity(5)
        .context("positive capacity should be accepted")?;
    let updated_capacity = updated.which_cache_capacity().get();
    ensure!(
        updated_capacity == 5,
        "updated capacity {updated_capacity} did not match 5"
    );

    let err = base_config
        .with_which_cache_capacity(0)
        .expect_err("zero capacity must be rejected");
    let expected = localization::message(keys::STDLIB_WHICH_CACHE_CAPACITY_POSITIVE).to_string();
    ensure!(
        err.to_string() == expected,
        "error '{err}' did not match '{expected}'"
    );
    Ok(())
}

#[rstest]
#[case(vec![""], keys::STDLIB_SKIP_DIR_EMPTY)]
#[case(vec!["."], keys::STDLIB_SKIP_DIR_NAVIGATION)]
#[case(vec![".."], keys::STDLIB_SKIP_DIR_NAVIGATION)]
#[case(vec!["dir/name"], keys::STDLIB_SKIP_DIR_SEPARATOR)]
#[case(vec!["dir\\name"], keys::STDLIB_SKIP_DIR_SEPARATOR)]
fn workspace_skip_dirs_validate_inputs(
    base_config: Result<StdlibConfig>,
    #[case] entries: Vec<&str>,
    #[case] message_key: &'static str,
) -> Result<()> {
    let err = base_config?
        .with_workspace_skip_dirs(entries)
        .expect_err("invalid skip entries should error");
    let expected = localization::message(message_key).to_string();
    ensure!(
        err.to_string() == expected,
        "error '{err}' did not match '{expected}'"
    );
    Ok(())
}

#[rstest]
fn workspace_skip_dirs_override_defaults(base_config: Result<StdlibConfig>) -> Result<()> {
    let config = base_config?
        .with_workspace_skip_dirs(["build", ".cache"])
        .context("configure skip dirs")?;
    let skip_dirs = config.workspace_skip_dirs();
    ensure!(
        skip_dirs == ["build".to_owned(), ".cache".to_owned()],
        "skip dirs {skip_dirs:?} did not match [\"build\", \".cache\"]"
    );
    Ok(())
}
