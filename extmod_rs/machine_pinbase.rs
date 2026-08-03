//! rewrite of extmod/machine_pinbase.c
// symmetry: done

use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType};
use py_rs::qstr;
use py_rs::runtime;

use crate::virtpin::{self, PinProtocol, MP_PIN_READ, MP_PIN_WRITE};

#[repr(C)]
struct PinBaseObj {
    base: ObjBase,
}

static mut PINBASE_SINGLETON: PinBaseObj = PinBaseObj {
    base: ObjBase {
        type_: core::ptr::null(),
    },
};

fn pinbase_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    init_pinbase();
    unsafe { obj::from_ptr(&raw const PINBASE_SINGLETON as *const PinBaseObj as *const ()) }
}

fn pinbase_ioctl(obj: Obj, request: u32, arg: usize, _errcode: *mut i32) -> usize {
    match request {
        MP_PIN_READ => {
            let mut dest = [obj::OBJ_NULL; 2];
            runtime::load_method(obj, qstr::from_str("value"), &mut dest);
            obj::get_int_truncated(runtime::call_method_n_kw(0, 0, &dest)) as usize
        }
        MP_PIN_WRITE => {
            let mut dest = [obj::OBJ_NULL; 2];
            runtime::load_method(obj, qstr::from_str("value"), &mut dest);
            let call_args = [
                dest[0],
                dest[1],
                if arg == 0 {
                    obj::CONST_FALSE
                } else {
                    obj::CONST_TRUE
                },
            ];
            runtime::call_method_n_kw(1, 0, &call_args);
            0
        }
        _ => usize::MAX,
    }
}

static PINBASE_PIN_P: PinProtocol = PinProtocol {
    ioctl: pinbase_ioctl,
};

static mut PINBASE_SLOTS: [*const (); 2] =
    [pinbase_make_new as *const (), &raw const PINBASE_PIN_P as *const ()];
static mut PINBASE_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { PINBASE_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_pinbase() {
    INIT.get_or_init(|| {
        unsafe {
            PINBASE_TYPE.name = qstr::from_str("PinBase");
            PINBASE_SINGLETON.base.type_ = &PINBASE_TYPE;
        }
    });
}

/// `machine_pinbase_type`
pub fn pinbase_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_PIN_BASE {
        panic!("PinBase disabled");
    }
    init_pinbase();
    unsafe { &PINBASE_TYPE }
}
