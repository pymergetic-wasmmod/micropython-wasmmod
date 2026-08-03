//! rewrite of py/emitnative.c
//!
//! Host-complete for unix+x64 native/viper emission; remaining gaps are port/arch only:
//! - inline-asm `fun_asm_call` (`EMIT_INLINE_ASM` off on host)
//! - non-unix+x64 arch backends (ARM/Thumb/Xtensa/RV32 stubs exist; not wired for host)
//! - viper binary power (upstream C also type-errors `int**int`)
//!
//! Implemented on unix+x64 (`emitnative_impl.rs` + `emitnx64` backend):
//! - try/finally, with/async-with, pop_except_jump, start_except_handler
//! - generator yield/yield_from resume loop; native_gen_wrap e2e
//! - yield_from throw into native/bytecode delegates on host
//! - e2e `@micropython.native` / `@micropython.viper` const/int-add (`compile.rs` tests)
//! - ModuleContext C-layout obj/qstr tables; emitglue NativePy/Viper/Asm fun objects
//! - entry/exit, viper stack typing, locals/globals, unary/binary ops, viper subscr ptr load/store
// symmetry: done
#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::collapsible_else_if,
    clippy::identity_op,
    clippy::manual_inspect,
    clippy::needless_return,
    clippy::type_complexity
)]

use core::mem::{self, size_of};

use crate::asmbase::{self, MpAsmBase, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};
use crate::bc::{self, encode_uint, ModuleContext, ObjFunBc};
use crate::bc0::{self, SCOPE_FLAG_GENERATOR, SCOPE_FLAG_VARARGS, SCOPE_FLAG_VARKEYWORDS};
use crate::emit::{
    self, EmitCommon, PassKind, EMIT_ATTR_DELETE, EMIT_ATTR_LOAD, EMIT_ATTR_STORE,
    EMIT_BREAK_FROM_FOR, EMIT_BUILD_LIST, EMIT_BUILD_MAP, EMIT_BUILD_SET, EMIT_BUILD_SLICE,
    EMIT_BUILD_TUPLE, EMIT_IDOP_GLOBAL_GLOBAL, EMIT_IDOP_GLOBAL_NAME, EMIT_IDOP_LOCAL_DEREF,
    EMIT_IDOP_LOCAL_FAST, EMIT_IMPORT_FROM, EMIT_IMPORT_NAME, EMIT_IMPORT_STAR,
    EMIT_SETUP_BLOCK_EXCEPT, EMIT_SETUP_BLOCK_FINALLY, EMIT_SETUP_BLOCK_WITH, EMIT_SUBSCR_DELETE,
    EMIT_SUBSCR_LOAD, EMIT_SUBSCR_STORE, EMIT_YIELD_FROM, EMIT_YIELD_VALUE,
};
use crate::emitglue::{self, RawCode, RawCodeKind};
use crate::lexer::TokenKind;
use crate::malloc;
use crate::mpconfig;
use crate::nativeglue::{
    self, NATIVE_TYPE_BOOL, NATIVE_TYPE_INT, NATIVE_TYPE_OBJ, NATIVE_TYPE_PTR, NATIVE_TYPE_PTR16,
    NATIVE_TYPE_PTR32, NATIVE_TYPE_PTR8, NATIVE_TYPE_UINT,
};
use crate::nlr::NlrBuf;
use crate::obj::{self, Obj};
use crate::objfun;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::{self, BinaryOp, UnaryOp};
use crate::scope::{self, IdInfoKind, Scope, ScopeKind};
use crate::smallint;

/// Function table indices (`mp_fun_kind_t` / `MP_F_*`).
pub mod mp_f {
    pub const CONST_NONE_OBJ: u32 = 0;
    pub const CONST_FALSE_OBJ: u32 = 1;
    pub const CONST_TRUE_OBJ: u32 = 2;
    pub const CONVERT_OBJ_TO_NATIVE: u32 = 3;
    pub const CONVERT_NATIVE_TO_OBJ: u32 = 4;
    pub const NATIVE_SWAP_GLOBALS: u32 = 5;
    pub const LOAD_NAME: u32 = 6;
    pub const LOAD_GLOBAL: u32 = 7;
    pub const LOAD_BUILD_CLASS: u32 = 8;
    pub const LOAD_ATTR: u32 = 9;
    pub const LOAD_METHOD: u32 = 10;
    pub const LOAD_SUPER_METHOD: u32 = 11;
    pub const STORE_NAME: u32 = 12;
    pub const STORE_GLOBAL: u32 = 13;
    pub const STORE_ATTR: u32 = 14;
    pub const OBJ_SUBSCR: u32 = 15;
    pub const OBJ_IS_TRUE: u32 = 16;
    pub const UNARY_OP: u32 = 17;
    pub const BINARY_OP: u32 = 18;
    pub const BUILD_TUPLE: u32 = 19;
    pub const BUILD_LIST: u32 = 20;
    pub const BUILD_MAP: u32 = 21;
    pub const BUILD_SET: u32 = 22;
    pub const STORE_SET: u32 = 23;
    pub const LIST_APPEND: u32 = 24;
    pub const STORE_MAP: u32 = 25;
    pub const MAKE_FUNCTION_FROM_PROTO_FUN: u32 = 26;
    pub const NATIVE_CALL_FUNCTION_N_KW: u32 = 27;
    pub const CALL_METHOD_N_KW: u32 = 28;
    pub const CALL_METHOD_N_KW_VAR: u32 = 29;
    pub const NATIVE_GETITER: u32 = 30;
    pub const NATIVE_ITERNEXT: u32 = 31;
    pub const NLR_PUSH: u32 = 32;
    pub const NLR_POP: u32 = 33;
    pub const NATIVE_RAISE: u32 = 34;
    pub const IMPORT_NAME: u32 = 35;
    pub const IMPORT_FROM: u32 = 36;
    pub const IMPORT_ALL: u32 = 37;
    pub const NEW_SLICE: u32 = 38;
    pub const UNPACK_SEQUENCE: u32 = 39;
    pub const UNPACK_EX: u32 = 40;
    pub const DELETE_NAME: u32 = 41;
    pub const DELETE_GLOBAL: u32 = 42;
    pub const NEW_CLOSURE: u32 = 43;
    pub const ARG_CHECK_NUM_SIG: u32 = 44;
    pub const SETUP_CODE_STATE: u32 = 45;
    pub const SMALL_INT_FLOOR_DIVIDE: u32 = 46;
    pub const SMALL_INT_MODULO: u32 = 47;
    pub const NATIVE_YIELD_FROM: u32 = 48;
    pub const SETJMP: u32 = 49;
    /// Host unix+x64: `finish_throw(code_state, throw) -> VmReturnKind` (same slot as `SETJMP`).
    pub const NATIVE_GEN_FINISH_THROW: u32 = SETJMP;
    pub const NUMBER_OF: u32 = 50;
}

pub const EMIT_OPT_VIPER: u16 = emitglue::EMIT_OPT_NATIVE_PYTHON + 1;
pub const MP_SCOPE_FLAG_GENERATOR: u16 = bc0::SCOPE_FLAG_GENERATOR as u16;
pub const MP_SCOPE_FLAG_VARARGS: u16 = bc0::SCOPE_FLAG_VARARGS as u16;
pub const MP_SCOPE_FLAG_VARKEYWORDS: u16 = bc0::SCOPE_FLAG_VARKEYWORDS as u16;
pub const MP_SCOPE_FLAG_DEFKWARGS: u16 = bc0::SCOPE_FLAG_DEFKWARGS as u16;
pub const MP_SCOPE_FLAG_REFGLOBALS: u16 = 0x10;
pub const MP_SCOPE_FLAG_HASCONSTS: u16 = 0x20;
pub const MP_SCOPE_FLAG_VIPERRET_POS: u16 = 8;
pub const MP_VM_RETURN_NORMAL: usize = 0;
pub const MP_VM_RETURN_YIELD: usize = 1;
pub const MP_VM_RETURN_EXCEPTION: usize = 2;
pub const MP_OBJ_ITER_BUF_NSLOTS: i32 = 2;

const SIZEOF_NLR_BUF: usize = size_of::<NlrBuf>() / size_of::<usize>();

const SIZEOF_CODE_STATE: usize = size_of::<crate::bc::CodeStateNative>() / size_of::<usize>();
const OFFSETOF_CODE_STATE_STATE: usize = SIZEOF_CODE_STATE;
const OFFSETOF_CODE_STATE_FUN_BC: usize =
    core::mem::offset_of!(crate::bc::CodeStateNative, fun_bc) / size_of::<usize>();
const OFFSETOF_CODE_STATE_IP: usize =
    core::mem::offset_of!(crate::bc::CodeStateNative, ip) / size_of::<usize>();
const OFFSETOF_CODE_STATE_SP: usize =
    core::mem::offset_of!(crate::bc::CodeStateNative, sp) / size_of::<usize>();
const OFFSETOF_CODE_STATE_N_STATE: usize =
    core::mem::offset_of!(crate::bc::CodeStateNative, n_state) / size_of::<usize>();
const OFFSETOF_OBJ_FUN_BC_CONTEXT: usize =
    core::mem::offset_of!(crate::bc::ObjFunBc, context) / size_of::<usize>();
const OFFSETOF_OBJ_FUN_BC_CHILD_TABLE: usize =
    core::mem::offset_of!(crate::bc::ObjFunBc, child_table) / size_of::<usize>();
const OFFSETOF_OBJ_FUN_BC_BYTECODE: usize =
    core::mem::offset_of!(crate::bc::ObjFunBc, bytecode) / size_of::<usize>();
const OFFSETOF_MODULE_CONTEXT_OBJ_TABLE: usize =
    core::mem::offset_of!(crate::bc::ModuleContext, constants.obj_table) / size_of::<usize>();
const OFFSETOF_MODULE_CONTEXT_QSTR_TABLE: usize =
    core::mem::offset_of!(crate::bc::ModuleContext, constants.qstr_table) / size_of::<usize>();
const _: () = assert!(OFFSETOF_MODULE_CONTEXT_OBJ_TABLE == 3);
const OFFSETOF_MODULE_CONTEXT_GLOBALS: usize =
    (core::mem::offset_of!(crate::bc::ModuleContext, module)
        + core::mem::offset_of!(crate::bc::ObjModule, globals))
        / size_of::<usize>();

const NLR_BUF_IDX_RET_VAL: usize = 1;

const UNWIND_LABEL_UNUSED: u16 = 0x7fff;
const UNWIND_LABEL_DO_FINAL_UNWIND: u16 = 0x7ffe;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum StackInfoKind {
    Value,
    Reg,
    Imm,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum VType {
    PyObj = 0x00 | NATIVE_TYPE_OBJ as u8,
    Bool = 0x00 | NATIVE_TYPE_BOOL as u8,
    Int = 0x00 | NATIVE_TYPE_INT as u8,
    Uint = 0x00 | NATIVE_TYPE_UINT as u8,
    Ptr = 0x00 | NATIVE_TYPE_PTR as u8,
    Ptr8 = 0x00 | NATIVE_TYPE_PTR8 as u8,
    Ptr16 = 0x00 | NATIVE_TYPE_PTR16 as u8,
    Ptr32 = 0x00 | NATIVE_TYPE_PTR32 as u8,
    PtrNone = 0x50 | NATIVE_TYPE_PTR as u8,
    Unbound = 0x60 | NATIVE_TYPE_OBJ as u8,
    BuiltinCast = 0x70 | NATIVE_TYPE_OBJ as u8,
}

#[derive(Copy, Clone)]
struct StackInfo {
    vtype: VType,
    kind: StackInfoKind,
    u_reg: i32,
    u_imm: i64,
}

#[derive(Copy, Clone)]
struct ExcStackEntry {
    label: u16,
    is_finally: bool,
    unwind_label: u16,
    is_active: bool,
    /// Eval-stack index at SETUP_FINALLY (ctx_mgr or UNWIND_JUMP base).
    finally_sp_index: i16,
}

/// Arch-specific backend: register layout + generic assembler API.
pub trait NativeBackend: Copy + Clone + 'static {
    type Asm: AsmContext;
    const WORD_SIZE: i32;
    const REG_RET: i32;
    const REG_ARG_1: i32;
    const REG_ARG_2: i32;
    const REG_ARG_3: i32;
    const REG_ARG_4: i32;
    const REG_TEMP0: i32;
    const REG_TEMP1: i32;
    const REG_TEMP2: i32;
    const REG_LOCAL_1: i32;
    const REG_LOCAL_2: i32;
    const REG_LOCAL_3: i32;
    const REG_FUN_TABLE: i32;
    const REG_GENERATOR_STATE: i32;
    const REG_QSTR_TABLE: i32;
    const REG_LOCAL_LAST: i32;
    const NLR_BUF_IDX_LOCAL_1: usize;
    const N_X86: bool;
    const N_X64: bool;
    const N_THUMB: bool;
    const N_ARM: bool;
    const N_XTENSA: bool;
    const N_XTENSAWIN: bool;
    const N_RV32: bool;
    const N_DEBUG: bool;
    const N_NLR_SETJMP: bool;
    const REG_ZERO: i32;
    const REG_PARENT_RET: i32;
    const REG_PARENT_ARG_1: i32;
    const REG_PARENT_ARG_2: i32;
    const REG_PARENT_ARG_3: i32;
    const REG_PARENT_ARG_4: i32;
    const HAS_ASM_MOV_REG_QSTR: bool;
    const HAS_ASM_LOAD8_REG_REG_OFFSET: bool;
    const HAS_ASM_LOAD16_REG_REG_OFFSET: bool;
    const HAS_ASM_LOAD32_REG_REG_OFFSET: bool;
    const HAS_ASM_LOAD8_REG_REG_REG: bool;
    const HAS_ASM_LOAD16_REG_REG_REG: bool;
    const HAS_ASM_LOAD32_REG_REG_REG: bool;
    const HAS_ASM_STORE8_REG_REG_OFFSET: bool;
    const HAS_ASM_STORE16_REG_REG_OFFSET: bool;
    const HAS_ASM_STORE32_REG_REG_OFFSET: bool;
    const HAS_ASM_STORE8_REG_REG_REG: bool;
    const HAS_ASM_STORE16_REG_REG_REG: bool;
    const HAS_ASM_STORE32_REG_REG_REG: bool;
    const HAS_ASM_NOT_REG: bool;
    const REG_LOCAL_TABLE: &'static [i32];
    fn mp_f_n_args(_fun: u32) -> u8 {
        0
    }
    fn new_asm(max_labels: usize) -> Self::Asm;
    fn asm_base(as_: &mut Self::Asm) -> &mut MpAsmBase;
    fn end_pass(as_: &mut Self::Asm);
    fn entry(as_: &mut Self::Asm, num_locals: i32, name: Option<&str>);
    fn exit(as_: &mut Self::Asm);
    fn jump(as_: &mut Self::Asm, label: usize);
    fn jump_if_reg_zero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool);
    fn jump_if_reg_nonzero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool);
    fn jump_if_reg_eq(as_: &mut Self::Asm, reg1: i32, reg2: i32, label: usize);
    fn jump_reg(as_: &mut Self::Asm, reg: i32);
    fn call_ind(as_: &mut Self::Asm, idx: u32);
    fn mov_local_reg(as_: &mut Self::Asm, local: i32, reg: i32);
    fn mov_reg_imm(as_: &mut Self::Asm, reg: i32, imm: usize);
    fn mov_reg_qstr(as_: &mut Self::Asm, reg: i32, qst: Qstr);
    fn mov_reg_local(as_: &mut Self::Asm, reg: i32, local: i32);
    fn mov_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn mov_reg_local_addr(as_: &mut Self::Asm, reg: i32, local: i32);
    fn mov_reg_pcrel(as_: &mut Self::Asm, reg: i32, label: usize);
    fn not_reg(as_: &mut Self::Asm, reg: i32);
    fn neg_reg(as_: &mut Self::Asm, reg: i32);
    fn lsl_reg(as_: &mut Self::Asm, reg: i32);
    fn lsr_reg(as_: &mut Self::Asm, reg: i32);
    fn asr_reg(as_: &mut Self::Asm, reg: i32);
    fn lsl_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn lsr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn asr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn or_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn xor_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn and_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn add_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn sub_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn mul_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32);
    fn load_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32);
    fn load8_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32);
    fn load8_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32);
    fn load16_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32);
    fn load16_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32);
    fn load32_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32);
    fn load32_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32);
    fn store_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32);
    fn store8_reg_reg(as_: &mut Self::Asm, src: i32, base: i32);
    fn store8_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32);
    fn store16_reg_reg(as_: &mut Self::Asm, src: i32, base: i32);
    fn store16_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32);
    fn store32_reg_reg(as_: &mut Self::Asm, src: i32, base: i32);
    fn store32_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32);
    fn clr_reg(as_: &mut Self::Asm, reg: i32);
    fn mov_local_mp_obj_null(as_: &mut Self::Asm, local: i32, reg_temp: i32);
    fn setup_code_state_call(as_: &mut Self::Asm);
    fn mov_arg_to_reg(as_: &mut Self::Asm, arg_idx: i32, reg: i32);
    fn binary_op_setcc(as_: &mut Self::Asm, op_idx: usize, dest: i32, lhs: i32, rhs: i32);
    fn binary_op_shift(as_: &mut Self::Asm, op: BinaryOp, dest: i32, shift_reg: i32);
}

pub trait AsmContext {
    fn base_mut(&mut self) -> &mut MpAsmBase;
}

/// Native emitter state (`emit_t` in emitnative.c).
pub struct EmitNative<B: NativeBackend> {
    pub emit_common: *mut EmitCommon,
    pub error_slot: *mut Obj,
    pub label_slot: *mut usize,
    pub exit_label: usize,
    pub pass: i32,
    pub do_viper_types: bool,
    pub local_vtype: Vec<VType>,
    pub stack_info: Vec<StackInfo>,
    pub saved_stack_vtype: VType,
    pub exc_stack: Vec<ExcStackEntry>,
    pub prelude_offset: i32,
    pub prelude_ptr_index: i32,
    pub start_offset: i32,
    pub n_state: i32,
    pub code_state_start: u16,
    pub stack_start: u16,
    pub stack_size: i32,
    pub n_info: u16,
    pub n_cell: u16,
    pub scope: *mut Scope,
    pub as_: B::Asm,
    _backend: core::marker::PhantomData<B>,
}

fn emit_ref<B: NativeBackend>(emit: *mut crate::emit::Emit) -> *mut EmitNative<B> {
    emit as *mut EmitNative<B>
}

fn emit_mut<B: NativeBackend>(emit: *mut crate::emit::Emit) -> &'static mut EmitNative<B> {
    unsafe { &mut *emit_ref(emit) }
}

#[path = "emitnative_impl.rs"]
mod imp;
pub use imp::*;

#[path = "emitnative_exports.rs"]
pub mod emitnative_exports;
