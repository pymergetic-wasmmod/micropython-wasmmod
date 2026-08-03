//! rewrite of mpy-cross/mpconfigport.h
// symmetry: done
//! mpy-cross port overrides (reference tree differs from host `py_rs::mpconfig` defaults).

/// `MICROPY_ALLOC_PATH_MAX`
pub const ALLOC_PATH_MAX: usize = libc::PATH_MAX as usize;

/// Persistent code: cross compiles only (`MICROPY_PERSISTENT_CODE_LOAD` / `_LOAD_NATIVE` = 0).
pub const PERSISTENT_CODE_LOAD: bool = false;
pub const PERSISTENT_CODE_LOAD_NATIVE: bool = false;
pub const PERSISTENT_CODE_SAVE: bool = true;
pub const PERSISTENT_CODE_SAVE_FILE: bool = true;

/// Native emitters enabled for cross-compilation output.
pub const EMIT_X64: bool = true;
pub const EMIT_X86: bool = true;
pub const EMIT_THUMB: bool = true;
pub const EMIT_INLINE_THUMB: bool = true;
pub const EMIT_ARM: bool = true;
pub const EMIT_XTENSA: bool = true;
pub const EMIT_INLINE_XTENSA: bool = true;
pub const EMIT_XTENSAWIN: bool = true;
pub const EMIT_RV32: bool = true;
pub const EMIT_INLINE_RV32: bool = true;
pub const EMIT_NATIVE_DEBUG: bool = true;

pub const DYNAMIC_COMPILER: bool = true;
pub const COMP_CONST_FOLDING: bool = true;
pub const COMP_MODULE_CONST: bool = true;
pub const COMP_CONST: bool = true;
pub const COMP_CONST_FLOAT: bool = true;
pub const COMP_DOUBLE_TUPLE_ASSIGN: bool = true;
pub const COMP_TRIPLE_TUPLE_ASSIGN: bool = true;
pub const COMP_RETURN_IF_EXPR: bool = true;

pub const READER_POSIX: bool = true;
/// Cross tool has no VM loop (`MICROPY_ENABLE_RUNTIME` = 0).
pub const ENABLE_RUNTIME: bool = false;
pub const ENABLE_GC: bool = true;
pub const STACK_CHECK: bool = true;
pub const HELPER_LEXER_UNIX: bool = true;
pub const LONGINT_IMPL_MPZ: bool = true;
pub const ENABLE_SOURCE_LINE: bool = true;
pub const ENABLE_DOC_STRING: bool = false;
pub const ERROR_REPORTING_DETAILED: bool = true;
pub const WARNINGS: bool = true;
pub const FLOAT_IMPL_DOUBLE: bool = true;
pub const CPYTHON_COMPAT: bool = true;
pub const USE_INTERNAL_PRINTF: bool = false;

pub const PY_FSTRINGS: bool = true;
pub const PY_TSTRINGS: bool = true;
pub const PY_BUILTINS_STR_UNICODE: bool = true;

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "aarch64",
))]
pub const GCREGS_SETJMP: bool = false;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "aarch64",
)))]
pub const GCREGS_SETJMP: bool = true;

/// `MICROPY_MODULE___FILE__`
pub const MODULE_FILE: bool = false;
pub const PY_ARRAY: bool = false;
pub const PY_ATTRTUPLE: bool = false;
pub const PY_COLLECTIONS: bool = false;
pub const PY_MATH: bool = COMP_CONST_FLOAT;
pub const PY_MATH_CONSTANTS: bool = COMP_CONST_FLOAT;
pub const PY_CMATH: bool = false;
pub const PY_GC: bool = false;
pub const PY_IO: bool = false;
pub const PY_SYS: bool = false;

/// `mp_off_t` on LP64 Unix hosts.
pub type OffT = i64;
