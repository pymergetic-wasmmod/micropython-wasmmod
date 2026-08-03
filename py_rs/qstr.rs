//! Faithful host translation of `py/qstr.h` and `py/qstr.c`.
// symmetry: done

use std::sync::Mutex;

use crate::misc::Byte;
use crate::mpconfig;

pub type Qstr = usize;
pub const QSTR_NULL: Qstr = 0;

/// A deliberately small replacement for generated qstrdefs.  It is a proper
/// immutable first pool, so dynamically interned strings always retain stable
/// IDs and later generated tables can be appended without changing the model.
const STATIC_QSTRS: &[&[u8]] = &[
    b"", b"__name__", b"__main__", b"__class__", b"__init__", b"__del__",
    b"__repr__", b"__str__", b"__iter__", b"__next__", b"None", b"True",
    b"False", b"Ellipsis", b"NotImplemented", b"print", b"len", b"range",
    b"Exception", b"TypeError", b"ValueError", b"RuntimeError", b"zip",
];
pub const QSTR_EMPTY: Qstr = 1;
/// Last qstr in the static pool (`QSTR_LAST_STATIC` / `MP_QSTR_zip`).
pub const QSTR_LAST_STATIC: Qstr = STATIC_QSTRS.len();

/// djb2 hash used by MicroPython (`qstr_compute_hash`).
pub fn compute_hash(data: &[Byte]) -> usize {
    let bits = 8 * mpconfig::QSTR_BYTES_IN_HASH;
    let mask = if bits >= usize::BITS as usize { usize::MAX } else { (1usize << bits) - 1 };
    let mut hash = 5381usize;
    for &byte in data {
        hash = hash.wrapping_mul(33) ^ usize::from(byte);
    }
    let hash = hash & mask;
    if hash == 0 { 1 } else { hash }
}

#[derive(Clone, Debug)]
struct QstrEntry {
    hash: usize,
    data: Vec<u8>,
}

/// Equivalent logical layout to `qstr_pool_t`: every pool has a preceding
/// length, capacity and entries.  Rust owns dynamic string bytes directly,
/// avoiding C's separate `qstr_last_chunk` allocation while retaining IDs.
#[derive(Debug)]
struct QstrPool {
    total_prev_len: usize,
    is_sorted: bool,
    alloc: usize,
    entries: Vec<QstrEntry>,
}

#[derive(Debug)]
struct PoolChain {
    pools: Vec<QstrPool>,
}

impl PoolChain {
    fn new() -> Self {
        let entries = STATIC_QSTRS.iter().map(|s| QstrEntry {
            hash: compute_hash(s),
            data: s.to_vec(),
        }).collect::<Vec<_>>();
        Self {
            pools: vec![QstrPool {
                total_prev_len: 0,
                is_sorted: false,
                alloc: entries.len(),
                entries,
            }],
        }
    }

    fn find(&self, data: &[u8]) -> Qstr {
        if data.is_empty() {
            return QSTR_EMPTY;
        }
        let hash = compute_hash(data);
        for pool in self.pools.iter().rev() {
            if let Some(index) = pool.entries.iter().position(|entry| {
                entry.hash == hash && entry.data.as_slice() == data
            }) {
                return pool.total_prev_len + index + 1;
            }
        }
        QSTR_NULL
    }

    fn add(&mut self, data: &[u8]) -> Qstr {
        if let Some(last) = self.pools.last() {
            if last.entries.len() >= last.alloc {
                let total_prev_len = last.total_prev_len + last.entries.len();
                self.pools.push(QstrPool {
                    total_prev_len,
                    is_sorted: false,
                    alloc: last.alloc.max(mpconfig::ALLOC_QSTR_ENTRIES_INIT) * 2,
                    entries: Vec::new(),
                });
            }
        }
        let pool = self.pools.last_mut().expect("qstr static pool");
        pool.entries.push(QstrEntry { hash: compute_hash(data), data: data.to_vec() });
        pool.total_prev_len + pool.entries.len()
    }

    fn entry(&self, q: Qstr) -> Option<&QstrEntry> {
        if q == QSTR_NULL {
            return None;
        }
        let index = q - 1;
        self.pools.iter().rev().find_map(|pool| {
            index.checked_sub(pool.total_prev_len)
                .and_then(|i| pool.entries.get(i))
        })
    }
}

static POOL: Mutex<Option<PoolChain>> = Mutex::new(None);

fn with_pool<R>(f: impl FnOnce(&mut PoolChain) -> R) -> R {
    let mut guard = POOL.lock().expect("qstr pool lock poisoned");
    let pool = guard.get_or_insert_with(PoolChain::new);
    f(pool)
}

/// Initialise / reset qstr state, corresponding to `qstr_init`.
pub fn init() {
    let mut guard = POOL.lock().expect("qstr pool lock poisoned");
    if guard.is_none() {
        *guard = Some(PoolChain::new());
    }
}

/// Find an interned byte string, returning `QSTR_NULL` when absent.
pub fn find_strn(data: &[u8]) -> Qstr {
    with_pool(|pool| pool.find(data))
}

/// Intern `data` and return its stable qstr ID (`qstr_from_strn`).
pub fn from_strn(data: &[u8]) -> Qstr {
    with_pool(|pool| {
        let existing = pool.find(data);
        if existing == QSTR_NULL { pool.add(data) } else { existing }
    })
}

pub fn from_str(s: &str) -> Qstr {
    from_strn(s.as_bytes())
}

pub fn qstr_hash(q: Qstr) -> Option<usize> {
    with_pool(|pool| pool.entry(q).map(|entry| entry.hash))
}

pub fn qstr_len(q: Qstr) -> Option<usize> {
    with_pool(|pool| pool.entry(q).map(|entry| entry.data.len()))
}

/// Host-safe equivalent of C's borrowed `qstr_str`: returns owned,
/// NUL-terminated bytes so callers cannot outlive the pool lock.
pub fn qstr_str(q: Qstr) -> Option<Vec<u8>> {
    with_pool(|pool| pool.entry(q).map(|entry| {
        let mut data = entry.data.clone();
        data.push(0);
        data
    }))
}

/// Host-safe equivalent of C's `qstr_data`.
pub fn qstr_data(q: Qstr) -> Option<(Vec<u8>, usize)> {
    with_pool(|pool| pool.entry(q).map(|entry| (entry.data.clone(), entry.data.len())))
}

pub fn str_data(q: Qstr) -> Option<Vec<u8>> {
    qstr_data(q).map(|(data, _)| data)
}

pub fn str_from_qstr(q: Qstr) -> Option<String> {
    str_data(q).and_then(|bytes| String::from_utf8(bytes).ok())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolInfo {
    pub pools: usize,
    pub qstrs: usize,
    pub string_data_bytes: usize,
    pub total_bytes: usize,
}

/// Total number of interned qstrs (`QSTR_TOTAL()`).
pub fn total() -> Qstr {
    with_pool(|pool| {
        pool.pools
            .last()
            .map(|p| p.total_prev_len + p.entries.len())
            .unwrap_or(0)
    })
}

pub fn pool_info() -> PoolInfo {
    with_pool(|chain| {
        let dynamic = chain.pools.iter().skip(1);
        let mut info = PoolInfo::default();
        for pool in dynamic {
            info.pools += 1;
            info.qstrs += pool.entries.len();
            info.string_data_bytes += pool.entries.iter().map(|entry| entry.data.len() + 1).sum::<usize>();
            info.total_bytes += std::mem::size_of::<QstrPool>()
                + pool.alloc * std::mem::size_of::<QstrEntry>();
        }
        info.total_bytes += info.string_data_bytes;
        info
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interned_strings_have_stable_identity_and_metadata() {
        init();
        let first = from_strn(b"metal\0python");
        assert_eq!(first, from_strn(b"metal\0python"));
        assert_eq!(qstr_len(first), Some(12));
        assert_eq!(qstr_hash(first), Some(compute_hash(b"metal\0python")));
        assert_eq!(qstr_str(first), Some(b"metal\0python\0".to_vec()));
        assert_eq!(find_strn(b"missing"), QSTR_NULL);
    }
}
