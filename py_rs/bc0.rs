//! rewrite of py/bc0.h
// symmetry: done

use crate::runtime0;

/// Scope flags used in bytecode prelude sig (`MP_SCOPE_FLAG_*`).
pub const SCOPE_FLAG_ALL_SIG: u32 = 0x0f;
pub const SCOPE_FLAG_GENERATOR: u32 = 0x01;
pub const SCOPE_FLAG_VARKEYWORDS: u32 = 0x02;
pub const SCOPE_FLAG_VARARGS: u32 = 0x04;
pub const SCOPE_FLAG_DEFKWARGS: u32 = 0x08;
pub const SCOPE_FLAG_REFGLOBALS: u32 = 0x10;
pub const SCOPE_FLAG_HASCONSTS: u32 = 0x20;

pub const MASK_FORMAT: u8 = 0xf0;
pub const MASK_EXTRA_BYTE: u8 = 0x9e;

pub const FORMAT_BYTE: u8 = 0;
pub const FORMAT_QSTR: u8 = 1;
pub const FORMAT_VAR_UINT: u8 = 2;
pub const FORMAT_OFFSET: u8 = 3;

pub const fn format(op: u8) -> u8 {
    (0x000003a4u32 >> (2 * ((op >> 4) as u32))) as u8 & 3
}

pub const BASE_RESERVED: u8 = 0x00;
pub const BASE_QSTR_O: u8 = 0x10;
pub const BASE_VINT_E: u8 = 0x20;
pub const BASE_VINT_O: u8 = 0x30;
pub const BASE_JUMP_E: u8 = 0x40;
pub const BASE_BYTE_O: u8 = 0x50;
pub const BASE_BYTE_E: u8 = 0x60;
pub const LOAD_CONST_SMALL_INT_MULTI: u8 = 0x70;
pub const LOAD_FAST_MULTI: u8 = 0xb0;
pub const STORE_FAST_MULTI: u8 = 0xc0;
pub const UNARY_OP_MULTI: u8 = 0xd0;
pub const BINARY_OP_MULTI: u8 = 0xd7;

pub const LOAD_CONST_SMALL_INT_MULTI_NUM: u8 = 64;
pub const LOAD_CONST_SMALL_INT_MULTI_EXCESS: u8 = 16;
pub const LOAD_FAST_MULTI_NUM: u8 = 16;
pub const STORE_FAST_MULTI_NUM: u8 = 16;
pub const UNARY_OP_MULTI_NUM: u8 = runtime0::UNARY_OP_NUM_BYTECODE;
pub const BINARY_OP_MULTI_NUM: u8 = runtime0::BINARY_OP_NUM_BYTECODE;

pub const LOAD_CONST_FALSE: u8 = BASE_BYTE_O + 0x00;
pub const LOAD_CONST_NONE: u8 = BASE_BYTE_O + 0x01;
pub const LOAD_CONST_TRUE: u8 = BASE_BYTE_O + 0x02;
pub const LOAD_CONST_SMALL_INT: u8 = BASE_VINT_E + 0x02;
pub const LOAD_CONST_STRING: u8 = BASE_QSTR_O + 0x00;
pub const LOAD_CONST_OBJ: u8 = BASE_VINT_E + 0x03;
pub const LOAD_NULL: u8 = BASE_BYTE_O + 0x03;

pub const LOAD_FAST_N: u8 = BASE_VINT_E + 0x04;
pub const LOAD_DEREF: u8 = BASE_VINT_E + 0x05;
pub const LOAD_NAME: u8 = BASE_QSTR_O + 0x01;
pub const LOAD_GLOBAL: u8 = BASE_QSTR_O + 0x02;
pub const LOAD_ATTR: u8 = BASE_QSTR_O + 0x03;
pub const LOAD_METHOD: u8 = BASE_QSTR_O + 0x04;
pub const LOAD_SUPER_METHOD: u8 = BASE_QSTR_O + 0x05;
pub const LOAD_BUILD_CLASS: u8 = BASE_BYTE_O + 0x04;
pub const LOAD_SUBSCR: u8 = BASE_BYTE_O + 0x05;

pub const STORE_FAST_N: u8 = BASE_VINT_E + 0x06;
pub const STORE_DEREF: u8 = BASE_VINT_E + 0x07;
pub const STORE_NAME: u8 = BASE_QSTR_O + 0x06;
pub const STORE_GLOBAL: u8 = BASE_QSTR_O + 0x07;
pub const STORE_ATTR: u8 = BASE_QSTR_O + 0x08;
pub const STORE_SUBSCR: u8 = BASE_BYTE_O + 0x06;

pub const DELETE_FAST: u8 = BASE_VINT_E + 0x08;
pub const DELETE_DEREF: u8 = BASE_VINT_E + 0x09;
pub const DELETE_NAME: u8 = BASE_QSTR_O + 0x09;
pub const DELETE_GLOBAL: u8 = BASE_QSTR_O + 0x0a;

pub const DUP_TOP: u8 = BASE_BYTE_O + 0x07;
pub const DUP_TOP_TWO: u8 = BASE_BYTE_O + 0x08;
pub const POP_TOP: u8 = BASE_BYTE_O + 0x09;
pub const ROT_TWO: u8 = BASE_BYTE_O + 0x0a;
pub const ROT_THREE: u8 = BASE_BYTE_O + 0x0b;

pub const UNWIND_JUMP: u8 = BASE_JUMP_E + 0x00;
pub const JUMP: u8 = BASE_JUMP_E + 0x02;
pub const POP_JUMP_IF_TRUE: u8 = BASE_JUMP_E + 0x03;
pub const POP_JUMP_IF_FALSE: u8 = BASE_JUMP_E + 0x04;
pub const JUMP_IF_TRUE_OR_POP: u8 = BASE_JUMP_E + 0x05;
pub const JUMP_IF_FALSE_OR_POP: u8 = BASE_JUMP_E + 0x06;
pub const SETUP_WITH: u8 = BASE_JUMP_E + 0x07;
pub const SETUP_EXCEPT: u8 = BASE_JUMP_E + 0x08;
pub const SETUP_FINALLY: u8 = BASE_JUMP_E + 0x09;
pub const POP_EXCEPT_JUMP: u8 = BASE_JUMP_E + 0x0a;
pub const FOR_ITER: u8 = BASE_JUMP_E + 0x0b;
pub const WITH_CLEANUP: u8 = BASE_BYTE_O + 0x0c;
pub const END_FINALLY: u8 = BASE_BYTE_O + 0x0d;
pub const GET_ITER: u8 = BASE_BYTE_O + 0x0e;
pub const GET_ITER_STACK: u8 = BASE_BYTE_O + 0x0f;

pub const BUILD_TUPLE: u8 = BASE_VINT_E + 0x0a;
pub const BUILD_LIST: u8 = BASE_VINT_E + 0x0b;
pub const BUILD_MAP: u8 = BASE_VINT_E + 0x0c;
pub const STORE_MAP: u8 = BASE_BYTE_E + 0x02;
pub const BUILD_SET: u8 = BASE_VINT_E + 0x0d;
pub const BUILD_SLICE: u8 = BASE_VINT_E + 0x0e;
pub const STORE_COMP: u8 = BASE_VINT_E + 0x0f;
pub const UNPACK_SEQUENCE: u8 = BASE_VINT_O + 0x00;
pub const UNPACK_EX: u8 = BASE_VINT_O + 0x01;

pub const RETURN_VALUE: u8 = BASE_BYTE_E + 0x03;
pub const RAISE_LAST: u8 = BASE_BYTE_E + 0x04;
pub const RAISE_OBJ: u8 = BASE_BYTE_E + 0x05;
pub const RAISE_FROM: u8 = BASE_BYTE_E + 0x06;
pub const YIELD_VALUE: u8 = BASE_BYTE_E + 0x07;
pub const YIELD_FROM: u8 = BASE_BYTE_E + 0x08;

pub const MAKE_FUNCTION: u8 = BASE_VINT_O + 0x02;
pub const MAKE_FUNCTION_DEFARGS: u8 = BASE_VINT_O + 0x03;
pub const MAKE_CLOSURE: u8 = BASE_VINT_E + 0x00;
pub const MAKE_CLOSURE_DEFARGS: u8 = BASE_VINT_E + 0x01;
pub const CALL_FUNCTION: u8 = BASE_VINT_O + 0x04;
pub const CALL_FUNCTION_VAR_KW: u8 = BASE_VINT_O + 0x05;
pub const CALL_METHOD: u8 = BASE_VINT_O + 0x06;
pub const CALL_METHOD_VAR_KW: u8 = BASE_VINT_O + 0x07;

pub const IMPORT_NAME: u8 = BASE_QSTR_O + 0x0b;
pub const IMPORT_FROM: u8 = BASE_QSTR_O + 0x0c;
pub const IMPORT_STAR: u8 = BASE_BYTE_E + 0x09;
