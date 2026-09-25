"""Parse the downstream canary runner's command line.

The ``downstream-canary`` composite action passes each list as a single input,
so the same one-line invocation works under Bash and PowerShell. This module
declares the three subcommands and splits those list inputs: targets on
whitespace, and selectors, extra environment, and forbidden patterns on lines,
since a forbidden pattern may itself contain spaces.
"""

import argparse
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def build_parser(
    handlers: cabc.Mapping[str, cabc.Callable[[argparse.Namespace], int]],
) -> argparse.ArgumentParser:
    """Build the command-line parser for the three canary steps.

    Parameters
    ----------
    handlers
        The ``generate``, ``run``, and ``report`` step functions, stored as
        each subcommand's ``handler`` default.

    Returns
    -------
    argparse.ArgumentParser
        The parser, with one subcommand per handler.
    """
    root = argparse.ArgumentParser(description="Run one downstream canary step.")
    commands = root.add_subparsers(dest="command", required=True)
    for name, handler in handlers.items():
        command = commands.add_parser(name)
        command.set_defaults(handler=handler)
        command.add_argument("--workdir", required=True, type=Path)
        command.add_argument("--state", required=True, type=Path)
        command.add_argument("--selector", action="append", default=[])
    for name in ("generate", "run"):
        commands.choices[name].add_argument(
            "--environment", action="append", default=[]
        )
    commands.choices["generate"].add_argument("--netsuke", required=True)
    commands.choices["run"].add_argument("--target", action="append", required=True)
    commands.choices["run"].add_argument("--forbid", action="append", default=[])
    add_report_arguments(commands.choices["report"])
    return root


def add_report_arguments(command: argparse.ArgumentParser) -> None:
    """Add the run-identity arguments that ``report`` records.

    Parameters
    ----------
    command
        The ``report`` subcommand parser.
    """
    command.add_argument("--target", action="append", required=True)
    command.add_argument("--canary", required=True)
    command.add_argument("--repository", required=True)
    command.add_argument("--downstream-revision", required=True)
    command.add_argument("--netsuke-revision", required=True)
    command.add_argument("--netsuke-version", required=True)
    command.add_argument("--platform", required=True)
    command.add_argument("--provenance", required=True, type=Path)
    command.add_argument("--summary", type=Path)


def split_values(values: cabc.Iterable[str], separator: str | None) -> list[str]:
    r"""Flatten list arguments that may each carry several separated values.

    A composite action passes each list as one input, so the same one-line
    invocation works under both Bash and PowerShell.

    Parameters
    ----------
    values
        The repeated argument's values.
    separator
        ``None`` to split on whitespace, or ``"\n"`` to split on lines.

    Returns
    -------
    list[str]
        The non-empty values, stripped, in order.

    Examples
    --------
    >>> split_values(["check-fmt lint", "test"], None)
    ['check-fmt', 'lint', 'test']
    >>> split_values(["MXD_BACKEND=sqlite\n\n"], "\n")
    ['MXD_BACKEND=sqlite']
    """
    return [
        part.strip()
        for value in values
        for part in value.split(separator)
        if part.strip()
    ]


def normalize_lists(arguments: argparse.Namespace) -> None:
    """Split every list argument in place, and require at least one target.

    Targets are whitespace-separated; selectors, extra environment, and
    forbidden patterns are one per line, since a pattern may contain spaces.

    Parameters
    ----------
    arguments
        The parsed arguments, updated in place.

    Raises
    ------
    ValueError
        When a ``run`` or ``report`` step names no target.
    """
    for name in ("selector", "environment", "forbid"):
        if hasattr(arguments, name):
            setattr(arguments, name, split_values(getattr(arguments, name), "\n"))
    if hasattr(arguments, "target"):
        arguments.target = split_values(arguments.target, None)
        if not arguments.target:
            msg = "no target was requested"
            raise ValueError(msg)
