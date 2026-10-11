r"""Pin where the gate stops when an artefact target cannot be truncated.

``load_configuration`` creates both artefact files before it reads anything
else, and it does so in two passes: every parent directory first, then every
file truncated empty. The first pass refuses a path whose *parent* cannot be
made, which :mod:`test_release_admission_environment_contract` pins. This
module pins the second, where the target itself cannot be opened -- it names an
existing *directory* -- and the order inside that pass is what an operator can
observe.

The order is the whole point, and it is asymmetric. The shell ran
``: >"$metrics_file"; : >"$trace_file"``, so a refused *metrics* target stopped
before the trace file was touched and left a previous run's trace bytes intact,
while a refused *trace* target had already truncated the metrics file to zero.
A port that truncated the two in the other order, or that created both files
before refusing either, would destroy or preserve a previous run's records on a
configuration it then refuses -- and the gate's reputation is that a refused
configuration changes nothing it was not told to change.

The refusal is induced through the filesystem rather than the environment: the
target is made a directory, so the ``mkdir`` of both parents succeeds and only
the truncation can fail. Nothing reaches an adapter afterwards, which is
asserted rather than assumed, because "refused before any operation ran" is a
claim the shell's ``set -e`` also made.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_artefact_truncation.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_support import (
    AdaptedBoundaries,
    GateRun,
    install_adapted,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"

#: What a previous run is imagined to have left in the artefact that is *not*
#: the refused target. Any non-empty text would do; JSON is chosen so the bytes
#: are the shape a real artefact holds.
PRESERVED = b'{"name": "a previous run left this here"}\n'

#: The two truncation boundaries, each with the bytes the *other* artefact must
#: hold once the gate has exited. ``None`` removes the guesswork: the expected
#: bytes are stated per case, because they differ, and the difference is the
#: contract.
TRUNCATION_CASES = [
    pytest.param(
        METRICS_FILE_VARIABLE,
        PRESERVED,
        id="metrics-refused-leaves-the-trace-untouched",
    ),
    pytest.param(
        TRACE_FILE_VARIABLE,
        b"",
        id="trace-refused-truncates-the-metrics-first",
    ),
]


@pytest.mark.parametrize(("refused_variable", "expected_bytes"), TRUNCATION_CASES)
def test_truncation_refusal_stops_at_the_artefact_it_cannot_create(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    refused_variable: str,
    expected_bytes: bytes,
) -> None:
    """Verify a refused artefact target stops the gate where the shell stopped.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated directory that owns the refused target and the survivor.
    refused_variable
        The artefact variable whose target is a directory, so it cannot be
        truncated.
    expected_bytes
        The bytes the surviving artefact must hold once the gate has exited.

    Notes
    -----
    The two cases differ in what must *survive*, and that is deliberate. When
    the metrics target is refused, the trace file has not been touched yet, so
    a previous run's trace bytes are still there. When the trace target is
    refused, the metrics file was already truncated on the way past, so the
    same previous bytes are gone and the file is empty. Asserting one value for
    both would pin the wrong contract in one of the two.

    Every double is asserted empty, not only the two command adapters: a gate
    that ran an operation would read the clock in front of it, and a gate that
    reached record delivery would reach a sink. The clock adapter is redirected
    at a double for exactly this, because the gate's default clock is the real
    interpreter and an absent call on a name it never resolves proves nothing.
    """
    adapted = install_adapted(cmd_mox)
    refused, survivor = _targets(tmp_path)
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            **_target_expressions(refused, survivor, refused_variable),
        },
    )

    _assert_refused_diagnostic(run, refused)
    _assert_no_boundary_was_reached(adapted)
    assert survivor.read_bytes() == expected_bytes, (
        "the artefact the gate did not refuse must hold what the shell left there"
    )
    assert run.outputs == {}, "a refusal before the trap writes no workflow output"


def _target_expressions(
    refused: pathlib.Path,
    survivor: pathlib.Path,
    refused_variable: str,
) -> dict[str, str]:
    """Point one artefact variable at the refused target and the other at the survivor.

    Returns
    -------
    dict[str, str]
        The two artefact variables, one naming the directory that cannot be
        truncated and one naming the file a previous run left behind.
    """
    targets = {
        METRICS_FILE_VARIABLE: survivor,
        TRACE_FILE_VARIABLE: survivor,
    }
    targets[refused_variable] = refused
    return {variable: str(path) for variable, path in targets.items()}


def _targets(tmp_path: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """Create the refused target and the survivor one case runs against.

    Returns
    -------
    tuple[pathlib.Path, pathlib.Path]
        The directory that cannot be truncated, and the file that holds a
        previous run's bytes.
    """
    refused = tmp_path / "refused-artefact"
    refused.mkdir()
    survivor = tmp_path / "previous-run.jsonl"
    survivor.write_bytes(PRESERVED)
    return refused, survivor


def _assert_refused_diagnostic(run: GateRun, refused: pathlib.Path) -> None:
    """Assert the gate refused the target in one bounded diagnostic."""
    assert run.result.returncode == 1, "a refused artefact target must fail closed"
    assert str(refused) in run.result.stderr, (
        "the diagnostic must carry the configured path the operator supplied"
    )
    assert "Is a directory" in run.result.stderr, (
        "the diagnostic must keep the operating system's own sentence"
    )
    assert "Traceback" not in run.result.stderr, (
        "a filesystem refusal must be reported, not left as an uncaught traceback"
    )


def _assert_no_boundary_was_reached(adapted: AdaptedBoundaries) -> None:
    """Assert no adapter and no sink was reached before the refusal.

    The refusal happens inside configuration loading, so nothing downstream may
    have run; asserting the empty call lists is what makes that a checked claim
    rather than a consequence of where the code happens to sit.
    """
    for boundary, name in (
        (adapted.gh, "GitHub"),
        (adapted.git, "Git"),
        (adapted.clock, "clock"),
        (adapted.metrics, "metric"),
        (adapted.output, "workflow-output"),
        (adapted.trace, "trace"),
    ):
        assert boundary.calls == [], (
            f"a refused artefact target must not reach the {name} boundary"
        )
