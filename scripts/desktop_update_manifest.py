"""Build the static Tauri updater manifest from this run's desktop artifacts."""
import json
import sys
from pathlib import Path
from urllib.parse import quote

# Multiple installer types on one platform must update like-for-like.
# Keep the original macOS lookup key for already-installed clients.
FORMATS = {
    "darwin-aarch64": {"": ".app.tar.gz"},
    "windows-x86_64": {"msi": ".msi", "nsis": ".exe"},
    "linux-x86_64": {"deb": ".deb", "appimage": ".AppImage"},
    "linux-aarch64": {"deb": ".deb", "appimage": ".AppImage"},
}


def build_manifest(dist: Path, tag: str, repo_url: str) -> dict:
    platforms = {}
    names = set()
    for platform, formats in FORMATS.items():
        directory = dist / f"mcpstore-desktop-{platform}"
        for asset in directory.iterdir():
            if asset.name in names:
                raise ValueError(f"Duplicate release asset name: {asset.name}")
            names.add(asset.name)
        for installer, suffix in formats.items():
            matches = list(directory.glob(f"*{suffix}"))
            if len(matches) != 1:
                raise ValueError(f"Expected one *{suffix} in {directory}, found {len(matches)}")
            asset = matches[0]
            signature = asset.with_name(asset.name + ".sig").read_text(encoding="utf-8").strip()
            if not signature:
                raise ValueError(f"Empty updater signature for {asset}")
            key = f"{platform}-{installer}" if installer else platform
            platforms[key] = {
                "url": f"{repo_url}/releases/download/{tag}/{quote(asset.name)}",
                "signature": signature,
            }
    return {"version": tag.removeprefix("v"), "platforms": platforms}


if __name__ == "__main__":
    dist, tag, repo_url = sys.argv[1:]
    manifest = build_manifest(Path(dist), tag, repo_url)
    (Path(dist) / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
