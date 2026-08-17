#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_version import sync_version


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Synchronize MCPStore release versions across Rust, Python, npm, desktop, and web",
    )
    parser.add_argument("version", help="Version without leading v")
    args = parser.parse_args()

    changed = sync_version(args.version)
    if changed:
        print(f"[ok] synchronized version {args.version.lstrip('v')} in {len(changed)} file(s):")
        for path in changed:
            print(f"  - {path}")
    else:
        print(f"[ok] version {args.version.lstrip('v')} already synchronized")


if __name__ == "__main__":
    main()
