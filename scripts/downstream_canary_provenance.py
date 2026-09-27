"""Build the bounded provenance record for one downstream migration canary.

A release reviewer needs to know which downstream repository and revision ran,
against which Netsuke commit and version, on which platform and lane, and which
targets passed. This module owns that record's schema and its closed status
vocabulary, and renders the one-line job summary. It performs no process work
of its own: ``run_downstream_canary.py`` supplies the recorded step state and
the observed downstream ``HEAD``.

The record is a provenance artefact, not a metric or trace series: it carries
identifiers deliberately, and nothing here emits them as metric labels.
"""

import collections.abc as cabc
import dataclasses

#: The provenance record's schema identifier.
SCHEMA = "netsuke.downstream-canary.provenance/v1"
PASSED = "passed"
FAILED = "failed"
NOT_RUN = "not_run"
#: Every status a phase or target may report.
STATUSES = frozenset({PASSED, FAILED, NOT_RUN})


@dataclasses.dataclass(frozen=True, slots=True)
class CanaryIdentity:
    """Identify one canary run for release review.

    Attributes
    ----------
    canary
        The matrix row's name, such as ``mxd-postgres``.
    repository
        The downstream ``owner/name``.
    downstream_revision
        The pinned downstream commit the run must use.
    netsuke_revision
        The Netsuke candidate commit.
    netsuke_version
        The candidate's package version.
    platform
        The runner's operating system.
    selectors
        Lane selector variables, such as ``{"MXD_BACKEND": "sqlite"}``.
    targets
        The requested targets, in run order.
    """

    canary: str
    repository: str
    downstream_revision: str
    netsuke_revision: str
    netsuke_version: str
    platform: str
    selectors: cabc.Mapping[str, str]
    targets: tuple[str, ...]


def bounded_status(value: object) -> str:
    """Return ``value`` when it is a known status, otherwise ``not_run``.

    The state file is written by earlier steps of the same job, but a missing
    or damaged entry must still produce a record inside the vocabulary.

    Parameters
    ----------
    value
        A recorded status, or anything else.

    Returns
    -------
    str
        A member of :data:`STATUSES`.

    Examples
    --------
    >>> bounded_status("failed")
    'failed'
    >>> bounded_status(None)
    'not_run'
    """
    return value if isinstance(value, str) and value in STATUSES else NOT_RUN


def provenance_record(
    identity: CanaryIdentity,
    state: cabc.Mapping[str, object],
    downstream_head: str | None,
) -> dict[str, object]:
    """Build the provenance record from the run's identity and state.

    The outcome passes only when generation, lane isolation, and every target
    passed, and the checkout's ``HEAD`` is the pinned downstream revision.

    Parameters
    ----------
    identity
        The run's identity.
    state
        The state the earlier steps recorded, possibly empty.
    downstream_head
        The checkout's observed ``HEAD``, or ``None`` when unreadable.

    Returns
    -------
    dict[str, object]
        The provenance record.
    """
    recorded = state.get("targets")
    statuses = recorded if isinstance(recorded, cabc.Mapping) else {}
    targets = [
        {"name": target, "status": bounded_status(statuses.get(target))}
        for target in identity.targets
    ]
    generate = bounded_status(state.get("generate"))
    isolation = bounded_status(state.get("isolation"))
    passed = (
        generate == PASSED
        and isolation == PASSED
        and all(target["status"] == PASSED for target in targets)
        and downstream_head == identity.downstream_revision
    )
    return {
        "schema": SCHEMA,
        "canary": identity.canary,
        "repository": identity.repository,
        "downstream_revision": identity.downstream_revision,
        "downstream_head": downstream_head,
        "netsuke_revision": identity.netsuke_revision,
        "netsuke_version": identity.netsuke_version,
        "platform": identity.platform,
        "selectors": dict(identity.selectors),
        "generate": generate,
        "isolation": isolation,
        "targets": targets,
        "outcome": PASSED if passed else FAILED,
    }


def summary_line(identity: CanaryIdentity, record: cabc.Mapping[str, object]) -> str:
    """Render one Markdown job-summary line for a provenance record.

    Parameters
    ----------
    identity
        The run's identity.
    record
        The record :func:`provenance_record` built for that identity.

    Returns
    -------
    str
        The summary line, ending in a newline.

    Examples
    --------
    >>> identity = CanaryIdentity(
    ...     "mxd-sqlite", "leynos/mxd", "a" * 40, "b" * 40, "0.1.0", "Linux",
    ...     {"MXD_BACKEND": "sqlite"}, ("test",))
    >>> record = provenance_record(
    ...     identity, {"generate": "passed", "isolation": "passed",
    ...                "targets": {"test": "passed"}}, "a" * 40)
    >>> summary_line(identity, record).rstrip()
    '- **mxd-sqlite** (sqlite) on Linux: passed. `leynos/mxd@aaaaaaaaaaaa` with Netsuke `bbbbbbbbbbbb` (0.1.0); targets: `test` passed'
    """  # ruff: ignore[line-too-long] - the doctest output is one summary line.
    lane = f" ({', '.join(identity.selectors.values())})" if identity.selectors else ""
    targets = record.get("targets")
    rendered = ", ".join(
        f"`{target['name']}` {target['status']}"
        for target in (targets if isinstance(targets, list) else [])
    )
    return (
        f"- **{identity.canary}**{lane} on {identity.platform}: "
        f"{record.get('outcome', FAILED)}. `{identity.repository}@"
        f"{identity.downstream_revision[:12]}` with Netsuke "
        f"`{identity.netsuke_revision[:12]}` ({identity.netsuke_version}); "
        f"targets: {rendered}\n"
    )
