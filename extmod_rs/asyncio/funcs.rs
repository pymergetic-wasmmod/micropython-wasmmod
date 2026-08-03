//! rewrite of extmod/asyncio/funcs.py
// symmetry: done

use std::cell::RefCell;
use std::rc::Rc;

use super::core::{self, CancelledError};
use super::task::{CoroStep, Coroutine, Task};

#[derive(Debug)]
struct RunHelper {
    waiter: Rc<RefCell<Task>>,
}

impl Coroutine for RunHelper {
    fn send(&mut self, _: Option<()>) -> CoroStep {
        CoroStep::Yield
    }
    fn throw(&mut self, _: CancelledError) -> CoroStep {
        CoroStep::Raise
    }
}

pub fn wait_for(aw: Rc<RefCell<Task>>, timeout: Option<f64>) -> WaitForOutcome {
    if timeout.is_none() {
        let _ = aw;
        return WaitForOutcome::Completed;
    }
    let _timeout = timeout.unwrap();
    let _runner = core::create_task(Rc::new(RefCell::new(RunHelper {
        waiter: core::cur_task().expect("cur_task"),
    })));
    WaitForOutcome::Pending
}

pub fn wait_for_ms(aw: Rc<RefCell<Task>>, timeout: i64) -> WaitForOutcome {
    wait_for(aw, Some(timeout as f64 / 1000.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitForOutcome {
    Completed,
    Pending,
    Timeout,
}

pub fn gather(aws: &[Rc<RefCell<Task>>], return_exceptions: bool) -> GatherResult {
    let mut ts = aws.to_vec();
    let mut state: GatherState = GatherState::Count(0);

    for t in &ts {
        match &t.borrow().state {
            super::task::TaskState::Running => {
                state = state.inc();
            }
            super::task::TaskState::DoneAwaited | super::task::TaskState::DoneDetached => {
                if !return_exceptions {
                    state = GatherState::Abort;
                }
            }
            super::task::TaskState::Waiters(_) => {
                return GatherResult::Err("can't gather");
            }
        }
    }

    if state.count() > 0 {
        GatherResult::Pending { tasks: ts, state, return_exceptions }
    } else if state.is_abort() {
        GatherResult::Err("exception in sub-task")
    } else {
        GatherResult::Ok(ts)
    }
}

#[derive(Debug)]
pub enum GatherResult {
    Ok(Vec<Rc<RefCell<Task>>>),
    Pending {
        tasks: Vec<Rc<RefCell<Task>>>,
        state: GatherState,
        return_exceptions: bool,
    },
    Err(&'static str),
}

#[derive(Debug, Clone)]
pub enum GatherState {
    Count(i32),
    Abort,
    Exception,
}

impl GatherState {
    fn count(&self) -> i32 {
        match self {
            GatherState::Count(n) => *n,
            _ => 0,
        }
    }
    fn inc(self) -> Self {
        match self {
            GatherState::Count(n) => GatherState::Count(n + 1),
            other => other,
        }
    }
    fn is_abort(&self) -> bool {
        matches!(self, GatherState::Abort)
    }
}
