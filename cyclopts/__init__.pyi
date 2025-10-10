from __future__ import annotations

import typing as typ

P = typ.ParamSpec("P")
T = typ.TypeVar("T")

class Parameter:
    def __init__(self, *names: object, **kwargs: object) -> None: ...

class App:
    def __init__(self, *args: object, **kwargs: object) -> None: ...
    def default(self, func: typ.Callable[P, T]) -> typ.Callable[P, T]: ...
    def __call__(self, *args: object, **kwargs: object) -> object: ...

class _Env:
    def __init__(self, prefix: str, command: bool = ...) -> None: ...
    def __call__(self, *args: object, **kwargs: object) -> object: ...

class _ConfigModule:
    Env: type[_Env]

config: _ConfigModule
