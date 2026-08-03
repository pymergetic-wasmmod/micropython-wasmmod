//! rewrite of extmod/modnetwork.c + extmod/modnetwork.h
//! Host-complete for pure helpers: `country`, `hostname`, `STA_IF`, `AP_IF`.
//! `WLAN`/`LAN`/`AbstractNIC`, `route()`, and `ipconfig()` need port NIC HAL (CYW43/WIZnet/lwIP/etc.).
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const MOD_NETWORK_STA_IF: isize = 0;
const MOD_NETWORK_AP_IF: isize = 1;

static mut MOD_NETWORK_COUNTRY_CODE: [u8; 2] = *b"XX";
static mut MOD_NETWORK_HOSTNAME_DATA: [u8; mpconfig::PY_NETWORK_HOSTNAME_MAX_LEN + 1] =
    [0; mpconfig::PY_NETWORK_HOSTNAME_MAX_LEN + 1];

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_storage() {
    INIT.get_or_init(|| {
        let default = mpconfig::PY_NETWORK_HOSTNAME_DEFAULT.as_bytes();
        let len = default.len().min(mpconfig::PY_NETWORK_HOSTNAME_MAX_LEN);
        unsafe {
            MOD_NETWORK_HOSTNAME_DATA[..len].copy_from_slice(&default[..len]);
            MOD_NETWORK_HOSTNAME_DATA[len] = 0;
        }
    });
}

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV: [*const (); 1] = [callv as *const ()];
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

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("network fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn network_country(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        let code = unsafe { MOD_NETWORK_COUNTRY_CODE };
        return objstr::new_str(&code);
    }
    let s = objstr::str_get_str(args[0]);
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        raise::raise(MpRaise::ValueError(""));
    }
    unsafe {
        MOD_NETWORK_COUNTRY_CODE[0] = bytes[0];
        MOD_NETWORK_COUNTRY_CODE[1] = bytes[1];
    }
    obj::CONST_NONE
}

/// `mod_network_hostname_data` — C string buffer for CYW43/mDNS hostname.
pub fn hostname_data() -> &'static [u8] {
    init_storage();
    unsafe {
        let len = MOD_NETWORK_HOSTNAME_DATA
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(MOD_NETWORK_HOSTNAME_DATA.len());
        &MOD_NETWORK_HOSTNAME_DATA[..len]
    }
}

/// `mod_network_hostname` — get/set the device hostname.
pub fn mod_network_hostname(n: usize, args: &[Obj]) -> Obj {
    init_storage();
    if n == 0 {
        let data = unsafe { MOD_NETWORK_HOSTNAME_DATA };
        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        return objstr::new_str(&data[..len]);
    }
    let s = objstr::str_get_str(args[0]);
    let bytes = s.as_bytes();
    if bytes.len() > mpconfig::PY_NETWORK_HOSTNAME_MAX_LEN {
        raise::raise(MpRaise::ValueError(""));
    }
    unsafe {
        MOD_NETWORK_HOSTNAME_DATA[..bytes.len()].copy_from_slice(bytes);
        MOD_NETWORK_HOSTNAME_DATA[bytes.len()] = 0;
    }
    obj::CONST_NONE
}

/// Register built-in `network` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_NETWORK {
        return obj::OBJ_NULL;
    }
    init_storage();
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("network")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("country")),
            value: mkv(0, 1, network_country),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("hostname")),
            value: mkv(0, 1, mod_network_hostname),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("STA_IF")),
            value: obj::new_small_int(MOD_NETWORK_STA_IF),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("AP_IF")),
            value: obj::new_small_int(MOD_NETWORK_AP_IF),
        },
    ];
    // Metal guest wired NIC façade (status/DHCP via pm_metal_net_ip_*).
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("LAN")),
        value: crate::network_metal::lan_type_obj(),
    });
    let ctx = malloc::new_obj::<ModuleContext>().expect("network module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("network"), module);
    module
}
