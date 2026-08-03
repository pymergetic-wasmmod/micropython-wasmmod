//! rewrite of ports/unix/mpthreadport.c + ports/unix/mpthreadport.h
// symmetry: done
//! Linux/BSD path matches C (SIGRTMIN+5 GC signal, sem post/wait, pthread TLS).
//! macOS-only gap: `mp_thread_set_realtime` Mach time-constraint policy (`-X realtime`).

use std::ffi::CStr;
use std::ptr;

use libc::{self, c_int, c_void, pthread_key_t, pthread_t, sem_t, sigaction, siginfo_t};
use py_rs::gc;
use py_rs::mpconfig;
use py_rs::mpstate::{self, ThreadState};
use py_rs::mpthread::{self, ThreadMutex, ThreadPort, ThreadRecursiveMutex};
use py_rs::raise::{self, MpRaise};
use shared_rs::runtime::gchelper;

const THREAD_STACK_OVERFLOW_MARGIN: usize = 8192;

// Match C: prefer SIGRTMIN+5 when available to avoid SIGUSR1 conflicts.
#[cfg(target_os = "linux")]
fn gc_signal() -> c_int {
    unsafe { libc::SIGRTMIN() + 5 }
}
#[cfg(not(target_os = "linux"))]
const fn gc_signal() -> c_int {
    libc::SIGUSR1
}

#[cfg(all(unix, not(target_os = "android")))]
const PTHREAD_CANCEL_ASYNCHRONOUS: c_int = 1;

#[cfg(all(unix, not(target_os = "android")))]
extern "C" {
    fn pthread_setcanceltype(type_: c_int, oldtype: *mut c_int) -> c_int;
}

struct MpThread {
    id: pthread_t,
    ready: bool,
    arg: *mut c_void,
    next: *mut MpThread,
}

static mut TLS_KEY: pthread_key_t = 0;
static mut THREAD_LIST: *mut MpThread = ptr::null_mut();
static mut THREAD_MUTEX: Option<ThreadRecursiveMutex> = None;

#[cfg(target_os = "macos")]
static mut THREAD_SIGNAL_DONE_NAME: [u8; 25] = [0; 25];
#[cfg(target_os = "macos")]
static mut THREAD_SIGNAL_DONE_P: *mut sem_t = ptr::null_mut();
#[cfg(not(target_os = "macos"))]
static mut THREAD_SIGNAL_DONE: sem_t = unsafe { std::mem::zeroed() };

fn mutex_ptr(m: &ThreadMutex) -> *mut libc::pthread_mutex_t {
    m as *const ThreadMutex as *mut libc::pthread_mutex_t
}

fn recursive_mutex_ptr(m: &ThreadRecursiveMutex) -> *mut libc::pthread_mutex_t {
    m as *const ThreadRecursiveMutex as *mut libc::pthread_mutex_t
}

fn signal_done_post() {
    unsafe {
        #[cfg(target_os = "macos")]
        libc::sem_post(THREAD_SIGNAL_DONE_P);
        #[cfg(not(target_os = "macos"))]
        libc::sem_post(&raw mut THREAD_SIGNAL_DONE);
    }
}

fn signal_done_wait() {
    unsafe {
        #[cfg(target_os = "macos")]
        libc::sem_wait(THREAD_SIGNAL_DONE_P);
        #[cfg(not(target_os = "macos"))]
        libc::sem_wait(&raw mut THREAD_SIGNAL_DONE);
    }
}

extern "C" fn thread_gc_handler(signo: c_int, _info: *mut siginfo_t, _ctx: *mut c_void) {
    if signo != gc_signal() {
        return;
    }
    gchelper::collect_regs_and_stack();
    signal_done_post();
}

fn install_gc_signal_handler() {
    unsafe {
        let mut sa: sigaction = std::mem::zeroed();
        sa.sa_flags = libc::SA_SIGINFO;
        sa.sa_sigaction = thread_gc_handler as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(gc_signal(), &sa, ptr::null_mut());
    }
}

#[cfg(target_os = "macos")]
fn init_signal_done_sem(main_id: pthread_t) {
    unsafe {
        let prefix = format!("micropython_sem_{}\0", main_id as u64);
        prefix
            .as_bytes()
            .iter()
            .take(25)
            .enumerate()
            .for_each(|(i, b)| {
                THREAD_SIGNAL_DONE_NAME[i] = *b;
            });
        THREAD_SIGNAL_DONE_P = libc::sem_open(
            THREAD_SIGNAL_DONE_NAME.as_ptr() as *const libc::c_char,
            libc::O_CREAT | libc::O_EXCL,
            0o666,
            0,
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn init_signal_done_sem(_main_id: pthread_t) {
    unsafe {
        libc::sem_init(&raw mut THREAD_SIGNAL_DONE, 0, 0);
    }
}

#[cfg(target_os = "macos")]
fn deinit_signal_done_sem() {
    unsafe {
        if !THREAD_SIGNAL_DONE_P.is_null() {
            libc::sem_close(THREAD_SIGNAL_DONE_P);
            let c_name = CStr::from_bytes_with_nul(&THREAD_SIGNAL_DONE_NAME)
                .unwrap_or(CStr::from_bytes_with_nul(b"\0").unwrap());
            libc::sem_unlink(c_name.as_ptr());
            THREAD_SIGNAL_DONE_P = ptr::null_mut();
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn deinit_signal_done_sem() {}

fn port_get_state() -> *mut ThreadState {
    unsafe {
        let p = libc::pthread_getspecific(TLS_KEY);
        p as *mut ThreadState
    }
}

fn port_set_state(state: *mut ThreadState) {
    unsafe {
        libc::pthread_setspecific(TLS_KEY, state as *mut c_void);
    }
}

fn port_create(
    entry: extern "C" fn(*mut c_void) -> *mut c_void,
    arg: *mut c_void,
    stack_size: &mut usize,
) -> u64 {
    if *stack_size == 0 {
        *stack_size = super::stack_size::DEFAULT_STACK_SIZE;
    }
    if *stack_size < libc::PTHREAD_STACK_MIN as usize {
        *stack_size = libc::PTHREAD_STACK_MIN as usize;
    }
    if *stack_size < 2 * THREAD_STACK_OVERFLOW_MARGIN {
        *stack_size = 2 * THREAD_STACK_OVERFLOW_MARGIN;
    }

    let mut attr: libc::pthread_attr_t = unsafe { std::mem::zeroed() };
    let mut ret = unsafe { libc::pthread_attr_init(&mut attr) };
    if ret != 0 {
        raise::raise(MpRaise::OSError(ret));
    }
    ret = unsafe { libc::pthread_attr_setstacksize(&mut attr, *stack_size) };
    if ret != 0 {
        unsafe {
            libc::pthread_attr_destroy(&mut attr);
        }
        raise::raise(MpRaise::OSError(ret));
    }
    ret = unsafe { libc::pthread_attr_setdetachstate(&mut attr, libc::PTHREAD_CREATE_DETACHED) };
    if ret != 0 {
        unsafe {
            libc::pthread_attr_destroy(&mut attr);
        }
        raise::raise(MpRaise::OSError(ret));
    }

    begin_atomic_section();

    let mut id: pthread_t = 0;
    ret = unsafe { libc::pthread_create(&mut id, &attr, entry, arg) };
    unsafe {
        libc::pthread_attr_destroy(&mut attr);
    }
    if ret != 0 {
        end_atomic_section();
        raise::raise(MpRaise::OSError(ret));
    }

    *stack_size -= THREAD_STACK_OVERFLOW_MARGIN;

    let th = unsafe { libc::malloc(std::mem::size_of::<MpThread>()) as *mut MpThread };
    if th.is_null() {
        end_atomic_section();
        raise::raise(MpRaise::OSError(libc::ENOMEM));
    }
    unsafe {
        (*th).id = id;
        (*th).ready = false;
        (*th).arg = arg;
        (*th).next = THREAD_LIST;
        THREAD_LIST = th;
    }

    end_atomic_section();
    id as u64
}

fn port_get_id() -> u64 {
    unsafe { libc::pthread_self() as u64 }
}

fn port_start() {
    #[cfg(all(unix, not(target_os = "android")))]
    unsafe {
        pthread_setcanceltype(PTHREAD_CANCEL_ASYNCHRONOUS, ptr::null_mut());
    }

    begin_atomic_section();
    unsafe {
        let self_id = libc::pthread_self();
        let mut th = THREAD_LIST;
        while !th.is_null() {
            if (*th).id == self_id {
                (*th).ready = true;
                break;
            }
            th = (*th).next;
        }
    }
    end_atomic_section();
}

fn port_finish() {
    begin_atomic_section();
    unsafe {
        let self_id = libc::pthread_self();
        let mut prev: *mut MpThread = ptr::null_mut();
        let mut th = THREAD_LIST;
        while !th.is_null() {
            if (*th).id == self_id {
                if prev.is_null() {
                    THREAD_LIST = (*th).next;
                } else {
                    (*prev).next = (*th).next;
                }
                libc::free(th as *mut c_void);
                break;
            }
            prev = th;
            th = (*th).next;
        }
    }
    end_atomic_section();
}

fn port_mutex_init(m: *mut ThreadMutex) {
    unsafe {
        libc::pthread_mutex_init(mutex_ptr(&mut *m), ptr::null());
    }
}

fn port_mutex_lock(m: *const ThreadMutex, wait: bool) -> i32 {
    let ret = if wait {
        unsafe { libc::pthread_mutex_lock(mutex_ptr(&*m)) }
    } else {
        unsafe { libc::pthread_mutex_trylock(mutex_ptr(&*m)) }
    };
    if ret == 0 {
        1
    } else if ret == libc::EBUSY {
        0
    } else {
        -ret
    }
}

fn port_mutex_unlock(m: *const ThreadMutex) {
    unsafe {
        libc::pthread_mutex_unlock(mutex_ptr(&*m));
    }
}

fn port_recursive_mutex_init(m: *mut ThreadRecursiveMutex) {
    unsafe {
        let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
        libc::pthread_mutexattr_init(&mut attr);
        libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
        libc::pthread_mutex_init(recursive_mutex_ptr(&mut *m), &attr);
        libc::pthread_mutexattr_destroy(&mut attr);
    }
}

fn port_recursive_mutex_lock(m: *const ThreadRecursiveMutex, wait: bool) -> i32 {
    port_mutex_lock(m as *const ThreadMutex, wait)
}

fn port_recursive_mutex_unlock(m: *const ThreadRecursiveMutex) {
    port_mutex_unlock(m as *const ThreadMutex)
}

/// `mp_thread_unix_begin_atomic_section`.
pub fn begin_atomic_section() {
    if !mpconfig::PY_THREAD {
        return;
    }
    unsafe {
        if let Some(m) = THREAD_MUTEX.as_ref() {
            let _ = port_recursive_mutex_lock(m, true);
        }
    }
}

/// `mp_thread_unix_end_atomic_section`.
pub fn end_atomic_section() {
    if !mpconfig::PY_THREAD {
        return;
    }
    unsafe {
        if let Some(m) = THREAD_MUTEX.as_ref() {
            port_recursive_mutex_unlock(m);
        }
    }
}

/// `mp_thread_init`.
pub fn init() {
    if !mpconfig::PY_THREAD {
        return;
    }

    mpthread::register_port(ThreadPort {
        get_state: port_get_state,
        set_state: port_set_state,
        create: port_create,
        get_id: port_get_id,
        start: port_start,
        finish: port_finish,
        mutex_init: port_mutex_init,
        mutex_lock: port_mutex_lock,
        mutex_unlock: port_mutex_unlock,
        recursive_mutex_init: port_recursive_mutex_init,
        recursive_mutex_lock: port_recursive_mutex_lock,
        recursive_mutex_unlock: port_recursive_mutex_unlock,
    });

    mpstate::init();

    unsafe {
        libc::pthread_key_create(&mut TLS_KEY, None);
        libc::pthread_setspecific(TLS_KEY, mpstate::main_thread_ptr() as *mut c_void);

        let mut m: ThreadRecursiveMutex = std::mem::zeroed();
        port_recursive_mutex_init(&mut m as *mut _);
        THREAD_MUTEX = Some(m);

        let main_id = libc::pthread_self();
        init_signal_done_sem(main_id);
        install_gc_signal_handler();

        let th = libc::malloc(std::mem::size_of::<MpThread>()) as *mut MpThread;
        if !th.is_null() {
            (*th).id = main_id;
            (*th).ready = true;
            (*th).arg = ptr::null_mut();
            (*th).next = ptr::null_mut();
            THREAD_LIST = th;
        }
    }
}

/// `mp_thread_deinit`.
pub fn deinit() {
    if !mpconfig::PY_THREAD {
        return;
    }

    begin_atomic_section();
    unsafe {
        while !THREAD_LIST.is_null() && !(*THREAD_LIST).next.is_null() {
            let th = THREAD_LIST;
            THREAD_LIST = (*th).next;
            libc::pthread_cancel((*th).id);
            libc::free(th as *mut c_void);
        }
        if !THREAD_LIST.is_null() {
            debug_assert!((*THREAD_LIST).id == libc::pthread_self());
            libc::free(THREAD_LIST as *mut c_void);
            THREAD_LIST = ptr::null_mut();
        }
    }
    end_atomic_section();
    deinit_signal_done_sem();
}

/// `mp_thread_gc_others` — signal other threads to scan registers/stack.
pub fn gc_others() {
    if !mpconfig::PY_THREAD {
        return;
    }

    begin_atomic_section();
    unsafe {
        let self_id = libc::pthread_self();
        let mut th = THREAD_LIST;
        while !th.is_null() {
            gc::collect_root_words(&(*th).arg as *const _ as *const u8, 1);
            if (*th).id != self_id && (*th).ready {
                libc::pthread_kill((*th).id, gc_signal());
                signal_done_wait();
            }
            th = (*th).next;
        }
    }
    end_atomic_section();
}

pub fn mutex_init(m: &mut ThreadMutex) {
    port_mutex_init(m as *mut ThreadMutex);
}

pub fn mutex_lock(m: &ThreadMutex, wait: bool) -> i32 {
    port_mutex_lock(m, wait)
}

pub fn mutex_unlock(m: &ThreadMutex) {
    port_mutex_unlock(m);
}

pub fn recursive_mutex_init(m: &mut ThreadRecursiveMutex) {
    port_recursive_mutex_init(m as *mut ThreadRecursiveMutex);
}

pub fn recursive_mutex_lock(m: &ThreadRecursiveMutex, wait: bool) -> i32 {
    port_recursive_mutex_lock(m, wait)
}

pub fn recursive_mutex_unlock(m: &ThreadRecursiveMutex) {
    port_recursive_mutex_unlock(m);
}

#[cfg(target_os = "macos")]
pub static REALTIME_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub fn set_realtime() {
    // Mach thread time-constraint policy (see C mp_thread_set_realtime).
}
