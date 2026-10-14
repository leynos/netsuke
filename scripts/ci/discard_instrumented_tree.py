#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
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

from ci_support import CiScriptError, RunSpec, catalogue, describe_failure, run

import cyclopts
from cyclopts import App

DF = "df"
ALLOWED = catalogue(DF)
DEFAULT_SUBTREES = ("llvm-cov-target", "llvm-cov")

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def show_disk_usage() -> None:
    """Print ``df -h .`` so the log records the volume's state."""
    result = run(DF, "-h", ".", spec=RunSpec(ALLOWED))
    if not result.ok:
        message = f"reading disk usage failed: {describe_failure(result)}"
        raise CiScriptError(message)


def _names_a_plain_component(name: str) -> bool:
    """Return whether ``name`` is one ordinary path component.

    Absolute names, names carrying a separator, and the three names that do not
    denote a distinct child -- the empty string, ``.`` and ``..`` -- are all
    rejected, so an accepted name always denotes one entry directly beneath the
    directory it is joined to.

    Returns
    -------
    bool
        ``True`` only for a name that resolves to one ordinary component.
    """
    component = pathlib.PurePosixPath(name)
    return (
        not component.is_absolute()
        and len(component.parts) == 1
        and component.parts[0] not in {"", ".", ".."}
    )


def _child_of(target_dir: pathlib.Path, name: str) -> pathlib.Path:
    """Return ``name`` as a direct child of ``target_dir``, or refuse it.

    The removal below is destructive, so the name is resolved through this
    check rather than trusted. A bare ``target_dir / name`` is not enough:
    an absolute ``name`` discards ``target_dir`` entirely, ``..`` walks out of
    it, and ``.`` or an empty name resolves to ``target_dir`` itself, which
    would remove the whole Cargo target directory rather than one instrumented
    tree inside it. Requiring a single plain path component makes "beneath
    ``target_dir``" a property of the path's shape instead of a claim about the
    caller's input.

    Parameters
    ----------
    target_dir
        The Cargo target directory the removal is confined to.
    name
        One subtree name, which must be a plain path component.

    Returns
    -------
    pathlib.Path
        ``target_dir`` joined with ``name``.

    Raises
    ------
    CiScriptError
        If ``name`` is not a single plain path component.
    """
    if not _names_a_plain_component(name):
        message = (
            f"refusing to remove {name!r}: a subtree name must be a single "
            f"plain path component beneath {target_dir}"
        )
        raise CiScriptError(message)
    return target_dir / name


def discard(target_dir: pathlib.Path, subtrees: tuple[str, ...]) -> list[pathlib.Path]:
    """Remove each existing subtree under ``target_dir`` and return them.

    Every name is validated by :func:`_child_of` before anything is touched, so
    the removal cannot reach outside ``target_dir``: a name that is not a
    single plain path component raises :class:`CiScriptError` while every
    named tree is still in place. The validation is hoisted into its own pass
    for that second property: validating inside the removal loop would remove
    the trees named before the bad one and then raise, so a caller passing
    ``("llvm-cov", "..")`` would lose ``llvm-cov`` and be told the step failed.
    Resolving the whole list first makes the all-or-nothing claim true rather
    than merely intended.

    The exception is documented here rather than under a ``Raises`` heading
    because it is raised by :func:`_child_of`, not by this function's own body.

    Returns
    -------
    list[pathlib.Path]
        The paths that were removed, in ``subtrees`` order. A name with
        nothing at it is skipped rather than reported.
    """
    trees = [_child_of(target_dir, name) for name in subtrees]
    removed = []
    for tree in trees:
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
