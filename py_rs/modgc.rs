//! rewrite of py/modgc.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::gc;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpstate;
use crate::obj::{self, Obj, ObjType};
use crate::objdict;
use crate::objmodule;
use crate::qstr;
use crate::runtime;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: obj::ObjBase,
    fun: BuiltinFn0,
}

#[repr(C)]
struct ObjFunBuiltinVar {
    base: obj::ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FUN0_SLOTS: [*const (); 1] = [fun0_call as *const ()];
static mut FUNVAR_SLOTS: [*const (); 1] = [funvar_call as *const ()];

static TYPE_FUN0: ObjType = ObjType {
    base: obj::ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FUN0_SLOTS.as_ptr() },
};

static TYPE_FUNVAR: ObjType = ObjType {
    base: obj::ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FUNVAR_SLOTS.as_ptr() },
};

fn fun0_call(self_in: Obj, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    crate::argcheck::check_num(n_args, n_kw, 0, 0, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin0) };
    (self_.fun)()
}

fn funvar_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}

fn new_fun0(fun: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("fun0");
    unsafe {
        (*o).base.type_ = &TYPE_FUN0 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}

fn new_fun_var(min: u8, max: u8, fun: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("funvar");
    unsafe {
        (*o).base.type_ = &TYPE_FUNVAR as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn py_gc_collect() -> Obj {
    gc::collect();
    if mpconfig::PY_GC_COLLECT_RETVAL {
        mpstate::with_mem(|mem| obj::new_small_int(mem.gc_collected as isize))
    } else {
        obj::CONST_NONE
    }
}

fn gc_disable() -> Obj {
    mpstate::with_mem(|mem| mem.gc_auto_collect_enabled = 0);
    obj::CONST_NONE
}

fn gc_enable() -> Obj {
    mpstate::with_mem(|mem| mem.gc_auto_collect_enabled = 1);
    obj::CONST_NONE
}

fn gc_isenabled() -> Obj {
    mpstate::with_mem(|mem| obj::new_bool(mem.gc_auto_collect_enabled != 0))
}

fn gc_mem_free() -> Obj {
    let info = gc::info_full();
    obj::new_small_int(info.free as isize)
}

fn gc_mem_alloc() -> Obj {
    let info = gc::info_full();
    obj::new_small_int(info.used as isize)
}

fn gc_threshold(_n_args: usize, _args: &[Obj]) -> Obj {
    obj::CONST_NONE
}

pub fn init_module() -> Obj {
    if !(mpconfig::PY_GC && mpconfig::ENABLE_GC) {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("gc")) },
        MapElem { key: obj::new_qstr(qstr::from_str("collect")), value: new_fun0(py_gc_collect) },
        MapElem { key: obj::new_qstr(qstr::from_str("disable")), value: new_fun0(gc_disable) },
        MapElem { key: obj::new_qstr(qstr::from_str("enable")), value: new_fun0(gc_enable) },
        MapElem { key: obj::new_qstr(qstr::from_str("isenabled")), value: new_fun0(gc_isenabled) },
        MapElem { key: obj::new_qstr(qstr::from_str("mem_free")), value: new_fun0(gc_mem_free) },
        MapElem { key: obj::new_qstr(qstr::from_str("mem_alloc")), value: new_fun0(gc_mem_alloc) },
    ];
    if mpconfig::GC_ALLOC_THRESHOLD {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("threshold")),
            value: new_fun_var(0, 1, gc_threshold),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("gc module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("gc"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn gc_module_collect() {
        let _ = gc::init();
        runtime::init();
        let m = init_module();
        assert!(obj::is_obj(m));
        assert_eq!(py_gc_collect(), obj::new_small_int(0));
    }
}
