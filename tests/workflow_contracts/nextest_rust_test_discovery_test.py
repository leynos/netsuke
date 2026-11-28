"""Guard the discovery corpus's refusal to read an incomplete tree.

The default corpus spans `src/` and `tests/`. `Path.rglob` suppresses
enumeration errors, so the reader walks with `Path.walk(on_error=...)`;
without that callback an unreadable subtree would shrink the corpus silently.
Run via ``make test-workflow-contracts``.
"""

import typing as typ
from pathlib import Path

import pytest
from nextest_rust_test_discovery import rust_test_sources
from workflow_loading import WorkflowReadError

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def test_an_enumeration_failure_is_raised_for_the_failed_directory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An unreadable subtree fails the read instead of shrinking the corpus."""
    root = tmp_path / "src"
    root.mkdir()
    failed = root / "locked"
    failed.mkdir()

    def refuse(
        _self: Path,
        *,
        _top_down: bool = True,
        on_error: cabc.Callable[[OSError], object] | None = None,
        _follow_symlinks: bool = False,
    ) -> cabc.Iterator[tuple[Path, list[str], list[str]]]:
        """Invoke the reader's error callback and yield no entries."""
        if on_error is not None:
            on_error(PermissionError(13, "Permission denied", str(failed)))
        return iter(())

    monkeypatch.setattr(Path, "walk", refuse)
    with pytest.raises(WorkflowReadError, match="could not be enumerated"):
        rust_test_sources(root)
