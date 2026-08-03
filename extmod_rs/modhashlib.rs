//! rewrite of extmod/modhashlib.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use sha2::{Digest, Sha256};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

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
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("hashlib fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("hashlib fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

#[repr(C)]
struct ObjHashSha256 {
    base: ObjBase,
    final_: bool,
    hasher: Sha256,
}

fn hash_ptr(o: Obj) -> *mut ObjHashSha256 {
    obj::as_ptr(o) as *mut ObjHashSha256
}

fn ensure_not_final(self_: &ObjHashSha256) {
    if self_.final_ {
        raise::raise(MpRaise::ValueError("hash is final"));
    }
}

fn get_buf(o: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    obj::get_buffer_raise(o, &mut info, obj::BUFFER_READ);
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

fn sha256_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjHashSha256>().expect("sha256");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).final_ = false;
        (*o).hasher = Sha256::new();
        if n_args == 1 {
            let data = get_buf(args[0]);
            (*o).hasher.update(&data);
        }
        obj::from_ptr(o as *const ObjHashSha256 as *const ())
    }
}

fn sha256_update(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *hash_ptr(self_in) };
    ensure_not_final(self_);
    self_.hasher.update(&get_buf(arg));
    obj::CONST_NONE
}

fn sha256_digest(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *hash_ptr(self_in) };
    ensure_not_final(self_);
    self_.final_ = true;
    let out = self_.hasher.clone().finalize();
    objstr::new_bytes(&out)
}

static mut SHA256_SLOTS: [*const (); 2] = [sha256_make_new as *const (), core::ptr::null()];
static mut TYPE_SHA256: ObjType = ObjType {
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
    slots: unsafe { SHA256_SLOTS.as_ptr() },
};

static SHA256_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_sha256_type() -> &'static ObjType {
    SHA256_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("update")),
                value: mk2(sha256_update),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("digest")),
                value: mk1(sha256_digest),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            SHA256_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_SHA256.name = qstr::from_str("sha256");
        }
    });
    unsafe { &TYPE_SHA256 }
}

/// Register built-in `hashlib` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_HASHLIB {
        return obj::OBJ_NULL;
    }
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("hashlib")),
    }];
    if mpconfig::PY_HASHLIB_SHA256 {
        let ty = init_sha256_type();
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("sha256")),
            value: obj::from_ptr(ty as *const ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("hashlib module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("hashlib"), module);
    module
}
