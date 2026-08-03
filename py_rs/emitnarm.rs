//! rewrite of py/emitnarm.c
// symmetry: done

#![allow(non_snake_case)]

use crate::asmarm::{
    self, AsmArm, ASM_ARM_CC_EQ, ASM_ARM_CC_NE, ASM_ARM_REG_FUN_TABLE, ASM_ARM_REG_R0,
    ASM_ARM_REG_R1, ASM_ARM_REG_R2, ASM_ARM_REG_R3, ASM_ARM_REG_R4, ASM_ARM_REG_R5, ASM_ARM_REG_R6,
};
use crate::asmbase::{self, MpAsmBase};
use crate::emitnative::{self, AsmContext, NativeBackend};
use crate::mpconfig;
use crate::qstr::Qstr;
use crate::runtime0::BinaryOp;

const ENABLED: bool = mpconfig::EMIT_ARM;

#[derive(Copy, Clone)]
pub struct BackendArm;

impl AsmContext for AsmArm {
    fn base_mut(&mut self) -> &mut MpAsmBase {
        &mut self.base
    }
}

impl NativeBackend for BackendArm {
    type Asm = AsmArm;
    const WORD_SIZE: i32 = 4;
    const REG_RET: i32 = ASM_ARM_REG_R0 as i32;
    const REG_ARG_1: i32 = ASM_ARM_REG_R0 as i32;
    const REG_ARG_2: i32 = ASM_ARM_REG_R1 as i32;
    const REG_ARG_3: i32 = ASM_ARM_REG_R2 as i32;
    const REG_ARG_4: i32 = ASM_ARM_REG_R3 as i32;
    const REG_TEMP0: i32 = ASM_ARM_REG_R0 as i32;
    const REG_TEMP1: i32 = ASM_ARM_REG_R1 as i32;
    const REG_TEMP2: i32 = ASM_ARM_REG_R2 as i32;
    const REG_LOCAL_1: i32 = ASM_ARM_REG_R4 as i32;
    const REG_LOCAL_2: i32 = ASM_ARM_REG_R5 as i32;
    const REG_LOCAL_3: i32 = ASM_ARM_REG_R6 as i32;
    const REG_FUN_TABLE: i32 = ASM_ARM_REG_FUN_TABLE as i32;
    const REG_GENERATOR_STATE: i32 = ASM_ARM_REG_R5 as i32;
    const REG_QSTR_TABLE: i32 = ASM_ARM_REG_R6 as i32;
    // See the comment on the x64 backend: with `PERSISTENT_CODE_SAVE` this must
    // be `REG_LOCAL_2`, not `REG_LOCAL_3` (`REG_QSTR_TABLE`).
    const REG_LOCAL_LAST: i32 = ASM_ARM_REG_R5 as i32;
    const NLR_BUF_IDX_LOCAL_1: usize = 3;
    const N_X86: bool = false;
    const N_X64: bool = false;
    const N_THUMB: bool = false;
    const N_ARM: bool = true;
    const N_XTENSA: bool = false;
    const N_XTENSAWIN: bool = false;
    const N_RV32: bool = false;
    const N_DEBUG: bool = false;
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
    const HAS_ASM_LOAD8_REG_REG_REG: bool = true;
    const HAS_ASM_LOAD16_REG_REG_REG: bool = true;
    const HAS_ASM_LOAD32_REG_REG_REG: bool = true;
    const HAS_ASM_STORE8_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE16_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE32_REG_REG_OFFSET: bool = true;
    const HAS_ASM_STORE8_REG_REG_REG: bool = true;
    const HAS_ASM_STORE16_REG_REG_REG: bool = true;
    const HAS_ASM_STORE32_REG_REG_REG: bool = true;
    const HAS_ASM_NOT_REG: bool = true;
    const REG_LOCAL_TABLE: &'static [i32] =
        &[Self::REG_LOCAL_1, Self::REG_LOCAL_2, Self::REG_LOCAL_3];

    fn new_asm(max_labels: usize) -> Self::Asm {
        let mut asm = AsmArm {
            base: MpAsmBase {
                pass: 0,
                suppress: false,
                code_offset: 0,
                code_size: 0,
                code_base: core::ptr::null_mut(),
                max_num_labels: 0,
                label_offsets: core::ptr::null_mut(),
            },
            push_reglist: 0,
            stack_adjust: 0,
        };
        asmbase::init(&mut asm.base, max_labels);
        asm
    }
    fn asm_base(as_: &mut Self::Asm) -> &mut MpAsmBase {
        &mut as_.base
    }
    fn end_pass(as_: &mut Self::Asm) {
        asmarm::end_pass(as_);
    }
    fn entry(as_: &mut Self::Asm, num_locals: i32, _name: Option<&str>) {
        asmarm::entry(as_, num_locals);
    }
    fn exit(as_: &mut Self::Asm) {
        asmarm::exit(as_);
    }
    fn jump(as_: &mut Self::Asm, label: usize) {
        asmarm::b_label(as_, label);
    }
    fn jump_if_reg_zero(as_: &mut Self::Asm, reg: i32, label: usize, _bool_test: bool) {
        asmarm::cmp_reg_i8(as_, reg as u32, 0);
        asmarm::bcc_label(as_, ASM_ARM_CC_EQ as i32, label);
    }
    fn jump_if_reg_nonzero(as_: &mut Self::Asm, reg: i32, label: usize, _bool_test: bool) {
        asmarm::cmp_reg_i8(as_, reg as u32, 0);
        asmarm::bcc_label(as_, ASM_ARM_CC_NE as i32, label);
    }
    fn jump_if_reg_eq(as_: &mut Self::Asm, reg1: i32, reg2: i32, label: usize) {
        asmarm::cmp_reg_reg(as_, reg1 as u32, reg2 as u32);
        asmarm::bcc_label(as_, ASM_ARM_CC_EQ as i32, label);
    }
    fn jump_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::bx_reg(as_, reg as u32);
    }
    fn call_ind(as_: &mut Self::Asm, idx: u32) {
        asmarm::bl_ind(as_, idx, ASM_ARM_REG_R3);
    }
    fn mov_local_reg(as_: &mut Self::Asm, local: i32, reg: i32) {
        asmarm::mov_local_reg(as_, local, reg as u32);
    }
    fn mov_reg_imm(as_: &mut Self::Asm, reg: i32, imm: usize) {
        asmarm::mov_reg_i32_optimised(as_, reg as u32, imm as i32);
    }
    fn mov_reg_qstr(as_: &mut Self::Asm, reg: i32, qst: Qstr) {
        asmarm::mov_reg_i32_optimised(as_, reg as u32, qst as i32);
    }
    fn mov_reg_local(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmarm::mov_reg_local(as_, reg as u32, local);
    }
    fn mov_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::mov_reg_reg(as_, dest as u32, src as u32);
    }
    fn mov_reg_local_addr(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmarm::mov_reg_local_addr(as_, reg as u32, local);
    }
    fn mov_reg_pcrel(as_: &mut Self::Asm, reg: i32, label: usize) {
        asmarm::mov_reg_pcrel(as_, reg as u32, label);
    }
    fn not_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::mvn_reg_reg(as_, reg as u32, reg as u32);
    }
    fn neg_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::rsb_reg_reg_imm(as_, reg as u32, reg as u32, 0);
    }
    fn lsl_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::lsl_reg_reg(as_, reg as u32, reg as u32);
    }
    fn lsr_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::lsr_reg_reg(as_, reg as u32, reg as u32);
    }
    fn asr_reg(as_: &mut Self::Asm, reg: i32) {
        asmarm::asr_reg_reg(as_, reg as u32, reg as u32);
    }
    fn lsl_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::lsl_reg_reg(as_, dest as u32, src as u32);
    }
    fn lsr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::lsr_reg_reg(as_, dest as u32, src as u32);
    }
    fn asr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::asr_reg_reg(as_, dest as u32, src as u32);
    }
    fn or_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::orr_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn xor_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::eor_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn and_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::and_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn add_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::add_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn sub_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::sub_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn mul_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmarm::mul_reg_reg_reg(as_, dest as u32, dest as u32, src as u32);
    }
    fn load_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmarm::ldr_reg_reg_offset(as_, dest as u32, base as u32, (off * 4) as u32);
    }
    fn load8_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmarm::ldrb_reg_reg_offset(as_, dest as u32, base as u32, 0);
    }
    fn load8_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmarm::ldrb_reg_reg_offset(as_, dest as u32, base as u32, off as u32);
    }
    fn load16_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmarm::ldrh_reg_reg_offset(as_, dest as u32, base as u32, 0);
    }
    fn load16_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmarm::ldrh_reg_reg_offset(as_, dest as u32, base as u32, (off * 2) as u32);
    }
    fn load32_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmarm::ldr_reg_reg_offset(as_, dest as u32, base as u32, 0);
    }
    fn load32_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmarm::ldr_reg_reg_offset(as_, dest as u32, base as u32, (off * 4) as u32);
    }
    fn store_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmarm::str_reg_reg_offset(as_, src as u32, base as u32, (off * 4) as u32);
    }
    fn store8_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmarm::strb_reg_reg_offset(as_, src as u32, base as u32, 0);
    }
    fn store8_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmarm::strb_reg_reg_offset(as_, src as u32, base as u32, off as u32);
    }
    fn store16_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmarm::strh_reg_reg_offset(as_, src as u32, base as u32, 0);
    }
    fn store16_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmarm::strh_reg_reg_offset(as_, src as u32, base as u32, (off * 2) as u32);
    }
    fn store32_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmarm::str_reg_reg_offset(as_, src as u32, base as u32, 0);
    }
    fn store32_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmarm::str_reg_reg_offset(as_, src as u32, base as u32, (off * 4) as u32);
    }
    fn clr_reg(as_: &mut Self::Asm, reg: i32) {
        Self::mov_reg_imm(as_, reg, 0);
    }
    fn mov_local_mp_obj_null(as_: &mut Self::Asm, local: i32, reg_temp: i32) {
        Self::clr_reg(as_, reg_temp);
        Self::mov_local_reg(as_, local, reg_temp);
    }
    fn setup_code_state_call(as_: &mut Self::Asm) {
        Self::call_ind(as_, emitnative::mp_f::SETUP_CODE_STATE);
    }
    fn mov_arg_to_reg(_as_: &mut Self::Asm, _arg_idx: i32, _reg: i32) {}
    fn binary_op_setcc(_as_: &mut Self::Asm, _op_idx: usize, _dest: i32, _lhs: i32, _rhs: i32) {}
    fn binary_op_shift(as_: &mut Self::Asm, op: BinaryOp, dest: i32, src: i32) {
        match op {
            BinaryOp::Lshift => Self::lsl_reg_reg(as_, dest, src),
            BinaryOp::Rshift => Self::lsr_reg_reg(as_, dest, src),
            _ => {}
        }
    }
}

crate::export_emit_native_prefixed!(arm, BackendArm);
