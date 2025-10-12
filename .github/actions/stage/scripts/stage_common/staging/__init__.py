"""Staging pipeline package exposing artefact staging utilities."""

from .output import RESERVED_OUTPUT_KEYS, write_github_output
from .pipeline import StageResult, StagedArtefact, stage_artefacts

__all__ = [
    "RESERVED_OUTPUT_KEYS",
    "StageResult",
    "StagedArtefact",
    "stage_artefacts",
    "write_github_output",
]
