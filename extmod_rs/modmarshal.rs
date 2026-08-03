//! rewrite of extmod/modmarshal.c
// symmetry: done
// Note: `loads` at `PY_BUILTINS_CODE_FULL` needs `objcode::new_code(context, rc, …)` when config is raised above BASIC.

use py_rs::bc::ModuleContext;
use py_rs::emitglue::{CompiledModule, ProtoFun};
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objcode;
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::persistentcode;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("marshal fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn marshal_dumps(value: Obj) -> Obj {
    if let Some(code) = objcode::as_code(value) {
        let consts = objcode::code_get_constants(code);
        let proto = objcode::code_get_proto_fun(code);
        return persistentcode::raw_code_save_fun_to_bytes(consts, proto);
    }
    raise::raise(MpRaise::ValueError("unmarshallable object"));
}

fn marshal_loads(data: Obj) -> Obj {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(data, &mut bufinfo, obj::BUFFER_READ);
    let buf = unsafe { std::slice::from_raw_parts(bufinfo.buf as *const u8, bufinfo.len) };

    let ctx = malloc::new_obj::<ModuleContext>().expect("marshal ctx");
    unsafe {
        (*ctx).module.globals = objdict::dict_ptr(runtime::globals_get());
    }
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };
    persistentcode::raw_code_load_mem(buf, &mut cm);

    if mpconfig::PY_BUILTINS_CODE <= mpconfig::PY_BUILTINS_CODE_BASIC {
        let constants = unsafe { (*ctx).constants.clone() };
        objcode::new_code(constants, cm.rc as ProtoFun)
    } else {
        // `mp_obj_new_code(context, rc, true)` at FULL level — not in objcode.rs yet.
        raise::raise(MpRaise::RuntimeError("marshal loads at FULL code level"));
    }
}

/// Register built-in `marshal` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_MARSHAL {
        return obj::OBJ_NULL;
    }
    let table = [
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("marshal")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("dumps")),
            value: mk1(marshal_dumps),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("loads")),
            value: mk1(marshal_loads),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("marshal module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("marshal"), module);
    module
}
