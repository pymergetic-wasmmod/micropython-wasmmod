//! rewrite of extmod/modbinascii.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objint;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
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
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("binascii fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("binascii fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn sextet(ch: u8) -> i32 {
    match ch {
        b'A'..=b'Z' => (ch - b'A') as i32,
        b'a'..=b'z' => (ch - b'a' + 26) as i32,
        b'0'..=b'9' => (ch - b'0' + 52) as i32,
        b'+' => 62,
        b'/' => 63,
        _ => -1,
    }
}

fn get_buf(o: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    obj::get_buffer_raise(o, &mut info, obj::BUFFER_READ);
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

// uzlib CRC32 nibble table (`lib/uzlib/crc32.c`).
const CRC32_TAB: [u32; 16] = [
    0x0000_0000, 0x1db7_1064, 0x3b6e_20c8, 0x26d9_30ac, 0x76dc_4190, 0x6b6b_51f4, 0x4db2_6158,
    0x5005_713c, 0xedb8_8320, 0xf00f_9344, 0xd6d6_a3e8, 0xcb61_b38c, 0x9b64_c2b0, 0x86d3_d2d4,
    0xa00a_e278, 0xbdbd_f21c,
];

fn uzlib_crc32(data: &[u8], mut crc: u32) -> u32 {
    for &b in data {
        crc ^= u32::from(b);
        crc = CRC32_TAB[(crc & 0x0f) as usize] ^ (crc >> 4);
        crc = CRC32_TAB[(crc & 0x0f) as usize] ^ (crc >> 4);
    }
    crc
}

fn crc32(n_args: usize, args: &[Obj]) -> Obj {
    let data = get_buf(args[0]);
    let mut crc = if n_args > 1 {
        obj::get_int_truncated(args[1]) as u32
    } else {
        0
    };
    crc = uzlib_crc32(&data, crc ^ 0xffff_ffff);
    objint::new_int_from_uint((crc ^ 0xffff_ffff) as py_rs::obj::Uint)
}

fn a2b_base64(data: Obj) -> Obj {
    let buf = get_buf(data);
    let mut out = Vec::with_capacity(buf.len() * 3 / 4 + 1);
    let mut shift: u32 = 0;
    let mut nbits = 0i32;
    let mut hadpad = false;
    for &b in &buf {
        if b == b'=' {
            if nbits == 2 || (nbits == 4 && hadpad) {
                nbits = 0;
                break;
            }
            hadpad = true;
            continue;
        }
        let s = sextet(b);
        if s < 0 {
            continue;
        }
        hadpad = false;
        shift = (shift << 6) | s as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((shift >> nbits) as u8);
        }
    }
    if nbits != 0 {
        raise::raise(MpRaise::ValueError("incorrect padding"));
    }
    objstr::new_bytes(&out)
}

fn b2a_base64(n: usize, args: &[Obj]) -> Obj {
    let newline = if n > 1 { obj::is_true(args[1]) } else { true };
    let buf = get_buf(args[0]);
    let base_len = if buf.is_empty() {
        0
    } else {
        ((buf.len() - 1) / 3 + 1) * 4
    };
    let mut v = vec![0u8; base_len + if newline { 1 } else { 0 }];
    let mut out_idx = 0usize;
    let mut i = buf.len();
    let mut inp = 0usize;
    while i >= 3 {
        v[out_idx] = (buf[inp] & 0xfc) >> 2;
        v[out_idx + 1] = (buf[inp] & 0x03) << 4 | (buf[inp + 1] & 0xf0) >> 4;
        v[out_idx + 2] = (buf[inp + 1] & 0x0f) << 2 | (buf[inp + 2] & 0xc0) >> 6;
        v[out_idx + 3] = buf[inp + 2] & 0x3f;
        out_idx += 4;
        inp += 3;
        i -= 3;
    }
    if i != 0 {
        v[out_idx] = (buf[inp] & 0xfc) >> 2;
        if i == 2 {
            v[out_idx + 1] = (buf[inp] & 0x03) << 4 | (buf[inp + 1] & 0xf0) >> 4;
            v[out_idx + 2] = (buf[inp + 1] & 0x0f) << 2;
        } else {
            v[out_idx + 1] = (buf[inp] & 0x03) << 4;
            v[out_idx + 2] = 64;
        }
        v[out_idx + 3] = 64;
    }
    for b in &mut v[..base_len] {
        *b = match *b {
            0..=25 => b'A' + *b,
            26..=51 => b'a' + (*b - 26),
            52..=61 => b'0' + (*b - 52),
            62 => b'+',
            63 => b'/',
            _ => b'=',
        };
    }
    if newline {
        v[base_len] = b'\n';
    }
    objstr::new_bytes(&v)
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_BINASCII {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("binascii")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("a2b_base64")),
            value: mk1(a2b_base64),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("b2a_base64")),
            value: mkv(1, 2, b2a_base64),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("hexlify")),
            value: mkv(1, 2, objstr::binascii_hexlify),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("unhexlify")),
            value: mkv(1, 1, objstr::binascii_unhexlify),
        },
    ];
    if mpconfig::PY_BINASCII_CRC32 && mpconfig::PY_DEFLATE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("crc32")),
            value: mkv(1, 2, crc32),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("binascii");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("binascii"), module);
    module
}
