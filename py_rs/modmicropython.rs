//! rewrite of py/modmicropython.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::cstack;
use crate::gc;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, VaArg};
use crate::mpstate;
use crate::mphal;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objmodule;
use crate::objringio;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::scheduler;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T0: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F0.as_ptr() },
};
static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { FV.as_ptr() },
};

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 0, 0, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin0) };
    (self_.fun)()
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("mp fun0");
    unsafe {
        (*o).base.type_ = &T0 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("mp fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("mp fun2");
    unsafe {
        (*o).base.type_ = &T2 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("mp funv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn opt_level(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        mpstate::with_vm(|vm| obj::new_small_int(vm.mp_optimise_value as isize))
    } else {
        mpstate::with_vm(|vm| vm.mp_optimise_value = obj::get_int(args[0]) as usize);
        obj::CONST_NONE
    }
}

fn stack_use() -> Obj {
    obj::new_small_int(cstack::usage() as isize)
}

fn heap_lock() -> Obj {
    gc::lock();
    obj::CONST_NONE
}

fn heap_unlock() -> Obj {
    gc::unlock();
    let depth = mpstate::with_thread(|t| t.gc_lock_depth) as isize;
    obj::new_small_int(depth)
}

fn mem_total() -> Obj {
    obj::new_small_int(gc::mem_total_bytes() as isize)
}
fn mem_current() -> Obj {
    obj::new_small_int(gc::mem_current_bytes() as isize)
}
fn mem_peak() -> Obj {
    obj::new_small_int(gc::mem_peak_bytes() as isize)
}

fn mem_info(n: usize, _args: &[Obj]) -> Obj {
    if mpconfig::MEM_STATS {
        mpprint::printf(
            &mpprint::PLAT_PRINT,
            "mem: total=%u, current=%u, peak=%u\n",
            [
                VaArg::USize(gc::mem_total_bytes()),
                VaArg::USize(gc::mem_current_bytes()),
                VaArg::USize(gc::mem_peak_bytes()),
            ]
            .into_iter(),
        );
    }
    if mpconfig::STACK_CHECK {
        let limit = mpstate::with_thread(|t| t.stack_limit);
        mpprint::printf(
            &mpprint::PLAT_PRINT,
            "stack: %u out of %u\n",
            [
                VaArg::USize(cstack::usage()),
                VaArg::USize(limit as usize),
            ]
            .into_iter(),
        );
    } else {
        mpprint::printf(
            &mpprint::PLAT_PRINT,
            "stack: %u\n",
            std::iter::once(VaArg::USize(cstack::usage())),
        );
    }
    if mpconfig::ENABLE_GC {
        gc::dump_info(&mpprint::PLAT_PRINT);
        if n == 1 {
            gc::dump_alloc_table(&mpprint::PLAT_PRINT);
        }
    }
    obj::CONST_NONE
}

fn qstr_info(n: usize, _args: &[Obj]) -> Obj {
    let info = qstr::pool_info();
    mpprint::printf(
        &mpprint::PLAT_PRINT,
        "qstr pool: n_pool=%u, n_qstr=%u, n_str_data_bytes=%u, n_total_bytes=%u\n",
        [
            VaArg::USize(info.pools),
            VaArg::USize(info.qstrs),
            VaArg::USize(info.string_data_bytes),
            VaArg::USize(info.total_bytes),
        ]
        .into_iter(),
    );
    if n == 1 {
        qstr::dump_data();
    }
    obj::CONST_NONE
}

fn kbd_intr(int_chr: Obj) -> Obj {
    mphal::set_interrupt_char(obj::get_int(int_chr) as i32);
    obj::CONST_NONE
}

fn schedule(function: Obj, arg: Obj) -> Obj {
    if !scheduler::sched_schedule(function, arg) {
        raise::raise(MpRaise::RuntimeError("schedule queue full"));
    }
    obj::CONST_NONE
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_MICROPYTHON {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("micropython")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("const")),
            // C: `mp_identity_obj` — return the argument unchanged (compile-time fold).
            value: mk1(obj::identity),
        },
    ];
    if mpconfig::ENABLE_COMPILER {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("opt_level")),
            value: mkv(0, 1, opt_level),
        });
    }
    if mpconfig::PY_MICROPYTHON_MEM_INFO {
        if mpconfig::MEM_STATS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("mem_total")),
                value: mk0(mem_total),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("mem_current")),
                value: mk0(mem_current),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("mem_peak")),
                value: mk0(mem_peak),
            });
        }
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("mem_info")),
            value: mkv(0, 1, mem_info),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("qstr_info")),
            value: mkv(0, 1, qstr_info),
        });
    }
    if mpconfig::PY_MICROPYTHON_STACK_USE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("stack_use")),
            value: mk0(stack_use),
        });
    }
    if mpconfig::ENABLE_GC {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("heap_lock")),
            value: mk0(heap_lock),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("heap_unlock")),
            value: mk0(heap_unlock),
        });
    }
    if mpconfig::KBD_EXCEPTION {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("kbd_intr")),
            value: mk1(kbd_intr),
        });
    }
    if mpconfig::PY_MICROPYTHON_RINGIO {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("RingIO")),
            value: obj::from_ptr(objringio::type_ringio() as *const ObjType as *const ()),
        });
    }
    if mpconfig::ENABLE_SCHEDULER {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("schedule")),
            value: mk2(schedule),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("micropython module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("micropython"), module);
    module
}
