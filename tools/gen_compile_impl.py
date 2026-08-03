#!/usr/bin/env python3
"""Translate py/compile.c static functions to Rust in compile_impl.inc."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = (ROOT / "py" / "compile.c").read_text()
OUT = ROOT / "py_rs" / "compile_impl.inc"

SKIP_REGIONS = (
    "MICROPY_EMIT_NATIVE",
    "MICROPY_EMIT_INLINE_ASM",
    "compile_scope_inline_asm",
    "compile_built_in_decorator",
    "compile_viper_type_annotation",
    "mp_compile_allow_top_level_await",
)

def strip_if_blocks(text: str) -> str:
    lines = []
    skip = 0
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("#if"):
            if any(k in s for k in ("MICROPY_EMIT_NATIVE", "MICROPY_EMIT_INLINE_ASM", "MICROPY_DYNAMIC_COMPILER")):
                skip += 1
            elif skip:
                skip += 1
            else:
                pass
        elif s.startswith("#elif") or s.startswith("#else"):
            pass
        elif s.startswith("#endif"):
            if skip:
                skip -= 1
        elif skip:
            continue
        else:
            if s.startswith("#") and not s.startswith("#define MP_PARSE"):
                continue
            lines.append(line)
    return "\n".join(lines)


def pascal(s: str) -> str:
    return "".join(p.capitalize() for p in s.split("_"))


def translate_line(line: str) -> str | None:
    if not line.strip() or line.strip().startswith("//"):
        return line
    if line.strip().startswith("#"):
        return None
    s = line
    subs = [
        (r"\bcompiler_t \*comp\b", "comp: &mut Compiler"),
        (r"\bcompiler_t \*comp,", "comp: &mut Compiler,"),
        (r"\bmp_parse_node_struct_t \*pns\b", "pns: *mut ParseNodeStruct"),
        (r"\bmp_parse_node_t pn\b", "pn: ParseNode"),
        (r"\bmp_parse_node_t \*([^,\)]+)", r"pn_\1: &mut ParseNode"),
        (r"\bqstr qst\b", "qst: Qstr"),
        (r"\bqstr q_base\b", "q_base: &mut Qstr"),
        (r"\buint emit_options\b", "emit_options: u16"),
        (r"\bpass_kind_t pass\b", "pass: PassKind"),
        (r"\bscope_kind_t kind\b", "kind: ScopeKind"),
        (r"\bassign_kind_t assign_kind\b", "assign_kind: AssignKind"),
        (r"\bpn_kind_t pn_list_kind\b", "list_rule: Rule"),
        (r"\bpn_kind_t pn_name\b", "pn_name: Rule"),
        (r"\bpn_kind_t pn_star\b", "pn_star: Rule"),
        (r"\bpn_kind_t pn_dbl_star\b", "pn_dbl_star: Rule"),
        (r"\bMP_OBJ_NULL\b", "obj::OBJ_NULL"),
        (r"\bMP_QSTRnull\b", "qstr::QSTR_NULL"),
        (r"\bMP_QSTR__star_\b", 'qstr::from_str("*")'),
        (r"\bMP_QSTR___class__\b", 'qstr::from_str("__class__")'),
        (r"\bMP_QSTR_BaseException\b", 'qstr::from_str("BaseException")'),
        (r"\bMP_QSTR_\b", 'qstr::from_str("")'),
        (r"\bMP_PARSE_NODE_NULL\b", "parse::PARSE_NODE_NULL"),
        (r"\bMP_PARSE_NODE_IS_NULL\(", "parse::parse_node_is_null("),
        (r"\bMP_PARSE_NODE_IS_STRUCT\(", "parse::parse_node_is_struct("),
        (r"\bMP_PARSE_NODE_IS_STRUCT_KIND\(([^,]+),\s*PN_(\w+)\)", lambda m: f"parse::parse_node_is_struct_kind({m.group(1)}, Rule::{pascal(m.group(2))})"),
        (r"\bMP_PARSE_NODE_STRUCT_KIND\(", "parse::parse_node_struct_kind("),
        (r"\bMP_PARSE_NODE_STRUCT_NUM_NODES\(", "parse::parse_node_struct_num_nodes("),
        (r"\bMP_PARSE_NODE_IS_ID\(", "parse::parse_node_is_id("),
        (r"\bMP_PARSE_NODE_IS_SMALL_INT\(", "parse::parse_node_is_small_int("),
        (r"\bMP_PARSE_NODE_IS_TOKEN_KIND\(([^,]+),\s*MP_TOKEN_(\w+)\)", r"parse::parse_node_is_token_kind(\1, TokenKind::\2)"),
        (r"\bMP_PARSE_NODE_IS_TOKEN\(", "parse::parse_node_is_token("),
        (r"\bMP_PARSE_NODE_LEAF_ARG\(", "parse::parse_node_leaf_arg("),
        (r"\bMP_PARSE_NODE_LEAF_SMALL_INT\(", "parse::parse_node_leaf_small_int("),
        (r"\bMP_PARSE_NODE_TESTLIST_COMP_HAS_COMP_FOR\(", "parse_node_testlist_comp_has_comp_for("),
        (r"\bpns->nodes\[(\d+)\]", r"parse::parse_node_struct_node(pns, \1)"),
        (r"\bpns_(\w+)->nodes\[(\d+)\]", r"parse::parse_node_struct_node(pns_\1, \2)"),
        (r"\bcomp->", "comp."),
        (r"\bMP_PASS_(\w+)\b", r"PassKind::\1"),
        (r"\bMP_EMIT_(\w+)\b", r"emit::EMIT_\1"),
        (r"\bMP_SCOPE_FLAG_(\w+)\b", r"bc0::SCOPE_FLAG_\1 as u16"),
        (r"\bID_INFO_KIND_(\w+)\b", r"IdInfoKind::\1"),
        (r"\bID_FLAG_(\w+)\b", r"scope::ID_FLAG_\1"),
        (r"\bASSIGN_(\w+)\b", r"AssignKind::\1"),
        (r"\bMP_BINARY_OP_(\w+)\b", r"BinaryOp::\1"),
        (r"\bMP_UNARY_OP_(\w+)\b", r"UnaryOp::\1"),
        (r"\bMP_TOKEN_(\w+)\b", r"TokenKind::\1"),
        (r"\bPN_(\w+)\b", lambda m: f"Rule::{pascal(m.group(1))}"),
        (r"\bSCOPE_(\w+)\b", lambda m: f"ScopeKind::{m.group(1)}" if m.group(1) in ("MODULE","CLASS","LAMBDA","LIST_COMP","DICT_COMP","SET_COMP","GEN_EXPR","FUNCTION") else m.group(0)),
        (r"\bSCOPE_IS_FUNC_LIKE\(", "scope::scope_is_func_like("),
        (r"\bSCOPE_IS_COMP_LIKE\(", "scope::scope_is_comp_like("),
        (r"\bMICROPY_(\w+)\b", r"mpconfig::\1"),
        (r"\bEMIT\((\w+)\)", r"EMIT!(comp, \1)"),
        (r"\bEMIT_ARG\((\w+),\s*([^)]+)\)", r"EMIT_ARG!(comp, \1, \2)"),
        (r"\bEMIT_LOAD_FAST\(([^,]+),\s*([^)]+)\)", r"EMIT_LOAD_FAST!(comp, \1, \2)"),
        (r"\bEMIT_LOAD_GLOBAL\(([^)]+)\)", r"EMIT_LOAD_GLOBAL!(comp, \1)"),
        (r"\bMP_ERROR_TEXT\(\"([^\"]*)\"\)", r'b"\1"'),
        (r"\bassert\(([^)]+)\);", r"debug_assert!(\1);"),
        (r"\bcomp->compile_error != MP_OBJ_NULL\b", "comp_has_error(comp)"),
        (r"\bcomp->compile_error == MP_OBJ_NULL\b", "!comp_has_error(comp)"),
        (r"\bNULL\b", "core::ptr::null_mut()"),
        (r"\bINVALID_LABEL\b", "INVALID_LABEL"),
        (r"\btrue\b", "true"),
        (r"\bfalse\b", "false"),
        (r"\breturn;\s*$", "return;"),
        (r"\breturn ([^;]+);", r"return \1;"),
        (r"\bfor \(int i = 0; i < ([^;]+); i\+\+\)", r"for i in 0..\1"),
        (r"\bfor \(size_t i = 0; i < ([^;]+); \+\+i\)", r"for i in 0..\1"),
        (r"\bfor \(usize i = 0; i < ([^;]+); i\+\+\)", r"for i in 0..\1"),
        (r"\bint n = ", "let n = "),
        (r"\bsize_t n = ", "let n = "),
        (r"\buint ", "let "),
        (r"\bqstr ", "let "),
        (r"\bbool ", "let "),
    ]
    for pat, repl in subs:
        if callable(repl):
            s = re.sub(pat, repl, s)
        else:
            s = re.sub(pat, repl, s)
    # C blocks to Rust (naive)
    s = s.replace("{", "{").replace("} ", "} ")
    return s.rstrip()


def extract_functions(text: str) -> list[tuple[str, str, str]]:
    """Return list of (name, params, body)."""
    funcs = []
    pattern = re.compile(
        r"static\s+(?:void|bool|int|qstr)\s+(\w+)\s*\(([^)]*)\)\s*\{",
        re.M,
    )
    pos = 0
    while True:
        m = pattern.search(text, pos)
        if not m:
            break
        name, params = m.group(1), m.group(2)
        if any(x in name for x in SKIP_REGIONS):
            pos = m.end()
            continue
        start = m.end()
        depth = 1
        i = start
        while i < len(text) and depth:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
            i += 1
        body = text[start : i - 1]
        funcs.append((name, params, body))
        pos = i
    return funcs


def rust_fn(name: str, params: str, body: str) -> str:
    if name in (
        "compile_error_set_line",
        "compile_syntax_error",
        "comp_next_label",
        "compile_increase_except_level",
        "compile_decrease_except_level",
        "scope_new_and_link",
        "apply_to_single_or_list",
        "compile_generic_all_nodes",
        "compile_load_id",
        "compile_store_id",
        "compile_delete_id",
        "compile_generic_tuple",
        "emit_common_init",
        "emit_common_start_pass",
        "emit_common_populate_module_context",
        "compile_node",
    ):
        return ""
    ret = ""
    if name == "compile_scope":
        ret = " -> bool"
    elif name in ("compile_built_in_decorator",):
        return ""
    lines = []
    for raw in body.splitlines():
        t = translate_line(raw)
        if t is None:
            continue
        # skip gotos labels for now - comment
        if re.match(r"\s*\w+:\s*;?\s*$", t):
            t = "    // " + t.strip()
        if "goto " in t:
            t = "    // " + t.strip()
        lines.append("    " + t.lstrip())
    param_rust = translate_line(f"fn x({params})") or ""
    param_rust = param_rust.replace("fn x(", "").rstrip(")")
    return f"fn {name}({param_rust}){ret} {{\n" + "\n".join(lines) + "\n}\n\n"


def main() -> None:
    text = strip_if_blocks(SRC)
    funcs = extract_functions(text)
    parts = ["// Generated compile.c functions (bytecode-only)\n\n"]
    for name, params, body in funcs:
        parts.append(rust_fn(name, params, body))
    # fix mp_compile name
    out = "".join(parts)
    out = out.replace("fn mp_compile_to_raw_code", "pub fn mp_compile_to_raw_code")
    out = out.replace("fn mp_compile(", "// fn mp_compile(")
    out = re.sub(r"let \*mut Scope \*(\w+)", r"let mut \1", out)
    out = re.sub(r"\*mut Scope \*(\w+)", r"*mut Scope", out)
    OUT.write_text(out)
    print(f"wrote {len(funcs)} functions to {OUT}")

if __name__ == "__main__":
    main()
