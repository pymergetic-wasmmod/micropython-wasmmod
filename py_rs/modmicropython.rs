//! rewrite of py/modmicropython.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::cstack;
use crate::gc;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpstate;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objmodule;
use crate::qstr;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T0: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
static TV: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
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
    obj::new_small_int(0)
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_MICROPYTHON {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("micropython")) },
        MapElem { key: obj::new_qstr(qstr::from_str("const")), value: mk0(|| obj::CONST_NONE) },
    ];
    if mpconfig::ENABLE_COMPILER {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("opt_level")), value: mkv(0, 1, opt_level) });
    }
    if mpconfig::PY_MICROPYTHON_STACK_USE {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("stack_use")), value: mk0(stack_use) });
    }
    if mpconfig::ENABLE_GC {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("heap_lock")), value: mk0(heap_lock) });
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("heap_unlock")), value: mk0(heap_unlock) });
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
