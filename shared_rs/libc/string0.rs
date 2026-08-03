//! rewrite of shared/libc/string0.c
// symmetry: done

#[inline]
fn likely_aligned(ptr: *const u8) -> bool {
    (ptr as usize) & 3 == 0
}

/// `memcpy`.
pub unsafe fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if likely_aligned(dst) && likely_aligned(src) {
        let mut d = dst as *mut u32;
        let mut s = src as *const u32;
        for _ in 0..(n >> 2) {
            d.write(s.read());
            d = d.add(1);
            s = s.add(1);
        }
        let mut tail = (n & 3) as isize;
        let mut d8 = d as *mut u8;
        let mut s8 = s as *const u8;
        if tail & 2 != 0 {
            (d8 as *mut u16).write_unaligned((s8 as *const u16).read_unaligned());
            d8 = d8.add(2);
            s8 = s8.add(2);
            tail -= 2;
        }
        if tail & 1 != 0 {
            *d8 = *s8;
        }
    } else {
        let mut d = dst;
        let mut s = src;
        for _ in 0..n {
            d.write(s.read());
            d = d.add(1);
            s = s.add(1);
        }
    }
    dst
}

/// `__memcpy_chk`.
pub unsafe fn memcpy_chk(dest: *mut u8, src: *const u8, len: usize, slen: usize) -> *mut u8 {
    if len > slen {
        return core::ptr::null_mut();
    }
    memcpy(dest, src, len)
}

/// `memmove`.
pub unsafe fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 && (dest as *const u8) < unsafe { src.add(n) } {
        let mut d = dest.add(n).sub(1);
        let mut s = src.add(n).sub(1);
        for _ in 0..n {
            d.write(s.read());
            d = d.sub(1);
            s = s.sub(1);
        }
        dest
    } else {
        memcpy(dest, src, n)
    }
}

/// `memset`.
pub unsafe fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let byte = c as u8;
    if byte == 0 && likely_aligned(s) {
        let mut s32 = s as *mut u32;
        for _ in 0..(n >> 2) {
            s32.write(0);
            s32 = s32.add(1);
        }
        let mut s8 = s32 as *mut u8;
        if n & 2 != 0 {
            (s8 as *mut u16).write_unaligned(0);
            s8 = s8.add(2);
        }
        if n & 1 != 0 {
            *s8 = 0;
        }
    } else {
        let mut p = s;
        for _ in 0..n {
            p.write(byte);
            p = p.add(1);
        }
    }
    s
}

/// `memcmp`.
pub unsafe fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut p1 = s1;
    let mut p2 = s2;
    for _ in 0..n {
        let c1 = p1.read() as i8;
        let c2 = p2.read() as i8;
        if c1 < c2 {
            return -1;
        }
        if c1 > c2 {
            return 1;
        }
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    0
}

/// `memchr`.
pub unsafe fn memchr(s: *const u8, c: i32, n: usize) -> *mut u8 {
    if n == 0 {
        return core::ptr::null_mut();
    }
    let target = c as u8;
    let mut p = s;
    for _ in 0..n {
        if p.read() == target {
            return p as *mut u8;
        }
        p = p.add(1);
    }
    core::ptr::null_mut()
}

/// `strlen`.
pub unsafe fn strlen(str: *const u8) -> usize {
    let mut len = 0usize;
    let mut s = str;
    while s.read() != 0 {
        len += 1;
        s = s.add(1);
    }
    len
}

/// `strcmp`.
pub unsafe fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut p1 = s1;
    let mut p2 = s2;
    loop {
        let c1 = p1.read();
        let c2 = p2.read();
        if c1 != 0 && c2 != 0 {
            if c1 < c2 {
                return -1;
            }
            if c1 > c2 {
                return 1;
            }
            p1 = p1.add(1);
            p2 = p2.add(1);
            continue;
        }
        if c2 != 0 {
            return -1;
        }
        if c1 != 0 {
            return 1;
        }
        return 0;
    }
}

/// `strncmp`.
pub unsafe fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut p1 = s1;
    let mut p2 = s2;
    let mut remaining = n;
    while remaining > 0 {
        let c1 = p1.read();
        let c2 = p2.read();
        if c1 != 0 && c2 != 0 {
            remaining -= 1;
            if c1 < c2 {
                return -1;
            }
            if c1 > c2 {
                return 1;
            }
            p1 = p1.add(1);
            p2 = p2.add(1);
            continue;
        }
        if remaining == 0 {
            return 0;
        }
        if c2 != 0 {
            return -1;
        }
        if c1 != 0 {
            return 1;
        }
        return 0;
    }
    0
}

/// `strcpy`.
pub unsafe fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    let mut d = dest;
    let mut s = src;
    while s.read() != 0 {
        d.write(s.read());
        d = d.add(1);
        s = s.add(1);
    }
    d.write(0);
    dest
}

/// `strncpy`.
pub unsafe fn strncpy(s1: *mut u8, s2: *const u8, n: usize) -> *mut u8 {
    let mut dst = s1;
    let mut src = s2;
    let mut remaining = n;
    while remaining > 0 {
        remaining -= 1;
        let ch = src.read();
        dst.write(ch);
        if ch == 0 {
            memset(dst.add(1), 0, remaining);
            break;
        }
        dst = dst.add(1);
        src = src.add(1);
    }
    s1
}

/// `stpcpy`.
pub unsafe fn stpcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    let mut d = dest;
    let mut s = src;
    while s.read() != 0 {
        d.write(s.read());
        d = d.add(1);
        s = s.add(1);
    }
    d.write(0);
    d
}

/// `strcat`.
pub unsafe fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    let mut d = dest;
    while d.read() != 0 {
        d = d.add(1);
    }
    let mut s = src;
    while s.read() != 0 {
        d.write(s.read());
        d = d.add(1);
        s = s.add(1);
    }
    d.write(0);
    dest
}

/// `strchr`.
pub unsafe fn strchr(s: *const u8, c: i32) -> *mut u8 {
    let target = c as u8;
    let mut p = s;
    while p.read() != 0 && p.read() != target {
        p = p.add(1);
    }
    if p.read() == target {
        p as *mut u8
    } else {
        core::ptr::null_mut()
    }
}

/// `strstr`.
pub unsafe fn strstr(haystack: *const u8, needle: *const u8) -> *mut u8 {
    if needle.read() == 0 {
        return haystack as *mut u8;
    }
    let needle_len = strlen(needle);
    let mut h = haystack as *mut u8;
    loop {
        h = strchr(h, needle.read() as i32);
        if h.is_null() {
            return core::ptr::null_mut();
        }
        if strncmp(h, needle, needle_len) == 0 {
            return h;
        }
        h = h.add(1);
    }
}

/// `strspn`.
pub unsafe fn strspn(s: *const u8, accept: *const u8) -> usize {
    let start = s;
    let mut p = s;
    while p.read() != 0 && !strchr(accept, p.read() as i32).is_null() {
        p = p.add(1);
    }
    p.offset_from(start) as usize
}

/// `strcspn`.
pub unsafe fn strcspn(s: *const u8, reject: *const u8) -> usize {
    let start = s;
    let mut p = s;
    while p.read() != 0 && strchr(reject, p.read() as i32).is_null() {
        p = p.add(1);
    }
    p.offset_from(start) as usize
}

/// Weak `atoi` for decimal non-negative integers.
pub fn atoi(num: *const u8) -> i32 {
    unsafe {
        let mut value = 0i32;
        let mut p = num;
        while p.read() >= b'0' && p.read() <= b'9' {
            value = value * 10 + (p.read() - b'0') as i32;
            p = p.add(1);
        }
        value
    }
}
