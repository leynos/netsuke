"""Create Linux packages for release artefacts using nFPM.

Examples
--------
Run the script after building the release binaries:

>>> python package_linux.py \
...     --bin-name netsuke --target x86_64-unknown-linux-gnu \
...     --version 1.2.3 --formats deb rpm \
...     --man-path dist/netsuke_linux_amd64/netsuke.1
"""

from __future__ import annotations

import argparse
import gzip
import os
import shutil
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

SUPPORTED_ARCHES = {
    "x86_64-unknown-linux-gnu": "amd64",
    "aarch64-unknown-linux-gnu": "arm64",
}


@dataclass(frozen=True)
class ManifestMetadata:
    """Metadata derived from Cargo.toml."""

    license: str
    description: str
    maintainer: str
    homepage: str


class PackagingError(RuntimeError):
    """Raised when the packaging inputs are invalid."""


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-dir", default=".", help="Workspace root")
    parser.add_argument("--bin-name", required=True)
    parser.add_argument("--package-name", help="Override package name")
    parser.add_argument("--target", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument(
        "--formats",
        nargs="+",
        default=("deb",),
        help="List of nfpm formats to build",
    )
    parser.add_argument(
        "--man-path",
        dest="man_paths",
        action="append",
        default=(),
        help="Man page path relative to project dir",
    )
    parser.add_argument("--man-section", default="1")
    parser.add_argument("--deb-depends", action="append", default=())
    parser.add_argument("--rpm-depends", action="append", default=None)
    parser.add_argument("--outdir", default="dist")
    parser.add_argument("--config-path", default="dist/nfpm.yaml")
    parser.add_argument("--license")
    parser.add_argument("--description")
    parser.add_argument("--maintainer")
    parser.add_argument("--homepage")
    parser.add_argument(
        "--nfpm-binary",
        default="nfpm",
        help="nfpm executable to invoke",
    )
    return parser.parse_args(argv)


def read_manifest(project_dir: Path) -> ManifestMetadata:
    manifest = project_dir / "Cargo.toml"
    if not manifest.is_file():
        raise PackagingError(f"Cargo manifest not found at {manifest}")
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    package = data.get("package", {})
    authors: Iterable[str] = package.get("authors", ())
    maintainer = next(iter(authors), "")
    return ManifestMetadata(
        license=package.get("license", ""),
        description=package.get("description", ""),
        maintainer=maintainer,
        homepage=package.get("homepage", ""),
    )


def normalise_version(version: str) -> str:
    cleaned = version.strip()
    return cleaned.lstrip("v") or cleaned


def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path


def ensure_file(path: Path, message: str) -> Path:
    if not path.is_file():
        raise PackagingError(f"{message}: {path}")
    return path


def dedupe_tokens(values: Iterable[str]) -> list[str]:
    tokens: list[str] = []
    for value in values:
        for token in value.replace(",", " ").split():
            if token and token not in tokens:
                tokens.append(token)
    return tokens


def nfpm_arch(target: str) -> str:
    try:
        return SUPPORTED_ARCHES[target]
    except KeyError as exc:  # pragma: no cover - defensive runtime guard
        raise PackagingError(f"unsupported target triple: {target}") from exc


def infer_section(path: Path, default: str) -> str:
    name = path.name.removesuffix(".gz")
    parts = name.split(".")
    return parts[-1] if len(parts) > 1 else default


def gzip_manpage(src: Path, stage_dir: Path) -> Path:
    if src.suffix == ".gz":
        return src
    ensure_dir(stage_dir)
    dest = stage_dir / f"{src.name}.gz"
    with src.open("rb") as reader, gzip.open(dest, "wb", mtime=0) as writer:
        shutil.copyfileobj(reader, writer)
    return dest


def build_contents(
    project_dir: Path,
    bin_name: str,
    target: str,
    man_paths: Iterable[Path],
    man_section: str,
    stage_dir: Path,
) -> list[dict[str, object]]:
    binary = ensure_file(
        project_dir / "target" / target / "release" / bin_name,
        "built binary missing",
    )
    contents: list[dict[str, object]] = [
        {
            "src": binary.as_posix(),
            "dst": f"/usr/bin/{bin_name}",
            "file_info": {"mode": "0755"},
        }
    ]
    license_file = project_dir / "LICENSE"
    if license_file.is_file():
        contents.append(
            {
                "src": license_file.as_posix(),
                "dst": f"/usr/share/doc/{bin_name}/copyright",
                "file_info": {"mode": "0644"},
            }
        )
    for src in man_paths:
        real_src = ensure_file(project_dir / src, "man page missing")
        gz_path = gzip_manpage(real_src, stage_dir)
        section = infer_section(gz_path, man_section)
        contents.append(
            {
                "src": gz_path.as_posix(),
                "dst": f"/usr/share/man/man{section}/{gz_path.name}",
                "file_info": {"mode": "0644"},
            }
        )
    return contents


def dump_yaml(value: object, indent: int = 0) -> list[str]:
    prefix = "  " * indent
    lines: list[str] = []
    if isinstance(value, dict):
        for key, item in value.items():
            if item in (None, "", [], {}):
                continue
            if isinstance(item, (dict, list)):
                lines.append(f"{prefix}{key}:")
                lines.extend(dump_yaml(item, indent + 1))
            else:
                lines.append(f"{prefix}{key}: {format_scalar(item)}")
    elif isinstance(value, list):
        for item in value:
            if item in (None, "", [], {}):
                continue
            if isinstance(item, (dict, list)):
                lines.append(f"{prefix}-")
                lines.extend(dump_yaml(item, indent + 1))
            else:
                lines.append(f"{prefix}- {format_scalar(item)}")
    else:
        lines.append(f"{prefix}{format_scalar(value)}")
    return lines


def format_scalar(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    text = str(value)
    needs_quote = (
        not text
        or text != text.strip()
        or any(ch in text for ch in ":{}[]#,&*?|-<>=!%@\\\"'")
        or " " in text
    )
    if needs_quote:
        escaped = text.replace("\\", "\\\\").replace("\"", "\\\"")
        return f'"{escaped}"'
    return text


def write_config(config: dict[str, object], destination: Path) -> None:
    ensure_dir(destination.parent)
    lines = dump_yaml(config)
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_nfpm(nfpm_bin: str, config_path: Path, outdir: Path, formats: Sequence[str]) -> None:
    failures: list[tuple[str, int]] = []
    for fmt in formats:
        cmd = [
            nfpm_bin,
            "package",
            "--packager",
            fmt,
            "-f",
            str(config_path),
            "-t",
            str(outdir),
        ]
        print("→", " ".join(cmd))
        try:
            subprocess.run(cmd, check=True)
        except subprocess.CalledProcessError as exc:
            ret = int(exc.returncode or 1)
            print(f"nfpm failed for format '{fmt}' (exit {ret})", file=sys.stderr)
            failures.append((fmt, ret))
    if failures:
        raise PackagingError(
            "; ".join(f"{fmt} failed with exit {code}" for fmt, code in failures)
        )


def main(argv: Sequence[str] | None = None) -> None:
    args = parse_args(argv)
    project_dir = Path(args.project_dir).resolve()
    metadata = read_manifest(project_dir)
    package_name = (args.package_name or args.bin_name).strip() or args.bin_name
    version = normalise_version(args.version)
    target = args.target.strip()
    formats = [fmt.strip() for fmt in args.formats if fmt.strip()]
    if not formats:
        raise PackagingError("no package formats specified")
    man_paths = [Path(path.strip()) for path in args.man_paths]
    stage_dir = project_dir / "dist" / ".man"
    contents = build_contents(
        project_dir,
        args.bin_name,
        target,
        man_paths,
        args.man_section.strip() or "1",
        stage_dir,
    )
    deb_requires = dedupe_tokens(args.deb_depends)
    rpm_values = args.rpm_depends if args.rpm_depends is not None else args.deb_depends
    rpm_requires = dedupe_tokens(rpm_values or ())
    config = {
        "name": package_name,
        "arch": nfpm_arch(target),
        "platform": "linux",
        "version": version,
        "release": "1",
        "maintainer": args.maintainer
        or metadata.maintainer
        or os.environ.get("GITHUB_ACTOR", ""),
        "homepage": args.homepage or metadata.homepage,
        "license": args.license or metadata.license,
        "description": args.description or metadata.description or package_name,
        "contents": contents,
        "overrides": {
            "deb": {"depends": deb_requires},
            "rpm": {"depends": rpm_requires},
        },
    }
    outdir = ensure_dir(project_dir / args.outdir)
    config_path = project_dir / args.config_path
    write_config(config, config_path)
    print(f"wrote {config_path}")
    run_nfpm(args.nfpm_binary, config_path, outdir, formats)


if __name__ == "__main__":  # pragma: no cover - manual invocation
    try:
        main()
    except PackagingError as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(2)
