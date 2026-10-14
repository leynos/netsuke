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

Start-up is four steps rather than one, and the entry point calls them in the
order the shell refused in. :func:`resolve_destination` and
:func:`resolve_configuration` are pure -- they read the mapping they are handed
and write nothing -- while :func:`prepare_destination` is the only step that
touches a filesystem, through the :class:`~.filesystem.FileSystem` it is handed
rather than one it looks up. :func:`require_workflow_output` performs the one
required-variable refusal that belongs *after* the artefacts exist.

The order is observable, and alerting keys off which refusal a rerun produced.
The shell resolved ``GITHUB_REPOSITORY`` and ``GITHUB_SHA`` with ``:?``, then
ran one ``mkdir -p`` naming both parents, then truncated both files with
``: >``, and only then expanded ``${GITHUB_OUTPUT:?}``.

Both of Bash's default forms appear in the shell this gate replaces, and
:mod:`environment` owns them: the colon form for every path, sink, adapter, and
mode value, and the no-colon form for ``NETSUKE_RELEASE_ADMISSION_ENFORCE``
alone, so an exported empty mode is refused by the gate's own check.
"""

import dataclasses
import functools
import typing as typ
from pathlib import Path

from . import commands, environment, filesystem, policy, telemetry

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
TEMPORARY_DIRECTORY_VARIABLE = "RUNNER_TEMP"

#: The default artefact paths, relative to ``RUNNER_TEMP`` or the fallback.
METRICS_FILE_NAME = "netsuke-release-admission-metrics.jsonl"
TRACE_FILE_NAME = "netsuke-release-admission-traces.jsonl"
# The shell used ``${RUNNER_TEMP:-/tmp}``, so the fallback directory is part of
# the gate's contract rather than a temporary file this module invents.
DEFAULT_TEMPORARY_DIRECTORY = "/tmp"  # ruff: ignore[hardcoded-temp-file] - the documented fallback, never a temporary file.


@dataclasses.dataclass(frozen=True, slots=True)
class Configuration:
    """Hold the gate's resolved settings, with the shell's own defaults.

    ``output`` is the one field :func:`resolve_configuration` leaves empty.
    """

    repository: str
    revision: str
    output: str
    evidence_state: str
    timeout_seconds: str
    enforcement: str
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

    Three steps raise it: :func:`resolve_configuration` and
    :func:`require_workflow_output` through :func:`_required`, and
    :func:`prepare_destination` through :func:`_refusing`.
    """


def resolve_destination(variables: cabc.Mapping[str, str]) -> Destination:
    """Resolve both artefact paths, creating nothing.

    The mapping is a parameter rather than an ambient lookup, so a test can
    observe what resolution reads without a child process.

    Returns
    -------
    Destination
        Both configured spellings and the parent each one implies.
    """
    temporary = environment.optional(
        variables, TEMPORARY_DIRECTORY_VARIABLE, DEFAULT_TEMPORARY_DIRECTORY
    )
    return Destination(
        metrics_file=OutputPath.of(
            variables, METRICS_FILE_VARIABLE, f"{temporary}/{METRICS_FILE_NAME}"
        ),
        trace_file=OutputPath.of(
            variables, TRACE_FILE_VARIABLE, f"{temporary}/{TRACE_FILE_NAME}"
        ),
    )


def resolve_configuration(
    variables: cabc.Mapping[str, str], destination: Destination
) -> Configuration:
    """Resolve the gate's settings, performing no filesystem access.

    An unset or empty ``GITHUB_REPOSITORY`` or ``GITHUB_SHA`` is refused here,
    with the shell's own diagnostic, through :func:`_required`.

    Returns
    -------
    Configuration
        The resolved variables, with both artefact spellings inside
        :attr:`Configuration.sinks`. :attr:`Configuration.output` is empty:
        see :func:`require_workflow_output` for why it arrives later.

    Notes
    -----
    The artefact spellings reach the sinks here and are never read from the
    environment again, so the value a sink is handed is the value the workflow
    output publishes. *destination* is passed in rather than resolved here:
    resolving it twice would give two steps their own answer to one question.
    """
    # Required variables are resolved first, and the artefacts are created
    # after the first two, because that is the order the shell refused them in
    # and the contract tests pin each refusal's position.
    repository = _required(variables, REPOSITORY_VARIABLE, REPOSITORY_DIAGNOSTIC)
    revision = _required(variables, REVISION_VARIABLE, REVISION_DIAGNOSTIC)
    return Configuration(
        repository=repository,
        revision=revision,
        output="",
        evidence_state=environment.optional(
            variables, EVIDENCE_VARIABLE, policy.EvidenceState.MISSING
        ),
        timeout_seconds=environment.optional(
            variables, TIMEOUT_VARIABLE, str(policy.DEFAULT_OPERATION_TIMEOUT_SECONDS)
        ),
        enforcement=environment.defaulted_only_when_unset(
            variables, ENFORCE_VARIABLE, policy.ADMISSION_OBSERVATION_MODE
        ),
        programs=commands.Programs.from_environment(variables),
        sinks=telemetry.Sinks(
            metrics_file=destination.metrics_file.configured,
            trace_file=destination.trace_file.configured,
            metrics_sink=environment.optional(
                variables, commands.METRICS_SINK_VARIABLE, ""
            ),
            output_sink=environment.optional(
                variables, commands.OUTPUT_SINK_VARIABLE, ""
            ),
            trace_sink=environment.optional(
                variables, commands.TRACE_SINK_VARIABLE, ""
            ),
        ),
    )


def prepare_destination(
    sinks: telemetry.Sinks, file_system: filesystem.FileSystem
) -> None:
    """Create both artefact parents, then truncate both files.

    The bindings are read from *sinks* because that is where the configured
    spellings already live: resolution puts them there and reads them from the
    environment only once, so initialization and the workflow outputs cannot
    disagree about which file a run wrote.

    *file_system* is required rather than defaulted: an initializer that looked
    one up could only be shown to work by a real refusal, and the choice of
    filesystem would sit inside the effect rather than in the caller that owns
    the outcome.

    Either effect the operating system refuses reaches the caller as a
    :class:`ConfigurationError` from :func:`_refusing`, whose message names the
    configured spelling and keeps the system's own reason, because the shell
    printed that sentence itself.

    Notes
    -----
    The two passes are the shell's, and the order is observable: ``mkdir -p``
    named *both* parents before either file was truncated, so a refused trace
    parent left a previous run's metrics file intact.
    """
    for target in sinks.bindings():
        _refusing(
            "release-admission could not create the directory for",
            target,
            functools.partial(file_system.make_parent, Path(_directory_name(target))),
        )
    for target in sinks.bindings():
        _refusing(
            "release-admission could not create",
            target,
            functools.partial(file_system.truncate, target),
        )


def require_workflow_output(variables: cabc.Mapping[str, str]) -> str:
    """Return ``GITHUB_OUTPUT``, refusing an unset or empty value.

    An unset or empty variable is refused with the diagnostic itself, so the
    caller prints the shell's own sentence and nothing derived from the
    environment.

    Returns
    -------
    str
        The configured workflow-output file.

    Notes
    -----
    This is the last refusal the shell made, *after* both artefact files
    existed: a run that dies here already has both files present and empty.
    """
    return _required(variables, OUTPUT_VARIABLE, OUTPUT_DIAGNOSTIC)


@dataclasses.dataclass(frozen=True, slots=True)
class Destination:
    """Hold the two artefact files the gate writes and names in its outputs.

    The paths are derived together and consumed together -- by the sinks, the
    artefact creation, and the workflow outputs -- so they travel as one value.
    """

    metrics_file: OutputPath
    trace_file: OutputPath


@dataclasses.dataclass(frozen=True, slots=True)
class OutputPath:
    """Carry one artefact path in both forms its two consumers need.

    ``configured`` is the string the operator supplied, verbatim, and
    ``directory`` is the parent the shell's ``mkdir -p "$(dirname "$file")"``
    named for it. They are not interchangeable, and the split is not "one
    spelling per boundary": the configured string reaches *every* observable
    boundary -- the argument a sink adapter is named with, the ``metrics-file``
    and ``trace-file`` workflow outputs, and the open that truncates the file --
    so ``./observations//metrics.jsonl`` stayed spelled that way everywhere the
    shell spelled it that way. ``directory`` is the ``mkdir`` operand alone,
    and :func:`_directory_name` is where its difference from the configured
    path's :attr:`~pathlib.Path.parent` is set out.

    Rendering ``Path`` back to text at any of the configured boundaries would
    rewrite the operator's spelling, and at the truncation open it would change
    which file is written: ``Path("metrics.jsonl/")`` drops the slash, so the
    resolved form creates the regular file where the shell's redirection
    refused one. A trailing slash is a filesystem distinction rather than a
    cosmetic one, which is why :class:`~.filesystem.FileSystem` truncates by
    the configured spelling and makes the parent by this operand.
    """

    configured: str
    directory: Path

    @classmethod
    def of(
        cls, variables: cabc.Mapping[str, str], variable: str, default: str
    ) -> OutputPath:
        """Resolve one artefact path from its variable and default."""
        configured = environment.optional(variables, variable, default)
        return cls(configured=configured, directory=Path(_directory_name(configured)))


def _required(variables: cabc.Mapping[str, str], variable: str, diagnostic: str) -> str:
    """Return a required variable, refusing an unset or empty value."""
    value = variables.get(variable)
    if not value:
        raise ConfigurationError(diagnostic)
    return value


def _directory_name(configured: str) -> str:
    """Return the directory the shell's ``dirname`` named for *configured*.

    The shell computed the ``mkdir`` operand as ``$(dirname "$file")``, so the
    operand is coreutils ``dirname(1)`` applied to the operator's own string --
    not the configured path's :attr:`~pathlib.Path.parent`, and not
    :func:`os.path.dirname` either. All three disagree somewhere, and the
    disagreement that matters is a final ``/.``: ``dirname(1)`` answers
    ``reports`` for ``reports/.``, ``Path("reports/.").parent`` answers ``.``,
    and making the latter is a no-op, so the truncation would then fail with
    ``No such file or directory`` where the shell had already created
    ``reports`` and failed with ``Is a directory``.

    :func:`os.path.dirname` is the near miss rather than the answer: it returns
    ``reports`` for ``reports/`` where ``dirname(1)`` returns ``.``, so
    adopting it would put a new divergence exactly where the trailing-slash
    cases already look.

    Returns
    -------
    str
        The parent directory of the configured string, spelled as the shell's
        ``dirname`` spelled it.

    Examples
    --------
    >>> _directory_name("reports/."), _directory_name("reports/")
    ('reports', '.')
    >>> _directory_name("/obs//m.jsonl"), _directory_name("obs//")
    ('/obs', '.')
    >>> _directory_name("/"), _directory_name("")
    ('/', '.')
    """
    # dirname(1)'s own algorithm: strip the trailing separators first, so a
    # string that was *only* separators has no final component to shed, then
    # shed everything from the last remaining separator onwards. A path with no
    # separator left has no directory part and names the current directory; one
    # whose directory part is empty is the filesystem root.
    stripped = configured.rstrip("/")
    if not stripped:
        return "/" if configured.startswith("/") else "."
    parent, separator, _ = stripped.rpartition("/")
    if not separator:
        return "."
    return parent.rstrip("/") or "/"


def _refusing(lead: str, target: str, operation: cabc.Callable[[], None]) -> None:
    """Perform one filesystem operation on *target*, refusing a rejection.

    ``mkdir`` and the redirection each reported their own sentence in the
    shell, and both reach the caller as the same bounded diagnostic: the
    message keeps the system's own text through :func:`_reason`, so the
    operator still reads *why* the path was refused. One helper serves both,
    because the failure handling is identical.

    Raises
    ------
    ConfigurationError
        When the operating system refuses the operation.
    """
    try:
        operation()
    except OSError as error:
        msg = f"{lead} {target}: {_reason(error)}"
        raise ConfigurationError(msg) from error


def _reason(error: OSError) -> str:
    """Render one operating-system refusal as a short reason.

    The shell printed the operating system's own sentence, so this carries its
    text whenever the system supplied one; the fallback names the class for
    the rare :class:`OSError` raised without a message.

    Returns
    -------
    str
        The system's own description, or the error's class name without one.
    """
    return error.strerror or error.__class__.__name__
