from __future__ import annotations

from collections.abc import Callable, Iterable, Sequence
from types import ModuleType
from typing import Any, ParamSpec, TypeVar

__all__ = [
    "App",
    "Parameter",
    "config",
]

P = ParamSpec("P")
R = TypeVar("R")


class Parameter:
    """Describe a single command-line parameter accepted by a command."""

    def __init__(
        self,
        name: str | Sequence[str] | None = ...,
        *aliases: str,
        converter: Callable[[Any, Sequence[Any]], Any] | None = ...,
        *,
        alias: str | Sequence[str] | None = ...,
        default: Any = ...,
        help: str | None = ...,
        required: bool | None = ...,
        **kwargs: Any,
    ) -> None: ...


class _Env:
    """Map environment variables with a common prefix to CLI inputs."""

    def __init__(self, prefix: str = "", show: bool = True, *, command: bool = True) -> None: ...

    def __call__(
        self,
        apps: list[App],
        commands: tuple[str, ...],
        arguments: Any,
    ) -> None: ...


class App:
    """Application CLI builder."""

    def __init__(
        self,
        name: str | Sequence[str] | None = ...,
        *,
        help: str | None = ...,
        config: _Env | Iterable[_Env] | None = ...,
        version: str | Callable[..., str] | None = ...,
    ) -> None: ...

    def default(self, func: Callable[P, R]) -> Callable[P, R]:
        """Register a default command handler."""

    def __call__(
        self,
        tokens: Sequence[str] | None = ...,
        *,
        exit_on_error: bool = ...,
        help_on_error: bool | None = ...,
        verbose: bool = ...,
    ) -> R | None:
        """Execute the application with given arguments."""


class _ConfigModule(ModuleType):
    """Container for configuration helpers exported by cyclopts."""

    Env: type[_Env]


config: _ConfigModule
