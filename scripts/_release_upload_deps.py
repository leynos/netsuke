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
    "load_cyclopts",
    "load_plumbum",
    "run_cli",
]


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


def load_cyclopts() -> CycloptsSupport:
    """Return Cyclopts components with graceful fallbacks."""

    try:
        import cyclopts as _cyclopts
        from cyclopts import App, Parameter
    except ModuleNotFoundError as exc:  # pragma: no cover - lean test environments.
        def _raise_missing_cyclopts(*_args: object, **_kwargs: object) -> None:
            message = (
                "Cyclopts is required for CLI usage; install it to run the script"
            )
            raise RuntimeError(message) from exc

        _raise_missing_cyclopts.default = lambda func: func  # type: ignore[attr-defined]

        class _ParameterStub:
            def __init__(self, *_args: object, **_kwargs: object) -> None:
                """Accept arguments for ``typing.Annotated`` compatibility."""

        return CycloptsSupport(False, _raise_missing_cyclopts, _ParameterStub)

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
        class _LocalStub:
            def __getitem__(self, name: str) -> None:  # pragma: no cover - defensive.
                message = (
                    "plumbum is required to execute release uploads; "
                    "install it or run in dry-run mode"
                )
                raise ModuleNotFoundError(message) from exc

        return PlumbumSupport(
            local=_LocalStub(),
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
    coerce_bool: Callable[[object], bool],
    main: Callable[..., int],
    tokens: Sequence[str] | None = None,
) -> int:
    """Execute the uploader CLI using Cyclopts or ``argparse`` fallback."""

    arguments = list(tokens) if tokens is not None else None

    if support.available:
        return support.app(arguments)

    parser = ArgumentParser(description=main.__doc__ or "Upload release assets.")
    parser.add_argument("--release-tag")
    parser.add_argument("--bin-name")
    parser.add_argument("--dist-dir")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(arguments if arguments is not None else sys.argv[1:])

    release_tag = args.release_tag or os.environ.get("INPUT_RELEASE_TAG")
    bin_name = args.bin_name or os.environ.get("INPUT_BIN_NAME")
    dist_dir_value = args.dist_dir or os.environ.get("INPUT_DIST_DIR") or "dist"
    dry_run_flag = args.dry_run

    if not dry_run_flag and (env_flag := os.environ.get("INPUT_DRY_RUN")):
        dry_run_flag = coerce_bool(env_flag)

    missing = [
        label
        for label, present in (
            ("--release-tag", release_tag),
            ("--bin-name", bin_name),
        )
        if not present
    ]
    if missing:
        joined = ", ".join(missing)
        parser.exit(status=1, message=f"Missing required argument(s): {joined}\n")

    return main(
        release_tag=release_tag,
        bin_name=bin_name,
        dist_dir=Path(dist_dir_value),
        dry_run=dry_run_flag,
    )
