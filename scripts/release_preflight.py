#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_version_from_text(path: Path, pattern: str) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        raise SystemExit(f"[error] Version not found in {path.relative_to(ROOT)}")
    return match.group(1)


def main() -> None:
    workspace_version = read_version_from_text(
        ROOT / "rust" / "Cargo.toml",
        r'^version\s*=\s*"([^"]+)"',
    )
    python_version = read_version_from_text(
        ROOT / "python" / "pyproject.toml",
        r'^version\s*=\s*"([^"]+)"',
    )
    init_version = read_version_from_text(
        ROOT / "python" / "src" / "mcpstore" / "__init__.py",
        r'^__version__\s*=\s*"([^"]+)"',
    )

    versions = {
        "rust workspace": workspace_version,
        "python pyproject": python_version,
        "python __init__": init_version,
    }

    package_paths = sorted((ROOT / "npm" / "packages").glob("*/package.json"))
    for package_path in package_paths:
        package = json.loads(package_path.read_text(encoding="utf-8"))
        versions[f"npm {package['name']}"] = package["version"]

    expected = workspace_version
    mismatches = {name: version for name, version in versions.items() if version != expected}
    if mismatches:
        for name, version in mismatches.items():
            print(f"[error] {name} version {version} != {expected}")
        raise SystemExit(1)

    platforms = tomllib.loads((ROOT / "platforms.toml").read_text(encoding="utf-8"))["platforms"]
    npm_packages = {json.loads(path.read_text(encoding="utf-8"))["name"] for path in package_paths}
    for platform in platforms:
        if platform["npm_package"] not in npm_packages:
            raise SystemExit(f"[error] Missing npm package: {platform['npm_package']}")

    print(f"[ok] release metadata is consistent at version {expected}")


if __name__ == "__main__":
    main()
