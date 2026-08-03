//! rewrite of py/sequence.c
// symmetry: done

use crate::mpconfig;
use crate::obj::{self, Int, Obj};
use crate::objlist;
use crate::objslice::{self, BoundSlice};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::BinaryOp;

/// `mp_seq_multiply`
pub fn multiply(items: &[u8], item_sz: usize, len: usize, times: usize, dest: &mut [u8]) {
    let copy_sz = item_sz * len;
    let mut offset = 0usize;
    for _ in 0..times {
        dest[offset..offset + copy_sz].copy_from_slice(&items[..copy_sz]);
        offset += copy_sz;
    }
}

/// `mp_seq_get_fast_slice_indexes`
pub fn get_fast_slice_indexes(len: usize, slice: Obj, indexes: &mut BoundSlice) -> bool {
    debug_assert!(mpconfig::PY_BUILTINS_SLICE);
    objslice::slice_indices(slice, len as Int, indexes);
    if indexes.step < 0 {
        indexes.stop += 1;
    }
    if indexes.step > 0 && indexes.start > indexes.stop {
        indexes.stop = indexes.start;
    } else if indexes.step < 0 && indexes.start < indexes.stop {
        indexes.stop = indexes.start + 1;
    }
    indexes.step == 1
}

/// `mp_seq_extract_slice`
pub fn extract_slice(seq: &[Obj], indexes: &BoundSlice) -> Obj {
    let mut start = indexes.start;
    let stop = indexes.stop;
    let step = indexes.step;
    let res = objlist::new_list(0, None);
    if step < 0 {
        while start >= stop {
            objlist::list_append(res, seq[start as usize]);
            start += step;
        }
    } else {
        while start < stop {
            objlist::list_append(res, seq[start as usize]);
            start += step;
        }
    }
    res
}

/// `mp_seq_cmp_bytes` — do not pass `NotEqual`.
pub fn cmp_bytes(op: BinaryOp, data1: &[u8], data2: &[u8]) -> bool {
    let mut op = op;
    let (data1, data2) = if matches!(op, BinaryOp::Less | BinaryOp::LessEqual) {
        op = match op {
            BinaryOp::Less => BinaryOp::More,
            BinaryOp::LessEqual => BinaryOp::MoreEqual,
            _ => op,
        };
        (data2, data1)
    } else {
        (data1, data2)
    };

    if op == BinaryOp::Equal && data1.len() != data2.len() {
        return false;
    }

    let min_len = data1.len().min(data2.len());
    let res = data1[..min_len].cmp(&data2[..min_len]);
    if op == BinaryOp::Equal {
        return res == std::cmp::Ordering::Equal;
    }
    if res == std::cmp::Ordering::Less {
        return false;
    }
    if res == std::cmp::Ordering::Greater {
        return true;
    }
    if data1.len() != data2.len() {
        if data1.len() < data2.len() {
            return false;
        }
    } else if op == BinaryOp::More {
        return false;
    }
    true
}

/// `mp_seq_cmp_objs` — do not pass `NotEqual`.
pub fn cmp_objs(op: BinaryOp, items1: &[Obj], items2: &[Obj]) -> bool {
    let mut op = op;
    let (items1, items2) = if matches!(op, BinaryOp::Less | BinaryOp::LessEqual) {
        op = match op {
            BinaryOp::Less => BinaryOp::More,
            BinaryOp::LessEqual => BinaryOp::MoreEqual,
            _ => op,
        };
        (items2, items1)
    } else {
        (items1, items2)
    };

    if op == BinaryOp::Equal && items1.len() != items2.len() {
        return false;
    }

    let len = items1.len().min(items2.len());
    for i in 0..len {
        if obj::equal(items1[i], items2[i]) {
            continue;
        }
        if op == BinaryOp::Equal {
            return false;
        }
        return obj::is_true(runtime::binary_op_obj(op, items1[i], items2[i]));
    }

    if items1.len() != items2.len() {
        if items1.len() < items2.len() {
            return false;
        }
    } else if op == BinaryOp::More {
        return false;
    }
    true
}

/// `mp_seq_index_obj`
pub fn index_obj(items: &[Obj], len: usize, n_args: usize, args: &[Obj]) -> Obj {
    let type_ = obj::get_type(args[0]);
    let value = args[1];
    let mut start = 0usize;
    let mut stop = len;
    if n_args >= 3 {
        start = obj::get_index(type_, len, args[2], true);
        if n_args >= 4 {
            stop = obj::get_index(type_, len, args[3], true);
        }
    }
    for i in start..stop {
        if obj::equal(items[i], value) {
            return obj::new_small_int(i as Int);
        }
    }
    raise::raise(MpRaise::ValueError("object not in sequence"));
}

/// `mp_seq_count_obj`
pub fn count_obj(items: &[Obj], len: usize, value: Obj) -> Obj {
    let mut count = 0usize;
    for i in 0..len {
        if obj::equal(items[i], value) {
            count += 1;
        }
    }
    obj::new_small_int(count as Int)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::objslice;

    fn setup() {
        let _ = gc::init();
    }

    #[test]
    fn multiply_copies() {
        let src = [1u8, 2, 3];
        let mut dest = [0u8; 6];
        multiply(&src, 1, 3, 2, &mut dest);
        assert_eq!(dest, [1, 2, 3, 1, 2, 3]);
    }

    #[test]
    fn cmp_bytes_ordering() {
        assert!(cmp_bytes(BinaryOp::Equal, b"abc", b"abc"));
        assert!(!cmp_bytes(BinaryOp::Equal, b"abc", b"abd"));
        assert!(cmp_bytes(BinaryOp::Less, b"abc", b"abd"));
        assert!(cmp_bytes(BinaryOp::More, b"abd", b"abc"));
    }

    #[test]
    fn cmp_objs_equal() {
        let a = [
            obj::new_small_int(1),
            obj::new_small_int(2),
            obj::new_small_int(3),
        ];
        let b = [
            obj::new_small_int(1),
            obj::new_small_int(2),
            obj::new_small_int(3),
        ];
        assert!(cmp_objs(BinaryOp::Equal, &a, &b));
        assert!(!cmp_objs(BinaryOp::Equal, &a, &a[..2]));
    }

    #[test]
    fn index_and_count() {
        setup();
        let list = objlist::new_list(0, None);
        let items = [
            obj::new_small_int(10),
            obj::new_small_int(20),
            obj::new_small_int(10),
        ];
        let args = [list, obj::new_small_int(10)];
        let idx = index_obj(&items, 3, 2, &args);
        assert_eq!(obj::small_int_value(idx), 0);
        let cnt = count_obj(&items, 3, obj::new_small_int(10));
        assert_eq!(obj::small_int_value(cnt), 2);
    }

    #[test]
    fn extract_slice_forward() {
        setup();
        let seq = [
            obj::new_small_int(0),
            obj::new_small_int(1),
            obj::new_small_int(2),
            obj::new_small_int(3),
        ];
        let slice = objslice::new_slice(
            obj::new_small_int(1),
            obj::new_small_int(4),
            obj::new_small_int(2),
        );
        let mut bounds = BoundSlice {
            start: 0,
            stop: 0,
            step: 1,
        };
        get_fast_slice_indexes(4, slice, &mut bounds);
        let out = extract_slice(&seq, &bounds);
        let (_, items) = objlist::list_get(out);
        assert_eq!(items.len(), 2);
        assert_eq!(obj::small_int_value(items[0]), 1);
        assert_eq!(obj::small_int_value(items[1]), 3);
    }
}
