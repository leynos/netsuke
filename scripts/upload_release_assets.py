#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9", "plumbum"]
# ///

"""Upload packaged release artefacts to a GitHub release.

The script discovers artefacts in a staging directory, validates their
filenames and sizes, and optionally uploads them using the GitHub CLI. It is
idempotent and supports a dry-run mode used by the release dry-run workflow to
assert expected asset names without mutating state.

Examples
--------
Upload artefacts to the ``v1.2.3`` release::

    upload_release_assets --release-tag v1.2.3 --bin-name netsuke

Inspect the planned uploads without publishing anything::

    upload_release_assets --release-tag v1.2.3 --bin-name netsuke --dry-run
"""

from __future__ import annotations

import dataclasses as dc
import sys
import typing as typ
from pathlib import Path

try:
    import cyclopts as _cyclopts
    from cyclopts import App, Parameter
except ModuleNotFoundError:  # pragma: no cover - executed in lean test envs.
    cyclopts: object | None = None

    class Parameter:  # type: ignore[empty-body]
        """Fallback placeholder that preserves ``typing.Annotated`` usage."""

        def __init__(self, *args: object, **kwargs: object) -> None:
            """Store arguments for debugging; behaviour is irrelevant."""
            self.args = args
            self.kwargs = kwargs

    class App:
        """Minimal shim to surface a descriptive error when Cyclopts is absent."""

        def __init__(self, *args: object, **kwargs: object) -> None:
            message = "Cyclopts is required for CLI usage; install it to run the script"
            self._error = RuntimeError(message)

        def default(
            self, func: typ.Callable[..., int]
        ) -> typ.Callable[..., int]:  # pragma: no cover - trivial stub
            """Return ``func`` unchanged in fallback mode."""
            return func

        def __call__(self) -> int:  # pragma: no cover - trivial stub
            """Raise because the CLI requires Cyclopts."""
            raise self._error
else:
    cyclopts = _cyclopts

try:  # pragma: no cover - exercised indirectly when dependencies are present.
    from plumbum import local  # type: ignore[import-not-found]
    from plumbum.commands import (  # type: ignore[import-not-found]
        CommandNotFound,
        ProcessExecutionError,
    )
except ModuleNotFoundError:  # pragma: no cover - executed in test/type-check envs.
    CommandNotFound = ModuleNotFoundError
    ProcessExecutionError = RuntimeError

    class _MissingLocal:
        """Placeholder that mirrors the ``plumbum.local`` interface."""

        def __getitem__(self, name: str) -> typ.NoReturn:
            message = (
                "plumbum is required to execute release uploads; "
                "install it or run in dry-run mode"
            )
            raise ModuleNotFoundError(message)

    local = _MissingLocal()

if typ.TYPE_CHECKING:
    from plumbum.commands.base import BoundCommand  # type: ignore[import-not-found]
else:
    BoundCommand = typ.Any


class AssetError(RuntimeError):
    """Raised when the staged artefacts are invalid."""


@dc.dataclass(frozen=True)
class ReleaseAsset:
    """Artefact staged for upload to a GitHub release."""

    path: Path
    asset_name: str
    size: int


if cyclopts is not None:
    app: App = App(config=cyclopts.config.Env("INPUT_", command=False))
else:
    app = App()


def _needs_manual_cli(tokens: list[str]) -> bool:
    """Return ``True`` when ``tokens`` require the argparse fallback."""
    if cyclopts is None:
        return True
    for index, argument in enumerate(tokens):
        if argument == "--dry-run":
            if index + 1 == len(tokens):
                return True
            next_token = tokens[index + 1]
            return bool(next_token.startswith("-"))
        if argument.startswith("--dry-run="):
            return False
    return False


def _is_candidate(path: Path, bin_name: str) -> bool:
    name = path.name
    if name in {bin_name, f"{bin_name}.exe", f"{bin_name}.1"}:
        return True
    if name.endswith(".sha256"):
        return True
    return path.suffix in {".deb", ".rpm", ".pkg", ".msi"}


def _coerce_bool(value: object) -> bool:
    """Return ``value`` as a strict boolean."""
    if isinstance(value, bool):
        return value
    if not isinstance(value, str):
        message = f"Cannot interpret {value!r} as a boolean"
        raise TypeError(message)
    normalised = value.strip().lower()
    if normalised in {"", "false", "0", "no", "off"}:
        return False
    if normalised in {"true", "1", "yes", "on"}:
        return True
    message = f"Cannot interpret {value!r} as a boolean"
    raise ValueError(message)


def _resolve_asset_name(path: Path, *, dist_dir: Path) -> str:
    """Return a unique asset name derived from ``path`` within ``dist_dir``."""
    relative_path = path.relative_to(dist_dir)
    if relative_path.parent == Path():
        return relative_path.name
    prefix = relative_path.parent.as_posix().replace("/", "__")
    return f"{prefix}-{relative_path.name}"


def _iter_candidate_paths(dist_dir: Path, bin_name: str) -> typ.Iterator[Path]:
    for path in sorted(dist_dir.rglob("*")):
        if path.is_file() and _is_candidate(path, bin_name):
            yield path


def _require_non_empty(path: Path) -> int:
    size = path.stat().st_size
    if size <= 0:
        message = f"Artefact {path} is empty"
        raise AssetError(message)
    return size


def _register_asset(asset_name: str, path: Path, seen: dict[str, Path]) -> None:
    if previous := seen.get(asset_name):
        message = (
            "Asset name collision: "
            f"{asset_name} would upload both {previous} and {path}"
        )
        raise AssetError(message)
    seen[asset_name] = path


def discover_assets(dist_dir: Path, *, bin_name: str) -> list[ReleaseAsset]:
    """Return the artefacts that should be published.

    Parameters
    ----------
    dist_dir : Path
        Root directory that contains the staged artefacts.
    bin_name : str
        Binary name used to match platform-specific artefacts.

    Returns
    -------
    list[ReleaseAsset]
        Ordered collection of artefacts ready to upload.

    Raises
    ------
    AssetError
        If no artefacts are found, an artefact is empty, or multiple files would
        upload with the same asset name.

    Examples
    --------
    >>> discover_assets(Path("dist"), bin_name="netsuke")  # doctest: +SKIP
    [ReleaseAsset(path=PosixPath('dist/netsuke'), ...)]
    """
    if not dist_dir.exists():
        message = f"Artefact directory {dist_dir} does not exist"
        raise AssetError(message)

    assets: list[ReleaseAsset] = []
    seen: dict[str, Path] = {}

    for path in _iter_candidate_paths(dist_dir, bin_name):
        size = _require_non_empty(path)
        asset_name = _resolve_asset_name(path, dist_dir=dist_dir)
        _register_asset(asset_name, path, seen)
        assets.append(ReleaseAsset(path=path, asset_name=asset_name, size=size))

    if not assets:
        message = f"No artefacts discovered in {dist_dir}"
        raise AssetError(message)

    return assets


def _render_summary(assets: typ.Iterable[ReleaseAsset]) -> str:
    lines = ["Planned uploads:"]
    lines.extend(
        f"  - {asset.asset_name} ({asset.size} bytes) -> {asset.path}"
        for asset in assets
    )
    return "\n".join(lines)


def upload_assets(
    *, release_tag: str, assets: typ.Iterable[ReleaseAsset], dry_run: bool = False
) -> None:
    """Upload artefacts to GitHub using the ``gh`` CLI.

    Parameters
    ----------
    release_tag : str
        Git tag identifying the release that should receive the artefacts.
    assets : Iterable[ReleaseAsset]
        Iterable of artefacts to publish.
    dry_run : bool
        When ``True``, print the planned ``gh`` invocations without executing
        them.

    Raises
    ------
    ProcessExecutionError
        If ``gh`` returns a non-zero status while uploading.
    CommandNotFound
        If the ``gh`` executable is not available in ``PATH``.

    Examples
    --------
    >>> upload_assets(  # doctest: +SKIP
    ...     release_tag="v1.2.3",
    ...     assets=[ReleaseAsset(Path("dist/netsuke"), "netsuke", 1024)],
    ...     dry_run=True,
    ... )
    """
    gh_cmd: BoundCommand | None = None
    for asset in assets:
        descriptor = f"{asset.path}#{asset.asset_name}"
        if dry_run:
            print(f"[dry-run] gh release upload {release_tag} {descriptor} --clobber")
            continue
        if gh_cmd is None:
            gh_cmd = local["gh"]
        gh_cmd[
            "release",
            "upload",
            release_tag,
            descriptor,
            "--clobber",
        ]()


def main(
    *,
    release_tag: str,
    bin_name: str,
    dist_dir: Path = Path("dist"),
    dry_run: bool = False,
) -> int:
    """Entry point shared by the CLI and tests.

    Parameters
    ----------
    release_tag : str
        Git tag identifying the release to publish to.
    bin_name : str
        Binary name used to derive artefact names during discovery.
    dist_dir : Path
        Directory containing staged artefacts.
    dry_run : bool
        When ``True``, validate artefacts and print the upload plan without
        uploading.

    Returns
    -------
    int
        Exit code: ``0`` on success, ``1`` when artefact discovery or upload
        fails.

    Examples
    --------
    >>> main(  # doctest: +SKIP
    ...     release_tag="v1.2.3",
    ...     bin_name="netsuke",
    ...     dist_dir=Path("dist"),
    ...     dry_run=True,
    ... )
    0
    """
    try:
        assets = discover_assets(dist_dir, bin_name=bin_name)
    except AssetError as exc:
        print(exc, file=sys.stderr)
        return 1

    if dry_run:
        print(_render_summary(assets))

    try:
        upload_assets(release_tag=release_tag, assets=assets, dry_run=dry_run)
    except (ProcessExecutionError, CommandNotFound) as exc:  # pragma: no cover
        print(exc, file=sys.stderr)
        return 1

    return 0


@app.default
def cli(
    *,
    release_tag: typ.Annotated[str, Parameter(required=True)],
    bin_name: typ.Annotated[str, Parameter(required=True)],
    dist_dir: Path = Path("dist"),
    dry_run: bool = False,
) -> int:
    """Cyclopts-bound CLI entry point."""
    return main(
        release_tag=release_tag,
        bin_name=bin_name,
        dist_dir=dist_dir,
        dry_run=_coerce_bool(dry_run),
    )


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    tokens = sys.argv[1:]
    if _needs_manual_cli(tokens):
        import argparse

        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--release-tag", required=True)
        parser.add_argument("--bin-name", required=True)
        parser.add_argument("--dist-dir", default="dist")
        parser.add_argument("--dry-run", action="store_true")
        args = parser.parse_args(tokens)
        exit_code = main(
            release_tag=args.release_tag,
            bin_name=args.bin_name,
            dist_dir=Path(args.dist_dir),
            dry_run=args.dry_run,
        )
        raise SystemExit(exit_code)
    raise SystemExit(app())
