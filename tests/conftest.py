from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SDK_PYTHON = ROOT / "sdk" / "python"

for p in (SDK_PYTHON, ROOT):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))
