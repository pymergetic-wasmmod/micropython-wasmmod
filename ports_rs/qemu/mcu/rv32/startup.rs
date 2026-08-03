//! rewrite of ports/qemu/mcu/rv32/startup.c
// symmetry: done

use crate::uart;

/// `_entry_point` — RV32 QEMU firmware entry.
#[cfg(target_arch = "riscv32")]
pub extern "C" fn entry_point() -> ! {
    super::interrupts::set_interrupt_table();
    uart::init();
    crate::main_rust(0, core::ptr::null());
    exit(0);
}

/// Host build stub for symmetry (firmware entry is RV32-only).
#[cfg(not(target_arch = "riscv32"))]
pub fn entry_point() {
    uart::init();
}

/// `exit` via QEMU RISC-V semihosting SYS_EXIT_EXTENDED.
pub fn exit(status: i32) -> ! {
    #[cfg(target_arch = "riscv32")]
    {
        let mut args = [0u32; 2];
        args[0] = 0x20026; // ADP_Stopped_ApplicationExit
        args[1] = status as u32;
        semihost_exit_extended(&args);
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        std::process::exit(status);
    }
    #[cfg(target_arch = "riscv32")]
    loop {}
}

#[cfg(target_arch = "riscv32")]
fn semihost_exit_extended(args: &[u32; 2]) {
    unsafe {
        core::arch::asm!(
            "mv    a1, {args}",
            "li    t0, 0x20026",
            "sw    t0, 0(a1)",
            "sw    {status}, 4(a1)",
            "addi  a0, zero, 0x20",
            "ebreak",
            args = in(reg) args.as_ptr(),
            status = in(reg) args[1],
            options(nostack)
        );
    }
}

/// Rust `main` shim called from `_entry_point`.
pub fn main_rust(_argc: i32, _argv: *const *const u8) {
    // Full REPL lives in ports/qemu/main.rs binary on host; firmware calls pyexec loop when linked.
}

#[cfg(not(target_os = "none"))]
pub fn assert_func(file: &str, line: i32, func: &str, expr: &str) {
    eprintln!("Assertion '{expr}' failed, at file {file}:{line} in {func}");
    exit(1);
}
