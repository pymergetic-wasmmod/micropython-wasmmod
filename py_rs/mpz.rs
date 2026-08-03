//! rewrite of py/mpz.c + py/mpz.h
// symmetry: done

use crate::malloc;
use crate::misc::Byte;
use crate::mpconfig;
use crate::obj::{self, Int, Uint};

pub const DIG_SIZE: u32 = 32;
pub type Dig = u32;
pub type DblDig = u64;
pub type DblDigSigned = i64;

pub const NUM_DIG_FOR_INT: usize =
    (std::mem::size_of::<Int>() * 8 + DIG_SIZE as usize - 1) / DIG_SIZE as usize;
pub const NUM_DIG_FOR_LL: usize =
    (std::mem::size_of::<i64>() * 8 + DIG_SIZE as usize - 1) / DIG_SIZE as usize;

const DIG_MASK: Dig = u32::MAX;
const DIG_MSB: Dig = 1 << (DIG_SIZE - 1);
const DIG_BASE: u64 = 1u64 << DIG_SIZE;
const MIN_ALLOC: usize = 2;

/// Arbitrary precision integer (`mpz_t`).
#[derive(Debug, Clone, Default)]
pub struct Mpz {
    pub neg: bool,
    pub fixed_dig: bool,
    pub alloc: usize,
    pub len: usize,
    pub dig: Vec<Dig>,
}

pub type MpzT = Mpz;

fn dig_copy(z: &Mpz) -> Vec<Dig> {
    z.dig[..z.len].to_vec()
}

fn mpn_remove_trailing_zeros(dig: &mut [Dig], len: usize) -> usize {
    let mut l = len;
    while l > 0 && dig[l - 1] == 0 {
        l -= 1;
    }
    l
}

fn mpn_cmp(idig: &[Dig], ilen: usize, jdig: &[Dig], jlen: usize) -> i32 {
    if ilen < jlen {
        return -1;
    }
    if ilen > jlen {
        return 1;
    }
    for i in (0..ilen).rev() {
        let cmp = idig[i] as DblDigSigned - jdig[i] as DblDigSigned;
        if cmp < 0 {
            return -1;
        }
        if cmp > 0 {
            return 1;
        }
    }
    0
}

fn mpn_shl(idig: &mut [Dig], jdig: &[Dig], jlen: usize, n: Uint) -> usize {
    let n_whole = (n as usize + DIG_SIZE as usize - 1) / DIG_SIZE as usize;
    let mut n_part = (n as usize) % DIG_SIZE as usize;
    if n_part == 0 {
        n_part = DIG_SIZE as usize;
    }
    let out_len = jlen + n_whole;
    for i in (0..jlen).rev() {
        let d = jdig[i] as DblDig;
        let dst = i + n_whole;
        if dst < idig.len() {
            idig[dst] = ((d >> (DIG_SIZE as usize - n_part)) & DIG_MASK as DblDig) as Dig;
        }
        if i + n_whole > 0 && i + n_whole - 1 < idig.len() {
            idig[i + n_whole - 1] |= ((d << n_part) & DIG_MASK as DblDig) as Dig;
        }
    }
    for i in 0..n_whole.saturating_sub(1).min(idig.len()) {
        idig[i] = 0;
    }
    let mut jlen_out = jlen + n_whole;
    while jlen_out > 0 && idig.get(jlen_out - 1).copied().unwrap_or(0) == 0 {
        jlen_out -= 1;
    }
    jlen_out
}

fn mpn_shr(idig: &mut [Dig], jdig: &[Dig], jlen: usize, n: Uint) -> usize {
    let n_whole = n as usize / DIG_SIZE as usize;
    let n_part = n as usize % DIG_SIZE as usize;
    if n_whole >= jlen {
        return 0;
    }
    let jlen2 = jlen - n_whole;
    for i in 0..jlen2 {
        let mut d = jdig[i + n_whole] as DblDig;
        if i + n_whole + 1 < jlen {
            d |= (jdig[i + n_whole + 1] as DblDig) << DIG_SIZE;
        }
        d >>= n_part;
        idig[i] = (d & DIG_MASK as DblDig) as Dig;
    }
    if jlen2 > 0 && idig[jlen2 - 1] == 0 {
        jlen2 - 1
    } else {
        jlen2
    }
}

fn mpn_add(idig: &mut [Dig], jdig: &[Dig], jlen: usize, kdig: &[Dig], klen: usize) -> usize {
    let mut carry: DblDig = 0;
    let mut out = 0usize;
    let extra = jlen.saturating_sub(klen);
    for i in 0..klen {
        carry += jdig[i] as DblDig + kdig[i] as DblDig;
        idig[i] = (carry & DIG_MASK as DblDig) as Dig;
        carry >>= DIG_SIZE;
        out = i + 1;
    }
    for i in klen..jlen {
        carry += jdig[i] as DblDig;
        idig[i] = (carry & DIG_MASK as DblDig) as Dig;
        carry >>= DIG_SIZE;
        out = i + 1;
    }
    if carry != 0 {
        idig[out] = carry as Dig;
        out += 1;
    }
    out
}

fn mpn_sub(idig: &mut [Dig], jdig: &[Dig], jlen: usize, kdig: &[Dig], klen: usize) -> usize {
    let mut borrow: DblDigSigned = 0;
    for i in 0..klen {
        borrow += jdig[i] as DblDigSigned - kdig[i] as DblDigSigned;
        idig[i] = (borrow & DIG_MASK as DblDigSigned) as Dig;
        borrow >>= DIG_SIZE as i32;
    }
    for i in klen..jlen {
        borrow += jdig[i] as DblDigSigned;
        idig[i] = (borrow & DIG_MASK as DblDigSigned) as Dig;
        borrow >>= DIG_SIZE as i32;
    }
    mpn_remove_trailing_zeros(idig, jlen)
}

#[cfg(any())]
fn mpn_and(idig: &mut [Dig], jdig: &[Dig], kdig: &[Dig], klen: usize) -> usize {
    for i in 0..klen {
        idig[i] = jdig[i] & kdig[i];
    }
    mpn_remove_trailing_zeros(idig, klen)
}

fn mpn_and_neg(
    idig: &mut [Dig],
    jdig: &[Dig],
    jlen: usize,
    kdig: &[Dig],
    klen: usize,
    mut carryi: DblDig,
    mut carryj: DblDig,
    mut carryk: DblDig,
) -> usize {
    let imask = if carryi == 0 { 0 } else { DIG_MASK };
    let jmask = if carryj == 0 { 0 } else { DIG_MASK };
    let kmask = if carryk == 0 { 0 } else { DIG_MASK };
    let mut out = 0usize;
    let mut kl = klen;
    let mut jl = jlen;
    for i in 0..jlen {
        carryj += (jdig[i] ^ jmask) as DblDig;
        carryk += if kl > 0 {
            kl -= 1;
            jl -= 1;
            (kdig[i] ^ kmask) as DblDig
        } else {
            jl -= 1;
            kmask as DblDig
        };
        carryi += ((carryj & carryk) ^ imask as DblDig) & DIG_MASK as DblDig;
        idig[i] = (carryi & DIG_MASK as DblDig) as Dig;
        carryk >>= DIG_SIZE;
        carryj >>= DIG_SIZE;
        carryi >>= DIG_SIZE;
        out = i + 1;
    }
    if carryi != 0 {
        idig[out] = carryi as Dig;
        out += 1;
    }
    mpn_remove_trailing_zeros(idig, out)
}

#[cfg(any())]
fn mpn_or(idig: &mut [Dig], jdig: &[Dig], jlen: usize, kdig: &[Dig], klen: usize) -> usize {
    let extra = jlen - klen;
    for i in 0..klen {
        idig[i] = jdig[i] | kdig[i];
    }
    for i in 0..extra {
        idig[klen + i] = jdig[klen + i];
    }
    jlen
}

fn mpn_or_neg(
    idig: &mut [Dig],
    jdig: &[Dig],
    jlen: usize,
    kdig: &[Dig],
    klen: usize,
    mut carryj: DblDig,
    mut carryk: DblDig,
) -> usize {
    let mut carryi: DblDig = 1;
    let jmask = if carryj == 0 { 0 } else { DIG_MASK };
    let kmask = if carryk == 0 { 0 } else { DIG_MASK };
    let mut out = 0usize;
    let mut kl = klen;
    let mut jl = jlen;
    for i in 0..jlen {
        carryj += (jdig[i] ^ jmask) as DblDig;
        carryk += if kl > 0 {
            kl -= 1;
            jl -= 1;
            (kdig[i] ^ kmask) as DblDig
        } else {
            jl -= 1;
            kmask as DblDig
        };
        carryi += ((carryj | carryk) ^ DIG_MASK as DblDig) & DIG_MASK as DblDig;
        idig[i] = (carryi & DIG_MASK as DblDig) as Dig;
        carryk >>= DIG_SIZE;
        carryj >>= DIG_SIZE;
        carryi >>= DIG_SIZE;
        out = i + 1;
    }
    debug_assert!(carryi == 0);
    mpn_remove_trailing_zeros(idig, out)
}

#[cfg(any())]
fn mpn_xor(idig: &mut [Dig], jdig: &[Dig], jlen: usize, kdig: &[Dig], klen: usize) -> usize {
    let extra = jlen - klen;
    for i in 0..klen {
        idig[i] = jdig[i] ^ kdig[i];
    }
    for i in 0..extra {
        idig[klen + i] = jdig[klen + i];
    }
    mpn_remove_trailing_zeros(idig, jlen)
}

fn mpn_xor_neg(
    idig: &mut [Dig],
    jdig: &[Dig],
    jlen: usize,
    kdig: &[Dig],
    klen: usize,
    mut carryi: DblDig,
    mut carryj: DblDig,
    mut carryk: DblDig,
) -> usize {
    let mut out = 0usize;
    let mut kl = klen;
    let mut jl = jlen;
    for i in 0..jlen {
        carryj += jdig[i] as DblDig + DIG_MASK as DblDig;
        carryk += if kl > 0 {
            kl -= 1;
            jl -= 1;
            kdig[i] as DblDig + DIG_MASK as DblDig
        } else {
            jl -= 1;
            DIG_MASK as DblDig
        };
        carryi += (carryj ^ carryk) & DIG_MASK as DblDig;
        idig[i] = (carryi & DIG_MASK as DblDig) as Dig;
        carryk >>= DIG_SIZE;
        carryj >>= DIG_SIZE;
        carryi >>= DIG_SIZE;
        out = i + 1;
    }
    if carryi != 0 {
        idig[out] = carryi as Dig;
        out += 1;
    }
    mpn_remove_trailing_zeros(idig, out)
}

fn mpn_mul_dig_add_dig(idig: &mut [Dig], ilen: usize, dmul: Dig, dadd: Dig) -> usize {
    let mut carry = dadd as DblDig;
    let mut out = ilen;
    for i in 0..ilen {
        carry += idig[i] as DblDig * dmul as DblDig;
        idig[i] = (carry & DIG_MASK as DblDig) as Dig;
        carry >>= DIG_SIZE;
    }
    if carry != 0 {
        idig[out] = carry as Dig;
        out += 1;
    }
    out
}

fn mpn_mul(idig: &mut [Dig], jdig: &[Dig], jlen: usize, kdig: &[Dig], klen: usize) -> usize {
    let mut ilen = 0usize;
    for k in 0..klen {
        let mut carry: DblDig = 0;
        for j in 0..jlen {
            let idx = k + j;
            carry += idig[idx] as DblDig + jdig[j] as DblDig * kdig[k] as DblDig;
            idig[idx] = (carry & DIG_MASK as DblDig) as Dig;
            carry >>= DIG_SIZE;
        }
        let idx = k + jlen;
        if carry != 0 {
            idig[idx] = carry as Dig;
            ilen = idx + 1;
        } else if idx > ilen {
            ilen = idx;
        }
    }
    ilen.max(jlen + klen)
}

fn mpn_div(
    num_dig: &mut [Dig],
    num_len: &mut usize,
    den_dig: &[Dig],
    den_len: usize,
    quo_dig: &mut [Dig],
    quo_len: &mut usize,
) {
    let cmp = mpn_cmp(&num_dig[..*num_len], *num_len, den_dig, den_len);
    if cmp == 0 {
        *num_len = 0;
        quo_dig[0] = 1;
        *quo_len = 1;
        return;
    }
    if cmp < 0 {
        *quo_len = 0;
        return;
    }
    let mut norm_shift = 0u32;
    {
        let mut d = den_dig[den_len - 1];
        while (d & DIG_MSB) == 0 {
            d <<= 1;
            norm_shift += 1;
        }
    }
    if *num_len >= num_dig.len() {
        return;
    }
    num_dig[*num_len] = 0;
    *num_len += 1;
    let mut carry_norm: Dig = 0;
    for i in 0..*num_len {
        let n = num_dig[i];
        num_dig[i] = ((n << norm_shift) | carry_norm) & DIG_MASK;
        carry_norm = n >> (DIG_SIZE - norm_shift);
    }
    let mut lead_den: DblDig = (den_dig[den_len - 1] as DblDig) << norm_shift;
    if den_len >= 2 {
        lead_den |= (den_dig[den_len - 2] as DblDig) >> (DIG_SIZE - norm_shift);
    }
    *quo_len = *num_len - den_len;
    let mut num_idx = *num_len - 1;
    for q in (0..*quo_len).rev() {
        let mut quo: DblDig =
            ((num_dig[num_idx] as DblDig) << DIG_SIZE) | num_dig[num_idx - 1] as DblDig;
        quo /= lead_den.max(1);
        let mut borrow: DblDigSigned = 0;
        let mut d_norm: DblDig = 0;
        for j in 0..den_len {
            d_norm = ((den_dig[j] as DblDig) << norm_shift) | (d_norm >> DIG_SIZE);
            let x = quo * (d_norm & DIG_MASK as DblDig);
            let n_idx = num_idx - den_len + j;
            let low = (borrow & DIG_MASK as DblDigSigned) as DblDig + num_dig[n_idx] as DblDig
                - (x & DIG_MASK as DblDig);
            num_dig[n_idx] = (low & DIG_MASK as DblDig) as Dig;
            borrow = (borrow >> DIG_SIZE as i32) - (x >> DIG_SIZE) as DblDigSigned
                + (low >> DIG_SIZE) as DblDigSigned;
        }
        borrow += num_dig[num_idx] as DblDigSigned;
        while borrow != 0 {
            quo -= 1;
            let mut carry: DblDig = 0;
            d_norm = 0;
            for j in 0..den_len {
                d_norm = ((den_dig[j] as DblDig) << norm_shift) | (d_norm >> DIG_SIZE);
                carry += num_dig[num_idx - den_len + j] as DblDig + (d_norm & DIG_MASK as DblDig);
                num_dig[num_idx - den_len + j] = (carry & DIG_MASK as DblDig) as Dig;
                carry >>= DIG_SIZE;
            }
            borrow += carry as DblDigSigned;
        }
        quo_dig[q] = (quo & DIG_MASK as DblDig) as Dig;
        num_idx -= 1;
        *num_len -= 1;
    }
    let mut carry_un: DblDig = 0;
    for i in (0..*num_len).rev() {
        let n = num_dig[i] as DblDig;
        num_dig[i] = ((n >> norm_shift) | carry_un) as Dig & DIG_MASK;
        carry_un = (n << (DIG_SIZE - norm_shift)) & DIG_MASK as DblDig;
    }
    while *quo_len > 0 && quo_dig[*quo_len - 1] == 0 {
        *quo_len -= 1;
    }
    *num_len = mpn_remove_trailing_zeros(num_dig, *num_len);
}

fn need_dig(z: &mut Mpz, need: usize) {
    let need = need.max(MIN_ALLOC);
    if z.fixed_dig {
        debug_assert!(z.alloc >= need);
        return;
    }
    if z.dig.len() < need {
        z.dig.resize(need, 0);
        z.alloc = need;
    }
}

pub fn init_zero(z: &mut Mpz) {
    *z = Mpz {
        neg: false,
        fixed_dig: false,
        alloc: 0,
        len: 0,
        dig: Vec::new(),
    };
}

pub fn init_from_int(z: &mut Mpz, val: Int) {
    init_zero(z);
    set_from_int(z, val);
}

pub fn init_fixed_from_int(z: &mut Mpz, dig: &mut [Dig], val: Int) {
    z.neg = false;
    z.fixed_dig = true;
    z.alloc = dig.len();
    z.len = 0;
    z.dig = dig.to_vec();
    set_from_int(z, val);
}

pub fn deinit(z: &mut Mpz) {
    if !z.fixed_dig {
        z.dig.clear();
        z.alloc = 0;
    }
}

fn mpz_clone(src: &Mpz) -> Mpz {
    debug_assert!(src.alloc != 0 || src.len == 0);
    let mut z = Mpz {
        neg: src.neg,
        fixed_dig: false,
        alloc: src.alloc.max(MIN_ALLOC),
        len: src.len,
        dig: vec![0; src.alloc.max(MIN_ALLOC)],
    };
    z.dig[..src.len].copy_from_slice(&src.dig[..src.len]);
    z
}

fn mpz_free(z: Option<Mpz>) {
    drop(z);
}

pub fn set(dest: &mut Mpz, src: &Mpz) {
    need_dig(dest, src.len);
    dest.neg = src.neg;
    dest.len = src.len;
    dest.dig[..src.len].copy_from_slice(&src.dig[..src.len]);
}

pub fn set_from_int(z: &mut Mpz, val: Int) {
    if val == 0 {
        z.neg = false;
        z.len = 0;
        return;
    }
    need_dig(z, NUM_DIG_FOR_INT);
    let (neg, mut uval) = if val < 0 {
        (true, (-val) as u64)
    } else {
        (false, val as u64)
    };
    z.neg = neg;
    z.len = 0;
    while uval > 0 {
        z.dig[z.len] = (uval & DIG_MASK as u64) as Dig;
        z.len += 1;
        uval >>= DIG_SIZE;
    }
}

pub fn set_from_ll(z: &mut Mpz, val: i64, is_signed: bool) {
    need_dig(z, NUM_DIG_FOR_LL);
    let (neg, mut uval) = if is_signed && val < 0 {
        (true, (-val) as u64)
    } else {
        (false, val as u64)
    };
    z.neg = neg;
    z.len = 0;
    while uval > 0 {
        z.dig[z.len] = (uval & DIG_MASK as u64) as Dig;
        z.len += 1;
        uval >>= DIG_SIZE;
    }
}

pub fn set_from_str(z: &mut Mpz, s: &str, neg: bool, base: u32) -> usize {
    debug_assert!(base <= 36);
    need_dig(z, s.len() * 8 / DIG_SIZE as usize + 1);
    z.neg = neg;
    z.len = 0;
    let mut consumed = 0usize;
    for &ch in s.as_bytes() {
        let v = match ch {
            b'0'..=b'9' => ch - b'0',
            b'A'..=b'Z' => ch - b'A' + 10,
            b'a'..=b'z' => ch - b'a' + 10,
            _ => break,
        };
        if u32::from(v) >= base {
            break;
        }
        z.len = mpn_mul_dig_add_dig(&mut z.dig, z.len.max(1), base as Dig, v as Dig);
        consumed += 1;
    }
    z.len = mpn_remove_trailing_zeros(&mut z.dig, z.len);
    consumed
}

pub fn set_from_bytes(z: &mut Mpz, big_endian: bool, buf: &[Byte]) {
    need_dig(
        z,
        (buf.len() * 8 + DIG_SIZE as usize - 1) / DIG_SIZE as usize,
    );
    let mut d: Dig = 0;
    let mut num_bits = 0u32;
    z.neg = false;
    z.len = 0;
    let iter: Box<dyn Iterator<Item = Byte>> = if big_endian {
        Box::new(buf.iter().copied().rev())
    } else {
        Box::new(buf.iter().copied())
    };
    for byte in iter {
        while num_bits < DIG_SIZE {
            d |= (byte as Dig) << num_bits;
            num_bits += 8;
            if num_bits >= DIG_SIZE {
                z.dig[z.len] = d & DIG_MASK;
                z.len += 1;
                d = 0;
                num_bits -= DIG_SIZE;
            }
        }
    }
    z.len = mpn_remove_trailing_zeros(&mut z.dig, z.len);
}

pub fn is_zero(z: &Mpz) -> bool {
    z.len == 0
}
pub fn is_neg(z: &Mpz) -> bool {
    z.neg
}

pub fn cmp(z1: &Mpz, z2: &Mpz) -> i32 {
    let mut c = z2.neg as i32 - z1.neg as i32;
    if c != 0 {
        return c;
    }
    c = mpn_cmp(&z1.dig[..z1.len], z1.len, &z2.dig[..z2.len], z2.len);
    if z1.neg {
        -c
    } else {
        c
    }
}

pub fn abs_inpl(dest: &mut Mpz, src: &Mpz) {
    if !std::ptr::eq(dest, src) {
        set(dest, src);
    }
    dest.neg = false;
}

pub fn neg_inpl(dest: &mut Mpz, src: &Mpz) {
    if !std::ptr::eq(dest, src) {
        set(dest, src);
    }
    if dest.len > 0 {
        dest.neg = !dest.neg;
    }
}

pub fn not_inpl(dest: &mut Mpz, src: &Mpz) {
    if !std::ptr::eq(dest, src) {
        set(dest, src);
    }
    if dest.len == 0 {
        need_dig(dest, 1);
        dest.dig[0] = 1;
        dest.len = 1;
        dest.neg = true;
    } else if dest.neg {
        dest.neg = false;
        let k = 1;
        let tmp = dig_copy(dest);
        dest.len = mpn_sub(&mut dest.dig, &tmp, tmp.len(), &[k], 1);
    } else {
        need_dig(dest, dest.len + 1);
        let k = 1;
        let tmp = dig_copy(dest);
        dest.len = mpn_add(&mut dest.dig, &tmp, tmp.len(), &[k], 1);
        dest.neg = true;
    }
}

pub fn shl_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: u32) {
    if lhs.len == 0 || rhs == 0 {
        set(dest, lhs);
    } else {
        need_dig(
            dest,
            lhs.len + (rhs as usize + DIG_SIZE as usize - 1) / DIG_SIZE as usize,
        );
        dest.len = mpn_shl(&mut dest.dig, &lhs.dig[..lhs.len], lhs.len, rhs as Uint);
        dest.neg = lhs.neg;
    }
}

pub fn shr_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: u32) {
    if lhs.len == 0 || rhs == 0 {
        set(dest, lhs);
    } else {
        need_dig(dest, lhs.len);
        dest.len = mpn_shr(&mut dest.dig, &lhs.dig[..lhs.len], lhs.len, rhs as Uint);
        dest.neg = lhs.neg;
        if dest.neg {
            let n_whole = rhs as usize / DIG_SIZE as usize;
            let n_part = rhs as usize % DIG_SIZE as usize;
            let mut round_up = 0u32;
            for i in 0..lhs.len.min(n_whole) {
                if lhs.dig[i] != 0 {
                    round_up = 1;
                    break;
                }
            }
            if n_whole < lhs.len && (lhs.dig[n_whole] & ((1 << n_part) - 1)) != 0 {
                round_up = 1;
            }
            if round_up != 0 {
                if dest.len == 0 {
                    dest.dig[0] = 1;
                    dest.len = 1;
                } else {
                    let tmp = dig_copy(dest);
                    dest.len = mpn_add(&mut dest.dig, &tmp, tmp.len(), &[round_up as Dig], 1);
                }
            }
        }
    }
}

pub fn add_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    let ld = dig_copy(lhs);
    let rd = dig_copy(rhs);
    let ln = lhs.neg;
    let rn = rhs.neg;
    let (big, small) = if mpn_cmp(&ld, ld.len(), &rd, rd.len()) >= 0 {
        (ld, rd)
    } else {
        (rd, ld)
    };
    let (bn, sn) = if mpn_cmp(&dig_copy(lhs), lhs.len, &dig_copy(rhs), rhs.len) >= 0 {
        (ln, rn)
    } else {
        (rn, ln)
    };
    if ln == rn {
        need_dig(dest, big.len() + 1);
        dest.len = mpn_add(&mut dest.dig, &big, big.len(), &small, small.len());
        dest.neg = bn && dest.len > 0;
    } else {
        need_dig(dest, big.len());
        dest.len = mpn_sub(&mut dest.dig, &big, big.len(), &small, small.len());
        dest.neg = bn && dest.len > 0;
    }
}

pub fn sub_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    let ld = dig_copy(lhs);
    let rd = dig_copy(rhs);
    let mut neg = false;
    let (big, small, bn, sn) = if mpn_cmp(&ld, ld.len(), &rd, rd.len()) >= 0 {
        (ld, rd, lhs.neg, rhs.neg)
    } else {
        neg = true;
        (rd, ld, rhs.neg, lhs.neg)
    };
    if bn != sn {
        need_dig(dest, big.len() + 1);
        dest.len = mpn_add(&mut dest.dig, &big, big.len(), &small, small.len());
    } else {
        need_dig(dest, big.len());
        dest.len = mpn_sub(&mut dest.dig, &big, big.len(), &small, small.len());
    }
    if dest.len == 0 {
        dest.neg = false;
    } else if neg {
        dest.neg = !bn;
    } else {
        dest.neg = bn;
    }
}

pub fn and_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    let (lhs, rhs) = if lhs.len < rhs.len {
        (rhs, lhs)
    } else {
        (lhs, rhs)
    };
    if mpconfig::OPT_MPZ_BITWISE && !lhs.neg && !rhs.neg {
        need_dig(dest, lhs.len);
        if mpconfig::OPT_MPZ_BITWISE {
            for i in 0..rhs.len {
                dest.dig[i] = lhs.dig[i] & rhs.dig[i];
            }
            dest.len = mpn_remove_trailing_zeros(&mut dest.dig, rhs.len);
        }
        dest.neg = false;
    } else {
        need_dig(dest, lhs.len + 1);
        dest.len = mpn_and_neg(
            &mut dest.dig,
            &lhs.dig[..lhs.len],
            lhs.len,
            &rhs.dig[..rhs.len],
            rhs.len,
            if lhs.neg == rhs.neg {
                lhs.neg as DblDig
            } else {
                0
            },
            lhs.neg as DblDig,
            rhs.neg as DblDig,
        );
        dest.neg = lhs.neg && rhs.neg;
    }
}

pub fn or_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    let (lhs, rhs) = if lhs.len < rhs.len {
        (rhs, lhs)
    } else {
        (lhs, rhs)
    };
    if mpconfig::OPT_MPZ_BITWISE && !lhs.neg && !rhs.neg {
        need_dig(dest, lhs.len);
        for i in 0..rhs.len {
            dest.dig[i] = lhs.dig[i] | rhs.dig[i];
        }
        for i in rhs.len..lhs.len {
            dest.dig[i] = lhs.dig[i];
        }
        dest.len = lhs.len;
        dest.neg = false;
    } else if mpconfig::OPT_MPZ_BITWISE {
        need_dig(dest, lhs.len + 1);
        dest.len = mpn_or_neg(
            &mut dest.dig,
            &lhs.dig[..lhs.len],
            lhs.len,
            &rhs.dig[..rhs.len],
            rhs.len,
            lhs.neg as DblDig,
            rhs.neg as DblDig,
        );
        dest.neg = true;
    } else {
        need_dig(dest, lhs.len + if lhs.neg || rhs.neg { 1 } else { 0 });
        dest.len = mpn_or_neg(
            &mut dest.dig,
            &lhs.dig[..lhs.len],
            lhs.len,
            &rhs.dig[..rhs.len],
            rhs.len,
            lhs.neg as DblDig,
            rhs.neg as DblDig,
        );
        dest.neg = lhs.neg || rhs.neg;
    }
}

pub fn xor_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    let (lhs, rhs) = if lhs.len < rhs.len {
        (rhs, lhs)
    } else {
        (lhs, rhs)
    };
    if mpconfig::OPT_MPZ_BITWISE && lhs.neg == rhs.neg {
        need_dig(dest, lhs.len + if lhs.neg { 0 } else { 0 });
        dest.len = if !lhs.neg {
            for i in 0..rhs.len {
                dest.dig[i] = lhs.dig[i] ^ rhs.dig[i];
            }
            for i in rhs.len..lhs.len {
                dest.dig[i] = lhs.dig[i];
            }
            mpn_remove_trailing_zeros(&mut dest.dig, lhs.len)
        } else {
            mpn_xor_neg(
                &mut dest.dig,
                &lhs.dig[..lhs.len],
                lhs.len,
                &rhs.dig[..rhs.len],
                rhs.len,
                0,
                0,
                0,
            )
        };
        dest.neg = false;
    } else if mpconfig::OPT_MPZ_BITWISE {
        need_dig(dest, lhs.len + 1);
        dest.len = mpn_xor_neg(
            &mut dest.dig,
            &lhs.dig[..lhs.len],
            lhs.len,
            &rhs.dig[..rhs.len],
            rhs.len,
            1,
            if !lhs.neg { 1 } else { 0 },
            if !rhs.neg { 1 } else { 0 },
        );
        dest.neg = true;
    } else {
        need_dig(dest, lhs.len + if lhs.neg || rhs.neg { 1 } else { 0 });
        dest.len = mpn_xor_neg(
            &mut dest.dig,
            &lhs.dig[..lhs.len],
            lhs.len,
            &rhs.dig[..rhs.len],
            rhs.len,
            if lhs.neg != rhs.neg { 1 } else { 0 },
            if !lhs.neg { 1 } else { 0 },
            if !rhs.neg { 1 } else { 0 },
        );
        dest.neg = lhs.neg ^ rhs.neg;
    }
}

pub fn mul_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    if lhs.len == 0 || rhs.len == 0 {
        set_from_int(dest, 0);
        return;
    }
    let mut lhs = lhs;
    let mut rhs = rhs;
    let mut tmp_lhs = None;
    let mut tmp_rhs = None;
    if std::ptr::eq(lhs, dest) {
        tmp_lhs = Some(mpz_clone(lhs));
        lhs = tmp_lhs.as_ref().unwrap();
        if std::ptr::eq(rhs, dest) {
            rhs = lhs;
        }
    } else if std::ptr::eq(rhs, dest) {
        tmp_rhs = Some(mpz_clone(rhs));
        rhs = tmp_rhs.as_ref().unwrap();
    }
    need_dig(dest, lhs.len + rhs.len);
    dest.dig.fill(0);
    dest.len = mpn_mul(
        &mut dest.dig,
        &lhs.dig[..lhs.len],
        lhs.len,
        &rhs.dig[..rhs.len],
        rhs.len,
    );
    dest.neg = lhs.neg != rhs.neg;
}

pub fn pow_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    if lhs.len == 0 || rhs.neg {
        set_from_int(dest, 0);
        return;
    }
    if rhs.len == 0 {
        set_from_int(dest, 1);
        return;
    }
    let mut x = mpz_clone(lhs);
    let mut n = mpz_clone(rhs);
    set_from_int(dest, 1);
    while n.len > 0 {
        if (n.dig[0] & 1) != 0 {
            let d = mpz_clone(dest);
            mul_inpl(dest, &d, &x);
        }
        let nlen = n.len;
        let nd = n.dig[..nlen].to_vec();
        n.len = mpn_shr(&mut n.dig, &nd, nlen, 1);
        if n.len == 0 {
            break;
        }
        let xc = mpz_clone(&x);
        mul_inpl(&mut x, &xc, &xc);
    }
}

pub fn pow3_inpl(dest: &mut Mpz, lhs: &Mpz, rhs: &Mpz, modulus: &Mpz) {
    if lhs.len == 0 || rhs.neg || (modulus.len == 1 && modulus.dig[0] == 1) {
        set_from_int(dest, 0);
        return;
    }
    set_from_int(dest, 1);
    if rhs.len == 0 {
        return;
    }
    let mut x = mpz_clone(lhs);
    let mut n = mpz_clone(rhs);
    let mut quo = Mpz {
        neg: false,
        fixed_dig: false,
        alloc: 0,
        len: 0,
        dig: Vec::new(),
    };
    init_zero(&mut quo);
    while n.len > 0 {
        if (n.dig[0] & 1) != 0 {
            let dcopy = mpz_clone(dest);
            mul_inpl(dest, &dcopy, &x);
            let mut rem = mpz_clone(dest);
            divmod_inpl(&mut quo, &mut rem, dest, modulus);
            set(dest, &rem);
        }
        let nlen = n.len;
        let nd = n.dig[..nlen].to_vec();
        n.len = mpn_shr(&mut n.dig, &nd, nlen, 1);
        if n.len == 0 {
            break;
        }
        let xc = mpz_clone(&x);
        mul_inpl(&mut x, &xc, &xc);
        let mut xrem = mpz_clone(&x);
        divmod_inpl(&mut quo, &mut xrem, &x, modulus);
        x = xrem;
    }
    deinit(&mut quo);
}

pub fn divmod_inpl(dest_quo: &mut Mpz, dest_rem: &mut Mpz, lhs: &Mpz, rhs: &Mpz) {
    debug_assert!(!is_zero(rhs));
    let lhs_copy = dig_copy(lhs);
    let rhs_copy = dig_copy(rhs);
    need_dig(dest_quo, lhs_copy.len() + 1);
    dest_quo.dig.fill(0);
    dest_quo.neg = false;
    dest_quo.len = 0;
    need_dig(dest_rem, lhs_copy.len() + 1);
    dest_rem.neg = lhs.neg;
    dest_rem.len = lhs_copy.len();
    dest_rem.dig[..lhs_copy.len()].copy_from_slice(&lhs_copy);
    let mut qlen = 0usize;
    mpn_div(
        &mut dest_rem.dig,
        &mut dest_rem.len,
        &rhs_copy,
        rhs_copy.len(),
        &mut dest_quo.dig,
        &mut qlen,
    );
    dest_quo.len = qlen;
    dest_rem.neg &= dest_rem.len > 0;
    if lhs_copy.len() > 0 && lhs.neg != rhs.neg {
        dest_quo.neg = dest_quo.len > 0;
        if !is_zero(dest_rem) {
            let mut m1 = Mpz::default();
            init_from_int(&mut m1, -1);
            let quo_tmp = dig_copy(dest_quo);
            let q = Mpz {
                neg: dest_quo.neg,
                fixed_dig: false,
                alloc: quo_tmp.len(),
                len: quo_tmp.len(),
                dig: quo_tmp,
            };
            add_inpl(dest_quo, &q, &m1);
            let rem_tmp = dig_copy(dest_rem);
            let r = Mpz {
                neg: dest_rem.neg,
                fixed_dig: false,
                alloc: rem_tmp.len(),
                len: rem_tmp.len(),
                dig: rem_tmp,
            };
            add_inpl(dest_rem, &r, rhs);
            deinit(&mut m1);
        }
    }
}

pub fn hash(z: &Mpz) -> Int {
    let mut val: Uint = 0;
    for &d in z.dig[..z.len].iter().rev() {
        val = (val << DIG_SIZE) | d as Uint;
    }
    if z.neg {
        -(val as Int)
    } else {
        val as Int
    }
}

pub fn as_int_checked(z: &Mpz, value: &mut Int) -> bool {
    let mut val: Uint = 0;
    for &d in z.dig[..z.len].iter().rev() {
        if val > (!(obj::WORD_MSBIT_HIGH) >> DIG_SIZE as usize) {
            return false;
        }
        val = (val << DIG_SIZE) | d as Uint;
    }
    if z.neg {
        val = val.wrapping_neg();
    }
    *value = val as Int;
    true
}

pub fn as_uint_checked(z: &Mpz, value: &mut Uint) -> bool {
    if z.neg {
        return false;
    }
    let mut val: Uint = 0;
    for &d in z.dig[..z.len].iter().rev() {
        if val > (!(obj::WORD_MSBIT_HIGH) >> (DIG_SIZE as usize - 1)) {
            return false;
        }
        val = (val << DIG_SIZE) | d as Uint;
    }
    *value = val;
    true
}

pub fn as_bytes(z: &Mpz, big_endian: bool, as_signed: bool, len: usize, buf: &mut [Byte]) -> bool {
    let fill_byte: Byte = if z.neg { 0xff } else { 0x00 };
    let mut olen = len;
    let mut bits = 0i32;
    let mut d: DblDig = 0;
    let mut carry: DblDig = 1;
    let mut val: Dig = 0;
    let mut zidx = 0usize;
    while zidx < z.len {
        bits += DIG_SIZE as i32;
        d = (d << DIG_SIZE) | z.dig[zidx] as DblDig;
        zidx += 1;
        while bits >= 8 {
            bits -= 8;
            d >>= 8;
            val = (d & 0xff) as Dig;
            if z.neg {
                val = (!val & 0xff) as Dig + carry as Dig;
                carry = (val >> 8) as DblDig;
                val &= 0xff;
            }
            if olen == 0 {
                if val as Byte != fill_byte {
                    return false;
                }
                continue;
            }
            if big_endian {
                buf[len - olen] = val as Byte;
            } else {
                buf[len - olen] = val as Byte;
            }
            olen -= 1;
        }
    }
    if olen == 0 && as_signed && ((val & 0x80) != (fill_byte as Dig & 0x80)) {
        return false;
    }
    if olen > 0 {
        buf[(len - olen)..len].fill(fill_byte);
    }
    true
}

pub fn as_float(z: &Mpz) -> f64 {
    let mut val = 0.0f64;
    for &d in z.dig[..z.len].iter().rev() {
        val = val * DIG_BASE as f64 + d as f64;
    }
    if z.neg {
        -val
    } else {
        val
    }
}

pub fn max_num_bits(z: &Mpz) -> usize {
    z.len * DIG_SIZE as usize
}

pub fn as_str_inpl(
    z: &Mpz,
    base: u32,
    prefix: Option<&str>,
    base_char: u8,
    comma: u8,
    out: &mut [u8],
) -> usize {
    debug_assert!((2..=32).contains(&base));
    let ilen = z.len;
    if ilen == 0 {
        let mut pos = 0usize;
        if let Some(p) = prefix {
            pos += p.len();
            out[..pos].copy_from_slice(p.as_bytes());
        }
        out[pos] = b'0';
        return pos + 1;
    }
    let mut dig = vec![0 as Dig; ilen];
    dig.copy_from_slice(&z.dig[..ilen]);
    let mut s = 0usize;
    let n_comma = if base == 10 { 3 } else { 4 };
    let mut last_comma = 0usize;
    loop {
        let mut a: DblDig = 0;
        let mut done = true;
        for d in dig.iter_mut().rev() {
            a = (a << DIG_SIZE) | *d as DblDig;
            *d = (a / base as DblDig) as Dig;
            a %= base as DblDig;
        }
        let mut ch = (a as u8 + b'0') as u8;
        if ch > b'9' {
            ch = ch - 10 + base_char;
        }
        out[s] = ch;
        s += 1;
        for d in &dig {
            if *d != 0 {
                done = false;
                break;
            }
        }
        if done {
            break;
        }
        if comma != 0 && (s - last_comma) == n_comma {
            out[s] = comma;
            s += 1;
            last_comma = s;
        }
    }
    if let Some(p) = prefix {
        for c in p.as_bytes().iter().rev() {
            out[s] = *c;
            s += 1;
        }
    }
    if z.neg {
        out[s] = b'-';
        s += 1;
    }
    out[..s].reverse();
    s
}
