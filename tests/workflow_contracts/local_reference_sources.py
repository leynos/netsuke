"""Preserve execution context while collecting reference-source text.

Workflow run commands need their effective working directory for relative
script paths. Reached scripts need their language comments removed before
their contents can establish indirect references.
"""

import dataclasses
import posixpath
import re


@dataclasses.dataclass(frozen=True, slots=True)
class ReferenceSource:
    """Carry text with the context needed to interpret local references."""

    text: str
    working_directory: str | None = None
    script_suffix: str | None = None
    source_suffix: str | None = None


def workflow_reference_sources(document: object) -> tuple[ReferenceSource, ...]:
    """Collect every workflow string and keep run-command directories.

    Non-run strings remain available as ordinary reference sources. Each run
    command is collected separately with step, job, workflow, or runner-root
    working-directory precedence.

    Parameters
    ----------
    document : object
        A parsed workflow or composite-action manifest.

    Returns
    -------
    tuple[ReferenceSource, ...]
        Workflow strings with command execution context where applicable.
    """
    sources = [ReferenceSource("\n".join(_non_run_strings(document)))]
    document_mapping = _mapping(document)
    workflow_directory = _default_working_directory(document_mapping)
    jobs = _mapping(document_mapping.get("jobs"))
    for job in jobs.values():
        job_mapping = _mapping(job)
        job_directory = (
            _default_working_directory(job_mapping) or workflow_directory or "."
        )
        sources.extend(_step_sources(job_mapping.get("steps"), job_directory))

    action_runs = _mapping(document_mapping.get("runs"))
    sources.extend(_step_sources(action_runs.get("steps"), workflow_directory or "."))
    return tuple(sources)


def relative_script_is_referenced(
    source: ReferenceSource,
    script_path: str,
) -> bool:
    """Resolve any static relative script path against the run directory.

    Parameters
    ----------
    source : ReferenceSource
        A workflow run command with a known working-directory value.
    script_path : str
        Repository-relative path of the inventoried script.

    Returns
    -------
    bool
        Whether a relative command resolves exactly to the inventoried path.
    """
    working_directory = _normalise_working_directory(source.working_directory)
    if working_directory is None:
        return False

    relative_path = posixpath.relpath(script_path, working_directory)
    escaped_path = re.escape(relative_path).replace(r"/", r"[/\\]")
    pattern = rf"(?<![\w./\\-])(?:\.[/\\])?{escaped_path}(?![\w.\\-])"
    return re.search(pattern, source.text) is not None


def scripts_package_imports_module(text: str, module_name: str) -> bool:
    """Match an exact module name in a scripts package import."""
    package_import_pattern = re.compile(
        r"(?ms)^[ \t]*from[ \t]+scripts[ \t]+import[ \t]*"
        r"(?:\((?P<parenthesized>.*?)\)|(?P<inline>[^\n]*))"
    )
    for match in package_import_pattern.finditer(text):
        import_list = match.group("parenthesized") or match.group("inline") or ""
        without_comments = re.sub(r"(?m)#.*$", "", import_list)
        imported_names = (entry.split() for entry in without_comments.split(","))
        if any(words and words[0] == module_name for words in imported_names):
            return True
    return False


def _mapping(value: object) -> dict[str, object]:
    """Return a parsed YAML mapping or an empty mapping for other values."""
    if isinstance(value, dict):
        return {key: item for key, item in value.items() if isinstance(key, str)}
    return {}


def _default_working_directory(mapping: dict[str, object]) -> str | None:
    """Read the run working-directory default from a workflow or job."""
    defaults = _mapping(mapping.get("defaults"))
    run_defaults = _mapping(defaults.get("run"))
    directory = run_defaults.get("working-directory")
    return directory if isinstance(directory, str) else None


def _step_sources(steps: object, default_directory: str) -> list[ReferenceSource]:
    """Collect run strings from a list of workflow or action steps."""
    if not isinstance(steps, list):
        return []
    sources: list[ReferenceSource] = []
    for step in steps:
        step_mapping = _mapping(step)
        command = step_mapping.get("run")
        if not isinstance(command, str):
            continue
        directory = step_mapping.get("working-directory")
        working_directory = (
            directory if isinstance(directory, str) else default_directory
        )
        sources.append(ReferenceSource(command, working_directory=working_directory))
    return sources


def _non_run_strings(value: object) -> list[str]:
    """Collect parsed YAML strings except run bodies collected with context."""
    match value:
        case str() as text:
            return [text]
        case dict() as mapping:
            strings: list[str] = []
            for key, item in mapping.items():
                strings.extend(_non_run_strings(key))
                if key == "run" and isinstance(item, str):
                    continue
                strings.extend(_non_run_strings(item))
            return strings
        case list() as sequence:
            strings = []
            for item in sequence:
                strings.extend(_non_run_strings(item))
            return strings
        case _:
            return []


def _normalise_working_directory(value: str | None) -> str | None:
    """Return a repository-relative static directory, if one can be resolved."""
    if value is None:
        return None
    if "$" in value:
        return None
    directory = posixpath.normpath(value.replace("\\", "/"))
    if directory == ".." or directory.startswith("../"):
        return None
    if directory.startswith("/"):
        return None
    return directory
