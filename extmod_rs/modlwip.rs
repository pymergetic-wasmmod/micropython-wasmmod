//! rewrite of extmod/modlwip.c
//! Host has no lwIP port HAL (OS abstraction, netif, sockets, `modnetwork` glue).
//! TCP/IP stack integration requires port-specific `lwipopts.h` and drivers.
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const MOD_NETWORK_AF_INET: isize = 2;
const MOD_NETWORK_AF_INET6: isize = 10;
const MOD_NETWORK_SOCK_STREAM: isize = 1;
const MOD_NETWORK_SOCK_DGRAM: isize = 2;
const MOD_NETWORK_SOCK_RAW: isize = 3;

const SOL_SOCKET: isize = 1;
const SOF_REUSEADDR: isize = 0x04;
const SOF_BROADCAST: isize = 0x20;
const IPPROTO_IP: isize = 0;
const IP_ADD_MEMBERSHIP: isize = 0x400;
const IP_DROP_MEMBERSHIP: isize = 0x401;
const IP_PROTO_TCP: isize = 6;
const TCP_NODELAY: isize = 0x01;
const MSG_PEEK: isize = 0x01;
const MSG_DONTWAIT: isize = 0x02;

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
    argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("lwip fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("lwip fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn lwip_unavailable() -> ! {
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        1,
        &[objstr::new_str(b"lwip not available")],
    ));
}

fn lwip_reset() -> Obj {
    lwip_unavailable();
}

fn lwip_callback(_n: usize, _args: &[Obj]) -> Obj {
    lwip_unavailable();
}

fn lwip_getaddrinfo(_n: usize, _args: &[Obj]) -> Obj {
    lwip_unavailable();
}

fn lwip_print_pcbs() -> Obj {
    lwip_unavailable();
}

fn lwip_socket_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    lwip_unavailable();
}

fn lwip_slip_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    lwip_unavailable();
}

static mut SOCKET_SLOTS: [*const (); 2] = [
    lwip_socket_make_new as MakeNewFn as *const (),
    core::ptr::null(),
];
static mut TYPE_SOCKET: ObjType = ObjType {
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
    slots: unsafe { SOCKET_SLOTS.as_ptr() },
};

static mut SLIP_SLOTS: [*const (); 2] = [
    lwip_slip_make_new as MakeNewFn as *const (),
    core::ptr::null(),
];
static mut TYPE_SLIP: ObjType = ObjType {
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
    slots: unsafe { SLIP_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    INIT.get_or_init(|| {
        let methods = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("close")),
            value: mk0(|| lwip_unavailable()),
        }];
        let dict = objdict::new_dict(methods.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, methods.clone());
            SOCKET_SLOTS[1] = objdict::dict_ptr(dict) as *const ();
            SLIP_SLOTS[1] = objdict::dict_ptr(dict) as *const ();
            TYPE_SOCKET.name = qstr::from_str("socket");
            TYPE_SLIP.name = qstr::from_str("slip");
        }
    });
}

fn type_socket() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_SOCKET }
}

fn type_slip() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_SLIP }
}

static MODULE_INIT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// Register built-in `lwip` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_LWIP {
        return obj::OBJ_NULL;
    }
    *MODULE_INIT.get_or_init(|| {
        init_types();
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("__name__")),
                value: obj::new_qstr(qstr::from_str("lwip")),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("reset")),
                value: mk0(lwip_reset),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("callback")),
                value: mkv(1, 2, lwip_callback),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("getaddrinfo")),
                value: mkv(2, 6, lwip_getaddrinfo),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("print_pcbs")),
                value: mk0(lwip_print_pcbs),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("socket")),
                value: obj::from_ptr(type_socket() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("slip")),
                value: obj::from_ptr(type_slip() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("AF_INET")),
                value: obj::new_small_int(MOD_NETWORK_AF_INET),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("AF_INET6")),
                value: obj::new_small_int(MOD_NETWORK_AF_INET6),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SOCK_STREAM")),
                value: obj::new_small_int(MOD_NETWORK_SOCK_STREAM),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SOCK_DGRAM")),
                value: obj::new_small_int(MOD_NETWORK_SOCK_DGRAM),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SOCK_RAW")),
                value: obj::new_small_int(MOD_NETWORK_SOCK_RAW),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SOL_SOCKET")),
                value: obj::new_small_int(SOL_SOCKET),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SO_REUSEADDR")),
                value: obj::new_small_int(SOF_REUSEADDR),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("SO_BROADCAST")),
                value: obj::new_small_int(SOF_BROADCAST),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IPPROTO_IP")),
                value: obj::new_small_int(IPPROTO_IP),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IP_ADD_MEMBERSHIP")),
                value: obj::new_small_int(IP_ADD_MEMBERSHIP),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IP_DROP_MEMBERSHIP")),
                value: obj::new_small_int(IP_DROP_MEMBERSHIP),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("IPPROTO_TCP")),
                value: obj::new_small_int(IP_PROTO_TCP),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("TCP_NODELAY")),
                value: obj::new_small_int(TCP_NODELAY),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MSG_PEEK")),
                value: obj::new_small_int(MSG_PEEK),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MSG_DONTWAIT")),
                value: obj::new_small_int(MSG_DONTWAIT),
            },
        ];
        let ctx = malloc::new_obj::<ModuleContext>().expect("lwip module");
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
            (*ctx).module.base.type_ = objmodule::type_module();
            (*ctx).module.globals = objdict::dict_ptr(dict);
            (*ctx).constants = Default::default();
        }
        let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
        objmodule::register_builtin_module(qstr::from_str("lwip"), module);
        module
    })
}
