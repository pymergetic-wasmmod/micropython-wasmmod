"""SHA-256 checkpoints for upstream + shadow trees."""

from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

from .models import FullReport, ShaDiff


def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def combined_digest(path_to_sha: dict[str, str]) -> str:
    h = hashlib.sha256()
    for path in sorted(path_to_sha):
        h.update(path.encode())
        h.update(b"\0")
        h.update(path_to_sha[path].encode())
        h.update(b"\n")
    return h.hexdigest()


class ShaStore:
    """Load / diff / write rewrite/symmetry_shas.json checkpoints."""

    def __init__(self, path: Path):
        self.path = path
        self.state: dict = {}

    def load(self) -> dict:
        if not self.path.is_file():
            self.state = {}
            return self.state
        self.state = json.loads(self.path.read_text(encoding="utf-8"))
        return self.state

    @staticmethod
    def snapshot_from_report(report: FullReport) -> dict:
        refs: dict[str, str] = {}
        shadows: dict[str, str] = {}
        stems: dict[str, dict] = {}
        for m in report.mirrors:
            for st in m.stems:
                key = f"{m.ref}/{st.stem}"
                refs.update(st.ref_shas)
                if st.shadow_sha:
                    shadows[st.shadow] = st.shadow_sha
                stems[key] = {
                    "ref_files": st.ref_files,
                    "ref_digest": st.ref_digest,
                    "shadow": st.shadow,
                    "shadow_sha": st.shadow_sha,
                    "status": st.status,
                }
        return {
            "updated_at": datetime.now(timezone.utc).isoformat(),
            "refs": dict(sorted(refs.items())),
            "shadows": dict(sorted(shadows.items())),
            "stems": dict(sorted(stems.items())),
        }

    def write(self, report: FullReport) -> dict:
        payload = self.snapshot_from_report(report)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        self.state = payload
        return payload

    def diff(self, report: FullReport, prev: dict | None = None) -> ShaDiff:
        prev = prev if prev is not None else self.state
        cur = self.snapshot_from_report(report)
        diff = ShaDiff(checkpoint_at=prev.get("updated_at") if prev else None)
        if not prev:
            return diff

        old_refs = prev.get("refs", {})
        old_shadows = prev.get("shadows", {})
        new_refs = cur["refs"]
        new_shadows = cur["shadows"]

        for p in sorted(set(new_refs) - set(old_refs)):
            diff.refs_added.append(p)
        for p in sorted(set(old_refs) - set(new_refs)):
            diff.refs_removed.append(p)
        for p in sorted(set(new_refs) & set(old_refs)):
            if new_refs[p] != old_refs[p]:
                diff.refs_changed.append(p)

        for p in sorted(set(new_shadows) - set(old_shadows)):
            diff.shadows_added.append(p)
        for p in sorted(set(old_shadows) - set(new_shadows)):
            diff.shadows_removed.append(p)
        for p in sorted(set(new_shadows) & set(old_shadows)):
            if new_shadows[p] != old_shadows[p]:
                diff.shadows_changed.append(p)
        return diff

    def apply_stale(self, report: FullReport, prev: dict | None = None) -> list[str]:
        prev = prev if prev is not None else self.state
        if not prev:
            return []
        old_stems = prev.get("stems", {})
        stale: list[str] = []
        for m in report.mirrors:
            for st in m.stems:
                if st.status == "missing":
                    continue
                key = f"{m.ref}/{st.stem}"
                old = old_stems.get(key)
                ref_changed = False
                if old:
                    ref_changed = old.get("ref_digest") != st.ref_digest
                else:
                    for rf, sha in st.ref_shas.items():
                        prev_sha = prev.get("refs", {}).get(rf)
                        if prev_sha is not None and prev_sha != sha:
                            ref_changed = True
                            break
                if ref_changed and st.status in {"done", "partial", "stub"}:
                    st.status = "stale"
                    extra = "upstream ref changed since checkpoint"
                    st.detail = f"{st.detail}; {extra}" if st.detail else extra
                    stale.append(st.shadow)
        return stale
