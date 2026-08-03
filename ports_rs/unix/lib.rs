//! MetalPython rewrite of MicroPython `ports/unix/`.
//! Shadow tree: `ports_rs/unix/`.
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

pub mod alloc;
pub mod coverage;
pub mod fatfs_port;
pub mod gccollect;
pub mod input;
pub mod modffi;
pub mod modjni;
pub mod modmachine;
pub mod modos;
pub mod modsocket;
pub mod modtermios;
pub mod modtime;
pub mod mpbthciport;
pub mod mpbtstackport;
pub mod mpbtstackport_common;
pub mod mpbtstackport_h4;
pub mod mpbtstackport_usb;
pub mod mpconfigport;
pub mod mphalport;
pub mod mpnimbleport;
pub mod mpthreadport;
pub mod qstrdefsport;
pub mod stack_size;
pub mod unix_mphal;
