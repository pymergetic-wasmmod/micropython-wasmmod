//! rewrite of ports/qemu/mphalport.c + ports/qemu/mphalport.h
// symmetry: done

use crate::uart;
use py_rs::mphal;
use py_rs::runtime::{self, HandlePendingBehaviour};

const USE_UART: bool = true;

/// `mp_hal_stdio_poll`
pub fn stdio_poll(poll_flags: u32) -> u32 {
    const MP_STREAM_POLL_RD: u32 = 1;
    if USE_UART && (poll_flags & MP_STREAM_POLL_RD != 0) && uart::rx_any() {
        return MP_STREAM_POLL_RD;
    }
    0
}

/// `mp_hal_stdin_rx_chr`
pub fn stdin_rx_chr() -> i32 {
    loop {
        if USE_UART {
            let c = uart::rx_chr();
            if c >= 0 {
                return c;
            }
        }
        runtime::handle_pending(HandlePendingBehaviour::CallbacksOnly);
    }
}

/// `mp_hal_stdout_tx_strn`
pub fn stdout_tx_strn(str: &[u8]) -> usize {
    if USE_UART {
        uart::tx_strn(str);
    }
    str.len()
}

pub fn ticks_ms() -> u32 {
    mphal::ticks_ms() as u32
}

pub fn ticks_us() -> u32 {
    mphal::ticks_us() as u32
}

pub fn delay_ms(ms: u32) {
    if ms != 0 {
        let start = ticks_ms();
        while ticks_ms().wrapping_sub(start) < ms {}
    } else {
        runtime::handle_pending(HandlePendingBehaviour::CallbacksOnly);
    }
}

pub fn delay_us(us: u32) {
    let start = ticks_us();
    while ticks_us().wrapping_sub(start) < us {}
}

pub fn ticks_cpu() -> u32 {
    0
}

static mut RANDOM_STATE: u32 = 0;

/// LCG random for tests (`mp_hal_get_random`).
pub fn get_random(n: usize, buf: &mut [u8]) {
    for byte in buf.iter_mut().take(n) {
        unsafe {
            RANDOM_STATE = RANDOM_STATE
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            *byte = (RANDOM_STATE >> 24) as u8;
        }
    }
}
