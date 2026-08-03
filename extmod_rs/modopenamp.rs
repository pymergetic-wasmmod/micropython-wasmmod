//! rewrite of extmod/modopenamp.c + extmod/modopenamp.h
//! Host has no OpenAMP/rpmsg HAL (remoteproc, virtio queues, shared memory).
//! RPMsg endpoint API requires port remote processor and mailbox driver.
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, Map, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const ENDPOINT_ADDR_ANY: isize = -1;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

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
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

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
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
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

static TK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FK.as_ptr() },
};

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("openamp fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("openamp fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("openamp fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn openamp_unavailable() -> ! {
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        1,
        &[objstr::new_str(b"openamp not available")],
    ));
}

fn endpoint_del(_self_in: Obj) -> Obj {
    obj::CONST_NONE
}

fn endpoint_send(_n: usize, _args: &[Obj], _kw: &Map) -> Obj {
    openamp_unavailable();
}

fn endpoint_is_ready(_self_in: Obj) -> Obj {
    obj::new_bool(false)
}

fn new_service_callback(callback: Obj) -> Obj {
    if callback != obj::CONST_NONE && !obj::is_callable(callback) {
        raise::raise(MpRaise::ValueError("invalid callback"));
    }
    openamp_unavailable();
}

fn endpoint_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    openamp_unavailable();
}

fn remoteproc_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    openamp_unavailable();
}

static mut ENDPOINT_SLOTS: [*const (); 2] = [
    endpoint_make_new as MakeNewFn as *const (),
    core::ptr::null(),
];
static mut TYPE_ENDPOINT: ObjType = ObjType {
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
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { ENDPOINT_SLOTS.as_ptr() },
};

static mut REMOTEPROC_SLOTS: [*const (); 2] = [
    remoteproc_make_new as MakeNewFn as *const (),
    core::ptr::null(),
];
static mut TYPE_REMOTEPROC: ObjType = ObjType {
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
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { REMOTEPROC_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<(Obj, Obj, Obj)> = std::sync::OnceLock::new();

fn endpoint_method_exports() -> (Obj, Obj, Obj) {
    *INIT.get_or_init(|| {
        let del_fn = mk1(endpoint_del);
        let send_fn = mk_kw(2, endpoint_send);
        let ready_fn = mk1(endpoint_is_ready);
        let endpoint_methods = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("__del__")),
                value: del_fn,
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("send")),
                value: send_fn,
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("is_ready")),
                value: ready_fn,
            },
        ];
        let remoteproc_methods = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("start")),
            value: mk0(|| openamp_unavailable()),
        }];
        let endpoint_dict = objdict::new_dict(endpoint_methods.len());
        let remoteproc_dict = objdict::new_dict(remoteproc_methods.len());
        unsafe {
            map::init_fixed_table(
                &mut (*objdict::dict_ptr(endpoint_dict)).map,
                endpoint_methods,
            );
            map::init_fixed_table(
                &mut (*objdict::dict_ptr(remoteproc_dict)).map,
                remoteproc_methods,
            );
            ENDPOINT_SLOTS[1] = objdict::dict_ptr(endpoint_dict) as *const ();
            REMOTEPROC_SLOTS[1] = objdict::dict_ptr(remoteproc_dict) as *const ();
            TYPE_ENDPOINT.name = qstr::from_str("Endpoint");
            TYPE_REMOTEPROC.name = qstr::from_str("RemoteProc");
        }
        (del_fn, send_fn, ready_fn)
    })
}

fn type_endpoint() -> &'static ObjType {
    let _ = endpoint_method_exports();
    unsafe { &TYPE_ENDPOINT }
}

fn type_remoteproc() -> &'static ObjType {
    let _ = endpoint_method_exports();
    unsafe { &TYPE_REMOTEPROC }
}

static MODULE_INIT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// Register built-in `openamp` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_OPENAMP {
        return obj::OBJ_NULL;
    }
    *MODULE_INIT.get_or_init(|| {
        let (del_fn, send_fn, ready_fn) = endpoint_method_exports();
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("__name__")),
                value: obj::new_qstr(qstr::from_str("openamp")),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__del__")),
                value: del_fn,
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("send")),
                value: send_fn,
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("is_ready")),
                value: ready_fn,
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ENDPOINT_ADDR_ANY")),
                value: obj::new_small_int(ENDPOINT_ADDR_ANY),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("new_service_callback")),
                value: mk1(new_service_callback),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("Endpoint")),
                value: obj::from_ptr(type_endpoint() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("RemoteProc")),
                value: obj::from_ptr(type_remoteproc() as *const ObjType as *const ()),
            },
        ];
        let ctx = malloc::new_obj::<ModuleContext>().expect("openamp module");
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
            (*ctx).module.base.type_ = objmodule::type_module();
            (*ctx).module.globals = objdict::dict_ptr(dict);
            (*ctx).constants = Default::default();
        }
        let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
        objmodule::register_builtin_module(qstr::from_str("openamp"), module);
        module
    })
}
