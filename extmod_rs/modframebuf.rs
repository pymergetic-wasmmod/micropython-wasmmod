//! rewrite of extmod/modframebuf.c
// symmetry: done

use crate::font_petme128_8x8::FONT_PETME128_8X8;
use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::binary;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtype;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const FRAMEBUF_MVLSB: u8 = 0;
const FRAMEBUF_RGB565: u8 = 1;
const FRAMEBUF_GS2_HMSB: u8 = 5;
const FRAMEBUF_GS4_HMSB: u8 = 2;
const FRAMEBUF_GS8: u8 = 6;
const FRAMEBUF_MHLSB: u8 = 3;
const FRAMEBUF_MHMSB: u8 = 4;

const ELLIPSE_MASK_FILL: isize = 0x10;
const ELLIPSE_MASK_ALL: isize = 0x0f;
const ELLIPSE_MASK_Q1: isize = 0x01;
const ELLIPSE_MASK_Q2: isize = 0x02;
const ELLIPSE_MASK_Q3: isize = 0x04;
const ELLIPSE_MASK_Q4: isize = 0x08;

#[repr(C)]
struct ObjFramebuf {
    base: ObjBase,
    buf_obj: Obj,
    buf: *mut u8,
    width: u16,
    height: u16,
    stride: u16,
    format: u8,
}

fn fb_ptr(o: Obj) -> *mut ObjFramebuf {
    obj::as_ptr(o) as *mut ObjFramebuf
}

fn fb_ref(o: Obj) -> &'static ObjFramebuf {
    unsafe { &*fb_ptr(o) }
}

type SetPixelFn = fn(&ObjFramebuf, u32, u32, u32);
type GetPixelFn = fn(&ObjFramebuf, u32, u32) -> u32;
type FillRectFn = fn(&ObjFramebuf, u32, u32, u32, u32, u32);

struct FramebufOps {
    setpixel: SetPixelFn,
    getpixel: GetPixelFn,
    fill_rect: FillRectFn,
}

fn mono_horiz_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    let index = ((x + y * fb.stride as u32) >> 3) as usize;
    let offset = if fb.format == FRAMEBUF_MHMSB {
        x & 0x07
    } else {
        7 - (x & 0x07)
    };
    unsafe {
        let b = fb.buf.add(index);
        *b = (*b & !(0x01 << offset)) | (((col != 0) as u8) << offset);
    }
}

fn mono_horiz_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    let index = ((x + y * fb.stride as u32) >> 3) as usize;
    let offset = if fb.format == FRAMEBUF_MHMSB {
        x & 0x07
    } else {
        7 - (x & 0x07)
    };
    unsafe { ((fb.buf.add(index).read() >> offset) & 0x01).into() }
}

fn mono_horiz_fill_rect(fb: &ObjFramebuf, x: u32, y: u32, w: u32, h: u32, col: u32) {
    let reverse = fb.format == FRAMEBUF_MHMSB;
    let advance = (fb.stride >> 3) as isize;
    let mut x = x;
    let mut w = w;
    while w > 0 {
        let mut b = unsafe { fb.buf.add((x >> 3) as usize + y as usize * advance as usize) };
        let offset = if reverse { x & 7 } else { 7 - (x & 7) };
        let mut hh = h;
        while hh > 0 {
            unsafe {
                *b = (*b & !(0x01 << offset)) | (((col != 0) as u8) << offset);
                b = b.offset(advance);
            }
            hh -= 1;
        }
        x += 1;
        w -= 1;
    }
}

fn mvlsb_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    let index = ((y >> 3) * fb.stride as u32 + x) as usize;
    let offset = (y & 0x07) as u8;
    unsafe {
        let b = fb.buf.add(index);
        *b = (*b & !(0x01 << offset)) | (((col != 0) as u8) << offset);
    }
}

fn mvlsb_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    let index = ((y >> 3) * fb.stride as u32 + x) as usize;
    unsafe { ((fb.buf.add(index).read() >> (y & 0x07)) & 0x01).into() }
}

fn mvlsb_fill_rect(fb: &ObjFramebuf, x: u32, mut y: u32, w: u32, mut h: u32, col: u32) {
    while h > 0 {
        let mut b = unsafe { fb.buf.add(((y >> 3) * fb.stride as u32 + x) as usize) };
        let offset = (y & 0x07) as u8;
        let mut ww = w;
        while ww > 0 {
            unsafe {
                *b = (*b & !(0x01 << offset)) | (((col != 0) as u8) << offset);
                b = b.add(1);
            }
            ww -= 1;
        }
        y += 1;
        h -= 1;
    }
}

fn rgb565_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    unsafe {
        let p = fb.buf as *mut u16;
        *p.add((x + y * fb.stride as u32) as usize) = col as u16;
    }
}

fn rgb565_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    unsafe {
        let p = fb.buf as *const u16;
        p.add((x + y * fb.stride as u32) as usize).read().into()
    }
}

fn rgb565_fill_rect(fb: &ObjFramebuf, x: u32, y: u32, w: u32, mut h: u32, col: u32) {
    let mut b = unsafe {
        (fb.buf as *mut u16).add((x + y * fb.stride as u32) as usize)
    };
    while h > 0 {
        let mut ww = w;
        while ww > 0 {
            unsafe {
                *b = col as u16;
                b = b.add(1);
            }
            ww -= 1;
        }
        unsafe {
            b = b.add(fb.stride as usize - w as usize);
        }
        h -= 1;
    }
}

fn gs2_hmsb_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    unsafe {
        let pixel = fb.buf.add(((x + y * fb.stride as u32) >> 2) as usize);
        let shift = ((x & 0x3) << 1) as u8;
        let mask = 0x3 << shift;
        let color = ((col & 0x3) << shift) as u8;
        *pixel = color | (*pixel & !mask);
    }
}

fn gs2_hmsb_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    unsafe {
        let pixel = fb.buf.add(((x + y * fb.stride as u32) >> 2) as usize).read();
        let shift = (x & 0x3) << 1;
        ((pixel >> shift) & 0x3).into()
    }
}

fn gs2_hmsb_fill_rect(fb: &ObjFramebuf, x: u32, y: u32, w: u32, h: u32, col: u32) {
    for xx in x..x + w {
        for yy in y..y + h {
            gs2_hmsb_setpixel(fb, xx, yy, col);
        }
    }
}

fn gs4_hmsb_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    unsafe {
        let pixel = fb.buf.add(((x + y * fb.stride as u32) >> 1) as usize);
        if x % 2 != 0 {
            *pixel = (col as u8 & 0x0f) | (*pixel & 0xf0);
        } else {
            *pixel = ((col as u8) << 4) | (*pixel & 0x0f);
        }
    }
}

fn gs4_hmsb_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    unsafe {
        let pixel = fb.buf.add(((x + y * fb.stride as u32) >> 1) as usize).read();
        if x % 2 != 0 {
            (pixel & 0x0f).into()
        } else {
            (pixel >> 4).into()
        }
    }
}

fn gs4_hmsb_fill_rect(fb: &ObjFramebuf, x: u32, y: u32, w: u32, mut h: u32, col: u32) {
    let col = col & 0x0f;
    let mut pixel_pair =
        unsafe { fb.buf.add(((x + y * fb.stride as u32) >> 1) as usize) };
    let col_shifted_left = (col << 4) as u8;
    let col_pixel_pair = col_shifted_left | col as u8;
    let pixel_count_till_next_line = (fb.stride - w as u16) >> 1;
    let odd_x = x % 2 == 1;

    while h > 0 {
        let mut ww = w;
        let mut pp = pixel_pair;

        if odd_x && ww > 0 {
            unsafe {
                *pp = (*pp & 0xf0) | col as u8;
                pp = pp.add(1);
            }
            ww -= 1;
        }

        unsafe {
            core::ptr::write_bytes(pp, col_pixel_pair, (ww >> 1) as usize);
            pp = pp.add((ww >> 1) as usize);
        }

        if ww % 2 != 0 {
            unsafe {
                *pp = col_shifted_left | (*pp & 0x0f);
                if !odd_x {
                    pp = pp.add(1);
                }
            }
        }

        unsafe {
            pixel_pair = pp.add(pixel_count_till_next_line as usize);
        }
        h -= 1;
    }
}

fn gs8_setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    unsafe {
        *fb.buf.add((x + y * fb.stride as u32) as usize) = (col & 0xff) as u8;
    }
}

fn gs8_getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    unsafe {
        fb.buf
            .add((x + y * fb.stride as u32) as usize)
            .read()
            .into()
    }
}

fn gs8_fill_rect(fb: &ObjFramebuf, x: u32, y: u32, w: u32, mut h: u32, col: u32) {
    let mut pixel = unsafe { fb.buf.add((x + y * fb.stride as u32) as usize) };
    while h > 0 {
        unsafe {
            core::ptr::write_bytes(pixel, col as u8, w as usize);
            pixel = pixel.add(fb.stride as usize);
        }
        h -= 1;
    }
}

fn format_ops(format: u8) -> FramebufOps {
    match format {
        FRAMEBUF_MVLSB => FramebufOps {
            setpixel: mvlsb_setpixel,
            getpixel: mvlsb_getpixel,
            fill_rect: mvlsb_fill_rect,
        },
        FRAMEBUF_RGB565 => FramebufOps {
            setpixel: rgb565_setpixel,
            getpixel: rgb565_getpixel,
            fill_rect: rgb565_fill_rect,
        },
        FRAMEBUF_GS2_HMSB => FramebufOps {
            setpixel: gs2_hmsb_setpixel,
            getpixel: gs2_hmsb_getpixel,
            fill_rect: gs2_hmsb_fill_rect,
        },
        FRAMEBUF_GS4_HMSB => FramebufOps {
            setpixel: gs4_hmsb_setpixel,
            getpixel: gs4_hmsb_getpixel,
            fill_rect: gs4_hmsb_fill_rect,
        },
        FRAMEBUF_GS8 => FramebufOps {
            setpixel: gs8_setpixel,
            getpixel: gs8_getpixel,
            fill_rect: gs8_fill_rect,
        },
        FRAMEBUF_MHLSB | FRAMEBUF_MHMSB => FramebufOps {
            setpixel: mono_horiz_setpixel,
            getpixel: mono_horiz_getpixel,
            fill_rect: mono_horiz_fill_rect,
        },
        _ => FramebufOps {
            setpixel: mvlsb_setpixel,
            getpixel: mvlsb_getpixel,
            fill_rect: mvlsb_fill_rect,
        },
    }
}

fn setpixel(fb: &ObjFramebuf, x: u32, y: u32, col: u32) {
    (format_ops(fb.format).setpixel)(fb, x, y, col);
}

fn setpixel_checked(fb: &ObjFramebuf, x: isize, y: isize, col: isize, mask: isize) {
    if mask != 0 && (0..fb.width as isize).contains(&x) && (0..fb.height as isize).contains(&y) {
        setpixel(fb, x as u32, y as u32, col as u32);
    }
}

fn getpixel(fb: &ObjFramebuf, x: u32, y: u32) -> u32 {
    (format_ops(fb.format).getpixel)(fb, x, y)
}

fn fill_rect(fb: &ObjFramebuf, x: isize, y: isize, w: isize, h: isize, col: u32) {
    if h < 1 || w < 1 || x + w <= 0 || y + h <= 0 || y >= fb.height as isize || x >= fb.width as isize
    {
        return;
    }
    let xend = (fb.width as isize).min(x + w);
    let yend = (fb.height as isize).min(y + h);
    let x0 = x.max(0);
    let y0 = y.max(0);
    (format_ops(fb.format).fill_rect)(
        fb,
        x0 as u32,
        y0 as u32,
        (xend - x0) as u32,
        (yend - y0) as u32,
        col,
    );
}

fn framebuf_make_new_helper(
    n_args: usize,
    args: &[Obj],
    buf_flags: u32,
    o: Option<&mut ObjFramebuf>,
) -> Obj {
    let width = obj::get_int(args[1]);
    let height = obj::get_int(args[2]);
    let format = obj::get_int(args[3]) as u8;
    let mut stride = if n_args >= 5 {
        obj::get_int(args[4])
    } else {
        width
    };

    if width < 1 || height < 1 || width > 0xffff || height > 0xffff || stride > 0xffff || stride < width
    {
        raise::raise(MpRaise::ValueError(""));
    }

    let mut bpp = 1usize;
    let mut height_required = height as usize;
    let mut width_required = width as usize;
    let mut strides_required = height as usize - 1;

    match format {
        FRAMEBUF_MVLSB => {
            height_required = (height as usize + 7) & !7;
            strides_required = height_required - 8;
        }
        FRAMEBUF_MHLSB | FRAMEBUF_MHMSB => {
            stride = (stride + 7) & !7;
            width_required = (width as usize + 7) & !7;
        }
        FRAMEBUF_GS2_HMSB => {
            stride = (stride + 3) & !3;
            width_required = (width as usize + 3) & !3;
            bpp = 2;
        }
        FRAMEBUF_GS4_HMSB => {
            stride = (stride + 1) & !1;
            width_required = (width as usize + 1) & !1;
            bpp = 4;
        }
        FRAMEBUF_GS8 => bpp = 8,
        FRAMEBUF_RGB565 => bpp = 16,
        _ => raise::raise(MpRaise::ValueError("invalid format")),
    }

    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut bufinfo, buf_flags);

    if (strides_required * stride as usize + (height_required - strides_required) * width_required)
        * bpp
        / 8
        > bufinfo.len
    {
        raise::raise(MpRaise::ValueError(""));
    }

    let obj_out = if let Some(existing) = o {
        existing.buf_obj = args[0];
        existing.buf = bufinfo.buf as *mut u8;
        existing.width = width as u16;
        existing.height = height as u16;
        existing.format = format;
        existing.stride = stride as u16;
        return obj::OBJ_NULL;
    } else {
        let o = malloc::new_obj::<ObjFramebuf>().expect("FrameBuffer");
        unsafe {
            (*o).base.type_ = type_framebuf() as *const ObjType;
            (*o).buf_obj = args[0];
            (*o).buf = bufinfo.buf as *mut u8;
            (*o).width = width as u16;
            (*o).height = height as u16;
            (*o).format = format;
            (*o).stride = stride as u16;
            obj::from_ptr(o as *const ObjFramebuf as *const ())
        }
    };
    obj_out
}

fn framebuf_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 4, 5, false);
    framebuf_make_new_helper(n_args, args, obj::BUFFER_WRITE, None)
}

fn framebuf_get_buffer(self_in: Obj, bufinfo: &mut BufferInfo, flags: u32) -> obj::Int {
    let self_ = fb_ref(self_in);
    if obj::get_buffer(self_.buf_obj, bufinfo, flags) {
        0
    } else {
        1
    }
}

fn framebuf_args(args: &[Obj], n: usize) -> Vec<isize> {
    (0..n).map(|i| obj::get_int(args[i + 1])).collect()
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
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
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
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
static mut F3: [*const (); 1] = [call3 as *const ()];
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
static T3: ObjType = ObjType {
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
    slots: unsafe { F3.as_ptr() },
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
fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("framebuf fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("framebuf fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("framebuf fn3");
    unsafe {
        (*o).base.type_ = &T3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("framebuf fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn framebuf_fill(self_in: Obj, col_in: Obj) -> Obj {
    let self_ = fb_ref(self_in);
    let col = obj::get_int(col_in);
    (format_ops(self_.format).fill_rect)(
        self_,
        0,
        0,
        self_.width as u32,
        self_.height as u32,
        col as u32,
    );
    obj::CONST_NONE
}

fn framebuf_fill_rect(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 5);
    fill_rect(self_, a[0], a[1], a[2], a[3], a[4] as u32);
    obj::CONST_NONE
}

fn framebuf_pixel(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let x = obj::get_int(args[1]);
    let y = obj::get_int(args[2]);
    if (0..self_.width as isize).contains(&x) && (0..self_.height as isize).contains(&y) {
        if n == 3 {
            return obj::new_small_int(getpixel(self_, x as u32, y as u32) as isize);
        }
        setpixel(self_, x as u32, y as u32, obj::get_int(args[3]) as u32);
    }
    obj::CONST_NONE
}

fn framebuf_hline(_n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 4);
    fill_rect(self_, a[0], a[1], a[2], 1, a[3] as u32);
    obj::CONST_NONE
}

fn framebuf_vline(_n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 4);
    fill_rect(self_, a[0], a[1], 1, a[2], a[3] as u32);
    obj::CONST_NONE
}

fn framebuf_rect(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 5);
    if n > 6 && obj::is_true(args[6]) {
        fill_rect(self_, a[0], a[1], a[2], a[3], a[4] as u32);
    } else {
        fill_rect(self_, a[0], a[1], a[2], 1, a[4] as u32);
        fill_rect(self_, a[0], a[1] + a[3] - 1, a[2], 1, a[4] as u32);
        fill_rect(self_, a[0], a[1], 1, a[3], a[4] as u32);
        fill_rect(self_, a[0] + a[2] - 1, a[1], 1, a[3], a[4] as u32);
    }
    obj::CONST_NONE
}

fn line(fb: &ObjFramebuf, mut x1: isize, mut y1: isize, x2: isize, y2: isize, col: isize) {
    let mut dx = x2 - x1;
    let mut sx = if dx > 0 {
        1isize
    } else {
        dx = -dx;
        -1
    };
    let mut dy = y2 - y1;
    let mut sy = if dy > 0 {
        1isize
    } else {
        dy = -dy;
        -1
    };
    let steep = if dy > dx {
        core::mem::swap(&mut x1, &mut y1);
        core::mem::swap(&mut dx, &mut dy);
        core::mem::swap(&mut sx, &mut sy);
        true
    } else {
        false
    };
    let mut e = 2 * dy - dx;
    for _ in 0..dx {
        if steep {
            if (0..fb.width as isize).contains(&y1) && (0..fb.height as isize).contains(&x1) {
                setpixel(fb, y1 as u32, x1 as u32, col as u32);
            }
        } else if (0..fb.width as isize).contains(&x1) && (0..fb.height as isize).contains(&y1) {
            setpixel(fb, x1 as u32, y1 as u32, col as u32);
        }
        while e >= 0 {
            y1 += sy;
            e -= 2 * dx;
        }
        x1 += sx;
        e += 2 * dy;
    }
    setpixel_checked(fb, x2, y2, col, 1);
}

fn framebuf_line(_n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 5);
    line(self_, a[0], a[1], a[2], a[3], a[4]);
    obj::CONST_NONE
}

fn draw_ellipse_points(
    fb: &ObjFramebuf,
    cx: isize,
    cy: isize,
    x: isize,
    y: isize,
    col: isize,
    mask: isize,
) {
    if mask & ELLIPSE_MASK_FILL != 0 {
        if mask & ELLIPSE_MASK_Q1 != 0 {
            fill_rect(fb, cx, cy - y, x + 1, 1, col as u32);
        }
        if mask & ELLIPSE_MASK_Q2 != 0 {
            fill_rect(fb, cx - x, cy - y, x + 1, 1, col as u32);
        }
        if mask & ELLIPSE_MASK_Q3 != 0 {
            fill_rect(fb, cx - x, cy + y, x + 1, 1, col as u32);
        }
        if mask & ELLIPSE_MASK_Q4 != 0 {
            fill_rect(fb, cx, cy + y, x + 1, 1, col as u32);
        }
    } else {
        setpixel_checked(fb, cx + x, cy - y, col, mask & ELLIPSE_MASK_Q1);
        setpixel_checked(fb, cx - x, cy - y, col, mask & ELLIPSE_MASK_Q2);
        setpixel_checked(fb, cx - x, cy + y, col, mask & ELLIPSE_MASK_Q3);
        setpixel_checked(fb, cx + x, cy + y, col, mask & ELLIPSE_MASK_Q4);
    }
}

fn framebuf_ellipse(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let a = framebuf_args(args, 5);
    let mut mask = if n > 6 && obj::is_true(args[6]) {
        ELLIPSE_MASK_FILL
    } else {
        0
    };
    if n > 7 {
        mask |= obj::get_int(args[7]) & ELLIPSE_MASK_ALL;
    } else {
        mask |= ELLIPSE_MASK_ALL;
    };
    if a[2] == 0 && a[3] == 0 {
        setpixel_checked(self_, a[0], a[1], a[4], mask & ELLIPSE_MASK_ALL);
        return obj::CONST_NONE;
    }
    let two_asquare = 2 * a[2] * a[2];
    let two_bsquare = 2 * a[3] * a[3];
    let mut x = a[2];
    let mut y = 0;
    let mut xchange = a[3] * a[3] * (1 - 2 * a[2]);
    let mut ychange = a[2] * a[2];
    let mut ellipse_error = 0;
    let mut stoppingx = two_bsquare * a[2];
    let mut stoppingy = 0;
    while stoppingx >= stoppingy {
        draw_ellipse_points(self_, a[0], a[1], x, y, a[4], mask);
        y += 1;
        stoppingy += two_asquare;
        ellipse_error += ychange;
        ychange += two_asquare;
        if 2 * ellipse_error + xchange > 0 {
            x -= 1;
            stoppingx -= two_bsquare;
            ellipse_error += xchange;
            xchange += two_bsquare;
        }
    }
    x = 0;
    y = a[3];
    xchange = a[3] * a[3];
    ychange = a[2] * a[2] * (1 - 2 * a[3]);
    ellipse_error = 0;
    stoppingx = 0;
    stoppingy = two_asquare * a[3];
    while stoppingx <= stoppingy {
        draw_ellipse_points(self_, a[0], a[1], x, y, a[4], mask);
        x += 1;
        stoppingx += two_bsquare;
        ellipse_error += xchange;
        xchange += two_bsquare;
        if 2 * ellipse_error + ychange > 0 {
            y -= 1;
            stoppingy -= two_asquare;
            ellipse_error += ychange;
            ychange += two_asquare;
        }
    }
    obj::CONST_NONE
}

fn poly_int(bufinfo: &BufferInfo, index: usize) -> isize {
    let data = unsafe { std::slice::from_raw_parts(bufinfo.buf as *const u8, bufinfo.len) };
    obj::get_int(binary::get_val_array(bufinfo.typecode as u8, data, index))
}

fn framebuf_poly(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let x = obj::get_int(args[1]);
    let y = obj::get_int(args[2]);
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[3], &mut bufinfo, obj::BUFFER_READ);
    let elem = binary::get_size(b'@', bufinfo.typecode as u8, None);
    let n_poly = bufinfo.len / (elem * 2);
    if n_poly == 0 {
        return obj::CONST_NONE;
    }
    let col = obj::get_int(args[4]);
    let fill = n > 5 && obj::is_true(args[5]);
    if fill {
        let mut y_min = isize::MAX;
        let mut y_max = isize::MIN;
        for i in 0..n_poly {
            let py = poly_int(&bufinfo, i * 2 + 1);
            y_min = y_min.min(py);
            y_max = y_max.max(py);
        }
        for row in y_min..=y_max {
            let mut nodes = vec![0isize; n_poly];
            let mut n_nodes = 0usize;
            let mut px1 = poly_int(&bufinfo, 0);
            let mut py1 = poly_int(&bufinfo, 1);
            let mut i = (n_poly * 2 - 1) as isize;
            loop {
                let py2 = poly_int(&bufinfo, i as usize);
                i -= 1;
                let px2 = poly_int(&bufinfo, i as usize);
                i -= 1;
                if py1 != py2 && ((py1 > row && py2 <= row) || (py1 <= row && py2 > row)) {
                    let node = (32 * px1 + 32 * (px2 - px1) * (row - py1) / (py2 - py1) + 16) / 32;
                    nodes[n_nodes] = node;
                    n_nodes += 1;
                } else if row == py1.max(py2) {
                    if py1 < py2 {
                        setpixel_checked(self_, x + px2, y + py2, col, 1);
                    } else if py2 < py1 {
                        setpixel_checked(self_, x + px1, y + py1, col, 1);
                    } else {
                        line(self_, x + px1, y + py1, x + px2, y + py2, col);
                    }
                }
                px1 = px2;
                py1 = py2;
                if i < 0 {
                    break;
                }
            }
            if n_nodes == 0 {
                continue;
            }
            let mut i = 0usize;
            while i < n_nodes - 1 {
                if nodes[i] > nodes[i + 1] {
                    nodes.swap(i, i + 1);
                    if i > 0 {
                        i -= 1;
                    }
                } else {
                    i += 1;
                }
            }
            let mut i = 0usize;
            while i < n_nodes {
                fill_rect(
                    self_,
                    x + nodes[i],
                    y + row,
                    (nodes[i + 1] - nodes[i]) + 1,
                    1,
                    col as u32,
                );
                i += 2;
            }
        }
    } else {
        let mut px1 = poly_int(&bufinfo, 0);
        let mut py1 = poly_int(&bufinfo, 1);
        let mut i = (n_poly * 2 - 1) as isize;
        loop {
            let py2 = poly_int(&bufinfo, i as usize);
            i -= 1;
            let px2 = poly_int(&bufinfo, i as usize);
            i -= 1;
            line(self_, x + px1, y + py1, x + px2, y + py2, col);
            px1 = px2;
            py1 = py2;
            if i < 0 {
                break;
            }
        }
    }
    obj::CONST_NONE
}

fn get_readonly_framebuffer(arg: Obj, rofb: &mut ObjFramebuf) {
    let type_obj = obj::from_ptr(type_framebuf() as *const ObjType as *const ());
    let fb = objtype::cast_to_native_base(arg, type_obj);
    if fb != obj::OBJ_NULL {
        *rofb = unsafe { fb_ptr(fb).read() };
    } else {
        let (len, items) = obj::get_array(arg);
        if len < 4 || len > 5 {
            raise::raise(MpRaise::ValueError(""));
        }
        framebuf_make_new_helper(len, &items, obj::BUFFER_READ, Some(rofb));
    }
}

fn framebuf_blit(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let mut source = ObjFramebuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf_obj: obj::OBJ_NULL,
        buf: core::ptr::null_mut(),
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
    };
    get_readonly_framebuffer(args[1], &mut source);
    let x = obj::get_int(args[2]);
    let y = obj::get_int(args[3]);
    let key = if n > 4 {
        obj::get_int(args[4])
    } else {
        -1
    };
    let mut palette = ObjFramebuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf_obj: obj::OBJ_NULL,
        buf: core::ptr::null_mut(),
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
    };
    let palette_ptr = if n > 5 && args[5] != obj::CONST_NONE {
        get_readonly_framebuffer(args[5], &mut palette);
        Some(&palette)
    } else {
        None
    };
    if x >= self_.width as isize
        || y >= self_.height as isize
        || -x >= source.width as isize
        || -y >= source.height as isize
    {
        return obj::CONST_NONE;
    }
    let mut x0 = 0isize.max(x);
    let mut y0 = 0isize.max(y);
    let mut x1 = 0isize.max(-x);
    let mut y1 = 0isize.max(-y);
    let x0end = (self_.width as isize).min(x + source.width as isize);
    let y0end = (self_.height as isize).min(y + source.height as isize);
    while y0 < y0end {
        let mut cx1 = x1;
        let mut cx0 = x0;
        while cx0 < x0end {
            let mut col = getpixel(&source, cx1 as u32, y1 as u32);
            if let Some(pal) = palette_ptr {
                col = getpixel(pal, col, 0);
            }
            if col as isize != key {
                setpixel(self_, cx0 as u32, y0 as u32, col);
            }
            cx1 += 1;
            cx0 += 1;
        }
        y1 += 1;
        y0 += 1;
    }
    obj::CONST_NONE
}

fn framebuf_scroll(self_in: Obj, xstep_in: Obj, ystep_in: Obj) -> Obj {
    let self_ = fb_ref(self_in);
    let xstep = obj::get_int(xstep_in);
    let ystep = obj::get_int(ystep_in);
    let (sx, xend, dx): (u32, u32, isize) = if xstep < 0 {
        if -xstep >= self_.width as isize {
            return obj::CONST_NONE;
        }
        (0, (self_.width as isize + xstep) as u32, 1)
    } else {
        if xstep >= self_.width as isize {
            return obj::CONST_NONE;
        }
        (
            (self_.width - 1) as u32,
            (xstep - 1) as u32,
            -1,
        )
    };
    let (mut y, yend, dy): (u32, u32, isize) = if ystep < 0 {
        if -ystep >= self_.height as isize {
            return obj::CONST_NONE;
        }
        (0, (self_.height as isize + ystep) as u32, 1)
    } else {
        if ystep >= self_.height as isize {
            return obj::CONST_NONE;
        }
        (
            (self_.height - 1) as u32,
            (ystep - 1) as u32,
            -1,
        )
    };
    while y != yend {
        let mut x = sx;
        while x != xend {
            let col = getpixel(
                self_,
                (x as isize - xstep) as u32,
                (y as isize - ystep) as u32,
            );
            setpixel(self_, x, y, col);
            x = (x as isize + dx) as u32;
        }
        y = (y as isize + dy) as u32;
    }
    obj::CONST_NONE
}

fn framebuf_text(n: usize, args: &[Obj]) -> Obj {
    let self_ = fb_ref(args[0]);
    let (str_data, str_len) = objstr::str_get_data(args[1]);
    let mut x0 = obj::get_int(args[2]);
    let y0 = obj::get_int(args[3]);
    let col = if n >= 5 {
        obj::get_int(args[4])
    } else {
        1
    };
    for &byte in &str_data[..str_len] {
        let mut chr = byte;
        if !(32..=127).contains(&chr) {
            chr = 127;
        }
        let chr_data = &FONT_PETME128_8X8[((chr - 32) * 8) as usize..][..8];
        for &vline_data in chr_data {
            if (0..self_.width as isize).contains(&x0) {
                let mut vline_data = vline_data;
                let mut y = y0;
                while vline_data != 0 {
                    if vline_data & 1 != 0 && (0..self_.height as isize).contains(&y) {
                        setpixel(self_, x0 as u32, y as u32, col as u32);
                    }
                    vline_data >>= 1;
                    y += 1;
                }
            }
            x0 += 1;
        }
    }
    obj::CONST_NONE
}

fn legacy_framebuffer1(n: usize, args: &[Obj]) -> Obj {
    let stride = if n >= 4 { args[3] } else { args[1] };
    let make_args = [
        args[0],
        args[1],
        args[2],
        obj::new_small_int(FRAMEBUF_MVLSB as isize),
        stride,
    ];
    framebuf_make_new(type_framebuf(), 5, 0, &make_args)
}

static mut FRAMEBUF_SLOTS: [*const (); 4] = [
    framebuf_make_new as *const (),
    framebuf_get_buffer as *const (),
    core::ptr::null(),
    core::ptr::null(),
];
static mut FRAMEBUF_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 2,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { FRAMEBUF_SLOTS.as_ptr() },
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_framebuf_type() -> &'static ObjType {
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("fill")),
                value: mk2(framebuf_fill),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("fill_rect")),
                value: mkv(6, 6, framebuf_fill_rect),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("pixel")),
                value: mkv(3, 4, framebuf_pixel),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("hline")),
                value: mkv(5, 5, framebuf_hline),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("vline")),
                value: mkv(5, 5, framebuf_vline),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rect")),
                value: mkv(6, 7, framebuf_rect),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("line")),
                value: mkv(6, 6, framebuf_line),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ellipse")),
                value: mkv(6, 8, framebuf_ellipse),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("blit")),
                value: mkv(4, 6, framebuf_blit),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("scroll")),
                value: mk3(framebuf_scroll),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("text")),
                value: mkv(4, 5, framebuf_text),
            },
        ];
        if mpconfig::PY_ARRAY {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("poly")),
                value: mkv(5, 6, framebuf_poly),
            });
        }
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            FRAMEBUF_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            FRAMEBUF_TYPE.name = qstr::from_str("FrameBuffer");
        }
    });
    unsafe { &FRAMEBUF_TYPE }
}

pub fn type_framebuf() -> &'static ObjType {
    init_framebuf_type()
}

/// Register built-in `framebuf` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_FRAMEBUF {
        return obj::OBJ_NULL;
    }
    init_framebuf_type();
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("framebuf")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("FrameBuffer")),
            value: obj::from_ptr(type_framebuf() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("FrameBuffer1")),
            value: mkv(3, 4, legacy_framebuffer1),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("MVLSB")),
            value: obj::new_small_int(FRAMEBUF_MVLSB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("MONO_VLSB")),
            value: obj::new_small_int(FRAMEBUF_MVLSB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("RGB565")),
            value: obj::new_small_int(FRAMEBUF_RGB565 as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("GS2_HMSB")),
            value: obj::new_small_int(FRAMEBUF_GS2_HMSB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("GS4_HMSB")),
            value: obj::new_small_int(FRAMEBUF_GS4_HMSB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("GS8")),
            value: obj::new_small_int(FRAMEBUF_GS8 as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("MONO_HLSB")),
            value: obj::new_small_int(FRAMEBUF_MHLSB as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("MONO_HMSB")),
            value: obj::new_small_int(FRAMEBUF_MHMSB as isize),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("framebuf module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("framebuf"), module);
    module
}
