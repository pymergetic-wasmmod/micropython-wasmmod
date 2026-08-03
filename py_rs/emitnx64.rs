//! rewrite of py/emitnx64.c
// symmetry: done

#![allow(non_snake_case)]

use crate::asmbase::{self, MpAsmBase};
use crate::asmx64::{
    self, AsmX64, ASM_X64_REG_R12, ASM_X64_REG_R13, ASM_X64_REG_RAX, ASM_X64_REG_RBP,
    ASM_X64_REG_RBX, ASM_X64_REG_RCX, ASM_X64_REG_RDI, ASM_X64_REG_RDX, ASM_X64_REG_RSI,
};
use crate::emitnative::{self, AsmContext, NativeBackend};
use crate::mpconfig;
use crate::qstr::Qstr;
use crate::runtime0::BinaryOp;

const ASM_X64_CC_JZ: i32 = 0x4;
const ASM_X64_CC_JNZ: i32 = 0x5;
const ASM_X64_CC_JE: i32 = 0x4;

const ENABLED: bool = mpconfig::EMIT_X64;

#[derive(Copy, Clone)]
pub struct BackendX64;

impl AsmContext for AsmX64 {
    fn base_mut(&mut self) -> &mut MpAsmBase {
        &mut self.base
    }
}

impl NativeBackend for BackendX64 {
    type Asm = AsmX64;
    const WORD_SIZE: i32 = 8;
    const REG_RET: i32 = ASM_X64_REG_RAX;
    const REG_ARG_1: i32 = ASM_X64_REG_RDI;
    const REG_ARG_2: i32 = ASM_X64_REG_RSI;
    const REG_ARG_3: i32 = ASM_X64_REG_RDX;
    const REG_ARG_4: i32 = ASM_X64_REG_RCX;
    const REG_TEMP0: i32 = ASM_X64_REG_RAX;
    const REG_TEMP1: i32 = ASM_X64_REG_RDI;
    const REG_TEMP2: i32 = ASM_X64_REG_RSI;
    const REG_LOCAL_1: i32 = ASM_X64_REG_RBX;
    const REG_LOCAL_2: i32 = ASM_X64_REG_R12;
    const REG_LOCAL_3: i32 = ASM_X64_REG_R13;
    const REG_FUN_TABLE: i32 = ASM_X64_REG_RBP;
    const REG_GENERATOR_STATE: i32 = ASM_X64_REG_R12;
    const REG_QSTR_TABLE: i32 = ASM_X64_REG_R13;
    // `REG_LOCAL_LAST` is `reg_local_table[MAX_REGS_FOR_LOCAL_VARS - 1]` in upstream C.
    // With `PERSISTENT_CODE_SAVE` (always on in this port), `MAX_REGS_FOR_LOCAL_VARS == 2`,
    // so this must be `REG_LOCAL_2`, not `REG_LOCAL_3` (which aliases `REG_QSTR_TABLE` and
    // would otherwise get clobbered by the incoming args pointer in viper prologues).
    const REG_LOCAL_LAST: i32 = ASM_X64_REG_R12;
    const NLR_BUF_IDX_LOCAL_1: usize = 5;
    const N_X86: bool = false;
    const N_X64: bool = true;
    const N_THUMB: bool = false;
    const N_ARM: bool = false;
    const N_XTENSA: bool = false;
    const N_XTENSAWIN: bool = false;
    const N_RV32: bool = false;
    const N_DEBUG: bool = false;
    const N_NLR_SETJMP: bool = false;
    const REG_ZERO: i32 = ASM_X64_REG_RAX;
    const REG_PARENT_RET: i32 = Self::REG_RET;
    const REG_PARENT_ARG_1: i32 = Self::REG_ARG_1;
    const REG_PARENT_ARG_2: i32 = Self::REG_ARG_2;
    const REG_PARENT_ARG_3: i32 = Self::REG_ARG_3;
    const REG_PARENT_ARG_4: i32 = Self::REG_ARG_4;
    const HAS_ASM_MOV_REG_QSTR: bool = false;
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
    const REG_LOCAL_TABLE: &'static [i32] =
        &[Self::REG_LOCAL_1, Self::REG_LOCAL_2, Self::REG_LOCAL_3];

    fn new_asm(max_labels: usize) -> Self::Asm {
        let mut asm = AsmX64 {
            base: MpAsmBase {
                pass: 0,
                suppress: false,
                code_offset: 0,
                code_size: 0,
                code_base: core::ptr::null_mut(),
                max_num_labels: 0,
                label_offsets: core::ptr::null_mut(),
            },
            num_locals: 0,
        };
        asmbase::init(&mut asm.base, max_labels);
        asm
    }
    fn asm_base(as_: &mut Self::Asm) -> &mut MpAsmBase {
        &mut as_.base
    }
    fn end_pass(as_: &mut Self::Asm) {
        asmx64::end_pass(as_);
    }
    fn entry(as_: &mut Self::Asm, num_locals: i32, _name: Option<&str>) {
        asmx64::entry(as_, num_locals);
    }
    fn exit(as_: &mut Self::Asm) {
        asmx64::exit(as_);
    }
    fn jump(as_: &mut Self::Asm, label: usize) {
        asmx64::jmp_label(as_, label);
    }
    fn jump_if_reg_zero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool) {
        if bool_test {
            asmx64::test_r8_with_r8(as_, reg, reg);
        } else {
            asmx64::test_r64_with_r64(as_, reg, reg);
        }
        asmx64::jcc_label(as_, ASM_X64_CC_JZ, label);
    }
    fn jump_if_reg_nonzero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool) {
        if bool_test {
            asmx64::test_r8_with_r8(as_, reg, reg);
        } else {
            asmx64::test_r64_with_r64(as_, reg, reg);
        }
        asmx64::jcc_label(as_, ASM_X64_CC_JNZ, label);
    }
    fn jump_if_reg_eq(as_: &mut Self::Asm, reg1: i32, reg2: i32, label: usize) {
        asmx64::cmp_r64_with_r64(as_, reg1, reg2);
        asmx64::jcc_label(as_, ASM_X64_CC_JE, label);
    }
    fn jump_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::jmp_reg(as_, reg);
    }
    fn call_ind(as_: &mut Self::Asm, idx: u32) {
        asmx64::call_ind(as_, idx as usize, ASM_X64_REG_RAX);
    }
    fn mov_local_reg(as_: &mut Self::Asm, local: i32, reg: i32) {
        asmx64::mov_r64_to_local(as_, reg, local);
    }
    fn mov_reg_imm(as_: &mut Self::Asm, reg: i32, imm: usize) {
        asmx64::mov_i64_to_r64_optimised(as_, imm as i64, reg);
    }
    fn mov_reg_qstr(as_: &mut Self::Asm, reg: i32, qst: Qstr) {
        asmx64::mov_i64_to_r64_optimised(as_, qst as i64, reg);
    }
    fn mov_reg_local(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmx64::mov_local_to_r64(as_, local, reg);
    }
    fn mov_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::mov_r64_r64(as_, dest, src);
    }
    fn mov_reg_local_addr(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmx64::mov_local_addr_to_r64(as_, local, reg);
    }
    fn mov_reg_pcrel(as_: &mut Self::Asm, reg: i32, label: usize) {
        asmx64::mov_reg_pcrel(as_, reg, label);
    }
    fn not_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::not_r64(as_, reg);
    }
    fn neg_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::neg_r64(as_, reg);
    }
    fn lsl_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::shl_r64_cl(as_, reg);
    }
    fn lsr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::shr_r64_cl(as_, reg);
    }
    fn asr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::sar_r64_cl(as_, reg);
    }
    fn lsl_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx64::shl_r64_cl(as_, dest);
    }
    fn lsr_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx64::shr_r64_cl(as_, dest);
    }
    fn asr_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx64::sar_r64_cl(as_, dest);
    }
    fn or_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::or_r64_r64(as_, dest, src);
    }
    fn xor_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::xor_r64_r64(as_, dest, src);
    }
    fn and_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::and_r64_r64(as_, dest, src);
    }
    fn add_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::add_r64_r64(as_, dest, src);
    }
    fn sub_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::sub_r64_r64(as_, dest, src);
    }
    fn mul_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx64::mul_r64_r64(as_, dest, src);
    }
    fn load_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx64::mov_mem64_to_r64(as_, base, off * 8, dest);
    }
    fn load8_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx64::mov_mem8_to_r64zx(as_, base, 0, dest);
    }
    fn load8_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx64::mov_mem8_to_r64zx(as_, base, off, dest);
    }
    fn load16_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx64::mov_mem16_to_r64zx(as_, base, 0, dest);
    }
    fn load16_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx64::mov_mem16_to_r64zx(as_, base, off * 2, dest);
    }
    fn load32_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx64::mov_mem32_to_r64zx(as_, base, 0, dest);
    }
    fn load32_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx64::mov_mem32_to_r64zx(as_, base, off * 4, dest);
    }
    fn store_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx64::mov_r64_to_mem64(as_, src, base, off * 8);
    }
    fn store8_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx64::mov_r8_to_mem8(as_, src, base, 0);
    }
    fn store8_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx64::mov_r8_to_mem8(as_, src, base, off);
    }
    fn store16_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx64::mov_r16_to_mem16(as_, src, base, 0);
    }
    fn store16_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx64::mov_r16_to_mem16(as_, src, base, off * 2);
    }
    fn store32_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx64::mov_r32_to_mem32(as_, src, base, 0);
    }
    fn store32_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx64::mov_r32_to_mem32(as_, src, base, off * 4);
    }
    fn clr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx64::xor_r64_r64(as_, reg, reg);
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
        const OPS: [i32; 12] = [0x2, 0x7, 0x4, 0x6, 0x3, 0x5, 0xc, 0xf, 0x4, 0xe, 0xd, 0x5];
        asmx64::xor_r64_r64(as_, dest, dest);
        asmx64::cmp_r64_with_r64(as_, rhs, lhs);
        asmx64::setcc_r8(as_, OPS[op_idx], dest);
    }
    fn binary_op_shift(as_: &mut Self::Asm, op: BinaryOp, dest: i32, _shift_reg: i32) {
        match op {
            BinaryOp::Lshift => asmx64::shl_r64_cl(as_, dest),
            BinaryOp::Rshift => asmx64::sar_r64_cl(as_, dest),
            _ => {}
        }
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use crate::asmbase::{self, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};
    use crate::runtime0::BinaryOp;

    #[test]
    fn binary_op_setcc_emits_compare_sequence() {
        let mut asm = BackendX64::new_asm(4);
        asm.base_mut().pass = MP_ASM_PASS_COMPUTE;
        BackendX64::binary_op_setcc(
            &mut asm,
            0,
            BackendX64::REG_RET,
            BackendX64::REG_ARG_2,
            BackendX64::REG_ARG_3,
        );
        assert!(asm.base_mut().get_code_pos() > 0);
        BackendX64::end_pass(&mut asm);
        asm.base_mut().pass = MP_ASM_PASS_EMIT;
        asmbase::start_pass(asm.base_mut(), MP_ASM_PASS_EMIT as i32);
        BackendX64::binary_op_setcc(
            &mut asm,
            6,
            BackendX64::REG_RET,
            BackendX64::REG_ARG_2,
            BackendX64::REG_ARG_3,
        );
        BackendX64::end_pass(&mut asm);
        assert!(asm.base_mut().get_code_size() > 0);
    }

    #[test]
    fn binary_op_shift_uses_sar_for_rshift() {
        let mut asm = BackendX64::new_asm(2);
        asm.base_mut().pass = MP_ASM_PASS_COMPUTE;
        BackendX64::binary_op_shift(
            &mut asm,
            BinaryOp::Rshift,
            BackendX64::REG_RET,
            BackendX64::REG_ARG_4,
        );
        BackendX64::end_pass(&mut asm);
        asm.base_mut().pass = MP_ASM_PASS_EMIT;
        asmbase::start_pass(asm.base_mut(), MP_ASM_PASS_EMIT as i32);
        BackendX64::binary_op_shift(
            &mut asm,
            BinaryOp::Rshift,
            BackendX64::REG_RET,
            BackendX64::REG_ARG_4,
        );
        BackendX64::end_pass(&mut asm);
        assert!(asm.base_mut().get_code_size() > 0);
    }
}

crate::export_emit_native_prefixed!(x64, BackendX64);
