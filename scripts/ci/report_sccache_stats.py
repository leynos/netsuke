#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.2", "cuprum==0.1.0"]
# ///
"""Record sccache statistics as text, JSON, and a step-summary section.

The compile-cache report runs after the last compile step, on success and on
failure alike. The human-readable statistics go to the log and to
``sccache-stats.txt``; the machine-readable form goes to ``sccache-stats.json``;
and the text is appended to the job summary under an ``sccache`` heading.

Parameters arrive from the environment: ``GITHUB_STEP_SUMMARY`` is ambient in
GitHub Actions; ``INPUT_OUTPUT_DIR`` overrides where the two files land.
"""

import pathlib
import sys
import typing as typ

from ci_support import CiScriptError, catalogue, describe_failure, run

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
    result = run(SCCACHE, "--show-stats", *extra, allowed=ALLOWED, echo=echo)
    if not result.ok:
        message = f"reading statistics failed: {describe_failure(result)}"
        raise CiScriptError(message)
    return result.stdout or ""


def summary_section(text: str) -> str:
    """Return the Markdown block appended to the job summary."""
    body = text if text.endswith("\n") else f"{text}\n"
    return f"{SUMMARY_HEADING}\n\n```text\n{body}```\n"


@app.default
def main(
    *,
    github_step_summary: typ.Annotated[
        pathlib.Path, Parameter(env_var="GITHUB_STEP_SUMMARY")
    ],
    output_dir: pathlib.Path = pathlib.Path(),
) -> int:
    """Write the statistics files and the summary section.

    Parameters
    ----------
    github_step_summary
        The ``GITHUB_STEP_SUMMARY`` file the section is appended to.
    output_dir
        Where ``sccache-stats.txt`` and ``sccache-stats.json`` are written.

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
        handle.write(summary_section(text))
    return 0


if __name__ == "__main__":
    app()
