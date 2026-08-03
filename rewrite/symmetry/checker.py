"""Orchestrator: discover, scan, SHA-diff, progress."""

from __future__ import annotations

from pathlib import Path

from .baseline import BaselineStore
from .config import Config
from .mirror import MirrorScanner
from .models import FullReport, MirrorReport, ShaDiff
from .pm import PmInventory
from .progress import ProgressTracker
from .sha import ShaStore


def repo_root() -> Path:
    # rewrite/symmetry/checker.py → parents[2] = repo root
    return Path(__file__).resolve().parents[2]


class SymmetryChecker:
    """Feature-complete conversion tracker for MicroPython → *_rs."""

    def __init__(self, repo: Path | None = None, config_path: Path | None = None):
        self.repo = repo or repo_root()
        self.config = Config.load(self.repo, config_path)
        self.sha_store = ShaStore(self.config.sha_path)
        self.baseline = BaselineStore(self.config.baseline_path)
        self.progress = ProgressTracker(self.config.history_path)
        self.mirrors = MirrorScanner(self.repo, self.config.ignore_dirs, self.config.ignore_files)
        self.pm = PmInventory(self.repo)

    def scan(
        self,
        *,
        trees: set[str] | None = None,
        include_pm: bool = True,
        compare_shas: bool = True,
        compare_progress: bool = True,
    ) -> FullReport:
        report = FullReport()
        for spec in self.config.mirrors():
            if trees and spec.name not in trees:
                continue
            if not spec.enabled:
                if trees is None:
                    report.mirrors.append(
                        MirrorReport(name=spec.name, ref=spec.ref, shadow=spec.shadow)
                    )
                continue
            report.mirrors.append(self.mirrors.scan(spec))

        prev = self.sha_store.load() if compare_shas else {}
        if compare_shas and report.mirrors:
            if prev:
                stale = self.sha_store.apply_stale(report, prev)
                diff = self.sha_store.diff(report, prev)
                diff.stale_stems = stale
                report.sha_diff = diff
            else:
                report.sha_diff = ShaDiff(checkpoint_at=None)

        if include_pm:
            mods, infra = self.pm.scan()
            report.pm_modules = mods
            report.pm_infra = infra

        if compare_progress:
            report.progress_delta = self.progress.delta_since_last(report)

        return report

    def checkpoint(self, report: FullReport) -> dict:
        """Update SHA store + append progress history."""
        sha_payload = self.sha_store.write(report)
        hist = self.progress.append(report)
        return {"sha": sha_payload, "history": hist}

    def write_baseline(self, report: FullReport) -> dict:
        return self.baseline.write(report)

    def regressions(self, report: FullReport) -> list[str]:
        return self.baseline.regressions(report)
