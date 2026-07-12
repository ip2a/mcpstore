#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_platforms() -> list[dict[str, str]]:
    data = tomllib.loads((ROOT / "platforms.toml").read_text(encoding="utf-8"))
    return data["platforms"]


def copy_binary(dist_root: Path, platform: dict[str, str]) -> None:
    source = dist_root / platform["id"] / platform["artifact_binary"]
    if not source.exists():
        raise FileNotFoundError(f"Platform artifact not found: {source}")

    target_dir = ROOT / "npm" / "packages" / platform["npm_dir"] / "bin"
    target_dir.mkdir(parents=True, exist_ok=True)
    target = target_dir / platform["artifact_binary"]

    shutil.copy2(source, target)
    if target.suffix != ".exe":
        target.chmod(0o755)

    print(f"[ok] wrote {target.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Assemble dist artifacts into npm platform packages")
    parser.add_argument("--dist-root", default="dist", help="dist artifacts root directory")
    args = parser.parse_args()

    dist_root = (ROOT / args.dist_root).resolve()
    if not dist_root.exists():
        raise FileNotFoundError(f"dist directory does not exist: {dist_root}")

    for platform in load_platforms():
        copy_binary(dist_root, platform)


if __name__ == "__main__":
    main()
