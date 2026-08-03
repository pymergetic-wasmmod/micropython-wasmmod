//! rewrite of py/emitinlinerv32.c
// symmetry: done

#![allow(
    non_snake_case,
    non_camel_case_types,
    clippy::too_many_arguments,
    clippy::identity_op,
    clippy::cast_possible_truncation
)]

use crate::asmbase::{self, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};
use crate::asmrv32::{self, AsmRv32};
use crate::emit::PassKind;
use crate::grammar::Rule;
use crate::lexer::TokenKind;
use crate::malloc;
use crate::misc::{self, mp_clz, mp_ctz};
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objexcept;
use crate::objstr;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};

const ENABLED: bool = mpconfig::EMIT_INLINE_RV32;

#[repr(C)]
pub struct EmitInlineAsm {
    pub as_: AsmRv32,
    pass: u16,
    error_slot: *mut Obj,
    max_num_labels: usize,
    label_lookup: *mut Qstr,
}

fn error_msg(emit: &mut EmitInlineAsm, msg: &'static [u8]) {
    unsafe {
        *emit.error_slot = objexcept::new_exception_args(
            objexcept::type_syntax_error(),
            1,
            &[objstr::new_str(msg)],
        );
    }
}

fn error_exc(emit: &mut EmitInlineAsm, exc: Obj) {
    unsafe {
        *emit.error_slot = exc;
    }
}

pub fn new(max_num_labels: usize) -> *mut EmitInlineAsm {
    if !ENABLED {
        return core::ptr::null_mut();
    }
    let emit = malloc::new_obj::<EmitInlineAsm>().expect("emit inline rv32");
    unsafe {
        (*emit).as_ = core::mem::zeroed();
        asmbase::init(&mut (*emit).as_.base, max_num_labels);
        (*emit).max_num_labels = max_num_labels;
        (*emit).label_lookup = malloc::new::<Qstr>(max_num_labels).expect("labels");
    }
    emit
}

pub fn free(emit: *mut EmitInlineAsm) {
    if emit.is_null() || !ENABLED {
        return;
    }
    unsafe {
        malloc::del((*emit).label_lookup, (*emit).max_num_labels);
        asmbase::deinit(&mut (*emit).as_.base, false);
        malloc::del_obj(emit);
    }
}

pub fn start_pass(emit: *mut EmitInlineAsm, pass: PassKind, error_slot: *mut Obj) {
    if emit.is_null() || !ENABLED {
        return;
    }
    unsafe {
        (*emit).pass = pass as u16;
        (*emit).error_slot = error_slot;
        if (*emit).pass == PassKind::CodeSize as u16 {
            // `write_bytes::<Qstr>` takes an *element* count, not a byte count.
            core::ptr::write_bytes((*emit).label_lookup, 0, (*emit).max_num_labels);
        }
        let asm_pass = if pass == PassKind::Emit {
            MP_ASM_PASS_EMIT
        } else {
            MP_ASM_PASS_COMPUTE
        };
        asmbase::start_pass(&mut (*emit).as_.base, asm_pass as i32);
    }
}

pub fn end_pass(emit: *mut EmitInlineAsm, _type_sig: usize) {
    if emit.is_null() || !ENABLED {
        return;
    }
    unsafe {
        asmrv32::opcode_cjr(&mut (*emit).as_, asmrv32::ASM_RV32_REG_RA);
        asmrv32::end_pass(&mut (*emit).as_);
    }
}

const REGISTERS_QSTR_TABLE: [&[u8]; 64] = [
    b"zero", b"ra", b"sp", b"gp", b"tp", b"t0", b"t1", b"t2", b"s0", b"s1", b"a0", b"a1", b"a2",
    b"a3", b"a4", b"a5", b"a6", b"a7", b"s2", b"s3", b"s4", b"s5", b"s6", b"s7", b"s8", b"s9",
    b"s10", b"s11", b"t3", b"t4", b"t5", b"t6", b"x0", b"x1", b"x2", b"x3", b"x4", b"x5", b"x6",
    b"x7", b"x8", b"x9", b"x10", b"x11", b"x12", b"x13", b"x14", b"x15", b"x16", b"x17", b"x18",
    b"x19", b"x20", b"x21", b"x22", b"x23", b"x24", b"x25", b"x26", b"x27", b"x28", b"x29", b"x30",
    b"x31",
];

fn parse_register_node(node: ParseNode, register_number: &mut u32, compressed: bool) -> bool {
    if !parse::parse_node_is_id(node) {
        return false;
    }
    let node_qstr = parse::parse_node_leaf_arg(node) as Qstr;
    if let Some((data, _)) = qstr::qstr_data(node_qstr) {
        let name = if data.last() == Some(&0) {
            &data[..data.len() - 1]
        } else {
            data.as_slice()
        };
        for (index, &reg_name) in REGISTERS_QSTR_TABLE.iter().enumerate() {
            if name == reg_name {
                let number = (index as u32) % asmrv32::RV32_AVAILABLE_REGISTERS_COUNT;
                if !compressed || asmrv32::rv32_is_in_c_register_window(number) {
                    *register_number = if compressed {
                        asmrv32::rv32_map_in_c_register_window(number)
                    } else {
                        number
                    };
                    return true;
                }
                break;
            }
        }
    }
    false
}

fn lookup_label(emit: &EmitInlineAsm, node: ParseNode, qstring: &mut Qstr) -> usize {
    *qstring = parse::parse_node_leaf_arg(node) as Qstr;
    unsafe {
        for label in 0..emit.max_num_labels {
            if *emit.label_lookup.add(label) == *qstring {
                return label;
            }
        }
    }
    emit.max_num_labels
}

fn label_code_offset(emit: &EmitInlineAsm, label_index: usize) -> isize {
    unsafe {
        (*emit.as_.base.label_offsets.add(label_index)) as isize
            - emit.as_.base.code_offset as isize
    }
}

pub fn count_params(
    emit: *mut EmitInlineAsm,
    parameters_count: usize,
    parameter_nodes: *mut ParseNode,
) -> usize {
    if emit.is_null() || !ENABLED {
        return 0;
    }
    if parameters_count > 4 {
        error_msg(
            unsafe { &mut *emit },
            b"can only have up to 4 parameters for RV32 assembly",
        );
        return 0;
    }
    for index in 0..parameters_count {
        let mut register_index = 0u32;
        let valid = parse_register_node(
            unsafe { *parameter_nodes.add(index) },
            &mut register_index,
            false,
        );
        if !valid || register_index != asmrv32::ASM_RV32_REG_A0 + index as u32 {
            error_msg(
                unsafe { &mut *emit },
                b"parameters must be registers in sequence a0 to a3",
            );
            return 0;
        }
    }
    parameters_count
}

pub fn label(emit: *mut EmitInlineAsm, label_num: usize, label_id: Qstr) -> bool {
    if emit.is_null() || !ENABLED {
        return false;
    }
    unsafe {
        debug_assert!(label_num < (*emit).max_num_labels);
        if (*emit).pass == PassKind::CodeSize as u16 {
            for index in 0..(*emit).max_num_labels {
                if *(*emit).label_lookup.add(index) == label_id {
                    return false;
                }
            }
        }
        *(*emit).label_lookup.add(label_num) = label_id;
        asmbase::label_assign(&mut (*emit).as_.base, label_num);
    }
    true
}

const N: u8 = 0;
const R: u8 = 1;
const I: u8 = 2;
const L: u8 = 3;
const C: u8 = 1 << 2;
const U: u8 = 1 << 2;
const Z: u8 = 1 << 3;
const RC: u8 = R | C;
const IU: u8 = I | U;
const IZ: u8 = I | Z;
const IUZ: u8 = I | U | Z;

#[derive(Copy, Clone, PartialEq, Eq)]
enum CallConvention {
    Rrr,
    Rr,
    Rri,
    Rrl,
    Ri,
    L,
    R,
    Rl,
    N,
    Rii,
    Rir,
}

#[derive(Copy, Clone)]
enum MaskIndex {
    NotUsed,
    Ffffffff,
    M00000fff,
    Mfffff000,
    M00001ffe,
    M0000001f,
    Mfffffffe,
    M0000003f,
    M0000ff00,
    M000003fc,
    M000001fe,
    M00000ffe,
    Mfffffffa,
    M0001f800,
    M0000007c,
    M000000fc,
    M001ffffe,
}

const OPCODE_MASKS: [u32; 17] = [
    0,
    0xffff_ffff,
    0x0000_0fff,
    0xffff_f000,
    0x0000_1ffe,
    0x0000_001f,
    0xffff_fffe,
    0x0000_003f,
    0x0000_ff00,
    0x0000_03fc,
    0x0000_01fe,
    0x0000_0ffe,
    0xffff_fffa,
    0x0001_f800,
    0x0000_007c,
    0x0000_00fc,
    0x001f_fffe,
];

#[derive(Copy, Clone)]
enum EmitterId {
    asm_rv32_opcode_add,
    asm_rv32_opcode_addi,
    asm_rv32_opcode_and,
    asm_rv32_opcode_andi,
    asm_rv32_opcode_auipc,
    asm_rv32_opcode_beq,
    asm_rv32_opcode_bge,
    asm_rv32_opcode_bgeu,
    asm_rv32_opcode_blt,
    asm_rv32_opcode_bltu,
    asm_rv32_opcode_bne,
    asm_rv32_opcode_cadd,
    asm_rv32_opcode_caddi,
    asm_rv32_opcode_caddi4spn,
    asm_rv32_opcode_cand,
    asm_rv32_opcode_candi,
    asm_rv32_opcode_cbeqz,
    asm_rv32_opcode_cbnez,
    asm_rv32_opcode_cebreak,
    asm_rv32_opcode_cj,
    asm_rv32_opcode_cjal,
    asm_rv32_opcode_cjalr,
    asm_rv32_opcode_cjr,
    asm_rv32_opcode_cli,
    asm_rv32_opcode_clui,
    asm_rv32_opcode_clw,
    asm_rv32_opcode_clwsp,
    asm_rv32_opcode_cmv,
    asm_rv32_opcode_cnop,
    asm_rv32_opcode_cor,
    asm_rv32_opcode_cslli,
    asm_rv32_opcode_csrai,
    asm_rv32_opcode_csrli,
    asm_rv32_opcode_csrrc,
    asm_rv32_opcode_csrrci,
    asm_rv32_opcode_csrrs,
    asm_rv32_opcode_csrrsi,
    asm_rv32_opcode_csrrw,
    asm_rv32_opcode_csrrwi,
    asm_rv32_opcode_csub,
    asm_rv32_opcode_csw,
    asm_rv32_opcode_cswsp,
    asm_rv32_opcode_cxor,
    asm_rv32_opcode_div,
    asm_rv32_opcode_divu,
    asm_rv32_opcode_ebreak,
    asm_rv32_opcode_ecall,
    asm_rv32_opcode_jal,
    asm_rv32_opcode_jalr,
    asm_rv32_opcode_lb,
    asm_rv32_opcode_lbu,
    asm_rv32_opcode_lh,
    asm_rv32_opcode_lhu,
    asm_rv32_opcode_lui,
    asm_rv32_opcode_lw,
    asm_rv32_opcode_mul,
    asm_rv32_opcode_mulh,
    asm_rv32_opcode_mulhsu,
    asm_rv32_opcode_mulhu,
    asm_rv32_opcode_or,
    asm_rv32_opcode_ori,
    asm_rv32_opcode_rem,
    asm_rv32_opcode_remu,
    asm_rv32_opcode_sb,
    asm_rv32_opcode_sh,
    asm_rv32_opcode_sh1add,
    asm_rv32_opcode_sh2add,
    asm_rv32_opcode_sh3add,
    asm_rv32_opcode_sll,
    asm_rv32_opcode_slli,
    asm_rv32_opcode_slt,
    asm_rv32_opcode_slti,
    asm_rv32_opcode_sltiu,
    asm_rv32_opcode_sltu,
    asm_rv32_opcode_sra,
    asm_rv32_opcode_srai,
    asm_rv32_opcode_srl,
    asm_rv32_opcode_srli,
    asm_rv32_opcode_sub,
    asm_rv32_opcode_sw,
    asm_rv32_opcode_xor,
    asm_rv32_opcode_xori,
    opcode_la,
    opcode_li,
}

struct Opcode {
    name: &'static [u8],
    arg1_mask: MaskIndex,
    arg2_mask: MaskIndex,
    arg3_mask: MaskIndex,
    parse_nodes: u8,
    calling_convention: CallConvention,
    arg1_kind: u8,
    arg1_shift: u8,
    arg2_kind: u8,
    arg2_shift: u8,
    arg3_kind: u8,
    arg3_shift: u8,
    required_extensions: u8,
    emitter: EmitterId,
}

const OPCODES: &[Opcode] = &[
    Opcode {
        name: b"add",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_add,
    },
    Opcode {
        name: b"addi",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_addi,
    },
    Opcode {
        name: b"and_",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_and,
    },
    Opcode {
        name: b"andi",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_andi,
    },
    Opcode {
        name: b"auipc",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Mfffff000,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 12,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_auipc,
    },
    Opcode {
        name: b"beq",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_beq,
    },
    Opcode {
        name: b"bge",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_bge,
    },
    Opcode {
        name: b"bgeu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_bgeu,
    },
    Opcode {
        name: b"blt",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_blt,
    },
    Opcode {
        name: b"bltu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_bltu,
    },
    Opcode {
        name: b"bne",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00001ffe,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: L,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_bne,
    },
    Opcode {
        name: b"csrrc",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrc,
    },
    Opcode {
        name: b"csrrs",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrs,
    },
    Opcode {
        name: b"csrrw",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrw,
    },
    Opcode {
        name: b"csrrci",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rii,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrci,
    },
    Opcode {
        name: b"csrrsi",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rii,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrsi,
    },
    Opcode {
        name: b"csrrwi",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rii,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrrwi,
    },
    Opcode {
        name: b"c_add",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::Mfffffffe,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cadd,
    },
    Opcode {
        name: b"c_addi",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::M0000003f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IZ,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_caddi,
    },
    Opcode {
        name: b"c_addi4spn",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M000003fc,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IUZ,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_caddi4spn,
    },
    Opcode {
        name: b"c_and",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000ff00,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: RC,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cand,
    },
    Opcode {
        name: b"c_andi",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000003f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_candi,
    },
    Opcode {
        name: b"c_beqz",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M000001fe,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rl,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: L,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cbeqz,
    },
    Opcode {
        name: b"c_bnez",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M000001fe,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rl,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: L,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cbnez,
    },
    Opcode {
        name: b"c_ebreak",
        arg1_mask: MaskIndex::NotUsed,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 0,
        calling_convention: CallConvention::N,
        arg1_kind: N,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cebreak,
    },
    Opcode {
        name: b"c_j",
        arg1_mask: MaskIndex::M00000ffe,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 1,
        calling_convention: CallConvention::L,
        arg1_kind: L,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cj,
    },
    Opcode {
        name: b"c_jal",
        arg1_mask: MaskIndex::M00000ffe,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 1,
        calling_convention: CallConvention::L,
        arg1_kind: L,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cjal,
    },
    Opcode {
        name: b"c_jalr",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 1,
        calling_convention: CallConvention::R,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cjalr,
    },
    Opcode {
        name: b"c_jr",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 1,
        calling_convention: CallConvention::R,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cjr,
    },
    Opcode {
        name: b"c_li",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::M0000003f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cli,
    },
    Opcode {
        name: b"c_lui",
        arg1_mask: MaskIndex::Mfffffffa,
        arg2_mask: MaskIndex::M0001f800,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IUZ,
        arg2_shift: 12,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_clui,
    },
    Opcode {
        name: b"c_lw",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000007c,
        arg3_mask: MaskIndex::M0000ff00,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: RC,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_clw,
    },
    Opcode {
        name: b"c_lwsp",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::M000000fc,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_clwsp,
    },
    Opcode {
        name: b"c_mv",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::Mfffffffe,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cmv,
    },
    Opcode {
        name: b"c_nop",
        arg1_mask: MaskIndex::NotUsed,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 0,
        calling_convention: CallConvention::N,
        arg1_kind: N,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cnop,
    },
    Opcode {
        name: b"c_or",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000ff00,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: RC,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cor,
    },
    Opcode {
        name: b"c_slli",
        arg1_mask: MaskIndex::Mfffffffe,
        arg2_mask: MaskIndex::M0000001f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cslli,
    },
    Opcode {
        name: b"c_srai",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000001f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrai,
    },
    Opcode {
        name: b"c_srli",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000001f,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: IU,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csrli,
    },
    Opcode {
        name: b"c_sub",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000ff00,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: RC,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csub,
    },
    Opcode {
        name: b"c_sw",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000007c,
        arg3_mask: MaskIndex::M0000ff00,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: RC,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_csw,
    },
    Opcode {
        name: b"c_swsp",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M000000fc,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cswsp,
    },
    Opcode {
        name: b"c_xor",
        arg1_mask: MaskIndex::M0000ff00,
        arg2_mask: MaskIndex::M0000ff00,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: RC,
        arg1_shift: 0,
        arg2_kind: RC,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cxor,
    },
    Opcode {
        name: b"div",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_div,
    },
    Opcode {
        name: b"divu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_divu,
    },
    Opcode {
        name: b"ebreak",
        arg1_mask: MaskIndex::NotUsed,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 0,
        calling_convention: CallConvention::N,
        arg1_kind: N,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_ebreak,
    },
    Opcode {
        name: b"ecall",
        arg1_mask: MaskIndex::NotUsed,
        arg2_mask: MaskIndex::NotUsed,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 0,
        calling_convention: CallConvention::N,
        arg1_kind: N,
        arg1_shift: 0,
        arg2_kind: N,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_ecall,
    },
    Opcode {
        name: b"jal",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M001ffffe,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: L,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_jal,
    },
    Opcode {
        name: b"jalr",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_jalr,
    },
    Opcode {
        name: b"la",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rl,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: L,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::opcode_la,
    },
    Opcode {
        name: b"lb",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lb,
    },
    Opcode {
        name: b"lbu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lbu,
    },
    Opcode {
        name: b"lh",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lh,
    },
    Opcode {
        name: b"lhu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lhu,
    },
    Opcode {
        name: b"li",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::opcode_li,
    },
    Opcode {
        name: b"lui",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Mfffff000,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Ri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 12,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lui,
    },
    Opcode {
        name: b"lw",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_lw,
    },
    Opcode {
        name: b"mv",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::NotUsed,
        parse_nodes: 2,
        calling_convention: CallConvention::Rr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: N,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_cmv,
    },
    Opcode {
        name: b"mul",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_mul,
    },
    Opcode {
        name: b"mulh",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_mulh,
    },
    Opcode {
        name: b"mulhsu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_mulhsu,
    },
    Opcode {
        name: b"mulhu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_mulhu,
    },
    Opcode {
        name: b"or_",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_or,
    },
    Opcode {
        name: b"ori",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_ori,
    },
    Opcode {
        name: b"rem",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_rem,
    },
    Opcode {
        name: b"remu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_remu,
    },
    Opcode {
        name: b"sb",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sb,
    },
    Opcode {
        name: b"sh",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sh,
    },
    Opcode {
        name: b"sh1add",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_ZBA,
        emitter: EmitterId::asm_rv32_opcode_sh1add,
    },
    Opcode {
        name: b"sh2add",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_ZBA,
        emitter: EmitterId::asm_rv32_opcode_sh2add,
    },
    Opcode {
        name: b"sh3add",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_ZBA,
        emitter: EmitterId::asm_rv32_opcode_sh3add,
    },
    Opcode {
        name: b"sll",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sll,
    },
    Opcode {
        name: b"slli",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_slli,
    },
    Opcode {
        name: b"slt",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_slt,
    },
    Opcode {
        name: b"slti",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_slti,
    },
    Opcode {
        name: b"sltiu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sltiu,
    },
    Opcode {
        name: b"sltu",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sltu,
    },
    Opcode {
        name: b"sra",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sra,
    },
    Opcode {
        name: b"srai",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_srai,
    },
    Opcode {
        name: b"srl",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_srl,
    },
    Opcode {
        name: b"srli",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M0000001f,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: IU,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_srli,
    },
    Opcode {
        name: b"sub",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sub,
    },
    Opcode {
        name: b"sw",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::M00000fff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 2,
        calling_convention: CallConvention::Rir,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: I,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_sw,
    },
    Opcode {
        name: b"xor",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::Ffffffff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rrr,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: R,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_xor,
    },
    Opcode {
        name: b"xori",
        arg1_mask: MaskIndex::Ffffffff,
        arg2_mask: MaskIndex::Ffffffff,
        arg3_mask: MaskIndex::M00000fff,
        parse_nodes: 3,
        calling_convention: CallConvention::Rri,
        arg1_kind: R,
        arg1_shift: 0,
        arg2_kind: R,
        arg2_shift: 0,
        arg3_kind: I,
        arg3_shift: 0,
        required_extensions: asmrv32::RV32_EXT_NONE,
        emitter: EmitterId::asm_rv32_opcode_xori,
    },
];

fn mask_value(index: MaskIndex) -> u32 {
    OPCODE_MASKS[index as usize]
}

fn is_in_signed_mask(mask: u32, value: u32) -> bool {
    let leading_zeroes = mp_clz(mask);
    if leading_zeroes == 0 {
        return true;
    }
    let positive_mask = !(mask & !(1u32 << (31 - leading_zeroes)));
    if (value & positive_mask) == 0 {
        return true;
    }
    let mut negative_mask = !(mask >> 1);
    let trailing_zeroes = mp_ctz(mask);
    if trailing_zeroes > 0 {
        let trailing_mask = (1u32 << trailing_zeroes) - 1;
        if (value & trailing_mask) != 0 {
            return false;
        }
        negative_mask &= !trailing_mask;
    }
    (value & negative_mask) == negative_mask
}

fn is_in_unsigned_mask(mask: u32, value: u32) -> bool {
    (value & !mask) == 0
}

fn validate_integer(value: u32, mask: u32, flags: u8) -> bool {
    if flags & U != 0 {
        if !is_in_unsigned_mask(mask, value) {
            return false;
        }
    } else if !is_in_signed_mask(mask, value) {
        return false;
    }
    !(flags & Z != 0 && value == 0)
}

fn qstr_eq_name(q: Qstr, name: &[u8]) -> bool {
    qstr::qstr_data(q)
        .map(|(d, _)| {
            let s = if d.last() == Some(&0) {
                &d[..d.len() - 1]
            } else {
                d.as_slice()
            };
            s == name
        })
        .unwrap_or(false)
}

fn syntax_err(msg: &'static [u8]) -> Obj {
    objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(msg)])
}

fn serialise_argument(
    emit: &mut EmitInlineAsm,
    opcode: &Opcode,
    node: ParseNode,
    node_index: usize,
    serialised: &mut u32,
) -> bool {
    let (kind, shift, mask) = match node_index {
        0 => (
            opcode.arg1_kind,
            opcode.arg1_shift,
            mask_value(opcode.arg1_mask),
        ),
        1 => (
            opcode.arg2_kind,
            opcode.arg2_shift,
            mask_value(opcode.arg2_mask),
        ),
        _ => (
            opcode.arg3_kind,
            opcode.arg3_shift,
            mask_value(opcode.arg3_mask),
        ),
    };
    match kind & 0x03 {
        N => {}
        R => {
            let mut register_index = 0u32;
            if !parse_register_node(node, &mut register_index, false) {
                error_exc(emit, syntax_err(b"expects register"));
                return false;
            }
            if (mask & (1u32 << register_index)) == 0 {
                error_exc(emit, syntax_err(b"unknown register"));
                return false;
            }
            *serialised = if kind & C != 0 {
                asmrv32::rv32_map_in_c_register_window(register_index)
            } else {
                register_index
            };
        }
        I => {
            let mut object = obj::OBJ_NULL;
            if !parse::parse_node_get_int_maybe(node, &mut object) {
                error_exc(emit, syntax_err(b"expects integer"));
                return false;
            }
            let immediate = (obj::get_int_truncated(object) as u32) << shift;
            if !validate_integer(immediate, mask, kind) {
                error_exc(emit, syntax_err(b"out of range"));
                return false;
            }
            *serialised = immediate;
        }
        L => {
            if !parse::parse_node_is_id(node) {
                error_exc(emit, syntax_err(b"expects label"));
                return false;
            }
            let mut qstring = qstr::QSTR_NULL;
            let label_index = lookup_label(emit, node, &mut qstring);
            if label_index >= emit.max_num_labels && emit.pass == PassKind::Emit as u16 {
                error_exc(emit, syntax_err(b"undefined label"));
                return false;
            }
            let displacement = label_code_offset(emit, label_index) as u32;
            if !validate_integer(displacement, mask, kind) {
                error_exc(emit, syntax_err(b"out of range"));
                return false;
            }
            *serialised = displacement;
        }
        _ => {}
    }
    true
}

fn serialise_register_offset_node(
    emit: &mut EmitInlineAsm,
    opcode_data: &Opcode,
    node: ParseNode,
    _node_index: usize,
    offset: &mut u32,
    base: &mut u32,
) -> bool {
    if !parse::parse_node_is_struct_kind(node, Rule::AtomExprNormal)
        && !parse::parse_node_is_struct_kind(node, Rule::Factor2)
    {
        error_exc(emit, syntax_err(b"expects offset"));
        return false;
    }
    let mut work = node;
    let mut negative = false;
    if parse::parse_node_is_struct_kind(node, Rule::Factor2) {
        let pns = node as *const ParseNodeStruct;
        if parse::parse_node_is_token_kind(
            parse::parse_node_struct_node(pns, 0),
            TokenKind::OpMinus,
        ) {
            negative = true;
        } else if !parse::parse_node_is_token_kind(
            parse::parse_node_struct_node(pns, 0),
            TokenKind::OpPlus,
        ) {
            error_exc(emit, syntax_err(b"expects offset"));
            return false;
        }
        work = parse::parse_node_struct_node(pns, 1);
        if !parse::parse_node_is_struct_kind(work, Rule::AtomExprNormal) {
            error_exc(emit, syntax_err(b"expects offset"));
            return false;
        }
    }
    let pns = work as *const ParseNodeStruct;
    if negative {
        let mut object = obj::OBJ_NULL;
        if !parse::parse_node_get_int_maybe(parse::parse_node_struct_node(pns, 0), &mut object) {
            error_exc(emit, syntax_err(b"expects integer"));
            return false;
        }
        let mut value = obj::get_int_truncated(object) as u32;
        value = value.wrapping_neg();
        if !validate_integer(
            value << opcode_data.arg2_shift,
            mask_value(opcode_data.arg2_mask),
            opcode_data.arg2_kind,
        ) {
            error_exc(emit, syntax_err(b"out of range"));
            return false;
        }
        *offset = value;
    } else if !serialise_argument(
        emit,
        opcode_data,
        parse::parse_node_struct_node(pns, 0),
        1,
        offset,
    ) {
        return false;
    }
    let tail = parse::parse_node_struct_node(pns, 1) as *const ParseNodeStruct;
    serialise_argument(
        emit,
        opcode_data,
        parse::parse_node_struct_node(tail, 0),
        2,
        base,
    )
}

fn opcode_la(state: &mut AsmRv32, rd: u32, displacement: i32) {
    let upper = (displacement as u32) & 0xffff_f000;
    let mut lower = (displacement as u32) & 0x0000_0fff;
    if (lower & 0x800) != 0 {
        lower = lower.wrapping_add(0x1000);
    }
    asmrv32::opcode_auipc(state, rd, upper as i32);
    asmrv32::opcode_addi(state, rd, rd, lower as i32);
}

fn handle_opcode(state: &mut AsmRv32, emitter: EmitterId, cc: CallConvention, args: [u32; 3]) {
    match emitter {
        EmitterId::asm_rv32_opcode_add => {
            asmrv32::opcode_add(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_addi => {
            asmrv32::opcode_addi(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_and => {
            asmrv32::opcode_and(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_andi => {
            asmrv32::opcode_andi(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_auipc => {
            asmrv32::opcode_auipc(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_beq => {
            asmrv32::opcode_beq(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_bge => {
            asmrv32::opcode_bge(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_bgeu => {
            asmrv32::opcode_bgeu(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_blt => {
            asmrv32::opcode_blt(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_bltu => {
            asmrv32::opcode_bltu(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_bne => {
            asmrv32::opcode_bne(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_cadd => {
            asmrv32::opcode_cadd(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_caddi => {
            asmrv32::opcode_caddi(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_caddi4spn => {
            asmrv32::opcode_caddi4spn(state, args[0] as u32, args[1]);
        }
        EmitterId::asm_rv32_opcode_cand => {
            asmrv32::opcode_cand(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_candi => {
            asmrv32::opcode_candi(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_cbeqz => {
            asmrv32::opcode_cbeqz(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_cbnez => {
            asmrv32::opcode_cbnez(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_cebreak => {
            asmrv32::opcode_cebreak(state);
        }
        EmitterId::asm_rv32_opcode_cj => {
            asmrv32::opcode_cj(state, args[0] as i32);
        }
        EmitterId::asm_rv32_opcode_cjal => {
            asmrv32::opcode_cjal(state, args[0] as i32);
        }
        EmitterId::asm_rv32_opcode_cjalr => {
            asmrv32::opcode_cjalr(state, args[0] as u32);
        }
        EmitterId::asm_rv32_opcode_cjr => {
            asmrv32::opcode_cjr(state, args[0] as u32);
        }
        EmitterId::asm_rv32_opcode_cli => {
            asmrv32::opcode_cli(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_clui => {
            asmrv32::opcode_clui(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_clw => {
            asmrv32::opcode_clw(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_clwsp => {
            asmrv32::opcode_clwsp(state, args[0] as u32, args[1]);
        }
        EmitterId::asm_rv32_opcode_cmv => {
            asmrv32::opcode_cmv(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_cnop => {
            asmrv32::opcode_cnop(state);
        }
        EmitterId::asm_rv32_opcode_cor => {
            asmrv32::opcode_cor(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_cslli => {
            asmrv32::opcode_cslli(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_csrai => {
            asmrv32::opcode_csrai(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_csrli => {
            asmrv32::opcode_csrli(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrc => {
            asmrv32::opcode_csrrc(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrci => {
            asmrv32::opcode_csrrci(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrs => {
            asmrv32::opcode_csrrs(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrsi => {
            asmrv32::opcode_csrrsi(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrw => {
            asmrv32::opcode_csrrw(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csrrwi => {
            asmrv32::opcode_csrrwi(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_csub => {
            asmrv32::opcode_csub(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_csw => {
            asmrv32::opcode_csw(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_cswsp => {
            asmrv32::opcode_cswsp(state, args[0] as u32, args[1]);
        }
        EmitterId::asm_rv32_opcode_cxor => {
            asmrv32::opcode_cxor(state, args[0] as u32, args[1] as u32);
        }
        EmitterId::asm_rv32_opcode_div => {
            asmrv32::opcode_div(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_divu => {
            asmrv32::opcode_divu(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_ebreak => {
            asmrv32::opcode_ebreak(state);
        }
        EmitterId::asm_rv32_opcode_ecall => {
            asmrv32::opcode_ecall(state);
        }
        EmitterId::asm_rv32_opcode_jal => {
            asmrv32::opcode_jal(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_jalr => {
            asmrv32::opcode_jalr(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_lb => {
            asmrv32::opcode_lb(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_lbu => {
            asmrv32::opcode_lbu(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_lh => {
            asmrv32::opcode_lh(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_lhu => {
            asmrv32::opcode_lhu(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_lui => {
            asmrv32::opcode_lui(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_lw => {
            asmrv32::opcode_lw(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_mul => {
            asmrv32::opcode_mul(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_mulh => {
            asmrv32::opcode_mulh(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_mulhsu => {
            asmrv32::opcode_mulhsu(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_mulhu => {
            asmrv32::opcode_mulhu(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_or => {
            asmrv32::opcode_or(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_ori => {
            asmrv32::opcode_ori(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_rem => {
            asmrv32::opcode_rem(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_remu => {
            asmrv32::opcode_remu(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sb => {
            asmrv32::opcode_sb(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_sh => {
            asmrv32::opcode_sh(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_sh1add => {
            asmrv32::opcode_sh1add(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sh2add => {
            asmrv32::opcode_sh2add(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sh3add => {
            asmrv32::opcode_sh3add(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sll => {
            asmrv32::opcode_sll(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_slli => {
            asmrv32::opcode_slli(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_slt => {
            asmrv32::opcode_slt(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_slti => {
            asmrv32::opcode_slti(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_sltiu => {
            asmrv32::opcode_sltiu(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_sltu => {
            asmrv32::opcode_sltu(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sra => {
            asmrv32::opcode_sra(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_srai => {
            asmrv32::opcode_srai(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_srl => {
            asmrv32::opcode_srl(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_srli => {
            asmrv32::opcode_srli(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::asm_rv32_opcode_sub => {
            asmrv32::opcode_sub(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_sw => {
            asmrv32::opcode_sw(state, args[0] as u32, args[2] as u32, args[1] as i32);
        }
        EmitterId::asm_rv32_opcode_xor => {
            asmrv32::opcode_xor(state, args[0] as u32, args[1] as u32, args[2] as u32);
        }
        EmitterId::asm_rv32_opcode_xori => {
            asmrv32::opcode_xori(state, args[0] as u32, args[1] as u32, args[2] as i32);
        }
        EmitterId::opcode_la => {
            opcode_la(state, args[0] as u32, args[1] as i32);
        }
        EmitterId::opcode_li => {
            asmrv32::emit_optimised_load_immediate(state, args[0] as u32, args[1] as i32);
        }
    }
}

fn extract_register_list(
    _emit: &mut EmitInlineAsm,
    _opcode: Qstr,
    node: ParseNode,
    reglist: &mut u32,
) -> bool {
    const REG_S10: u32 = 26;
    const REG_S11: u32 = 27;
    if !parse::parse_node_is_struct_kind(node, Rule::AtomBrace) {
        return false;
    }
    let pns = node as *const ParseNodeStruct;
    if parse::parse_node_struct_num_nodes(pns) != 1 {
        return false;
    }
    let inner = parse::parse_node_struct_node(pns, 0);
    let mut register_id = 0u32;
    if parse::parse_node_is_id(inner) {
        if !parse_register_node(inner, &mut register_id, false) {
            return false;
        }
        *reglist = 4;
        return register_id == asmrv32::ASM_RV32_REG_RA;
    }
    if !parse::parse_node_is_struct_kind(inner, Rule::Dictorsetmaker) {
        return false;
    }
    let pns = inner as *const ParseNodeStruct;
    if parse::parse_node_struct_num_nodes(pns) != 2
        || !parse::parse_node_is_id(parse::parse_node_struct_node(pns, 0))
        || !parse::parse_node_is_struct_kind(
            parse::parse_node_struct_node(pns, 1),
            Rule::DictorsetmakerList,
        )
        || !parse_register_node(
            parse::parse_node_struct_node(pns, 0),
            &mut register_id,
            false,
        )
        || register_id != asmrv32::ASM_RV32_REG_RA
    {
        return false;
    }
    let list_pns = parse::parse_node_struct_node(pns, 1) as *const ParseNodeStruct;
    let mut list_nodes_ptr: *mut ParseNode = core::ptr::null_mut();
    let mut list_pn = parse::parse_node_struct_node(list_pns, 0);
    let list_nodes_count = parse::parse_node_extract_list(
        &mut list_pn,
        Rule::DictorsetmakerList2,
        &mut list_nodes_ptr,
    );
    if list_nodes_count != 1
        || !parse::parse_node_is_struct_kind(unsafe { *list_nodes_ptr }, Rule::DictorsetmakerList)
    {
        return false;
    }
    let pns = unsafe { *list_nodes_ptr } as *const ParseNodeStruct;
    if parse::parse_node_struct_num_nodes(pns) != 1 {
        return false;
    }
    let item = parse::parse_node_struct_node(pns, 0);
    if parse::parse_node_is_id(item) {
        if !parse_register_node(item, &mut register_id, false)
            || register_id != asmrv32::ASM_RV32_REG_S0
        {
            return false;
        }
        *reglist = 5;
        return true;
    }
    if parse::parse_node_is_struct_kind(item, Rule::ArithExpr) {
        let pns = item as *const ParseNodeStruct;
        if parse::parse_node_struct_num_nodes(pns) != 3
            || !parse::parse_node_is_id(parse::parse_node_struct_node(pns, 0))
            || !parse::parse_node_is_token_kind(
                parse::parse_node_struct_node(pns, 1),
                TokenKind::OpMinus,
            )
            || !parse::parse_node_is_id(parse::parse_node_struct_node(pns, 2))
        {
            return false;
        }
        if !parse_register_node(
            parse::parse_node_struct_node(pns, 0),
            &mut register_id,
            false,
        ) || register_id != asmrv32::ASM_RV32_REG_S0
        {
            return false;
        }
        if !parse_register_node(
            parse::parse_node_struct_node(pns, 2),
            &mut register_id,
            false,
        ) || register_id == REG_S10
        {
            return false;
        }
        if register_id == asmrv32::ASM_RV32_REG_S1 {
            *reglist = 6;
            return true;
        }
        if register_id >= asmrv32::ASM_RV32_REG_S2 && register_id <= REG_S11 {
            *reglist = 7 + core::cmp::min(register_id, REG_S10) - asmrv32::ASM_RV32_REG_S2;
            return true;
        }
    }
    false
}

const ZCMP_OPCODE_NAMES: [&[u8]; 6] = [
    b"cm_push",
    b"cm_pop",
    b"cm_popret",
    b"cm_popretz",
    b"cm_mva01s",
    b"cm_mvsa01",
];

fn handle_zcmp_opcode(
    emit: &mut EmitInlineAsm,
    opcode: Qstr,
    argument_nodes: *mut ParseNode,
) -> bool {
    for (index, &name) in ZCMP_OPCODE_NAMES.iter().enumerate() {
        if !qstr_eq_name(opcode, name) {
            continue;
        }
        if qstr_eq_name(opcode, b"cm_mva01s") || qstr_eq_name(opcode, b"cm_mvsa01") {
            let mut register_lhs = 0u32;
            let mut register_rhs = 0u32;
            if !parse_register_node(unsafe { *argument_nodes }, &mut register_lhs, false)
                || (1u32 << register_lhs) & 0x00fc_0300 == 0
            {
                error_exc(emit, syntax_err(b"wrong register(s)"));
                return false;
            }
            if !parse_register_node(unsafe { *argument_nodes.add(1) }, &mut register_rhs, false)
                || (1u32 << register_rhs) & 0x00fc_0300 == 0
            {
                error_exc(emit, syntax_err(b"wrong register(s)"));
                return false;
            }
            if register_lhs == register_rhs {
                error_exc(emit, syntax_err(b"registers must be different"));
                return false;
            }
            if index == 4 {
                asmrv32::opcode_cmmva01s(&mut emit.as_, register_lhs, register_rhs);
            } else {
                asmrv32::opcode_cmmvsa01(&mut emit.as_, register_lhs, register_rhs);
            }
            return true;
        }
        let mut register_list = 0u32;
        if !extract_register_list(emit, opcode, unsafe { *argument_nodes }, &mut register_list) {
            error_exc(emit, syntax_err(b"malformed register list"));
            return false;
        }
        let mut stack_adjustment_object = obj::OBJ_NULL;
        if !parse::parse_node_get_int_maybe(
            unsafe { *argument_nodes.add(1) },
            &mut stack_adjustment_object,
        ) {
            error_exc(emit, syntax_err(b"expects integer"));
            return false;
        }
        let stack_adjustment = obj::get_int_truncated(stack_adjustment_object);
        let abs_adj = if stack_adjustment < 0 {
            (-stack_adjustment) as u32
        } else {
            stack_adjustment as u32
        };
        if (abs_adj & !0x30) != 0
            || (qstr_eq_name(opcode, b"cm_push") && stack_adjustment > 0)
            || (!qstr_eq_name(opcode, b"cm_push") && stack_adjustment < 0)
        {
            error_exc(emit, syntax_err(b"invalid stack adjustment"));
            return false;
        }
        let adj = abs_adj;
        match index {
            0 => asmrv32::opcode_cmpush(&mut emit.as_, register_list, adj),
            1 => asmrv32::opcode_cmpop(&mut emit.as_, register_list, adj),
            2 => asmrv32::opcode_cmpopret(&mut emit.as_, register_list, adj),
            3 => asmrv32::opcode_cmpopretz(&mut emit.as_, register_list, adj),
            _ => {}
        }
        return true;
    }
    false
}

pub fn op(
    emit: *mut EmitInlineAsm,
    opcode: Qstr,
    arguments_count: usize,
    argument_nodes: *mut ParseNode,
) {
    if emit.is_null() || !ENABLED {
        return;
    }
    let emit = unsafe { &mut *emit };
    let mut opcode_data: Option<&Opcode> = None;
    for entry in OPCODES {
        if qstr_eq_name(opcode, entry.name) {
            opcode_data = Some(entry);
            break;
        }
    }
    if (asmrv32::allowed_extensions() & asmrv32::RV32_EXT_ZCMP) != 0
        && opcode_data.is_none()
        && arguments_count == 2
        && handle_zcmp_opcode(emit, opcode, argument_nodes)
    {
        return;
    }
    let Some(opcode_data) = opcode_data else {
        error_exc(emit, syntax_err(b"invalid RV32 instruction"));
        return;
    };
    if (asmrv32::allowed_extensions() & opcode_data.required_extensions)
        != opcode_data.required_extensions
    {
        error_exc(emit, syntax_err(b"invalid RV32 instruction"));
        return;
    }
    if arguments_count as u8 != opcode_data.parse_nodes {
        error_exc(emit, syntax_err(b"wrong argument count"));
        return;
    }
    let mut serialised_arguments = [0u32; 3];
    if opcode_data.parse_nodes >= 1
        && !serialise_argument(
            emit,
            opcode_data,
            unsafe { *argument_nodes },
            0,
            &mut serialised_arguments[0],
        )
    {
        return;
    }
    if opcode_data.calling_convention == CallConvention::Rir {
        let mut offset = 0u32;
        let mut base = 0u32;
        if !serialise_register_offset_node(
            emit,
            opcode_data,
            unsafe { *argument_nodes.add(1) },
            1,
            &mut offset,
            &mut base,
        ) {
            return;
        }
        serialised_arguments[1] = offset;
        serialised_arguments[2] = base;
    } else {
        if opcode_data.parse_nodes >= 2
            && !serialise_argument(
                emit,
                opcode_data,
                unsafe { *argument_nodes.add(1) },
                1,
                &mut serialised_arguments[1],
            )
        {
            return;
        }
        if opcode_data.parse_nodes >= 3
            && !serialise_argument(
                emit,
                opcode_data,
                unsafe { *argument_nodes.add(2) },
                2,
                &mut serialised_arguments[2],
            )
        {
            return;
        }
    }
    handle_opcode(
        &mut emit.as_,
        opcode_data.emitter,
        opcode_data.calling_convention,
        serialised_arguments,
    );
}
