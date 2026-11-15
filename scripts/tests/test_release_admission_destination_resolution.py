r"""Pin where destination resolution puts both artefacts, and under what name.

Configuration start-up is two steps behind one seam, and this module pins the
first: :func:`~_release_admission.configuration.resolve_destination` reads
``RUNNER_TEMP`` and the two path variables and answers the pair of artefact
paths every later step is handed. The second step -- the settings read from
every other variable -- is pinned in
:mod:`test_release_admission_configuration_resolution`, which also carries the
claims that hold for the pair jointly, such as resolution writing nothing.

The two steps are tested apart because their subjects differ. This module is
about *where* the artefacts go and *what string* each is called by: the default
directory, the operator's own spelling, and the parent coreutils ``dirname(1)``
named for that spelling. Its sibling is about *what settings* the gate runs
with.

Every case builds its own mapping in this process: resolution cannot consult
``os.environ``, so the ambient environment cannot make a case pass.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest \
        scripts/tests/test_release_admission_destination_resolution.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
from _release_admission import configuration

METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"
TEMPORARY_DIRECTORY_VARIABLE = "RUNNER_TEMP"

#: The temporary directory every case gives the gate, so that no case can
#: observe the ambient ``RUNNER_TEMP`` or reach the ``/tmp`` fallback.
TEMPORARY_DIRECTORY = "/nonexistent/runner-temp"

#: A spelling whose two derivations genuinely disagree: coreutils ``dirname(1)``
#: answers ``reports`` for it, while :attr:`pathlib.Path.parent` answers ``.``.
#: The shell passed the spelling on unchanged and computed the ``mkdir`` operand
#: as ``$(dirname "$file")``, so both halves are pinned by one case. The
#: divergence is not cosmetic: making ``.`` instead of ``reports`` is a no-op,
#: so the truncation would fail ``No such file or directory`` where the shell
#: had created ``reports`` and failed ``Is a directory``.
DIVERGENT_SPELLING = "reports/."

#: The parent coreutils ``dirname(1)`` names for that spelling.
DIVERGENT_PARENT = "reports"

#: The variables destination resolution reads, so each case states only the one
#: it varies.
BASELINE_ENVIRONMENT = {TEMPORARY_DIRECTORY_VARIABLE: TEMPORARY_DIRECTORY}


def test_default_artefact_paths_are_derived_from_the_temporary_directory() -> None:
    """Verify both default paths hang off ``RUNNER_TEMP``.

    Notes
    -----
    ``RUNNER_TEMP`` is read with the colon form, so an empty value must reach
    the default exactly as an unset one does. The empty arm is the reason: a
    resolver reading the no-colon form would build a path from the empty string
    and pass the unset arm alone.
    """
    variables = dict(BASELINE_ENVIRONMENT)
    del variables[TEMPORARY_DIRECTORY_VARIABLE]
    fallback = configuration.DEFAULT_TEMPORARY_DIRECTORY
    expected = (
        f"{fallback}/{configuration.METRICS_FILE_NAME}",
        f"{fallback}/{configuration.TRACE_FILE_NAME}",
    )

    unset = configuration.resolve_destination(variables)
    empty = configuration.resolve_destination({
        **BASELINE_ENVIRONMENT,
        TEMPORARY_DIRECTORY_VARIABLE: "",
    })

    assert (unset.metrics_file.configured, unset.trace_file.configured) == expected, (
        "an unset RUNNER_TEMP must name both default artefacts under the fallback"
    )
    assert empty == unset, "an empty RUNNER_TEMP must fall back as an unset one does"


@pytest.mark.parametrize(
    "variable",
    [
        pytest.param(METRICS_FILE_VARIABLE, id="metrics"),
        pytest.param(TRACE_FILE_VARIABLE, id="trace"),
    ],
)
def test_configured_spelling_is_kept_while_its_parent_is_derived(variable: str) -> None:
    """Verify a configured path is kept verbatim with its parent derived.

    Notes
    -----
    The two halves are one contract, not two claims: the configured string
    reaches every observable boundary unchanged, while the *parent* is what
    ``dirname(1)`` would have named. For this spelling the parent differs from
    the configured path's normalized :attr:`~pathlib.Path.parent`, so a resolver
    computing either half from a :class:`~pathlib.Path` fails here rather than at
    some later refusal.
    """
    destination = configuration.resolve_destination({
        **BASELINE_ENVIRONMENT,
        variable: DIVERGENT_SPELLING,
    })
    configured = (
        destination.metrics_file
        if variable == METRICS_FILE_VARIABLE
        else destination.trace_file
    )

    assert configured.configured == DIVERGENT_SPELLING, (
        "resolution must keep the operator's own spelling rather than normalize it"
    )
    assert str(Path(configured.configured)) != configured.configured, (
        "this case only proves the spelling was kept if Path would have changed it"
    )
    assert configured.directory == Path(DIVERGENT_PARENT), (
        "the parent must be the one dirname(1) named for the configured string"
    )
    assert configured.directory != Path(configured.configured).parent, (
        "the parent must not be the configured path's own Path.parent"
    )
