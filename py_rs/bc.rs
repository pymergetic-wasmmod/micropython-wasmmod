//! rewrite of py/bc.c + py/bc.h (bytecode helpers and code-state setup)
// symmetry: done

use core::mem::size_of;
use core::ptr;

use crate::bc0;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objcell;
use crate::objdict::{self, ObjDict};
use crate::objtuple;
use crate::qstr::Qstr;
use crate::raise::{self, MpRaise};

/// Module header (`mp_obj_module_t`).
#[repr(C)]
pub struct ObjModule {
    pub base: obj::ObjBase,
    pub globals: *mut ObjDict,
}

/// Module constants table (`mp_module_constants_t`).
///
/// C-compatible: bare pointers to qstr/obj arrays (not `Vec` metadata).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ModuleConstants {
    pub qstr_table: *mut Qstr,
    pub obj_table: *mut Obj,
}

impl ModuleConstants {
    pub unsafe fn qstr_at(&self, idx: usize) -> Qstr {
        *self.qstr_table.add(idx)
    }

    pub unsafe fn obj_at(&self, idx: usize) -> Obj {
        *self.obj_table.add(idx)
    }
}

/// Module object + constants (`mp_module_context_t`).
#[repr(C)]
pub struct ModuleContext {
    pub module: ObjModule,
    pub constants: ModuleConstants,
    pub n_qstr: usize,
    pub n_obj: usize,
}

impl ModuleContext {
    pub fn qstr_table(&self) -> &[Qstr] {
        if self.n_qstr == 0 || self.constants.qstr_table.is_null() {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.constants.qstr_table, self.n_qstr) }
        }
    }

    pub fn obj_table(&self) -> &[Obj] {
        if self.n_obj == 0 || self.constants.obj_table.is_null() {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.constants.obj_table, self.n_obj) }
        }
    }

    pub fn qstr_table_mut(&mut self) -> &mut [Qstr] {
        if self.n_qstr == 0 || self.constants.qstr_table.is_null() {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(self.constants.qstr_table, self.n_qstr) }
        }
    }

    pub fn obj_table_mut(&mut self) -> &mut [Obj] {
        if self.n_obj == 0 || self.constants.obj_table.is_null() {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(self.constants.obj_table, self.n_obj) }
        }
    }
}

/// Bytecode function object (`mp_obj_fun_bc_t`).
#[repr(C)]
pub struct ObjFunBc {
    pub base: obj::ObjBase,
    pub context: *const ModuleContext,
    pub child_table: *const *const (),
    pub bytecode: *const u8,
    // flexible extra_args[]
}

/// Exception stack entry (`mp_exc_stack_t`).
#[repr(C)]
pub struct ExcStack {
    pub handler: *const u8,
    pub val_sp: *mut Obj,
    pub prev_exc: *mut obj::ObjBase,
}

/// Executing native function state (`mp_code_state_native_t`).
#[repr(C)]
pub struct CodeStateNative {
    pub fun_bc: Obj,
    pub ip: *const u8,
    pub sp: *mut Obj,
    pub n_state: u16,
    pub exc_sp_idx: u16,
    pub old_globals: *mut ObjDict,
}

impl CodeStateNative {
    pub fn state_ptr(&self) -> *mut Obj {
        unsafe {
            (self as *const CodeStateNative as *mut u8).add(size_of::<CodeStateNative>()) as *mut Obj
        }
    }
}

/// Executing function state (`mp_code_state_t`).
#[repr(C)]
pub struct CodeState {
    pub fun_bc: *mut ObjFunBc,
    pub ip: *const u8,
    pub sp: *mut Obj,
    pub n_state: u16,
    pub exc_sp_idx: u16,
    pub old_globals: *mut ObjDict,
}

impl CodeState {
    pub fn state_ptr(&self) -> *mut Obj {
        unsafe { (self as *const CodeState as *mut u8).add(size_of::<CodeState>()) as *mut Obj }
    }
}

pub fn tagptr_ptr(x: *mut Obj) -> *mut Obj {
    unsafe { ((x as usize) & !3) as *mut Obj }
}

pub fn tagptr_tag1(x: *mut Obj) -> bool {
    (x as usize) & 2 != 0
}

pub fn tagptr_make(ptr: *mut Obj, tag: usize) -> *mut Obj {
    unsafe { ((ptr as usize) | tag) as *mut Obj }
}

pub fn exc_sp_idx_from_ptr(exc_stack: *const ExcStack, exc_sp: *const ExcStack) -> u16 {
    unsafe { (exc_sp.offset_from(exc_stack) + 1) as u16 }
}

pub fn exc_sp_idx_to_ptr(exc_stack: *mut ExcStack, exc_sp_idx: u16) -> *mut ExcStack {
    unsafe { exc_stack.offset(exc_sp_idx as isize - 1) }
}

pub const ENCODE_UINT_MAX_BYTES: usize = (mpconfig::BYTES_PER_OBJ_WORD as usize * 8 + 6) / 7;

pub fn encode_uint(out: &mut dyn FnMut(u8), mut val: usize) {
    let mut buf = [0u8; ENCODE_UINT_MAX_BYTES];
    let mut p = buf.len();
    loop {
        p -= 1;
        buf[p] = (val & 0x7f) as u8;
        val >>= 7;
        if val == 0 {
            break;
        }
    }
    while p + 1 < buf.len() {
        out(buf[p] | 0x80);
        p += 1;
    }
    out(buf[p]);
}

pub fn decode_uint(ptr: &mut *const u8) -> usize {
    let mut unum = 0usize;
    loop {
        let val = unsafe { **ptr };
        *ptr = unsafe { ptr.add(1) };
        unum = (unum << 7) | (val as usize & 0x7f);
        if val & 0x80 == 0 {
            break;
        }
    }
    unum
}

pub fn decode_uint_value(ptr: *const u8) -> usize {
    let mut p = ptr;
    decode_uint(&mut p)
}

pub fn decode_uint_skip(ptr: *const u8) -> *const u8 {
    let mut p = ptr;
    decode_uint(&mut p);
    p
}

pub struct PreludeSig {
    pub n_state: usize,
    pub n_exc_stack: usize,
    pub scope_flags: usize,
    pub n_pos_args: usize,
    pub n_kwonly_args: usize,
    pub n_def_pos_args: usize,
}

pub fn prelude_sig_decode_into(ip: &mut *const u8) -> PreludeSig {
    let z = unsafe { **ip };
    *ip = unsafe { ip.add(1) };
    let mut s = ((z >> 3) & 0xf) as usize;
    let mut e = ((z >> 2) & 0x1) as usize;
    let mut f = 0usize;
    let mut a = (z & 0x3) as usize;
    let mut k = 0usize;
    let mut d = 0usize;
    let mut n = 0u32;
    let mut z = z;
    while z & 0x80 != 0 {
        z = unsafe { **ip };
        *ip = unsafe { ip.add(1) };
        s |= ((z & 0x30) as usize) << (2 * n);
        e |= ((z & 0x02) as usize) << n;
        f |= (((z & 0x40) >> 6) as usize) << n;
        a |= ((z & 0x4) as usize) << n;
        k |= (((z & 0x08) >> 3) as usize) << n;
        d |= ((z & 0x1) as usize) << n;
        n += 1;
    }
    s += 1;
    PreludeSig {
        n_state: s,
        n_exc_stack: e,
        scope_flags: f,
        n_pos_args: a,
        n_kwonly_args: k,
        n_def_pos_args: d,
    }
}

pub fn prelude_size_decode(ip: &mut *const u8) -> (usize, usize) {
    let mut c = 0usize;
    let mut i = 0usize;
    let mut bit = 0usize;
    loop {
        let z = unsafe { **ip };
        *ip = unsafe { ip.add(1) };
        c |= (z as usize & 1) << bit;
        i |= ((z as usize & 0x7e) >> 1) << (6 * bit);
        if z & 0x80 == 0 {
            break;
        }
        bit += 1;
    }
    (i, c)
}

/// Line-info delta (`mp_code_lineinfo_t`).
#[derive(Copy, Clone, Debug, Default)]
pub struct CodeLineInfo {
    pub bc_increment: usize,
    pub line_increment: usize,
}

/// Decode one line-info record (`mp_bytecode_decode_lineinfo`).
pub fn decode_lineinfo(line_info: &mut *const u8) -> CodeLineInfo {
    let c = unsafe { **line_info };
    if c & 0x80 == 0 {
        *line_info = unsafe { line_info.add(1) };
        CodeLineInfo {
            bc_increment: (c & 0x1f) as usize,
            line_increment: (c >> 5) as usize,
        }
    } else {
        let second = unsafe { *line_info.add(1) };
        *line_info = unsafe { line_info.add(2) };
        CodeLineInfo {
            bc_increment: (c & 0xf) as usize,
            line_increment: (((c as usize) << 4) & 0x700) | second as usize,
        }
    }
}

/// Map bytecode offset to source line (`mp_bytecode_get_source_line`).
pub fn get_source_line(line_info: *const u8, line_info_top: *const u8, mut bc_offset: usize) -> usize {
    let mut source_line = 1usize;
    let mut li = line_info;
    while li < line_info_top {
        let mut p = li;
        let decoded = decode_lineinfo(&mut p);
        if bc_offset >= decoded.bc_increment {
            bc_offset -= decoded.bc_increment;
            source_line += decoded.line_increment;
            li = p;
        } else {
            break;
        }
    }
    source_line
}

pub fn decode_code_state_size(bytecode: *const u8) -> (usize, usize) {
    let mut ip = bytecode;
    let sig = prelude_sig_decode_into(&mut ip);
    let (_n_info, _n_cell) = prelude_size_decode(&mut ip);
    let state_size = sig.n_state * size_of::<Obj>() + sig.n_exc_stack * size_of::<ExcStack>();
    (sig.n_state, state_size)
}

fn fun_bc_extra_args(fun: &ObjFunBc, n: usize) -> &[Obj] {
    if n == 0 {
        return &[];
    }
    unsafe {
        std::slice::from_raw_parts((fun as *const ObjFunBc).add(1) as *const Obj, n)
    }
}

fn fun_pos_args_mismatch(_expected: usize, _given: usize) -> ! {
    raise::raise(MpRaise::TypeError("argument num/types mismatch"));
}

fn setup_code_state_helper(code_state: &mut CodeState, n_args: usize, n_kw: usize, args: &[Obj]) {
    let self_ = unsafe { &*code_state.fun_bc };
    let n_state = code_state.n_state as usize;
    let mut ip = code_state.ip;
    let sig = prelude_sig_decode_into(&mut ip);
    let (n_info, n_cell) = prelude_size_decode(&mut ip);
    code_state.exc_sp_idx = 0;

    let state_base = unsafe { code_state.sp.add(1) };
    unsafe {
        ptr::write_bytes(state_base, 0, n_state);
    }

    let kwargs = &args[n_args..];
    let mut pos_args = n_args;
    let mut var_pos_kw_args = unsafe { state_base.add(n_state - 1 - sig.n_pos_args - sig.n_kwonly_args) };

    if pos_args > sig.n_pos_args {
        if sig.scope_flags & bc0::SCOPE_FLAG_VARARGS as usize == 0 {
            fun_pos_args_mismatch(sig.n_pos_args, pos_args);
        }
        unsafe {
            *var_pos_kw_args =
                objtuple::new_tuple(pos_args - sig.n_pos_args, Some(&args[sig.n_pos_args..]));
            var_pos_kw_args = var_pos_kw_args.sub(1);
        }
        pos_args = sig.n_pos_args;
    } else if sig.scope_flags & bc0::SCOPE_FLAG_VARARGS as usize != 0 {
        unsafe {
            *var_pos_kw_args = objtuple::const_empty_tuple();
            var_pos_kw_args = var_pos_kw_args.sub(1);
        }
    }

    if n_kw == 0 && sig.scope_flags & bc0::SCOPE_FLAG_DEFKWARGS as usize == 0 {
        if pos_args >= sig.n_pos_args - sig.n_def_pos_args {
            let extra = fun_bc_extra_args(self_, sig.n_def_pos_args + if sig.scope_flags & bc0::SCOPE_FLAG_DEFKWARGS as usize != 0 { 1 } else { 0 });
            for i in pos_args..sig.n_pos_args {
                unsafe {
                    *state_base.add(n_state - 1 - i) = extra[i - (sig.n_pos_args - sig.n_def_pos_args)];
                }
            }
        } else if pos_args < sig.n_pos_args - sig.n_def_pos_args {
            fun_pos_args_mismatch(sig.n_pos_args - sig.n_def_pos_args, pos_args);
        }
    }

    for i in 0..pos_args {
        unsafe {
            *state_base.add(n_state - 1 - i) = args[i];
        }
    }

    if n_kw != 0 || sig.scope_flags & bc0::SCOPE_FLAG_DEFKWARGS as usize != 0 {
        let mut dict = obj::OBJ_NULL;
        if sig.scope_flags & bc0::SCOPE_FLAG_VARKEYWORDS as usize != 0 {
            dict = objdict::new_dict(n_kw);
            unsafe {
                *var_pos_kw_args = dict;
            }
        }
        for i in 0..n_kw {
            let wanted = kwargs[i * 2];
            let mut arg_names = ip;
            arg_names = decode_uint_skip(arg_names);
            let mut found = false;
            for j in 0..sig.n_pos_args + sig.n_kwonly_args {
                let mut arg_qstr = decode_uint(&mut arg_names) as Qstr;
                if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
                    let ctx = unsafe { &*self_.context };
                    arg_qstr = ctx.qstr_table()[arg_qstr as usize];
                }
                if wanted == obj::new_qstr(arg_qstr) {
                    if unsafe { *state_base.add(n_state - 1 - j) } != obj::OBJ_NULL {
                        raise::raise(MpRaise::TypeError("function got multiple values for argument"));
                    }
                    unsafe {
                        *state_base.add(n_state - 1 - j) = kwargs[i * 2 + 1];
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                if sig.scope_flags & bc0::SCOPE_FLAG_VARKEYWORDS as usize == 0 {
                    raise::raise(MpRaise::TypeError("unexpected keyword argument"));
                }
                objdict::dict_store(dict, wanted, kwargs[i * 2 + 1]);
            }
        }
    } else if sig.n_kwonly_args != 0 {
        raise::raise(MpRaise::TypeError("function missing keyword-only argument"));
    } else if sig.scope_flags & bc0::SCOPE_FLAG_VARKEYWORDS as usize != 0 {
        unsafe {
            *var_pos_kw_args = objdict::new_dict(0);
        }
    }

    let mut ip_cells = unsafe { ip.add(n_info) };
    for _ in 0..n_cell {
        let local_num = unsafe { *ip_cells };
        ip_cells = unsafe { ip_cells.add(1) };
        let idx = n_state - 1 - local_num as usize;
        let val = unsafe { *state_base.add(idx) };
        unsafe {
            *state_base.add(idx) = objcell::new_cell(val);
        }
    }
    code_state.ip = ip_cells;
}

/// `mp_setup_code_state`
pub fn setup_code_state(code_state: &mut CodeState, n_args: usize, n_kw: usize, args: &[Obj]) {
    unsafe {
        code_state.ip = (*code_state.fun_bc).bytecode;
        code_state.sp = code_state.state_ptr().sub(1);
    }
    setup_code_state_helper(code_state, n_args, n_kw, args);
}

/// `mp_setup_code_state_native`
pub fn setup_code_state_native(
    code_state: &mut CodeStateNative,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) {
    let fun_bc = unsafe { &*(obj::as_ptr(code_state.fun_bc) as *const ObjFunBc) };
    code_state.ip = crate::objfun::fun_native_get_prelude_ptr(fun_bc);
    code_state.sp = unsafe { code_state.state_ptr().sub(1) };
    setup_code_state_helper(
        unsafe { &mut *(code_state as *mut CodeStateNative as *mut CodeState) },
        n_args,
        n_kw,
        args,
    );
}
