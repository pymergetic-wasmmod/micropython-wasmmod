"""Regression baseline for done stems / present pm symbols."""

from __future__ import annotations

import json
from pathlib import Path

from .models import FullReport


class BaselineStore:
    def __init__(self, path: Path):
        self.path = path

    def snapshot(self, report: FullReport) -> dict:
        done_stems = [st.shadow for _, st in report.iter_stems("done")]
        present_syms = [
            s.symbol
            for syms in report.pm_modules.values()
            for s in syms
            if s.status == "present"
        ]
        present_syms += [
            s.symbol for s in report.pm_infra if s.status == "present"
        ]
        return {
            "done_stems": sorted(done_stems),
            "present_symbols": sorted(present_syms),
            "meta": {"note": "symmetry baseline"},
        }

    def write(self, report: FullReport) -> dict:
        snap = self.snapshot(report)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(snap, indent=2) + "\n", encoding="utf-8")
        return snap

    def regressions(self, report: FullReport) -> list[str]:
        if not self.path.is_file():
            return [f"no baseline at {self.path} (run --write-baseline)"]
        old = json.loads(self.path.read_text(encoding="utf-8"))
        snap = self.snapshot(report)
        problems = []
        for s in sorted(set(old.get("done_stems", [])) - set(snap["done_stems"])):
            problems.append(f"regression: stem was done, now not: {s}")
        for s in sorted(
            set(old.get("present_symbols", [])) - set(snap["present_symbols"])
        ):
            problems.append(f"regression: symbol was present, now not: {s}")
        return problems
