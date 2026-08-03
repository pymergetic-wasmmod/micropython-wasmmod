//! MetalPython rewrite of MicroPython `extmod/wasmmod/`.
pub mod fetch;
pub mod finder;
pub mod forward;
pub mod host;
pub mod pack;
pub mod runtime;
pub mod verify;
pub mod wasmmod;

/// Compatibility alias — upstream renamed `modwasm.c` → `wasmmod.c`.
pub use wasmmod as modwasm;
