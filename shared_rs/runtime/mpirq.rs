//! rewrite of shared/runtime/mpirq.c + shared/runtime/mpirq.h
// symmetry: done

use py_rs::gc;
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint;
use py_rs::nlr;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_NONE};
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;
use py_rs::scheduler;

pub const INFO_FLAGS: u32 = 0;
pub const INFO_TRIGGERS: u32 = 1;

pub type IrqTriggerFn = fn(Obj, u32) -> u32;
pub type IrqInfoFn = fn(Obj, u32) -> u32;

#[repr(C)]
pub struct IrqMethods {
    pub trigger: IrqTriggerFn,
    pub info: IrqInfoFn,
}

#[repr(C)]
pub struct IrqObj {
    pub base: ObjBase,
    pub methods: *mut IrqMethods,
    pub parent: Obj,
    pub handler: Obj,
    pub ishard: bool,
}

pub fn new(methods: *mut IrqMethods, parent: Obj) -> *mut IrqObj {
    let self_ = malloc::new_obj::<IrqObj>().expect("mp_irq alloc");
    unsafe {
        init(&mut *self_, methods, parent);
    }
    self_
}

pub fn init(self_: &mut IrqObj, methods: *mut IrqMethods, parent: Obj) {
    self_.base.type_ = irq_type() as *const ObjType;
    self_.methods = methods;
    self_.parent = parent;
    self_.handler = obj::OBJ_NULL;
    self_.ishard = false;
}

pub fn dispatch(handler: Obj, parent: Obj, ishard: bool) -> i32 {
    if handler == obj::OBJ_NULL {
        return 0;
    }
    if ishard {
        scheduler::sched_lock();
        gc::lock();
        let mut nlr_buf = nlr::NlrBuf::default();
        let result = match nlr::protect(&mut nlr_buf, || runtime::call_function_1(handler, parent))
        {
            Ok(_) => 0,
            Err(_) => {
                let _ = mpprint::print_str(
                    &mpprint::PLAT_PRINT,
                    "Uncaught exception in IRQ callback handler\n",
                );
                -1
            }
        };
        gc::unlock();
        scheduler::sched_unlock();
        result
    } else {
        scheduler::sched_schedule(handler, parent);
        0
    }
}

pub fn handler(self_: &mut IrqObj) {
    if dispatch(self_.handler, self_.parent, self_.ishard) < 0 {
        unsafe {
            ((*self_.methods).trigger)(self_.parent, 0);
        }
        self_.handler = obj::OBJ_NULL;
    }
}

static mut IRQ_TYPE: Option<ObjType> = None;

pub fn irq_type() -> &'static ObjType {
    unsafe {
        if IRQ_TYPE.is_none() {
            IRQ_TYPE = Some(ObjType {
                base: ObjBase {
                    type_: core::ptr::null(),
                },
                flags: TYPE_FLAG_NONE,
                name: py_rs::qstr::from_str("irq"),
                slot_index_make_new: 0,
                slot_index_print: 0,
                slot_index_call: 0,
                slot_index_unary_op: 0,
                slot_index_binary_op: 0,
                slot_index_attr: 0,
                slot_index_subscr: 0,
                slot_index_iter: 0,
                slot_index_buffer: 0,
                slot_index_protocol: 0,
                slot_index_parent: 0,
                slot_index_locals_dict: 0,
                slots: core::ptr::null(),
            });
        }
        IRQ_TYPE.as_ref().unwrap()
    }
}

pub fn ensure_scheduler_enabled() {
    if !mpconfig::ENABLE_SCHEDULER {
        raise::raise(MpRaise::RuntimeError("scheduler disabled"));
    }
}
