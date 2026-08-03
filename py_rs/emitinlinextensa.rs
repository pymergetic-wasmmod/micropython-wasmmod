//! rewrite of py/emitinlinextensa.c
// symmetry: done

#![allow(non_snake_case, clippy::too_many_arguments)]

use crate::asmbase::{self, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};
use crate::asmxtensa::{self, AsmXtensa};
use crate::emit::PassKind;
use crate::grammar::Rule;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objexcept;
use crate::objstr;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};

const ENABLED: bool = mpconfig::EMIT_INLINE_XTENSA;

#[repr(C)]
pub struct EmitInlineAsm {
    pub as_: AsmXtensa,
    pass: u16,
    error_slot: *mut Obj,
    max_num_labels: usize,
    label_lookup: *mut Qstr,
}

fn emit_windowed_code() -> bool {
    mpconfig::EMIT_XTENSAWIN
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
    let emit = malloc::new_obj::<EmitInlineAsm>().expect("emit inline xtensa");
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
        if emit_windowed_code() {
            asmxtensa::entry_win(&mut (*emit).as_, 0);
        } else {
            asmxtensa::entry(&mut (*emit).as_, 0);
        }
    }
}

pub fn end_pass(emit: *mut EmitInlineAsm, _type_sig: usize) {
    if emit.is_null() || !ENABLED {
        return;
    }
    unsafe {
        if emit_windowed_code() {
            asmxtensa::exit_win(&mut (*emit).as_);
        } else {
            asmxtensa::exit(&mut (*emit).as_);
        }
        asmxtensa::end_pass(&mut (*emit).as_);
    }
}

pub fn count_params(emit: *mut EmitInlineAsm, n_params: usize, pn_params: *mut ParseNode) -> usize {
    if emit.is_null() || !ENABLED {
        return 0;
    }
    if n_params > 4 {
        error_msg(
            unsafe { &mut *emit },
            b"can only have up to 4 parameters to Xtensa assembly",
        );
        return 0;
    }
    for i in 0..n_params {
        let pn = unsafe { *pn_params.add(i) };
        if !parse::parse_node_is_id(pn) {
            error_msg(
                unsafe { &mut *emit },
                b"parameters must be registers in sequence a2 to a5",
            );
            return 0;
        }
        let p = qstr::qstr_str(parse::parse_node_leaf_arg(pn) as Qstr).unwrap_or_default();
        let p = if p.last() == Some(&0) {
            &p[..p.len() - 1]
        } else {
            p.as_slice()
        };
        if !(p.len() == 2 && p[0] == b'a' && p[1] == b'2' + i as u8) {
            error_msg(
                unsafe { &mut *emit },
                b"parameters must be registers in sequence a2 to a5",
            );
            return 0;
        }
    }
    n_params
}

pub fn label(emit: *mut EmitInlineAsm, label_num: usize, label_id: Qstr) -> bool {
    if emit.is_null() || !ENABLED {
        return false;
    }
    unsafe {
        debug_assert!(label_num < (*emit).max_num_labels);
        if (*emit).pass == PassKind::CodeSize as u16 {
            for i in 0..(*emit).max_num_labels {
                if *(*emit).label_lookup.add(i) == label_id {
                    return false;
                }
            }
        }
        *(*emit).label_lookup.add(label_num) = label_id;
        asmbase::label_assign(&mut (*emit).as_.base, label_num);
    }
    true
}

const REGISTERS: [&[u8]; 16] = [
    b"a0", b"a1", b"a2", b"a3", b"a4", b"a5", b"a6", b"a7", b"a8", b"a9", b"a10", b"a11", b"a12",
    b"a13", b"a14", b"a15",
];

fn get_arg_reg(emit: &mut EmitInlineAsm, op: &[u8], pn: ParseNode) -> usize {
    if parse::parse_node_is_id(pn) {
        let node_qstr = parse::parse_node_leaf_arg(pn) as Qstr;
        if let Some(data) = qstr::qstr_data(node_qstr) {
            let name = if data.0.last() == Some(&0) {
                &data.0[..data.0.len() - 1]
            } else {
                data.0.as_slice()
            };
            for (i, &reg) in REGISTERS.iter().enumerate() {
                if name == reg {
                    return i;
                }
            }
        }
    }
    error_exc(
        emit,
        objexcept::new_exception_args(
            objexcept::type_syntax_error(),
            1,
            &[objstr::new_str(b"' expects a register")],
        ),
    );
    0
}

fn get_arg_i(emit: &mut EmitInlineAsm, op: &[u8], pn: ParseNode, min: i32, max: i32) -> u32 {
    let mut o = obj::OBJ_NULL;
    if !parse::parse_node_get_int_maybe(pn, &mut o) {
        error_exc(
            emit,
            objexcept::new_exception_args(
                objexcept::type_syntax_error(),
                1,
                &[objstr::new_str(b"' expects an integer")],
            ),
        );
        return 0;
    }
    let i = obj::get_int_truncated(o) as u32;
    if min != max && ((i as i32) < min || (i as i32) > max) {
        error_exc(
            emit,
            objexcept::new_exception_args(
                objexcept::type_syntax_error(),
                1,
                &[objstr::new_str(b"'%s' integer out of range")],
            ),
        );
        return 0;
    }
    i
}

fn get_arg_label(emit: &mut EmitInlineAsm, op: &[u8], pn: ParseNode) -> i32 {
    if !parse::parse_node_is_id(pn) {
        error_exc(
            emit,
            objexcept::new_exception_args(
                objexcept::type_syntax_error(),
                1,
                &[objstr::new_str(b"' expects a label")],
            ),
        );
        return 0;
    }
    let label_qstr = parse::parse_node_leaf_arg(pn) as Qstr;
    unsafe {
        for i in 0..emit.max_num_labels {
            if *emit.label_lookup.add(i) == label_qstr {
                return i as i32;
            }
        }
        if emit.pass == PassKind::Emit as u16 {
            error_exc(
                emit,
                objexcept::new_exception_args(
                    objexcept::type_syntax_error(),
                    1,
                    &[objstr::new_str(b"label not defined")],
                ),
            );
        }
    }
    0
}

const RRR_R0: u32 = 1 << 4;
const RRR_R1: u32 = 2 << 4;
const RRR_R2: u32 = 3 << 4;
const RRR: u8 = 0;
const RRI8: u8 = 1;
const RRRN: u8 = 2;

struct OpcodeEntry {
    name: &'static [u8],
    operands: u8,
    op2: u8,
    op1: u8,
    op0: u8,
    r: u32,
    s: u32,
    t: u32,
    shift: u8,
    kind: u8,
}

macro_rules! xtensa_op {
    ($name:literal, $ops:expr, $op2:expr, $op1:expr, $op0:expr, $r:expr, $s:expr, $t:expr, $shift:expr, $kind:expr) => {
        OpcodeEntry {
            name: $name,
            operands: $ops,
            op2: $op2,
            op1: $op1,
            op0: $op0,
            r: $r,
            s: $s,
            t: $t,
            shift: $shift,
            kind: $kind,
        }
    };
}

const OPCODE_TABLE: &[OpcodeEntry] = &[
    xtensa_op!(b"abs_", 2, 6, 0, 0, RRR_R0, 1, RRR_R1, 0, RRR),
    xtensa_op!(b"add", 3, 8, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"add_n", 3, 0, 0, 10, RRR_R0, RRR_R1, RRR_R2, 0, RRRN),
    xtensa_op!(b"addi", 3, 0, 0, 2, 12, RRR_R1, RRR_R0, 0, RRI8),
    xtensa_op!(b"addx2", 3, 9, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"addx4", 3, 10, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"addx8", 3, 11, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"and_", 3, 1, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"callx0", 1, 0, 0, 0, 0, RRR_R0, 12, 0, RRR),
    xtensa_op!(b"jx", 1, 0, 0, 0, 0, RRR_R0, 10, 0, RRR),
    xtensa_op!(b"l16si", 3, 0, 0, 2, 9, RRR_R1, RRR_R0, 3, RRI8),
    xtensa_op!(b"l16ui", 3, 0, 0, 2, 1, RRR_R1, RRR_R0, 3, RRI8),
    xtensa_op!(b"l32i", 3, 0, 0, 2, 2, RRR_R1, RRR_R0, 5, RRI8),
    xtensa_op!(b"l8ui", 3, 0, 0, 2, 0, RRR_R1, RRR_R0, 1, RRI8),
    xtensa_op!(b"mov", 2, 0, 0, 13, 0, RRR_R1, RRR_R0, 0, RRRN),
    xtensa_op!(b"mov_n", 2, 0, 0, 13, 0, RRR_R1, RRR_R0, 0, RRRN),
    xtensa_op!(b"mull", 3, 8, 2, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"neg", 2, 6, 0, 0, RRR_R0, 0, RRR_R1, 0, RRR),
    xtensa_op!(b"nop", 0, 0, 0, 0, 2, 0, 15, 0, RRR),
    xtensa_op!(b"nop_n", 0, 0, 0, 13, 15, 0, 3, 0, RRRN),
    xtensa_op!(b"nsa", 2, 4, 0, 0, 14, RRR_R1, RRR_R0, 0, RRR),
    xtensa_op!(b"nsau", 2, 4, 0, 0, 15, RRR_R1, RRR_R0, 0, RRR),
    xtensa_op!(b"or_", 3, 2, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"ret", 0, 0, 0, 13, 15, 0, 0, 0, RRRN),
    xtensa_op!(b"ret_n", 0, 0, 0, 13, 15, 0, 0, 0, RRRN),
    xtensa_op!(b"s16i", 3, 0, 0, 2, 5, RRR_R1, RRR_R0, 3, RRI8),
    xtensa_op!(b"s32i", 3, 0, 0, 2, 6, RRR_R1, RRR_R0, 5, RRI8),
    xtensa_op!(b"s8i", 3, 0, 0, 2, 4, RRR_R1, RRR_R0, 1, RRI8),
    xtensa_op!(b"sll", 2, 10, 1, 0, RRR_R0, RRR_R1, 0, 0, RRR),
    xtensa_op!(b"sra", 2, 11, 1, 0, RRR_R0, 0, RRR_R1, 0, RRR),
    xtensa_op!(b"src", 3, 8, 1, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"srl", 2, 9, 1, 0, RRR_R0, 0, RRR_R1, 0, RRR),
    xtensa_op!(b"ssa8b", 1, 4, 0, 0, 3, RRR_R0, 0, 0, RRR),
    xtensa_op!(b"ssa8l", 1, 4, 0, 0, 2, RRR_R0, 0, 0, RRR),
    xtensa_op!(b"ssl", 1, 4, 0, 0, 1, RRR_R0, 0, 0, RRR),
    xtensa_op!(b"ssr", 1, 4, 0, 0, 0, RRR_R0, 0, 0, RRR),
    xtensa_op!(b"sub", 3, 12, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"subx2", 3, 13, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"subx4", 3, 14, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"subx8", 3, 15, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
    xtensa_op!(b"xor", 3, 3, 0, 0, RRR_R0, RRR_R1, RRR_R2, 0, RRR),
];

const BCCZ_OPCODES: [&[u8]; 6] = [b"beqz", b"bnez", b"bltz", b"bgez", b"beqz_n", b"bnez_n"];

const BRANCH_OPCODE_NAMES: [&[u8]; 14] = [
    b"bnone", b"beq", b"blt", b"bltu", b"ball", b"bbc", b"", b"", b"bany", b"bne", b"bge", b"bgeu",
    b"bnall", b"bbs",
];

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

pub fn op(emit: *mut EmitInlineAsm, op: Qstr, n_args: usize, pn_args: *mut ParseNode) {
    if emit.is_null() || !ENABLED {
        return;
    }
    let emit = unsafe { &mut *emit };
    let mut op_buf = [0u8; 32];
    let op_str = if let Some((d, l)) = qstr::qstr_data(op) {
        let len = if d.last() == Some(&0) {
            l.saturating_sub(1)
        } else {
            l
        };
        let len = len.min(op_buf.len());
        op_buf[..len].copy_from_slice(&d[..len]);
        &op_buf[..len]
    } else {
        &[][..]
    };

    for entry in OPCODE_TABLE {
        if qstr_eq_name(op, entry.name) {
            if n_args as u8 != entry.operands {
                goto_unknown(emit, op_str, n_args);
                return;
            }
            let mut opcode = ((entry.r & 0x0f) << 12)
                | ((entry.s & 0x0f) << 8)
                | ((entry.t & 0x0f) << 4)
                | entry.op0 as u32;
            if entry.kind == RRR {
                opcode |= (entry.op2 as u32) << 20 | (entry.op1 as u32) << 16;
            } else if entry.kind == RRI8 {
                let (min, max, shift) = if entry.shift > 0 {
                    (0i32, 0xff << (entry.shift >> 1), entry.shift >> 1)
                } else {
                    (-128, 127, 0)
                };
                let pn2 = unsafe { *pn_args.add(2) };
                let immediate = get_arg_i(emit, op_str, pn2, min, max);
                opcode |= ((immediate >> shift) & 0xff) << 16;
            }
            if entry.r >= RRR_R0 {
                let pn = unsafe { *pn_args.add(((entry.r >> 4) - 1) as usize) };
                opcode |= (get_arg_reg(emit, op_str, pn) as u32) << 12;
            }
            if entry.s >= RRR_R0 {
                let pn = unsafe { *pn_args.add(((entry.s >> 4) - 1) as usize) };
                opcode |= (get_arg_reg(emit, op_str, pn) as u32) << 8;
            }
            if entry.t >= RRR_R0 {
                let pn = unsafe { *pn_args.add(((entry.t >> 4) - 1) as usize) };
                opcode |= (get_arg_reg(emit, op_str, pn) as u32) << 4;
            }
            if entry.kind == RRRN {
                asmxtensa::op16(&mut emit.as_, (opcode & 0xffff) as u16);
            } else {
                asmxtensa::op24(&mut emit.as_, opcode);
            }
            return;
        }
    }

    if n_args == 1 {
        if qstr_eq_name(op, b"j") {
            let label = get_arg_label(emit, op_str, unsafe { *pn_args });
            asmxtensa::j_label(&mut emit.as_, label as usize);
        } else if qstr_eq_name(op, b"ssai") {
            let sa = get_arg_i(emit, op_str, unsafe { *pn_args }, 0, 31);
            asmxtensa::op24(
                &mut emit.as_,
                asmxtensa::encode_rrr(0, 0, 4, 4, sa & 0x0f, (sa >> 4) & 0x01),
            );
        } else if qstr_eq_name(op, b"call0") {
            let label = get_arg_label(emit, op_str, unsafe { *pn_args }) as usize;
            asmxtensa::call0(&mut emit.as_, label);
        } else {
            goto_unknown(emit, op_str, n_args);
        }
    } else if n_args == 2 {
        let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
        for (index, &name) in BCCZ_OPCODES.iter().enumerate() {
            if qstr_eq_name(op, name) {
                let label = get_arg_label(emit, op_str, unsafe { *pn_args.add(1) }) as usize;
                asmxtensa::bccz_reg_label(&mut emit.as_, (index & 0x03) as u32, r0 as u32, label);
                return;
            }
        }
        if qstr_eq_name(op, b"movi") {
            let imm = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0, 0);
            asmxtensa::mov_reg_i32_optimised(&mut emit.as_, r0 as u32, imm);
        } else if qstr_eq_name(op, b"l32r") {
            let label = get_arg_label(emit, op_str, unsafe { *pn_args.add(1) }) as usize;
            asmxtensa::l32r(&mut emit.as_, r0, label);
        } else if qstr_eq_name(op, b"movi_n") {
            let imm = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, -32, 95) as i32;
            asmxtensa::op_movi_n(&mut emit.as_, r0 as u32, imm);
        } else {
            goto_unknown(emit, op_str, n_args);
        }
    } else if n_args == 3 {
        for (index, &name) in BRANCH_OPCODE_NAMES.iter().enumerate() {
            if !name.is_empty() && qstr_eq_name(op, name) {
                let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
                let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
                let label = get_arg_label(emit, op_str, unsafe { *pn_args.add(2) }) as usize;
                asmxtensa::bcc_reg_reg_label(
                    &mut emit.as_,
                    index as u32,
                    r0 as u32,
                    r1 as u32,
                    label,
                );
                return;
            }
        }
        if qstr_eq_name(op, b"addi_n") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let imm4 = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, -1, 15) as i32;
            let t = if imm4 != 0 { imm4 as u32 } else { 0xf };
            asmxtensa::op16(
                &mut emit.as_,
                asmxtensa::encode_rrrn(11, r0 as u32, r1 as u32, t),
            );
        } else if qstr_eq_name(op, b"addmi") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let imm8 = get_arg_i(
                emit,
                op_str,
                unsafe { *pn_args.add(2) },
                -128 * 256,
                127 * 256,
            ) as i32;
            if (imm8 & 0xff) != 0 {
                error_exc(
                    emit,
                    objexcept::new_exception_args(
                        objexcept::type_syntax_error(),
                        1,
                        &[objstr::new_str(b"not a multiple of 256")],
                    ),
                );
            } else {
                asmxtensa::op24(
                    &mut emit.as_,
                    asmxtensa::encode_rri8(2, 13, r1 as u32, r0 as u32, (imm8 >> 8) as u32),
                );
            }
        } else if qstr_eq_name(op, b"bbci") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let bit = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0, 31);
            let label = get_arg_label(emit, op_str, unsafe { *pn_args.add(2) }) as usize;
            asmxtensa::bit_branch(&mut emit.as_, r0, bit as usize, label, 6);
        } else if qstr_eq_name(op, b"bbsi") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let bit = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0, 31);
            let label = get_arg_label(emit, op_str, unsafe { *pn_args.add(2) }) as usize;
            asmxtensa::bit_branch(&mut emit.as_, r0, bit as usize, label, 14);
        } else if qstr_eq_name(op, b"slli") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let bits = 32 - get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 1, 31);
            asmxtensa::op24(
                &mut emit.as_,
                asmxtensa::encode_rrr(
                    0,
                    1,
                    0 | (((bits >> 4) & 0x01) as u32),
                    r0 as u32,
                    r1 as u32,
                    bits & 0x0f,
                ),
            );
        } else if qstr_eq_name(op, b"srai") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let bits = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0, 31);
            asmxtensa::op24(
                &mut emit.as_,
                asmxtensa::encode_rrr(
                    0,
                    1,
                    2 | (((bits >> 4) & 0x01) as u32),
                    r0 as u32,
                    bits & 0x0f,
                    r1 as u32,
                ),
            );
        } else if qstr_eq_name(op, b"srli") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let bits = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0, 15);
            asmxtensa::op24(
                &mut emit.as_,
                asmxtensa::encode_rrr(0, 1, 4, r0 as u32, bits, r1 as u32),
            );
        } else if qstr_eq_name(op, b"l32i_n") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let imm = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0, 60);
            if (imm & 0x03) != 0 {
                error_exc(
                    emit,
                    objexcept::new_exception_args(
                        objexcept::type_syntax_error(),
                        1,
                        &[objstr::new_str(b"not a multiple of 4")],
                    ),
                );
            } else {
                asmxtensa::op_l32i_n(&mut emit.as_, r0 as u32, r1 as u32, imm >> 2);
            }
        } else if qstr_eq_name(op, b"s32i_n") {
            let r0 = get_arg_reg(emit, op_str, unsafe { *pn_args });
            let r1 = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) });
            let imm = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0, 60);
            if (imm & 0x03) != 0 {
                error_exc(
                    emit,
                    objexcept::new_exception_args(
                        objexcept::type_syntax_error(),
                        1,
                        &[objstr::new_str(b"not a multiple of 4")],
                    ),
                );
            } else {
                asmxtensa::op_s32i_n(&mut emit.as_, r0 as u32, r1 as u32, imm >> 2);
            }
        } else {
            goto_unknown(emit, op_str, n_args);
        }
    } else {
        goto_unknown(emit, op_str, n_args);
    }
}

fn goto_unknown(emit: &mut EmitInlineAsm, op_str: &[u8], n_args: usize) {
    error_exc(
        emit,
        objexcept::new_exception_args(
            objexcept::type_syntax_error(),
            1,
            &[objstr::new_str(b"unsupported Xtensa instruction")],
        ),
    );
    let _ = n_args;
}
