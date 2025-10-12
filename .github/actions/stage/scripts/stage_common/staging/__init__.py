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
    _initialize_staging_dir,
    _iter_staged_artefacts,
    _stage_single_artefact,
    stage_artefacts,
)
from .resolution import (
    _contains_glob,
    _glob_root_and_pattern,
    _iter_absolute_matches,
    _match_candidate_path,
    _match_glob_candidate,
    _mtime_key,
    _newest_file,
)

__all__ = [
    "RESERVED_OUTPUT_KEYS",
    "StageResult",
    "StagedArtefact",
    "stage_artefacts",
    "write_github_output",
]

