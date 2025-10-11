"""Template helpers used by the staging pipeline."""

from __future__ import annotations

import dataclasses
import typing as typ
from pathlib import Path

from .errors import StageError
from .glob_utils import match_candidate_path

if typ.TYPE_CHECKING:
    from .config import ArtefactConfig

__all__ = [
    "RenderAttempt",
    "render_template",
    "resolve_artefact_source",
]


@dataclasses.dataclass(slots=True)
class RenderAttempt:
    """Template render attempted when locating an artefact."""

    template: str
    rendered: str


def render_template(template: str, context: dict[str, typ.Any]) -> str:
    """Return ``template`` formatted with ``context``."""

    try:
        return template.format(**context)
    except KeyError as exc:  # pragma: no cover - defensive guard.
        message = f"Invalid template key {exc} in '{template}'"
        raise StageError(message) from exc


def resolve_artefact_source(
    workspace: Path, artefact: ArtefactConfig, context: dict[str, typ.Any]
) -> tuple[Path | None, list[RenderAttempt]]:
    """Return the first artefact path matching ``artefact``'s templates."""

    attempts: list[RenderAttempt] = []
    patterns = [artefact.source, *artefact.alternatives]
    for pattern in patterns:
        rendered = render_template(pattern, context)
        attempts.append(RenderAttempt(pattern, rendered))
        if (candidate := match_candidate_path(workspace, rendered)) is not None:
            return candidate, attempts
    return None, attempts
