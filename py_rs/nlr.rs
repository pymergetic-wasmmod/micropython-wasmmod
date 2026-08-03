//! Host implementation of MicroPython's non-local-return (`nlr`) interface.
//!
//! Rust cannot soundly expose C `setjmp`/`longjmp`: long-jumping across Rust
//! frames skips destructors.  The equivalent host mechanism is a tagged panic
//! caught by `nlr_protect`, while `NlrBuf` keeps the same push/pop chain and
//! return-value semantics as `nlr_buf_t`.
// symmetry: done

use std::any::Any;
use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

#[derive(Debug, Default)]
pub struct NlrBuf {
    prev: Option<usize>,
    ret_val: Option<usize>,
    token: usize,
}

#[derive(Debug)]
struct NlrJump {
    token: usize,
    value: usize,
}

type Callback = Box<dyn FnOnce()>;

#[derive(Default)]
struct NlrState {
    top: Option<usize>,
    /// Previous `top` values, mirroring `nlr_buf_t.prev` for `pop_top`.
    top_stack: Vec<Option<usize>>,
    next_token: usize,
    callbacks: Vec<(usize, Callback)>,
}

thread_local! {
    static STATE: RefCell<NlrState> = RefCell::new(NlrState::default());
}

/// Push a buffer onto the current thread's NLR chain (`nlr_push_tail`).
/// Returns zero, matching the ordinary return from `setjmp`.
pub fn push_tail(buf: &mut NlrBuf) -> u32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.next_token = state.next_token.wrapping_add(1).max(1);
        buf.token = state.next_token;
        buf.prev = state.top;
        buf.ret_val = None;
        let prev_top = state.top;
        state.top_stack.push(prev_top);
        state.top = Some(buf.token);
    });
    0
}

pub fn push(buf: &mut NlrBuf) -> u32 {
    push_tail(buf)
}

/// Pop the current NLR handler (`nlr_pop`).
pub fn pop(buf: &mut NlrBuf) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert_eq!(state.top, Some(buf.token), "nlr_pop on non-top buffer");
        state.top = buf.prev;
        state.top_stack.pop();
        state.callbacks.retain(|(token, _)| *token != buf.token);
    });
}

/// Pop the top NLR handler without a buffer pointer (`nlr_pop` in C).
pub fn pop_top() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.top = state.top_stack.pop().flatten();
    });
}

/// Run `f` with `buf` as the active exception handler.  `Ok` is normal
/// completion; `Err(value)` is the value supplied to `nlr_jump`.
pub fn protect<R>(buf: &mut NlrBuf, f: impl FnOnce() -> R) -> Result<R, usize> {
    push_tail(buf);
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(value) => {
            pop(buf);
            Ok(value)
        }
        Err(payload) => match payload.downcast::<NlrJump>() {
            Ok(jump) if jump.token == buf.token => {
                buf.ret_val = Some(jump.value);
                run_callbacks(buf.token);
                STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.top = buf.prev;
                    state.top_stack.pop();
                });
                Err(jump.value)
            }
            Err(payload) => panic::resume_unwind(payload),
            Ok(jump) => panic::resume_unwind(jump),
        },
    }
}

/// Raise to the most recently pushed buffer (`nlr_jump`).  This never returns.
pub fn jump(value: usize) -> ! {
    let token = STATE.with(|state| state.borrow().top);
    match token {
        Some(token) => panic::panic_any(NlrJump { token, value }),
        None => jump_fail(value),
    }
}

pub fn raise(value: usize) -> ! {
    jump(value)
}

pub fn ret_val(buf: &NlrBuf) -> Option<usize> {
    buf.ret_val
}

pub fn jump_fail(value: usize) -> ! {
    panic!("uncaught MetalPython NLR value {value:#x}")
}

/// Register cleanup to run when the active NLR target catches a jump.
pub fn push_jump_callback(callback: impl FnOnce() + 'static) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let token = state.top.expect("NLR callback without active buffer");
        state.callbacks.push((token, Box::new(callback)));
    });
}

/// Pop the latest callback, optionally executing it (`nlr_pop_jump_callback`).
pub fn pop_jump_callback(run_callback: bool) {
    let callback = STATE.with(|state| state.borrow_mut().callbacks.pop());
    if run_callback {
        if let Some((_, callback)) = callback { callback(); }
    }
}

fn run_callbacks(token: usize) {
    let callbacks = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let mut selected = Vec::new();
        let mut retained = Vec::new();
        for (owner, callback) in state.callbacks.drain(..) {
            if owner == token { selected.push(callback); } else { retained.push((owner, callback)); }
        }
        state.callbacks = retained;
        selected
    });
    for callback in callbacks.into_iter().rev() {
        callback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn protected_jump_returns_payload_and_unwinds_callbacks() {
        let called = Arc::new(Mutex::new(false));
        let callback_called = Arc::clone(&called);
        let mut buf = NlrBuf::default();
        let result = protect(&mut buf, || {
            push_jump_callback(move || *callback_called.lock().unwrap() = true);
            jump(0xfeed);
        });
        assert_eq!(result, Err(0xfeed));
        assert_eq!(ret_val(&buf), Some(0xfeed));
        assert!(*called.lock().unwrap());
    }
}
