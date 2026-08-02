"""Discover and inventory pm_mpy_* API surface from MicroPython modules."""

from __future__ import annotations

import re
from collections import defaultdict
from pathlib import Path
from typing import Iterable

from .classify import classify_rs
from .constants import DEFAULT_INFRA, PM_SEARCH
from .models import PmSymbolResult

QSTR_KEY_RE = re.compile(
    r"\{\s*MP_ROM_QSTR\s*\(\s*MP_QSTR_([A-Za-z0-9_]+)\s*\)"
)
TABLE_START_RE = re.compile(
    r"(?:static\s+)?const\s+mp_rom_map_elem_t\s+(\w+_globals_table)\s*\[\s*\]\s*=\s*\{"
)
REGISTER_RE = re.compile(
    r"MP_REGISTER_(?:EXTENSIBLE_)?MODULE\s*\(\s*MP_QSTR_([A-Za-z0-9_]+)\s*,"
)
SYM_DEF_RE = re.compile(
    r"(?:#\[no_mangle\][^\n]*\n\s*)?(?:pub\s+)?(?:unsafe\s+)?extern\s+\"C\"\s+fn\s+(pm_mpy_\w+)"
    r"|^\s*(?:pm_mpy_status_t|pm_mpy_obj_t|void|int|bool|size_t|const\s+char\s*\*)\s+(pm_mpy_\w+)\s*\(",
    re.MULTILINE,
)
SYM_ANY_RE = re.compile(r"\bpm_mpy_[A-Za-z0-9_]+\b")


class PmInventory:
    def __init__(self, repo: Path, search_paths: Iterable[str] = PM_SEARCH):
        self.repo = repo
        self.search_paths = tuple(search_paths)

    @staticmethod
    def expand_x_macro_list(source_text: str, list_name: str) -> list[str]:
        pat = re.compile(
            rf"#ifndef\s+{re.escape(list_name)}\s*\n\s*#define\s+{re.escape(list_name)}\s*\\?\s*\n"
            r"(.*?)\n#endif",
            re.DOTALL,
        )
        m = pat.search(source_text)
        if not m:
            pat2 = re.compile(
                rf"#define\s+{re.escape(list_name)}\s*\\?\s*\n(.*?)(?=\n\s*#|\nstatic\s|\nconst\s|\Z)",
                re.DOTALL,
            )
            m = pat2.search(source_text)
        if not m:
            return []
        return re.findall(r"\bX\s*\(\s*([A-Za-z0-9_]+)\s*\)", m.group(1))

    @staticmethod
    def table_body(source_text: str, table: str) -> str | None:
        pat = re.compile(
            rf"(?:static\s+)?const\s+mp_rom_map_elem_t\s+{re.escape(table)}\s*\[\s*\]\s*=\s*\{{",
            re.MULTILINE,
        )
        m = pat.search(source_text)
        if not m:
            return None
        start = m.end()
        depth = 1
        i = start
        while i < len(source_text) and depth:
            if source_text[i] == "{":
                depth += 1
            elif source_text[i] == "}":
                depth -= 1
            i += 1
        return source_text[start : i - 1]

    def extract_table_names(self, source_text: str, table: str | None) -> list[str]:
        body = self.table_body(source_text, table) if table else source_text
        if body is None:
            body = source_text
        names: list[str] = []
        seen: set[str] = set()
        for m in QSTR_KEY_RE.finditer(body):
            name = m.group(1)
            if name not in seen:
                seen.add(name)
                names.append(name)
        for list_name in re.findall(r"\b([A-Z0-9_]+_LIST)\b", body):
            for name in self.expand_x_macro_list(source_text, list_name):
                if name not in seen:
                    seen.add(name)
                    names.append(name)
        return names

    @staticmethod
    def pick_globals_table(source_text: str, module_name: str) -> str | None:
        tables = TABLE_START_RE.findall(source_text)
        if not tables:
            return None
        needle = module_name.lstrip("_")
        for t in tables:
            if needle in t:
                return t
        for t in tables:
            if "module" in t and t.endswith("_globals_table"):
                return t
        return tables[0]

    @staticmethod
    def prefix_for(module_name: str) -> str:
        return f"pm_mpy_{module_name.lstrip('_')}_"

    @staticmethod
    def _source_score(path: Path, mod_name: str) -> tuple[int, str]:
        stem = path.stem
        want = "mod" + mod_name.lstrip("_")
        if stem == want:
            return (0, path.as_posix())
        if want in stem or mod_name.lstrip("_") in stem:
            return (1, path.as_posix())
        return (2, path.as_posix())

    def discover_modules(self) -> list[dict]:
        best: dict[str, tuple[tuple[int, str], dict]] = {}
        for tree in ("py", "extmod"):
            root = self.repo / tree
            if not root.is_dir():
                continue
            for path in sorted(root.glob("mod*.c")):
                text = path.read_text(encoding="utf-8", errors="replace")
                modules = REGISTER_RE.findall(text)
                for mod_name in modules:
                    table = self.pick_globals_table(text, mod_name)
                    names = [
                        n
                        for n in self.extract_table_names(text, table)
                        if n != "__name__"
                    ]
                    entry = {
                        "name": mod_name,
                        "source": path.relative_to(self.repo).as_posix(),
                        "table": table,
                        "prefix": self.prefix_for(mod_name),
                        "exports": names,
                    }
                    score = self._source_score(path, mod_name)
                    prev = best.get(mod_name)
                    if prev is None or score < prev[0]:
                        best[mod_name] = (score, entry)
        return [best[k][1] for k in sorted(best)]

    def collect_defined(self) -> dict[str, list[str]]:
        found: dict[str, list[str]] = defaultdict(list)
        for rel in self.search_paths:
            root = self.repo / rel
            if not root.exists():
                continue
            paths = (
                [root]
                if root.is_file()
                else [
                    p
                    for p in root.rglob("*")
                    if p.is_file() and p.suffix in {".rs", ".h", ".c"}
                ]
            )
            for path in paths:
                text = path.read_text(encoding="utf-8", errors="replace")
                rel_path = path.relative_to(self.repo).as_posix()
                for m in SYM_DEF_RE.finditer(text):
                    sym = m.group(1) or m.group(2)
                    if sym and rel_path not in found[sym]:
                        found[sym].append(rel_path)
                for m in SYM_ANY_RE.finditer(text):
                    sym = m.group(0)
                    if f"fn {sym}" in text or f" {sym}(" in text:
                        if rel_path not in found[sym]:
                            found[sym].append(rel_path)
        return found

    def _symbol_status(
        self, sym: str, defined: dict[str, list[str]]
    ) -> tuple[str, str]:
        locs = defined.get(sym, [])
        if not locs:
            return "missing", ""
        statuses = []
        for rel in locs:
            path = self.repo / rel
            if path.suffix == ".rs":
                statuses.append(classify_rs(path)[0])
            else:
                statuses.append("done")
        if all(s == "stub" for s in statuses):
            return "stub", ", ".join(locs)
        if any(s == "partial" for s in statuses):
            return "partial", ", ".join(locs)
        if any(s == "stub" for s in statuses) and not any(s == "done" for s in statuses):
            return "stub", ", ".join(locs)
        if any(s == "done" for s in statuses):
            return "present", ", ".join(locs)
        return statuses[0], ", ".join(locs)

    def scan(self) -> tuple[dict[str, list[PmSymbolResult]], list[PmSymbolResult]]:
        defined = self.collect_defined()
        modules: dict[str, list[PmSymbolResult]] = {}
        for mod in self.discover_modules():
            results = []
            for n in mod["exports"]:
                sym = mod["prefix"] + n
                status, detail = self._symbol_status(sym, defined)
                results.append(
                    PmSymbolResult(
                        module=mod["name"],
                        name=n,
                        symbol=sym,
                        status=status,
                        detail=detail,
                    )
                )
            modules[mod["name"]] = results

        infra = []
        for sym in DEFAULT_INFRA:
            status, detail = self._symbol_status(sym, defined)
            infra.append(
                PmSymbolResult(
                    module="infra",
                    name=sym.removeprefix("pm_mpy_"),
                    symbol=sym,
                    status=status,
                    detail=detail,
                )
            )
        return modules, infra
