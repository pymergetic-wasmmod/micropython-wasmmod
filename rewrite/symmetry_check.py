#!/usr/bin/env python3
"""Backward-compatible launcher for the rewrite.symmetry package.

Prefer:
    python -m rewrite.symmetry
    python -m rewrite.symmetry --checkpoint
"""

from __future__ import annotations

import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
if str(_ROOT) not in sys.path:
    sys.path.insert(0, str(_ROOT))

from rewrite.symmetry.cli import main

if __name__ == "__main__":
    sys.exit(main())
