"""Verify how the canary runner reads the composite action's list inputs.

The action passes each list as one input so a single invocation works under
Bash and PowerShell. Targets split on whitespace; selectors, extra environment,
and forbidden patterns split on lines.
"""

import typing as typ

import pytest
from downstream_canary_arguments import split_values
from downstream_canary_test_support import Canary

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture
def canary(tmp_path: Path) -> Canary:
    """Provide a fresh fake downstream checkout.

    Returns
    -------
    Canary
        The fake checkout and its working files.
    """
    return Canary(tmp_path)


@pytest.mark.parametrize(
    ("values", "separator", "expected"),
    [
        (["check-fmt lint", " test "], None, ["check-fmt", "lint", "test"]),
        (["A=1\nB=2\n", ""], "\n", ["A=1", "B=2"]),
        (
            ["--features 'postgres test-support'"],
            "\n",
            ["--features 'postgres test-support'"],
        ),
        (["", "  "], None, []),
    ],
)
def test_split_values_flattens_each_input(
    values: list[str], separator: str | None, expected: list[str]
) -> None:
    """Blank entries vanish, and a line keeps its internal spaces."""
    assert split_values(values, separator) == expected, f"split of {values!r}"


def test_one_input_can_carry_every_target_and_selector(canary: Canary) -> None:
    """A whitespace list of targets and a line list of selectors both apply."""
    generate_status = canary.step(
        "generate",
        "--netsuke",
        str(canary.netsuke),
        "--selector",
        "MXD_BACKEND=sqlite\n",
    )
    run_status = canary.step(
        "run", "--target", "lint test", "--selector", "MXD_BACKEND=sqlite\n"
    )

    assert (generate_status, run_status) == (0, 0), "both steps should pass"
    assert canary.log.read_text(encoding="utf-8").splitlines()[1:] == [
        "ninja -f build.ninja lint lane=sqlite url=-",
        "ninja -f build.ninja test lane=sqlite url=-",
    ], "each listed target should run with the selected lane"


def test_extra_environment_reaches_tools_but_not_the_record(canary: Canary) -> None:
    """A service URL is passed to the tools and kept out of the provenance."""
    url = "POSTGRES_TEST_URL=postgres://postgres:password@127.0.0.1/test"
    canary.step("generate", "--netsuke", str(canary.netsuke), "--environment", url)
    canary.step("run", "--target", "test", "--environment", url)

    record = canary.report("test")

    assert all(
        line.endswith("url=postgres://postgres:password@127.0.0.1/test")
        for line in canary.log.read_text(encoding="utf-8").splitlines()
    ), "every tool should see the extra environment"
    assert "password" not in canary.provenance.read_text(encoding="utf-8"), (
        "the provenance record must not carry the extra environment"
    )
    assert record["outcome"] == "passed", "the run should still pass"


def test_a_step_without_targets_is_refused(canary: Canary) -> None:
    """An empty target list is a configuration error, not a vacuous pass."""
    assert canary.step("run", "--target", "  ") == 2, "an empty list is refused"
