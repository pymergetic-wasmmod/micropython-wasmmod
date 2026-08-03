//! rewrite of py/nativeglue.c + py/nativeglue.h
// symmetry: done
// host fun-table notes (intentional nulls, not gaps):
// - index 49 (setjmp): null; host NLR uses nlr_push_tail (indices 32–33), not setjmp/longjmp
// - indices 53–54 (mp_printf/mp_vprintf): null; variadic C ABI unused (host uses mpprint + raise_msg)
// - indices 73–86: type/stream object pointers; filled by init_fun_table_extras() after GC in runtime::init

use core::ffi::c_void;
use std::sync::Once;

use crate::argcheck;
use crate::bc::{self, ModuleContext};
use crate::binary;
use crate::emitglue;
use crate::emitnative;
use crate::gc;
use crate::map::Map;
use crate::mpconfig;
use crate::mpprint::{self, Print};
use crate::nlr;
use crate::obj::{self, BufferInfo, Obj, ObjType};
use crate::objarray;
use crate::objclosure;
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objfloat;
use crate::objfun;
use crate::objlist;
use crate::objset;
use crate::objslice;
use crate::objstr;
use crate::objtuple;
use crate::objtype;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime::{self, VmReturnKind};
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::smallint;
use crate::stream;

/// Native type codes (`mp_native_type_t` low nibble).
pub const NATIVE_TYPE_OBJ: u32 = 0;
pub const NATIVE_TYPE_BOOL: u32 = 1;
pub const NATIVE_TYPE_INT: u32 = 2;
pub const NATIVE_TYPE_UINT: u32 = 3;
pub const NATIVE_TYPE_PTR: u32 = 4;
pub const NATIVE_TYPE_PTR8: u32 = 5;
pub const NATIVE_TYPE_PTR16: u32 = 6;
pub const NATIVE_TYPE_PTR32: u32 = 7;
pub const NATIVE_TYPE_QSTR: u32 = 8;

/// Function table index (`mp_fun_kind_t`).
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FunKind {
    ConstNoneObj = 0,
    NumberOf = 50,
}

pub fn native_type_from_qstr(qst: Qstr) -> i32 {
    let s = crate::qstr::str_from_qstr(qst).unwrap_or_default();
    match s.as_str() {
        "object" => NATIVE_TYPE_OBJ as i32,
        "bool" => NATIVE_TYPE_BOOL as i32,
        "int" => NATIVE_TYPE_INT as i32,
        "uint" => NATIVE_TYPE_UINT as i32,
        "ptr" => NATIVE_TYPE_PTR as i32,
        _ => -1,
    }
}

pub fn native_from_obj(obj_in: Obj, typ: u32) -> usize {
    match typ & 0xf {
        NATIVE_TYPE_OBJ => obj_in.0,
        NATIVE_TYPE_BOOL => obj::is_true(obj_in) as usize,
        NATIVE_TYPE_INT | NATIVE_TYPE_UINT => obj::get_int(obj_in) as usize,
        _ => {
            let mut buf = obj::BufferInfo::default();
            if obj::get_buffer(obj_in, &mut buf, obj::BUFFER_READ) {
                buf.buf as usize
            } else {
                obj::get_int(obj_in) as usize
            }
        }
    }
}

pub fn native_to_obj(val: usize, typ: u32) -> Obj {
    match typ & 0xf {
        NATIVE_TYPE_OBJ => Obj(val),
        NATIVE_TYPE_BOOL => obj::new_bool(val != 0),
        NATIVE_TYPE_INT => obj::new_int(val as i64 as crate::obj::Int),
        NATIVE_TYPE_UINT => crate::objint::new_int_from_uint(val),
        NATIVE_TYPE_QSTR => obj::new_qstr(val as Qstr),
        _ => crate::objint::new_int_from_uint(val),
    }
}

// --- C-ABI shims for `mp_fun_table_t` -----------------------------------------

extern "C" fn shim_native_from_obj(obj_in: Obj, typ: u32) -> usize {
    native_from_obj(obj_in, typ)
}

extern "C" fn shim_native_to_obj(val: usize, typ: u32) -> Obj {
    native_to_obj(val, typ)
}

extern "C" fn shim_native_swap_globals(new_globals: *mut ObjDict) -> *mut ObjDict {
    if new_globals.is_null() {
        return core::ptr::null_mut();
    }
    let new_g = obj::from_ptr(new_globals as *const ());
    let old = runtime::globals_get();
    if old == new_g {
        return core::ptr::null_mut();
    }
    runtime::globals_set(new_g);
    objdict::dict_ptr(old)
}

extern "C-unwind" fn shim_load_name(qst: Qstr) -> Obj {
    runtime::load_name(qst)
}

extern "C-unwind" fn shim_load_global(qst: Qstr) -> Obj {
    runtime::load_global(qst)
}

extern "C-unwind" fn shim_load_build_class() -> Obj {
    runtime::load_build_class()
}

extern "C-unwind" fn shim_load_attr(base: Obj, attr: Qstr) -> Obj {
    runtime::load_attr(base, attr)
}

extern "C-unwind" fn shim_load_method(base: Obj, attr: Qstr, dest: *mut Obj) {
    let dest = unsafe { &mut *(dest as *mut [Obj; 2]) };
    runtime::load_method(base, attr, dest);
}

extern "C-unwind" fn shim_load_super_method(attr: Qstr, dest: *mut Obj) {
    let dest = unsafe { &mut *(dest as *mut [Obj; 3]) };
    objtype::load_super_method(attr, dest);
}

extern "C-unwind" fn shim_store_name(qst: Qstr, value: Obj) {
    runtime::store_name(qst, value);
}

extern "C-unwind" fn shim_store_global(qst: Qstr, value: Obj) {
    runtime::store_global(qst, value);
}

extern "C-unwind" fn shim_store_attr(base: Obj, attr: Qstr, value: Obj) {
    runtime::store_attr(base, attr, value);
}

extern "C-unwind" fn shim_obj_subscr(base: Obj, index: Obj, value: Obj) -> Obj {
    obj::subscr(base, index, value)
}

extern "C-unwind" fn shim_obj_is_true(arg: Obj) -> bool {
    obj::is_true(arg)
}

extern "C-unwind" fn shim_unary_op(op: UnaryOp, arg: Obj) -> Obj {
    runtime::unary_op_obj(op, arg)
}

extern "C-unwind" fn shim_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    runtime::binary_op_obj(op, lhs, rhs)
}

extern "C" fn shim_new_tuple(n: usize, items: *const Obj) -> Obj {
    let items = if n == 0 {
        None
    } else {
        Some(unsafe { core::slice::from_raw_parts(items, n) })
    };
    objtuple::new_tuple(n, items)
}

extern "C" fn shim_new_list(n: usize, items: *mut Obj) -> Obj {
    let items = if n == 0 {
        None
    } else {
        Some(unsafe { core::slice::from_raw_parts(items, n) })
    };
    objlist::new_list(n, items)
}

extern "C" fn shim_new_dict(n_args: usize) -> Obj {
    objdict::new_dict(n_args)
}

extern "C" fn shim_new_set(n_args: usize, items: *mut Obj) -> Obj {
    let items = if n_args == 0 {
        None
    } else {
        Some(unsafe { core::slice::from_raw_parts(items, n_args) })
    };
    objset::new_set(n_args, items)
}

extern "C-unwind" fn shim_set_store(self_in: Obj, item: Obj) {
    objset::set_store(self_in, item);
}

extern "C-unwind" fn shim_list_append(self_in: Obj, arg: Obj) -> Obj {
    objlist::list_append(self_in, arg)
}

extern "C-unwind" fn shim_dict_store(self_in: Obj, key: Obj, value: Obj) -> Obj {
    objdict::dict_store(self_in, key, value)
}

extern "C" fn shim_make_function_from_proto_fun(
    proto_fun: *const (),
    context: *const ModuleContext,
    def_args: *const Obj,
) -> Obj {
    let def = if def_args.is_null() {
        None
    } else {
        Some(unsafe { [*def_args, *def_args.add(1)] })
    };
    emitglue::make_function_from_proto_fun(proto_fun as emitglue::ProtoFun, context, def.as_ref())
}

fn shim_args_slice<'a>(args: *const Obj, len: usize) -> &'a [Obj] {
    if len == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(args, len) }
    }
}

extern "C-unwind" fn shim_native_call_function_n_kw(
    fun_in: Obj,
    n_args_kw: usize,
    args: *const Obj,
) -> Obj {
    let n_args = n_args_kw & 0xff;
    let n_kw = (n_args_kw >> 8) & 0xff;
    let args = shim_args_slice(args, n_args + n_kw * 2);
    runtime::call_function_n_kw(fun_in, n_args, n_kw, args)
}

extern "C-unwind" fn shim_call_method_n_kw(n_args: usize, n_kw: usize, args: *const Obj) -> Obj {
    let args = shim_args_slice(args, n_args + n_kw * 2 + 2);
    runtime::call_method_n_kw(n_args, n_kw, args)
}

extern "C-unwind" fn shim_call_method_n_kw_var(
    have_self: bool,
    n_args_n_kw: usize,
    args: *const Obj,
) -> Obj {
    let args = shim_args_slice(args, n_args_n_kw + 2);
    runtime::call_method_n_kw_var(have_self, n_args_n_kw, args)
}

extern "C-unwind" fn shim_native_getiter(obj_in: Obj, iter: *mut obj::ObjIterBuf) -> Obj {
    if iter.is_null() {
        return runtime::getiter(obj_in, None);
    }
    let iter = unsafe { &mut *iter };
    let result = runtime::getiter(obj_in, Some(iter));
    if result != obj::from_ptr(iter as *const obj::ObjIterBuf as *const ()) {
        iter.base.type_ = core::ptr::null();
        iter.buf[0] = result;
    }
    obj::OBJ_NULL
}

extern "C-unwind" fn shim_native_iternext(iter: *mut obj::ObjIterBuf) -> Obj {
    let iter = unsafe { &*iter };
    let obj_in = if iter.base.type_.is_null() {
        iter.buf[0]
    } else {
        obj::from_ptr(iter as *const obj::ObjIterBuf as *const ())
    };
    runtime::iternext(obj_in)
}

extern "C" fn shim_nlr_push(buf: *mut nlr::NlrBuf) -> u32 {
    nlr::push_tail(unsafe { &mut *buf })
}

extern "C" fn shim_nlr_pop() {
    nlr::pop_top();
}

extern "C-unwind" fn shim_native_raise(o: Obj) {
    if o != obj::OBJ_NULL && o != obj::CONST_NONE {
        nlr::jump(runtime::make_raise_obj(o).0);
    }
}

extern "C-unwind" fn shim_import_name(name: Qstr, fromlist: Obj, level: Obj) -> Obj {
    runtime::import_name(name, fromlist, level)
}

extern "C-unwind" fn shim_import_from(module: Obj, name: Qstr) -> Obj {
    runtime::import_from(module, name)
}

extern "C-unwind" fn shim_import_all(module: Obj) {
    runtime::import_all(module);
}

extern "C" fn shim_new_slice(start: Obj, stop: Obj, step: Obj) -> Obj {
    objslice::new_slice(start, stop, step)
}

extern "C-unwind" fn shim_unpack_sequence(seq: Obj, num: usize, items: *mut Obj) {
    let items = unsafe { core::slice::from_raw_parts_mut(items, num) };
    runtime::unpack_sequence(seq, num, items);
}

extern "C-unwind" fn shim_unpack_ex(seq: Obj, num: usize, items: *mut Obj) {
    let items = unsafe { core::slice::from_raw_parts_mut(items, num + 1) };
    runtime::unpack_ex(seq, num, items);
}

extern "C-unwind" fn shim_delete_name(qst: Qstr) {
    runtime::delete_name(qst);
}

extern "C-unwind" fn shim_delete_global(qst: Qstr) {
    runtime::delete_global(qst);
}

extern "C" fn shim_new_closure(fun: Obj, n_closed_over: usize, closed: *const Obj) -> Obj {
    let closed = if n_closed_over == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(closed, n_closed_over) }
    };
    objclosure::new_closure(fun, n_closed_over, closed)
}

extern "C-unwind" fn shim_arg_check_num_sig(n_args: usize, n_kw: usize, sig: u32) {
    argcheck::check_num_sig(n_args, n_kw, sig);
}

extern "C" fn shim_setup_code_state_native(
    code_state: *mut bc::CodeStateNative,
    n_args: usize,
    n_kw: usize,
    args: *const Obj,
) {
    let args = unsafe { core::slice::from_raw_parts(args, n_args + n_kw * 2) };
    bc::setup_code_state_native(unsafe { &mut *code_state }, n_args, n_kw, args);
}

extern "C" fn shim_small_int_floor_divide(
    num: crate::obj::Int,
    denom: crate::obj::Int,
) -> crate::obj::Int {
    smallint::floor_divide(num, denom)
}

extern "C" fn shim_small_int_modulo(
    dividend: crate::obj::Int,
    divisor: crate::obj::Int,
) -> crate::obj::Int {
    smallint::modulo(dividend, divisor)
}

extern "C" fn shim_native_gen_finish_throw(
    throw_val: Obj,
    code_state: *mut bc::CodeStateNative,
) -> usize {
    let exc = if objexcept::is_exception_instance(throw_val)
        || objexcept::is_native_exception_instance(throw_val)
    {
        throw_val
    } else {
        runtime::make_raise_obj(throw_val)
    };
    unsafe {
        *(*code_state).state_ptr() = exc;
    }
    emitnative::MP_VM_RETURN_EXCEPTION
}

extern "C-unwind" fn shim_native_yield_from(
    gen: Obj,
    send_value: Obj,
    ret_value: *mut Obj,
    parked_return: *mut Obj,
) -> u32 {
    let mut ret = unsafe { *ret_value };
    let throw_value = ret;
    let skip_ret_write = !parked_return.is_null() && ret_value == parked_return;
    let ret_kind = nlr::protect(&mut nlr::NlrBuf::default(), || {
        let send = if throw_value != obj::OBJ_NULL {
            obj::OBJ_NULL
        } else {
            send_value
        };
        runtime::resume(gen, send, throw_value, &mut ret)
    });
    if !skip_ret_write {
        unsafe {
            *ret_value = ret;
        }
    }
    let host_return = mpconfig::NLR_SETJMP;
    match ret_kind {
        Ok(VmReturnKind::Yield) => return emitnative::MP_VM_RETURN_YIELD as u32,
        Ok(VmReturnKind::Normal) => {
            if ret == obj::OBJ_STOP_ITERATION {
                if !skip_ret_write {
                    unsafe {
                        *ret_value = obj::CONST_NONE;
                    }
                }
            }
        }
        Ok(VmReturnKind::Exception) | Err(_) => {
            if !objexcept::exception_match(
                ret,
                obj::from_ptr(objexcept::type_stop_iteration() as *const ObjType as *const ()),
            ) {
                if host_return {
                    return emitnative::MP_VM_RETURN_EXCEPTION as u32;
                }
                nlr::jump(ret.0);
            }
            if !skip_ret_write {
                unsafe {
                    *ret_value = objexcept::exception_get_value(ret);
                }
            }
        }
    }
    if throw_value != obj::OBJ_NULL
        && objexcept::exception_match(
            throw_value,
            obj::from_ptr(objexcept::type_generator_exit() as *const ObjType as *const ()),
        )
    {
        if host_return {
            if !skip_ret_write {
                unsafe {
                    *ret_value = if objexcept::is_exception_instance(throw_value)
                        || objexcept::is_native_exception_instance(throw_value)
                    {
                        throw_value
                    } else {
                        runtime::make_raise_obj(throw_value)
                    };
                }
            }
            return emitnative::MP_VM_RETURN_EXCEPTION as u32;
        }
        nlr::jump(runtime::make_raise_obj(throw_value).0);
    }
    emitnative::MP_VM_RETURN_NORMAL as u32
}

extern "C" fn shim_memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    unsafe {
        core::ptr::write_bytes(s, c as u8, n);
    }
    s
}

extern "C" fn shim_memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    unsafe {
        core::ptr::copy(src as *const u8, dest as *mut u8, n);
    }
    dest
}

extern "C" fn shim_gc_realloc(ptr: *mut u8, n_bytes: usize, allow_move: bool) -> *mut u8 {
    gc::realloc(ptr, n_bytes, allow_move).unwrap_or(core::ptr::null_mut())
}

extern "C-unwind" fn shim_raise_msg(exc_type: *const ObjType, msg: *const u8) {
    let msg = if msg.is_null() {
        ""
    } else {
        unsafe { core::ffi::CStr::from_ptr(msg as *const i8) }
            .to_str()
            .unwrap_or("")
    };
    raise::raise_obj(objexcept::new_exception_args(
        unsafe { &*exc_type },
        1,
        &[objstr::new_str(msg.as_bytes())],
    ));
}

extern "C" fn shim_obj_get_type(o_in: Obj) -> *const ObjType {
    obj::get_type(o_in) as *const ObjType
}

extern "C" fn shim_obj_new_str(data: *const u8, len: usize) -> Obj {
    let data = unsafe { core::slice::from_raw_parts(data, len) };
    objstr::new_str(data)
}

extern "C" fn shim_obj_new_bytes(data: *const u8, len: usize) -> Obj {
    let data = unsafe { core::slice::from_raw_parts(data, len) };
    objstr::new_bytes(data)
}

extern "C" fn shim_obj_new_bytearray_by_ref(n: usize, items: *mut u8) -> Obj {
    objarray::new_bytearray_by_ref(n, items)
}

extern "C-unwind" fn shim_load_method_maybe(base: Obj, attr: Qstr, dest: *mut Obj) {
    let dest = unsafe { &mut *(dest as *mut [Obj; 2]) };
    runtime::load_method_maybe(base, attr, dest);
}

extern "C" fn shim_get_buffer(obj_in: Obj, bufinfo: *mut BufferInfo, flags: u32) -> bool {
    obj::get_buffer(obj_in, unsafe { &mut *bufinfo }, flags)
}

extern "C-unwind" fn shim_get_stream_raise(self_in: Obj, flags: i32) -> *const stream::StreamP {
    stream::get_stream_raise(self_in, flags) as *const stream::StreamP
}

extern "C-unwind" fn shim_arg_parse_all(
    n_pos: usize,
    pos: *const Obj,
    kws: *mut Map,
    n_allowed: usize,
    allowed: *const argcheck::Arg,
    out_vals: *mut argcheck::ArgVal,
) {
    let pos = unsafe { core::slice::from_raw_parts(pos, n_pos) };
    let allowed = unsafe { core::slice::from_raw_parts(allowed, n_allowed) };
    let out_vals = unsafe { core::slice::from_raw_parts_mut(out_vals, n_allowed) };
    argcheck::parse_all(
        n_pos,
        pos,
        unsafe { &mut *kws },
        n_allowed,
        allowed,
        out_vals,
    );
}

extern "C-unwind" fn shim_arg_parse_all_kw_array(
    n_pos: usize,
    n_kw: usize,
    args: *const Obj,
    n_allowed: usize,
    allowed: *const argcheck::Arg,
    out_vals: *mut argcheck::ArgVal,
) {
    let args = unsafe { core::slice::from_raw_parts(args, n_pos + n_kw * 2) };
    let allowed = unsafe { core::slice::from_raw_parts(allowed, n_allowed) };
    let out_vals = unsafe { core::slice::from_raw_parts_mut(out_vals, n_allowed) };
    argcheck::parse_all_kw_array(n_pos, n_kw, args, n_allowed, allowed, out_vals);
}

extern "C" fn shim_binary_get_size(struct_type: u8, val_type: u8, palign: *mut usize) -> usize {
    binary::get_size(
        struct_type,
        val_type,
        if palign.is_null() {
            None
        } else {
            Some(unsafe { &mut *palign })
        },
    )
}

extern "C-unwind" fn shim_binary_get_val_array(typecode: u8, p: *const u8, index: usize) -> Obj {
    if p.is_null() {
        return obj::OBJ_NULL;
    }
    let len = index + binary::get_size(b'@', typecode, None);
    binary::get_val_array(
        typecode,
        unsafe { core::slice::from_raw_parts(p, len) },
        index,
    )
}

extern "C-unwind" fn shim_binary_set_val_array(
    typecode: u8,
    p: *mut u8,
    index: usize,
    val_in: Obj,
) {
    if p.is_null() {
        return;
    }
    let len = index + binary::get_size(b'@', typecode, None);
    binary::set_val_array(
        typecode,
        unsafe { core::slice::from_raw_parts_mut(p, len) },
        index,
        val_in,
    );
}

/// Word count of `mp_fun_table_t` (`MP_F_NUMBER_OF` + dynamic-runtime entries).
pub const FUN_TABLE_RELOC_WORDS: usize = 87;

fn init_table_core(table: &mut [usize; FUN_TABLE_RELOC_WORDS]) {
    table[0] = crate::obj::CONST_NONE.0;
    table[1] = crate::obj::CONST_FALSE.0;
    table[2] = crate::obj::CONST_TRUE.0;
    table[3] = shim_native_from_obj as *const () as usize;
    table[4] = shim_native_to_obj as *const () as usize;
    table[5] = shim_native_swap_globals as *const () as usize;
    table[6] = shim_load_name as *const () as usize;
    table[7] = shim_load_global as *const () as usize;
    table[8] = shim_load_build_class as *const () as usize;
    table[9] = shim_load_attr as *const () as usize;
    table[10] = shim_load_method as *const () as usize;
    table[11] = shim_load_super_method as *const () as usize;
    table[12] = shim_store_name as *const () as usize;
    table[13] = shim_store_global as *const () as usize;
    table[14] = shim_store_attr as *const () as usize;
    table[15] = shim_obj_subscr as *const () as usize;
    table[16] = shim_obj_is_true as *const () as usize;
    table[17] = shim_unary_op as *const () as usize;
    table[18] = shim_binary_op as *const () as usize;
    table[19] = shim_new_tuple as *const () as usize;
    table[20] = shim_new_list as *const () as usize;
    table[21] = shim_new_dict as *const () as usize;
    table[22] = shim_new_set as *const () as usize;
    table[23] = shim_set_store as *const () as usize;
    table[24] = shim_list_append as *const () as usize;
    table[25] = shim_dict_store as *const () as usize;
    table[26] = shim_make_function_from_proto_fun as *const () as usize;
    table[27] = shim_native_call_function_n_kw as *const () as usize;
    table[28] = shim_call_method_n_kw as *const () as usize;
    table[29] = shim_call_method_n_kw_var as *const () as usize;
    table[30] = shim_native_getiter as *const () as usize;
    table[31] = shim_native_iternext as *const () as usize;
    table[32] = shim_nlr_push as *const () as usize;
    table[33] = shim_nlr_pop as *const () as usize;
    table[34] = shim_native_raise as *const () as usize;
    table[35] = shim_import_name as *const () as usize;
    table[36] = shim_import_from as *const () as usize;
    table[37] = shim_import_all as *const () as usize;
    table[38] = shim_new_slice as *const () as usize;
    table[39] = shim_unpack_sequence as *const () as usize;
    table[40] = shim_unpack_ex as *const () as usize;
    table[41] = shim_delete_name as *const () as usize;
    table[42] = shim_delete_global as *const () as usize;
    table[43] = shim_new_closure as *const () as usize;
    table[44] = shim_arg_check_num_sig as *const () as usize;
    table[45] = shim_setup_code_state_native as *const () as usize;
    table[46] = shim_small_int_floor_divide as *const () as usize;
    table[47] = shim_small_int_modulo as *const () as usize;
    table[48] = shim_native_yield_from as *const () as usize;
    table[49] = shim_native_gen_finish_throw as *const () as usize;
    table[50] = shim_memset as *const () as usize;
    table[51] = shim_memmove as *const () as usize;
    table[52] = shim_gc_realloc as *const () as usize;
    table[55] = shim_raise_msg as *const () as usize;
    table[56] = shim_obj_get_type as *const () as usize;
    table[57] = shim_obj_new_str as *const () as usize;
    table[58] = shim_obj_new_bytes as *const () as usize;
    table[59] = shim_obj_new_bytearray_by_ref as *const () as usize;
    table[60] = objfloat::new_float_from_f as *const () as usize;
    table[61] = objfloat::new_float_from_d as *const () as usize;
    table[62] = objfloat::get_float_to_f as *const () as usize;
    table[63] = objfloat::get_float_to_d as *const () as usize;
    table[64] = shim_load_method_maybe as *const () as usize;
    table[65] = shim_get_buffer as *const () as usize;
    table[66] = shim_get_stream_raise as *const () as usize;
    table[67] = shim_arg_parse_all as *const () as usize;
    table[68] = shim_arg_parse_all_kw_array as *const () as usize;
    table[69] = shim_binary_get_size as *const () as usize;
    table[70] = shim_binary_get_val_array as *const () as usize;
    table[71] = shim_binary_set_val_array as *const () as usize;
    table[72] = core::ptr::from_ref(&mpprint::PLAT_PRINT) as usize;
}

fn init_table_extras(table: &mut [usize; FUN_TABLE_RELOC_WORDS]) {
    table[73] = objtype::type_type() as *const ObjType as usize;
    table[74] = objstr::type_str() as *const ObjType as usize;
    table[75] = objlist::type_list() as *const ObjType as usize;
    table[76] = objdict::type_dict() as *const ObjType as usize;
    table[77] = objfun::type_fun_builtin_0() as *const ObjType as usize;
    table[78] = objfun::type_fun_builtin_1() as *const ObjType as usize;
    table[79] = objfun::type_fun_builtin_2() as *const ObjType as usize;
    table[80] = objfun::type_fun_builtin_3() as *const ObjType as usize;
    table[81] = objfun::type_fun_builtin_var() as *const ObjType as usize;
    table[82] = objexcept::type_exception() as *const ObjType as usize;
    table[83] = stream::stream_read_obj().0;
    table[84] = stream::stream_readinto_obj().0;
    table[85] = stream::stream_unbuffered_readline_obj().0;
    table[86] = stream::stream_write_obj().0;
}

static mut MP_FUN_TABLE: [usize; FUN_TABLE_RELOC_WORDS] = [0; FUN_TABLE_RELOC_WORDS];
static TABLE_CORE_INIT: Once = Once::new();
static TABLE_EXTRAS_INIT: Once = Once::new();

fn ensure_table_core() {
    TABLE_CORE_INIT.call_once(|| unsafe {
        init_table_core(&mut MP_FUN_TABLE);
    });
}

/// Fill type/stream object slots; requires GC (`runtime::init` path).
pub fn init_fun_table_extras() {
    ensure_table_core();
    TABLE_EXTRAS_INIT.call_once(|| unsafe {
        init_table_extras(&mut MP_FUN_TABLE);
    });
}

pub fn fun_table() -> usize {
    if mpconfig::ENABLE_NATIVE_CODE {
        ensure_table_core();
        unsafe { MP_FUN_TABLE.as_ptr() as usize }
    } else {
        0
    }
}

/// Base address of the native function table (`&mp_fun_table`).
pub fn fun_table_reloc_base() -> usize {
    ensure_table_core();
    unsafe { MP_FUN_TABLE.as_ptr() as usize }
}

/// Native function table as a flat pointer array for relocation indexing.
pub fn fun_table_reloc_entries() -> *const usize {
    ensure_table_core();
    unsafe { MP_FUN_TABLE.as_ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asmbase;
    use crate::compile;
    use crate::emitnative;
    use crate::lexer;
    use crate::mpstate;
    use crate::nlr;
    use crate::obj;
    use crate::objdict;
    use crate::objexcept;
    use crate::objgenerator;
    use crate::parse;
    use crate::qstr;
    use crate::reader::READER_IS_ROM;
    use crate::runtime::{self, VmReturnKind};

    fn setup() {
        runtime::init();
    }

    #[test]
    fn native_yield_from_shim_reraises_generator_exit() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        ensure_table_core();
        let src = "\
def inner():
    try:
        yield 1
    except GeneratorExit:
        pass
";
        let lex = lexer::Lexer::new_from_str_len(
            qstr::from_str("<stdin>"),
            src.as_bytes(),
            READER_IS_ROM,
        );
        let mut tree = parse::parse(lex, parse::ParseInputKind::FileInput);
        let module_fun = compile::compile(&mut tree, qstr::from_str("<stdin>"), false);
        runtime::call_function_n_kw(module_fun, 0, 0, &[]);
        let inner_obj = objdict::dict_get(
            mpstate::globals_get(),
            obj::new_qstr(qstr::from_str("inner")),
        );
        let inner_gen = runtime::call_function_n_kw(inner_obj, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        assert_eq!(
            objgenerator::gen_resume(inner_gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret),
            VmReturnKind::Yield
        );
        assert_eq!(obj::small_int_value(ret), 1);

        let mut slot = objgenerator::const_generator_exit();
        type YieldFromFn = unsafe extern "C-unwind" fn(Obj, Obj, *mut Obj, *mut Obj) -> u32;
        let shim: YieldFromFn = unsafe {
            core::mem::transmute(MP_FUN_TABLE[emitnative::mp_f::NATIVE_YIELD_FROM as usize])
        };
        let ret_kind =
            unsafe { shim(inner_gen, obj::CONST_NONE, &mut slot, core::ptr::null_mut()) };
        assert_eq!(
            ret_kind,
            emitnative::MP_VM_RETURN_EXCEPTION as u32,
            "native yield_from shim must re-raise GeneratorExit after delegate swallows it",
        );
        assert!(
            objexcept::is_exception_instance(slot) || objexcept::is_native_exception_instance(slot),
            "expected exception object, got {slot:?}",
        );
    }

    #[test]
    fn fun_table_has_runtime_entries() {
        ensure_table_core();
        unsafe {
            assert_ne!(MP_FUN_TABLE[3], 0);
            assert_ne!(MP_FUN_TABLE[6], 0);
            assert_ne!(MP_FUN_TABLE[26], 0);
            assert_eq!(MP_FUN_TABLE[0], crate::obj::CONST_NONE.0);
        }
    }
}
