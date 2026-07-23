"""Executable contracts for examples in the README and user's guide."""

from __future__ import annotations

import json
import shlex
import typing as typ

import pytest
from documentation_examples import (
    NetsukeRunner,
    documented_example,
    load_documented_examples,
    manifest_workspace,
    run_program,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

    from cmd_mox import CmdMox, Invocation

MANIFEST_IDS = (
    "readme-first-build-manifest",
    "guide-first-build-manifest",
    "guide-complete-manifest",
    "guide-foreach-manifest",
    "guide-macro-manifest",
    "guide-command-available-manifest",
    "guide-stdlib-manifest",
)
FIRST_RUN_CASES = (
    ("readme-first-build-manifest", "readme-first-build-commands"),
    ("guide-first-build-manifest", "guide-first-build-commands"),
)
SOURCE_INSTALL_IDS = ("readme-source-install", "guide-source-install")
SINGLE_EXAMPLE_IDS = (
    "guide-windows-help",
    "guide-cli-usage",
    "guide-project-anchor",
    "guide-utility-commands",
    "guide-project-config",
    "guide-output-streams",
    "guide-accessible-output",
    "guide-json-command",
    "guide-json-output",
)
EXPECTED_EXAMPLE_IDS = frozenset(
    (
        *MANIFEST_IDS,
        *(commands for _, commands in FIRST_RUN_CASES),
        *SOURCE_INSTALL_IDS,
        *SINGLE_EXAMPLE_IDS,
    )
)
REPOSITORY_EXAMPLES = (
    "examples/basic_c.yml",
    "examples/photo_edit.yml",
    "examples/visual_design.yml",
    "examples/website.yml",
    "examples/writing.yml",
    "examples/hello-world/Netsukefile",
)


def _stdout(result: object) -> str:
    value = getattr(result, "stdout", None)
    assert isinstance(value, str), "command should return captured stdout"
    return value


def _stderr(result: object) -> str:
    value = getattr(result, "stderr", None)
    assert isinstance(value, str), "command should return captured stderr"
    return value.replace("\u2068", "").replace("\u2069", "")


def _assert_success(result: object, context: str) -> None:
    assert getattr(result, "ok", False), (
        f"{context} should succeed; stdout:\n{_stdout(result)}"
        f"\nstderr:\n{_stderr(result)}"
    )


def _assert_default_edges_exist(ninja: str, context: str) -> None:
    defaults = (
        target
        for line in ninja.splitlines()
        if line.startswith("default ")
        for target in line.removeprefix("default ").split()
    )
    for target in defaults:
        assert any(
            line.startswith(f"build {target}:") for line in ninja.splitlines()
        ), f"{context} default {target!r} should have a generated build edge"


def _mock_ninja_build(cmd_mox: CmdMox, workspace: Path) -> None:
    def run_ninja(invocation: Invocation) -> tuple[str, str, int]:
        assert "-f" in invocation.args, "Netsuke should pass a generated Ninja file"
        (workspace / "hello.txt").write_text(
            "Hello from Netsuke!\n",
            encoding="utf-8",
        )
        return "", "", 0

    cmd_mox.mock("ninja").runs(run_ninja)


def test_every_fence_has_a_known_unique_identifier(repository: Path) -> None:
    """Fail when a fenced example lacks an executable test route."""
    examples = load_documented_examples(repository)
    actual = frozenset(example.identifier for example in examples)
    assert actual == EXPECTED_EXAMPLE_IDS


@pytest.mark.parametrize("example_id", MANIFEST_IDS)
def test_documented_manifest_generates_ninja(
    repository: Path,
    netsuke: NetsukeRunner,
    tmp_path: Path,
    example_id: str,
) -> None:
    """Compile each exact YAML fence in an isolated workspace."""
    workspace = manifest_workspace(repository, tmp_path, example_id)
    result = netsuke.run(workspace, "--progress", "false", "manifest", "-")

    _assert_success(result, example_id)
    ninja = _stdout(result)
    assert "rule " in ninja
    assert "build " in ninja
    _assert_default_edges_exist(ninja, example_id)


@pytest.mark.parametrize(("manifest_id", "commands_id"), FIRST_RUN_CASES)
def test_documented_first_run_flow_builds(
    repository: Path,
    netsuke: NetsukeRunner,
    cmd_mox: CmdMox,
    tmp_path: Path,
    manifest_id: str,
    commands_id: str,
) -> None:
    """Run both introductory manifests through a per-test Ninja mock."""
    commands = documented_example(repository, commands_id)
    assert commands.body == "netsuke\ncat hello.txt\n"
    workspace = manifest_workspace(repository, tmp_path, manifest_id)
    _mock_ninja_build(cmd_mox, workspace)

    result = netsuke.run(workspace)

    _assert_success(result, commands_id)
    assert "Build complete." in _stderr(result)
    assert (workspace / "hello.txt").read_text(encoding="utf-8") == (
        "Hello from Netsuke!\n"
    )


@pytest.mark.parametrize("example_id", SOURCE_INSTALL_IDS)
def test_source_install_commands_are_isolated(
    repository: Path,
    cmd_mox: CmdMox,
    tmp_path: Path,
    example_id: str,
) -> None:
    """Execute source-install commands through mocked Git and Cargo shims."""
    example = documented_example(repository, example_id)
    lines = example.body.splitlines()
    assert lines == [
        "git clone https://github.com/leynos/netsuke.git",
        "cd netsuke",
        "cargo install --path .",
    ]
    git_args = shlex.split(lines[0])
    cargo_args = shlex.split(lines[2])
    cmd_mox.mock("git").with_args(*git_args[1:]).returns()
    cmd_mox.mock("cargo").with_args(*cargo_args[1:]).returns()

    clone = run_program(tmp_path, git_args[0], *git_args[1:])
    _assert_success(clone, f"{example_id} clone")
    checkout = tmp_path / lines[1].removeprefix("cd ")
    checkout.mkdir()
    install = run_program(checkout, cargo_args[0], *cargo_args[1:])
    _assert_success(install, f"{example_id} install")


def test_windows_help_example_matches_release_contract(repository: Path) -> None:
    """Tie the PowerShell example to staged Windows help artefacts."""
    example = documented_example(repository, "guide-windows-help")
    assert example.body == "Get-Help Netsuke -Full\n"
    staging = (repository / ".github/release-staging.toml").read_text(encoding="utf-8")
    assert "Netsuke-help.xml" in staging
    assert "about_Netsuke.help.txt" in staging


def test_documented_cli_shape_matches_live_help(
    repository: Path,
    netsuke: NetsukeRunner,
    tmp_path: Path,
) -> None:
    """Compare the synopsis fence with live localized help."""
    example = documented_example(repository, "guide-cli-usage")
    assert example.body == (
        "netsuke [OPTIONS] [COMMAND]\nnetsuke [OPTIONS] build [TARGETS]...\n"
    )
    top_level = netsuke.run(tmp_path, "--locale", "en-US", "--help")
    build = netsuke.run(tmp_path, "--locale", "en-US", "build", "--help")

    _assert_success(top_level, "top-level help")
    _assert_success(build, "build help")
    assert "Usage: netsuke [OPTIONS] [COMMAND]" in _stdout(top_level)
    assert "Usage: netsuke build [OPTIONS] [TARGETS]..." in _stdout(build)


def test_directory_and_utility_commands_run(
    repository: Path,
    netsuke: NetsukeRunner,
    cmd_mox: CmdMox,
    tmp_path: Path,
) -> None:
    """Exercise the directory and utility command fences."""
    anchor = documented_example(repository, "guide-project-anchor")
    assert anchor.body == "netsuke --directory /path/to/project build\n"
    utility = documented_example(repository, "guide-utility-commands")
    assert utility.body == (
        "netsuke clean\n"
        "netsuke graph --output build.dot\n"
        "netsuke graph --html --output graph.html\n"
        "netsuke manifest -\n"
        "netsuke build --emit build.ninja\n"
    )
    workspace = manifest_workspace(
        repository,
        tmp_path,
        "guide-first-build-manifest",
    )
    ninja = cmd_mox.spy("ninja").returns().times_called(3)

    results = (
        netsuke.run(tmp_path, "--directory", str(workspace), "build"),
        netsuke.run(workspace, "clean"),
        netsuke.run(workspace, "graph", "--output", "build.dot"),
        netsuke.run(
            workspace,
            "graph",
            "--html",
            "--output",
            "graph.html",
        ),
        netsuke.run(workspace, "manifest", "-"),
        netsuke.run(workspace, "build", "--emit", "build.ninja"),
    )

    for result in results:
        _assert_success(result, "documented utility command")
    assert ninja.call_count == 3
    for output in ("build.dot", "graph.html", "build.ninja"):
        assert (workspace / output).is_file()


def test_project_configuration_is_accepted(
    repository: Path,
    netsuke: NetsukeRunner,
    tmp_path: Path,
) -> None:
    """Load the exact project configuration fence."""
    example = documented_example(repository, "guide-project-config")
    workspace = manifest_workspace(
        repository,
        tmp_path,
        "guide-first-build-manifest",
    )
    config = workspace / "example.toml"
    config.write_text(example.body, encoding="utf-8")

    result = netsuke.run(
        workspace,
        "--config",
        str(config),
        "--progress",
        "false",
        "manifest",
        "-",
    )

    _assert_success(result, "project configuration")


def test_output_stream_and_accessibility_examples(
    repository: Path,
    netsuke: NetsukeRunner,
    cmd_mox: CmdMox,
    tmp_path: Path,
) -> None:
    """Verify output routing and every documented accessible status line."""
    streams = documented_example(repository, "guide-output-streams")
    assert streams.body == (
        "netsuke graph > build.dot\n"
        "netsuke --progress false build\n"
        "netsuke manifest - > build.ninja\n"
    )
    workspace = manifest_workspace(
        repository,
        tmp_path,
        "guide-first-build-manifest",
    )
    cmd_mox.mock("ninja").returns().times(2)

    graph = netsuke.run(workspace, "graph")
    manifest = netsuke.run(workspace, "manifest", "-")
    quiet = netsuke.run(workspace, "--progress", "false", "build")
    accessible = netsuke.run(workspace, "--accessible", "true", "build")

    for result in (graph, manifest, quiet, accessible):
        _assert_success(result, "documented output command")
    assert "digraph" in _stdout(graph)
    assert "rule " in _stdout(manifest)
    assert "build " in _stdout(manifest)
    expected = documented_example(repository, "guide-accessible-output")
    for line in expected.body.splitlines():
        assert line in _stderr(accessible)


def test_json_diagnostic_matches_live_schema(
    repository: Path,
    netsuke: NetsukeRunner,
    tmp_path: Path,
) -> None:
    """Compare the documented JSON envelope with a live failure."""
    command = documented_example(repository, "guide-json-command")
    assert command.body == "netsuke --diag-json --file missing.yml build\n"
    expected = json.loads(documented_example(repository, "guide-json-output").body)

    result = netsuke.run(
        tmp_path,
        "--diag-json",
        "--file",
        "missing.yml",
        "build",
    )

    assert not result.ok
    assert _stdout(result) == ""
    assert json.loads(_stderr(result)) == expected


@pytest.mark.parametrize("relative_path", REPOSITORY_EXAMPLES)
def test_linked_repository_example_generates_ninja(
    repository: Path,
    netsuke: NetsukeRunner,
    tmp_path: Path,
    relative_path: str,
) -> None:
    """Compile each linked complete manifest in an isolated workspace."""
    source = repository / relative_path
    workspace = tmp_path / source.stem
    workspace.mkdir()
    (workspace / "Netsukefile").write_text(
        source.read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    result = netsuke.run(
        workspace,
        "--progress",
        "false",
        "manifest",
        "-",
    )

    _assert_success(result, relative_path)
    ninja = _stdout(result)
    assert "rule " in ninja
    assert "build " in ninja
    _assert_default_edges_exist(ninja, relative_path)
