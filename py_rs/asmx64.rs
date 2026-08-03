//! rewrite of py/asmx64.c + py/asmx64.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase};
use crate::mpconfig;

const ENABLED: bool = mpconfig::EMIT_X64;
const WORD_SIZE: i32 = 8;

#[repr(C)]
pub struct AsmX64 {
    pub base: MpAsmBase,
    pub num_locals: i32,
}

pub const ASM_X64_REG_RAX: i32 = 0;
pub const ASM_X64_REG_RCX: i32 = 1;
pub const ASM_X64_REG_RDX: i32 = 2;
pub const ASM_X64_REG_RBX: i32 = 3;
pub const ASM_X64_REG_RSP: i32 = 4;
pub const ASM_X64_REG_RBP: i32 = 5;
pub const ASM_X64_REG_RSI: i32 = 6;
pub const ASM_X64_REG_RDI: i32 = 7;
pub const ASM_X64_REG_R08: i32 = 8;
pub const ASM_X64_REG_R09: i32 = 9;
pub const ASM_X64_REG_R10: i32 = 10;
pub const ASM_X64_REG_R11: i32 = 11;
pub const ASM_X64_REG_R12: i32 = 12;
pub const ASM_X64_REG_R13: i32 = 13;
pub const ASM_X64_REG_R14: i32 = 14;
pub const ASM_X64_REG_R15: i32 = 15;
pub const ASM_X64_REG_FUN_TABLE: i32 = ASM_X64_REG_RBP;

const OPCODE_NOP: u8 = 0x90;
const OPCODE_PUSH_R64: u8 = 0x50;
const OPCODE_POP_R64: u8 = 0x58;
const OPCODE_RET: u8 = 0xc3;
const OPCODE_MOV_I64_TO_R64: u8 = 0xb8;
const OPCODE_MOV_R8_TO_RM8: u8 = 0x88;
const OPCODE_MOV_R64_TO_RM64: u8 = 0x89;
const OPCODE_MOV_RM64_TO_R64: u8 = 0x8b;
const OPCODE_MOVZX_RM8_TO_R64: u8 = 0xb6;
const OPCODE_MOVZX_RM16_TO_R64: u8 = 0xb7;
const OPCODE_LEA_MEM_TO_R64: u8 = 0x8d;
const OPCODE_NOT_RM64: u8 = 0xf7;
const OPCODE_NEG_RM64: u8 = 0xf7;
const OPCODE_AND_R64_TO_RM64: u8 = 0x21;
const OPCODE_OR_R64_TO_RM64: u8 = 0x09;
const OPCODE_XOR_R64_TO_RM64: u8 = 0x31;
const OPCODE_ADD_R64_TO_RM64: u8 = 0x01;
const OPCODE_SUB_R64_FROM_RM64: u8 = 0x29;
const OPCODE_SUB_I32_FROM_RM64: u8 = 0x81;
const OPCODE_SUB_I8_FROM_RM64: u8 = 0x83;
const OPCODE_SHL_RM64_CL: u8 = 0xd3;
const OPCODE_SHR_RM64_CL: u8 = 0xd3;
const OPCODE_SAR_RM64_CL: u8 = 0xd3;
const OPCODE_CMP_R64_WITH_RM64: u8 = 0x39;
const OPCODE_TEST_R8_WITH_RM8: u8 = 0x84;
const OPCODE_TEST_R64_WITH_RM64: u8 = 0x85;
const OPCODE_JMP_REL8: u8 = 0xeb;
const OPCODE_JMP_REL32: u8 = 0xe9;
const OPCODE_JMP_RM64: u8 = 0xff;
const OPCODE_JCC_REL8: u8 = 0x70;
const OPCODE_JCC_REL32_A: u8 = 0x0f;
const OPCODE_JCC_REL32_B: u8 = 0x80;
const OPCODE_SETCC_RM8_A: u8 = 0x0f;
const OPCODE_SETCC_RM8_B: u8 = 0x90;
const OPCODE_CALL_RM32: u8 = 0xff;
const OP_SIZE_PREFIX: u8 = 0x66;
const REX_PREFIX: u8 = 0x40;
const REX_W: u8 = 0x08;
const MODRM_RM_DISP0: u8 = 0x00;
const MODRM_RM_DISP8: u8 = 0x40;
const MODRM_RM_DISP32: u8 = 0x80;
const MODRM_RM_REG: u8 = 0xc0;

#[inline]
const fn modrm_r64(x: i32) -> u8 {
    ((x & 0x7) << 3) as u8
}

#[inline]
const fn modrm_rm_r64(x: i32) -> u8 {
    (x & 0x7) as u8
}

#[inline]
const fn rex_w_from_r64(r64: i32) -> u8 {
    ((r64 >> 0) & 0x08) as u8
}

#[inline]
const fn rex_r_from_r64(r64: i32) -> u8 {
    ((r64 >> 1) & 0x04) as u8
}

#[inline]
const fn rex_b_from_r64(r64: i32) -> u8 {
    ((r64 >> 3) & 0x01) as u8
}

#[inline]
const fn imm32_l0(x: i32) -> u8 {
    (x & 0xff) as u8
}

#[inline]
const fn imm32_l1(x: i32) -> u8 {
    ((x >> 8) & 0xff) as u8
}

#[inline]
const fn imm32_l2(x: i32) -> u8 {
    ((x >> 16) & 0xff) as u8
}

#[inline]
const fn imm32_l3(x: i32) -> u8 {
    ((x >> 24) & 0xff) as u8
}

#[inline]
const fn imm64_l4(x: i64) -> u8 {
    ((x >> 32) & 0xff) as u8
}

#[inline]
const fn imm64_l5(x: i64) -> u8 {
    ((x >> 40) & 0xff) as u8
}

#[inline]
const fn imm64_l6(x: i64) -> u8 {
    ((x >> 48) & 0xff) as u8
}

#[inline]
const fn imm64_l7(x: i64) -> u8 {
    ((x >> 56) & 0xff) as u8
}

#[inline]
const fn unsigned_fit32(x: i64) -> bool {
    (x as u64 & 0xffff_ffff_0000_0000u64) == 0
}

#[inline]
const fn signed_fit8(x: isize) -> bool {
    (x & 0xffffff80) == 0 || (x & 0xffffff80) == 0xffffff80
}

fn get_cur_to_write_bytes(asm: &mut AsmX64, n: usize) -> *mut u8 {
    asmbase::get_cur_to_write_bytes(&mut asm.base, n)
}

fn write_byte_1(asm: &mut AsmX64, b1: u8) {
    let c = get_cur_to_write_bytes(asm, 1);
    if !c.is_null() {
        unsafe { *c = b1 };
    }
}

fn write_byte_2(asm: &mut AsmX64, b1: u8, b2: u8) {
    let c = get_cur_to_write_bytes(asm, 2);
    if !c.is_null() {
        unsafe {
            *c = b1;
            *c.add(1) = b2;
        }
    }
}

fn write_byte_3(asm: &mut AsmX64, b1: u8, b2: u8, b3: u8) {
    let c = get_cur_to_write_bytes(asm, 3);
    if !c.is_null() {
        unsafe {
            *c = b1;
            *c.add(1) = b2;
            *c.add(2) = b3;
        }
    }
}

fn write_word32(asm: &mut AsmX64, w32: i32) {
    let c = get_cur_to_write_bytes(asm, 4);
    if !c.is_null() {
        unsafe {
            *c = imm32_l0(w32);
            *c.add(1) = imm32_l1(w32);
            *c.add(2) = imm32_l2(w32);
            *c.add(3) = imm32_l3(w32);
        }
    }
}

fn write_word64(asm: &mut AsmX64, w64: i64) {
    let c = get_cur_to_write_bytes(asm, 8);
    if !c.is_null() {
        unsafe {
            *c = imm32_l0(w64 as i32);
            *c.add(1) = imm32_l1(w64 as i32);
            *c.add(2) = imm32_l2(w64 as i32);
            *c.add(3) = imm32_l3(w64 as i32);
            *c.add(4) = imm64_l4(w64);
            *c.add(5) = imm64_l5(w64);
            *c.add(6) = imm64_l6(w64);
            *c.add(7) = imm64_l7(w64);
        }
    }
}

fn write_r64_disp(asm: &mut AsmX64, r64: i32, disp_r64: i32, disp_offset: i32) {
    let rm_disp = if disp_offset == 0 && (disp_r64 & 7) != ASM_X64_REG_RBP {
        MODRM_RM_DISP0
    } else if signed_fit8(disp_offset as isize) {
        MODRM_RM_DISP8
    } else {
        MODRM_RM_DISP32
    };
    write_byte_1(
        asm,
        modrm_r64(r64) | rm_disp | modrm_rm_r64(disp_r64),
    );
    if (disp_r64 & 7) == ASM_X64_REG_RSP {
        write_byte_1(asm, 0x24);
    }
    if rm_disp == MODRM_RM_DISP8 {
        write_byte_1(asm, imm32_l0(disp_offset));
    } else if rm_disp == MODRM_RM_DISP32 {
        write_word32(asm, disp_offset);
    }
}

fn generic_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32, op: u8) {
    write_byte_3(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(src_r64) | rex_b_from_r64(dest_r64),
        op,
        modrm_r64(src_r64) | MODRM_RM_REG | modrm_rm_r64(dest_r64),
    );
}

pub fn end_pass(_asm: &mut AsmX64) {}

pub fn nop(asm: &mut AsmX64) {
    if !ENABLED {
        return;
    }
    write_byte_1(asm, OPCODE_NOP);
}

pub fn push_r64(asm: &mut AsmX64, src_r64: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 {
        write_byte_1(asm, OPCODE_PUSH_R64 | src_r64 as u8);
    } else {
        write_byte_2(asm, REX_PREFIX | rex_b_from_r64(src_r64), OPCODE_PUSH_R64 | (src_r64 & 7) as u8);
    }
}

pub fn pop_r64(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    if dest_r64 < 8 {
        write_byte_1(asm, OPCODE_POP_R64 | dest_r64 as u8);
    } else {
        write_byte_2(asm, REX_PREFIX | rex_b_from_r64(dest_r64), OPCODE_POP_R64 | (dest_r64 & 7) as u8);
    }
}

fn ret(asm: &mut AsmX64) {
    write_byte_1(asm, OPCODE_RET);
}

pub fn mov_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_MOV_R64_TO_RM64);
}

pub fn mov_r8_to_mem8(asm: &mut AsmX64, src_r64: i32, dest_r64: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_1(asm, OPCODE_MOV_R8_TO_RM8);
    } else {
        write_byte_2(
            asm,
            REX_PREFIX | rex_r_from_r64(src_r64) | rex_b_from_r64(dest_r64),
            OPCODE_MOV_R8_TO_RM8,
        );
    }
    write_r64_disp(asm, src_r64, dest_r64, dest_disp);
}

pub fn mov_r16_to_mem16(asm: &mut AsmX64, src_r64: i32, dest_r64: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_2(asm, OP_SIZE_PREFIX, OPCODE_MOV_R64_TO_RM64);
    } else {
        write_byte_3(
            asm,
            OP_SIZE_PREFIX,
            REX_PREFIX | rex_r_from_r64(src_r64) | rex_b_from_r64(dest_r64),
            OPCODE_MOV_R64_TO_RM64,
        );
    }
    write_r64_disp(asm, src_r64, dest_r64, dest_disp);
}

pub fn mov_r32_to_mem32(asm: &mut AsmX64, src_r64: i32, dest_r64: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_1(asm, OPCODE_MOV_R64_TO_RM64);
    } else {
        write_byte_2(
            asm,
            REX_PREFIX | rex_r_from_r64(src_r64) | rex_b_from_r64(dest_r64),
            OPCODE_MOV_R64_TO_RM64,
        );
    }
    write_r64_disp(asm, src_r64, dest_r64, dest_disp);
}

pub fn mov_r64_to_mem64(asm: &mut AsmX64, src_r64: i32, dest_r64: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(src_r64) | rex_b_from_r64(dest_r64),
        OPCODE_MOV_R64_TO_RM64,
    );
    write_r64_disp(asm, src_r64, dest_r64, dest_disp);
}

pub fn mov_mem8_to_r64zx(asm: &mut AsmX64, src_r64: i32, src_disp: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_2(asm, 0x0f, OPCODE_MOVZX_RM8_TO_R64);
    } else {
        write_byte_3(
            asm,
            REX_PREFIX | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
            0x0f,
            OPCODE_MOVZX_RM8_TO_R64,
        );
    }
    write_r64_disp(asm, dest_r64, src_r64, src_disp);
}

pub fn mov_mem16_to_r64zx(asm: &mut AsmX64, src_r64: i32, src_disp: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_2(asm, 0x0f, OPCODE_MOVZX_RM16_TO_R64);
    } else {
        write_byte_3(
            asm,
            REX_PREFIX | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
            0x0f,
            OPCODE_MOVZX_RM16_TO_R64,
        );
    }
    write_r64_disp(asm, dest_r64, src_r64, src_disp);
}

pub fn mov_mem32_to_r64zx(asm: &mut AsmX64, src_r64: i32, src_disp: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    if src_r64 < 8 && dest_r64 < 8 {
        write_byte_1(asm, OPCODE_MOV_RM64_TO_R64);
    } else {
        write_byte_2(
            asm,
            REX_PREFIX | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
            OPCODE_MOV_RM64_TO_R64,
        );
    }
    write_r64_disp(asm, dest_r64, src_r64, src_disp);
}

pub fn mov_mem64_to_r64(asm: &mut AsmX64, src_r64: i32, src_disp: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
        OPCODE_MOV_RM64_TO_R64,
    );
    write_r64_disp(asm, dest_r64, src_r64, src_disp);
}

fn lea_disp_to_r64(asm: &mut AsmX64, src_r64: i32, src_disp: i32, dest_r64: i32) {
    write_byte_2(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
        OPCODE_LEA_MEM_TO_R64,
    );
    write_r64_disp(asm, dest_r64, src_r64, src_disp);
}

pub fn mov_i32_to_r64(asm: &mut AsmX64, src_i32: i32, dest_r64: i32) -> usize {
    if !ENABLED {
        return 0;
    }
    if dest_r64 < 8 {
        write_byte_1(asm, OPCODE_MOV_I64_TO_R64 | dest_r64 as u8);
    } else {
        write_byte_2(asm, REX_PREFIX | rex_b_from_r64(dest_r64), OPCODE_MOV_I64_TO_R64 | (dest_r64 & 7) as u8);
    }
    let loc = asm.base.get_code_pos();
    write_word32(asm, src_i32);
    loc
}

pub fn mov_i64_to_r64(asm: &mut AsmX64, src_i64: i64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(
        asm,
        REX_PREFIX | REX_W | if dest_r64 < 8 { 0 } else { rex_b_from_r64(dest_r64) },
        OPCODE_MOV_I64_TO_R64 | (dest_r64 & 7) as u8,
    );
    write_word64(asm, src_i64);
}

pub fn mov_i64_to_r64_optimised(asm: &mut AsmX64, src_i64: i64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    if unsigned_fit32(src_i64) {
        mov_i32_to_r64(asm, (src_i64 & 0xffffffff) as i32, dest_r64);
    } else {
        mov_i64_to_r64(asm, src_i64, dest_r64);
    }
}

pub fn not_r64(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, 2, OPCODE_NOT_RM64);
}

pub fn neg_r64(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, 3, OPCODE_NEG_RM64);
}

pub fn and_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_AND_R64_TO_RM64);
}

pub fn or_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_OR_R64_TO_RM64);
}

pub fn xor_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_XOR_R64_TO_RM64);
}

pub fn shl_r64_cl(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, 4, OPCODE_SHL_RM64_CL);
}

pub fn shr_r64_cl(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, 5, OPCODE_SHR_RM64_CL);
}

pub fn sar_r64_cl(asm: &mut AsmX64, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, 7, OPCODE_SAR_RM64_CL);
}

pub fn add_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_ADD_R64_TO_RM64);
}

pub fn sub_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, dest_r64, src_r64, OPCODE_SUB_R64_FROM_RM64);
}

pub fn mul_r64_r64(asm: &mut AsmX64, dest_r64: i32, src_r64: i32) {
    if !ENABLED {
        return;
    }
    write_byte_1(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(dest_r64) | rex_b_from_r64(src_r64),
    );
    write_byte_3(
        asm,
        0x0f,
        0xaf,
        modrm_r64(dest_r64) | MODRM_RM_REG | modrm_rm_r64(src_r64),
    );
}

fn sub_r64_i32(asm: &mut AsmX64, dest_r64: i32, src_i32: i32) {
    assert!(dest_r64 < 8);
    if signed_fit8(src_i32 as isize) {
        write_byte_3(
            asm,
            REX_PREFIX | REX_W,
            OPCODE_SUB_I8_FROM_RM64,
            modrm_r64(5) | MODRM_RM_REG | modrm_rm_r64(dest_r64),
        );
        write_byte_1(asm, (src_i32 & 0xff) as u8);
    } else {
        write_byte_3(
            asm,
            REX_PREFIX | REX_W,
            OPCODE_SUB_I32_FROM_RM64,
            modrm_r64(5) | MODRM_RM_REG | modrm_rm_r64(dest_r64),
        );
        write_word32(asm, src_i32);
    }
}

pub fn cmp_r64_with_r64(asm: &mut AsmX64, src_r64_a: i32, src_r64_b: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, src_r64_b, src_r64_a, OPCODE_CMP_R64_WITH_RM64);
}

pub fn test_r8_with_r8(asm: &mut AsmX64, src_r64_a: i32, src_r64_b: i32) {
    if !ENABLED {
        return;
    }
    assert!(src_r64_a < 8);
    assert!(src_r64_b < 8);
    write_byte_2(
        asm,
        OPCODE_TEST_R8_WITH_RM8,
        modrm_r64(src_r64_a) | MODRM_RM_REG | modrm_rm_r64(src_r64_b),
    );
}

pub fn test_r64_with_r64(asm: &mut AsmX64, src_r64_a: i32, src_r64_b: i32) {
    if !ENABLED {
        return;
    }
    generic_r64_r64(asm, src_r64_b, src_r64_a, OPCODE_TEST_R64_WITH_RM64);
}

pub fn setcc_r8(asm: &mut AsmX64, jcc_type: i32, dest_r8: i32) {
    if !ENABLED {
        return;
    }
    assert!(dest_r8 < 8);
    write_byte_3(
        asm,
        OPCODE_SETCC_RM8_A,
        OPCODE_SETCC_RM8_B | jcc_type as u8,
        modrm_r64(0) | MODRM_RM_REG | modrm_rm_r64(dest_r8),
    );
}

pub fn jmp_reg(asm: &mut AsmX64, src_r64: i32) {
    if !ENABLED {
        return;
    }
    assert!(src_r64 < 8);
    write_byte_2(
        asm,
        OPCODE_JMP_RM64,
        modrm_r64(4) | MODRM_RM_REG | modrm_rm_r64(src_r64),
    );
}

fn get_label_dest(asm: &AsmX64, label: usize) -> usize {
    assert!(label < asm.base.max_num_labels);
    unsafe { *asm.base.label_offsets.add(label) }
}

pub fn jmp_label(asm: &mut AsmX64, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(asm, label);
    let mut rel = dest as isize - asm.base.code_offset as isize;
    if dest != usize::MAX && rel < 0 {
        rel -= 2;
        if signed_fit8(rel) {
            write_byte_2(asm, OPCODE_JMP_REL8, (rel & 0xff) as u8);
            return;
        }
        rel += 2;
    }
    rel -= 5;
    write_byte_1(asm, OPCODE_JMP_REL32);
    write_word32(asm, rel as i32);
}

pub fn jcc_label(asm: &mut AsmX64, jcc_type: i32, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(asm, label);
    let mut rel = dest as isize - asm.base.code_offset as isize;
    if dest != usize::MAX && rel < 0 {
        rel -= 2;
        if signed_fit8(rel) {
            write_byte_2(asm, OPCODE_JCC_REL8 | jcc_type as u8, (rel & 0xff) as u8);
            return;
        }
        rel += 2;
    }
    rel -= 6;
    write_byte_2(asm, OPCODE_JCC_REL32_A, OPCODE_JCC_REL32_B | jcc_type as u8);
    write_word32(asm, rel as i32);
}

pub fn entry(asm: &mut AsmX64, num_locals: i32) {
    if !ENABLED {
        return;
    }
    assert!(num_locals >= 0);
    push_r64(asm, ASM_X64_REG_RBP);
    push_r64(asm, ASM_X64_REG_RBX);
    push_r64(asm, ASM_X64_REG_R12);
    push_r64(asm, ASM_X64_REG_R13);
    let num_locals = num_locals | 1;
    sub_r64_i32(asm, ASM_X64_REG_RSP, num_locals * WORD_SIZE);
    asm.num_locals = num_locals;
}

pub fn exit(asm: &mut AsmX64) {
    if !ENABLED {
        return;
    }
    sub_r64_i32(asm, ASM_X64_REG_RSP, -asm.num_locals * WORD_SIZE);
    pop_r64(asm, ASM_X64_REG_R13);
    pop_r64(asm, ASM_X64_REG_R12);
    pop_r64(asm, ASM_X64_REG_RBX);
    pop_r64(asm, ASM_X64_REG_RBP);
    ret(asm);
}

fn local_offset_from_rsp(_asm: &AsmX64, local_num: i32) -> i32 {
    local_num * WORD_SIZE
}

pub fn mov_local_to_r64(asm: &mut AsmX64, src_local_num: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    mov_mem64_to_r64(
        asm,
        ASM_X64_REG_RSP,
        local_offset_from_rsp(asm, src_local_num),
        dest_r64,
    );
}

pub fn mov_r64_to_local(asm: &mut AsmX64, src_r64: i32, dest_local_num: i32) {
    if !ENABLED {
        return;
    }
    mov_r64_to_mem64(
        asm,
        src_r64,
        ASM_X64_REG_RSP,
        local_offset_from_rsp(asm, dest_local_num),
    );
}

pub fn mov_local_addr_to_r64(asm: &mut AsmX64, local_num: i32, dest_r64: i32) {
    if !ENABLED {
        return;
    }
    let offset = local_offset_from_rsp(asm, local_num);
    if offset == 0 {
        mov_r64_r64(asm, dest_r64, ASM_X64_REG_RSP);
    } else {
        lea_disp_to_r64(asm, ASM_X64_REG_RSP, offset, dest_r64);
    }
}

pub fn mov_reg_pcrel(asm: &mut AsmX64, dest_r64: i32, label: usize) {
    if !ENABLED {
        return;
    }
    let dest = get_label_dest(asm, label);
    let rel = dest as isize - (asm.base.code_offset as isize + 7);
    write_byte_3(
        asm,
        REX_PREFIX | REX_W | rex_r_from_r64(dest_r64),
        OPCODE_LEA_MEM_TO_R64,
        modrm_r64(dest_r64) | modrm_rm_r64(5),
    );
    write_word32(asm, rel as i32);
}

pub fn call_ind(asm: &mut AsmX64, fun_id: usize, temp_r64: i32) {
    if !ENABLED {
        return;
    }
    assert!(temp_r64 < 8);
    mov_mem64_to_r64(
        asm,
        ASM_X64_REG_FUN_TABLE,
        (fun_id * WORD_SIZE as usize) as i32,
        temp_r64,
    );
    write_byte_2(
        asm,
        OPCODE_CALL_RM32,
        modrm_r64(2) | MODRM_RM_REG | modrm_rm_r64(temp_r64),
    );
}
