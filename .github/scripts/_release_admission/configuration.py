"""Resolve the release-admission gate's configuration from the environment.

This module owns everything the gate reads *before* it runs an operation: the
three required variables and their diagnostics, the optional values with their
documented defaults, the two artefact paths, and the adapter programs and sinks
the rest of the run is wired from.

It is separate from :mod:`gate` because the two halves answer different
questions. This one asks what the gate was *told*; ``gate`` asks what it then
*does*. Keeping them apart also keeps each reviewable on its own: the default
forms below are Bash's, and getting one of them wrong changes admission
behaviour without changing a single operation.

Two of Bash's default forms appear here, and neither is interchangeable with
the other. ``${VAR:-default}``, the colon form, substitutes when the variable
is unset *or empty*; ``${VAR-default}``, the no-colon form, substitutes only
when it is unset. The shell used the first for every adapter, path, and mode
value and the second for ``NETSUKE_RELEASE_ADMISSION_ENFORCE`` alone, so the
enforcement mode keeps an exported empty value and refuses it in the gate's own
mode check rather than quietly becoming observation mode.
"""

import dataclasses
import os
from pathlib import Path

from . import commands, policy, telemetry

#: Environment variables the gate requires, and the diagnostic each produces.
REPOSITORY_VARIABLE = "GITHUB_REPOSITORY"
REPOSITORY_DIAGNOSTIC = "GITHUB_REPOSITORY must identify the release repository"
REVISION_VARIABLE = "GITHUB_SHA"
REVISION_DIAGNOSTIC = "GITHUB_SHA must identify the release candidate revision"
OUTPUT_VARIABLE = "GITHUB_OUTPUT"
OUTPUT_DIAGNOSTIC = "GITHUB_OUTPUT must identify the workflow output file"

#: Environment variables the gate reads with a documented default.
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
EVIDENCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"

#: The default artefact paths, relative to ``RUNNER_TEMP`` or the fallback.
METRICS_FILE_NAME = "netsuke-release-admission-metrics.jsonl"
TRACE_FILE_NAME = "netsuke-release-admission-traces.jsonl"
# The shell used ``${RUNNER_TEMP:-/tmp}``, so the fallback directory is part of
# the gate's contract rather than a temporary file this module invents.
DEFAULT_TEMPORARY_DIRECTORY = "/tmp"  # ruff: ignore[hardcoded-temp-file] - the documented fallback, never a temporary file.


@dataclasses.dataclass(frozen=True, slots=True)
class Configuration:
    """Hold the gate's resolved environment, with the shell's own defaults."""

    repository: str
    revision: str
    output: str
    evidence_state: str
    timeout_seconds: str
    enforcement: str
    metrics_file: Path
    trace_file: Path
    programs: commands.Programs
    sinks: telemetry.Sinks


class ConfigurationError(Exception):
    """Report a required variable that is unset or empty.

    The message is the diagnostic itself, so the caller prints exactly the
    sentence the shell's ``:?`` form produced and nothing derived from the
    environment.
    """


def load_configuration() -> Configuration:
    """Read the gate's configuration, creating the two artefact files.

    Returns
    -------
    Configuration
        The resolved environment.

    Notes
    -----
    A required variable that is unset or empty is refused by
    :func:`_required`, whose :class:`ConfigurationError` carries the diagnostic
    itself so the caller prints exactly the sentence the shell's ``:?`` form
    produced. The two default forms are Bash's, and each has its own helper:
    :func:`_optional` is the colon form and :func:`_defaulted_only_when_unset`
    the no-colon one.
    """
    # Required variables are resolved first, and the artefacts are created
    # after the first two, because that is the order the shell refused them in
    # and the contract tests pin each refusal's position.
    repository = _required(REPOSITORY_VARIABLE, REPOSITORY_DIAGNOSTIC)
    revision = _required(REVISION_VARIABLE, REVISION_DIAGNOSTIC)
    outputs = _OutputPaths.from_environment()
    return Configuration(
        repository=repository,
        revision=revision,
        output=_required(OUTPUT_VARIABLE, OUTPUT_DIAGNOSTIC),
        evidence_state=_optional(EVIDENCE_VARIABLE, policy.DEFAULT_EVIDENCE_STATE),
        timeout_seconds=_optional(
            TIMEOUT_VARIABLE, str(policy.DEFAULT_OPERATION_TIMEOUT_SECONDS)
        ),
        enforcement=_defaulted_only_when_unset(
            ENFORCE_VARIABLE, policy.ADMISSION_OBSERVATION_MODE
        ),
        metrics_file=outputs.metrics_file,
        trace_file=outputs.trace_file,
        programs=commands.Programs.from_environment(),
        sinks=telemetry.Sinks(
            metrics_file=outputs.metrics_file,
            trace_file=outputs.trace_file,
            metrics_sink=_optional(commands.METRICS_SINK_VARIABLE, ""),
            output_sink=_optional(commands.OUTPUT_SINK_VARIABLE, ""),
            trace_sink=_optional(commands.TRACE_SINK_VARIABLE, ""),
        ),
    )


@dataclasses.dataclass(frozen=True, slots=True)
class _OutputPaths:
    """Hold the two artefact files the gate writes and names in its outputs.

    The paths are derived together and consumed together -- by the sinks, by
    the artefact creation below, and by the configuration -- so they travel as
    one value rather than as two arguments threaded through each caller.
    """

    metrics_file: Path
    trace_file: Path

    @classmethod
    def from_environment(cls) -> _OutputPaths:
        """Resolve both artefact paths, creating each file empty.

        Returns
        -------
        _OutputPaths
            The metric and trace paths, with both files created.
        """
        temporary = Path(_optional("RUNNER_TEMP", DEFAULT_TEMPORARY_DIRECTORY))
        paths = cls(
            metrics_file=Path(
                _optional(METRICS_FILE_VARIABLE, str(temporary / METRICS_FILE_NAME))
            ),
            trace_file=Path(
                _optional(TRACE_FILE_VARIABLE, str(temporary / TRACE_FILE_NAME))
            ),
        )
        _create_artefacts(paths.metrics_file, paths.trace_file)
        return paths


def _required(variable: str, diagnostic: str) -> str:
    """Return a required variable, refusing an unset or empty value."""
    value = os.environ.get(variable)
    if not value:
        raise ConfigurationError(diagnostic)
    return value


def _optional(variable: str, default: str) -> str:
    """Return a variable's value, or *default* when it is unset or empty.

    This is Bash's ``${VAR:-default}``, colon included: the shell used that
    form for every adapter, path, and mode value this gate reads, so an
    exported empty value substituted the default exactly as an unset one did.

    Returns
    -------
    str
        The configured value, or the default when it is unset or empty.
    """
    return os.environ.get(variable) or default


def _defaulted_only_when_unset(variable: str, default: str) -> str:
    """Return a variable's value, or *default* only when it is unset.

    This is Bash's ``${VAR-default}``, with no colon, which the shell wrote for
    ``NETSUKE_RELEASE_ADMISSION_ENFORCE`` alone. An exported *empty* value was
    therefore kept and refused by the gate's own mode check rather than
    replaced here, and that refusal is load-bearing: substituting the default
    would let a refused configuration run as observation mode.
    ``os.environ.get(variable, default)`` implements the colon form and would
    do exactly that, which is why this helper asks membership instead.

    Returns
    -------
    str
        The configured value, empty when it was exported empty, or the default
        when the variable is unset.
    """
    if variable not in os.environ:
        return default
    return os.environ[variable]


def _create_artefacts(*paths: Path) -> None:
    """Create the artefact files empty, as the shell truncated them."""
    for path in paths:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"")
