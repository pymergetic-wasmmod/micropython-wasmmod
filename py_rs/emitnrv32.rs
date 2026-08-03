//! rewrite of py/emitnrv32.c
// symmetry: done

#![allow(non_snake_case)]

use crate::asmbase::{self, MpAsmBase};
use crate::asmrv32::{
    self, AsmRv32, ASM_RV32_REG_A0, ASM_RV32_REG_A1, ASM_RV32_REG_A2, ASM_RV32_REG_A3,
    ASM_RV32_REG_A4, ASM_RV32_REG_A5, ASM_RV32_REG_A6, ASM_RV32_REG_S1, ASM_RV32_REG_S2,
    ASM_RV32_REG_S3, ASM_RV32_REG_S4, ASM_RV32_REG_ZERO,
};
use crate::emitnative::{self, AsmContext, NativeBackend};
use crate::mpconfig;
use crate::qstr::Qstr;
use crate::runtime0::BinaryOp;

const ENABLED: bool = mpconfig::EMIT_RV32;

#[derive(Copy, Clone)]
pub struct BackendRv32;

impl AsmContext for AsmRv32 {
    fn base_mut(&mut self) -> &mut MpAsmBase {
        &mut self.base
    }
}

impl NativeBackend for BackendRv32 {
    type Asm = AsmRv32;
    const WORD_SIZE: i32 = 4;
    const REG_RET: i32 = ASM_RV32_REG_A0 as i32;
    const REG_ARG_1: i32 = ASM_RV32_REG_A0 as i32;
    const REG_ARG_2: i32 = ASM_RV32_REG_A1 as i32;
    const REG_ARG_3: i32 = ASM_RV32_REG_A2 as i32;
    const REG_ARG_4: i32 = ASM_RV32_REG_A3 as i32;
    const REG_TEMP0: i32 = ASM_RV32_REG_A4 as i32;
    const REG_TEMP1: i32 = ASM_RV32_REG_A5 as i32;
    const REG_TEMP2: i32 = ASM_RV32_REG_A6 as i32;
    const REG_LOCAL_1: i32 = ASM_RV32_REG_S3 as i32;
    const REG_LOCAL_2: i32 = ASM_RV32_REG_S2 as i32;
    const REG_LOCAL_3: i32 = ASM_RV32_REG_S4 as i32;
    const REG_FUN_TABLE: i32 = ASM_RV32_REG_S1 as i32;
    const REG_GENERATOR_STATE: i32 = ASM_RV32_REG_S2 as i32;
    const REG_QSTR_TABLE: i32 = ASM_RV32_REG_S4 as i32;
    // See the comment on the x64 backend: with `PERSISTENT_CODE_SAVE` this must
    // be `REG_LOCAL_2`, not `REG_LOCAL_3` (`REG_QSTR_TABLE`).
    const REG_LOCAL_LAST: i32 = ASM_RV32_REG_S2 as i32;
    const NLR_BUF_IDX_LOCAL_1: usize = 6;
    const N_X86: bool = false;
    const N_X64: bool = false;
    const N_THUMB: bool = false;
    const N_ARM: bool = false;
    const N_XTENSA: bool = false;
    const N_XTENSAWIN: bool = false;
    const N_RV32: bool = true;
    const N_DEBUG: bool = false;
    const N_NLR_SETJMP: bool = false;
    const REG_ZERO: i32 = ASM_RV32_REG_ZERO as i32;
    const REG_PARENT_RET: i32 = Self::REG_RET;
    const REG_PARENT_ARG_1: i32 = Self::REG_ARG_1;
    const REG_PARENT_ARG_2: i32 = Self::REG_ARG_2;
    const REG_PARENT_ARG_3: i32 = Self::REG_ARG_3;
    const REG_PARENT_ARG_4: i32 = Self::REG_ARG_4;
    const HAS_ASM_MOV_REG_QSTR: bool = false;
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
        let mut asm = AsmRv32 {
            base: MpAsmBase {
                pass: 0,
                suppress: false,
                code_offset: 0,
                code_size: 0,
                code_base: core::ptr::null_mut(),
                max_num_labels: 0,
                label_offsets: core::ptr::null_mut(),
            },
            saved_registers_mask: 0,
            locals_count: 0,
            stack_size: 0,
            locals_stack_offset: 0,
        };
        asmbase::init(&mut asm.base, max_labels);
        asm
    }
    fn asm_base(as_: &mut Self::Asm) -> &mut MpAsmBase {
        &mut as_.base
    }
    fn end_pass(as_: &mut Self::Asm) {
        asmrv32::end_pass(as_);
    }
    fn entry(as_: &mut Self::Asm, num_locals: i32, _name: Option<&str>) {
        asmrv32::entry(as_, num_locals as u32);
    }
    fn exit(as_: &mut Self::Asm) {
        asmrv32::exit(as_);
    }
    fn jump(as_: &mut Self::Asm, label: usize) {
        asmrv32::emit_jump(as_, label);
    }
    fn jump_if_reg_zero(as_: &mut Self::Asm, reg: i32, label: usize, _bool_test: bool) {
        asmrv32::emit_jump_if_reg_eq(as_, reg as u32, ASM_RV32_REG_ZERO, label);
    }
    fn jump_if_reg_nonzero(as_: &mut Self::Asm, reg: i32, label: usize, _bool_test: bool) {
        asmrv32::emit_jump_if_reg_nonzero(as_, reg as u32, label);
    }
    fn jump_if_reg_eq(as_: &mut Self::Asm, reg1: i32, reg2: i32, label: usize) {
        asmrv32::emit_jump_if_reg_eq(as_, reg1 as u32, reg2 as u32, label);
    }
    fn jump_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_cjr(as_, reg as u32);
    }
    fn call_ind(as_: &mut Self::Asm, idx: u32) {
        asmrv32::emit_call_ind(as_, idx);
    }
    fn mov_local_reg(as_: &mut Self::Asm, local: i32, reg: i32) {
        asmrv32::emit_mov_local_reg(as_, local as u32, reg as u32);
    }
    fn mov_reg_imm(as_: &mut Self::Asm, reg: i32, imm: usize) {
        asmrv32::emit_optimised_load_immediate(as_, reg as u32, imm as i32);
    }
    fn mov_reg_qstr(as_: &mut Self::Asm, reg: i32, qst: Qstr) {
        asmrv32::emit_optimised_load_immediate(as_, reg as u32, qst as i32);
    }
    fn mov_reg_local(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmrv32::emit_mov_reg_local(as_, reg as u32, local as u32);
    }
    fn mov_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_cmv(as_, dest as u32, src as u32);
    }
    fn mov_reg_local_addr(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmrv32::emit_mov_reg_local_addr(as_, reg as u32, local as u32);
    }
    fn mov_reg_pcrel(as_: &mut Self::Asm, reg: i32, label: usize) {
        asmrv32::emit_mov_reg_pcrel(as_, reg as u32, label);
    }
    fn not_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_xori(as_, reg as u32, reg as u32, -1);
    }
    fn neg_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_sub(as_, reg as u32, ASM_RV32_REG_ZERO, reg as u32);
    }
    fn lsl_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_sll(as_, reg as u32, reg as u32, reg as u32);
    }
    fn lsr_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_srl(as_, reg as u32, reg as u32, reg as u32);
    }
    fn asr_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::opcode_sra(as_, reg as u32, reg as u32, reg as u32);
    }
    fn lsl_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_sll(as_, dest as u32, dest as u32, src as u32);
    }
    fn lsr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_srl(as_, dest as u32, dest as u32, src as u32);
    }
    fn asr_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_sra(as_, dest as u32, dest as u32, src as u32);
    }
    fn or_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_or(as_, dest as u32, dest as u32, src as u32);
    }
    fn xor_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::emit_optimised_xor(as_, dest as u32, src as u32);
    }
    fn and_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_and(as_, dest as u32, src as u32, dest as u32);
    }
    fn add_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_cadd(as_, dest as u32, src as u32);
    }
    fn sub_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_sub(as_, dest as u32, dest as u32, src as u32);
    }
    fn mul_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmrv32::opcode_mul(as_, dest as u32, dest as u32, src as u32);
    }
    fn load_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, off * 4, 2);
    }
    fn load8_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, 0, 0);
    }
    fn load8_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, off, 0);
    }
    fn load16_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, 0, 1);
    }
    fn load16_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, off * 2, 1);
    }
    fn load32_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, 0, 2);
    }
    fn load32_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmrv32::emit_load_reg_reg_offset(as_, dest as u32, base as u32, off * 4, 2);
    }
    fn store_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, off * 4, 2);
    }
    fn store8_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, 0, 0);
    }
    fn store8_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, off, 0);
    }
    fn store16_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, 0, 1);
    }
    fn store16_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, off * 2, 1);
    }
    fn store32_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, 0, 2);
    }
    fn store32_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmrv32::emit_store_reg_reg_offset(as_, src as u32, base as u32, off * 4, 2);
    }
    fn clr_reg(as_: &mut Self::Asm, reg: i32) {
        asmrv32::emit_optimised_xor(as_, reg as u32, reg as u32);
    }
    fn mov_local_mp_obj_null(as_: &mut Self::Asm, local: i32, reg_temp: i32) {
        Self::clr_reg(as_, reg_temp);
        Self::mov_local_reg(as_, local, reg_temp);
    }
    fn setup_code_state_call(as_: &mut Self::Asm) {
        Self::call_ind(as_, emitnative::mp_f::SETUP_CODE_STATE);
    }
    fn mov_arg_to_reg(_as_: &mut Self::Asm, _arg_idx: i32, _reg: i32) {}
    fn binary_op_setcc(as_: &mut Self::Asm, op_idx: usize, dest: i32, lhs: i32, rhs: i32) {
        match op_idx {
            0 => asmrv32::meta_comparison_lt(as_, lhs as u32, rhs as u32, dest as u32, false),
            1 => asmrv32::meta_comparison_lt(as_, rhs as u32, lhs as u32, dest as u32, false),
            2 => asmrv32::meta_comparison_eq(as_, lhs as u32, rhs as u32, dest as u32),
            3 => asmrv32::meta_comparison_ne(as_, lhs as u32, rhs as u32, dest as u32),
            4 => asmrv32::meta_comparison_le(as_, lhs as u32, rhs as u32, dest as u32, false),
            5 => asmrv32::meta_comparison_le(as_, rhs as u32, lhs as u32, dest as u32, false),
            _ => {}
        }
    }
    fn binary_op_shift(as_: &mut Self::Asm, op: BinaryOp, dest: i32, src: i32) {
        match op {
            BinaryOp::Lshift => Self::lsl_reg_reg(as_, dest, src),
            BinaryOp::Rshift => Self::lsr_reg_reg(as_, dest, src),
            _ => {}
        }
    }
}

crate::export_emit_native_prefixed!(rv32, BackendRv32);
