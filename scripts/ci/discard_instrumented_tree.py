#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.2", "cuprum @ git+https://github.com/leynos/cuprum@a2134c7a3966b224eaed917efb94f8090ce5104a"]
# ///
"""Remove the instrumented coverage build trees before any cache save.

Once the coverage report exists the ``llvm-cov`` trees have no consumer, and
they are the second build tree on a volume a sibling repository has exhausted
before. Disk usage is shown before and after so the log records what the
removal freed. A tree that does not exist is not an error.

Parameters arrive from the environment: ``INPUT_TARGET_DIR`` overrides the
Cargo target directory and ``INPUT_SUBTREES`` the whitespace-separated names
removed beneath it.
"""

import pathlib
import shutil
import sys

from ci_support import CiScriptError, catalogue, describe_failure, run

import cyclopts
from cyclopts import App

DF = "df"
ALLOWED = catalogue(DF)
DEFAULT_SUBTREES = ("llvm-cov-target", "llvm-cov")

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def show_disk_usage() -> None:
    """Print ``df -h .`` so the log records the volume's state."""
    result = run(DF, "-h", ".", allowed=ALLOWED)
    if not result.ok:
        message = f"reading disk usage failed: {describe_failure(result)}"
        raise CiScriptError(message)


def discard(target_dir: pathlib.Path, subtrees: tuple[str, ...]) -> list[pathlib.Path]:
    """Remove each existing subtree under ``target_dir`` and return them."""
    removed = []
    for name in subtrees:
        tree = target_dir / name
        if tree.is_symlink() or tree.is_file():
            tree.unlink()
            removed.append(tree)
        elif tree.is_dir():
            shutil.rmtree(tree)
            removed.append(tree)
    return removed


@app.default
def main(
    *,
    target_dir: pathlib.Path = pathlib.Path("target"),
    subtrees: list[str] | None = None,
) -> int:
    """Discard the instrumented trees, reporting disk usage around the removal.

    Parameters
    ----------
    target_dir
        The Cargo target directory holding the instrumented trees.
    subtrees
        Names beneath ``target_dir`` to remove; the two ``llvm-cov`` trees by
        default.

    Returns
    -------
    int
        ``0`` on success, ``1`` if disk usage or a removal fails.
    """
    names = tuple(subtrees) if subtrees else DEFAULT_SUBTREES
    try:
        show_disk_usage()
        removed = discard(target_dir, names)
        show_disk_usage()
    except (CiScriptError, OSError) as error:
        print(f"discard_instrumented_tree: {error}", file=sys.stderr)
        return 1
    for tree in removed:
        print(f"removed {tree}")
    return 0


if __name__ == "__main__":
    app()
