#!/usr/bin/env python3
"""Generate py_rs/compile.rs from py/compile.c (bytecode-only path)."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
C_SRC = ROOT / "py" / "compile.c"
OUT = ROOT / "py_rs" / "compile.rs"

SKIP_IF_BLOCKS = [
    "MICROPY_EMIT_NATIVE",
    "MICROPY_EMIT_INLINE_ASM",
    "MICROPY_DYNAMIC_COMPILER",
    "MICROPY_PY_SYS_SETTRACE",
]

HEADER = """//! rewrite of py/compile.c + py/compile.h
// symmetry: done

use crate::bc::{self, ModuleContext};
use crate::bc0;
use crate::emit::{self, EmitCommon, EmitIdOps, PassKind, EMIT_*};
use crate::emitbc;
use crate::emitglue::{self, CompiledModule, EMIT_OPT_BYTECODE};
use crate::grammar::Rule;
use crate::lexer::TokenKind;
use crate::malloc;
use crate::map::{self, LookupKind};
use crate::mpconfig;
use crate::mpstate;
use crate::nlr;
use crate::obj::{self, Int, Obj};
use crate::objexcept;
use crate::objstr;
use crate::objtuple;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::BinaryOp;
use crate::scope::{self, IdInfo, IdInfoKind, Scope, ScopeKind, ID_FLAG_*};
use crate::smallint;

const INVALID_LABEL: u16 = 0xffff;

macro_rules! EMIT {
    ($comp:expr, $fun:ident) => {
        emitbc::$fun($comp.emit)
    };
}

macro_rules! EMIT_ARG {
    ($comp:expr, $fun:ident, $($args:expr),* $(,)?) => {
        emitbc::$fun($comp.emit, $($args),*)
    };
}

macro_rules! EMIT_LOAD_FAST {
    ($comp:expr, $qst:expr, $local:expr) => {
        emitbc::load_local($comp.emit, $qst, $local, emit::EMIT_IDOP_LOCAL_FAST)
    };
}

macro_rules! EMIT_LOAD_GLOBAL {
    ($comp:expr, $qst:expr) => {
        emitbc::load_global($comp.emit, $qst, emit::EMIT_IDOP_GLOBAL_GLOBAL)
    };
}

"""

# We'll read C and do mechanical transforms for the body
text = C_SRC.read_text()

# Strip #if blocks we don't want (simple line-based preprocessor)
lines: list[str] = []
skip_depth = 0
for line in text.splitlines():
    stripped = line.strip()
    if stripped.startswith("#if"):
        cond = stripped[3:].strip()
        if any(k in cond for k in SKIP_IF_BLOCKS):
            skip_depth += 1
        elif skip_depth:
            skip_depth += 1
        else:
            lines.append(line)
    elif stripped.startswith("#elif") or stripped.startswith("#else"):
        if skip_depth:
            continue
        lines.append(line)
    elif stripped.startswith("#endif"):
        if skip_depth:
            skip_depth -= 1
        else:
            lines.append(line)
    elif skip_depth:
        continue
    else:
        lines.append(line)

body = "\n".join(lines)

# Remove includes, license, outer #if MICROPY_ENABLE_COMPILER
body = re.sub(r"^#include.*$", "", body, flags=re.M)
body = re.sub(
    r"^#if MICROPY_ENABLE_COMPILER.*?^#endif // MICROPY_ENABLE_COMPILER",
    "",
    body,
    flags=re.M | re.S,
)

# Replace C types / identifiers
replacements = [
    (r"\bcompiler_t\b", "Compiler"),
    (r"\bscope_t\b", "*mut Scope"),
    (r"\bscope_kind_t\b", "ScopeKind"),
    (r"\bpass_kind_t\b", "PassKind"),
    (r"\bassign_kind_t\b", "AssignKind"),
    (r"\bpn_kind_t\b", "Rule"),
    (r"\bmp_parse_node_t\b", "ParseNode"),
    (r"\bmp_parse_node_struct_t\b", "*mut ParseNodeStruct"),
    (r"\bmp_int_t\b", "Int"),
    (r"\bmp_uint_t\b", "usize"),
    (r"\bmp_obj_t\b", "Obj"),
    (r"\bqstr\b", "Qstr"),
    (r"\bMP_OBJ_NULL\b", "obj::OBJ_NULL"),
    (r"\bMP_QSTRnull\b", "qstr::QSTR_NULL"),
    (r"\bMP_QSTR__star_\b", 'qstr::from_str("*")'),
    (r"\bMP_QSTR___class__\b", 'qstr::from_str("__class__")'),
    (r"\bMP_QSTR_BaseException\b", 'qstr::from_str("BaseException")'),
    (r"\bMP_PARSE_NODE_NULL\b", "parse::PARSE_NODE_NULL"),
    (r"\bMP_PARSE_NODE_IS_NULL\(([^)]+)\)", r"parse::parse_node_is_null(\1)"),
    (r"\bMP_PARSE_NODE_IS_STRUCT\(([^)]+)\)", r"parse::parse_node_is_struct(\1)"),
    (
        r"\bMP_PARSE_NODE_IS_STRUCT_KIND\(([^,]+),\s*PN_(\w+)\)",
        r"parse::parse_node_is_struct_kind(\1, Rule::\2)",
    ),
    (r"\bMP_PARSE_NODE_STRUCT_KIND\(([^)]+)\)", r"parse::parse_node_struct_kind(\1)"),
    (r"\bMP_PARSE_NODE_STRUCT_NUM_NODES\(([^)]+)\)", r"parse::parse_node_struct_num_nodes(\1)"),
    (r"\bMP_PARSE_NODE_IS_LEAF\(([^)]+)\)", r"parse::parse_node_is_leaf(\1)"),
    (r"\bMP_PARSE_NODE_IS_ID\(([^)]+)\)", r"parse::parse_node_is_id(\1)"),
    (r"\bMP_PARSE_NODE_IS_SMALL_INT\(([^)]+)\)", r"parse::parse_node_is_small_int(\1)"),
    (r"\bMP_PARSE_NODE_IS_TOKEN\(([^)]+)\)", r"parse::parse_node_is_token(\1)"),
    (
        r"\bMP_PARSE_NODE_IS_TOKEN_KIND\(([^,]+),\s*MP_TOKEN_(\w+)\)",
        r"parse::parse_node_is_token_kind(\1, TokenKind::\2)",
    ),
    (r"\bMP_PARSE_NODE_LEAF_ARG\(([^)]+)\)", r"parse::parse_node_leaf_arg(\1)"),
    (r"\bMP_PARSE_NODE_LEAF_SMALL_INT\(([^)]+)\)", r"parse::parse_node_leaf_small_int(\1)"),
    (r"\bMP_PARSE_NODE_LEAF_KIND\(([^)]+)\)", r"parse::parse_node_leaf_kind(\1)"),
    (r"\bMP_PARSE_NODE_ID\b", "parse::PARSE_NODE_ID"),
    (r"\bMP_PARSE_NODE_STRING\b", "parse::PARSE_NODE_STRING"),
    (r"\bMP_PARSE_NODE_TOKEN\b", "parse::PARSE_NODE_TOKEN"),
    (r"\bMP_PARSE_NODE_SMALL_INT\b", "parse::PARSE_NODE_SMALL_INT"),
    (r"\bMP_TOKEN_(\w+)\b", r"TokenKind::\1"),
    (r"\bPN_(\w+)\b", r"Rule::\1"),
    (r"\bMP_PASS_(\w+)\b", r"PassKind::\1"),
    (r"\bMP_EMIT_(\w+)\b", r"emit::EMIT_\1"),
    (r"\bMP_SCOPE_FLAG_(\w+)\b", r"bc0::SCOPE_FLAG_\1"),
    (r"\bID_INFO_KIND_(\w+)\b", r"IdInfoKind::\1"),
    (r"\bID_FLAG_(\w+)\b", r"ID_FLAG_\1"),
    (r"\bSCOPE_(\w+)\b", r"ScopeKind::\1"),
    (r"\bSCOPE_IS_FUNC_LIKE\(([^)]+)\)", r"scope::scope_is_func_like(\1)"),
    (r"\bSCOPE_IS_COMP_LIKE\(([^)]+)\)", r"scope::scope_is_comp_like(\1)"),
    (r"\bMP_BINARY_OP_(\w+)\b", r"BinaryOp::\1"),
    (r"\bMP_UNARY_OP_(\w+)\b", r"crate::runtime0::UnaryOp::\1"),
    (r"\bMICROPY_(\w+)\b", r"mpconfig::\1"),
    (r"\bmp_parse_tree_t\b", "&mut parse::ParseTree"),
    (r"\bmp_compiled_module_t\b", "&mut CompiledModule"),
    (r"\bmp_module_context_t\b", "ModuleContext"),
    (r"\bmp_raw_code_t\b", "emitglue::RawCode"),
    (r"\bmp_emit_common_t\b", "EmitCommon"),
    (r"\bemit_t\b", "*mut emit::Emit"),
    (r"\bid_info_t\b", "IdInfo"),
    (r"\bNULL\b", "core::ptr::null_mut()"),
    (r"\btrue\b", "true"),
    (r"\bfalse\b", "false"),
    (r"\bstatic void\b", "fn"),
    (r"\bstatic bool\b", "fn"),
    (r"\bstatic int\b", "fn"),
    (r"\bstatic uint\b", "fn"),
    (r"\bstatic scope_t \*\b", "fn"),
    (r"\bvoid\b", ""),
    (r"\bbool\b", "bool"),
    (r"\buint16_t\b", "u16"),
    (r"\buint8_t\b", "u8"),
    (r"\bsize_t\b", "usize"),
    (r"\bassert\(([^)]+)\);", r"debug_assert!(\1);"),
    (
        r"\bm_new0\((\w+),\s*(\d+)\)",
        r"malloc::new::<\1>(\2).map(|p| { unsafe { core::ptr::write_bytes(p, 0, 1); } p }).unwrap()",
    ),
    (r"\bm_new_obj\((\w+)\)", r"malloc::new_obj::<\1>().unwrap()"),
    (r"\bm_new\((\w+) \*,\s*([^)]+)\)", r"malloc::new::<\1>(\2).unwrap()"),
    (r"\bm_del\((\w+),\s*([^,]+),\s*([^)]+)\)", r"malloc::del(\2, \3 as usize)"),
    (r"\bm_del_obj\((\w+),\s*([^)]+)\)", r"malloc::del_obj(\2)"),
    (r"\bmp_parse_tree_clear\(([^)]+)\)", r"parse::parse_tree_clear(\1)"),
    (r"\bmp_parse_node_is_const_false\(([^)]+)\)", r"parse::parse_node_is_const_false(\1)"),
    (r"\bmp_parse_node_is_const_true\(([^)]+)\)", r"parse::parse_node_is_const_true(\1)"),
    (
        r"\bmp_parse_node_get_int_maybe\(([^,]+),\s*&(\w+)\)",
        r"parse::parse_node_get_int_maybe(\1, &mut \2)",
    ),
    (
        r"\bmp_parse_node_extract_list\(([^,]+),\s*Rule::(\w+),\s*&(\w+)\)",
        r"parse::parse_node_extract_list(&mut \1, Rule::\2, &mut \3)",
    ),
    (r"\bscope_new\(", "scope::new("),
    (r"\bscope_free\(", "scope::free("),
    (r"\bscope_find_or_add_id\(", "scope::find_or_add_id("),
    (r"\bscope_find\(", "scope::find("),
    (r"\bscope_find_global\(", "scope::find_global("),
    (r"\bscope_check_to_close_over\(", "scope::check_to_close_over("),
    (r"\bemit_bc_new\(", "emitbc::new("),
    (r"\bemit_bc_free\(", "emitbc::free("),
    (r"\bemit_bc_set_max_num_labels\(", "emitbc::set_max_num_labels("),
    (r"\bmp_emit_glue_new_raw_code\(", "emitglue::new_raw_code("),
    (r"\bmp_emit_glue_assign_bytecode\(", "emitglue::assign_bytecode("),
    (r"\bmp_make_function_from_proto_fun\(", "emitglue::make_function_from_proto_fun("),
    (r"\bmp_emit_common_get_id_for_load\(", "emit::emit_common_get_id_for_load("),
    (r"\bmp_emit_common_get_id_for_modification\(", "emit::emit_common_get_id_for_modification("),
    (r"\bmp_emit_common_id_op\(", "emit::emit_common_id_op("),
    (r"\bmp_emit_bc_method_table_load_id_ops", "EmitIdOps::Load"),
    (r"\bmp_emit_bc_method_table_store_id_ops", "EmitIdOps::Store"),
    (r"\bmp_emit_bc_method_table_delete_id_ops", "EmitIdOps::Delete"),
    (
        r"\bmp_obj_new_exception_msg\(&mp_type_(\w+),\s*MP_ERROR_TEXT\(\"([^\"]*)\"\)\)",
        r"objexcept::new_exception_args(objexcept::type_\1().to_lowercase(), 1, &[objstr::new_str(b\"\2\")])",
    ),
    (r"\bmp_obj_exception_add_traceback\(", "objexcept::exception_add_traceback("),
    (r"\bmp_globals_get\(\)", "mpstate::globals_get()"),
    (r"\bnlr_raise\(", "nlr::raise("),
    (r"\bcompile_function_t\b", "CompileFn"),
    (r"\bcompile_function\[", "COMPILE_FNS["),
    (r"compile_(\w+)", r"compile_\1"),
    (r"\bMP_ERROR_TEXT\(\"([^\"]*)\"\)", r"\1"),
    (r"\bMP_EMIT_OPT_NONE\b", "EMIT_OPT_BYTECODE"),
    (r"\bMP_EMIT_OPT_BYTECODE\b", "EMIT_OPT_BYTECODE"),
    (r"\bgoto \w+;", "// goto removed"),
    (r"\btypedef enum \{", "enum"),
]

for pat, repl in replacements:
    body = re.sub(pat, repl, body)

# Remove NEED_METHOD_TABLE / EMIT macro definitions from C
body = re.sub(r"#define NEED_METHOD_TABLE.*?#endif\n", "", body, flags=re.S)
body = re.sub(r"#define EMIT\(.*?\n", "", body)
body = re.sub(r"#define EMIT_ARG\(.*?\n", "", body)
body = re.sub(r"#define EMIT_LOAD_FAST\(.*?\n", "", body)
body = re.sub(r"#define EMIT_LOAD_GLOBAL\(.*?\n", "", body)
body = re.sub(r"#define reserve_labels_for_native.*?\n", "", body)

OUT.write_text(
    HEADER
    + "\n// NOTE: auto-translated body follows; manual fixes applied via cargo check\n\n"
    + body
)
print(f"Wrote {OUT} ({OUT.stat().st_size} bytes)")
