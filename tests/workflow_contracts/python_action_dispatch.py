"""Parse trusted Python workflow dispatches without matching incidental text."""

import ast
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def assert_python_action_dispatch(
    step: cabc.Mapping[str, object], module_path: str, command: str
) -> None:
    """Assert a step assigns the exact argv before running its Python module.

    Parameters
    ----------
    step
        Parsed GitHub Actions step containing a Python ``run`` program.
    module_path
        Checked-in trusted Python module passed as ``sys.argv[0]``.
    command
        Fixed command passed as ``sys.argv[1]``.
    """
    assert step.get("shell") == "python", "trusted scripts must use Python"
    script = str(step["run"])
    arguments = _run_path_arguments(ast.parse(script))
    assert arguments[0] == module_path, "wrong trusted action module"
    assert arguments[1] == command, f"wrong trusted action command {command!r}"


def _run_path_arguments(program: ast.Module) -> list[str]:
    """Return argv from the assignment immediately preceding ``runpy.run_path``."""
    for index, statement in enumerate(program.body):
        if _is_run_path(statement):
            assert index > 0, "runpy.run_path must have a preceding sys.argv assignment"
            return _assigned_argv(program.body[index - 1])
    raise DispatchError.missing_runner()


def _is_run_path(statement: ast.stmt) -> bool:
    """Return whether one statement invokes ``runpy.run_path``."""
    match statement:
        case ast.Expr(
            value=ast.Call(
                func=ast.Attribute(value=ast.Name(id="runpy"), attr="run_path")
            )
        ):
            return True
        case _:
            return False


def _assigned_argv(statement: ast.stmt) -> list[str]:
    """Return a literal sys.argv assignment or raise an exact contract error."""
    match statement:
        case ast.Assign(
            targets=[ast.Attribute(value=ast.Name(id="sys"), attr="argv")],
            value=ast.List(elts=elements),
        ):
            return [_string_literal(element) for element in elements]
        case _:
            raise DispatchError.missing_assignment()


def _string_literal(expression: ast.expr) -> str:
    """Return one literal argv element or reject dynamic workflow dispatch."""
    if not isinstance(expression, ast.Constant) or not isinstance(
        expression.value, str
    ):
        raise DispatchError.dynamic_argument()
    return expression.value


class DispatchError(AssertionError):
    """Describe a malformed trusted Python workflow-dispatch program."""

    @classmethod
    def missing_runner(cls) -> typ.Self:
        """Build the missing checked-in module execution failure."""
        return cls("trusted script must run the checked-in module")

    @classmethod
    def missing_assignment(cls) -> typ.Self:
        """Build the missing immediate argv-assignment failure."""
        return cls("runpy.run_path must immediately follow sys.argv")

    @classmethod
    def dynamic_argument(cls) -> typ.Self:
        """Build the dynamic argv-element failure."""
        return cls("trusted sys.argv must contain only literal strings")
