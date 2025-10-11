//! Cucumber step implementations for stdlib path, file, network, and command
//! helpers.
//!
//! Sets up a temporary workspace, renders templates with stdlib registered,
//! and asserts outputs and errors. Provides HTTP fixtures for fetch scenarios
//! and compiles reusable command helpers for shell and grep coverage.
use crate::CliWorld;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::{given, then, when};
use minijinja::{Environment, context, value::Value};
use netsuke::stdlib;
use std::ffi::OsStr;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};
use test_support::{
    command_helper::{compile_failure_helper, compile_uppercase_helper},
    env::set_var,
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

pub(crate) fn spawn_http_server(body: String) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind http listener");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let bytes_read = stream.read(&mut buf).unwrap_or(0);
            if bytes_read > 0 {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    (url, handle)
}

fn ensure_workspace(world: &mut CliWorld) -> Utf8PathBuf {
    if let Some(root) = &world.stdlib_root {
        return root.clone();
    }
    let temp = tempfile::tempdir().expect("create stdlib workspace");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    handle.write("file", b"data").expect("write file");
    handle
        .write("lines.txt", LINES_FIXTURE.as_bytes())
        .expect("write lines fixture");
    #[cfg(unix)]
    handle.symlink("file", "link").expect("create symlink");
    #[cfg(not(unix))]
    handle.write("link", b"data").expect("write link fixture");
    world.temp = Some(temp);
    world.stdlib_root = Some(root.clone());
    root
}

fn render_template_with_context(world: &mut CliWorld, template: &TemplateContent, ctx: Value) {
    let mut env = Environment::new();
    let state = stdlib::register(&mut env);
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
}

fn render_template(world: &mut CliWorld, template: &TemplateContent, path: &TemplatePath) {
    let ctx = context!(path => path.as_path().as_str());
    render_template_with_context(world, template, ctx);
}

#[given("a stdlib workspace")]
fn stdlib_workspace(world: &mut CliWorld) {
    let root = ensure_workspace(world);
    world.stdlib_root = Some(root);
}

#[given("an uppercase stdlib command helper")]
fn uppercase_stdlib_command_helper(world: &mut CliWorld) {
    let root = ensure_workspace(world);
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    let helper = compile_uppercase_helper(&handle, &root, "cmd_upper");
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
}

#[given("a failing stdlib command helper")]
fn failing_stdlib_command_helper(world: &mut CliWorld) {
    let root = ensure_workspace(world);
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    let helper = compile_failure_helper(&handle, &root, "cmd_fail");
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
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
fn http_server_returning(world: &mut CliWorld, body: String) {
    if let Some(host) = world.stdlib_url.as_deref().and_then(extract_host_from_url) {
        let _ = TcpStream::connect(host);
    }
    world.start_http_server(body);
}

#[given(regex = r#"^the stdlib file "(.+)" contains "(.+)"$"#)]
fn write_stdlib_file(world: &mut CliWorld, path: String, contents: String) {
    let root = ensure_workspace(world);
    let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("open workspace");
    let relative_path = TemplatePath::from(path);
    let file_content = FileContent::from(contents);
    if let Some(parent) = relative_path
        .as_path()
        .parent()
        .filter(|p| !p.as_str().is_empty())
    {
        handle
            .create_dir_all(parent)
            .expect("create fixture directories");
    }
    handle
        .write(relative_path.as_path(), file_content.as_bytes())
        .expect("write stdlib file");
}

#[given("HOME points to the stdlib workspace root")]
fn home_points_to_stdlib_root(world: &mut CliWorld) {
    let root = ensure_workspace(world);
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
fn render_stdlib_template(world: &mut CliWorld, template: String, path: String) {
    let root = ensure_workspace(world);
    let template_content = TemplateContent::from(template);
    let relative_path = RelativePath::from(path);
    let target = resolve_template_path(root.as_path(), &relative_path);
    render_template(world, &template_content, &target);
}

#[when(regex = r#"^I render the stdlib template "(.+)"$"#)]
fn render_stdlib_template_without_path(world: &mut CliWorld, template: String) {
    let template_content = TemplateContent::from(template);
    render_template_with_context(world, &template_content, context! {});
}

#[when(regex = r#"^I render "(.+)" with stdlib url$"#)]
fn render_stdlib_template_with_url(world: &mut CliWorld, template: String) {
    let url = world
        .stdlib_url
        .clone()
        .expect("expected HTTP server to be initialised");
    let template_content = TemplateContent::from(template);
    render_template_with_context(world, &template_content, context!(url => url));
}

#[when(regex = r#"^I render the stdlib template "(.+)" using the stdlib command helper$"#)]
fn render_stdlib_template_with_command(world: &mut CliWorld, template: String) {
    let command = world
        .stdlib_command
        .clone()
        .expect("expected stdlib command helper to be compiled");
    let template_content = TemplateContent::from(template);
    render_template_with_context(world, &template_content, context!(cmd => command));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib output is "(.+)"$"#)]
fn assert_stdlib_output(world: &mut CliWorld, expected: String) {
    let output = world
        .stdlib_output
        .as_ref()
        .expect("expected stdlib output");
    assert_eq!(output, &expected);
}

fn stdlib_root_and_output(world: &CliWorld) -> (&Utf8Path, &str) {
    let root = world
        .stdlib_root
        .as_ref()
        .expect("expected stdlib workspace root")
        .as_path();
    let output = world
        .stdlib_output
        .as_deref()
        .expect("expected stdlib output");
    (root, output)
}

fn stdlib_output(world: &CliWorld) -> &str {
    world
        .stdlib_output
        .as_deref()
        .expect("expected stdlib output")
}

fn parse_iso_timestamp(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Iso8601::DEFAULT).expect("valid ISO8601 timestamp")
}

fn parse_expected_offset(raw: &str) -> UtcOffset {
    if raw.eq_ignore_ascii_case("z") {
        return UtcOffset::UTC;
    }

    let mut chars = raw.chars();
    let first = chars
        .next()
        .unwrap_or_else(|| panic!("unsupported offset format: {raw}"));
    let rest = chars.as_str();
    let (sign, rest) = match first {
        '+' => (1, rest),
        '-' => (-1, rest),
        _ => panic!("unsupported offset format: {raw}"),
    };

    let mut parts = rest.split(':');
    let hours: i8 = parts
        .next()
        .expect("hour component")
        .parse()
        .expect("valid hour");
    let minutes: i8 = parts
        .next()
        .map_or(0, |value| value.parse().expect("valid minute"));
    let seconds: i8 = parts
        .next()
        .map_or(0, |value| value.parse().expect("valid second"));

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds).expect("offset within range")
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib error contains "(.+)"$"#)]
fn assert_stdlib_error(world: &mut CliWorld, fragment: String) {
    let error = world.stdlib_error.as_ref().expect("expected stdlib error");
    assert!(
        error.contains(&fragment),
        "error `{error}` should contain `{fragment}`",
    );
}

#[then("the stdlib template is impure")]
fn assert_stdlib_impure(world: &mut CliWorld) {
    let state = world
        .stdlib_state
        .as_ref()
        .expect("stdlib state should be initialised");
    assert!(state.is_impure(), "expected template to be impure");
}

#[then("the stdlib template is pure")]
fn assert_stdlib_pure(world: &mut CliWorld) {
    let state = world
        .stdlib_state
        .as_ref()
        .expect("stdlib state should be initialised");
    assert!(!state.is_impure(), "expected template to remain pure");
}

#[then("the stdlib output equals the workspace root")]
fn assert_stdlib_output_is_root(world: &mut CliWorld) {
    let (root, output) = stdlib_root_and_output(world);
    assert_eq!(output, root.as_str());
}

#[then(regex = r#"^the stdlib output is the workspace path "(.+)"$"#)]
fn assert_stdlib_output_is_workspace_path(world: &mut CliWorld, relative: String) {
    let (root, output) = stdlib_root_and_output(world);
    let relative_path = RelativePath::from(relative);
    let expected = root.join(relative_path.to_path_buf());
    assert_eq!(output, expected.as_str());
}

#[then("the stdlib output is an ISO8601 UTC timestamp")]
fn assert_stdlib_output_is_utc_timestamp(world: &mut CliWorld) {
    let output = stdlib_output(world);
    let parsed = parse_iso_timestamp(output);
    let now = OffsetDateTime::now_utc();
    let delta = (now - parsed).abs();
    assert!(
        delta <= Duration::seconds(5),
        "timestamp `{output}` should be within five seconds of now",
    );
    assert_eq!(parsed.offset(), UtcOffset::UTC);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned capture arguments"
)]
#[then(regex = r#"^the stdlib output offset is "(.+)"$"#)]
fn assert_stdlib_output_offset(world: &mut CliWorld, expected: String) {
    let output = stdlib_output(world);
    let parsed = parse_iso_timestamp(output);
    let expected_offset = parse_expected_offset(&expected);
    assert_eq!(parsed.offset(), expected_offset);
}
