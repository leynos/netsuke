r"""Pin where the gate stops when an artefact target cannot be truncated.

``load_configuration`` creates both artefact files before it reads anything
else, and it does so in two passes: every parent directory first, then every
file truncated empty. The first pass refuses a path whose *parent* cannot be
made, which :mod:`test_release_admission_environment_contract` pins. This
module pins the second, where the target itself cannot be opened, and it
separates two contracts a single technique cannot both hold.

**The order** is what the directory cases pin, and it is asymmetric. The shell
ran ``: >"$metrics_file"; : >"$trace_file"``, so a refused *metrics* target
stopped before the trace file was touched and left a previous run's trace bytes
intact, while a refused *trace* target had already truncated the metrics file
to zero. A port that truncated the two in the other order, or that created both
files before refusing either, would destroy or preserve a previous run's
records on a configuration it then refuses -- and the gate's reputation is that
a refused configuration changes nothing it was not told to change.

**The operand** is what the slash cases pin, and a directory target alone
cannot pin it. The defect this module exists for was that ``_truncate`` wrote
to the *resolved* path, so a target spelled with a trailing slash was rewritten
to the regular file the shell had refused: ``Path("metrics.jsonl/")`` drops the
slash and ``write_bytes`` then created ``metrics.jsonl``. A directory target is
refused by that implementation too -- ``write_bytes`` on a directory raises
``IsADirectoryError``, whose message contains *Is a directory*, which is what
the order cases assert on -- so the module would have kept passing had the
defect been reintroduced. What discriminates is a configured string ending in
``/``: opening it refuses with the system's own sentence, while writing to the
resolved path silently succeeds at a target that does not exist and silently
truncates one that does. The slash cases therefore assert a refusal, that the
nonexistent target was not created, and that an existing target's prior bytes
survive -- the last of which is the difference between a refusal and a
``write_bytes`` that destroyed the file before reporting anything.

**The parent** is what the ``/.`` case pins, and it is a third contract rather
than a variant of the operand one. The shell's ``mkdir`` operand was
``$(dirname "$file")`` -- coreutils ``dirname(1)`` over the configured string --
while the port derived it from the configured path's
:attr:`~pathlib.Path.parent`. The two agree on every slash spelling and diverge
on a final ``/.``: ``dirname(1)`` answers ``reports`` for ``reports/.``, which
the shell then *created*, while ``Path("reports/.").parent`` answers ``.``,
whose ``mkdir`` makes nothing. Both then refuse the truncation and both fail
closed, which is precisely why the case has to assert on the *filesystem* and
on the operating system's sentence: the parent directory must exist afterwards,
and the reason must be ``Is a directory`` rather than the ``No such file or
directory`` a missing parent produces.

The refusal is induced through the filesystem rather than the environment, so
the ``mkdir`` of both parents succeeds and only the truncation can fail.
Nothing reaches an adapter afterwards, which is asserted rather than assumed,
because "refused before any operation ran" is a claim the shell's ``set -e``
also made.

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

#: The suffix that separates the two spellings. ``Path`` normalizes it away, so
#: it exists only in the configured string -- which is why a case that asserts
#: on a :class:`Path` cannot see the difference the defect turned on.
SLASH_SUFFIX = "/"

#: The suffix that separates the shell's ``mkdir`` operand from the configured
#: path's :attr:`~pathlib.Path.parent`. ``Path`` normalizes *this* away too, but
#: the two forms then name different directories rather than the same one in
#: two spellings: the shell made the target's parent, and the normalised parent
#: is the directory the target sits in.
DOT_COMPONENT_SUFFIX = "/."

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

#: The two states a slash-terminated target can be in, which the defect
#: distinguished by *succeeding*: at a target that did not exist it created the
#: regular file the shell refused, and at one that did it truncated the file to
#: zero. Both are asserted, because the module's earlier revision only ever
#: exercised the refusal a directory also produces.
SLASH_CASES = [
    pytest.param(False, id="slash-target-does-not-exist"),
    pytest.param(True, id="slash-target-is-an-existing-file"),
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


@pytest.mark.parametrize("target_exists", SLASH_CASES)
def test_slash_target_is_refused_in_the_configured_spelling(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    *,
    target_exists: bool,
) -> None:
    """Verify a slash-terminated artefact target is refused, not rewritten.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated directory that owns the slash-terminated target.
    target_exists
        Whether the target already exists as a regular file. Both states are
        run because the defect succeeded in a *different* way in each: at an
        absent target it created the file the shell refused, and at an existing
        one it truncated the operator's file to zero before the gate went on to
        fail for some other reason.

    Notes
    -----
    The substitution is the whole discriminator. ``bash`` refuses
    ``: >"metrics.jsonl/"`` with ``Is a directory`` in both states and leaves an
    existing file's bytes alone, so a *directory* target pins nothing about the
    operand: ``write_bytes`` on a directory raises ``IsADirectoryError`` too,
    with the same sentence this module asserts on. Only the trailing slash
    separates "the configured spelling was opened" from "the resolved path was
    written", and the assertions below are what the resolved form cannot
    satisfy -- it creates the absent target, and it empties the existing one.

    The two artefact variables both name the refused target, so the refusal is
    reached whichever of the two the gate truncates first. That is deliberate
    here and is *not* the order contract: this case claims nothing about which
    file is touched first, which is what the directory cases above pin.
    """
    adapted = install_adapted(cmd_mox)
    slash_target = tmp_path / "metrics.jsonl"
    if target_exists:
        slash_target.write_bytes(PRESERVED)
    configured = f"{slash_target}{SLASH_SUFFIX}"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            METRICS_FILE_VARIABLE: configured,
            TRACE_FILE_VARIABLE: configured,
        },
    )

    # The filesystem claim is asserted before the diagnostic one: it is the
    # claim the defect cannot satisfy, so a reintroduction should name the
    # destroyed or created target rather than a missing refusal status that is
    # merely downstream of it.
    if target_exists:
        assert slash_target.read_bytes() == PRESERVED, (
            "a slash-terminated target that already exists must keep its bytes, "
            "because the shell's redirection refused it without opening it"
        )
    else:
        assert not slash_target.exists(), (
            "a slash-terminated target must not be created as the regular file "
            "the resolved spelling would have written"
        )
    _assert_refused_diagnostic(run, configured)
    _assert_no_boundary_was_reached(adapted)
    assert run.outputs == {}, "a refusal before the trap writes no workflow output"


def test_dot_component_target_creates_the_parent_the_shell_created(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    r"""Verify a ``/.``-terminated target's parent is made as ``dirname(1)`` made it.

    Notes
    -----
    A configured target ending in ``/.`` is the one spelling where the shell's
    ``mkdir`` operand and the configured path's :attr:`~pathlib.Path.parent`
    name different directories. The shell asked coreutils ``dirname(1)``, which
    answers ``reports`` for ``reports/.``, so ``mkdir -p reports`` ran and the
    directory existed by the time the redirection refused the target with
    ``Is a directory``. ``Path("reports/.")`` normalises the trailing component
    away, so a port deriving the operand from it asks for ``.``, makes nothing,
    and refuses the same target with ``No such file or directory``.

    Both outcomes fail closed, so the exit status alone cannot tell them apart.
    The parent's *existence* is the discriminating claim, and the operating
    system's sentence is the second half of it: ``Is a directory`` can only be
    reported against a directory that is there. The parent is deliberately left
    absent at the start -- the harness names artefact paths inside ``tmp_path``
    and creates none of their parents -- so a port that made nothing cannot pass
    by finding a directory some earlier step had already made.

    The target's own directory *is* the parent, so the assertion is stated as
    the parent's existence rather than as ``not target.exists()``: an absent
    target is exactly what the failing implementation leaves behind, and
    asserting it would pin the defect rather than the contract.
    """
    adapted = install_adapted(cmd_mox)
    parent = tmp_path / "reports"
    configured = f"{parent}{DOT_COMPONENT_SUFFIX}"
    run = run_gate(
        cmd_mox,
        tmp_path,
        extra_environment={
            **adapted.variables(cmd_mox),
            METRICS_FILE_VARIABLE: configured,
            TRACE_FILE_VARIABLE: configured,
        },
    )

    assert parent.is_dir(), (
        "the shell's dirname named the parent and mkdir -p created it, so the "
        "port must create it too rather than mkdir-ing the normalised parent"
    )
    _assert_refused_diagnostic(run, configured)
    _assert_no_boundary_was_reached(adapted)
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


def _assert_refused_diagnostic(run: GateRun, configured: str | pathlib.Path) -> None:
    """Assert the gate refused the target in one bounded diagnostic.

    The operand is the *configured* string rather than a :class:`Path`, because
    the slash cases need the diagnostic to carry the trailing slash the
    operator supplied; rendering a ``Path`` back to text would drop it, which is
    the very rewriting those cases exist to catch.
    """
    assert run.result.returncode == 1, "a refused artefact target must fail closed"
    assert str(configured) in run.result.stderr, (
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
