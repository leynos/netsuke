"""Optional dependency loaders and CLI fallback for the release uploader."""

from __future__ import annotations

import os
import sys
from argparse import ArgumentParser
from pathlib import Path
from typing import Any, Callable, NamedTuple, Sequence

__all__ = [
    "CycloptsSupport",
    "PlumbumSupport",
    "RuntimeOptions",
    "load_cyclopts",
    "load_plumbum",
    "run_cli",
]


class _FallbackParameter:
    """Placeholder preserving ``typing.Annotated`` compatibility."""

    def __init__(self, *_args: object, **_kwargs: object) -> None:
        """Accept and ignore all arguments."""


class _FallbackApp:
    """Stub ``cyclopts.App`` that raises a descriptive error when invoked."""

    def __init__(self, cause: ModuleNotFoundError) -> None:
        self._cause = cause

    def default(self, func: Callable[..., Any]) -> Callable[..., Any]:
        return func

    def __call__(self, *_args: object, **_kwargs: object) -> None:  # pragma: no cover
        message = "Cyclopts is required for CLI usage; install it to run the script"
        raise RuntimeError(message) from self._cause


class _MissingLocal:
    """Placeholder mirroring the ``plumbum.local`` interface."""

    def __init__(self, cause: ModuleNotFoundError) -> None:
        self._cause = cause

    def __getitem__(self, name: str) -> None:  # pragma: no cover - defensive guard.
        message = (
            "plumbum is required to execute release uploads; "
            "install it or run in dry-run mode"
        )
        raise ModuleNotFoundError(message) from self._cause


class CycloptsSupport(NamedTuple):
    """Expose Cyclopts components and availability information."""

    available: bool
    app: Any
    parameter: type


class PlumbumSupport(NamedTuple):
    """Expose Plumbum components required by the uploader."""

    local: Any
    command_not_found: type[Exception]
    process_execution_error: type[Exception]
    bound_command: Any


class RuntimeOptions(NamedTuple):
    """Arguments required by :func:`upload_release_assets.main`."""

    release_tag: str
    bin_name: str
    dist_dir: Path
    dry_run: bool


def load_cyclopts() -> CycloptsSupport:
    """Return Cyclopts components with graceful fallbacks."""

    try:
        import cyclopts as _cyclopts
        from cyclopts import App, Parameter
    except ModuleNotFoundError as exc:  # pragma: no cover - lean test environments.
        return CycloptsSupport(False, _FallbackApp(exc), _FallbackParameter)

    app = App(config=_cyclopts.config.Env("INPUT_", command=False))
    return CycloptsSupport(True, app, Parameter)


def load_plumbum() -> PlumbumSupport:
    """Return Plumbum components with graceful fallbacks."""

    try:  # pragma: no cover - exercised indirectly when dependencies exist.
        from plumbum import local  # type: ignore[import-not-found]
        from plumbum.commands import (  # type: ignore[import-not-found]
            CommandNotFound,
            ProcessExecutionError,
        )
        from plumbum.commands.base import BoundCommand  # type: ignore[import-not-found]
    except ModuleNotFoundError as exc:  # pragma: no cover - lean type-check envs.
        return PlumbumSupport(
            local=_MissingLocal(exc),
            command_not_found=ModuleNotFoundError,
            process_execution_error=RuntimeError,
            bound_command=Any,
        )

    return PlumbumSupport(
        local=local,
        command_not_found=CommandNotFound,
        process_execution_error=ProcessExecutionError,
        bound_command=BoundCommand,
    )


def run_cli(
    support: CycloptsSupport,
    *,
    prepare_options: Callable[..., RuntimeOptions],
    main: Callable[..., int],
    tokens: Sequence[str] | None = None,
) -> int:
    """Execute the uploader CLI using Cyclopts or ``argparse`` fallback."""

    if support.available:
        return support.app()

    parser = ArgumentParser(description=main.__doc__ or "Upload release assets.")
    parser.add_argument("--release-tag")
    parser.add_argument("--bin-name")
    parser.add_argument("--dist-dir")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(list(tokens) if tokens is not None else sys.argv[1:])

    try:
        options = prepare_options(
            inputs={
                "release_tag": args.release_tag,
                "bin_name": args.bin_name,
                "dist_dir": args.dist_dir,
                "dry_run": args.dry_run,
            },
            environ=os.environ,
        )
    except ValueError as exc:
        parser.exit(status=1, message=f"{exc}\n")

    return main(
        release_tag=options.release_tag,
        bin_name=options.bin_name,
        dist_dir=options.dist_dir,
        dry_run=options.dry_run,
    )
