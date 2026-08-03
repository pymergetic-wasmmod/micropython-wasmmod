//! rewrite of extmod/wasmmod/io.h
//!
//! v1 sync I/O ops only; `FetchCb` / `ProbeCb` and reserved slots are for future
//! Metal async without an ABI break (see upstream `io.h`).
// symmetry: done

use std::sync::{Mutex, OnceLock};

pub const IO_OPS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IoResult {
    Ok = 0,
    Decline = 1,
    Err = -1,
}

/// Future async fetch callback (`mp_wasm_io_fetch_cb_t`; unused at v1).
pub type FetchCb = fn(ctx: *mut core::ffi::c_void, st: IoResult, buf: *mut u8, len: u32);

/// Future async probe callback (`mp_wasm_io_probe_cb_t`; unused at v1).
pub type ProbeCb = fn(ctx: *mut core::ffi::c_void, st: IoResult);

pub type FetchFn = fn(
    uri: &str,
    out_bytes: &mut Option<Vec<u8>>,
    out_len: &mut u32,
    errbuf: &mut [u8],
) -> IoResult;

pub type ProbeFn = fn(uri: &str) -> IoResult;

pub type YieldFn = fn();

/// Mirror `mp_wasm_io_ops_t` (v1 — async fields reserved).
pub struct IoOps {
    pub version: u32,
    pub fetch: Option<FetchFn>,
    pub probe: Option<ProbeFn>,
    pub yield_fn: Option<YieldFn>,
    /// Reserved for future `fetch_async` (v1: 0).
    pub reserved0: usize,
    /// Reserved for future `probe_async` (v1: 0).
    pub reserved1: usize,
    /// Opaque port userdata (`mp_wasm_io_ops_t.userdata`).
    pub userdata: usize,
}

fn decline_fetch(
    _uri: &str,
    _out_bytes: &mut Option<Vec<u8>>,
    _out_len: &mut u32,
    _errbuf: &mut [u8],
) -> IoResult {
    IoResult::Decline
}

fn decline_probe(_uri: &str) -> IoResult {
    IoResult::Decline
}

impl Default for IoOps {
    fn default() -> Self {
        Self {
            version: IO_OPS_VERSION,
            fetch: Some(decline_fetch),
            probe: Some(decline_probe),
            yield_fn: None,
            reserved0: 0,
            reserved1: 0,
            userdata: 0,
        }
    }
}

static DEFAULT_OPS: IoOps = IoOps {
    version: IO_OPS_VERSION,
    fetch: Some(decline_fetch),
    probe: Some(decline_probe),
    yield_fn: None,
    reserved0: 0,
    reserved1: 0,
    userdata: 0,
};

static OPS: OnceLock<Mutex<&'static IoOps>> = OnceLock::new();

fn ops_lock() -> &'static Mutex<&'static IoOps> {
    OPS.get_or_init(|| Mutex::new(&DEFAULT_OPS))
}

/// `mp_wasm_io_set` — NULL restores built-in defaults.
pub fn set(ops: Option<&'static IoOps>) {
    let mut guard = ops_lock().lock().unwrap();
    *guard = ops.unwrap_or(&DEFAULT_OPS);
}

/// `mp_wasm_io_get`
pub fn get() -> &'static IoOps {
    *ops_lock().lock().unwrap()
}

/// `mp_wasm_io_yield`
pub fn yield_io() {
    if let Some(y) = get().yield_fn {
        y();
    }
}

/// Port fetch outcome for `fetch.c` chaining (ops → legacy → native → VFS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Ok(Vec<u8>),
    Decline,
    Err,
}

/// Invoke port `fetch` hook; caller frees bytes on OK in C via `MICROPY_WASM_FREE`.
pub fn invoke_fetch(uri: &str, errbuf: &mut [u8]) -> FetchOutcome {
    errbuf[0] = 0;
    let Some(f) = get().fetch else {
        return FetchOutcome::Decline;
    };
    let mut bytes = None;
    let mut len = 0;
    match f(uri, &mut bytes, &mut len, errbuf) {
        IoResult::Ok => FetchOutcome::Ok(bytes.unwrap_or_default()),
        IoResult::Decline => FetchOutcome::Decline,
        IoResult::Err => FetchOutcome::Err,
    }
}

fn fetch_fn_eq(a: FetchFn, b: FetchFn) -> bool {
    a as *const () == b as *const ()
}

/// Invoke port `probe` hook; synthesize via fetch when probe declines (mirror `fetch.c`).
pub fn invoke_probe(uri: &str) -> IoResult {
    let ops = get();
    if let Some(p) = ops.probe {
        match p(uri) {
            r @ (IoResult::Ok | IoResult::Err) => return r,
            IoResult::Decline => {}
        }
    }
    if let Some(f) = ops.fetch {
        if !fetch_fn_eq(f, decline_fetch) {
            let mut err = [0u8; 64];
            let mut bytes = None;
            let mut len = 0;
            return match f(uri, &mut bytes, &mut len, &mut err) {
                IoResult::Ok => IoResult::Ok,
                IoResult::Err => IoResult::Err,
                IoResult::Decline => IoResult::Decline,
            };
        }
    }
    IoResult::Decline
}

#[cfg(test)]
mod tests {
    use super::*;

    static CUSTOM_OPS: IoOps = IoOps {
        version: IO_OPS_VERSION,
        fetch: Some(custom_fetch),
        probe: None,
        yield_fn: None,
        reserved0: 0,
        reserved1: 0,
        userdata: 0,
    };

    fn custom_fetch(
        _uri: &str,
        out_bytes: &mut Option<Vec<u8>>,
        out_len: &mut u32,
        _errbuf: &mut [u8],
    ) -> IoResult {
        *out_bytes = Some(b"ok".to_vec());
        *out_len = 3;
        IoResult::Ok
    }

    #[test]
    fn set_get_roundtrip() {
        set(None);
        assert_eq!(get().version, IO_OPS_VERSION);
        set(Some(&CUSTOM_OPS));
        assert!(matches!(
            invoke_fetch("test://x", &mut [0u8; 8]),
            FetchOutcome::Ok(ref b) if b == b"ok"
        ));
        set(None);
        assert!(matches!(
            invoke_fetch("test://x", &mut [0u8; 8]),
            FetchOutcome::Decline
        ));
    }

    #[test]
    fn yield_noop_by_default() {
        yield_io();
    }
}
