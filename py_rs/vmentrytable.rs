//! rewrite of py/vmentrytable.h
// symmetry: done
//!
//! In the C VM (`py/vm.c`), computed gotos dispatch via `entry_table[opcode]`.
//! The Rust VM (`py_rs/vm.rs`) uses a `match` on opcodes instead; this module
//! documents the label mapping and provides lookup helpers for debug tooling.

use crate::bc0;
use crate::mpconfig;

/// Name of the default fallback label (`entry_default` in C).
pub const ENTRY_DEFAULT: &str = "entry_default";

/// Prefix for per-opcode computed-goto labels in C (`entry_MP_BC_*`).
pub fn entry_label_name(opcode: u8) -> &'static str {
    match opcode {
        bc0::LOAD_CONST_FALSE => "entry_MP_BC_LOAD_CONST_FALSE",
        bc0::LOAD_CONST_NONE => "entry_MP_BC_LOAD_CONST_NONE",
        bc0::LOAD_CONST_TRUE => "entry_MP_BC_LOAD_CONST_TRUE",
        bc0::LOAD_CONST_SMALL_INT => "entry_MP_BC_LOAD_CONST_SMALL_INT",
        bc0::LOAD_CONST_STRING => "entry_MP_BC_LOAD_CONST_STRING",
        bc0::LOAD_CONST_OBJ => "entry_MP_BC_LOAD_CONST_OBJ",
        bc0::LOAD_NULL => "entry_MP_BC_LOAD_NULL",
        bc0::LOAD_FAST_N => "entry_MP_BC_LOAD_FAST_N",
        bc0::LOAD_DEREF => "entry_MP_BC_LOAD_DEREF",
        bc0::LOAD_NAME => "entry_MP_BC_LOAD_NAME",
        bc0::LOAD_GLOBAL => "entry_MP_BC_LOAD_GLOBAL",
        bc0::LOAD_ATTR => "entry_MP_BC_LOAD_ATTR",
        bc0::LOAD_METHOD => "entry_MP_BC_LOAD_METHOD",
        bc0::LOAD_SUPER_METHOD => "entry_MP_BC_LOAD_SUPER_METHOD",
        bc0::LOAD_BUILD_CLASS => "entry_MP_BC_LOAD_BUILD_CLASS",
        bc0::LOAD_SUBSCR => "entry_MP_BC_LOAD_SUBSCR",
        bc0::STORE_FAST_N => "entry_MP_BC_STORE_FAST_N",
        bc0::STORE_DEREF => "entry_MP_BC_STORE_DEREF",
        bc0::STORE_NAME => "entry_MP_BC_STORE_NAME",
        bc0::STORE_GLOBAL => "entry_MP_BC_STORE_GLOBAL",
        bc0::STORE_ATTR => "entry_MP_BC_STORE_ATTR",
        bc0::STORE_SUBSCR => "entry_MP_BC_STORE_SUBSCR",
        bc0::DELETE_FAST => "entry_MP_BC_DELETE_FAST",
        bc0::DELETE_DEREF => "entry_MP_BC_DELETE_DEREF",
        bc0::DELETE_NAME => "entry_MP_BC_DELETE_NAME",
        bc0::DELETE_GLOBAL => "entry_MP_BC_DELETE_GLOBAL",
        bc0::DUP_TOP => "entry_MP_BC_DUP_TOP",
        bc0::DUP_TOP_TWO => "entry_MP_BC_DUP_TOP_TWO",
        bc0::POP_TOP => "entry_MP_BC_POP_TOP",
        bc0::ROT_TWO => "entry_MP_BC_ROT_TWO",
        bc0::ROT_THREE => "entry_MP_BC_ROT_THREE",
        bc0::JUMP => "entry_MP_BC_JUMP",
        bc0::POP_JUMP_IF_TRUE => "entry_MP_BC_POP_JUMP_IF_TRUE",
        bc0::POP_JUMP_IF_FALSE => "entry_MP_BC_POP_JUMP_IF_FALSE",
        bc0::JUMP_IF_TRUE_OR_POP => "entry_MP_BC_JUMP_IF_TRUE_OR_POP",
        bc0::JUMP_IF_FALSE_OR_POP => "entry_MP_BC_JUMP_IF_FALSE_OR_POP",
        bc0::SETUP_WITH => "entry_MP_BC_SETUP_WITH",
        bc0::WITH_CLEANUP => "entry_MP_BC_WITH_CLEANUP",
        bc0::UNWIND_JUMP => "entry_MP_BC_UNWIND_JUMP",
        bc0::SETUP_EXCEPT => "entry_MP_BC_SETUP_EXCEPT",
        bc0::SETUP_FINALLY => "entry_MP_BC_SETUP_FINALLY",
        bc0::END_FINALLY => "entry_MP_BC_END_FINALLY",
        bc0::GET_ITER => "entry_MP_BC_GET_ITER",
        bc0::GET_ITER_STACK => "entry_MP_BC_GET_ITER_STACK",
        bc0::FOR_ITER => "entry_MP_BC_FOR_ITER",
        bc0::POP_EXCEPT_JUMP => "entry_MP_BC_POP_EXCEPT_JUMP",
        bc0::BUILD_TUPLE => "entry_MP_BC_BUILD_TUPLE",
        bc0::BUILD_LIST => "entry_MP_BC_BUILD_LIST",
        bc0::BUILD_MAP => "entry_MP_BC_BUILD_MAP",
        bc0::STORE_MAP => "entry_MP_BC_STORE_MAP",
        bc0::BUILD_SET => "entry_MP_BC_BUILD_SET",
        bc0::BUILD_SLICE => "entry_MP_BC_BUILD_SLICE",
        bc0::STORE_COMP => "entry_MP_BC_STORE_COMP",
        bc0::UNPACK_SEQUENCE => "entry_MP_BC_UNPACK_SEQUENCE",
        bc0::UNPACK_EX => "entry_MP_BC_UNPACK_EX",
        bc0::MAKE_FUNCTION => "entry_MP_BC_MAKE_FUNCTION",
        bc0::MAKE_FUNCTION_DEFARGS => "entry_MP_BC_MAKE_FUNCTION_DEFARGS",
        bc0::MAKE_CLOSURE => "entry_MP_BC_MAKE_CLOSURE",
        bc0::MAKE_CLOSURE_DEFARGS => "entry_MP_BC_MAKE_CLOSURE_DEFARGS",
        bc0::CALL_FUNCTION => "entry_MP_BC_CALL_FUNCTION",
        bc0::CALL_FUNCTION_VAR_KW => "entry_MP_BC_CALL_FUNCTION_VAR_KW",
        bc0::CALL_METHOD => "entry_MP_BC_CALL_METHOD",
        bc0::CALL_METHOD_VAR_KW => "entry_MP_BC_CALL_METHOD_VAR_KW",
        bc0::RETURN_VALUE => "entry_MP_BC_RETURN_VALUE",
        bc0::RAISE_LAST => "entry_MP_BC_RAISE_LAST",
        bc0::RAISE_OBJ => "entry_MP_BC_RAISE_OBJ",
        bc0::RAISE_FROM => "entry_MP_BC_RAISE_FROM",
        bc0::YIELD_VALUE => "entry_MP_BC_YIELD_VALUE",
        bc0::YIELD_FROM => "entry_MP_BC_YIELD_FROM",
        bc0::IMPORT_NAME => "entry_MP_BC_IMPORT_NAME",
        bc0::IMPORT_FROM => "entry_MP_BC_IMPORT_FROM",
        bc0::IMPORT_STAR => "entry_MP_BC_IMPORT_STAR",
        op if op >= bc0::LOAD_CONST_SMALL_INT_MULTI
            && op < bc0::LOAD_CONST_SMALL_INT_MULTI + bc0::LOAD_CONST_SMALL_INT_MULTI_NUM =>
        {
            "entry_MP_BC_LOAD_CONST_SMALL_INT_MULTI"
        }
        op if op >= bc0::LOAD_FAST_MULTI
            && op < bc0::LOAD_FAST_MULTI + bc0::LOAD_FAST_MULTI_NUM =>
        {
            "entry_MP_BC_LOAD_FAST_MULTI"
        }
        op if op >= bc0::STORE_FAST_MULTI
            && op < bc0::STORE_FAST_MULTI + bc0::STORE_FAST_MULTI_NUM =>
        {
            "entry_MP_BC_STORE_FAST_MULTI"
        }
        op if op >= bc0::UNARY_OP_MULTI && op < bc0::UNARY_OP_MULTI + bc0::UNARY_OP_MULTI_NUM => {
            "entry_MP_BC_UNARY_OP_MULTI"
        }
        op if op >= bc0::BINARY_OP_MULTI
            && op < bc0::BINARY_OP_MULTI + bc0::BINARY_OP_MULTI_NUM =>
        {
            "entry_MP_BC_BINARY_OP_MULTI"
        }
        _ => ENTRY_DEFAULT,
    }
}

/// Whether `opcode` has an explicit C VM entry (not `entry_default`).
pub fn has_explicit_entry(opcode: u8) -> bool {
    entry_label_name(opcode) != ENTRY_DEFAULT
}

/// Build the 256-entry dispatch table metadata matching `vmentrytable.h`.
pub fn entry_table() -> [&'static str; 256] {
    let mut table = [ENTRY_DEFAULT; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        *slot = entry_label_name(i as u8);
    }
    table
}

/// True when the host build uses computed goto (`MICROPY_OPT_COMPUTED_GOTO`).
pub const USES_COMPUTED_GOTO: bool = mpconfig::OPT_COMPUTED_GOTO;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_opcodes_map_to_named_entries() {
        assert_eq!(
            entry_label_name(bc0::RETURN_VALUE),
            "entry_MP_BC_RETURN_VALUE"
        );
        assert!(has_explicit_entry(bc0::LOAD_NAME));
    }

    #[test]
    fn table_has_256_slots() {
        assert_eq!(entry_table().len(), 256);
    }
}
