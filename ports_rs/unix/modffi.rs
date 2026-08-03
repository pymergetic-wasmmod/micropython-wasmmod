//! rewrite of ports/unix/modffi.c
// symmetry: done
// Note: same upstream TODOs — libffi type objects / opaqueval unused

use libffi::middle::{arg, ret, Cif, CodePtr, Ret, Type};
use libffi::low::{self, ffi_cif, ffi_closure};
use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::bc::ModuleContext;
use py_rs::binary;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::mperrno;
use py_rs::mpstate;
use py_rs::nlr::{self, NlrBuf};
use py_rs::obj::{
    self, BufferInfo, Int, Obj, ObjBase, ObjType, Uint, TYPE_FLAG_BINDS_SELF,
    TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objarray;
use py_rs::objdict::{self, ObjDict};
use py_rs::objfloat;
use py_rs::objint;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime::{self, call_function_n_kw};
use std::ffi::CString;
use std::os::raw::c_void;

/// `MICROPY_PY_FFI` unix module — libffi/dlopen bindings.
pub fn enabled() -> bool {
    mpconfig::PY_FFI || crate::mpconfigport::PY_FFI
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn raise_errno() -> ! {
    raise::raise(MpRaise::OSError(errno()));
}

fn raise_type(msg: &'static str) -> ! {
    raise::raise(MpRaise::TypeError(msg));
}

fn raise_value(msg: &'static str) -> ! {
    raise::raise(MpRaise::ValueError(msg));
}

fn char2type(c: u8) -> Option<Type> {
    match c {
        b'b' => Some(Type::c_schar()),
        b'B' => Some(Type::c_uchar()),
        b'h' => Some(Type::c_short()),
        b'H' => Some(Type::c_ushort()),
        b'i' => Some(Type::c_int()),
        b'I' => Some(Type::c_uint()),
        b'l' => Some(Type::c_long()),
        b'L' => Some(Type::c_ulong()),
        b'q' => Some(Type::c_longlong()),
        b'Q' => Some(Type::c_ulonglong()),
        b'f' if mpconfig::PY_BUILTINS_FLOAT => Some(Type::f32()),
        b'd' if mpconfig::PY_BUILTINS_FLOAT => Some(Type::f64()),
        b'O' | b'C' | b'P' | b'p' | b's' => Some(Type::pointer()),
        b'v' => Some(Type::void()),
        _ => None,
    }
}

fn get_ffi_type(o_in: Obj) -> Type {
    if obj::is_str_or_bytes(o_in) {
        let s = objstr::str_get_str(o_in);
        if let Some(first) = s.as_bytes().first() {
            if let Some(t) = char2type(*first) {
                return t;
            }
        }
    }
    raise_type("unknown type");
}

fn argtypes_from_obj(argtypes_in: Obj) -> Vec<u8> {
    objstr::str_get_str(argtypes_in).into_bytes()
}

fn types_from_argcodes(codes: &[u8]) -> Vec<Type> {
    codes
        .iter()
        .map(|&c| char2type(c).unwrap_or_else(|| raise_type("unknown type")))
        .collect()
}

#[repr(C)]
union FfiUnion {
    ffi: libffi::raw::ffi_arg,
    b: u8,
    h: u16,
    i: u32,
    l: u32,
    q: u64,
    flt: f32,
    dbl: f64,
}

fn ffi_get_int_value(o: Obj) -> u64 {
    if obj::is_small_int(o) {
        obj::small_int_value(o) as u64
    } else {
        objint::int_get_truncated(o) as u64
    }
}

fn ffi_int_obj_to_union(o: Obj, argtype: u8) -> FfiUnion {
    let mut ret = FfiUnion { ffi: 0 };
    if (argtype | 0x20) == b'q' {
        unsafe {
            ret.q = ffi_get_int_value(o);
        }
        return ret;
    }
    let val = objint::int_get_truncated(o) as u64;
    unsafe {
        match argtype {
            b'b' | b'B' => ret.b = val as u8,
            b'h' | b'H' => ret.h = val as u16,
            b'i' | b'I' => ret.i = val as u32,
            b'l' | b'L' => ret.l = val as u32,
            _ => ret.ffi = val as libffi::raw::ffi_arg,
        }
    }
    ret
}

fn return_ffi_value(val: &FfiUnion, typecode: u8) -> Obj {
    unsafe {
        match typecode {
            b's' => {
                let s = val.ffi as usize as *const i8;
                if s.is_null() {
                    obj::CONST_NONE
                } else {
                    objstr::new_str(std::ffi::CStr::from_ptr(s).to_bytes())
                }
            }
            b'v' => obj::CONST_NONE,
            b'f' if mpconfig::PY_BUILTINS_FLOAT => objfloat::new_float_from_f(val.flt),
            b'd' if mpconfig::PY_BUILTINS_FLOAT => objfloat::new_float_from_d(val.dbl),
            b'b' | b'h' | b'i' | b'l' => objint::new_int(val.ffi as i64 as Int),
            b'I' => objint::new_int_from_uint((val.ffi as u64 & 0xFFFF_FFFF) as Uint),
            b'B' | b'H' | b'L' => objint::new_int_from_uint(val.ffi as Uint),
            b'q' => objint::new_int_from_ll(val.q as i64),
            b'Q' => objint::new_int_from_ull(val.q),
            b'O' => Obj(val.ffi as usize),
            _ => objint::new_int(val.ffi as i64 as Int),
        }
    }
}

fn arg_to_union(a: Obj, argtype: u8) -> FfiUnion {
    let mut values = FfiUnion { ffi: 0 };
    if argtype == b'O' {
        unsafe {
            values.ffi = a.0 as libffi::raw::ffi_arg;
        }
    } else if mpconfig::PY_BUILTINS_FLOAT && argtype == b'f' {
        unsafe {
            values.flt = objfloat::get_float_to_f(a);
        }
    } else if mpconfig::PY_BUILTINS_FLOAT && argtype == b'd' {
        unsafe {
            values.dbl = objfloat::get_float_to_d(a);
        }
    } else if a == obj::CONST_NONE {
        unsafe {
            values.ffi = 0;
        }
    } else if obj::is_int(a) {
        values = ffi_int_obj_to_union(a, argtype);
    } else if obj::is_str_or_bytes(a) {
        raise_type("don't know how to pass object to native function");
    } else if let Some(buf_fn) = obj::type_get_buffer(obj::get_type(a)) {
        let mut bufinfo = BufferInfo::default();
        if buf_fn(a, &mut bufinfo, obj::BUFFER_READ) != 0 {
            raise_type("don't know how to pass object to native function");
        }
        unsafe {
            values.ffi = bufinfo.buf as libffi::raw::ffi_arg;
        }
    } else if obj::is_type(a, type_fficallback()) {
        let p = fficallback_ptr(a);
        unsafe {
            values.ffi = (*p).code.0 as libffi::raw::ffi_arg;
        }
    } else {
        raise_type("don't know how to pass object to native function");
    }
    values
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &map::Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}
#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};
static T3: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F3.as_ptr() },
};
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FV.as_ptr() },
};
static TK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FK.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise_type("argument num/types mismatch");
    }
    let mut kw = map::Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("ffi fn1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("ffi fn2");
    unsafe {
        (*o).base.type_ = &T2 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("ffi fn3");
    unsafe {
        (*o).base.type_ = &T3 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("ffi fnv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}
fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("ffi fnkw");
    unsafe {
        (*o).base.type_ = &TK as *const ObjType;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

#[repr(C)]
struct ObjFfiMod {
    base: ObjBase,
    handle: *mut c_void,
}

fn ffimod_ptr(o: Obj) -> *mut ObjFfiMod {
    obj::as_ptr(o) as *mut ObjFfiMod
}

fn ffimod_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*ffimod_ptr(self_in) };
    let _ = mpprint::printf(
        print,
        "<ffimod %p>",
        std::iter::once(VaArg::USize(self_.handle as usize)),
    );
}

fn ffimod_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 2, false);
    let handle = unsafe {
        if args[0] == obj::CONST_NONE {
            libc::dlopen(core::ptr::null(), libc::RTLD_NOW | libc::RTLD_LOCAL)
        } else {
            let name = objstr::str_get_str(args[0]);
            let cname = CString::new(name).unwrap_or_else(|_| raise_type("bad library name"));
            libc::dlopen(cname.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL)
        }
    };
    if handle.is_null() {
        raise_errno();
    }
    let o = malloc::new_obj::<ObjFfiMod>().expect("ffimod");
    unsafe {
        (*o).base.type_ = type_ffimod() as *const ObjType;
        (*o).handle = handle;
        obj::from_ptr(o as *const ObjFfiMod as *const ())
    }
}

fn ffimod_close(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *ffimod_ptr(self_in) };
    if !self_.handle.is_null() {
        unsafe {
            libc::dlclose(self_.handle);
        }
        self_.handle = core::ptr::null_mut();
    }
    obj::CONST_NONE
}

fn ffimod_addr(self_in: Obj, symname_in: Obj) -> Obj {
    let self_ = unsafe { &*ffimod_ptr(self_in) };
    let sym = lookup_sym(self_.handle, &objstr::str_get_str(symname_in));
    objint::new_int_from_ull(sym as usize as u64)
}

fn ffimod_var(self_in: Obj, vartype_in: Obj, symname_in: Obj) -> Obj {
    let self_ = unsafe { &*ffimod_ptr(self_in) };
    let rettype = objstr::str_get_str(vartype_in);
    let sym = lookup_sym(self_.handle, &objstr::str_get_str(symname_in));
    let o = malloc::new_obj::<ObjFfiVar>().expect("ffivar");
    unsafe {
        (*o).base.type_ = type_ffivar() as *const ObjType;
        (*o).var = sym;
        (*o).typecode = *rettype.as_bytes().first().unwrap_or(&b'i');
        obj::from_ptr(o as *const ObjFfiVar as *const ())
    }
}

fn ffimod_func(self_in: Obj, rettype_in: Obj, symname_in: Obj, argtypes_in: Obj) -> Obj {
    let self_ = unsafe { &*ffimod_ptr(self_in) };
    let sym = lookup_sym(self_.handle, &objstr::str_get_str(symname_in));
    make_func(rettype_in, sym, argtypes_in)
}

fn lookup_sym(handle: *mut c_void, name: &str) -> *mut c_void {
    let cname = CString::new(name).unwrap_or_else(|_| raise_type("bad symbol name"));
    let sym = unsafe { libc::dlsym(handle, cname.as_ptr()) };
    if sym.is_null() {
        raise::raise(MpRaise::OSError(mperrno::ENOENT));
    }
    sym
}

fn mod_ffi_open(n_args: usize, args: &[Obj]) -> Obj {
    ffimod_make_new(type_ffimod(), n_args, 0, args)
}

struct FfiFuncInner {
    func: *mut c_void,
    rettype: u8,
    argtypes: Vec<u8>,
    cif: Cif,
}

#[repr(C)]
struct ObjFfiFunc {
    base: ObjBase,
    inner: *mut FfiFuncInner,
}

fn ffifunc_ptr(o: Obj) -> *mut ObjFfiFunc {
    obj::as_ptr(o) as *mut ObjFfiFunc
}

fn make_func(rettype_in: Obj, func: *mut c_void, argtypes_in: Obj) -> Obj {
    let rettype_str = objstr::str_get_str(rettype_in);
    let rettype = *rettype_str.as_bytes().first().unwrap_or(&b'v');
    let argtypes = argtypes_from_obj(argtypes_in);
    let params = types_from_argcodes(&argtypes);
    let ret_type = char2type(rettype).unwrap_or_else(|| raise_type("unknown type"));
    let cif = Cif::new(params, ret_type);
    let inner = Box::new(FfiFuncInner {
        func,
        rettype,
        argtypes,
        cif,
    });
    let o = malloc::new_obj::<ObjFfiFunc>().expect("ffifunc");
    unsafe {
        (*o).base.type_ = type_ffifunc() as *const ObjType;
        (*o).inner = Box::into_raw(inner);
        obj::from_ptr(o as *const ObjFfiFunc as *const ())
    }
}

fn mod_ffi_func(rettype: Obj, addr_in: Obj, argtypes: Obj) -> Obj {
    let addr = objint::int_get_truncated(addr_in) as usize as *mut c_void;
    make_func(rettype, addr, argtypes)
}

fn ffifunc_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*ffifunc_ptr(self_in) };
    let func = unsafe { (*self_.inner).func };
    let _ = mpprint::printf(
        print,
        "<ffifunc %p>",
        std::iter::once(VaArg::USize(func as usize)),
    );
}

fn ffifunc_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if n_kw != 0 {
        raise_type("argument num/types mismatch");
    }
    let self_ = unsafe { &*ffifunc_ptr(self_in) };
    let inner = unsafe { &*self_.inner };
    if n_args != inner.argtypes.len() {
        raise_type("argument num/types mismatch");
    }
    let mut pin_bufs: Vec<Vec<u8>> = Vec::new();
    let mut values: Vec<FfiUnion> = Vec::with_capacity(n_args);
    for (i, &arg) in args.iter().enumerate() {
        let ty = inner.argtypes[i];
        if obj::is_str_or_bytes(arg) {
            let (data, _len) = objstr::str_get_data(arg);
            let mut val = FfiUnion { ffi: 0 };
            unsafe {
                val.ffi = data.as_ptr() as libffi::raw::ffi_arg;
            }
            pin_bufs.push(data);
            values.push(val);
        } else {
            values.push(arg_to_union(arg, ty));
        }
    }
    let mut ffi_args = Vec::with_capacity(n_args);
    for (i, val) in values.iter_mut().enumerate() {
        ffi_args.push(match inner.argtypes[i] {
            b'f' if mpconfig::PY_BUILTINS_FLOAT => unsafe { arg(&mut val.flt) },
            b'd' if mpconfig::PY_BUILTINS_FLOAT => unsafe { arg(&mut val.dbl) },
            _ => unsafe { arg(val) },
        });
    }
    let mut retval = FfiUnion { ffi: 0 };
    unsafe {
        if inner.rettype == b'v' {
            inner.cif.call_return_into(
                CodePtr::from_ptr(inner.func),
                &ffi_args,
                Ret::void(),
            );
            return obj::CONST_NONE;
        }
        inner.cif.call_return_into(
            CodePtr::from_ptr(inner.func),
            &ffi_args,
            ret(&mut retval),
        );
    }
    return_ffi_value(&retval, inner.rettype)
}

struct FfiCallbackInner {
    rettype: u8,
    pyfunc: Obj,
    code: CodePtr,
    cif: Cif,
    closure: *mut ffi_closure,
    lock: bool,
}

#[repr(C)]
struct ObjFfiCallback {
    base: ObjBase,
    inner: *mut FfiCallbackInner,
    code: CodePtr,
}

fn fficallback_ptr(o: Obj) -> *mut ObjFfiCallback {
    obj::as_ptr(o) as *mut ObjFfiCallback
}

unsafe extern "C" fn py_ffi_callback(
    cif: &ffi_cif,
    result: &mut FfiUnion,
    args: *const *const c_void,
    inner: &FfiCallbackInner,
) {
    let mut run = || {
        let n = cif.nargs as usize;
        let mut pyargs = vec![obj::OBJ_NULL; n];
        for i in 0..n {
            let arg_ptr = *args.add(i);
            let v = *(arg_ptr as *const i64);
            pyargs[i] = objint::new_int(v as Int);
        }
        let res = call_function_n_kw(inner.pyfunc, n, 0, &pyargs);
        if res != obj::CONST_NONE {
            result.ffi = objint::int_get_truncated(res) as libffi::raw::ffi_arg;
        }
    };
    if inner.lock {
        if mpconfig::ENABLE_SCHEDULER {
            runtime::sched_lock();
        }
        mpstate::gc_lock();
        let mut nlr_buf = NlrBuf::default();
        if nlr::protect(&mut nlr_buf, || run()).is_err() {
            let exc = nlr::ret_val(&nlr_buf).map(Obj).unwrap_or(obj::OBJ_NULL);
            let _ =
                mpprint::print_str(&mpprint::PLAT_PRINT, "Uncaught exception in FFI callback\n");
            if exc != obj::OBJ_NULL {
                obj::print_exception(&mpprint::PLAT_PRINT, exc);
            }
        }
        mpstate::gc_unlock();
        if mpconfig::ENABLE_SCHEDULER {
            runtime::sched_unlock();
        }
    } else {
        run();
    }
}

fn mod_ffi_callback(n_pos: usize, pos: &[Obj], kw: &map::Map) -> Obj {
    if n_pos < 3 {
        raise_type("argument num/types mismatch");
    }
    let rettype_in = pos[0];
    let func_in = pos[1];
    let paramtypes_in = pos[2];
    let mut vals = [ArgVal::Bool(false); 1];
    let allowed = [Arg {
        qst: qstr::from_str("lock"),
        flags: ArgFlag::KwOnly as u16 | ArgFlag::Bool as u16,
        defval: ArgVal::Bool(false),
    }];
    let mut kw_map = map::Map::default();
    map::init(&mut kw_map, kw.used);
    for i in 0..kw.alloc {
        if map::slot_is_filled(kw, i) {
            if let Some(slot) =
                map::lookup(&mut kw_map, kw.table[i].key, map::LookupKind::AddIfNotFound)
            {
                slot.value = kw.table[i].value;
            }
        }
    }
    argcheck::parse_all(
        n_pos.saturating_sub(3),
        &pos[3..],
        &mut kw_map,
        1,
        &allowed,
        &mut vals,
    );
    let lock_in = matches!(vals[0], ArgVal::Bool(true));

    let rettype_str = objstr::str_get_str(rettype_in);
    let rettype = *rettype_str.as_bytes().first().unwrap_or(&b'v');
    let argtypes = argtypes_from_obj(paramtypes_in);
    let params = types_from_argcodes(&argtypes);
    let ret_type = char2type(rettype).unwrap_or_else(|| raise_type("unknown type"));
    let cif = Cif::new(params, ret_type);

    let (closure, code) = low::closure_alloc();
    let mut inner = Box::new(FfiCallbackInner {
        rettype,
        pyfunc: func_in,
        code,
        cif,
        closure,
        lock: lock_in,
    });
    let inner_ptr = inner.as_mut() as *mut FfiCallbackInner;
    unsafe {
        low::prep_closure(
            closure,
            inner.cif.as_raw_ptr(),
            py_ffi_callback,
            inner_ptr,
            code,
        )
        .unwrap_or_else(|_| raise_value("ffi_prep_closure_loc"));
    }

    let o = malloc::new_obj::<ObjFfiCallback>().expect("fficallback");
    unsafe {
        (*o).base.type_ = type_fficallback() as *const ObjType;
        (*o).inner = Box::into_raw(inner);
        (*o).code = code;
        obj::from_ptr(o as *const ObjFfiCallback as *const ())
    }
}

fn fficallback_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*fficallback_ptr(self_in) };
    let _ = mpprint::printf(
        print,
        "<fficallback %p>",
        std::iter::once(VaArg::USize(self_.code.0 as usize)),
    );
}

fn fficallback_cfun(self_in: Obj) -> Obj {
    let self_ = unsafe { &*fficallback_ptr(self_in) };
    objint::new_int_from_ull(self_.code.0 as usize as u64)
}

#[repr(C)]
struct ObjFfiVar {
    base: ObjBase,
    var: *mut c_void,
    typecode: u8,
}

fn ffivar_ptr(o: Obj) -> *mut ObjFfiVar {
    obj::as_ptr(o) as *mut ObjFfiVar
}

fn ffivar_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*ffivar_ptr(self_in) };
    let val = unsafe { *(self_.var as *const i32) };
    let _ = mpprint::printf(
        print,
        "<ffivar @%p: 0x%x>",
        [VaArg::USize(self_.var as usize), VaArg::Int(val as i32)].into_iter(),
    );
}

fn ffivar_get(self_in: Obj) -> Obj {
    let self_ = unsafe { &*ffivar_ptr(self_in) };
    let data = unsafe { std::slice::from_raw_parts(self_.var as *const u8, 8) };
    binary::get_val_array(self_.typecode, data, 0)
}

fn ffivar_set(self_in: Obj, val_in: Obj) -> Obj {
    let self_ = unsafe { &mut *ffivar_ptr(self_in) };
    let data = unsafe { std::slice::from_raw_parts_mut(self_.var as *mut u8, 8) };
    binary::set_val_array(self_.typecode, data, 0, val_in);
    obj::CONST_NONE
}

fn mod_ffi_as_bytearray(ptr: Obj, size: Obj) -> Obj {
    let n = objint::int_get_truncated(size) as usize;
    let p = objint::int_get_truncated(ptr) as usize as *mut u8;
    objarray::new_bytearray_by_ref(n, p)
}

static mut FFIMOD_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_FFIMOD: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { FFIMOD_SLOTS.as_ptr() },
};

static mut FFIFUNC_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_FFIFUNC: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 2,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FFIFUNC_SLOTS.as_ptr() },
};

static mut FFICALLBACK_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_FFICALLBACK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { FFICALLBACK_SLOTS.as_ptr() },
};

static mut FFIVAR_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_FFIVAR: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { FFIVAR_SLOTS.as_ptr() },
};

static mut FFIMOD_DICT: *const () = core::ptr::null();
static mut FFICALLBACK_DICT: *const () = core::ptr::null();
static mut FFIVAR_DICT: *const () = core::ptr::null();

static FFIMOD_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static FFIFUNC_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static FFICALLBACK_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static FFIVAR_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn ffimod_func_bound(_n: usize, a: &[Obj]) -> Obj {
    ffimod_func(a[0], a[1], a[2], a[3])
}

fn ffimod_var_bound(_n: usize, a: &[Obj]) -> Obj {
    ffimod_var(a[0], a[1], a[2])
}

fn init_ffimod_locals_dict() {
    if unsafe { !FFIMOD_DICT.is_null() } {
        return;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("func")),
            value: mkv(4, 4, ffimod_func_bound),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("var")),
            value: mkv(3, 3, ffimod_var_bound),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("addr")),
            value: mk2(ffimod_addr),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("close")),
            value: mk1(ffimod_close),
        },
    ];
    let ptr =
        obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
    unsafe {
        map::init_fixed_table(&mut (*ptr).map, table);
        FFIMOD_DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
    }
}

fn init_fficallback_locals_dict() {
    if unsafe { !FFICALLBACK_DICT.is_null() } {
        return;
    }
    let table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("cfun")),
        value: mk1(fficallback_cfun),
    }];
    let ptr =
        obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
    unsafe {
        map::init_fixed_table(&mut (*ptr).map, table);
        FFICALLBACK_DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
    }
}

fn init_ffivar_locals_dict() {
    if unsafe { !FFIVAR_DICT.is_null() } {
        return;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("get")),
            value: mk1(ffivar_get),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("set")),
            value: mk2(ffivar_set),
        },
    ];
    let ptr =
        obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
    unsafe {
        map::init_fixed_table(&mut (*ptr).map, table);
        FFIVAR_DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
    }
}

pub fn type_ffimod() -> &'static ObjType {
    FFIMOD_INIT.get_or_init(|| {
        init_ffimod_locals_dict();
        unsafe {
            FFIMOD_SLOTS[0] = ffimod_make_new as *const ();
            FFIMOD_SLOTS[1] = ffimod_print as *const ();
            FFIMOD_SLOTS[2] = FFIMOD_DICT;
            TYPE_FFIMOD.name = qstr::from_str("ffimod");
        }
    });
    unsafe { &TYPE_FFIMOD }
}

pub fn type_ffifunc() -> &'static ObjType {
    FFIFUNC_INIT.get_or_init(|| {
        unsafe {
            FFIFUNC_SLOTS[0] = ffifunc_print as *const ();
            FFIFUNC_SLOTS[1] = ffifunc_call as *const ();
            TYPE_FFIFUNC.name = qstr::from_str("ffifunc");
        }
    });
    unsafe { &TYPE_FFIFUNC }
}

pub fn type_fficallback() -> &'static ObjType {
    FFICALLBACK_INIT.get_or_init(|| {
        init_fficallback_locals_dict();
        unsafe {
            FFICALLBACK_SLOTS[0] = fficallback_print as *const ();
            FFICALLBACK_SLOTS[1] = FFICALLBACK_DICT;
            TYPE_FFICALLBACK.name = qstr::from_str("fficallback");
        }
    });
    unsafe { &TYPE_FFICALLBACK }
}

pub fn type_ffivar() -> &'static ObjType {
    FFIVAR_INIT.get_or_init(|| {
        init_ffivar_locals_dict();
        unsafe {
            FFIVAR_SLOTS[0] = ffivar_print as *const ();
            FFIVAR_SLOTS[1] = FFIVAR_DICT;
            TYPE_FFIVAR.name = qstr::from_str("ffivar");
        }
    });
    unsafe { &TYPE_FFIVAR }
}

/// Register built-in `ffi` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !enabled() {
        return obj::OBJ_NULL;
    }
    type_ffimod();
    type_ffifunc();
    type_fficallback();
    type_ffivar();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("ffi")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("open")),
            value: mkv(1, 2, |n, a| mod_ffi_open(n, a)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("callback")),
            value: mk_kw(3, mod_ffi_callback),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("func")),
            value: mk3(mod_ffi_func),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("as_bytearray")),
            value: mk2(mod_ffi_as_bytearray),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("ffi module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("ffi"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_typecodes() {
        assert!(char2type(b'i').is_some());
        assert!(char2type(b'v').is_some());
        assert!(char2type(b'p').is_some());
        assert!(char2type(b'?').is_none());
    }
}
