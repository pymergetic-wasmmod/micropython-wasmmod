//! rewrite of extmod/modbtree.c
//! When `PY_BTREE` enabled: in-memory `BTreeMap` only — no Berkeley DB 1.xx on-disk format / `__bt_*` API.
//! `open()` stream kwargs (`flags`, `cachesize`, `pagesize`, `minkeypage`) are ignored.
// symmetry: done

use std::collections::BTreeMap;

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, LookupKind, Map, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{
    self, BufferInfo, GetIterFn, GetiterIternextCustom, IterNextFn, Obj, ObjBase, ObjIterBuf,
    ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_CUSTOM,
};
use py_rs::objdict;
use py_rs::objexcept;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime0::BinaryOp;
use py_rs::stream::{self, STREAM_OP_IOCTL, STREAM_OP_READ, STREAM_OP_WRITE};

const FLAG_END_KEY_INCL: u8 = 1;
const FLAG_DESC: u8 = 2;
const FLAG_ITER_TYPE_MASK: u8 = 0xc0;
const FLAG_ITER_KEYS: u8 = 0x40;
const FLAG_ITER_VALUES: u8 = 0x80;
const FLAG_ITER_ITEMS: u8 = 0xc0;

const R_FIRST: i32 = 0;
const R_NEXT: i32 = 1;
const R_PREV: i32 = 2;
const R_LAST: i32 = 3;
const R_CURSOR: i32 = 4;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

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
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

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
        if let Some(slot) = map::lookup(&mut kw, key, LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("btree fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("btree fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("btree fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

struct BtreeStore {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

#[repr(C)]
struct ObjBtree {
    base: ObjBase,
    stream: Obj,
    store: *mut BtreeStore,
    start_key: Obj,
    end_key: Obj,
    flags: u8,
    next_flags: u8,
    iter_key: Obj,
}

fn btree_ptr(o: Obj) -> *mut ObjBtree {
    obj::as_ptr(o) as *mut ObjBtree
}

fn check_open(self_: &ObjBtree) {
    if self_.store.is_null() {
        raise::raise(MpRaise::ValueError("database closed"));
    }
}

fn buf_to_vec(obj_in: Obj) -> Vec<u8> {
    let mut info = BufferInfo {
        buf: core::ptr::null_mut(),
        len: 0,
        typecode: 0,
    };
    unsafe {
        obj::get_buffer_raise(obj_in, &mut info, obj::BUFFER_READ);
    }
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

fn cmp_keys(a: &[u8], b: &[u8]) -> i32 {
    match a.cmp(b) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

fn store_ptr(self_: &ObjBtree) -> &BtreeStore {
    unsafe { &*self_.store }
}

fn store_mut(self_: &mut ObjBtree) -> &mut BtreeStore {
    unsafe { &mut *self_.store }
}

fn btree_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*btree_ptr(self_in) };
    mpprint::printf(print, "<btree {:p}>", [VaArg::USize(self_.store as usize)]);
}

fn btree_flush(self_in: Obj) -> Obj {
    let self_ = unsafe { &*btree_ptr(self_in) };
    check_open(self_);
    obj::new_small_int(0)
}

fn btree_close(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *btree_ptr(self_in) };
    if !self_.store.is_null() {
        unsafe {
            drop(Box::from_raw(self_.store));
        }
        self_.store = core::ptr::null_mut();
    }
    obj::new_small_int(0)
}

fn btree_put_call(n: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n, 0, 3, 4, false);
    let self_ = unsafe { &mut *btree_ptr(args[0]) };
    check_open(self_);
    let key = buf_to_vec(args[1]);
    let val = buf_to_vec(args[2]);
    store_mut(self_).map.insert(key, val);
    obj::new_small_int(0)
}

fn btree_get_call(n: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n, 0, 2, 3, false);
    let self_ = unsafe { &*btree_ptr(args[0]) };
    check_open(self_);
    let key = buf_to_vec(args[1]);
    match store_ptr(self_).map.get(&key) {
        Some(val) => objstr::new_bytes(val),
        None => {
            if n > 2 {
                args[2]
            } else {
                obj::CONST_NONE
            }
        }
    }
}

fn btree_seq_pair(
    store: &BtreeStore,
    flags: i32,
    key_in: Option<&[u8]>,
) -> Option<(Vec<u8>, Vec<u8>)> {
    match flags {
        R_FIRST => store.map.iter().next().map(|(k, v)| (k.clone(), v.clone())),
        R_LAST => store
            .map
            .iter()
            .next_back()
            .map(|(k, v)| (k.clone(), v.clone())),
        R_NEXT => {
            let key = key_in?;
            store
                .map
                .iter()
                .find(|(k, _)| k.as_slice() > key)
                .map(|(k, v)| (k.clone(), v.clone()))
        }
        R_PREV => {
            let key = key_in?;
            store
                .map
                .iter()
                .rev()
                .find(|(k, _)| k.as_slice() < key)
                .map(|(k, v)| (k.clone(), v.clone()))
        }
        R_CURSOR => {
            let key = key_in?;
            store.map.get(key).map(|v| (key.to_vec(), v.clone()))
        }
        _ => None,
    }
}

fn btree_seq_call(n: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n, 0, 2, 4, false);
    let self_ = unsafe { &*btree_ptr(args[0]) };
    check_open(self_);
    let flags = obj::small_int_value(args[1]) as i32;
    let key_vec = if n > 2 {
        Some(buf_to_vec(args[2]))
    } else {
        None
    };
    match btree_seq_pair(store_ptr(self_), flags, key_vec.as_deref()) {
        Some((key, val)) => {
            objtuple::new_tuple(2, Some(&[objstr::new_bytes(&key), objstr::new_bytes(&val)]))
        }
        None => obj::CONST_NONE,
    }
}

fn btree_init_iter(n: usize, args: &[Obj], iter_type: u8) -> Obj {
    let self_ = unsafe { &mut *btree_ptr(args[0]) };
    self_.next_flags = iter_type;
    self_.start_key = obj::CONST_NONE;
    self_.end_key = obj::CONST_NONE;
    self_.iter_key = obj::OBJ_NULL;
    if n > 1 {
        self_.start_key = args[1];
        if n > 2 {
            self_.end_key = args[2];
            if n > 3 {
                self_.next_flags = iter_type | obj::small_int_value(args[3]) as u8;
            }
        }
    }
    args[0]
}

fn btree_keys_call(n: usize, args: &[Obj]) -> Obj {
    btree_init_iter(n, args, FLAG_ITER_KEYS)
}

fn btree_values_call(n: usize, args: &[Obj]) -> Obj {
    btree_init_iter(n, args, FLAG_ITER_VALUES)
}

fn btree_items_call(n: usize, args: &[Obj]) -> Obj {
    btree_init_iter(n, args, FLAG_ITER_ITEMS)
}

fn iter_yield(self_: &ObjBtree, key: &[u8], val: &[u8]) -> Obj {
    match self_.flags & FLAG_ITER_TYPE_MASK {
        FLAG_ITER_KEYS => objstr::new_bytes(key),
        FLAG_ITER_VALUES => objstr::new_bytes(val),
        _ => objtuple::new_tuple(2, Some(&[objstr::new_bytes(key), objstr::new_bytes(val)])),
    }
}

fn past_end(self_: &mut ObjBtree, key: &[u8]) -> bool {
    if self_.end_key == obj::CONST_NONE {
        return false;
    }
    let end = buf_to_vec(self_.end_key);
    let mut cmp = cmp_keys(key, &end);
    if (self_.flags & FLAG_DESC) != 0 {
        cmp = -cmp;
    }
    if (self_.flags & FLAG_END_KEY_INCL) != 0 {
        cmp -= 1;
    }
    if cmp >= 0 {
        self_.end_key = obj::OBJ_NULL;
        return true;
    }
    false
}

fn btree_getiter(self_in: Obj, _iter_buf: *mut ObjIterBuf) -> Obj {
    let self_ = unsafe { &mut *btree_ptr(self_in) };
    if self_.next_flags != 0 {
        self_.flags = self_.next_flags;
        self_.next_flags = 0;
    } else {
        self_.flags = FLAG_ITER_KEYS;
        self_.start_key = obj::CONST_NONE;
        self_.end_key = obj::CONST_NONE;
    }
    self_.iter_key = obj::OBJ_NULL;
    self_in
}

fn btree_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *btree_ptr(self_in) };
    check_open(self_);
    let desc = (self_.flags & FLAG_DESC) != 0;
    let pair = if self_.start_key != obj::OBJ_NULL {
        let flags = if self_.start_key == obj::CONST_NONE {
            if desc {
                R_LAST
            } else {
                R_FIRST
            }
        } else {
            R_CURSOR
        };
        let key_vec = if self_.start_key == obj::CONST_NONE {
            None
        } else {
            Some(buf_to_vec(self_.start_key))
        };
        self_.start_key = obj::OBJ_NULL;
        btree_seq_pair(store_ptr(self_), flags, key_vec.as_deref())
    } else {
        let key_ref = if self_.iter_key == obj::OBJ_NULL {
            None
        } else {
            Some(buf_to_vec(self_.iter_key))
        };
        let flags = if desc { R_PREV } else { R_NEXT };
        btree_seq_pair(store_ptr(self_), flags, key_ref.as_deref())
    };
    let Some((key, val)) = pair else {
        return obj::OBJ_STOP_ITERATION;
    };
    if past_end(self_, &key) {
        return obj::OBJ_STOP_ITERATION;
    }
    self_.iter_key = objstr::new_bytes(&key);
    iter_yield(self_, &key, &val)
}

fn raise_key_error() -> ! {
    raise::raise_obj(objexcept::new_exception(objexcept::type_key_error()));
}

fn btree_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    let self_ = unsafe { &mut *btree_ptr(self_in) };
    check_open(self_);
    let key = buf_to_vec(index);
    if value == obj::OBJ_NULL {
        match store_mut(self_).map.remove(&key) {
            Some(_) => obj::CONST_NONE,
            None => raise_key_error(),
        }
    } else if value == obj::OBJ_SENTINEL {
        match store_ptr(self_).map.get(&key) {
            Some(val) => objstr::new_bytes(val),
            None => raise_key_error(),
        }
    } else {
        let val = buf_to_vec(value);
        store_mut(self_).map.insert(key, val);
        obj::CONST_NONE
    }
}

fn btree_binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    if op != BinaryOp::Contains {
        return obj::OBJ_NULL;
    }
    let self_ = unsafe { &*btree_ptr(lhs) };
    check_open(self_);
    let key = buf_to_vec(rhs);
    obj::new_bool(store_ptr(self_).map.contains_key(&key))
}

static BTREE_ITER: GetiterIternextCustom = GetiterIternextCustom {
    getiter: btree_getiter as GetIterFn,
    iternext: btree_iternext as IterNextFn,
};

static mut BTREE_SLOTS: [*const (); 7] = [
    core::ptr::null(),
    btree_print as *const (),
    &BTREE_ITER as *const GetiterIternextCustom as *const (),
    btree_subscr as *const (),
    btree_binary_op as *const (),
    core::ptr::null(),
    core::ptr::null(),
];

static mut TYPE_BTREE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_CUSTOM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 5,
    slot_index_attr: 0,
    slot_index_subscr: 4,
    slot_index_iter: 3,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 7,
    slots: unsafe { BTREE_SLOTS.as_ptr() },
};

static BTREE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_btree() -> &'static ObjType {
    BTREE_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: mk1(btree_close),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("flush")),
                value: mk1(btree_flush),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("get")),
                value: mkv(2, 3, btree_get_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("put")),
                value: mkv(3, 4, btree_put_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("seq")),
                value: mkv(2, 4, btree_seq_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("keys")),
                value: mkv(1, 4, btree_keys_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("values")),
                value: mkv(1, 4, btree_values_call),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("items")),
                value: mkv(1, 4, btree_items_call),
            },
        ];
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
            BTREE_SLOTS[6] = objdict::dict_ptr(dict) as *const ();
            TYPE_BTREE.name = qstr::from_str("btree");
        }
    });
    unsafe { &TYPE_BTREE }
}

fn btree_new(stream: Obj) -> Obj {
    let store = Box::into_raw(Box::new(BtreeStore {
        map: BTreeMap::new(),
    }));
    let o = malloc::new_obj::<ObjBtree>().expect("btree");
    unsafe {
        (*o).base.type_ = type_btree();
        (*o).stream = stream;
        (*o).store = store;
        (*o).start_key = obj::CONST_NONE;
        (*o).end_key = obj::CONST_NONE;
        (*o).next_flags = 0;
        (*o).flags = 0;
        (*o).iter_key = obj::OBJ_NULL;
        obj::from_ptr(o as *const ObjBtree as *const ())
    }
}

fn mod_btree_open(stream: Obj) -> Obj {
    stream::get_stream_raise(stream, STREAM_OP_READ | STREAM_OP_WRITE | STREAM_OP_IOCTL);
    btree_new(stream)
}

fn mod_btree_open_kw(_n: usize, pos: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("flags"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("cachesize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("pagesize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("minkeypage"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(0, &[], &mut kw_copy, allowed.len(), &allowed, &mut vals);
    let _ = vals;
    mod_btree_open(pos[0])
}

/// Register built-in `btree` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_BTREE {
        return obj::OBJ_NULL;
    }
    type_btree();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("btree")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("open")),
            value: mk_kw(1, mod_btree_open_kw),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("INCL")),
            value: obj::new_small_int(FLAG_END_KEY_INCL as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("DESC")),
            value: obj::new_small_int(FLAG_DESC as isize),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("btree module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("btree"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_store() -> BtreeStore {
        let mut map = BTreeMap::new();
        map.insert(b"a".to_vec(), b"1".to_vec());
        map.insert(b"b".to_vec(), b"2".to_vec());
        map.insert(b"c".to_vec(), b"3".to_vec());
        BtreeStore { map }
    }

    #[test]
    fn btree_seq_first_last_and_cursor() {
        let store = sample_store();
        let first = btree_seq_pair(&store, R_FIRST, None).unwrap();
        assert_eq!(first.0, b"a");
        let last = btree_seq_pair(&store, R_LAST, None).unwrap();
        assert_eq!(last.0, b"c");
        let cursor = btree_seq_pair(&store, R_CURSOR, Some(b"b")).unwrap();
        assert_eq!(cursor.1, b"2");
        let next = btree_seq_pair(&store, R_NEXT, Some(b"b")).unwrap();
        assert_eq!(next.0, b"c");
        let prev = btree_seq_pair(&store, R_PREV, Some(b"b")).unwrap();
        assert_eq!(prev.0, b"a");
    }

    #[test]
    fn cmp_keys_orders_bytes() {
        assert_eq!(cmp_keys(b"a", b"b"), -1);
        assert_eq!(cmp_keys(b"x", b"x"), 0);
        assert_eq!(cmp_keys(b"z", b"y"), 1);
    }
}
