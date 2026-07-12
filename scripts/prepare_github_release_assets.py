#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import re
import tarfile
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_workspace_version() -> str:
    cargo_toml = (ROOT / "rust" / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml, flags=re.MULTILINE)
    if not match:
        raise SystemExit("[error] rust/Cargo.toml workspace version is missing")
    return match.group(1)


def load_platform_config() -> tuple[dict[str, str], list[dict[str, str]]]:
    data = tomllib.loads((ROOT / "platforms.toml").read_text(encoding="utf-8"))
    return data["meta"], data["platforms"]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def display_path(path: Path) -> Path:
    try:
        return path.relative_to(ROOT)
    except ValueError:
        return path


def add_common_files_tar(archive: tarfile.TarFile) -> None:
    for name in ["README.md", "LICENSE"]:
        path = ROOT / name
        if path.exists():
            archive.add(path, arcname=name)


def add_common_files_zip(archive: zipfile.ZipFile) -> None:
    for name in ["README.md", "LICENSE"]:
        path = ROOT / name
        if path.exists():
            archive.write(path, arcname=name)


def add_executable_tar(archive: tarfile.TarFile, source: Path, arcname: str) -> None:
    info = archive.gettarinfo(str(source), arcname=arcname)
    info.mode = 0o755
    with source.open("rb") as file:
        archive.addfile(info, file)


def write_platform_archive(
    dist_root: Path,
    assets_dir: Path,
    release_name: str,
    version: str,
    platform: dict[str, str],
) -> Path:
    platform_id = platform["id"]
    binary_name = platform["artifact_binary"]
    source = dist_root / platform_id / binary_name
    checksum = dist_root / platform_id / "SHA256SUMS"
    if not source.exists():
        raise FileNotFoundError(f"Platform binary not found: {source}")
    if not checksum.exists():
        raise FileNotFoundError(f"Platform checksum not found: {checksum}")

    base_name = f"{release_name}-v{version}-{platform_id}"
    if platform_id.startswith("win32-"):
        archive_path = assets_dir / f"{base_name}.zip"
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.write(source, arcname=binary_name)
            archive.write(checksum, arcname="SHA256SUMS")
            add_common_files_zip(archive)
    else:
        archive_path = assets_dir / f"{base_name}.tar.gz"
        with tarfile.open(archive_path, "w:gz") as archive:
            add_executable_tar(archive, source, binary_name)
            archive.add(checksum, arcname="SHA256SUMS")
            add_common_files_tar(archive)

    print(f"[ok] wrote {display_path(archive_path)}")
    return archive_path


def main() -> None:
    parser = argparse.ArgumentParser(description="Prepare GitHub Release archives from dist artifacts")
    parser.add_argument("--dist-root", default="dist", help="dist artifacts root directory")
    parser.add_argument("--assets-dir", default="release-assets", help="output release assets directory")
    args = parser.parse_args()

    dist_root = (ROOT / args.dist_root).resolve()
    assets_dir = (ROOT / args.assets_dir).resolve()
    assets_dir.mkdir(parents=True, exist_ok=True)

    meta, platforms = load_platform_config()
    release_name = meta.get("binary_name", "mcpstore")
    version = read_workspace_version()
    archives = [
        write_platform_archive(dist_root, assets_dir, release_name, version, platform)
        for platform in platforms
    ]

    checksum_path = assets_dir / "SHA256SUMS.txt"
    checksum_lines = [f"{sha256(path)}  {path.name}\n" for path in archives]
    checksum_path.write_text("".join(checksum_lines), encoding="utf-8")
    print(f"[ok] wrote {display_path(checksum_path)}")


if __name__ == "__main__":
    main()
