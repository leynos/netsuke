"""Hold the source-build detector to the forms it must catch.

`is_source_built` is what keeps a retired exception retired, so its own
blind spots are the thing worth testing. An earlier pattern skipped tokens by
shape, requiring each to begin with a dash or a quote, which let the crate
name hide behind any option value: `cargo install --version 0.9.1
cargo-orthohelp` passed the guard while compiling exactly the tool the guard
exists to forbid. The same pattern matched by prefix, so it would have
rejected `cargo-orthohelp-extra`, a different crate.

Run via ``make test-workflow-contracts``.
"""

import pytest
from source_build_data import is_source_built, source_build_commands

TOOL = "cargo-orthohelp"


@pytest.mark.parametrize(
    "command",
    [
        pytest.param("cargo install cargo-orthohelp", id="bare"),
        pytest.param("cargo install --locked cargo-orthohelp@0.9.1", id="pinned"),
        pytest.param(
            "cargo install --version 0.9.1 cargo-orthohelp", id="separate-version"
        ),
        pytest.param(
            "cargo install --index https://example.invalid cargo-orthohelp",
            id="separate-index",
        ),
        pytest.param(
            "cargo install --root /tmp/tools cargo-orthohelp", id="separate-root"
        ),
        pytest.param(
            "cargo install --git https://example.invalid/repo cargo-orthohelp",
            id="separate-git",
        ),
        pytest.param('cargo install --locked "cargo-orthohelp"', id="quoted"),
        pytest.param("cargo install --locked 'cargo-orthohelp'", id="single-quoted"),
        pytest.param(
            'CARGO_TARGET_DIR="${DIR}" \\\n            '
            "cargo install --locked cargo-orthohelp@0.9.0",
            id="continued-line",
        ),
        pytest.param("cargo  install   --locked  cargo-orthohelp", id="extra-spaces"),
    ],
)
def test_a_source_build_is_detected(command: str) -> None:
    """Catch every `cargo install` form that would compile the tool.

    The separate-value cases are the ones that matter. An option and its value
    are two tokens, and the value is indistinguishable from a crate name
    without knowing which options take one, so a detector that skips tokens by
    shape stops at the value and never reaches the crate.
    """
    assert is_source_built(TOOL, command), f"missed a source build: {command!r}"


@pytest.mark.parametrize(
    "command",
    [
        pytest.param(
            "cargo install --locked cargo-orthohelp-extra", id="longer-crate-name"
        ),
        pytest.param("cargo install --locked mdtablefix@0.5.0", id="another-crate"),
        pytest.param(
            "cargo binstall --no-confirm --locked "
            "--disable-strategies compile cargo-orthohelp@0.9.1",
            id="binstall",
        ),
        pytest.param(
            "cargo-orthohelp --version | grep -Eq '0[.]9[.]1'", id="version-probe"
        ),
        pytest.param("# cargo install cargo-orthohelp-docs", id="different-crate"),
    ],
)
def test_a_non_source_build_is_not_flagged(command: str) -> None:
    """Leave alone what does not compile the tool.

    `cargo-orthohelp-extra` is a different crate, so a prefix match would be a
    false positive, and `cargo binstall` is the form this change adopts, so
    flagging it would make the contract unsatisfiable.
    """
    assert not is_source_built(TOOL, command), f"false positive: {command!r}"


def test_the_detector_finds_a_build_among_other_commands() -> None:
    """Find one offending command inside a larger script.

    The real caller passes every workflow and composite action concatenated,
    so the detector has to locate a single `cargo install` among thousands of
    unrelated lines rather than examine one command in isolation.
    """
    script = "\n".join([
        "set -euo pipefail",
        "cargo binstall --no-confirm --locked mdtablefix@0.5.0",
        "cargo install --version 0.9.1 cargo-orthohelp",
        "echo done",
    ])

    assert is_source_built(TOOL, script), (
        "the detector must find an offending command among unrelated lines"
    )


@pytest.mark.parametrize(
    "command",
    [
        pytest.param("cargo install some-tool", id="single-space"),
        pytest.param("cargo  install some-tool", id="double-space"),
        pytest.param("cargo\tinstall some-tool", id="tab"),
        pytest.param("cargo +nightly install some-tool", id="toolchain-selector"),
        pytest.param("cargo \\\n            install some-tool", id="line-continuation"),
    ],
)
def test_every_cargo_install_spelling_is_found(command: str) -> None:
    """Catch the shell spellings a literal substring search would miss.

    `cargo  install`, a tab, a `+toolchain` selector and a line continuation
    all run the same command. The general no-source-build rule is enforced by
    searching for these, so a spelling it cannot see is a hole in the policy
    rather than in one tool's guard.
    """
    assert source_build_commands(command), f"missed a source build: {command!r}"


@pytest.mark.parametrize(
    "command",
    [
        pytest.param("cargo binstall some-tool", id="binstall"),
        pytest.param("cargo install-update some-tool", id="different-subcommand"),
        pytest.param("# cargo installs are forbidden", id="prose"),
    ],
)
def test_a_non_install_command_is_not_flagged(command: str) -> None:
    """Leave alone what is not a `cargo install`.

    `cargo binstall` is the form this repository adopts, so flagging it would
    make the policy unsatisfiable.
    """
    assert not source_build_commands(command), f"false positive: {command!r}"
