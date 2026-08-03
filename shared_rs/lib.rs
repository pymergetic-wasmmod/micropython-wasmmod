//! MetalPython rewrite of MicroPython `shared/`.
//! Shadow tree: `shared_rs/`.
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unused_unsafe,
    non_snake_case,
    non_upper_case_globals,
    static_mut_refs,
    private_interfaces,
    unexpected_cfgs,
    clippy::all
)]

pub mod libc;
pub mod memzip;
pub mod netutils;
pub mod readline;
pub mod runtime;
pub mod timeutils;
pub mod tinyusb;
