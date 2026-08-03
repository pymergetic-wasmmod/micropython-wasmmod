//! rewrite of extmod/modselect.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::map::{self, LookupKind, Map, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_ITERNEXT};
use py_rs::objdict::{self, ObjDict};
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{self, StreamIoctlFn, STREAM_GET_FILENO, STREAM_POLL};

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
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
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
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
static T3: ObjType = ObjType {
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
    slots: unsafe { F3.as_ptr() },
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

fn call0(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("select fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("select fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("select fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("select fn3");
    unsafe {
        (*o).base.type_ = &T3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("select fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

struct PollEntry {
    obj: Obj,
    events: u32,
    revents: u32,
    fd: i32,
    ioctl: Option<StreamIoctlFn>,
}

#[repr(C)]
struct ObjPoll {
    base: ObjBase,
    entries: Vec<PollEntry>,
    iter_cnt: i32,
    iter_idx: usize,
    ret_tuple: Obj,
}

fn poll_ptr(o: Obj) -> *mut ObjPoll {
    obj::as_ptr(o) as *mut ObjPoll
}

fn stream_fd(obj_in: Obj) -> (i32, Option<StreamIoctlFn>) {
    if obj::is_small_int(obj_in) {
        let fd = obj::small_int_value(obj_in) as i32;
        if fd < 0 {
            raise::raise(MpRaise::ValueError(""));
        }
        return (fd, None);
    }
    let stream_p = stream::get_stream_raise(obj_in, stream::STREAM_OP_IOCTL);
    let ioctl = stream_p.ioctl;
    if let Some(ioctl_fn) = ioctl {
        let mut err = 0;
        let res = ioctl_fn(obj_in, STREAM_GET_FILENO, 0, &mut err);
        if res != stream::STREAM_ERROR {
            return (res as i32, Some(ioctl_fn));
        }
    }
    (-1, ioctl)
}

fn poll_register(self_in: Obj, obj_in: Obj) -> Obj {
    let n = if obj::is_exact_type(obj_in, objlist::type_list()) || obj::is_exact_type(obj_in, objtuple::type_tuple()) {
        let len = obj::get_int(obj::len(obj_in)) as usize;
        let mut i = 0;
        while i < len {
            let item = obj::subscr(obj_in, obj::new_small_int(i as isize), obj::OBJ_SENTINEL);
            poll_register(self_in, item);
            i += 1;
        }
        return obj::CONST_NONE;
    } else {
        1
    };
    let _ = n;
    let events = stream::STREAM_POLL_RD | stream::STREAM_POLL_WR;
    poll_modify(self_in, obj_in, obj::new_small_int(events as isize));
    obj::CONST_NONE
}

fn poll_unregister(self_in: Obj, obj_in: Obj) -> Obj {
    let self_ = unsafe { &mut *poll_ptr(self_in) };
    let key = obj::id(obj_in);
    self_.entries.retain(|e| obj::id(e.obj) != key);
    obj::CONST_NONE
}

fn poll_modify(self_in: Obj, obj_in: Obj, eventmask: Obj) -> Obj {
    let events = obj::get_int(eventmask) as u32;
    let (fd, ioctl) = stream_fd(obj_in);
    let self_ = unsafe { &mut *poll_ptr(self_in) };
    let key = obj::id(obj_in);
    if let Some(entry) = self_.entries.iter_mut().find(|e| obj::id(e.obj) == key) {
        entry.events = events;
        entry.revents = 0;
        entry.fd = fd;
        entry.ioctl = ioctl;
    } else {
        self_.entries.push(PollEntry {
            obj: obj_in,
            events,
            revents: 0,
            fd,
            ioctl,
        });
    }
    obj::CONST_NONE
}

fn poll_once(entries: &mut [PollEntry]) -> usize {
    let mut fds: Vec<libc::pollfd> = Vec::new();
    let mut fd_map: Vec<usize> = Vec::new();
    for (i, e) in entries.iter_mut().enumerate() {
        e.revents = 0;
        if e.fd >= 0 {
            fds.push(libc::pollfd {
                fd: e.fd,
                events: e.events as i16,
                revents: 0,
            });
            fd_map.push(i);
        }
    }
    if !fds.is_empty() {
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 0) };
        if ret < 0 {
            let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            raise::raise(MpRaise::OSError(err));
        }
        for (j, pfd) in fds.iter().enumerate() {
            if pfd.revents != 0 {
                entries[fd_map[j]].revents = pfd.revents as u32;
            }
        }
    }
    let mut n_ready = 0;
    for e in entries.iter_mut() {
        if e.fd < 0 {
            if let Some(ioctl) = e.ioctl {
                let mut err = 0;
                let ret = ioctl(e.obj, STREAM_POLL, e.events as usize, &mut err);
                if ret == stream::STREAM_ERROR as usize {
                    raise::raise(MpRaise::OSError(err));
                }
                e.revents = ret as u32;
            }
        }
        if e.revents != 0 {
            n_ready += 1;
        }
    }
    n_ready
}

fn poll_poll(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &mut *poll_ptr(args[0]) };
    let timeout = if n > 1 {
        obj::get_int(args[1]) as u32
    } else {
        u32::MAX
    };
    let start = mphal::ticks_ms();
    loop {
        let n_ready = poll_once(&mut self_.entries);
        if n_ready > 0 || timeout == u32::MAX {
            break;
        }
        let elapsed = mphal::ticks_ms().wrapping_sub(start);
        if elapsed >= timeout as usize {
            break;
        }
        mphal::delay_ms(1);
    }
    let mut ready = Vec::new();
    for e in &self_.entries {
        if e.revents != 0 {
            ready.push((e.obj, obj::new_small_int(e.revents as isize)));
        }
    }
    let items: Vec<Obj> = ready
        .into_iter()
        .map(|(o, ev)| objtuple::new_tuple(2, Some(&[o, ev])))
        .collect();
    objlist::new_list(items.len(), Some(&items))
}

fn poll_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *poll_ptr(self_in) };
    if self_.iter_cnt == 0 {
        return obj::OBJ_STOP_ITERATION;
    }
    self_.iter_cnt -= 1;
    while self_.iter_idx < self_.entries.len() {
        let idx = self_.iter_idx;
        self_.iter_idx += 1;
        let e = &self_.entries[idx];
        if e.revents != 0 {
            return objtuple::new_tuple(2, Some(&[e.obj, obj::new_small_int(e.revents as isize)]));
        }
    }
    self_.iter_cnt = 0;
    obj::OBJ_STOP_ITERATION
}

fn poll_ipoll(n: usize, args: &[Obj]) -> Obj {
    let _ = poll_poll(n, args);
    let self_ = unsafe { &mut *poll_ptr(args[0]) };
    self_.iter_cnt = self_.entries.iter().filter(|e| e.revents != 0).count() as i32;
    self_.iter_idx = 0;
    args[0]
}

static mut POLL_SLOTS: [*const (); 2] = [poll_iternext as *const (), core::ptr::null()];
static mut TYPE_POLL: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_ITERNEXT,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 1,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { POLL_SLOTS.as_ptr() },
};

static POLL_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_poll_type() -> &'static ObjType {
    POLL_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("register")),
                value: mk2(poll_register),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("unregister")),
                value: mk2(poll_unregister),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("modify")),
                value: mk3(poll_modify),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("poll")),
                value: mkv(1, 2, poll_poll),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ipoll")),
                value: mkv(1, 3, poll_ipoll),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            POLL_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_POLL.name = qstr::from_str("poll");
        }
    });
    unsafe { &TYPE_POLL }
}

fn select_poll() -> Obj {
    let ty = init_poll_type();
    let o = malloc::new_obj::<ObjPoll>().expect("poll");
    unsafe {
        (*o).base.type_ = ty as *const ObjType;
        (*o).entries = Vec::new();
        (*o).iter_cnt = 0;
        (*o).iter_idx = 0;
        (*o).ret_tuple = obj::OBJ_NULL;
        obj::from_ptr(o as *const ObjPoll as *const ())
    }
}

/// Register built-in `select` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_SELECT {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("select")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("poll")),
            value: mk0(select_poll),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("POLLIN")),
            value: obj::new_small_int(stream::STREAM_POLL_RD as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("POLLOUT")),
            value: obj::new_small_int(stream::STREAM_POLL_WR as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("POLLERR")),
            value: obj::new_small_int(stream::STREAM_POLL_ERR as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("POLLHUP")),
            value: obj::new_small_int(stream::STREAM_POLL_HUP as isize),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("select module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("select"), module);
    module
}
