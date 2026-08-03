//! rewrite of extmod/wasmmod/host.c + extmod/wasmmod/host.h
// symmetry: done

use std::sync::Mutex;

use py_rs::mpconfig;
use py_rs::mpstate;
use py_rs::obj::{self, Obj};
use py_rs::objdict;
use py_rs::objlist;
use py_rs::objstr;
use py_rs::qstr;

use super::forward;
use super::pack::HOST_MODULE;
use super::runtime;

pub(crate) const HOST_NAME_MAX: usize = 64;

fn slots_enabled() -> bool {
    mpconfig::PY_WASM
}

fn host_slots_ensure() {
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_host_slots != obj::OBJ_NULL
            && obj::is_exact_type(vm.mp_wasm_host_slots, objlist::type_list())
        {
            return;
        }
        vm.mp_wasm_host_slots = objlist::new_list(0, None);
    });
}

fn handles_ensure() {
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_handles != obj::OBJ_NULL
            && obj::is_exact_type(vm.mp_wasm_handles, objlist::type_list())
        {
            return;
        }
        vm.mp_wasm_handles = objlist::new_list(0, None);
    });
}

fn host_slots_grow_to(need_len: usize) {
    host_slots_ensure();
    mpstate::with_vm(|vm| {
        let list = vm.mp_wasm_host_slots;
        let (len, _) = objlist::list_get(list);
        for _ in len..need_len {
            objlist::list_append(list, obj::CONST_NONE);
        }
    });
}

fn list_set_item(list: Obj, idx: usize, val: Obj) {
    let list_ptr = obj::as_ptr(list) as *mut objlist::ObjList;
    unsafe {
        if idx >= (*list_ptr).len {
            objlist::list_append(list, obj::CONST_NONE);
        }
        *(*list_ptr).items.add(idx) = val;
    }
}

/// `mp_wasm_host_clear_all`
pub fn clear_all() {
    if !slots_enabled() {
        return;
    }
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_host_slots == obj::OBJ_NULL
            || !obj::is_exact_type(vm.mp_wasm_host_slots, objlist::type_list())
        {
            return;
        }
        let len = objlist::list_get(vm.mp_wasm_host_slots).0;
        for i in 0..len {
            list_set_item(vm.mp_wasm_host_slots, i, obj::CONST_NONE);
        }
    });
}

/// `mp_wasm_host_slot_count`
pub fn slot_count() -> usize {
    if !slots_enabled() {
        return 0;
    }
    mpstate::with_vm(|vm| {
        if vm.mp_wasm_host_slots == obj::OBJ_NULL
            || !obj::is_exact_type(vm.mp_wasm_host_slots, objlist::type_list())
        {
            0
        } else {
            objlist::list_get(vm.mp_wasm_host_slots).0
        }
    })
}

fn slot_callable(slot: i32) -> Obj {
    if slot < 0 {
        return obj::OBJ_NULL;
    }
    host_slots_ensure();
    mpstate::with_vm(|vm| {
        let (len, items) = objlist::list_get(vm.mp_wasm_host_slots);
        if slot as usize >= len {
            return obj::OBJ_NULL;
        }
        let cb = items[slot as usize];
        if cb == obj::CONST_NONE || !obj::is_callable(cb) {
            obj::OBJ_NULL
        } else {
            cb
        }
    })
}

static HOST_REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `mp_wasm_host_register` — ensures slot/handle lists; WAMR natives are a port gap.
pub fn register_host() -> bool {
    if !slots_enabled() {
        return false;
    }
    host_slots_ensure();
    handles_ensure();
    if HOST_REGISTERED.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    let _ = HOST_MODULE;
    // Port gap: wasm_runtime_register_natives(HOST_MODULE, host_symbols, …)
    HOST_REGISTERED.store(true, std::sync::atomic::Ordering::Relaxed);
    true
}

/// `mp_wasm_host_set_slot`
pub fn set_slot(slot: i32, callable: Obj) -> bool {
    if !slots_enabled() || slot < 0 {
        return false;
    }
    if callable != obj::CONST_NONE && !obj::is_callable(callable) {
        return false;
    }
    host_slots_grow_to(slot as usize + 1);
    mpstate::with_vm(|vm| {
        list_set_item(vm.mp_wasm_host_slots, slot as usize, callable);
    });
    true
}

/// `mp_wasm_host_get_slot`
pub fn get_slot(slot: i32) -> Obj {
    if !slots_enabled() || slot < 0 {
        return obj::CONST_NONE;
    }
    host_slots_ensure();
    mpstate::with_vm(|vm| {
        let (len, items) = objlist::list_get(vm.mp_wasm_host_slots);
        if slot as usize >= len {
            obj::CONST_NONE
        } else {
            items[slot as usize]
        }
    })
}

#[derive(Default)]
struct Cookie {
    data: Vec<u8>,
    used: bool,
}

static COOKIES: Mutex<Vec<Cookie>> = Mutex::new(Vec::new());

fn cookie_get(cookie: i32) -> Option<usize> {
    if cookie <= 0 {
        return None;
    }
    let idx = cookie as usize - 1;
    let cookies = COOKIES.lock().unwrap();
    if idx >= cookies.len() || !cookies[idx].used {
        None
    } else {
        Some(idx)
    }
}

/// `mp_wasm_mem_alloc`
pub fn mem_alloc(size: u32) -> i32 {
    let mut cookies = COOKIES.lock().unwrap();
    let slot = cookies
        .iter()
        .position(|c| !c.used)
        .unwrap_or(cookies.len());
    if slot == cookies.len() {
        cookies.push(Cookie::default());
    }
    cookies[slot].data = if size > 0 {
        vec![0u8; size as usize]
    } else {
        Vec::new()
    };
    cookies[slot].used = true;
    (slot + 1) as i32
}

/// `mp_wasm_mem_alloc_copy`
pub fn mem_alloc_copy(data: &[u8], len: u32) -> i32 {
    let c = mem_alloc(len);
    if c == 0 {
        return 0;
    }
    if len > 0 {
        let mut cookies = COOKIES.lock().unwrap();
        cookies[c as usize - 1]
            .data
            .copy_from_slice(&data[..len as usize]);
    }
    c
}

/// `mp_wasm_mem_free`
pub fn mem_free(cookie: i32) -> bool {
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return false,
    };
    let mut cookies = COOKIES.lock().unwrap();
    cookies[idx] = Cookie::default();
    true
}

/// `mp_wasm_mem_clear_all`
pub fn mem_clear_all() {
    COOKIES.lock().unwrap().clear();
}

/// `mp_wasm_mem_valid`
pub fn mem_valid(cookie: i32) -> bool {
    cookie_get(cookie).is_some()
}

/// `mp_wasm_mem_len`
pub fn mem_len(cookie: i32) -> u32 {
    cookie_get(cookie)
        .map(|idx| COOKIES.lock().unwrap()[idx].data.len() as u32)
        .unwrap_or(0)
}

/// Copy cookie bytes (Python `mem_get` helper).
pub fn mem_bytes(cookie: i32) -> Option<Vec<u8>> {
    let idx = cookie_get(cookie)?;
    Some(COOKIES.lock().unwrap()[idx].data.clone())
}

/// `mp_wasm_mem_set`
pub fn mem_set(cookie: i32, data: &[u8], len: u32) -> bool {
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return false,
    };
    let mut cookies = COOKIES.lock().unwrap();
    cookies[idx].data = if len > 0 {
        data[..len as usize].to_vec()
    } else {
        Vec::new()
    };
    true
}

/// `mp_wasm_handle_register`
pub fn handle_register(obj: Obj) -> i32 {
    if obj == obj::CONST_NONE {
        return 0;
    }
    handles_ensure();
    mpstate::with_vm(|vm| {
        let list = vm.mp_wasm_handles;
        let (len, items) = objlist::list_get(list);
        for i in 0..len {
            if items[i] == obj::CONST_NONE {
                list_set_item(list, i, obj);
                return (i + 1) as i32;
            }
        }
        objlist::list_append(list, obj);
        (len + 1) as i32
    })
}

/// `mp_wasm_handle_resolve`
pub fn handle_resolve(handle: i32) -> Obj {
    if handle <= 0 {
        return obj::CONST_NONE;
    }
    handles_ensure();
    mpstate::with_vm(|vm| {
        let (len, items) = objlist::list_get(vm.mp_wasm_handles);
        let idx = handle as usize - 1;
        if idx >= len {
            obj::CONST_NONE
        } else {
            items[idx]
        }
    })
}

/// `mp_wasm_handle_free`
pub fn handle_free(handle: i32) -> bool {
    if handle <= 0 {
        return false;
    }
    handles_ensure();
    mpstate::with_vm(|vm| {
        let (len, items) = objlist::list_get(vm.mp_wasm_handles);
        let idx = handle as usize - 1;
        if idx >= len || items[idx] == obj::CONST_NONE {
            false
        } else {
            list_set_item(vm.mp_wasm_handles, idx, obj::CONST_NONE);
            true
        }
    })
}

/// `mp_wasm_handle_clear_all`
pub fn handle_clear_all() {
    handles_ensure();
    mpstate::with_vm(|vm| {
        let len = objlist::list_get(vm.mp_wasm_handles).0;
        for i in 0..len {
            list_set_item(vm.mp_wasm_handles, i, obj::CONST_NONE);
        }
    });
}

fn copy_name(dst: &mut [u8], src: &[u8]) -> bool {
    if src.is_empty() || src.len() >= dst.len() {
        return false;
    }
    dst[..src.len()].copy_from_slice(src);
    dst[src.len()] = 0;
    true
}

fn loaded_module(name: &str) -> Obj {
    mpstate::with_vm(|vm| {
        if vm.mp_loaded_modules_dict == obj::OBJ_NULL {
            return obj::OBJ_NULL;
        }
        let key = obj::new_qstr(qstr::from_str(name));
        let val = objdict::dict_get(vm.mp_loaded_modules_dict, key);
        if val == obj::OBJ_NULL {
            obj::OBJ_NULL
        } else {
            val
        }
    })
}

/// `mp_wasm_host_call_export_i32`
pub fn call_export_i32(pack: &[u8], func: &[u8], nargs: u32, args: &[i32], out: &mut i32) -> i32 {
    let mut pname = [0u8; HOST_NAME_MAX];
    let mut fname = [0u8; HOST_NAME_MAX];
    if !copy_name(&mut pname, pack) || !copy_name(&mut fname, func) {
        return -1;
    }
    if nargs > 0 && args.is_empty() {
        return -1;
    }
    let pack_s = std::str::from_utf8(&pack[..pack.len().min(HOST_NAME_MAX - 1)]).unwrap_or("");
    if !forward::registry_find(pack_s) {
        return -1;
    }
    let fname_s =
        std::str::from_utf8(&fname[..fname.iter().position(|&b| b == 0).unwrap_or(fname.len())])
            .unwrap_or("");
    let call_args = if nargs > 0 {
        &args[..nargs as usize]
    } else {
        &[]
    };
    if runtime::call_export_i32_named(pack_s, fname_s, call_args, out) {
        return 0;
    }
    -1
}

/// `mp_wasm_host_call_attr`
pub fn call_attr(mod_name: &[u8], attr: &[u8], has_arg: i32, arg: i32, out: &mut Obj) -> i32 {
    let mut mname = [0u8; HOST_NAME_MAX];
    let mut aname = [0u8; HOST_NAME_MAX];
    if !copy_name(&mut mname, mod_name) || !copy_name(&mut aname, attr) {
        return -1;
    }
    let module = loaded_module(std::str::from_utf8(mod_name).unwrap_or(""));
    if module == obj::OBJ_NULL {
        return -1;
    }
    let attr_q = qstr::from_str(std::str::from_utf8(attr).unwrap_or(""));
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        let fn_obj = py_rs::runtime::load_attr(module, attr_q);
        if has_arg != 0 {
            py_rs::runtime::call_function_1(fn_obj, obj::new_small_int(arg as isize))
        } else {
            py_rs::runtime::call_function_0(fn_obj)
        }
    }) {
        Ok(v) => {
            *out = v;
            0
        }
        Err(_) => -1,
    }
}

pub(crate) fn call_slot0_i32(slot: i32) -> i32 {
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || py_rs::runtime::call_function_0(cb)) {
        Ok(v) => obj::get_int_truncated(v) as i32,
        Err(_) => -1,
    }
}

pub(crate) fn call_slot_i32(slot: i32, arg: i32) -> i32 {
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        py_rs::runtime::call_function_1(cb, obj::new_small_int(arg as isize))
    }) {
        Ok(v) => obj::get_int_truncated(v) as i32,
        Err(_) => -1,
    }
}

pub(crate) fn call_slot_i64(slot: i32, arg: i64) -> i64 {
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        py_rs::runtime::call_function_1(cb, py_rs::objint::new_int_from_ll(arg))
    }) {
        Ok(v) => py_rs::objint::int_get_truncated(v) as i64,
        Err(_) => -1,
    }
}

pub(crate) fn call_slot_f32(slot: i32, arg: f32) -> f32 {
    call_slot_f64(slot, arg as f64) as f32
}

pub(crate) fn call_slot_f64(slot: i32, arg: f64) -> f64 {
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1.0;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        py_rs::runtime::call_function_1(cb, py_rs::objfloat::new_float(arg))
    }) {
        Ok(v) => py_rs::objfloat::get_float_to_d(v),
        Err(_) => -1.0,
    }
}

pub(crate) fn call_slot_buf(slot: i32, data: &[u8]) -> i32 {
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        let buf = objstr::new_bytes(data);
        py_rs::runtime::call_function_1(cb, buf)
    }) {
        Ok(v) => obj::get_int_truncated(v) as i32,
        Err(_) => -1,
    }
}

pub(crate) fn call_slot_mem(slot: i32, cookie: i32) -> i32 {
    let Some(data) = mem_bytes(cookie) else {
        return -1;
    };
    call_slot_buf(slot, &data)
}

pub(crate) fn call_slot_obj(slot: i32, handle: i32) -> i32 {
    if handle <= 0 {
        return -1;
    }
    let obj_val = handle_resolve(handle);
    if obj_val == obj::CONST_NONE {
        return -1;
    }
    let cb = slot_callable(slot);
    if cb == obj::OBJ_NULL {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || {
        py_rs::runtime::call_function_1(cb, obj_val)
    }) {
        Ok(v) => obj::get_int_truncated(v) as i32,
        Err(_) => -1,
    }
}

pub(crate) fn mem_copy_in(cookie: i32, src: &[u8]) -> i32 {
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return -1,
    };
    let n = src.len();
    let mut cookies = COOKIES.lock().unwrap();
    if n > cookies[idx].data.len() {
        return -1;
    }
    if n > 0 {
        cookies[idx].data[..n].copy_from_slice(src);
    }
    0
}

pub(crate) fn mem_copy_out(cookie: i32, dest: &mut [u8]) -> i32 {
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return -1,
    };
    let n = dest.len();
    let cookies = COOKIES.lock().unwrap();
    if n > cookies[idx].data.len() {
        return -1;
    }
    if n > 0 {
        dest.copy_from_slice(&cookies[idx].data[..n]);
    }
    0
}

pub(crate) fn mem_copy_in_at(cookie: i32, cookie_off: i32, src: &[u8]) -> i32 {
    if cookie_off < 0 {
        return -1;
    }
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return -1,
    };
    let n = src.len();
    let off = cookie_off as usize;
    let mut cookies = COOKIES.lock().unwrap();
    if off > cookies[idx].data.len() || n > cookies[idx].data.len() - off {
        return -1;
    }
    if n > 0 {
        cookies[idx].data[off..off + n].copy_from_slice(src);
    }
    0
}

pub(crate) fn mem_copy_out_at(cookie: i32, cookie_off: i32, dest: &mut [u8]) -> i32 {
    if cookie_off < 0 {
        return -1;
    }
    let idx = match cookie_get(cookie) {
        Some(i) => i,
        None => return -1,
    };
    let n = dest.len();
    let off = cookie_off as usize;
    let cookies = COOKIES.lock().unwrap();
    if off > cookies[idx].data.len() || n > cookies[idx].data.len() - off {
        return -1;
    }
    if n > 0 {
        dest.copy_from_slice(&cookies[idx].data[off..off + n]);
    }
    0
}

/// Guest `call0_py` / `call_py` — resolve sys.modules names and call Python.
pub(crate) fn call0_py(mod_name: &str, attr_name: &str) -> i32 {
    let mut out = obj::OBJ_NULL;
    if call_attr(mod_name.as_bytes(), attr_name.as_bytes(), 0, 0, &mut out) != 0 {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || obj::get_int_truncated(out) as i32) {
        Ok(v) => v,
        Err(_) => -1,
    }
}

pub(crate) fn call_py(mod_name: &str, attr_name: &str, arg: i32) -> i32 {
    let mut out = obj::OBJ_NULL;
    if call_attr(mod_name.as_bytes(), attr_name.as_bytes(), 1, arg, &mut out) != 0 {
        return -1;
    }
    let mut nlr_buf = py_rs::nlr::NlrBuf::default();
    match py_rs::nlr::protect(&mut nlr_buf, || obj::get_int_truncated(out) as i32) {
        Ok(v) => v,
        Err(_) => -1,
    }
}
