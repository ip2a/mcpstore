"""Local example import bridge; implementation lives in _shared."""
from pathlib import Path
import sys

_root = next(parent for parent in Path(__file__).resolve().parents if (parent / "_shared" / "example_utils.py").exists())
if str(_root) not in sys.path:
    sys.path.insert(0, str(_root))

from _shared.example_utils import *  # noqa: F401,F403,E402
