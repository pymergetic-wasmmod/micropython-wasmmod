//! rewrite of extmod/modwebrepl.c
// symmetry: done

use std::sync::Mutex;

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::builtin;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_STREAM,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{
    self, StreamP, STREAM_CLOSE, STREAM_ERROR, STREAM_GET_DATA_OPTS, STREAM_OP_IOCTL,
    STREAM_OP_READ, STREAM_OP_WRITE, STREAM_SET_DATA_OPTS,
};

use crate::modwebsocket::FRAME_BIN;

const PUT_FILE: u8 = 1;
const GET_FILE: u8 = 2;
const GET_VER: u8 = 3;

const STATE_PASSWD: u8 = 0;
const STATE_NORMAL: u8 = 1;

const STREAM_RETRY: usize = usize::MAX - 1;

const PASSWD_PROMPT: &str = "Password: ";
const CONNECTED_PROMPT: &str = "\r\nWebREPL connected\r\n>>> ";
const DENIED_PROMPT: &str = "\r\nAccess denied\r\n";

#[repr(C, packed)]
struct WebreplFile {
    sig: [u8; 2],
    op_type: u8,
    flags: u8,
    offset: u64,
    size: u32,
    fname_len: u16,
    fname: [u8; 64],
}

#[repr(C)]
struct ObjWebrepl {
    base: ObjBase,
    sock: Obj,
    state: u8,
    hdr_to_recv: usize,
    data_to_recv: u32,
    hdr: WebreplFile,
    cur_file: Obj,
    passwd_len: u8,
}

static WEBREPL_PASSWD: Mutex<[u8; 10]> = Mutex::new([0; 10]);

fn webrepl_ptr(o: Obj) -> *mut ObjWebrepl {
    obj::as_ptr(o) as *mut ObjWebrepl
}

fn write_webrepl(websock: Obj, buf: &[u8]) {
    let stream_p = stream::get_stream(websock);
    let mut err = 0;
    let old_opts =
        stream_p.ioctl.expect("ioctl")(websock, STREAM_SET_DATA_OPTS, FRAME_BIN as usize, &mut err);
    if let Some(write) = stream_p.write {
        write(websock, buf.as_ptr(), buf.len(), &mut err);
    }
    stream_p.ioctl.expect("ioctl")(websock, STREAM_SET_DATA_OPTS, old_opts, &mut err);
}

fn write_webrepl_str(websock: Obj, data: &[u8]) {
    let stream_p = stream::get_stream(websock);
    let mut err = 0;
    if let Some(write) = stream_p.write {
        write(websock, data.as_ptr(), data.len(), &mut err);
    }
}

fn write_webrepl_resp(websock: Obj, code: u16) {
    let buf = [b'W', b'B', (code & 0xff) as u8, (code >> 8) as u8];
    write_webrepl(websock, &buf);
}

fn check_file_op_finished(self_: &mut ObjWebrepl) {
    if self_.data_to_recv == 0 {
        stream::stream_close(self_.cur_file);
        self_.hdr_to_recv = core::mem::size_of::<WebreplFile>();
        write_webrepl_resp(self_.sock, 0);
    }
}

fn write_file_chunk(self_: &mut ObjWebrepl) -> usize {
    let stream_p = stream::get_stream(self_.cur_file);
    let mut readbuf = [0u8; 258];
    let mut err = 0;
    let out_sz =
        stream_p.read.expect("read")(self_.cur_file, readbuf[2..].as_mut_ptr(), 256, &mut err);
    if out_sz == STREAM_ERROR {
        return out_sz;
    }
    readbuf[0] = out_sz as u8;
    readbuf[1] = (out_sz >> 8) as u8;
    write_webrepl(self_.sock, &readbuf[..2 + out_sz]);
    out_sz
}

fn handle_op(self_: &mut ObjWebrepl) {
    match self_.hdr.op_type {
        GET_VER => {
            let ver = [
                mpconfig::VERSION_MAJOR as u8,
                mpconfig::VERSION_MINOR as u8,
                mpconfig::VERSION_MICRO as u8,
            ];
            write_webrepl(self_.sock, &ver);
            self_.hdr_to_recv = core::mem::size_of::<WebreplFile>();
            return;
        }
        _ => {}
    }

    let fname_end = self_
        .hdr
        .fname
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(self_.hdr.fname.len());
    let fname = std::str::from_utf8(&self_.hdr.fname[..fname_end]).unwrap_or("");
    let fname_obj = objstr::new_str(fname.as_bytes());
    let mode = if self_.hdr.op_type == PUT_FILE {
        obj::new_qstr(qstr::from_str("wb"))
    } else {
        obj::new_qstr(qstr::from_str("rb"))
    };
    self_.cur_file = builtin::builtin_open(2, &[fname_obj, mode], None);
    write_webrepl_resp(self_.sock, 0);

    if self_.hdr.op_type == PUT_FILE {
        self_.data_to_recv = self_.hdr.size;
        check_file_op_finished(self_);
    } else if self_.hdr.op_type == GET_FILE {
        self_.data_to_recv = 1;
    }
}

fn webrepl_read_inner(self_in: Obj, buf: *mut u8, _size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *webrepl_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    let sock_stream = stream::get_stream(self_.sock);
    let out_sz = sock_stream.read.expect("read")(self_.sock, buf, 1, errcode);
    if out_sz == 0 || out_sz == STREAM_ERROR {
        return out_sz;
    }

    if self_.state == STATE_PASSWD {
        let c = unsafe { *buf };
        if c == b'\r' || c == b'\n' {
            let passwd = WEBREPL_PASSWD.lock().unwrap();
            let entered =
                std::str::from_utf8(&self_.hdr.fname[..self_.passwd_len as usize]).unwrap_or("");
            let stored = std::str::from_utf8(
                &passwd[..passwd.iter().position(|&b| b == 0).unwrap_or(passwd.len())],
            )
            .unwrap_or("");
            if entered != stored {
                write_webrepl_str(self_.sock, DENIED_PROMPT.as_bytes());
                return 0;
            }
            self_.state = STATE_NORMAL;
            self_.data_to_recv = 0;
            write_webrepl_str(self_.sock, CONNECTED_PROMPT.as_bytes());
        } else if self_.passwd_len < 10 {
            self_.hdr.fname[self_.passwd_len as usize] = c;
            self_.passwd_len += 1;
        }
        return STREAM_RETRY;
    }

    let mut err = 0;
    if sock_stream.ioctl.expect("ioctl")(self_.sock, STREAM_GET_DATA_OPTS, 0, &mut err) == 1 {
        return out_sz;
    }

    if self_.hdr_to_recv != 0 {
        let hdr_size = core::mem::size_of::<WebreplFile>();
        let filled = hdr_size - self_.hdr_to_recv;
        let p = unsafe { (self_ as *mut ObjWebrepl as *mut u8).add(filled) };
        unsafe {
            *p = *buf;
        }
        self_.hdr_to_recv -= 1;
        if self_.hdr_to_recv != 0 {
            let mut errcode = 0;
            let p = unsafe { (self_ as *mut ObjWebrepl as *mut u8).add(filled + 1) };
            let hdr_sz =
                sock_stream.read.expect("read")(self_.sock, p, self_.hdr_to_recv, &mut errcode);
            if hdr_sz == STREAM_ERROR {
                return hdr_sz;
            }
            self_.hdr_to_recv -= hdr_sz;
            if self_.hdr_to_recv != 0 {
                return STREAM_RETRY;
            }
        }
        handle_op(self_);
        return STREAM_RETRY;
    }

    if self_.data_to_recv != 0 {
        let mut filebuf = [0u8; 512];
        filebuf[0] = unsafe { *buf };
        let mut buf_sz = 1usize;
        self_.data_to_recv -= 1;
        if self_.data_to_recv != 0 {
            let to_read = core::cmp::min(filebuf.len() - 1, self_.data_to_recv as usize);
            let mut errcode = 0;
            let sz = sock_stream.read.expect("read")(
                self_.sock,
                filebuf[1..].as_mut_ptr(),
                to_read,
                &mut errcode,
            );
            if sz == STREAM_ERROR {
                return sz;
            }
            self_.data_to_recv -= sz as u32;
            buf_sz += sz;
        }

        if self_.hdr.op_type == PUT_FILE {
            let mut err = 0;
            stream::stream_write_exactly(self_.cur_file, &mut filebuf[..buf_sz], &mut err);
        } else if self_.hdr.op_type == GET_FILE {
            let out_sz = write_file_chunk(self_);
            if out_sz != 0 {
                self_.data_to_recv = 1;
            }
        }
        check_file_op_finished(self_);
    }

    STREAM_RETRY
}

fn webrepl_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    loop {
        let out_sz = webrepl_read_inner(self_in, buf, size, errcode);
        if out_sz != STREAM_RETRY {
            return out_sz;
        }
    }
}

fn webrepl_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*webrepl_ptr(self_in) };
    if self_.state == STATE_PASSWD {
        return size;
    }
    let stream_p = stream::get_stream(self_.sock);
    stream_p.write.expect("write")(self_in, buf, size, errcode)
}

fn webrepl_ioctl(self_in: Obj, request: u32, _arg: usize, errcode: *mut i32) -> usize {
    match request {
        STREAM_CLOSE => {
            let self_ = unsafe { &*webrepl_ptr(self_in) };
            stream::stream_close(self_.sock);
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

static WEBREPL_STREAM: StreamP = StreamP {
    read: Some(webrepl_read),
    write: Some(webrepl_write),
    ioctl: Some(webrepl_ioctl),
    is_text: false,
};

fn webrepl_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 2, false);
    stream::get_stream_raise(args[0], STREAM_OP_READ | STREAM_OP_WRITE | STREAM_OP_IOCTL);
    let o = malloc::new_obj::<ObjWebrepl>().expect("webrepl");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).sock = args[0];
        (*o).hdr_to_recv = core::mem::size_of::<WebreplFile>();
        (*o).data_to_recv = 0;
        (*o).state = STATE_PASSWD;
        (*o).cur_file = obj::OBJ_NULL;
        (*o).passwd_len = 0;
        (*o).hdr = WebreplFile {
            sig: [b'W', b'B'],
            op_type: 0,
            flags: 0,
            offset: 0,
            size: 0,
            fname_len: 0,
            fname: [0; 64],
        };
        write_webrepl_str(args[0], PASSWD_PROMPT.as_bytes());
        obj::from_ptr(o as *const ObjWebrepl as *const ())
    }
}

fn webrepl_set_password(passwd_in: Obj) -> Obj {
    let s = objstr::str_get_str(passwd_in);
    if s.len() > 9 {
        raise::raise(MpRaise::ValueError(""));
    }
    let mut buf = [0u8; 10];
    buf[..s.len()].copy_from_slice(s.as_bytes());
    *WEBREPL_PASSWD.lock().unwrap() = buf;
    obj::CONST_NONE
}

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("webrepl fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn locals_dict() -> *const () {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    static mut DICT: *const () = core::ptr::null();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: stream::stream_read_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: stream::stream_readinto_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
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

static mut WEBREPL_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_WEBREPL: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_STREAM,
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
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { WEBREPL_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_webrepl() -> &'static ObjType {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            WEBREPL_SLOTS[0] = webrepl_make_new as *const ();
            WEBREPL_SLOTS[1] = &raw const WEBREPL_STREAM as *const StreamP as *const ();
            WEBREPL_SLOTS[2] = dict;
            TYPE_WEBREPL.name = qstr::from_str("_webrepl");
        }
    });
    unsafe { &TYPE_WEBREPL }
}

/// Register built-in `_webrepl` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_WEBSOCKET {
        return obj::OBJ_NULL;
    }
    type_webrepl();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("_webrepl")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("_webrepl")),
            value: obj::from_ptr(type_webrepl() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("password")),
            value: mk1(webrepl_set_password),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("webrepl module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("_webrepl"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modwebsocket;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::objstringio;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
        let _ = modwebsocket::init_module();
        let _ = init_module();
    }

    fn bytesio(initial: &[u8]) -> Obj {
        objstringio::type_bytesio();
        let make_new =
            obj::type_get_make_new(objstringio::type_bytesio()).expect("bytesio make_new");
        if initial.is_empty() {
            make_new(objstringio::type_bytesio(), 0, 0, &[])
        } else {
            make_new(
                objstringio::type_bytesio(),
                1,
                0,
                &[objstr::new_bytes(initial)],
            )
        }
    }

    #[test]
    fn module_registers_when_enabled() {
        setup();
        let m = init_module();
        assert_ne!(m, obj::OBJ_NULL);
    }

    #[test]
    fn make_new_prompts_for_password() {
        setup();
        let bio = bytesio(&[]);
        let ws = webrepl_make_new(type_webrepl(), 1, 0, &[bio]);
        assert_ne!(ws, obj::OBJ_NULL);
    }
}
