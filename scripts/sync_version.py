#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_version(path: Path, pattern: str, replacement: str) -> None:
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"[error] Could not update version in {path.relative_to(ROOT)}")
    path.write_text(updated, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Synchronize MCPStore release versions")
    parser.add_argument("version", help="Version without leading v")
    args = parser.parse_args()
    version = args.version.lstrip("v")

    replace_version(ROOT / "rust" / "Cargo.toml", r'^version\s*=\s*"[^"]+"', f'version = "{version}"')
    replace_version(ROOT / "python" / "pyproject.toml", r'^version\s*=\s*"[^"]+"', f'version = "{version}"')
    replace_version(
        ROOT / "python" / "src" / "mcpstore" / "__init__.py",
        r'^__version__\s*=\s*"[^"]+"',
        f'__version__ = "{version}"',
    )

    main_package = ROOT / "npm" / "packages" / "mcpstore" / "package.json"
    main_data = json.loads(main_package.read_text(encoding="utf-8"))
    main_data["version"] = version
    for dep in main_data.get("optionalDependencies", {}):
        main_data["optionalDependencies"][dep] = version
    main_package.write_text(json.dumps(main_data, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")

    for package_path in sorted((ROOT / "npm" / "packages").glob("mcpstore-bin-*/package.json")):
        data = json.loads(package_path.read_text(encoding="utf-8"))
        data["version"] = version
        package_path.write_text(json.dumps(data, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")

    print(f"[ok] synchronized version {version}")


if __name__ == "__main__":
    main()
