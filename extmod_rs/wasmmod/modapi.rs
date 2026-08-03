//! rewrite of extmod/wasmmod/modapi.c
// symmetry: done
// Note: Python callables are registered directly in wasmmod.rs `init_module`.

use py_rs::obj::Obj;

/// Placeholder until Python callables are registered on the `wasm` module dict.
pub fn register_api(_module_globals: Obj) -> bool {
    false
}
