#!/usr/bin/env python3
"""Central registry for MCPStore release versions, split into SDK and App domains.

SDK domain: canonical = [workspace.package] version in the root Cargo.toml.
    Files: root Cargo.toml, root Cargo.lock (mcpstore, mcpstore_python), and
    satellite/python (pyproject, __init__, uv.lock).
    Tags: sdk-vX.Y.Z (no prerelease).

App domain: canonical = literal version in satellite/cli/Cargo.toml.
    Files: satellite/cli (Cargo.toml + Cargo.lock), satellite/desktop/tauri
    (Cargo.toml + tauri.conf.json + Cargo.lock), satellite/web/package.json,
    satellite/npm packages.
    Tags: vX.Y.Z / vX.Y.Z-rc.N.

Cross-domain contract (enforced by app and all verification, maintained by
sync_sdk): the CLI satellite pins the SDK with a path + minor version req
"S.X", and its downstream mcpstore lock blocks (satellite/cli and
satellite/desktop/tauri Cargo.lock) must carry the current SDK version.
sync_sdk updates both, so an SDK-only release leaves the satellite
workspaces `cargo check --locked`-viable and all-domain CI green without
touching the App's own version anywhere.
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
UV_LOCK_PATH = ROOT / "satellite" / "python" / "uv.lock"
APP_CANONICAL_PATH = ROOT / "satellite" / "cli" / "Cargo.toml"
APP_LOCK_PATH = APP_CANONICAL_PATH.parent / "Cargo.lock"
TAURI_LOCK_PATH = ROOT / "satellite" / "desktop" / "tauri" / "Cargo.lock"

# The single cross-domain contract: the CLI satellite pins the SDK with
# path + version req "S.X"; its minor must track the SDK canonical minor.
CLI_SDK_REQ_PATTERN = re.compile(r'(mcpstore = \{[^}]*?version = )"?(\d+)\.(\d+)(?:\.(\d+))?"')


def _lock_block_pattern(name: str) -> re.Pattern[str]:
    return re.compile(rf'(\[\[package\]\]\nname = "{re.escape(name)}"\nversion = )"([^"]+)"')


# SDK-domain lock blocks: rewritten by sync_sdk, verified against the SDK
# version. The root workspace lock covers crates.io (--locked publish) and
# the maturin PyPI wheel; uv.lock covers uv-based dev flows.
SDK_LOCK_TARGETS: list[tuple[Path, str]] = [
    (SDK_LOCK_PATH, "mcpstore"),
    (SDK_LOCK_PATH, "mcpstore_python"),
    (UV_LOCK_PATH, "mcpstore"),
]
# App-domain mirrors of the SDK path dependency: maintained by sync_sdk
# (keep --locked builds viable across SDK bumps), verified against the SDK version.
DOWNSTREAM_SDK_LOCK_TARGETS: list[tuple[Path, str]] = [
    (APP_LOCK_PATH, "mcpstore"),
    (TAURI_LOCK_PATH, "mcpstore"),
]
# App-pinned lock blocks: rewritten by sync_app, verified against the App version.
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
    versions: dict[str, str] = {}
    for path, name in targets:
        try:
            text = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise SystemExit(f"[error] Lock file not found: {path.relative_to(ROOT)}")
        match = _lock_block_pattern(name).search(text)
        if not match:
            raise SystemExit(
                f"[error] No [[package]] block for {name} in {path.relative_to(ROOT)}"
            )
        versions[f"{path.relative_to(ROOT)} ({name})"] = match.group(2)
    return versions


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


def _read_web_lock_versions() -> dict[str, str]:
    data = json.loads(
        (ROOT / "satellite" / "web" / "package-lock.json").read_text(encoding="utf-8")
    )
    root_pkg = data.get("packages", {}).get("")
    if not isinstance(root_pkg, dict):
        raise SystemExit("[error] satellite/web/package-lock.json has no packages[\"\"] entry")
    return {
        "web package-lock": data.get("version", ""),
        "web package-lock root pkg": root_pkg.get("version", ""),
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
        **_read_web_lock_versions(),
        **read_lock_versions(APP_LOCK_TARGETS),
    }

    for package_path in _npm_package_paths():
        package = json.loads(package_path.read_text(encoding="utf-8"))
        versions[f"npm {package['name']}"] = package["version"]

    return versions


def _validated(version: str) -> str:
    version = version.lstrip("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version):
        raise SystemExit(f"[error] Invalid version {version!r}: expected X.Y.Z")
    return version


def sync_sdk(version: str) -> list[str]:
    """Write the SDK version into SDK-domain files and maintain the
    cross-domain contract: refresh the CLI's SDK version req minor and the
    satellite mirrors of the SDK lock blocks, so an SDK-only release keeps
    `cargo check --locked` viable in satellite/cli and satellite/desktop/tauri.
    The App's own version (cli/tauri/npm/web) is never touched here."""
    version = _validated(version)
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

    # Cross-domain contract: track the new SDK minor in the CLI req and
    # re-lock the satellite mirrors of the SDK path dependency.
    req_minor = f"{version.split('.')[0]}.{version.split('.')[1]}"
    req_text = APP_CANONICAL_PATH.read_text(encoding="utf-8")
    req_updated = CLI_SDK_REQ_PATTERN.sub(lambda m: f'{m.group(1)}"{req_minor}"', req_text, count=1)
    if req_updated != req_text:
        APP_CANONICAL_PATH.write_text(req_updated, encoding="utf-8")
        changed.append(f"{APP_CANONICAL_PATH.relative_to(ROOT)} (sdk version req)")

    changed.extend(_sync_lock_blocks(DOWNSTREAM_SDK_LOCK_TARGETS, version))
    return changed


def sync_app(version: str) -> list[str]:
    """Write the App version into App-domain files only. The cross-domain
    contract is owned by sync_sdk; this never reads or writes SDK files."""
    version = _validated(version)
    changed: list[str] = []

    text_targets = [
        (APP_CANONICAL_PATH, CARGO_VERSION_PATTERN, f'version = "{version}"'),
        (ROOT / "satellite" / "desktop" / "tauri" / "Cargo.toml", CARGO_VERSION_PATTERN, f'version = "{version}"'),
    ]
    for path, pattern, replacement in text_targets:
        if _write_regex_version(path, pattern, replacement):
            changed.append(str(path.relative_to(ROOT)))

    changed.extend(_sync_lock_blocks(APP_LOCK_TARGETS, version))

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

    # npm lock mirrors of the web package version (root + packages[""]).
    web_lock_path = ROOT / "satellite" / "web" / "package-lock.json"
    web_lock = json.loads(web_lock_path.read_text(encoding="utf-8"))
    web_lock_updated = False
    if web_lock.get("version") != version:
        web_lock["version"] = version
        web_lock_updated = True
    root_pkg = web_lock.get("packages", {}).get("")
    if isinstance(root_pkg, dict) and root_pkg.get("version") != version:
        root_pkg["version"] = version
        web_lock_updated = True
    if web_lock_updated and _write_json_file(web_lock_path, web_lock):
        changed.append(str(web_lock_path.relative_to(ROOT)))

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


def verify_sdk() -> str:
    sdk = read_sdk_version()
    versions = _collect_sdk_versions()
    mismatches = {name: value for name, value in versions.items() if value != sdk}
    if mismatches:
        for name, value in mismatches.items():
            print(f"[error] {name} version {value} != SDK version {sdk}")
        raise SystemExit(1)
    return sdk


def verify_app() -> str:
    app = read_app_version()
    sdk = read_sdk_version()

    req_minor = read_cli_sdk_req_minor()
    if req_minor != int(sdk.split(".")[1]):
        raise SystemExit(
            f"[error] CLI SDK version req minor {req_minor} != SDK canonical minor {sdk.split('.')[1]} "
            "(run scripts/sync_version.py sdk <version> to refresh the cross-domain contract)"
        )

    versions = _collect_app_versions()
    mismatches = {name: value for name, value in versions.items() if value != app}
    if mismatches:
        for name, value in mismatches.items():
            print(f"[error] {name} version {value} != App version {app}")
        raise SystemExit(1)

    stale = {
        name: value
        for name, value in read_lock_versions(DOWNSTREAM_SDK_LOCK_TARGETS).items()
        if value != sdk
    }
    if stale:
        for name, value in stale.items():
            print(f"[error] {name} version {value} != SDK version {sdk}")
        raise SystemExit(
            "[error] Satellite SDK lock blocks are stale; "
            "run scripts/sync_version.py sdk <version> to re-lock against the current SDK"
        )
    return app


def verify_release_metadata(domain: str = "all") -> dict[str, str]:
    """Verify the requested domain's version files agree; return per-domain versions."""
    if domain == "sdk":
        return {"sdk": verify_sdk()}
    if domain == "app":
        result = {"app": verify_app()}
    elif domain == "all":
        result = {"sdk": verify_sdk(), "app": verify_app()}
    else:
        raise SystemExit(f"[error] Unknown domain: {domain}")

    # platforms.toml <-> npm packages cross-check: App release inputs.
    platforms = tomllib.loads(
        (ROOT / "satellite" / "platforms.toml").read_text(encoding="utf-8")
    )["platforms"]
    npm_packages = {
        json.loads(path.read_text(encoding="utf-8"))["name"] for path in _npm_package_paths()
    }
    for platform in platforms:
        if platform["npm_package"] not in npm_packages:
            raise SystemExit(f"[error] Missing npm package: {platform['npm_package']}")
    return result
