//! rewrite of py/showbc.c
// symmetry: done

use crate::bc::{self, ModuleConstants};
use crate::bc0;
use crate::emitglue::RawCode;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind, VaArg};
use crate::obj::{self, Obj};
use crate::qstr::{self, Qstr};
use crate::runtime0::{BinaryOp, UnaryOp};

fn ps(print: &Print, s: &str) {
    mpprint::print_str(print, s);
}

fn decode_ulabel(ip: &mut *const u8) -> usize {
    let first = unsafe { **ip };
    if first & 0x80 != 0 {
        let second = unsafe { *ip.add(1) };
        *ip = unsafe { ip.add(2) };
        ((first & 0x7f) as usize) | ((second as usize) << 7)
    } else {
        *ip = unsafe { ip.add(1) };
        first as usize
    }
}

fn decode_slabel(ip: &mut *const u8) -> isize {
    let first = unsafe { **ip };
    if first & 0x80 != 0 {
        let second = unsafe { *ip.add(1) };
        *ip = unsafe { ip.add(2) };
        (((first & 0x7f) as isize) | ((second as isize) << 7)) - 0x4000
    } else {
        *ip = unsafe { ip.add(1) };
        (first as isize) - 0x40
    }
}

fn decode_qstr(ip: &mut *const u8, cm: &ModuleConstants) -> Qstr {
    let idx = bc::decode_uint(ip);
    if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        unsafe { cm.qstr_at(idx) }
    } else {
        idx as Qstr
    }
}

fn child_at(rc: &RawCode, idx: usize) -> *const RawCode {
    if rc.children.is_null() {
        core::ptr::null()
    } else {
        unsafe { *rc.children.add(idx) }
    }
}

fn decode_ptr(ip: &mut *const u8, rc: &RawCode) -> *const RawCode {
    let idx = bc::decode_uint(ip);
    child_at(rc, idx)
}

fn decode_obj(ip: &mut *const u8, cm: &ModuleConstants) -> Obj {
    let idx = bc::decode_uint(ip);
    unsafe { cm.obj_at(idx) }
}

fn unary_op_method_name(op: u8) -> Qstr {
    match op {
        x if x == UnaryOp::Positive as u8 => qstr::from_str("__pos__"),
        x if x == UnaryOp::Negative as u8 => qstr::from_str("__neg__"),
        x if x == UnaryOp::Invert as u8 => qstr::from_str("__invert__"),
        x if x == UnaryOp::Not as u8 => qstr::from_str("__bool__"),
        _ => 0,
    }
}

fn binary_op_method_name(op: u8) -> Qstr {
    match op {
        x if x == BinaryOp::Less as u8 => qstr::from_str("__lt__"),
        x if x == BinaryOp::More as u8 => qstr::from_str("__gt__"),
        x if x == BinaryOp::Equal as u8 => qstr::from_str("__eq__"),
        x if x == BinaryOp::NotEqual as u8 => qstr::from_str("__ne__"),
        x if x == BinaryOp::LessEqual as u8 => qstr::from_str("__le__"),
        x if x == BinaryOp::MoreEqual as u8 => qstr::from_str("__ge__"),
        x if x == BinaryOp::InplaceAdd as u8 => qstr::from_str("__iadd__"),
        x if x == BinaryOp::InplaceSubtract as u8 => qstr::from_str("__isub__"),
        x if x == BinaryOp::Add as u8 => qstr::from_str("__add__"),
        x if x == BinaryOp::Subtract as u8 => qstr::from_str("__sub__"),
        _ => 0,
    }
}

/// `mp_bytecode_print_str`
pub fn bytecode_print_str(
    print: &Print,
    ip_start: *const u8,
    mut ip: *const u8,
    rc: &RawCode,
    cm: &ModuleConstants,
) -> *const u8 {
    if !mpconfig::DEBUG_PRINTERS {
        return ip;
    }

    let opcode = unsafe { *ip };
    ip = unsafe { ip.add(1) };
    match opcode {
        bc0::LOAD_CONST_FALSE => ps(print, "LOAD_CONST_FALSE"),
        bc0::LOAD_CONST_NONE => ps(print, "LOAD_CONST_NONE"),
        bc0::LOAD_CONST_TRUE => ps(print, "LOAD_CONST_TRUE"),
        bc0::LOAD_CONST_SMALL_INT => {
            let mut num: isize = 0;
            if unsafe { *ip } & 0x40 != 0 {
                num -= 1;
            }
            loop {
                let val = unsafe { *ip };
                ip = unsafe { ip.add(1) };
                num = (num << 7) | ((val & 0x7f) as isize);
                if val & 0x80 == 0 {
                    break;
                }
            }
            let _ = mpprint::printf(
                print,
                "LOAD_CONST_SMALL_INT %d",
                std::iter::once(VaArg::Int(num as i32)),
            );
        }
        bc0::LOAD_CONST_STRING => {
            let mut p = ip;
            let qst = decode_qstr(&mut p, cm);
            ip = p;
            let _ = mpprint::printf(
                print,
                "LOAD_CONST_STRING '%q'",
                std::iter::once(VaArg::Qstr(qst)),
            );
        }
        bc0::LOAD_CONST_OBJ => {
            let mut p = ip;
            let val = decode_obj(&mut p, cm);
            ip = p;
            let _ = mpprint::printf(
                print,
                "LOAD_CONST_OBJ %p=",
                std::iter::once(VaArg::USize(obj::as_ptr(val) as usize)),
            );
            obj::print_helper(print, val, PrintKind::Repr);
        }
        bc0::LOAD_NULL => ps(print, "LOAD_NULL"),
        bc0::LOAD_FAST_N
        | bc0::LOAD_DEREF
        | bc0::STORE_FAST_N
        | bc0::STORE_DEREF
        | bc0::DELETE_FAST
        | bc0::DELETE_DEREF => {
            let mut p = ip;
            let unum = bc::decode_uint(&mut p);
            ip = p;
            let name = match opcode {
                bc0::LOAD_FAST_N => "LOAD_FAST_N",
                bc0::LOAD_DEREF => "LOAD_DEREF",
                bc0::STORE_FAST_N => "STORE_FAST_N",
                bc0::STORE_DEREF => "STORE_DEREF",
                bc0::DELETE_FAST => "DELETE_FAST",
                _ => "DELETE_DEREF",
            };
            let _ = mpprint::printf(
                print,
                "%s %u",
                [VaArg::Str(name), VaArg::USize(unum as usize)].into_iter(),
            );
        }
        bc0::LOAD_NAME
        | bc0::LOAD_GLOBAL
        | bc0::LOAD_ATTR
        | bc0::LOAD_METHOD
        | bc0::LOAD_SUPER_METHOD
        | bc0::STORE_NAME
        | bc0::STORE_GLOBAL
        | bc0::STORE_ATTR
        | bc0::DELETE_NAME
        | bc0::DELETE_GLOBAL
        | bc0::IMPORT_NAME
        | bc0::IMPORT_FROM => {
            let mut p = ip;
            let qst = decode_qstr(&mut p, cm);
            ip = p;
            let name = match opcode {
                bc0::LOAD_NAME => "LOAD_NAME",
                bc0::LOAD_GLOBAL => "LOAD_GLOBAL",
                bc0::LOAD_ATTR => "LOAD_ATTR",
                bc0::LOAD_METHOD => "LOAD_METHOD",
                bc0::LOAD_SUPER_METHOD => "LOAD_SUPER_METHOD",
                bc0::STORE_NAME => "STORE_NAME",
                bc0::STORE_GLOBAL => "STORE_GLOBAL",
                bc0::STORE_ATTR => "STORE_ATTR",
                bc0::DELETE_NAME => "DELETE_NAME",
                bc0::DELETE_GLOBAL => "DELETE_GLOBAL",
                bc0::IMPORT_NAME => "IMPORT_NAME",
                _ => "IMPORT_FROM",
            };
            if opcode == bc0::IMPORT_NAME || opcode == bc0::IMPORT_FROM {
                let _ = mpprint::printf(
                    print,
                    "%s '%q'",
                    [VaArg::Str(name), VaArg::Qstr(qst)].into_iter(),
                );
            } else {
                let _ = mpprint::printf(
                    print,
                    "%s %q",
                    [VaArg::Str(name), VaArg::Qstr(qst)].into_iter(),
                );
            }
        }
        bc0::LOAD_BUILD_CLASS => ps(print, "LOAD_BUILD_CLASS"),
        bc0::LOAD_SUBSCR => ps(print, "LOAD_SUBSCR"),
        bc0::STORE_SUBSCR => ps(print, "STORE_SUBSCR"),
        bc0::DUP_TOP => ps(print, "DUP_TOP"),
        bc0::DUP_TOP_TWO => ps(print, "DUP_TOP_TWO"),
        bc0::POP_TOP => ps(print, "POP_TOP"),
        bc0::ROT_TWO => ps(print, "ROT_TWO"),
        bc0::ROT_THREE => ps(print, "ROT_THREE"),
        bc0::JUMP | bc0::POP_JUMP_IF_TRUE | bc0::POP_JUMP_IF_FALSE => {
            let mut p = ip;
            let unum = decode_slabel(&mut p);
            ip = p;
            let target = unsafe { p.offset(unum as isize).offset_from(ip_start) as usize };
            let name = match opcode {
                bc0::JUMP => "JUMP",
                bc0::POP_JUMP_IF_TRUE => "POP_JUMP_IF_TRUE",
                _ => "POP_JUMP_IF_FALSE",
            };
            let _ = mpprint::printf(
                print,
                "%s %u",
                [VaArg::Str(name), VaArg::USize(target as usize)].into_iter(),
            );
        }
        bc0::JUMP_IF_TRUE_OR_POP
        | bc0::JUMP_IF_FALSE_OR_POP
        | bc0::SETUP_WITH
        | bc0::SETUP_EXCEPT
        | bc0::SETUP_FINALLY
        | bc0::FOR_ITER
        | bc0::POP_EXCEPT_JUMP => {
            let mut p = ip;
            let unum = decode_ulabel(&mut p);
            ip = p;
            let target = unsafe { p.offset(unum as isize).offset_from(ip_start) as usize };
            let name = match opcode {
                bc0::JUMP_IF_TRUE_OR_POP => "JUMP_IF_TRUE_OR_POP",
                bc0::JUMP_IF_FALSE_OR_POP => "JUMP_IF_FALSE_OR_POP",
                bc0::SETUP_WITH => "SETUP_WITH",
                bc0::SETUP_EXCEPT => "SETUP_EXCEPT",
                bc0::SETUP_FINALLY => "SETUP_FINALLY",
                bc0::FOR_ITER => "FOR_ITER",
                _ => "POP_EXCEPT_JUMP",
            };
            let _ = mpprint::printf(
                print,
                "%s %u",
                [VaArg::Str(name), VaArg::USize(target as usize)].into_iter(),
            );
        }
        bc0::WITH_CLEANUP => ps(print, "WITH_CLEANUP"),
        bc0::UNWIND_JUMP => {
            let mut p = ip;
            let unum = decode_slabel(&mut p);
            let extra = unsafe { *p };
            ip = unsafe { p.add(1) };
            let target = unsafe { ip.offset(unum as isize).offset_from(ip_start) as usize };
            let _ = mpprint::printf(
                print,
                "UNWIND_JUMP %u %d",
                [VaArg::USize(target as usize), VaArg::Int(extra as i32)].into_iter(),
            );
        }
        bc0::END_FINALLY => ps(print, "END_FINALLY"),
        bc0::GET_ITER => ps(print, "GET_ITER"),
        bc0::GET_ITER_STACK => ps(print, "GET_ITER_STACK"),
        bc0::BUILD_TUPLE
        | bc0::BUILD_LIST
        | bc0::BUILD_MAP
        | bc0::BUILD_SET
        | bc0::BUILD_SLICE
        | bc0::STORE_COMP
        | bc0::UNPACK_SEQUENCE
        | bc0::UNPACK_EX => {
            if opcode == bc0::BUILD_SLICE && !mpconfig::PY_BUILTINS_SLICE {
                ip = unsafe { ip.offset(-1) };
                return bytecode_print_str(print, ip_start, ip, rc, cm);
            }
            let mut p = ip;
            let unum = bc::decode_uint(&mut p);
            ip = p;
            let name = match opcode {
                bc0::BUILD_TUPLE => "BUILD_TUPLE",
                bc0::BUILD_LIST => "BUILD_LIST",
                bc0::BUILD_MAP => "BUILD_MAP",
                bc0::BUILD_SET => "BUILD_SET",
                bc0::BUILD_SLICE => "BUILD_SLICE",
                bc0::STORE_COMP => "STORE_COMP",
                bc0::UNPACK_SEQUENCE => "UNPACK_SEQUENCE",
                _ => "UNPACK_EX",
            };
            let _ = mpprint::printf(
                print,
                "%s %u",
                [VaArg::Str(name), VaArg::USize(unum as usize)].into_iter(),
            );
        }
        bc0::STORE_MAP => ps(print, "STORE_MAP"),
        bc0::MAKE_FUNCTION | bc0::MAKE_FUNCTION_DEFARGS => {
            let mut p = ip;
            let ptr = decode_ptr(&mut p, rc);
            ip = p;
            let name = if opcode == bc0::MAKE_FUNCTION {
                "MAKE_FUNCTION"
            } else {
                "MAKE_FUNCTION_DEFARGS"
            };
            let _ = mpprint::printf(
                print,
                "%s %p",
                [VaArg::Str(name), VaArg::USize(ptr as usize)].into_iter(),
            );
        }
        bc0::MAKE_CLOSURE | bc0::MAKE_CLOSURE_DEFARGS => {
            let mut p = ip;
            let ptr = decode_ptr(&mut p, rc);
            let n_closed = unsafe { *p };
            ip = unsafe { p.add(1) };
            let name = if opcode == bc0::MAKE_CLOSURE {
                "MAKE_CLOSURE"
            } else {
                "MAKE_CLOSURE_DEFARGS"
            };
            let _ = mpprint::printf(
                print,
                "%s %p %u",
                [
                    VaArg::Str(name),
                    VaArg::USize(ptr as usize),
                    VaArg::USize(n_closed as usize),
                ]
                .into_iter(),
            );
        }
        bc0::CALL_FUNCTION
        | bc0::CALL_FUNCTION_VAR_KW
        | bc0::CALL_METHOD
        | bc0::CALL_METHOD_VAR_KW => {
            let mut p = ip;
            let unum = bc::decode_uint(&mut p);
            ip = p;
            let name = match opcode {
                bc0::CALL_FUNCTION => "CALL_FUNCTION",
                bc0::CALL_FUNCTION_VAR_KW => "CALL_FUNCTION_VAR_KW",
                bc0::CALL_METHOD => "CALL_METHOD",
                _ => "CALL_METHOD_VAR_KW",
            };
            let _ = mpprint::printf(
                print,
                "%s n=%u nkw=%u",
                [
                    VaArg::Str(name),
                    VaArg::USize((unum & 0xff) as usize),
                    VaArg::USize(((unum >> 8) & 0xff) as usize),
                ]
                .into_iter(),
            );
        }
        bc0::RETURN_VALUE => ps(print, "RETURN_VALUE"),
        bc0::RAISE_LAST => ps(print, "RAISE_LAST"),
        bc0::RAISE_OBJ => ps(print, "RAISE_OBJ"),
        bc0::RAISE_FROM => ps(print, "RAISE_FROM"),
        bc0::YIELD_VALUE => ps(print, "YIELD_VALUE"),
        bc0::YIELD_FROM => ps(print, "YIELD_FROM"),
        bc0::IMPORT_STAR => ps(print, "IMPORT_STAR"),
        _ => {
            if opcode < bc0::LOAD_CONST_SMALL_INT_MULTI + bc0::LOAD_CONST_SMALL_INT_MULTI_NUM {
                let num = opcode as isize
                    - bc0::LOAD_CONST_SMALL_INT_MULTI as isize
                    - bc0::LOAD_CONST_SMALL_INT_MULTI_EXCESS as isize;
                let _ = mpprint::printf(
                    print,
                    "LOAD_CONST_SMALL_INT %d",
                    std::iter::once(VaArg::Int(num as i32)),
                );
            } else if opcode < bc0::LOAD_FAST_MULTI + bc0::LOAD_FAST_MULTI_NUM {
                let unum = opcode - bc0::LOAD_FAST_MULTI;
                let _ = mpprint::printf(
                    print,
                    "LOAD_FAST %u",
                    std::iter::once(VaArg::USize(unum as usize)),
                );
            } else if opcode < bc0::STORE_FAST_MULTI + bc0::STORE_FAST_MULTI_NUM {
                let unum = opcode - bc0::STORE_FAST_MULTI;
                let _ = mpprint::printf(
                    print,
                    "STORE_FAST %u",
                    std::iter::once(VaArg::USize(unum as usize)),
                );
            } else if opcode < bc0::UNARY_OP_MULTI + bc0::UNARY_OP_MULTI_NUM {
                let op = opcode - bc0::UNARY_OP_MULTI;
                let _ = mpprint::printf(
                    print,
                    "UNARY_OP %u %q",
                    [
                        VaArg::USize(op as usize),
                        VaArg::Qstr(unary_op_method_name(op)),
                    ]
                    .into_iter(),
                );
            } else if opcode < bc0::BINARY_OP_MULTI + bc0::BINARY_OP_MULTI_NUM {
                let op = opcode - bc0::BINARY_OP_MULTI;
                let _ = mpprint::printf(
                    print,
                    "BINARY_OP %u %q",
                    [
                        VaArg::USize(op as usize),
                        VaArg::Qstr(binary_op_method_name(op)),
                    ]
                    .into_iter(),
                );
            } else {
                let _ = mpprint::printf(
                    print,
                    "code %p, byte code 0x%02x not implemented\n",
                    [
                        VaArg::USize(unsafe { ip.offset(-1) } as usize),
                        VaArg::USize(opcode as usize),
                    ]
                    .into_iter(),
                );
                debug_assert!(false, "unimplemented opcode 0x{opcode:02x}");
                return ip;
            }
        }
    }
    ip
}

/// `mp_bytecode_print2`
pub fn bytecode_print2(
    print: &Print,
    ip: *const u8,
    len: usize,
    rc: &RawCode,
    cm: &ModuleConstants,
) {
    if !mpconfig::DEBUG_PRINTERS {
        return;
    }
    let ip_start = ip;
    let mut cur = ip;
    let end = unsafe { ip.add(len) };
    while cur < end {
        let offset = unsafe { cur.offset_from(ip_start) as usize };
        let _ = mpprint::printf(
            print,
            "%02u ",
            std::iter::once(VaArg::USize(offset as usize)),
        );
        cur = bytecode_print_str(print, ip_start, cur, rc, cm);
        mpprint::print_str(print, "\n");
    }
}

/// `mp_bytecode_print`
pub fn bytecode_print(print: &Print, rc: &RawCode, fun_data_len: usize, cm: &ModuleConstants) {
    if !mpconfig::DEBUG_PRINTERS {
        return;
    }

    let ip_start = rc.fun_data;
    let mut ip = ip_start;
    let sig = bc::prelude_sig_decode_into(&mut ip);
    let (n_info, n_cell) = bc::prelude_size_decode(&mut ip);
    let code_info = ip;

    let mut ci = code_info;
    let block_name_idx = bc::decode_uint(&mut ci);
    let block_name = if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        unsafe { cm.qstr_at(block_name_idx) }
    } else {
        block_name_idx as Qstr
    };
    let source_file = if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        unsafe { cm.qstr_at(0) }
    } else {
        0
    };

    let _ = mpprint::printf(
        print,
        "File %q, code block '%q' (descriptor: %p, bytecode @%p %u bytes)\n",
        [
            VaArg::Qstr(source_file),
            VaArg::Qstr(block_name),
            VaArg::USize(rc as *const _ as usize),
            VaArg::USize(ip_start as usize),
            VaArg::USize(fun_data_len as usize),
        ]
        .into_iter(),
    );

    let prelude_size = unsafe { ip.offset_from(ip_start) as usize } + n_info + n_cell;
    let _ = mpprint::printf(
        print,
        "Raw bytecode (code_info_size=%u, bytecode_size=%u):\n",
        [
            VaArg::USize(prelude_size as usize),
            VaArg::USize((fun_data_len - prelude_size) as usize),
        ]
        .into_iter(),
    );
    for i in 0..fun_data_len {
        if i > 0 && i % 16 == 0 {
            mpprint::print_str(print, "\n");
        }
        let byte = unsafe { *ip_start.add(i) };
        let _ = mpprint::printf(print, " %02x", std::iter::once(VaArg::USize(byte as usize)));
    }
    mpprint::print_str(print, "\n");

    mpprint::print_str(print, "arg names:");
    for _ in 0..sig.n_pos_args + sig.n_kwonly_args {
        let mut p = ci;
        let q_idx = bc::decode_uint(&mut p);
        let qst = if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
            unsafe { cm.qstr_at(q_idx) }
        } else {
            q_idx as Qstr
        };
        ci = p;
        let _ = mpprint::printf(print, " %q", std::iter::once(VaArg::Qstr(qst)));
    }
    mpprint::print_str(print, "\n");

    let _ = mpprint::printf(
        print,
        "(N_STATE %u)\n",
        std::iter::once(VaArg::USize(sig.n_state as usize)),
    );
    let _ = mpprint::printf(
        print,
        "(N_EXC_STACK %u)\n",
        std::iter::once(VaArg::USize(sig.n_exc_stack as usize)),
    );

    ip = unsafe { ip.add(n_info) };
    let line_info_top = ip;

    let mut cell_ip = ip;
    for _ in 0..n_cell {
        let local_num = unsafe { *cell_ip };
        cell_ip = unsafe { cell_ip.add(1) };
        let _ = mpprint::printf(
            print,
            "(INIT_CELL %u)\n",
            std::iter::once(VaArg::USize(local_num as usize)),
        );
    }

    let mut bc = 0usize;
    let mut source_line = 1usize;
    let _ = mpprint::printf(
        print,
        "  bc=%d line=%u\n",
        [VaArg::Int(bc as i32), VaArg::USize(source_line as usize)].into_iter(),
    );
    let mut li = code_info;
    while li < line_info_top {
        let mut p = li;
        let decoded = bc::decode_lineinfo(&mut p);
        bc += decoded.bc_increment;
        source_line += decoded.line_increment;
        li = p;
        let _ = mpprint::printf(
            print,
            "  bc=%d line=%u\n",
            [VaArg::Int(bc as i32), VaArg::USize(source_line as usize)].into_iter(),
        );
    }

    bytecode_print2(print, cell_ip, fun_data_len - prelude_size, rc, cm);
}
