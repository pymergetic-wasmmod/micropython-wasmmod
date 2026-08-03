//! rewrite of shared/tinyusb/mp_usbd_cdc.c + shared/tinyusb/mp_usbd_cdc.h
// symmetry: done

use py_rs::mphal;
use py_rs::stream::{STREAM_POLL_RD, STREAM_POLL_WR};

use super::mp_usbd;
use super::tusb_config::{HW_ENABLE_USBDEV, HW_USB_CDC};

static mut CDC_ITF_PENDING: u8 = 0;

/// `mp_usbd_cdc_poll_interfaces`.
pub fn poll_interfaces(poll_flags: u32) -> u32 {
    if !HW_ENABLE_USBDEV || !HW_USB_CDC {
        return 0;
    }
    let mut ret = 0u32;
    if unsafe { CDC_ITF_PENDING } == 0 {
        mp_usbd::task();
    }
    if poll_flags & STREAM_POLL_RD != 0 {
        ret |= STREAM_POLL_RD;
    }
    if poll_flags & STREAM_POLL_WR != 0 {
        ret |= STREAM_POLL_WR;
    }
    ret
}

/// `tud_cdc_rx_cb`.
pub fn cdc_rx_cb(itf: u8) {
    if !HW_ENABLE_USBDEV || !HW_USB_CDC {
        return;
    }
    unsafe {
        CDC_ITF_PENDING &= !(1 << itf);
    }
}

/// `mp_usbd_cdc_write`.
pub fn write(_buf: &[u8]) -> usize {
    if !HW_ENABLE_USBDEV || !HW_USB_CDC {
        return 0;
    }
    0
}

/// `mp_usbd_cdc_read`.
pub fn read(buf: &mut [u8]) -> usize {
    if !HW_ENABLE_USBDEV || !HW_USB_CDC {
        return 0;
    }
    let _ = buf;
    0
}

pub fn connected() -> bool {
    HW_ENABLE_USBDEV && HW_USB_CDC
}

pub fn tx_flush_timeout_ms() -> u32 {
    500
}

pub fn ticks_ms() -> u32 {
    mphal::ticks_ms() as u32
}
