"""Run a command, and run it against the coverage lane's build tree.

Every listing this package performs must reuse the instrumented tree the
coverage step already populated rather than compiling a second one. Both the
subprocess boundary and the environment that selects that tree live here, so
the reuse is one function's job rather than a habit each caller repeats.
"""

import os
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the cargo boundary is this package's job.
import sys

#: One `export NAME=value` line of `cargo llvm-cov show-env --export-prefix`
#: output. The value is single-quoted only when it needs to be, so both
#: spellings are accepted; requiring the quotes silently drops the unquoted
#: lines, which are most of them.
EXPORTED_VARIABLE = re.compile(
    r"^export (?P<name>[A-Za-z_][A-Za-z0-9_]*)="
    r"(?:'(?P<single>.*)'|(?P<bare>\S*))$"
)

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
            value = (
                match.group("single")
                if match.group("single") is not None
                else match.group("bare")
            )
            env[match.group("name")] = value
    target_dir = env.get(LLVM_COV_TARGET_DIR)
    if not target_dir:
        fail(
            f"`cargo llvm-cov show-env` reported no {LLVM_COV_TARGET_DIR}, so "
            "there is no instrumented tree to reuse"
        )
    env["CARGO_TARGET_DIR"] = f"{target_dir}/llvm-cov-target"
    return env
