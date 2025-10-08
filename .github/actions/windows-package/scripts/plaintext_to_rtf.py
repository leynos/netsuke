#!/usr/bin/env python3
"""Convert UTF-8 plain text license files to RTF for WiX."""

from __future__ import annotations

import argparse
from pathlib import Path


def _positive_int(value: str) -> int:
    """Return ``value`` as an integer ensuring it is positive."""
    try:
        point_size = int(value)
    except ValueError as exc:
        message = "Invalid point size: must be an integer"
        raise argparse.ArgumentTypeError(message) from exc
    if point_size < 1:
        message = "Invalid point size: must be >= 1"
        raise argparse.ArgumentTypeError(message)
    return point_size


_ESCAPE_RTF = {
    "\\": r"\\",
    "{": r"\{",
    "}": r"\}",
    "\t": r"\tab ",
    "\n": r"\par\n",
}

_LINE_SEPARATOR_TRANSLATION = {0x2028: "\n", 0x2029: "\n"}


def _unicode_to_rtf_units(ch: str) -> list[str]:
    r"""Return the RTF ``\u`` escapes needed to represent ``ch``."""
    data = ch.encode("utf-16le")
    units: list[str] = []
    for index in range(0, len(data), 2):
        code_unit = int.from_bytes(data[index : index + 2], "little", signed=True)
        units.append(rf"\u{code_unit}?")
    return units


def _escape_plaintext_to_rtf(text: str) -> str:
    """Escape plain text for inclusion in an RTF body."""
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    text = text.translate(_LINE_SEPARATOR_TRANSLATION)
    text = text.removeprefix("\ufeff")
    out: list[str] = []
    for ch in text:
        if esc := _ESCAPE_RTF.get(ch):
            out.append(esc)
        elif " " <= ch <= "~":
            out.append(ch)
        else:
            out.extend(_unicode_to_rtf_units(ch))
    return "".join(out)


def _validate_font_name(font: str) -> None:
    """Ensure ``font`` does not contain disallowed characters."""
    if any(ch in font for ch in {";", "\r", "\n"}):
        message = "font must not contain ';' or newline characters"
        raise ValueError(message)
    if any(ord(ch) < 0x20 for ch in font):
        message = "font must not contain control characters"
        raise ValueError(message)


def _escape_font_name(font: str) -> str:
    """Return ``font`` escaped for inclusion inside the font table."""
    _validate_font_name(font)
    escaped: list[str] = []
    for ch in font:
        if ch in {"\\", "{", "}"}:
            escaped.append(_ESCAPE_RTF[ch])
        elif " " <= ch <= "~":
            escaped.append(ch)
        else:
            escaped.extend(_unicode_to_rtf_units(ch))
    return "".join(escaped)


def _font_argument(value: str) -> str:
    """Validate ``value`` for argparse, surfacing user-friendly errors."""
    try:
        _validate_font_name(value)
    except ValueError as exc:
        message = f"Invalid font: {exc}"
        raise argparse.ArgumentTypeError(message) from exc
    return value


_RTF_HEADER_TEMPLATE = (
    r"{{\rtf1\ansi\deff0\uc1"
    r"{{\fonttbl{{\f0 {font};}}}}"
    r"\f0\fs{fs}\pard "
    "\n"
)


def _build_parser() -> argparse.ArgumentParser:
    """Return an argument parser for the converter CLI."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input", required=True, help="UTF-8 plain text license file to convert."
    )
    parser.add_argument(
        "--output",
        help="Destination RTF path. Defaults to replacing the input suffix with .rtf.",
    )
    parser.add_argument(
        "--font",
        default="Calibri",
        type=_font_argument,
        help="Font name embedded in the RTF header.",
    )
    parser.add_argument(
        "--point-size",
        type=_positive_int,
        default=11,
        help="Font point size for the generated RTF (integer, default: 11).",
    )
    return parser


def text_to_rtf(text: str, font: str = "Calibri", pt_size: int = 11) -> str:
    """Return an RTF document containing ``text`` rendered with ``font``."""
    fs = max(1, pt_size * 2)
    header = _RTF_HEADER_TEMPLATE.format(font=_escape_font_name(font), fs=fs)
    body = _escape_plaintext_to_rtf(text)
    return header + body + "}"


def convert_file(
    in_path: str,
    out_path: str | None = None,
    *,
    font: str = "Calibri",
    pt_size: int = 11,
) -> Path:
    """Convert ``in_path`` to RTF, returning the written destination path."""
    src = Path(in_path)
    if out_path:
        dst = Path(out_path)
    elif src.suffix:
        dst = src.with_suffix(".rtf")
    else:
        dst = src.with_name(f"{src.name}.rtf")
    dst.write_text(
        text_to_rtf(
            src.read_text(encoding="utf-8-sig"),
            font=font,
            pt_size=pt_size,
        ),
        encoding="utf-8",
        newline="\n",
    )
    return dst


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Return parsed command-line arguments for the converter CLI."""
    parser = _build_parser()
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    """Invoke :func:`convert_file` and print the generated path."""
    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        output = convert_file(
            args.input, args.output, font=args.font, pt_size=args.point_size
        )
    except ValueError as exc:
        parser.error(f"Invalid font: {exc}")
    print(output)


if __name__ == "__main__":
    main()
