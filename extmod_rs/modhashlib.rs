//! rewrite of extmod/modhashlib.c
// symmetry: done

use md5::{Digest as Md5Digest, Md5};
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha256};

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

fn ensure_not_final(final_: bool) {
    if final_ {
        raise::raise(MpRaise::ValueError("hash is final"));
    }
}

fn get_buf(o: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    obj::get_buffer_raise(o, &mut info, obj::BUFFER_READ);
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

#[repr(C)]
struct ObjHashSha256 {
    base: ObjBase,
    final_: bool,
    hasher: Sha256,
}
#[repr(C)]
struct ObjHashSha1 {
    base: ObjBase,
    final_: bool,
    hasher: Sha1,
}
#[repr(C)]
struct ObjHashMd5 {
    base: ObjBase,
    final_: bool,
    hasher: Md5,
}

fn sha256_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjHashSha256>().expect("sha256");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).final_ = false;
        (*o).hasher = Sha256::new();
        if n_args == 1 {
            Sha2Digest::update(&mut (*o).hasher, &get_buf(args[0]));
        }
        obj::from_ptr(o as *const ObjHashSha256 as *const ())
    }
}
fn sha256_update(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashSha256) };
    ensure_not_final(self_.final_);
    Sha2Digest::update(&mut self_.hasher, &get_buf(arg));
    obj::CONST_NONE
}
fn sha256_digest(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashSha256) };
    ensure_not_final(self_.final_);
    self_.final_ = true;
    let out = Sha2Digest::finalize(self_.hasher.clone());
    objstr::new_bytes(&out)
}

fn sha1_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjHashSha1>().expect("sha1");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).final_ = false;
        (*o).hasher = Sha1::new();
        if n_args == 1 {
            Sha1Digest::update(&mut (*o).hasher, &get_buf(args[0]));
        }
        obj::from_ptr(o as *const ObjHashSha1 as *const ())
    }
}
fn sha1_update(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashSha1) };
    ensure_not_final(self_.final_);
    Sha1Digest::update(&mut self_.hasher, &get_buf(arg));
    obj::CONST_NONE
}
fn sha1_digest(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashSha1) };
    ensure_not_final(self_.final_);
    self_.final_ = true;
    let out = Sha1Digest::finalize(self_.hasher.clone());
    objstr::new_bytes(&out)
}

fn md5_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjHashMd5>().expect("md5");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).final_ = false;
        (*o).hasher = Md5::new();
        if n_args == 1 {
            Md5Digest::update(&mut (*o).hasher, &get_buf(args[0]));
        }
        obj::from_ptr(o as *const ObjHashMd5 as *const ())
    }
}
fn md5_update(self_in: Obj, arg: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashMd5) };
    ensure_not_final(self_.final_);
    Md5Digest::update(&mut self_.hasher, &get_buf(arg));
    obj::CONST_NONE
}
fn md5_digest(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjHashMd5) };
    ensure_not_final(self_.final_);
    self_.final_ = true;
    let out = Md5Digest::finalize(self_.hasher.clone());
    objstr::new_bytes(&out)
}

fn init_hash_type(
    name: &str,
    make_new: fn(&ObjType, usize, usize, &[Obj]) -> Obj,
    update: BuiltinFn2,
    digest: BuiltinFn1,
    slots: &mut [*const (); 2],
    ty: &mut ObjType,
) {
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("update")),
            value: mk2(update),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("digest")),
            value: mk1(digest),
        },
    ];
    let ptr =
        obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
    unsafe {
        map::init_fixed_table(&mut (*ptr).map, table);
        slots[0] = make_new as *const ();
        slots[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        ty.name = qstr::from_str(name);
        ty.slot_index_make_new = 1;
        ty.slot_index_locals_dict = 2;
        ty.slots = slots.as_ptr();
    }
}

static mut SHA256_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_SHA256: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
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
    slot_index_locals_dict: 0,
    slots: unsafe { SHA256_SLOTS.as_ptr() },
};
static mut SHA1_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_SHA1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
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
    slot_index_locals_dict: 0,
    slots: unsafe { SHA1_SLOTS.as_ptr() },
};
static mut MD5_SLOTS: [*const (); 2] = [core::ptr::null(); 2];
static mut TYPE_MD5: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
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
    slot_index_locals_dict: 0,
    slots: unsafe { MD5_SLOTS.as_ptr() },
};

static HASHLIB_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    HASHLIB_INIT.get_or_init(|| unsafe {
        if mpconfig::PY_HASHLIB_SHA256 {
            init_hash_type(
                "sha256",
                sha256_make_new,
                sha256_update,
                sha256_digest,
                &mut SHA256_SLOTS,
                &mut TYPE_SHA256,
            );
        }
        if mpconfig::PY_HASHLIB_SHA1 {
            init_hash_type(
                "sha1",
                sha1_make_new,
                sha1_update,
                sha1_digest,
                &mut SHA1_SLOTS,
                &mut TYPE_SHA1,
            );
        }
        if mpconfig::PY_HASHLIB_MD5 {
            init_hash_type(
                "md5",
                md5_make_new,
                md5_update,
                md5_digest,
                &mut MD5_SLOTS,
                &mut TYPE_MD5,
            );
        }
    });
}

/// Register built-in `hashlib` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_HASHLIB {
        return obj::OBJ_NULL;
    }
    init_types();
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("hashlib")),
    }];
    unsafe {
        if mpconfig::PY_HASHLIB_SHA256 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("sha256")),
                value: obj::from_ptr(&raw const TYPE_SHA256 as *const ()),
            });
        }
        if mpconfig::PY_HASHLIB_SHA1 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("sha1")),
                value: obj::from_ptr(&raw const TYPE_SHA1 as *const ()),
            });
        }
        if mpconfig::PY_HASHLIB_MD5 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("md5")),
                value: obj::from_ptr(&raw const TYPE_MD5 as *const ()),
            });
        }
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
