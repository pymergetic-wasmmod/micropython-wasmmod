//! rewrite of py/binary.c + py/binary.h
// symmetry: done

use crate::misc::{self, Byte};
use crate::mpconfig;
use crate::obj::{self, Int, Obj, Uint};
use crate::objfloat;
use crate::objint;
use crate::qstr;
use crate::raise::{self, MpRaise};

/// Special typecode for bytearray repr (`BYTEARRAY_TYPECODE`).
pub const BYTEARRAY_TYPECODE: u8 = 1;

const OVERFLOW_CHECKS: bool = mpconfig::PREVIEW_VERSION_2;

#[inline]
fn is_signed(typecode: u8) -> bool {
    typecode > b'Z'
}

/// `mp_binary_get_size`
pub fn get_size(struct_type: u8, val_type: u8, palign: Option<&mut usize>) -> usize {
    let (size, align) = match struct_type {
        b'<' | b'>' => match val_type {
            b'b' | b'B' => (1, 1),
            b'h' | b'H' => (2, 2),
            b'i' | b'I' | b'l' | b'L' => (4, 4),
            b'q' | b'Q' => (8, 8),
            b'e' => (2, 2),
            b'f' => (4, 4),
            b'd' => (8, 8),
            #[allow(unreachable_patterns)]
            b'P' | b'O' | b'S' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
                (std::mem::size_of::<usize>(), std::mem::size_of::<usize>())
            }
            _ => (0, 1),
        },
        b'@' => match val_type {
            BYTEARRAY_TYPECODE | b'b' | b'B' => (1, 1),
            b'h' | b'H' => (std::mem::size_of::<i16>(), std::mem::align_of::<i16>()),
            b'i' | b'I' => (std::mem::size_of::<i32>(), std::mem::align_of::<i32>()),
            b'l' | b'L' => (std::mem::size_of::<i32>(), std::mem::align_of::<i32>()),
            b'q' | b'Q' => (std::mem::size_of::<i64>(), std::mem::align_of::<i64>()),
            b'e' => (2, 2),
            b'f' => (std::mem::size_of::<f32>(), std::mem::align_of::<f32>()),
            b'd' => (std::mem::size_of::<f64>(), std::mem::align_of::<f64>()),
            #[allow(unreachable_patterns)]
            b'P' | b'O' | b'S' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
                (std::mem::size_of::<usize>(), std::mem::align_of::<usize>())
            }
            _ => (0, 1),
        },
        _ => (0, 1),
    };

    if size == 0 {
        raise::raise(MpRaise::ValueError("bad typecode"));
    }
    if let Some(a) = palign {
        *a = align;
    }
    size
}

mod half_float {
    use super::*;

    pub fn decode(hf: u16) -> f32 {
        let m = hf & 0x3ff;
        let mut e = ((hf >> 10) & 0x1f) as u32;
        let bits: u32;
        if e == 0x1f {
            e = 0xff;
            bits = ((hf as u32 & 0x8000) << 16) | (e << 23) | ((m as u32) << 13);
        } else if e != 0 {
            e += 127 - 15;
            bits = ((hf as u32 & 0x8000) << 16) | (e << 23) | ((m as u32) << 13);
        } else if m != 0 {
            e = 127 - 15;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e = e.saturating_sub(1);
            }
            m -= 0x400;
            e += 1;
            bits = ((hf as u32 & 0x8000) << 16) | (e << 23) | ((m as u32) << 13);
        } else {
            bits = (hf as u32 & 0x8000) << 16;
        }
        f32::from_bits(bits)
    }

    pub fn encode(x: f32) -> u16 {
        let fpu = x.to_bits();
        let mut m = (fpu >> 13) & 0x3ff;
        if fpu & (1 << 12) != 0 {
            m += 1;
        }
        let mut e = ((fpu >> 23) & 0xff) as i32;
        if e == 0xff {
            e = 0x1f;
        } else if e != 0 {
            e -= 127 - 15;
            if e < 0 {
                if e >= -11 {
                    m = (m | 0x400) >> (-e);
                    if m & 1 != 0 {
                        m = (m >> 1) + 1;
                    } else {
                        m >>= 1;
                    }
                } else {
                    m = 0;
                }
                e = 0;
            } else if e > 0x3f {
                e = 0x1f;
                m = 0;
            }
        }
        (((fpu >> 16) & 0x8000) | ((e as u32) << 10) | m) as u16
    }
}

/// `mp_binary_get_val_array`
pub fn get_val_array(typecode: u8, p: &[u8], index: usize) -> Obj {
    let elem_size = get_size(b'@', typecode, None);
    let offset = index * elem_size;
    let bytes = &p[offset..offset + elem_size];
    match typecode {
        b'b' => obj::new_small_int(bytes[0] as i8 as Int),
        BYTEARRAY_TYPECODE | b'B' => obj::new_small_int(bytes[0] as Int),
        b'h' => obj::new_small_int(i16::from_ne_bytes(bytes[..2].try_into().unwrap()) as Int),
        b'H' => obj::new_small_int(u16::from_ne_bytes(bytes[..2].try_into().unwrap()) as Int),
        b'i' => objint::new_int(i32::from_ne_bytes(bytes[..4].try_into().unwrap()) as Int),
        b'I' => objint::new_int_from_uint(u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as Uint),
        b'l' => objint::new_int(i32::from_ne_bytes(bytes[..4].try_into().unwrap()) as Int),
        b'L' => objint::new_int_from_uint(u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as Uint),
        b'q' => objint::new_int_from_ll(i64::from_ne_bytes(bytes[..8].try_into().unwrap())),
        b'Q' => objint::new_int_from_ull(u64::from_ne_bytes(bytes[..8].try_into().unwrap())),
        b'f' if mpconfig::PY_BUILTINS_FLOAT => {
            objfloat::new_float_from_f(f32::from_ne_bytes(bytes[..4].try_into().unwrap()))
        }
        b'd' if mpconfig::PY_BUILTINS_FLOAT => {
            objfloat::new_float_from_d(f64::from_ne_bytes(bytes[..8].try_into().unwrap()))
        }
        b'O' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
            obj::Obj(usize::from_ne_bytes(bytes[..std::mem::size_of::<usize>()].try_into().unwrap()))
        }
        b'P' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
            objint::new_int(usize::from_ne_bytes(bytes[..std::mem::size_of::<usize>()].try_into().unwrap()) as Int)
        }
        _ => obj::new_small_int(0),
    }
}

/// `mp_binary_get_int`
pub fn get_int(size: usize, is_signed: bool, big_endian: bool, src: &[Byte]) -> i64 {
    assert!(src.len() >= size);
    let src = &src[..size];
    let (delta, start) = if big_endian {
        (1isize, 0usize)
    } else {
        (-1, size - 1)
    };
    let mut src_idx = start;
    let mut val: u64 = 0;
    if is_signed && src[src_idx] & 0x80 != 0 {
        val = u64::MAX;
    }
    for _ in 0..size {
        val = (val << 8) | u64::from(src[src_idx]);
        src_idx = ((src_idx as isize) + delta) as usize;
    }
    val as i64
}

/// `mp_binary_get_val`
pub fn get_val(struct_type: u8, val_type: u8, buf: &[Byte], ptr: &mut usize) -> Obj {
    let mut p = *ptr;
    let mut align = 1usize;
    let mut struct_type = struct_type;
    let size = get_size(struct_type, val_type, Some(&mut align));
    if struct_type == b'@' {
        p = misc::align_up(p, align);
        struct_type = if mpconfig::ENDIANNESS_LITTLE { b'<' } else { b'>' };
    }
    *ptr = p + size;
    let p_bytes = &buf[p..p + size];
    let val = get_int(size, is_signed(val_type), struct_type == b'>', p_bytes);

    if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 && val_type == b'O' {
        return obj::Obj(val as usize);
    }
    if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 && val_type == b'S' {
        let cstr = unsafe { std::ffi::CStr::from_ptr(val as usize as *const i8) };
        return obj::new_qstr(qstr::from_str(cstr.to_str().unwrap_or("")));
    }
    if mpconfig::PY_BUILTINS_FLOAT {
        if val_type == b'e' {
            return objfloat::new_float_from_f(half_float::decode(val as u16));
        }
        if val_type == b'f' {
            return objfloat::new_float_from_f(f32::from_bits(val as u32));
        }
        if val_type == b'd' {
            return objfloat::new_float_from_d(f64::from_bits(val as u64));
        }
    }
    if is_signed(val_type) {
        if (smallint::MIN as i64) <= val && val <= (smallint::MAX as i64) {
            objint::new_int(val as Int)
        } else {
            objint::new_int_from_ll(val)
        }
    } else if (val as u64) <= (smallint::MAX as u64) {
        objint::new_int_from_uint(val as Uint)
    } else {
        objint::new_int_from_ull(val as u64)
    }
}

/// Store integer `val` into `dest` (`mp_binary_set_int`).
pub fn set_int(dest_sz: usize, dest: &mut [Byte], val_sz: usize, val: Uint, big_endian: bool) {
    assert!(dest.len() >= dest_sz);
    let dest = &mut dest[..dest_sz];
    let signed_negative = (val as Int) < 0;
    let val = val;
    let mut val_sz = val_sz;

    if dest_sz > val_sz {
        let fill = if signed_negative { 0xff } else { 0x00 };
        dest.fill(fill);
        if big_endian {
            let offset = dest_sz - val_sz;
            write_val(&mut dest[offset..], val, val_sz, big_endian);
            return;
        }
    } else if dest_sz < val_sz {
        val_sz = dest_sz;
    }

    write_val(dest, val, val_sz, big_endian);
}

fn write_val(dest: &mut [Byte], val: Uint, val_sz: usize, big_endian: bool) {
    let dest = &mut dest[..val_sz];
    if mpconfig::ENDIANNESS_LITTLE && !big_endian {
        let bytes = val.to_le_bytes();
        dest.copy_from_slice(&bytes[..val_sz]);
    } else if mpconfig::ENDIANNESS_BIG && big_endian {
        let bytes = val.to_be_bytes();
        dest.copy_from_slice(&bytes[bytes.len() - val_sz..]);
    } else if mpconfig::ENDIANNESS_LITTLE {
        let bytes = val.to_le_bytes();
        for (i, slot) in dest.iter_mut().enumerate() {
            *slot = bytes[val_sz - 1 - i];
        }
    } else {
        let bytes = val.to_be_bytes();
        for (i, slot) in dest.iter_mut().enumerate() {
            *slot = bytes[bytes.len() - val_sz + i];
        }
    }
}

/// Convenience wrapper matching C call sites that pass `mp_int_t`.
pub fn set_int_signed(dest_sz: usize, dest: &mut [Byte], val: Int, big_endian: bool) {
    set_int(dest_sz, dest, std::mem::size_of::<Int>(), val as Uint, big_endian);
}

/// `mp_binary_set_val`
pub fn set_val(struct_type: u8, val_type: u8, val_in: Obj, buf: &mut [Byte], ptr: &mut usize) {
    let mut p = *ptr;
    let mut align = 1usize;
    let mut struct_type = struct_type;
    let size = get_size(struct_type, val_type, Some(&mut align));
    if struct_type == b'@' {
        p = misc::align_up(p, align);
        struct_type = if mpconfig::ENDIANNESS_LITTLE { b'<' } else { b'>' };
    }
    *ptr = p + size;
    let p_slice = &mut buf[p..p + size];

    let mut val: Uint = 0;
    match val_type {
        b'O' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
            val = val_in.0;
        }
        b'e' if mpconfig::PY_BUILTINS_FLOAT => {
            val = half_float::encode(objfloat::get_float_to_f(val_in)) as Uint;
        }
        b'f' if mpconfig::PY_BUILTINS_FLOAT => {
            val = objfloat::get_float_to_f(val_in).to_bits() as Uint;
        }
        b'd' if mpconfig::PY_BUILTINS_FLOAT => {
            let fp = objfloat::get_float_to_d(val_in).to_bits();
            set_int(size, p_slice, size, fp as Uint, struct_type == b'>');
            return;
        }
        _ => {
            if mpconfig::LONGINT_IMPL != mpconfig::LONGINT_IMPL_NONE
                && obj::is_exact_type(val_in, objint::type_int())
            {
                if size <= std::mem::size_of::<Uint>() {
                    val = objint::int_get_truncated(val_in) as Uint;
                    set_int(size, p_slice, std::mem::size_of::<Uint>(), val, struct_type == b'>');
                    return;
                }
                objint::int_to_bytes(
                    val_in,
                    size,
                    p_slice,
                    struct_type == b'>',
                    is_signed(val_type),
                    false,
                );
                return;
            }
            val = obj::get_int(val_in) as Uint;
        }
    }
    set_int(size, p_slice, std::mem::size_of::<Uint>(), val, struct_type == b'>');
}

/// `mp_binary_set_val_array`
pub fn set_val_array(typecode: u8, p: &mut [u8], index: usize, val_in: Obj) {
    let elem_size = get_size(b'@', typecode, None);
    let offset = index * elem_size;
    match typecode {
        b'f' if mpconfig::PY_BUILTINS_FLOAT => {
            let v = objfloat::get_float_to_f(val_in);
            p[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
        }
        b'd' if mpconfig::PY_BUILTINS_FLOAT => {
            let v = objfloat::get_float_to_d(val_in);
            p[offset..offset + 8].copy_from_slice(&v.to_ne_bytes());
        }
        b'O' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
            p[offset..offset + std::mem::size_of::<usize>()]
                .copy_from_slice(&val_in.0.to_ne_bytes());
        }
        _ => {
            if mpconfig::LONGINT_IMPL != mpconfig::LONGINT_IMPL_NONE
                && obj::is_exact_type(val_in, objint::type_int())
            {
                let size = elem_size;
                if !OVERFLOW_CHECKS && size <= std::mem::size_of::<Int>() {
                    set_val_array_from_int(typecode, p, index, objint::int_get_truncated(val_in));
                    return;
                }
                let dest = &mut p[offset..offset + size];
                objint::int_to_bytes(
                    val_in,
                    size,
                    dest,
                    mpconfig::ENDIANNESS_BIG,
                    is_signed(typecode),
                    OVERFLOW_CHECKS,
                );
                return;
            }
            set_val_array_from_int(typecode, p, index, obj::get_int(val_in));
        }
    }
}

    fn set_val_array_from_int(typecode: u8, p: &mut [u8], index: usize, val: Int) {
    let elem_size = get_size(b'@', typecode, None);
    let offset = index * elem_size;
    let dest = &mut p[offset..offset + elem_size];
    macro_rules! set_val_as {
        ($ty:ty, $signed:expr) => {{
            let tmp = val as $ty;
            if OVERFLOW_CHECKS {
                let back = tmp as Int;
                if back != val || (!$signed && val < 0) {
                    raise::raise(MpRaise::OverflowError("integer out of range"));
                }
            }
            dest.copy_from_slice(&<$ty>::to_ne_bytes(tmp));
        }};
    }
    match typecode {
        b'b' => set_val_as!(i8, true),
        BYTEARRAY_TYPECODE | b'B' => set_val_as!(u8, false),
        b'h' => set_val_as!(i16, true),
        b'H' => set_val_as!(u16, false),
        b'i' => set_val_as!(i32, true),
        b'I' => set_val_as!(u32, false),
        b'l' => set_val_as!(i32, true),
        b'L' => set_val_as!(u32, false),
        b'q' => set_val_as!(i64, true),
        b'Q' => set_val_as!(u64, false),
        b'P' if mpconfig::PY_STRUCT_UNSAFE_TYPECODES != 0 => {
            dest.copy_from_slice(&(val as usize).to_ne_bytes());
        }
        _ => {}
    }
}

use crate::smallint;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::objfloat;
    use crate::objint;

    fn setup() {
        let _ = gc::init();
    }

    #[test]
    fn get_size_endian_codes() {
        assert_eq!(get_size(b'<', b'i', None), 4);
        assert_eq!(get_size(b'>', b'q', None), 8);
        let mut align = 0;
        assert_eq!(get_size(b'@', b'd', Some(&mut align)), 8);
        assert!(align >= 4);
    }

    #[test]
    fn set_and_get_int_roundtrip() {
        let mut buf = [0u8; 4];
        set_int_signed(4, &mut buf, -1, true);
        assert_eq!(get_int(4, true, true, &buf), -1);
        set_int_signed(2, &mut buf, 0x1234, false);
        assert_eq!(get_int(2, false, false, &buf) as u16, 0x1234);
    }

    #[test]
    fn get_set_val_struct() {
        setup();
        let mut buf = vec![0u8; 8];
        let mut ptr = 0usize;
        set_val(b'<', b'i', objint::new_int(0x01020304), &mut buf, &mut ptr);
        assert_eq!(ptr, 4);
        ptr = 0;
        let v = get_val(b'<', b'i', &buf, &mut ptr);
        assert_eq!(obj::get_int(v), 0x01020304);
    }

    #[test]
    fn val_array_i32() {
        setup();
        let mut buf = [0u8; 8];
        set_val_array(b'i', &mut buf, 0, objint::new_int(42));
        set_val_array(b'i', &mut buf, 1, objint::new_int(-7));
        assert_eq!(obj::get_int(get_val_array(b'i', &buf, 0)), 42);
        assert_eq!(obj::get_int(get_val_array(b'i', &buf, 1)), -7);
    }

    #[test]
    fn half_float_codec() {
        if !mpconfig::PY_BUILTINS_FLOAT {
            return;
        }
        let bits = half_float::encode(1.5);
        let back = half_float::decode(bits);
        assert!((back - 1.5).abs() < 0.01);
    }

    #[test]
    fn float_array() {
        if !mpconfig::PY_BUILTINS_FLOAT {
            return;
        }
        setup();
        let mut buf = [0u8; 8];
        set_val_array(b'f', &mut buf, 0, objfloat::new_float(3.25));
        let v = get_val_array(b'f', &buf, 0);
        assert!((objfloat::float_get(v) - 3.25).abs() < 1e-6);
    }
}
