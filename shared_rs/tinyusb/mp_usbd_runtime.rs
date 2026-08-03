//! rewrite of shared/tinyusb/mp_usbd_runtime.c
// symmetry: done

use py_rs::obj::Obj;

use super::tusb_config::{HW_ENABLE_USBDEV, HW_ENABLE_USB_RUNTIME_DEVICE};

pub const MAX_PEND_EXCS: usize = 4;

#[repr(C)]
pub struct UsbDevice {
    pub base: py_rs::obj::ObjBase,
    pub num_pend_excs: u8,
    pub pend_excs: [Obj; MAX_PEND_EXCS],
}

static mut IN_USBD_TASK: bool = false;

/// `mp_usbd_task`.
pub fn task() {
    if !HW_ENABLE_USBDEV || !HW_ENABLE_USB_RUNTIME_DEVICE {
        return;
    }
    if unsafe { IN_USBD_TASK } {
        return;
    }
    unsafe {
        IN_USBD_TASK = true;
    }
    task_inner();
    unsafe {
        IN_USBD_TASK = false;
    }
}

fn task_inner() {}

/// `mp_usbd_disconnect`.
pub fn disconnect(_usbd: &mut UsbDevice) {}

/// Pend an exception from a USB callback.
pub fn pend_exception(usbd: &mut UsbDevice, exception: Obj) {
    if usbd.num_pend_excs < MAX_PEND_EXCS as u8 {
        usbd.pend_excs[usbd.num_pend_excs as usize] = exception;
    }
    usbd.num_pend_excs = usbd.num_pend_excs.saturating_add(1);
}

/// Drain pending exceptions raised during TinyUSB callbacks.
pub fn flush_pending_exceptions(usbd: &mut UsbDevice) {
    for i in 0..usbd.num_pend_excs as usize {
        let exc = usbd.pend_excs[i];
        if exc != py_rs::obj::OBJ_NULL {
            py_rs::obj::print_exception(&py_rs::mpprint::PLAT_PRINT, exc);
        }
    }
    usbd.num_pend_excs = 0;
}

pub fn in_task() -> bool {
    unsafe { IN_USBD_TASK }
}
