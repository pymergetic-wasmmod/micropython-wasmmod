//! rewrite of py/emitnx86.c
// symmetry: done

#![allow(non_snake_case)]

use crate::asmbase::{self, MpAsmBase};
use crate::asmx86::{
    self, AsmX86, ASM_X86_REG_EAX, ASM_X86_REG_EBP, ASM_X86_REG_EBX, ASM_X86_REG_ECX,
    ASM_X86_REG_EDI, ASM_X86_REG_EDX, ASM_X86_REG_ESI,
};
use crate::emitnative::{self, AsmContext, NativeBackend};
use crate::mpconfig;
use crate::qstr::Qstr;
use crate::runtime0::BinaryOp;

const ASM_X86_CC_JZ: i32 = 0x4;
const ASM_X86_CC_JNZ: i32 = 0x5;
const ASM_X86_CC_JE: i32 = 0x4;

const ENABLED: bool = mpconfig::EMIT_X86;

static MP_F_N_ARGS: [u8; emitnative::mp_f::NUMBER_OF as usize] = {
    let mut t = [0u8; emitnative::mp_f::NUMBER_OF as usize];
    t[emitnative::mp_f::CONVERT_OBJ_TO_NATIVE as usize] = 2;
    t[emitnative::mp_f::CONVERT_NATIVE_TO_OBJ as usize] = 2;
    t[emitnative::mp_f::NATIVE_SWAP_GLOBALS as usize] = 1;
    t[emitnative::mp_f::LOAD_NAME as usize] = 1;
    t[emitnative::mp_f::LOAD_GLOBAL as usize] = 1;
    t[emitnative::mp_f::LOAD_ATTR as usize] = 2;
    t[emitnative::mp_f::LOAD_METHOD as usize] = 3;
    t[emitnative::mp_f::LOAD_SUPER_METHOD as usize] = 2;
    t[emitnative::mp_f::STORE_NAME as usize] = 2;
    t[emitnative::mp_f::STORE_GLOBAL as usize] = 2;
    t[emitnative::mp_f::STORE_ATTR as usize] = 3;
    t[emitnative::mp_f::OBJ_SUBSCR as usize] = 3;
    t[emitnative::mp_f::OBJ_IS_TRUE as usize] = 1;
    t[emitnative::mp_f::UNARY_OP as usize] = 2;
    t[emitnative::mp_f::BINARY_OP as usize] = 3;
    t[emitnative::mp_f::BUILD_TUPLE as usize] = 2;
    t[emitnative::mp_f::BUILD_LIST as usize] = 2;
    t[emitnative::mp_f::BUILD_MAP as usize] = 1;
    t[emitnative::mp_f::BUILD_SET as usize] = 2;
    t[emitnative::mp_f::STORE_SET as usize] = 2;
    t[emitnative::mp_f::LIST_APPEND as usize] = 2;
    t[emitnative::mp_f::STORE_MAP as usize] = 3;
    t[emitnative::mp_f::MAKE_FUNCTION_FROM_PROTO_FUN as usize] = 3;
    t[emitnative::mp_f::NATIVE_CALL_FUNCTION_N_KW as usize] = 3;
    t[emitnative::mp_f::CALL_METHOD_N_KW as usize] = 3;
    t[emitnative::mp_f::CALL_METHOD_N_KW_VAR as usize] = 3;
    t[emitnative::mp_f::NATIVE_GETITER as usize] = 2;
    t[emitnative::mp_f::NATIVE_ITERNEXT as usize] = 1;
    t[emitnative::mp_f::NLR_PUSH as usize] = 1;
    t[emitnative::mp_f::NATIVE_RAISE as usize] = 1;
    t[emitnative::mp_f::IMPORT_NAME as usize] = 3;
    t[emitnative::mp_f::IMPORT_FROM as usize] = 2;
    t[emitnative::mp_f::IMPORT_ALL as usize] = 1;
    t[emitnative::mp_f::NEW_SLICE as usize] = 3;
    t[emitnative::mp_f::UNPACK_SEQUENCE as usize] = 3;
    t[emitnative::mp_f::UNPACK_EX as usize] = 3;
    t[emitnative::mp_f::DELETE_NAME as usize] = 1;
    t[emitnative::mp_f::DELETE_GLOBAL as usize] = 1;
    t[emitnative::mp_f::NEW_CLOSURE as usize] = 3;
    t[emitnative::mp_f::ARG_CHECK_NUM_SIG as usize] = 3;
    t[emitnative::mp_f::SETUP_CODE_STATE as usize] = 4;
    t[emitnative::mp_f::SMALL_INT_FLOOR_DIVIDE as usize] = 2;
    t[emitnative::mp_f::SMALL_INT_MODULO as usize] = 2;
    t[emitnative::mp_f::NATIVE_YIELD_FROM as usize] = 4;
    t[emitnative::mp_f::SETJMP as usize] = 2;
    t[emitnative::mp_f::NATIVE_GEN_FINISH_THROW as usize] = 2;
    t
};

#[derive(Copy, Clone)]
pub struct BackendX86;

impl AsmContext for AsmX86 {
    fn base_mut(&mut self) -> &mut MpAsmBase {
        &mut self.base
    }
}

impl NativeBackend for BackendX86 {
    type Asm = AsmX86;
    const WORD_SIZE: i32 = 4;
    const REG_RET: i32 = ASM_X86_REG_EAX;
    const REG_ARG_1: i32 = ASM_X86_REG_EAX;
    const REG_ARG_2: i32 = ASM_X86_REG_ECX;
    const REG_ARG_3: i32 = ASM_X86_REG_EDX;
    const REG_ARG_4: i32 = ASM_X86_REG_EBX;
    const REG_TEMP0: i32 = ASM_X86_REG_EAX;
    const REG_TEMP1: i32 = ASM_X86_REG_ECX;
    const REG_TEMP2: i32 = ASM_X86_REG_EDX;
    const REG_LOCAL_1: i32 = ASM_X86_REG_EBX;
    const REG_LOCAL_2: i32 = ASM_X86_REG_ESI;
    const REG_LOCAL_3: i32 = ASM_X86_REG_EDI;
    const REG_FUN_TABLE: i32 = ASM_X86_REG_EBP;
    const REG_GENERATOR_STATE: i32 = ASM_X86_REG_ESI;
    const REG_QSTR_TABLE: i32 = ASM_X86_REG_EDI;
    // See the comment on the x64 backend: with `PERSISTENT_CODE_SAVE` this must
    // be `REG_LOCAL_2`, not `REG_LOCAL_3` (`REG_QSTR_TABLE`).
    const REG_LOCAL_LAST: i32 = ASM_X86_REG_ESI;
    const NLR_BUF_IDX_LOCAL_1: usize = 5;
    const N_X86: bool = true;
    const N_X64: bool = false;
    const N_THUMB: bool = false;
    const N_ARM: bool = false;
    const N_XTENSA: bool = false;
    const N_XTENSAWIN: bool = false;
    const N_RV32: bool = false;
    const N_DEBUG: bool = false;
    const N_NLR_SETJMP: bool = false;
    const REG_ZERO: i32 = ASM_X86_REG_EAX;
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

    fn mp_f_n_args(fun: u32) -> u8 {
        MP_F_N_ARGS.get(fun as usize).copied().unwrap_or(0)
    }

    fn new_asm(max_labels: usize) -> Self::Asm {
        let mut asm = AsmX86 {
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
        asmx86::end_pass(as_);
    }
    fn entry(as_: &mut Self::Asm, num_locals: i32, _name: Option<&str>) {
        asmx86::entry(as_, num_locals);
    }
    fn exit(as_: &mut Self::Asm) {
        asmx86::exit(as_);
    }
    fn jump(as_: &mut Self::Asm, label: usize) {
        asmx86::jmp_label(as_, label);
    }
    fn jump_if_reg_zero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool) {
        if bool_test {
            asmx86::test_r8_with_r8(as_, reg, reg);
        } else {
            asmx86::test_r32_with_r32(as_, reg, reg);
        }
        asmx86::jcc_label(as_, ASM_X86_CC_JZ, label);
    }
    fn jump_if_reg_nonzero(as_: &mut Self::Asm, reg: i32, label: usize, bool_test: bool) {
        if bool_test {
            asmx86::test_r8_with_r8(as_, reg, reg);
        } else {
            asmx86::test_r32_with_r32(as_, reg, reg);
        }
        asmx86::jcc_label(as_, ASM_X86_CC_JNZ, label);
    }
    fn jump_if_reg_eq(as_: &mut Self::Asm, reg1: i32, reg2: i32, label: usize) {
        asmx86::cmp_r32_with_r32(as_, reg1, reg2);
        asmx86::jcc_label(as_, ASM_X86_CC_JE, label);
    }
    fn jump_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::jmp_reg(as_, reg);
    }
    fn call_ind(as_: &mut Self::Asm, idx: u32) {
        asmx86::call_ind(
            as_,
            idx as usize,
            Self::mp_f_n_args(idx) as usize,
            ASM_X86_REG_EAX,
        );
    }
    fn mov_local_reg(as_: &mut Self::Asm, local: i32, reg: i32) {
        asmx86::mov_r32_to_local(as_, reg, local);
    }
    fn mov_reg_imm(as_: &mut Self::Asm, reg: i32, imm: usize) {
        asmx86::mov_i32_to_r32(as_, imm as i32, reg);
    }
    fn mov_reg_qstr(as_: &mut Self::Asm, reg: i32, qst: Qstr) {
        asmx86::mov_i32_to_r32(as_, qst as i32, reg);
    }
    fn mov_reg_local(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmx86::mov_local_to_r32(as_, local, reg);
    }
    fn mov_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::mov_r32_r32(as_, dest, src);
    }
    fn mov_reg_local_addr(as_: &mut Self::Asm, reg: i32, local: i32) {
        asmx86::mov_local_addr_to_r32(as_, local, reg);
    }
    fn mov_reg_pcrel(as_: &mut Self::Asm, reg: i32, label: usize) {
        asmx86::mov_reg_pcrel(as_, reg, label);
    }
    fn not_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::not_r32(as_, reg);
    }
    fn neg_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::neg_r32(as_, reg);
    }
    fn lsl_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::shl_r32_cl(as_, reg);
    }
    fn lsr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::shr_r32_cl(as_, reg);
    }
    fn asr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::sar_r32_cl(as_, reg);
    }
    fn lsl_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx86::shl_r32_cl(as_, dest);
    }
    fn lsr_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx86::shr_r32_cl(as_, dest);
    }
    fn asr_reg_reg(as_: &mut Self::Asm, dest: i32, _src: i32) {
        asmx86::sar_r32_cl(as_, dest);
    }
    fn or_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::or_r32_r32(as_, dest, src);
    }
    fn xor_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::xor_r32_r32(as_, dest, src);
    }
    fn and_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::and_r32_r32(as_, dest, src);
    }
    fn add_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::add_r32_r32(as_, dest, src);
    }
    fn sub_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::sub_r32_r32(as_, dest, src);
    }
    fn mul_reg_reg(as_: &mut Self::Asm, dest: i32, src: i32) {
        asmx86::mul_r32_r32(as_, dest, src);
    }
    fn load_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx86::mov_mem32_to_r32(as_, base, off * 4, dest);
    }
    fn load8_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx86::mov_mem8_to_r32zx(as_, base, 0, dest);
    }
    fn load8_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx86::mov_mem8_to_r32zx(as_, base, off, dest);
    }
    fn load16_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx86::mov_mem16_to_r32zx(as_, base, 0, dest);
    }
    fn load16_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx86::mov_mem16_to_r32zx(as_, base, off * 2, dest);
    }
    fn load32_reg_reg(as_: &mut Self::Asm, dest: i32, base: i32) {
        asmx86::mov_mem32_to_r32(as_, base, 0, dest);
    }
    fn load32_reg_reg_offset(as_: &mut Self::Asm, dest: i32, base: i32, off: i32) {
        asmx86::mov_mem32_to_r32(as_, base, off * 4, dest);
    }
    fn store_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx86::mov_r32_to_mem32(as_, src, base, off * 4);
    }
    fn store8_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx86::mov_r8_to_mem8(as_, src, base, 0);
    }
    fn store8_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx86::mov_r8_to_mem8(as_, src, base, off);
    }
    fn store16_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx86::mov_r16_to_mem16(as_, src, base, 0);
    }
    fn store16_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx86::mov_r16_to_mem16(as_, src, base, off * 2);
    }
    fn store32_reg_reg(as_: &mut Self::Asm, src: i32, base: i32) {
        asmx86::mov_r32_to_mem32(as_, src, base, 0);
    }
    fn store32_reg_reg_offset(as_: &mut Self::Asm, src: i32, base: i32, off: i32) {
        asmx86::mov_r32_to_mem32(as_, src, base, off * 4);
    }
    fn clr_reg(as_: &mut Self::Asm, reg: i32) {
        asmx86::xor_r32_r32(as_, reg, reg);
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
        asmx86::xor_r32_r32(as_, dest, dest);
        asmx86::cmp_r32_with_r32(as_, rhs, lhs);
        asmx86::setcc_r8(as_, OPS[op_idx], dest);
    }
    fn binary_op_shift(as_: &mut Self::Asm, op: BinaryOp, dest: i32, _shift_reg: i32) {
        match op {
            BinaryOp::Lshift => asmx86::shl_r32_cl(as_, dest),
            BinaryOp::Rshift => asmx86::shr_r32_cl(as_, dest),
            _ => {}
        }
    }
}

crate::export_emit_native_prefixed!(x86, BackendX86);
