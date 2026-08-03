//! rewrite of extmod/modbluetooth.c + extmod/modbluetooth.h
//! Host has no BLE controller HAL (NimBLE/BTstack HCI transport, GAP/GATT stack).
//! `bluetooth.BLE` IRQ and bonding require port HCI and link-layer driver.
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const FLAG_READ: isize = 0x0002;
const FLAG_WRITE_NO_RESPONSE: isize = 0x0004;
const FLAG_WRITE: isize = 0x0008;
const FLAG_NOTIFY: isize = 0x0010;
const FLAG_INDICATE: isize = 0x0020;

const UUID_TYPE_16: u8 = 2;
const UUID_TYPE_32: u8 = 4;
const UUID_TYPE_128: u8 = 16;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;

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
struct ObjBluetoothUuid {
    base: ObjBase,
    uuid_type: u8,
    data: [u8; 16],
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];

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

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("bluetooth fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("bluetooth fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn bluetooth_unavailable() -> ! {
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_os_error(),
        1,
        &[objstr::new_str(b"bluetooth not available")],
    ));
}

fn ble_make_new(_type_in: &ObjType, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    bluetooth_unavailable();
}

fn uuid_parse_int(value: Obj, self_: &mut ObjBluetoothUuid) {
    let v = obj::get_int(value);
    if v < 0 || v > 65535 {
        raise::raise(MpRaise::ValueError("invalid UUID"));
    }
    self_.uuid_type = UUID_TYPE_16;
    self_.data[0] = (v & 0xff) as u8;
    self_.data[1] = ((v >> 8) & 0xff) as u8;
}

fn uuid_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let o = malloc::new_obj::<ObjBluetoothUuid>().expect("UUID");
    unsafe {
        (*o).base.type_ = type_uuid();
        (*o).uuid_type = 0;
        (*o).data = [0; 16];
        if obj::is_int(args[0]) {
            uuid_parse_int(args[0], &mut *o);
        } else {
            let mut info = BufferInfo {
                buf: core::ptr::null_mut(),
                len: 0,
                typecode: 0,
            };
            obj::get_buffer_raise(args[0], &mut info, obj::BUFFER_READ);
            let slice = unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len) };
            if slice.len() == 2 || slice.len() == 4 || slice.len() == 16 {
                (*o).uuid_type = slice.len() as u8;
                (&mut (*o).data)[..slice.len()].copy_from_slice(slice);
            } else {
                raise::raise(MpRaise::ValueError("invalid UUID"));
            }
        }
        obj::from_ptr(o as *const ObjBluetoothUuid as *const ())
    }
}

static mut BLE_SLOTS: [*const (); 2] = [ble_make_new as MakeNewFn as *const (), core::ptr::null()];
static mut TYPE_BLE: ObjType = ObjType {
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
    slots: unsafe { BLE_SLOTS.as_ptr() },
};

static mut UUID_SLOTS: [*const (); 2] =
    [uuid_make_new as MakeNewFn as *const (), core::ptr::null()];
static mut TYPE_UUID: ObjType = ObjType {
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
    slots: unsafe { UUID_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    INIT.get_or_init(|| {
        let ble_methods = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("active")),
            value: mk0(|| bluetooth_unavailable()),
        }];
        let uuid_methods = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("uuid")),
            value: mk1(|_| bluetooth_unavailable()),
        }];
        let ble_dict = objdict::new_dict(ble_methods.len());
        let uuid_dict = objdict::new_dict(uuid_methods.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(ble_dict)).map, ble_methods);
            map::init_fixed_table(&mut (*objdict::dict_ptr(uuid_dict)).map, uuid_methods);
            BLE_SLOTS[1] = objdict::dict_ptr(ble_dict) as *const ();
            UUID_SLOTS[1] = objdict::dict_ptr(uuid_dict) as *const ();
            TYPE_BLE.name = qstr::from_str("BLE");
            TYPE_UUID.name = qstr::from_str("UUID");
        }
    });
}

fn type_ble() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_BLE }
}

fn type_uuid() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_UUID }
}

static MODULE_INIT: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// Register built-in `bluetooth` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_BLUETOOTH {
        return obj::OBJ_NULL;
    }
    *MODULE_INIT.get_or_init(|| {
        init_types();
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("__name__")),
                value: obj::new_qstr(qstr::from_str("bluetooth")),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("BLE")),
                value: obj::from_ptr(type_ble() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("UUID")),
                value: obj::from_ptr(type_uuid() as *const ObjType as *const ()),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_READ")),
                value: obj::new_small_int(FLAG_READ),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_WRITE")),
                value: obj::new_small_int(FLAG_WRITE),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_NOTIFY")),
                value: obj::new_small_int(FLAG_NOTIFY),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_INDICATE")),
                value: obj::new_small_int(FLAG_INDICATE),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("FLAG_WRITE_NO_RESPONSE")),
                value: obj::new_small_int(FLAG_WRITE_NO_RESPONSE),
            },
        ];
        let ctx = malloc::new_obj::<ModuleContext>().expect("bluetooth module");
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
            (*ctx).module.base.type_ = objmodule::type_module();
            (*ctx).module.globals = objdict::dict_ptr(dict);
            (*ctx).constants = Default::default();
        }
        let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
        objmodule::register_builtin_module(qstr::from_str("bluetooth"), module);
        module
    })
}
