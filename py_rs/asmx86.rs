//! rewrite of py/asmx86.c + py/asmx86.h
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::collapsible_else_if
)]

use crate::asmbase::{self, MpAsmBase};
use crate::mpconfig;

const ENABLED: bool = mpconfig::EMIT_X86;
const WORD_SIZE: i32 = 4;

#[repr(C)]
pub struct AsmX86 {
    pub base: MpAsmBase,
    pub num_locals: i32,
}

pub const ASM_X86_REG_EAX: i32 = 0;
pub const ASM_X86_REG_ECX: i32 = 1;
pub const ASM_X86_REG_EDX: i32 = 2;
pub const ASM_X86_REG_EBX: i32 = 3;
pub const ASM_X86_REG_ESP: i32 = 4;
pub const ASM_X86_REG_EBP: i32 = 5;
pub const ASM_X86_REG_ESI: i32 = 6;
pub const ASM_X86_REG_EDI: i32 = 7;

pub const ASM_X86_REG_ARG_1: i32 = ASM_X86_REG_EAX;
pub const ASM_X86_REG_ARG_2: i32 = ASM_X86_REG_ECX;
pub const ASM_X86_REG_ARG_3: i32 = ASM_X86_REG_EDX;
pub const ASM_X86_REG_ARG_4: i32 = ASM_X86_REG_EBX;

pub const ASM_X86_CC_JB: i32 = 0x2;
pub const ASM_X86_CC_JAE: i32 = 0x3;
pub const ASM_X86_CC_JZ: i32 = 0x4;
pub const ASM_X86_CC_JE: i32 = 0x4;
pub const ASM_X86_CC_JNZ: i32 = 0x5;
pub const ASM_X86_CC_JNE: i32 = 0x5;
pub const ASM_X86_CC_JBE: i32 = 0x6;
pub const ASM_X86_CC_JA: i32 = 0x7;
pub const ASM_X86_CC_JL: i32 = 0xc;
pub const ASM_X86_CC_JGE: i32 = 0xd;
pub const ASM_X86_CC_JLE: i32 = 0xe;
pub const ASM_X86_CC_JG: i32 = 0xf;

pub const ASM_X86_REG_FUN_TABLE: i32 = ASM_X86_REG_EBP;

const OPCODE_PUSH_R32: u8 = 0x50;
const OPCODE_POP_R32: u8 = 0x58;
const OPCODE_RET: u8 = 0xc3;
const OPCODE_MOV_I32_TO_R32: u8 = 0xb8;
const OPCODE_MOV_R8_TO_RM8: u8 = 0x88;
const OPCODE_MOV_R32_TO_RM32: u8 = 0x89;
const OPCODE_MOV_RM32_TO_R32: u8 = 0x8b;
const OPCODE_MOVZX_RM8_TO_R32: u8 = 0xb6;
const OPCODE_MOVZX_RM16_TO_R32: u8 = 0xb7;
const OPCODE_LEA_MEM_TO_R32: u8 = 0x8d;
const OPCODE_NOT_RM32: u8 = 0xf7;
const OPCODE_NEG_RM32: u8 = 0xf7;
const OPCODE_AND_R32_TO_RM32: u8 = 0x21;
const OPCODE_OR_R32_TO_RM32: u8 = 0x09;
const OPCODE_XOR_R32_TO_RM32: u8 = 0x31;
const OPCODE_ADD_R32_TO_RM32: u8 = 0x01;
const OPCODE_ADD_I32_TO_RM32: u8 = 0x81;
const OPCODE_ADD_I8_TO_RM32: u8 = 0x83;
const OPCODE_SUB_R32_FROM_RM32: u8 = 0x29;
const OPCODE_SUB_I32_FROM_RM32: u8 = 0x81;
const OPCODE_SUB_I8_FROM_RM32: u8 = 0x83;
const OPCODE_SHL_RM32_CL: u8 = 0xd3;
const OPCODE_SHR_RM32_CL: u8 = 0xd3;
const OPCODE_SAR_RM32_CL: u8 = 0xd3;
const OPCODE_CMP_R32_WITH_RM32: u8 = 0x39;
const OPCODE_TEST_R8_WITH_RM8: u8 = 0x84;
const OPCODE_TEST_R32_WITH_RM32: u8 = 0x85;
const OPCODE_JMP_REL8: u8 = 0xeb;
const OPCODE_JMP_REL32: u8 = 0xe9;
const OPCODE_JMP_RM32: u8 = 0xff;
const OPCODE_JCC_REL8: u8 = 0x70;
const OPCODE_JCC_REL32_A: u8 = 0x0f;
const OPCODE_JCC_REL32_B: u8 = 0x80;
const OPCODE_SETCC_RM8_A: u8 = 0x0f;
const OPCODE_SETCC_RM8_B: u8 = 0x90;
const OPCODE_CALL_REL32: u8 = 0xe8;
const OPCODE_CALL_RM32: u8 = 0xff;
const OP_SIZE_PREFIX: u8 = 0x66;

const MODRM_RM_DISP0: u8 = 0x00;
const MODRM_RM_DISP8: u8 = 0x40;
const MODRM_RM_DISP32: u8 = 0x80;
const MODRM_RM_REG: u8 = 0xc0;

#[inline]
const fn modrm_r32(x: i32) -> u8 {
    ((x & 0x7) << 3) as u8
}

#[inline]
const fn modrm_rm_r32(x: i32) -> u8 {
    (x & 0x7) as u8
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
const fn signed_fit8(x: isize) -> bool {
    (x & 0xffffff80) == 0 || (x & 0xffffff80) == 0xffffff80
}

fn get_cur_to_write_bytes(asm: &mut AsmX86, n: usize) -> *mut u8 {
    asmbase::get_cur_to_write_bytes(&mut asm.base, n)
}

fn write_byte_1(asm: &mut AsmX86, b1: u8) {
    let c = get_cur_to_write_bytes(asm, 1);
    if !c.is_null() {
        unsafe { *c = b1 };
    }
}

fn write_byte_2(asm: &mut AsmX86, b1: u8, b2: u8) {
    let c = get_cur_to_write_bytes(asm, 2);
    if !c.is_null() {
        unsafe {
            *c = b1;
            *c.add(1) = b2;
        }
    }
}

fn write_byte_3(asm: &mut AsmX86, b1: u8, b2: u8, b3: u8) {
    let c = get_cur_to_write_bytes(asm, 3);
    if !c.is_null() {
        unsafe {
            *c = b1;
            *c.add(1) = b2;
            *c.add(2) = b3;
        }
    }
}

fn write_word32(asm: &mut AsmX86, w32: i32) {
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

fn write_r32_disp(asm: &mut AsmX86, r32: i32, disp_r32: i32, disp_offset: i32) {
    let rm_disp = if disp_offset == 0 && disp_r32 != ASM_X86_REG_EBP {
        MODRM_RM_DISP0
    } else if signed_fit8(disp_offset as isize) {
        MODRM_RM_DISP8
    } else {
        MODRM_RM_DISP32
    };
    write_byte_1(asm, modrm_r32(r32) | rm_disp | modrm_rm_r32(disp_r32));
    if disp_r32 == ASM_X86_REG_ESP {
        write_byte_1(asm, 0x24);
    }
    if rm_disp == MODRM_RM_DISP8 {
        write_byte_1(asm, imm32_l0(disp_offset));
    } else if rm_disp == MODRM_RM_DISP32 {
        write_word32(asm, disp_offset);
    }
}

fn generic_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32, op: u8) {
    write_byte_2(
        asm,
        op,
        modrm_r32(src_r32) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
    );
}

fn push_r32(asm: &mut AsmX86, src_r32: i32) {
    write_byte_1(asm, OPCODE_PUSH_R32 | src_r32 as u8);
}

fn pop_r32(asm: &mut AsmX86, dest_r32: i32) {
    write_byte_1(asm, OPCODE_POP_R32 | dest_r32 as u8);
}

fn ret(asm: &mut AsmX86) {
    write_byte_1(asm, OPCODE_RET);
}

fn lea_disp_to_r32(asm: &mut AsmX86, src_r32: i32, src_disp: i32, dest_r32: i32) {
    write_byte_1(asm, OPCODE_LEA_MEM_TO_R32);
    write_r32_disp(asm, dest_r32, src_r32, src_disp);
}

fn add_i32_to_r32(asm: &mut AsmX86, src_i32: i32, dest_r32: i32) {
    if signed_fit8(src_i32 as isize) {
        write_byte_2(
            asm,
            OPCODE_ADD_I8_TO_RM32,
            modrm_r32(0) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
        );
        write_byte_1(asm, (src_i32 & 0xff) as u8);
    } else {
        write_byte_2(
            asm,
            OPCODE_ADD_I32_TO_RM32,
            modrm_r32(0) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
        );
        write_word32(asm, src_i32);
    }
}

fn sub_r32_i32(asm: &mut AsmX86, dest_r32: i32, src_i32: i32) {
    if signed_fit8(src_i32 as isize) {
        write_byte_2(
            asm,
            OPCODE_SUB_I8_FROM_RM32,
            modrm_r32(5) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
        );
        write_byte_1(asm, (src_i32 & 0xff) as u8);
    } else {
        write_byte_2(
            asm,
            OPCODE_SUB_I32_FROM_RM32,
            modrm_r32(5) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
        );
        write_word32(asm, src_i32);
    }
}

fn get_label_dest(asm: &AsmX86, label: usize) -> usize {
    assert!(label < asm.base.max_num_labels);
    unsafe { *asm.base.label_offsets.add(label) }
}

fn arg_offset_from_esp(asm: &AsmX86, arg_num: usize) -> i32 {
    (asm.num_locals + 4 + 1 + arg_num as i32) * WORD_SIZE
}

fn local_offset_from_esp(_asm: &AsmX86, local_num: i32) -> i32 {
    local_num * WORD_SIZE
}

pub fn end_pass(_asm: &mut AsmX86) {}

pub fn mov_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_MOV_R32_TO_RM32);
}

pub fn mov_r8_to_mem8(asm: &mut AsmX86, src_r32: i32, dest_r32: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    write_byte_1(asm, OPCODE_MOV_R8_TO_RM8);
    write_r32_disp(asm, src_r32, dest_r32, dest_disp);
}

pub fn mov_r16_to_mem16(asm: &mut AsmX86, src_r32: i32, dest_r32: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(asm, OP_SIZE_PREFIX, OPCODE_MOV_R32_TO_RM32);
    write_r32_disp(asm, src_r32, dest_r32, dest_disp);
}

pub fn mov_r32_to_mem32(asm: &mut AsmX86, src_r32: i32, dest_r32: i32, dest_disp: i32) {
    if !ENABLED {
        return;
    }
    write_byte_1(asm, OPCODE_MOV_R32_TO_RM32);
    write_r32_disp(asm, src_r32, dest_r32, dest_disp);
}

pub fn mov_mem8_to_r32zx(asm: &mut AsmX86, src_r32: i32, src_disp: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(asm, 0x0f, OPCODE_MOVZX_RM8_TO_R32);
    write_r32_disp(asm, dest_r32, src_r32, src_disp);
}

pub fn mov_mem16_to_r32zx(asm: &mut AsmX86, src_r32: i32, src_disp: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(asm, 0x0f, OPCODE_MOVZX_RM16_TO_R32);
    write_r32_disp(asm, dest_r32, src_r32, src_disp);
}

pub fn mov_mem32_to_r32(asm: &mut AsmX86, src_r32: i32, src_disp: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    write_byte_1(asm, OPCODE_MOV_RM32_TO_R32);
    write_r32_disp(asm, dest_r32, src_r32, src_disp);
}

pub fn mov_i32_to_r32(asm: &mut AsmX86, src_i32: i32, dest_r32: i32) -> usize {
    if !ENABLED {
        return 0;
    }
    write_byte_1(asm, OPCODE_MOV_I32_TO_R32 | dest_r32 as u8);
    let loc = asm.base.get_code_pos();
    write_word32(asm, src_i32);
    loc
}

pub fn not_r32(asm: &mut AsmX86, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, 2, OPCODE_NOT_RM32);
}

pub fn neg_r32(asm: &mut AsmX86, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, 3, OPCODE_NEG_RM32);
}

pub fn and_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_AND_R32_TO_RM32);
}

pub fn or_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_OR_R32_TO_RM32);
}

pub fn xor_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_XOR_R32_TO_RM32);
}

pub fn shl_r32_cl(asm: &mut AsmX86, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, 4, OPCODE_SHL_RM32_CL);
}

pub fn shr_r32_cl(asm: &mut AsmX86, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, 5, OPCODE_SHR_RM32_CL);
}

pub fn sar_r32_cl(asm: &mut AsmX86, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, 7, OPCODE_SAR_RM32_CL);
}

pub fn add_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_ADD_R32_TO_RM32);
}

pub fn sub_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, dest_r32, src_r32, OPCODE_SUB_R32_FROM_RM32);
}

pub fn mul_r32_r32(asm: &mut AsmX86, dest_r32: i32, src_r32: i32) {
    if !ENABLED {
        return;
    }
    write_byte_3(
        asm,
        0x0f,
        0xaf,
        modrm_r32(dest_r32) | MODRM_RM_REG | modrm_rm_r32(src_r32),
    );
}

pub fn cmp_r32_with_r32(asm: &mut AsmX86, src_r32_a: i32, src_r32_b: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, src_r32_b, src_r32_a, OPCODE_CMP_R32_WITH_RM32);
}

pub fn test_r8_with_r8(asm: &mut AsmX86, src_r32_a: i32, src_r32_b: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(
        asm,
        OPCODE_TEST_R8_WITH_RM8,
        modrm_r32(src_r32_a) | MODRM_RM_REG | modrm_rm_r32(src_r32_b),
    );
}

pub fn test_r32_with_r32(asm: &mut AsmX86, src_r32_a: i32, src_r32_b: i32) {
    if !ENABLED {
        return;
    }
    generic_r32_r32(asm, src_r32_b, src_r32_a, OPCODE_TEST_R32_WITH_RM32);
}

pub fn setcc_r8(asm: &mut AsmX86, jcc_type: i32, dest_r8: i32) {
    if !ENABLED {
        return;
    }
    write_byte_3(
        asm,
        OPCODE_SETCC_RM8_A,
        OPCODE_SETCC_RM8_B | jcc_type as u8,
        modrm_r32(0) | MODRM_RM_REG | modrm_rm_r32(dest_r8),
    );
}

pub fn jmp_reg(asm: &mut AsmX86, src_r32: i32) {
    if !ENABLED {
        return;
    }
    write_byte_2(
        asm,
        OPCODE_JMP_RM32,
        modrm_r32(4) | MODRM_RM_REG | modrm_rm_r32(src_r32),
    );
}

pub fn jmp_label(asm: &mut AsmX86, label: usize) {
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

pub fn jcc_label(asm: &mut AsmX86, jcc_type: i32, label: usize) {
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

pub fn entry(asm: &mut AsmX86, num_locals: i32) {
    if !ENABLED {
        return;
    }
    assert!(num_locals >= 0);
    push_r32(asm, ASM_X86_REG_EBP);
    push_r32(asm, ASM_X86_REG_EBX);
    push_r32(asm, ASM_X86_REG_ESI);
    push_r32(asm, ASM_X86_REG_EDI);
    let num_locals = num_locals | 3;
    sub_r32_i32(asm, ASM_X86_REG_ESP, num_locals * WORD_SIZE);
    asm.num_locals = num_locals;
}

pub fn exit(asm: &mut AsmX86) {
    if !ENABLED {
        return;
    }
    sub_r32_i32(asm, ASM_X86_REG_ESP, -asm.num_locals * WORD_SIZE);
    pop_r32(asm, ASM_X86_REG_EDI);
    pop_r32(asm, ASM_X86_REG_ESI);
    pop_r32(asm, ASM_X86_REG_EBX);
    pop_r32(asm, ASM_X86_REG_EBP);
    ret(asm);
}

pub fn mov_arg_to_r32(asm: &mut AsmX86, src_arg_num: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    mov_mem32_to_r32(
        asm,
        ASM_X86_REG_ESP,
        arg_offset_from_esp(asm, src_arg_num as usize),
        dest_r32,
    );
}

pub fn mov_local_to_r32(asm: &mut AsmX86, src_local_num: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    mov_mem32_to_r32(
        asm,
        ASM_X86_REG_ESP,
        local_offset_from_esp(asm, src_local_num),
        dest_r32,
    );
}

pub fn mov_r32_to_local(asm: &mut AsmX86, src_r32: i32, dest_local_num: i32) {
    if !ENABLED {
        return;
    }
    mov_r32_to_mem32(
        asm,
        src_r32,
        ASM_X86_REG_ESP,
        local_offset_from_esp(asm, dest_local_num),
    );
}

pub fn mov_local_addr_to_r32(asm: &mut AsmX86, local_num: i32, dest_r32: i32) {
    if !ENABLED {
        return;
    }
    let offset = local_offset_from_esp(asm, local_num);
    if offset == 0 {
        mov_r32_r32(asm, dest_r32, ASM_X86_REG_ESP);
    } else {
        lea_disp_to_r32(asm, ASM_X86_REG_ESP, offset, dest_r32);
    }
}

pub fn mov_reg_pcrel(asm: &mut AsmX86, dest_r32: i32, label: usize) {
    if !ENABLED {
        return;
    }
    write_byte_1(asm, OPCODE_CALL_REL32);
    write_word32(asm, 0);
    let dest = get_label_dest(asm, label);
    let rel = dest as i32 - asm.base.code_offset as i32;
    pop_r32(asm, dest_r32);
    write_byte_2(
        asm,
        OPCODE_ADD_I32_TO_RM32,
        modrm_r32(0) | MODRM_RM_REG | modrm_rm_r32(dest_r32),
    );
    write_word32(asm, rel);
}

pub fn call_ind(asm: &mut AsmX86, fun_id: usize, n_args: usize, temp_r32: i32) {
    if !ENABLED {
        return;
    }
    assert!(n_args <= 4);

    let align = ((n_args + 3) & !3) - n_args;
    if align != 0 {
        sub_r32_i32(asm, ASM_X86_REG_ESP, (align * WORD_SIZE as usize) as i32);
    }

    if n_args > 3 {
        push_r32(asm, ASM_X86_REG_ARG_4);
    }
    if n_args > 2 {
        push_r32(asm, ASM_X86_REG_ARG_3);
    }
    if n_args > 1 {
        push_r32(asm, ASM_X86_REG_ARG_2);
    }
    if n_args > 0 {
        push_r32(asm, ASM_X86_REG_ARG_1);
    }

    mov_mem32_to_r32(
        asm,
        ASM_X86_REG_FUN_TABLE,
        (fun_id * WORD_SIZE as usize) as i32,
        temp_r32,
    );
    write_byte_2(
        asm,
        OPCODE_CALL_RM32,
        modrm_r32(2) | MODRM_RM_REG | modrm_rm_r32(temp_r32),
    );

    if n_args > 0 {
        add_i32_to_r32(
            asm,
            ((n_args + align) * WORD_SIZE as usize) as i32,
            ASM_X86_REG_ESP,
        );
    }
}
