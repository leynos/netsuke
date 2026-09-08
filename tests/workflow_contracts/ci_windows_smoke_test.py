"""Contract tests for the Windows native-recipe smoke steps.

The smoke test used to be a second job, ``windows-native-recipe-smoke``, that
``needs``-ed ``build-test-windows``. That made it a strict serial tail on every
pull request: a measured median of 236s, of which only 7s ran the fixture. It
is now two steps at the end of the gate job.

Folding it in is only faithful while those steps keep launching Netsuke from
PowerShell, because that is the whole contract (see #599) and the job's default
shell is Git Bash. A step that lost ``shell: pwsh`` would still carry its name
and its explanatory comment, so these assertions match the shell and the
command rather than the step name alone. They live here rather than in
``ci_windows_job_test.py`` to keep both modules inside the repository's
400-line file limit. Shared parsing helpers live in ``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

WINDOWS_JOB = "build-test-windows"

#: PowerShell's block comment, which spans lines. Matched non-greedily so two
#: separate blocks do not swallow the executable lines between them.
_BLOCK_COMMENT = re.compile(r"<#.*?#>", re.DOTALL)

#: Both halves of the Windows gate, in declaration order.
EXPECTED_WINDOWS_JOBS = ("lint-windows", WINDOWS_JOB)


def command_lines(run: str) -> list[str]:
    """Return the executable PowerShell lines of `run`, comments removed.

    A substring search over the whole block is satisfied by a commented-out
    command, which builds nothing, and by an altered command that happens to
    contain the expected text. Matching against stripped, non-comment lines and
    anchoring at their start rules both out.

    PowerShell has two comment forms and both must go. `#` runs to end of line;
    `<# ... #>` spans lines and would otherwise leave every command it wraps
    looking executable, which is the same defeat by a different syntax.

    Parameters
    ----------
    run:
        The step's PowerShell ``run`` block.

    Returns
    -------
    list[str]
        Each non-blank, non-comment line, stripped of surrounding whitespace,
        in declaration order.
    """
    without_blocks = _BLOCK_COMMENT.sub(" ", run)
    return [
        stripped
        for line in without_blocks.splitlines()
        if (stripped := line.strip()) and not stripped.startswith("#")
    ]


@pytest.mark.parametrize(
    ("run", "expected"),
    [
        ("cargo build --locked --bin netsuke", ["cargo build --locked --bin netsuke"]),
        ("# cargo build --locked --bin netsuke", []),
        ("<#\ncargo build --locked --bin netsuke\n#>", []),
        ("<# cargo build --locked --bin netsuke #>", []),
        (
            "<#\nfirst --commented\n#>\nsecond --live\n<#\nthird --commented\n#>",
            ["second --live"],
        ),
    ],
    ids=[
        "a-bare-command-is-executable",
        "a-line-comment-hides-it",
        "a-multi-line-block-comment-hides-it",
        "a-single-line-block-comment-hides-it",
        "two-blocks-do-not-swallow-the-command-between-them",
    ],
)
def test_command_lines_ignores_both_comment_forms(
    run: str, expected: list[str]
) -> None:
    """Only commands PowerShell would execute may count as executable.

    Scenario: the step assertions match against this helper's output, so
    anything it wrongly reports as a command can satisfy them while nothing
    runs. Invariant: both PowerShell comment forms are removed, including a
    block comment spanning lines, and two blocks do not swallow the live
    command between them, which a greedy match would.
    """
    assert command_lines(run) == expected, (
        f"{run!r} should yield the executable lines {expected!r}, "
        f"got {command_lines(run)!r}"
    )


@pytest.fixture
def windows_steps() -> list[dict[str, object]]:
    """Return the build-test-windows job's steps, in declaration order."""
    return job_steps(load_workflow(CI_WINDOWS_WORKFLOW_PATH), WINDOWS_JOB)


def test_windows_job_runs_the_native_recipe_smoke_after_the_test_gate(
    windows_steps: list[dict[str, object]],
) -> None:
    """The folded smoke test must launch Netsuke from PowerShell, after Test.

    Scenario: the native-recipe smoke test used to be a second job that
    ``needs``-ed this one, a strict serial tail; it is now two steps here.
    Invariant: both steps declare ``pwsh`` rather than inheriting the job's Git
    Bash default, the smoke script is invoked with the binary and manifest it
    exercises, and both follow the Test step that builds the workspace they
    reuse. A PowerShell-launched Netsuke is the whole point of the smoke test
    (see #599), so a step that lost its shell would still pass a name check
    while testing nothing.
    """
    step_names = [str(step.get("name", "")) for step in windows_steps]
    for required in ("Test", "Build Netsuke", "Exercise native Windows recipes"):
        assert required in step_names, (
            f"{WINDOWS_JOB} must declare a {required!r} step, "
            f"got step order {step_names!r}"
        )
    assert (
        step_names.index("Test")
        < step_names.index("Build Netsuke")
        < step_names.index("Exercise native Windows recipes")
    ), (
        "the smoke steps must follow Test so the binary links against the "
        f"workspace that step compiled, got step order {step_names!r}"
    )

    for step_name, fragments in (
        ("Build Netsuke", ("cargo build --locked --bin netsuke",)),
        (
            "Exercise native Windows recipes",
            (
                "./scripts/windows-recipe-smoke.ps1",
                "-Netsuke ./target/debug/netsuke.exe",
                "-Manifest ./tests/data/windows-recipe-smoke.yml",
            ),
        ),
    ):
        step = named_step(windows_steps, step_name)
        assert step.get("shell") == "pwsh", (
            f"{step_name} must declare the PowerShell Core shell rather than "
            "inherit the job's Git Bash default, because launching Netsuke from "
            f"PowerShell is what it exercises, got {step.get('shell')!r}"
        )
        match step.get("run"):
            case str() as run:
                pass
            case _:
                pytest.fail(f"{step_name} must declare a PowerShell run block")
        lines = command_lines(run)
        missing = [
            expected
            for expected in fragments
            if not any(line.startswith(expected) for line in lines)
        ]
        assert not missing, (
            f"{step_name} must invoke the command it exists to run as an "
            f"executable line, not merely mention it: commenting the command "
            f"out or renaming it must fail here. Missing {missing!r} from "
            f"{lines!r}"
        )


def test_windows_workflow_declares_no_serialised_job() -> None:
    """The Windows lane must stay two concurrent jobs and no more.

    Scenario: `windows-native-recipe-smoke` used to be a third job that
    ``needs``-ed the gate, costing a measured median 236s of serial tail for 7s
    of work. Invariant: `ci-windows.yml` declares exactly the lint and test
    jobs, so a reinstated serial job has to justify the tail rather than arrive
    quietly.
    """
    workflow = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "ci-windows.yml jobs")
    assert list(jobs) == list(EXPECTED_WINDOWS_JOBS), (
        f"ci-windows.yml must declare exactly {list(EXPECTED_WINDOWS_JOBS)!r}; the "
        f"native-recipe smoke test is folded into the test job as PowerShell "
        f"steps, got {list(jobs)!r}"
    )
