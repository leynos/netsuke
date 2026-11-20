"""Build the offline probe and the wheel its PEP 723 block installs.

The isolation cases need a script whose dependency can be resolved without a
network and a module whose identity proves where it came from. Both are
handcrafted here rather than fetched: a real index would make the case depend
on the network and on a package nobody pins, and an identity any index could
serve would leave a successful run ambiguous about which source supplied it.

Run via ``make test-workflow-contracts``.
"""

import base64
import hashlib
import typing as typ
import zipfile

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Printed by the probe the gate step is pointed at, so a run that reached the
#: script is distinguishable from one uv refused before starting it.
PROBE_SENTINEL = "verifier-probe-ran"

#: The identity the handcrafted wheel's module reports. It is a value no index
#: could serve, so printing it proves the dependency came from the local file
#: this case built rather than from anywhere else.
PROBE_IDENTITY = "local-wheel"

#: The dependency the offline probe declares, and the module it installs. The
#: name carries no hyphen because uv rejected the handcrafted file name with
#: one -- `admission-probe-dependency-0.1.0-py3-none-any.whl` was refused with
#: "Python tag must not start with a digit" -- and this case is about
#: resolution, not about uv's file-name parsing.
DEPENDENCY_NAME = "admissionprobe"
DEPENDENCY_MODULE = "admissionprobe"
DEPENDENCY_VERSION = "0.1.0"

#: The plain probe: a script that only prints the sentinel. It declares no
#: dependency, so uv runs it in the project environment when project discovery
#: is allowed, which is what makes the project trap observable.
PLAIN_PROBE = f'print("{PROBE_SENTINEL}")\n'


def build_wheel(directory: Path) -> Path:
    """Build a wheel for the probe's declared dependency, with no build tool.

    A real index would make the case depend on the network and on a package
    nobody pins. The wheel is written directly: one module, the metadata that
    names it, the wheel marker, and a `RECORD` whose digests match the bytes
    written, which is what an installer checks.

    Parameters
    ----------
    directory
        Directory to write the wheel into.

    Returns
    -------
    Path
        The wheel file the run is told to resolve from.
    """
    directory.mkdir(parents=True, exist_ok=True)
    stem = f"{DEPENDENCY_NAME}-{DEPENDENCY_VERSION}"
    files = {
        f"{DEPENDENCY_MODULE}/__init__.py": f'IDENTITY = "{PROBE_IDENTITY}"\n',
        f"{stem}.dist-info/METADATA": (
            f"Metadata-Version: 2.1\nName: {DEPENDENCY_NAME}\n"
            f"Version: {DEPENDENCY_VERSION}\nSummary: verifier probe\n"
            "Requires-Python: >=3.8\n"
        ),
        f"{stem}.dist-info/WHEEL": (
            "Wheel-Version: 1.0\nGenerator: handcrafted\n"
            "Root-Is-Purelib: true\nTag: py3-none-any\n"
        ),
    }
    files[f"{stem}.dist-info/RECORD"] = _record(files)
    wheel = directory / f"{stem}-py3-none-any.whl"
    with zipfile.ZipFile(wheel, "w") as archive:
        for path, text in files.items():
            archive.writestr(path, text)
    return wheel


def _record(files: dict[str, str]) -> str:
    """Return a `RECORD` file naming every written member and its digest.

    Parameters
    ----------
    files
        The members written so far, excluding the record itself.

    Returns
    -------
    str
        The record's contents, with the record's own line left unhashed.
    """
    lines = []
    for path, text in files.items():
        encoded = text.encode("utf-8")
        digest = base64.urlsafe_b64encode(hashlib.sha256(encoded).digest())
        checksum = digest.rstrip(b"=").decode("ascii")
        lines.append(f"{path},sha256={checksum},{len(encoded)}")
    lines.append(f"{DEPENDENCY_NAME}-{DEPENDENCY_VERSION}.dist-info/RECORD,,")
    return "\n".join(lines) + "\n"


def dependency_probe() -> str:
    """Return a script whose PEP 723 block declares the local dependency.

    The block is what the gate's real script uses for its own pinned
    dependencies, so this exercises the same resolution path.

    Returns
    -------
    str
        The script the command under test runs.
    """
    return (
        "# /// script\n"
        f'# dependencies = ["{DEPENDENCY_NAME}=={DEPENDENCY_VERSION}"]\n'
        "# ///\n"
        f"import {DEPENDENCY_MODULE}\n"
        f'print("{PROBE_SENTINEL}", {DEPENDENCY_MODULE}.IDENTITY)\n'
    )


__all__ = (
    "DEPENDENCY_MODULE",
    "DEPENDENCY_NAME",
    "DEPENDENCY_VERSION",
    "PLAIN_PROBE",
    "PROBE_IDENTITY",
    "PROBE_SENTINEL",
    "build_wheel",
    "dependency_probe",
)
