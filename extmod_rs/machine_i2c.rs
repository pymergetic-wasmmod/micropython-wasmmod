//! rewrite of extmod/machine_i2c.c (SoftI2C bitbang + shared I2C protocol helpers)
//! Host-complete for `machine.SoftI2C`; HW `machine.I2C` needs MCU I2C controller HAL on port builds.
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::map::{self, Map, MapElem};
use py_rs::malloc;
use py_rs::mperrno;
use py_rs::mpconfig;
use py_rs::mphal;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, BufferInfo, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objlist;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;
use py_rs::vstr::{self, Vstr};

use crate::hal_pin;

const SOFT_I2C_DEFAULT_TIMEOUT_US: u32 = 50_000;
const FLAG_READ: u32 = 0x01;
const FLAG_STOP: u32 = 0x02;

#[repr(C)]
pub struct I2cBuf {
    pub len: usize,
    pub buf: *mut u8,
}

type I2cInitFn = fn(Obj, usize, &[Obj], &Map);
type I2cDeinitFn = fn(Obj);
type I2cStartFn = fn(Obj) -> i32;
type I2cStopFn = fn(Obj) -> i32;
type I2cReadFn = fn(Obj, *mut u8, usize, bool) -> i32;
type I2cWriteFn = fn(Obj, *const u8, usize) -> i32;
type I2cTransferFn = fn(Obj, u16, usize, *mut I2cBuf, u32) -> i32;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct I2cProtocol {
    pub init: I2cInitFn,
    pub deinit: Option<I2cDeinitFn>,
    pub start: Option<I2cStartFn>,
    pub stop: Option<I2cStopFn>,
    pub read: Option<I2cReadFn>,
    pub write: Option<I2cWriteFn>,
    pub transfer: I2cTransferFn,
}

fn i2c_protocol(o: Obj) -> Option<I2cProtocol> {
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
    Some(unsafe { *(slots.add(idx as usize - 1) as *const I2cProtocol) })
}

fn raise_os_error(ret: i32) -> ! {
    raise::raise(MpRaise::OSError(-ret));
}

// --- SoftI2C bitbang HAL ---

#[repr(C)]
struct SoftI2cObj {
    base: ObjBase,
    us_delay: u32,
    us_timeout: u32,
    scl: Obj,
    sda: Obj,
}

fn soft_i2c_ptr(o: Obj) -> *mut SoftI2cObj {
    obj::as_ptr(o) as *mut SoftI2cObj
}

fn i2c_delay(self_: &SoftI2cObj) {
    mphal::delay_us(self_.us_delay as usize);
}

fn i2c_scl_low(self_: &SoftI2cObj) {
    hal_pin::pin_od_low(self_.scl);
}

fn i2c_scl_release(self_: &SoftI2cObj) -> i32 {
    let mut count = self_.us_timeout;
    hal_pin::pin_od_high(self_.scl);
    i2c_delay(self_);
    while hal_pin::pin_read(self_.scl) == 0 && count > 0 {
        mphal::delay_us(1);
        count -= 1;
    }
    if count == 0 {
        return -mperrno::ETIMEDOUT;
    }
    0
}

fn i2c_sda_low(self_: &SoftI2cObj) {
    hal_pin::pin_od_low(self_.sda);
}

fn i2c_sda_release(self_: &SoftI2cObj) {
    hal_pin::pin_od_high(self_.sda);
}

fn i2c_sda_read(self_: &SoftI2cObj) -> i32 {
    hal_pin::pin_read(self_.sda)
}

fn i2c_start(self_: &SoftI2cObj) -> i32 {
    i2c_sda_release(self_);
    i2c_delay(self_);
    let ret = i2c_scl_release(self_);
    if ret != 0 {
        return ret;
    }
    i2c_sda_low(self_);
    i2c_delay(self_);
    0
}

fn i2c_stop(self_: &SoftI2cObj) -> i32 {
    i2c_delay(self_);
    i2c_sda_low(self_);
    i2c_delay(self_);
    let ret = i2c_scl_release(self_);
    i2c_sda_release(self_);
    i2c_delay(self_);
    ret
}

fn i2c_bus_init(self_: &mut SoftI2cObj, freq: u32) {
    self_.us_delay = 500_000 / freq;
    if self_.us_delay == 0 {
        self_.us_delay = 1;
    }
    hal_pin::pin_open_drain(self_.scl);
    hal_pin::pin_open_drain(self_.sda);
    let _ = i2c_stop(self_);
}

fn i2c_write_byte(self_: &SoftI2cObj, val: u8) -> i32 {
    i2c_delay(self_);
    i2c_scl_low(self_);
    for i in (0..=7).rev() {
        if (val >> i) & 1 != 0 {
            i2c_sda_release(self_);
        } else {
            i2c_sda_low(self_);
        }
        i2c_delay(self_);
        let ret = i2c_scl_release(self_);
        if ret != 0 {
            i2c_sda_release(self_);
            return ret;
        }
        i2c_scl_low(self_);
    }
    i2c_sda_release(self_);
    i2c_delay(self_);
    let ret = i2c_scl_release(self_);
    if ret != 0 {
        return ret;
    }
    let ack = i2c_sda_read(self_);
    i2c_delay(self_);
    i2c_scl_low(self_);
    ack
}

fn i2c_read_byte(self_: &SoftI2cObj, val: &mut u8, nack: bool) -> i32 {
    i2c_delay(self_);
    i2c_scl_low(self_);
    i2c_delay(self_);
    let mut data = 0u8;
    for _ in (0..=7).rev() {
        let ret = i2c_scl_release(self_);
        if ret != 0 {
            return ret;
        }
        data = (data << 1) | (i2c_sda_read(self_) as u8);
        i2c_scl_low(self_);
        i2c_delay(self_);
    }
    *val = data;
    if !nack {
        i2c_sda_low(self_);
    }
    i2c_delay(self_);
    let ret = i2c_scl_release(self_);
    if ret != 0 {
        i2c_sda_release(self_);
        return ret;
    }
    i2c_scl_low(self_);
    i2c_sda_release(self_);
    0
}

fn soft_i2c_transfer(self_in: Obj, addr: u16, n: usize, bufs: *mut I2cBuf, flags: u32) -> i32 {
    let self_ = unsafe { &*soft_i2c_ptr(self_in) };
    let ret = i2c_start(self_);
    if ret != 0 {
        return ret;
    }
    let ret = i2c_write_byte(self_, ((addr << 1) | (flags & FLAG_READ) as u16) as u8);
    if ret < 0 {
        return ret;
    } else if ret != 0 {
        let _ = i2c_stop(self_);
        return -mperrno::ENODEV;
    }
    let mut transfer_ret = 0i32;
    let mut remaining = n;
    let mut bufp = bufs;
    while remaining > 0 {
        remaining -= 1;
        let len = unsafe { (*bufp).len };
        let mut buf = unsafe { (*bufp).buf };
        bufp = unsafe { bufp.add(1) };
        if flags & FLAG_READ != 0 {
            while len > 0 {
                let nack = remaining == 0 && len == 1;
                let mut b = 0u8;
                let ret = i2c_read_byte(self_, &mut b, nack);
                if ret != 0 {
                    return ret;
                }
                unsafe {
                    *buf = b;
                    buf = buf.add(1);
                }
            }
        } else {
            let mut left = len;
            while left > 0 {
                let ret = i2c_write_byte(self_, unsafe { *buf });
                buf = unsafe { buf.add(1) };
                left -= 1;
                if ret < 0 {
                    return ret;
                } else if ret != 0 {
                    remaining = 0;
                    break;
                }
                transfer_ret += 1;
            }
        }
    }
    if flags & FLAG_STOP != 0 {
        let ret = i2c_stop(self_);
        if ret != 0 {
            return ret;
        }
    }
    transfer_ret
}

fn soft_i2c_read(self_in: Obj, dest: *mut u8, len: usize, nack: bool) -> i32 {
    let self_ = unsafe { &*soft_i2c_ptr(self_in) };
    let mut p = dest;
    let mut left = len;
    while left > 0 {
        let mut b = 0u8;
        let ret = i2c_read_byte(self_, &mut b, nack && left == 1);
        if ret != 0 {
            return ret;
        }
        unsafe {
            *p = b;
            p = p.add(1);
        }
        left -= 1;
    }
    0
}

fn soft_i2c_write(self_in: Obj, src: *const u8, len: usize) -> i32 {
    let self_ = unsafe { &*soft_i2c_ptr(self_in) };
    let mut num_acks = 0i32;
    let mut p = src;
    let mut left = len;
    while left > 0 {
        let ret = i2c_write_byte(self_, unsafe { *p });
        p = unsafe { p.add(1) };
        left -= 1;
        if ret < 0 {
            return ret;
        } else if ret != 0 {
            break;
        }
        num_acks += 1;
    }
    num_acks
}

// --- shared I2C helpers ---

fn i2c_readfrom_inner(self_in: Obj, addr: u16, dest: *mut u8, len: usize, stop: bool) -> i32 {
    let p = i2c_protocol(self_in).expect("I2C protocol");
    let mut buf = I2cBuf { len, buf: dest };
    let flags = FLAG_READ | if stop { FLAG_STOP } else { 0 };
    (p.transfer)(self_in, addr, 1, &mut buf, flags)
}

fn i2c_writeto_inner(self_in: Obj, addr: u16, src: *const u8, len: usize, stop: bool) -> i32 {
    let p = i2c_protocol(self_in).expect("I2C protocol");
    let mut buf = I2cBuf {
        len,
        buf: src as *mut u8,
    };
    let flags = if stop { FLAG_STOP } else { 0 };
    (p.transfer)(self_in, addr, 1, &mut buf, flags)
}

type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;

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
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}

static mut FK: [*const (); 1] = [call_kw as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];

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
static T1: ObjType = ObjType {
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
    slots: unsafe { F1.as_ptr() },
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

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("i2c fnkw");
    unsafe {
        (*o).base.type_ = &TK;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("i2c fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("i2c fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("i2c fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}

fn i2c_init(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let p = i2c_protocol(args[0]).expect("I2C protocol");
    (p.init)(args[0], n - 1, &args[1..n], kw);
    obj::CONST_NONE
}

fn i2c_deinit(self_in: Obj) -> Obj {
    if let Some(p) = i2c_protocol(self_in) {
        if let Some(deinit) = p.deinit {
            deinit(self_in);
        }
    }
    obj::CONST_NONE
}

fn i2c_scan(self_in: Obj) -> Obj {
    let list = objlist::new_list(0, None);
    for addr in 0x08..0x78 {
        let ret = i2c_writeto_inner(self_in, addr, core::ptr::null(), 0, true);
        if ret == 0 {
            let _ = objlist::list_append(list, obj::new_small_int(addr as isize));
        }
        runtime::event_handle_nowait();
    }
    list
}

fn i2c_start_py(self_in: Obj) -> Obj {
    let p = i2c_protocol(self_in).expect("I2C protocol");
    let start = p.start.expect("I2C operation not supported");
    let ret = start(self_in);
    if ret != 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_stop_py(self_in: Obj) -> Obj {
    let p = i2c_protocol(self_in).expect("I2C protocol");
    let stop = p.stop.expect("I2C operation not supported");
    let ret = stop(self_in);
    if ret != 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_readinto(n: usize, args: &[Obj]) -> Obj {
    let p = i2c_protocol(args[0]).expect("I2C protocol");
    let read = p.read.expect("I2C operation not supported");
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_WRITE);
    let nack = n == 2 || obj::is_true(args[2]);
    let ret = read(args[0], bufinfo.buf, bufinfo.len, nack);
    if ret != 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_write(self_in: Obj, buf_in: Obj) -> Obj {
    let p = i2c_protocol(self_in).expect("I2C protocol");
    let write = p.write.expect("I2C operation not supported");
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(buf_in, &mut bufinfo, obj::BUFFER_READ);
    let ret = write(self_in, bufinfo.buf as *const u8, bufinfo.len);
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::new_small_int(ret as isize)
}

fn i2c_readfrom(n: usize, args: &[Obj]) -> Obj {
    let addr = obj::get_int(args[1]) as u16;
    let len = obj::get_int(args[2]) as usize;
    let stop = n == 3 || obj::is_true(args[3]);
    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init_len(&mut v, len);
    let ret = i2c_readfrom_inner(args[0], addr, v.buf, len, stop);
    if ret < 0 {
        raise_os_error(ret);
    }
    objstr::new_bytes_from_vstr(&mut v)
}

fn i2c_readfrom_into(n: usize, args: &[Obj]) -> Obj {
    let addr = obj::get_int(args[1]) as u16;
    let stop = n == 3 || obj::is_true(args[3]);
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[2], &mut bufinfo, obj::BUFFER_WRITE);
    let ret = i2c_readfrom_inner(args[0], addr, bufinfo.buf, bufinfo.len, stop);
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_writeto(n: usize, args: &[Obj]) -> Obj {
    let addr = obj::get_int(args[1]) as u16;
    let stop = n == 3 || obj::is_true(args[3]);
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[2], &mut bufinfo, obj::BUFFER_READ);
    let ret = i2c_writeto_inner(args[0], addr, bufinfo.buf as *const u8, bufinfo.len, stop);
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::new_small_int(ret as isize)
}

fn i2c_writevto(n: usize, args: &[Obj]) -> Obj {
    let addr = obj::get_int(args[1]) as u16;
    let stop = n == 3 || obj::is_true(args[3]);
    let (nitems, items) = obj::get_array(args[2]);
    let mut bufs: Vec<I2cBuf> = Vec::with_capacity(nitems.max(1));
    for item in items.iter().take(nitems) {
        let mut bufinfo = BufferInfo::default();
        obj::get_buffer_raise(*item, &mut bufinfo, obj::BUFFER_READ);
        if bufinfo.len > 0 {
            bufs.push(I2cBuf {
                len: bufinfo.len,
                buf: bufinfo.buf,
            });
        }
    }
    if bufs.is_empty() {
        bufs.push(I2cBuf {
            len: 0,
            buf: core::ptr::null_mut(),
        });
    }
    let p = i2c_protocol(args[0]).expect("I2C protocol");
    let flags = if stop { FLAG_STOP } else { 0 };
    let ret = (p.transfer)(args[0], addr, bufs.len(), bufs.as_mut_ptr(), flags);
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::new_small_int(ret as isize)
}

fn fill_memaddr_buf(memaddr: u32, addrsize: u8) -> (usize, [u8; 4]) {
    if (addrsize & 7) != 0 || addrsize > 32 {
        raise::raise(MpRaise::ValueError("invalid addrsize"));
    }
    let mut memaddr_buf = [0u8; 4];
    let mut memaddr_len = 0usize;
    let mut i = i16::from(addrsize) - 8;
    while i >= 0 {
        memaddr_buf[memaddr_len] = (memaddr >> i) as u8;
        memaddr_len += 1;
        i -= 8;
    }
    (memaddr_len, memaddr_buf)
}

fn read_mem(self_in: Obj, addr: u16, memaddr: u32, addrsize: u8, buf: *mut u8, len: usize) -> i32 {
    let (memaddr_len, memaddr_buf) = fill_memaddr_buf(memaddr, addrsize);
    let ret = i2c_writeto_inner(self_in, addr, memaddr_buf.as_ptr(), memaddr_len, false);
    if ret != memaddr_len as i32 {
        let _ = i2c_writeto_inner(self_in, addr, core::ptr::null(), 0, true);
        return ret;
    }
    i2c_readfrom_inner(self_in, addr, buf, len, true)
}

fn write_mem(
    self_in: Obj,
    addr: u16,
    memaddr: u32,
    addrsize: u8,
    buf: *const u8,
    len: usize,
) -> i32 {
    let (memaddr_len, mut memaddr_buf) = fill_memaddr_buf(memaddr, addrsize);
    let mut bufs = [
        I2cBuf {
            len: memaddr_len,
            buf: memaddr_buf.as_mut_ptr(),
        },
        I2cBuf {
            len,
            buf: buf as *mut u8,
        },
    ];
    let p = i2c_protocol(self_in).expect("I2C protocol");
    (p.transfer)(self_in, addr, 2, bufs.as_mut_ptr(), FLAG_STOP)
}

fn i2c_readfrom_mem(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("addr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("memaddr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("arg"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("addrsize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(8),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n - 1, &args[1..n], &mut kw_copy, allowed.len(), &allowed, &mut vals);
    let addr = match vals[0] {
        ArgVal::Int(v) => v as u16,
        _ => 0,
    };
    let memaddr = match vals[1] {
        ArgVal::Int(v) => v as u32,
        _ => 0,
    };
    let n_obj = match vals[2] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let addrsize = match vals[3] {
        ArgVal::Int(v) => v as u8,
        _ => 8,
    };
    let len = obj::get_int(n_obj) as usize;
    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init_len(&mut v, len);
    let ret = read_mem(args[0], addr, memaddr, addrsize, v.buf, len);
    if ret < 0 {
        raise_os_error(ret);
    }
    objstr::new_bytes_from_vstr(&mut v)
}

fn i2c_readfrom_mem_into(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("addr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("memaddr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("arg"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("addrsize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(8),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n - 1, &args[1..n], &mut kw_copy, allowed.len(), &allowed, &mut vals);
    let addr = match vals[0] {
        ArgVal::Int(v) => v as u16,
        _ => 0,
    };
    let memaddr = match vals[1] {
        ArgVal::Int(v) => v as u32,
        _ => 0,
    };
    let buf_obj = match vals[2] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let addrsize = match vals[3] {
        ArgVal::Int(v) => v as u8,
        _ => 8,
    };
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(buf_obj, &mut bufinfo, obj::BUFFER_WRITE);
    let ret = read_mem(
        args[0],
        addr,
        memaddr,
        addrsize,
        bufinfo.buf,
        bufinfo.len,
    );
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_writeto_mem(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("addr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("memaddr"),
            flags: ArgFlag::Required as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(0),
        },
        Arg {
            qst: qstr::from_str("arg"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("addrsize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(8),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n - 1, &args[1..n], &mut kw_copy, allowed.len(), &allowed, &mut vals);
    let addr = match vals[0] {
        ArgVal::Int(v) => v as u16,
        _ => 0,
    };
    let memaddr = match vals[1] {
        ArgVal::Int(v) => v as u32,
        _ => 0,
    };
    let buf_obj = match vals[2] {
        ArgVal::Obj(v) => v,
        _ => obj::OBJ_NULL,
    };
    let addrsize = match vals[3] {
        ArgVal::Int(v) => v as u8,
        _ => 8,
    };
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(buf_obj, &mut bufinfo, obj::BUFFER_READ);
    let ret = write_mem(
        args[0],
        addr,
        memaddr,
        addrsize,
        bufinfo.buf as *const u8,
        bufinfo.len,
    );
    if ret < 0 {
        raise_os_error(ret);
    }
    obj::CONST_NONE
}

fn i2c_locals_dict() -> Obj {
    static mut DICT: Option<Obj> = None;
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("init")),
                value: mk_kw(1, i2c_init),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("deinit")),
                value: mk1(i2c_deinit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("scan")),
                value: mk1(i2c_scan),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("start")),
                value: mk1(i2c_start_py),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("stop")),
                value: mk1(i2c_stop_py),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readinto")),
                value: mkv(2, 3, i2c_readinto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("write")),
                value: mk2(i2c_write),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readfrom")),
                value: mkv(3, 4, i2c_readfrom),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readfrom_into")),
                value: mkv(3, 4, i2c_readfrom_into),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("writeto")),
                value: mkv(3, 4, i2c_writeto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("writevto")),
                value: mkv(3, 4, i2c_writevto),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readfrom_mem")),
                value: mk_kw(1, i2c_readfrom_mem),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("readfrom_mem_into")),
                value: mk_kw(1, i2c_readfrom_mem_into),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("writeto_mem")),
                value: mk_kw(1, i2c_writeto_mem),
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

fn soft_i2c_init(self_in: Obj, n_pos: usize, pos: &[Obj], kw: &Map) {
    let allowed = [
        Arg {
            qst: qstr::from_str("scl"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("sda"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("freq"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(400_000),
        },
        Arg {
            qst: qstr::from_str("timeout"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(SOFT_I2C_DEFAULT_TIMEOUT_US as isize),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    let mut kw_copy = kw.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);
    let self_ = unsafe { &mut *soft_i2c_ptr(self_in) };
    let scl = match vals[0] {
        ArgVal::Obj(v) => hal_pin::get_pin_obj(v),
        _ => obj::OBJ_NULL,
    };
    let sda = match vals[1] {
        ArgVal::Obj(v) => hal_pin::get_pin_obj(v),
        _ => obj::OBJ_NULL,
    };
    self_.scl = scl;
    self_.sda = sda;
    self_.us_timeout = match vals[3] {
        ArgVal::Int(v) => v as u32,
        _ => SOFT_I2C_DEFAULT_TIMEOUT_US,
    };
    let freq = match vals[2] {
        ArgVal::Int(v) => v as u32,
        _ => 400_000,
    };
    i2c_bus_init(self_, freq);
}

fn soft_i2c_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let o = malloc::new_obj::<SoftI2cObj>().expect("SoftI2C");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
    }
    let self_obj = unsafe { obj::from_ptr(o as *const SoftI2cObj as *const ()) };
    let mut kw = Map::default();
    map::init(&mut kw, n_kw);
    for i in 0..n_kw {
        if let Some(slot) = map::lookup(&mut kw, args[n_args + i * 2], map::LookupKind::AddIfNotFound) {
            slot.value = args[n_args + i * 2 + 1];
        }
    }
    soft_i2c_init(self_obj, n_args, args, &kw);
    self_obj
}

fn soft_i2c_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*soft_i2c_ptr(self_in) };
    let freq = if self_.us_delay > 0 {
        500_000 / self_.us_delay
    } else {
        0
    };
    mpprint::printf(print, "SoftI2C(freq=%u)", [VaArg::UInt(freq)]);
}

fn soft_i2c_start(self_in: Obj) -> i32 {
    i2c_start(unsafe { &*soft_i2c_ptr(self_in) })
}

fn soft_i2c_stop(self_in: Obj) -> i32 {
    i2c_stop(unsafe { &*soft_i2c_ptr(self_in) })
}

static SOFT_I2C_P: I2cProtocol = I2cProtocol {
    init: soft_i2c_init,
    deinit: None,
    start: Some(soft_i2c_start),
    stop: Some(soft_i2c_stop),
    read: Some(soft_i2c_read),
    write: Some(soft_i2c_write),
    transfer: soft_i2c_transfer,
};

static mut SOFT_I2C_SLOTS: [*const (); 4] = [
    soft_i2c_make_new as MakeNewFn as *const (),
    soft_i2c_print as *const (),
    &raw const SOFT_I2C_P as *const (),
    core::ptr::null(),
];

static mut SOFT_I2C_TYPE: ObjType = ObjType {
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
    slots: unsafe { SOFT_I2C_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_soft_i2c_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        unsafe {
            SOFT_I2C_SLOTS[3] = i2c_locals_dict().0 as *const ();
            SOFT_I2C_TYPE.name = qstr::from_str("SoftI2C");
        }
    });
    unsafe { &SOFT_I2C_TYPE }
}

/// `machine.SoftI2C` type (bitbang I2C on Pin protocol pins).
pub fn soft_i2c_type() -> &'static ObjType {
    if !mpconfig::PY_MACHINE_SOFTI2C {
        panic!("SoftI2C disabled");
    }
    init_soft_i2c_type()
}

#[cfg(feature = "machine")]
pub fn enabled() -> bool {
    mpconfig::PY_MACHINE && (mpconfig::PY_MACHINE_I2C || mpconfig::PY_MACHINE_SOFTI2C)
}

#[cfg(not(feature = "machine"))]
pub fn enabled() -> bool {
    false
}

pub fn init_types() -> Obj {
    if mpconfig::PY_MACHINE_SOFTI2C {
        return obj::from_ptr(soft_i2c_type() as *const ObjType as *const ());
    }
    Obj(0)
}
