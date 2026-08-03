//! rewrite of py/scheduler.c
// symmetry: done

use crate::mpconfig;
use crate::mpstate::{self, SchedItem};
use crate::nlr;
use crate::obj::{self, Obj};
use crate::raise;
use crate::runtime::{self, HandlePendingBehaviour};

const IDX_MASK: fn(u8) -> u8 = |i| i & (mpconfig::SCHEDULER_DEPTH - 1);

/// Pending scheduler queue length.
pub fn sched_num_pending() -> u8 {
    mpstate::with_vm(|vm| vm.sched_len)
}

fn sched_empty() -> bool {
    sched_num_pending() == 0
}

fn sched_full() -> bool {
    sched_num_pending() == mpconfig::SCHEDULER_DEPTH
}

/// `mp_sched_exception` — schedule an exception on the main thread.
pub fn sched_exception(exc: Obj) {
    mpstate::set_pending_exception(exc);

    if mpconfig::ENABLE_SCHEDULER && !mpconfig::PY_THREAD {
        mpstate::with_vm(|vm| {
            if vm.sched_state == mpstate::SCHED_IDLE {
                vm.sched_state = mpstate::SCHED_PENDING;
            }
        });
    }
}

/// `mp_sched_keyboard_interrupt`.
pub fn sched_keyboard_interrupt() {
    if !mpconfig::KBD_EXCEPTION {
        return;
    }
    let exc = mpstate::with_vm(|vm| vm.mp_emergency_exception_obj);
    if exc != obj::OBJ_NULL {
        sched_exception(exc);
    }
}

/// `mp_sched_vm_abort`.
pub fn sched_vm_abort() {
    if mpconfig::ENABLE_VM_ABORT {
        mpstate::with_vm(|vm| vm.vm_abort = true);
    }
}

fn sched_run_pending() {
    let item = mpstate::with_vm(|vm| {
        if vm.sched_state != mpstate::SCHED_PENDING {
            return None;
        }
        vm.sched_state = mpstate::SCHED_LOCKED;
        if vm.sched_len == 0 {
            return None;
        }
        let item = vm.sched_queue[vm.sched_idx as usize];
        vm.sched_idx = IDX_MASK(vm.sched_idx.wrapping_add(1));
        vm.sched_len = vm.sched_len.saturating_sub(1);
        Some(item)
    });
    if let Some(item) = item {
        call_function_1_protected(item.func, item.arg);
    }
    sched_unlock();
}

/// `mp_sched_lock`.
pub fn sched_lock() {
    mpstate::with_vm(|vm| {
        if vm.sched_state < 0 {
            vm.sched_state -= 1;
        } else {
            vm.sched_state = mpstate::SCHED_LOCKED;
        }
    });
}

/// `mp_sched_unlock`.
pub fn sched_unlock() {
    let pending_exc = mpstate::pending_exception();
    mpstate::with_vm(|vm| {
        debug_assert!(vm.sched_state < 0);
        vm.sched_state += 1;
        if vm.sched_state == 0 {
            if (!mpconfig::PY_THREAD && pending_exc != obj::OBJ_NULL) || vm.sched_len != 0 {
                vm.sched_state = mpstate::SCHED_PENDING;
            } else {
                vm.sched_state = mpstate::SCHED_IDLE;
            }
        }
    });
}

/// `mp_sched_schedule`.
pub fn sched_schedule(function: Obj, arg: Obj) -> bool {
    if !mpconfig::ENABLE_SCHEDULER {
        return false;
    }
    mpstate::with_vm(|vm| {
        if vm.sched_len == mpconfig::SCHEDULER_DEPTH {
            return false;
        }
        if vm.sched_state == mpstate::SCHED_IDLE {
            vm.sched_state = mpstate::SCHED_PENDING;
        }
        let iput = IDX_MASK(vm.sched_idx.wrapping_add(vm.sched_len));
        vm.sched_len = vm.sched_len.saturating_add(1);
        vm.sched_queue[iput as usize] = SchedItem {
            func: function,
            arg,
        };
        true
    })
}

/// `mp_handle_pending`.
pub fn handle_pending(behavior: HandlePendingBehaviour) {
    let handle_exceptions = behavior != HandlePendingBehaviour::CallbacksOnly;
    let raise_exceptions = behavior == HandlePendingBehaviour::CallbacksAndExceptions;

    if mpconfig::ENABLE_VM_ABORT && handle_exceptions && mpstate::is_main_thread() {
        let abort = mpstate::with_vm(|vm| vm.vm_abort);
        if abort {
            mpstate::with_vm(|vm| vm.vm_abort = false);
            // Host NLR abort path not wired on this port.
        }
    }

    if handle_exceptions {
        let exc = mpstate::pending_exception();
        if exc != obj::OBJ_NULL {
            mpstate::set_pending_exception(obj::OBJ_NULL);
            if raise_exceptions {
                raise::raise_obj(exc);
            }
        }
    }

    if mpconfig::ENABLE_SCHEDULER {
        let mut run = mpstate::with_vm(|vm| vm.sched_state == mpstate::SCHED_PENDING);
        if mpconfig::PY_THREAD && !mpconfig::PY_THREAD_GIL {
            run = run && mpstate::is_main_thread();
        }
        if run {
            sched_run_pending();
        }
    }
}

/// `mp_event_handle_nowait`.
pub fn event_handle_nowait() {
    crate::mphal::internal_event_hook();
    handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
}

/// `mp_event_wait_indefinite`.
pub fn event_wait_indefinite() {
    event_handle_nowait();
    crate::mphal::internal_wfe(usize::MAX);
}

/// `mp_event_wait_ms`.
pub fn event_wait_ms(timeout_ms: usize) {
    event_handle_nowait();
    crate::mphal::internal_wfe(timeout_ms);
}

/// Protected call used when running scheduled callbacks (`mp_call_function_1_protected`).
pub fn call_function_1_protected(fun: Obj, arg: Obj) {
    let mut nlr_buf = nlr::NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::call_function_1(fun, arg)) {
        Ok(_) => {}
        Err(exc) => raise::raise_obj(Obj(exc)),
    }
}
