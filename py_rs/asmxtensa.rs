//! rewrite of py/asmxtensa.c + py/asmxtensa.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase, MP_ASM_PASS_EMIT};
use crate::misc;
use crate::mpconfig;
use crate::raise::{self, MpRaise};

const ENABLED: bool =
    mpconfig::EMIT_XTENSA || mpconfig::EMIT_INLINE_XTENSA || mpconfig::EMIT_XTENSAWIN;
const REG_TEMP: u32 = if mpconfig::EMIT_XTENSAWIN {
    ASM_XTENSA_REG_TEMPORARY_WIN
} else {
    ASM_XTENSA_REG_TEMPORARY
};
const WORD_SIZE: i32 = 4;

#[repr(C)]
pub struct AsmXtensa {
    pub base: MpAsmBase,
    pub cur_const: u32,
    pub num_const: u32,
    pub const_table: *mut u32,
    pub stack_adjust: u32,
}

pub const ASM_XTENSA_REG_A0: u32 = 0;
pub const ASM_XTENSA_REG_A1: u32 = 1;
pub const ASM_XTENSA_REG_A2: u32 = 2;
pub const ASM_XTENSA_REG_A3: u32 = 3;
pub const ASM_XTENSA_REG_A4: u32 = 4;
pub const ASM_XTENSA_REG_A5: u32 = 5;
pub const ASM_XTENSA_REG_A6: u32 = 6;
pub const ASM_XTENSA_REG_A7: u32 = 7;
pub const ASM_XTENSA_REG_A8: u32 = 8;
pub const ASM_XTENSA_REG_A9: u32 = 9;
pub const ASM_XTENSA_REG_A10: u32 = 10;
pub const ASM_XTENSA_REG_A11: u32 = 11;
pub const ASM_XTENSA_REG_A12: u32 = 12;
pub const ASM_XTENSA_REG_A13: u32 = 13;
pub const ASM_XTENSA_REG_A14: u32 = 14;
pub const ASM_XTENSA_REG_A15: u32 = 15;

pub const ASM_XTENSA_CCZ_EQ: u32 = 0;
pub const ASM_XTENSA_CCZ_NE: u32 = 1;
pub const ASM_XTENSA_CCZ_LT: u32 = 2;
pub const ASM_XTENSA_CCZ_GE: u32 = 3;
pub const ASM_XTENSA_CC_EQ: u32 = 1;
pub const ASM_XTENSA_CC_LT: u32 = 2;

pub const ASM_XTENSA_NUM_REGS_SAVED: u32 = 5;
pub const ASM_XTENSA_NUM_REGS_SAVED_WIN: u32 = 1;
pub const ASM_XTENSA_REG_FUN_TABLE: u32 = ASM_XTENSA_REG_A15;
pub const ASM_XTENSA_REG_FUN_TABLE_WIN: u32 = ASM_XTENSA_REG_A7;
pub const ASM_XTENSA_REG_TEMPORARY: u32 = ASM_XTENSA_REG_A6;
pub const ASM_XTENSA_REG_TEMPORARY_WIN: u32 = ASM_XTENSA_REG_A12;

#[inline]
pub const fn encode_rrr(op0: u32, op1: u32, op2: u32, r: u32, s: u32, t: u32) -> u32 {
    (op2 << 20) | (op1 << 16) | (r << 12) | (s << 8) | (t << 4) | op0
}

#[inline]
pub const fn encode_rri8(op0: u32, r: u32, s: u32, t: u32, imm8: u32) -> u32 {
    (imm8 << 16) | (r << 12) | (s << 8) | (t << 4) | op0
}

#[inline]
const fn encode_ri16(op0: u32, t: u32, imm16: u32) -> u32 {
    (imm16 << 8) | (t << 4) | op0
}

#[inline]
const fn encode_call(op0: u32, n: u32, offset: u32) -> u32 {
    (offset << 6) | (n << 4) | op0
}

#[inline]
const fn encode_callx(op0: u32, op1: u32, op2: u32, r: u32, s: u32, m: u32, n: u32) -> u32 {
    (op2 << 20) | (op1 << 16) | (r << 12) | (s << 8) | (m << 6) | (n << 4) | op0
}

#[inline]
const fn encode_bri12(op0: u32, s: u32, m: u32, n: u32, imm12: u32) -> u32 {
    (imm12 << 12) | (s << 8) | (m << 6) | (n << 4) | op0
}

#[inline]
pub const fn encode_rrrn(op0: u32, r: u32, s: u32, t: u32) -> u16 {
    (r << 12) as u16 | (s << 8) as u16 | (t << 4) as u16 | op0 as u16
}

#[inline]
const fn encode_ri7(op0: u32, s: u32, imm7: i32) -> u16 {
    (((imm7 as u32) & 0xf) << 12) as u16 | (s << 8) as u16 | ((imm7 as u32) & 0x70) as u16 | op0 as u16
}

#[inline]
const fn signed_fit8(x: i32) -> bool {
    misc::fit_signed(8, x)
}

#[inline]
const fn signed_fit12(x: i32) -> bool {
    misc::fit_signed(12, x)
}

#[inline]
const fn signed_fit18(x: i32) -> bool {
    misc::fit_signed(18, x)
}

fn get_label_dest(state: &AsmXtensa, label: usize) -> u32 {
    assert!(label < state.base.max_num_labels);
    unsafe { *state.base.label_offsets.add(label) as u32 }
}

pub fn op16(state: &mut AsmXtensa, op: u16) {
    if !ENABLED {
        return;
    }
    let c = asmbase::get_cur_to_write_bytes(&mut state.base, 2);
    if !c.is_null() {
        unsafe {
            *c = op as u8;
            *c.add(1) = (op >> 8) as u8;
        }
    }
}

pub fn op24(state: &mut AsmXtensa, op: u32) {
    if !ENABLED {
        return;
    }
    let c = asmbase::get_cur_to_write_bytes(&mut state.base, 3);
    if !c.is_null() {
        unsafe {
            *c = op as u8;
            *c.add(1) = (op >> 8) as u8;
            *c.add(2) = (op >> 16) as u8;
        }
    }
}

pub fn op_entry(state: &mut AsmXtensa, reg_src: u32, num_bytes: i32) {
    op24(
        state,
        encode_bri12(6, reg_src, 0, 3, ((num_bytes / 8) & 0xfff) as u32),
    );
}

pub fn op_add_n(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op16(state, encode_rrrn(10, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_addi(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32, imm8: i32) {
    op24(
        state,
        encode_rri8(2, 12, reg_src, reg_dest, (imm8 & 0xff) as u32),
    );
}

pub fn op_addx2(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 9, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_addx4(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 10, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_and(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 1, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_bcc(state: &mut AsmXtensa, cond: u32, reg_src1: u32, reg_src2: u32, rel8: i32) {
    op24(
        state,
        encode_rri8(7, cond, reg_src1, reg_src2, (rel8 & 0xff) as u32),
    );
}

pub fn op_bccz(state: &mut AsmXtensa, cond: u32, reg_src: u32, rel12: i32) {
    op24(
        state,
        encode_bri12(6, reg_src, cond, 1, (rel12 & 0xfff) as u32),
    );
}

pub fn op_call0(state: &mut AsmXtensa, rel18: i32) {
    op24(state, encode_call(5, 0, (rel18 & 0x3ffff) as u32));
}

pub fn op_callx0(state: &mut AsmXtensa, reg: u32) {
    op24(state, encode_callx(0, 0, 0, 0, reg, 3, 0));
}

pub fn op_callx8(state: &mut AsmXtensa, reg: u32) {
    op24(state, encode_callx(0, 0, 0, 0, reg, 3, 2));
}

pub fn op_j(state: &mut AsmXtensa, rel18: i32) {
    op24(state, encode_call(6, 0, (rel18 & 0x3ffff) as u32));
}

pub fn op_jx(state: &mut AsmXtensa, reg: u32) {
    op24(state, encode_callx(0, 0, 0, 0, reg, 2, 2));
}

pub fn op_l8ui(state: &mut AsmXtensa, reg_dest: u32, reg_base: u32, byte_offset: u32) {
    op24(
        state,
        encode_rri8(2, 0, reg_base, reg_dest, byte_offset & 0xff),
    );
}

pub fn op_l16ui(state: &mut AsmXtensa, reg_dest: u32, reg_base: u32, half_word_offset: u32) {
    op24(
        state,
        encode_rri8(2, 1, reg_base, reg_dest, half_word_offset & 0xff),
    );
}

pub fn op_l32i(state: &mut AsmXtensa, reg_dest: u32, reg_base: u32, word_offset: u32) {
    op24(
        state,
        encode_rri8(2, 2, reg_base, reg_dest, word_offset & 0xff),
    );
}

pub fn op_l32i_n(state: &mut AsmXtensa, reg_dest: u32, reg_base: u32, word_offset: u32) {
    op16(state, encode_rrrn(8, word_offset & 0xf, reg_base, reg_dest));
}

pub fn op_l32r(state: &mut AsmXtensa, reg_dest: u32, op_off: u32, dest_off: u32) {
    op24(
        state,
        encode_ri16(1, reg_dest, ((dest_off - ((op_off + 3) & !3)) >> 2) & 0xffff),
    );
}

pub fn op_mov_n(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32) {
    op16(state, encode_rrrn(13, 0, reg_src, reg_dest));
}

pub fn op_movi(state: &mut AsmXtensa, reg_dest: u32, imm12: i32) {
    op24(
        state,
        encode_rri8(
            2,
            10,
            ((imm12 >> 8) & 0xf) as u32,
            reg_dest,
            (imm12 & 0xff) as u32,
        ),
    );
}

pub fn op_movi_n(state: &mut AsmXtensa, reg_dest: u32, imm7: i32) {
    op16(state, encode_ri7(12, reg_dest, imm7));
}

pub fn op_mull(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 2, 8, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_neg(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32) {
    op24(state, encode_rrr(0, 0, 6, reg_dest, 0, reg_src));
}

pub fn op_or(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 2, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_ret_n(state: &mut AsmXtensa) {
    op16(state, encode_rrrn(13, 15, 0, 0));
}

pub fn op_retw_n(state: &mut AsmXtensa) {
    op16(state, encode_rrrn(13, 15, 0, 1));
}

pub fn op_s8i(state: &mut AsmXtensa, reg_src: u32, reg_base: u32, byte_offset: u32) {
    op24(
        state,
        encode_rri8(2, 4, reg_base, reg_src, byte_offset & 0xff),
    );
}

pub fn op_s16i(state: &mut AsmXtensa, reg_src: u32, reg_base: u32, half_word_offset: u32) {
    op24(
        state,
        encode_rri8(2, 5, reg_base, reg_src, half_word_offset & 0xff),
    );
}

pub fn op_s32i(state: &mut AsmXtensa, reg_src: u32, reg_base: u32, word_offset: u32) {
    op24(
        state,
        encode_rri8(2, 6, reg_base, reg_src, word_offset & 0xff),
    );
}

pub fn op_s32i_n(state: &mut AsmXtensa, reg_src: u32, reg_base: u32, word_offset: u32) {
    op16(state, encode_rrrn(9, word_offset & 0xf, reg_base, reg_src));
}

pub fn op_sll(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32) {
    op24(state, encode_rrr(0, 1, 10, reg_dest, reg_src, 0));
}

pub fn op_srl(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32) {
    op24(state, encode_rrr(0, 1, 9, reg_dest, 0, reg_src));
}

pub fn op_sra(state: &mut AsmXtensa, reg_dest: u32, reg_src: u32) {
    op24(state, encode_rrr(0, 1, 11, reg_dest, 0, reg_src));
}

pub fn op_ssl(state: &mut AsmXtensa, reg_src: u32) {
    op24(state, encode_rrr(0, 0, 4, 1, reg_src, 0));
}

pub fn op_ssr(state: &mut AsmXtensa, reg_src: u32) {
    op24(state, encode_rrr(0, 0, 4, 0, reg_src, 0));
}

pub fn op_sub(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 12, reg_dest, reg_src_a, reg_src_b));
}

pub fn op_xor(state: &mut AsmXtensa, reg_dest: u32, reg_src_a: u32, reg_src_b: u32) {
    op24(state, encode_rrr(0, 0, 3, reg_dest, reg_src_a, reg_src_b));
}

pub fn end_pass(state: &mut AsmXtensa) {
    if !ENABLED {
        return;
    }
    state.num_const = state.cur_const;
    state.cur_const = 0;
}

pub fn entry(state: &mut AsmXtensa, num_locals: i32) {
    if !ENABLED {
        return;
    }
    if state.num_const > 0 {
        op_j(state, (state.num_const * WORD_SIZE as u32 + 4 - 4) as i32);
        asmbase::get_cur_to_write_bytes(&mut state.base, 1);
        state.const_table =
            asmbase::get_cur_to_write_bytes(&mut state.base, state.num_const as usize * 4)
                as *mut u32;
    }

    state.stack_adjust =
        (((ASM_XTENSA_NUM_REGS_SAVED + num_locals as u32) * WORD_SIZE as u32) + 15) & !15;
    if signed_fit8(-(state.stack_adjust as i32)) {
        op_addi(
            state,
            ASM_XTENSA_REG_A1,
            ASM_XTENSA_REG_A1,
            -(state.stack_adjust as i32),
        );
    } else {
        op_movi(state, ASM_XTENSA_REG_A9, state.stack_adjust as i32);
        op_sub(
            state,
            ASM_XTENSA_REG_A1,
            ASM_XTENSA_REG_A1,
            ASM_XTENSA_REG_A9,
        );
    }

    op_s32i_n(state, ASM_XTENSA_REG_A0, ASM_XTENSA_REG_A1, 0);
    for i in 1..ASM_XTENSA_NUM_REGS_SAVED {
        op_s32i_n(
            state,
            ASM_XTENSA_REG_A11 + i,
            ASM_XTENSA_REG_A1,
            i,
        );
    }
}

pub fn exit(state: &mut AsmXtensa) {
    if !ENABLED {
        return;
    }
    for i in (1..ASM_XTENSA_NUM_REGS_SAVED).rev() {
        op_l32i_n(
            state,
            ASM_XTENSA_REG_A11 + i,
            ASM_XTENSA_REG_A1,
            i,
        );
    }
    op_l32i_n(state, ASM_XTENSA_REG_A0, ASM_XTENSA_REG_A1, 0);

    if signed_fit8(state.stack_adjust as i32) {
        op_addi(
            state,
            ASM_XTENSA_REG_A1,
            ASM_XTENSA_REG_A1,
            state.stack_adjust as i32,
        );
    } else {
        op_movi(state, ASM_XTENSA_REG_A9, state.stack_adjust as i32);
        op_add_n(state, ASM_XTENSA_REG_A1, ASM_XTENSA_REG_A1, ASM_XTENSA_REG_A9);
    }
    op_ret_n(state);
}

pub fn entry_win(state: &mut AsmXtensa, num_locals: i32) {
    if !ENABLED {
        return;
    }
    if state.num_const > 0 {
        op_j(state, (state.num_const * WORD_SIZE as u32 + 4 - 4) as i32);
        asmbase::get_cur_to_write_bytes(&mut state.base, 1);
        state.const_table =
            asmbase::get_cur_to_write_bytes(&mut state.base, state.num_const as usize * 4)
                as *mut u32;
    }

    state.stack_adjust = 32
        + ((((ASM_XTENSA_NUM_REGS_SAVED_WIN + num_locals as u32) * WORD_SIZE as u32) + 15) & !15);
    op_entry(state, ASM_XTENSA_REG_A1, state.stack_adjust as i32);
    op_s32i_n(state, ASM_XTENSA_REG_A0, ASM_XTENSA_REG_A1, 0);
}

pub fn exit_win(state: &mut AsmXtensa) {
    if !ENABLED {
        return;
    }
    op_l32i_n(state, ASM_XTENSA_REG_A0, ASM_XTENSA_REG_A1, 0);
    op_retw_n(state);
}

pub fn j_label(state: &mut AsmXtensa, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(state, label);
    let rel = dest as i32 - state.base.code_offset as i32 - 4;
    op_j(state, rel);
}

fn calculate_branch_displacement(state: &AsmXtensa, label: usize) -> (bool, i32) {
    let label_offset = get_label_dest(state, label);
    let displacement = label_offset as i32 - state.base.code_offset as i32 - 4;
    (label_offset != u32::MAX && displacement < 0, displacement)
}

pub fn bccz_reg_label(state: &mut AsmXtensa, cond: u32, reg: u32, label: usize) {
    if !ENABLED {
        return;
    }
    let (can_emit_short_jump, rel) = calculate_branch_displacement(state, label);

    if can_emit_short_jump && signed_fit12(rel) {
        op_bccz(state, cond, reg, rel);
        return;
    }

    if state.base.pass == MP_ASM_PASS_EMIT && !signed_fit18(rel - 6) {
        raise::raise(MpRaise::RuntimeError("ERROR: xtensa bccz out of range"));
    }

    op_bccz(state, cond ^ 1, reg, 6 - 4);
    op_j(state, rel - 3);
}

pub fn bcc_reg_reg_label(
    state: &mut AsmXtensa,
    cond: u32,
    reg1: u32,
    reg2: u32,
    label: usize,
) {
    if !ENABLED {
        return;
    }
    let (can_emit_short_jump, rel) = calculate_branch_displacement(state, label);

    if can_emit_short_jump && signed_fit8(rel) {
        op_bcc(state, cond, reg1, reg2, rel);
        return;
    }

    if state.base.pass == MP_ASM_PASS_EMIT && !signed_fit18(rel - 6) {
        raise::raise(MpRaise::RuntimeError("ERROR: xtensa bcc out of range"));
    }

    op_bcc(state, cond ^ 8, reg1, reg2, 6 - 4);
    op_j(state, rel - 3);
}

pub fn setcc_reg_reg_reg(
    state: &mut AsmXtensa,
    cond: u32,
    reg_dest: u32,
    reg_src1: u32,
    reg_src2: u32,
) {
    if !ENABLED {
        return;
    }
    op_movi_n(state, reg_dest, 1);
    op_bcc(state, cond, reg_src1, reg_src2, 1);
    op_movi_n(state, reg_dest, 0);
}

pub fn mov_reg_i32(state: &mut AsmXtensa, reg_dest: u32, i32: u32) -> usize {
    if !ENABLED {
        return 0;
    }
    let const_table_offset = if state.const_table.is_null() {
        0
    } else {
        unsafe {
            (state.const_table as *const u8).offset_from(state.base.code_base as *const u8) as u32
        }
    };
    let loc = const_table_offset as usize + state.cur_const as usize * WORD_SIZE as usize;
    op_l32r(state, reg_dest, state.base.code_offset as u32, loc as u32);
    if !state.const_table.is_null() {
        unsafe {
            *state.const_table.add(state.cur_const as usize) = i32;
        }
    } else {
        assert!(state.base.pass != MP_ASM_PASS_EMIT);
    }
    state.cur_const += 1;
    loc
}

pub fn mov_reg_i32_optimised(state: &mut AsmXtensa, reg_dest: u32, i32: u32) {
    if !ENABLED {
        return;
    }
    let si = i32 as i32;
    if (-32..=95).contains(&si) {
        op_movi_n(state, reg_dest, si);
    } else if signed_fit12(si) {
        op_movi(state, reg_dest, si);
    } else {
        mov_reg_i32(state, reg_dest, i32);
    }
}

pub fn mov_local_reg(state: &mut AsmXtensa, local_num: i32, reg_src: u32) {
    if !ENABLED {
        return;
    }
    op_s32i(state, reg_src, ASM_XTENSA_REG_A1, local_num as u32);
}

pub fn mov_reg_local(state: &mut AsmXtensa, reg_dest: u32, local_num: i32) {
    if !ENABLED {
        return;
    }
    op_l32i(state, reg_dest, ASM_XTENSA_REG_A1, local_num as u32);
}

pub fn mov_reg_local_addr(state: &mut AsmXtensa, reg_dest: u32, local_num: i32) {
    if !ENABLED {
        return;
    }
    let off = (local_num * WORD_SIZE) as u32;
    if signed_fit8(off as i32) {
        op_addi(state, reg_dest, ASM_XTENSA_REG_A1, off as i32);
    } else {
        op_movi(state, reg_dest, off as i32);
        op_add_n(state, reg_dest, reg_dest, ASM_XTENSA_REG_A1);
    }
}

pub fn mov_reg_pcrel(state: &mut AsmXtensa, reg_dest: u32, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(state, label);
    let mut rel = dest as i32 - state.base.code_offset as i32;
    rel -= 3 + 3;
    op_movi(state, reg_dest, rel);

    let off = (state.base.code_offset >> 1) & 1;
    let pad = (5 - state.base.code_offset) & 3;
    op_call0(state, off as i32);
    asmbase::get_cur_to_write_bytes(&mut state.base, pad);
    op_add_n(state, reg_dest, reg_dest, ASM_XTENSA_REG_A0);
}

fn l32i_optimised(state: &mut AsmXtensa, reg_dest: u32, reg_base: u32, word_offset: u32) {
    if word_offset < 16 {
        op_l32i_n(state, reg_dest, reg_base, word_offset);
    } else if word_offset < 256 {
        op_l32i(state, reg_dest, reg_base, word_offset);
    } else {
        mov_reg_i32_optimised(state, reg_dest, word_offset * 4);
        op_add_n(state, reg_dest, reg_base, reg_dest);
        op_l32i_n(state, reg_dest, reg_dest, 0);
    }
}

pub fn load_reg_reg_offset(
    state: &mut AsmXtensa,
    reg_dest: u32,
    reg_base: u32,
    offset: u32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    assert!(operation_size <= 2);

    if operation_size == 2 && misc::fit_unsigned(4, offset) {
        op_l32i_n(state, reg_dest, reg_base, offset);
        return;
    }

    if misc::fit_unsigned(8, offset) {
        op24(
            state,
            encode_rri8(2, operation_size, reg_base, reg_dest, offset),
        );
        return;
    }

    mov_reg_i32_optimised(state, reg_dest, offset << operation_size);
    op_add_n(state, reg_dest, reg_base, reg_dest);
    if operation_size == 2 {
        op_l32i_n(state, reg_dest, reg_dest, 0);
    } else {
        op24(
            state,
            encode_rri8(2, operation_size, reg_dest, reg_dest, 0),
        );
    }
}

pub fn store_reg_reg_offset(
    state: &mut AsmXtensa,
    reg_src: u32,
    reg_base: u32,
    offset: u32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    assert!(operation_size <= 2);

    if operation_size == 2 && misc::fit_unsigned(4, offset) {
        op_s32i_n(state, reg_src, reg_base, offset);
        return;
    }

    if misc::fit_unsigned(8, offset) {
        op24(
            state,
            encode_rri8(2, 0x04 | operation_size, reg_base, reg_src, offset),
        );
        return;
    }

    mov_reg_i32_optimised(state, REG_TEMP, offset << operation_size);
    op_add_n(state, REG_TEMP, reg_base, REG_TEMP);
    if operation_size == 2 {
        op_s32i_n(state, reg_src, REG_TEMP, 0);
    } else {
        op24(
            state,
            encode_rri8(2, 0x04 | operation_size, REG_TEMP, reg_src, 0),
        );
    }
}

pub fn call_ind(state: &mut AsmXtensa, idx: u32) {
    if !ENABLED {
        return;
    }
    l32i_optimised(state, ASM_XTENSA_REG_A0, ASM_XTENSA_REG_FUN_TABLE, idx);
    op_callx0(state, ASM_XTENSA_REG_A0);
}

pub fn call_ind_win(state: &mut AsmXtensa, idx: u32) {
    if !ENABLED {
        return;
    }
    l32i_optimised(state, ASM_XTENSA_REG_A8, ASM_XTENSA_REG_FUN_TABLE_WIN, idx);
    op_callx8(state, ASM_XTENSA_REG_A8);
}

pub fn bit_branch(
    state: &mut AsmXtensa,
    reg: usize,
    bit: usize,
    label: usize,
    condition: u32,
) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(state, label);
    let rel = dest as i32 - state.base.code_offset as i32 - 4;
    if state.base.pass == MP_ASM_PASS_EMIT && !signed_fit8(rel) {
        raise::raise(MpRaise::RuntimeError("ERROR: xtensa bit_branch out of range"));
    }
    op24(
        state,
        encode_rri8(
            7,
            condition | (((bit >> 4) & 0x01) as u32),
            reg as u32,
            (bit & 0x0f) as u32,
            (rel & 0xff) as u32,
        ),
    );
}

pub fn call0(state: &mut AsmXtensa, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(state, label);
    let rel = dest as i32 - state.base.code_offset as i32 - 3;
    if state.base.pass == MP_ASM_PASS_EMIT {
        if (dest & 0x03) != 0 {
            raise::raise(MpRaise::RuntimeError("ERROR: call0 target not word-aligned"));
        }
        if (rel & 0x03) != 0 {
            raise::raise(MpRaise::RuntimeError("ERROR: call0 location not word-aligned"));
        }
        if !signed_fit18(rel) {
            raise::raise(MpRaise::RuntimeError("ERROR: xtensa call0 out of range"));
        }
    }
    op_call0(state, rel);
}

pub fn l32r(state: &mut AsmXtensa, reg: usize, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(state, label);
    let rel = dest as i32 - state.base.code_offset as i32;
    if state.base.pass == MP_ASM_PASS_EMIT {
        if (dest & 0x03) != 0 {
            raise::raise(MpRaise::RuntimeError("ERROR: l32r target not word-aligned"));
        }
        if (rel & 0x03) != 0 {
            raise::raise(MpRaise::RuntimeError("ERROR: l32r location not word-aligned"));
        }
        if !signed_fit18(rel) || rel >= 0 {
            raise::raise(MpRaise::RuntimeError("ERROR: xtensa l32r out of range"));
        }
    }
    op_l32r(state, reg as u32, state.base.code_offset as u32, dest);
}
