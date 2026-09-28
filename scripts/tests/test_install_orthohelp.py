"""Test cache probing and prebuilt cargo-orthohelp installation."""

import typing as typ

import install_orthohelp as installer
import pytest

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)
VERSION = "0.9.1"


def test_matching_cached_version_skips_install(
    cmd_mox: CmdMox,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A cache hit prints the existing message and never runs binstall."""
    probe = (
        cmd_mox
        .mock("cargo-orthohelp")
        .with_args("--version")
        .returns(exit_code=0, stdout=f"cargo-orthohelp {VERSION}\n")
    )
    monkeypatch.setenv("INPUT_VERSION", VERSION)

    installer.app([], result_action="return_value")

    assert len(probe.invocations) == 1, "the cached version probe must run once"
    assert (
        capsys.readouterr().out
        == f"cargo-orthohelp {VERSION} restored from the cache\n"
    ), "a cache hit keeps the existing workflow message"


def test_cache_miss_uses_only_the_non_compiling_binstall_strategy(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A mismatched cache invokes binstall with the release asset version."""
    cmd_mox.mock("cargo-orthohelp").with_args("--version").returns(
        exit_code=0, stdout="cargo-orthohelp 0.9.10\n"
    ).in_order()
    installation = (
        cmd_mox
        .mock("cargo")
        .with_args(
            "binstall",
            "--no-confirm",
            "--locked",
            "--disable-strategies",
            "compile",
            f"cargo-orthohelp@{VERSION}",
        )
        .returns(exit_code=0)
        .in_order()
    )
    monkeypatch.setenv("INPUT_VERSION", VERSION)

    installer.app([], result_action="return_value")

    assert len(installation.invocations) == 1, (
        "a cache miss installs the pinned release"
    )


def test_failed_binstall_propagates_its_exit_status(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A failed prebuilt install fails the workflow step with its status."""
    cmd_mox.mock("cargo-orthohelp").with_args("--version").returns(
        exit_code=127
    ).in_order()
    cmd_mox.mock("cargo").with_args(
        "binstall",
        "--no-confirm",
        "--locked",
        "--disable-strategies",
        "compile",
        f"cargo-orthohelp@{VERSION}",
    ).returns(exit_code=23).in_order()
    monkeypatch.setenv("INPUT_VERSION", VERSION)

    with pytest.raises(SystemExit) as error:
        installer.app([])

    assert error.value.code == 23, (
        "the step must preserve cargo binstall's failure status"
    )
