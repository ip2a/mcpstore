#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_version import verify_release_metadata


def main() -> None:
    expected = verify_release_metadata()
    print(f"[ok] release metadata is consistent at version {expected}")


if __name__ == "__main__":
    main()
