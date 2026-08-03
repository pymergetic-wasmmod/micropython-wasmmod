//! rewrite of py/asmarm.c + py/asmarm.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase};
use crate::misc;
use crate::mpconfig;

const ENABLED: bool = mpconfig::EMIT_ARM;
const REG_TEMP: u32 = ASM_ARM_REG_R8;

#[repr(C)]
pub struct AsmArm {
    pub base: MpAsmBase,
    pub push_reglist: u32,
    pub stack_adjust: u32,
}

pub const ASM_ARM_REG_R0: u32 = 0;
pub const ASM_ARM_REG_R1: u32 = 1;
pub const ASM_ARM_REG_R2: u32 = 2;
pub const ASM_ARM_REG_R3: u32 = 3;
pub const ASM_ARM_REG_R4: u32 = 4;
pub const ASM_ARM_REG_R5: u32 = 5;
pub const ASM_ARM_REG_R6: u32 = 6;
pub const ASM_ARM_REG_R7: u32 = 7;
pub const ASM_ARM_REG_R8: u32 = 8;
pub const ASM_ARM_REG_R9: u32 = 9;
pub const ASM_ARM_REG_R10: u32 = 10;
pub const ASM_ARM_REG_R11: u32 = 11;
pub const ASM_ARM_REG_R12: u32 = 12;
pub const ASM_ARM_REG_R13: u32 = 13;
pub const ASM_ARM_REG_R14: u32 = 14;
pub const ASM_ARM_REG_R15: u32 = 15;
pub const ASM_ARM_REG_SP: u32 = ASM_ARM_REG_R13;
pub const ASM_ARM_REG_LR: u32 = ASM_ARM_REG_R14;
pub const ASM_ARM_REG_PC: u32 = ASM_ARM_REG_R15;

pub const ASM_ARM_CC_EQ: u32 = 0x0 << 28;
pub const ASM_ARM_CC_NE: u32 = 0x1 << 28;
pub const ASM_ARM_CC_CS: u32 = 0x2 << 28;
pub const ASM_ARM_CC_CC: u32 = 0x3 << 28;
pub const ASM_ARM_CC_MI: u32 = 0x4 << 28;
pub const ASM_ARM_CC_PL: u32 = 0x5 << 28;
pub const ASM_ARM_CC_VS: u32 = 0x6 << 28;
pub const ASM_ARM_CC_VC: u32 = 0x7 << 28;
pub const ASM_ARM_CC_HI: u32 = 0x8 << 28;
pub const ASM_ARM_CC_LS: u32 = 0x9 << 28;
pub const ASM_ARM_CC_GE: u32 = 0xa << 28;
pub const ASM_ARM_CC_LT: u32 = 0xb << 28;
pub const ASM_ARM_CC_GT: u32 = 0xc << 28;
pub const ASM_ARM_CC_LE: u32 = 0xd << 28;
pub const ASM_ARM_CC_AL: u32 = 0xe << 28;

pub const ASM_ARM_REG_FUN_TABLE: u32 = ASM_ARM_REG_R7;

fn emit(asm: &mut AsmArm, op: u32) {
    if !ENABLED {
        return;
    }
    let c = asmbase::get_cur_to_write_bytes(&mut asm.base, 4);
    if !c.is_null() {
        unsafe {
            *(c as *mut u32) = op;
        }
    }
}

fn emit_al(asm: &mut AsmArm, op: u32) {
    emit(asm, op | ASM_ARM_CC_AL);
}

const fn op_push(reglist: u32) -> u32 {
    0x92d0000 | (reglist & 0xffff)
}

const fn op_pop(reglist: u32) -> u32 {
    0x8bd0000 | (reglist & 0xffff)
}

const fn op_mov_reg(rd: u32, rn: u32) -> u32 {
    0x1a00000 | (rd << 12) | rn
}

const fn op_mov_imm(rd: u32, imm: u32) -> u32 {
    0x3a00000 | (rd << 12) | imm
}

const fn op_mvn_imm(rd: u32, imm: u32) -> u32 {
    0x3e00000 | (rd << 12) | imm
}

const fn op_mvn_reg(rd: u32, rm: u32) -> u32 {
    0x1e00000 | (rd << 12) | rm
}

const fn op_add_imm(rd: u32, rn: u32, imm: u32) -> u32 {
    0x2800000 | (rn << 16) | (rd << 12) | (imm & 0xff)
}

const fn op_add_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0x0800000 | (rn << 16) | (rd << 12) | rm
}

const fn op_sub_imm(rd: u32, rn: u32, imm: u32) -> u32 {
    0x2400000 | (rn << 16) | (rd << 12) | (imm & 0xff)
}

const fn op_sub_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0x0400000 | (rn << 16) | (rd << 12) | rm
}

const fn op_rsb_imm(rd: u32, rn: u32, imm: u32) -> u32 {
    0x2600000 | (rn << 16) | (rd << 12) | (imm & 0xff)
}

fn op_mul_reg(rd: u32, rm: u32, rs: u32) -> u32 {
    assert!(rd != rm);
    0x0000090 | (rd << 16) | (rs << 8) | rm
}

const fn op_and_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0x0000000 | (rn << 16) | (rd << 12) | rm
}

const fn op_eor_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0x0200000 | (rn << 16) | (rd << 12) | rm
}

const fn op_orr_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0x1800000 | (rn << 16) | (rd << 12) | rm
}

pub fn end_pass(_asm: &mut AsmArm) {}

pub fn bkpt(asm: &mut AsmArm) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1200070);
}

pub fn entry(asm: &mut AsmArm, num_locals: i32) {
    if !ENABLED {
        return;
    }
    assert!(num_locals >= 0);

    asm.stack_adjust = 0;
    asm.push_reglist = (1 << ASM_ARM_REG_R1)
        | (1 << ASM_ARM_REG_R2)
        | (1 << ASM_ARM_REG_R3)
        | (1 << ASM_ARM_REG_R4)
        | (1 << ASM_ARM_REG_R5)
        | (1 << ASM_ARM_REG_R6)
        | (1 << ASM_ARM_REG_R7)
        | (1 << ASM_ARM_REG_R8);

    if num_locals > 3 {
        asm.stack_adjust = (num_locals * 4) as u32;
        if (num_locals & 1) != 0 {
            asm.stack_adjust += 4;
        }
    }

    emit_al(asm, op_push(asm.push_reglist | (1 << ASM_ARM_REG_LR)));
    if asm.stack_adjust > 0 {
        if asm.stack_adjust < 0x100 {
            emit_al(
                asm,
                op_sub_imm(ASM_ARM_REG_SP, ASM_ARM_REG_SP, asm.stack_adjust),
            );
        } else {
            mov_reg_i32_optimised(asm, REG_TEMP, asm.stack_adjust as i32);
            emit_al(asm, op_sub_reg(ASM_ARM_REG_SP, ASM_ARM_REG_SP, REG_TEMP));
        }
    }
}

pub fn exit(asm: &mut AsmArm) {
    if !ENABLED {
        return;
    }
    if asm.stack_adjust > 0 {
        if asm.stack_adjust < 0x100 {
            emit_al(
                asm,
                op_add_imm(ASM_ARM_REG_SP, ASM_ARM_REG_SP, asm.stack_adjust),
            );
        } else {
            mov_reg_i32_optimised(asm, REG_TEMP, asm.stack_adjust as i32);
            emit_al(asm, op_add_reg(ASM_ARM_REG_SP, ASM_ARM_REG_SP, REG_TEMP));
        }
    }
    emit_al(asm, op_pop(asm.push_reglist | (1 << ASM_ARM_REG_PC)));
}

pub fn push(asm: &mut AsmArm, reglist: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_push(reglist));
}

pub fn pop(asm: &mut AsmArm, reglist: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_pop(reglist));
}

pub fn mov_reg_reg(asm: &mut AsmArm, reg_dest: u32, reg_src: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_mov_reg(reg_dest, reg_src));
}

pub fn mov_reg_i32(asm: &mut AsmArm, rd: u32, imm: i32) -> usize {
    if !ENABLED {
        return 0;
    }
    emit_al(asm, 0x59f0000 | (rd << 12));
    emit_al(asm, 0xa000000);
    let loc = asm.base.get_code_pos();
    emit(asm, imm as u32);
    loc
}

pub fn mov_reg_i32_optimised(asm: &mut AsmArm, rd: u32, imm: i32) {
    if !ENABLED {
        return;
    }
    if (imm & 0xff) == imm {
        emit_al(asm, op_mov_imm(rd, imm as u32));
    } else if imm < 0 && imm >= -256 {
        emit_al(asm, op_mvn_imm(rd, (!imm) as u32));
    } else {
        mov_reg_i32(asm, rd, imm);
    }
}

pub fn mov_local_reg(asm: &mut AsmArm, local_num: i32, rd: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x58d0000 | (rd << 12) | ((local_num as u32) << 2));
}

pub fn mov_reg_local(asm: &mut AsmArm, rd: u32, local_num: i32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x59d0000 | (rd << 12) | ((local_num as u32) << 2));
}

pub fn cmp_reg_i8(asm: &mut AsmArm, rd: u32, imm: i32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x3500000 | (rd << 16) | (imm as u32 & 0xff));
}

pub fn cmp_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1500000 | (rd << 16) | rn);
}

pub fn setcc_reg(asm: &mut AsmArm, rd: u32, cond: u32) {
    if !ENABLED {
        return;
    }
    emit(asm, op_mov_imm(rd, 1) | cond);
    emit(asm, op_mov_imm(rd, 0) | (cond ^ (1 << 28)));
}

pub fn mvn_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_mvn_reg(rd, rm));
}

pub fn add_reg_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_add_reg(rd, rn, rm));
}

pub fn rsb_reg_reg_imm(asm: &mut AsmArm, rd: u32, rn: u32, imm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_rsb_imm(rd, rn, imm));
}

pub fn sub_reg_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_sub_reg(rd, rn, rm));
}

pub fn mul_reg_reg_reg(asm: &mut AsmArm, rd: u32, rs: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_mul_reg(rd, rm, rs));
}

pub fn and_reg_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_and_reg(rd, rn, rm));
}

pub fn eor_reg_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_eor_reg(rd, rn, rm));
}

pub fn orr_reg_reg_reg(asm: &mut AsmArm, rd: u32, rn: u32, rm: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, op_orr_reg(rd, rn, rm));
}

pub fn mov_reg_local_addr(asm: &mut AsmArm, rd: u32, local_num: i32) {
    if !ENABLED {
        return;
    }
    if local_num >= 0x40 {
        mov_reg_i32_optimised(asm, REG_TEMP, local_num << 2);
        emit_al(asm, op_add_reg(rd, ASM_ARM_REG_SP, REG_TEMP));
    } else {
        emit_al(asm, op_add_imm(rd, ASM_ARM_REG_SP, (local_num as u32) << 2));
    }
}

pub fn mov_reg_pcrel(asm: &mut AsmArm, reg_dest: u32, label: usize) {
    if !ENABLED {
        return;
    }
    assert!(label < asm.base.max_num_labels);
    let dest = unsafe { *asm.base.label_offsets.add(label) };
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 12 + 8;

    emit_al(asm, 0x59f0000 | (reg_dest << 12));
    emit_al(asm, 0xa000000);
    emit(asm, rel as u32);
    add_reg_reg_reg(asm, reg_dest, reg_dest, ASM_ARM_REG_PC);
}

pub fn lsl_reg_reg(asm: &mut AsmArm, rd: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1a00010 | (rd << 12) | (rs << 8) | rd);
}

pub fn lsr_reg_reg(asm: &mut AsmArm, rd: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1a00030 | (rd << 12) | (rs << 8) | rd);
}

pub fn asr_reg_reg(asm: &mut AsmArm, rd: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1a00050 | (rd << 12) | (rs << 8) | rd);
}

pub fn ldr_reg_reg_offset(asm: &mut AsmArm, rd: u32, rn: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x1000 {
        emit_al(asm, 0x5900000 | (rn << 16) | (rd << 12) | byte_offset);
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x7900000 | (rn << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn ldrh_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1a00080 | (REG_TEMP << 12) | rn);
    emit_al(asm, 0x19000b0 | (rm << 16) | (rd << 12) | REG_TEMP);
}

pub fn ldrh_reg_reg_offset(asm: &mut AsmArm, rd: u32, rn: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x100 {
        emit_al(
            asm,
            0x1d000b0 | (rn << 16) | (rd << 12) | ((byte_offset & 0xf0) << 4) | (byte_offset & 0xf),
        );
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x19000b0 | (rn << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn ldrb_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x7d00000 | (rm << 16) | (rd << 12) | rn);
}

pub fn ldrb_reg_reg_offset(asm: &mut AsmArm, rd: u32, rn: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x1000 {
        emit_al(asm, 0x5d00000 | (rn << 16) | (rd << 12) | byte_offset);
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x7d00000 | (rn << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn ldr_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x7900100 | (rm << 16) | (rd << 12) | rn);
}

pub fn str_reg_reg_offset(asm: &mut AsmArm, rd: u32, rm: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x1000 {
        emit_al(asm, 0x5800000 | (rm << 16) | (rd << 12) | byte_offset);
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x7800000 | (rm << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn strh_reg_reg_offset(asm: &mut AsmArm, rd: u32, rn: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x100 {
        emit_al(
            asm,
            0x1c000b0 | (rn << 16) | (rd << 12) | ((byte_offset & 0xf0) << 4) | (byte_offset & 0xf),
        );
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x18000b0 | (rn << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn strb_reg_reg_offset(asm: &mut AsmArm, rd: u32, rm: u32, byte_offset: u32) {
    if !ENABLED {
        return;
    }
    if byte_offset < 0x1000 {
        emit_al(asm, 0x5c00000 | (rm << 16) | (rd << 12) | byte_offset);
    } else {
        mov_reg_i32_optimised(asm, REG_TEMP, byte_offset as i32);
        emit_al(asm, 0x7c00000 | (rm << 16) | (rd << 12) | REG_TEMP);
    }
}

pub fn str_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x7800100 | (rm << 16) | (rd << 12) | rn);
}

pub fn strh_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x1a00080 | (REG_TEMP << 12) | rn);
    emit_al(asm, 0x18000b0 | (rm << 16) | (rd << 12) | REG_TEMP);
}

pub fn strb_reg_reg_reg(asm: &mut AsmArm, rd: u32, rm: u32, rn: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x7c00000 | (rm << 16) | (rd << 12) | rn);
}

pub fn bcc_label(asm: &mut AsmArm, cond: i32, label: usize) {
    if !ENABLED {
        return;
    }
    assert!(label < asm.base.max_num_labels);
    let dest = unsafe { *asm.base.label_offsets.add(label) };
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 8;
    rel >>= 2;

    if misc::fit_signed(24, rel) {
        emit(asm, (cond as u32) | 0xa000000 | (rel as u32 & 0xffffff));
    }
}

pub fn b_label(asm: &mut AsmArm, label: usize) {
    if !ENABLED {
        return;
    }
    bcc_label(asm, ASM_ARM_CC_AL as i32, label);
}

pub fn bl_ind(asm: &mut AsmArm, fun_id: u32, _reg_temp: u32) {
    if !ENABLED {
        return;
    }
    assert!(fun_id < (0x1000 / 4));
    emit_al(asm, op_mov_reg(ASM_ARM_REG_LR, ASM_ARM_REG_PC));
    emit_al(asm, 0x597f000 | (fun_id << 2));
}

pub fn bx_reg(asm: &mut AsmArm, reg_src: u32) {
    if !ENABLED {
        return;
    }
    emit_al(asm, 0x012fff10 | reg_src);
}
