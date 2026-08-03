//! rewrite of extmod/wasmmod/wasmmod.c + modapi.c + modobj.c (Python surface)
//!
//! Remaining gaps:
//! - `install_hook` namespace discovery / VFS listdir parity incomplete (leaf packs work)
//! - AOT pack paths when `PY_WASM_AOT` enabled
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mpstate;
use py_rs::nlr;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime as mp_runtime;

use super::fetch;
use super::host;
use super::modobj;
use super::packload;
use super::verify;

pub const VERIFY_CONST: i32 = verify::WASM_VERIFY as i32;
pub const AOT_CONST: i32 = mpconfig::PY_WASM_AOT as i32;
pub const JIT_CONST: i32 = mpconfig::PY_WASM_JIT as i32;
pub const FAST_JIT_CONST: i32 = mpconfig::PY_WASM_FAST_JIT as i32;
pub const MODE_CONST: i32 = 0; // Mode_Interp when no JIT features

const WASM_VERSION: &str = "0.1.1-alpha";

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];

static T0: ObjType = ObjType {
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
    slots: unsafe { F0.as_ptr() },
};
static T1: ObjType = ObjType {
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
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
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
    slots: unsafe { F2.as_ptr() },
};
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
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
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("wasm fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("wasm fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("wasm fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("wasm fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn type_wasm_module() -> &'static ObjType {
    modobj::type_wasm_module()
}

/// `ends_with` (AOT path helpers in wasmmod.c)
pub fn ends_with(s: &str, suf: &str) -> bool {
    s.ends_with(suf)
}

/// `replace_suffix` — writes into `out` when `path` ends with `old_suf`.
pub fn replace_suffix(path: &str, old_suf: &str, new_suf: &str, out: &mut String) -> bool {
    if !path.ends_with(old_suf) {
        return false;
    }
    out.clear();
    out.push_str(&path[..path.len() - old_suf.len()]);
    out.push_str(new_suf);
    true
}

/// `path_is_package_init`
pub fn path_is_package_init(path: &str) -> bool {
    path == "__init__.py" || path.ends_with("/__init__.py")
}

/// `stem_from_path`
pub fn stem_from_path(path: &str) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut n = base.len();
    if base.ends_with(".wasm") {
        n -= 5;
    } else if base.ends_with(".aot") {
        n -= 4;
    }
    if n == 8 && base.as_bytes().get(..8) == Some(b"__init__") {
        if let Some(slash) = path.rfind(['/', '\\']) {
            let dir = &path[..slash];
            return dir.rsplit(['/', '\\']).next().unwrap_or(dir).to_string();
        }
    }
    base[..n].to_string()
}

/// `path_to_dotted` — append a pack-relative path onto `pack_name`.
pub fn path_to_dotted(pack_name: &str, path: &str) -> String {
    let mut n = path.len();
    if path.ends_with(".py") {
        n -= 3;
    } else if path.ends_with(".mpy") {
        n -= 4;
    }
    let trimmed = &path[..n];
    if trimmed == "__init__" {
        return pack_name.to_string();
    }
    let trimmed = trimmed.strip_suffix("/__init__").unwrap_or(trimmed);
    let mut out = String::with_capacity(pack_name.len() + trimmed.len() + 2);
    out.push_str(pack_name);
    out.push('.');
    for c in trimmed.chars() {
        out.push(if c == '/' { '.' } else { c });
    }
    out
}

fn path_ensure() {
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_path == obj::OBJ_NULL
            || !obj::is_exact_type(vm.mp_wasm_path, objlist::type_list())
        {
            vm.mp_wasm_path = objlist::new_list(0, None);
        }
    });
}

fn arch_ensure() {
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_arch == obj::OBJ_NULL
            || !obj::is_exact_type(vm.mp_wasm_arch, objlist::type_list())
        {
            vm.mp_wasm_arch = objlist::new_list(0, None);
            if !mpconfig::WASM_PACK_ARCH.is_empty() {
                objlist::list_append(
                    vm.mp_wasm_arch,
                    objstr::new_str(mpconfig::WASM_PACK_ARCH.as_bytes()),
                );
            }
        }
    });
}

fn path_obj() -> Obj {
    path_ensure();
    mpstate::with_vm(|vm| vm.mp_wasm_path)
}

fn arch_obj() -> Obj {
    arch_ensure();
    mpstate::with_vm(|vm| vm.mp_wasm_arch)
}

fn raise_runtime(msg: &'static str) -> ! {
    raise::raise(MpRaise::RuntimeError(msg));
}

fn raise_not_implemented(msg: &'static str) -> ! {
    let exc = objexcept::new_exception_args(
        objexcept::type_not_implemented_error(),
        1,
        &[objstr::new_str(msg.as_bytes())],
    );
    raise::raise_obj(exc);
}

fn py_load(data: Obj) -> Obj {
    modobj::load(data)
}

fn py_load_pack(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        raise::raise(MpRaise::TypeError("load_pack needs path or bytes"));
    }
    let name_override = if n > 1 && args[1] != obj::CONST_NONE {
        let (bytes, len) = objstr::get_str_data_len(args[1]);
        std::str::from_utf8(&bytes[..len])
            .ok()
            .map(|s| s.to_string())
    } else {
        None
    };
    if obj::is_str_or_bytes(args[0]) {
        let (bytes, len) = objstr::get_str_data_len(args[0]);
        let path = std::str::from_utf8(&bytes[..len]).unwrap_or("");
        return packload::load_pack_path(path, name_override.as_deref());
    }
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut bufinfo, obj::BUFFER_READ);
    let code = bufinfo.as_bytes();
    packload::load_pack_from_parts(code, None, None, name_override.as_deref())
}

fn py_unload(name: Obj) -> Obj {
    let (bytes, len) = objstr::get_str_data_len(name);
    let s = std::str::from_utf8(&bytes[..len]).unwrap_or("");
    packload::unload(s)
}

fn py_import_wasm(name: Obj) -> Obj {
    let (bytes, len) = objstr::get_str_data_len(name);
    let s = std::str::from_utf8(&bytes[..len]).unwrap_or("");
    packload::import_wasm(s)
}

fn wasm_path_append_unique(root: &str) {
    path_ensure();
    mpstate::with_vm(|vm| {
        let list = vm.mp_wasm_path;
        let (n, items) = objlist::list_get(list);
        for i in 0..n {
            if !obj::is_str_or_bytes(items[i]) {
                continue;
            }
            let (data, elen) = objstr::get_str_data_len(items[i]);
            let Ok(existing) = std::str::from_utf8(&data[..elen]) else {
                continue;
            };
            if existing == root {
                return;
            }
            let rlen = root.len();
            if !existing.is_empty()
                && existing.as_bytes().last() == Some(&b'/')
                && elen == rlen + 1
                && existing.starts_with(root)
            {
                return;
            }
            if !root.is_empty()
                && root.as_bytes().last() == Some(&b'/')
                && rlen == elen + 1
                && root.starts_with(existing)
            {
                return;
            }
        }
        objlist::list_append(list, objstr::new_str(root.as_bytes()));
    });
}

fn call_prev_import(prev: Obj, n: usize, args: &[Obj]) -> Obj {
    mp_runtime::call_function_n_kw(prev, n, 0, args)
}

fn py_import_hook(n: usize, args: &[Obj]) -> Obj {
    let prev = mpstate::with_vm(|vm| {
        if vm.mp_wasm_prev_import != obj::OBJ_NULL {
            vm.mp_wasm_prev_import
        } else {
            py_rs::builtinimport::builtin___import___obj()
        }
    });

    if packload::import_hook_depth() == 0 && n >= 1 && obj::is_str_or_bytes(args[0]) {
        let (bytes, len) = objstr::get_str_data_len(args[0]);
        if let Ok(name) = std::str::from_utf8(&bytes[..len]) {
            let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
            let existing = objdict::dict_get(loaded, obj::new_qstr(qstr::from_str(name)));
            if existing != obj::OBJ_NULL {
                return call_prev_import(prev, n, args);
            }
            if let Some(path) = super::finder::find_pack_on_wasm_path(name) {
                let mut nlr_pack = nlr::NlrBuf::default();
                match nlr::protect(&mut nlr_pack, || {
                    packload::import_wasm_at(name, Some(&path));
                }) {
                    Ok(_) => return call_prev_import(prev, n, args),
                    Err(val) => {
                        nlr::jump(val);
                    }
                }
            }
        }
    }

    let mut nlr_buf = nlr::NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || call_prev_import(prev, n, args)) {
        Ok(res) => res,
        Err(val) => {
            let exc = obj::from_ptr(val as *const ());
            let import_err =
                obj::from_ptr(objexcept::type_import_error() as *const obj::ObjType as *const ());
            if !objexcept::exception_match(exc, import_err) || packload::import_hook_depth() > 0 {
                nlr::jump(val);
            }
            if n >= 1 && obj::is_str_or_bytes(args[0]) {
                let (bytes, len) = objstr::get_str_data_len(args[0]);
                if let Ok(name) = std::str::from_utf8(&bytes[..len]) {
                    let mut nlr2 = nlr::NlrBuf::default();
                    match nlr::protect(&mut nlr2, || packload::import_wasm(name)) {
                        Ok(_) => return call_prev_import(prev, n, args),
                        Err(val2) => {
                            let exc2 = obj::from_ptr(val2 as *const ());
                            if objexcept::exception_match(exc2, import_err) {
                                nlr::jump(val);
                            }
                            nlr::jump(val2);
                        }
                    }
                }
            }
            nlr::jump(val);
        }
    }
}

fn py_install_hook(_n: usize, args: &[Obj]) -> Obj {
    if !mpconfig::CAN_OVERRIDE_BUILTINS {
        raise_not_implemented("wasm.install_hook requires MICROPY_CAN_OVERRIDE_BUILTINS");
    }
    if _n >= 1 && args[0] != obj::CONST_NONE && obj::is_str_or_bytes(args[0]) {
        let (bytes, len) = objstr::get_str_data_len(args[0]);
        if let Ok(url) = std::str::from_utf8(&bytes[..len]) {
            if !fetch::uri_is_http(url) {
                raise::raise(MpRaise::ValueError("install_hook: url must be http(s)"));
            }
            let mut root = String::from(url);
            if root.is_empty() || !root.ends_with('/') {
                root.push('/');
            }
            wasm_path_append_unique(&root);
        }
    }
    let prev_set = mpstate::with_vm(|vm| vm.mp_wasm_prev_import != obj::OBJ_NULL);
    if prev_set {
        return obj::CONST_NONE;
    }
    let builtins = objmodule::module_get_builtin(qstr::from_str("builtins"), false);
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    mp_runtime::load_method_maybe(builtins, qstr::from_str("__import__"), &mut dest);
    mpstate::with_vm(|vm| {
        vm.mp_wasm_prev_import = if dest[0] != obj::OBJ_NULL {
            dest[0]
        } else {
            py_rs::builtinimport::builtin___import___obj()
        };
    });
    mp_runtime::store_attr(
        builtins,
        qstr::from_str("__import__"),
        mkv(1, 5, py_import_hook),
    );
    obj::CONST_NONE
}

fn py_uninstall_hook() -> Obj {
    if !mpconfig::CAN_OVERRIDE_BUILTINS {
        return obj::CONST_NONE;
    }
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_prev_import == obj::OBJ_NULL {
            return;
        }
        let builtins = objmodule::module_get_builtin(qstr::from_str("builtins"), false);
        mp_runtime::store_attr(
            builtins,
            qstr::from_str("__import__"),
            vm.mp_wasm_prev_import,
        );
        vm.mp_wasm_prev_import = obj::OBJ_NULL;
    });
    packload::set_import_hook_depth(0);
    obj::CONST_NONE
}

fn py_verify(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        return obj::new_bool(verify::get_verify_enabled());
    }
    verify::set_verify_enabled(obj::is_true(args[0]));
    obj::CONST_NONE
}

fn py_add_trust(key: Obj) -> Obj {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(key, &mut bufinfo, obj::BUFFER_READ);
    let data = bufinfo.as_bytes();
    if !verify::trust_add(data) {
        raise::raise(MpRaise::ValueError("wasm.add_trust: bad key or trust full"));
    }
    obj::CONST_NONE
}

fn py_trust_clear() -> Obj {
    verify::trust_clear();
    obj::CONST_NONE
}

fn py_trust_count() -> Obj {
    obj::new_small_int(verify::trust_count() as isize)
}

fn py_host_set(slot: Obj, callable: Obj) -> Obj {
    let s = obj::get_int_truncated(slot) as i32;
    if !host::set_slot(s, callable) {
        raise::raise(MpRaise::ValueError("wasm.host_set: bad slot or callable"));
    }
    obj::CONST_NONE
}

fn py_host_get(slot: Obj) -> Obj {
    host::get_slot(obj::get_int_truncated(slot) as i32)
}

fn py_host_clear(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        host::clear_all();
        return obj::CONST_NONE;
    }
    if !host::set_slot(obj::get_int_truncated(args[0]) as i32, obj::CONST_NONE) {
        raise::raise(MpRaise::ValueError("wasm.host_clear: bad slot"));
    }
    obj::CONST_NONE
}

fn py_mem_alloc(arg: Obj) -> Obj {
    if obj::is_int(arg) {
        let n = obj::get_int_truncated(arg);
        if n < 0 {
            raise::raise(MpRaise::ValueError("mem_alloc: negative size"));
        }
        let c = host::mem_alloc(n as u32);
        if c == 0 && n != 0 {
            raise::raise(MpRaise::OSError(12));
        }
        return obj::new_small_int(c as isize);
    }
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(arg, &mut bufinfo, obj::BUFFER_READ);
    let data = bufinfo.as_bytes();
    let c = host::mem_alloc_copy(data, bufinfo.len as u32);
    if c == 0 && bufinfo.len != 0 {
        raise::raise(MpRaise::OSError(12));
    }
    obj::new_small_int(c as isize)
}

fn py_mem_free(cookie: Obj) -> Obj {
    if !host::mem_free(obj::get_int_truncated(cookie) as i32) {
        raise::raise(MpRaise::ValueError("mem_free: bad cookie"));
    }
    obj::CONST_NONE
}

fn py_mem_get(cookie: Obj) -> Obj {
    let c = obj::get_int_truncated(cookie) as i32;
    if !host::mem_valid(c) {
        raise::raise(MpRaise::ValueError("mem_get: bad cookie"));
    }
    let data = host::mem_bytes(c).unwrap_or_default();
    objstr::new_bytes(&data)
}

fn py_mem_set(cookie: Obj, data: Obj) -> Obj {
    let c = obj::get_int_truncated(cookie) as i32;
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(data, &mut bufinfo, obj::BUFFER_READ);
    let slice = bufinfo.as_bytes();
    if !host::mem_set(c, slice, bufinfo.len as u32) {
        raise::raise(MpRaise::ValueError("mem_set: bad cookie or OOM"));
    }
    obj::CONST_NONE
}

fn py_mem_clear() -> Obj {
    host::mem_clear_all();
    obj::CONST_NONE
}

fn py_handle_register(o: Obj) -> Obj {
    let h = host::handle_register(o);
    if h <= 0 {
        raise::raise(MpRaise::OSError(12));
    }
    obj::new_small_int(h as isize)
}

fn py_handle_resolve(handle: Obj) -> Obj {
    host::handle_resolve(obj::get_int_truncated(handle) as i32)
}

fn py_handle_free(handle: Obj) -> Obj {
    if !host::handle_free(obj::get_int_truncated(handle) as i32) {
        raise::raise(MpRaise::ValueError("handle_free: bad handle"));
    }
    obj::CONST_NONE
}

fn py_handle_clear() -> Obj {
    host::handle_clear_all();
    obj::CONST_NONE
}

fn py_init() -> Obj {
    mpstate::with_vm(|vm| {
        vm.mp_wasm_path = objlist::new_list(0, None);
        vm.mp_wasm_arch = objlist::new_list(0, None);
        if !mpconfig::WASM_PACK_ARCH.is_empty() {
            objlist::list_append(
                vm.mp_wasm_arch,
                objstr::new_str(mpconfig::WASM_PACK_ARCH.as_bytes()),
            );
        }
        vm.mp_wasm_prev_import = obj::OBJ_NULL;
        vm.mp_wasm_host_slots = obj::OBJ_NULL;
        vm.mp_wasm_handles = obj::OBJ_NULL;
    });
    verify::set_verify_enabled(true);
    verify::trust_init_session();
    host::clear_all();
    host::mem_clear_all();
    host::handle_clear_all();
    obj::CONST_NONE
}

/// Register built-in `wasm` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_WASM {
        return obj::OBJ_NULL;
    }
    path_ensure();
    arch_ensure();

    let version = objstr::new_str(WASM_VERSION.as_bytes());
    let path = path_obj();
    let arch = arch_obj();
    let wasm_module_type = obj::from_ptr(type_wasm_module() as *const ObjType as *const ());

    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("wasm")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("version")),
            value: version,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("path")),
            value: path,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("arch")),
            value: arch,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("VERIFY")),
            value: obj::new_small_int(VERIFY_CONST as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("verify")),
            value: mkv(0, 1, py_verify),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("AOT")),
            value: obj::new_small_int(AOT_CONST as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("JIT")),
            value: obj::new_small_int(JIT_CONST as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("FAST_JIT")),
            value: obj::new_small_int(FAST_JIT_CONST as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("MODE")),
            value: obj::new_small_int(MODE_CONST as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("load")),
            value: mk1(py_load),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("load_pack")),
            value: mkv(1, 2, py_load_pack),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("import_wasm")),
            value: mk1(py_import_wasm),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("install_hook")),
            value: mkv(0, 1, py_install_hook),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("uninstall_hook")),
            value: mk0(py_uninstall_hook),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("unload")),
            value: mk1(py_unload),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("add_trust")),
            value: mk1(py_add_trust),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("trust_clear")),
            value: mk0(py_trust_clear),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("trust_count")),
            value: mk0(py_trust_count),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("host_set")),
            value: mk2(py_host_set),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("host_get")),
            value: mk1(py_host_get),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("host_clear")),
            value: mkv(0, 1, py_host_clear),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("mem_alloc")),
            value: mk1(py_mem_alloc),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("mem_free")),
            value: mk1(py_mem_free),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("mem_get")),
            value: mk1(py_mem_get),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("mem_set")),
            value: mk2(py_mem_set),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("mem_clear")),
            value: mk0(py_mem_clear),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("handle_register")),
            value: mk1(py_handle_register),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("handle_resolve")),
            value: mk1(py_handle_resolve),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("handle_free")),
            value: mk1(py_handle_free),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("handle_clear")),
            value: mk0(py_handle_clear),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("WasmModule")),
            value: wasm_module_type,
        },
    ];

    if mpconfig::MODULE_BUILTIN_INIT {
        table.insert(
            1,
            MapElem {
                key: obj::new_qstr(qstr::from_str("__init__")),
                value: mk0(py_init),
            },
        );
    }

    let ctx = malloc::new_obj::<ModuleContext>().expect("wasm module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("wasm"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_dotted_strips_py() {
        assert_eq!(path_to_dotted("mypack", "sub/mod.py"), "mypack.sub.mod");
    }

    #[test]
    fn stem_from_wasm_file() {
        assert_eq!(stem_from_path("dir/pkg.wasm"), "pkg");
    }
}
