"""Scan reference trees and classify *_rs shadows."""

from __future__ import annotations

from collections import defaultdict
from fnmatch import fnmatch
from pathlib import Path

from .classify import classify_rs
from .constants import (
    SKIP_DIR_NAMES,
    SKIP_DIR_PREFIXES,
    SKIP_DIR_SUFFIXES,
    SKIP_FILE_GLOBS,
)
from .models import MirrorReport, MirrorSpec, StemResult
from .sha import combined_digest, file_sha256


class MirrorScanner:
    def __init__(self, repo: Path, ignore_dirs: set[str], ignore_files: set[str]):
        self.repo = repo
        self.ignore_dirs = ignore_dirs
        self.ignore_files = ignore_files

    @staticmethod
    def _dir_skipped(rel_parts: tuple[str, ...]) -> bool:
        for part in rel_parts:
            if part in SKIP_DIR_NAMES:
                return True
            if any(part.endswith(suf) for suf in SKIP_DIR_SUFFIXES):
                return True
            if any(part == pref or part.startswith(pref + "-") for pref in SKIP_DIR_PREFIXES):
                return True
        return False

    @staticmethod
    def _file_skipped(name: str) -> bool:
        return any(fnmatch(name, pat) for pat in SKIP_FILE_GLOBS)

    @staticmethod
    def _in_only_dirs(rel: str, only_dirs: list[str] | None) -> bool:
        if only_dirs is None:
            return True
        if "/" not in rel:
            return "" in only_dirs
        for d in only_dirs:
            if d and (rel == d or rel.startswith(d + "/")):
                return True
        return False

    def collect_stems(self, spec: MirrorSpec) -> dict[str, list[Path]]:
        stems: dict[str, list[Path]] = defaultdict(list)
        ref_dir = self.repo / spec.ref
        if not ref_dir.is_dir():
            return stems

        suffixes = {".c", ".h"}
        if spec.python_to_rs:
            suffixes.add(".py")
        if spec.asm_ok:
            suffixes.update({".S", ".s"})

        for path in sorted(ref_dir.rglob("*")):
            if not path.is_file() or path.suffix not in suffixes:
                continue
            rel = path.relative_to(ref_dir).as_posix()
            repo_rel = path.relative_to(self.repo).as_posix()
            if self._dir_skipped(Path(rel).parts):
                continue
            if self._file_skipped(path.name):
                continue
            if any(repo_rel == d or repo_rel.startswith(d + "/") for d in self.ignore_dirs):
                continue
            if repo_rel in self.ignore_files or rel in self.ignore_files:
                continue
            if not self._in_only_dirs(rel, spec.only_dirs):
                continue
            stem_rel = path.relative_to(ref_dir).with_suffix("").as_posix()
            stems[stem_rel].append(path)
        return stems

    @staticmethod
    def shadow_candidates(shadow_root: Path, stem: str, asm_ok: bool) -> list[Path]:
        base = shadow_root / stem
        cands = [base.with_suffix(".rs")]
        if asm_ok:
            cands += [base.with_suffix(".S"), base.with_suffix(".s")]
        return cands

    def scan(self, spec: MirrorSpec) -> MirrorReport:
        report = MirrorReport(name=spec.name, ref=spec.ref, shadow=spec.shadow)
        if not spec.enabled:
            return report

        stems = self.collect_stems(spec)
        shadow_dir = self.repo / spec.shadow
        for stem, files in sorted(stems.items()):
            ref_shas = {
                p.relative_to(self.repo).as_posix(): file_sha256(p)
                for p in sorted(files, key=lambda x: x.as_posix())
            }
            ref_files = sorted(ref_shas)
            ref_digest = combined_digest(ref_shas)
            only_asm = all(p.suffix.lower() == ".s" for p in files)
            cands = self.shadow_candidates(shadow_dir, stem, asm_ok=spec.asm_ok or only_asm)
            existing = next((c for c in cands if c.is_file()), None)
            shadow_rel = cands[0].relative_to(self.repo).as_posix()
            if existing is None:
                report.stems.append(
                    StemResult(
                        stem=stem,
                        ref_files=ref_files,
                        shadow=shadow_rel,
                        status="missing",
                        ref_shas=ref_shas,
                        ref_digest=ref_digest,
                    )
                )
                continue
            status, detail = classify_rs(existing)
            report.stems.append(
                StemResult(
                    stem=stem,
                    ref_files=ref_files,
                    shadow=existing.relative_to(self.repo).as_posix(),
                    status=status,
                    detail=detail,
                    ref_shas=ref_shas,
                    ref_digest=ref_digest,
                    shadow_sha=file_sha256(existing),
                )
            )
        return report
