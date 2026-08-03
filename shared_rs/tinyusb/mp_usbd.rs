//! rewrite of shared/tinyusb/mp_usbd.c + shared/tinyusb/mp_usbd.h
// symmetry: done

use py_rs::scheduler;

use super::tusb_config::HW_ENABLE_USBDEV;

/// `mp_usbd_task`.
pub fn task() {
    if !HW_ENABLE_USBDEV {
        return;
    }
    tud_task_ext(0, false);
}

fn tud_task_ext(_rhport: u8, _in_isr: bool) {}

/// `mp_usbd_task_callback`.
pub fn task_callback(_node: *mut ()) {
    task();
}

/// `mp_usbd_schedule_task`.
pub fn schedule_task() {
    if !HW_ENABLE_USBDEV {
        return;
    }
    static mut USBD_TASK_NODE: *mut () = core::ptr::null_mut();
    unsafe {
        scheduler::sched_schedule(
            py_rs::obj::OBJ_NULL,
            py_rs::obj::from_ptr(USBD_TASK_NODE),
        );
    }
}

/// `tud_event_hook_cb` wrapper.
pub fn event_hook_cb(_rhport: u8, _eventid: u32, _in_isr: bool) {
    schedule_task();
    py_rs::mphal::internal_event_hook();
}

/// `mp_usbd_hex_str`.
pub fn hex_str(out_str: &mut [u8], bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hex_len = bytes.len() * 2;
    for i in 0..hex_len {
        out_str[i] = HEX[(bytes[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) as usize & 0x0f];
    }
    if hex_len < out_str.len() {
        out_str[hex_len] = 0;
    }
}

pub fn wake_main_task_from_isr() {
    py_rs::mphal::internal_event_hook();
}
