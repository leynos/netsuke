//! Cucumber step implementations for stdlib path, file, network, and command
//! helpers.
//!
//! Sets up a temporary workspace, renders templates with stdlib registered,
//! and asserts outputs and errors. Provides HTTP fixtures for fetch scenarios
//! and compiles reusable command helpers for shell and grep coverage.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros reuse parameter identifiers for captures"
)]
use crate::CliWorld;
use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::{given, then, when};
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib::{self, StdlibConfig};
use std::ffi::OsStr;
use test_support::{
    command_helper::{compile_failure_helper, compile_uppercase_helper},
    env::set_var,
    hash,
};
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Iso8601};

const LINES_FIXTURE: &str = concat!(
    "one
", "two
", "three
",
);

#[derive(Debug, Clone)]
struct TemplatePath(Utf8PathBuf);

impl TemplatePath {
    fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl From<String> for TemplatePath {
    fn from(value: String) -> Self {
        Self(Utf8PathBuf::from(value))
    }
}

impl From<Utf8PathBuf> for TemplatePath {
    fn from(value: Utf8PathBuf) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
struct TemplateContent(String);

impl TemplateContent {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TemplateContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
struct FileContent(String);

impl FileContent {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<String> for FileContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
struct RelativePath(String);

impl RelativePath {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn to_path_buf(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(self.as_str())
    }
}

impl From<String> for RelativePath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub(crate) fn server_host(url: &str) -> Option<&str> {
    extract_host_from_url(url)
}

fn ensure_workspace(world: &mut CliWorld) -> Result<Utf8PathBuf> {
    if let Some(root) = &world.stdlib_root {
        return Ok(root.clone());
    }
    let temp = tempfile::tempdir().context("create stdlib workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("stdlib workspace path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    handle
        .write("file", b"data")
        .context("write stdlib file fixture")?;
    handle
        .write("lines.txt", LINES_FIXTURE.as_bytes())
        .context("write stdlib lines fixture")?;
    #[cfg(unix)]
    handle
        .symlink("file", "link")
        .context("create stdlib symlink fixture")?;
    #[cfg(not(unix))]
    handle
        .write("link", b"data")
        .context("write stdlib link fixture")?;
    world.temp = Some(temp);
    world.stdlib_root = Some(root.clone());
    Ok(root)
}

fn render_template_with_context(
    world: &mut CliWorld,
    template: &TemplateContent,
    ctx: Value,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let mut env = Environment::new();
    let workspace = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let config = StdlibConfig::new(workspace);
    let state = stdlib::register_with_config(&mut env, config);
    state.reset_impure();
    world.stdlib_state = Some(state.clone());
    let render = env.render_str(template.as_str(), ctx);
    match render {
        Ok(output) => {
            world.stdlib_output = Some(output);
            world.stdlib_error = None;
        }
        Err(err) => {
            world.stdlib_output = None;
            world.stdlib_error = Some(err.to_string());
        }
    }
    Ok(())
}

fn render_template(
    world: &mut CliWorld,
    template: &TemplateContent,
    path: &TemplatePath,
) -> Result<()> {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(world, template, ctx)
}

#[given("a stdlib workspace")]
fn stdlib_workspace(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    world.stdlib_root = Some(root);
    Ok(())
}

#[given("an uppercase stdlib command helper")]
fn uppercase_stdlib_command_helper(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper = compile_uppercase_helper(&handle, &root, "cmd_upper")
        .context("compile uppercase helper")?;
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
    Ok(())
}

#[given("a failing stdlib command helper")]
fn failing_stdlib_command_helper(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper =
        compile_failure_helper(&handle, &root, "cmd_fail").context("compile failing helper")?;
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
    Ok(())
}

/// Extracts the host portion from an HTTP or HTTPS URL.
///
/// Returns None if the URL doesn't have a valid http/https prefix or host.
fn extract_host_from_url(url: &str) -> Option<&str> {
    let addr = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    addr.split('/').next()
}

#[given(regex = r#"^an HTTP server returning "(.+)"$"#)]
fn http_server_returning(world: &mut CliWorld, body: String) -> Result<()> {
    world
        .start_http_server(body)
        .context("start stdlib HTTP fixture")?;
    Ok(())
}

#[given(regex = r#"^the stdlib file "(.+)" contains "(.+)"$"#)]
fn write_stdlib_file(world: &mut CliWorld, path: String, contents: String) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let relative_path = TemplatePath::from(path);
    let file_content = FileContent::from(contents);
    if let Some(parent) = relative_path
        .as_path()
        .parent()
        .filter(|p| !p.as_str().is_empty())
    {
        handle
            .create_dir_all(parent)
            .context("create stdlib fixture directories")?;
    }
    handle
        .write(relative_path.as_path(), file_content.as_bytes())
        .context("write stdlib fixture file")?;
    Ok(())
}

#[given("HOME points to the stdlib workspace root")]
fn home_points_to_stdlib_root(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let os_root = OsStr::new(root.as_str());
    let previous = set_var("HOME", os_root);
    world.env_vars.entry("HOME".into()).or_insert(previous);
    #[cfg(windows)]
    {
        let previous = set_var("USERPROFILE", os_root);
        world
            .env_vars
            .entry("USERPROFILE".into())
            .or_insert(previous);
    }
    world.stdlib_root = Some(root);
    Ok(())
}

fn resolve_template_path(root: &Utf8Path, raw: &RelativePath) -> TemplatePath {
    if raw.as_str().starts_with('~') {
        return TemplatePath::from(raw.as_str().to_owned());
    }
    let candidate = raw.to_path_buf();
    if candidate.is_absolute() {
        TemplatePath::from(candidate)
    } else {
        TemplatePath::from(root.join(candidate))
    }
}

#[when(regex = r#"^I render "(.+)" with stdlib path "(.+)"$"#)]
fn render_stdlib_template(
    world: &mut CliWorld,
    template_source: String,
    raw_path: String,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let template_content = TemplateContent::from(template_source);
    let relative_path = RelativePath::from(raw_path);
    let target = resolve_template_path(root.as_path(), &relative_path);
    render_template(world, &template_content, &target)
}

#[when(regex = r#"^I render the stdlib template "(.+)"$"#)]
fn render_stdlib_template_without_path(
    world: &mut CliWorld,
    template_source: String,
) -> Result<()> {
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context! {})
}

#[when(regex = r#"^I render "(.+)" with stdlib url$"#)]
fn render_stdlib_template_with_url(world: &mut CliWorld, template_source: String) -> Result<()> {
    let url = world
        .stdlib_url
        .clone()
        .context("expected stdlib HTTP server to be initialised")?;
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context!(url => url))
}

#[when(regex = r#"^I render the stdlib template "(.+)" using the stdlib command helper$"#)]
fn render_stdlib_template_with_command(
    world: &mut CliWorld,
    template_source: String,
) -> Result<()> {
    let command = world
        .stdlib_command
        .clone()
        .context("expected stdlib command helper to be compiled")?;
    let template_content = TemplateContent::from(template_source);
    render_template_with_context(world, &template_content, context!(cmd => command))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib output is "(.+)"$"#)]
fn assert_stdlib_output(world: &mut CliWorld, expected_output: String) -> Result<()> {
    let output = world
        .stdlib_output
        .as_ref()
        .context("expected stdlib output to be available")?;
    ensure!(
        output == &expected_output,
        "expected stdlib output '{expected_output}', got '{output}'"
    );
    Ok(())
}

fn stdlib_root_and_output(world: &CliWorld) -> Result<(&Utf8Path, &str)> {
    let root = world
        .stdlib_root
        .as_ref()
        .map(Utf8PathBuf::as_path)
        .context("expected stdlib workspace root")?;
    let output = world
        .stdlib_output
        .as_deref()
        .context("expected stdlib output")?;
    Ok((root, output))
}

fn stdlib_output(world: &CliWorld) -> Result<&str> {
    world
        .stdlib_output
        .as_deref()
        .context("expected stdlib output")
}

fn parse_iso_timestamp(raw: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Iso8601::DEFAULT)
        .with_context(|| format!("parse ISO8601 timestamp from '{raw}'"))
}

fn parse_expected_offset(raw: &str) -> Result<UtcOffset> {
    if raw.eq_ignore_ascii_case("z") {
        return Ok(UtcOffset::UTC);
    }

    let mut chars = raw.chars();
    let first = chars
        .next()
        .ok_or_else(|| anyhow!("unsupported offset format: {raw}"))?;
    let rest = chars.as_str();
    let (sign, rest) = match first {
        '+' => (1, rest),
        '-' => (-1, rest),
        _ => bail!("unsupported offset format: {raw}"),
    };

    let mut parts = rest.split(':');
    let hours: i8 = parts
        .next()
        .ok_or_else(|| anyhow!("offset missing hour component: {raw}"))?
        .parse()
        .with_context(|| format!("parse hour component from '{raw}'"))?;
    let minutes: i8 = parts.next().map_or(Ok(0), |value| {
        value
            .parse()
            .with_context(|| format!("parse minute component from '{raw}'"))
    })?;
    let seconds: i8 = parts.next().map_or(Ok(0), |value| {
        value
            .parse()
            .with_context(|| format!("parse second component from '{raw}'"))
    })?;

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds)
        .with_context(|| format!("offset components out of range in '{raw}'"))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib error contains "(.+)"$"#)]
fn assert_stdlib_error(world: &mut CliWorld, expected_fragment: String) -> Result<()> {
    let error = world
        .stdlib_error
        .as_ref()
        .context("expected stdlib error")?;
    ensure!(
        error.contains(&expected_fragment),
        "error `{error}` should contain `{expected_fragment}`",
    );
    Ok(())
}

#[then("the stdlib template is impure")]
fn assert_stdlib_impure(world: &mut CliWorld) -> Result<()> {
    let state = world
        .stdlib_state
        .as_ref()
        .context("stdlib state should be initialised")?;
    ensure!(state.is_impure(), "expected template to be impure");
    Ok(())
}

#[then("the stdlib template is pure")]
fn assert_stdlib_pure(world: &mut CliWorld) -> Result<()> {
    let state = world
        .stdlib_state
        .as_ref()
        .context("stdlib state should be initialised")?;
    ensure!(!state.is_impure(), "expected template to remain pure");
    Ok(())
}

#[then("the stdlib workspace contains the fetch cache for stdlib url")]
fn assert_fetch_cache_present(world: &mut CliWorld) -> Result<()> {
    let root = world
        .stdlib_root
        .as_ref()
        .context("expected stdlib workspace root")?;
    let url = world
        .stdlib_url
        .as_ref()
        .context("expected stdlib url for cache check")?;
    let key = hash::sha256_hex(url.as_bytes());
    let cache_path = root.join(".netsuke").join("fetch").join(key);
    ensure!(
        std::fs::metadata(cache_path.as_std_path()).is_ok(),
        "expected fetch cache at {cache_path}"
    );
    Ok(())
}

#[then("the stdlib output equals the workspace root")]
fn assert_stdlib_output_is_root(world: &mut CliWorld) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    ensure!(
        output == root.as_str(),
        "expected output to equal workspace root"
    );
    Ok(())
}

#[then(regex = r#"^the stdlib output is the workspace path "(.+)"$"#)]
fn assert_stdlib_output_is_workspace_path(
    world: &mut CliWorld,
    relative_path: String,
) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    let relative_path = RelativePath::from(relative_path);
    let expected = root.join(relative_path.to_path_buf());
    ensure!(
        output == expected.as_str(),
        "expected output '{}', got '{output}'",
        expected
    );
    Ok(())
}

#[then("the stdlib output is an ISO8601 UTC timestamp")]
fn assert_stdlib_output_is_utc_timestamp(world: &mut CliWorld) -> Result<()> {
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(output)?;
    let now = OffsetDateTime::now_utc();
    let delta = (now - parsed).abs();
    ensure!(
        delta <= Duration::seconds(5),
        "timestamp `{output}` should be within five seconds of now",
    );
    ensure!(
        parsed.offset() == UtcOffset::UTC,
        "timestamp `{output}` should have a UTC offset"
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib output offset is "(.+)"$"#)]
fn assert_stdlib_output_offset(world: &mut CliWorld, expected_offset_text: String) -> Result<()> {
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(output)?;
    let expected_offset = parse_expected_offset(&expected_offset_text)?;
    ensure!(
        parsed.offset() == expected_offset,
        "timestamp `{output}` offset {:?} did not match expected {expected_offset_text}",
        parsed.offset()
    );
    Ok(())
}
