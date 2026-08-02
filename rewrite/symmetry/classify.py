"""Classify shadow Rust files as stub / partial / done."""

from __future__ import annotations

from pathlib import Path


def classify_rs(path: Path) -> tuple[str, str]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        return "missing", f"unreadable: {e}"

    if "// symmetry: stub" in text or "/* symmetry: stub */" in text:
        return "stub", "marked stub"
    if "// symmetry: partial" in text or "/* symmetry: partial */" in text:
        return "partial", "marked partial"
    if "// symmetry: done" in text or "/* symmetry: done */" in text:
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
