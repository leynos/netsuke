"""Public interface for the staging helper package."""

from .config import ArtefactConfig, StagingConfig, load_config
from .errors import StageError
from .staging import StageResult, stage_artefacts

__all__ = [
    "ArtefactConfig",
    "StageError",
    "StageResult",
    "StagingConfig",
    "load_config",
    "stage_artefacts",
]
