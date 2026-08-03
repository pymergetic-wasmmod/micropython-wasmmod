//! MetalPython rewrite of MicroPython `ports/qemu/`.
//! Shadow tree: `ports_rs/qemu/`.
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

pub mod mcu;
pub mod modmachine;
pub mod mpconfigport;
pub mod mphalport;
pub mod qstrdefsport;
pub mod uart;
pub mod vfs_rom_ioctl;
