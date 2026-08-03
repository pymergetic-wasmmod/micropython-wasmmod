//! rewrite of py/asmthumb.c + py/asmthumb.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase, MP_ASM_PASS_EMIT};
use crate::misc::{self, mp_clz, mp_ctz};
use crate::mpconfig;
use crate::raise::{self, MpRaise};

const ENABLED: bool = mpconfig::EMIT_THUMB || mpconfig::EMIT_INLINE_THUMB;

#[repr(C)]
pub struct AsmThumb {
    pub base: MpAsmBase,
    pub push_reglist: u32,
    pub stack_adjust: u32,
}

pub const ASM_THUMB_REG_R0: u32 = 0;
pub const ASM_THUMB_REG_R1: u32 = 1;
pub const ASM_THUMB_REG_R2: u32 = 2;
pub const ASM_THUMB_REG_R3: u32 = 3;
pub const ASM_THUMB_REG_R4: u32 = 4;
pub const ASM_THUMB_REG_R5: u32 = 5;
pub const ASM_THUMB_REG_R6: u32 = 6;
pub const ASM_THUMB_REG_R7: u32 = 7;
pub const ASM_THUMB_REG_R8: u32 = 8;
pub const ASM_THUMB_REG_R9: u32 = 9;
pub const ASM_THUMB_REG_R10: u32 = 10;
pub const ASM_THUMB_REG_R11: u32 = 11;
pub const ASM_THUMB_REG_R12: u32 = 12;
pub const ASM_THUMB_REG_R13: u32 = 13;
pub const ASM_THUMB_REG_R14: u32 = 14;
pub const ASM_THUMB_REG_R15: u32 = 15;
pub const ASM_THUMB_REG_SP: u32 = ASM_THUMB_REG_R13;
pub const ASM_THUMB_REG_LR: u32 = ASM_THUMB_REG_R14;

pub const ASM_THUMB_CC_EQ: i32 = 0x0;
pub const ASM_THUMB_CC_NE: i32 = 0x1;
pub const ASM_THUMB_CC_CS: i32 = 0x2;
pub const ASM_THUMB_CC_CC: i32 = 0x3;
pub const ASM_THUMB_CC_MI: i32 = 0x4;
pub const ASM_THUMB_CC_PL: i32 = 0x5;
pub const ASM_THUMB_CC_VS: i32 = 0x6;
pub const ASM_THUMB_CC_VC: i32 = 0x7;
pub const ASM_THUMB_CC_HI: i32 = 0x8;
pub const ASM_THUMB_CC_LS: i32 = 0x9;
pub const ASM_THUMB_CC_GE: i32 = 0xa;
pub const ASM_THUMB_CC_LT: i32 = 0xb;
pub const ASM_THUMB_CC_GT: i32 = 0xc;
pub const ASM_THUMB_CC_LE: i32 = 0xd;

pub const ASM_THUMB_OP_NOP: u16 = 0xbf00;
pub const ASM_THUMB_OP_WFI: u16 = 0xbf30;
pub const ASM_THUMB_OP_CPSID_I: u16 = 0xb672;
pub const ASM_THUMB_OP_CPSIE_I: u16 = 0xb662;
pub const ASM_THUMB_OP_MOVW: u16 = 0xf240;
pub const ASM_THUMB_OP_MOVT: u16 = 0xf2c0;
pub const ASM_THUMB_REG_FUN_TABLE: u32 = ASM_THUMB_REG_R7;

pub const ASM_THUMB_FORMAT_1_LSL: u16 = 0x0000;
pub const ASM_THUMB_FORMAT_1_LSR: u16 = 0x0800;
pub const ASM_THUMB_FORMAT_1_ASR: u16 = 0x1000;
pub const ASM_THUMB_FORMAT_2_ADD: u16 = 0x1800;
pub const ASM_THUMB_FORMAT_2_SUB: u16 = 0x1a00;
pub const ASM_THUMB_FORMAT_2_REG_OPERAND: u16 = 0x0000;
pub const ASM_THUMB_FORMAT_2_IMM_OPERAND: u16 = 0x0400;
pub const ASM_THUMB_FORMAT_3_MOV: u16 = 0x2000;
pub const ASM_THUMB_FORMAT_3_CMP: u16 = 0x2800;
pub const ASM_THUMB_FORMAT_3_ADD: u16 = 0x3000;
pub const ASM_THUMB_FORMAT_3_SUB: u16 = 0x3800;
pub const ASM_THUMB_FORMAT_4_AND: u16 = 0x4000;
pub const ASM_THUMB_FORMAT_4_EOR: u32 = 0x4040;
pub const ASM_THUMB_FORMAT_4_LSL: u32 = 0x4080;
pub const ASM_THUMB_FORMAT_4_LSR: u32 = 0x40c0;
pub const ASM_THUMB_FORMAT_4_ASR: u32 = 0x4100;
pub const ASM_THUMB_FORMAT_4_ADC: u32 = 0x4140;
pub const ASM_THUMB_FORMAT_4_SBC: u32 = 0x4180;
pub const ASM_THUMB_FORMAT_4_ROR: u32 = 0x41c0;
pub const ASM_THUMB_FORMAT_4_TST: u32 = 0x4200;
pub const ASM_THUMB_FORMAT_4_NEG: u16 = 0x4240;
pub const ASM_THUMB_FORMAT_4_CMP: u16 = 0x4280;
pub const ASM_THUMB_FORMAT_4_CMN: u16 = 0x42c0;
pub const ASM_THUMB_FORMAT_4_ORR: u16 = 0x4300;
pub const ASM_THUMB_FORMAT_4_MUL: u16 = 0x4340;
pub const ASM_THUMB_FORMAT_4_BIC: u16 = 0x4380;
pub const ASM_THUMB_FORMAT_4_MVN: u16 = 0x43c0;
pub const ASM_THUMB_FORMAT_9_STR: u16 = 0x6000;
pub const ASM_THUMB_FORMAT_9_LDR: u16 = 0x6800;
pub const ASM_THUMB_FORMAT_9_WORD_TRANSFER: u16 = 0x0000;
pub const ASM_THUMB_FORMAT_9_BYTE_TRANSFER: u16 = 0x1000;
pub const ASM_THUMB_FORMAT_10_STRH: u16 = 0x8000;
pub const ASM_THUMB_FORMAT_10_LDRH: u16 = 0x8800;

const ASM_THUMB_FORMAT_3_LDR: u16 = 0x4800;
const ASM_THUMB_FORMAT_5_ADD: u16 = 0x4400;
const ASM_THUMB_FORMAT_5_BX: u16 = 0x4700;
const ASM_THUMB_FORMAT_7_LDR: u16 = 0x5800;
const ASM_THUMB_FORMAT_7_STR: u16 = 0x5000;
const ASM_THUMB_FORMAT_7_WORD_TRANSFER: u16 = 0x0000;
const ASM_THUMB_FORMAT_7_BYTE_TRANSFER: u16 = 0x0400;
const ASM_THUMB_FORMAT_8_LDRH: u16 = 0x5a00;
const ASM_THUMB_FORMAT_8_STRH: u16 = 0x5200;
const ASM_THUMB_FORMAT_11_SXTH: u16 = 0xb200;

const OP_LDR_STR_TABLE: [u16; 3] = [0x0e, 0x10, 0x0c];
const OP_LDR: u16 = 0x01;
const OP_STR: u16 = 0x00;
const OP_LDR_W: u16 = 0x10;
const OP_STR_W: u16 = 0x00;

#[inline]
const fn unsigned_fit7(x: u32) -> bool {
    x < 128
}

#[inline]
const fn unsigned_fit8(x: i32) -> bool {
    (x as u32 & 0xffffff00) == 0
}

#[inline]
const fn unsigned_fit16(x: i32) -> bool {
    (x as u32 & 0xffff0000) == 0
}

#[inline]
const fn signed_fit8(x: i32) -> bool {
    misc::fit_signed(8, x)
}

#[inline]
const fn signed_fit9(x: i32) -> bool {
    misc::fit_signed(9, x)
}

#[inline]
const fn signed_fit12(x: i32) -> bool {
    misc::fit_signed(12, x)
}

#[inline]
const fn signed_fit23(x: i32) -> bool {
    misc::fit_signed(23, x)
}

#[inline]
const fn op_add_w_rri_hi(reg_src: u32) -> u16 {
    0xf200 | reg_src as u16
}

#[inline]
const fn op_add_w_rri_lo(reg_dest: u32, imm11: u32) -> u16 {
    ((imm11 << 4) & 0x7000) as u16 | (reg_dest << 8) as u16 | (imm11 & 0xff) as u16
}

#[inline]
const fn op_sub_w_rri_hi(reg_src: u32) -> u16 {
    0xf2a0 | reg_src as u16
}

#[inline]
const fn op_sub_w_rri_lo(reg_dest: u32, imm11: u32) -> u16 {
    ((imm11 << 4) & 0x7000) as u16 | (reg_dest << 8) as u16 | (imm11 & 0xff) as u16
}

#[inline]
const fn op_push_rlist(rlolist: u32) -> u16 {
    0xb400 | rlolist as u16
}

#[inline]
const fn op_push_rlist_lr(rlolist: u32) -> u16 {
    0xb400 | 0x0100 | rlolist as u16
}

#[inline]
const fn op_pop_rlist_pc(rlolist: u32) -> u16 {
    0xbc00 | 0x0100 | rlolist as u16
}

#[inline]
const fn op_sub_sp(num_words: u32) -> u16 {
    0xb080 | num_words as u16
}

#[inline]
const fn op_add_sp(num_words: u32) -> u16 {
    0xb000 | num_words as u16
}

#[inline]
const fn op_b_n(byte_offset: i32) -> u16 {
    0xe000 | (((byte_offset >> 1) & 0x07ff) as u16)
}

#[inline]
const fn op_bcc_n(cond: i32, byte_offset: i32) -> u16 {
    0xd000 | ((cond as u16) << 8) | (((byte_offset >> 1) & 0x00ff) as u16)
}

#[inline]
const fn op_bcc_w_hi(cond: i32, byte_offset: i32) -> u16 {
    0xf000
        | ((cond as u16) << 6)
        | (((byte_offset >> 10) & 0x0400) as u16)
        | (((byte_offset >> 12) & 0x003f) as u16)
}

#[inline]
const fn op_bcc_w_lo(byte_offset: i32) -> u16 {
    0x8000
        | (((byte_offset >> 5) & 0x2000) as u16)
        | (((byte_offset >> 8) & 0x0800) as u16)
        | (((byte_offset >> 1) & 0x07ff) as u16)
}

#[inline]
const fn op_bl_hi(byte_offset: i32) -> u16 {
    0xf000 | (((byte_offset >> 12) & 0x07ff) as u16)
}

#[inline]
const fn op_bl_lo(byte_offset: i32) -> u16 {
    0xf800 | (((byte_offset >> 1) & 0x07ff) as u16)
}

#[inline]
const fn op_bw_hi(byte_offset: i32) -> u16 {
    0xf000 | (((byte_offset >> 12) & 0x07ff) as u16)
}

#[inline]
const fn op_bw_lo(byte_offset: i32) -> u16 {
    0xb800 | (((byte_offset >> 1) & 0x07ff) as u16)
}

#[inline]
const fn op_blx(reg: u32) -> u16 {
    0x4780 | ((reg << 3) as u16)
}

#[inline]
const fn op_format_4(op: u16, rlo_dest: u32, rlo_src: u32) -> u16 {
    op | ((rlo_src << 3) as u16) | rlo_dest as u16
}

#[inline]
const fn op_format_1_encode(op: u16, rlo_dest: u32, rlo_src: u32, offset: u32) -> u16 {
    op | ((offset << 6) as u16) | ((rlo_src << 3) as u16) | rlo_dest as u16
}

#[inline]
const fn op_format_2_encode(op: u16, rlo_dest: u32, rlo_src: u32, src_b: u32) -> u16 {
    op | ((src_b << 6) as u16) | ((rlo_src << 3) as u16) | rlo_dest as u16
}

#[inline]
const fn op_format_3_encode(op: u16, rlo: u32, i8: u32) -> u16 {
    op | ((rlo << 8) as u16) | (i8 as u16)
}

#[inline]
const fn op_format_5_encode(op: u16, r_dest: u32, r_src: u32) -> u16 {
    op | ((r_dest << 4) & 0x0080) as u16 | ((r_src << 3) as u16) | (r_dest & 0x0007) as u16
}

#[inline]
const fn op_format_7_8_encode(op: u16, rlo_dest: u32, rlo_base: u32, rlo_index: u32) -> u16 {
    op | ((rlo_index << 6) as u16) | ((rlo_base << 3) as u16) | rlo_dest as u16
}

#[inline]
const fn op_format_11_encode(op: u16, rlo_dest: u32, rlo_src: u32) -> u16 {
    op | ((rlo_src << 3) as u16) | rlo_dest as u16
}

#[inline]
const fn op_str_to_sp_offset(rlo_dest: u32, word_offset: u32) -> u16 {
    0x9000 | ((rlo_dest << 8) as u16) | ((word_offset & 0x00ff) as u16)
}

#[inline]
const fn op_ldr_from_sp_offset(rlo_dest: u32, word_offset: u32) -> u16 {
    0x9800 | ((rlo_dest << 8) as u16) | ((word_offset & 0x00ff) as u16)
}

#[inline]
const fn op_add_reg_sp_offset(rlo_dest: u32, word_offset: u32) -> u16 {
    0xa800 | ((rlo_dest << 8) as u16) | ((word_offset & 0x00ff) as u16)
}

#[inline]
const fn op_ldr_str_w_hi(operation_size: u32, reg: u32) -> u16 {
    (0xf880 | (operation_size << 5) | reg) as u16
}

#[inline]
const fn op_ldr_str_w_lo(reg: u32, imm12: u32) -> u16 {
    ((reg << 12) | imm12) as u16
}

pub fn allow_armv7m(_asm: &AsmThumb) -> bool {
    mpconfig::EMIT_THUMB_ARMV7M
}

pub fn end_pass(_asm: &mut AsmThumb) {}

fn get_cur_to_write_bytes(asm: &mut AsmThumb, n: usize) -> *mut u8 {
    asmbase::get_cur_to_write_bytes(&mut asm.base, n)
}

fn get_label_dest(asm: &AsmThumb, label: usize) -> usize {
    assert!(label < asm.base.max_num_labels);
    unsafe { *asm.base.label_offsets.add(label) }
}

pub fn op16(asm: &mut AsmThumb, op: u16) {
    if !ENABLED {
        return;
    }
    let c = get_cur_to_write_bytes(asm, 2);
    if !c.is_null() {
        unsafe {
            *c = op as u8;
            *c.add(1) = (op >> 8) as u8;
        }
    }
}

pub fn op32(asm: &mut AsmThumb, op1: u32, op2: u32) {
    if !ENABLED {
        return;
    }
    let c = get_cur_to_write_bytes(asm, 4);
    if !c.is_null() {
        unsafe {
            *c = op1 as u8;
            *c.add(1) = (op1 >> 8) as u8;
            *c.add(2) = op2 as u8;
            *c.add(3) = (op2 >> 8) as u8;
        }
    }
}

pub fn format_4(asm: &mut AsmThumb, op: u16, rlo_dest: u32, rlo_src: u32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    assert!(rlo_src < ASM_THUMB_REG_R8);
    op16(asm, op_format_4(op, rlo_dest, rlo_src));
}

pub fn format_1(asm: &mut AsmThumb, op: u16, rlo_dest: u32, rlo_src: u32, offset: u32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    assert!(rlo_src < ASM_THUMB_REG_R8);
    op16(asm, op_format_1_encode(op, rlo_dest, rlo_src, offset));
}

pub fn format_2(asm: &mut AsmThumb, op: u16, rlo_dest: u32, rlo_src: u32, src_b: u32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    assert!(rlo_src < ASM_THUMB_REG_R8);
    op16(asm, op_format_2_encode(op, rlo_dest, rlo_src, src_b));
}

pub fn add_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src_a: u32, rlo_src_b: u32) {
    format_2(
        asm,
        ASM_THUMB_FORMAT_2_ADD | ASM_THUMB_FORMAT_2_REG_OPERAND,
        rlo_dest,
        rlo_src_a,
        rlo_src_b,
    );
}

pub fn add_rlo_i8(asm: &mut AsmThumb, rlo: u32, i8: i32) {
    format_2(
        asm,
        ASM_THUMB_FORMAT_2_ADD | ASM_THUMB_FORMAT_2_IMM_OPERAND,
        rlo,
        rlo,
        i8 as u32,
    );
}

pub fn sub_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src_a: u32, rlo_src_b: u32) {
    format_2(
        asm,
        ASM_THUMB_FORMAT_2_SUB | ASM_THUMB_FORMAT_2_REG_OPERAND,
        rlo_dest,
        rlo_src_a,
        rlo_src_b,
    );
}

pub fn format_3(asm: &mut AsmThumb, op: u16, rlo: u32, i8: i32) {
    assert!(rlo < ASM_THUMB_REG_R8);
    op16(asm, op_format_3_encode(op, rlo, i8 as u32));
}

pub fn mov_rlo_i8(asm: &mut AsmThumb, rlo: u32, i8: i32) {
    format_3(asm, ASM_THUMB_FORMAT_3_MOV, rlo, i8);
}

pub fn cmp_rlo_i8(asm: &mut AsmThumb, rlo: u32, i8: i32) {
    format_3(asm, ASM_THUMB_FORMAT_3_CMP, rlo, i8);
}

pub fn ldr_rlo_pcrel_i8(asm: &mut AsmThumb, rlo: u32, i8: u32) {
    format_3(asm, ASM_THUMB_FORMAT_3_LDR, rlo, i8 as i32);
}

pub fn cmp_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src: u32) {
    format_4(asm, ASM_THUMB_FORMAT_4_CMP as u16, rlo_dest, rlo_src);
}

pub fn mvn_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src: u32) {
    format_4(asm, ASM_THUMB_FORMAT_4_MVN as u16, rlo_dest, rlo_src);
}

pub fn neg_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src: u32) {
    format_4(asm, ASM_THUMB_FORMAT_4_NEG as u16, rlo_dest, rlo_src);
}

pub fn format_9_10(asm: &mut AsmThumb, op: u16, rlo_dest: u32, rlo_base: u32, offset: u32) {
    op16(
        asm,
        op | (((offset << 6) & 0x07c0) as u16) | ((rlo_base as u16) << 3) | rlo_dest as u16,
    );
}

pub fn it_cc(asm: &mut AsmThumb, cc: u32, mask: u32) {
    op16(asm, 0xbf00 | ((cc as u16) << 4) | mask as u16);
}

fn format_5(asm: &mut AsmThumb, op: u16, r_dest: u32, r_src: u32) {
    op16(asm, op_format_5_encode(op, r_dest, r_src));
}

pub fn add_reg_reg(asm: &mut AsmThumb, r_dest: u32, r_src: u32) {
    format_5(asm, ASM_THUMB_FORMAT_5_ADD, r_dest, r_src);
}

pub fn bx_reg(asm: &mut AsmThumb, r_src: u32) {
    format_5(asm, ASM_THUMB_FORMAT_5_BX, 0, r_src);
}

fn format_7_8(asm: &mut AsmThumb, op: u16, rlo_dest: u32, rlo_base: u32, rlo_index: u32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    assert!(rlo_base < ASM_THUMB_REG_R8);
    assert!(rlo_index < ASM_THUMB_REG_R8);
    op16(asm, op_format_7_8_encode(op, rlo_dest, rlo_base, rlo_index));
}

pub fn ldrb_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(
        asm,
        ASM_THUMB_FORMAT_7_LDR | ASM_THUMB_FORMAT_7_BYTE_TRANSFER,
        rlo_dest,
        rlo_base,
        rlo_index,
    );
}

pub fn ldrh_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(asm, ASM_THUMB_FORMAT_8_LDRH, rlo_dest, rlo_base, rlo_index);
}

pub fn ldr_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(
        asm,
        ASM_THUMB_FORMAT_7_LDR | ASM_THUMB_FORMAT_7_WORD_TRANSFER,
        rlo_dest,
        rlo_base,
        rlo_index,
    );
}

pub fn strb_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_src: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(
        asm,
        ASM_THUMB_FORMAT_7_STR | ASM_THUMB_FORMAT_7_BYTE_TRANSFER,
        rlo_src,
        rlo_base,
        rlo_index,
    );
}

pub fn strh_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(asm, ASM_THUMB_FORMAT_8_STRH, rlo_dest, rlo_base, rlo_index);
}

pub fn str_rlo_rlo_rlo(asm: &mut AsmThumb, rlo_src: u32, rlo_base: u32, rlo_index: u32) {
    format_7_8(
        asm,
        ASM_THUMB_FORMAT_7_STR | ASM_THUMB_FORMAT_7_WORD_TRANSFER,
        rlo_src,
        rlo_base,
        rlo_index,
    );
}

pub fn lsl_rlo_rlo_i5(asm: &mut AsmThumb, rlo_dest: u32, rlo_src: u32, shift: u32) {
    format_1(asm, ASM_THUMB_FORMAT_1_LSL, rlo_dest, rlo_src, shift);
}

pub fn sxth_rlo_rlo(asm: &mut AsmThumb, rlo_dest: u32, rlo_src: u32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    assert!(rlo_src < ASM_THUMB_REG_R8);
    op16(asm, op_format_11_encode(ASM_THUMB_FORMAT_11_SXTH, rlo_dest, rlo_src));
}

pub fn mov_reg_reg(asm: &mut AsmThumb, reg_dest: u32, reg_src: u32) {
    let mut op_lo = if reg_src < 8 {
        reg_src << 3
    } else {
        0x40 | ((reg_src - 8) << 3)
    };
    if reg_dest < 8 {
        op_lo |= reg_dest;
    } else {
        op_lo |= 0x80 | (reg_dest - 8);
    }
    op16(asm, 0x4600 | op_lo as u16);
}

pub fn mov_reg_i16(asm: &mut AsmThumb, mov_op: u16, reg_dest: u32, i16_src: i32) {
    assert!(reg_dest < ASM_THUMB_REG_R15);
    op32(
        asm,
        (mov_op as u32)
            | (((i16_src >> 1) & 0x0400) as u32)
            | (((i16_src >> 12) & 0xf) as u32),
        ((((i16_src as u16 as u32) << 4) & 0x7000)
            | (reg_dest << 8)
            | ((i16_src & 0xff) as u32)) as u32,
    );
}

fn mov_rlo_i16(asm: &mut AsmThumb, rlo_dest: u32, i16_src: i32) {
    mov_rlo_i8(asm, rlo_dest, (i16_src >> 8) & 0xff);
    lsl_rlo_rlo_i5(asm, rlo_dest, rlo_dest, 8);
    add_rlo_i8(asm, rlo_dest, i16_src & 0xff);
}

pub fn entry(asm: &mut AsmThumb, num_locals: i32) {
    if !ENABLED {
        return;
    }
    assert!(num_locals >= 0);

    if mpconfig::EMIT_ARM {
        op32(asm, 0x4010, 0xe92d);
        op32(asm, 0xe009, 0xe28f);
        op32(asm, 0xff3e, 0xe12f);
        op32(asm, 0x4010, 0xe8bd);
        op32(asm, 0xff1e, 0xe12f);
    }

    let (reglist, stack_adjust) = match num_locals {
        0 | 1 => (0xf2u32, 0u32),
        2 | 3 => (0xfeu32, 0u32),
        _ => (0xfeu32, (((num_locals - 3) + 1) & !1) as u32),
    };

    op16(asm, op_push_rlist_lr(reglist));
    if stack_adjust > 0 {
        if allow_armv7m(asm) {
            if unsigned_fit7(stack_adjust) {
                op16(asm, op_sub_sp(stack_adjust));
            } else {
                op32(
                    asm,
                    op_sub_w_rri_hi(ASM_THUMB_REG_SP) as u32,
                    op_sub_w_rri_lo(ASM_THUMB_REG_SP, stack_adjust * 4) as u32,
                );
            }
        } else {
            let mut adj = stack_adjust;
            while !unsigned_fit7(adj) {
                op16(asm, op_sub_sp(127));
                adj -= 127;
            }
            op16(asm, op_sub_sp(adj));
        }
    }
    asm.push_reglist = reglist;
    asm.stack_adjust = stack_adjust;
}

pub fn exit(asm: &mut AsmThumb) {
    if !ENABLED {
        return;
    }
    if asm.stack_adjust > 0 {
        if allow_armv7m(asm) {
            if unsigned_fit7(asm.stack_adjust) {
                op16(asm, op_add_sp(asm.stack_adjust));
            } else {
                op32(
                    asm,
                    op_add_w_rri_hi(ASM_THUMB_REG_SP) as u32,
                    op_add_w_rri_lo(ASM_THUMB_REG_SP, asm.stack_adjust * 4) as u32,
                );
            }
        } else {
            let mut adj = asm.stack_adjust;
            while !unsigned_fit7(adj) {
                op16(asm, op_add_sp(127));
                adj -= 127;
            }
            op16(asm, op_add_sp(adj));
        }
    }
    op16(asm, op_pop_rlist_pc(asm.push_reglist));
}

pub fn b_n_label(asm: &mut AsmThumb, label: usize) -> bool {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 4;
    op16(asm, op_b_n(rel));
    asm.base.pass != MP_ASM_PASS_EMIT || signed_fit12(rel)
}

pub fn bcc_nw_label(asm: &mut AsmThumb, cond: i32, label: usize, wide: bool) -> bool {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 4;
    if !wide {
        op16(asm, op_bcc_n(cond, rel));
        return asm.base.pass != MP_ASM_PASS_EMIT || signed_fit9(rel);
    } else if allow_armv7m(asm) {
        op32(asm, op_bcc_w_hi(cond, rel) as u32, op_bcc_w_lo(rel) as u32);
        return true;
    }
    false
}

pub fn bl_label(asm: &mut AsmThumb, label: usize) -> bool {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 4;
    op32(asm, op_bl_hi(rel) as u32, op_bl_lo(rel) as u32);
    asm.base.pass != MP_ASM_PASS_EMIT || signed_fit23(rel)
}

pub fn mov_reg_i32(asm: &mut AsmThumb, reg_dest: u32, i32: u32) -> usize {
    let loc = asm.base.get_code_pos();
    if allow_armv7m(asm) {
        mov_reg_i16(asm, ASM_THUMB_OP_MOVW, reg_dest, i32 as i16 as i32);
        mov_reg_i16(asm, ASM_THUMB_OP_MOVT, reg_dest, (i32 >> 16) as i16 as i32);
    } else {
        assert!(reg_dest < ASM_THUMB_REG_R8);
        if asm.base.code_offset & 2 != 0 {
            op16(asm, ASM_THUMB_OP_NOP);
        }
        ldr_rlo_pcrel_i8(asm, reg_dest, 0);
        op16(asm, op_b_n(2));
        op16(asm, (i32 & 0xffff) as u16);
        op16(asm, (i32 >> 16) as u16);
    }
    loc
}

pub fn mov_reg_i32_optimised(asm: &mut AsmThumb, reg_dest: u32, mut i32: i32) {
    if reg_dest < 8 && unsigned_fit8(i32) {
        mov_rlo_i8(asm, reg_dest, i32);
    } else if allow_armv7m(asm) {
        if unsigned_fit16(i32) {
            mov_reg_i16(asm, ASM_THUMB_OP_MOVW, reg_dest, i32 as i16 as i32);
        } else {
            mov_reg_i32(asm, reg_dest, i32 as u32);
        }
    } else {
        let rlo_dest = reg_dest;
        assert!(rlo_dest < ASM_THUMB_REG_R8);

        let mut negate = i32 < 0 && (i32 as u32).wrapping_add(i32 as u32) != 0;
        if negate {
            i32 = -i32;
        }

        let clz = mp_clz(i32 as u32);
        let ctz = if i32 != 0 { mp_ctz(i32 as u32) } else { 0 };
        assert!(clz + ctz <= 32);
        if clz + ctz >= 24 {
            mov_rlo_i8(asm, rlo_dest, ((i32 >> ctz) & 0xff) as i32);
            lsl_rlo_rlo_i5(asm, rlo_dest, rlo_dest, ctz);
        } else if unsigned_fit16(i32) {
            mov_rlo_i16(asm, rlo_dest, i32);
        } else {
            if negate {
                negate = false;
                i32 = -i32;
            }
            mov_reg_i32(asm, rlo_dest, i32 as u32);
        }
        if negate {
            neg_rlo_rlo(asm, rlo_dest, rlo_dest);
        }
    }
}

fn mov_local_check(asm: &AsmThumb, word_offset: i32) {
    if asm.base.pass >= MP_ASM_PASS_EMIT {
        assert!(word_offset >= 0);
        if !unsigned_fit8(word_offset) {
            raise::raise(MpRaise::RuntimeError(
                "too many locals for native method",
            ));
        }
    }
}

pub fn mov_local_reg(asm: &mut AsmThumb, local_num: i32, rlo_src: u32) {
    assert!(rlo_src < ASM_THUMB_REG_R8);
    let word_offset = local_num;
    mov_local_check(asm, word_offset);
    op16(asm, op_str_to_sp_offset(rlo_src, word_offset as u32));
}

pub fn mov_reg_local(asm: &mut AsmThumb, rlo_dest: u32, local_num: i32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    let word_offset = local_num;
    mov_local_check(asm, word_offset);
    op16(asm, op_ldr_from_sp_offset(rlo_dest, word_offset as u32));
}

pub fn mov_reg_local_addr(asm: &mut AsmThumb, rlo_dest: u32, local_num: i32) {
    assert!(rlo_dest < ASM_THUMB_REG_R8);
    let word_offset = local_num;
    assert!(asm.base.pass < MP_ASM_PASS_EMIT || word_offset >= 0);
    op16(asm, op_add_reg_sp_offset(rlo_dest, word_offset as u32));
}

pub fn mov_reg_pcrel(asm: &mut AsmThumb, rlo_dest: u32, label: usize) {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel |= 1;
    if allow_armv7m(asm) {
        rel -= 6 + 4;
        mov_reg_i16(asm, ASM_THUMB_OP_MOVW, rlo_dest, rel as i16 as i32);
        sxth_rlo_rlo(asm, rlo_dest, rlo_dest);
    } else {
        rel -= 8 + 4;
        mov_rlo_i16(asm, rlo_dest, rel);
        sxth_rlo_rlo(asm, rlo_dest, rlo_dest);
    }
    add_reg_reg(asm, rlo_dest, ASM_THUMB_REG_R15);
}

fn add_reg_reg_offset(
    asm: &mut AsmThumb,
    reg_dest: u32,
    reg_base: u32,
    offset: u32,
    offset_shift: u32,
) {
    if reg_dest < ASM_THUMB_REG_R8 && reg_base < ASM_THUMB_REG_R8 {
        if offset << offset_shift < 256 {
            if reg_dest != reg_base {
                mov_reg_reg(asm, reg_dest, reg_base);
            }
            add_rlo_i8(asm, reg_dest, ((offset << offset_shift) & 0xff) as i32);
        } else if unsigned_fit8(offset as i32) && reg_dest != reg_base {
            mov_rlo_i8(asm, reg_dest, offset as i32);
            lsl_rlo_rlo_i5(asm, reg_dest, reg_dest, offset_shift);
            add_rlo_rlo_rlo(asm, reg_dest, reg_dest, reg_base);
        } else if reg_dest != reg_base {
            mov_reg_i32_optimised(asm, reg_dest, (offset << offset_shift) as i32);
            add_rlo_rlo_rlo(asm, reg_dest, reg_dest, reg_base);
        } else {
            let reg_other = reg_dest ^ 7;
            op16(asm, op_push_rlist(1 << reg_other));
            mov_reg_i32_optimised(asm, reg_other, (offset << offset_shift) as i32);
            add_rlo_rlo_rlo(asm, reg_dest, reg_dest, reg_other);
            op16(asm, 0xbc00 | (1 << reg_other) as u16);
        }
    } else {
        assert!(false, "should never be called for ARMV6M");
    }
}

pub fn load_reg_reg_offset(
    asm: &mut AsmThumb,
    reg_dest: u32,
    reg_base: u32,
    offset: u32,
    operation_size: u32,
) {
    assert!(operation_size <= 2);
    if misc::fit_unsigned(5, offset)
        && reg_dest < ASM_THUMB_REG_R8
        && reg_base < ASM_THUMB_REG_R8
    {
        op16(
            asm,
            ((OP_LDR_STR_TABLE[operation_size as usize] | OP_LDR) << 11) as u16
                | ((offset << 6) as u16)
                | ((reg_base << 3) as u16)
                | reg_dest as u16,
        );
    } else if allow_armv7m(asm) && misc::fit_unsigned(12, offset << operation_size) {
        op32(
            asm,
            op_ldr_str_w_hi(operation_size, reg_base) as u32 | OP_LDR_W as u32,
            op_ldr_str_w_lo(reg_dest, offset << operation_size) as u32,
        );
    } else {
        add_reg_reg_offset(asm, reg_dest, reg_base, offset - 31, operation_size);
        op16(
            asm,
            ((OP_LDR_STR_TABLE[operation_size as usize] | OP_LDR) << 11) as u16
                | (31 << 6) as u16
                | ((reg_dest << 3) as u16)
                | reg_dest as u16,
        );
    }
}

pub fn store_reg_reg_offset(
    asm: &mut AsmThumb,
    reg_src: u32,
    reg_base: u32,
    offset: u32,
    operation_size: u32,
) {
    assert!(operation_size <= 2);
    if misc::fit_unsigned(5, offset)
        && reg_src < ASM_THUMB_REG_R8
        && reg_base < ASM_THUMB_REG_R8
    {
        op16(
            asm,
            ((OP_LDR_STR_TABLE[operation_size as usize] | OP_STR) << 11) as u16
                | ((offset << 6) as u16)
                | ((reg_base << 3) as u16)
                | reg_src as u16,
        );
    } else if allow_armv7m(asm) && misc::fit_unsigned(12, offset << operation_size) {
        op32(
            asm,
            op_ldr_str_w_hi(operation_size, reg_base) as u32 | OP_STR_W as u32,
            op_ldr_str_w_lo(reg_src, offset << operation_size) as u32,
        );
    } else {
        op16(asm, op_push_rlist(1 << reg_base));
        add_reg_reg_offset(asm, reg_base, reg_base, offset - 31, operation_size);
        op16(
            asm,
            ((OP_LDR_STR_TABLE[operation_size as usize] | OP_STR) << 11) as u16
                | (31 << 6) as u16
                | ((reg_base << 3) as u16)
                | reg_src as u16,
        );
        op16(asm, 0xbc00 | (1 << reg_base) as u16);
    }
}

pub fn b_label(asm: &mut AsmThumb, label: usize) {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 4;

    if dest != usize::MAX && rel <= -4 && signed_fit12(rel) {
        op16(asm, op_b_n(rel));
        return;
    }

    if allow_armv7m(asm) {
        op32(asm, op_bw_hi(rel) as u32, op_bw_lo(rel) as u32);
    } else {
        let need_align = asm.base.code_offset & 2 != 0;
        if signed_fit12(rel) {
            op16(asm, op_b_n(rel));
            op16(asm, ASM_THUMB_OP_NOP);
            op16(asm, ASM_THUMB_OP_NOP);
            op16(asm, ASM_THUMB_OP_NOP);
            if need_align {
                op16(asm, ASM_THUMB_OP_NOP);
            }
        } else {
            rel -= 2;
            if need_align {
                op16(asm, ASM_THUMB_OP_NOP);
                rel -= 2;
            }
            ldr_rlo_pcrel_i8(asm, ASM_THUMB_REG_R1, 0);
            add_reg_reg(asm, ASM_THUMB_REG_R15, ASM_THUMB_REG_R1);
            op16(asm, (rel & 0xffff) as u16);
            op16(asm, (rel >> 16) as u16);
        }
    }
}

pub fn bcc_label(asm: &mut AsmThumb, cond: i32, label: usize) {
    let dest = get_label_dest(asm, label);
    let mut rel = dest as i32 - asm.base.code_offset as i32;
    rel -= 4;

    if dest != usize::MAX && rel <= -4 && signed_fit9(rel) {
        op16(asm, op_bcc_n(cond, rel));
        return;
    }

    if allow_armv7m(asm) {
        op32(asm, op_bcc_w_hi(cond, rel) as u32, op_bcc_w_lo(rel) as u32);
    } else {
        let code_offset_start = asm.base.code_offset;
        let c = get_cur_to_write_bytes(asm, 2);
        b_label(asm, label);
        let bytes_to_skip = asm.base.code_offset - code_offset_start;
        let op = op_bcc_n(cond ^ 1, bytes_to_skip as i32 - 4);
        if !c.is_null() {
            unsafe {
                *c = op as u8;
                *c.add(1) = (op >> 8) as u8;
            }
        }
    }
}

pub fn bcc_rel9(asm: &mut AsmThumb, cond: i32, mut rel: i32) {
    rel -= 4;
    assert!(signed_fit9(rel));
    op16(asm, op_bcc_n(cond, rel));
}

pub fn b_rel12(asm: &mut AsmThumb, mut rel: i32) {
    rel -= 4;
    assert!(signed_fit12(rel));
    op16(asm, op_b_n(rel));
}

pub fn bl_ind(asm: &mut AsmThumb, fun_id: u32, reg_temp: u32) {
    load_reg_reg_offset(asm, reg_temp, ASM_THUMB_REG_FUN_TABLE, fun_id, 2);
    op16(asm, op_blx(reg_temp));
}
