//! rewrite of shared/runtime/softtimer.c + shared/runtime/softtimer.h
// symmetry: done

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, Once};
use std::thread;
use std::time::Duration;

use py_rs::gc;
use py_rs::mphal;
use py_rs::obj::Obj;
use py_rs::pairheap::{self, PairHeap, PairHeapLt};
use py_rs::runtime::{self, HandlePendingBehaviour};

pub const FLAG_PY_CALLBACK: u16 = 1;
pub const FLAG_GC_ALLOCATED: u16 = 2;
pub const FLAG_HARD_CALLBACK: u16 = 4;

pub const MODE_ONE_SHOT: u16 = 1;
pub const MODE_PERIODIC: u16 = 2;

pub static SOFT_TIMER_NEXT: AtomicU32 = AtomicU32::new(0);

#[repr(C)]
pub struct SoftTimerEntry {
    pub pairheap: PairHeap,
    pub flags: u16,
    pub mode: u16,
    pub expiry_ms: u32,
    pub delta_ms: u32,
    pub py_callback: Obj,
    pub c_callback: Option<fn(*mut SoftTimerEntry)>,
}

static mut SOFT_TIMER_HEAP: *mut SoftTimerEntry = std::ptr::null_mut();

fn soft_timer_lt(n1: *mut PairHeap, n2: *mut PairHeap) -> bool {
    let e1 = n1 as *mut SoftTimerEntry;
    let e2 = n2 as *mut SoftTimerEntry;
    soft_timer_ticks_diff(unsafe { (*e1).expiry_ms }, unsafe { (*e2).expiry_ms }) < 0
}

pub fn soft_timer_ticks_diff(t1: u32, t0: u32) -> i32 {
    t1.wrapping_sub(t0) as i32
}

fn soft_timer_get_ms() -> u32 {
    mphal::ticks_ms() as u32
}

fn soft_timer_schedule_at_ms(ticks_ms: u32) {
    let uw_tick = mphal::ticks_ms() as u32;
    let next = if soft_timer_ticks_diff(ticks_ms, uw_tick) <= 0 {
        uw_tick.wrapping_add(1)
    } else {
        ticks_ms
    };
    SOFT_TIMER_NEXT.store(next, Ordering::Relaxed);
}

pub fn deinit() {
    unsafe {
        let mut heap_from = SOFT_TIMER_HEAP;
        let mut heap_to = pairheap::new_heap(soft_timer_lt as PairHeapLt) as *mut SoftTimerEntry;
        while !heap_from.is_null() {
            let entry = pairheap::peek(soft_timer_lt as PairHeapLt, heap_from as *mut PairHeap)
                as *mut SoftTimerEntry;
            heap_from = pairheap::pop(soft_timer_lt as PairHeapLt, heap_from as *mut PairHeap)
                as *mut SoftTimerEntry;
            if (*entry).flags & FLAG_GC_ALLOCATED == 0 {
                heap_to = pairheap::push(
                    soft_timer_lt as PairHeapLt,
                    heap_to as *mut PairHeap,
                    entry as *mut PairHeap,
                ) as *mut SoftTimerEntry;
            }
        }
        SOFT_TIMER_HEAP = heap_to;
    }
}

pub fn handler() {
    unsafe {
        let ticks_ms = soft_timer_get_ms();
        let mut heap = SOFT_TIMER_HEAP;
        while !heap.is_null() && soft_timer_ticks_diff((*heap).expiry_ms, ticks_ms) <= 0 {
            let entry = heap;
            heap = pairheap::pop(soft_timer_lt as PairHeapLt, heap as *mut PairHeap) as *mut SoftTimerEntry;
            if (*entry).flags & FLAG_PY_CALLBACK != 0 {
                if super::mpirq::dispatch(
                    (*entry).py_callback,
                    py_rs::obj::from_ptr(entry as *const ()),
                    (*entry).flags & FLAG_HARD_CALLBACK != 0,
                ) != 0
                {
                    (*entry).mode = MODE_ONE_SHOT;
                }
            } else if let Some(cb) = (*entry).c_callback {
                cb(entry);
            }
            if (*entry).mode == MODE_PERIODIC {
                (*entry).expiry_ms = (*entry).expiry_ms.wrapping_add((*entry).delta_ms);
                heap = pairheap::push(
                    soft_timer_lt as PairHeapLt,
                    heap as *mut PairHeap,
                    entry as *mut PairHeap,
                ) as *mut SoftTimerEntry;
            }
        }
        SOFT_TIMER_HEAP = heap;
        if !heap.is_null() {
            soft_timer_schedule_at_ms((*heap).expiry_ms);
        }
    }
}

pub fn gc_mark_all() {
    unsafe {
        let mut heap_from = SOFT_TIMER_HEAP;
        let mut heap_to = pairheap::new_heap(soft_timer_lt as PairHeapLt) as *mut SoftTimerEntry;
        while !heap_from.is_null() {
            let entry = pairheap::peek(soft_timer_lt as PairHeapLt, heap_from as *mut PairHeap)
                as *mut SoftTimerEntry;
            heap_from = pairheap::pop(soft_timer_lt as PairHeapLt, heap_from as *mut PairHeap)
                as *mut SoftTimerEntry;
            if (*entry).flags & FLAG_GC_ALLOCATED != 0 {
                gc::collect_root(&[entry as *mut u8]);
            }
            heap_to = pairheap::push(
                soft_timer_lt as PairHeapLt,
                heap_to as *mut PairHeap,
                entry as *mut PairHeap,
            ) as *mut SoftTimerEntry;
        }
        SOFT_TIMER_HEAP = heap_to;
    }
}

pub fn static_init(entry: &mut SoftTimerEntry, mode: u16, delta_ms: u32, cb: fn(*mut SoftTimerEntry)) {
    assert_eq!(core::mem::offset_of!(SoftTimerEntry, pairheap), 0);
    unsafe {
        pairheap::init_node(soft_timer_lt as PairHeapLt, &mut entry.pairheap as *mut PairHeap);
    }
    entry.flags = 0;
    entry.mode = mode;
    entry.delta_ms = delta_ms;
    entry.c_callback = Some(cb);
    entry.py_callback = py_rs::obj::OBJ_NULL;
}

pub fn insert(entry: &mut SoftTimerEntry, initial_delta_ms: u32) {
    unsafe {
        pairheap::init_node(soft_timer_lt as PairHeapLt, &mut entry.pairheap as *mut PairHeap);
    }
    entry.expiry_ms = soft_timer_get_ms().wrapping_add(initial_delta_ms);
    unsafe {
        SOFT_TIMER_HEAP = pairheap::push(
            soft_timer_lt as PairHeapLt,
            SOFT_TIMER_HEAP as *mut PairHeap,
            entry as *mut _ as *mut PairHeap,
        ) as *mut SoftTimerEntry;
        if entry as *const SoftTimerEntry == SOFT_TIMER_HEAP {
            soft_timer_schedule_at_ms(entry.expiry_ms);
        }
    }
}

pub fn remove(entry: &mut SoftTimerEntry) {
    unsafe {
        SOFT_TIMER_HEAP = pairheap::delete(
            soft_timer_lt as PairHeapLt,
            SOFT_TIMER_HEAP as *mut PairHeap,
            &mut entry.pairheap as *mut PairHeap,
        ) as *mut SoftTimerEntry;
    }
}

pub fn reinsert(entry: &mut SoftTimerEntry, initial_delta_ms: u32) {
    remove(entry);
    insert(entry, initial_delta_ms);
}

static HANDLER_LOCK: Mutex<()> = Mutex::new(());

fn handler_locked() {
    let _guard = HANDLER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    handler();
}

/// Run due soft timers and dispatch scheduled callbacks (host unix path).
pub fn poll() {
    let now = mphal::ticks_ms() as u32;
    let next = SOFT_TIMER_NEXT.load(Ordering::Relaxed);
    if next != 0 && soft_timer_ticks_diff(next, now) <= 0 {
        handler_locked();
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
    }
}

/// Sleep for `ms`, servicing soft timers and scheduler callbacks while waiting.
pub fn delay_ms(ms: u32) {
    if ms == 0 {
        poll();
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
        return;
    }
    let end = mphal::ticks_ms().wrapping_add(ms as usize) as u32;
    loop {
        poll();
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
        let now = mphal::ticks_ms() as u32;
        if soft_timer_ticks_diff(end, now) <= 0 {
            break;
        }
        let next = SOFT_TIMER_NEXT.load(Ordering::Relaxed);
        let wait = if next != 0 {
            soft_timer_ticks_diff(next, now).max(0) as u64
        } else {
            soft_timer_ticks_diff(end, now) as u64
        };
        thread::sleep(Duration::from_millis(wait.min(10).max(1)));
    }
}

static HOST_INIT: Once = Once::new();

/// Start background thread that fires soft timers using `mphal::ticks_ms`.
pub fn init_host() {
    HOST_INIT.call_once(|| {
        gc::register_collect_hook(gc_mark_all);
        thread::Builder::new()
            .name("mpy-soft-timer".into())
            .spawn(host_thread_main)
            .expect("soft timer thread");
    });
}

fn host_thread_main() {
    loop {
        let now = mphal::ticks_ms() as u32;
        let next = SOFT_TIMER_NEXT.load(Ordering::Relaxed);
        if next != 0 && soft_timer_ticks_diff(next, now) <= 0 {
            handler_locked();
        } else {
            let sleep_ms = if next != 0 {
                soft_timer_ticks_diff(next, now).max(0) as u64
            } else {
                10
            };
            thread::sleep(Duration::from_millis(sleep_ms.min(10).max(1)));
        }
    }
}
