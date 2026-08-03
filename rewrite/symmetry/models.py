"""Data models for symmetry reports."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path

from .constants import STATUS_ORDER, STATUS_WEIGHT


@dataclass
class StemResult:
    stem: str
    ref_files: list[str]
    shadow: str
    status: str
    detail: str = ""
    ref_shas: dict[str, str] = field(default_factory=dict)
    ref_digest: str = ""
    shadow_sha: str = ""


@dataclass
class ShaDiff:
    refs_added: list[str] = field(default_factory=list)
    refs_changed: list[str] = field(default_factory=list)
    refs_removed: list[str] = field(default_factory=list)
    shadows_added: list[str] = field(default_factory=list)
    shadows_changed: list[str] = field(default_factory=list)
    shadows_removed: list[str] = field(default_factory=list)
    stale_stems: list[str] = field(default_factory=list)
    checkpoint_at: str | None = None

    @property
    def any(self) -> bool:
        return bool(
            self.refs_added
            or self.refs_changed
            or self.refs_removed
            or self.shadows_added
            or self.shadows_changed
            or self.shadows_removed
            or self.stale_stems
        )


@dataclass
class PmSymbolResult:
    module: str
    name: str
    symbol: str
    status: str
    detail: str = ""


@dataclass
class MirrorSpec:
    name: str
    ref: str
    shadow: str
    python_to_rs: bool = False
    asm_ok: bool = False
    enabled: bool = True
    only_dirs: list[str] | None = None


@dataclass
class MirrorReport:
    name: str
    ref: str
    shadow: str
    stems: list[StemResult] = field(default_factory=list)

    def counts(self) -> dict[str, int]:
        c = {s: 0 for s in STATUS_ORDER}
        for st in self.stems:
            c[st.status] = c.get(st.status, 0) + 1
        return c

    def progress_pct(self) -> float:
        c = self.counts()
        tracked = sum(
            c[s] for s in ("done", "gaps", "partial", "stub", "stale", "missing")
        )
        if tracked == 0:
            return 100.0
        weight = sum(c[s] * STATUS_WEIGHT.get(s, 0.0) for s in c)
        return 100.0 * weight / tracked


@dataclass
class FullReport:
    mirrors: list[MirrorReport] = field(default_factory=list)
    pm_modules: dict[str, list[PmSymbolResult]] = field(default_factory=dict)
    pm_infra: list[PmSymbolResult] = field(default_factory=list)
    sha_diff: ShaDiff | None = None
    progress_delta: dict | None = None

    def total_counts(self) -> dict[str, int]:
        tot = {s: 0 for s in STATUS_ORDER}
        for m in self.mirrors:
            for k, v in m.counts().items():
                tot[k] = tot.get(k, 0) + v
        return tot

    def file_progress_pct(self) -> float:
        c = self.total_counts()
        tracked = sum(
            c[s] for s in ("done", "gaps", "partial", "stub", "stale", "missing")
        )
        if tracked == 0:
            return 100.0
        weight = sum(c[s] * STATUS_WEIGHT.get(s, 0.0) for s in c)
        return 100.0 * weight / tracked

    def pm_counts(self) -> dict[str, int]:
        out = {"present": 0, "partial": 0, "stub": 0, "missing": 0, "total": 0}
        for syms in self.pm_modules.values():
            for s in syms:
                out["total"] += 1
                out[s.status] = out.get(s.status, 0) + 1
        for s in self.pm_infra:
            out["total"] += 1
            out[s.status] = out.get(s.status, 0) + 1
        return out

    def pm_progress_pct(self) -> float:
        c = self.pm_counts()
        if c["total"] == 0:
            return 100.0
        weight = (
            c.get("present", 0) * 1.0
            + c.get("partial", 0) * 0.5
            + c.get("stub", 0) * 0.25
        )
        return 100.0 * weight / c["total"]

    def to_jsonable(self) -> dict:
        out = {
            "summary": {
                "file_progress_pct": round(self.file_progress_pct(), 1),
                "pm_progress_pct": round(self.pm_progress_pct(), 1),
                "file_counts": self.total_counts(),
                "pm_counts": self.pm_counts(),
                "conversion": self.conversion_stats(),
            },
            "mirrors": [
                {
                    "name": m.name,
                    "ref": m.ref,
                    "shadow": m.shadow,
                    "counts": m.counts(),
                    "progress_pct": round(m.progress_pct(), 1),
                    "stems": [asdict(s) for s in m.stems],
                }
                for m in self.mirrors
            ],
            "pm_modules": {
                name: [asdict(s) for s in syms] for name, syms in self.pm_modules.items()
            },
            "pm_infra": [asdict(s) for s in self.pm_infra],
        }
        if self.sha_diff is not None:
            out["sha_diff"] = asdict(self.sha_diff)
        if self.progress_delta is not None:
            out["progress_delta"] = self.progress_delta
        return out

    def iter_stems(self, status: str | None = None):
        for m in self.mirrors:
            for st in m.stems:
                if status is None or st.status == status:
                    yield m, st

    def conversion_stats(self) -> dict:
        """Stats: source kinds (.c/.h/.py/…) → shadow (.rs) by stem status."""
        from collections import Counter, defaultdict

        # Per source extension: count of ref files whose stem has each status.
        by_ext_status: dict[str, Counter] = defaultdict(Counter)
        # Stem input shape → status (e.g. "c+h", "c", "h", "py", "c+h+py").
        by_shape_status: dict[str, Counter] = defaultdict(Counter)
        # Shadow suffix counts by status.
        by_shadow_status: dict[str, Counter] = defaultdict(Counter)
        ref_files_total = 0
        stems_total = 0

        for _m, st in self.iter_stems():
            stems_total += 1
            exts = []
            for rf in st.ref_files:
                ref_files_total += 1
                ext = Path(rf).suffix.lower() or "(none)"
                by_ext_status[ext][st.status] += 1
                # Normalize .S/.s
                tag = ext.lstrip(".") or "none"
                if tag not in exts:
                    exts.append(tag)
            # Stable shape label: c+h, h, py, S, …
            order = {"c": 0, "h": 1, "py": 2, "s": 3, "S": 3}
            shape_parts = sorted(set(exts), key=lambda x: (order.get(x, 9), x))
            shape = "+".join(shape_parts) if shape_parts else "?"
            by_shape_status[shape][st.status] += 1

            shadow_ext = Path(st.shadow).suffix.lower() or "(none)"
            by_shadow_status[shadow_ext][st.status] += 1

        def _rows(mapping: dict[str, Counter]) -> list[dict]:
            rows = []
            for key in sorted(mapping):
                c = mapping[key]
                rows.append(
                    {
                        "key": key,
                        "done": c.get("done", 0),
                        "gaps": c.get("gaps", 0),
                        "partial": c.get("partial", 0),
                        "stub": c.get("stub", 0),
                        "stale": c.get("stale", 0),
                        "missing": c.get("missing", 0),
                        "total": sum(c.values()),
                    }
                )
            return rows

        return {
            "ref_files_total": ref_files_total,
            "stems_total": stems_total,
            "by_source_ext": _rows(by_ext_status),
            "by_stem_shape": _rows(by_shape_status),
            "by_shadow_ext": _rows(by_shadow_status),
        }
