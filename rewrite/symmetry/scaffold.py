"""Scaffold *_rs stub shadows and Cargo module trees (never touches originals)."""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from .checker import SymmetryChecker
from .models import FullReport, MirrorReport, MirrorSpec

# Rust keywords / reserved that cannot be bare module names.
_RUST_KEYWORDS = frozenset(
    {
        "as",
        "async",
        "await",
        "break",
        "const",
        "continue",
        "crate",
        "dyn",
        "else",
        "enum",
        "extern",
        "false",
        "fn",
        "for",
        "if",
        "impl",
        "in",
        "let",
        "loop",
        "match",
        "mod",
        "move",
        "mut",
        "pub",
        "ref",
        "return",
        "self",
        "Self",
        "static",
        "struct",
        "super",
        "trait",
        "true",
        "type",
        "unsafe",
        "use",
        "where",
        "while",
        "abstract",
        "become",
        "box",
        "do",
        "final",
        "macro",
        "override",
        "priv",
        "try",
        "typeof",
        "unsized",
        "virtual",
        "yield",
        "gen",
    }
)

# Binary crates: stem `main` is the Cargo bin root, not a lib module.
_PORT_CRATES = frozenset({"unix", "qemu"})


@dataclass
class ScaffoldResult:
    created: list[str] = field(default_factory=list)
    skipped: list[str] = field(default_factory=list)
    mod_files: list[str] = field(default_factory=list)
    cargo_files: list[str] = field(default_factory=list)


def rust_mod_name(name: str) -> str:
    """Map a filesystem stem to a Rust module identifier."""
    name = name.replace("-", "_")
    if name in _RUST_KEYWORDS:
        return f"r#{name}"
    return name


def stub_text(ref_files: list[str], *, is_bin_main: bool = False, port: str | None = None) -> str:
    refs = " + ".join(ref_files) if ref_files else "(unknown)"
    lines = [
        f"//! rewrite of {refs}",
        "// symmetry: stub",
        "",
    ]
    if is_bin_main:
        msg = "metalpython" if port != "qemu" else "metalpython-qemu"
        lines += [
            "fn main() {",
            f'    println!("{msg}");',
            "}",
            "",
        ]
    return "\n".join(lines)


def _mod_rs_body(dirs: list[str], files: list[str], *, crate_doc: str | None = None) -> str:
    lines: list[str] = []
    if crate_doc:
        lines.append(crate_doc)
        lines.append("#![allow(dead_code, unused_imports, unused_variables)]")
        lines.append("")
    for d in sorted(dirs):
        # Skip invalid / build-output directory names in module trees.
        if d.startswith("build") or "-" in d:
            continue
        lines.append(f"pub mod {rust_mod_name(d)};")
    for f in sorted(files):
        mod = rust_mod_name(f)
        # File name already matches when not a keyword; keywords need #[path].
        if f in _RUST_KEYWORDS or "-" in f:
            lines.append(f'#[path = "{f}.rs"]')
            lines.append(f"pub mod {mod};")
        else:
            lines.append(f"pub mod {mod};")
    if not lines:
        lines.append("// (no modules yet)")
    lines.append("")
    return "\n".join(lines)


def build_module_tree(
    stems: list[str], *, skip_stems: set[str] | None = None
) -> dict[str, tuple[list[str], list[str]]]:
    """
    Map directory ('' = crate root) → (subdir_names, file_stems).
    """
    skip = skip_stems or set()
    dirs_at: dict[str, set[str]] = defaultdict(set)
    files_at: dict[str, set[str]] = defaultdict(set)

    for stem in stems:
        if stem in skip:
            continue
        parts = stem.split("/")
        # Ensure intermediate directories exist in the tree.
        for i in range(len(parts) - 1):
            parent = "/".join(parts[:i])
            child = parts[i]
            dirs_at[parent].add(child)
        parent = "/".join(parts[:-1])
        files_at[parent].add(parts[-1])

    # Union of all directory keys that need a mod.rs / lib.rs
    keys = set(dirs_at) | set(files_at)
    # Also ensure parents of nested dirs appear even if empty of files
    for key in list(keys):
        if not key:
            continue
        parts = key.split("/")
        for i in range(len(parts)):
            keys.add("/".join(parts[:i]))

    out: dict[str, tuple[list[str], list[str]]] = {}
    for key in sorted(keys):
        out[key] = (sorted(dirs_at.get(key, ())), sorted(files_at.get(key, ())))
    return out


class Scaffolder:
    """Create stub .rs shadows + Cargo workspace wiring from a symmetry scan."""

    def __init__(self, checker: SymmetryChecker):
        self.checker = checker
        self.repo = checker.repo

    def run(
        self,
        report: FullReport | None = None,
        *,
        force: bool = False,
        write_cargo: bool = True,
        write_mods: bool = True,
    ) -> ScaffoldResult:
        if report is None:
            report = self.checker.scan(
                include_pm=False, compare_shas=False, compare_progress=False
            )
        result = ScaffoldResult()
        specs = {s.name: s for s in self.checker.config.mirrors() if s.enabled}

        for mirror in report.mirrors:
            spec = specs.get(mirror.name)
            if spec is None or not spec.enabled:
                continue
            self._scaffold_mirror(mirror, spec, result, force=force)

        if write_mods:
            for mirror in report.mirrors:
                spec = specs.get(mirror.name)
                if spec is None or not spec.enabled:
                    continue
                self._write_module_files(mirror, spec, result)

        if write_cargo:
            self._write_cargo(report, result)

        return result

    def _scaffold_mirror(
        self,
        mirror: MirrorReport,
        spec: MirrorSpec,
        result: ScaffoldResult,
        *,
        force: bool,
    ) -> None:
        for st in mirror.stems:
            path = self.repo / st.shadow
            is_bin_main = mirror.name in _PORT_CRATES and st.stem == "main"
            if path.is_file() and not force:
                result.skipped.append(st.shadow)
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            text = stub_text(st.ref_files, is_bin_main=is_bin_main, port=mirror.name)
            path.write_text(text, encoding="utf-8")
            result.created.append(st.shadow)

    def _write_module_files(
        self, mirror: MirrorReport, spec: MirrorSpec, result: ScaffoldResult
    ) -> None:
        shadow_root = self.repo / spec.shadow
        shadow_root.mkdir(parents=True, exist_ok=True)
        stems = [st.stem for st in mirror.stems]
        # Preserve helper .rs files that aren't discovered stems (e.g. raise.rs).
        discovered = {Path(s).name for s in stems}
        for extra in sorted(shadow_root.glob("*.rs")):
            stem = extra.stem
            if stem in ("lib", "main", "mod"):
                continue
            # Path-included helper fragments (not standalone crate modules).
            if stem.endswith("_impl") or stem.endswith("_exports"):
                continue
            if stem not in discovered and stem not in stems:
                stems.append(stem)
        skip = {"main"} if mirror.name in _PORT_CRATES else set()
        tree = build_module_tree(stems, skip_stems=skip)

        for dir_key, (dirs, files) in tree.items():
            if dir_key == "":
                path = shadow_root / "lib.rs"
                doc = (
                    f"//! MetalPython rewrite of MicroPython `{spec.ref}/`.\n"
                    f"//! Shadow tree: `{spec.shadow}/`."
                )
                body = _mod_rs_body(dirs, files, crate_doc=doc)
            else:
                path = shadow_root / dir_key / "mod.rs"
                body = _mod_rs_body(dirs, files)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body, encoding="utf-8")
            result.mod_files.append(path.relative_to(self.repo).as_posix())

        # Port crates: ensure main.rs exists (binary entry).
        if mirror.name in _PORT_CRATES:
            main = shadow_root / "main.rs"
            if not main.is_file():
                refs = next(
                    (st.ref_files for st in mirror.stems if st.stem == "main"),
                    [f"{spec.ref}/main.c"],
                )
                main.write_text(
                    stub_text(refs, is_bin_main=True, port=mirror.name),
                    encoding="utf-8",
                )
                result.created.append(main.relative_to(self.repo).as_posix())

    def _write_cargo(self, report: FullReport, result: ScaffoldResult) -> None:
        members = []
        for mirror in report.mirrors:
            if not any(s.name == mirror.name and s.enabled for s in self.checker.config.mirrors()):
                continue
            members.append(mirror.shadow)
            self._write_crate_toml(mirror, result)

        root = self.repo / "Cargo.toml"
        member_lines = ",\n".join(f'    "{m}"' for m in members)
        root.write_text(
            "\n".join(
                [
                    "# MetalPython — Rust rewrite workspace (shadow *_rs trees).",
                    "# Upstream MicroPython C/Python trees are reference-only; do not edit them.",
                    "[workspace]",
                    'resolver = "2"',
                    "members = [",
                    member_lines,
                    "]",
                    "",
                    "[workspace.package]",
                    'edition = "2021"',
                    'license = "MIT"',
                    "",
                    "[workspace.dependencies]",
                    'py_rs = { path = "py_rs" }',
                    'shared_rs = { path = "shared_rs" }',
                    'extmod_rs = { path = "extmod_rs" }',
                    "",
                ]
            ),
            encoding="utf-8",
        )
        result.cargo_files.append("Cargo.toml")

    def _write_crate_toml(self, mirror: MirrorReport, result: ScaffoldResult) -> None:
        root = self.repo / mirror.shadow
        root.mkdir(parents=True, exist_ok=True)
        pkg = mirror.shadow.replace("/", "_")
        is_port = mirror.name in _PORT_CRATES

        lines = [
            "[package]",
            f'name = "{pkg}"',
            'version = "0.0.0"',
            "edition.workspace = true",
            "license.workspace = true",
            "publish = false",
            "",
            "[lib]",
            f'name = "{pkg}"',
            'path = "lib.rs"',
            "",
        ]
        if is_port:
            bin_name = "metalpython" if mirror.name == "unix" else f"metalpython-{mirror.name}"
            lines += [
                "[[bin]]",
                f'name = "{bin_name}"',
                'path = "main.rs"',
                "",
            ]

        deps: list[str] = []
        if mirror.name == "shared":
            deps.append("py_rs = { workspace = true }")
        elif mirror.name == "extmod":
            deps.append("py_rs = { workspace = true }")
        elif mirror.name in _PORT_CRATES:
            deps += [
                "py_rs = { workspace = true }",
                "shared_rs = { workspace = true }",
                "extmod_rs = { workspace = true }",
            ]
        if deps:
            lines.append("[dependencies]")
            lines.extend(deps)
            lines.append("")

        path = root / "Cargo.toml"
        path.write_text("\n".join(lines), encoding="utf-8")
        result.cargo_files.append(path.relative_to(self.repo).as_posix())
