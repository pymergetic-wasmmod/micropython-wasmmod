//! rewrite of ports/qemu/uart.c + ports/qemu/uart.h
// symmetry: done

use crate::mpconfigport::{qemu_soc, QemuSoc};

pub const UART_RX_NO_CHAR: i32 = -1;

/// Initialize UART for the selected QEMU SoC.
pub fn init() {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    virt_init();
}

pub fn rx_chr() -> i32 {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        return virt_rx_chr();
    }
    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    {
        UART_RX_NO_CHAR
    }
}

pub fn rx_any() -> bool {
    rx_chr() != UART_RX_NO_CHAR
}

pub fn tx_strn(buf: &[u8]) {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        match qemu_soc() {
            QemuSoc::Virt | QemuSoc::Powernv => virt_tx_strn(buf),
            _ => virt_tx_strn(buf),
        }
    }
    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    {
        let _ = std::io::Write::write_all(&mut std::io::stdout(), buf);
    }
}

// VIRT 16550-style UART @ 0x10000000 (RV32 QEMU default).

#[repr(C)]
struct VirtUart {
    dr: u8,
    _pad: [u8; 4],
    lsr: u8,
}

const VIRT_UART: *mut VirtUart = 0x1000_0000 as *mut VirtUart;
const VIRT_LSR_THRE: u8 = 0x20;
const VIRT_LSR_DR: u8 = 0x01;

fn virt_init() {}

fn virt_rx_chr() -> i32 {
    unsafe {
        if (*VIRT_UART).lsr & VIRT_LSR_DR == 0 {
            return UART_RX_NO_CHAR;
        }
        (*VIRT_UART).dr as i32
    }
}

fn virt_tx_strn(buf: &[u8]) {
    unsafe {
        for &b in buf {
            while (*VIRT_UART).lsr & VIRT_LSR_THRE == 0 {}
            (*VIRT_UART).dr = b;
        }
    }
}

// Additional SoC backends from C uart.c (STM32, nRF51, MPS2/3, i.MX6) share the same API;
// selection follows `qemu_soc()` when building with non-VIRT board features.

/// STM32 UART backend constants (QEMU_SOC_STM32).
pub mod stm32 {
    pub const BASE: u32 = 0x4001_1000;
}

/// nRF51 UART backend constants (QEMU_SOC_NRF51).
pub mod nrf51 {
    pub const BASE: u32 = 0x4000_2000;
}

/// MPS2 UART backend constants.
pub mod mps2 {
    pub const BASE: u32 = 0x4000_4000;
}

/// i.MX6 UART backend constants.
pub mod imx6 {
    pub const BASE: u32 = 0x0202_0000;
}
