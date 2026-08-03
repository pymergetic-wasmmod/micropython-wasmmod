//! rewrite of extmod/machine_uart.c (unix host tty/stdio path when `cfg(unix)`)
//! MCU hardware UART, `UART.irq` (when `PY_MACHINE_UART_IRQ`), and pin-mux args need port serial HAL.
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::malloc;
use py_rs::map::{self, Map, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{
    self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{
    self, StreamP, STREAM_ERROR, STREAM_FLUSH, STREAM_POLL, STREAM_POLL_RD, STREAM_POLL_WR,
};

const UART_RTS: i32 = 1;
const UART_CTS: i32 = 2;

const DEFAULT_BAUDRATE: u32 = 115_200;
const PATH_MAX: usize = 64;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct ObjUart {
    base: ObjBase,
    read_fd: i32,
    write_fd: i32,
    path: [u8; PATH_MAX],
    path_len: u8,
    baudrate: u32,
    bits: u8,
    parity: u8,
    stop: u8,
    flow: u8,
    timeout: u16,
    timeout_char: u16,
    /// When set, `deinit` does not close stdio fds.
    is_stdio: bool,
}

fn host_uart_enabled() -> bool {
    mpconfig::PY_MACHINE_UART && cfg!(unix)
}

fn uart_ptr(o: Obj) -> *mut ObjUart {
    obj::as_ptr(o) as *mut ObjUart
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn raise_os_error() -> ! {
    raise::raise(MpRaise::OSError(errno()));
}

fn parity_name(p: u8) -> &'static str {
    match p {
        1 => "0",
        2 => "1",
        _ => "None",
    }
}

fn write_path(out: &mut [u8], s: &[u8]) -> u8 {
    let n = s.len().min(out.len());
    out[..n].copy_from_slice(&s[..n]);
    n as u8
}

fn write_u32(mut n: u32, out: &mut [u8], start: usize) -> usize {
    if n == 0 {
        out[start] = b'0';
        return start + 1;
    }
    let mut digits = [0u8; 10];
    let mut len = 0usize;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in 0..len {
        out[start + i] = digits[len - 1 - i];
    }
    start + len
}

fn format_id_path(prefix: &[u8], id: i32, out: &mut [u8]) -> u8 {
    let start = write_path(out, prefix);
    write_u32(id as u32, out, start as usize) as u8
}

fn try_open_id(id: i32) -> (i32, [u8; PATH_MAX], u8) {
    let mut path = [0u8; PATH_MAX];
    let len = format_id_path(b"/dev/ttyUSB", id, &mut path);
    let mut fd = open_device(&path[..len as usize]);
    if fd >= 0 {
        return (fd, path, len);
    }
    let len = format_id_path(b"/dev/ttyS", id, &mut path);
    fd = open_device(&path[..len as usize]);
    (fd, path, len)
}

fn open_device(path: &[u8]) -> i32 {
    unsafe {
        libc::open(
            path.as_ptr() as *const _,
            libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
        )
    }
}

fn baud_to_speed(baud: u32) -> libc::speed_t {
    match baud {
        50 => libc::B50,
        75 => libc::B75,
        110 => libc::B110,
        134 => libc::B134,
        150 => libc::B150,
        200 => libc::B200,
        300 => libc::B300,
        600 => libc::B600,
        1200 => libc::B1200,
        1800 => libc::B1800,
        2400 => libc::B2400,
        4800 => libc::B4800,
        9600 => libc::B9600,
        19200 => libc::B19200,
        38400 => libc::B38400,
        57600 => libc::B57600,
        115200 => libc::B115200,
        230400 => libc::B230400,
        460800 => libc::B460800,
        921600 => libc::B921600,
        _ => 0,
    }
}

fn apply_termios(self_: &mut ObjUart) {
    unsafe {
        let fd = self_.read_fd;
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            raise_os_error();
        }
        libc::cfmakeraw(&mut t);
        t.c_cflag |= libc::CREAD | libc::CLOCAL;
        t.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
        t.c_cc[libc::VMIN as usize] = 0;
        t.c_cc[libc::VTIME as usize] = 0;

        t.c_cflag &= !libc::CSIZE;
        match self_.bits {
            5 => t.c_cflag |= libc::CS5,
            6 => t.c_cflag |= libc::CS6,
            7 => t.c_cflag |= libc::CS7,
            8 => t.c_cflag |= libc::CS8,
            _ => raise::raise(MpRaise::ValueError("invalid data bits")),
        }

        t.c_cflag &= !(libc::PARENB | libc::PARODD);
        match self_.parity {
            0 => {}
            1 => {
                t.c_cflag |= libc::PARENB;
            }
            2 => {
                t.c_cflag |= libc::PARENB | libc::PARODD;
            }
            _ => raise::raise(MpRaise::ValueError("invalid parity")),
        }

        if self_.stop == 1 {
            t.c_cflag &= !libc::CSTOPB;
        } else if self_.stop == 2 {
            t.c_cflag |= libc::CSTOPB;
        } else {
            raise::raise(MpRaise::ValueError("invalid stop bits"));
        }

        if self_.flow & (UART_RTS as u8 | UART_CTS as u8) != 0 {
            t.c_cflag |= libc::CRTSCTS;
        } else {
            t.c_cflag &= !libc::CRTSCTS;
        }

        let speed = baud_to_speed(self_.baudrate);
        if speed != 0 {
            libc::cfsetispeed(&mut t, speed);
            libc::cfsetospeed(&mut t, speed);
        }

        if libc::tcsetattr(fd, libc::TCSANOW, &t) != 0 {
            raise_os_error();
        }
    }
}

fn open_from_id(id_obj: Obj) -> *mut ObjUart {
    let id = obj::get_int(id_obj) as i32;
    let o = malloc::new_obj::<ObjUart>().expect("uart");
    let self_ = unsafe { &mut *o };
    self_.base.type_ = uart_type();
    self_.baudrate = DEFAULT_BAUDRATE;
    self_.bits = 8;
    self_.parity = 0;
    self_.stop = 1;
    self_.flow = 0;
    self_.timeout = 0;
    self_.timeout_char = 0;
    self_.is_stdio = false;

    if id == -1 {
        self_.read_fd = 0;
        self_.write_fd = 1;
        self_.is_stdio = true;
        self_.path_len = write_path(&mut self_.path, b"stdio");
        return o;
    }

    let (fd, path, len) = try_open_id(id);
    if fd < 0 {
        raise_os_error();
    }
    self_.read_fd = fd;
    self_.write_fd = fd;
    self_.path = path;
    self_.path_len = len;
    o
}

fn open_from_path(path_obj: Obj) -> *mut ObjUart {
    let path_s = objstr::str_get_str(path_obj);
    let path_bytes = path_s.as_bytes();
    let o = malloc::new_obj::<ObjUart>().expect("uart");
    let self_ = unsafe { &mut *o };
    self_.base.type_ = uart_type();
    self_.baudrate = DEFAULT_BAUDRATE;
    self_.bits = 8;
    self_.parity = 0;
    self_.stop = 1;
    self_.flow = 0;
    self_.timeout = 0;
    self_.timeout_char = 0;
    self_.is_stdio = false;
    self_.path_len = write_path(&mut self_.path, path_bytes);

    if path_s == "stdio" {
        self_.read_fd = 0;
        self_.write_fd = 1;
        self_.is_stdio = true;
        return o;
    }

    let fd = open_device(path_bytes);
    if fd < 0 {
        raise_os_error();
    }
    self_.read_fd = fd;
    self_.write_fd = fd;
    o
}

fn uart_init_helper(self_: *mut ObjUart, n_pos: usize, pos: &[Obj], kw: &Map) {
    let allowed = [
        Arg {
            qst: qstr::from_str("baudrate"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(DEFAULT_BAUDRATE as isize),
        },
        Arg {
            qst: qstr::from_str("bits"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(8),
        },
        Arg {
            qst: qstr::from_str("parity"),
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: qstr::from_str("stop"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(1),
        },
        Arg {
            qst: qstr::from_str("timeout"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("timeout_char"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("flow"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
    ];
    let mut vals = [ArgVal::default(); 7];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);

    let u = unsafe { &mut *self_ };
    if let ArgVal::Int(v) = vals[0] {
        u.baudrate = v as u32;
    }
    if let ArgVal::Int(v) = vals[1] {
        u.bits = v as u8;
    }
    if let ArgVal::Obj(v) = vals[2] {
        if v == obj::CONST_NONE {
            u.parity = 0;
        } else {
            let p = obj::get_int(v);
            if p == 0 {
                u.parity = 1;
            } else if p == 1 {
                u.parity = 2;
            } else {
                raise::raise(MpRaise::ValueError("invalid parity"));
            }
        }
    }
    if let ArgVal::Int(v) = vals[3] {
        u.stop = v as u8;
    }
    if let ArgVal::Int(v) = vals[4] {
        u.timeout = v as u16;
    }
    if let ArgVal::Int(v) = vals[5] {
        u.timeout_char = v as u16;
    }
    if let ArgVal::Int(v) = vals[6] {
        let flow = v as u8;
        if flow != 0 && flow != (UART_RTS | UART_CTS) as u8 {
            raise::raise(MpRaise::ValueError("invalid flow control"));
        }
        u.flow = flow;
    }

    if host_uart_enabled() && !u.is_stdio {
        apply_termios(u);
    }
}

fn uart_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let u = unsafe { &*uart_ptr(self_in) };
    let path = std::str::from_utf8(&u.path[..u.path_len as usize]).unwrap_or("?");
    mpprint::printf(
        print,
        "UART(\"{}\", baudrate={}, bits={}, parity={}, stop={}, timeout={}, timeout_char={})",
        [
            VaArg::Str(path),
            VaArg::Int(u.baudrate as i32),
            VaArg::Int(u.bits as i32),
            VaArg::Str(parity_name(u.parity)),
            VaArg::Int(u.stop as i32),
            VaArg::Int(u.timeout as i32),
            VaArg::Int(u.timeout_char as i32),
        ],
    );
}

fn uart_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if !mpconfig::PY_MACHINE_UART {
        raise::raise(MpRaise::RuntimeError("machine.UART disabled"));
    }
    if !host_uart_enabled() {
        raise::raise(MpRaise::OSError(0));
    }
    argcheck::check_num(n_args, n_kw, 1, 65535, true);

    let o = if obj::is_str_or_bytes(args[0]) {
        open_from_path(args[0])
    } else {
        open_from_id(args[0])
    };

    let mut kw = Map::default();
    map::init(&mut kw, n_kw);
    for i in 0..n_kw {
        if let Some(slot) = map::lookup(
            &mut kw,
            args[n_args + i * 2],
            map::LookupKind::AddIfNotFound,
        ) {
            slot.value = args[n_args + i * 2 + 1];
        }
    }
    uart_init_helper(o, n_args - 1, &args[1..n_args], &kw);
    obj::from_ptr(o as *const ObjUart as *const ())
}

fn uart_deinit(self_in: Obj) -> Obj {
    let u = unsafe { &mut *uart_ptr(self_in) };
    if !u.is_stdio {
        if u.read_fd >= 0 {
            unsafe {
                libc::close(u.read_fd);
            }
        }
        if u.write_fd >= 0 && u.write_fd != u.read_fd {
            unsafe {
                libc::close(u.write_fd);
            }
        }
    }
    u.read_fd = -1;
    u.write_fd = -1;
    obj::CONST_NONE
}

fn poll_readable(fd: i32, timeout_ms: u16) -> bool {
    if fd < 0 {
        return false;
    }
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = if timeout_ms == 0 {
        0
    } else {
        timeout_ms as i32
    };
    unsafe { libc::poll(&mut pfd, 1, ms) > 0 && (pfd.revents & libc::POLLIN) != 0 }
}

fn uart_any(self_in: Obj) -> Obj {
    let u = unsafe { &*uart_ptr(self_in) };
    let n = if poll_readable(u.read_fd, 0) {
        let mut nread = 0i32;
        unsafe {
            libc::ioctl(u.read_fd, libc::FIONREAD, &mut nread as *mut _);
        }
        if nread < 0 {
            0
        } else {
            nread
        }
    } else {
        0
    };
    obj::new_small_int(n as isize)
}

fn uart_txdone(self_in: Obj) -> Obj {
    let u = unsafe { &*uart_ptr(self_in) };
    if u.write_fd < 0 {
        return obj::CONST_FALSE;
    }
    unsafe {
        if libc::tcdrain(u.write_fd) == 0 {
            obj::CONST_TRUE
        } else {
            obj::CONST_FALSE
        }
    }
}

fn uart_sendbreak(self_in: Obj) -> Obj {
    let u = unsafe { &*uart_ptr(self_in) };
    if u.write_fd < 0 {
        raise::raise(MpRaise::OSError(9)); // EBADF
    }
    unsafe {
        if libc::tcsendbreak(u.write_fd, 0) != 0 {
            raise_os_error();
        }
    }
    obj::CONST_NONE
}

fn uart_readchar(self_in: Obj) -> Obj {
    let u = unsafe { &*uart_ptr(self_in) };
    if !poll_readable(u.read_fd, u.timeout) {
        raise::raise(MpRaise::OSError(11)); // EAGAIN
    }
    let mut c = 0u8;
    let n = unsafe { libc::read(u.read_fd, &mut c as *mut _ as *mut _, 1) };
    if n <= 0 {
        raise_os_error();
    }
    obj::new_small_int(c as isize)
}

fn uart_writechar(self_in: Obj, char_in: Obj) -> Obj {
    let c = obj::get_int(char_in) as u8;
    let u = unsafe { &*uart_ptr(self_in) };
    let n = unsafe { libc::write(u.write_fd, &c as *const _ as *const _, 1) };
    if n != 1 {
        raise_os_error();
    }
    obj::CONST_NONE
}

fn uart_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    if size == 0 {
        return 0;
    }
    let u = unsafe { &*uart_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if !poll_readable(u.read_fd, u.timeout) {
        unsafe {
            *errcode = 11;
        }
        return STREAM_ERROR;
    }
    let mut total = 0usize;
    while total < size {
        let n = unsafe { libc::read(u.read_fd, buf.add(total) as *mut _, size - total) };
        if n < 0 {
            let e = errno();
            if total > 0 && (e == 11 || e == 4) {
                break;
            }
            unsafe {
                *errcode = e;
            }
            return if total > 0 { total } else { STREAM_ERROR };
        }
        if n == 0 {
            break;
        }
        total += n as usize;
        if total < size && !poll_readable(u.read_fd, u.timeout_char) {
            break;
        }
    }
    total
}

fn uart_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let u = unsafe { &*uart_ptr(self_in) };
    unsafe {
        *errcode = 0;
        let n = libc::write(u.write_fd, buf as *const _, size);
        if n < 0 {
            *errcode = errno();
            return STREAM_ERROR;
        }
        n as usize
    }
}

fn uart_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let u = unsafe { &*uart_ptr(self_in) };
    match request {
        STREAM_POLL => {
            let flags = arg as u32;
            let mut ret = 0u32;
            if (flags & STREAM_POLL_RD) != 0 && poll_readable(u.read_fd, 0) {
                ret |= STREAM_POLL_RD;
            }
            if (flags & STREAM_POLL_WR) != 0 {
                ret |= STREAM_POLL_WR;
            }
            ret as usize
        }
        STREAM_FLUSH => {
            unsafe {
                if libc::tcdrain(u.write_fd) != 0 {
                    *errcode = errno();
                    return STREAM_ERROR;
                }
            }
            0
        }
        _ => {
            unsafe {
                *errcode = 22;
            }
            STREAM_ERROR
        }
    }
}

static UART_STREAM: StreamP = StreamP {
    read: Some(uart_read),
    write: Some(uart_write),
    ioctl: Some(uart_ioctl),
    is_text: false,
};

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
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FK: [*const (); 1] = [call_kw as *const ()];

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
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        if let Some(slot) = map::lookup(&mut kw, a[n + i * 2], map::LookupKind::AddIfNotFound) {
            slot.value = a[n + i * 2 + 1];
        }
    }
    (self_.fun)(n, a, &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("uart fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("uart fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("uart fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn uart_init(n: usize, args: &[Obj], kw: &Map) -> Obj {
    uart_init_helper(uart_ptr(args[0]), n - 1, &args[1..n], kw);
    obj::CONST_NONE
}

fn locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, uart_init),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk1(uart_deinit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("flush")),
                value: stream::stream_flush_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read1_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readline")),
                value: stream::stream_unbuffered_readline_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: stream::stream_readinto1_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write1_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("any")),
                value: mk1(uart_any),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("txdone")),
                value: mk1(uart_txdone),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("RTS")),
                value: obj::new_small_int(UART_RTS as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("CTS")),
                value: obj::new_small_int(UART_CTS as isize),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__del__")),
                value: mk1(uart_deinit),
            },
        ];
        if mpconfig::PY_MACHINE_UART_SENDBREAK {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("sendbreak")),
                value: mk1(uart_sendbreak),
            });
        }
        if mpconfig::PY_MACHINE_UART_READCHAR_WRITECHAR {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("readchar")),
                value: mk1(uart_readchar),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("writechar")),
                value: mk2(uart_writechar),
            });
        }
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

static mut UART_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut UART_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { UART_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// `machine_uart_type`
pub fn uart_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_UART {
        panic!("UART disabled");
    }
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            UART_SLOTS[0] = uart_make_new as MakeNewFn as *const ();
            UART_SLOTS[1] = uart_print as *const ();
            UART_SLOTS[2] = &UART_STREAM as *const StreamP as *const ();
            UART_SLOTS[3] = dict;
            UART_TYPE.name = qstr::from_str("UART");
        }
    });
    unsafe { &UART_TYPE }
}

pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && mpconfig::PY_MACHINE_UART
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_path_format() {
        let mut buf = [0u8; PATH_MAX];
        let n = format_id_path(b"/dev/ttyUSB", 0, &mut buf);
        assert_eq!(&buf[..n as usize], b"/dev/ttyUSB0");
        let n = format_id_path(b"/dev/ttyS", 12, &mut buf);
        assert_eq!(&buf[..n as usize], b"/dev/ttyS12");
    }

    #[test]
    fn baud_speed_mapping() {
        assert_eq!(baud_to_speed(115200), libc::B115200);
        assert_eq!(baud_to_speed(99999), 0);
    }
}
