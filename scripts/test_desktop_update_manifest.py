from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase, main

from desktop_update_manifest import build_manifest


class DesktopUpdateTest(TestCase):
    def test_installers_update_like_for_like(self):
        files = {
            "darwin-aarch64": "mcpstore.app.tar.gz",
            "windows-x86_64-msi": "mcpstore_x64_en-US.msi",
            "windows-x86_64-nsis": "mcpstore_x64-setup.exe",
            "linux-x86_64-deb": "mcpstore_amd64.deb",
            "linux-x86_64-appimage": "mcpstore_amd64.AppImage",
            "linux-aarch64-deb": "mcpstore_arm64.deb",
            "linux-aarch64-appimage": "mcpstore_aarch64.AppImage",
        }
        repo = "https://example.org/repo"
        with TemporaryDirectory() as temp:
            dist = Path(temp)
            expected = {}
            for key, name in files.items():
                platform = "-".join(key.split("-")[:2])
                directory = dist / f"mcpstore-desktop-{platform}"
                directory.mkdir(exist_ok=True)
                (directory / name).touch()
                (directory / f"{name}.sig").write_text(f"{key}\n")
                expected[key] = {"url": f"{repo}/releases/download/v1.2.3/{name}", "signature": key}
            self.assertEqual(build_manifest(dist, "v1.2.3", repo), {"version": "1.2.3", "platforms": expected})
            (dist / "mcpstore-desktop-windows-x86_64/mcpstore_x64-setup.exe.sig").unlink()
            with self.assertRaises(FileNotFoundError):
                build_manifest(dist, "v1.2.3", repo)


if __name__ == "__main__":
    main()
