//! rewrite of extmod/modheapq.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime0::BinaryOp;

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
static T2: ObjType = ObjType {
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
    slots: unsafe { F2.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("heapq fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("heapq fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn get_heap(heap_in: Obj) -> *mut py_rs::objlist::ObjList {
    if !obj::is_exact_type(heap_in, objlist::type_list()) {
        raise::raise(MpRaise::TypeError("heap must be a list"));
    }
    objlist::list_optional_arg(heap_in, 0)
}

fn less(a: Obj, b: Obj) -> bool {
    py_rs::runtime::binary_op_obj(BinaryOp::Less, a, b) == obj::CONST_TRUE
}

unsafe fn item_at(items: *mut Obj, i: usize) -> Obj {
    *items.add(i)
}

unsafe fn set_item_at(items: *mut Obj, i: usize, v: Obj) {
    *items.add(i) = v;
}

fn siftdown(heap: *mut py_rs::objlist::ObjList, start: usize, mut pos: usize) {
    unsafe {
        let item = item_at((*heap).items, pos);
        while pos > start {
            let parent = (pos - 1) >> 1;
            let p = item_at((*heap).items, parent);
            if less(item, p) {
                set_item_at((*heap).items, pos, p);
                pos = parent;
            } else {
                break;
            }
        }
        set_item_at((*heap).items, pos, item);
    }
}

fn siftup(heap: *mut py_rs::objlist::ObjList, mut pos: usize) {
    unsafe {
        let start = pos;
        let end = (*heap).len;
        let item = item_at((*heap).items, pos);
        let mut child = 2 * pos + 1;
        while child < end {
            if child + 1 < end
                && !less(
                    item_at((*heap).items, child),
                    item_at((*heap).items, child + 1),
                )
            {
                child += 1;
            }
            set_item_at((*heap).items, pos, item_at((*heap).items, child));
            pos = child;
            child = 2 * pos + 1;
        }
        set_item_at((*heap).items, pos, item);
        siftdown(heap, start, pos);
    }
}

fn heappush(heap_in: Obj, item: Obj) -> Obj {
    let heap = get_heap(heap_in);
    objlist::list_append(heap_in, item);
    unsafe {
        siftdown(heap, 0, (*heap).len - 1);
    }
    obj::CONST_NONE
}

fn heappop(heap_in: Obj) -> Obj {
    let heap = get_heap(heap_in);
    unsafe {
        if (*heap).len == 0 {
            raise::raise(MpRaise::RuntimeError("empty heap"));
        }
        let item = item_at((*heap).items, 0);
        (*heap).len -= 1;
        set_item_at((*heap).items, 0, item_at((*heap).items, (*heap).len));
        set_item_at((*heap).items, (*heap).len, obj::OBJ_NULL);
        if (*heap).len > 0 {
            siftup(heap, 0);
        }
        item
    }
}

fn heapify(heap_in: Obj) -> Obj {
    let heap = get_heap(heap_in);
    unsafe {
        let mut i = (*heap).len / 2;
        while i > 0 {
            i -= 1;
            siftup(heap, i);
        }
    }
    obj::CONST_NONE
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_HEAPQ {
        return obj::OBJ_NULL;
    }
    let table = [
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("heapq")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("heappush")),
            value: mk2(heappush),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("heappop")),
            value: mk1(heappop),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("heapify")),
            value: mk1(heapify),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("heapq");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("heapq"), module);
    module
}
