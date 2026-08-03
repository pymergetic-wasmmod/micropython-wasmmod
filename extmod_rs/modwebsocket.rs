//! rewrite of extmod/modwebsocket.c
// symmetry: done

use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mperrno::{EAGAIN, EIO, EINVAL, ENOBUFS};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_ITER_IS_STREAM};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::runtime;
use py_rs::stream::{
    self, StreamP, STREAM_CLOSE, STREAM_ERROR, STREAM_GET_DATA_OPTS, STREAM_OP_IOCTL,
    STREAM_OP_READ, STREAM_OP_WRITE, STREAM_SET_DATA_OPTS,
};

pub const FRAME_OPCODE_MASK: u8 = 0x0f;

pub const FRAME_CONT: u8 = 0;
pub const FRAME_TXT: u8 = 1;
pub const FRAME_BIN: u8 = 2;
pub const FRAME_CLOSE: u8 = 0x8;
pub const FRAME_PING: u8 = 0x9;
pub const FRAME_PONG: u8 = 0xa;

const FRAME_HEADER: u8 = 0;
const FRAME_OPT: u8 = 1;
const PAYLOAD: u8 = 2;
const CONTROL: u8 = 3;

const BLOCKING_WRITE: u8 = 0x80;

#[repr(C)]
struct ObjWebsocket {
    base: ObjBase,
    sock: Obj,
    msg_sz: u32,
    mask: [u8; 4],
    state: u8,
    to_recv: u8,
    mask_pos: u8,
    buf_pos: u8,
    buf: [u8; 6],
    opts: u8,
    ws_flags: u8,
    last_flags: u8,
}

fn websocket_ptr(o: Obj) -> *mut ObjWebsocket {
    obj::as_ptr(o) as *mut ObjWebsocket
}

fn set_stream_blocking(sock: Obj, blocking: bool) {
    let mut dest = [obj::OBJ_NULL; 3];
    runtime::load_method(sock, qstr::from_str("setblocking"), &mut dest[..2].try_into().unwrap());
    dest[2] = obj::new_bool(blocking);
    runtime::call_method_n_kw(1, 0, &dest);
}

fn websocket_write_raw(
    self_in: Obj,
    header: &[u8],
    buf: &[u8],
    errcode: &mut i32,
) -> usize {
    let self_ = unsafe { &mut *websocket_ptr(self_in) };
    if self_.opts & BLOCKING_WRITE != 0 {
        set_stream_blocking(self_.sock, true);
    }

    let mut hdr = header.to_vec();
    let mut out_sz = stream::stream_write_exactly(self_.sock, &mut hdr, errcode);
    if *errcode == 0 && !buf.is_empty() {
        let mut payload = buf.to_vec();
        out_sz = stream::stream_write_exactly(self_.sock, &mut payload, errcode);
    }

    if self_.opts & BLOCKING_WRITE != 0 {
        set_stream_blocking(self_.sock, false);
    }

    if *errcode != 0 {
        STREAM_ERROR
    } else {
        out_sz
    }
}

fn websocket_write(self_in: Obj, buf: *const u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &*websocket_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if size >= 0x10000 {
        unsafe {
            *errcode = ENOBUFS;
        }
        return STREAM_ERROR;
    }
    let mut header = [0u8; 4];
    header[0] = 0x80 | (self_.opts & FRAME_OPCODE_MASK);
    let hdr_sz = if size < 126 {
        header[1] = size as u8;
        2
    } else {
        header[1] = 126;
        header[2] = (size >> 8) as u8;
        header[3] = (size & 0xff) as u8;
        4
    };
    let payload = unsafe { std::slice::from_raw_parts(buf, size) };
    let mut err = 0;
    websocket_write_raw(self_in, &header[..hdr_sz], payload, &mut err)
}

fn websocket_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *websocket_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    let stream_p = stream::get_stream(self_.sock);
    let read = stream_p.read.expect("websocket read");

    loop {
        if self_.to_recv != 0 {
            let out_sz = read(
                self_.sock,
                unsafe {
                    self_.buf
                        .as_mut_ptr()
                        .add(self_.buf_pos as usize)
                },
                self_.to_recv as usize,
                errcode,
            );
            if out_sz == 0 || out_sz == STREAM_ERROR {
                return out_sz;
            }
            self_.buf_pos += out_sz as u8;
            self_.to_recv -= out_sz as u8;
            if self_.to_recv != 0 {
                unsafe {
                    *errcode = EAGAIN;
                }
                return STREAM_ERROR;
            }
        }

        match self_.state {
            FRAME_HEADER => {
                let frame_type = self_.buf[0];
                self_.last_flags = frame_type;
                let opcode = frame_type & FRAME_OPCODE_MASK;

                if (self_.buf[0] & FRAME_OPCODE_MASK) == FRAME_CONT {
                    self_.ws_flags =
                        (self_.ws_flags & FRAME_OPCODE_MASK) | (self_.buf[0] & !FRAME_OPCODE_MASK);
                } else {
                    self_.ws_flags = self_.buf[0];
                }

                self_.mask = [0; 4];

                let mut to_recv = 0u8;
                let sz = (self_.buf[1] & 0x7f) as u32;
                if sz == 126 {
                    to_recv += 2;
                } else if sz == 127 {
                    stream::stream_close(self_.sock);
                    unsafe {
                        *errcode = EIO;
                    }
                    return STREAM_ERROR;
                }
                if self_.buf[1] & 0x80 != 0 {
                    to_recv += 4;
                }
                self_.buf_pos = 0;
                self_.to_recv = to_recv;
                self_.msg_sz = sz;
                if to_recv != 0 {
                    self_.state = FRAME_OPT;
                } else if opcode >= FRAME_CLOSE {
                    self_.state = CONTROL;
                } else {
                    self_.state = PAYLOAD;
                }
                continue;
            }

            FRAME_OPT => {
                if self_.buf_pos & 2 != 0 {
                    debug_assert!(self_.buf_pos == 2 || self_.buf_pos == 6);
                    self_.msg_sz =
                        ((self_.buf[0] as u32) << 8) | (self_.buf[1] as u32);
                }
                if self_.buf_pos & 4 != 0 {
                    self_.mask.copy_from_slice(
                        &self_.buf[(self_.buf_pos as usize - 4)..(self_.buf_pos as usize)],
                    );
                }
                self_.buf_pos = 0;
                if (self_.last_flags & FRAME_OPCODE_MASK) >= FRAME_CLOSE {
                    self_.state = CONTROL;
                } else {
                    self_.state = PAYLOAD;
                }
                continue;
            }

            PAYLOAD | CONTROL => {
                if self_.msg_sz == 0 {
                    let last_state = self_.state;
                    self_.state = FRAME_HEADER;
                    self_.to_recv = 2;
                    self_.mask_pos = 0;
                    self_.buf_pos = 0;

                    if last_state == CONTROL {
                        let frame_type = self_.last_flags & FRAME_OPCODE_MASK;
                        if frame_type == FRAME_CLOSE {
                            let close_resp: [u8; 2] = [0x88, 0];
                            let mut err = 0;
                            websocket_write_raw(
                                self_in,
                                &close_resp,
                                &[],
                                &mut err,
                            );
                            return 0;
                        }
                        continue;
                    }
                    continue;
                }

                let sz = size.min(self_.msg_sz as usize);
                let out_sz = read(self_.sock, buf, sz, errcode);
                if out_sz == 0 || out_sz == STREAM_ERROR {
                    return out_sz;
                }

                let mut sz = out_sz;
                let mut p = buf;
                while sz > 0 {
                    unsafe {
                        *p ^= self_.mask[(self_.mask_pos & 3) as usize];
                        p = p.add(1);
                    }
                    self_.mask_pos = self_.mask_pos.wrapping_add(1);
                    sz -= 1;
                }

                self_.msg_sz -= out_sz as u32;
                if self_.msg_sz == 0 {
                    let last_state = self_.state;
                    self_.state = FRAME_HEADER;
                    self_.to_recv = 2;
                    self_.mask_pos = 0;
                    self_.buf_pos = 0;

                    if last_state == CONTROL {
                        let frame_type = self_.last_flags & FRAME_OPCODE_MASK;
                        if frame_type == FRAME_CLOSE {
                            let close_resp: [u8; 2] = [0x88, 0];
                            let mut err = 0;
                            websocket_write_raw(
                                self_in,
                                &close_resp,
                                &[],
                                &mut err,
                            );
                            return 0;
                        }
                        continue;
                    }
                }

                if out_sz != 0 {
                    return out_sz;
                }
                continue;
            }

            _ => unreachable!(),
        }
    }
}

fn websocket_ioctl(self_in: Obj, request: u32, arg: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *websocket_ptr(self_in) };
    match request {
        STREAM_CLOSE => {
            stream::stream_close(self_.sock);
            0
        }
        STREAM_GET_DATA_OPTS => (self_.ws_flags & FRAME_OPCODE_MASK) as usize,
        STREAM_SET_DATA_OPTS => {
            let cur = self_.opts & FRAME_OPCODE_MASK;
            self_.opts = (self_.opts & !FRAME_OPCODE_MASK) | ((arg as u8) & FRAME_OPCODE_MASK);
            cur as usize
        }
        _ => {
            unsafe {
                *errcode = EINVAL;
            }
            STREAM_ERROR
        }
    }
}

static WEBSOCKET_STREAM: StreamP = StreamP {
    read: Some(websocket_read),
    write: Some(websocket_write),
    ioctl: Some(websocket_ioctl),
    is_text: false,
};

fn websocket_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 2, false);
    let _ = stream::get_stream_raise(
        args[0],
        STREAM_OP_READ | STREAM_OP_WRITE | STREAM_OP_IOCTL,
    );
    let o = malloc::new_obj::<ObjWebsocket>().expect("websocket");
    unsafe {
        (*o).base.type_ = type_websocket();
        (*o).sock = args[0];
        (*o).state = FRAME_HEADER;
        (*o).to_recv = 2;
        (*o).mask_pos = 0;
        (*o).buf_pos = 0;
        (*o).opts = FRAME_TXT;
        if n_args > 1 && args[1] == obj::CONST_TRUE {
            (*o).opts |= BLOCKING_WRITE;
        }
        (*o).msg_sz = 0;
        (*o).mask = [0; 4];
        (*o).buf = [0; 6];
        (*o).ws_flags = 0;
        (*o).last_flags = 0;
        obj::from_ptr(o as *const ObjWebsocket as *const ())
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
                key: obj::new_qstr(qstr::from_str("readline")),
                value: stream::stream_unbuffered_readline_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: stream::stream_write_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ioctl")),
                value: stream::stream_ioctl_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("close")),
                value: stream::stream_close_obj(),
            },
        ];
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
    unsafe { DICT }
}

static mut WEBSOCKET_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_WEBSOCKET: ObjType = ObjType {
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
    slots: unsafe { WEBSOCKET_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_websocket() -> &'static ObjType {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            WEBSOCKET_SLOTS[0] = websocket_make_new as *const ();
            WEBSOCKET_SLOTS[1] = &WEBSOCKET_STREAM as *const StreamP as *const ();
            WEBSOCKET_SLOTS[2] = dict;
            TYPE_WEBSOCKET.name = qstr::from_str("websocket");
        }
    });
    unsafe { &TYPE_WEBSOCKET }
}

/// Register built-in `websocket` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_WEBSOCKET {
        return obj::OBJ_NULL;
    }
    type_websocket();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("websocket")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("websocket")),
            value: obj::from_ptr(type_websocket() as *const ObjType as *const ()),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("websocket module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("websocket"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::objstr;
    use py_rs::objstringio;
    use py_rs::stream::{self, SEEK_SET, STREAM_RW_READ, STREAM_RW_WRITE};

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
        let _ = init_module();
    }

    fn bytesio(initial: &[u8]) -> Obj {
        objstringio::type_bytesio();
        let make_new = obj::type_get_make_new(objstringio::type_bytesio()).expect("bytesio make_new");
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

    fn make_ws(sock: Obj) -> Obj {
        websocket_make_new(type_websocket(), 1, 0, &[sock])
    }

    fn stream_read_bytes(stream: Obj, sz: usize) -> Vec<u8> {
        let mut buf = vec![0u8; sz];
        let mut err = 0;
        let n = stream::stream_rw(stream, &mut buf, &mut err, STREAM_RW_READ);
        if n == stream::STREAM_ERROR {
            panic!("read failed: {err}");
        }
        buf.truncate(n);
        buf
    }

    fn ws_read(msg: &[u8], sz: usize) -> Vec<u8> {
        let ws = make_ws(bytesio(msg));
        stream_read_bytes(ws, sz)
    }

    fn ws_write(msg: &[u8], raw_sz: usize) -> Vec<u8> {
        let bio = bytesio(&[]);
        let ws = make_ws(bio);
        let mut payload = msg.to_vec();
        let mut err = 0;
        stream::stream_rw(ws, &mut payload, &mut err, STREAM_RW_WRITE);
        let mut err = 0;
        stream::stream_seek(bio, 0, SEEK_SET, &mut err);
        stream_read_bytes(bio, raw_sz)
    }

    #[test]
    fn module_registers_when_enabled() {
        setup();
        let m = init_module();
        assert_ne!(m, obj::OBJ_NULL);
    }

    #[test]
    fn read_basic_text_frame() {
        setup();
        assert_eq!(ws_read(b"\x81\x04ping", 4), b"ping");
    }

    #[test]
    fn write_basic_text_frame() {
        setup();
        assert_eq!(ws_write(b"pong", 6), b"\x81\x04pong");
    }

    #[test]
    fn read_masked_frames_and_ioctl() {
        setup();
        let bio = bytesio(b"\x81\x88maskmaskMASK");
        let ws = make_ws(bio);
        assert_eq!(stream_read_bytes(ws, 8), b"\x00\x00\x00\x00    ");

        let bio2 = bytesio(b"\x81\xfe\x00\x08maskmaskMASK");
        let ws2 = make_ws(bio2);
        assert_eq!(stream_read_bytes(ws2, 8), b"\x00\x00\x00\x00    ");

        let bio3 = bytesio(b"\x88\x00");
        let ws3 = make_ws(bio3);
        assert_eq!(stream_read_bytes(ws3, 1), b"");
        let mut err = 0;
        stream::stream_seek(bio3, 2, SEEK_SET, &mut err);
        assert_eq!(stream_read_bytes(bio3, 4), b"\x88\x00");

        let ws4 = make_ws(bytesio(&[]));
        assert_eq!(websocket_ioctl(ws4, STREAM_GET_DATA_OPTS, 0, &mut 0), 0);
        assert_eq!(websocket_ioctl(ws4, STREAM_SET_DATA_OPTS, 2, &mut 0), 1);
        assert_eq!(websocket_ioctl(ws4, STREAM_SET_DATA_OPTS, 0, &mut 0), 2);
        let mut err = 22;
        assert_eq!(websocket_ioctl(ws4, 999, 0, &mut err), STREAM_ERROR);
        assert_eq!(err, EINVAL);
    }

    #[test]
    fn write_rejects_oversized_payload() {
        setup();
        let ws = make_ws(bytesio(&[]));
        let huge = vec![b'x'; 0x10000];
        let mut err = 0;
        assert_eq!(
            websocket_write(ws, huge.as_ptr(), huge.len(), &mut err),
            STREAM_ERROR
        );
        assert_eq!(err, ENOBUFS);
    }
}
