//! rewrite of py/emitndebug.c
// symmetry: done

#![allow(non_snake_case)]

use crate::asmbase::{self, MpAsmBase};
use crate::emitnative::{self, AsmContext, NativeBackend};
use crate::mpconfig;
use crate::qstr::Qstr;
use crate::runtime0::BinaryOp;

const ENABLED: bool = mpconfig::EMIT_NATIVE_DEBUG;

#[repr(C)]
pub struct AsmDebug {
    pub base: MpAsmBase,
}

#[derive(Copy, Clone)]
pub struct BackendDebug;

impl AsmContext for AsmDebug {
    fn base_mut(&mut self) -> &mut MpAsmBase {
        &mut self.base
    }
}

impl NativeBackend for BackendDebug {
    type Asm = AsmDebug;
    const WORD_SIZE: i32 = 8;
    const REG_RET: i32 = 0;
    const REG_ARG_1: i32 = 1;
    const REG_ARG_2: i32 = 2;
    const REG_ARG_3: i32 = 3;
    const REG_ARG_4: i32 = 4;
    const REG_TEMP0: i32 = 5;
    const REG_TEMP1: i32 = 6;
    const REG_TEMP2: i32 = 7;
    const REG_LOCAL_1: i32 = 8;
    const REG_LOCAL_2: i32 = 9;
    const REG_LOCAL_3: i32 = 10;
    const REG_FUN_TABLE: i32 = 11;
    const REG_GENERATOR_STATE: i32 = 9;
    const REG_QSTR_TABLE: i32 = 10;
    const REG_LOCAL_LAST: i32 = 10;
    const NLR_BUF_IDX_LOCAL_1: usize = 5;
    const N_X86: bool = false;
    const N_X64: bool = false;
    const N_THUMB: bool = false;
    const N_ARM: bool = false;
    const N_XTENSA: bool = false;
    const N_XTENSAWIN: bool = false;
    const N_RV32: bool = false;
    const N_DEBUG: bool = true;
    const N_NLR_SETJMP: bool = false;
    const REG_ZERO: i32 = 0;
    const REG_PARENT_RET: i32 = Self::REG_RET;
    const REG_PARENT_ARG_1: i32 = Self::REG_ARG_1;
    const REG_PARENT_ARG_2: i32 = Self::REG_ARG_2;
    const REG_PARENT_ARG_3: i32 = Self::REG_ARG_3;
    const REG_PARENT_ARG_4: i32 = Self::REG_ARG_4;
    const HAS_ASM_MOV_REG_QSTR: bool = true;
    const HAS_ASM_LOAD8_REG_REG_OFFSET: bool = true;
    const HAS_ASM_LOAD16_REG_REG_OFFSET: bool = true;
    const HAS_ASM_LOAD32_REG_REG_OFFSET: bool = true;
    const HAS_ASM_LOAD8_REG_REG_REG: bool = false;
    const HAS_ASM_LOAD16_REG_REG_REG: bool = false;
    const HAS_ASM_LOAD32_REG_REG_REG: bool = false;
    const HAS_ASM_STORE8_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE16_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE32_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE8_REG_REG_REG: bool = false;
    const HAS_ASM_STORE16_REG_REG_REG: bool = false;
    const HAS_ASM_STORE32_REG_REG_REG: bool = false;
    const HAS_ASM_NOT_REG: bool = true;
    const REG_LOCAL_TABLE: &'static [i32] = &[Self::REG_LOCAL_1, Self::REG_LOCAL_2, Self::REG_LOCAL_3];

    fn new_asm(max_labels: usize) -> Self::Asm {
        let mut asm = AsmDebug {
            base: MpAsmBase {
                pass: 0,
                suppress: false,
                code_offset: 0,
                code_size: 0,
                code_base: core::ptr::null_mut(),
                max_num_labels: 0,
                label_offsets: core::ptr::null_mut(),
            },
        };
        asmbase::init(&mut asm.base, max_labels);
        asm
    }
    fn asm_base(as_: &mut Self::Asm) -> &mut MpAsmBase {
        &mut as_.base
    }
    fn end_pass(_as_: &mut Self::Asm) {}
    fn entry(_as_: &mut Self::Asm, _num_locals: i32, _name: Option<&str>) {}
    fn exit(_as_: &mut Self::Asm) {}
    fn jump(_as_: &mut Self::Asm, _label: usize) {}
    fn jump_if_reg_zero(_as_: &mut Self::Asm, _reg: i32, _label: usize, _bool_test: bool) {}
    fn jump_if_reg_nonzero(_as_: &mut Self::Asm, _reg: i32, _label: usize, _bool_test: bool) {}
    fn jump_if_reg_eq(_as_: &mut Self::Asm, _reg1: i32, _reg2: i32, _label: usize) {}
    fn jump_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn call_ind(_as_: &mut Self::Asm, _idx: u32) {}
    fn mov_local_reg(_as_: &mut Self::Asm, _local: i32, _reg: i32) {}
    fn mov_reg_imm(_as_: &mut Self::Asm, _reg: i32, _imm: usize) {}
    fn mov_reg_qstr(_as_: &mut Self::Asm, _reg: i32, _qst: Qstr) {}
    fn mov_reg_local(_as_: &mut Self::Asm, _reg: i32, _local: i32) {}
    fn mov_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn mov_reg_local_addr(_as_: &mut Self::Asm, _reg: i32, _local: i32) {}
    fn mov_reg_pcrel(_as_: &mut Self::Asm, _reg: i32, _label: usize) {}
    fn not_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn neg_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn lsl_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn lsr_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn asr_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn lsl_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn lsr_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn asr_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn or_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn xor_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn and_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn add_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn sub_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn mul_reg_reg(_as_: &mut Self::Asm, _dest: i32, _src: i32) {}
    fn load_reg_reg_offset(_as_: &mut Self::Asm, _dest: i32, _base: i32, _off: i32) {}
    fn load8_reg_reg(_as_: &mut Self::Asm, _dest: i32, _base: i32) {}
    fn load8_reg_reg_offset(_as_: &mut Self::Asm, _dest: i32, _base: i32, _off: i32) {}
    fn load16_reg_reg(_as_: &mut Self::Asm, _dest: i32, _base: i32) {}
    fn load16_reg_reg_offset(_as_: &mut Self::Asm, _dest: i32, _base: i32, _off: i32) {}
    fn load32_reg_reg(_as_: &mut Self::Asm, _dest: i32, _base: i32) {}
    fn load32_reg_reg_offset(_as_: &mut Self::Asm, _dest: i32, _base: i32, _off: i32) {}
    fn store_reg_reg_offset(_as_: &mut Self::Asm, _src: i32, _base: i32, _off: i32) {}
    fn store8_reg_reg(_as_: &mut Self::Asm, _src: i32, _base: i32) {}
    fn store8_reg_reg_offset(_as_: &mut Self::Asm, _src: i32, _base: i32, _off: i32) {}
    fn store16_reg_reg(_as_: &mut Self::Asm, _src: i32, _base: i32) {}
    fn store16_reg_reg_offset(_as_: &mut Self::Asm, _src: i32, _base: i32, _off: i32) {}
    fn store32_reg_reg(_as_: &mut Self::Asm, _src: i32, _base: i32) {}
    fn store32_reg_reg_offset(_as_: &mut Self::Asm, _src: i32, _base: i32, _off: i32) {}
    fn clr_reg(_as_: &mut Self::Asm, _reg: i32) {}
    fn mov_local_mp_obj_null(_as_: &mut Self::Asm, _local: i32, _reg_temp: i32) {}
    fn setup_code_state_call(_as_: &mut Self::Asm) {
        Self::call_ind(_as_, emitnative::mp_f::SETUP_CODE_STATE);
    }
    fn mov_arg_to_reg(_as_: &mut Self::Asm, _arg_idx: i32, _reg: i32) {}
    fn binary_op_setcc(_as_: &mut Self::Asm, _op_idx: usize, _dest: i32, _lhs: i32, _rhs: i32) {}
    fn binary_op_shift(_as_: &mut Self::Asm, _op: BinaryOp, _dest: i32, _shift_reg: i32) {}
}

crate::export_emit_native_prefixed!(debug, BackendDebug);
