#!/usr/bin/env python3
"""Central registry for MCPStore release versions, split into SDK and App domains.

SDK domain: canonical = [workspace.package] version in the root Cargo.toml.
    Files: root Cargo.toml, satellite/python (pyproject + __init__), and the
    mcpstore / mcpstore_python blocks of every downstream Cargo.lock.
    Tags: sdk-vX.Y.Z (no prerelease).

App domain: canonical = literal version in satellite/cli/Cargo.toml.
    Files: satellite/cli (Cargo.toml + Cargo.lock), satellite/desktop/tauri
    (Cargo.toml + tauri.conf.json + Cargo.lock), satellite/web/package.json,
    satellite/npm packages.
    Tags: vX.Y.Z / vX.Y.Z-rc.N.
"""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CARGO_VERSION_PATTERN = r'^version\s*=\s*"([^"]+)"'
PYTHON_VERSION_PATTERN = r'^__version__\s*=\s*"([^"]+)"'

SDK_CANONICAL_PATH = ROOT / "Cargo.toml"
SDK_LOCK_PATH = ROOT / "Cargo.lock"
APP_CANONICAL_PATH = ROOT / "satellite" / "cli" / "Cargo.toml"
APP_LOCK_PATH = APP_CANONICAL_PATH.parent / "Cargo.lock"
TAURI_LOCK_PATH = ROOT / "satellite" / "desktop" / "tauri" / "Cargo.lock"

# The single cross-domain contract: the CLI satellite pins the SDK with
# path + version req "S.X"; its minor must track the SDK canonical minor.
CLI_SDK_REQ_PATTERN = re.compile(r'(mcpstore = \{[^}]*?version = )"?(\d+)\.(\d+)(?:\.(\d+))?"')


def _lock_block_pattern(name: str) -> re.Pattern[str]:
    return re.compile(rf'(\[\[package\]\]\nname = "{re.escape(name)}"\nversion = )"([^"]+)"')


# Path dependencies keep lock entries: SDK bumps touch every downstream
# mcpstore block; App bumps touch every mcpstore-cli block.
SDK_LOCK_TARGETS: list[tuple[Path, str]] = [
    (SDK_LOCK_PATH, "mcpstore"),
    (SDK_LOCK_PATH, "mcpstore_python"),
    (APP_LOCK_PATH, "mcpstore"),
    (TAURI_LOCK_PATH, "mcpstore"),
]
APP_LOCK_TARGETS: list[tuple[Path, str]] = [
    (APP_LOCK_PATH, "mcpstore-cli"),
    (TAURI_LOCK_PATH, "mcpstore-cli"),
]


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


def _sync_lock_blocks(targets: list[tuple[Path, str]], version: str) -> list[str]:
    changed: list[str] = []
    for path, name in targets:
        text = path.read_text(encoding="utf-8")
        updated = _lock_block_pattern(name).sub(lambda m: f'{m.group(1)}"{version}"', text)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
            changed.append(f"{path.relative_to(ROOT)} ({name})")
    return changed


def read_lock_versions(targets: list[tuple[Path, str]]) -> dict[str, str]:
    return {
        f"{path.relative_to(ROOT)} ({name})": _lock_block_pattern(name).search(
            path.read_text(encoding="utf-8")
        ).group(2)
        for path, name in targets
    }


def read_sdk_version() -> str:
    return _read_regex_version(SDK_CANONICAL_PATH, CARGO_VERSION_PATTERN)


def read_app_version() -> str:
    return _read_regex_version(APP_CANONICAL_PATH, CARGO_VERSION_PATTERN)


def read_cli_sdk_req_minor() -> int:
    text = APP_CANONICAL_PATH.read_text(encoding="utf-8")
    match = CLI_SDK_REQ_PATTERN.search(text)
    if not match:
        raise SystemExit(
            "[error] CLI satellite has no version requirement on the SDK crate "
            f'(mcpstore = {{ path = "../..", version = "S.X", ... }}) in {APP_CANONICAL_PATH.relative_to(ROOT)}'
        )
    return int(match.group(3))


def _npm_package_paths() -> list[Path]:
    return sorted((ROOT / "satellite" / "npm" / "packages").glob("*/package.json"))


def _collect_sdk_versions() -> dict[str, str]:
    return {
        "sdk canonical (root Cargo.toml)": read_sdk_version(),
        "python pyproject": _read_regex_version(
            ROOT / "satellite" / "python" / "pyproject.toml",
            CARGO_VERSION_PATTERN,
        ),
        "python __init__": _read_regex_version(
            ROOT / "satellite" / "python" / "mcpstore" / "__init__.py",
            PYTHON_VERSION_PATTERN,
        ),
        **read_lock_versions(SDK_LOCK_TARGETS),
    }


def _collect_app_versions() -> dict[str, str]:
    versions = {
        "app canonical (cli Cargo.toml)": read_app_version(),
        "desktop tauri Cargo.toml": _read_regex_version(
            ROOT / "satellite" / "desktop" / "tauri" / "Cargo.toml",
            CARGO_VERSION_PATTERN,
        ),
        "desktop tauri.conf.json": _read_json_version(
            ROOT / "satellite" / "desktop" / "tauri" / "tauri.conf.json",
        ),
        "web package.json": _read_json_version(ROOT / "satellite" / "web" / "package.json"),
        **read_lock_versions(APP_LOCK_TARGETS),
    }

    for package_path in _npm_package_paths():
        package = json.loads(package_path.read_text(encoding="utf-8"))
        versions[f"npm {package['name']}"] = package["version"]

    return versions


def sync_sdk(version: str) -> list[str]:
    version = version.lstrip("v")
    changed: list[str] = []

    text_targets = [
        (SDK_CANONICAL_PATH, CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (ROOT / "satellite" / "python" / "pyproject.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (
            ROOT / "satellite" / "python" / "mcpstore" / "__init__.py",
            PYTHON_VERSION_PATTERN,
            f'__version__ = "{version}"',
        ),
    ]
    for path, pattern, replacement in text_targets:
        if _write_regex_version(path, pattern, replacement):
            changed.append(str(path.relative_to(ROOT)))

    changed.extend(_sync_lock_blocks(SDK_LOCK_TARGETS, version))
    return changed


def sync_app(version: str) -> list[str]:
    version = version.lstrip("v")
    changed: list[str] = []

    text_targets = [
        (APP_CANONICAL_PATH, CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (ROOT / "satellite" / "desktop" / "tauri" / "Cargo.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
    ]
    for path, pattern, replacement in text_targets:
        if _write_regex_version(path, pattern, replacement):
            changed.append(str(path.relative_to(ROOT)))

    changed.extend(_sync_lock_blocks(APP_LOCK_TARGETS, version))

    # Track the current SDK minor in the cross-domain contract.
    sdk_version = read_sdk_version()
    req_minor = f"{sdk_version.split('.')[0]}.{sdk_version.split('.')[1]}"
    req_text = APP_CANONICAL_PATH.read_text(encoding="utf-8")
    req_updated = CLI_SDK_REQ_PATTERN.sub(lambda m: f'{m.group(1)}"{req_minor}"', req_text, count=1)
    if req_updated != req_text:
        APP_CANONICAL_PATH.write_text(req_updated, encoding="utf-8")
        changed.append(f"{APP_CANONICAL_PATH.relative_to(ROOT)} (sdk version req)")

    json_targets = [
        ROOT / "satellite" / "desktop" / "tauri" / "tauri.conf.json",
        ROOT / "satellite" / "web" / "package.json",
    ]
    for path in json_targets:
        data = json.loads(path.read_text(encoding="utf-8"))
        if data.get("version") == version:
            continue
        data["version"] = version
        if _write_json_file(path, data):
            changed.append(str(path.relative_to(ROOT)))

    main_package = ROOT / "satellite" / "npm" / "packages" / "mcpstore" / "package.json"
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

    for package_path in sorted((ROOT / "satellite" / "npm" / "packages").glob("mcpstore-bin-*/package.json")):
        data = json.loads(package_path.read_text(encoding="utf-8"))
        if data.get("version") == version:
            continue
        data["version"] = version
        if _write_json_file(package_path, data):
            changed.append(str(package_path.relative_to(ROOT)))

    return changed


def _verify_domain_versions(domain: str) -> str:
    if domain == "sdk":
        expected = read_sdk_version()
        versions = _collect_sdk_versions()
    elif domain == "app":
        expected = read_app_version()
        versions = _collect_app_versions()
    else:
        sdk = read_sdk_version()
        req_minor = read_cli_sdk_req_minor()
        if req_minor != int(sdk.split(".")[1]):
            raise SystemExit(
                f"[error] CLI satellite SDK version req minor {req_minor} != SDK canonical minor {sdk.split('.')[1]}"
            )
        expected = sdk
        versions = {**_collect_sdk_versions(), **_collect_app_versions()}

    mismatches = {name: value for name, value in versions.items() if value != expected}
    if mismatches:
        for name, value in mismatches.items():
            print(f"[error] {name} version {value} != {expected}")
        raise SystemExit(1)
    return expected


def verify_release_metadata(domain: str = "all") -> str:
    """Verify the requested domain's version files agree; return the version to tag."""
    version = _verify_domain_versions(domain)
    if domain != "app":
        # platforms.toml ↔ npm packages cross-check belongs to neither single domain.
        platforms = tomllib.loads(
            (ROOT / "satellite" / "platforms.toml").read_text(encoding="utf-8")
        )["platforms"]
        npm_packages = {
            json.loads(path.read_text(encoding="utf-8"))["name"] for path in _npm_package_paths()
        }
        for platform in platforms:
            if platform["npm_package"] not in npm_packages:
                raise SystemExit(f"[error] Missing npm package: {platform['npm_package']}")
    return version
