#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
# ///
"""Record sccache statistics as text, JSON, and a step-summary section.

The compile-cache report runs after the last compile step, on success and on
failure alike. The human-readable statistics go to the log and to
``sccache-stats.txt``; the machine-readable form goes to ``sccache-stats.json``;
and the text is appended to the job summary under an ``sccache`` heading.

Parameters arrive from the environment: ``GITHUB_STEP_SUMMARY`` is ambient in
GitHub Actions; ``INPUT_OUTPUT_DIR`` overrides where the two files land, and
``SCCACHE_BACKEND`` names the cache backend the lane selected. That one keeps
its own name rather than taking the ``INPUT_`` prefix, because it is the
variable `setup-rust` and the Windows lanes already export.
"""

import pathlib
import sys
import typing as typ

from ci_support import CiScriptError, RunSpec, catalogue, describe_failure, run

import cyclopts
from cyclopts import App, Parameter

SCCACHE = "sccache"
ALLOWED = catalogue(SCCACHE)
TEXT_NAME = "sccache-stats.txt"
JSON_NAME = "sccache-stats.json"
SUMMARY_HEADING = "### sccache"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def show_stats(*extra: str, echo: bool) -> str:
    """Return ``sccache --show-stats`` output, failing on a non-zero exit."""
    result = run(SCCACHE, "--show-stats", *extra, spec=RunSpec(ALLOWED, echo=echo))
    if not result.ok:
        message = f"reading statistics failed: {describe_failure(result)}"
        raise CiScriptError(message)
    return result.stdout or ""


def summary_section(text: str, backend: str = "") -> str:
    """Return the Markdown block appended to the job summary.

    Parameters
    ----------
    text
        The human-readable ``sccache --show-stats`` output.
    backend
        The cache backend `setup-rust` selected, named above the statistics.
        Empty on a lane that wires its own cache, which then gets no backend
        line. The statistics' own ``Cache location`` cannot stand in for it:
        that reads ``ghac`` for the Ubicloud cache proxy and GitHub's own
        service alike, so it cannot tell the two apart.

    Returns
    -------
    str
        The fenced section, always closing the block on its own line.
    """
    body = text if text.endswith("\n") else f"{text}\n"
    named = f"- backend: {backend}\n\n" if backend else ""
    return f"{SUMMARY_HEADING}\n\n{named}```text\n{body}```\n"


@app.default
def main(
    *,
    github_step_summary: typ.Annotated[
        pathlib.Path, Parameter(env_var="GITHUB_STEP_SUMMARY")
    ],
    output_dir: pathlib.Path = pathlib.Path(),
    backend: typ.Annotated[str, Parameter(env_var="SCCACHE_BACKEND")] = "",
) -> int:
    """Write the statistics files and the summary section.

    Parameters
    ----------
    github_step_summary
        The ``GITHUB_STEP_SUMMARY`` file the section is appended to.
    output_dir
        Where ``sccache-stats.txt`` and ``sccache-stats.json`` are written.
    backend
        The backend `setup-rust` selected; empty on a lane that wires its
        own compiler cache, which then reports no backend.

    Returns
    -------
    int
        ``0`` when both reports were captured, ``1`` otherwise.
    """
    try:
        text = show_stats(echo=True)
        machine = show_stats("--stats-format=json", echo=False)
    except CiScriptError as error:
        print(f"report_sccache_stats: {error}", file=sys.stderr)
        return 1
    (output_dir / TEXT_NAME).write_text(text, encoding="utf-8")
    (output_dir / JSON_NAME).write_text(machine, encoding="utf-8")
    with github_step_summary.open("a", encoding="utf-8") as handle:
        handle.write(summary_section(text, backend))
    return 0


if __name__ == "__main__":
    app()
