//! rewrite of extmod/asyncio/lock.py
// symmetry: done

use std::cell::RefCell;
use std::rc::Rc;

use super::core::{self, CancelledError, TaskQueue};
use super::task::{Task, TaskWaitKind};

/// Lock state: 0 unlocked, 1 locked, or a task scheduled to acquire next.
#[derive(Debug)]
pub enum LockState {
    Unlocked,
    Locked,
    Pending(Rc<RefCell<Task>>),
}

pub struct Lock {
    state: LockState,
    waiting: TaskQueue,
}

impl Lock {
    pub fn new() -> Self {
        Self {
            state: LockState::Unlocked,
            waiting: TaskQueue::new(),
        }
    }

    pub fn locked(&self) -> bool {
        matches!(self.state, LockState::Locked)
    }

    pub fn release(&mut self) {
        if !matches!(self.state, LockState::Locked) {
            panic!("Lock not acquired");
        }
        if let Some(next) = self.waiting.pop() {
            self.state = LockState::Pending(next.clone());
            core::task_queue_push(next, None);
        } else {
            self.state = LockState::Unlocked;
        }
    }

    pub fn acquire(&mut self) -> LockAcquireStep {
        if !matches!(self.state, LockState::Unlocked) {
            let cur = core::cur_task().expect("cur_task");
            self.waiting.push(cur.clone(), None);
            cur.borrow_mut().wait_queue = Some(Rc::new(RefCell::new(TaskQueue::new())));
            cur.borrow_mut().data = Some(TaskWaitKind::None);
            return LockAcquireStep::Yield;
        }
        self.state = LockState::Locked;
        LockAcquireStep::Acquired
    }

    pub fn acquire_handle_cancel(&mut self, cancelled: bool, cur: &Rc<RefCell<Task>>) {
        if cancelled {
            if matches!(self.state, LockState::Pending(_)) {
                if Rc::ptr_eq(
                    &match &self.state {
                        LockState::Pending(t) => t.clone(),
                        _ => unreachable!(),
                    },
                    cur,
                ) {
                    self.state = LockState::Locked;
                    self.release();
                }
            }
        }
    }

    pub fn aenter(&mut self) -> LockAcquireStep {
        self.acquire()
    }

    pub fn aexit(
        &mut self,
        _exc_type: Option<()>,
        exc: Option<CancelledError>,
        _tb: Option<()>,
    ) -> bool {
        if let Some(er) = exc {
            if let Some(cur) = core::cur_task() {
                self.acquire_handle_cancel(true, &cur);
            }
            let _ = er;
        }
        self.release();
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockAcquireStep {
    Acquired,
    Yield,
}
