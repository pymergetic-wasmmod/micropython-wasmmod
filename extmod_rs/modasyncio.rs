//! rewrite of extmod/modasyncio.c
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mphal;
use py_rs::mpconfig;
use py_rs::obj::{
    self, GetIterFn, GetiterIternextCustom, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType,
    TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_CUSTOM,
};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objtype;
use py_rs::pairheap::{self, PairHeap, PairHeapLt};
use py_rs::objexcept;
use py_rs::objstr;
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;

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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];

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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("_asyncio fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("_asyncio fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

/// Core asyncio context (`cur_task`, `_task_queue`, `CancelledError`).
static mut MP_ASYNCIO_CONTEXT: Obj = obj::OBJ_NULL;

pub fn asyncio_context() -> Obj {
    unsafe { MP_ASYNCIO_CONTEXT }
}

fn ticks() -> Obj {
    obj::new_small_int((mphal::ticks_ms() as u64 & (mpconfig::PY_TIME_TICKS_PERIOD - 1)) as isize)
}

fn ticks_diff(t1_in: Obj, t0_in: Obj) -> isize {
    let t0 = obj::small_int_value(t0_in) as u64;
    let t1 = obj::small_int_value(t1_in) as u64;
    let period = mpconfig::PY_TIME_TICKS_PERIOD;
    let half = (period / 2) as i64;
    ((((t1.wrapping_sub(t0) + half as u64) & (period - 1)) as i64) - half) as isize
}

fn task_ptr(o: Obj) -> *mut ObjTask {
    obj::as_ptr(o) as *mut ObjTask
}

fn task_queue_ptr(o: Obj) -> *mut ObjTaskQueue {
    obj::as_ptr(o) as *mut ObjTaskQueue
}

fn task_lt(n1: *mut PairHeap, n2: *mut PairHeap) -> bool {
    let t1 = n1 as *mut ObjTask;
    let t2 = n2 as *mut ObjTask;
    ticks_diff(unsafe { (*t1).ph_key }, unsafe { (*t2).ph_key }) < 0
}

const TASK_LT: PairHeapLt = task_lt;

fn task_is_done(task: &ObjTask) -> bool {
    task.state == obj::CONST_NONE || task.state == obj::CONST_FALSE
}

#[repr(C)]
pub struct ObjTask {
    pairheap: PairHeap,
    coro: Obj,
    data: Obj,
    state: Obj,
    ph_key: Obj,
}

#[repr(C)]
pub struct ObjTaskQueue {
    base: ObjBase,
    heap: *mut ObjTask,
}

fn task_queue_peek(self_in: Obj) -> Obj {
    let self_ = unsafe { &*task_queue_ptr(self_in) };
    if self_.heap.is_null() {
        obj::CONST_NONE
    } else {
        obj::from_ptr(self_.heap as *const ObjTask as *const ())
    }
}

fn task_queue_push(n_args: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &mut *task_queue_ptr(args[0]) };
    let task = unsafe { &mut *task_ptr(args[1]) };
    task.data = obj::CONST_NONE;
    if n_args == 2 {
        task.ph_key = ticks();
    } else {
        debug_assert!(obj::is_small_int(args[2]));
        task.ph_key = args[2];
    }
    unsafe {
        self_.heap = pairheap::push(
            TASK_LT,
            self_.heap as *mut PairHeap,
            &mut task.pairheap as *mut PairHeap,
        ) as *mut ObjTask;
    }
    obj::CONST_NONE
}

fn task_queue_pop(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *task_queue_ptr(self_in) };
    let head = unsafe {
        pairheap::peek(TASK_LT, self_.heap as *mut PairHeap) as *mut ObjTask
    };
    if head.is_null() {
        raise::raise_obj(objexcept::new_exception_args(
            objexcept::type_index_error(),
            1,
            &[objstr::new_str(b"empty heap")],
        ));
    }
    unsafe {
        self_.heap = pairheap::pop(TASK_LT, self_.heap as *mut PairHeap) as *mut ObjTask;
    }
    obj::from_ptr(head as *const ObjTask as *const ())
}

fn task_queue_remove(self_in: Obj, task_in: Obj) -> Obj {
    let self_ = unsafe { &mut *task_queue_ptr(self_in) };
    let task = unsafe { &mut *task_ptr(task_in) };
    unsafe {
        self_.heap = pairheap::delete(
            TASK_LT,
            self_.heap as *mut PairHeap,
            &mut task.pairheap as *mut PairHeap,
        ) as *mut ObjTask;
    }
    obj::CONST_NONE
}

fn task_queue_make_new(_type_: &ObjType, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    let max_kw = if mpconfig::PY_ASYNCIO_TASK_QUEUE_PUSH_CALLBACK {
        1
    } else {
        0
    };
    argcheck::check_num(n_args, n_kw, 0, max_kw, false);
    let o = malloc::new_obj::<ObjTaskQueue>().expect("TaskQueue");
    unsafe {
        (*o).base.type_ = type_task_queue();
        (*o).heap = std::ptr::null_mut();
    }
    obj::from_ptr(o as *const ObjTaskQueue as *const ())
}

fn task_done(self_in: Obj) -> Obj {
    let self_ = unsafe { &*task_ptr(self_in) };
    obj::new_bool(task_is_done(self_))
}

fn task_cancel(self_in: Obj) -> Obj {
    let mut self_ = unsafe { &mut *task_ptr(self_in) };
    if task_is_done(self_) {
        return obj::CONST_FALSE;
    }
    let ctx = asyncio_context();
    let cur_task = objdict::dict_get(ctx, obj::new_qstr(qstr::from_str("cur_task")));
    if self_in == cur_task {
        raise::raise(MpRaise::RuntimeError("can't cancel self"));
    }
    while objtype::is_subclass_fast(
        obj::from_ptr(obj::get_type(self_.data) as *const ObjType as *const ()),
        obj::from_ptr(type_task() as *const ObjType as *const ()),
    ) {
        self_ = unsafe { &mut *task_ptr(self_.data) };
    }
    let task_queue = objdict::dict_get(ctx, obj::new_qstr(qstr::from_str("_task_queue")));
    let mut dest = [obj::OBJ_NULL; 3];
    runtime::load_method_maybe(self_.data, qstr::from_str("remove"), &mut dest);
    if dest[0] != obj::OBJ_NULL {
        dest[2] = obj::from_ptr(self_ as *const ObjTask as *const ());
        runtime::call_method_n_kw(1, 0, &dest);
        dest[0] = task_queue;
        dest[1] = obj::from_ptr(self_ as *const ObjTask as *const ());
        task_queue_push(2, &dest);
    } else if ticks_diff(self_.ph_key, ticks()) > 0 {
        task_queue_remove(task_queue, obj::from_ptr(self_ as *const ObjTask as *const ()));
        dest[0] = task_queue;
        dest[1] = obj::from_ptr(self_ as *const ObjTask as *const ());
        task_queue_push(2, &dest);
    }
    self_.data = objdict::dict_get(ctx, obj::new_qstr(qstr::from_str("CancelledError")));
    obj::CONST_TRUE
}

fn task_make_new(_type_: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 2, false);
    let o = malloc::new_obj::<ObjTask>().expect("Task");
    unsafe {
        (*o).pairheap.base.type_ = type_task();
        pairheap::init_node(TASK_LT, &mut (*o).pairheap as *mut PairHeap);
        (*o).coro = args[0];
        (*o).data = obj::CONST_NONE;
        (*o).state = obj::CONST_TRUE;
        (*o).ph_key = obj::new_small_int(0);
        if n_args == 2 {
            MP_ASYNCIO_CONTEXT = args[1];
        }
    }
    obj::from_ptr(o as *const ObjTask as *const ())
}

fn task_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ = unsafe { &mut *task_ptr(self_in) };
    if dest[0] == obj::OBJ_NULL {
        if attr == qstr::from_str("coro") {
            dest[0] = self_.coro;
        } else if attr == qstr::from_str("data") {
            dest[0] = self_.data;
        } else if attr == qstr::from_str("state") {
            dest[0] = self_.state;
        } else if attr == qstr::from_str("done") {
            dest[0] = unsafe { TASK_DONE_FUN };
            dest[1] = self_in;
        } else if attr == qstr::from_str("cancel") {
            dest[0] = unsafe { TASK_CANCEL_FUN };
            dest[1] = self_in;
        } else if attr == qstr::from_str("ph_key") {
            dest[0] = self_.ph_key;
        }
    } else if dest[1] != obj::OBJ_NULL {
        if attr == qstr::from_str("data") {
            self_.data = dest[1];
            dest[0] = obj::OBJ_NULL;
        } else if attr == qstr::from_str("state") {
            self_.state = dest[1];
            dest[0] = obj::OBJ_NULL;
        }
    }
}

fn task_getiter(self_in: Obj, _iter_buf: *mut ObjIterBuf) -> Obj {
    let self_ = unsafe { &mut *task_ptr(self_in) };
    if task_is_done(self_) {
        self_.state = obj::CONST_FALSE;
    } else if self_.state == obj::CONST_TRUE {
        self_.state = task_queue_make_new(type_task_queue(), 0, 0, &[]);
    } else if !obj::is_exact_type(self_.state, type_task_queue()) {
        raise::raise(MpRaise::RuntimeError("can't wait"));
    }
    self_in
}

fn task_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *task_ptr(self_in) };
    if task_is_done(self_) {
        raise::raise_obj(self_.data);
    }
    let ctx = asyncio_context();
    let cur_task = objdict::dict_get(ctx, obj::new_qstr(qstr::from_str("cur_task")));
    let args = [self_.state, cur_task];
    task_queue_push(2, &args);
    unsafe {
        (*task_ptr(cur_task)).data = self_in;
    }
    obj::CONST_NONE
}

static mut TASK_QUEUE_SLOTS: [*const (); 3] = [
    task_queue_make_new as *const (),
    core::ptr::null(),
    core::ptr::null(),
];

static mut TYPE_TASK_QUEUE: ObjType = ObjType {
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
    slot_index_locals_dict: 3,
    slots: unsafe { TASK_QUEUE_SLOTS.as_ptr() },
};

static TASK_ITER: GetiterIternextCustom = GetiterIternextCustom {
    getiter: task_getiter as GetIterFn,
    iternext: task_iternext as IterNextFn,
};

static mut TASK_SLOTS: [*const (); 4] = [
    task_make_new as *const (),
    task_attr as *const (),
    &TASK_ITER as *const GetiterIternextCustom as *const (),
    core::ptr::null(),
];

static mut TYPE_TASK: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_CUSTOM,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 2,
    slot_index_subscr: 0,
    slot_index_iter: 3,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { TASK_SLOTS.as_ptr() },
};

static TASK_QUEUE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static TASK_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

static mut TASK_DONE_FUN: Obj = obj::OBJ_NULL;
static mut TASK_CANCEL_FUN: Obj = obj::OBJ_NULL;

pub fn type_task_queue() -> &'static ObjType {
    TASK_QUEUE_INIT.get_or_init(|| {
        let table = [
            MapElem {
                key: obj::new_qstr(qstr::from_str("peek")),
                value: mk1(task_queue_peek),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("push")),
                value: mkv(2, 3, task_queue_push),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("pop")),
                value: mk1(task_queue_pop),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("remove")),
                value: mkv(2, 2, task_queue_remove_call),
            },
        ];
        let dict = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
            TASK_QUEUE_SLOTS[2] = objdict::dict_ptr(dict) as *const ();
            TYPE_TASK_QUEUE.name = qstr::from_str("TaskQueue");
        }
    });
    unsafe { &TYPE_TASK_QUEUE }
}

fn task_queue_remove_call(n: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n, 0, 2, 2, false);
    task_queue_remove(args[0], args[1])
}

pub fn type_task() -> &'static ObjType {
    TASK_INIT.get_or_init(|| {
        unsafe {
            TASK_DONE_FUN = mk1(task_done);
            TASK_CANCEL_FUN = mk1(task_cancel);
            TYPE_TASK.name = qstr::from_str("Task");
        }
    });
    unsafe { &TYPE_TASK }
}

/// Register built-in `_asyncio` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_ASYNCIO {
        return obj::OBJ_NULL;
    }
    type_task_queue();
    type_task();
    let table = [
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("_asyncio")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("TaskQueue")),
            value: obj::from_ptr(type_task_queue() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("Task")),
            value: obj::from_ptr(type_task() as *const ObjType as *const ()),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("_asyncio module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table.to_vec());
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("_asyncio"), module);
    module
}
