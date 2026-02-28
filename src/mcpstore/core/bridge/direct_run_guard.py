from __future__ import annotations

import re
from pathlib import Path

PATTERN = re.compile(r"(?:\bbridge|\bself\._bridge)\.run\(")
ALLOWLIST = {
    "core/bridge/unified_executor.py",
    "core/bridge/direct_run_guard.py",
}


def _collect_violations(package_root: Path) -> list[str]:
    violations: list[str] = []
    for file_path in package_root.rglob("*.py"):
        relative = file_path.relative_to(package_root).as_posix()
        if relative in ALLOWLIST:
            continue
        lines = file_path.read_text(encoding="utf-8").splitlines()
        for index, line in enumerate(lines, start=1):
            if PATTERN.search(line):
                violations.append(f"{relative}:{index}: {line.strip()}")
    return violations


def main() -> int:
    package_root = Path(__file__).resolve().parents[2]
    violations = _collect_violations(package_root)
    if not violations:
        print("[OK] No direct bridge.run() usage found outside allowlist.")
        return 0

    print("[ERROR] Direct bridge.run() usage detected outside allowlist:")
    for item in violations:
        print(item)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
