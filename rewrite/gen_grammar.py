#!/usr/bin/env python3
"""Generate py_rs/grammar.rs from py/grammar.h (MicroPython parse tables)."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GRAMMAR_H = ROOT / "py" / "grammar.h"
OUT = ROOT / "py_rs" / "grammar.rs"

# Host mpconfig defaults (must match py_rs/mpconfig.rs).
CONFIG = {
    "MICROPY_PY_ASYNC_AWAIT": True,
    "MICROPY_PY_ASSIGN_EXPR": True,
    "MICROPY_PY_BUILTINS_SLICE": True,
    "MICROPY_PY_BUILTINS_SET": True,
}

RULE_ACT_OR = 0x10
RULE_ACT_AND = 0x20
RULE_ACT_ALLOW_IDENT = 0x40
RULE_ACT_ADD_BLANK = 0x80
RULE_ACT_LIST = 0x30

RULE_ARG_TOK = 0x1000
RULE_ARG_RULE = 0x2000
RULE_ARG_OPT_RULE = 0x3000

ACTION_MACROS = {
    "one_or_more": RULE_ACT_LIST | 2,
    "list": RULE_ACT_LIST | 1,
    "list_with_end": RULE_ACT_LIST | 3,
}


def pascal_from_token(name: str) -> str:
    parts = name.split("_")
    if parts[0] == "DEL":
        return "Del" + "".join(p.capitalize() for p in parts[1:])
    if parts[0] == "KW":
        return "Kw" + "".join(p.capitalize() for p in parts[1:])
    if parts[0] == "OP":
        return "Op" + "".join(p.capitalize() for p in parts[1:])
    if name == "ELLIPSIS":
        return "Ellipsis"
    if name == "INTEGER":
        return "Integer"
    if name == "FLOAT_OR_IMAG":
        return "FloatOrImag"
    if name == "STRING":
        return "String"
    if name == "BYTES":
        return "Bytes"
    if name == "NAME":
        return "Name"
    if name == "NEWLINE":
        return "Newline"
    if name == "INDENT":
        return "Indent"
    if name == "DEDENT":
        return "Dedent"
    if name == "END":
        return "End"
    raise ValueError(f"unknown token name {name!r}")


def snake_rule(name: str) -> str:
    return name


def pascal_rule(name: str) -> str:
    return "".join(p.capitalize() for p in name.split("_"))


@dataclass
class RuleDef:
    name: str
    has_compile: bool
    action_expr: str
    args: list[str] = field(default_factory=list)


def expand_action(expr: str) -> int:
    expr = expr.strip()
    for macro, val in ACTION_MACROS.items():
        if expr == macro:
            return val
    m = re.fullmatch(r"or\((\d+)\)", expr)
    if m:
        return RULE_ACT_OR | int(m.group(1))
    m = re.fullmatch(r"and\((\d+)\)", expr)
    if m:
        return RULE_ACT_AND | int(m.group(1))
    m = re.fullmatch(r"and_ident\((\d+)\)", expr)
    if m:
        return RULE_ACT_AND | int(m.group(1)) | RULE_ACT_ALLOW_IDENT
    m = re.fullmatch(r"and_blank\((\d+)\)", expr)
    if m:
        return RULE_ACT_AND | int(m.group(1)) | RULE_ACT_ADD_BLANK
    raise ValueError(f"unknown action {expr!r}")


def parse_args(arg_str: str) -> list[str]:
    if not arg_str.strip():
        return []
    return [a.strip() for a in arg_str.split(",") if a.strip()]


def encode_arg(arg: str, rule_ids: dict[str, int]) -> str:
    m = re.fullmatch(r"tok\((\w+)\)", arg)
    if m:
        kind = pascal_from_token(m.group(1))
        return f"RULE_ARG_TOK | TokenKind::{kind} as u16"
    m = re.fullmatch(r"rule\((\w+)\)", arg)
    if m:
        rid = rule_ids[m.group(1)]
        return str(RULE_ARG_RULE | rid)
    m = re.fullmatch(r"opt_rule\((\w+)\)", arg)
    if m:
        rid = rule_ids[m.group(1)]
        return str(RULE_ARG_OPT_RULE | rid)
    raise ValueError(f"unknown arg {arg!r}")


def preprocess(text: str) -> str:
    lines: list[str] = []
    stack: list[bool] = [True]
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("#if"):
            cond = line[3:].strip()
            active = bool(eval_cond(cond))
            stack.append(active and stack[-1])
            continue
        if line.startswith("#else"):
            stack[-1] = (not stack[-1]) and (len(stack) > 1 and all(stack[:-1]))
            continue
        if line.startswith("#endif"):
            stack.pop()
            continue
        if stack[-1]:
            lines.append(raw)
    return "\n".join(lines)


def eval_cond(cond: str) -> bool:
    cond = cond.strip()
    return bool(CONFIG.get(cond, False))


def collect_rules(text: str) -> list[RuleDef]:
    rules: list[RuleDef] = []
    pat_nc = re.compile(r"^DEF_RULE_NC\((\w+),\s*([^,]+),\s*(.*)\)\s*$")
    pat = re.compile(r"^DEF_RULE\((\w+),\s*([^,]+),\s*([^,]+),\s*(.*)\)\s*$")
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("DEF_RULE_NC"):
            m = pat_nc.match(line)
            if not m:
                raise ValueError(f"cannot parse rule line: {line}")
            name, action, rest = m.group(1), m.group(2).strip(), m.group(3)
            rules.append(
                RuleDef(
                    name=name,
                    has_compile=False,
                    action_expr=action,
                    args=parse_args(rest),
                )
            )
        elif line.startswith("DEF_RULE"):
            m = pat.match(line)
            if not m:
                raise ValueError(f"cannot parse rule line: {line}")
            name, _comp, action, rest = m.group(1), m.group(2).strip(), m.group(3).strip(), m.group(4)
            rules.append(
                RuleDef(
                    name=name,
                    has_compile=True,
                    action_expr=action,
                    args=parse_args(rest),
                )
            )
    return rules


def build_rule_ids(rules: list[RuleDef]) -> dict[str, int]:
    ids: dict[str, int] = {}
    idx = 0
    for r in rules:
        if r.has_compile:
            ids[r.name] = idx
            idx += 1
    ids["const_object"] = idx
    idx += 1
    for r in rules:
        if not r.has_compile:
            ids[r.name] = idx
            idx += 1
    return ids


def emit(rules: list[RuleDef], rule_ids: dict[str, int]) -> str:
    ordered: list[RuleDef] = []
    for r in rules:
        if r.has_compile:
            ordered.append(r)
    ordered.append(RuleDef("const_object", False, "0", []))
    for r in rules:
        if not r.has_compile:
            ordered.append(r)

    act_table: list[int] = []
    combined: list[str] = []
    offsets: list[int] = []
    names: list[str] = []
    off = 0
    first_above_255: int | None = None

    for r in ordered:
        if r.name == "const_object":
            act = 0
        else:
            act = expand_action(r.action_expr)
        act_table.append(act)
        offsets.append(off & 0xFF)
        if off >= 0x100 and first_above_255 is None:
            first_above_255 = rule_ids[r.name]
        names.append(r.name)
        for arg in r.args:
            combined.append(encode_arg(arg, rule_ids))
        off += len(r.args)

    if first_above_255 is None:
        first_above_255 = len(rule_ids)

    lines: list[str] = [
        "//! rewrite of py/grammar.h",
        "// symmetry: done",
        "",
        "use crate::lexer::TokenKind;",
        "",
        "#[repr(u8)]",
        "#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]",
        "pub enum Rule {",
    ]
    for name, rid in sorted(rule_ids.items(), key=lambda kv: kv[1]):
        lines.append(f"    {pascal_rule(name)} = {rid},")
    lines.append("}")
    lines.append("")
    lines.append("pub const RULE_ACT_ARG_MASK: u8 = 0x0f;")
    lines.append("pub const RULE_ACT_KIND_MASK: u8 = 0x30;")
    lines.append("pub const RULE_ACT_ALLOW_IDENT: u8 = 0x40;")
    lines.append("pub const RULE_ACT_ADD_BLANK: u8 = 0x80;")
    lines.append("pub const RULE_ACT_OR: u8 = 0x10;")
    lines.append("pub const RULE_ACT_AND: u8 = 0x20;")
    lines.append("pub const RULE_ACT_LIST: u8 = 0x30;")
    lines.append("")
    lines.append("pub const RULE_ARG_KIND_MASK: u16 = 0xf000;")
    lines.append("pub const RULE_ARG_ARG_MASK: u16 = 0x0fff;")
    lines.append("pub const RULE_ARG_TOK: u16 = 0x1000;")
    lines.append("pub const RULE_ARG_RULE: u16 = 0x2000;")
    lines.append("pub const RULE_ARG_OPT_RULE: u16 = 0x3000;")
    lines.append("")
    lines.append(f"pub const FIRST_RULE_WITH_OFFSET_ABOVE_255: u8 = {first_above_255};")
    lines.append("")
    lines.append("pub const RULE_ACT_TABLE: &[u8] = &[")
    for v in act_table:
        lines.append(f"    {v},")
    lines.append("];")
    lines.append("")
    lines.append("pub const RULE_ARG_COMBINED_TABLE: &[u16] = &[")
    for v in combined:
        lines.append(f"    {v},")
    lines.append("];")
    lines.append("")
    lines.append("pub const RULE_ARG_OFFSET_TABLE: &[u8] = &[")
    for v in offsets:
        lines.append(f"    {v},")
    lines.append("];")
    lines.append("")
    if True:
        lines.append("pub const RULE_NAME_TABLE: &[&str] = &[")
        for n in names:
            lines.append(f'    "{n}",')
        lines.append("];")
    lines.append("")
    lines.append("pub fn rule_arg_offset(rule_id: u8) -> usize {")
    lines.append("    let mut off = RULE_ARG_OFFSET_TABLE[rule_id as usize] as usize;")
    lines.append("    if rule_id >= FIRST_RULE_WITH_OFFSET_ABOVE_255 {")
    lines.append("        off |= 0x100;")
    lines.append("    }")
    lines.append("    off")
    lines.append("}")
    lines.append("")
    lines.append("pub fn rule_arg(rule_id: u8) -> &'static [u16] {")
    lines.append("    let off = rule_arg_offset(rule_id);")
    lines.append("    let end = if (rule_id as usize + 1) < RULE_ARG_OFFSET_TABLE.len() {")
    lines.append("        rule_arg_offset(rule_id + 1)")
    lines.append("    } else {")
    lines.append("        RULE_ARG_COMBINED_TABLE.len()")
    lines.append("    };")
    lines.append("    &RULE_ARG_COMBINED_TABLE[off..end]")
    lines.append("}")
    lines.append("")
    return "\n".join(lines) + "\n"


def main() -> int:
    text = preprocess(GRAMMAR_H.read_text())
    rules = collect_rules(text)
    rule_ids = build_rule_ids(rules)
    OUT.write_text(emit(rules, rule_ids))
    print(f"wrote {OUT} ({len(rule_ids)} rules)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
