"""Config loading and mirror discovery."""

from __future__ import annotations

import tomllib
from pathlib import Path

from .models import MirrorSpec


class Config:
    def __init__(self, repo: Path, raw: dict | None = None, path: Path | None = None):
        self.repo = repo
        self.path = path
        self.raw = raw or {}
        general = self.raw.get("general", {})
        self.ports = list(general.get("ports", ["unix", "qemu"]))
        self.qemu_mcu = list(general.get("qemu_mcu", ["rv32"]))
        self.python_to_rs = set(general.get("python_to_rs", ["extmod"]))
        self.mpy_cross = bool(general.get("mpy_cross", False))
        self.baseline_path = repo / general.get("baseline_path", "rewrite/symmetry_baseline.json")
        self.sha_path = repo / general.get("sha_path", "rewrite/symmetry_shas.json")
        self.history_path = repo / general.get("history_path", "rewrite/symmetry_history.jsonl")
        ign = self.raw.get("ignore", {})
        self.ignore_dirs = {d.rstrip("/") for d in ign.get("dirs", [])}
        self.ignore_files = set(ign.get("files", []))

    @classmethod
    def load(cls, repo: Path, path: Path | None = None) -> Config:
        cfg_path = path or (repo / "rewrite" / "symmetry_check.toml")
        raw: dict = {}
        if cfg_path.is_file():
            with cfg_path.open("rb") as f:
                raw = tomllib.load(f)
        return cls(repo, raw=raw, path=cfg_path)

    def mirrors(self) -> list[MirrorSpec]:
        py_trees = self.python_to_rs
        mirrors = [
            MirrorSpec("py", "py", "py_rs", python_to_rs="py" in py_trees),
            MirrorSpec("shared", "shared", "shared_rs", asm_ok=True),
            MirrorSpec("extmod", "extmod", "extmod_rs", python_to_rs="extmod" in py_trees),
        ]
        for port in self.ports:
            if port == "qemu" and self.qemu_mcu:
                only = [""] + [f"mcu/{m}" for m in self.qemu_mcu]
                mirrors.append(
                    MirrorSpec(
                        port,
                        f"ports/{port}",
                        f"ports_rs/{port}",
                        only_dirs=only,
                    )
                )
            else:
                mirrors.append(MirrorSpec(port, f"ports/{port}", f"ports_rs/{port}"))
        mirrors.append(
            MirrorSpec(
                "mpy-cross",
                "mpy-cross",
                "mpy-cross_rs",
                enabled=self.mpy_cross,
            )
        )
        return mirrors
