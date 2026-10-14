r"""Pin that artefact initialization owns the filesystem effects and refusals.

Configuration start-up is two steps behind one seam. Resolution reads the
variables and writes nothing -- what it reads, and that it writes nothing, is
what :mod:`test_release_admission_configuration_resolution` pins. This module
pins the other side: :func:`~_release_admission.configuration.prepare_destination`
is handed a resolved :class:`~_release_admission.telemetry.Sinks` and a
:class:`~_release_admission.filesystem.FileSystem`, and performs exactly four
effects -- both parents first, then both truncations -- in the shell's order,
reporting either refusal as a
:class:`~_release_admission.configuration.ConfigurationError` whose message
names the configured spelling and keeps the system's own reason.

The port here is a *recording* one rather than the operating system's. The
effects under test are pure orchestration: which operand each effect is given,
in what order, and what happens when one is refused. Driving them through a real
filesystem would tie the claims to a temporary directory and leave the order to
be inferred from which bytes survived, which is the weaker of the two
statements. The end-to-end correspondence is pinned where it belongs, in
:mod:`test_release_admission_artefact_truncation`, which runs the real entry
point against real paths.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' \
        --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_artefact_initialization.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import errno
import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
from _release_admission import configuration, filesystem, telemetry

#: The metrics binding doubles its separator so that it and the trace binding
#: stay distinct strings while naming the same directory. The doubling is the
#: point: it survives in the configured spelling, so a truncation handed the
#: binding rather than a resolved :class:`~pathlib.Path` is observably different.
METRICS_BINDING = "/observations//metrics.jsonl"
TRACE_BINDING = "/observations/traces.jsonl"

#: The ``mkdir`` operand the shell's ``dirname(1)`` named for both bindings.
#: ``dirname(1)`` collapses the doubled separator, and the port reuses the
#: already-derived parent rather than re-deriving one per binding.
EXPECTED_PARENT = "/observations"

#: The sentences the operating system supplies for the two refusals, kept as
#: the shell kept them: ``mkdir`` for a parent it cannot make, and the
#: redirection for a target it cannot open.
PARENT_REFUSAL_REASON = "Not a directory"
TARGET_REFUSAL_REASON = "Is a directory"

#: The two members every double must provide, named once so the conformance
#: case and the port cannot drift apart silently.
PORT_MEMBERS = ("make_parent", "truncate")


def sinks(
    metrics_file: str = METRICS_BINDING, trace_file: str = TRACE_BINDING
) -> telemetry.Sinks:
    """Return sinks carrying the two bindings under test.

    Returns
    -------
    telemetry.Sinks
        Sinks whose only configured members are the two artefact bindings; the
        three adapters are left empty, so initialization cannot reach a sink.
    """
    return telemetry.Sinks(
        metrics_file=metrics_file,
        trace_file=trace_file,
        metrics_sink="",
        output_sink="",
        trace_sink="",
    )


class RecordingFileSystem:
    """Record every effect the initializer performs.

    The record is the whole value of the double: an ordered list of
    ``(effect, operand)`` pairs, which is what lets a case assert the sequence
    rather than the set. ``refuse_truncation`` raises the system's own refusal
    for the truncation of one named binding and records nothing for it.
    """

    def __init__(self, *, refuse_truncation: str | None = None) -> None:
        """Start with an empty record, optionally refusing one truncation."""
        self.calls: list[tuple[str, str]] = []
        self._refuse_truncation = refuse_truncation

    def make_parent(self, directory: Path, /) -> None:
        """Record one parent creation."""
        self.calls.append(("make_parent", str(directory)))

    def truncate(self, configured: str, /) -> None:
        """Record one truncation, unless this binding is the refused one."""
        if configured == self._refuse_truncation:
            raise OSError(errno.EISDIR, TARGET_REFUSAL_REASON, configured)
        self.calls.append(("truncate", configured))


class RefusingParentFileSystem(RecordingFileSystem):
    """Record the first parent, then refuse the second.

    The refusal is on the *second* parent because the first pass is what this
    double exists to pin: a case that refused the first would say only that the
    gate stopped, not what it had already done. Both bindings derive the same
    parent, so the position is counted rather than matched by name.
    """

    def __init__(self) -> None:
        """Start with an empty record and no parent yet asked for."""
        super().__init__()
        self._made = 0

    def make_parent(self, directory: Path, /) -> None:
        """Record the parent, refusing the second one this pass is asked for."""
        self._made += 1
        if self._made > 1:
            raise OSError(errno.ENOTDIR, PARENT_REFUSAL_REASON, str(directory))
        super().make_parent(directory)

    def truncate(self, configured: str, /) -> None:
        """Fail the case: a refused parent must stop the pass before this."""
        msg = f"a refused parent must stop the pass, but {configured} was truncated"
        raise AssertionError(msg)


class ReasonlessFileSystem(RecordingFileSystem):
    """Refuse the first truncation with a :class:`OSError` carrying no sentence."""

    def truncate(self, configured: str, /) -> None:
        """Raise a refusal the operating system supplied no text for."""
        raise OSError(errno.ENOENT, None, configured)


#: Every double in this module, typed as the port. The annotation is the check
#: that matters: a member renamed or re-signatured in
#: :class:`~_release_admission.filesystem.FileSystem` fails here rather than in
#: whichever case happens to reach that double first.
DOUBLES: typ.Final[tuple[filesystem.FileSystem, ...]] = (
    RecordingFileSystem(),
    RefusingParentFileSystem(),
    ReasonlessFileSystem(),
)


def test_every_double_satisfies_the_filesystem_port() -> None:
    """Verify each double really implements both port members.

    Notes
    -----
    The static half of this claim is carried by the annotation on
    :data:`DOUBLES`; the runtime half is here so the module fails loudly rather
    than statically if a member is renamed in one place only. A double missing a
    member would otherwise be bound to an inherited stub -- or, for a double
    that inherits, to the recording base's implementation -- and keep passing.
    """
    missing = [
        (double.__class__.__name__, member)
        for double in DOUBLES
        for member in PORT_MEMBERS
        if not callable(getattr(double, member, None))
    ]

    assert missing == [], f"every double must implement the port, missing {missing!r}"


def test_both_parents_are_made_before_either_file_is_truncated() -> None:
    """Verify the four effects run in the shell's own order with its operands.

    Notes
    -----
    The order is a contract rather than an implementation detail: the shell ran
    one ``mkdir -p`` naming *both* parents and only then truncated both files, so
    a refused trace parent left a previous run's metrics file intact. An
    initializer that made each parent immediately before its own truncation
    performs the same four effects in a different order and destroys that file,
    which is why the sequence is asserted rather than the set.
    """
    file_system = RecordingFileSystem()
    configuration.prepare_destination(sinks(), file_system)

    parents, truncations = file_system.calls[:2], file_system.calls[2:]

    assert parents == [("make_parent", EXPECTED_PARENT)] * 2, (
        "the shell made every parent before it truncated either file"
    )
    assert truncations == [
        ("truncate", METRICS_BINDING),
        ("truncate", TRACE_BINDING),
    ], "both truncations must follow every parent, in the shell's own order"


def test_the_truncation_operand_is_the_configured_spelling() -> None:
    """Verify each truncation is handed the configured spelling, not a Path.

    Notes
    -----
    ``Path("/observations//metrics.jsonl")`` renders back as
    ``/observations/metrics.jsonl``, so an initializer that took a
    :class:`~pathlib.Path` would open a different file than the operator named --
    and for a trailing slash it would create the regular file the shell's
    redirection refused. The binding is asserted verbatim for that reason; the
    port's own docstring carries the *why*, and this is the observable half.
    """
    file_system = RecordingFileSystem()
    configuration.prepare_destination(sinks(), file_system)

    truncations = [
        operand for effect, operand in file_system.calls if effect == "truncate"
    ]

    assert truncations == [METRICS_BINDING, TRACE_BINDING], (
        "each truncation must be handed the binding the sink was configured with"
    )
    assert str(Path(METRICS_BINDING)) != METRICS_BINDING, (
        "this case only proves the spelling was kept if Path would have changed it"
    )


def test_a_refused_parent_stops_before_any_file_is_truncated() -> None:
    """Verify a refused parent is reported and leaves both files untouched.

    Notes
    -----
    The first pass is the shell's ``mkdir -p``, so a parent it cannot make stops
    the gate before the second pass runs at all. That is what keeps a refused
    configuration from destroying a previous run's records, and it is the
    difference between refusing and truncating-then-refusing. The double's own
    ``truncate`` fails the case if it is reached at all.
    """
    file_system = RefusingParentFileSystem()
    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.prepare_destination(sinks(), file_system)

    assert str(refusal.value) == (
        "release-admission could not create the directory for "
        f"{TRACE_BINDING}: {PARENT_REFUSAL_REASON}"
    ), "the diagnostic must name the configured spelling and the system's reason"
    assert file_system.calls == [("make_parent", EXPECTED_PARENT)], (
        "the refusal must stop the pass rather than continue to the trace parent"
    )


def test_a_refused_truncation_keeps_the_system_reason_and_the_diagnostic_lead() -> None:
    """Verify a refused truncation names the target and the system's sentence.

    Notes
    -----
    The shell printed ``mkdir``'s and the redirection's own sentences, so the
    port's diagnostic keeps the system's text rather than substituting a Python
    class name. That is why the reason is asserted as the ``strerror`` the
    operating system supplied rather than as the exception's full rendering,
    which would repeat the error number and the target inside the sentence.
    """
    file_system = RecordingFileSystem(refuse_truncation=METRICS_BINDING)
    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.prepare_destination(sinks(), file_system)

    assert str(refusal.value) == (
        f"release-admission could not create {METRICS_BINDING}: {TARGET_REFUSAL_REASON}"
    ), "a refused truncation must report the target it could not create"
    assert file_system.calls == [
        ("make_parent", EXPECTED_PARENT),
        ("make_parent", EXPECTED_PARENT),
    ], "both parents were made in the pass before the truncation was attempted"


def test_a_refusal_without_a_system_sentence_names_the_error_class() -> None:
    """Verify a reasonless refusal still yields one bounded diagnostic.

    Notes
    -----
    A :class:`OSError` raised without a system sentence has no ``strerror``, and
    the shell in that state printed a message naming the failure rather than an
    empty one. Naming the class is the bounded substitute: it carries no path and
    no identifier beyond the target the operator already supplied, so the
    diagnostic stays releasable.
    """
    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.prepare_destination(sinks(), ReasonlessFileSystem())

    assert str(refusal.value) == (
        f"release-admission could not create {METRICS_BINDING}: FileNotFoundError"
    ), "a refusal without a system sentence must still name what failed"


def test_production_filesystem_performs_both_effects(tmp_path: Path) -> None:
    """Verify the production port really creates a parent and truncates a file.

    Notes
    -----
    Every other case here substitutes a double, so without this one the
    production adapter could delegate to nothing and the suite would still pass.
    The file is asserted *empty* rather than absent, because absent is also what
    never creating it looks like: truncation is the one outcome that cannot be
    confused with having done nothing.
    """
    file_system = filesystem.production_filesystem()
    parent = tmp_path / "nested" / "deeper"
    target = tmp_path / "overwritten.jsonl"
    target.write_bytes(b'{"left": "by a previous run"}\n')

    file_system.make_parent(parent)
    file_system.truncate(str(target))

    assert parent.is_dir(), "the production port must create every missing parent"
    assert target.read_bytes() == b"", (
        "the production port must truncate the file, not leave its bytes in place"
    )
