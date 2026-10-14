#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = [
#   "cuprum==0.1.0",
#   "cyclopts==4.25.3",
# ]
# ///
"""Emit bounded release-admission observations while checking RFC 0005 inputs.

The gate runs five operations in a fixed order, classifies each without letting
a failure message, a path, or an identifier reach the published records, and
writes a metric file, a trace file, and four workflow outputs. In observation
mode -- the default -- a failed admission is reported and the step still
succeeds; in enforcement mode it fails the step.

This replaced a Bash gate of three sourced scripts. The refusal wording, the
operation order, the argument vectors, the records, and the exit statuses are
unchanged, so an operator's alerting survives the rewrite; the measurements the
port was held to live in ``scripts/tests/release_admission_test_support.py``.
Two differences are intentional, and both are observable only on stderr:

- An adapter the operating system refuses to launch is reported as
  ``release-admission adapter could not be run: <program>: <reason>``. The
  shell reached the same state through GNU ``timeout``, which printed
  ``timeout: failed to run command '<program>': <reason>``. One diagnostic,
  in the same position, with the same exit status and classification; only the
  sentence differs.
- A refused configuration and a refused metric write have no other stderr at
  all, exactly as before. The enforcement mode's own value never decides the
  status, so a refused observation mode still fails the step.

One behaviour is deliberately preserved although it reads as a contradiction:
``check_scan_freshness`` admits ``fresh``, but ``verify_evidence`` refuses it
unless the state is *not* fresh and a workflow run identifier is present. The
freshness check passing is never sufficient for admission. The port's own
module docstrings record each remaining difference from the shell, and
``scripts/tests/release_admission_test_support.py`` holds the measurements both
were made from.

Usage
-----
In the workflow, from the repository root:

```
uv run --script .github/scripts/release_admission.py
```
"""

import dataclasses
import os
import sys
import typing as typ
from pathlib import Path

# The workflow runs this file by path, from the repository root, so the package
# beside it is not importable by default. `lint-workflow-scripts` loads it
# through `runpy`, where `sys.path[0]` is the empty string -- the current
# directory at import time, not this file's directory -- so relying on the
# ambient path would resolve the import in one of the two callers and not the
# other. Doing it explicitly here makes both work.
sys.path.insert(0, str(Path(__file__).resolve().parent))

#: The status a configuration that nothing can be observed from exits with.
CONFIGURATION_FAILURE = 1

if typ.TYPE_CHECKING:
    # An annotation-only import, named absolutely because a relative one would
    # not resolve from a module the loader runs as ``__main__``. It is never
    # executed, so the ``runpy`` load above is unaffected; the only executed
    # import of the package is the one inside :func:`run`.
    from _release_admission import configuration


def run() -> int:
    """Build the Cyclopts application, run it, and return its exit status.

    Every dependency is imported here rather than at module scope, because
    ``lint-workflow-scripts`` loads this file with ``runpy`` under the bare
    Python baseline -- no ``cyclopts``, no ``cuprum`` -- and a module-scope
    import would make that gate fail on a dependency it deliberately does not
    install. Nothing above this function is executed by that loader, so the
    contract holds without weakening the check.

    Returns
    -------
    int
        ``0`` for an observation, ``1`` for a refused configuration or an
        enforced failure, and otherwise the failing sink's own status.
    """
    from _release_admission import (
        configuration,
        gate,
    )

    from cyclopts import (
        App,
        Parameter,
    )

    app = App(help=__doc__)

    @app.default
    def main(
        *,
        repository: typ.Annotated[
            str, Parameter(env_var=configuration.REPOSITORY_VARIABLE)
        ] = "",
        revision: typ.Annotated[
            str, Parameter(env_var=configuration.REVISION_VARIABLE)
        ] = "",
        output: typ.Annotated[
            str, Parameter(env_var=configuration.OUTPUT_VARIABLE)
        ] = "",
    ) -> int:
        """Run the admission gate and return the status it exits with.

        The three required variables are bound as ordinary parameters so
        Cyclopts shows them in ``--help``. An empty value is left for the gate
        itself to refuse, because the shell's diagnostic named the variable and
        the value's emptiness is the operator's actual problem.

        Returns
        -------
        int
            ``0`` for an observation, ``1`` for a refused configuration or an
            enforced failure, and otherwise the failing sink's own status.
        """
        del repository, revision, output
        try:
            settings = _start_up()
        except configuration.ConfigurationError as error:
            print(error, file=sys.stderr)
            return CONFIGURATION_FAILURE
        admission = gate.Gate(settings)
        if not admission.should_stop():
            admission.run_operations()
        return admission.finish()

    # Cyclopts' default result action is ``print_non_int_return_int_as_exit_code``,
    # which returns an integer result unchanged. Dropping it here would make
    # every enforced failure exit ``0``, which is the one thing the shell's
    # ``set -e`` semantics must survive the port intact.
    return typ.cast("int", app())


def _start_up() -> configuration.Configuration:
    """Resolve the gate's settings, creating its artefacts, in the shell's order.

    A required variable that is unset or empty, or an artefact effect the
    operating system refuses, reaches the caller as
    :class:`_release_admission.configuration.ConfigurationError`; the caller
    prints its message, which is the diagnostic itself.

    Returns
    -------
    configuration.Configuration
        The resolved settings, with ``output`` filled in from
        ``GITHUB_OUTPUT``.

    Notes
    -----
    Four steps, in the order the shell refused in: resolve the artefact
    spellings, resolve the settings, create both parents and truncate both
    files, and only then read ``GITHUB_OUTPUT``. Each refusal's *position* is
    part of the contract -- the environment-contract tests run the workflow's
    step order to pin it -- so the order is stated here, in the one place that
    owns it, rather than inside any of the steps.
    """
    from _release_admission import configuration, filesystem

    variables = os.environ
    destination = configuration.resolve_destination(variables)
    settings = configuration.resolve_configuration(variables, destination)
    file_system = filesystem.production_filesystem()
    configuration.prepare_destination(settings.sinks, file_system)
    return dataclasses.replace(
        settings, output=configuration.require_workflow_output(variables)
    )


if __name__ == "__main__":
    raise SystemExit(run())
