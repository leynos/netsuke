//! Windows-only command filter tests.

use anyhow::{anyhow, ensure, Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::context;
use rstest::{fixture, rstest};
use std::ffi::OsString;
use std::fs;
use tempfile::tempdir;
use test_support::command_helper::compile_rust_helper;

use super::{StdlibConfig, fallible, streaming_match_payload};
use super::super::support::{EnvLock, EnvVarGuard};

#[fixture]
fn env_lock() -> EnvLock {
    EnvLock::acquire()
}

const GREP_STUB: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut args: Vec<String> = std::env::args().skip(1).collect();\n",
    "    let pattern = args.pop().expect(\"pattern\");\n",
    "    let mut input = String::new();\n",
    "    io::stdin().read_to_string(&mut input).expect(\"stdin\");\n",
    "    if pattern == \"^line2\" && input.contains(\"line2\") {\n",
    "        print!(\"line2\\n\");\n",
    "    } else {\n",
    "        eprintln!(\"pattern:{pattern} input:{input}\", pattern = pattern, input = input);\n",
    "        std::process::exit(1);\n",
    "    }\n",
    "}\n",
);

const GREP_STREAM_STUB: &str = concat!(
    "use std::io::{self, Write};\n",
    "fn main() {\n",
    "    let pattern = std::env::args().last().expect(\"pattern\");\n",
    "    if pattern != \"match\" {\n",
    "        std::process::exit(1);\n",
    "    }\n",
    "    let mut out = io::stdout();\n",
    "    let line = b\"match\\n\";\n",
    "    for _ in 0..32_768 {\n",
    "        out.write_all(line).expect(\"stdout\");\n",
    "    }\n",
    "}\n",
);

const ARGS_STUB: &str = concat!(
    "fn main() {\n",
    "    let mut args = std::env::args().skip(1);\n",
    "    if let Some(arg) = args.next() {\n",
    "        print!(\"{arg}\", arg = arg);\n",
    "    }\n",
    "}\n",
);

fn windows_command_setup(
    tempdir_context: &'static str,
    root_context: &'static str,
    dir_context: &'static str,
    compile_context: &'static str,
    helper_name: &str,
    helper_source: &str,
) -> Result<(tempfile::TempDir, EnvVarGuard, Utf8PathBuf)> {
    let temp = tempdir().context(tempdir_context)?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("{root_context}: {path:?}"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).context(dir_context)?;
    let helper = compile_rust_helper(&dir, &root, helper_name, helper_source)
        .with_context(|| compile_context.to_string())?;

    let mut path_value = OsString::from(root.as_str());
    path_value.push(";");
    path_value.push(std::env::var_os("PATH").unwrap_or_default());
    let path_guard = EnvVarGuard::set("PATH", &path_value);

    Ok((temp, path_guard, helper))
}

#[rstest]
fn grep_on_windows_bypasses_shell(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let (_temp, _path, _helper) = windows_command_setup(
        "create windows grep tempdir",
        "windows grep root is not valid UTF-8",
        "open windows grep temp dir",
        "compile grep helper for windows tests",
        "grep",
        GREP_STUB,
    )?;

    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_win",
        r#"{{ 'line1
line2
' | grep('^line2') | trim }}"#,
    )?;
    let template = env
        .get_template("grep_win")
        .context("fetch template 'grep_win'")?;
    let rendered = template
        .render(context! {})
        .context("render windows grep template")?;
    ensure!(rendered == "line2", "expected 'line2' but rendered {rendered}");
    ensure!(state.is_impure(), "grep should mark template impure");
    Ok(())
}

#[rstest]
fn grep_streams_large_output_on_windows(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let (_temp, _path, _helper) = windows_command_setup(
        "create windows grep stream tempdir",
        "windows grep root is not valid UTF-8",
        "open windows grep stream temp dir",
        "compile streaming grep helper for windows tests",
        "grep",
        GREP_STREAM_STUB,
    )?;

    let config = StdlibConfig::default()
        .with_command_max_output_bytes(512)?
        .with_command_max_stream_bytes(200_000)?;
    let (mut env, mut state) = fallible::stdlib_env_with_config(config)?;
    state.reset_impure();
    fallible::register_template(
        &mut env,
        "grep_win_stream",
        "{{ text | grep('match', none, {'mode': 'tempfile'}) }}",
    )?;
    let template = env
        .get_template("grep_win_stream")
        .context("fetch template 'grep_win_stream'")?;
    let payload = streaming_match_payload();
    let rendered = template
        .render(context!(text => payload.clone()))
        .context("render windows grep streaming template")?;
    ensure!(state.is_impure(), "grep streaming should mark template impure");
    let path = camino::Utf8Path::new(rendered.as_str());
    let metadata = fs::metadata(path.as_std_path())
        .with_context(|| format!("stat streamed windows grep output {}", path))?;
    ensure!(
        metadata.len() >= payload.len() as u64,
        "streamed grep output should retain payload size"
    );
    let contents = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("read streamed windows grep output {}", path))?;
    ensure!(
        contents == payload,
        "streamed grep file should contain the helper payload"
    );
    Ok(())
}

#[rstest]
fn shell_preserves_cmd_meta_characters(env_lock: EnvLock) -> Result<()> {
    let _lock = env_lock;
    let (_temp, _path, exe) = windows_command_setup(
        "create windows shell tempdir",
        "windows shell root is not valid UTF-8",
        "open windows shell temp dir",
        "compile echo_args helper for windows tests",
        "echo_args",
        ARGS_STUB,
    )?;

    let command = format!("\"{}\" \"literal %%^!\"", exe);
    let (mut env, mut state) = fallible::stdlib_env_with_state()?;
    state.reset_impure();
    fallible::register_template(&mut env, "shell_meta", "{{ '' | shell(cmd) }}")?;
    let template = env
        .get_template("shell_meta")
        .context("fetch template 'shell_meta'")?;
    let rendered = template
        .render(context!(cmd => command))
        .context("render shell meta template")?;
    ensure!(rendered.trim() == "literal %^!", "expected literal %^! but rendered {rendered}");
    ensure!(
        state.is_impure(),
        "shell filter should mark template impure"
    );
    Ok(())
}
