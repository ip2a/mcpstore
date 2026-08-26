#!/usr/bin/env python3
"""Central registry for MCPStore release version read/write."""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CANONICAL_PATH = ROOT / "rust" / "Cargo.toml"
CARGO_VERSION_PATTERN = r'^version\s*=\s*"([^"]+)"'
PYTHON_VERSION_PATTERN = r'^__version__\s*=\s*"([^"]+)"'
LOCK_PATH = ROOT / "rust" / "Cargo.lock"
# Cargo.lock package blocks for workspace crates (mcpstore, mcpstore-cli, mcpstore_python, ...).
# Wheels and crates publish with --locked, so a stale lock version fails the release build.
LOCK_PACKAGE_PATTERN = re.compile(r'(\[\[package\]\]\nname = "(mcpstore[^"]*)"\nversion = )"([^"]+)"')


def _read_regex_version(path: Path, pattern: str) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        raise SystemExit(f"[error] Version not found in {path.relative_to(ROOT)}")
    return match.group(1)


def _write_regex_version(path: Path, pattern: str, replacement: str) -> bool:
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"[error] Could not update version in {path.relative_to(ROOT)}")
    if updated == text:
        return False
    path.write_text(updated, encoding="utf-8")
    return True


def _read_json_version(path: Path) -> str:
    data = json.loads(path.read_text(encoding="utf-8"))
    version = data.get("version")
    if not isinstance(version, str) or not version:
        raise SystemExit(f"[error] JSON version not found in {path.relative_to(ROOT)}")
    return version


def _write_json_file(path: Path, data: dict) -> bool:
    original = path.read_text(encoding="utf-8")
    updated = json.dumps(data, indent=2, ensure_ascii=True) + "\n"
    if updated == original:
        return False
    path.write_text(updated, encoding="utf-8")
    return True


def _write_lock_versions(version: str) -> bool:
    text = LOCK_PATH.read_text(encoding="utf-8")
    updated, _ = LOCK_PACKAGE_PATTERN.subn(lambda m: f'{m.group(1)}"{version}"', text)
    if updated == text:
        return False
    LOCK_PATH.write_text(updated, encoding="utf-8")
    return True


def read_lock_versions() -> dict[str, str]:
    return {
        match.group(2): match.group(3)
        for match in LOCK_PACKAGE_PATTERN.finditer(LOCK_PATH.read_text(encoding="utf-8"))
    }


def read_canonical_version() -> str:
    return _read_regex_version(CANONICAL_PATH, CARGO_VERSION_PATTERN)


def _npm_package_paths() -> list[Path]:
    return sorted((ROOT / "npm" / "packages").glob("*/package.json"))


def collect_versions() -> dict[str, str]:
    versions = {
        "rust workspace": read_canonical_version(),
        "python pyproject": _read_regex_version(
            ROOT / "python" / "pyproject.toml",
            CARGO_VERSION_PATTERN,
        ),
        "python __init__": _read_regex_version(
            ROOT / "python" / "src" / "mcpstore" / "__init__.py",
            PYTHON_VERSION_PATTERN,
        ),
        "desktop tauri Cargo.toml": _read_regex_version(
            ROOT / "desktop" / "tauri" / "Cargo.toml",
            CARGO_VERSION_PATTERN,
        ),
        "desktop tauri.conf.json": _read_json_version(
            ROOT / "desktop" / "tauri" / "tauri.conf.json",
        ),
        "web package.json": _read_json_version(ROOT / "web" / "package.json"),
    }

    for package_path in _npm_package_paths():
        package = json.loads(package_path.read_text(encoding="utf-8"))
        versions[f"npm {package['name']}"] = package["version"]

    return versions


def sync_version(version: str) -> list[str]:
    version = version.lstrip("v")
    changed: list[str] = []

    text_targets = [
        (ROOT / "rust" / "Cargo.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (ROOT / "python" / "pyproject.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (
            ROOT / "python" / "src" / "mcpstore" / "__init__.py",
            PYTHON_VERSION_PATTERN,
            f'__version__ = "{version}"',
        ),
        (ROOT / "desktop" / "tauri" / "Cargo.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
    ]
    for path, pattern, replacement in text_targets:
        if _write_regex_version(path, pattern, replacement):
            changed.append(str(path.relative_to(ROOT)))

    if _write_lock_versions(version):
        changed.append(str(LOCK_PATH.relative_to(ROOT)))

    json_targets = [
        ROOT / "desktop" / "tauri" / "tauri.conf.json",
        ROOT / "web" / "package.json",
    ]
    for path in json_targets:
        data = json.loads(path.read_text(encoding="utf-8"))
        if data.get("version") == version:
            continue
        data["version"] = version
        if _write_json_file(path, data):
            changed.append(str(path.relative_to(ROOT)))

    main_package = ROOT / "npm" / "packages" / "mcpstore" / "package.json"
    main_data = json.loads(main_package.read_text(encoding="utf-8"))
    main_updated = False
    if main_data.get("version") != version:
        main_data["version"] = version
        main_updated = True
    for dep in main_data.get("optionalDependencies", {}):
        if main_data["optionalDependencies"][dep] != version:
            main_data["optionalDependencies"][dep] = version
            main_updated = True
    if main_updated and _write_json_file(main_package, main_data):
        changed.append(str(main_package.relative_to(ROOT)))

    for package_path in sorted((ROOT / "npm" / "packages").glob("mcpstore-bin-*/package.json")):
        data = json.loads(package_path.read_text(encoding="utf-8"))
        if data.get("version") == version:
            continue
        data["version"] = version
        if _write_json_file(package_path, data):
            changed.append(str(package_path.relative_to(ROOT)))

    return changed


def verify_release_metadata() -> str:
    expected = read_canonical_version()
    versions = collect_versions()
    versions.update({f"Cargo.lock {name}": value for name, value in read_lock_versions().items()})
    mismatches = {name: value for name, value in versions.items() if value != expected}
    if mismatches:
        for name, value in mismatches.items():
            print(f"[error] {name} version {value} != {expected}")
        raise SystemExit(1)

    platforms = tomllib.loads((ROOT / "platforms.toml").read_text(encoding="utf-8"))["platforms"]
    npm_packages = {
        json.loads(path.read_text(encoding="utf-8"))["name"] for path in _npm_package_paths()
    }
    for platform in platforms:
        if platform["npm_package"] not in npm_packages:
            raise SystemExit(f"[error] Missing npm package: {platform['npm_package']}")

    return expected
