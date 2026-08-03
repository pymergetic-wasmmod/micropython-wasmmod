//! rewrite of py/mpstate.c + py/mpstate.h
// symmetry: done

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::gc;
use crate::mpconfig;
use crate::obj::{self, Obj, Uint};

/// VFS mount table entry (`mp_vfs_mount_t`).
#[derive(Debug, Clone)]
pub struct VfsMount {
    /// Mount point including leading `/` (e.g. `"/"`, `"/flash"`).
    pub mount_point: String,
    pub obj: Obj,
}

/// Current VFS working directory (`MP_STATE_VM(vfs_cur)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsCur {
    Root,
    Mount(usize),
}

/// Scheduler states (`MP_SCHED_*`).
pub const SCHED_IDLE: i16 = 1;
pub const SCHED_LOCKED: i16 = -1;
pub const SCHED_PENDING: i16 = 0;

/// GC memory area metadata (`mp_state_mem_area_t` subset for host).
#[derive(Debug, Clone, Default)]
pub struct MemArea {
    pub gc_last_free_atb_index: usize,
    pub gc_last_used_block: usize,
}

/// GC / allocator state (`mp_state_mem_t` host projection).
#[derive(Debug, Clone)]
pub struct MemState {
    pub area: MemArea,
    pub gc_stack_overflow: i32,
    /// C `gc_init` sets this to 1 (auto-collect on by default).
    pub gc_auto_collect_enabled: u16,
    pub gc_collected: usize,
}

impl Default for MemState {
    fn default() -> Self {
        Self {
            area: MemArea::default(),
            gc_stack_overflow: 0,
            gc_auto_collect_enabled: 1,
            gc_collected: 0,
        }
    }
}

/// Scheduled callback queue entry (`mp_sched_item_t`).
#[derive(Debug, Clone, Copy)]
pub struct SchedItem {
    pub func: Obj,
    pub arg: Obj,
}

/// VM / runtime state with GC root pointers (`mp_state_vm_t`).
#[derive(Debug, Clone)]
pub struct VmState {
    pub last_pool: usize,
    pub mp_emergency_exception_obj: Obj,
    pub mp_loaded_modules_dict: Obj,
    pub dict_main: Obj,
    pub mp_module_builtins_override_dict: Option<Obj>,
    pub qstr_last_chunk: Option<Vec<u8>>,
    pub qstr_last_alloc: usize,
    pub qstr_last_used: usize,
    pub mp_optimise_value: Uint,
    pub default_emit_opt: u8,
    pub mp_verbose_flag: Uint,
    pub sched_state: i16,
    pub sched_len: u8,
    pub sched_idx: u8,
    pub sched_queue: Vec<SchedItem>,
    pub vm_abort: bool,
    pub map_lookup_cache: Vec<u8>,
    pub vfs_mount_table: Vec<VfsMount>,
    pub vfs_cur: VfsCur,
    /// `MP_STATE_VM(dupterm_objs[])` when `PY_OS_DUPTERM > 0`.
    pub dupterm_objs: Vec<Obj>,
    /// GC-rooted host callback slots (`MP_STATE_VM(mp_wasm_host_slots)`).
    pub mp_wasm_host_slots: Obj,
    /// GC-rooted Python object handles (`MP_STATE_VM(mp_wasm_handles)`).
    pub mp_wasm_handles: Obj,
    /// Pack search path list (`MP_STATE_VM(mp_wasm_path_obj)`).
    pub mp_wasm_path: Obj,
    /// Pack arch tags list (`MP_STATE_VM(mp_wasm_arch_obj)`).
    pub mp_wasm_arch: Obj,
    /// Saved builtin `__import__` when hook installed (`MP_STATE_VM(mp_wasm_prev_import)`).
    pub mp_wasm_prev_import: Obj,
    /// `sys.argv` list (`MP_STATE_VM(mp_sys_argv_obj)`).
    pub mp_sys_argv: Obj,
    /// `sys.path` list (`MP_STATE_VM(sys_mutable[MP_SYS_MUTABLE_PATH])`).
    pub mp_sys_path: Obj,
    /// `sys.ps1` (`MP_STATE_VM(sys_mutable[MP_SYS_MUTABLE_PS1])`).
    pub sys_ps1: Obj,
    /// `sys.ps2` (`MP_STATE_VM(sys_mutable[MP_SYS_MUTABLE_PS2])`).
    pub sys_ps2: Obj,
    /// `sys.atexit` callback (`MP_STATE_VM(sys_exitfunc)`).
    pub sys_exitfunc: Obj,
    /// GIL mutex when `PY_THREAD_GIL` (`MP_STATE_VM(gil_mutex)`).
    pub gil_mutex: crate::mpthread::ThreadMutex,
}

impl Default for VmState {
    fn default() -> Self {
        Self {
            last_pool: 0,
            mp_emergency_exception_obj: obj::OBJ_NULL,
            mp_loaded_modules_dict: obj::OBJ_NULL,
            dict_main: obj::OBJ_NULL,
            mp_module_builtins_override_dict: None,
            qstr_last_chunk: None,
            qstr_last_alloc: 0,
            qstr_last_used: 0,
            mp_optimise_value: 0,
            default_emit_opt: 0,
            mp_verbose_flag: 0,
            sched_state: SCHED_IDLE,
            sched_len: 0,
            sched_idx: 0,
            sched_queue: vec![
                SchedItem {
                    func: obj::OBJ_NULL,
                    arg: obj::OBJ_NULL,
                };
                mpconfig::SCHEDULER_DEPTH as usize
            ],
            vm_abort: false,
            map_lookup_cache: vec![0; mpconfig::OPT_MAP_LOOKUP_CACHE_SIZE as usize],
            vfs_mount_table: Vec::new(),
            vfs_cur: VfsCur::Root,
            dupterm_objs: vec![obj::OBJ_NULL; mpconfig::PY_OS_DUPTERM],
            mp_wasm_host_slots: obj::OBJ_NULL,
            mp_wasm_handles: obj::OBJ_NULL,
            mp_wasm_path: obj::OBJ_NULL,
            mp_wasm_arch: obj::OBJ_NULL,
            mp_wasm_prev_import: obj::OBJ_NULL,
            mp_sys_argv: obj::OBJ_NULL,
            mp_sys_path: obj::OBJ_NULL,
            sys_ps1: obj::OBJ_NULL,
            sys_ps2: obj::OBJ_NULL,
            sys_exitfunc: obj::CONST_NONE,
            gil_mutex: crate::mpthread::ThreadMutex::default(),
        }
    }
}

/// Per-thread runtime state (`mp_state_thread_t`).
#[derive(Debug, Clone)]
pub struct ThreadState {
    pub stack_top: *mut u8,
    pub stack_limit: Uint,
    pub pystack_start: *mut u8,
    pub pystack_end: *mut u8,
    pub pystack_cur: *mut u8,
    pub gc_lock_depth: u16,
    pub dict_locals: Obj,
    pub dict_globals: Obj,
    pub nlr_top: Option<usize>,
    pub nlr_jump_callback_top: Option<usize>,
    pub mp_pending_exception: Obj,
    pub stop_iteration_arg: Obj,
    pub prof_trace_callback: Obj,
    pub prof_callback_is_executing: bool,
    pub current_code_state: Option<usize>,
}

impl Default for ThreadState {
    fn default() -> Self {
        Self {
            stack_top: std::ptr::null_mut(),
            stack_limit: 0,
            pystack_start: std::ptr::null_mut(),
            pystack_end: std::ptr::null_mut(),
            pystack_cur: std::ptr::null_mut(),
            gc_lock_depth: 0,
            dict_locals: obj::OBJ_NULL,
            dict_globals: obj::OBJ_NULL,
            nlr_top: None,
            nlr_jump_callback_top: None,
            mp_pending_exception: obj::OBJ_NULL,
            stop_iteration_arg: obj::OBJ_NULL,
            prof_trace_callback: obj::OBJ_NULL,
            prof_callback_is_executing: false,
            current_code_state: None,
        }
    }
}

/// Combined MicroPython state (`mp_state_ctx_t`).
#[derive(Debug, Clone, Default)]
pub struct StateCtx {
    pub thread: ThreadState,
    pub vm: VmState,
    pub mem: MemState,
}

thread_local! {
    static STATE: RefCell<Option<StateCtx>> = RefCell::new(None);
}

fn with_ctx<R>(f: impl FnOnce(&mut StateCtx) -> R) -> R {
    STATE.with(|state| {
        let mut guard = state.borrow_mut();
        if guard.is_none() {
            *guard = Some(StateCtx::default());
        }
        f(guard.as_mut().unwrap())
    })
}

/// Initialise global state container (`mp_state_ctx` bootstrap).
pub fn init() {
    with_ctx(|_| {});
}

/// Mark GC roots held in `mp_state_ctx` (C `gc_collect_start` root scan).
///
/// Intended as a collect hook so both `gc::collect()` and port `gc_collect`
/// keep live module/dict objects reachable. Dict maps use Rust `Vec` storage,
/// so entries are marked explicitly (the GC bitmap scan cannot see them).
pub fn mark_gc_roots() {
    if !mpconfig::ENABLE_GC {
        return;
    }
    let mut roots: Vec<*mut u8> = Vec::new();
    let mut push = |o: Obj| {
        if o != obj::OBJ_NULL {
            roots.push(obj::to_ptr(o) as *mut u8);
        }
    };
    with_thread(|t| {
        push(t.dict_locals);
        push(t.dict_globals);
        push(t.mp_pending_exception);
        push(t.stop_iteration_arg);
        push(t.prof_trace_callback);
        if mpconfig::ENABLE_PYSTACK && !t.pystack_start.is_null() && t.pystack_cur > t.pystack_start
        {
            let words =
                (t.pystack_cur as usize - t.pystack_start as usize) / core::mem::size_of::<usize>();
            gc::collect_root_words(t.pystack_start, words);
        }
    });
    with_vm(|vm| {
        push(vm.mp_emergency_exception_obj);
        push(vm.mp_loaded_modules_dict);
        push(vm.dict_main);
        if let Some(d) = vm.mp_module_builtins_override_dict {
            push(d);
        }
        for item in &vm.sched_queue {
            push(item.func);
            push(item.arg);
        }
        for m in &vm.vfs_mount_table {
            push(m.obj);
        }
        for o in &vm.dupterm_objs {
            push(*o);
        }
        push(vm.mp_wasm_host_slots);
        push(vm.mp_wasm_handles);
        push(vm.mp_wasm_path);
        push(vm.mp_wasm_arch);
        push(vm.mp_wasm_prev_import);
        push(vm.mp_sys_argv);
        push(vm.mp_sys_path);
        push(vm.sys_ps1);
        push(vm.sys_ps2);
        push(vm.sys_exitfunc);
    });
    if !roots.is_empty() {
        gc::collect_root(&roots);
    }

    // Deep-mark dict maps (Rust Vec) reachable from core roots.
    let (locals, globals, loaded, main) = with_ctx(|ctx| {
        (
            ctx.thread.dict_locals,
            ctx.thread.dict_globals,
            ctx.vm.mp_loaded_modules_dict,
            ctx.vm.dict_main,
        )
    });
    let mut visited = std::collections::HashSet::new();
    mark_dict_deep(locals, &mut visited);
    mark_dict_deep(globals, &mut visited);
    mark_dict_deep(main, &mut visited);
    mark_dict_deep(loaded, &mut visited);
    if let Some(b) = crate::objmodule::registered_builtins_globals() {
        mark_dict_deep(b, &mut visited);
    }
}

fn mark_dict_deep(dict_obj: Obj, visited: &mut std::collections::HashSet<usize>) {
    if dict_obj == obj::OBJ_NULL || !obj::is_obj(dict_obj) {
        return;
    }
    let ptr = obj::to_ptr(dict_obj) as usize;
    if !visited.insert(ptr) {
        return;
    }
    gc::collect_root(&[ptr as *mut u8]);
    if !crate::objdict::is_dict_or_ordereddict(dict_obj) {
        return;
    }
    let map = unsafe { &(*crate::objdict::dict_ptr(dict_obj)).map };
    crate::map::mark_table(map);
    for elem in &map.table {
        if elem.key == obj::OBJ_NULL || elem.key == obj::OBJ_SENTINEL {
            continue;
        }
        let val = elem.value;
        if val == obj::OBJ_NULL {
            continue;
        }
        if obj::is_exact_type(val, crate::objmodule::type_module()) {
            gc::collect_root(&[obj::to_ptr(val) as *mut u8]);
            let globals = unsafe {
                obj::from_ptr(crate::objmodule::module_get_globals(val)
                    as *const crate::objdict::ObjDict
                    as *const ())
            };
            mark_dict_deep(globals, visited);
        } else if crate::objdict::is_dict_or_ordereddict(val) {
            mark_dict_deep(val, visited);
        }
    }
}

/// Access VM state (`MP_STATE_VM(...)`).
pub fn with_vm<R>(f: impl FnOnce(&mut VmState) -> R) -> R {
    with_ctx(|ctx| f(&mut ctx.vm))
}

/// Access memory state (`MP_STATE_MEM(...)`).
pub fn with_mem<R>(f: impl FnOnce(&mut MemState) -> R) -> R {
    with_ctx(|ctx| f(&mut ctx.mem))
}

/// Pointer to the main thread state (`&mp_state_ctx.thread`).
pub fn main_thread_ptr() -> *mut ThreadState {
    with_ctx(|ctx| &mut ctx.thread as *mut ThreadState)
}

/// Access the current thread's MicroPython state (`MP_STATE_THREAD(...)`).
pub fn with_thread<R>(f: impl FnOnce(&mut ThreadState) -> R) -> R {
    if mpconfig::PY_THREAD {
        let ts = crate::mpthread::get_state();
        if !ts.is_null() {
            return f(unsafe { &mut *ts });
        }
    }
    with_ctx(|ctx| f(&mut ctx.thread))
}

/// Main-thread accessor when threading is disabled (`MP_STATE_MAIN_THREAD`).
pub fn with_main_thread<R>(f: impl FnOnce(&mut ThreadState) -> R) -> R {
    with_thread(f)
}

/// Whether the active thread is the main thread (`mp_thread_is_main_thread`).
pub fn is_main_thread() -> bool {
    if mpconfig::PY_THREAD {
        crate::mpthread::get_state() == main_thread_ptr()
    } else {
        true
    }
}

/// Current locals dict (`mp_locals_get`).
pub fn locals_get() -> Obj {
    with_thread(|t| t.dict_locals)
}

/// Set locals dict (`mp_locals_set`).
pub fn locals_set(dict: Obj) {
    with_thread(|t| t.dict_locals = dict);
}

/// Current globals dict (`mp_globals_get`).
pub fn globals_get() -> Obj {
    with_thread(|t| t.dict_globals)
}

/// Set globals dict (`mp_globals_set`).
pub fn globals_set(dict: Obj) {
    with_thread(|t| t.dict_globals = dict);
}

pub fn pending_exception() -> Obj {
    with_thread(|t| t.mp_pending_exception)
}

pub fn set_pending_exception(exc: Obj) {
    with_thread(|t| t.mp_pending_exception = exc);
}

pub fn stop_iteration_arg() -> Obj {
    with_thread(|t| t.stop_iteration_arg)
}

pub fn set_stop_iteration_arg(arg: Obj) {
    with_thread(|t| t.stop_iteration_arg = arg);
}

pub fn gc_lock_depth() -> u16 {
    with_thread(|t| t.gc_lock_depth)
}

pub fn set_gc_lock_depth(depth: u16) {
    with_thread(|t| t.gc_lock_depth = depth);
}

pub fn set_nlr_top(token: Option<usize>) {
    with_thread(|t| t.nlr_top = token);
}

pub fn nlr_top() -> Option<usize> {
    with_thread(|t| t.nlr_top)
}

pub fn snapshot() -> StateCtx {
    with_ctx(|ctx| ctx.clone())
}

pub fn gc_lock() {
    with_thread(|t| t.gc_lock_depth = t.gc_lock_depth.wrapping_add(1));
    gc::lock();
}

pub fn gc_unlock() {
    with_thread(|t| t.gc_lock_depth = t.gc_lock_depth.saturating_sub(1));
    gc::unlock();
}

pub fn thread_init_state(
    locals: Option<Obj>,
    globals: Option<Obj>,
    stack_size: usize,
    stack_top: *mut u8,
) {
    with_thread(|ts| {
        ts.stack_top = stack_top;
        if mpconfig::STACK_CHECK {
            ts.stack_limit =
                stack_size.saturating_sub(mpconfig::STACK_CHECK_MARGIN as usize) as Uint;
        }
        ts.gc_lock_depth = 0;
        ts.nlr_top = None;
        ts.nlr_jump_callback_top = None;
        ts.mp_pending_exception = obj::OBJ_NULL;
        ts.stop_iteration_arg = obj::OBJ_NULL;
        ts.prof_trace_callback = obj::OBJ_NULL;
        ts.prof_callback_is_executing = false;
        ts.current_code_state = None;
    });
    if let Some(loc) = locals {
        locals_set(loc);
    }
    if let Some(glob) = globals {
        globals_set(glob);
    }
}

static COMPILE_ONLY: AtomicBool = AtomicBool::new(false);

/// Set at port startup via `-X compile-only` (`mp_compile_only` in C).
pub fn set_compile_only(enabled: bool) {
    COMPILE_ONLY.store(enabled, Ordering::Relaxed);
}

/// Whether compilation should skip execution (`mp_compile_only`).
pub fn compile_only() -> bool {
    COMPILE_ONLY.load(Ordering::Relaxed)
}

/// Skip running compiled code when the port `-X compile-only` flag or
/// `PYEXEC_COMPILE_ONLY` is active.
pub fn skip_compiled_execution() -> bool {
    compile_only() || mpconfig::PYEXEC_COMPILE_ONLY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_state_roundtrip() {
        init();
        with_thread(|t| {
            t.stack_limit = 42;
            t.mp_pending_exception = obj::new_small_int(7);
        });
        with_thread(|t| {
            assert_eq!(t.stack_limit, 42);
            assert_eq!(obj::small_int_value(t.mp_pending_exception), 7);
        });
    }
}
