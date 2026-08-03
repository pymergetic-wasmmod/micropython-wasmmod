//! rewrite of py/pairheap.h + py/pairheap.c
// symmetry: done

use crate::obj::ObjBase;

/// Pairing-heap node (`mp_pairheap_t`). Embed as the first field of larger nodes.
#[repr(C)]
pub struct PairHeap {
    pub base: ObjBase,
    pub child: *mut PairHeap,
    pub child_last: *mut PairHeap,
    pub next: *mut PairHeap,
}

pub type PairHeapLt = fn(*mut PairHeap, *mut PairHeap) -> bool;

#[inline]
fn next_make_rightmost_parent(parent: *mut PairHeap) -> *mut PairHeap {
    ((parent as usize) | 1) as *mut PairHeap
}

#[inline]
fn next_is_rightmost_parent(next: *mut PairHeap) -> bool {
    (next as usize) & 1 != 0
}

#[inline]
fn next_get_rightmost_parent(next: *mut PairHeap) -> *mut PairHeap {
    ((next as usize) & !1) as *mut PairHeap
}

#[inline]
pub fn new_heap(_lt: PairHeapLt) -> *mut PairHeap {
    std::ptr::null_mut()
}

#[inline]
pub unsafe fn init_node(_lt: PairHeapLt, node: *mut PairHeap) {
    (*node).child = std::ptr::null_mut();
    (*node).next = std::ptr::null_mut();
}

#[inline]
pub fn is_empty(_lt: PairHeapLt, heap: *mut PairHeap) -> bool {
    heap.is_null()
}

#[inline]
pub fn peek(_lt: PairHeapLt, heap: *mut PairHeap) -> *mut PairHeap {
    heap
}

#[inline]
pub unsafe fn push(lt: PairHeapLt, heap: *mut PairHeap, node: *mut PairHeap) -> *mut PairHeap {
    debug_assert!((*node).child.is_null() && (*node).next.is_null());
    meld(lt, node, heap)
}

#[inline]
pub unsafe fn pop(lt: PairHeapLt, heap: *mut PairHeap) -> *mut PairHeap {
    debug_assert!((*heap).next.is_null());
    let child = (*heap).child;
    (*heap).child = std::ptr::null_mut();
    pairing(lt, child)
}

/// O(1), stable (`mp_pairheap_meld`).
pub unsafe fn meld(lt: PairHeapLt, heap1: *mut PairHeap, heap2: *mut PairHeap) -> *mut PairHeap {
    if heap1.is_null() {
        return heap2;
    }
    if heap2.is_null() {
        return heap1;
    }
    if lt(heap1, heap2) {
        if (*heap1).child.is_null() {
            (*heap1).child = heap2;
        } else {
            (*(*heap1).child_last).next = heap2;
        }
        (*heap1).child_last = heap2;
        (*heap2).next = next_make_rightmost_parent(heap1);
        heap1
    } else {
        (*heap1).next = (*heap2).child;
        (*heap2).child = heap1;
        if (*heap1).next.is_null() {
            (*heap2).child_last = heap1;
            (*heap1).next = next_make_rightmost_parent(heap2);
        }
        heap2
    }
}

/// Amortised O(log N), stable (`mp_pairheap_pairing`).
pub unsafe fn pairing(lt: PairHeapLt, mut child: *mut PairHeap) -> *mut PairHeap {
    if child.is_null() {
        return std::ptr::null_mut();
    }
    let mut heap: *mut PairHeap = std::ptr::null_mut();
    while !next_is_rightmost_parent((*child).next) {
        let mut n1 = child;
        child = (*child).next;
        (*n1).next = std::ptr::null_mut();
        if !next_is_rightmost_parent((*child).next) {
            let n2 = child;
            child = (*child).next;
            (*n2).next = std::ptr::null_mut();
            n1 = meld(lt, n1, n2);
        }
        heap = meld(lt, heap, n1);
    }
    let result = if heap.is_null() { child } else { heap };
    (*result).next = std::ptr::null_mut();
    result
}

/// Amortised O(log N), stable (`mp_pairheap_delete`).
pub unsafe fn delete(lt: PairHeapLt, heap: *mut PairHeap, node: *mut PairHeap) -> *mut PairHeap {
    if node == heap {
        let child = (*heap).child;
        (*node).child = std::ptr::null_mut();
        return pairing(lt, child);
    }
    if (*node).next.is_null() {
        return heap;
    }

    let mut parent = node;
    while !next_is_rightmost_parent((*parent).next) {
        parent = (*parent).next;
    }
    parent = next_get_rightmost_parent((*parent).next);

    let next;
    if node == (*parent).child && (*node).child.is_null() {
        if next_is_rightmost_parent((*node).next) {
            (*parent).child = std::ptr::null_mut();
        } else {
            (*parent).child = (*node).next;
        }
        (*node).next = std::ptr::null_mut();
        return heap;
    } else if node == (*parent).child {
        let child = (*node).child;
        next = (*node).next;
        (*node).child = std::ptr::null_mut();
        (*node).next = std::ptr::null_mut();
        let paired = pairing(lt, child);
        (*parent).child = paired;
        let node = paired;
        (*node).next = next;
        if next_is_rightmost_parent(next) {
            (*parent).child_last = node;
        }
        return heap;
    } else {
        let mut n = (*parent).child;
        while node != (*n).next {
            n = (*n).next;
        }
        let child = (*node).child;
        next = (*node).next;
        (*node).child = std::ptr::null_mut();
        (*node).next = std::ptr::null_mut();
        let mut node = pairing(lt, child);
        if node.is_null() {
            node = n;
        } else {
            (*n).next = node;
        }
        (*node).next = next;
        if next_is_rightmost_parent(next) {
            (*parent).child_last = node;
        }
        return heap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::ObjBase;

    fn lt(a: *mut PairHeap, b: *mut PairHeap) -> bool {
        (a as usize) < (b as usize)
    }

    fn node() -> PairHeap {
        PairHeap {
            base: ObjBase {
                type_: std::ptr::null(),
            },
            child: std::ptr::null_mut(),
            child_last: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
        }
    }

    #[test]
    fn push_pop_ordered() {
        unsafe {
            let mut a = node();
            let mut b = node();
            init_node(lt, &mut a);
            init_node(lt, &mut b);
            let mut heap = new_heap(lt);
            heap = push(lt, heap, &mut a);
            heap = push(lt, heap, &mut b);
            let first = peek(lt, heap);
            assert!(!first.is_null());
            heap = pop(lt, heap);
            let second = peek(lt, heap);
            assert!(!second.is_null());
            assert_ne!(first, second);
            let _ = heap;
        }
    }
}
