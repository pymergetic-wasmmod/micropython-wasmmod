//! rewrite of py/modthread.c
// symmetry: done

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::bc::ModuleContext;
use crate::cstack;
use crate::map::{self, MapElem};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, PrintKind};
use crate::mpstate::{self, ThreadState};
use crate::mpthread::{self, ThreadMutex};
use crate::nlr::{self, NlrBuf};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objint_mpz;
use crate::objmodule;
use crate::objtype;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FUN0: [*const (); 1] = [call0 as *const ()];
static mut FUN1: [*const (); 1] = [call1 as *const ()];
static mut FUNV: [*const (); 1] = [callv as *const ()];

static TYPE_FUN0: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FUN0.as_ptr() },
};
static TYPE_FUN1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FUN1.as_ptr() },
};
static TYPE_FUNVAR: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FUNV.as_ptr() },
};

fn call0(_s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 0, 0, false);
    let self_ = unsafe { &*(obj::as_ptr(_s) as *const ObjFunBuiltin0) };
    (self_.fun)()
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("_thread fun0");
    unsafe {
        (*o).base.type_ = &TYPE_FUN0 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("_thread fun1");
    unsafe {
        (*o).base.type_ = &TYPE_FUN1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("_thread funv");
    unsafe {
        (*o).base.type_ = &TYPE_FUNVAR as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

// --- Lock object ----------------------------------------------------------------

#[repr(C)]
struct ObjThreadLock {
    base: ObjBase,
    mutex: ThreadMutex,
    locked: bool,
}

static mut LOCK_SLOTS: [*const (); 2] = [lock_make_new as *const _, core::ptr::null()];
static mut TYPE_LOCK: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { LOCK_SLOTS.as_ptr() },
};

static LOCK_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn lock_ptr(o: Obj) -> *mut ObjThreadLock {
    obj::as_ptr(o) as *mut ObjThreadLock
}

fn new_thread_lock() -> Obj {
    let o = malloc::new_obj::<ObjThreadLock>().expect("thread lock");
    unsafe {
        (*o).base.type_ = type_thread_lock() as *const ObjType;
        mpthread::mutex_init(&mut (*o).mutex);
        (*o).locked = false;
        obj::from_ptr(o as *const ObjThreadLock as *const ())
    }
}

fn lock_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    crate::argcheck::check_num(n_args, n_kw, 0, 0, false);
    let o = malloc::new_obj::<ObjThreadLock>().expect("thread lock");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        mpthread::mutex_init(&mut (*o).mutex);
        (*o).locked = false;
        obj::from_ptr(o as *const ObjThreadLock as *const ())
    }
}

fn lock_acquire(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &mut *lock_ptr(args[0]) };
    let wait = if n > 1 {
        obj::is_true(args[1])
    } else {
        true
    };
    mpthread::thread_gil_exit();
    let ret = mpthread::mutex_lock(&self_.mutex, wait);
    mpthread::thread_gil_enter();
    if ret == 0 {
        obj::CONST_FALSE
    } else if ret == 1 {
        self_.locked = true;
        obj::CONST_TRUE
    } else {
        raise::raise(MpRaise::OSError(-ret));
    }
}

fn lock_release(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *lock_ptr(self_in) };
    if !self_.locked {
        raise::raise(MpRaise::RuntimeError(""));
    }
    self_.locked = false;
    mpthread::thread_gil_exit();
    mpthread::mutex_unlock(&self_.mutex);
    mpthread::thread_gil_enter();
    obj::CONST_NONE
}

fn lock_locked(self_in: Obj) -> Obj {
    let self_ = unsafe { &*lock_ptr(self_in) };
    obj::new_bool(self_.locked)
}

fn lock_exit(_n: usize, args: &[Obj]) -> Obj {
    lock_release(args[0])
}

fn init_lock_type() -> &'static ObjType {
    LOCK_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("acquire")),
                value: mkv(1, 3, lock_acquire),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("release")),
                value: mk1(lock_release),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("locked")),
                value: mk1(lock_locked),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__enter__")),
                value: mkv(1, 3, lock_acquire),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__exit__")),
                value: mkv(4, 4, lock_exit),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            LOCK_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const _;
            TYPE_LOCK.name = qstr::from_str("lock");
        }
    });
    unsafe { &TYPE_LOCK }
}

pub fn type_thread_lock() -> &'static ObjType {
    init_lock_type()
}

// --- _thread module -------------------------------------------------------------

static THREAD_STACK_SIZE: AtomicUsize = AtomicUsize::new(0);

fn mod_get_ident() -> Obj {
    objint_mpz::new_int_from_uint(mpthread::get_id() as obj::Uint)
}

fn mod_stack_size(n: usize, args: &[Obj]) -> Obj {
    let prev = THREAD_STACK_SIZE.load(Ordering::Relaxed);
    let ret = objint_mpz::new_int_from_uint(prev as obj::Uint);
    if n == 0 {
        THREAD_STACK_SIZE.store(0, Ordering::Relaxed);
    } else {
        THREAD_STACK_SIZE.store(obj::get_int(args[0]) as usize, Ordering::Relaxed);
    }
    ret
}

struct ThreadEntryArgs {
    dict_locals: Obj,
    dict_globals: Obj,
    stack_size: usize,
    fun: Obj,
    n_args: usize,
    n_kw: usize,
    args: Vec<Obj>,
}

fn thread_entry(args: &ThreadEntryArgs) {
    let ts = Box::new(ThreadState::default());
    let ts_ptr = Box::leak(ts);
    mpthread::set_state(ts_ptr);

    let stack_size = if args.stack_size == 0 {
        32 * 1024
    } else {
        args.stack_size
    };
    cstack::init_with_sp_here(stack_size);
    mpstate::thread_init_state(
        Some(args.dict_locals),
        Some(args.dict_globals),
        stack_size,
        mpstate::with_thread(|t| t.stack_top),
    );

    mpthread::thread_gil_enter();
    mpthread::start();

    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || {
        runtime::call_function_n_kw(args.fun, args.n_args, args.n_kw, &args.args)
    }) {
        Ok(_) => {}
        Err(exc) => {
            let exc_obj = Obj(exc);
            let exc_type = obj::from_ptr(obj::get_type(exc_obj) as *const ObjType as *const ());
            let system_exit =
                obj::from_ptr(objexcept::type_system_exit() as *const ObjType as *const ());
            if objtype::is_subclass_fast(exc_type, system_exit) {
                // swallow SystemExit in worker threads
            } else {
                let print = &mpprint::PLAT_PRINT;
                let _ = mpprint::print_str(print, "Unhandled exception in thread started by ");
                obj::print_helper(print, args.fun, PrintKind::Repr);
                let _ = mpprint::print_str(print, "\n");
                objexcept::exception_print(print, exc_obj, PrintKind::Exc);
            }
        }
    }

    mpthread::finish();
    mpthread::thread_gil_exit();
}

extern "C" fn thread_entry_c(arg: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    let args = unsafe { &*(arg as *const ThreadEntryArgs) };
    thread_entry(args);
    core::ptr::null_mut()
}

fn mod_start_new_thread(n: usize, args: &[Obj]) -> Obj {
    let (pos_len, pos_items) = obj::get_array(args[1]);

    let (n_kw, extra_args) = if n == 2 {
        (0usize, Vec::new())
    } else {
        if !obj::is_type(args[2], objdict::type_dict()) {
            raise::raise(MpRaise::TypeError("expecting a dict for keyword args"));
        }
        let dict = unsafe { &*(obj::as_ptr(args[2]) as *const ObjDict) };
        let mut kw_pairs = Vec::new();
        for i in 0..dict.map.alloc {
            if map::slot_is_filled(&dict.map, i) {
                let slot = &dict.map.table[i];
                kw_pairs.push(slot.key);
                kw_pairs.push(slot.value);
            }
        }
        (kw_pairs.len() / 2, kw_pairs)
    };

    let mut call_args = pos_items;
    call_args.extend(extra_args);

    let th_args = Box::new(ThreadEntryArgs {
        dict_locals: mpstate::locals_get(),
        dict_globals: mpstate::globals_get(),
        stack_size: THREAD_STACK_SIZE.load(Ordering::Relaxed),
        fun: args[0],
        n_args: pos_len,
        n_kw,
        args: call_args,
    });
    let th_args_ptr = Box::leak(th_args) as *mut ThreadEntryArgs as *mut core::ffi::c_void;

    let mut stack_size = THREAD_STACK_SIZE.load(Ordering::Relaxed);
    let id = mpthread::create(thread_entry_c, th_args_ptr, &mut stack_size);
    objint_mpz::new_int_from_uint(id as obj::Uint)
}

fn mod_exit() -> Obj {
    raise::raise_obj(objexcept::new_exception(objexcept::type_system_exit()));
}

fn mod_allocate_lock() -> Obj {
    new_thread_lock()
}

static mut MODULE: Option<Obj> = None;

/// Register built-in `_thread` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_THREAD {
        return obj::OBJ_NULL;
    }
    unsafe {
        if let Some(m) = MODULE {
            return m;
        }
    }

    let lock_type = init_lock_type();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("_thread")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("LockType")),
            value: obj::from_ptr(lock_type as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("get_ident")),
            value: mk0(mod_get_ident),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("stack_size")),
            value: mkv(0, 1, mod_stack_size),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("start_new_thread")),
            value: mkv(2, 3, mod_start_new_thread),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("exit")),
            value: mk0(mod_exit),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("allocate_lock")),
            value: mk0(mod_allocate_lock),
        },
    ];

    let ctx = malloc::new_obj::<ModuleContext>().expect("_thread module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("_thread"), module);
    unsafe {
        MODULE = Some(module);
    }
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn thread_module_exports() {
        let _ = gc::init();
        runtime::init();
        let m = init_module();
        assert!(obj::is_obj(m));
        let globals = objmodule::module_get_globals(m);
        let ident = map::lookup(
            unsafe { &mut (*globals).map },
            obj::new_qstr(qstr::from_str("get_ident")),
            map::LookupKind::Lookup,
        )
        .expect("get_ident");
        let id = runtime::call_function_0(ident.value);
        assert_eq!(id, objint_mpz::new_int_from_uint(0));
    }
}
