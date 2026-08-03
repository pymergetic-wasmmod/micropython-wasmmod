//! rewrite of extmod/wasmmod/host.c + extmod/wasmmod/host.h
// symmetry: gaps
// gaps:
// - WAMR `wasm_runtime_register_natives("micropython.host", …)` for call_i32/call0_i32/call_i64/call_f32/call_f64

use py_rs::mpconfig;
use py_rs::mpstate;
use py_rs::obj::{self, Obj};
use py_rs::objlist;

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

/// `mp_wasm_host_register` — ensures slot list; WAMR natives are a port gap.
pub fn register_host() -> bool {
    if !slots_enabled() {
        return false;
    }
    host_slots_ensure();
    false
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub(crate) fn call_slot_f32(slot: i32, arg: f32) -> f32 {
    call_slot_f64(slot, arg as f64) as f32
}

#[allow(dead_code)]
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
