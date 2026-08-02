"""Conversion progress history and deltas."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

from .models import FullReport


class ProgressTracker:
    """Append-only JSONL history under rewrite/ for intermediate check-ins."""

    def __init__(self, path: Path):
        self.path = path

    def summary_entry(self, report: FullReport) -> dict:
        fc = report.total_counts()
        pc = report.pm_counts()
        return {
            "at": datetime.now(timezone.utc).isoformat(),
            "file_progress_pct": round(report.file_progress_pct(), 1),
            "pm_progress_pct": round(report.pm_progress_pct(), 1),
            "files": {
                "done": fc.get("done", 0),
                "partial": fc.get("partial", 0),
                "stub": fc.get("stub", 0),
                "stale": fc.get("stale", 0),
                "missing": fc.get("missing", 0),
            },
            "pm": {
                "present": pc.get("present", 0),
                "partial": pc.get("partial", 0),
                "stub": pc.get("stub", 0),
                "missing": pc.get("missing", 0),
                "total": pc.get("total", 0),
            },
            "done_stems": sorted(
                st.shadow for _, st in report.iter_stems("done")
            ),
            "present_symbols": sorted(
                [s.symbol for syms in report.pm_modules.values() for s in syms if s.status == "present"]
                + [s.symbol for s in report.pm_infra if s.status == "present"]
            ),
        }

    def append(self, report: FullReport) -> dict:
        entry = self.summary_entry(report)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with self.path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(entry, separators=(",", ":")) + "\n")
        return entry

    def load_all(self) -> list[dict]:
        if not self.path.is_file():
            return []
        out = []
        for line in self.path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line:
                out.append(json.loads(line))
        return out

    def last(self) -> dict | None:
        all_entries = self.load_all()
        return all_entries[-1] if all_entries else None

    def delta_since_last(self, report: FullReport) -> dict | None:
        prev = self.last()
        if not prev:
            return None
        cur = self.summary_entry(report)
        prev_done = set(prev.get("done_stems", []))
        cur_done = set(cur.get("done_stems", []))
        prev_syms = set(prev.get("present_symbols", []))
        cur_syms = set(cur.get("present_symbols", []))
        return {
            "since": prev.get("at"),
            "file_progress_pct": {
                "from": prev.get("file_progress_pct", 0),
                "to": cur["file_progress_pct"],
                "delta": round(cur["file_progress_pct"] - prev.get("file_progress_pct", 0), 1),
            },
            "pm_progress_pct": {
                "from": prev.get("pm_progress_pct", 0),
                "to": cur["pm_progress_pct"],
                "delta": round(cur["pm_progress_pct"] - prev.get("pm_progress_pct", 0), 1),
            },
            "stems_newly_done": sorted(cur_done - prev_done),
            "stems_no_longer_done": sorted(prev_done - cur_done),
            "symbols_newly_present": sorted(cur_syms - prev_syms),
            "symbols_no_longer_present": sorted(prev_syms - cur_syms),
        }

    def format_history(self, limit: int = 20) -> str:
        entries = self.load_all()
        if not entries:
            return "(no history yet — run with --checkpoint)"
        lines = ["Conversion history", "-" * 72]
        for e in entries[-limit:]:
            f = e.get("files", {})
            p = e.get("pm", {})
            lines.append(
                f"{e.get('at', '?'):<32}  "
                f"files {e.get('file_progress_pct', 0):5.1f}% "
                f"(done={f.get('done', 0)} stub={f.get('stub', 0)} miss={f.get('missing', 0)})  "
                f"pm {e.get('pm_progress_pct', 0):5.1f}% "
                f"({p.get('present', 0)}/{p.get('total', 0)})"
            )
        return "\n".join(lines)
