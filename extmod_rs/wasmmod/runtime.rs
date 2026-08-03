//! rewrite of extmod/wasmmod/runtime.c + extmod/wasmmod/runtime.h
// symmetry: gaps
// gaps:
// - WAMR init/load/instantiate (`wasm_runtime_*`, `wasm_export.h`) not linked on host rewrite
// - `mp_wasm_module_call*` / export enumeration require WAMR exec env + instance types

use std::sync::atomic::{AtomicBool, Ordering};

use py_rs::mpconfig;

pub const MP_WASM_NAME_MAX: usize = 255;
pub const MP_WASM_ERRBUF: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValKind {
    I32,
    I64,
    F32,
    F64,
}

/// Opaque loaded-module handle (bytes + metadata; WAMR fields are a port gap).
pub struct WasmModule {
    name: [u8; MP_WASM_NAME_MAX + 1],
    name_len: usize,
    buf: Vec<u8>,
    meta: Vec<u8>,
    #[allow(dead_code)]
    meta_owned: bool,
}

impl WasmModule {
    fn new(name: Option<&str>, code: &[u8], meta: Option<&[u8]>) -> Self {
        let mut out = Self {
            name: [0; MP_WASM_NAME_MAX + 1],
            name_len: 0,
            buf: code.to_vec(),
            meta: Vec::new(),
            meta_owned: false,
        };
        out.set_name(name.unwrap_or("wasm"));
        if let Some(m) = meta {
            out.meta = m.to_vec();
            out.meta_owned = true;
        } else {
            out.meta = out.buf.clone();
        }
        out
    }

    fn set_name(&mut self, name: &str) {
        self.name.fill(0);
        let n = name.len().min(MP_WASM_NAME_MAX);
        self.name[..n].copy_from_slice(&name.as_bytes()[..n]);
        self.name_len = n;
    }
}

static RUNTIME_READY: AtomicBool = AtomicBool::new(false);

pub fn set_err(errbuf: &mut [u8], msg: &str) {
    if errbuf.is_empty() {
        return;
    }
    let n = msg.len().min(errbuf.len() - 1);
    errbuf[..n].copy_from_slice(&msg.as_bytes()[..n]);
    errbuf[n] = 0;
}

/// `mp_wasm_valkind_is_numeric`
pub fn valkind_is_numeric(kind: WasmValKind) -> bool {
    matches!(
        kind,
        WasmValKind::I32 | WasmValKind::I64 | WasmValKind::F32 | WasmValKind::F64
    )
}

/// `mp_wasm_runtime_init`
pub fn runtime_init() -> bool {
    if !mpconfig::PY_WASM {
        return false;
    }
    if RUNTIME_READY.load(Ordering::Relaxed) {
        return true;
    }
    let _ = super::host::register_host();
    false
}

/// `mp_wasm_runtime_deinit`
pub fn runtime_deinit() {
    if !mpconfig::PY_WASM {
        return;
    }
    RUNTIME_READY.store(false, Ordering::Relaxed);
}

/// Whether the WAMR runtime is ready for module load.
pub fn runtime_ready() -> bool {
    RUNTIME_READY.load(Ordering::Relaxed)
}

/// `mp_wasm_module_name`
pub fn module_name(mod_: &WasmModule) -> &str {
    std::str::from_utf8(&mod_.name[..mod_.name_len]).unwrap_or("")
}

/// `mp_wasm_module_set_name`
pub fn module_set_name(mod_: &mut WasmModule, name: &str) {
    mod_.set_name(name);
}

/// `mp_wasm_module_bytes`
pub fn module_bytes(mod_: &WasmModule) -> &[u8] {
    &mod_.buf
}

/// `mp_wasm_module_meta_bytes`
pub fn module_meta_bytes(mod_: &WasmModule) -> &[u8] {
    &mod_.meta
}

/// `mp_wasm_module_load_ex` — validates bytes; WAMR instantiate is a port gap.
pub fn module_load_ex(
    code: &[u8],
    meta: Option<&[u8]>,
    name: Option<&str>,
    path_hint: Option<&str>,
    errbuf: &mut [u8],
) -> Option<Box<WasmModule>> {
    errbuf[0] = 0;
    if !runtime_init() {
        set_err(errbuf, "wasm runtime init failed");
        return None;
    }
    if code.is_empty() {
        set_err(errbuf, "empty wasm");
        return None;
    }
    if !super::verify::verify_bytes(code, path_hint, errbuf) {
        return None;
    }
    let _ = meta;
    let _ = name;
    set_err(errbuf, "wasm runtime init failed");
    None
}

/// `mp_wasm_module_load`
pub fn module_load(
    bytes: &[u8],
    name: Option<&str>,
    errbuf: &mut [u8],
) -> Option<Box<WasmModule>> {
    module_load_ex(bytes, None, name, None, errbuf)
}

/// `mp_wasm_module_close`
pub fn module_close(mod_: Box<WasmModule>) {
    super::forward::registry_remove(mod_.as_ref());
    drop(mod_);
}

/// `mp_wasm_module_func_types`
pub fn module_func_types(_mod_: &WasmModule, _func: &str) -> bool {
    false
}

/// `mp_wasm_module_call_vals`
pub fn module_call_vals(_mod_: &WasmModule, _func: &str, errbuf: &mut [u8]) -> bool {
    set_err(errbuf, "invalid module");
    false
}

/// `mp_wasm_module_call0`
pub fn module_call0(mod_: &WasmModule, func: &str, out_result: &mut i32, errbuf: &mut [u8]) -> bool {
    module_call_i32(mod_, func, &[], out_result, errbuf)
}

/// `mp_wasm_module_call_i32`
pub fn module_call_i32(
    _mod_: &WasmModule,
    _func: &str,
    _args: &[i32],
    _out_result: &mut i32,
    errbuf: &mut [u8],
) -> bool {
    set_err(errbuf, "invalid module");
    false
}

/// `mp_wasm_module_export_names`
pub fn module_export_names(_mod_: &WasmModule) -> Vec<&'static str> {
    Vec::new()
}

/// `mp_wasm_module_numeric_export_arity`
pub fn module_numeric_export_arity(_mod_: &WasmModule, _name: &str) -> bool {
    false
}

/// `mp_wasm_module_i32_export_arity`
pub fn module_i32_export_arity(_mod_: &WasmModule, _name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_roundtrip() {
        let m = WasmModule::new(Some("hello"), b"\0asm\x01\0\0\0", None);
        assert_eq!(module_name(&m), "hello");
    }
}
