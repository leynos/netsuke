"""Load and execute examples from user-facing Netsuke documentation."""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from cuprum import (
    CommandResult,
    ExecutionContext,
    Program,
    ProgramCatalogue,
    ProjectSettings,
    ScopeConfig,
    scoped,
    sh,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

DOCUMENT_PATHS = ("README.md", "docs/users-guide.md")
MARKER_PREFIX = "<!-- tested-example: "
MARKER_SUFFIX = " -->"


@dc.dataclass(frozen=True)
class DocumentedExample:
    """A marked fenced example loaded from a user-facing document."""

    identifier: str
    language: str
    body: str


@dc.dataclass(frozen=True)
class NetsukeRunner:
    """Run the repository's Netsuke binary through Cuprum's allowlist."""

    binary: Path

    def run(self, workspace: Path, *arguments: str) -> CommandResult:
        """Run Netsuke in an isolated workspace and capture its output."""
        program = Program(str(self.binary.resolve()))
        project = ProjectSettings(
            name="netsuke-documentation-tests",
            programs=(program,),
            documentation_locations=("README.md", "docs/users-guide.md"),
            noise_rules=(),
        )
        catalogue = ProgramCatalogue(projects=(project,))
        command = sh.make(program, catalogue=catalogue)(*arguments)
        context = ExecutionContext(
            cwd=workspace,
            env={
                "HOME": str(workspace),
                "XDG_CONFIG_HOME": str(workspace / ".config"),
                "NETSUKE_NINJA": "ninja",
            },
        )
        with scoped(ScopeConfig(allowlist=catalogue.allowlist)):
            return command.run_sync(context=context)


def run_program(workspace: Path, name: str, *arguments: str) -> CommandResult:
    """Run one explicitly allowlisted program through Cuprum."""
    program = Program(name)
    project = ProjectSettings(
        name=f"documentation-{name}",
        programs=(program,),
        documentation_locations=(),
        noise_rules=(),
    )
    catalogue = ProgramCatalogue(projects=(project,))
    command = sh.make(program, catalogue=catalogue)(*arguments)
    with scoped(ScopeConfig(allowlist=catalogue.allowlist)):
        return command.run_sync(context=ExecutionContext(cwd=workspace))


def load_documented_examples(repository: Path) -> tuple[DocumentedExample, ...]:
    """Load marked examples and reject unmarked or duplicate fences."""
    examples = tuple(
        example
        for relative_path in DOCUMENT_PATHS
        for example in _load_document(repository / relative_path)
    )
    identifiers = [example.identifier for example in examples]
    if len(identifiers) != len(set(identifiers)):
        msg = "tested-example identifiers must be unique"
        raise ValueError(msg)
    return examples


def documented_example(repository: Path, identifier: str) -> DocumentedExample:
    """Return one documented example by its stable identifier."""
    examples = load_documented_examples(repository)
    try:
        return next(example for example in examples if example.identifier == identifier)
    except StopIteration as error:
        msg = f"documented example {identifier!r} should exist"
        raise ValueError(msg) from error


def manifest_workspace(repository: Path, root: Path, identifier: str) -> Path:
    """Create an isolated workspace from one documented YAML manifest."""
    example = documented_example(repository, identifier)
    if example.language != "yaml":
        msg = (
            f"documented example {identifier!r} should be YAML, "
            f"got {example.language!r}"
        )
        raise ValueError(msg)
    workspace = root / identifier
    workspace.mkdir()
    (workspace / "Netsukefile").write_text(example.body, encoding="utf-8")
    return workspace


def _load_document(path: Path) -> tuple[DocumentedExample, ...]:
    lines = path.read_text(encoding="utf-8").splitlines()
    examples: list[DocumentedExample] = []
    line_index = 0
    while line_index < len(lines):
        line = lines[line_index]
        identifier = _parse_marker(line)
        if identifier is None:
            _reject_unmarked_fence(path, line_index, line)
            line_index += 1
            continue
        example, line_index = _read_marked_example(
            path,
            lines,
            line_index,
            identifier,
        )
        examples.append(example)
    return tuple(examples)


def _parse_marker(line: str) -> str | None:
    if not line.startswith(MARKER_PREFIX) or not line.endswith(MARKER_SUFFIX):
        return None
    return line.removeprefix(MARKER_PREFIX).removesuffix(MARKER_SUFFIX)


def _reject_unmarked_fence(path: Path, line_index: int, line: str) -> None:
    if line.startswith("```"):
        msg = f"{path}:{line_index + 1} fence lacks a tested-example marker"
        raise ValueError(msg)


def _read_marked_example(
    path: Path,
    lines: list[str],
    marker_index: int,
    identifier: str,
) -> tuple[DocumentedExample, int]:
    fence_index = marker_index + 1
    while fence_index < len(lines) and not lines[fence_index]:
        fence_index += 1
    if fence_index >= len(lines) or not lines[fence_index].startswith("```"):
        msg = f"{path}:{marker_index + 1} marker has no opening fence"
        raise ValueError(msg)
    language = lines[fence_index].removeprefix("```")
    if not language:
        msg = f"{path}:{fence_index + 1} fence should declare a language"
        raise ValueError(msg)

    closing_index = fence_index + 1
    body_lines: list[str] = []
    while closing_index < len(lines) and lines[closing_index] != "```":
        body_lines.append(lines[closing_index])
        closing_index += 1
    if closing_index >= len(lines):
        msg = f"{path}:{fence_index + 1} fence is not terminated"
        raise ValueError(msg)

    body = "\n".join(body_lines) + "\n"
    return DocumentedExample(identifier, language, body), closing_index + 1
