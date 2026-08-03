//! MetalPython rewrite of MicroPython `extmod/wasmmod/`.
pub mod alloc;
pub mod fetch;
pub mod finder;
pub mod forward;
pub mod host;
pub mod io;
pub mod loader;
pub mod modapi;
pub mod modobj;
pub mod pack;
pub mod packload;
pub mod ports;
pub mod runtime;
pub mod verify;
pub mod version;
pub mod wasmmod;

/// Compatibility alias — upstream renamed `modwasm.c` → `wasmmod.c`.
pub use wasmmod as modwasm;
