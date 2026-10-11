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
import typing as typ
from pathlib import Path

from . import commands, policy, telemetry

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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
    metrics_file: OutputPath
    trace_file: OutputPath
    programs: commands.Programs
    sinks: telemetry.Sinks


class ConfigurationError(Exception):
    """Report a required variable that is unset or empty, or a refused path.

    The message is the diagnostic itself, so the caller prints exactly the
    sentence the shell's ``:?`` form produced and nothing derived from the
    environment. The shell printed no sentence of its own for a filesystem
    refusal -- it let ``mkdir`` or the redirection report one -- so this class
    is also what carries the one bounded diagnostic the port prints in that
    state instead of escaping as an uncaught traceback.
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
            metrics_file=outputs.metrics_file.configured,
            trace_file=outputs.trace_file.configured,
            metrics_sink=_optional(commands.METRICS_SINK_VARIABLE, ""),
            output_sink=_optional(commands.OUTPUT_SINK_VARIABLE, ""),
            trace_sink=_optional(commands.TRACE_SINK_VARIABLE, ""),
        ),
    )


@dataclasses.dataclass(frozen=True, slots=True)
class OutputPath:
    """Carry one artefact path in both spellings its two consumers need.

    ``configured`` is the string the operator supplied, verbatim, and
    ``resolved`` is the same path as a filesystem object. They are not
    interchangeable. The shell passed the configured string to a sink adapter
    and published it in a workflow output, so ``./observations//metrics.jsonl``
    stayed spelled that way on both boundaries; only ``mkdir`` and the
    redirection saw a normalized form. Rendering ``Path`` back to text would
    rewrite the operator's spelling at two observable boundaries and can even
    change whether a target is treated as a file or refused as directory-shaped,
    because a trailing slash is a filesystem distinction rather than a
    cosmetic one.
    """

    configured: str
    resolved: Path

    @classmethod
    def of(cls, variable: str, default: str) -> OutputPath:
        """Resolve one artefact path from its variable and default."""
        configured = _optional(variable, default)
        return cls(configured=configured, resolved=Path(configured))


@dataclasses.dataclass(frozen=True, slots=True)
class _OutputPaths:
    """Hold the two artefact files the gate writes and names in its outputs.

    The paths are derived together and consumed together -- by the sinks, by
    the artefact creation below, and by the configuration -- so they travel as
    one value rather than as two arguments threaded through each caller.
    """

    metrics_file: OutputPath
    trace_file: OutputPath

    @classmethod
    def from_environment(cls) -> _OutputPaths:
        """Resolve both artefact paths and create both files empty.

        Returns
        -------
        _OutputPaths
            The metric and trace paths, with both files created.
        """
        temporary = _optional("RUNNER_TEMP", DEFAULT_TEMPORARY_DIRECTORY)
        paths = cls(
            metrics_file=OutputPath.of(
                METRICS_FILE_VARIABLE, f"{temporary}/{METRICS_FILE_NAME}"
            ),
            trace_file=OutputPath.of(
                TRACE_FILE_VARIABLE, f"{temporary}/{TRACE_FILE_NAME}"
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
    ``os.environ.get(variable, default)`` happens to agree here, because it
    substitutes only for an *unset* variable too; the membership test is what
    states that, so a reader cannot mistake the helper for the colon form the
    two above it implement.

    Returns
    -------
    str
        The configured value, empty when it was exported empty, or the default
        when the variable is unset.
    """
    if variable not in os.environ:
        return default
    return os.environ[variable]


def _create_artefacts(*paths: OutputPath) -> None:
    """Create the artefact files empty, refusing a path the system rejects.

    The shell made *both* parent directories before truncating *either* file:
    ``mkdir -p "$(dirname "$metrics_file")" "$(dirname "$trace_file")"`` then
    ``: >"$metrics_file"; : >"$trace_file"``. The order is observable. A refused
    trace parent left the metrics file untouched, because the whole ``mkdir``
    ran first; interleaving the two would truncate metrics before discovering
    that trace could not be created, destroying a previous run's records on a
    configuration the gate then refuses.

    A refusal by the operating system reaches the caller as a
    :class:`ConfigurationError` from :func:`_refusing`, so the gate reports one
    bounded diagnostic rather than an uncaught traceback. ``mkdir`` and the
    truncation each reported their own sentence in the shell, so the diagnostic
    is a port-specific wording; the position, the reason, and the exit status
    are the shell's.
    """
    for path in paths:
        _make_parent(path)
    for path in paths:
        _truncate(path)


def _make_parent(path: OutputPath) -> None:
    """Create one artefact's parent directory, refusing a rejection.

    ``Path`` normalizes a trailing slash away, but the *directory* is the same
    either way -- ``mkdir -p "dir/"`` and ``mkdir -p "dir"`` both succeed in the
    shell -- so the resolved parent is the correct operand here. The file's own
    operand is not, which is why :func:`_truncate` opens the configured
    spelling instead, and why the refusal travels through :func:`_refusing` as
    a :class:`ConfigurationError`.
    """

    def create() -> None:
        """Make the parent directory, tolerating one that already exists."""
        path.resolved.parent.mkdir(parents=True, exist_ok=True)

    _refusing("release-admission could not create the directory for", path, create)


def _truncate(path: OutputPath) -> None:
    r"""Create one artefact file empty, refusing a rejection.

    The operand is the operator's *configured* string rather than the resolved
    :class:`~pathlib.Path`, because a trailing slash is a filesystem
    distinction rather than a cosmetic one. ``Path("metrics.jsonl/")`` drops
    the slash, so ``write_bytes`` on it silently creates the regular file the
    shell's ``: >"metrics.jsonl/"`` refused with ``Is a directory``; opening
    the configured spelling refuses it as well, and with the system's own
    sentence. This mirrors ``delivery._append_in_process``, which opens the
    configured spelling for the same reason, and the refusal travels through
    :func:`_refusing` as a :class:`ConfigurationError`.
    """

    def open_empty() -> None:
        """Open the configured path for writing, truncating it to nothing."""
        # ruff: ignore[builtin-open] - the configured string is the operand.
        with open(path.configured, "wb"):
            pass

    _refusing("release-admission could not create", path, open_empty)


def _refusing(lead: str, path: OutputPath, operation: cabc.Callable[[], None]) -> None:
    """Perform one filesystem operation, refusing a rejection.

    ``mkdir`` and the redirection each reported their own sentence in the
    shell, and both reach the caller as the same bounded diagnostic: the
    message keeps the system's own text through :func:`_reason`, so the
    operator still reads *why* the path was refused. One helper serves both
    operations because the failure handling is identical and duplicating the
    seek-and-name logic twice would only invite the two to drift apart.

    Raises
    ------
    ConfigurationError
        When the operating system refuses the operation.
    """
    try:
        operation()
    except OSError as error:
        msg = f"{lead} {path.configured}: {_reason(error)}"
        raise ConfigurationError(msg) from error


def _reason(error: OSError) -> str:
    """Render one operating-system refusal as a short reason.

    The shell printed the operating system's own sentence, so this carries its
    text rather than a class name whenever the system supplied one. The
    fallback is for the rare :class:`OSError` raised without a message, where
    the class is the only thing left to name.

    Returns
    -------
    str
        The system's own description, or the error's class name without one.
    """
    return error.strerror or error.__class__.__name__
