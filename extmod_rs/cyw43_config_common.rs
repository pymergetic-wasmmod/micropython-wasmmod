//! rewrite of extmod/cyw43_config_common.h (MicroPython-specific CYW43 constants and inline delays)
//! `CYW43_PRINTF`, thread enter/exit, pendsv dispatch, `cyw43_hal_*` pin/MAC hooks, and mDNS netif hooks need port `cyw43` + lwIP wiring.
// symmetry: done

use py_rs::mperrno;

pub const CYW43_IOCTL_TIMEOUT_US: u32 = 1_000_000;
pub const CYW43_NETUTILS: u8 = 1;

pub const CYW43_EPERM: i32 = mperrno::EPERM;
pub const CYW43_EIO: i32 = mperrno::EIO;
pub const CYW43_EINVAL: i32 = mperrno::EINVAL;
pub const CYW43_ETIMEDOUT: i32 = mperrno::ETIMEDOUT;

/// `CYW43_HAL_PIN_MODE_*` — canonical MicroPython HAL values (ports may remap).
pub const CYW43_HAL_PIN_MODE_INPUT: u32 = 0;
pub const CYW43_HAL_PIN_MODE_OUTPUT: u32 = 1;
pub const CYW43_HAL_PIN_PULL_NONE: u32 = 0;
pub const CYW43_HAL_PIN_PULL_UP: u32 = 1;
pub const CYW43_HAL_PIN_PULL_DOWN: u32 = 2;

/// `CYW43_HAL_MAC_*` — indices for `mp_hal_get_mac`.
pub const CYW43_HAL_MAC_WLAN0: u32 = 0;
pub const CYW43_HAL_MAC_BDADDR: u32 = 2;

/// `CYW43_ARRAY_SIZE(a)` — length of a fixed array/slice.
#[inline]
pub const fn array_size<T>(a: &[T]) -> usize {
    a.len()
}

/// `CYW43_HOST_NAME` — device hostname C string used by CYW43/mDNS.
pub fn host_name() -> &'static [u8] {
    crate::modnetwork::hostname_data()
}

#[cfg(feature = "cyw43")]
pub fn cyw43_event_poll_hook() {
    py_rs::runtime::event_handle_nowait();
}

#[cfg(feature = "cyw43")]
pub fn cyw43_delay_us(us: u32) {
    let start = py_rs::mphal::ticks_us();
    while py_rs::mphal::ticks_us().wrapping_sub(start) < us {}
}

#[cfg(feature = "cyw43")]
pub fn cyw43_delay_ms(ms: u32) {
    let us = ms * 1000;
    let start = py_rs::mphal::ticks_us();
    while py_rs::mphal::ticks_us().wrapping_sub(start) < us {
        cyw43_event_poll_hook();
    }
}

#[cfg(feature = "cyw43")]
pub fn cyw43_post_poll_hook() {}
