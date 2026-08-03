//! rewrite of extmod/virtpin.c + extmod/virtpin.h
// symmetry: done

use py_rs::obj::{self, Obj, ObjBase, ObjType};

pub const MP_PIN_READ: u32 = 1;
pub const MP_PIN_WRITE: u32 = 2;
pub const MP_PIN_INPUT: u32 = 3;
pub const MP_PIN_OUTPUT: u32 = 4;

pub type PinIoctl = fn(Obj, u32, usize, *mut i32) -> usize;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PinProtocol {
    pub ioctl: PinIoctl,
}

fn pin_protocol(pin: Obj) -> Option<PinProtocol> {
    let base = obj::as_ptr(pin) as *const ObjBase;
    let type_ = unsafe { (*base).type_ };
    if type_.is_null() {
        return None;
    }
    let idx = unsafe { (*type_).slot_index_protocol };
    if idx == 0 {
        return None;
    }
    let slots = unsafe { (*type_).slots };
    if slots.is_null() {
        return None;
    }
    Some(unsafe { *(slots.add(idx as usize - 1) as *const PinProtocol) })
}

/// Whether `obj` exposes the pin protocol slot.
pub fn has_pin_protocol(pin: Obj) -> bool {
    pin_protocol(pin).is_some()
}

/// `mp_virtual_pin_read`
pub fn virtual_pin_read(pin: Obj) -> i32 {
    pin_protocol(pin)
        .map(|p| (p.ioctl)(pin, MP_PIN_READ, 0, std::ptr::null_mut()) as i32)
        .unwrap_or(0)
}

/// `mp_virtual_pin_write`
pub fn virtual_pin_write(pin: Obj, value: i32) {
    if let Some(p) = pin_protocol(pin) {
        (p.ioctl)(pin, MP_PIN_WRITE, value as usize, std::ptr::null_mut());
    }
}
