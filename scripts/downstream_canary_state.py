"""Read and write the state the downstream canary steps share.

``generate``, ``run``, and ``report`` are separate workflow steps, so they hand
their phase statuses on through one small JSON file. The loader reports an
unusable file as :class:`StateUnreadableError` rather than an empty state;
:func:`state_or_nothing_run` is the edges' shared decision that such a file
means nothing was recorded, so every unreached phase reports ``not_run``.
"""

import json
import sys
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path


class StateUnreadableError(Exception):
    """The shared step state is missing, unreadable, or not a JSON object."""


def read_state(path: Path) -> dict[str, object]:
    """Read the shared step state.

    Parameters
    ----------
    path
        The state file.

    Returns
    -------
    dict[str, object]
        The recorded state.

    Raises
    ------
    StateUnreadableError
        When the file is missing or unreadable, is not JSON, or does not hold
        an object. The caller decides what an unusable state means.
    """
    try:
        state = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        msg = f"cannot read the step state: {type(error).__name__}"
        raise StateUnreadableError(msg) from error
    if not isinstance(state, dict):
        msg = "the step state is not a JSON object"
        raise StateUnreadableError(msg)
    return state


def state_or_nothing_run(path: Path) -> dict[str, object]:
    """Return the step state, or an empty one when it cannot be read.

    An edge's decision, not the loader's: ``run`` and ``report`` always
    proceed, and an empty state records every unreached phase as ``not_run``.
    The reason is printed, so the job log shows why nothing was recorded.

    Parameters
    ----------
    path
        The state file.

    Returns
    -------
    dict[str, object]
        The recorded state, or ``{}`` when it cannot be read.
    """
    try:
        return read_state(path)
    except StateUnreadableError as error:
        print(f"{error}; recording unreached phases as not_run", file=sys.stderr)
        return {}


def save_state(path: Path, state: cabc.Mapping[str, object]) -> None:
    """Write the shared step state.

    Parameters
    ----------
    path
        The state file.
    state
        The state to record.
    """
    path.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")


__all__ = [
    "StateUnreadableError",
    "read_state",
    "save_state",
    "state_or_nothing_run",
]
