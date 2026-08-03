//! rewrite of extmod/modsocket.c + ports/unix/modsocket.c (unix host POSIX)
//! Embedded network-NIC delegation path from extmod/modsocket.c (non-unix) needs port NIC HAL.
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objfloat;
use py_rs::objlist;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::objtype;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime::{self, HandlePendingBehaviour};
use py_rs::stream::{
    self, StreamIoFn, StreamIoctlFn, StreamP, STREAM_CLOSE, STREAM_ERROR, STREAM_GET_FILENO,
    STREAM_POLL, STREAM_POLL_ERR, STREAM_POLL_HUP, STREAM_POLL_NVAL, STREAM_POLL_RD,
    STREAM_POLL_WR,
};

use crate::vfs;

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn raise_errno() -> ! {
    raise::raise(MpRaise::OSError(errno()));
}

fn retry_syscall<F: FnMut() -> i32>(mut f: F) -> i32 {
    loop {
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
        let ret = f();
        if ret != -1 || errno() != libc::EINTR {
            return ret;
        }
    }
}

#[repr(C)]
pub struct ObjSocket {
    base: ObjBase,
    fd: i32,
    blocking: bool,
}

fn socket_ptr(o: Obj) -> *mut ObjSocket {
    obj::as_ptr(o) as *mut ObjSocket
}

fn socket_new(fd: i32) -> Obj {
    let o = malloc::new_obj::<ObjSocket>().expect("socket");
    unsafe {
        (*o).base.type_ = type_socket();
        (*o).fd = fd;
        (*o).blocking = true;
        obj::from_ptr(o as *const ObjSocket as *const ())
    }
}

fn socket_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*socket_ptr(self_in) };
    mpprint::printf(print, "<_socket {}>", [VaArg::Int(self_.fd)]);
}

fn socket_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*socket_ptr(self_in) };
    unsafe {
        *errcode = 0;
        let r = retry_syscall(|| unsafe { libc::read(self_.fd, buf as *mut _, size) as i32 });
        if r == -1 {
            let mut err = errno();
            if err == libc::EAGAIN && self_.blocking {
                err = py_rs::mperrno::ETIMEDOUT;
            }
            *errcode = err;
            return STREAM_ERROR;
        }
        r as usize
    }
}

fn socket_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*socket_ptr(self_in) };
    unsafe {
        *errcode = 0;
        let r = retry_syscall(|| unsafe { libc::write(self_.fd, buf as *const _, size) as i32 });
        if r == -1 {
            let mut err = errno();
            if err == libc::EAGAIN && self_.blocking {
                err = py_rs::mperrno::ETIMEDOUT;
            }
            *errcode = err;
            return STREAM_ERROR;
        }
        r as usize
    }
}

fn socket_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    match request {
        STREAM_CLOSE => {
            if self_.fd >= 0 {
                unsafe {
                    libc::close(self_.fd);
                }
            }
            self_.fd = -1;
            0
        }
        STREAM_GET_FILENO => self_.fd as usize,
        STREAM_POLL if mpconfig::PY_SELECT => {
            let mut ret = 0u32;
            let mut pollevents = 0i16;
            if (arg as u32 & STREAM_POLL_RD) != 0 {
                pollevents |= libc::POLLIN;
            }
            if (arg as u32 & STREAM_POLL_WR) != 0 {
                pollevents |= libc::POLLOUT;
            }
            let mut pfd = libc::pollfd {
                fd: self_.fd,
                events: pollevents,
                revents: 0,
            };
            if unsafe { libc::poll(&mut pfd, 1, 0) } > 0 {
                if pfd.revents & libc::POLLIN != 0 {
                    ret |= STREAM_POLL_RD;
                }
                if pfd.revents & libc::POLLOUT != 0 {
                    ret |= STREAM_POLL_WR;
                }
                if pfd.revents & libc::POLLERR != 0 {
                    ret |= STREAM_POLL_ERR;
                }
                if pfd.revents & libc::POLLHUP != 0 {
                    ret |= STREAM_POLL_HUP;
                }
                if pfd.revents & libc::POLLNVAL != 0 {
                    ret |= STREAM_POLL_NVAL;
                }
            }
            ret as usize
        }
        _ => {
            unsafe {
                *errcode = 22;
            }
            STREAM_ERROR
        }
    }
}

static SOCKET_STREAM: StreamP = StreamP {
    read: Some(socket_read as StreamIoFn),
    write: Some(socket_write),
    ioctl: Some(socket_ioctl as StreamIoctlFn),
    is_text: false,
};

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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
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
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("socket fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("socket fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("socket fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn get_buffer(o: Obj) -> (usize, *const u8) {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(o, &mut bufinfo, obj::BUFFER_READ);
    (bufinfo.len, bufinfo.buf as *const u8)
}

fn socket_fileno(self_in: Obj) -> Obj {
    let self_ = unsafe { &*socket_ptr(self_in) };
    obj::new_small_int(self_.fd as isize)
}

fn socket_connect(self_in: Obj, addr: Obj) -> Obj {
    let self_ = unsafe { &*socket_ptr(self_in) };
    let (len, data) = get_buffer(addr);
    let r = retry_syscall(|| unsafe {
        libc::connect(
            self_.fd,
            data as *const libc::sockaddr,
            len as libc::socklen_t,
        )
    });
    if r == -1 {
        let mut err = errno();
        if self_.blocking && err == libc::EINPROGRESS {
            err = py_rs::mperrno::ETIMEDOUT;
        }
        raise::raise(MpRaise::OSError(err));
    }
    obj::CONST_NONE
}

fn socket_bind(self_in: Obj, addr: Obj) -> Obj {
    let self_ = unsafe { &*socket_ptr(self_in) };
    let (len, data) = get_buffer(addr);
    let r = retry_syscall(|| unsafe {
        libc::bind(
            self_.fd,
            data as *const libc::sockaddr,
            len as libc::socklen_t,
        )
    });
    if r == -1 {
        raise_errno();
    }
    obj::CONST_NONE
}

fn socket_listen(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let mut backlog = mpconfig::PY_SOCKET_LISTEN_BACKLOG_DEFAULT as i32;
    if n > 1 {
        backlog = obj::get_int(args[1]) as i32;
        if backlog < 0 {
            backlog = 0;
        }
    }
    let r = retry_syscall(|| unsafe { libc::listen(self_.fd, backlog) });
    if r == -1 {
        raise_errno();
    }
    obj::CONST_NONE
}

fn socket_accept(self_in: Obj) -> Obj {
    let self_ = unsafe { &*socket_ptr(self_in) };
    let mut addr: [u8; 32] = [0; 32];
    let mut addrlen = addr.len() as libc::socklen_t;
    let fd = retry_syscall(|| unsafe {
        libc::accept(
            self_.fd,
            addr.as_mut_ptr() as *mut libc::sockaddr,
            &mut addrlen,
        )
    });
    if fd == -1 {
        let mut err = errno();
        if self_.blocking && err == libc::EAGAIN {
            err = py_rs::mperrno::ETIMEDOUT;
        }
        raise::raise(MpRaise::OSError(err));
    }
    let items = [socket_new(fd), objstr::new_bytes(&addr[..addrlen as usize])];
    objtuple::new_tuple(2, Some(&items))
}

fn socket_recv(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let sz = obj::get_int(args[1]) as usize;
    let flags = if n > 2 {
        obj::get_int(args[2]) as i32
    } else {
        0
    };
    let mut buf = vec![0u8; sz];
    let out_sz = retry_syscall(|| unsafe {
        libc::recv(self_.fd, buf.as_mut_ptr() as *mut _, sz, flags) as i32
    });
    if out_sz == -1 {
        raise_errno();
    }
    objstr::new_bytes(&buf[..out_sz as usize])
}

fn socket_recvfrom(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let sz = obj::get_int(args[1]) as usize;
    let flags = if n > 2 {
        obj::get_int(args[2]) as i32
    } else {
        0
    };
    let mut addr: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut addrlen = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let mut buf = vec![0u8; sz];
    let out_sz = retry_syscall(|| unsafe {
        libc::recvfrom(
            self_.fd,
            buf.as_mut_ptr() as *mut _,
            sz,
            flags,
            &mut addr as *mut _ as *mut libc::sockaddr,
            &mut addrlen,
        ) as i32
    });
    if out_sz == -1 {
        raise_errno();
    }
    let data = objstr::new_bytes(&buf[..out_sz as usize]);
    let peer = objstr::new_bytes(unsafe {
        std::slice::from_raw_parts(&addr as *const _ as *const u8, addrlen as usize)
    });
    objtuple::new_tuple(2, Some(&[data, peer]))
}

fn socket_send(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let is_sendall = n < 2;
    let flags = if n > 2 {
        obj::get_int(args[2]) as i32
    } else {
        0
    };
    let (len, data) = get_buffer(args[1]);
    let out_sz =
        retry_syscall(|| unsafe { libc::send(self_.fd, data as *const _, len, flags) as i32 });
    if out_sz == -1 {
        raise_errno();
    }
    if is_sendall && out_sz as usize != len {
        raise::raise(MpRaise::OSError(libc::EINTR));
    }
    obj::new_small_int(out_sz as isize)
}

fn socket_sendall(n: usize, args: &[Obj]) -> Obj {
    socket_send(n.saturating_sub(2), args);
    obj::CONST_NONE
}

fn socket_sendto(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let mut dst = args[2];
    let mut flags = 0i32;
    if n > 3 {
        flags = obj::get_int(args[2]) as i32;
        dst = args[3];
    }
    let (dlen, ddata) = get_buffer(args[1]);
    let (alen, adata) = get_buffer(dst);
    let out_sz = retry_syscall(|| unsafe {
        libc::sendto(
            self_.fd,
            ddata as *const _,
            dlen,
            flags,
            adata as *const libc::sockaddr,
            alen as libc::socklen_t,
        ) as i32
    });
    if out_sz == -1 {
        raise_errno();
    }
    obj::new_small_int(out_sz as isize)
}

fn socket_setsockopt(_n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let level = obj::get_int(args[1]) as i32;
    let option = obj::get_int(args[2]) as i32;
    let (optval, optlen) = if obj::is_int(args[3]) {
        let val = obj::get_int(args[3]) as i32;
        (
            &val as *const i32 as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t,
        )
    } else {
        let (len, data) = get_buffer(args[3]);
        (data as *const libc::c_void, len as libc::socklen_t)
    };
    let r = retry_syscall(|| unsafe { libc::setsockopt(self_.fd, level, option, optval, optlen) });
    if r == -1 {
        raise_errno();
    }
    obj::CONST_NONE
}

fn socket_setblocking(self_in: Obj, flag: Obj) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    let val = obj::is_true(flag);
    let flags = unsafe { libc::fcntl(self_.fd, libc::F_GETFL, 0) };
    if flags == -1 {
        raise_errno();
    }
    let new_flags = if val {
        flags & !libc::O_NONBLOCK
    } else {
        flags | libc::O_NONBLOCK
    };
    if unsafe { libc::fcntl(self_.fd, libc::F_SETFL, new_flags) } == -1 {
        raise_errno();
    }
    self_.blocking = val;
    obj::CONST_NONE
}

fn socket_settimeout(self_in: Obj, timeout: Obj) -> Obj {
    let self_ = unsafe { &mut *socket_ptr(self_in) };
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut new_blocking = true;
    if timeout != obj::CONST_NONE {
        if mpconfig::PY_BUILTINS_FLOAT {
            let val = objfloat::get_float(timeout);
            let ipart = val.trunc();
            let frac = val - ipart;
            tv.tv_sec = ipart as i64;
            tv.tv_usec = (frac * 1_000_000.0).round() as i64;
        } else {
            tv.tv_sec = obj::get_int(timeout) as i64;
            tv.tv_usec = 0;
        }
        if tv.tv_sec == 0 && tv.tv_usec == 0 {
            new_blocking = false;
        }
    }
    if new_blocking {
        if unsafe {
            libc::setsockopt(
                self_.fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        } == -1
        {
            raise_errno();
        }
        if unsafe {
            libc::setsockopt(
                self_.fd,
                libc::SOL_SOCKET,
                libc::SO_SNDTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        } == -1
        {
            raise_errno();
        }
    }
    if self_.blocking != new_blocking {
        socket_setblocking(self_in, obj::new_bool(new_blocking));
    }
    obj::CONST_NONE
}

fn socket_makefile(n: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*socket_ptr(args[0]) };
    let mut pos_args = vec![obj::new_small_int(self_.fd as isize)];
    pos_args.extend_from_slice(&args[1..n]);
    let mut kw = py_rs::map::Map::default();
    map::init(&mut kw, 0);
    vfs::open(pos_args.len(), &pos_args, &mut kw)
}

fn socket_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 3, false);
    let mut family = libc::AF_INET;
    let mut kind = libc::SOCK_STREAM;
    let mut proto = 0;
    if n_args > 0 {
        family = obj::get_int(args[0]) as i32;
        if n_args > 1 {
            kind = obj::get_int(args[1]) as i32;
            if n_args > 2 {
                proto = obj::get_int(args[2]) as i32;
            }
        }
    }
    let fd = retry_syscall(|| unsafe { libc::socket(family, kind, proto) });
    if fd == -1 {
        raise_errno();
    }
    socket_new(fd)
}

fn mod_inet_pton(family: Obj, addr: Obj) -> Obj {
    use std::net::IpAddr;
    let family = obj::get_int(family) as i32;
    let s = objstr::str_get_str(addr);
    let parsed = match s.parse::<IpAddr>() {
        Ok(v) => v,
        Err(_) => raise::raise(MpRaise::OSError(22)),
    };
    match (family, parsed) {
        (libc::AF_INET, IpAddr::V4(v4)) => objstr::new_bytes(&v4.octets()),
        (libc::AF_INET6, IpAddr::V6(v6)) => objstr::new_bytes(&v6.octets()),
        _ => raise::raise(MpRaise::OSError(22)),
    }
}

fn mod_inet_ntop(family: Obj, binaddr: Obj) -> Obj {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let family = obj::get_int(family) as i32;
    let (len, data) = get_buffer(binaddr);
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let text = match (family, bytes) {
        (libc::AF_INET, [a, b, c, d]) => IpAddr::V4(Ipv4Addr::new(*a, *b, *c, *d)).to_string(),
        (libc::AF_INET6, b) if b.len() == 16 => {
            let mut oct = [0u8; 16];
            oct.copy_from_slice(b);
            IpAddr::V6(Ipv6Addr::from(oct)).to_string()
        }
        _ => raise::raise(MpRaise::OSError(22)),
    };
    objstr::new_str(text.as_bytes())
}

fn mod_getaddrinfo(n: usize, args: &[Obj]) -> Obj {
    let host = objstr::str_get_str(args[0]);
    let host_c = std::ffi::CString::new(host).unwrap_or_default();
    let (serv_c, mut hints) = if obj::is_int(args[1]) {
        let port = obj::get_int(args[1]) as u16;
        let buf = format!("{port}");
        (
            std::ffi::CString::new(buf).unwrap_or_default(),
            libc::addrinfo {
                ai_flags: libc::AI_NUMERICSERV,
                ai_family: 0,
                ai_socktype: 0,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_addr: std::ptr::null_mut(),
                ai_canonname: std::ptr::null_mut(),
                ai_next: std::ptr::null_mut(),
            },
        )
    } else {
        (
            std::ffi::CString::new(objstr::str_get_str(args[1])).unwrap_or_default(),
            libc::addrinfo {
                ai_flags: 0,
                ai_family: 0,
                ai_socktype: 0,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_addr: std::ptr::null_mut(),
                ai_canonname: std::ptr::null_mut(),
                ai_next: std::ptr::null_mut(),
            },
        )
    };
    if n > 2 {
        hints.ai_family = obj::get_int(args[2]) as i32;
        if n > 3 {
            hints.ai_socktype = obj::get_int(args[3]) as i32;
            if n > 4 {
                hints.ai_protocol = obj::get_int(args[4]) as i32;
                if n > 5 {
                    hints.ai_flags = obj::get_int(args[5]) as i32;
                }
            }
        }
    }
    let mut res: *mut libc::addrinfo = std::ptr::null_mut();
    let err = retry_syscall(|| unsafe {
        libc::getaddrinfo(host_c.as_ptr(), serv_c.as_ptr(), &hints, &mut res)
    });
    if err != 0 {
        raise::raise(MpRaise::OSError(err));
    }
    let mut items = Vec::new();
    let mut cur = res;
    while !cur.is_null() {
        unsafe {
            let ai = &*cur;
            let canon = if ai.ai_canonname.is_null() {
                obj::CONST_NONE
            } else {
                obj::new_qstr(qstr::from_str(
                    std::ffi::CStr::from_ptr(ai.ai_canonname)
                        .to_str()
                        .unwrap_or(""),
                ))
            };
            let tuple_items = [
                obj::new_small_int(ai.ai_family as isize),
                obj::new_small_int(ai.ai_socktype as isize),
                obj::new_small_int(ai.ai_protocol as isize),
                canon,
                objstr::new_bytes(std::slice::from_raw_parts(
                    ai.ai_addr as *const u8,
                    ai.ai_addrlen as usize,
                )),
            ];
            items.push(objtuple::new_tuple(5, Some(&tuple_items)));
            cur = ai.ai_next;
        }
    }
    unsafe {
        libc::freeaddrinfo(res);
    }
    objlist::new_list(items.len(), Some(&items))
}

fn mod_sockaddr(addr: Obj) -> Obj {
    let (len, data) = get_buffer(addr);
    let sa = unsafe { &*(data as *const libc::sockaddr) };
    match sa.sa_family as i32 {
        libc::AF_INET => {
            let sa = unsafe { &*(data as *const libc::sockaddr_in) };
            let items = [
                obj::new_small_int(libc::AF_INET as isize),
                objstr::new_bytes(unsafe {
                    std::slice::from_raw_parts(&sa.sin_addr as *const _ as *const u8, 4)
                }),
                obj::new_small_int(u16::from_be(sa.sin_port) as isize),
            ];
            objtuple::new_tuple(3, Some(&items))
        }
        libc::AF_INET6 => {
            let sa = unsafe { &*(data as *const libc::sockaddr_in6) };
            let items = [
                obj::new_small_int(libc::AF_INET6 as isize),
                objstr::new_bytes(unsafe {
                    std::slice::from_raw_parts(&sa.sin6_addr as *const _ as *const u8, 16)
                }),
                obj::new_small_int(u16::from_be(sa.sin6_port) as isize),
                obj::new_small_int(u32::from_be(sa.sin6_flowinfo) as isize),
                obj::new_small_int(sa.sin6_scope_id as isize),
            ];
            objtuple::new_tuple(5, Some(&items))
        }
        _ => {
            let off = std::mem::offset_of!(libc::sockaddr, sa_data);
            let items = [
                obj::new_small_int(sa.sa_family as isize),
                objstr::new_bytes(if len > off {
                    unsafe { std::slice::from_raw_parts(data.add(off), len - off) }
                } else {
                    &[]
                }),
            ];
            objtuple::new_tuple(2, Some(&items))
        }
    }
}

fn locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("fileno")),
                value: mk1(socket_fileno),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("makefile")),
                value: mkv(1, 3, socket_makefile),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: stream::stream_readinto_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readline")),
                value: stream::stream_unbuffered_readline_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("connect")),
                value: mk2(|s, a| socket_connect(s, a)),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("bind")),
                value: mk2(|s, a| socket_bind(s, a)),
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
                key: obj::new_qstr(qstr::from_str("recvfrom")),
                value: mkv(2, 3, socket_recvfrom),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("send")),
                value: mkv(2, 3, socket_send),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("sendall")),
                value: mkv(2, 3, socket_sendall),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("sendto")),
                value: mkv(3, 4, socket_sendto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setsockopt")),
                value: mkv(4, 4, socket_setsockopt),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("setblocking")),
                value: mk2(|s, f| socket_setblocking(s, f)),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("settimeout")),
                value: mk2(|s, t| socket_settimeout(s, t)),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: stream::stream_close_obj(),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

static mut SOCKET_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut TYPE_SOCKET: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
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
    slots: unsafe { SOCKET_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_socket() -> &'static ObjType {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            TYPE_SOCKET.base.type_ = objtype::type_type() as *const ObjType;
            SOCKET_SLOTS[0] = socket_make_new as *const ();
            SOCKET_SLOTS[1] = socket_print as *const ();
            SOCKET_SLOTS[2] = &SOCKET_STREAM as *const StreamP as *const ();
            SOCKET_SLOTS[3] = dict;
            TYPE_SOCKET.name = qstr::from_str("socket");
        }
    });
    unsafe { &TYPE_SOCKET }
}

macro_rules! sock_const {
    ($table:expr, $name:ident) => {
        $table.push(MapElem {
            key: obj::new_qstr(qstr::from_str(stringify!($name))),
            value: obj::new_small_int(libc::$name as isize),
        });
    };
}

/// Register built-in `socket` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
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
            key: obj::new_qstr(qstr::from_str("inet_pton")),
            value: mk2(mod_inet_pton),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("inet_ntop")),
            value: mk2(mod_inet_ntop),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("sockaddr")),
            value: mk1(mod_sockaddr),
        },
    ];
    sock_const!(table, AF_UNIX);
    sock_const!(table, AF_INET);
    sock_const!(table, AF_INET6);
    sock_const!(table, SOCK_STREAM);
    sock_const!(table, SOCK_DGRAM);
    sock_const!(table, SOCK_RAW);
    sock_const!(table, MSG_DONTROUTE);
    sock_const!(table, MSG_DONTWAIT);
    sock_const!(table, MSG_PEEK);
    sock_const!(table, SOL_SOCKET);
    sock_const!(table, SO_BROADCAST);
    sock_const!(table, SO_ERROR);
    sock_const!(table, SO_KEEPALIVE);
    sock_const!(table, SO_LINGER);
    sock_const!(table, SO_REUSEADDR);
    sock_const!(table, SO_SNDTIMEO);
    sock_const!(table, SO_RCVTIMEO);
    sock_const!(table, IP_ADD_MEMBERSHIP);
    sock_const!(table, IP_DROP_MEMBERSHIP);
    let ctx = malloc::new_obj::<ModuleContext>().expect("socket module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("socket"), module);
    module
}
