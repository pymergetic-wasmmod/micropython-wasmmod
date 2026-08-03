//! Metal guest `network.LAN` façade over `pm_metal_net_ip_if_*`.
//!
//! Enabled with `feature = "metal_net"`. No NIC drivers here — status/DHCP
//! only, matching MicroPython's AbstractNIC subset for a wired link.

use core::mem::size_of;
use py_rs::argcheck;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::metal_net;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

#[repr(C)]
struct ObjLan {
    base: ObjBase,
    active: bool,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut LAN_SLOTS: [*const (); 3] = [
    lan_make_new as *const (),
    core::ptr::null(),
    core::ptr::null(),
];

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

static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

static mut TYPE_LAN: ObjType = ObjType {
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
    slots: unsafe { LAN_SLOTS.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
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

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("lan fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("lan fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn lan_ptr(o: Obj) -> *mut ObjLan {
    obj::as_ptr(o) as *mut ObjLan
}

fn lan_make_new(_type_: &ObjType, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjLan>().expect("LAN");
    unsafe {
        (*o).base.type_ = type_lan();
        (*o).active = metal_net::metal_net_enabled();
        obj::from_ptr(o as *const ObjLan as *const ())
    }
}

fn lan_active(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &mut *lan_ptr(args[0]) };
    if n == 1 {
        return obj::new_bool(self_.active);
    }
    self_.active = obj::is_true(args[1]);
    obj::CONST_NONE
}

fn lan_isconnected(self_in: Obj) -> Obj {
    let self_ = unsafe { &*lan_ptr(self_in) };
    if !self_.active {
        return obj::CONST_FALSE;
    }
    #[cfg(feature = "metal_net")]
    {
        metal_net::status::poll();
        return obj::new_bool(metal_net::status::dhcp_ready("eth0").is_some());
    }
    #[cfg(not(feature = "metal_net"))]
    {
        obj::CONST_FALSE
    }
}

fn lan_status(self_in: Obj) -> Obj {
    lan_isconnected(self_in)
}

fn lan_ifconfig(n: usize, args: &[Obj]) -> Obj {
    let _ = unsafe { &*lan_ptr(args[0]) };
    if n > 1 {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EOPNOTSUPP as i32));
    }
    #[cfg(feature = "metal_net")]
    {
        metal_net::status::poll();
        let ip = metal_net::status::dhcp_ready("eth0").unwrap_or_default();
        let items = [
            objstr::new_str(ip.as_bytes()),
            objstr::new_str(b"0.0.0.0"),
            objstr::new_str(b"0.0.0.0"),
            objstr::new_str(b"0.0.0.0"),
        ];
        return objtuple::new_tuple(4, Some(&items));
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = n;
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV as i32));
    }
}

static LAN_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_lan() -> &'static ObjType {
    LAN_INIT.get_or_init(|| unsafe {
        TYPE_LAN.name = qstr::from_str("LAN");
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("active")),
                value: mkv(1, 2, lan_active),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("isconnected")),
                value: mk1(lan_isconnected),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("status")),
                value: mk1(lan_status),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ifconfig")),
                value: mkv(1, 2, lan_ifconfig),
            },
        ];
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        map::init_fixed_table(&mut (*ptr).map, table);
        LAN_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
    });
    unsafe { &TYPE_LAN }
}

/// Object to register as `network.LAN` (or `OBJ_NULL` when network disabled).
pub fn lan_type_obj() -> Obj {
    obj::from_ptr(type_lan() as *const ObjType as *const ())
}
