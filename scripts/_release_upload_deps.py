"""Support helpers for optional release-upload dependencies."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Any, Protocol

__all__ = [
    "CycloptsSupport",
    "PlumbumSupport",
    "load_cyclopts",
    "load_plumbum",
]


class SupportsDefault(Protocol):
    """Protocols for callables that expose a ``default`` decorator."""

    default: Callable[[Callable[..., Any]], Callable[..., Any]]


@dataclass(frozen=True)
class CycloptsSupport:
    """Expose cyclopts types while tolerating missing dependencies."""

    available: bool
    app: SupportsDefault | Callable[..., Any]
    parameter: type[Any]


def load_cyclopts() -> CycloptsSupport:
    """Load cyclopts support objects.

    Returns
    -------
    CycloptsSupport
        Packed availability state alongside the ``cyclopts.App`` factory and
        ``cyclopts.Parameter`` type or their stub fallbacks.
    """

    try:
        from cyclopts import App, Parameter

        return CycloptsSupport(available=True, app=App, parameter=Parameter)
    except ModuleNotFoundError as exc:  # pragma: no cover - exercised in CI jobs.
        cause = exc

        def _raise_missing_cyclopts(*_args: object, **_kwargs: object) -> None:
            message = (
                "Cyclopts is required for CLI usage; install it to run the script"
            )
            raise RuntimeError(message) from cause

        _raise_missing_cyclopts.default = lambda func: func  # type: ignore[attr-defined]

        class _ParameterStub:
            """Accept arguments for ``typing.Annotated`` compatibility."""

            def __init__(
                self,
                *_names: str,
                converter: Callable[[Any, Sequence[Any]], Any] | None = None,
                **_kwargs: Any,
            ) -> None:
                return

        return CycloptsSupport(
            available=False,
            app=_raise_missing_cyclopts,
            parameter=_ParameterStub,
        )


@dataclass(frozen=True)
class PlumbumSupport:
    """Expose plumbum helpers while providing fallbacks for dry runs."""

    local: Any
    command_not_found: type[Exception]
    process_execution_error: type[Exception]
    bound_command: type[Any]


def load_plumbum() -> PlumbumSupport:
    """Load plumbum support helpers.

    Returns
    -------
    PlumbumSupport
        Bound helpers exposing ``plumbum.local`` along with the relevant command
        exception types or descriptive stubs when the dependency is absent.
    """

    try:
        from plumbum import local
        from plumbum.commands import CommandNotFound, ProcessExecutionError
        from plumbum.commands.base import BoundCommand

        return PlumbumSupport(
            local=local,
            command_not_found=CommandNotFound,
            process_execution_error=ProcessExecutionError,
            bound_command=BoundCommand,
        )
    except ModuleNotFoundError as exc:  # pragma: no cover - exercised in CI jobs.
        cause = exc

        class _LocalStub:
            """Raise a descriptive error when `plumbum` is not installed."""

            def __getitem__(self, name: str) -> None:  # pragma: no cover - defensive.
                message = (
                    "plumbum is required to execute release uploads; "
                    "install it or run in dry-run mode"
                )
                raise ModuleNotFoundError(message) from cause

        return PlumbumSupport(
            local=_LocalStub(),
            command_not_found=ModuleNotFoundError,
            process_execution_error=RuntimeError,
            bound_command=Any,
        )
