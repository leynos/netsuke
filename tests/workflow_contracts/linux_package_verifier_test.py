"""Contract tests for the installed-package verifier.

The sandbox runs ``verify-installed-package.sh`` against an installed package,
and that script alone decides what "installed" means:

- ``--version`` and ``man -w`` prove the package really works;
- the dependency must be declared in Debian or RPM metadata;
- a missing inspection tool must fail rather than pass vacuously; and
- the verifier must invoke each package tool with the arguments it checks.

Each case runs the script against a private ``PATH`` of recording stubs, so the
behaviour is proven rather than scanned. Sandbox and image contracts live in
``linux_package_validation_sandbox_test.py``; workflow wiring lives in
``linux_package_validation_gate_test.py``. Run via
``make test-workflow-contracts``.
"""

import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the verifier's own script is under test.
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

VALIDATION_DIR = REPO_ROOT / ".github" / "package-validation"
VERIFY_SCRIPT = VALIDATION_DIR / "verify-installed-package.sh"

#: The exact argv the verifier promises each package tool. A stub that accepted
#: any arguments would keep passing after the script stopped naming the package,
#: so every dependency stub below rejects an argv it did not expect.
DEB_ARGV = ("-W", "-f=${Depends}\\n", "netsuke")
RPM_ARGV = ("-q", "--requires", "netsuke")
VERSION_ARGV = ("--version",)
MAN_ARGV = ("-w", "netsuke")


def _pinned_stub(command: str, expected: tuple[str, ...], output: str) -> str:
    """Return a stub that refuses any argv but ``expected`` and prints ``output``.

    The count check comes first because an unset positional parameter would
    otherwise compare equal to an empty expected argument and let a shortened
    invocation through.

    Returns
    -------
    str
        The shell body of a stub that prints ``output`` when the argv matches.
    """
    conditions = [f'[ "$#" -eq {len(expected)} ]']
    conditions.extend(
        f"[ \"${position}\" = '{argument}' ]"
        for position, argument in enumerate(expected, start=1)
    )
    guarded = " &&\n    ".join(conditions)
    return (
        f"if ! {{ {guarded}; }}\n"
        "then\n"
        f"    printf 'unexpected {command} argv: %s\\n' \"$*\" >&2\n"
        "    exit 9\n"
        "fi\n"
        f"printf '{output}'"
    )


def _deb_stub(depends: str) -> str:
    """Return a ``dpkg-query`` stub reporting ``depends`` for the package."""
    return _pinned_stub("dpkg-query", DEB_ARGV, f"{depends}\\n")


def _rpm_stub(requirements: str) -> str:
    """Return an ``rpm`` stub reporting ``requirements`` for the package."""
    return _pinned_stub("rpm", RPM_ARGV, f"{requirements}\\n")


PASSING_STUBS = {
    "netsuke": _pinned_stub("netsuke", VERSION_ARGV, "netsuke 0.1.0\\n"),
    "man": _pinned_stub("man", MAN_ARGV, "/usr/share/man/man1/netsuke.1.gz\\n"),
    "dpkg-query": _deb_stub("ninja-build"),
}


def _write_stub(bin_dir: Path, name: str, body: str) -> None:
    """Write an executable shell stub named ``name`` into ``bin_dir``."""
    path = bin_dir / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def _link_grep(bin_dir: Path) -> None:
    """Link the host ``grep`` into ``bin_dir`` so an isolated PATH can find it."""
    grep = shutil.which("grep")
    assert grep is not None, "the host must provide grep for the verifier to work"
    (bin_dir / "grep").symlink_to(grep)


def _run_verifier(
    tmp_path: Path, stubs: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run the verification script against a private set of command stubs."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for name, body in stubs.items():
        _write_stub(bin_dir, name, body)
    _link_grep(bin_dir)
    # A minimal PATH keeps each dependency probe deterministic for its case.
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the script under test is checked in.
        ["/bin/sh", str(VERIFY_SCRIPT), "netsuke"],
        check=False,
        capture_output=True,
        text=True,
        env={"PATH": str(bin_dir)},
    )


@pytest.mark.parametrize(
    ("stubs", "expected_status", "expected_message"),
    [
        (PASSING_STUBS, 0, "PASS: Debian package netsuke declares ninja-build"),
        (
            {**PASSING_STUBS, "netsuke": "exit 1"},
            1,
            "FAIL: netsuke --version failed",
        ),
        (
            {**PASSING_STUBS, "man": "exit 0"},
            1,
            "FAIL: man -w netsuke returned an empty path",
        ),
        (
            {**PASSING_STUBS, "dpkg-query": _deb_stub("libc6 (>= 2.34)")},
            1,
            "FAIL: Debian package netsuke does not declare ninja-build",
        ),
        (
            {**PASSING_STUBS, "dpkg-query": _deb_stub("libc6, ninja-build")},
            0,
            "PASS: Debian package netsuke declares ninja-build",
        ),
        (
            {
                **PASSING_STUBS,
                "dpkg-query": _deb_stub("libc6, ninja-build:any (>= 1.10)"),
            },
            0,
            "PASS: Debian package netsuke declares ninja-build",
        ),
        (
            {**PASSING_STUBS, "dpkg-query": _deb_stub("ninja-build-extra")},
            1,
            "FAIL: Debian package netsuke does not declare ninja-build",
        ),
    ],
)
def test_verification_script_enforces_the_declared_checks(
    tmp_path: Path,
    stubs: dict[str, str],
    expected_status: int,
    expected_message: str,
) -> None:
    """Exercise the script's pass and fail paths against stub commands.

    The script is the only part of the gate that decides what "installed"
    means, so it is run rather than scanned: each case withholds one thing the
    release promises and the expected diagnostic proves which check fired.
    """
    result = _run_verifier(tmp_path, stubs)
    assert result.returncode == expected_status, (
        f"expected exit {expected_status}, got {result.returncode}: "
        f"{result.stdout}{result.stderr}"
    )
    assert expected_message in result.stdout + result.stderr, (
        f"expected {expected_message!r} in the verifier's output, got "
        f"{result.stdout!r} and {result.stderr!r}"
    )


@pytest.mark.parametrize(
    ("requirements", "expected_status", "expected_message"),
    [
        (
            "libc.so.6()(64bit)\\nninja-build",
            0,
            "PASS: RPM package netsuke declares ninja-build",
        ),
        (
            "libc.so.6()(64bit)\\nninja-build-devel",
            1,
            "FAIL: RPM package netsuke does not declare ninja-build",
        ),
    ],
)
def test_verification_script_reads_rpm_requirements(
    tmp_path: Path,
    requirements: str,
    expected_status: int,
    expected_message: str,
) -> None:
    """Read a declared dependency from RPM metadata when dpkg is absent.

    The RPM image has no ``dpkg-query``, so the script's other branch has to
    inspect ``rpm -q --requires`` output. Both directions are exercised: a
    requirement list containing ``ninja-build`` passes, and one whose only
    similar entry is ``ninja-build-devel`` fails, which proves the branch reads
    metadata instead of trusting the package name. A private PATH models that
    image rather than depending on the test host's own tools.
    """
    stubs = {
        "netsuke": PASSING_STUBS["netsuke"],
        "man": PASSING_STUBS["man"],
        "rpm": _rpm_stub(requirements),
    }
    result = _run_verifier(tmp_path, stubs)
    assert result.returncode == expected_status, (
        f"expected exit {expected_status} for {requirements!r}, got "
        f"{result.returncode}: {result.stdout}{result.stderr}"
    )
    assert expected_message in result.stdout + result.stderr, (
        f"expected {expected_message!r} in the verifier's output, got "
        f"{result.stdout!r} and {result.stderr!r}"
    )


def test_verification_script_fails_without_a_dependency_tool(tmp_path: Path) -> None:
    """Fail clearly when neither package tool can inspect the dependency.

    Silently passing here would turn a missing check into a green gate, which
    is the one failure mode this gate must never have.
    """
    result = _run_verifier(
        tmp_path, {"netsuke": PASSING_STUBS["netsuke"], "man": PASSING_STUBS["man"]}
    )
    assert result.returncode == 1, (
        f"the verifier must fail when it cannot read dependencies, got "
        f"{result.returncode}: {result.stdout}{result.stderr}"
    )
    assert "neither dpkg-query nor rpm is available" in result.stderr, (
        f"the verifier must name the missing tools, got {result.stderr!r}"
    )
