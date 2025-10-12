"""Staging pipeline package exposing artefact staging utilities."""

from .output import (
    RESERVED_OUTPUT_KEYS,
    _prepare_output_data,
    _validate_no_reserved_key_collisions,
    write_github_output,
)
from .pipeline import (
    StageResult,
    StagedArtefact,
    _RenderAttempt,
    _StagingEnvironment,
    _ensure_source_available,
    _initialize_staging_dir,
    _iter_staged_artefacts,
    _render_template,
    _resolve_artefact_source,
    _safe_destination_path,
    _stage_single_artefact,
    _write_checksum,
    stage_artefacts,
)
from .resolution import _match_candidate_path

__all__ = [
    "RESERVED_OUTPUT_KEYS",
    "StageResult",
    "StagedArtefact",
    "stage_artefacts",
    "write_github_output",
]
