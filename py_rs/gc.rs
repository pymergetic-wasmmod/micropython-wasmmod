//! Host mark-and-sweep translation of `py/gc.h` and `py/gc.c`.
// symmetry: done

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::mpconfig;
use crate::mpstate;

const BLOCK_BYTES: usize = mpconfig::BYTES_PER_GC_BLOCK as usize;
const AT_FREE: u8 = 0;
const AT_HEAD: u8 = 1;
const AT_TAIL: u8 = 2;
const AT_MARK: u8 = 3;
pub const ALLOC_FLAG_HAS_FINALISER: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcInfo {
    pub total: usize,
    pub used: usize,
    pub free: usize,
    pub max_free: usize,
    pub num_1block: usize,
    pub num_2block: usize,
    pub max_block: usize,
}

struct Heap {
    /// The allocation table uses the same two-bit states as MicroPython's ATB.
    atb: Vec<u8>,
    finalisers: Vec<bool>,
    buf: Vec<u8>,
    roots: Vec<usize>,
    lock_depth: usize,
    collecting: bool,
    auto_collect: bool,
    last_free: usize,
    mark_work: Vec<usize>,
    /// `MP_STATE_MEM(total_bytes_allocated)` when `MEM_STATS`.
    total_bytes_allocated: usize,
    /// `MP_STATE_MEM(current_bytes_allocated)`.
    current_bytes_allocated: usize,
    /// `MP_STATE_MEM(peak_bytes_allocated)`.
    peak_bytes_allocated: usize,
}

impl Heap {
    fn new(size: usize) -> Self {
        let blocks = size / BLOCK_BYTES;
        Self {
            atb: vec![AT_FREE; blocks],
            finalisers: vec![false; blocks],
            // Extra alignment is supplied by Vec's allocator; allocation starts
            // are always block aligned relative to this backing store.
            buf: vec![0; blocks * BLOCK_BYTES],
            roots: Vec::new(),
            lock_depth: 0,
            collecting: false,
            auto_collect: true,
            last_free: 0,
            mark_work: Vec::new(),
            total_bytes_allocated: 0,
            current_bytes_allocated: 0,
            peak_bytes_allocated: 0,
        }
    }

    fn ptr_for(&mut self, block: usize) -> *mut u8 {
        unsafe { self.buf.as_mut_ptr().add(block * BLOCK_BYTES) }
    }

    fn block_for_ptr(&self, ptr: *const u8) -> Option<usize> {
        let base = self.buf.as_ptr() as usize;
        let address = ptr as usize;
        if address < base || address >= base + self.buf.len() || (address - base) % BLOCK_BYTES != 0
        {
            return None;
        }
        Some((address - base) / BLOCK_BYTES)
    }

    fn allocation_blocks(&self, start: usize) -> usize {
        if self.atb.get(start) != Some(&AT_HEAD) && self.atb.get(start) != Some(&AT_MARK) {
            return 0;
        }
        1 + self.atb[start + 1..]
            .iter()
            .take_while(|&&kind| kind == AT_TAIL)
            .count()
    }

    fn find_free(&self, needed: usize) -> Option<usize> {
        let blocks = self.atb.len();
        for offset in 0..blocks {
            let start = (self.last_free + offset) % blocks;
            if start + needed <= blocks
                && self.atb[start..start + needed]
                    .iter()
                    .all(|&kind| kind == AT_FREE)
            {
                return Some(start);
            }
        }
        None
    }

    fn allocate(&mut self, bytes: usize, flags: u32) -> Option<*mut u8> {
        if bytes == 0 || self.lock_depth != 0 {
            return None;
        }
        let needed = bytes.checked_add(BLOCK_BYTES - 1)? / BLOCK_BYTES;
        let mut start = self.find_free(needed);
        if start.is_none() && self.auto_collect {
            self.collect();
            start = self.find_free(needed);
        }
        let start = start?;
        self.atb[start] = AT_HEAD;
        for kind in &mut self.atb[start + 1..start + needed] {
            *kind = AT_TAIL;
        }
        self.finalisers[start] = flags & ALLOC_FLAG_HAS_FINALISER != 0;
        self.last_free = (start + needed) % self.atb.len().max(1);
        let allocated = needed * BLOCK_BYTES;
        let ptr = self.ptr_for(start);
        unsafe { std::ptr::write_bytes(ptr, 0, allocated) };
        if mpconfig::MEM_STATS {
            self.total_bytes_allocated = self.total_bytes_allocated.saturating_add(allocated);
            self.current_bytes_allocated = self.current_bytes_allocated.saturating_add(allocated);
            if self.current_bytes_allocated > self.peak_bytes_allocated {
                self.peak_bytes_allocated = self.current_bytes_allocated;
            }
        }
        Some(ptr)
    }

    fn mark_block(&mut self, start: usize, work: &mut Vec<usize>) {
        if self.atb.get(start) == Some(&AT_HEAD) {
            self.atb[start] = AT_MARK;
            work.push(start);
        }
    }

    fn collect_start(&mut self) {
        assert!(!self.collecting, "nested gc collection");
        self.collecting = true;
        self.mark_work.clear();
    }

    fn mark_roots(&mut self, roots: &[usize]) {
        let mut work = std::mem::take(&mut self.mark_work);
        for &root in roots {
            if let Some(block) = self.block_for_ptr(root as *const u8) {
                self.mark_block(block, &mut work);
            }
        }
        self.mark_work = work;
    }

    fn trace_and_sweep(&mut self) {
        run_collect_hooks();
        let hook_roots = HOOK_ROOTS.with(|r| std::mem::take(&mut *r.borrow_mut()));
        if !hook_roots.is_empty() {
            let roots: Vec<usize> = hook_roots.iter().map(|&p| p as usize).collect();
            self.mark_roots(&roots);
        }
        let mut work = std::mem::take(&mut self.mark_work);
        debug_assert!(self.collecting);
        while let Some(block) = work.pop() {
            let bytes = self.allocation_blocks(block) * BLOCK_BYTES;
            let base = block * BLOCK_BYTES;
            for offset in (0..bytes).step_by(std::mem::size_of::<usize>()) {
                let candidate = unsafe {
                    std::ptr::read_unaligned(self.buf.as_ptr().add(base + offset) as *const usize)
                };
                if let Some(child) = self.block_for_ptr(candidate as *const u8) {
                    self.mark_block(child, &mut work);
                }
            }
        }
        self.sweep();
        self.collecting = false;
    }

    fn collect(&mut self) {
        if self.collecting {
            return;
        }
        self.collect_start();
        self.mark_roots(&self.roots.clone());
        self.trace_and_sweep();
    }

    fn sweep(&mut self) {
        let mut block = 0;
        while block < self.atb.len() {
            match self.atb[block] {
                AT_MARK => {
                    self.atb[block] = AT_HEAD;
                    block += 1;
                }
                AT_HEAD => {
                    let count = self.allocation_blocks(block);
                    for index in block..block + count {
                        self.atb[index] = AT_FREE;
                        self.finalisers[index] = false;
                        let ptr = self.ptr_for(index);
                        unsafe { std::ptr::write_bytes(ptr, 0, BLOCK_BYTES) };
                    }
                    block += count;
                }
                _ => block += 1,
            }
        }
        self.last_free = 0;
    }

    fn free(&mut self, ptr: *mut u8) {
        if ptr.is_null() || (self.lock_depth != 0 && !self.collecting) {
            return;
        }
        let Some(block) = self.block_for_ptr(ptr) else {
            return;
        };
        let count = self.allocation_blocks(block);
        if count == 0 {
            return;
        }
        for index in block..block + count {
            self.atb[index] = AT_FREE;
            self.finalisers[index] = false;
        }
        if mpconfig::MEM_STATS {
            let bytes = count * BLOCK_BYTES;
            self.current_bytes_allocated = self.current_bytes_allocated.saturating_sub(bytes);
        }
        self.last_free = self.last_free.min(block);
    }

    fn info(&self) -> GcInfo {
        let mut info = GcInfo {
            total: self.buf.len(),
            ..GcInfo::default()
        };
        let mut block = 0;
        let mut free_run = 0;
        while block < self.atb.len() {
            if self.atb[block] == AT_FREE {
                info.free += BLOCK_BYTES;
                free_run += 1;
                block += 1;
                continue;
            }
            info.max_free = info.max_free.max(free_run * BLOCK_BYTES);
            free_run = 0;
            let count = self.allocation_blocks(block);
            if count == 0 {
                block += 1;
                continue;
            }
            info.used += count * BLOCK_BYTES;
            info.max_block = info.max_block.max(count);
            if count == 1 {
                info.num_1block += 1;
            }
            if count == 2 {
                info.num_2block += 1;
            }
            block += count;
        }
        info.max_free = info.max_free.max(free_run * BLOCK_BYTES);
        info
    }
}

static HEAP: Mutex<Option<Heap>> = Mutex::new(None);
static COLLECT_HOOKS: Mutex<Vec<fn()>> = Mutex::new(Vec::new());

thread_local! {
    /// True while `trace_and_sweep` runs hooks — `collect_root*` must not re-lock HEAP.
    static IN_COLLECT_HOOKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static HOOK_ROOTS: std::cell::RefCell<Vec<*mut u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Register a callback invoked during collection after roots are marked and
/// before tracing (mirrors port `gc_collect` hooks such as `soft_timer_gc_mark_all`).
pub fn register_collect_hook(hook: fn()) {
    COLLECT_HOOKS
        .lock()
        .expect("gc hook lock poisoned")
        .push(hook);
}

fn run_collect_hooks() {
    IN_COLLECT_HOOKS.with(|c| c.set(true));
    {
        let hooks = COLLECT_HOOKS.lock().expect("gc hook lock poisoned");
        for hook in hooks.iter() {
            hook();
        }
    }
    IN_COLLECT_HOOKS.with(|c| c.set(false));
}

static HEAP_SIZE_OVERRIDE: AtomicUsize = AtomicUsize::new(0);

/// Override the default GC heap size before the first `init()` (e.g. `-X heapsize=`).
pub fn set_heap_size(size: usize) {
    HEAP_SIZE_OVERRIDE.store(size, Ordering::Relaxed);
}

fn configured_heap_size() -> usize {
    let override_size = HEAP_SIZE_OVERRIDE.load(Ordering::Relaxed);
    if override_size >= 700 {
        override_size
    } else {
        mpconfig::GC_HEAP_SIZE
    }
}

/// Initialise the GC heap with an explicit size (sets override then inits).
pub fn init_with_size(size: usize) {
    set_heap_size(size);
    init();
}

/// Initialise the GC heap once. Subsequent calls are no-ops so static types that
/// hold GC pointers (locals dicts, etc.) are not left dangling across test/`mp_init` reentry.
pub fn init() {
    let mut heap = HEAP.lock().expect("gc lock poisoned");
    if heap.is_none() {
        *heap = Some(Heap::new(configured_heap_size()));
    }
    // Mirror C `gc_init`: allow auto collection.
    drop(heap);
    mpstate::with_mem(|mem| mem.gc_auto_collect_enabled = 1);
    set_auto_collect(true);
}

fn with_heap<R>(f: impl FnOnce(&mut Heap) -> R) -> Option<R> {
    let mut guard = HEAP.lock().expect("gc lock poisoned");
    guard.as_mut().map(f)
}

/// Rust-facing allocator retained for the existing `malloc` translation.
/// GC blocks are naturally aligned; requests exceeding block alignment fail.
pub fn alloc(bytes: usize, align: usize) -> Option<*mut u8> {
    if align > BLOCK_BYTES || !align.is_power_of_two() {
        return None;
    }
    with_heap(|heap| heap.allocate(bytes, 0)).flatten()
}

pub fn gc_alloc(bytes: usize, flags: u32) -> Option<*mut u8> {
    with_heap(|heap| heap.allocate(bytes, flags)).flatten()
}

pub fn free(ptr: *mut u8) {
    let _ = with_heap(|heap| heap.free(ptr));
}

pub fn gc_free(ptr: *mut u8) {
    free(ptr);
}

pub fn nbytes(ptr: *const u8) -> usize {
    with_heap(|heap| {
        heap.block_for_ptr(ptr)
            .map(|block| heap.allocation_blocks(block) * BLOCK_BYTES)
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

pub fn gc_nbytes(ptr: *const u8) -> usize {
    nbytes(ptr)
}

pub fn realloc(ptr: *mut u8, bytes: usize, allow_move: bool) -> Option<*mut u8> {
    if ptr.is_null() {
        return gc_alloc(bytes, 0);
    }
    if bytes == 0 {
        free(ptr);
        return None;
    }
    with_heap(|heap| {
        let block = heap.block_for_ptr(ptr)?;
        let old_blocks = heap.allocation_blocks(block);
        if old_blocks == 0 || heap.lock_depth != 0 {
            return None;
        }
        let new_blocks = bytes.checked_add(BLOCK_BYTES - 1)? / BLOCK_BYTES;
        if new_blocks <= old_blocks {
            for index in block + new_blocks..block + old_blocks {
                heap.atb[index] = AT_FREE;
            }
            return Some(ptr);
        }
        if block + new_blocks <= heap.atb.len()
            && heap.atb[block + old_blocks..block + new_blocks]
                .iter()
                .all(|&kind| kind == AT_FREE)
        {
            for index in block + old_blocks..block + new_blocks {
                heap.atb[index] = AT_TAIL;
            }
            return Some(ptr);
        }
        if !allow_move {
            return None;
        }
        let new_ptr = heap.allocate(bytes, 0)?;
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, new_ptr, old_blocks * BLOCK_BYTES);
        }
        heap.free(ptr);
        Some(new_ptr)
    })
    .flatten()
}

pub fn gc_realloc(ptr: *mut u8, bytes: usize, allow_move: bool) -> Option<*mut u8> {
    realloc(ptr, bytes, allow_move)
}

pub fn lock() {
    let _ = with_heap(|heap| heap.lock_depth += 1);
}
pub fn unlock() {
    let _ = with_heap(|heap| heap.lock_depth = heap.lock_depth.saturating_sub(1));
}
pub fn is_locked() -> bool {
    with_heap(|heap| heap.lock_depth != 0).unwrap_or(false)
}
pub fn set_auto_collect(enabled: bool) {
    let _ = with_heap(|heap| heap.auto_collect = enabled);
}

/// Register a precise root.  Root addresses must be GC allocation heads.
pub fn add_root(ptr: *mut u8) {
    let _ = with_heap(|heap| heap.roots.push(ptr as usize));
}
pub fn remove_root(ptr: *mut u8) {
    let _ = with_heap(|heap| heap.roots.retain(|&root| root != ptr as usize));
}

/// Mark explicit root pointers, mirroring `gc_collect_root`.
pub fn collect_root(ptrs: &[*mut u8]) {
    if IN_COLLECT_HOOKS.with(|c| c.get()) {
        HOOK_ROOTS.with(|r| r.borrow_mut().extend_from_slice(ptrs));
        return;
    }
    let _ = with_heap(|heap| {
        assert!(heap.collecting, "gc_collect_root without gc_collect_start");
        let roots = ptrs.iter().map(|&ptr| ptr as usize).collect::<Vec<_>>();
        heap.mark_roots(&roots);
    });
}

/// Scan `len` pointer-sized words at `base` for heap references (C `gc_collect_root`).
pub fn collect_root_words(base: *const u8, len: usize) {
    if IN_COLLECT_HOOKS.with(|c| c.get()) {
        // Treat each word as a potential root pointer (same as C gc_collect_root).
        HOOK_ROOTS.with(|r| {
            let mut v = r.borrow_mut();
            for i in 0..len {
                let ptr = unsafe {
                    std::ptr::read_unaligned(
                        base.add(i * core::mem::size_of::<usize>()) as *const usize
                    )
                };
                v.push(ptr as *mut u8);
            }
        });
        return;
    }
    let _ = with_heap(|heap| {
        assert!(heap.collecting, "gc_collect_root without gc_collect_start");
        let mut work = std::mem::take(&mut heap.mark_work);
        for i in 0..len {
            let ptr = unsafe {
                std::ptr::read_unaligned(base.add(i * core::mem::size_of::<usize>()) as *const usize)
            };
            if let Some(block) = heap.block_for_ptr(ptr as *const u8) {
                heap.mark_block(block, &mut work);
            }
        }
        heap.mark_work = work;
    });
}
pub fn collect_start() {
    let _ = with_heap(|heap| heap.collect_start());
}
pub fn collect_end() {
    let _ = with_heap(|heap| heap.trace_and_sweep());
}
pub fn collect() {
    let _ = with_heap(|heap| heap.collect());
}
pub fn sweep_all() {
    collect();
}
pub fn info_full() -> GcInfo {
    with_heap(|heap| heap.info()).unwrap_or_default()
}
/// Backward-compatible compact info: `(used, total)`.
pub fn info() -> (usize, usize) {
    let info = info_full();
    (info.used, info.total)
}

/// `m_get_total_bytes_allocated`
pub fn mem_total_bytes() -> usize {
    with_heap(|h| h.total_bytes_allocated).unwrap_or(0)
}
/// `m_get_current_bytes_allocated`
pub fn mem_current_bytes() -> usize {
    with_heap(|h| h.current_bytes_allocated).unwrap_or(0)
}
/// `m_get_peak_bytes_allocated`
pub fn mem_peak_bytes() -> usize {
    with_heap(|h| h.peak_bytes_allocated).unwrap_or(0)
}

/// `gc_dump_info`
pub fn dump_info(print: &crate::mpprint::Print) {
    let info = info_full();
    crate::mpprint::printf(
        print,
        "GC: total: %u, used: %u, free: %u\n",
        [
            crate::mpprint::VaArg::USize(info.total),
            crate::mpprint::VaArg::USize(info.used),
            crate::mpprint::VaArg::USize(info.free),
        ]
        .into_iter(),
    );
    crate::mpprint::printf(
        print,
        " No. of 1-blocks: %u, 2-blocks: %u, max blk sz: %u, max free sz: %u\n",
        [
            crate::mpprint::VaArg::USize(info.num_1block),
            crate::mpprint::VaArg::USize(info.num_2block),
            crate::mpprint::VaArg::USize(info.max_block),
            crate::mpprint::VaArg::USize(info.max_free),
        ]
        .into_iter(),
    );
}

/// `gc_dump_alloc_table` — compact ATB dump when `mem_info` is given an arg.
pub fn dump_alloc_table(print: &crate::mpprint::Print) {
    let _ = with_heap(|heap| {
        crate::mpprint::printf(
            print,
            "GC memory layout; from %p\n",
            std::iter::once(crate::mpprint::VaArg::USize(heap.buf.as_ptr() as usize)),
        );
        let mut col = 0usize;
        for (i, &kind) in heap.atb.iter().enumerate() {
            let ch: &[u8] = match kind {
                AT_FREE => b".",
                AT_HEAD => b"h",
                AT_TAIL => b"=",
                AT_MARK => b"m",
                _ => b"?",
            };
            if col == 0 {
                crate::mpprint::printf(
                    print,
                    "%04u: ",
                    std::iter::once(crate::mpprint::VaArg::USize(i)),
                );
            }
            crate::mpprint::print_str(print, core::str::from_utf8(ch).unwrap_or("?"));
            col += 1;
            if col >= 64 {
                crate::mpprint::print_str(print, "\n");
                col = 0;
            }
        }
        if col != 0 {
            crate::mpprint::print_str(print, "\n");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweeps_unreachable_and_retains_explicit_roots() {
        init();
        let rooted = gc_alloc(1, 0).unwrap();
        let unreachable = gc_alloc(BLOCK_BYTES + 1, 0).unwrap();
        add_root(rooted);
        collect();
        assert_eq!(gc_nbytes(rooted), BLOCK_BYTES);
        assert_eq!(gc_nbytes(unreachable), 0);
        remove_root(rooted);
        collect();
        assert_eq!(gc_nbytes(rooted), 0);
    }

    #[test]
    fn collect_hook_runs_during_collection() {
        init();
        static CALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn hook() {
            CALLED.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        register_collect_hook(hook);
        collect();
        assert!(CALLED.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn realloc_grows_and_free_reclaims_chain() {
        init();
        let ptr = gc_alloc(1, 0).unwrap();
        let ptr = gc_realloc(ptr, BLOCK_BYTES * 2, false).unwrap();
        assert_eq!(gc_nbytes(ptr), BLOCK_BYTES * 2);
        gc_free(ptr);
        assert_eq!(gc_nbytes(ptr), 0);
    }
}
