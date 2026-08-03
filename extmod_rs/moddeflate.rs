//! rewrite of extmod/moddeflate.c
// symmetry: done

use std::io::{self, BufRead, Read};

use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN, TYPE_FLAG_ITER_IS_STREAM};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::stream::{self, StreamP, STREAM_CLOSE, STREAM_ERROR, STREAM_OP_READ};

const FORMAT_AUTO: u8 = 0;
const FORMAT_RAW: u8 = 1;
const FORMAT_ZLIB: u8 = 2;
const FORMAT_GZIP: u8 = 3;
const DEFAULT_WBITS: u8 = 8;

enum InflateDecoder {
    Raw(DeflateDecoder<io::BufReader<MpStreamReader>>),
    Zlib(ZlibDecoder<io::BufReader<MpStreamReader>>),
    Gzip(GzDecoder<io::BufReader<MpStreamReader>>),
}

impl Read for InflateDecoder {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Raw(d) => d.read(buf),
            Self::Zlib(d) => d.read(buf),
            Self::Gzip(d) => d.read(buf),
        }
    }
}

struct ReadState {
    decoder: InflateDecoder,
    eof: bool,
}

#[repr(C)]
struct ObjDeflateIO {
    base: ObjBase,
    stream: Obj,
    format: u8,
    window_bits: u8,
    close: bool,
    read: Option<ReadState>,
}

struct MpStreamReader {
    stream: Obj,
}

impl Read for MpStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let stream_p = stream::get_stream_raise(self.stream, STREAM_OP_READ);
        let read = stream_p.read.expect("deflate read");
        let mut err = 0;
        let n = read(self.stream, buf.as_mut_ptr(), buf.len(), &mut err);
        if n == STREAM_ERROR {
            return Err(io::Error::from_raw_os_error(err));
        }
        Ok(n)
    }
}

fn is_gzip_magic(b0: u8, b1: u8) -> bool {
    b0 == 0x1f && b1 == 0x8b
}

fn is_zlib_header(b0: u8, b1: u8) -> bool {
    (b0 & 0x0f) == 0x08 && ((b0 as u16) * 256 + b1 as u16) % 31 == 0
}

fn make_decoder(
    format: u8,
    window_bits: u8,
    reader: io::BufReader<MpStreamReader>,
) -> Result<InflateDecoder, ()> {
    match format {
        FORMAT_RAW => {
            let wbits = if window_bits == 0 {
                DEFAULT_WBITS
            } else {
                window_bits
            };
            Ok(InflateDecoder::Raw(DeflateDecoder::new(reader)))
        }
        FORMAT_ZLIB => Ok(InflateDecoder::Zlib(ZlibDecoder::new(reader))),
        FORMAT_GZIP => Ok(InflateDecoder::Gzip(GzDecoder::new(reader))),
        FORMAT_AUTO => {
            let mut peek = reader;
            let header = match peek.fill_buf() {
                Ok(b) if b.len() >= 2 => [b[0], b[1]],
                _ => return Err(()),
            };
            if is_gzip_magic(header[0], header[1]) {
                Ok(InflateDecoder::Gzip(GzDecoder::new(peek)))
            } else if is_zlib_header(header[0], header[1]) {
                Ok(InflateDecoder::Zlib(ZlibDecoder::new(peek)))
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}

fn init_read(self_: &mut ObjDeflateIO) -> bool {
    if self_.read.is_some() {
        return true;
    }
    if self_.stream == obj::OBJ_NULL {
        return false;
    }
    let _ = stream::get_stream_raise(self_.stream, STREAM_OP_READ);
    let reader = io::BufReader::new(MpStreamReader {
        stream: self_.stream,
    });
    let format = if self_.format == FORMAT_AUTO {
        FORMAT_AUTO
    } else {
        self_.format
    };
    match make_decoder(format, self_.window_bits, reader) {
        Ok(decoder) => {
            self_.read = Some(ReadState {
                decoder,
                eof: false,
            });
            true
        }
        Err(()) => false,
    }
}

fn deflateio_ptr(o: Obj) -> *mut ObjDeflateIO {
    obj::as_ptr(o) as *mut ObjDeflateIO
}

fn deflateio_read(self_in: Obj, buf: *mut u8, size: usize, errcode: *mut i32) -> usize {
    let self_ = unsafe { &mut *deflateio_ptr(self_in) };
    unsafe {
        *errcode = 0;
    }
    if self_.stream == obj::OBJ_NULL || !init_read(self_) {
        unsafe {
            *errcode = 22;
        }
        return STREAM_ERROR;
    }
    let read = self_.read.as_mut().unwrap();
    if read.eof {
        return 0;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, size) };
    match read.decoder.read(slice) {
        Ok(0) => {
            read.eof = true;
            0
        }
        Ok(n) => n,
        Err(e) => {
            unsafe {
                *errcode = e.raw_os_error().unwrap_or(22);
            }
            STREAM_ERROR
        }
    }
}

fn deflateio_ioctl(self_in: Obj, request: u32, _arg: usize, errcode: *mut i32) -> usize {
    if request != STREAM_CLOSE {
        unsafe {
            *errcode = 22;
        }
        return STREAM_ERROR;
    }
    let self_ = unsafe { &mut *deflateio_ptr(self_in) };
    if self_.stream != obj::OBJ_NULL {
        if self_.close {
            stream::stream_close(self_.stream);
        }
        self_.stream = obj::OBJ_NULL;
    }
    self_.read = None;
    0
}

static DEFLATEIO_STREAM: StreamP = StreamP {
    read: Some(deflateio_read),
    write: None,
    ioctl: Some(deflateio_ioctl),
    is_text: false,
};

type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
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
    slots: unsafe { F1.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("deflate fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn deflateio_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let _ = self_in;
    mpprint::print_str(print, "<DeflateIO>");
}

fn deflateio_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 4, false);
    let format = if n_args > 1 {
        obj::get_int(args[1]) as u8
    } else {
        FORMAT_AUTO
    };
    let wbits = if n_args > 2 {
        obj::get_int(args[2]) as u8
    } else {
        0
    };
    if format > FORMAT_GZIP {
        raise::raise(MpRaise::ValueError("format"));
    }
    if wbits != 0 && (wbits < 5 || wbits > 15) {
        raise::raise(MpRaise::ValueError("wbits"));
    }
    let close = n_args > 3 && obj::is_true(args[3]);
    let o = malloc::new_obj::<ObjDeflateIO>().expect("DeflateIO");
    unsafe {
        (*o).base.type_ = type_deflateio();
        (*o).stream = args[0];
        (*o).format = format;
        (*o).window_bits = wbits;
        (*o).close = close;
        (*o).read = None;
        obj::from_ptr(o as *const ObjDeflateIO as *const ())
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
                key: obj::new_qstr(qstr::from_str("close")),
                value: stream::stream_close_obj(),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__enter__")),
                value: mk1(|o| o),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("__exit__")),
                value: stream::stream___exit___obj(),
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

static mut DEFLATEIO_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut TYPE_DEFLATEIO: ObjType = ObjType {
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
    slots: unsafe { DEFLATEIO_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn type_deflateio() -> &'static ObjType {
    TYPE_INIT.get_or_init(|| {
        let dict = locals_dict();
        unsafe {
            DEFLATEIO_SLOTS[0] = deflateio_make_new as *const ();
            DEFLATEIO_SLOTS[1] = deflateio_print as *const ();
            DEFLATEIO_SLOTS[2] = &DEFLATEIO_STREAM as *const StreamP as *const ();
            DEFLATEIO_SLOTS[3] = dict;
            TYPE_DEFLATEIO.name = qstr::from_str("DeflateIO");
        }
    });
    unsafe { &TYPE_DEFLATEIO }
}

/// Register built-in `deflate` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_DEFLATE {
        return obj::OBJ_NULL;
    }
    type_deflateio();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("deflate")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("DeflateIO")),
            value: obj::from_ptr(type_deflateio() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("AUTO")),
            value: obj::new_small_int(FORMAT_AUTO as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("RAW")),
            value: obj::new_small_int(FORMAT_RAW as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("ZLIB")),
            value: obj::new_small_int(FORMAT_ZLIB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("GZIP")),
            value: obj::new_small_int(FORMAT_GZIP as isize),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("deflate module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("deflate"), module);
    module
}
