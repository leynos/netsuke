"""Optional dependency loaders for the release uploader CLI."""

from __future__ import annotations

import dataclasses as dc
import typing as typ
from typing import ParamSpec, TypeVar

P = ParamSpec("P")
T = TypeVar("T")

__all__ = [
    "CycloptsSupport",
    "PlumbumSupport",
    "load_cyclopts",
    "load_plumbum",
]


@dc.dataclass(frozen=True)
class CycloptsSupport:
    """Expose Cyclopts components and availability information."""

    available: bool
    app: typ.Any
    parameter: type


@dc.dataclass(frozen=True)
class PlumbumSupport:
    """Expose Plumbum components required by the uploader."""

    local: typ.Any
    command_not_found: type[Exception]
    process_execution_error: type[Exception]
    bound_command: typ.Any


def load_cyclopts() -> CycloptsSupport:
    """Return Cyclopts components with graceful fallbacks."""

    try:
        import cyclopts as _cyclopts
        from cyclopts import App, Parameter
    except ModuleNotFoundError:  # pragma: no cover - lean test environments.
        class Parameter:  # type: ignore[empty-body]
            """Fallback placeholder that preserves ``typing.Annotated`` usage."""

            def __init__(self, *_args: object, **_kwargs: object) -> None:
                """Accept arguments for compatibility; behaviour is irrelevant."""

        class _FallbackApp:
            """Stub that surfaces a descriptive error when invoked."""

            def default(self, func: typ.Callable[P, T]) -> typ.Callable[P, T]:
                return func

            def __call__(self, *_args: P.args, **_kwargs: P.kwargs) -> typ.NoReturn:
                message = (
                    "Cyclopts is required for CLI usage; install it to run the script"
                )
                raise RuntimeError(message)

        return CycloptsSupport(False, _FallbackApp(), Parameter)

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
    except ModuleNotFoundError:  # pragma: no cover - lean type-check envs.
        class _MissingLocal:
            """Placeholder mirroring the ``plumbum.local`` interface."""

            def __getitem__(self, name: str) -> typ.NoReturn:
                message = (
                    "plumbum is required to execute release uploads; "
                    "install it or run in dry-run mode"
                )
                raise ModuleNotFoundError(message)

        return PlumbumSupport(
            local=_MissingLocal(),
            command_not_found=ModuleNotFoundError,
            process_execution_error=RuntimeError,
            bound_command=typ.Any,
        )

    return PlumbumSupport(
        local=local,
        command_not_found=CommandNotFound,
        process_execution_error=ProcessExecutionError,
        bound_command=BoundCommand,
    )
