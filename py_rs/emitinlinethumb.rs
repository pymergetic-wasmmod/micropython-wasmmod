//! rewrite of py/emitinlinethumb.c
// symmetry: done

#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::identity_op,
    unused_labels
)]

use crate::asmbase::{self, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};
use crate::asmthumb::{self, AsmThumb};
use crate::emit::PassKind;
use crate::grammar::Rule;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objexcept;
use crate::objstr;
use crate::parse::{self, ParseNode, ParseNodeStruct};
use crate::qstr::{self, Qstr};

const ENABLED: bool = mpconfig::EMIT_INLINE_THUMB;

#[repr(C)]
pub struct EmitInlineAsm {
    pub as_: AsmThumb,
    pass: u16,
    error_slot: *mut Obj,
    max_num_labels: usize,
    label_lookup: *mut Qstr,
}

fn allow_float(_emit: &EmitInlineAsm) -> bool {
    mpconfig::EMIT_INLINE_THUMB_FLOAT
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
    unsafe { *emit.error_slot = exc; }
}

pub fn new(max_num_labels: usize) -> *mut EmitInlineAsm {
    if !ENABLED {
        return core::ptr::null_mut();
    }
    let emit = malloc::new_obj::<EmitInlineAsm>().expect("emit inline thumb");
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
            core::ptr::write_bytes(
                (*emit).label_lookup,
                0,
                (*emit).max_num_labels * core::mem::size_of::<Qstr>(),
            );
        }
        let asm_pass = if pass == PassKind::Emit {
            MP_ASM_PASS_EMIT
        } else {
            MP_ASM_PASS_COMPUTE
        };
        asmbase::start_pass(&mut (*emit).as_.base, asm_pass as i32);
        asmthumb::entry(&mut (*emit).as_, 0);
    }
}

pub fn end_pass(emit: *mut EmitInlineAsm, _type_sig: usize) {
    if emit.is_null() || !ENABLED {
        return;
    }
    unsafe {
        asmthumb::exit(&mut (*emit).as_);
        asmthumb::end_pass(&mut (*emit).as_);
    }
}

pub fn count_params(emit: *mut EmitInlineAsm, n_params: usize, pn_params: *mut ParseNode) -> usize {
    if emit.is_null() || !ENABLED {
        return 0;
    }
    if n_params > 4 {
        error_msg(unsafe { &mut *emit }, b"can only have up to 4 parameters to Thumb assembly");
        return 0;
    }
    for i in 0..n_params {
        let pn = unsafe { *pn_params.add(i) };
        if !parse::parse_node_is_id(pn) {
            error_msg(unsafe { &mut *emit }, b"parameters must be registers in sequence r0 to r3");
            return 0;
        }
        let p = qstr::qstr_str(parse::parse_node_leaf_arg(pn) as Qstr).unwrap_or_default();
        let p = if p.last() == Some(&0) { &p[..p.len() - 1] } else { p.as_slice() };
        if !(p.len() == 2 && p[0] == b'r' && p[1] == b'0' + i as u8) {
            error_msg(unsafe { &mut *emit }, b"parameters must be registers in sequence r0 to r3");
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

struct RegName {
    reg: u8,
    name: [u8; 3],
}

const REG_NAME_TABLE: &[RegName] = &[
    RegName { reg: 0, name: [b'r', b'0', 0] },
    RegName { reg: 1, name: [b'r', b'1', 0] },
    RegName { reg: 2, name: [b'r', b'2', 0] },
    RegName { reg: 3, name: [b'r', b'3', 0] },
    RegName { reg: 4, name: [b'r', b'4', 0] },
    RegName { reg: 5, name: [b'r', b'5', 0] },
    RegName { reg: 6, name: [b'r', b'6', 0] },
    RegName { reg: 7, name: [b'r', b'7', 0] },
    RegName { reg: 8, name: [b'r', b'8', 0] },
    RegName { reg: 9, name: [b'r', b'9', 0] },
    RegName { reg: 10, name: [b'r', b'1', b'0'] },
    RegName { reg: 11, name: [b'r', b'1', b'1'] },
    RegName { reg: 12, name: [b'r', b'1', b'2'] },
    RegName { reg: 13, name: [b'r', b'1', b'3'] },
    RegName { reg: 14, name: [b'r', b'1', b'4'] },
    RegName { reg: 15, name: [b'r', b'1', b'5'] },
    RegName { reg: 10, name: [b's', b'l', 0] },
    RegName { reg: 11, name: [b'f', b'p', 0] },
    RegName { reg: 13, name: [b's', b'p', 0] },
    RegName { reg: 14, name: [b'l', b'r', 0] },
    RegName { reg: 15, name: [b'p', b'c', 0] },
];

struct SpecialRegName {
    reg: u8,
    name: &'static [u8],
}

const SPECIAL_REG_NAME_TABLE: &[SpecialRegName] = &[
    SpecialRegName { reg: 5, name: b"IPSR" },
    SpecialRegName { reg: 17, name: b"BASEPRI" },
];

const CONDITION_CODES: [u16; 14] = [
    encode_cc(b'e', b'q'),
    encode_cc(b'n', b'e'),
    encode_cc(b'c', b's'),
    encode_cc(b'c', b'c'),
    encode_cc(b'm', b'i'),
    encode_cc(b'p', b'l'),
    encode_cc(b'v', b's'),
    encode_cc(b'v', b'c'),
    encode_cc(b'h', b'i'),
    encode_cc(b'l', b's'),
    encode_cc(b'g', b'e'),
    encode_cc(b'l', b't'),
    encode_cc(b'g', b't'),
    encode_cc(b'l', b'e'),
];

const fn encode_cc(c1: u8, c2: u8) -> u16 {
    ((c1 as u16) << 8) | c2 as u16
}

struct Format4Op {
    op: u8,
    name: [u8; 3],
}

const fn x(x: u32) -> u8 {
    ((x >> 4) & 0xff) as u8
}

const FORMAT_4_OP_TABLE: &[Format4Op] = &[
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_EOR), name: [b'e', b'o', b'r'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_LSL), name: [b'l', b's', b'l'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_LSR), name: [b'l', b's', b'r'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_ASR), name: [b'a', b's', b'r'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_ADC), name: [b'a', b'd', b'c'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_SBC), name: [b's', b'b', b'c'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_ROR), name: [b'r', b'o', b'r'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_TST), name: [b't', b's', b't'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_NEG as u32), name: [b'n', b'e', b'g'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_CMP as u32), name: [b'c', b'm', b'p'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_CMN as u32), name: [b'c', b'm', b'n'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_ORR as u32), name: [b'o', b'r', b'r'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_MUL as u32), name: [b'm', b'u', b'l'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_BIC as u32), name: [b'b', b'i', b'c'] },
    Format4Op { op: x(asmthumb::ASM_THUMB_FORMAT_4_MVN as u32), name: [b'm', b'v', b'n'] },
];

struct Format910Op {
    op: u16,
    name: &'static [u8],
}

const FORMAT_9_10_OP_TABLE: &[Format910Op] = &[
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_9_LDR | asmthumb::ASM_THUMB_FORMAT_9_WORD_TRANSFER, name: b"ldr" },
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_9_LDR | asmthumb::ASM_THUMB_FORMAT_9_BYTE_TRANSFER, name: b"ldrb" },
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_10_LDRH, name: b"ldrh" },
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_9_STR | asmthumb::ASM_THUMB_FORMAT_9_WORD_TRANSFER, name: b"str" },
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_9_STR | asmthumb::ASM_THUMB_FORMAT_9_BYTE_TRANSFER, name: b"strb" },
    Format910Op { op: asmthumb::ASM_THUMB_FORMAT_10_STRH, name: b"strh" },
];

struct FormatVfpOp {
    op: u8,
    name: [u8; 3],
}

const FORMAT_VFP_OP_TABLE: &[FormatVfpOp] = &[
    FormatVfpOp { op: 0x30, name: [b'a', b'd', b'd'] },
    FormatVfpOp { op: 0x34, name: [b's', b'u', b'b'] },
    FormatVfpOp { op: 0x20, name: [b'm', b'u', b'l'] },
    FormatVfpOp { op: 0x80, name: [b'd', b'i', b'v'] },
];

fn qstr_eq_name(q: Qstr, name: &[u8]) -> bool {
    qstr::qstr_data(q)
        .map(|(d, _)| {
            let s = if d.last() == Some(&0) { &d[..d.len() - 1] } else { d.as_slice() };
            s == name
        })
        .unwrap_or(false)
}

fn get_arg_str(pn: ParseNode) -> Vec<u8> {
    if parse::parse_node_is_id(pn) {
        qstr::qstr_str(parse::parse_node_leaf_arg(pn) as Qstr).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn reg_name_matches(reg_str: &[u8], r: &RegName) -> bool {
    reg_str.first() == Some(&r.name[0])
        && reg_str.get(1) == Some(&r.name[1])
        && (reg_str.get(2).copied().unwrap_or(0) == r.name[2])
        && (r.name[2] == 0 || reg_str.get(3).copied().unwrap_or(0) == 0)
}

fn get_arg_reg(emit: &mut EmitInlineAsm, _op: &[u8], pn: ParseNode, max_reg: usize) -> usize {
    let reg_str = get_arg_str(pn);
    let reg_slice = if reg_str.last() == Some(&0) {
        &reg_str[..reg_str.len() - 1]
    } else {
        reg_str.as_slice()
    };
    for r in REG_NAME_TABLE {
        if reg_name_matches(reg_slice, r) {
            if r.reg as usize > max_reg {
                error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"register out of range")]));
                return 0;
            }
            return r.reg as usize;
        }
    }
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects a register")]));
    0
}

fn get_arg_special_reg(emit: &mut EmitInlineAsm, _op: &[u8], pn: ParseNode) -> usize {
    let reg_str = get_arg_str(pn);
    let reg_slice = if reg_str.last() == Some(&0) {
        &reg_str[..reg_str.len() - 1]
    } else {
        reg_str.as_slice()
    };
    for r in SPECIAL_REG_NAME_TABLE {
        if reg_slice == r.name {
            return r.reg as usize;
        }
    }
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects a special register")]));
    0
}

fn get_arg_vfpreg(emit: &mut EmitInlineAsm, _op: &[u8], pn: ParseNode) -> usize {
    let reg_str = get_arg_str(pn);
    let reg_slice = if reg_str.last() == Some(&0) {
        &reg_str[..reg_str.len() - 1]
    } else {
        reg_str.as_slice()
    };
    if reg_slice.first() == Some(&b's') && reg_slice.len() > 1 {
        let mut regno = 0usize;
        for &c in &reg_slice[1..] {
            if !c.is_ascii_digit() {
                goto_malformed_vfp(emit);
                return 0;
            }
            regno = 10 * regno + (c - b'0') as usize;
        }
        if regno > 31 {
            error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"vfp register out of range")]));
            return 0;
        }
        return regno;
    }
    goto_malformed_vfp(emit);
    0
}

fn goto_malformed_vfp(emit: &mut EmitInlineAsm) {
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects an FPU register")]));
}

fn get_arg_reglist(emit: &mut EmitInlineAsm, op: &[u8], pn: ParseNode) -> u32 {
    if !parse::parse_node_is_struct_kind(pn, Rule::AtomBrace) {
        goto_bad_reglist(emit, op);
        return 0;
    }
    let pns = pn as *const ParseNodeStruct;
    debug_assert_eq!(parse::parse_node_struct_num_nodes(pns), 1);
    let inner = parse::parse_node_struct_node(pns, 0);
    let mut reglist = 0u32;
    if parse::parse_node_is_id(inner) {
        reglist |= 1 << get_arg_reg(emit, op, inner, 15);
    } else if parse::parse_node_is_struct(inner) {
        let pns = inner as *const ParseNodeStruct;
        if parse::parse_node_struct_kind(pns) == Rule::Dictorsetmaker as u32 {
            reglist |= 1 << get_arg_reg(emit, op, parse::parse_node_struct_node(pns, 0), 15);
            let pns1 = parse::parse_node_struct_node(pns, 1) as *const ParseNodeStruct;
            if parse::parse_node_struct_kind(pns1) == Rule::DictorsetmakerList as u32 {
                reglist |= 1 << get_arg_reg(emit, op, parse::parse_node_struct_node(pns1, 0), 15);
                let mut nodes_ptr: *mut ParseNode = core::ptr::null_mut();
                let mut list_pn = parse::parse_node_struct_node(pns1, 0);
                let n = parse::parse_node_extract_list(&mut list_pn, Rule::DictorsetmakerList2, &mut nodes_ptr);
                for i in 0..n {
                    reglist |= 1 << get_arg_reg(emit, op, unsafe { *nodes_ptr.add(i) }, 15);
                }
            } else {
                goto_bad_reglist(emit, op);
                return 0;
            }
        } else {
            goto_bad_reglist(emit, op);
            return 0;
        }
    } else {
        goto_bad_reglist(emit, op);
        return 0;
    }
    reglist
}

fn goto_bad_reglist(emit: &mut EmitInlineAsm, _op: &[u8]) {
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects {r0, r1, ...}")]));
}

fn get_arg_i(emit: &mut EmitInlineAsm, _op: &[u8], pn: ParseNode, fit_mask: u32) -> u32 {
    let mut o = obj::OBJ_NULL;
    if !parse::parse_node_get_int_maybe(pn, &mut o) {
        error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects an integer")]));
        return 0;
    }
    let i = obj::get_int_truncated(o) as u32;
    if (i & !fit_mask) != 0 {
        error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"integer doesn't fit in mask")]));
        return 0;
    }
    i
}

fn get_arg_addr(emit: &mut EmitInlineAsm, op: &[u8], pn: ParseNode) -> Option<(ParseNode, ParseNode)> {
    if !parse::parse_node_is_struct_kind(pn, Rule::AtomBracket) {
        goto_bad_addr(emit, op);
        return None;
    }
    let pns = pn as *const ParseNodeStruct;
    let inner = parse::parse_node_struct_node(pns, 0);
    if !parse::parse_node_is_struct_kind(inner, Rule::TestlistComp) {
        goto_bad_addr(emit, op);
        return None;
    }
    let pns = inner as *const ParseNodeStruct;
    if parse::parse_node_struct_num_nodes(pns) != 2 {
        goto_bad_addr(emit, op);
        return None;
    }
    Some((parse::parse_node_struct_node(pns, 0), parse::parse_node_struct_node(pns, 1)))
}

fn goto_bad_addr(emit: &mut EmitInlineAsm, _op: &[u8]) {
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects an address of the form [a, b]")]));
}

fn get_arg_label(emit: &mut EmitInlineAsm, _op: &[u8], pn: ParseNode) -> i32 {
    if !parse::parse_node_is_id(pn) {
        error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"expects a label")]));
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
            error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"label not defined")]));
        }
    }
    0
}

pub fn op(emit: *mut EmitInlineAsm, op: Qstr, n_args: usize, pn_args: *mut ParseNode) {
    if emit.is_null() || !ENABLED {
        return;
    }
    let emit = unsafe { &mut *emit };
    let mut op_buf = [0u8; 32];
    let (op_str, op_len) = if let Some((d, l)) = qstr::qstr_data(op) {
        let len = if d.last() == Some(&0) {
            l.saturating_sub(1)
        } else {
            l
        };
        let len = len.min(op_buf.len());
        op_buf[..len].copy_from_slice(&d[..len]);
        (&op_buf[..len], len)
    } else {
        (&[][..], 0usize)
    };

    if allow_float(emit) && op_str.first() == Some(&b'v') {
        emit_vfp_op(emit, op, op_str, n_args, pn_args);
        return;
    }

    if n_args == 0 {
        if qstr_eq_name(op, b"nop") {
            asmthumb::op16(&mut emit.as_, asmthumb::ASM_THUMB_OP_NOP);
        } else if qstr_eq_name(op, b"wfi") {
            asmthumb::op16(&mut emit.as_, asmthumb::ASM_THUMB_OP_WFI);
        } else {
            unknown_op(emit, op_str, n_args);
        }
    } else if n_args == 1 {
        emit_one_arg(emit, op, op_str, op_len, pn_args);
    } else if n_args == 2 {
        emit_two_args(emit, op, op_str, pn_args);
    } else if n_args == 3 {
        emit_three_args(emit, op, op_str, pn_args);
    } else {
        unknown_op(emit, op_str, n_args);
    }
}

fn emit_vfp_op(emit: &mut EmitInlineAsm, op: Qstr, op_str: &[u8], n_args: usize, pn_args: *mut ParseNode) {
    if n_args == 2 {
        let op_code = 0x0ac0u32;
        if qstr_eq_name(op, b"vcmp") {
            let vd = get_arg_vfpreg(emit, op_str, unsafe { *pn_args });
            let vm = get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(1) });
            asmthumb::op32(
                &mut emit.as_,
                0xeeb4u32 | (((vd as u32) & 1) << 6),
                op_code | (((vd as u32) & 0x1e) << 11) | (((vm as u32) & 1) << 5) | (((vm as u32) & 0x1e) >> 1),
            );
        } else if qstr_eq_name(op, b"vsqrt") || qstr_eq_name(op, b"vcvt_f32_s32") || qstr_eq_name(op, b"vcvt_s32_f32") {
            let hi = if qstr_eq_name(op, b"vsqrt") { 0xeeb1 } else if qstr_eq_name(op, b"vcvt_f32_s32") { 0xeeb8 } else { 0xeebd };
            let vd = get_arg_vfpreg(emit, op_str, unsafe { *pn_args });
            let vm = get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(1) });
            asmthumb::op32(&mut emit.as_, hi | (((vd as u32) & 1) << 6), op_code | (((vd as u32) & 0x1e) << 11) | (((vm as u32) & 1) << 5) | ((vm as u32) & 0x1e) >> 1);
        } else if qstr_eq_name(op, b"vneg") {
            let vd = get_arg_vfpreg(emit, op_str, unsafe { *pn_args });
            let vm = get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(1) });
            asmthumb::op32(&mut emit.as_, 0xeeb1 | (((vd as u32) & 1) << 6), 0x0a40 | (((vd as u32) & 0x1e) << 11) | (((vm as u32) & 1) << 5) | ((vm as u32) & 0x1e) >> 1);
        } else if qstr_eq_name(op, b"vmrs") {
            let reg_str0 = get_arg_str(unsafe { *pn_args });
            let reg_dest = if reg_str0.starts_with(b"APSR_nzcv") { 15 } else { get_arg_reg(emit, op_str, unsafe { *pn_args }, 15) };
            let reg_str1 = get_arg_str(unsafe { *pn_args.add(1) });
            if reg_str1.starts_with(b"FPSCR") {
                asmthumb::op32(&mut emit.as_, 0xeef1, 0x0a10 | ((reg_dest as u32) << 12));
            } else {
                unknown_op(emit, op_str, 2);
            }
        } else if qstr_eq_name(op, b"vmov") {
            let reg_str = get_arg_str(unsafe { *pn_args });
            let (op_code_hi, r_arm, vm) = if reg_str.first() == Some(&b'r') {
                (0xee10, get_arg_reg(emit, op_str, unsafe { *pn_args }, 15), get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(1) }))
            } else {
                (0xee00, get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 15), get_arg_vfpreg(emit, op_str, unsafe { *pn_args }))
            };
            asmthumb::op32(&mut emit.as_, op_code_hi | (((vm as u32) & 0x1e) >> 1), 0x0a10 | ((r_arm as u32) << 12) | (((vm as u32) & 1) << 7));
        } else if qstr_eq_name(op, b"vldr") || qstr_eq_name(op, b"vstr") {
            let op_code_hi = if qstr_eq_name(op, b"vldr") { 0xed90 } else { 0xed80 };
            let vd = get_arg_vfpreg(emit, op_str, unsafe { *pn_args });
            if let Some((pn_base, pn_offset)) = get_arg_addr(emit, op_str, unsafe { *pn_args.add(1) }) {
                let rlo_base = get_arg_reg(emit, op_str, pn_base, 7);
                let i8 = get_arg_i(emit, op_str, pn_offset, 0x3fc) >> 2;
                asmthumb::op32(&mut emit.as_, op_code_hi | (rlo_base as u32), 0x0a00 | (((vd as u32) & 0x1e) << 11) | (i8 as u32));
            }
        } else {
            unknown_op(emit, op_str, 2);
        }
    } else if n_args == 3 {
        for entry in FORMAT_VFP_OP_TABLE {
            if op_str.len() >= 4 && op_str[1..4] == entry.name && op_str.get(4).copied().unwrap_or(0) == 0 {
                let op_code_hi = 0xee00 | ((entry.op & 0xf0) as u32);
                let op_code = 0x0a00 | (((entry.op & 0x0f) as u32) << 4);
                let vd = get_arg_vfpreg(emit, op_str, unsafe { *pn_args });
                let vn = get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(1) });
                let vm = get_arg_vfpreg(emit, op_str, unsafe { *pn_args.add(2) });
                asmthumb::op32(&mut emit.as_, op_code_hi | (((vd as u32) & 1) << 6) | ((vn as u32) >> 1), op_code | ((vm as u32) >> 1) | (((vm as u32) & 1) << 5) | (((vd as u32) & 0x1e) << 11) | (((vn as u32) & 1) << 7));
                return;
            }
        }
        unknown_op(emit, op_str, 3);
    } else {
        unknown_op(emit, op_str, n_args);
    }
}

fn emit_one_arg(emit: &mut EmitInlineAsm, op: Qstr, op_str: &[u8], op_len: usize, pn_args: *mut ParseNode) {
    if qstr_eq_name(op, b"b") {
        let label_num = get_arg_label(emit, op_str, unsafe { *pn_args });
        if !asmthumb::b_n_label(&mut emit.as_, label_num as usize) {
            branch_not_in_range(emit);
        }
    } else if qstr_eq_name(op, b"bl") {
        let label_num = get_arg_label(emit, op_str, unsafe { *pn_args });
        if !asmthumb::bl_label(&mut emit.as_, label_num as usize) {
            branch_not_in_range(emit);
        }
    } else if qstr_eq_name(op, b"bx") {
        let r = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        asmthumb::op16(&mut emit.as_, 0x4700 | ((r as u16) << 3));
    } else if op_str.first() == Some(&b'b')
        && (op_len == 3 || (op_len == 5 && op_str[3] == b'_' && (op_str[4] == b'n' || (asmthumb::allow_armv7m(&emit.as_) && op_str[4] == b'w'))))
    {
        let condition_code = encode_cc(op_str[1], op_str[2]);
        let mut cc = None;
        for (i, &c) in CONDITION_CODES.iter().enumerate() {
            if condition_code == c {
                cc = Some(i);
                break;
            }
        }
        if cc.is_none() {
            unknown_op(emit, op_str, 1);
            return;
        }
        let label_num = get_arg_label(emit, op_str, unsafe { *pn_args });
        let wide = op_len == 5 && op_str[4] == b'w';
        if wide && !asmthumb::allow_armv7m(&emit.as_) {
            unknown_op(emit, op_str, 1);
            return;
        }
        if !asmthumb::bcc_nw_label(&mut emit.as_, cc.unwrap() as i32, label_num as usize, wide) {
            branch_not_in_range(emit);
        }
    } else if asmthumb::allow_armv7m(&emit.as_) && op_str.starts_with(b"it") {
        let arg_str = get_arg_str(unsafe { *pn_args });
        let arg_slice = if arg_str.last() == Some(&0) { &arg_str[..arg_str.len()-1] } else { arg_str.as_slice() };
        if arg_slice.len() != 2 {
            unknown_op(emit, op_str, 1);
            return;
        }
        let condition_code = encode_cc(arg_slice[0], arg_slice[1]);
        let mut cc = None;
        for (i, &c) in CONDITION_CODES.iter().enumerate() {
            if condition_code == c { cc = Some(i); break; }
        }
        if cc.is_none() {
            unknown_op(emit, op_str, 1);
            return;
        }
        let cc = cc.unwrap();
        if op_str.len() > 5 {
            unknown_op(emit, op_str, 1);
            return;
        }
        let mut it_mask = 8u32;
        let mut os = op_str.len();
        while os > 2 {
            os -= 1;
            it_mask >>= 1;
            if op_str[os] == b't' {
                it_mask |= (cc as u32 & 1) << 3;
            } else if op_str[os] == b'e' {
                it_mask |= (((!cc) as u32) & 1) << 3;
            } else {
                unknown_op(emit, op_str, 1);
                return;
            }
        }
        asmthumb::it_cc(&mut emit.as_, cc as u32, it_mask);
    } else if qstr_eq_name(op, b"cpsid") {
        asmthumb::op16(&mut emit.as_, asmthumb::ASM_THUMB_OP_CPSID_I);
    } else if qstr_eq_name(op, b"cpsie") {
        asmthumb::op16(&mut emit.as_, asmthumb::ASM_THUMB_OP_CPSIE_I);
    } else if qstr_eq_name(op, b"push") {
        let reglist = get_arg_reglist(emit, op_str, unsafe { *pn_args });
        if (reglist & 0xbf00) == 0 {
            if (reglist & (1 << 14)) == 0 {
                asmthumb::op16(&mut emit.as_, 0xb400 | reglist as u16);
            } else {
                asmthumb::op16(&mut emit.as_, 0xb500 | (reglist & 0xff) as u16);
            }
        } else if asmthumb::allow_armv7m(&emit.as_) {
            asmthumb::op32(&mut emit.as_, 0xe92d, reglist);
        } else {
            unknown_op(emit, op_str, 1);
        }
    } else if qstr_eq_name(op, b"pop") {
        let reglist = get_arg_reglist(emit, op_str, unsafe { *pn_args });
        if (reglist & 0x7f00) == 0 {
            if (reglist & (1 << 15)) == 0 {
                asmthumb::op16(&mut emit.as_, 0xbc00 | reglist as u16);
            } else {
                asmthumb::op16(&mut emit.as_, 0xbd00 | (reglist & 0xff) as u16);
            }
        } else if asmthumb::allow_armv7m(&emit.as_) {
            asmthumb::op32(&mut emit.as_, 0xe8bd, reglist);
        } else {
            unknown_op(emit, op_str, 1);
        }
    } else {
        unknown_op(emit, op_str, 1);
    }
}

fn emit_two_args(emit: &mut EmitInlineAsm, op: Qstr, op_str: &[u8], pn_args: *mut ParseNode) {
    if parse::parse_node_is_id(unsafe { *pn_args.add(1) }) {
        if qstr_eq_name(op, b"mov") {
            let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
            let reg_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 15);
            asmthumb::mov_reg_reg(&mut emit.as_, reg_dest as u32, reg_src as u32);
        } else if asmthumb::allow_armv7m(&emit.as_) && (qstr_eq_name(op, b"clz") || qstr_eq_name(op, b"rbit")) {
            let (hi, lo) = if qstr_eq_name(op, b"clz") { (0xfab0, 0xf080) } else { (0xfa90, 0xf0a0) };
            let rd = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
            let rm = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 15);
            asmthumb::op32(&mut emit.as_, hi | (rm as u32), lo | ((rd as u32) << 8) | (rm as u32));
        } else if asmthumb::allow_armv7m(&emit.as_) && qstr_eq_name(op, b"mrs") {
            let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 12);
            let reg_src = get_arg_special_reg(emit, op_str, unsafe { *pn_args.add(1) });
            asmthumb::op32(&mut emit.as_, 0xf3ef, 0x8000 | ((reg_dest as u32) << 8) | (reg_src as u32));
        } else if qstr_eq_name(op, b"and_") {
            let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
            let reg_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
            asmthumb::format_4(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_4_AND, reg_dest as u32, reg_src as u32);
        } else {
            for entry in FORMAT_4_OP_TABLE {
                if op_str.len() >= 3 && op_str[..3] == entry.name[..3] && op_str.get(3).copied().unwrap_or(0) == 0 {
                    let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
                    let reg_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
                    asmthumb::format_4(&mut emit.as_, 0x4000 | ((entry.op as u16) << 4), reg_dest as u32, reg_src as u32);
                    return;
                }
            }
            unknown_op(emit, op_str, 2);
        }
    } else if qstr_eq_name(op, b"mov") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let i8_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xff);
        asmthumb::format_3(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_3_MOV, rlo_dest as u32, i8_src as i32);
    } else if qstr_eq_name(op, b"cmp") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let i8_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xff);
        asmthumb::format_3(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_3_CMP, rlo_dest as u32, i8_src as i32);
    } else if qstr_eq_name(op, b"add") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let i8_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xff);
        asmthumb::format_3(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_3_ADD, rlo_dest as u32, i8_src as i32);
    } else if qstr_eq_name(op, b"sub") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let i8_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xff);
        asmthumb::format_3(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_3_SUB, rlo_dest as u32, i8_src as i32);
    } else if asmthumb::allow_armv7m(&emit.as_) && (qstr_eq_name(op, b"movw") || qstr_eq_name(op, b"movt")) {
        let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        let i_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xffff) as i32;
        let mov_op = if qstr_eq_name(op, b"movw") { asmthumb::ASM_THUMB_OP_MOVW } else { asmthumb::ASM_THUMB_OP_MOVT };
        asmthumb::mov_reg_i16(&mut emit.as_, mov_op, reg_dest as u32, i_src);
    } else if asmthumb::allow_armv7m(&emit.as_) && qstr_eq_name(op, b"movwt") {
        let reg_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        let i_src = get_arg_i(emit, op_str, unsafe { *pn_args.add(1) }, 0xffff_ffff);
        asmthumb::mov_reg_i16(&mut emit.as_, asmthumb::ASM_THUMB_OP_MOVW, reg_dest as u32, (i_src & 0xffff) as i32);
        asmthumb::mov_reg_i16(&mut emit.as_, asmthumb::ASM_THUMB_OP_MOVT, reg_dest as u32, ((i_src >> 16) & 0xffff) as i32);
    } else if asmthumb::allow_armv7m(&emit.as_) && qstr_eq_name(op, b"ldrex") {
        let r_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        if let Some((pn_base, pn_offset)) = get_arg_addr(emit, op_str, unsafe { *pn_args.add(1) }) {
            let r_base = get_arg_reg(emit, op_str, pn_base, 15);
            let i8 = get_arg_i(emit, op_str, pn_offset, 0xff) >> 2;
            asmthumb::op32(&mut emit.as_, 0xe850 | (r_base as u32), 0x0f00 | ((r_dest as u32) << 12) | i8);
        }
    } else {
        for entry in FORMAT_9_10_OP_TABLE {
            if qstr_eq_name(op, entry.name) {
                let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
                if let Some((pn_base, pn_offset)) = get_arg_addr(emit, op_str, unsafe { *pn_args.add(1) }) {
                    let rlo_base = get_arg_reg(emit, op_str, pn_base, 7);
                    let i5 = if entry.op & asmthumb::ASM_THUMB_FORMAT_9_BYTE_TRANSFER != 0 {
                        get_arg_i(emit, op_str, pn_offset, 0x1f)
                    } else if entry.op & asmthumb::ASM_THUMB_FORMAT_10_STRH != 0 {
                        get_arg_i(emit, op_str, pn_offset, 0x3e) >> 1
                    } else {
                        get_arg_i(emit, op_str, pn_offset, 0x7c) >> 2
                    };
                    asmthumb::format_9_10(&mut emit.as_, entry.op, rlo_dest as u32, rlo_base as u32, i5);
                }
                return;
            }
        }
        unknown_op(emit, op_str, 2);
    }
}

fn emit_three_args(emit: &mut EmitInlineAsm, op: Qstr, op_str: &[u8], pn_args: *mut ParseNode) {
    if qstr_eq_name(op, b"lsl") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let rlo_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
        let i5 = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0x1f);
        asmthumb::format_1(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_1_LSL, rlo_dest as u32, rlo_src as u32, i5);
    } else if qstr_eq_name(op, b"lsr") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let rlo_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
        let i5 = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0x1f);
        asmthumb::format_1(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_1_LSR, rlo_dest as u32, rlo_src as u32, i5);
    } else if qstr_eq_name(op, b"asr") {
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let rlo_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
        let i5 = get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0x1f);
        asmthumb::format_1(&mut emit.as_, asmthumb::ASM_THUMB_FORMAT_1_ASR, rlo_dest as u32, rlo_src as u32, i5);
    } else if qstr_eq_name(op, b"add") || qstr_eq_name(op, b"sub") {
        let base_op = if qstr_eq_name(op, b"add") { asmthumb::ASM_THUMB_FORMAT_2_ADD } else { asmthumb::ASM_THUMB_FORMAT_2_SUB };
        let mut op_code = base_op;
        let rlo_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 7);
        let rlo_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 7);
        let src_b = if parse::parse_node_is_id(unsafe { *pn_args.add(2) }) {
            op_code |= asmthumb::ASM_THUMB_FORMAT_2_REG_OPERAND;
            get_arg_reg(emit, op_str, unsafe { *pn_args.add(2) }, 7) as i32
        } else {
            op_code |= asmthumb::ASM_THUMB_FORMAT_2_IMM_OPERAND;
            get_arg_i(emit, op_str, unsafe { *pn_args.add(2) }, 0x7) as i32
        };
        asmthumb::format_2(
            &mut emit.as_,
            op_code as u16,
            rlo_dest as u32,
            rlo_src as u32,
            src_b as u32,
        );
    } else if asmthumb::allow_armv7m(&emit.as_) && (qstr_eq_name(op, b"sdiv") || qstr_eq_name(op, b"udiv")) {
        let op_code = if qstr_eq_name(op, b"sdiv") { 0xfb90 } else { 0xfbb0 };
        let rd = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        let rn = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 15);
        let rm = get_arg_reg(emit, op_str, unsafe { *pn_args.add(2) }, 15);
        asmthumb::op32(&mut emit.as_, op_code | (rn as u32), 0xf0f0 | ((rd as u32) << 8) | (rm as u32));
    } else if asmthumb::allow_armv7m(&emit.as_) && qstr_eq_name(op, b"strex") {
        let r_dest = get_arg_reg(emit, op_str, unsafe { *pn_args }, 15);
        let r_src = get_arg_reg(emit, op_str, unsafe { *pn_args.add(1) }, 15);
        if let Some((pn_base, pn_offset)) = get_arg_addr(emit, op_str, unsafe { *pn_args.add(2) }) {
            let r_base = get_arg_reg(emit, op_str, pn_base, 15);
            let i8 = get_arg_i(emit, op_str, pn_offset, 0xff) >> 2;
            asmthumb::op32(
                &mut emit.as_,
                0xe840 | (r_base as u32),
                ((r_src as u32) << 12) | ((r_dest as u32) << 8) | i8,
            );
        }
    } else {
        unknown_op(emit, op_str, 3);
    }
}

fn unknown_op(emit: &mut EmitInlineAsm, _op_str: &[u8], _n_args: usize) {
    error_exc(emit, objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[objstr::new_str(b"unsupported Thumb instruction")]));
}

fn branch_not_in_range(emit: &mut EmitInlineAsm) {
    error_msg(emit, b"branch not in range");
}
