//! rewrite of py/emitbc.c
// symmetry: done

use crate::bc::{encode_uint, ENCODE_UINT_MAX_BYTES};
use crate::bc0::{self, SCOPE_FLAG_ALL_SIG, SCOPE_FLAG_GENERATOR};
use crate::emit::{
    self, EmitCommon, PassKind, EMIT_ATTR_DELETE, EMIT_ATTR_LOAD, EMIT_ATTR_STORE, EMIT_BUILD_LIST,
    EMIT_BUILD_MAP, EMIT_BUILD_SET, EMIT_BUILD_SLICE, EMIT_BUILD_TUPLE, EMIT_IDOP_GLOBAL_GLOBAL,
    EMIT_IDOP_GLOBAL_NAME, EMIT_IDOP_LOCAL_DEREF, EMIT_IDOP_LOCAL_FAST, EMIT_IMPORT_FROM,
    EMIT_IMPORT_NAME, EMIT_SETUP_BLOCK_WITH, EMIT_SUBSCR_DELETE, EMIT_SUBSCR_LOAD,
    EMIT_SUBSCR_STORE, EMIT_YIELD_FROM, EMIT_YIELD_VALUE,
};
use crate::emitglue;
use crate::lexer::TokenKind;
use crate::malloc;
use crate::mpconfig;
use crate::mpstate;
use crate::obj::{self, Obj};
use crate::objsingleton;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::scope::{IdInfoKind, Scope, ScopeKind};
use crate::smallint;

const DUMMY_DATA_SIZE: usize = ENCODE_UINT_MAX_BYTES;

pub struct EmitBc {
    dummy_data: [u8; DUMMY_DATA_SIZE],
    pass: PassKind,
    suppress: bool,
    stack_size: i32,
    emit_common: *mut EmitCommon,
    scope: *mut Scope,
    last_source_line_offset: usize,
    last_source_line: usize,
    max_num_labels: usize,
    label_offsets: Vec<usize>,
    code_info_offset: usize,
    code_info_size: usize,
    bytecode_offset: usize,
    bytecode_size: usize,
    code_base: Vec<u8>,
    overflow: bool,
    n_info: usize,
    n_cell: usize,
}

fn emit_ref(emit: *mut crate::emit::Emit) -> *mut EmitBc {
    emit as *mut EmitBc
}

fn emit_ref_mut(emit: *mut crate::emit::Emit) -> &'static mut EmitBc {
    unsafe { &mut *emit_ref(emit) }
}

fn get_cur_to_write_code_info(emit: &mut EmitBc, num_bytes: usize) -> *mut u8 {
    if (emit.pass as u8) < PassKind::Emit as u8 {
        emit.code_info_offset += num_bytes;
        emit.dummy_data.as_mut_ptr()
    } else {
        debug_assert!(emit.code_info_offset + num_bytes <= emit.code_info_size);
        let c = emit.code_info_offset;
        emit.code_info_offset += num_bytes;
        unsafe { emit.code_base.as_mut_ptr().add(c) }
    }
}

fn write_code_info_byte(emit: &mut EmitBc, val: u8) {
    unsafe {
        *get_cur_to_write_code_info(emit, 1) = val;
    }
}

fn write_code_info_qstr(emit: &mut EmitBc, qst: Qstr) {
    let idx = emit::emit_common_use_qstr(unsafe { &mut *emit.emit_common }, qst);
    encode_uint(
        &mut |b| unsafe { *get_cur_to_write_code_info(emit, 1) = b },
        idx,
    );
}

fn get_cur_to_write_bytecode(emit: &mut EmitBc, num_bytes: usize) -> *mut u8 {
    if emit.suppress {
        return emit.dummy_data.as_mut_ptr();
    }
    if (emit.pass as u8) < PassKind::Emit as u8 {
        emit.bytecode_offset += num_bytes;
        emit.dummy_data.as_mut_ptr()
    } else {
        debug_assert!(emit.bytecode_offset + num_bytes <= emit.bytecode_size);
        let c = emit.code_info_size + emit.bytecode_offset;
        emit.bytecode_offset += num_bytes;
        unsafe { emit.code_base.as_mut_ptr().add(c) }
    }
}

fn write_bytecode_raw_byte(emit: &mut EmitBc, b1: u8) {
    unsafe {
        *get_cur_to_write_bytecode(emit, 1) = b1;
    }
}

pub fn adjust_stack_size(emit: *mut crate::emit::Emit, delta: i32) {
    let emit = emit_ref_mut(emit);
    if emit.pass == PassKind::Scope {
        return;
    }
    debug_assert!(emit.stack_size + delta >= 0);
    emit.stack_size += delta;
    unsafe {
        if emit.stack_size as u16 > (*emit.scope).stack_size {
            (*emit.scope).stack_size = emit.stack_size as u16;
        }
    }
}

fn write_bytecode_byte(emit: &mut EmitBc, stack_adj: i32, b1: u8) {
    if emit.pass != PassKind::Scope {
        debug_assert!(emit.stack_size + stack_adj >= 0);
        emit.stack_size += stack_adj;
        unsafe {
            if emit.stack_size as u16 > (*emit.scope).stack_size {
                (*emit.scope).stack_size = emit.stack_size as u16;
            }
        }
    }
    write_bytecode_raw_byte(emit, b1);
}

fn write_bytecode_byte_int(emit: &mut EmitBc, stack_adj: i32, b1: u8, mut num: i64) {
    write_bytecode_byte(emit, stack_adj, b1);
    let mut buf = [0u8; ENCODE_UINT_MAX_BYTES];
    let mut p = buf.len();
    loop {
        p -= 1;
        buf[p] = (num & 0x7f) as u8;
        num >>= 7;
        if num == 0 || num == -1 {
            break;
        }
    }
    if num == -1 && (buf[p] & 0x40) == 0 {
        p -= 1;
        buf[p] = 0x7f;
    } else if num == 0 && (buf[p] & 0x40) != 0 {
        p -= 1;
        buf[p] = 0;
    }
    let len = buf.len() - p;
    let c = get_cur_to_write_bytecode(emit, len);
    unsafe {
        for i in 0..len - 1 {
            *c.add(i) = buf[p + i] | 0x80;
        }
        *c.add(len - 1) = buf[p + len - 1];
    }
}

fn write_bytecode_byte_uint(emit: &mut EmitBc, stack_adj: i32, b: u8, val: usize) {
    write_bytecode_byte(emit, stack_adj, b);
    encode_uint(
        &mut |byte| unsafe { *get_cur_to_write_bytecode(emit, 1) = byte },
        val,
    );
}

fn write_bytecode_byte_const(emit: &mut EmitBc, stack_adj: i32, b: u8, n: usize) {
    write_bytecode_byte_uint(emit, stack_adj, b, n);
}

fn write_bytecode_byte_qstr(emit: &mut EmitBc, stack_adj: i32, b: u8, qst: Qstr) {
    let idx = emit::emit_common_use_qstr(unsafe { &mut *emit.emit_common }, qst);
    write_bytecode_byte_uint(emit, stack_adj, b, idx);
}

fn write_bytecode_byte_obj(emit: &mut EmitBc, stack_adj: i32, b: u8, obj_val: Obj) {
    let n = emit::emit_common_use_const_obj(unsafe { &mut *emit.emit_common }, obj_val);
    write_bytecode_byte_const(emit, stack_adj, b, n);
}

fn write_bytecode_byte_child(emit: &mut EmitBc, stack_adj: i32, b: u8, rc: *mut emitglue::RawCode) {
    let n = emit::emit_common_alloc_const_child(unsafe { &mut *emit.emit_common }, rc);
    write_bytecode_byte_const(emit, stack_adj, b, n);
}

fn write_bytecode_byte_label(emit: &mut EmitBc, stack_adj: i32, b1: u8, label: usize) {
    if emit.pass != PassKind::Scope {
        debug_assert!(emit.stack_size + stack_adj >= 0);
        emit.stack_size += stack_adj;
        unsafe {
            if emit.stack_size as u16 > (*emit.scope).stack_size {
                (*emit.scope).stack_size = emit.stack_size as u16;
            }
        }
    }
    if emit.suppress {
        return;
    }
    let is_signed = b1 <= bc0::POP_JUMP_IF_FALSE;
    let mut jump_encoding_size = 1usize;
    let mut bytecode_offset: isize = 0;
    if (emit.pass as u8) >= PassKind::CodeSize as u8 {
        bytecode_offset = emit.label_offsets[label] as isize - emit.bytecode_offset as isize - 2;
        if (is_signed && -64 <= bytecode_offset && bytecode_offset <= 63)
            || (!is_signed && bytecode_offset >= 0 && bytecode_offset as usize <= 127)
        {
            jump_encoding_size = 0;
        }
        bytecode_offset -= jump_encoding_size as isize;
        debug_assert!(is_signed || bytecode_offset >= 0);
    }
    let total = 2 + jump_encoding_size;
    let c = get_cur_to_write_bytecode(emit, total);
    unsafe {
        *c = b1;
        if jump_encoding_size == 0 {
            let mut off = bytecode_offset;
            if is_signed {
                off += 0x40;
            }
            debug_assert!(0 <= off && off <= 0x7f);
            *c.add(1) = off as u8;
        } else {
            let mut off = bytecode_offset;
            if is_signed {
                off += 0x4000;
            }
            if emit.pass == PassKind::Emit && !(0 <= off && off <= 0x7fff) {
                emit.overflow = true;
            }
            *c.add(1) = 0x80 | (off as u8 & 0x7f);
            *c.add(2) = (off >> 7) as u8;
        }
    }
}

fn prelude_sig_encode(emit: &mut EmitBc, scope: &Scope) {
    let mut s = scope.num_locals as usize + scope.stack_size as usize;
    if s == 0 {
        s = 1;
    }
    if mpconfig::DEBUG_VM_STACK_OVERFLOW != 0 {
        s += 1;
    }
    let mut e = scope.exc_stack_size as usize;
    let mut f = scope.scope_flags as usize & SCOPE_FLAG_ALL_SIG as usize;
    let mut a = scope.num_pos_args as usize;
    let mut k = scope.num_kwonly_args as usize;
    let mut d = scope.num_def_pos_args as usize;
    s -= 1;
    let mut z: u8 = ((s & 0xf) << 3) as u8 | ((e & 1) << 2) as u8 | (a & 3) as u8;
    s >>= 4;
    e >>= 1;
    a >>= 2;
    while s | e | f | a | k | d != 0 {
        write_code_info_byte(emit, 0x80 | z);
        z = ((f & 1) << 6) as u8
            | ((s & 3) << 4) as u8
            | ((k & 1) << 3) as u8
            | ((a & 1) << 2) as u8
            | ((e & 1) << 1) as u8
            | (d & 1) as u8;
        s >>= 2;
        e >>= 1;
        f >>= 1;
        a >>= 1;
        k >>= 1;
        d >>= 1;
    }
    write_code_info_byte(emit, z);
}

fn prelude_size_encode(emit: &mut EmitBc, mut i: usize, mut c: usize) {
    loop {
        let mut z = ((i & 0x3f) << 1) | (c & 1);
        c >>= 1;
        i >>= 6;
        if c | i != 0 {
            z |= 0x80;
        }
        write_code_info_byte(emit, z as u8);
        if c | i == 0 {
            break;
        }
    }
}

pub fn new(emit_common: *mut EmitCommon) -> *mut crate::emit::Emit {
    let emit = malloc::new_obj::<EmitBc>().expect("emit bc alloc");
    unsafe {
        (*emit_ref(emit as *mut _)).emit_common = emit_common;
        (*emit_ref(emit as *mut _)).label_offsets = Vec::new();
        (*emit_ref(emit as *mut _)).code_base = Vec::new();
    }
    emit as *mut crate::emit::Emit
}

pub fn set_max_num_labels(emit: *mut crate::emit::Emit, max_num_labels: usize) {
    let e = emit_ref_mut(emit);
    e.max_num_labels = max_num_labels;
    e.label_offsets = vec![0; max_num_labels];
}

pub fn free(emit: *mut crate::emit::Emit) {
    if !emit.is_null() {
        malloc::del_obj(emit_ref(emit));
    }
}

pub fn start_pass(emit: *mut crate::emit::Emit, pass: PassKind, scope: *mut Scope) {
    let emit = emit_ref_mut(emit);
    emit.pass = pass;
    emit.stack_size = 0;
    emit.suppress = false;
    emit.scope = scope;
    emit.last_source_line_offset = 0;
    emit.last_source_line = 1;
    emit.bytecode_offset = 0;
    emit.code_info_offset = 0;
    emit.overflow = false;

    unsafe {
        prelude_sig_encode(emit, &*scope);
    }

    if (pass as u8) >= PassKind::CodeSize as u8 {
        prelude_size_encode(emit, emit.n_info, emit.n_cell);
    }

    emit.n_info = emit.code_info_offset;

    unsafe {
        write_code_info_qstr(emit, (*scope).simple_name);
        for i in 0..(*scope).num_pos_args as usize + (*scope).num_kwonly_args as usize {
            let mut qst = qstr::from_str("*");
            for id in &(*scope).id_info {
                if id.flags & crate::scope::ID_FLAG_IS_PARAM != 0 && id.local_num as usize == i {
                    qst = id.qst;
                    break;
                }
            }
            write_code_info_qstr(emit, qst);
        }
    }
}

pub fn end_pass(emit: *mut crate::emit::Emit) -> bool {
    let emit = emit_ref_mut(emit);
    if emit.pass == PassKind::Scope {
        return true;
    }
    debug_assert!(emit.stack_size == 0);
    emit.n_info = emit.code_info_offset - emit.n_info;
    emit.n_cell = 0;
    unsafe {
        for id in &(*emit.scope).id_info {
            if id.kind == IdInfoKind::Cell {
                debug_assert!(id.local_num <= 255);
                write_code_info_byte(emit, id.local_num as u8);
                emit.n_cell += 1;
            }
        }
    }

    if emit.pass == PassKind::CodeSize {
        emit.code_info_size = emit.code_info_offset;
        emit.bytecode_size = emit.bytecode_offset;
        emit.code_base = vec![0; emit.code_info_size + emit.bytecode_size];
    } else if emit.pass == PassKind::Emit {
        debug_assert!(emit.code_info_offset <= emit.code_info_size);
        debug_assert!(emit.bytecode_offset <= emit.bytecode_size);
        if emit.code_info_offset != emit.code_info_size
            || emit.bytecode_offset != emit.bytecode_size
        {
            emit.code_info_size = emit.code_info_offset;
            emit.bytecode_size = emit.bytecode_offset;
            return false;
        }
        if emit.overflow {
            raise::raise(MpRaise::RuntimeError("bytecode overflow"));
        }
        unsafe {
            let len = emit.code_info_size + emit.bytecode_size;
            let permanent = malloc::new::<u8>(len).expect("bytecode alloc");
            std::ptr::copy_nonoverlapping(emit.code_base.as_ptr(), permanent, len);
            emitglue::assign_bytecode_ex(
                (*emit.scope).raw_code,
                permanent,
                (*emit.emit_common).children,
                (*emit.scope).scope_flags,
                len as u32,
                0,
            );
            #[cfg(debug_assertions)]
            {
                (*emit.scope).raw_code_data_len = len;
            }
        }
    }
    true
}

pub fn set_source_line(emit: *mut crate::emit::Emit, source_line: usize) {
    if !mpconfig::ENABLE_SOURCE_LINE {
        return;
    }
    let emit = emit_ref_mut(emit);
    if mpstate::with_vm(|vm| vm.mp_optimise_value) >= 3 {
        return;
    }
    if source_line > emit.last_source_line {
        let bytes_to_skip = emit.bytecode_offset - emit.last_source_line_offset;
        let lines_to_skip = source_line - emit.last_source_line;
        write_source_lines(emit, bytes_to_skip, lines_to_skip);
        emit.last_source_line_offset = emit.bytecode_offset;
        emit.last_source_line = source_line;
    }
}

fn write_source_lines(emit: &mut EmitBc, mut bytes_to_skip: usize, mut lines_to_skip: usize) {
    while bytes_to_skip > 0 || lines_to_skip > 0 {
        let (b, l, two): (usize, usize, bool) = if lines_to_skip <= 6 || bytes_to_skip > 0xf {
            let b = bytes_to_skip.min(0x1f);
            let l = if b < bytes_to_skip {
                0
            } else {
                lines_to_skip.min(0x3)
            };
            (b, l, false)
        } else {
            (bytes_to_skip.min(0xf), lines_to_skip.min(0x7ff), true)
        };
        if two {
            let ci = get_cur_to_write_code_info(emit, 2);
            unsafe {
                *ci = 0x80 | (b as u8) | (((l >> 4) & 0x70) as u8);
                *ci.add(1) = l as u8;
            }
        } else {
            unsafe {
                *get_cur_to_write_code_info(emit, 1) = (b as u8) | ((l as u8) << 5);
            }
        }
        bytes_to_skip -= b;
        lines_to_skip -= l;
    }
}

pub fn label_assign(emit: *mut crate::emit::Emit, l: usize) {
    let emit = emit_ref_mut(emit);
    emit.suppress = false;
    if emit.pass == PassKind::Scope {
        return;
    }
    debug_assert!(l < emit.max_num_labels);
    emit.label_offsets[l] = emit.bytecode_offset;
}

pub fn import(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
    let e = emit_ref_mut(emit);
    let stack_adj = if kind == EMIT_IMPORT_FROM { 1 } else { -1 };
    if kind == crate::emit::EMIT_IMPORT_STAR {
        write_bytecode_byte(e, stack_adj, bc0::IMPORT_STAR);
    } else {
        write_bytecode_byte_qstr(e, stack_adj, bc0::IMPORT_NAME + kind as u8, qst);
    }
}

pub fn load_const_tok(emit: *mut crate::emit::Emit, tok: TokenKind) {
    let e = emit_ref_mut(emit);
    if tok == TokenKind::Ellipsis {
        write_bytecode_byte_obj(e, 1, bc0::LOAD_CONST_OBJ, objsingleton::const_ellipsis());
    } else {
        write_bytecode_byte(
            e,
            1,
            bc0::LOAD_CONST_FALSE + (tok as u8 - TokenKind::KwFalse as u8),
        );
    }
}

pub fn load_const_small_int(emit: *mut crate::emit::Emit, arg: i64) {
    let e = emit_ref_mut(emit);
    debug_assert!(smallint::fits(arg as crate::obj::Int));
    let excess = bc0::LOAD_CONST_SMALL_INT_MULTI_EXCESS as i64;
    if -excess <= arg && arg < bc0::LOAD_CONST_SMALL_INT_MULTI_NUM as i64 - excess {
        // `arg` may be negative (e.g. -1); do the addition in a wide signed
        // type like C's `mp_int_t` arithmetic (which implicitly truncates to
        // `byte` at the call boundary) instead of casting `arg` to `u8`
        // first, which would wrap a small negative number into a huge
        // positive one and overflow the subsequent `u8` addition.
        let byte_val = bc0::LOAD_CONST_SMALL_INT_MULTI as i64
            + bc0::LOAD_CONST_SMALL_INT_MULTI_EXCESS as i64
            + arg;
        write_bytecode_byte(e, 1, byte_val as u8);
    } else {
        write_bytecode_byte_int(e, 1, bc0::LOAD_CONST_SMALL_INT, arg);
    }
}

pub fn load_const_str(emit: *mut crate::emit::Emit, qst: Qstr) {
    write_bytecode_byte_qstr(emit_ref_mut(emit), 1, bc0::LOAD_CONST_STRING, qst);
}

pub fn load_const_obj(emit: *mut crate::emit::Emit, obj_val: Obj) {
    write_bytecode_byte_obj(emit_ref_mut(emit), 1, bc0::LOAD_CONST_OBJ, obj_val);
}

pub fn load_null(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 1, bc0::LOAD_NULL);
}

pub fn load_local(emit: *mut crate::emit::Emit, _qst: Qstr, local_num: usize, kind: i32) {
    let e = emit_ref_mut(emit);
    if kind == EMIT_IDOP_LOCAL_FAST && local_num <= 15 {
        write_bytecode_byte(e, 1, bc0::LOAD_FAST_MULTI + local_num as u8);
    } else {
        write_bytecode_byte_uint(e, 1, bc0::LOAD_FAST_N + kind as u8, local_num);
    }
}

pub fn load_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
    write_bytecode_byte_qstr(emit_ref_mut(emit), 1, bc0::LOAD_NAME + kind as u8, qst);
}

pub fn load_method(emit: *mut crate::emit::Emit, qst: Qstr, is_super: bool) {
    let stack_adj = 1 - 2 * is_super as i32;
    let op = if is_super {
        bc0::LOAD_SUPER_METHOD
    } else {
        bc0::LOAD_METHOD
    };
    write_bytecode_byte_qstr(emit_ref_mut(emit), stack_adj, op, qst);
}

pub fn load_build_class(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 1, bc0::LOAD_BUILD_CLASS);
}

pub fn subscr(emit: *mut crate::emit::Emit, kind: i32) {
    let e = emit_ref_mut(emit);
    if kind == EMIT_SUBSCR_LOAD {
        write_bytecode_byte(e, -1, bc0::LOAD_SUBSCR);
    } else {
        if kind == EMIT_SUBSCR_DELETE {
            load_null(emit);
            rot_three(emit);
        }
        write_bytecode_byte(e, -3, bc0::STORE_SUBSCR);
    }
}

pub fn attr(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
    let e = emit_ref_mut(emit);
    if kind == EMIT_ATTR_LOAD {
        write_bytecode_byte_qstr(e, 0, bc0::LOAD_ATTR, qst);
    } else {
        if kind == EMIT_ATTR_DELETE {
            load_null(emit);
            rot_two(emit);
        }
        write_bytecode_byte_qstr(e, -2, bc0::STORE_ATTR, qst);
    }
}

pub fn store_local(emit: *mut crate::emit::Emit, _qst: Qstr, local_num: usize, kind: i32) {
    let e = emit_ref_mut(emit);
    if kind == EMIT_IDOP_LOCAL_FAST && local_num <= 15 {
        write_bytecode_byte(e, -1, bc0::STORE_FAST_MULTI + local_num as u8);
    } else {
        write_bytecode_byte_uint(e, -1, bc0::STORE_FAST_N + kind as u8, local_num);
    }
}

pub fn store_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
    write_bytecode_byte_qstr(emit_ref_mut(emit), -1, bc0::STORE_NAME + kind as u8, qst);
}

pub fn delete_local(emit: *mut crate::emit::Emit, _qst: Qstr, local_num: usize, kind: i32) {
    write_bytecode_byte_uint(
        emit_ref_mut(emit),
        0,
        bc0::DELETE_FAST + kind as u8,
        local_num,
    );
}

pub fn delete_global(emit: *mut crate::emit::Emit, qst: Qstr, kind: i32) {
    write_bytecode_byte_qstr(emit_ref_mut(emit), 0, bc0::DELETE_NAME + kind as u8, qst);
}

pub fn dup_top(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 1, bc0::DUP_TOP);
}

pub fn dup_top_two(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 2, bc0::DUP_TOP_TWO);
}

pub fn pop_top(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), -1, bc0::POP_TOP);
}

pub fn rot_two(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 0, bc0::ROT_TWO);
}

pub fn rot_three(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), 0, bc0::ROT_THREE);
}

pub fn jump(emit: *mut crate::emit::Emit, label: usize) {
    write_bytecode_byte_label(emit_ref_mut(emit), 0, bc0::JUMP, label);
    emit_ref_mut(emit).suppress = true;
}

pub fn pop_jump_if(emit: *mut crate::emit::Emit, cond: bool, label: usize) {
    let op = if cond {
        bc0::POP_JUMP_IF_TRUE
    } else {
        bc0::POP_JUMP_IF_FALSE
    };
    write_bytecode_byte_label(emit_ref_mut(emit), -1, op, label);
}

pub fn jump_if_or_pop(emit: *mut crate::emit::Emit, cond: bool, label: usize) {
    let op = if cond {
        bc0::JUMP_IF_TRUE_OR_POP
    } else {
        bc0::JUMP_IF_FALSE_OR_POP
    };
    write_bytecode_byte_label(emit_ref_mut(emit), -1, op, label);
}

pub fn unwind_jump(emit: *mut crate::emit::Emit, label: usize, except_depth: usize) {
    let e = emit_ref_mut(emit);
    let break_for = label & crate::emit::EMIT_BREAK_FROM_FOR as usize != 0;
    let label = label & !crate::emit::EMIT_BREAK_FROM_FOR as usize;
    if except_depth == 0 {
        if break_for {
            write_bytecode_raw_byte(e, bc0::POP_TOP);
            for _ in 0..obj::ITER_BUF_NSLOTS - 1 {
                write_bytecode_raw_byte(e, bc0::POP_TOP);
            }
        }
        write_bytecode_byte_label(e, 0, bc0::JUMP, label);
    } else {
        write_bytecode_byte_label(e, 0, bc0::UNWIND_JUMP, label);
        write_bytecode_raw_byte(e, ((if break_for { 0x80 } else { 0 }) | except_depth) as u8);
    }
    e.suppress = true;
}

pub fn setup_block(emit: *mut crate::emit::Emit, label: usize, kind: i32) {
    let stack_adj = if kind == EMIT_SETUP_BLOCK_WITH { 2 } else { 0 };
    write_bytecode_byte_label(
        emit_ref_mut(emit),
        stack_adj,
        bc0::SETUP_WITH + kind as u8,
        label,
    );
}

pub fn with_cleanup(emit: *mut crate::emit::Emit, label: usize) {
    load_const_tok(emit, TokenKind::KwNone);
    label_assign(emit, label);
    write_bytecode_byte(emit_ref_mut(emit), 2, bc0::WITH_CLEANUP);
    adjust_stack_size(emit, -4);
}

pub fn end_finally(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), -1, bc0::END_FINALLY);
}

pub fn get_iter(emit: *mut crate::emit::Emit, use_stack: bool) {
    let stack_adj = if use_stack {
        obj::ITER_BUF_NSLOTS as i32 - 1
    } else {
        0
    };
    let op = if use_stack {
        bc0::GET_ITER_STACK
    } else {
        bc0::GET_ITER
    };
    write_bytecode_byte(emit_ref_mut(emit), stack_adj, op);
}

pub fn for_iter(emit: *mut crate::emit::Emit, label: usize) {
    write_bytecode_byte_label(emit_ref_mut(emit), 1, bc0::FOR_ITER, label);
}

pub fn for_iter_end(emit: *mut crate::emit::Emit) {
    adjust_stack_size(emit, -(obj::ITER_BUF_NSLOTS as i32));
}

pub fn pop_except_jump(emit: *mut crate::emit::Emit, label: usize, _within: bool) {
    write_bytecode_byte_label(emit_ref_mut(emit), 0, bc0::POP_EXCEPT_JUMP, label);
    emit_ref_mut(emit).suppress = true;
}

pub fn unary_op(emit: *mut crate::emit::Emit, op: UnaryOp) {
    write_bytecode_byte(emit_ref_mut(emit), 0, bc0::UNARY_OP_MULTI + op as u8);
}

pub fn binary_op(emit: *mut crate::emit::Emit, op: BinaryOp) {
    let e = emit_ref_mut(emit);
    let (op, invert) = match op {
        BinaryOp::NotIn => (BinaryOp::In, true),
        BinaryOp::IsNot => (BinaryOp::Is, true),
        other => (other, false),
    };
    write_bytecode_byte(e, -1, bc0::BINARY_OP_MULTI + op as u8);
    if invert {
        write_bytecode_byte(e, 0, bc0::UNARY_OP_MULTI + UnaryOp::Not as u8);
    }
}

pub fn build(emit: *mut crate::emit::Emit, n_args: usize, kind: i32) {
    let stack_adj = if kind == EMIT_BUILD_MAP {
        1
    } else {
        1 - n_args as i32
    };
    write_bytecode_byte_uint(
        emit_ref_mut(emit),
        stack_adj,
        bc0::BUILD_TUPLE + kind as u8,
        n_args,
    );
}

pub fn store_map(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), -2, bc0::STORE_MAP);
}

pub fn store_comp(emit: *mut crate::emit::Emit, kind: ScopeKind, collection_stack_index: usize) {
    let (n, t) = match kind {
        ScopeKind::ListComp => (0usize, 0usize),
        ScopeKind::DictComp => (1, 1),
        ScopeKind::SetComp => (0, 2),
        _ => (1, 1),
    };
    write_bytecode_byte_uint(
        emit_ref_mut(emit),
        -1 - n as i32,
        bc0::STORE_COMP,
        ((collection_stack_index + n) << 2) | t,
    );
}

pub fn unpack_sequence(emit: *mut crate::emit::Emit, n_args: usize) {
    write_bytecode_byte_uint(
        emit_ref_mut(emit),
        -1 + n_args as i32,
        bc0::UNPACK_SEQUENCE,
        n_args,
    );
}

pub fn unpack_ex(emit: *mut crate::emit::Emit, n_left: usize, n_right: usize) {
    write_bytecode_byte_uint(
        emit_ref_mut(emit),
        -1 + n_left as i32 + n_right as i32 + 1,
        bc0::UNPACK_EX,
        n_left | (n_right << 8),
    );
}

pub fn make_function(
    emit: *mut crate::emit::Emit,
    scope: *mut Scope,
    n_pos_defaults: usize,
    n_kw_defaults: usize,
) {
    let e = emit_ref_mut(emit);
    unsafe {
        if n_pos_defaults == 0 && n_kw_defaults == 0 {
            write_bytecode_byte_child(e, 1, bc0::MAKE_FUNCTION, (*scope).raw_code);
        } else {
            write_bytecode_byte_child(e, -1, bc0::MAKE_FUNCTION_DEFARGS, (*scope).raw_code);
        }
    }
}

pub fn make_closure(
    emit: *mut crate::emit::Emit,
    scope: *mut Scope,
    n_closed_over: usize,
    n_pos_defaults: usize,
    n_kw_defaults: usize,
) {
    let e = emit_ref_mut(emit);
    unsafe {
        if n_pos_defaults == 0 && n_kw_defaults == 0 {
            let stack_adj = -(n_closed_over as i32) + 1;
            write_bytecode_byte_child(e, stack_adj, bc0::MAKE_CLOSURE, (*scope).raw_code);
            write_bytecode_raw_byte(e, n_closed_over as u8);
        } else {
            debug_assert!(n_closed_over <= 255);
            let stack_adj = -2 - n_closed_over as i32 + 1;
            write_bytecode_byte_child(e, stack_adj, bc0::MAKE_CLOSURE_DEFARGS, (*scope).raw_code);
            write_bytecode_raw_byte(e, n_closed_over as u8);
        }
    }
}

fn call_helper(
    emit: *mut crate::emit::Emit,
    stack_adj: i32,
    bytecode_base: u8,
    n_positional: usize,
    n_keyword: usize,
    star_flags: u8,
) {
    let e = emit_ref_mut(emit);
    if star_flags != 0 {
        let adj = stack_adj - n_positional as i32 - 2 * n_keyword as i32 - 1;
        write_bytecode_byte_uint(e, adj, bytecode_base + 1, (n_keyword << 8) | n_positional);
    } else {
        let adj = stack_adj - n_positional as i32 - 2 * n_keyword as i32;
        write_bytecode_byte_uint(e, adj, bytecode_base, (n_keyword << 8) | n_positional);
    }
}

pub fn call_function(
    emit: *mut crate::emit::Emit,
    n_positional: usize,
    n_keyword: usize,
    star_flags: u8,
) {
    call_helper(
        emit,
        0,
        bc0::CALL_FUNCTION,
        n_positional,
        n_keyword,
        star_flags,
    );
}

pub fn call_method(
    emit: *mut crate::emit::Emit,
    n_positional: usize,
    n_keyword: usize,
    star_flags: u8,
) {
    call_helper(
        emit,
        -1,
        bc0::CALL_METHOD,
        n_positional,
        n_keyword,
        star_flags,
    );
}

pub fn return_value(emit: *mut crate::emit::Emit) {
    write_bytecode_byte(emit_ref_mut(emit), -1, bc0::RETURN_VALUE);
    emit_ref_mut(emit).suppress = true;
}

pub fn raise_varargs(emit: *mut crate::emit::Emit, n_args: usize) {
    debug_assert!(n_args <= 2);
    write_bytecode_byte(
        emit_ref_mut(emit),
        -(n_args as i32),
        bc0::RAISE_LAST + n_args as u8,
    );
    emit_ref_mut(emit).suppress = true;
}

pub fn yield_(emit: *mut crate::emit::Emit, kind: i32) {
    write_bytecode_byte(emit_ref_mut(emit), -kind, bc0::YIELD_VALUE + kind as u8);
    unsafe {
        (*emit_ref_mut(emit).scope).scope_flags |= SCOPE_FLAG_GENERATOR as u16;
    }
}

pub fn start_except_handler(emit: *mut crate::emit::Emit) {
    adjust_stack_size(emit, 4);
}

pub fn end_except_handler(emit: *mut crate::emit::Emit) {
    adjust_stack_size(emit, -3);
}

pub fn async_with_setup_finally(
    emit: *mut crate::emit::Emit,
    label_aexit_no_exc: usize,
    label_finally_block: usize,
    label_ret_unwind_jump: usize,
) {
    load_const_tok(emit, TokenKind::KwNone);
    rot_two(emit);
    jump(emit, label_aexit_no_exc);
    label_assign(emit, label_finally_block);
    dup_top(emit);
    load_global(
        emit,
        qstr::from_str("BaseException"),
        EMIT_IDOP_GLOBAL_GLOBAL,
    );
    binary_op(emit, BinaryOp::ExceptionMatch);
    pop_jump_if(emit, false, label_ret_unwind_jump);
}
