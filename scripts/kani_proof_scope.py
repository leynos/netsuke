#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = [
#   "cuprum==0.1.0",
#   "cyclopts==4.25.3",
# ]
# ///
"""Decide whether the `kani-smoke` job must run its proofs.

`kani-smoke` is a required check, so it must report on every pull request.
Running every harness on every pull request costs about seven minutes a run,
although most pull requests change nothing a proof depends on. This script is
the job's first step: it reads the proof scope in `tools/kani/proof-scope.toml`
and writes `run-proofs=true` or `run-proofs=false` to the step's outputs,
which every later step of the job is conditioned on. A skipped job still ends
green and says why in the job summary.

The decision is conservative in both directions it can fail:

- Any event other than `pull_request` (a push to `main`, the nightly
  `schedule`, a manual dispatch) runs every harness, whatever it changes.
- On a pull request, the change set is the diff between the merge commit
  GitHub checks out and its first parent, the base branch tip. When that diff
  cannot be read (the checkout is not a two-parent merge, or git fails), the
  proofs run.

A malformed scope file is an error rather than a decision, so a broken scope
fails the step loudly instead of silently running, or skipping, everything.

Usage
-----
In the workflow, with ``INPUT_EVENT_NAME`` set from ``github.event_name``:

```
uv run --script scripts/kani_proof_scope.py
```

Locally, with a pull request's merge commit checked out, to see what it would
decide:

```
uv run --script scripts/kani_proof_scope.py --event-name pull_request
```
"""

import dataclasses
import tomllib
import typing as typ
from pathlib import Path

from cuprum import ExecutionContext, Program, ProgramCatalogue, ProjectSettings
from cuprum import sh as cuprum_sh

import cyclopts
from cyclopts import App, Parameter

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCOPE_FILE = REPO_ROOT / "tools" / "kani" / "proof-scope.toml"
SCOPE_KEYS = ("sources", "infrastructure")
#: How many matching paths the summary names before eliding the rest.
SUMMARY_PATH_LIMIT = 10
#: `git rev-list --parents` prints a merge commit followed by its two parents.
MERGE_COMMIT_FIELDS = 3
GIT = Program("git")
CATALOGUE = ProgramCatalogue(
    projects=[
        ProjectSettings(
            name="git",
            programs=(GIT,),
            documentation_locations=("https://git-scm.com/docs",),
            noise_rules=(),
        )
    ]
)

app = App(help=__doc__, config=cyclopts.config.Env("INPUT_", command=False))


class ScopeFileError(Exception):
    """Report a proof-scope file the decision cannot trust."""


@dataclasses.dataclass(frozen=True, slots=True)
class Decision:
    """Whether the proofs run, and the sentence explaining why."""

    run_proofs: bool
    reason: str


@Parameter(name="*")
@dataclasses.dataclass(frozen=True, slots=True)
class StepFiles:
    """The files GitHub Actions reads a step's outputs and job summary from."""

    output: typ.Annotated[Path | None, Parameter(env_var="GITHUB_OUTPUT")] = None
    summary: typ.Annotated[Path | None, Parameter(env_var="GITHUB_STEP_SUMMARY")] = None


def read_scope(scope_file: Path) -> tuple[str, ...]:
    """Return every path in the scope file, sources first.

    An entry ending in `/` names a directory and matches everything beneath
    it; any other entry names one file exactly.

    Returns
    -------
    tuple[str, ...]
        The `sources` entries followed by the `infrastructure` entries.

    Raises
    ------
    ScopeFileError
        When the file is missing, is not TOML, or lacks a non-empty list of
        strings under either key of its `[scope]` table.
    """
    try:
        document = tomllib.loads(scope_file.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        msg = f"cannot read the proof scope {scope_file}: {error}"
        raise ScopeFileError(msg) from error
    scope = document.get("scope")
    entries: list[str] = []
    for key in SCOPE_KEYS:
        values = scope.get(key) if isinstance(scope, dict) else None
        if not values or not all(isinstance(value, str) and value for value in values):
            msg = f"{scope_file}: [scope] {key} must be a non-empty list of paths"
            raise ScopeFileError(msg)
        entries.extend(values)
    return tuple(entries)


def is_in_scope(path: str, scope: tuple[str, ...]) -> bool:
    """Return whether a repository-relative ``path`` falls under ``scope``.

    Returns
    -------
    bool
        Whether a directory entry is a whole-segment prefix of ``path``, or a
        file entry equals it.

    Examples
    --------
    >>> is_in_scope("src/ir/graph.rs", ("src/ir/", "Cargo.toml"))
    True
    >>> is_in_scope("src/irony.rs", ("src/ir/",))
    False
    """
    return any(
        path.startswith(entry) if entry.endswith("/") else path == entry
        for entry in scope
    )


def _git(repository: Path, *arguments: str) -> str | None:
    """Return a git command's standard output, or `None` when it fails."""
    command = cuprum_sh.make(GIT, catalogue=CATALOGUE)(*arguments)
    result = command.run_sync(context=ExecutionContext(cwd=repository))
    return (
        result.stdout if result.exit_code == 0 and result.stdout is not None else None
    )


def pull_request_changes(repository: Path) -> list[str] | None:
    """Return the paths a pull request's merge commit changes.

    `actions/checkout` checks out the merge commit `refs/pull/N/merge`, whose
    first parent is the base branch tip, so the diff against that parent is
    exactly what merging would change. `--no-renames` reports a rename as a
    deletion and an addition, so a file moved out of the scope still counts.

    Returns
    -------
    list[str] | None
        The changed paths, or `None` when HEAD is not a two-parent merge or
        git fails, so that the caller runs the proofs.
    """
    parents = _git(repository, "rev-list", "--parents", "--max-count=1", "HEAD")
    if parents is None or len(parents.split()) != MERGE_COMMIT_FIELDS:
        return None
    changed = _git(repository, "diff", "--name-only", "--no-renames", "HEAD^1", "HEAD")
    return None if changed is None else [line for line in changed.splitlines() if line]


def decide(
    event_name: str, scope: tuple[str, ...], changes: list[str] | None
) -> Decision:
    """Return the decision for one event and its change set.

    Returns
    -------
    Decision
        Run on every event but a pull request, and on a pull request whose
        change set is unreadable or touches the scope; skip otherwise.

    Examples
    --------
    >>> decide("push", ("src/ir/",), None).run_proofs
    True
    >>> decide("pull_request", ("src/ir/",), ["README.md"]).run_proofs
    False
    """
    if event_name != "pull_request":
        return Decision(
            run_proofs=True, reason=f"A `{event_name}` run verifies every harness."
        )
    if changes is None:
        return Decision(
            run_proofs=True,
            reason="The pull request's change set could not be read, so every "
            "harness runs.",
        )
    matched = [path for path in changes if is_in_scope(path, scope)]
    if matched:
        named = ", ".join(f"`{path}`" for path in matched[:SUMMARY_PATH_LIMIT])
        more = len(matched) - SUMMARY_PATH_LIMIT
        elided = f" and {more} more" if more > 0 else ""
        return Decision(
            run_proofs=True,
            reason=f"This pull request changes proof inputs: {named}{elided}.",
        )
    return Decision(
        run_proofs=False,
        reason=f"This pull request changes {len(changes)} path(s), none of them among "
        f"the {len(scope)} proof inputs in `tools/kani/proof-scope.toml`, so the "
        "harnesses would verify the same inputs as on `main`. Every push to "
        "`main` and the nightly schedule run them in full.",
    )


def publish(decision: Decision, files: StepFiles) -> None:
    """Write the decision to the step outputs, the job summary and the log."""
    value = "true" if decision.run_proofs else "false"
    if files.output is not None:
        with files.output.open("a", encoding="utf-8") as stream:
            stream.write(f"run-proofs={value}\n")
    heading = "Kani proofs run" if decision.run_proofs else "Kani proofs skipped"
    if files.summary is not None:
        with files.summary.open("a", encoding="utf-8") as stream:
            stream.write(f"### {heading}\n\n{decision.reason}\n")
    print(f"::notice title={heading}::{decision.reason}")


@app.default
def main(
    *,
    event_name: typ.Annotated[str, Parameter(required=True)],
    repository: Path = REPO_ROOT,
    scope_file: Path = DEFAULT_SCOPE_FILE,
    files: StepFiles | None = None,
) -> None:
    """Decide whether the proofs run and publish the decision.

    A scope file that cannot be trusted raises out of here, which fails the
    step rather than deciding from it.
    """
    scope = read_scope(scope_file)
    changes = pull_request_changes(repository) if event_name == "pull_request" else None
    publish(decide(event_name, scope, changes), files or StepFiles())


if __name__ == "__main__":
    app()
