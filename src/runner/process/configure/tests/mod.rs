//! Unit tests for Ninja command configuration without spawning Ninja.

use super::*;
use crate::runner::{NinjaJobCount, StderrMode};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::time::Duration;
use tempfile::{NamedTempFile, TempPath};

mod temp_file_retry;

/// Number of create attempts made before a persistent denial is reported.
const TEMP_FILE_ATTEMPTS: u32 = 5;

/// Fixed backoffs between permission-denied attempts, in milliseconds.
const TEMP_FILE_RETRY_DELAYS_MILLIS: [u64; 4] = [10, 20, 40, 80];

/// Create the temporary build file that the configuration cases consume.
#[fixture]
fn temp_file() -> Result<NamedTempFile> {
    temp_file_with(NamedTempFile::new, report_temp_file_retry)
}

/// Create a temporary build file, retrying transient permission denials.
///
/// The default `%TEMP%` directory is kept: the retry exists because that
/// shared directory is the contended resource, so pointing the fixture
/// elsewhere would stop exercising the condition it must survive. See
/// "Windows temp-file fixture retries" in `docs/developers-guide.md`.
///
/// # Errors
///
/// Returns the first error that is not a permission denial, or the final
/// permission denial once the attempt budget is spent, with context naming the
/// operation. The original [`io::Error`] stays the root cause.
fn temp_file_with(
    create: impl FnMut() -> io::Result<NamedTempFile>,
    before_retry: impl FnMut(&io::Error, u32, Duration),
) -> Result<NamedTempFile> {
    temp_file_retry::retry_permission_denied(create, before_retry).map_err(|error| {
        let context = if error.kind() == io::ErrorKind::PermissionDenied {
            format!("create a temporary build file after {TEMP_FILE_ATTEMPTS} attempts")
        } else {
            String::from("create a temporary build file")
        };
        anyhow::Error::new(error).context(context)
    })
}

/// Announce a permission-denied retry on standard error, then back off.
///
/// The cases that consume the fixture carry `success-output = "immediate"` in
/// `.config/nextest.toml`, so this line reaches the Windows Test step log on a
/// passing run rather than being inferred from a pass.
fn report_temp_file_retry(error: &io::Error, attempt: u32, delay: Duration) {
    // Losing the diagnostic channel must not turn a recoverable denial into a
    // test failure; the retry, not the announcement, is what protects the test.
    let _write_result = writeln!(
        io::stderr().lock(),
        "temp_file retry: attempt {attempt}/{TEMP_FILE_ATTEMPTS} failed with {error}; \
         retrying in {} ms",
        delay.as_millis()
    );
    std::thread::sleep(delay);
}

#[fixture]
fn options() -> io::Result<NinjaProcessOptions> {
    Ok(NinjaProcessOptions {
        jobs: Some(NinjaJobCount::try_new(4)?),
        ..NinjaProcessOptions::default()
    })
}

#[fixture]
fn env() -> CommandEnv {
    CommandEnv::inherit()
}

fn command_arguments(cmd: &Command) -> Vec<OsString> {
    cmd.get_args().map(OsStr::to_os_string).collect()
}

fn expected_base_arguments(build_file: &Utf8Path) -> Result<Vec<OsString>> {
    Ok(vec![
        OsString::from("-j"),
        OsString::from("4"),
        OsString::from("-f"),
        canonicalize_utf8_path(build_file)?
            .into_std_path_buf()
            .into_os_string(),
    ])
}

/// Convert the fixture's temporary file into a UTF-8 build-file path.
///
/// The returned [`TempPath`] must stay alive for as long as the path is used:
/// dropping it deletes the file that command configuration canonicalizes.
fn utf8_temp_file(temp_file: Result<NamedTempFile>) -> Result<(TempPath, Utf8PathBuf)> {
    let temp_build_file = temp_file?.into_temp_path();
    let path = Utf8PathBuf::from_path_buf(temp_build_file.to_path_buf())
        .map_err(|path| anyhow::anyhow!("tempfile path is not UTF-8: {}", path.display()))?;
    Ok((temp_build_file, path))
}

/// The request shape from which a configured Ninja command is built.
#[derive(Clone, Copy)]
enum NinjaInvocation<'a> {
    /// A build command carrying its target operands.
    Build(&'a [String]),
    /// A tool command naming the tool to run.
    Tool(&'a str),
}

/// Configure a Ninja command and return the arguments it produced.
fn configured_arguments(
    build_file: &Utf8Path,
    options: &NinjaProcessOptions,
    env: &CommandEnv,
    invocation: NinjaInvocation<'_>,
) -> Result<Vec<OsString>> {
    let mut cmd = Command::new("ninja");
    match invocation {
        NinjaInvocation::Build(target_names) => {
            let targets = super::super::BuildTargets::new(target_names);
            let request = NinjaBuildRequest {
                program: Utf8Path::new("ninja"),
                options,
                build_file,
                targets: &targets,
                env,
                stderr_mode: StderrMode::Forward,
            };
            configure_ninja_build_command(&mut cmd, &request)?;
        }
        NinjaInvocation::Tool(tool) => {
            let request = NinjaToolRequest {
                program: Utf8Path::new("ninja"),
                options,
                build_file,
                tool,
                env,
                stderr_mode: StderrMode::Forward,
            };
            configure_ninja_tool_command(&mut cmd, &request)?;
        }
    }
    Ok(command_arguments(&cmd))
}

#[rstest]
fn build_configuration_preserves_argument_order(
    temp_file: Result<NamedTempFile>,
    options: io::Result<NinjaProcessOptions>,
    env: CommandEnv,
) -> Result<()> {
    let (_temp_build_file, build_file) = utf8_temp_file(temp_file)?;
    let actual = configured_arguments(
        &build_file,
        &options?,
        &env,
        NinjaInvocation::Build(&[String::from("default")]),
    )?;
    let mut expected = expected_base_arguments(&build_file)?;
    expected.extend([OsString::from("--"), OsString::from("default")]);
    ensure!(
        actual == expected,
        "build command argument order changed: {actual:?}"
    );
    Ok(())
}

#[rstest]
fn build_configuration_terminates_multi_target_operands(
    temp_file: Result<NamedTempFile>,
    options: io::Result<NinjaProcessOptions>,
    env: CommandEnv,
) -> Result<()> {
    let (_temp_build_file, build_file) = utf8_temp_file(temp_file)?;
    let actual = configured_arguments(
        &build_file,
        &options?,
        &env,
        NinjaInvocation::Build(&[String::from("first"), String::from("second")]),
    )?;
    let base = expected_base_arguments(&build_file)?;
    ensure!(
        actual.get(..base.len()) == Some(base.as_slice()),
        "multi-target build command must preserve the base arguments: {actual:?}"
    );
    ensure!(
        actual.get(base.len()..)
            == Some(
                [
                    OsString::from("--"),
                    OsString::from("first"),
                    OsString::from("second"),
                ]
                .as_slice()
            ),
        "multi-target build command must terminate operands with --: {actual:?}"
    );
    Ok(())
}

#[test]
fn build_configuration_uses_uncanonicalized_missing_build_file() -> Result<()> {
    let temporary_directory = tempfile::tempdir().context("create temporary directory")?;
    let utf8_temporary_directory =
        Utf8PathBuf::from_path_buf(temporary_directory.path().to_path_buf()).map_err(|path| {
            anyhow::anyhow!("temporary directory is not UTF-8: {}", path.display())
        })?;
    let build_file = utf8_temporary_directory.join("missing-build.ninja");
    let options = NinjaProcessOptions::default();
    let target_names = Vec::new();
    let targets = super::super::BuildTargets::new(&target_names);
    let env = CommandEnv::inherit();
    let request = NinjaBuildRequest {
        program: Utf8Path::new("ninja"),
        options: &options,
        build_file: &build_file,
        targets: &targets,
        env: &env,
        stderr_mode: StderrMode::Forward,
    };
    let mut cmd = Command::new("ninja");

    ensure!(
        canonicalize_utf8_path(&build_file).is_err(),
        "the missing build file must not canonicalize"
    );
    configure_ninja_build_command(&mut cmd, &request)?;

    let expected = vec![
        OsString::from("-f"),
        build_file.into_std_path_buf().into_os_string(),
    ];
    let actual = command_arguments(&cmd);
    ensure!(
        actual == expected,
        "missing build file should be passed unchanged after -f: {actual:?}"
    );
    ensure!(
        !actual.iter().any(|argument| argument == "--"),
        "no-target build command must not append an option terminator: {actual:?}"
    );
    Ok(())
}

#[rstest]
fn tool_configuration_preserves_argument_order(
    temp_file: Result<NamedTempFile>,
    options: io::Result<NinjaProcessOptions>,
    env: CommandEnv,
) -> Result<()> {
    let (_temp_build_file, build_file) = utf8_temp_file(temp_file)?;
    let actual =
        configured_arguments(&build_file, &options?, &env, NinjaInvocation::Tool("clean"))?;
    let base = expected_base_arguments(&build_file)?;
    ensure!(
        actual.get(..base.len()) == Some(base.as_slice()),
        "tool command must preserve the base arguments: {actual:?}"
    );
    ensure!(
        actual.get(base.len()..)
            == Some([OsString::from("-t"), OsString::from("clean")].as_slice()),
        "tool command must append the tool selector: {actual:?}"
    );
    Ok(())
}

#[test]
fn temp_file_with_reports_a_persistent_denial_with_operation_context() -> Result<()> {
    let mut calls = 0_u32;
    let mut retries = 0_u32;
    let error = temp_file_with(
        || {
            calls += 1;
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "persistent denial",
            ))
        },
        |_, _, _| retries += 1,
    )
    .err()
    .ok_or_else(|| anyhow::anyhow!("a persistent permission denial must fail the fixture"))?;
    ensure!(
        calls == TEMP_FILE_ATTEMPTS,
        "the fixture must attempt {TEMP_FILE_ATTEMPTS} creates; attempted {calls} times"
    );
    ensure!(
        retries == 4,
        "the fixture must announce the four retryable attempts; announced {retries} times"
    );
    let rendered = format!("{error:#}");
    ensure!(
        rendered.contains("create a temporary build file"),
        "the context must name the operation: {rendered}"
    );
    ensure!(
        rendered.contains(&format!("after {TEMP_FILE_ATTEMPTS} attempts")),
        "the context must record the attempt count: {rendered}"
    );
    let root = error
        .downcast_ref::<io::Error>()
        .ok_or_else(|| anyhow::anyhow!("the root cause must remain an io::Error: {rendered}"))?;
    ensure!(
        root.kind() == io::ErrorKind::PermissionDenied,
        "the root error must stay a permission denial: {root:?}"
    );
    ensure!(
        root.to_string() == "persistent denial",
        "the root error must be preserved: {root}"
    );
    Ok(())
}

#[rstest]
fn temp_file_fixture_creates_a_real_file(temp_file: Result<NamedTempFile>) -> Result<()> {
    let file = temp_file?;
    ensure!(
        file.path().exists(),
        "the fixture must create a real temporary file: {}",
        file.path().display()
    );
    Ok(())
}
