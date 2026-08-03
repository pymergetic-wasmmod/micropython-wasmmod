//! rewrite of ports/unix/modsocket.c
// symmetry: done

use py_rs::obj::Obj;
use py_rs::objlist;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime::{self, HandlePendingBehaviour};
use std::ffi::CString;

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn raise_errno() -> ! {
    raise::raise(MpRaise::OSError(errno()));
}

/// Unix socket object (`mp_obj_socket_t`).
#[repr(C)]
pub struct Socket {
    pub fd: i32,
    pub blocking: bool,
}

impl Socket {
    pub fn new(fd: i32) -> Self {
        Self { fd, blocking: true }
    }
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

pub fn fileno(sock: &Socket) -> Obj {
    py_rs::obj::new_small_int(sock.fd as isize)
}

pub fn connect(sock: &mut Socket, addr: Obj) -> Obj {
    let (len, data) = get_buffer(addr);
    let ret = retry_syscall(|| unsafe {
        libc::connect(
            sock.fd,
            data.as_ptr() as *const libc::sockaddr,
            len as libc::socklen_t,
        )
    }) ;
    if ret == -1 {
        raise_errno();
    }
    py_rs::obj::CONST_NONE
}

pub fn bind(sock: &Socket, addr: Obj) -> Obj {
    let (len, data) = get_buffer(addr);
    let ret = retry_syscall(|| unsafe {
        libc::bind(
            sock.fd,
            data.as_ptr() as *const libc::sockaddr,
            len as libc::socklen_t,
        )
    }) ;
    if ret == -1 {
        raise_errno();
    }
    py_rs::obj::CONST_NONE
}

pub fn listen(sock: &Socket, backlog: Option<i32>) -> Obj {
    let backlog = backlog.unwrap_or(crate::mpconfigport::PY_SOCKET_LISTEN_BACKLOG_DEFAULT as i32);
    let ret = retry_syscall(|| unsafe { libc::listen(sock.fd, backlog) });
    if ret == -1 {
        raise_errno();
    }
    py_rs::obj::CONST_NONE
}

pub fn accept(sock: &Socket) -> (Obj, Obj) {
    let mut addr: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut addrlen = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let fd = retry_syscall(|| unsafe {
        libc::accept(
            sock.fd,
            &mut addr as *mut _ as *mut libc::sockaddr,
            &mut addrlen,
        )
    }) ;
    if fd == -1 {
        raise_errno();
    }
    let peer = objstr::new_str(&bytes_from_sockaddr(&addr, addrlen));
    (new_socket_obj(fd), peer)
}

pub fn recv(sock: &Socket, max_len: usize) -> Obj {
    let mut buf = vec![0u8; max_len];
    let n = retry_syscall(|| unsafe {
        libc::recv(
            sock.fd,
            buf.as_mut_ptr() as *mut _,
            max_len,
            0,
        ) as i32
    });
    if n == -1 {
        raise_errno();
    }
    objstr::new_str(&buf[..n as usize])
}

pub fn send(sock: &Socket, data: Obj) -> Obj {
    let (len, bytes) = get_buffer(data);
    let n = retry_syscall(|| unsafe {
        libc::send(sock.fd, bytes.as_ptr() as *const _, len, 0) as i32
    });
    if n == -1 {
        raise_errno();
    }
    py_rs::obj::new_small_int(n as isize)
}

pub fn setblocking(sock: &mut Socket, flag: bool) -> Obj {
    let flags = unsafe { libc::fcntl(sock.fd, libc::F_GETFL, 0) };
    if flags == -1 {
        raise_errno();
    }
    let new_flags = if flag {
        flags & !libc::O_NONBLOCK
    } else {
        flags | libc::O_NONBLOCK
    };
    if unsafe { libc::fcntl(sock.fd, libc::F_SETFL, new_flags) } == -1 {
        raise_errno();
    }
    sock.blocking = flag;
    py_rs::obj::CONST_NONE
}

pub fn make_new(family: i32, kind: i32, proto: i32) -> Obj {
    let fd = retry_syscall(|| unsafe { libc::socket(family, kind, proto) });
    if fd == -1 {
        raise_errno();
    }
    new_socket_obj(fd)
}

fn new_socket_obj(fd: i32) -> Obj {
    let _ = Socket::new(fd);
    // Full Obj allocation wired when extmod socket module registers the type.
    py_rs::obj::new_small_int(fd as isize)
}

pub fn inet_pton(family: Obj, addr: Obj) -> Obj {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let family = py_rs::obj::get_int(family) as i32;
    let s = objstr::str_get_str(addr);
    let bytes = match family {
        libc::AF_INET => {
            let ip: Ipv4Addr = s.parse().unwrap_or(Ipv4Addr::UNSPECIFIED);
            ip.octets().to_vec()
        }
        libc::AF_INET6 => {
            let ip: Ipv6Addr = s.parse().unwrap_or(Ipv6Addr::UNSPECIFIED);
            ip.octets().to_vec()
        }
        _ => raise::raise(MpRaise::OSError(libc::EINVAL)),
    };
    objstr::new_str(&bytes)
}

pub fn inet_ntop(family: Obj, binaddr: Obj) -> Obj {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let family = py_rs::obj::get_int(family) as i32;
    let (_, data) = get_buffer(binaddr);
    let s = match family {
        libc::AF_INET if data.len() >= 4 => {
            Ipv4Addr::new(data[0], data[1], data[2], data[3]).to_string()
        }
        libc::AF_INET6 if data.len() >= 16 => {
            let mut oct = [0u8; 16];
            oct.copy_from_slice(&data[..16]);
            Ipv6Addr::from(oct).to_string()
        }
        _ => raise::raise(MpRaise::OSError(libc::EINVAL)),
    };
    objstr::new_str(s.as_bytes())
}

pub fn getaddrinfo(host: Obj, port: Obj, family: i32, kind: i32, proto: i32, flags: i32) -> Obj {
    let host = CString::new(objstr::str_get_str(host)).unwrap_or_default();
    let port_s = if py_rs::obj::is_int(port) {
        py_rs::obj::get_int(port).to_string()
    } else {
        objstr::str_get_str(port)
    };
    let port = CString::new(port_s).unwrap_or_default();
    let mut res: *mut libc::addrinfo = std::ptr::null_mut();
    let hints = libc::addrinfo {
        ai_flags: flags,
        ai_family: family,
        ai_socktype: kind,
        ai_protocol: proto,
        ai_addrlen: 0,
        ai_addr: std::ptr::null_mut(),
        ai_canonname: std::ptr::null_mut(),
        ai_next: std::ptr::null_mut(),
    };
    let err = unsafe { libc::getaddrinfo(host.as_ptr(), port.as_ptr(), &hints, &mut res) };
    if err != 0 {
        raise::raise(MpRaise::OSError(err));
    }
    let mut list_items = Vec::new();
    let mut cur = res;
    while !cur.is_null() {
        unsafe {
            let ai = &*cur;
            let tuple_items = [
                py_rs::obj::new_small_int(ai.ai_family as isize),
                py_rs::obj::new_small_int(ai.ai_socktype as isize),
                py_rs::obj::new_small_int(ai.ai_protocol as isize),
                py_rs::obj::CONST_NONE,
                objstr::new_str(std::slice::from_raw_parts(
                    ai.ai_addr as *const u8,
                    ai.ai_addrlen as usize,
                )),
            ];
            list_items.push(objtuple::new_tuple(5, Some(&tuple_items)));
            cur = ai.ai_next;
        }
    }
    unsafe {
        libc::freeaddrinfo(res);
    }
    objlist::new_list(list_items.len(), Some(&list_items))
}

pub fn sockaddr(addr: Obj) -> Obj {
    let (_, buf) = get_buffer(addr);
    let sa = unsafe { &*(buf.as_ptr() as *const libc::sockaddr) };
    match sa.sa_family as i32 {
        libc::AF_INET => {
            let sa = unsafe { &*(buf.as_ptr() as *const libc::sockaddr_in) };
            let items = [
                py_rs::obj::new_small_int(libc::AF_INET as isize),
                objstr::new_str(unsafe {
                    std::slice::from_raw_parts(
                        &sa.sin_addr as *const _ as *const u8,
                        4,
                    )
                }),
                py_rs::obj::new_small_int(u16::from_be(sa.sin_port) as isize),
            ];
            objtuple::new_tuple(3, Some(&items))
        }
        _ => py_rs::obj::CONST_NONE,
    }
}

fn get_buffer(o: Obj) -> (usize, Vec<u8>) {
    let s = objstr::str_get_str(o);
    (s.len(), s.into_bytes())
}

fn bytes_from_sockaddr(addr: &libc::sockaddr_storage, len: libc::socklen_t) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(addr as *const _ as *const u8, len as usize).to_vec() }
}

/// Socket module constants (partial; extended at registration time).
pub fn module_constants() -> Vec<(&'static str, i32)> {
    vec![
        ("AF_INET", libc::AF_INET),
        ("AF_INET6", libc::AF_INET6),
        ("AF_UNIX", libc::AF_UNIX),
        ("SOCK_STREAM", libc::SOCK_STREAM),
        ("SOCK_DGRAM", libc::SOCK_DGRAM),
        ("SOCK_RAW", libc::SOCK_RAW),
    ]
}
