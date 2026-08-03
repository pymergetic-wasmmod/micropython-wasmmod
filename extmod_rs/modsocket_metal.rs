//! Metal guest `socket` module — TCP via `pm_metal_net_ip_tcp_*`.
//!
//! Selected from [`crate::init_host`] when `feature = "metal_net"`. Host unix
//! keeps POSIX [`crate::modsocket`]. No NIC drivers live here.

use core::mem::size_of;

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{
    self, StreamIoFn, StreamIoWriteFn, StreamIoctlFn, StreamP, STREAM_CLOSE, STREAM_ERROR,
};

use crate::metal_net::{self, TcpHandle, HANDLE_INVALID};

const AF_INET: i32 = 2;
const SOCK_STREAM: i32 = 1;
const SOCK_DGRAM: i32 = 2;

#[repr(C)]
struct ObjSocket {
    base: ObjBase,
    handle: TcpHandle,
    is_listen: bool,
    family: i32,
    kind: i32,
    bind_port: u16,
}

fn socket_ptr(o: Obj) -> *mut ObjSocket {
    obj::as_ptr(o) as *mut ObjSocket
}

fn socket_new(family: i32, kind: i32) -> Obj {
    let o = malloc::new_obj::<ObjSocket>().expect("socket");
    unsafe {
        (*o).base.type_ = type_socket();
        (*o).handle = HANDLE_INVALID;
        (*o).is_listen = false;
        (*o).family = family;
        (*o).kind = kind;
        (*o).bind_port = 0;
        obj::from_ptr(o as *const ObjSocket as *const ())
    }
}

fn socket_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*socket_ptr(self_in) };
    mpprint::printf(
        print,
        "<_socket metal h={}>",
        [VaArg::Int(self_.handle as i32)],
    );
}

fn ensure_tcp(self_: &ObjSocket) {
    if self_.kind != SOCK_STREAM || self_.family != AF_INET {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EAFNOSUPPORT));
    }
    if !metal_net::metal_net_enabled() {
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn parse_addr(addr: Obj) -> (String, u16) {
    // MicroPython-style `(host, port)` tuple.
    if obj::is_type(addr, objtuple::type_tuple()) {
        let (len, items) = objtuple::tuple_get(addr);
        if len < 2 {
            raise::raise(MpRaise::OSError(py_rs::mperrno::EINVAL));
        }
        let host = if obj::is_str_or_bytes(items[0]) {
            objstr::str_get_str(items[0]).to_string()
        } else {
            raise::raise(MpRaise::TypeError("host must be str"));
        };
        let port = obj::get_int(items[1]) as u16;
        return (host, port);
    }
    raise::raise(MpRaise::TypeError("addr must be (host, port)"));
}

fn socket_connect(self_in: Obj, addr: Obj) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    ensure_tcp(self_);
    let (host, port) = parse_addr(addr);
    #[cfg(feature = "metal_net")]
    {
        match metal_net::tcp::connect(&host, port) {
            Some(h) => {
                self_.handle = h;
                self_.is_listen = false;
                obj::CONST_NONE
            }
            None => raise::raise(MpRaise::OSError(py_rs::mperrno::ECONNREFUSED)),
        }
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = (host, port);
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn socket_bind(self_in: Obj, addr: Obj) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    ensure_tcp(self_);
    let (_host, port) = parse_addr(addr);
    self_.bind_port = port;
    obj::CONST_NONE
}

fn socket_listen(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(args[0]) };
    ensure_tcp(self_);
    let port = if self_.bind_port != 0 {
        self_.bind_port
    } else if n > 1 {
        obj::get_int(args[1]) as u16
    } else {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EINVAL));
    };
    let _ = n;
    #[cfg(feature = "metal_net")]
    {
        match metal_net::tcp::listen(port) {
            Some(h) => {
                self_.handle = h;
                self_.is_listen = true;
                obj::CONST_NONE
            }
            None => raise::raise(MpRaise::OSError(py_rs::mperrno::EADDRINUSE)),
        }
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = port;
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn socket_accept(self_in: Obj) -> Obj {
    let self_ = unsafe { &*socket_ptr(self_in) };
    ensure_tcp(self_);
    if !self_.is_listen || self_.handle == HANDLE_INVALID {
        raise::raise(MpRaise::OSError(py_rs::mperrno::EINVAL));
    }
    #[cfg(feature = "metal_net")]
    {
        match metal_net::tcp::accept(self_.handle) {
            Some(h) => {
                let child = socket_new(self_.family, self_.kind);
                unsafe {
                    (*socket_ptr(child)).handle = h;
                    (*socket_ptr(child)).is_listen = false;
                }
                let peer = objtuple::new_tuple(
                    2,
                    Some(&[objstr::new_str(b"0.0.0.0"), obj::new_small_int(0)]),
                );
                objtuple::new_tuple(2, Some(&[child, peer]))
            }
            None => raise::raise(MpRaise::OSError(py_rs::mperrno::EAGAIN)),
        }
    }
    #[cfg(not(feature = "metal_net"))]
    {
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn socket_recv(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    ensure_tcp(self_);
    if self_.handle == HANDLE_INVALID {
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENOTCONN));
    }
    let sz = obj::get_int(args[1]) as usize;
    let _ = n;
    let mut buf = vec![0u8; sz];
    #[cfg(feature = "metal_net")]
    {
        let out = metal_net::tcp::read(self_.handle, &mut buf) as usize;
        objstr::new_bytes(&buf[..out.min(sz)])
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = buf;
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn socket_send(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    ensure_tcp(self_);
    if self_.handle == HANDLE_INVALID {
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENOTCONN));
    }
    let data = args[1];
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(data, &mut bufinfo, obj::BUFFER_READ);
    let slice = bufinfo.as_bytes();
    let _ = n;
    #[cfg(feature = "metal_net")]
    {
        let written = metal_net::tcp::write(self_.handle, slice);
        obj::new_small_int(written as isize)
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = slice;
        raise::raise(MpRaise::OSError(py_rs::mperrno::ENODEV));
    }
}

fn socket_close(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    #[cfg(feature = "metal_net")]
    {
        if self_.handle != HANDLE_INVALID {
            if self_.is_listen {
                metal_net::tcp::listen_close(self_.handle);
            } else {
                metal_net::tcp::close(self_.handle);
            }
            self_.handle = HANDLE_INVALID;
        }
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = self_;
    }
    obj::CONST_NONE
}

fn socket_setblocking(_self_in: Obj, _flag: Obj) -> Obj {
    obj::CONST_NONE
}

fn socket_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*socket_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if self_.handle == HANDLE_INVALID {
        unsafe {
            *errcode = py_rs::mperrno::ENOTCONN;
        }
        return STREAM_ERROR;
    }
    #[cfg(feature = "metal_net")]
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, size) };
        metal_net::tcp::read(self_.handle, slice) as usize
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = (buf, size);
        unsafe {
            *errcode = py_rs::mperrno::ENODEV;
        }
        STREAM_ERROR
    }
}

fn socket_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*socket_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if self_.handle == HANDLE_INVALID {
        unsafe {
            *errcode = py_rs::mperrno::ENOTCONN;
        }
        return STREAM_ERROR;
    }
    #[cfg(feature = "metal_net")]
    {
        let slice = unsafe { std::slice::from_raw_parts(buf, size) };
        metal_net::tcp::write(self_.handle, slice) as usize
    }
    #[cfg(not(feature = "metal_net"))]
    {
        let _ = (buf, size);
        unsafe {
            *errcode = py_rs::mperrno::ENODEV;
        }
        STREAM_ERROR
    }
}

fn socket_ioctl(self_in: Obj, request: u32, _arg: usize, errcode: *mut i32) -> usize {
    unsafe {
        *errcode = 0;
    }
    if request == STREAM_CLOSE {
        socket_close(self_in);
        return 0;
    }
    0
}

fn socket_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 3, false);
    let mut family = AF_INET;
    let mut kind = SOCK_STREAM;
    if n_args > 0 {
        family = obj::get_int(args[0]) as i32;
        if n_args > 1 {
            kind = obj::get_int(args[1]) as i32;
        }
    }
    socket_new(family, kind)
}

fn mod_getaddrinfo(n: usize, args: &[Obj]) -> Obj {
    let host = objstr::str_get_str(args[0]).to_string();
    let port = if obj::is_int(args[1]) {
        obj::get_int(args[1]) as u16
    } else {
        0
    };
    let _ = n;
    let sockaddr = objtuple::new_tuple(
        2,
        Some(&[
            objstr::new_str(host.as_bytes()),
            obj::new_small_int(port as isize),
        ]),
    );
    let entry = objtuple::new_tuple(
        5,
        Some(&[
            obj::new_small_int(AF_INET as isize),
            obj::new_small_int(SOCK_STREAM as isize),
            obj::new_small_int(0),
            objstr::new_str(b""),
            sockaddr,
        ]),
    );
    let list = py_rs::objlist::new_list(1, Some(&[entry]));
    list
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F1S: [*const (); 1] = [call1 as *const ()];
static mut F2S: [*const (); 1] = [call2 as *const ()];
static mut FVS: [*const (); 1] = [callv as *const ()];

static TF1: ObjType = ObjType {
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
    slots: unsafe { F1S.as_ptr() },
};
static TF2: ObjType = ObjType {
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
    slots: unsafe { F2S.as_ptr() },
};
static TFV: ObjType = ObjType {
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
    slots: unsafe { FVS.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin2) };
    (self_.fun)(a[0], a[1])
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

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("fun2");
    unsafe {
        (*o).base.type_ = &TF2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("funv");
    unsafe {
        (*o).base.type_ = &TFV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

static mut SOCKET_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut SOCKET_STREAM: StreamP = StreamP {
    read: None,
    write: None,
    ioctl: None,
    is_text: false,
};

static mut TYPE_SOCKET: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 3,
    slot_index_parent: 0,
    slot_index_locals_dict: 4,
    slots: core::ptr::null(),
};

fn type_socket() -> &'static ObjType {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("connect")),
                value: mk2(socket_connect),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("bind")),
                value: mk2(socket_bind),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("listen")),
                value: mkv(1, 2, socket_listen),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("accept")),
                value: mk1(socket_accept),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("recv")),
                value: mkv(2, 3, socket_recv),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("send")),
                value: mkv(2, 3, socket_send),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: mk1(socket_close),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setblocking")),
                value: mk2(socket_setblocking),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
            },
        ];
        let dict_obj = objdict::new_dict(table.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict_obj)).map, table);
            SOCKET_STREAM.read = Some(socket_read as StreamIoFn);
            SOCKET_STREAM.write = Some(socket_write as StreamIoWriteFn);
            SOCKET_STREAM.ioctl = Some(socket_ioctl as StreamIoctlFn);
            SOCKET_SLOTS[0] = socket_make_new as *const ();
            SOCKET_SLOTS[1] = socket_print as *const ();
            SOCKET_SLOTS[2] = &SOCKET_STREAM as *const StreamP as *const ();
            SOCKET_SLOTS[3] = dict_obj.0 as *const ();
            TYPE_SOCKET.slots = SOCKET_SLOTS.as_ptr();
            TYPE_SOCKET.name = qstr::from_str("socket");
        }
    });
    unsafe { &TYPE_SOCKET }
}

/// Register built-in `socket` over metal TCP faces.
pub fn init_module() -> Obj {
    if !mpconfig::PY_SOCKET {
        return obj::OBJ_NULL;
    }
    type_socket();
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("socket")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("socket")),
            value: obj::from_ptr(type_socket() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("getaddrinfo")),
            value: mkv(2, 6, mod_getaddrinfo),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("AF_INET")),
            value: obj::new_small_int(AF_INET as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("SOCK_STREAM")),
            value: obj::new_small_int(SOCK_STREAM as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("SOCK_DGRAM")),
            value: obj::new_small_int(SOCK_DGRAM as isize),
        },
    ];
    let _ = size_of::<ObjSocket>();
    let ctx = malloc::new_obj::<ModuleContext>().expect("socket module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(
            &mut (*objdict::dict_ptr(dict)).map,
            std::mem::take(&mut table),
        );
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("socket"), module);
    module
}
