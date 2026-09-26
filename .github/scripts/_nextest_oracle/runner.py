"""Run a command, and run it against the coverage lane's build tree.

Every listing this package performs must reuse the instrumented tree the
coverage step already populated rather than compiling a second one. Both the
subprocess boundary and the environment that selects that tree live here, so
the reuse is one function's job rather than a habit each caller repeats.
"""

import os
import re
import shlex
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the cargo boundary is this package's job.
import sys

#: One `export NAME=value` line of `cargo llvm-cov show-env --export-prefix`
#: output. This pattern only *locates* the assignment and names it; the value
#: is decoded by [`_decode_value`] rather than read straight out of the match.
EXPORTED_VARIABLE = re.compile(r"^export (?P<name>[A-Za-z_][A-Za-z0-9_]*)=.*$")


def _decode_value(assignment: str) -> str:
    """Return the effective value of one ``export``-prefixed assignment.

    `cargo llvm-cov show-env --export-prefix` prints shell-sourceable text, so
    a value is quoted only when it needs to be and a quoted value escapes an
    embedded apostrophe as ``'\\''``. Reading such a value back verbatim would
    carry that shell syntax into the environment, so a path containing an
    apostrophe would export the wrong string and make this step's fingerprint
    differ from the coverage run's -- which is the one thing the reuse of the
    instrumented tree depends on.

    The whole `NAME=value` word is decoded rather than the quoted fragment
    alone, because the escaping is a property of the word: the fragment
    `\\'` is unterminated once the surrounding quotes are stripped.

    Either a word the shell cannot parse, or an unquoted value containing
    whitespace, ends the run through `fail`. The first would otherwise surface
    as a traceback; the second would be truncated at the first space, exporting
    a plausible-looking wrong path -- and a silent truncation here is the same
    class of fault as the escaping bug, so it is refused rather than guessed.

    Parameters
    ----------
    assignment : str
        The text after ``export ``, such as ``NAME='/tmp/a b'``.

    Returns
    -------
    str
        The value the shell would assign.
    """
    try:
        words = shlex.split(assignment)
    except ValueError as error:
        fail(
            f"`cargo llvm-cov show-env` printed an assignment this step "
            f"cannot parse ({error}): {assignment}"
        )
    if len(words) != 1:
        fail(
            f"`cargo llvm-cov show-env` printed an assignment whose value is "
            f"not shell-quoted, so its extent is ambiguous and this step would "
            f"truncate it at the first space: {assignment}"
        )
    return words[0].split("=", 1)[1]


#: The environment variable holding the instrumented build tree.
LLVM_COV_TARGET_DIR = "CARGO_LLVM_COV_TARGET_DIR"


def fail(message: str) -> None:
    """Report ``message`` on stderr and exit non-zero."""
    print(f"::error title=Nextest anchored filters::{message}", file=sys.stderr)
    raise SystemExit(1)


def run_command(
    argv: list[str], env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run ``argv`` and return the completed process, echoing what ran."""
    print(f"+ {' '.join(argv)}", flush=True)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed argument list, never a shell.
        argv, capture_output=True, check=False, env=env, shell=False, text=True
    )


def instrumented_environment() -> dict[str, str]:
    """Return the environment of the coverage step's instrumented build tree.

    `cargo llvm-cov show-env --export-prefix` prints the coverage action's own
    environment as shell assignments. Exporting them and then selecting the
    coverage target directory puts this step's build in the tree the coverage
    run already populated, so `cargo nextest list` reuses it instead of
    compiling. The ambient `RUSTFLAGS` is preserved by the command and joined
    with the instrumentation flags, which is what keeps this step's fingerprint
    equal to the coverage run's.

    A `cargo llvm-cov` that is absent, or that reports no target directory, is
    refused by name rather than allowed to proceed. That is the one condition
    under which reuse quietly becomes a second build, and a build that happens
    to succeed on a warm cache would hide it.

    Returns
    -------
    dict[str, str]
        The environment to run `cargo nextest list` under.
    """
    completed = run_command(
        ["cargo", "llvm-cov", "show-env", "--export-prefix"], dict(os.environ)
    )
    if completed.returncode != 0:
        fail(
            "`cargo llvm-cov show-env` failed, so the instrumented build tree "
            f"cannot be reused and this step would become a second build: "
            f"{completed.stderr.strip()}"
        )
    env = dict(os.environ)
    for line in completed.stdout.splitlines():
        match = EXPORTED_VARIABLE.match(line)
        if match is not None:
            env[match.group("name")] = _decode_value(line.removeprefix("export "))
    target_dir = env.get(LLVM_COV_TARGET_DIR)
    if not target_dir:
        fail(
            f"`cargo llvm-cov show-env` reported no {LLVM_COV_TARGET_DIR}, so "
            "there is no instrumented tree to reuse"
        )
    env["CARGO_TARGET_DIR"] = f"{target_dir}/llvm-cov-target"
    return env
