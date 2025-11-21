"""
Safe emoji remover for Python source files.

Features:
- Recursively scans target directory for .py files (configurable)
- Dry-run by default: reports files and counts without modifying anything
- Optional apply mode with per-file backups in a timestamped folder
- Preserves UTF-8 BOM if present and preserves original newline style
- Skips undecodable files to avoid corruption
- Uses the `emoji` library if available for accurate removal; falls back to
  comprehensive Unicode range regex otherwise

Usage examples:
  Dry run from current directory:
    python src/mcpstore/scripts/remove_emojis.py --root .

  Apply changes with backups to default backup dir:
    python src/mcpstore/scripts/remove_emojis.py --root . --apply

  Customize ignore directories and add extensions:
    python src/mcpstore/scripts/remove_emojis.py --root . --apply \
      --ignore-dirs .git .venv venv node_modules __pycache__ build dist \
      --ext .py
"""

from __future__ import annotations

import argparse
import datetime as _dt
import os
import re
import sys
from pathlib import Path
from typing import Iterable, List, Optional, Sequence, Tuple


def try_import_emoji():
    try:
        import emoji  # type: ignore
        return emoji
    except Exception:
        return None


def build_fallback_emoji_regex(aggressive: bool) -> re.Pattern[str]:
    """Return a compiled regex that matches most emoji code points.

    This is a best-effort fallback when the `emoji` library is unavailable.
    It covers common Unicode emoji blocks and optionally removes common
    sequence modifiers (ZWJ, variation selectors, skin tones) when aggressive.
    """

    # Core emoji ranges
    ranges = [
        "\U0001F300-\U0001F5FF",  # Misc Symbols and Pictographs
        "\U0001F600-\U0001F64F",  # Emoticons
        "\U0001F680-\U0001F6FF",  # Transport and Map
        "\U0001F700-\U0001F77F",  # Alchemical Symbols
        "\U0001F780-\U0001F7FF",  # Geometric Shapes Extended
        "\U0001F800-\U0001F8FF",  # Supplemental Arrows-C
        "\U0001F900-\U0001F9FF",  # Supplemental Symbols and Pictographs
        "\U0001FA00-\U0001FA6F",  # Chess Symbols, etc.
        "\U0001FA70-\U0001FAFF",  # Symbols and Pictographs Extended-A
        "\U00002702-\U000027B0",  # Dingbats (partial)
        "\U000024C2-\U0001F251",  # Enclosed characters
        "\U00002600-\U000026FF",  # Misc symbols
        "\U0001F1E6-\U0001F1FF",  # Regional indicator symbols (flags)
    ]

    # Build base class
    base_class = "[" + "".join(ranges) + "]"

    # Additional single code points often involved in emoji sequences
    singles = ["\u200D", "\uFE0E", "\uFE0F"]  # ZWJ, text/emoji variation selectors
    # Fitzpatrick skin tone modifiers
    if aggressive:
        ranges_extra = ["\U0001F3FB-\U0001F3FF"]
    else:
        ranges_extra = []

    extra_class = "[" + "".join(ranges_extra) + "]" if ranges_extra else None

    parts = [base_class]
    parts.extend(map(re.escape, singles))
    if extra_class:
        parts.append(extra_class)

    pattern = "|".join(parts)
    return re.compile(pattern)


def remove_emojis_from_text(
    text: str,
    emoji_mod: Optional[object],
    aggressive: bool,
) -> Tuple[str, int]:
    """Remove emoji-like characters from text.

    Returns (cleaned_text, removed_count).
    """
    removed_count = 0

    if emoji_mod is not None:
        # Prefer the library if available for accuracy across Unicode versions
        try:
            # emoji>=2.0
            replaced = emoji_mod.replace_emoji(text, replace="")  # type: ignore[attr-defined]
        except Exception:
            # Older API
            try:
                replaced = emoji_mod.replace_emoji(text, "")  # type: ignore[misc]
            except Exception:
                replaced = text

        # Remove common sequence joiners/selectors if aggressive
        if aggressive:
            replaced2 = re.sub("[\u200D\uFE0E\uFE0F\U0001F3FB-\U0001F3FF]", "", replaced)
        else:
            replaced2 = replaced

        removed_count = len(text) - len(replaced2)
        return replaced2, removed_count

    # Fallback regex
    pattern = build_fallback_emoji_regex(aggressive=aggressive)

    def _sub_func(match: re.Match[str]) -> str:
        nonlocal removed_count
        removed_count += len(match.group(0))
        return ""

    cleaned = pattern.sub(_sub_func, text)
    return cleaned, removed_count


def detect_bom(raw_bytes: bytes) -> bool:
    return raw_bytes.startswith(b"\xef\xbb\xbf")


def iter_target_files(
    root: Path,
    exts: Sequence[str],
    ignore_dirs: Sequence[str],
    include_hidden: bool,
) -> Iterable[Path]:
    normalized_exts = {e.lower() for e in exts}
    ignore_set = set(ignore_dirs)

    for dirpath, dirnames, filenames in os.walk(root):
        # Prune ignored directories in-place for efficiency
        pruned = []
        for d in list(dirnames):
            if d in ignore_set or (not include_hidden and d.startswith(".")):
                pruned.append(d)
        for d in pruned:
            dirnames.remove(d)

        for fname in filenames:
            if not include_hidden and fname.startswith("."):
                continue
            if Path(fname).suffix.lower() in normalized_exts:
                yield Path(dirpath) / fname


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Safely find and optionally remove emoji characters from source files.",
    )
    parser.add_argument("--root", type=str, default=".", help="Directory to scan recursively.")
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply changes. Without this flag, runs in dry-run mode.",
    )
    parser.add_argument(
        "--backup-dir",
        type=str,
        default=None,
        help="Directory to store backups. Defaults to emoji_backups_YYYYmmddHHMMSS under root.",
    )
    parser.add_argument(
        "--no-backup",
        action="store_true",
        help="Do not write backups (not recommended).",
    )
    parser.add_argument(
        "--ext",
        dest="exts",
        nargs="+",
        default=[".py"],
        help="File extensions to include (e.g., .py .pyw).",
    )
    parser.add_argument(
        "--ignore-dirs",
        nargs="+",
        default=[
            ".git",
            ".hg",
            ".svn",
            ".venv",
            "venv",
            "node_modules",
            "__pycache__",
            ".mypy_cache",
            ".pytest_cache",
            ".idea",
            ".vscode",
            "dist",
            "build",
        ],
        help="Directory names to ignore at any depth.",
    )
    parser.add_argument(
        "--include-hidden",
        action="store_true",
        help="Include hidden files and directories (names starting with a dot).",
    )
    parser.add_argument(
        "--aggressive",
        action="store_true",
        help="Also remove ZWJ, variation selectors, and skin tone modifiers.",
    )

    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    if not root.exists() or not root.is_dir():
        print(f"[ERROR] Root directory not found: {root}", file=sys.stderr)
        return 2

    emoji_mod = try_import_emoji()
    if emoji_mod is None:
        print("[INFO] 'emoji' library not found. Using regex fallback.")
        print("       For best accuracy: pip install emoji")
    else:
        try:
            ver = getattr(emoji_mod, "__version__", None)
        except Exception:
            ver = None
        print(f"[INFO] Using 'emoji' library{f' v{ver}' if ver else ''}.")

    backup_dir: Optional[Path]
    timestamp = _dt.datetime.now().strftime("%Y%m%d%H%M%S")
    if args.apply and not args.no_backup:
        backup_dir = Path(args.backup_dir) if args.backup_dir else (root / f"emoji_backups_{timestamp}")
        backup_dir.mkdir(parents=True, exist_ok=True)
        print(f"[INFO] Backups will be stored under: {backup_dir}")
    else:
        backup_dir = None

    total_files = 0
    changed_files = 0
    total_removed = 0
    skipped_files: List[Tuple[Path, str]] = []

    for path in iter_target_files(
        root=root,
        exts=args.exts,
        ignore_dirs=args.ignore_dirs,
        include_hidden=args.include_hidden,
    ):
        total_files += 1
        try:
            raw = path.read_bytes()
        except Exception as e:
            skipped_files.append((path, f"read error: {e}"))
            continue

        had_bom = detect_bom(raw)

        try:
            # Preserve newline characters as-is by decoding from bytes directly
            text = raw.decode("utf-8-sig")
        except UnicodeDecodeError:
            skipped_files.append((path, "not UTF-8 (or UTF-8 with BOM)"))
            continue

        cleaned, removed = remove_emojis_from_text(text, emoji_mod, aggressive=args.aggressive)

        if removed > 0:
            print(f"[CHANGE] {path}  removed={removed}")
            total_removed += removed
            changed_files += 1

            if args.apply:
                # Backup original bytes
                if backup_dir is not None:
                    backup_path = backup_dir / path.relative_to(root)
                    backup_path.parent.mkdir(parents=True, exist_ok=True)
                    try:
                        backup_path.write_bytes(raw)
                    except Exception as e:
                        print(f"[WARN] Failed to backup {path}: {e}", file=sys.stderr)

                # Preserve newline style by writing text with newlines untouched
                encoding = "utf-8-sig" if had_bom else "utf-8"
                try:
                    with open(path, "w", encoding=encoding, newline="") as fw:
                        fw.write(cleaned)
                except Exception as e:
                    print(f"[ERROR] Failed to write {path}: {e}", file=sys.stderr)
        else:
            # No change
            pass

    print("\n=== Summary ===")
    print(f"Scanned files: {total_files}")
    print(f"Files with changes: {changed_files}")
    print(f"Total characters removed: {total_removed}")
    if skipped_files:
        print(f"Skipped files: {len(skipped_files)}")
        for p, reason in skipped_files[:20]:
            print(f"  - {p}: {reason}")
        if len(skipped_files) > 20:
            print(f"  ... and {len(skipped_files) - 20} more")

    if not args.apply:
        print("\nNo files were modified (dry-run). Re-run with --apply to make changes.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())


