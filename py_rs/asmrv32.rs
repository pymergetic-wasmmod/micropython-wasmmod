//! rewrite of py/asmrv32.c + py/asmrv32.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase, MP_ASM_PASS_EMIT};
use crate::misc::{self, mp_clz, mp_popcount};
use crate::mpconfig;

const ENABLED: bool = mpconfig::EMIT_RV32;
const INTERNAL_TEMPORARY: u32 = ASM_RV32_REG_S0;
const ASM_WORD_SIZE: usize = 4;
const ASM_HALFWORD_SIZE: usize = 2;

pub const ASM_RV32_REG_X0: u32 = 0;
pub const ASM_RV32_REG_X1: u32 = 1;
pub const ASM_RV32_REG_X2: u32 = 2;
pub const ASM_RV32_REG_X5: u32 = 5;
pub const ASM_RV32_REG_X6: u32 = 6;
pub const ASM_RV32_REG_X7: u32 = 7;
pub const ASM_RV32_REG_X8: u32 = 8;
pub const ASM_RV32_REG_X9: u32 = 9;
pub const ASM_RV32_REG_X10: u32 = 10;
pub const ASM_RV32_REG_X11: u32 = 11;
pub const ASM_RV32_REG_X12: u32 = 12;
pub const ASM_RV32_REG_X13: u32 = 13;
pub const ASM_RV32_REG_X14: u32 = 14;
pub const ASM_RV32_REG_X15: u32 = 15;
pub const ASM_RV32_REG_X16: u32 = 16;
pub const ASM_RV32_REG_X17: u32 = 17;
pub const ASM_RV32_REG_X18: u32 = 18;
pub const ASM_RV32_REG_X19: u32 = 19;
pub const ASM_RV32_REG_X20: u32 = 20;
pub const ASM_RV32_REG_X28: u32 = 28;

pub const ASM_RV32_REG_ZERO: u32 = ASM_RV32_REG_X0;
pub const ASM_RV32_REG_RA: u32 = ASM_RV32_REG_X1;
pub const ASM_RV32_REG_SP: u32 = ASM_RV32_REG_X2;
pub const ASM_RV32_REG_A0: u32 = ASM_RV32_REG_X10;
pub const ASM_RV32_REG_A1: u32 = ASM_RV32_REG_X11;
pub const ASM_RV32_REG_A2: u32 = ASM_RV32_REG_X12;
pub const ASM_RV32_REG_A3: u32 = ASM_RV32_REG_X13;
pub const ASM_RV32_REG_A4: u32 = ASM_RV32_REG_X14;
pub const ASM_RV32_REG_A5: u32 = ASM_RV32_REG_X15;
pub const ASM_RV32_REG_A6: u32 = ASM_RV32_REG_X16;
pub const ASM_RV32_REG_A7: u32 = ASM_RV32_REG_X17;
pub const ASM_RV32_REG_T0: u32 = ASM_RV32_REG_X5;
pub const ASM_RV32_REG_T1: u32 = ASM_RV32_REG_X6;
pub const ASM_RV32_REG_T2: u32 = ASM_RV32_REG_X7;
pub const ASM_RV32_REG_S0: u32 = ASM_RV32_REG_X8;
pub const ASM_RV32_REG_S1: u32 = ASM_RV32_REG_X9;
pub const ASM_RV32_REG_S2: u32 = ASM_RV32_REG_X18;
pub const ASM_RV32_REG_S3: u32 = ASM_RV32_REG_X19;
pub const ASM_RV32_REG_S4: u32 = ASM_RV32_REG_X20;

pub const RV32_AVAILABLE_REGISTERS_COUNT: u32 = 32;

pub const REG_RET: u32 = ASM_RV32_REG_A0;
pub const REG_ARG_1: u32 = ASM_RV32_REG_A0;
pub const REG_ARG_2: u32 = ASM_RV32_REG_A1;
pub const REG_ARG_3: u32 = ASM_RV32_REG_A2;
pub const REG_ARG_4: u32 = ASM_RV32_REG_A3;
pub const REG_TEMP0: u32 = ASM_RV32_REG_A4;
pub const REG_TEMP1: u32 = ASM_RV32_REG_A5;
pub const REG_TEMP2: u32 = ASM_RV32_REG_A6;
pub const REG_FUN_TABLE: u32 = ASM_RV32_REG_S1;
pub const REG_LOCAL_1: u32 = ASM_RV32_REG_S3;
pub const REG_LOCAL_2: u32 = ASM_RV32_REG_S2;
pub const REG_LOCAL_3: u32 = ASM_RV32_REG_S4;

pub const RV32_EXT_NONE: u8 = 0;
pub const RV32_EXT_ZBA: u8 = 1 << 0;
pub const RV32_EXT_ZCMP: u8 = 1 << 1;
pub const RV32_EXT_ALL: u8 = RV32_EXT_ZBA | RV32_EXT_ZCMP;

#[repr(C)]
pub struct AsmRv32BackendOptions {
    pub allowed_extensions: u8,
}

#[repr(C)]
pub struct AsmRv32 {
    pub base: MpAsmBase,
    pub saved_registers_mask: u32,
    pub locals_count: u32,
    pub stack_size: u32,
    pub locals_stack_offset: u32,
}

#[inline]
pub const fn rv32_map_in_c_register_window(register_number: u32) -> u32 {
    register_number - ASM_RV32_REG_X8
}

#[inline]
pub const fn rv32_is_in_c_register_window(register_number: u32) -> bool {
    register_number >= ASM_RV32_REG_X8 && register_number <= ASM_RV32_REG_X15
}

#[inline]
const fn fit_unsigned(bits: u32, value: u32) -> bool {
    (value & (!0u32 << bits)) == 0
}

#[inline]
const fn fit_signed(bits: u32, value: i32) -> bool {
    fit_unsigned(bits - 1, value as u32)
        || ((value as u32) & (!0u32 << (bits - 1))) == (!0u32 << (bits - 1))
}

const RV32_LOAD_OPCODE_FT3: [u32; 3] = [0x0, 0x1, 0x2];

#[inline]
const fn encode_type_b(op: u32, ft3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (op & 0x7f)
        | ((ft3 & 0x07) << 12)
        | ((imm & 0x800) >> 4)
        | ((imm & 0x1e) << 7)
        | ((rs1 & 0x1f) << 15)
        | ((rs2 & 0x1f) << 20)
        | ((imm & 0x7e0) << 20)
        | ((imm & 0x1000) << 19)
}

#[inline]
const fn encode_type_i(op: u32, ft3: u32, rd: u32, rs: u32, imm: i32) -> u32 {
    (op & 0x7f) | ((rd & 0x1f) << 7) | ((ft3 & 0x07) << 12) | ((rs & 0x1f) << 15) | ((imm as u32 & 0xfff) << 20)
}

#[inline]
const fn encode_type_r(op: u32, ft3: u32, ft7: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (op & 0x7f)
        | ((rd & 0x1f) << 7)
        | ((ft3 & 0x07) << 12)
        | ((rs1 & 0x1f) << 15)
        | ((rs2 & 0x1f) << 20)
        | ((ft7 & 0x7f) << 25)
}

#[inline]
const fn encode_type_s(op: u32, ft3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (op & 0x7f)
        | ((imm & 0x1f) << 7)
        | ((ft3 & 0x07) << 12)
        | ((rs1 & 0x1f) << 15)
        | ((rs2 & 0x1f) << 20)
        | ((imm & 0xfe0) << 20)
}

#[inline]
const fn encode_type_u(op: u32, rd: u32, imm: i32) -> u32 {
    (op & 0x7f) | ((rd & 0x1f) << 7) | (imm as u32 & 0xfffff000)
}

#[inline]

const fn encode_type_j(op: u32, rd: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (op & 0x7f)
        | ((rd & 0x1f) << 7)
        | (imm & 0xff000)
        | ((imm & 0x800) << 9)
        | ((imm & 0x7fe) << 20)
        | ((imm & 0x100000) << 11)
}

#[inline]
const fn encode_type_csri(op: u32, ft3: u32, rd: u32, csr: u32, imm: u32) -> u32 {
    (op & 0x7f)
        | ((rd & 0x1f) << 7)
        | ((ft3 & 0x07) << 12)
        | ((csr & 0xfff) << 20)
        | ((imm & 0x1f) << 15)
}

#[inline]
const fn encode_type_cmmv(op: u32, ft6: u32, ft2: u32, r1s: u32, r2s: u32) -> u16 {
    let r1c = if r1s >= ASM_RV32_REG_S0 && r1s <= ASM_RV32_REG_S1 {
        r1s - ASM_RV32_REG_S0
    } else {
        r1s - ASM_RV32_REG_S2 + 2
    };
    let r2c = if r2s >= ASM_RV32_REG_S0 && r2s <= ASM_RV32_REG_S1 {
        r2s - ASM_RV32_REG_S0
    } else {
        r2s - ASM_RV32_REG_S2 + 2
    };
    ((op & 0x03) as u16)
        | (((ft6 & 0x3f) << 10) as u16)
        | (((ft2 & 0x03) << 5) as u16)
        | (((r1c & 0x07) << 7) as u16)
        | (((r2c & 0x07) << 2) as u16)
}

#[inline]
const fn encode_type_ca(op: u32, ft6: u32, ft2: u32, rd: u32, rs: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((ft6 & 0x3f) << 10) as u16)
        | (((ft2 & 0x03) << 5) as u16)
        | (((rd & 0x03) << 7) as u16)
        | (((rs & 0x03) << 2) as u16)
}

#[inline]
const fn encode_type_cb(op: u32, ft3: u32, rs: u32, imm: i32) -> u16 {
    let imm = imm as u32;
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((rs & 0x07) << 7) as u16)
        | (((imm & 0xe0) << 5) as u16)
        | (((imm & 0x1f) << 2) as u16)
}

#[inline]
const fn encode_type_ci(op: u32, ft3: u32, rd: u32, imm: i32) -> u16 {
    let imm = imm as u32;
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((rd & 0x1f) << 7) as u16)
        | (((imm & 0x20) << 7) as u16)
        | (((imm & 0x1f) << 2) as u16)
}

#[inline]
const fn encode_type_ciw(op: u32, ft3: u32, rd: u32, imm: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((rd & 0x07) << 2) as u16)
        | (((imm & 0x3c0) << 1) as u16)
        | (((imm & 0x30) << 7) as u16)
        | (((imm & 0x08) << 2) as u16)
        | (((imm & 0x04) << 4) as u16)
}

#[inline]
const fn encode_type_cj(op: u32, ft3: u32, imm: i32) -> u16 {
    let imm = imm as u32;
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((imm & 0x0e) << 2) as u16)
        | (((imm & 0x300) << 1) as u16)
        | (((imm & 0x800) << 1) as u16)
        | (((imm & 0x400) >> 2) as u16)
        | (((imm & 0x80) >> 1) as u16)
        | (((imm & 0x40) << 1) as u16)
        | (((imm & 0x20) >> 3) as u16)
        | (((imm & 0x10) << 7) as u16)
}

#[inline]
const fn encode_type_cl(op: u32, ft3: u32, rd: u32, rs: u32, imm: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((rd & 0x07) << 2) as u16)
        | (((rs & 0x07) << 7) as u16)
        | (((imm & 0x40) >> 1) as u16)
        | (((imm & 0x38) << 7) as u16)
        | (((imm & 0x04) << 4) as u16)
}

#[inline]
const fn encode_type_cmpp(op: u32, ft6: u32, ft2: u32, rlist: u32, imm: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((ft6 & 0x3f) << 10) as u16)
        | (((ft2 & 0x03) << 8) as u16)
        | (((rlist & 0x0f) << 4) as u16)
        | (((imm & 0x03) << 2) as u16)
}

#[inline]
const fn encode_type_cr(op: u32, ft4: u32, rs1: u32, rs2: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((rs2 & 0x1f) << 2) as u16)
        | (((rs1 & 0x1f) << 7) as u16)
        | (((ft4 & 0x0f) << 12) as u16)
}

#[inline]
const fn encode_type_css(op: u32, ft3: u32, rs: u32, imm: u32) -> u16 {
    ((op & 0x03) as u16)
        | (((ft3 & 0x07) << 13) as u16)
        | (((rs & 0x1f) << 2) as u16)
        | (((imm & 0x3f) << 7) as u16)
}

pub fn allowed_extensions() -> u8 {
    (if mpconfig::EMIT_RV32_ZBA {
        RV32_EXT_ZBA
    } else {
        0
    }) | (if mpconfig::EMIT_RV32_ZCMP {
        RV32_EXT_ZCMP
    } else {
        0
    })
}

fn allow_zba_opcodes() -> bool {
    allowed_extensions() & RV32_EXT_ZBA != 0
}

fn allow_zcmp_opcodes() -> bool {
    allowed_extensions() & RV32_EXT_ZCMP != 0
}

pub fn emit_word_opcode(state: &mut AsmRv32, word: u32) {
    if !ENABLED {
        return;
    }
    let cursor = asmbase::get_cur_to_write_bytes(&mut state.base, 4);
    if cursor.is_null() {
        return;
    }
    unsafe {
        *cursor = word as u8;
        *cursor.add(1) = (word >> 8) as u8;
        *cursor.add(2) = (word >> 16) as u8;
        *cursor.add(3) = (word >> 24) as u8;
    }
}

pub fn emit_halfword_opcode(state: &mut AsmRv32, word: u16) {
    if !ENABLED {
        return;
    }
    let cursor = asmbase::get_cur_to_write_bytes(&mut state.base, 2);
    if cursor.is_null() {
        return;
    }
    unsafe {
        *cursor = word as u8;
        *cursor.add(1) = (word >> 8) as u8;
    }
}

macro_rules! rv32_word_r {
    ($name:ident, $op:expr, $ft3:expr, $ft7:expr) => {
        pub fn $name(state: &mut AsmRv32, rd: u32, rs1: u32, rs2: u32) {
            if !ENABLED {
                return;
            }
            emit_word_opcode(state, encode_type_r($op, $ft3, $ft7, rd, rs1, rs2));
        }
    };
}

macro_rules! rv32_word_i {
    ($name:ident, $op:expr, $ft3:expr) => {
        pub fn $name(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
            if !ENABLED {
                return;
            }
            emit_word_opcode(state, encode_type_i($op, $ft3, rd, rs, immediate));
        }
    };
}

macro_rules! rv32_half_ci {
    ($name:ident, $op:expr, $ft3:expr) => {
        pub fn $name(state: &mut AsmRv32, rd: u32, immediate: i32) {
            if !ENABLED {
                return;
            }
            emit_halfword_opcode(state, encode_type_ci($op, $ft3, rd, immediate));
        }
    };
}

rv32_word_r!(opcode_add, 0x33, 0x00, 0x00);
rv32_word_i!(opcode_addi, 0x13, 0x00);
rv32_word_r!(opcode_and, 0x33, 0x07, 0x00);
rv32_word_i!(opcode_andi, 0x13, 0x07);
rv32_word_r!(opcode_or, 0x33, 0x06, 0x00);
rv32_word_r!(opcode_xor, 0x33, 0x04, 0x00);
rv32_word_i!(opcode_xori, 0x13, 0x04);
rv32_word_r!(opcode_sub, 0x33, 0x00, 0x20);
rv32_word_r!(opcode_mul, 0x33, 0x00, 0x01);
rv32_word_r!(opcode_sll, 0x33, 0x01, 0x00);
rv32_word_i!(opcode_slli, 0x13, 0x01);
rv32_word_r!(opcode_srl, 0x33, 0x05, 0x00);
rv32_word_r!(opcode_sra, 0x33, 0x05, 0x20);
rv32_word_r!(opcode_slt, 0x33, 0x02, 0x00);
rv32_word_i!(opcode_sltiu, 0x13, 0x03);
rv32_word_r!(opcode_sltu, 0x33, 0x03, 0x00);

macro_rules! rv32_word_b {
    ($name:ident, $ft3:expr) => {
        pub fn $name(state: &mut AsmRv32, rs1: u32, rs2: u32, offset: i32) {
            if !ENABLED { return; }
            emit_word_opcode(state, encode_type_b(0x63, $ft3, rs1, rs2, offset));
        }
    };
}

rv32_word_b!(opcode_bge, 0x05);
rv32_word_b!(opcode_bgeu, 0x07);
rv32_word_b!(opcode_blt, 0x04);
rv32_word_b!(opcode_bltu, 0x06);
rv32_word_i!(opcode_slti, 0x13, 0x02);
rv32_word_i!(opcode_ori, 0x13, 0x06);
rv32_word_i!(opcode_lb, 0x03, 0x00);
rv32_word_i!(opcode_lbu, 0x03, 0x04);
rv32_word_i!(opcode_lh, 0x03, 0x01);
rv32_word_i!(opcode_lhu, 0x03, 0x05);
rv32_word_i!(opcode_sb, 0x23, 0x00);
rv32_word_i!(opcode_sh, 0x23, 0x01);
rv32_word_r!(opcode_div, 0x33, 0x04, 0x01);
rv32_word_r!(opcode_divu, 0x33, 0x05, 0x01);
rv32_word_r!(opcode_rem, 0x33, 0x06, 0x01);
rv32_word_r!(opcode_remu, 0x33, 0x07, 0x01);
rv32_word_r!(opcode_mulh, 0x33, 0x01, 0x01);
rv32_word_r!(opcode_mulhsu, 0x33, 0x02, 0x01);
rv32_word_r!(opcode_mulhu, 0x33, 0x03, 0x01);
rv32_word_r!(opcode_sh1add, 0x33, 0x02, 0x10);
rv32_word_r!(opcode_sh2add, 0x33, 0x04, 0x10);
rv32_word_r!(opcode_sh3add, 0x33, 0x06, 0x10);

pub fn opcode_jal(state: &mut AsmRv32, rd: u32, offset: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_j(0x6f, rd, offset));
}

pub fn opcode_ebreak(state: &mut AsmRv32) {
    if !ENABLED { return; }
    emit_word_opcode(state, 0x0010_0073);
}

pub fn opcode_ecall(state: &mut AsmRv32) {
    if !ENABLED { return; }
    emit_word_opcode(state, 0x0000_0073);
}

pub fn opcode_cmv(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cr(0x02, 0x08, rd, rs));
}

pub fn opcode_cnop(state: &mut AsmRv32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, 0x0001);
}

pub fn opcode_cebreak(state: &mut AsmRv32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, 0x9002);
}

pub fn opcode_cjal(state: &mut AsmRv32, offset: i32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cj(0x01, 0x01, offset));
}

pub fn opcode_cand(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_ca(0x01, 0x23, 0x03, rd, rs));
}

pub fn opcode_candi(state: &mut AsmRv32, rd: u32, immediate: i32) {
    if !ENABLED { return; }
    let imm = ((immediate & 0x20) << 2) | (immediate & 0x1f) | 0x40;
    emit_halfword_opcode(state, encode_type_cb(0x01, 0x04, rd, imm));
}

pub fn opcode_cor(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_ca(0x01, 0x23, 0x02, rd, rs));
}

pub fn opcode_csub(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_ca(0x01, 0x23, 0x03, rd, rs));
}

pub fn opcode_cxor(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_ca(0x01, 0x23, 0x01, rd, rs));
}

rv32_half_ci!(opcode_cslli, 0x02, 0x00);
rv32_half_ci!(opcode_csrai, 0x02, 0x01);
rv32_half_ci!(opcode_csrli, 0x02, 0x02);

pub fn opcode_srai(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_i(0x13, 0x05, rd, rs, immediate & 0x1f));
}

pub fn opcode_srli(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_i(0x13, 0x05, rd, rs, immediate & 0x1f));
}

pub fn opcode_csrrc(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_i(0x73, 0x03, rd, rs, immediate));
}

pub fn opcode_csrrs(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_i(0x73, 0x02, rd, rs, immediate));
}

pub fn opcode_csrrw(state: &mut AsmRv32, rd: u32, rs: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_i(0x73, 0x01, rd, rs, immediate));
}

pub fn opcode_csrrci(state: &mut AsmRv32, rd: u32, csr: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_csri(0x73, 0x07, rd, csr, immediate as u32));
}

pub fn opcode_csrrsi(state: &mut AsmRv32, rd: u32, csr: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_csri(0x73, 0x06, rd, csr, immediate as u32));
}

pub fn opcode_csrrwi(state: &mut AsmRv32, rd: u32, csr: u32, immediate: i32) {
    if !ENABLED { return; }
    emit_word_opcode(state, encode_type_csri(0x73, 0x05, rd, csr, immediate as u32));
}

pub fn opcode_cmpop(state: &mut AsmRv32, reg_list: u32, immediate: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cmpp(0x02, 0x2e, 0x02, reg_list, immediate));
}

pub fn opcode_cmpopretz(state: &mut AsmRv32, reg_list: u32, immediate: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cmpp(0x02, 0x2f, 0x00, reg_list, immediate));
}

pub fn opcode_cmmva01s(state: &mut AsmRv32, r1s: u32, r2s: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cmmv(0x02, 0x2b, 0x03, r1s, r2s));
}

pub fn opcode_cmmvsa01(state: &mut AsmRv32, r1s: u32, r2s: u32) {
    if !ENABLED { return; }
    emit_halfword_opcode(state, encode_type_cmmv(0x02, 0x2b, 0x01, r1s, r2s));
}



pub fn opcode_auipc(state: &mut AsmRv32, rd: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_u(0x17, rd, offset));
}

pub fn opcode_lui(state: &mut AsmRv32, rd: u32, immediate: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_u(0x37, rd, immediate));
}

pub fn opcode_beq(state: &mut AsmRv32, rs1: u32, rs2: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_b(0x63, 0x00, rs1, rs2, offset));
}

pub fn opcode_bne(state: &mut AsmRv32, rs1: u32, rs2: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_b(0x63, 0x01, rs1, rs2, offset));
}

pub fn opcode_jalr(state: &mut AsmRv32, rd: u32, rs: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_i(0x67, 0x00, rd, rs, offset));
}

pub fn opcode_lw(state: &mut AsmRv32, rd: u32, rs: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_i(0x03, 0x02, rd, rs, offset));
}

pub fn opcode_sw(state: &mut AsmRv32, rs2: u32, rs1: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(state, encode_type_s(0x23, 0x02, rs1, rs2, offset));
}

pub fn opcode_cadd(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cr(0x02, 0x09, rd, rs));
}

rv32_half_ci!(opcode_caddi, 0x01, 0x00);
rv32_half_ci!(opcode_cli, 0x01, 0x02);

pub fn opcode_clui(state: &mut AsmRv32, rd: u32, immediate: i32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_ci(0x01, 0x03, rd, immediate >> 12));
}

pub fn opcode_caddi4spn(state: &mut AsmRv32, rd: u32, immediate: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_ciw(0x00, 0x00, rd, immediate));
}

pub fn opcode_cbnez(state: &mut AsmRv32, rs: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    let imm = ((offset & 0x100) >> 1)
        | ((offset & 0xc0) >> 3)
        | ((offset & 0x20) >> 5)
        | ((offset & 0x18) << 2)
        | (offset & 0x06);
    emit_halfword_opcode(state, encode_type_cb(0x01, 0x07, rs, imm));
}

pub fn opcode_cbeqz(state: &mut AsmRv32, rs: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    let imm = ((offset & 0x100) >> 1)
        | ((offset & 0xc0) >> 3)
        | ((offset & 0x20) >> 5)
        | ((offset & 0x18) << 2)
        | (offset & 0x06);
    emit_halfword_opcode(state, encode_type_cb(0x01, 0x06, rs, imm));
}

pub fn opcode_cj(state: &mut AsmRv32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cj(0x01, 0x05, offset));
}

pub fn opcode_cjalr(state: &mut AsmRv32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cr(0x02, 0x09, rs, 0));
}

pub fn opcode_cjr(state: &mut AsmRv32, rs: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cr(0x02, 0x08, rs, 0));
}

pub fn opcode_clw(state: &mut AsmRv32, rd: u32, rs: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cl(0x00, 0x02, rd, rs, offset as u32));
}

pub fn opcode_clwsp(state: &mut AsmRv32, rd: u32, offset: u32) {
    if !ENABLED {
        return;
    }
    let imm = ((offset & 0xc0) >> 6) | (offset & 0x3c);
    emit_halfword_opcode(state, encode_type_ci(0x02, 0x02, rd, imm as i32));
}

pub fn opcode_csw(state: &mut AsmRv32, rs1: u32, rs2: u32, offset: i32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cl(0x00, 0x06, rs1, rs2, offset as u32));
}

pub fn opcode_cswsp(state: &mut AsmRv32, rs: u32, offset: u32) {
    if !ENABLED {
        return;
    }
    let imm = ((offset & 0xc0) >> 6) | (offset & 0x3c);
    emit_halfword_opcode(state, encode_type_css(0x02, 0x06, rs, imm));
}

pub fn opcode_cmpush(state: &mut AsmRv32, reg_list: u32, immediate: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cmpp(0x02, 0x2e, 0x00, reg_list, immediate));
}

pub fn opcode_cmpopret(state: &mut AsmRv32, reg_list: u32, immediate: u32) {
    if !ENABLED {
        return;
    }
    emit_halfword_opcode(state, encode_type_cmpp(0x02, 0x2f, 0x02, reg_list, immediate));
}

fn split_immediate(immediate: i32, upper: &mut u32, lower: &mut u32) {
    let unsigned_immediate = immediate as u32;
    *upper = unsigned_immediate & 0xfffff000;
    *lower = unsigned_immediate & 0x00000fff;
    if (*lower & 0x800) != 0 {
        *upper += 0x1000;
    }
}

fn load_upper_immediate(state: &mut AsmRv32, rd: u32, immediate: u32) {
    if fit_signed(17, immediate as i32) && ((immediate >> 12) != 0) {
        opcode_clui(state, rd, immediate as i32);
    } else {
        opcode_lui(state, rd, immediate as i32);
    }
}

fn load_lower_immediate(state: &mut AsmRv32, rd: u32, immediate: u32) {
    if immediate == 0 {
        return;
    }
    if fit_signed(6, immediate as i32) {
        opcode_caddi(state, rd, immediate as i32);
    } else {
        opcode_addi(state, rd, rd, immediate as i32);
    }
}

fn load_full_immediate(state: &mut AsmRv32, rd: u32, immediate: i32) {
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(immediate, &mut upper, &mut lower);
    load_upper_immediate(state, rd, upper);
    load_lower_immediate(state, rd, lower);
}

pub fn emit_optimised_load_immediate(state: &mut AsmRv32, rd: u32, immediate: i32) {
    if !ENABLED {
        return;
    }
    if fit_signed(6, immediate) {
        opcode_cli(state, rd, immediate);
        return;
    }
    if fit_signed(12, immediate) {
        opcode_addi(state, rd, ASM_RV32_REG_ZERO, immediate);
        return;
    }
    load_full_immediate(state, rd, immediate);
}

fn emit_registers_store(state: &mut AsmRv32, registers_mask: u32) {
    let mut offset = 0u32;
    for register_index in 0..RV32_AVAILABLE_REGISTERS_COUNT {
        if registers_mask & (1 << register_index) != 0 {
            assert!(fit_unsigned(6, offset >> 2));
            opcode_cswsp(state, register_index, offset);
            offset += 4;
        }
    }
}

fn emit_registers_load(state: &mut AsmRv32, registers_mask: u32) {
    let mut offset = 0u32;
    for register_index in 0..RV32_AVAILABLE_REGISTERS_COUNT {
        if registers_mask & (1 << register_index) != 0 {
            assert!(fit_unsigned(6, offset >> 2));
            opcode_clwsp(state, register_index, offset);
            offset += 4;
        }
    }
}

fn adjust_stack(state: &mut AsmRv32, stack_size: i32) {
    if stack_size == 0 {
        return;
    }
    if fit_signed(6, stack_size) {
        opcode_caddi(state, ASM_RV32_REG_SP, stack_size);
        return;
    }
    if fit_signed(12, stack_size) {
        opcode_addi(
            state,
            ASM_RV32_REG_SP,
            ASM_RV32_REG_SP,
            stack_size,
        );
        return;
    }
    load_full_immediate(state, REG_TEMP0, stack_size);
    opcode_cadd(state, ASM_RV32_REG_SP, REG_TEMP0);
}

fn emit_function_prologue(state: &mut AsmRv32, registers: u32) {
    let registers_count = mp_popcount(registers);
    state.stack_size = (registers_count + state.locals_count) * 4;
    let old_saved_registers_mask = state.saved_registers_mask;
    adjust_stack(state, -(state.stack_size as i32));
    emit_registers_store(state, registers);
    state.locals_stack_offset = registers_count * 4;
    state.saved_registers_mask = old_saved_registers_mask;
}

fn emit_function_epilogue(state: &mut AsmRv32, registers: u32) {
    let old_saved_registers_mask = state.saved_registers_mask;
    emit_registers_load(state, registers);
    adjust_stack(state, state.stack_size as i32);
    state.saved_registers_mask = old_saved_registers_mask;
}

fn compute_zcmp_sequence_length(registers: u32) -> u32 {
    assert!(registers != 0 && (registers & !0x0ffc0302) == 0);
    let mut length = 32
        - mp_clz(
            ((registers & 0x00000002) >> 1)
                | ((registers & 0x00000300) >> 7)
                | ((registers & 0x0ffc0000) >> 15),
        );
    if length == 12 {
        length = 13;
    }
    length
}

fn emit_compressed_function_prologue(state: &mut AsmRv32, registers_mask: u32) {
    let sequence_length = compute_zcmp_sequence_length(registers_mask);
    let allocated_stack = (sequence_length + 3) & !3;
    let tail_slack = allocated_stack - sequence_length;
    let locals_left = if state.locals_count < tail_slack {
        0
    } else {
        state.locals_count - tail_slack
    };
    let adjustment_chunks = core::cmp::min(3, locals_left / 4);
    let locals_left = locals_left - adjustment_chunks * 4;
    let stack_size = (locals_left * 4) as i32;
    let reg_list = core::cmp::min(3 + sequence_length, 15);
    opcode_cmpush(state, reg_list, adjustment_chunks);
    adjust_stack(state, -stack_size);
    state.stack_size = (stack_size as u32) | adjustment_chunks;
}

fn emit_compressed_function_epilogue(state: &mut AsmRv32, registers_mask: u32) {
    let sequence_length = compute_zcmp_sequence_length(registers_mask);
    let stack_size = state.stack_size & !0x03;
    adjust_stack(state, stack_size as i32);
    opcode_cmpopret(
        state,
        core::cmp::min(3 + sequence_length, 15),
        state.stack_size & 0x03,
    );
}

fn calculate_displacement_for_label(state: &AsmRv32, label: usize) -> (bool, isize) {
    let label_offset = unsafe { *state.base.label_offsets.add(label) };
    let displacement = label_offset as isize - state.base.code_offset as isize;
    (
        label_offset != usize::MAX && displacement < 0,
        displacement,
    )
}

pub fn entry(state: &mut AsmRv32, locals: u32) {
    if !ENABLED {
        return;
    }
    state.locals_count = locals;
    state.saved_registers_mask |= (1 << REG_FUN_TABLE)
        | (1 << REG_LOCAL_1)
        | (1 << REG_LOCAL_2)
        | (1 << REG_LOCAL_3);
    if allow_zcmp_opcodes() {
        emit_compressed_function_prologue(state, state.saved_registers_mask);
    } else {
        emit_function_prologue(state, state.saved_registers_mask);
    }
}

pub fn exit(state: &mut AsmRv32) {
    if !ENABLED {
        return;
    }
    if allow_zcmp_opcodes() {
        emit_compressed_function_epilogue(state, state.saved_registers_mask);
    } else {
        emit_function_epilogue(state, state.saved_registers_mask);
        opcode_cjr(state, ASM_RV32_REG_RA);
    }
}

pub fn end_pass(_state: &mut AsmRv32) {}

pub fn emit_call_ind(state: &mut AsmRv32, index: u32) {
    if !ENABLED {
        return;
    }
    let offset = index * ASM_WORD_SIZE as u32;
    state.saved_registers_mask |= 1 << ASM_RV32_REG_RA;

    if rv32_is_in_c_register_window(REG_FUN_TABLE)
        && rv32_is_in_c_register_window(INTERNAL_TEMPORARY)
        && fit_unsigned(6, offset)
    {
        state.saved_registers_mask |= 1 << INTERNAL_TEMPORARY;
        opcode_clw(
            state,
            rv32_map_in_c_register_window(INTERNAL_TEMPORARY),
            rv32_map_in_c_register_window(REG_FUN_TABLE),
            offset as i32,
        );
        opcode_cjalr(state, INTERNAL_TEMPORARY);
        return;
    }

    if fit_unsigned(11, offset) {
        opcode_lw(state, REG_TEMP2, REG_FUN_TABLE, offset as i32);
        opcode_cjalr(state, REG_TEMP2);
        return;
    }

    let mut upper = 0;
    let mut lower = 0;
    split_immediate(offset as i32, &mut upper, &mut lower);
    load_upper_immediate(state, REG_TEMP2, upper);
    opcode_cadd(state, REG_TEMP2, REG_FUN_TABLE);
    opcode_lw(state, REG_TEMP2, REG_TEMP2, lower as i32);
    opcode_cjalr(state, REG_TEMP2);
}

pub fn emit_jump_if_reg_eq(state: &mut AsmRv32, rs1: u32, rs2: u32, label: usize) {
    if !ENABLED {
        return;
    }
    let (can_emit_short_jump, mut displacement) = calculate_displacement_for_label(state, label);

    if can_emit_short_jump && fit_signed(13, displacement as i32) {
        opcode_beq(state, rs1, rs2, displacement as i32);
        return;
    }

    displacement -= ASM_WORD_SIZE as isize;
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(displacement as i32, &mut upper, &mut lower);
    opcode_bne(state, rs1, rs2, 12);
    opcode_auipc(state, REG_TEMP2, upper as i32);
    opcode_jalr(state, ASM_RV32_REG_ZERO, REG_TEMP2, lower as i32);
}

pub fn emit_jump_if_reg_nonzero(state: &mut AsmRv32, rs: u32, label: usize) {
    if !ENABLED {
        return;
    }
    let (can_emit_short_jump, mut displacement) = calculate_displacement_for_label(state, label);

    if can_emit_short_jump
        && fit_signed(8, displacement as i32)
        && rv32_is_in_c_register_window(rs)
    {
        opcode_cbnez(state, rv32_map_in_c_register_window(rs), displacement as i32);
        return;
    }

    if can_emit_short_jump && fit_signed(13, displacement as i32) {
        opcode_bne(state, rs, ASM_RV32_REG_ZERO, displacement as i32);
        return;
    }

    if can_emit_short_jump && rv32_is_in_c_register_window(rs) {
        opcode_cbeqz(state, rv32_map_in_c_register_window(rs), 10);
        displacement -= ASM_HALFWORD_SIZE as isize;
    } else {
        opcode_beq(state, rs, ASM_RV32_REG_ZERO, 12);
        displacement -= ASM_WORD_SIZE as isize;
    }

    let mut upper = 0;
    let mut lower = 0;
    split_immediate(displacement as i32, &mut upper, &mut lower);
    opcode_auipc(state, REG_TEMP2, upper as i32);
    opcode_jalr(state, ASM_RV32_REG_ZERO, REG_TEMP2, lower as i32);
}

pub fn emit_mov_local_reg(state: &mut AsmRv32, local: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    let offset = state.locals_stack_offset + (local * ASM_WORD_SIZE as u32);
    if fit_unsigned(6, offset >> 2) {
        opcode_cswsp(state, rs, offset);
        return;
    }
    if fit_unsigned(11, offset) {
        opcode_sw(state, rs, ASM_RV32_REG_SP, offset as i32);
        return;
    }
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(offset as i32, &mut upper, &mut lower);
    load_upper_immediate(state, REG_TEMP2, upper);
    opcode_cadd(state, REG_TEMP2, ASM_RV32_REG_SP);
    opcode_sw(state, rs, REG_TEMP2, lower as i32);
}

pub fn emit_mov_reg_local(state: &mut AsmRv32, rd: u32, local: u32) {
    if !ENABLED {
        return;
    }
    let offset = state.locals_stack_offset + (local * ASM_WORD_SIZE as u32);
    if fit_unsigned(6, offset >> 2) {
        opcode_clwsp(state, rd, offset);
        return;
    }
    if fit_unsigned(11, offset) {
        opcode_lw(state, rd, ASM_RV32_REG_SP, offset as i32);
        return;
    }
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(offset as i32, &mut upper, &mut lower);
    load_upper_immediate(state, rd, upper);
    opcode_cadd(state, rd, ASM_RV32_REG_SP);
    opcode_lw(state, rd, rd, lower as i32);
}

pub fn emit_mov_reg_local_addr(state: &mut AsmRv32, rd: u32, local: u32) {
    if !ENABLED {
        return;
    }
    let offset = state.locals_stack_offset + (local * ASM_WORD_SIZE as u32);
    if fit_unsigned(10, offset)
        && offset != 0
        && rv32_is_in_c_register_window(rd)
    {
        opcode_caddi4spn(state, rv32_map_in_c_register_window(rd), offset);
        return;
    }
    if fit_unsigned(11, offset) {
        opcode_addi(state, rd, ASM_RV32_REG_SP, offset as i32);
        return;
    }
    load_full_immediate(state, rd, offset as i32);
    opcode_cadd(state, rd, ASM_RV32_REG_SP);
}

pub fn emit_load_reg_reg_offset(
    state: &mut AsmRv32,
    rd: u32,
    rs: u32,
    offset: i32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    assert!(operation_size <= 2);
    let scaled_offset = offset << operation_size;
    if scaled_offset >= 0
        && operation_size == 2
        && rv32_is_in_c_register_window(rd)
        && rv32_is_in_c_register_window(rs)
        && misc::fit_unsigned(6, scaled_offset as u32)
    {
        opcode_clw(
            state,
            rv32_map_in_c_register_window(rd),
            rv32_map_in_c_register_window(rs),
            scaled_offset,
        );
        return;
    }
    if misc::fit_signed(12, scaled_offset) {
        emit_word_opcode(
            state,
            encode_type_i(
                0x03,
                RV32_LOAD_OPCODE_FT3[operation_size as usize],
                rd,
                rs,
                scaled_offset,
            ),
        );
        return;
    }
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(scaled_offset, &mut upper, &mut lower);
    load_upper_immediate(state, rd, upper);
    opcode_cadd(state, rd, rs);
    emit_word_opcode(
        state,
        encode_type_i(
            0x03,
            RV32_LOAD_OPCODE_FT3[operation_size as usize],
            rd,
            rd,
            lower as i32,
        ),
    );
}

pub fn emit_jump(state: &mut AsmRv32, label: usize) {
    if !ENABLED {
        return;
    }
    let (can_emit_short_jump, displacement) = calculate_displacement_for_label(state, label);
    if can_emit_short_jump && fit_signed(12, displacement as i32) {
        opcode_cj(state, displacement as i32);
        return;
    }
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(displacement as i32, &mut upper, &mut lower);
    opcode_auipc(state, REG_TEMP2, upper as i32);
    opcode_jalr(state, ASM_RV32_REG_ZERO, REG_TEMP2, lower as i32);
}

pub fn emit_store_reg_reg_offset(
    state: &mut AsmRv32,
    rd: u32,
    rs: u32,
    offset: i32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    assert!(operation_size <= 2);
    let scaled_offset = offset << operation_size;
    if scaled_offset >= 0
        && operation_size == 2
        && rv32_is_in_c_register_window(rd)
        && rv32_is_in_c_register_window(rs)
        && misc::fit_unsigned(6, scaled_offset as u32)
    {
        opcode_csw(
            state,
            rv32_map_in_c_register_window(rd),
            rv32_map_in_c_register_window(rs),
            scaled_offset,
        );
        return;
    }
    if misc::fit_signed(12, scaled_offset) {
        emit_word_opcode(
            state,
            encode_type_s(0x23, operation_size, rs, rd, scaled_offset),
        );
        return;
    }
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(scaled_offset, &mut upper, &mut lower);
    load_upper_immediate(state, REG_TEMP2, upper);
    opcode_cadd(state, REG_TEMP2, rs);
    emit_word_opcode(
        state,
        encode_type_s(0x23, operation_size, REG_TEMP2, rd, lower as i32),
    );
}

pub fn emit_mov_reg_pcrel(state: &mut AsmRv32, rd: u32, label: usize) {
    if !ENABLED {
        return;
    }
    let displacement =
        unsafe { *state.base.label_offsets.add(label) as i32 - state.base.code_offset as i32 };
    let mut upper = 0;
    let mut lower = 0;
    split_immediate(displacement, &mut upper, &mut lower);
    opcode_auipc(state, rd, upper as i32);
    opcode_addi(state, rd, rd, lower as i32);
}

pub fn emit_optimised_xor(state: &mut AsmRv32, rd: u32, rs: u32) {
    if !ENABLED {
        return;
    }
    if rs == rd {
        opcode_cli(state, rd, 0);
        return;
    }
    opcode_xor(state, rd, rd, rs);
}

fn fix_up_scaled_reg_reg_reg(state: &mut AsmRv32, rs1: u32, rs2: u32, operation_size: u32) {
    assert!(operation_size <= 2);
    if operation_size > 0 && allow_zba_opcodes() {
        emit_word_opcode(
            state,
            encode_type_r(
                0x33,
                1 << operation_size,
                0x10,
                REG_TEMP2,
                rs2,
                rs1,
            ),
        );
    } else if operation_size > 0 {
        opcode_slli(state, REG_TEMP2, rs2, operation_size as i32);
        opcode_cadd(state, REG_TEMP2, rs1);
    } else {
        opcode_add(state, REG_TEMP2, rs1, rs2);
    }
}

pub fn emit_load_reg_reg_reg(
    state: &mut AsmRv32,
    rd: u32,
    rs1: u32,
    rs2: u32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    fix_up_scaled_reg_reg_reg(state, rs1, rs2, operation_size);
    emit_load_reg_reg_offset(state, rd, REG_TEMP2, 0, operation_size);
}

pub fn emit_store_reg_reg_reg(
    state: &mut AsmRv32,
    rd: u32,
    rs1: u32,
    rs2: u32,
    operation_size: u32,
) {
    if !ENABLED {
        return;
    }
    fix_up_scaled_reg_reg_reg(state, rs1, rs2, operation_size);
    emit_store_reg_reg_offset(state, rd, REG_TEMP2, 0, operation_size);
}

pub fn meta_comparison_eq(state: &mut AsmRv32, rs1: u32, rs2: u32, rd: u32) {
    if !ENABLED {
        return;
    }
    opcode_sub(state, rd, rs1, rs2);
    opcode_sltiu(state, rd, rd, 1);
}

pub fn meta_comparison_ne(state: &mut AsmRv32, rs1: u32, rs2: u32, rd: u32) {
    if !ENABLED {
        return;
    }
    opcode_sub(state, rd, rs1, rs2);
    opcode_sltu(state, rd, ASM_RV32_REG_ZERO, rd);
}

pub fn meta_comparison_lt(
    state: &mut AsmRv32,
    rs1: u32,
    rs2: u32,
    rd: u32,
    unsigned_comparison: bool,
) {
    if !ENABLED {
        return;
    }
    emit_word_opcode(
        state,
        encode_type_r(
            0x33,
            0x02 | if unsigned_comparison { 1 } else { 0 },
            0x00,
            rd,
            rs1,
            rs2,
        ),
    );
}

pub fn meta_comparison_le(
    state: &mut AsmRv32,
    rs1: u32,
    rs2: u32,
    rd: u32,
    unsigned_comparison: bool,
) {
    if !ENABLED {
        return;
    }
    meta_comparison_lt(state, rs2, rs1, rd, unsigned_comparison);
    opcode_xori(state, rd, rd, 1);
}
