//! End-to-end contracts for user-facing build examples.

#![cfg(unix)]

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use documentation_examples::{assert_success, documented_example, manifest_workspace};
use rstest::rstest;
use rustix::fs::{Dev, FileType, Mode, mknodat};
use std::path::Path;
use std::process::Command;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};
use test_support::{ninja::ninja_integration_workspace, write_exec};

fn executable_path(stub_directory: &Utf8Path) -> Result<String> {
    let host_path = std::env::var("PATH").context("read host PATH")?;
    Ok(format!("{stub_directory}:{host_path}"))
}

fn write_stub(directory: &Utf8Path, name: &str, script: &str) -> Result<()> {
    let path = write_exec(directory, name)?;
    test_fs::write(path.as_std_path(), script).with_context(|| format!("write {name} stub"))?;
    Ok(())
}

fn run_build(workspace: &Path, args: &[&str], path: Option<&str>) -> Result<NetsukeRun> {
    let mut environment = vec![("NETSUKE_NINJA", "ninja")];
    if let Some(executable_path) = path {
        environment.push(("PATH", executable_path));
    }
    run_netsuke_in_with_env(workspace, args, &environment)
}

#[rstest]
#[case("readme-first-build-manifest", "readme-first-build-commands")]
#[case("guide-first-build-manifest", "guide-first-build-commands")]
fn documented_first_run_produces_the_reported_output(
    #[case] manifest_id: &str,
    #[case] commands_id: &str,
) -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let commands = documented_example(commands_id)?;
    ensure!(
        commands.body == "netsuke\ncat hello.txt\n",
        "{commands_id} should describe the executed flow"
    );
    let workspace = manifest_workspace(manifest_id)?;
    let run = run_build(workspace.path(), &[], None)?;
    assert_success(&run, commands_id)?;

    let output = Command::new("cat")
        .arg("hello.txt")
        .current_dir(workspace.path())
        .output()
        .context("execute documented cat command")?;
    ensure!(
        output.status.success(),
        "documented cat command should succeed"
    );
    ensure!(
        output.stdout == b"Hello from Netsuke!\n",
        "documented cat output should match the guide"
    );
    Ok(())
}

#[test]
fn configured_default_builds_the_first_run_target() -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let config = documented_example("guide-project-config")?;
    let config_path = workspace.path().join("example.toml");
    test_fs::write(&config_path, config.body).context("write documented config")?;
    let config_path_text = config_path
        .to_str()
        .context("temporary config path should be UTF-8")?;

    let run = run_build(workspace.path(), &["--config", config_path_text], None)?;
    assert_success(&run, "configured default build")?;
    ensure!(
        test_fs::read_to_string(workspace.path().join("hello.txt"))? == "Hello from Netsuke!\n",
        "configured default should build hello.txt"
    );
    Ok(())
}

#[test]
fn photo_edit_example_produces_declared_jpegs_and_gallery() -> Result<()> {
    let Ok(workspace) = ninja_integration_workspace() else {
        return Ok(());
    };
    test_fs::write(
        workspace.path().join("Netsukefile"),
        include_str!("../examples/photo_edit.yml"),
    )?;
    test_fs::create_dir_all(workspace.path().join("raw_photos"))?;
    for photo in ["portrait", "landscape"] {
        test_fs::write(
            workspace.path().join(format!("raw_photos/{photo}.CR2")),
            format!("{photo} pixels\n"),
        )?;
    }
    let stub_directory = Utf8PathBuf::from_path_buf(workspace.path().join("bin"))
        .map_err(|_| anyhow::anyhow!("stub directory should be UTF-8"))?;
    test_fs::create_dir(stub_directory.as_std_path())?;
    write_stub(
        &stub_directory,
        "darktable-cli",
        "#!/bin/sh\nset -eu\nmkdir -p \"$(dirname \"$2\")\"\ncp \"$1\" \"$2\"\n",
    )?;
    write_stub(
        &stub_directory,
        "make-gallery",
        "#!/bin/sh\nset -eu\nprintf '<html>%s</html>\\n' \"$1\"\n",
    )?;
    let path = executable_path(&stub_directory)?;

    let run = run_build(workspace.path(), &[], Some(&path))?;
    assert_success(&run, "photo edit example")?;
    for output in [
        "processed/portrait.jpg",
        "processed/landscape.jpg",
        "processed/gallery.html",
    ] {
        ensure!(
            test_fs::exists(workspace.path().join(output)),
            "photo edit example should produce {output}"
        );
    }
    Ok(())
}

#[test]
fn writing_example_combines_chapters_into_the_declared_pdf() -> Result<()> {
    let Ok(workspace) = ninja_integration_workspace() else {
        return Ok(());
    };
    test_fs::write(
        workspace.path().join("Netsukefile"),
        include_str!("../examples/writing.yml"),
    )?;
    test_fs::create_dir_all(workspace.path().join("chapters"))?;
    test_fs::write(
        workspace.path().join("chapters/introduction.md"),
        "Introduction marker\n",
    )?;
    test_fs::write(
        workspace.path().join("chapters/architecture.md"),
        "Architecture marker\n",
    )?;
    let stub_directory = Utf8PathBuf::from_path_buf(workspace.path().join("bin"))
        .map_err(|_| anyhow::anyhow!("stub directory should be UTF-8"))?;
    test_fs::create_dir(stub_directory.as_std_path())?;
    write_stub(
        &stub_directory,
        "pandoc",
        concat!(
            "#!/bin/sh\nset -eu\noutput=\ninputs=\n",
            "while [ \"$#\" -gt 0 ]; do\n",
            "  case \"$1\" in\n",
            "    -o) output=$2; shift 2 ;;\n",
            "    -*) shift ;;\n",
            "    *) inputs=\"$inputs $1\"; shift ;;\n",
            "  esac\n",
            "done\n",
            ": > \"$output\"\n",
            "for input in $inputs; do [ -f \"$input\" ] && cat \"$input\" >> \"$output\"; done\n"
        ),
    )?;
    write_stub(
        &stub_directory,
        "latexmk",
        concat!(
            "#!/bin/sh\nset -eu\ninput=\n",
            "for argument in \"$@\"; do\n",
            "  case \"$argument\" in *.tex) input=$argument ;; esac\n",
            "done\n",
            "cp \"$input\" build/book.pdf\n"
        ),
    )?;
    let path = executable_path(&stub_directory)?;

    let run = run_build(workspace.path(), &[], Some(&path))?;
    assert_success(&run, "writing example")?;
    let book_tex = test_fs::read_to_string(workspace.path().join("build/book.tex"))?;
    ensure!(
        book_tex.contains("Introduction marker") && book_tex.contains("Architecture marker"),
        "combined book.tex should preserve both chapters"
    );
    ensure!(
        test_fs::exists(workspace.path().join("build/book.pdf")),
        "writing example should produce build/book.pdf"
    );
    Ok(())
}

#[test]
fn stdlib_path_and_collection_example_reports_fixture_values() -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let workspace = manifest_workspace("stdlib-path-and-collection-manifest")?;
    test_fs::create_dir(workspace.path().join("fixtures"))?;
    test_fs::write(workspace.path().join("fixtures/message.txt"), "alpha")?;
    let root_path = Utf8PathBuf::from_path_buf(workspace.path().to_path_buf())
        .map_err(|_| anyhow::anyhow!("path example workspace should be UTF-8"))?;
    let root = Dir::open_ambient_dir(&root_path, ambient_authority())?;
    root.symlink("message.txt", "fixtures/message-link")?;
    let home = workspace.path().to_string_lossy().into_owned();
    let run = run_netsuke_in_with_env(
        workspace.path(),
        &[],
        &[("NETSUKE_NINJA", "ninja"), ("HOME", home.as_str())],
    )?;
    assert_success(&run, "stdlib path and collection example")?;

    let expected = format!(
        concat!(
            "report.tmp\nfixtures\nfixtures/report.txt\nreport.tmp\nfixtures/message.txt\n",
            "{}/notes.txt\nalpha\n5\n1\n",
            "8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8\n",
            "8ed3f6ad\nalpha,beta\n1,2,3\n2\n"
        ),
        workspace.path().display(),
    );
    let output = test_fs::read_to_string(workspace.path().join("stdlib-paths.txt"))?;
    ensure!(
        output == expected,
        "stdlib path output mismatch; expected {expected:?}, got {output:?}"
    );
    Ok(())
}

#[test]
fn stdlib_file_test_example_identifies_unix_fixture_types() -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let workspace = manifest_workspace("stdlib-file-tests-manifest")?;
    test_fs::create_dir(workspace.path().join("fixtures"))?;
    test_fs::write(workspace.path().join("fixtures/message.txt"), "alpha")?;
    let root_path = Utf8PathBuf::from_path_buf(workspace.path().to_path_buf())
        .map_err(|_| anyhow::anyhow!("file-test workspace should be UTF-8"))?;
    let root = Dir::open_ambient_dir(&root_path, ambient_authority())?;
    root.symlink("message.txt", "fixtures/message-link")?;
    let fixtures = root.open_dir("fixtures")?;
    mknodat(
        &fixtures,
        "events.pipe",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )?;

    let run = run_build(workspace.path(), &[], None)?;
    assert_success(&run, "stdlib file-test example")?;
    ensure!(
        test_fs::read_to_string(workspace.path().join("stdlib-file-tests.txt"))?
            == "true\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\n",
        "stdlib file tests should report the documented Unix fixture types"
    );
    Ok(())
}

#[test]
fn stdlib_time_example_emits_timestamp_and_duration() -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let workspace = manifest_workspace("stdlib-time-manifest")?;
    let run = run_build(workspace.path(), &[], None)?;
    assert_success(&run, "stdlib time example")?;
    let output = test_fs::read_to_string(workspace.path().join("stdlib-time.txt"))?;
    let mut lines = output.lines();
    let timestamp = lines
        .next()
        .context("time example should emit a timestamp")?;
    ensure!(
        timestamp.contains('T') && timestamp.ends_with("+02:00"),
        "time example should preserve the documented +02:00 offset: {timestamp}"
    );
    ensure!(
        lines.next() == Some("P1DT2H30M") && lines.next().is_none(),
        "time example should emit the documented duration: {output}"
    );
    Ok(())
}

#[test]
fn stdlib_host_context_example_uses_controlled_process_state() -> Result<()> {
    let Ok(_ninja_probe) = ninja_integration_workspace() else {
        return Ok(());
    };
    let workspace = manifest_workspace("stdlib-host-context-manifest")?;
    test_fs::create_dir(workspace.path().join("fixtures"))?;
    test_fs::write(workspace.path().join("fixtures/a.txt"), "a")?;
    test_fs::write(workspace.path().join("fixtures/b.txt"), "b")?;
    let stub_directory = Utf8PathBuf::from_path_buf(workspace.path().join("bin"))
        .map_err(|_| anyhow::anyhow!("stub directory should be UTF-8"))?;
    test_fs::create_dir(stub_directory.as_std_path())?;
    write_stub(
        &stub_directory,
        "uppercase",
        "#!/bin/sh\nset -eu\ntr '[:lower:]' '[:upper:]'\n",
    )?;
    write_stub(
        &stub_directory,
        "grep",
        concat!(
            "#!/bin/sh\nset -eu\npattern=$1\n",
            "while IFS= read -r line; do\n",
            "  [ \"$line\" = \"$pattern\" ] && printf '%s\\n' \"$line\" || :\n",
            "done\n"
        ),
    )?;
    write_exec(&stub_directory, "guide-tool")?;
    let path = executable_path(&stub_directory)?;
    let run = run_netsuke_in_with_env(
        workspace.path(),
        &[],
        &[
            ("NETSUKE_NINJA", "ninja"),
            ("PATH", path.as_str()),
            ("NETSUKE_STDLIB_TOKEN", "from-env"),
        ],
    )?;
    assert_success(&run, "stdlib host-context example")?;
    let output = test_fs::read_to_string(workspace.path().join("stdlib-host-context.txt"))?;
    ensure!(
        output
            == format!(
                "HELLO\nbeta\n{stub_directory}/guide-tool\ntrue\nfrom-env\nfixtures/a.txt,fixtures/b.txt\n"
            ),
        "stdlib host-context output should use only controlled fixtures: {output}"
    );
    Ok(())
}
