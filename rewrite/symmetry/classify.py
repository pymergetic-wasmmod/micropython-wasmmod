"""Classify shadow Rust files as stub / gaps / partial / done."""

from __future__ import annotations

import re
from pathlib import Path

_GAPS_LINE = re.compile(r"^\s*//\s*gaps:\s*(.*)$")


def parse_gap_list(text: str) -> list[str]:
    """Extract `// gaps:` bullet lines from a shadow file."""
    gaps: list[str] = []
    in_block = False
    for ln in text.splitlines():
        m = _GAPS_LINE.match(ln)
        if m:
            in_block = True
            rest = m.group(1).strip()
            if rest:
                gaps.append(rest.lstrip("- ").strip())
            continue
        if in_block:
            s = ln.strip()
            if s.startswith("// -") or s.startswith("//-"):
                gaps.append(s.lstrip("/ ").lstrip("- ").strip())
            elif s.startswith("//") and s[2:].strip():
                # continuation comment under gaps block
                body = s[2:].strip()
                if body.startswith("-"):
                    gaps.append(body.lstrip("- ").strip())
                else:
                    # end of gaps block on non-bullet comment / code
                    if not body.startswith("symmetry:"):
                        break
            else:
                break
    return [g for g in gaps if g]


def classify_rs(path: Path) -> tuple[str, str]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        return "missing", f"unreadable: {e}"

    # Explicit markers win (gaps before done — false "done" must be corrected).
    if "// symmetry: stub" in text or "/* symmetry: stub */" in text:
        return "stub", "marked stub"
    if "// symmetry: gaps" in text or "/* symmetry: gaps */" in text:
        items = parse_gap_list(text)
        detail = "; ".join(items[:3]) if items else "marked gaps"
        if len(items) > 3:
            detail += f" (+{len(items) - 3} more)"
        return "gaps", detail
    if "// symmetry: partial" in text or "/* symmetry: partial */" in text:
        return "partial", "marked partial"
    if "// symmetry: done" in text or "/* symmetry: done */" in text:
        # Safety net: done file that still lists gaps: is mis-marked.
        items = parse_gap_list(text)
        if items:
            return "gaps", "done+gaps list: " + "; ".join(items[:3])
        return "done", "marked done"

    stripped = "\n".join(
        ln for ln in text.splitlines() if ln.strip() and not ln.strip().startswith("//")
    )
    if not stripped.strip():
        return "stub", "empty"

    todo_count = text.count("todo!") + text.count("unimplemented!")
    if "unimplemented!" in text and len(stripped) < 200:
        return "stub", "unimplemented placeholder"
    if "todo!" in text or "todo!(" in text:
        non_todo_lines = [
            ln
            for ln in stripped.splitlines()
            if "todo!" not in ln and "unimplemented!" not in ln
        ]
        if len(non_todo_lines) < 3:
            return "stub", "todo! placeholder"
        return "partial", f"{todo_count} todo!/unimplemented!"

    if len(stripped) < 40:
        return "stub", "too small"
    return "done", ""
