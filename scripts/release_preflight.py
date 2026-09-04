#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_version import verify_release_metadata

_WINDOWS_INVALID_CHARS = frozenset('<>:"\\|?*')
_WINDOWS_RESERVED_NAME = re.compile(
    r"^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$",
    re.IGNORECASE,
)


def verify_windows_checkout_paths() -> None:
    listed = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
    )
    paths = [path.decode("utf-8", errors="surrogateescape") for path in listed.split(b"\0") if path]
    invalid: list[str] = []
    folded: dict[str, str] = {}

    for path in paths:
        for component in path.split("/"):
            if (
                any(char in _WINDOWS_INVALID_CHARS or ord(char) < 32 for char in component)
                or component.endswith((" ", "."))
                or _WINDOWS_RESERVED_NAME.match(component)
            ):
                invalid.append(path)
                break

        key = path.casefold()
        previous = folded.get(key)
        if previous is not None and previous != path:
            invalid.extend([previous, path])
        else:
            folded[key] = path

    if invalid:
        for path in sorted(set(invalid)):
            print(f"[error] Windows-incompatible release path: {path}")
        raise SystemExit(1)

    print(f"[ok] {len(paths)} release paths are compatible with Windows checkout")


def verify_release_tag(expected: str, allow_prerelease: bool = False) -> None:
    expected_tag = f"v{expected}"
    ref_type = os.environ.get("GITHUB_REF_TYPE")
    ref_name = os.environ.get("GITHUB_REF_NAME")
    tag_ok = ref_name is not None and (
        ref_name == expected_tag
        or (allow_prerelease and ref_name.startswith(f"{expected_tag}-"))
    )
    if ref_type != "tag" or not tag_ok:
        hint = f" or a {expected_tag}-<prerelease> tag" if allow_prerelease else ""
        raise SystemExit(
            f"[error] Release workflow requires tag {expected_tag}{hint}; "
            f"received {ref_type or '<unset>'} {ref_name or '<unset>'}"
        )
    print(f"[ok] release workflow ref matches {ref_name}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate MCPStore release inputs")
    parser.add_argument(
        "--require-tag",
        action="store_true",
        help="require GITHUB_REF_TYPE/GITHUB_REF_NAME to match the canonical version tag",
    )
    parser.add_argument(
        "--allow-prerelease-tag",
        action="store_true",
        help="also accept <canonical>-<prerelease> tags (build workflows only; publish stays exact)",
    )
    args = parser.parse_args()

    expected = verify_release_metadata()
    verify_windows_checkout_paths()
    if args.require_tag:
        verify_release_tag(expected, allow_prerelease=args.allow_prerelease_tag)
    print(f"[ok] release metadata is consistent at version {expected}")


if __name__ == "__main__":
    main()
