//! rewrite of py/modsys.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpstate;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objexcept;
use crate::objmodule;
use crate::objstr;
use crate::objtuple;
use crate::qstr;
use crate::raise::{self, MpRaise};

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
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
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("sys fun0");
    unsafe {
        (*o).base.type_ = &T0 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("sys funv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn sys_exit(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        raise::raise_obj(objexcept::new_exception(objexcept::type_system_exit()));
    }
    raise::raise_obj(objexcept::new_exception_args(objexcept::type_system_exit(), 1, &[args[0]]));
}

fn sys_print_exception(n: usize, args: &[Obj]) -> Obj {
    let _ = n;
    objexcept::exception_print(&crate::mpprint::PLAT_PRINT, args[0], crate::mpprint::PrintKind::Exc);
    obj::CONST_NONE
}

fn sys_exc_info() -> Obj {
    let cur = mpstate::pending_exception();
    let items = if cur == obj::OBJ_NULL {
        vec![obj::CONST_NONE, obj::CONST_NONE, obj::CONST_NONE]
    } else {
        vec![obj::from_ptr(obj::get_type(cur) as *const ObjType as *const ()), cur, obj::CONST_NONE]
    };
    objtuple::new_tuple(3, Some(&items))
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_SYS {
        return obj::OBJ_NULL;
    }
    let version = objstr::new_str(b"3.4.0; metalpython");
    let version_info = objtuple::new_tuple(
        3,
        Some(&[
            obj::new_small_int(3),
            obj::new_small_int(4),
            obj::new_small_int(0),
        ]),
    );
    let mut table = vec![
        MapElem { key: obj::new_qstr(qstr::from_str("__name__")), value: obj::new_qstr(qstr::from_str("sys")) },
        MapElem { key: obj::new_qstr(qstr::from_str("version")), value: version },
        MapElem { key: obj::new_qstr(qstr::from_str("version_info")), value: version_info },
        MapElem {
            key: obj::new_qstr(qstr::from_str("byteorder")),
            value: obj::new_qstr(qstr::from_str(if mpconfig::ENDIANNESS_LITTLE { "little" } else { "big" })),
        },
        MapElem { key: obj::new_qstr(qstr::from_str("print_exception")), value: mkv(1, 2, sys_print_exception) },
    ];
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("platform")),
        value: objstr::new_str(mpconfig::PY_SYS_PLATFORM.as_bytes()),
    });
    if mpconfig::PY_SYS_MAXSIZE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("maxsize")),
            value: obj::new_small_int(isize::MAX),
        });
    }
    if mpconfig::PY_SYS_EXIT {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("exit")), value: mkv(0, 1, sys_exit) });
    }
    if mpconfig::PY_SYS_EXC_INFO {
        table.push(MapElem { key: obj::new_qstr(qstr::from_str("exc_info")), value: mk0(sys_exc_info) });
    }
    if mpconfig::PY_SYS_MODULES {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("modules")),
            value: mpstate::with_vm(|vm| vm.mp_loaded_modules_dict),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("sys module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("sys"), module);
    module
}
