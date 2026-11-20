#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_version import sync_app, sync_sdk


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Synchronize release versions for one domain: sdk (crate+PyPI) or app (CLI/desktop/web/npm)",
    )
    parser.add_argument("domain", choices=("sdk", "app"), help="which version domain to write")
    parser.add_argument("version", help="Version without leading v")
    args = parser.parse_args()

    sync = sync_sdk if args.domain == "sdk" else sync_app
    changed = sync(args.version)
    if changed:
        print(f"[ok] synchronized {args.domain} version {args.version.lstrip('v')} in {len(changed)} file(s):")
        for path in changed:
            print(f"  - {path}")
    else:
        print(f"[ok] {args.domain} version {args.version.lstrip('v')} already synchronized")


if __name__ == "__main__":
    main()
