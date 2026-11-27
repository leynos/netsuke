"""Judge whether a downstream canary's generated manifest stays in its lane.

The MXD canaries build one workspace three ways, selected by ``MXD_BACKEND``.
A lane's non-interference contract is that no other lane's commands survive
manifest-time expansion, so the ``run`` step refuses a manifest that contains
any of the lane's forbidden patterns before running a target. The verdict and
the step's exit status are pure functions here; ``run_downstream_canary.py``
reads the manifest, prints, and records the state.
"""

import typing as typ

from downstream_canary_provenance import FAILED, PASSED

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def forbidden_matches(manifest: str, patterns: cabc.Iterable[str]) -> list[str]:
    r"""Return the patterns that occur in a generated Ninja manifest.

    A lane's non-interference contract is that no other lane's commands
    survive manifest-time expansion, so a match is a canary failure.

    Parameters
    ----------
    manifest
        The generated manifest's text.
    patterns
        Literal substrings the manifest must not contain.

    Returns
    -------
    list[str]
        The patterns found, in the order given.

    Examples
    --------
    >>> forbidden_matches("build test: phony\n", ["postgres", "test"])
    ['test']
    """
    return [pattern for pattern in patterns if pattern in manifest]


class Isolation(typ.NamedTuple):
    """The lane-isolation verdict on one generated manifest.

    Examples
    --------
    >>> Isolation(()).status, Isolation(("postgres",)).status
    ('passed', 'failed')
    """

    found: tuple[str, ...]

    @property
    def holds(self) -> bool:
        """Whether no other lane's pattern reached the manifest."""
        return not self.found

    @property
    def status(self) -> str:
        """The phase status this verdict records: ``passed`` or ``failed``."""
        return PASSED if self.holds else FAILED


def judge_isolation(manifest: str, patterns: cabc.Iterable[str]) -> Isolation:
    r"""Judge whether a generated manifest stays inside its lane.

    Parameters
    ----------
    manifest
        The generated manifest's text.
    patterns
        Literal substrings another lane's commands would leave behind.

    Returns
    -------
    Isolation
        The verdict, naming every pattern found in the order given.

    Examples
    --------
    >>> judge_isolation("build x: phony postgres\n", ["postgres"]).found
    ('postgres',)
    """
    return Isolation(tuple(forbidden_matches(manifest, patterns)))


def run_status(isolation: Isolation, statuses: cabc.Mapping[str, str]) -> int:
    """Return the ``run`` step's exit status for a verdict and target statuses.

    Parameters
    ----------
    isolation
        The manifest's lane-isolation verdict.
    statuses
        Each target's recorded status.

    Returns
    -------
    int
        ``0`` when isolation holds and no target failed, otherwise ``1``.

    Examples
    --------
    >>> run_status(Isolation(()), {"lint": "passed"})
    0
    >>> run_status(Isolation(()), {"lint": "failed"})
    1
    >>> run_status(Isolation(("postgres",)), {"lint": "not_run"})
    1
    """
    return 0 if isolation.holds and FAILED not in statuses.values() else 1


__all__ = ["Isolation", "forbidden_matches", "judge_isolation", "run_status"]
