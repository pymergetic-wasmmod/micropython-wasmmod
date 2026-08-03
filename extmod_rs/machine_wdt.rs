//! rewrite of extmod/machine_wdt.c
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::qstr;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

#[repr(C)]
struct MachineWdt {
    base: ObjBase,
    timeout_ms: i32,
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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
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
static T2: ObjType = ObjType {
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
    slots: unsafe { F2.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("wdt fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("wdt fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn wdt_ptr(o: Obj) -> *mut MachineWdt {
    obj::as_ptr(o) as *mut MachineWdt
}

static mut WDT_DEFAULT: MachineWdt = MachineWdt {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    timeout_ms: 5000,
};

/// Soft host WDT — accepts timeout/feed calls but does not reset the host.
fn soft_wdt_make_new(_id: Obj, timeout_ms: i32) -> Obj {
    let ty = init_wdt_type();
    unsafe {
        WDT_DEFAULT.timeout_ms = timeout_ms;
        WDT_DEFAULT.base.type_ = ty as *const ObjType;
        obj::from_ptr(&raw const WDT_DEFAULT as *const MachineWdt as *const ())
    }
}

fn wdt_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("id"),
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::new_small_int(0)),
        },
        Arg {
            qst: qstr::from_str("timeout"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(5000),
        },
    ];
    let mut vals = [ArgVal::default(); 2];
    argcheck::parse_all_kw_array(n_args, n_kw, args, allowed.len(), &allowed, &mut vals);
    let id = match vals[0] {
        ArgVal::Obj(o) => o,
        _ => obj::new_small_int(0),
    };
    let timeout_ms = match vals[1] {
        ArgVal::Int(v) => v as i32,
        _ => 5000,
    };
    soft_wdt_make_new(id, timeout_ms)
}

fn wdt_feed(self_in: Obj) -> Obj {
    let _ = unsafe { &*wdt_ptr(self_in) };
    obj::CONST_NONE
}

fn wdt_timeout_ms(self_in: Obj, timeout_in: Obj) -> Obj {
    let timeout_ms = obj::get_int(timeout_in) as i32;
    unsafe {
        (*wdt_ptr(self_in)).timeout_ms = timeout_ms;
    }
    obj::CONST_NONE
}

static mut WDT_SLOTS: [*const (); 2] = [wdt_make_new as MakeNewFn as *const (), core::ptr::null()];
static mut WDT_TYPE: ObjType = ObjType {
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
    slots: unsafe { WDT_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_wdt_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let mut table = vec![MapElem {
            key: obj::new_qstr(qstr::from_str("feed")),
            value: mk1(wdt_feed),
        }];
        if mpconfig::PY_MACHINE_WDT_TIMEOUT_MS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("timeout_ms")),
                value: mk2(wdt_timeout_ms),
            });
        }
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            WDT_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            WDT_TYPE.name = qstr::from_str("WDT");
        }
    });
    unsafe { &WDT_TYPE }
}

/// `machine_wdt_type`
pub fn wdt_type() -> &'static ObjType {
    if !enabled() {
        panic!("WDT disabled");
    }
    init_wdt_type()
}

/// Board-specific `machine_wdt` helpers — enabled when `PY_MACHINE_WDT`.
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_WDT
}
