//! rewrite of extmod/wasmmod/ports/micropython/mpconfig_wasm.h
// symmetry: done

/// Optional default wasm.arch entry for AOT filename tags; empty = plain .aot names.
pub const WASM_PACK_ARCH: &str = "";

/// Signature policy: 0=off, 1=require, 2=verify-when-present.
pub const WASM_VERIFY: u8 = 0;

pub const PY_WASM_AOT: bool = false;
pub const PY_WASM_JIT: bool = false;
pub const PY_WASM_FAST_JIT: bool = false;
pub const PY_WASM_MATRIX: bool = false;
