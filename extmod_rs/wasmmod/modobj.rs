//! rewrite of extmod/wasmmod/modobj.c + extmod/wasmmod/mod.h object types
// symmetry: done

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objfloat;
use py_rs::objint;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use wasmi::Val;

use super::fetch;
use super::forward;
use super::runtime::{self, WasmModule, WasmValKind, MP_WASM_ERRBUF};

static MODULE_STORE: LazyLock<Mutex<HashMap<u64, Box<WasmModule>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_MOD_ID: AtomicU64 = AtomicU64::new(1);

fn module_retain(mod_: Box<WasmModule>) -> u64 {
    let id = NEXT_MOD_ID.fetch_add(1, Ordering::Relaxed);
    forward::registry_add(mod_.as_ref(), id);
    MODULE_STORE.lock().unwrap().insert(id, mod_);
    id
}

fn module_release(id: u64) -> Option<Box<WasmModule>> {
    MODULE_STORE.lock().unwrap().remove(&id)
}

fn module_open(id: u64) -> bool {
    id != 0 && MODULE_STORE.lock().unwrap().contains_key(&id)
}

fn with_module<R>(id: u64, f: impl FnOnce(&WasmModule) -> R) -> Option<R> {
    let store = MODULE_STORE.lock().unwrap();
    store.get(&id).map(|b| f(b))
}

fn with_module_mut<R>(id: u64, f: impl FnOnce(&mut WasmModule) -> R) -> Option<R> {
    let mut store = MODULE_STORE.lock().unwrap();
    store.get_mut(&id).map(|b| f(b))
}

fn py_to_wasm_val(o: Obj, kind: WasmValKind) -> Option<Val> {
    Some(match kind {
        WasmValKind::I32 => Val::I32(obj::get_int_truncated(o) as i32),
        WasmValKind::I64 => Val::I64(objint::int_get_truncated(o) as i64),
        WasmValKind::F32 => Val::F32(objfloat::get_float_to_f(o).into()),
        WasmValKind::F64 => Val::F64(objfloat::get_float_to_d(o).into()),
    })
}

fn wasm_val_to_py(v: &Val) -> Obj {
    match v {
        Val::I32(x) => obj::new_small_int(*x as isize),
        Val::I64(x) => objint::new_int_from_ll(*x),
        Val::F32(x) => objfloat::new_float(f32::from(*x) as f64),
        Val::F64(x) => objfloat::new_float(f64::from(*x)),
        _ => obj::CONST_NONE,
    }
}

fn call_export_py(mod_id: u64, fname: &str, args: &[Obj]) -> Obj {
    let (pkinds, rkinds) = with_module(mod_id, |m| runtime::module_func_type_kinds(m, fname))
        .flatten()
        .unwrap_or_else(|| raise_value("wasm export missing or non-numeric"));
    if args.len() != pkinds.len() {
        raise::raise(MpRaise::TypeError("wrong number of args"));
    }
    let mut inputs = Vec::with_capacity(pkinds.len());
    for (i, kind) in pkinds.iter().enumerate() {
        inputs
            .push(py_to_wasm_val(args[i], *kind).unwrap_or_else(|| {
                raise::raise(MpRaise::TypeError("unsupported wasm value type"))
            }));
    }
    let mut outputs: Vec<Val> = rkinds
        .iter()
        .map(|k| match k {
            WasmValKind::I32 => Val::I32(0),
            WasmValKind::I64 => Val::I64(0),
            WasmValKind::F32 => Val::F32(0f32.into()),
            WasmValKind::F64 => Val::F64(0f64.into()),
        })
        .collect();
    let mut err = [0u8; MP_WASM_ERRBUF];
    let ok = with_module(mod_id, |m| {
        runtime::module_call_vals(m, fname, &inputs, &mut outputs, &mut err)
    })
    .unwrap_or(false);
    if !ok {
        let msg = std::str::from_utf8(&err)
            .unwrap_or("")
            .trim_end_matches('\0');
        raise_runtime_msg(if msg.is_empty() {
            "wasm call failed".into()
        } else {
            format!("wasm: {msg}")
        });
    }
    if rkinds.is_empty() {
        obj::CONST_NONE
    } else if rkinds.len() == 1 {
        wasm_val_to_py(&outputs[0])
    } else {
        let items: Vec<Obj> = outputs.iter().map(wasm_val_to_py).collect();
        objtuple::new_tuple(items.len(), Some(&items))
    }
}

/// Guest→guest forward hook registered on the wasmi runtime.
fn invoke_export_by_name(
    pack: &str,
    func: &str,
    inputs: &[Val],
    outputs: &mut [Val],
) -> Result<(), ()> {
    let mod_id = forward::registry_mod_id(pack).ok_or(())?;
    match with_module(mod_id, |m| {
        runtime::module_invoke_export(m, func, inputs, outputs)
    }) {
        Some(r) => r,
        None => Err(()),
    }
}

fn call_export_i32_by_id(id: u64, func: &str, args: &[i32], out: &mut i32) -> bool {
    with_module(id, |m| {
        let mut err = [0u8; MP_WASM_ERRBUF];
        runtime::module_call_i32(m, func, args, out, &mut err)
    })
    .unwrap_or(false)
}

fn raise_value(msg: &'static str) -> ! {
    raise::raise(MpRaise::ValueError(msg));
}

fn raise_runtime_msg(msg: impl Into<String>) -> ! {
    let s = msg.into();
    let exc = objexcept::new_exception_args(
        objexcept::type_runtime_error(),
        1,
        &[objstr::new_str(s.as_bytes())],
    );
    raise::raise_obj(exc);
}

#[repr(C)]
pub struct ObjWasmModule {
    base: ObjBase,
    mod_id: u64,
    pack_name: Qstr,
}

#[repr(C)]
struct ObjWasmFunc {
    base: ObjBase,
    mod_id: u64,
    export_name: Qstr,
}

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
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

static mut BF0: [*const (); 1] = [bf_call0 as *const ()];
static mut BF1: [*const (); 1] = [bf_call1 as *const ()];
static mut BF2: [*const (); 1] = [bf_call2 as *const ()];
static mut BF3: [*const (); 1] = [bf_call3 as *const ()];
static mut BFV: [*const (); 1] = [bf_callv as *const ()];

static BT0: ObjType = ObjType {
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
    slots: unsafe { BF0.as_ptr() },
};
static BT1: ObjType = ObjType {
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
    slots: unsafe { BF1.as_ptr() },
};
static BT2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
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
    slot_index_make_new: 0,
    slots: unsafe { BF2.as_ptr() },
};
static BT3: ObjType = ObjType {
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
    slots: unsafe { BF3.as_ptr() },
};
static BTV: ObjType = ObjType {
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
    slots: unsafe { BFV.as_ptr() },
};

fn bf_call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn bf_call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn bf_call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn bf_call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}
fn bf_callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("wasm bf0");
    unsafe {
        (*o).base.type_ = &BT0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("wasm bf1");
    unsafe {
        (*o).base.type_ = &BT1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("wasm bf2");
    unsafe {
        (*o).base.type_ = &BT2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("wasm bf3");
    unsafe {
        (*o).base.type_ = &BT3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("wasm bfv");
    unsafe {
        (*o).base.type_ = &BTV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn wasm_module_ptr(o: Obj) -> *mut ObjWasmModule {
    obj::as_ptr(o) as *mut ObjWasmModule
}

fn wasm_module_ensure_open(self_: &ObjWasmModule) {
    if !module_open(self_.mod_id) {
        raise_value("wasm module closed");
    }
}

fn wasm_module_close(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *wasm_module_ptr(self_in) };
    if self_.mod_id != 0 {
        if let Some(mod_) = module_release(self_.mod_id) {
            runtime::module_close(mod_);
        }
        self_.mod_id = 0;
        self_.pack_name = 0;
    }
    obj::CONST_NONE
}

fn wasm_module_call(n: usize, args: &[Obj]) -> Obj {
    if n < 2 {
        raise::raise(MpRaise::TypeError("call needs export name"));
    }
    let self_ = unsafe { &*wasm_module_ptr(args[0]) };
    wasm_module_ensure_open(self_);
    let (fname_bytes, fname_len) = objstr::get_str_data_len(args[1]);
    let fname = std::str::from_utf8(&fname_bytes[..fname_len]).unwrap_or("");
    call_export_py(self_.mod_id, fname, &args[2..n])
}

fn wasm_module_memory_read(self_in: Obj, off_in: Obj, n_in: Obj) -> Obj {
    let self_ = unsafe { &*wasm_module_ptr(self_in) };
    wasm_module_ensure_open(self_);
    let off = obj::get_int_truncated(off_in) as u32;
    let n = obj::get_int_truncated(n_in);
    if n < 0 {
        raise_value("memory_read: negative length");
    }
    let n = n as u32;
    let mut buf = vec![0u8; n as usize];
    let ok = with_module(self_.mod_id, |m| {
        runtime::module_mem_read(m, off, n, &mut buf)
    })
    .unwrap_or(false);
    if !ok {
        raise_value("memory_read: bad offset/length");
    }
    objstr::new_bytes(&buf)
}

fn wasm_module_memory_write(self_in: Obj, off_in: Obj, data_in: Obj) -> Obj {
    let self_ = unsafe { &*wasm_module_ptr(self_in) };
    wasm_module_ensure_open(self_);
    let off = obj::get_int_truncated(off_in) as u32;
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(data_in, &mut bufinfo, obj::BUFFER_READ);
    let src = bufinfo.as_bytes();
    let ok = with_module(self_.mod_id, |m| {
        runtime::module_mem_write(m, off, src.len() as u32, src)
    })
    .unwrap_or(false);
    if !ok {
        raise_value("memory_write: bad offset/length");
    }
    obj::CONST_NONE
}

fn wasm_module_memory_alloc(self_in: Obj, n_in: Obj) -> Obj {
    let self_ = unsafe { &*wasm_module_ptr(self_in) };
    wasm_module_ensure_open(self_);
    let n = obj::get_int_truncated(n_in);
    if n < 0 {
        raise_value("memory_alloc: negative size");
    }
    let off = with_module(self_.mod_id, |m| runtime::module_mem_alloc(m, n as u32)).unwrap_or(0);
    if off == 0 && n != 0 {
        raise::raise(MpRaise::OSError(12));
    }
    objint::new_int_from_uint(off as usize)
}

fn wasm_module_memory_free(self_in: Obj, off_in: Obj) -> Obj {
    let self_ = unsafe { &*wasm_module_ptr(self_in) };
    wasm_module_ensure_open(self_);
    let off = obj::get_int_truncated(off_in) as u32;
    let _ = with_module_mut(self_.mod_id, |m| runtime::module_mem_free(m, off));
    obj::CONST_NONE
}

fn wasm_func_call(self_in: Obj, n: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if n_kw != 0 {
        raise::raise(MpRaise::TypeError("unexpected kwargs"));
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjWasmFunc) };
    if !module_open(self_.mod_id) {
        raise_value("wasm module closed");
    }
    let fname = qstr::str_from_qstr(self_.export_name).unwrap_or_default();
    call_export_py(self_.mod_id, &fname, args)
}

fn module_locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("call")),
                value: mkv(2, 255, wasm_module_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: mk1(wasm_module_close),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("memory_read")),
                value: mk3(wasm_module_memory_read),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("memory_write")),
                value: mk3(wasm_module_memory_write),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("memory_alloc")),
                value: mk2(wasm_module_memory_alloc),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("memory_free")),
                value: mk2(wasm_module_memory_free),
            },
        ];
        let ptr = obj::malloc_helper(
            core::mem::size_of::<objdict::ObjDict>(),
            objdict::type_dict(),
        ) as *mut objdict::ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const objdict::ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

static mut WASM_MODULE_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut TYPE_WASM_MODULE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: 0,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
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
    slots: unsafe { WASM_MODULE_SLOTS.as_ptr() },
};

static mut WASM_FUNC_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_WASM_FUNC: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: 0,
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
    slots: unsafe { WASM_FUNC_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    TYPE_INIT.get_or_init(|| unsafe {
        runtime::set_invoke_by_name(invoke_export_by_name);
        runtime::set_call_export_i32(call_export_i32_by_id);
        TYPE_WASM_MODULE.name = qstr::from_str("WasmModule");
        WASM_MODULE_SLOTS[3] = module_locals_dict();
        TYPE_WASM_FUNC.name = qstr::from_str("WasmFunc");
        WASM_FUNC_SLOTS[1] = wasm_func_call as *const ();
    });
}

/// `mp_type_wasm_module`
pub fn type_wasm_module() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_WASM_MODULE }
}

fn type_wasm_func() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_WASM_FUNC }
}

/// `mp_wasm_wrap_loaded`
pub fn wrap_loaded(mod_: Box<WasmModule>) -> Obj {
    let id = module_retain(mod_);
    let o = malloc::new_obj::<ObjWasmModule>().expect("WasmModule");
    unsafe {
        (*o).base.type_ = type_wasm_module();
        (*o).mod_id = id;
        (*o).pack_name = 0;
        obj::from_ptr(o as *const ObjWasmModule as *const ())
    }
}

/// Set pack name qstr on a wrapped module (after load_pack).
pub fn set_pack_name(wasm_obj: Obj, pack_name: Qstr) {
    if obj::is_exact_type(wasm_obj, type_wasm_module()) {
        unsafe {
            (*wasm_module_ptr(wasm_obj)).pack_name = pack_name;
        }
    }
}

/// `mp_wasm_func_new`
pub fn func_new(mod_id: u64, export_name: Qstr) -> Obj {
    let o = malloc::new_obj::<ObjWasmFunc>().expect("WasmFunc");
    unsafe {
        (*o).base.type_ = type_wasm_func();
        (*o).mod_id = mod_id;
        (*o).export_name = export_name;
        obj::from_ptr(o as *const ObjWasmFunc as *const ())
    }
}

/// Module id from a wrapped `WasmModule` object.
pub fn module_id_from_obj(wasm_obj: Obj) -> Option<u64> {
    if !obj::is_exact_type(wasm_obj, type_wasm_module()) {
        return None;
    }
    let id = unsafe { (*wasm_module_ptr(wasm_obj)).mod_id };
    if id == 0 {
        None
    } else {
        Some(id)
    }
}

/// Call a nullary export on a wrapped module (e.g. ``mp_pack_load``).
pub fn call0_on_obj(wasm_obj: Obj, func: &str, errbuf: &mut [u8]) -> bool {
    let Some(id) = module_id_from_obj(wasm_obj) else {
        return false;
    };
    with_module(id, |m| {
        let mut out = 0i32;
        runtime::module_call0(m, func, &mut out, errbuf)
    })
    .unwrap_or(false)
}

/// `wasm_module_close`
pub fn close_module(wasm_obj: Obj) -> Obj {
    wasm_module_close(wasm_obj)
}

fn load_bytes(code: &[u8], path_hint: Option<&str>, name: Option<&str>) -> Obj {
    let mut err = [0u8; MP_WASM_ERRBUF];
    let mod_ =
        runtime::module_load_ex(code, None, name, path_hint, &mut err).unwrap_or_else(|| {
            let msg = std::str::from_utf8(&err)
                .unwrap_or("")
                .trim_end_matches('\0');
            raise_runtime_msg(if msg.is_empty() {
                "wasm load failed"
            } else {
                msg
            });
        });
    wrap_loaded(mod_)
}

/// `mod_wasm_load` — bytes or path → load + wrap.
pub fn load(data: Obj) -> Obj {
    if obj::is_str_or_bytes(data) {
        let (bytes, len) = objstr::get_str_data_len(data);
        let path = std::str::from_utf8(&bytes[..len]).ok();
        if let Some(path) = path {
            let mut fetch_err = [0u8; 64];
            if let Some(code) = fetch::fetch(path, &mut fetch_err) {
                return load_bytes(&code, Some(path), Some(path));
            }
            raise::raise(MpRaise::OSError(2));
        }
    }
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(data, &mut bufinfo, obj::BUFFER_READ);
    let code = bufinfo.as_bytes();
    load_bytes(code, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::runtime;

    fn setup() {
        let _ = gc::init();
        py_rs::qstr::init();
        mpstate::init();
        runtime::init();
        mpstate::with_vm(|vm| {
            if vm.mp_loaded_modules_dict == obj::OBJ_NULL {
                vm.mp_loaded_modules_dict =
                    objdict::new_dict(py_rs::mpconfig::LOADED_MODULES_DICT_SIZE as usize);
            }
        });
        init_types();
    }

    #[test]
    fn wrap_and_close_module() {
        setup();
        let mut err = [0u8; MP_WASM_ERRBUF];
        let m = super::runtime::module_load_ex(b"\0asm\x01\0\0\0", None, Some("t"), None, &mut err)
            .expect("load stub wasm");
        let w = wrap_loaded(m);
        assert!(obj::is_exact_type(w, type_wasm_module()));
        close_module(w);
    }
}
