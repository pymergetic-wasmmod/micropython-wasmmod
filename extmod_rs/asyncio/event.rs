//! rewrite of extmod/asyncio/event.py
// symmetry: done

use std::cell::RefCell;
use std::rc::Rc;

use super::core::{self, TaskQueue};
use super::task::TaskWaitKind;

pub struct Event {
    state: bool,
    waiting: TaskQueue,
}

impl Event {
    pub fn new() -> Self {
        Self {
            state: false,
            waiting: TaskQueue::new(),
        }
    }

    pub fn is_set(&self) -> bool {
        self.state
    }

    pub fn set(&mut self) {
        while self.waiting.peek().is_some() {
            if let Some(t) = self.waiting.pop() {
                core::task_queue_push(t, None);
            }
        }
        self.state = true;
    }

    pub fn clear(&mut self) {
        self.state = false;
    }

    /// Async `wait`: returns `true` immediately if set, otherwise schedules caller.
    pub fn wait(&mut self) -> EventWaitStep {
        if self.state {
            EventWaitStep::Ready
        } else {
            let cur = core::cur_task().expect("cur_task");
            self.waiting.push(cur.clone(), None);
            cur.borrow_mut().wait_queue = Some(Rc::new(RefCell::new(TaskQueue::new())));
            cur.borrow_mut().data = Some(TaskWaitKind::None);
            EventWaitStep::Yield
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventWaitStep {
    Ready,
    Yield,
}

pub struct ThreadSafeFlag {
    state: u8,
}

impl ThreadSafeFlag {
    pub fn new() -> Self {
        Self { state: 0 }
    }

    pub fn ioctl(&self, req: i32, flags: i32) -> i32 {
        if req == 3 {
            return (self.state as i32) * flags;
        }
        -1
    }

    pub fn set(&mut self) {
        self.state = 1;
    }

    pub fn clear(&mut self) {
        self.state = 0;
    }

    pub fn wait(&mut self, stream_id: usize) -> ThreadSafeWaitStep {
        if self.state == 0 {
            core::with_io_queue(|io| io.queue_read(stream_id));
            ThreadSafeWaitStep::Yield
        } else {
            self.state = 0;
            ThreadSafeWaitStep::Ready
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSafeWaitStep {
    Ready,
    Yield,
}
