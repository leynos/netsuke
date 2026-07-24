//! End-to-end contracts for user-facing build examples.

#![cfg(unix)]

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use documentation_examples::{assert_success, documented_example, manifest_workspace};
use rstest::rstest;
use std::path::Path;
use std::process::Command;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};
use test_support::{ninja::ninja_integration_workspace, write_exec};

fn executable_path(stub_directory: &Path) -> Result<String> {
    let host_path = std::env::var("PATH").context("read host PATH")?;
    Ok(format!("{}:{host_path}", stub_directory.display()))
}

fn write_stub(directory: &Path, name: &str, script: &str) -> Result<()> {
    let utf8_directory =
        Utf8Path::from_path(directory).context("stub directory should be UTF-8")?;
    let path = write_exec(utf8_directory, name)?;
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
    let stub_directory = workspace.path().join("bin");
    test_fs::create_dir(&stub_directory)?;
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
    let stub_directory = workspace.path().join("bin");
    test_fs::create_dir(&stub_directory)?;
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
