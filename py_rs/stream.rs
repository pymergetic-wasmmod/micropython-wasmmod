//! rewrite of py/stream.c + py/stream.h
// symmetry: done

use core::cmp::min;

use crate::argcheck;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use crate::objlist;
use crate::objstr;
use crate::raise::{self, MpRaise};
use crate::unicode::utf8_is_nonascii;
use crate::vstr::{self, Vstr};

pub const STREAM_ERROR: usize = usize::MAX;

pub const STREAM_FLUSH: u32 = 1;
pub const STREAM_SEEK: u32 = 2;
pub const STREAM_POLL: u32 = 3;
pub const STREAM_CLOSE: u32 = 4;
pub const STREAM_TIMEOUT: u32 = 5;
pub const STREAM_GET_OPTS: u32 = 6;
pub const STREAM_SET_OPTS: u32 = 7;
pub const STREAM_GET_DATA_OPTS: u32 = 8;
pub const STREAM_SET_DATA_OPTS: u32 = 9;
pub const STREAM_GET_FILENO: u32 = 10;
pub const STREAM_GET_BUFFER_SIZE: u32 = 11;
pub const STREAM_RAISE_ERROR: u32 = 12;

pub const STREAM_POLL_RD: u32 = 0x0001;
pub const STREAM_POLL_WR: u32 = 0x0004;
pub const STREAM_POLL_ERR: u32 = 0x0008;
pub const STREAM_POLL_HUP: u32 = 0x0010;
pub const STREAM_POLL_NVAL: u32 = 0x0020;

pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

pub const STREAM_OP_READ: i32 = 1;
pub const STREAM_OP_WRITE: i32 = 2;
pub const STREAM_OP_IOCTL: i32 = 4;

pub const STREAM_RW_READ: u8 = 0;
pub const STREAM_RW_WRITE: u8 = 2;
pub const STREAM_RW_ONCE: u8 = 1;

const DEFAULT_BUFFER_SIZE: usize = 256;
const MP_EAGAIN: i32 = 11;
const MP_EWOULDBLOCK: i32 = 11;
const MP_EINVAL: i32 = 22;

pub type StreamIoFn = fn(Obj, *mut u8, usize, *mut i32) -> usize;
pub type StreamIoWriteFn = fn(Obj, *const u8, usize, *mut i32) -> usize;
pub type StreamIoctlFn = fn(Obj, u32, usize, *mut i32) -> usize;

/// Stream protocol (`mp_stream_p_t`).
#[repr(C)]
pub struct StreamP {
    pub read: Option<StreamIoFn>,
    pub write: Option<StreamIoWriteFn>,
    pub ioctl: Option<StreamIoctlFn>,
    pub is_text: bool,
}

/// Seek ioctl argument (`mp_stream_seek_t`).
#[repr(C)]
pub struct StreamSeek {
    pub offset: i64,
    pub whence: i32,
}

pub fn is_nonblocking_error(errno: i32) -> bool {
    if mpconfig::STREAMS_NON_BLOCK {
        errno == MP_EAGAIN || errno == MP_EWOULDBLOCK
    } else {
        false
    }
}

/// `mp_get_stream`
pub fn get_stream(self_in: Obj) -> &'static StreamP {
    let t = obj::get_type(self_in);
    unsafe { &*(obj::type_get_protocol(t).expect("stream protocol") as *const StreamP) }
}

/// `mp_get_stream_raise`
pub fn get_stream_raise(self_in: Obj, flags: i32) -> &'static StreamP {
    let t = obj::get_type(self_in);
    if let Some(proto) = obj::type_get_protocol(t) {
        let stream_p = unsafe { &*(proto as *const StreamP) };
        if !((flags & STREAM_OP_READ != 0) && stream_p.read.is_none())
            && !((flags & STREAM_OP_WRITE != 0) && stream_p.write.is_none())
            && !((flags & STREAM_OP_IOCTL != 0) && stream_p.ioctl.is_none())
        {
            return stream_p;
        }
    }
    raise::raise(MpRaise::OSError(0));
}

fn stream_raise_error(stream: Obj, error: i32) -> ! {
    if mpconfig::STREAMS_DELEGATE_ERROR {
        let stream_p = get_stream(stream);
        if let Some(ioctl) = stream_p.ioctl {
            let mut err = 0;
            let _ = ioctl(stream, STREAM_RAISE_ERROR, error as usize, &mut err);
        }
    }
    raise::raise(MpRaise::OSError(error));
}

/// `mp_stream_rw`
pub fn stream_rw(stream: Obj, buf: &mut [u8], errcode: &mut i32, flags: u8) -> usize {
    let stream_p = get_stream(stream);
    *errcode = 0;
    let mut done = 0usize;
    let mut buf_off = 0usize;
    let mut size = buf.len();
    while size > 0 {
        let out_sz = if flags & STREAM_RW_WRITE != 0 {
            stream_p.write.expect("write")(
                stream,
                unsafe { buf.as_ptr().add(buf_off) },
                size,
                errcode,
            )
        } else {
            stream_p.read.expect("read")(
                stream,
                unsafe { buf.as_mut_ptr().add(buf_off) },
                size,
                errcode,
            )
        };
        if out_sz == 0 {
            return done;
        }
        if out_sz == STREAM_ERROR {
            if is_nonblocking_error(*errcode) && done != 0 {
                *errcode = 0;
            }
            return done;
        }
        if flags & STREAM_RW_ONCE != 0 {
            return out_sz;
        }
        buf_off += out_sz;
        size -= out_sz;
        done += out_sz;
    }
    done
}

pub fn stream_write_exactly(stream: Obj, buf: &mut [u8], errcode: &mut i32) -> usize {
    stream_rw(stream, buf, errcode, STREAM_RW_WRITE)
}

pub fn stream_read_exactly(stream: Obj, buf: &mut [u8], errcode: &mut i32) -> usize {
    stream_rw(stream, buf, errcode, STREAM_RW_READ)
}

/// `mp_stream_seek`
pub fn stream_seek(stream: Obj, offset: i64, whence: i32, errcode: &mut i32) -> i64 {
    let mut seek_s = StreamSeek { offset, whence };
    let stream_p = get_stream(stream);
    let res = stream_p.ioctl.expect("ioctl")(
        stream,
        STREAM_SEEK,
        &mut seek_s as *mut _ as usize,
        errcode,
    );
    if res == STREAM_ERROR {
        return -1;
    }
    seek_s.offset
}

/// `mp_stream_write`
pub fn stream_write(self_in: Obj, buf: &[u8], flags: u8) -> Obj {
    let stream_p = get_stream(self_in);
    let mut error = 0;
    let mut done = 0usize;
    let mut buf_off = 0usize;
    let mut size = buf.len();
    let flags = flags | STREAM_RW_WRITE;
    while size > 0 {
        let out_sz = stream_p.write.expect("write")(
            self_in,
            unsafe { buf.as_ptr().add(buf_off) },
            size,
            &mut error,
        );
        if out_sz == 0 {
            break;
        }
        if out_sz == STREAM_ERROR {
            if is_nonblocking_error(error) && done != 0 {
                error = 0;
            }
            break;
        }
        if flags & STREAM_RW_ONCE != 0 {
            return obj::new_small_int(out_sz as isize);
        }
        buf_off += out_sz;
        size -= out_sz;
        done += out_sz;
    }
    if error != 0 {
        if is_nonblocking_error(error) {
            return obj::CONST_NONE;
        }
        stream_raise_error(self_in, error);
    }
    obj::new_small_int(done as isize)
}

/// `mp_stream_write_adaptor`
pub fn stream_write_adaptor(self_: *mut (), buf: &[u8]) {
    let _ = stream_write(obj::from_ptr(self_ as *const ()), buf, STREAM_RW_WRITE);
}

fn stream_readall(self_in: Obj) -> Obj {
    let stream_p = get_stream(self_in);
    let mut total_size = 0usize;
    let mut vstr = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init(&mut vstr, DEFAULT_BUFFER_SIZE);
    let mut p = vstr.buf;
    let mut current_read = DEFAULT_BUFFER_SIZE;
    loop {
        let mut error = 0;
        let out_sz = stream_p.read.expect("read")(self_in, p, current_read, &mut error);
        if out_sz == STREAM_ERROR {
            if is_nonblocking_error(error) {
                if total_size == 0 {
                    return obj::CONST_NONE;
                }
                break;
            }
            stream_raise_error(self_in, error);
        }
        if out_sz == 0 {
            break;
        }
        total_size += out_sz;
        if out_sz < current_read {
            current_read -= out_sz;
            p = unsafe { p.add(out_sz) };
        } else {
            p = vstr::extend(&mut vstr, DEFAULT_BUFFER_SIZE);
            current_read = DEFAULT_BUFFER_SIZE;
        }
    }
    vstr.len = total_size;
    if stream_p.is_text {
        objstr::new_str_from_vstr(&mut vstr)
    } else {
        objstr::new_bytes_from_vstr(&mut vstr)
    }
}

fn stream_read_generic(n_args: usize, args: &[Obj], flags: u8) -> Obj {
    if n_args == 1 || obj::get_int_truncated(args[1]) == -1 {
        return stream_readall(args[0]);
    }
    let sz = obj::get_int_truncated(args[1]) as usize;
    let stream_p = get_stream(args[0]);

    if mpconfig::PY_BUILTINS_STR_UNICODE && stream_p.is_text {
        let mut vstr = Vstr {
            alloc: 0,
            len: 0,
            buf: core::ptr::null_mut(),
            fixed_buf: false,
        };
        vstr::init(&mut vstr, sz);
        let mut more_bytes = sz;
        let mut last_buf_offset = 0usize;
        let mut chars_left = sz;
        while more_bytes > 0 {
            let p = vstr::add_len(&mut vstr, more_bytes);
            let mut error = 0;
            let slice = unsafe { std::slice::from_raw_parts_mut(p, more_bytes) };
            let out_sz = stream_read_exactly(args[0], slice, &mut error);
            if error != 0 {
                vstr::cut_tail_bytes(&mut vstr, more_bytes);
                if is_nonblocking_error(error) {
                    if vstr.len == 0 {
                        vstr::clear(&mut vstr);
                        return obj::CONST_NONE;
                    }
                    break;
                }
                stream_raise_error(args[0], error);
            }
            if out_sz < more_bytes {
                vstr::cut_tail_bytes(&mut vstr, more_bytes - out_sz);
                if out_sz == 0 {
                    break;
                }
            }
            let buf = unsafe { std::slice::from_raw_parts(vstr.buf, vstr.len) };
            let mut off = last_buf_offset;
            loop {
                if off >= buf.len() {
                    more_bytes = chars_left;
                    break;
                }
                let b = buf[off];
                let n = if !utf8_is_nonascii(b) {
                    1
                } else if b & 0xe0 == 0xc0 {
                    2
                } else if b & 0xf0 == 0xe0 {
                    3
                } else if b & 0xf8 == 0xf0 {
                    4
                } else {
                    5
                };
                if off + n <= buf.len() {
                    off += n;
                    chars_left -= 1;
                    last_buf_offset = off;
                    if off >= buf.len() {
                        more_bytes = chars_left;
                        break;
                    }
                } else {
                    more_bytes = (off + n - buf.len()) + (chars_left - 1);
                    break;
                }
            }
        }
        return objstr::new_str_from_vstr(&mut vstr);
    }

    let mut vstr = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init_len(&mut vstr, sz);
    let mut error = 0;
    let slice = unsafe { std::slice::from_raw_parts_mut(vstr.buf, sz) };
    let out_sz = stream_rw(args[0], slice, &mut error, flags);
    if error != 0 {
        vstr::clear(&mut vstr);
        if is_nonblocking_error(error) {
            return obj::CONST_NONE;
        }
        stream_raise_error(args[0], error);
    }
    vstr.len = out_sz;
    if stream_p.is_text {
        objstr::new_str_from_vstr(&mut vstr)
    } else {
        objstr::new_bytes_from_vstr(&mut vstr)
    }
}

fn stream_readinto_write_generic(n_args: usize, args: &[Obj], flags: u8) -> Obj {
    let mut bufinfo = BufferInfo {
        buf: core::ptr::null_mut(),
        len: 0,
        typecode: 0,
    };
    let buf_flags = if flags & STREAM_RW_WRITE != 0 {
        obj::BUFFER_READ
    } else {
        obj::BUFFER_WRITE
    };
    obj::get_buffer_raise(args[1], &mut bufinfo, buf_flags);

    let mut max_len = usize::MAX;
    let mut off = 0usize;
    if n_args == 3 {
        max_len = obj::get_int_truncated(args[2]) as usize;
    } else if n_args == 4 {
        off = obj::get_int_truncated(args[2]) as usize;
        max_len = obj::get_int_truncated(args[3]) as usize;
        if off > bufinfo.len {
            off = bufinfo.len;
        }
    }
    let avail = bufinfo.len - off;
    let slice = bufinfo.as_bytes();
    stream_write(args[0], &slice[off..min(avail, max_len)], flags)
}

fn stream_unbuffered_readline(n_args: usize, args: &[Obj]) -> Obj {
    let stream_p = get_stream(args[0]);
    let mut max_size = -1isize;
    if n_args > 1 {
        max_size = obj::small_int_value(args[1]);
    }

    let mut vstr = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    if max_size != -1 {
        vstr::init(&mut vstr, max_size as usize);
    } else {
        vstr::init(&mut vstr, 16);
    }

    let mut remaining = max_size;
    loop {
        if max_size != -1 {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
        }
        let p = vstr::add_len(&mut vstr, 1);
        let mut error = 0;
        let out_sz = stream_p.read.expect("read")(args[0], p, 1, &mut error);
        if out_sz == STREAM_ERROR {
            if is_nonblocking_error(error) {
                if vstr.len == 1 {
                    vstr::clear(&mut vstr);
                    return obj::CONST_NONE;
                }
                break;
            }
            stream_raise_error(args[0], error);
        }
        if out_sz == 0 {
            vstr::cut_tail_bytes(&mut vstr, 1);
            break;
        }
        if unsafe { *p } == b'\n' {
            break;
        }
    }

    if stream_p.is_text {
        objstr::new_str_from_vstr(&mut vstr)
    } else {
        objstr::new_bytes_from_vstr(&mut vstr)
    }
}

/// `mp_stream_unbuffered_iter`
pub fn stream_unbuffered_iter(self_in: Obj) -> Obj {
    let line = stream_unbuffered_readline(1, &[self_in]);
    if obj::is_true(line) {
        line
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

/// `mp_stream_close`
pub fn stream_close(stream: Obj) -> Obj {
    let stream_p = get_stream(stream);
    let mut error = 0;
    let res = stream_p.ioctl.expect("ioctl")(stream, STREAM_CLOSE, 0, &mut error);
    if res == STREAM_ERROR {
        stream_raise_error(stream, error);
    }
    obj::CONST_NONE
}

fn stream_seek_method(n_args: usize, args: &[Obj]) -> Obj {
    let offset = obj::get_int_truncated(args[1]);
    let mut whence = SEEK_SET as isize;
    if n_args == 3 {
        whence = obj::get_int_truncated(args[2]);
    }
    if whence == SEEK_SET as isize && offset < 0 {
        raise::raise(MpRaise::OSError(MP_EINVAL));
    }
    let mut error = 0;
    let res = stream_seek(args[0], offset as i64, whence as i32, &mut error);
    if res == -1 {
        stream_raise_error(args[0], error);
    }
    obj::new_small_int(res as isize)
}

fn stream_tell(self_in: Obj) -> Obj {
    stream_seek_method(
        3,
        &[
            self_in,
            obj::new_small_int(0),
            obj::new_small_int(SEEK_CUR as isize),
        ],
    )
}

fn stream_flush(self_in: Obj) -> Obj {
    let stream_p = get_stream(self_in);
    let mut error = 0;
    let res = stream_p.ioctl.expect("ioctl")(self_in, STREAM_FLUSH, 0, &mut error);
    if res == STREAM_ERROR {
        stream_raise_error(self_in, error);
    }
    obj::CONST_NONE
}

fn stream_ioctl(n_args: usize, args: &[Obj]) -> Obj {
    let mut val = 0usize;
    if n_args > 2 {
        let mut bufinfo = BufferInfo {
            buf: core::ptr::null_mut(),
            len: 0,
            typecode: 0,
        };
        if obj::get_buffer(args[2], &mut bufinfo, obj::BUFFER_WRITE) {
            val = bufinfo.buf as usize;
        } else {
            val = obj::get_int_truncated(args[2]) as usize;
        }
    }
    let stream_p = get_stream(args[0]);
    let mut error = 0;
    let request = obj::get_int_truncated(args[1]) as u32;
    let res = stream_p.ioctl.expect("ioctl")(args[0], request, val, &mut error);
    if res == STREAM_ERROR {
        stream_raise_error(args[0], error);
    }
    obj::new_small_int(res as isize)
}

// --- builtin method wrappers --------------------------------------------------

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

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];

static TYPE_FUN_BUILTIN_1: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_1_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_VAR_SLOTS.as_ptr() },
};

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n_args,
        n_kw,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n_args, args)
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun_builtin_1 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun_builtin_var(min_args: u8, max_args: u8, fun: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("fun_builtin_var alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn stream_read(n_args: usize, args: &[Obj]) -> Obj {
    stream_read_generic(n_args, args, STREAM_RW_READ)
}

fn stream_read1(n_args: usize, args: &[Obj]) -> Obj {
    stream_read_generic(n_args, args, STREAM_RW_READ | STREAM_RW_ONCE)
}

fn stream_write_method(n_args: usize, args: &[Obj]) -> Obj {
    stream_readinto_write_generic(n_args, args, STREAM_RW_WRITE)
}

fn stream_write1_method(n_args: usize, args: &[Obj]) -> Obj {
    stream_readinto_write_generic(n_args, args, STREAM_RW_WRITE | STREAM_RW_ONCE)
}

fn stream_readinto(n_args: usize, args: &[Obj]) -> Obj {
    stream_readinto_write_generic(n_args, args, STREAM_RW_READ)
}

fn stream_readinto1(n_args: usize, args: &[Obj]) -> Obj {
    stream_readinto_write_generic(n_args, args, STREAM_RW_READ | STREAM_RW_ONCE)
}

fn stream_unbuffered_readlines(self_in: Obj) -> Obj {
    let lines = objlist::new_list(0, None);
    loop {
        let line = stream_unbuffered_readline(1, &[self_in]);
        if !obj::is_true(line) {
            break;
        }
        objlist::list_append(lines, line);
    }
    lines
}

fn stream___exit__(n_args: usize, args: &[Obj]) -> Obj {
    let _ = n_args;
    stream_close(args[0])
}

pub fn stream_read_obj() -> Obj {
    new_fun_builtin_var(1, 2, stream_read)
}

pub fn stream_read1_obj() -> Obj {
    new_fun_builtin_var(1, 2, stream_read1)
}

pub fn stream_readinto_obj() -> Obj {
    new_fun_builtin_var(2, 3, stream_readinto)
}

pub fn stream_readinto1_obj() -> Obj {
    new_fun_builtin_var(2, 3, stream_readinto1)
}

pub fn stream_unbuffered_readline_obj() -> Obj {
    new_fun_builtin_var(1, 2, stream_unbuffered_readline)
}

pub fn stream_unbuffered_readlines_obj() -> Obj {
    new_fun_builtin_1(stream_unbuffered_readlines)
}

pub fn stream_write_obj() -> Obj {
    new_fun_builtin_var(2, 4, stream_write_method)
}

pub fn stream_write1_obj() -> Obj {
    new_fun_builtin_var(2, 4, stream_write1_method)
}

pub fn stream_close_obj() -> Obj {
    new_fun_builtin_1(stream_close)
}

pub fn stream___exit___obj() -> Obj {
    new_fun_builtin_var(4, 4, stream___exit__)
}

pub fn stream_seek_obj() -> Obj {
    new_fun_builtin_var(2, 3, stream_seek_method)
}

pub fn stream_tell_obj() -> Obj {
    new_fun_builtin_1(stream_tell)
}

pub fn stream_flush_obj() -> Obj {
    new_fun_builtin_1(stream_flush)
}

pub fn stream_ioctl_obj() -> Obj {
    new_fun_builtin_var(2, 3, stream_ioctl)
}

#[cfg(unix)]
pub fn stream_posix_write(stream: *mut (), buf: &[u8]) -> isize {
    let o = obj::from_ptr(stream as *const ());
    let stream_p = get_stream(o);
    let mut errno = 0;
    let out_sz = stream_p.write.expect("write")(o, buf.as_ptr(), buf.len(), &mut errno);
    if out_sz == STREAM_ERROR {
        -1
    } else {
        out_sz as isize
    }
}

#[cfg(unix)]
pub fn stream_posix_read(stream: *mut (), buf: &mut [u8]) -> isize {
    let o = obj::from_ptr(stream as *const ());
    let stream_p = get_stream(o);
    let mut errno = 0;
    let out_sz = stream_p.read.expect("read")(o, buf.as_mut_ptr(), buf.len(), &mut errno);
    if out_sz == STREAM_ERROR {
        -1
    } else {
        out_sz as isize
    }
}

#[cfg(unix)]
pub fn stream_posix_lseek(stream: *mut (), offset: i64, whence: i32) -> i64 {
    let o = obj::from_ptr(stream as *const ());
    let mut errno = 0;
    let res = stream_seek(o, offset, whence, &mut errno);
    if res == -1 {
        -1
    } else {
        res
    }
}

#[cfg(unix)]
pub fn stream_posix_fsync(stream: *mut ()) -> i32 {
    let o = obj::from_ptr(stream as *const ());
    let stream_p = get_stream(o);
    let mut errno = 0;
    let res = stream_p.ioctl.expect("ioctl")(o, STREAM_FLUSH, 0, &mut errno);
    if res == STREAM_ERROR {
        -1
    } else {
        res as i32
    }
}
