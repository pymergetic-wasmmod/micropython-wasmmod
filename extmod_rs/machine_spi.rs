//! rewrite of extmod/machine_spi.c (SoftSPI bitbang + shared SPI protocol helpers)
//! Host-complete for `machine.SoftSPI`; HW `machine.SPI` needs MCU SPI controller HAL on port builds.
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::map::{self, Map, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, BufferInfo, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::vstr::{self, Vstr};

use crate::hal_pin;

const SPI_MSB: isize = 0;
const SPI_LSB: isize = 1;

type SpiInitFn = fn(Obj, usize, &[Obj], &Map);
type SpiDeinitFn = fn(Obj);
type SpiTransferFn = fn(Obj, usize, *const u8, *mut u8);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SpiProtocol {
    pub init: SpiInitFn,
    pub deinit: Option<SpiDeinitFn>,
    pub transfer: SpiTransferFn,
}

fn spi_protocol(o: Obj) -> Option<SpiProtocol> {
    let base = obj::as_ptr(o) as *const ObjBase;
    let type_ = unsafe { (*base).type_ };
    if type_.is_null() {
        return None;
    }
    let idx = unsafe { (*type_).slot_index_protocol };
    if idx == 0 {
        return None;
    }
    let slots = unsafe { (*type_).slots };
    if slots.is_null() {
        return None;
    }
    Some(unsafe { *(slots.add(idx as usize - 1) as *const SpiProtocol) })
}

fn spi_transfer(self_in: Obj, len: usize, src: *const u8, dest: *mut u8) {
    let p = spi_protocol(self_in).expect("SPI protocol");
    (p.transfer)(self_in, len, src, dest);
}

type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
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

static mut FK: [*const (); 1] = [call_kw as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];

static TK: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
static TV: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
static T2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    base: ObjBase { type_: core::ptr::null() },
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

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}

fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("spi fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("spi fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("spi fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("spi fn3");
    unsafe {
        (*o).base.type_ = &T3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}

fn spi_init(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let p = spi_protocol(args[0]).expect("SPI protocol");
    (p.init)(args[0], n - 1, &args[1..n], kw);
    obj::CONST_NONE
}

fn spi_deinit(self_in: Obj) -> Obj {
    if let Some(p) = spi_protocol(self_in) {
        if let Some(deinit) = p.deinit {
            deinit(self_in);
        }
    }
    obj::CONST_NONE
}

fn spi_read(n: usize, args: &[Obj]) -> Obj {
    let len = obj::get_int(args[1]) as usize;
    let fill = if n == 3 { obj::get_int(args[2]) as u8 } else { 0 };
    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init_len(&mut v, len);
    unsafe {
        core::ptr::write_bytes(v.buf, fill, len);
        spi_transfer(args[0], len, v.buf, v.buf);
    }
    objstr::new_bytes_from_vstr(&mut v)
}

fn spi_readinto(n: usize, args: &[Obj]) -> Obj {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_WRITE);
    let fill = if n == 3 { obj::get_int(args[2]) as u8 } else { 0 };
    unsafe {
        core::ptr::write_bytes(bufinfo.buf, fill, bufinfo.len);
        spi_transfer(args[0], bufinfo.len, bufinfo.buf, bufinfo.buf);
    }
    obj::CONST_NONE
}

fn spi_write(self_in: Obj, wr_buf: Obj) -> Obj {
    let mut src = BufferInfo::default();
    obj::get_buffer_raise(wr_buf, &mut src, obj::BUFFER_READ);
    unsafe {
        spi_transfer(self_in, src.len, src.buf as *const u8, core::ptr::null_mut());
    }
    obj::CONST_NONE
}

fn spi_write_readinto(self_in: Obj, wr_buf: Obj, rd_buf: Obj) -> Obj {
    let mut src = BufferInfo::default();
    obj::get_buffer_raise(wr_buf, &mut src, obj::BUFFER_READ);
    let mut dest = BufferInfo::default();
    obj::get_buffer_raise(rd_buf, &mut dest, obj::BUFFER_WRITE);
    if src.len != dest.len {
        raise::raise(MpRaise::ValueError("buffers must be the same length"));
    }
    unsafe {
        spi_transfer(self_in, src.len, src.buf as *const u8, dest.buf);
    }
    obj::CONST_NONE
}

fn spi_locals_dict() -> Obj {
    static mut DICT: Option<Obj> = None;
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, spi_init),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk2(|s, _| spi_deinit(s)),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("read")),
                value: mkv(2, 3, spi_read),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: mkv(2, 3, spi_readinto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: mk2(spi_write),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write_readinto")),
                value: mk3(spi_write_readinto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("MSB")),
                value: obj::new_small_int(SPI_MSB),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("LSB")),
                value: obj::new_small_int(SPI_LSB),
            },
        ];
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            DICT = Some(obj::from_ptr(ptr as *const ObjDict as *const ()));
        }
    });
    unsafe { DICT.unwrap() }
}

#[repr(C)]
struct SoftSpiBus {
    delay_half: u32,
    polarity: u8,
    phase: u8,
    firstbit: u8,
    sck: Obj,
    mosi: Obj,
    miso: Obj,
}

#[repr(C)]
struct SoftSpiObj {
    base: ObjBase,
    spi: SoftSpiBus,
}

fn soft_spi_ptr(o: Obj) -> *mut SoftSpiObj {
    obj::as_ptr(o) as *mut SoftSpiObj
}

fn baudrate_from_delay_half(delay_half: u32) -> u32 {
    500_000 / delay_half
}

fn baudrate_to_delay_half(baudrate: u32) -> u32 {
    let mut delay_half = 500_000 / baudrate;
    if 500_000 % baudrate != 0 {
        delay_half += 1;
    }
    delay_half
}

fn swap_bits(byte: u8) -> u8 {
    const SWAP: [u8; 16] = [
        0x00, 0x08, 0x04, 0x0c, 0x02, 0x0a, 0x06, 0x0e, 0x01, 0x09, 0x05, 0x0d, 0x03, 0x0b, 0x07,
        0x0f,
    ];
    (SWAP[(byte & 0x0f) as usize] << 4) | SWAP[(byte >> 4) as usize]
}

fn soft_spi_ioctl(spi: &mut SoftSpiBus) {
    hal_pin::pin_write(spi.sck, spi.polarity as i32);
    hal_pin::pin_output(spi.sck);
    hal_pin::pin_output(spi.mosi);
    hal_pin::pin_input(spi.miso);
}

fn soft_spi_transfer_impl(spi: &SoftSpiBus, len: usize, src: *const u8, dest: *mut u8) {
    let delay_half = spi.delay_half;
    for i in 0..len {
        let mut data_out = unsafe { *src.add(i) };
        if spi.firstbit as isize != SPI_MSB {
            data_out = swap_bits(data_out);
        }
        let mut data_in = 0u8;
        for _ in 0..8 {
            hal_pin::pin_write(spi.mosi, (data_out & 1) as i32);
            data_out >>= 1;
            if spi.phase == 0 {
                mphal::delay_us(delay_half as usize);
                hal_pin::pin_write(spi.sck, 1 - spi.polarity as i32);
            } else {
                hal_pin::pin_write(spi.sck, 1 - spi.polarity as i32);
                mphal::delay_us(delay_half as usize);
            }
            data_in = (data_in << 1) | (hal_pin::pin_read(spi.miso) as u8);
            if spi.phase == 0 {
                mphal::delay_us(delay_half as usize);
                hal_pin::pin_write(spi.sck, spi.polarity as i32);
            } else {
                hal_pin::pin_write(spi.sck, spi.polarity as i32);
                mphal::delay_us(delay_half as usize);
            }
        }
        if !dest.is_null() {
            let out = if spi.firstbit as isize == SPI_MSB {
                data_in
            } else {
                swap_bits(data_in)
            };
            unsafe {
                *dest.add(i) = out;
            }
        }
    }
}

fn soft_spi_init(self_in: Obj, n_pos: usize, pos: &[Obj], kw: &Map) {
    let self_ = unsafe { &mut *soft_spi_ptr(self_in) };
    let allowed = [
        Arg {
            qst: qstr::from_str("baudrate"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("polarity"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("phase"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("firstbit"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(-1),
        },
        Arg {
            qst: qstr::from_str("sck"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("mosi"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("miso"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
    ];
    let mut vals = [ArgVal::default(); 7];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);

    if let ArgVal::Int(v) = vals[0] {
        if v != -1 {
            self_.spi.delay_half = baudrate_to_delay_half(v as u32);
        }
    }
    if let ArgVal::Int(v) = vals[1] {
        if v != -1 {
            self_.spi.polarity = v as u8;
        }
    }
    if let ArgVal::Int(v) = vals[2] {
        if v != -1 {
            self_.spi.phase = v as u8;
        }
    }
    if let ArgVal::Int(v) = vals[3] {
        if v != -1 {
            self_.spi.firstbit = v as u8;
        }
    }
    if let ArgVal::Obj(v) = vals[4] {
        if v != obj::OBJ_NULL {
            self_.spi.sck = hal_pin::get_pin_obj(v);
        }
    }
    if let ArgVal::Obj(v) = vals[5] {
        if v != obj::OBJ_NULL {
            self_.spi.mosi = hal_pin::get_pin_obj(v);
        }
    }
    if let ArgVal::Obj(v) = vals[6] {
        if v != obj::OBJ_NULL {
            self_.spi.miso = hal_pin::get_pin_obj(v);
        }
    }
    soft_spi_ioctl(&mut self_.spi);
}

fn soft_spi_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("baudrate"),
            flags: ArgFlag::Int as u16,
            defval: ArgVal::Int(500_000),
        },
        Arg {
            qst: qstr::from_str("polarity"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("phase"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("bits"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(8),
        },
        Arg {
            qst: qstr::from_str("firstbit"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(SPI_MSB),
        },
        Arg {
            qst: qstr::from_str("sck"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("mosi"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("miso"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
    ];
    let mut vals = [ArgVal::default(); 8];
    argcheck::parse_all_kw_array(n_args, n_kw, args, allowed.len(), &allowed, &mut vals);

    if let ArgVal::Int(bits) = vals[3] {
        if bits != 8 {
            raise::raise(MpRaise::ValueError("bits must be 8"));
        }
    }
    let sck = match vals[5] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let mosi = match vals[6] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let miso = match vals[7] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    if sck == obj::OBJ_NULL || mosi == obj::OBJ_NULL || miso == obj::OBJ_NULL {
        raise::raise(MpRaise::ValueError("must specify all of sck/mosi/miso"));
    }

    let baud = match vals[0] {
        ArgVal::Int(v) => v as u32,
        _ => 500_000,
    };
    let o = malloc::new_obj::<SoftSpiObj>().expect("SoftSPI");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).spi.delay_half = baudrate_to_delay_half(baud);
        (*o).spi.polarity = match vals[1] {
            ArgVal::Int(v) => v as u8,
            _ => 0,
        };
        (*o).spi.phase = match vals[2] {
            ArgVal::Int(v) => v as u8,
            _ => 0,
        };
        (*o).spi.firstbit = match vals[4] {
            ArgVal::Int(v) => v as u8,
            _ => SPI_MSB as u8,
        };
        (*o).spi.sck = hal_pin::get_pin_obj(sck);
        (*o).spi.mosi = hal_pin::get_pin_obj(mosi);
        (*o).spi.miso = hal_pin::get_pin_obj(miso);
        soft_spi_ioctl(&mut (*o).spi);
        obj::from_ptr(o as *const SoftSpiObj as *const ())
    }
}

fn soft_spi_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*soft_spi_ptr(self_in) };
    mpprint::printf(
        print,
        "SoftSPI(baudrate=%u, polarity=%u, phase=%u, firstbit=%u)",
        [
            VaArg::UInt(baudrate_from_delay_half(self_.spi.delay_half)),
            VaArg::UInt(self_.spi.polarity as u32),
            VaArg::UInt(self_.spi.phase as u32),
            VaArg::UInt(self_.spi.firstbit as u32),
        ],
    );
}

fn soft_spi_transfer(self_in: Obj, len: usize, src: *const u8, dest: *mut u8) {
    let self_ = unsafe { &*soft_spi_ptr(self_in) };
    soft_spi_transfer_impl(&self_.spi, len, src, dest);
}

static SOFT_SPI_P: SpiProtocol = SpiProtocol {
    init: soft_spi_init,
    deinit: None,
    transfer: soft_spi_transfer,
};

static mut SOFT_SPI_SLOTS: [*const (); 4] = [
    soft_spi_make_new as MakeNewFn as *const (),
    soft_spi_print as *const (),
    &raw const SOFT_SPI_P as *const (),
    core::ptr::null(),
];

static mut SOFT_SPI_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_protocol: 3,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 4,
    slots: unsafe { SOFT_SPI_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_soft_spi_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        unsafe {
            SOFT_SPI_SLOTS[3] = spi_locals_dict().0 as *const ();
            SOFT_SPI_TYPE.name = qstr::from_str("SoftSPI");
        }
    });
    unsafe { &SOFT_SPI_TYPE }
}

/// `machine.SoftSPI` type (bitbang SPI on Pin protocol pins).
pub fn soft_spi_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_SOFTSPI {
        panic!("SoftSPI disabled");
    }
    init_soft_spi_type()
}

#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && (mpconfig::PY_MACHINE_SPI || mpconfig::PY_MACHINE_SOFTSPI)
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

pub fn init_types() -> Obj {
    if mpconfig::PY_MACHINE_SOFTSPI {
        return obj::from_ptr(soft_spi_type() as *const ObjType as *const ());
    }
    Obj(0)
}
