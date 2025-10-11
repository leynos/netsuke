"""Error types shared across the staging helper package."""


class StageError(RuntimeError):
    """Raised when the staging pipeline cannot continue."""
