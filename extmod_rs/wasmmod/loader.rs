//! rewrite of extmod/wasmmod/loader.c + extmod/wasmmod/loader.h
//!
//! Guest `wasmmod` imports are registered on the wasmi linker in `runtime.rs`.
// symmetry: done

use std::sync::atomic::{AtomicBool, Ordering};

use wasmi::Caller;

use super::host;
use super::pack::WASM_MODULE;
use super::runtime;
use super::verify;
use super::version;

const LOADER_MODE: i32 = 0; // Mode_Interp when no JIT features

const LOADER_NAME_MAX: usize = 64;
const LOADER_CALL_ARGS_MAX: u32 = 8;

static LOADER_REGISTERED: AtomicBool = AtomicBool::new(false);

/// `mp_wasm_loader_register` — wasmi linker hooks are installed from `runtime`.
pub fn register() -> bool {
    if LOADER_REGISTERED.load(Ordering::Relaxed) {
        return true;
    }
    let _ = WASM_MODULE;
    LOADER_REGISTERED.store(true, Ordering::Relaxed);
    true
}

pub(crate) fn guest_version(caller: &mut Caller<'_, ()>, off: i32, maxlen: i32) -> i32 {
    let ver = version::VERSION.as_bytes();
    if maxlen < 0 || ver.len() > maxlen as usize {
        return -1;
    }
    if !runtime::caller_linear_write(caller, off as u32, ver) {
        return -1;
    }
    ver.len() as i32
}

pub(crate) fn guest_mode() -> i32 {
    LOADER_MODE
}

pub(crate) fn guest_verify() -> i32 {
    if verify::get_verify_enabled() {
        1
    } else {
        0
    }
}

pub(crate) fn guest_trust_count() -> i32 {
    verify::trust_count() as i32
}

pub(crate) fn guest_call_i32(
    caller: &Caller<'_, ()>,
    pack_off: i32,
    pack_len: i32,
    func_off: i32,
    func_len: i32,
    nargs: i32,
    args_off: i32,
) -> i32 {
    let mut pack_buf = [0u8; LOADER_NAME_MAX];
    let mut func_buf = [0u8; LOADER_NAME_MAX];
    let Some(pack) = guest_name(caller, pack_off, pack_len, &mut pack_buf) else {
        return i32::MIN;
    };
    let Some(func) = guest_name(caller, func_off, func_len, &mut func_buf) else {
        return i32::MIN;
    };
    if nargs < 0 || nargs as u32 > LOADER_CALL_ARGS_MAX {
        return i32::MIN;
    }
    let mut args_buf = [0i32; LOADER_CALL_ARGS_MAX as usize];
    let args: &[i32] = if nargs > 0 {
        let byte_len = (nargs as usize) * size_of::<i32>();
        let mut raw = vec![0u8; byte_len];
        if !runtime::caller_linear_read(caller, args_off as u32, byte_len as u32, &mut raw) {
            return i32::MIN;
        }
        for i in 0..nargs as usize {
            args_buf[i] = i32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap());
        }
        &args_buf[..nargs as usize]
    } else {
        &[]
    };
    let mut out = 0i32;
    if host::call_export_i32(
        pack.as_bytes(),
        func.as_bytes(),
        args.len() as u32,
        args,
        &mut out,
    ) != 0
    {
        return i32::MIN;
    }
    out
}

fn guest_name<'a>(
    caller: &Caller<'_, ()>,
    off: i32,
    len: i32,
    buf: &'a mut [u8],
) -> Option<&'a str> {
    if len < 0 || len as usize >= buf.len() {
        return None;
    }
    if len > 0
        && !runtime::caller_linear_read(caller, off as u32, len as u32, &mut buf[..len as usize])
    {
        return None;
    }
    buf[len as usize] = 0;
    std::str::from_utf8(&buf[..len as usize]).ok()
}
