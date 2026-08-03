//! rewrite of extmod/asyncio/task.py
// symmetry: done
//! Pairing-heap `TaskQueue` and `Task` (Python fallback when C `_asyncio` unavailable).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::core::{self, CancelledError, StopIteration};

/// Coroutine yield / completion (mirrors generator `send` / `throw`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroStep {
    Yield,
    Return,
    Raise,
}

pub trait Coroutine: std::fmt::Debug {
    fn send(&mut self, val: Option<()>) -> CoroStep;
    fn throw(&mut self, exc: CancelledError) -> CoroStep;
}

pub type CoroHandle = Rc<RefCell<dyn Coroutine>>;

/// Task completion / wait state (`True`, `False`, `None`, callback, or waiter queue).
#[derive(Debug)]
pub enum TaskState {
    Running,
    DoneAwaited,
    DoneDetached,
    Waiters(Rc<RefCell<TaskQueue>>),
}

impl TaskState {
    pub fn is_done(&self) -> bool {
        !matches!(self, TaskState::Running)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitKind {
    None,
    Cancelled,
    IoQueue,
}

#[derive(Debug)]
pub struct Task {
    pub coro: CoroHandle,
    pub data: Option<TaskWaitKind>,
    pub result: Option<StopIteration>,
    pub state: TaskState,
    pub ph_key: u64,
    pub ph_child: Option<Rc<RefCell<Task>>>,
    pub ph_child_last: Option<Rc<RefCell<Task>>>,
    pub ph_next: Option<Rc<RefCell<Task>>>,
    pub ph_rightmost_parent: Option<Weak<RefCell<Task>>>,
    pub wait_queue: Option<Rc<RefCell<TaskQueue>>>,
    pub parent: Option<Rc<RefCell<Task>>>,
}

#[derive(Debug)]
pub struct TaskQueue {
    heap: Option<Rc<RefCell<Task>>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self { heap: None }
    }

    pub fn peek(&self) -> Option<Rc<RefCell<Task>>> {
        self.heap.clone()
    }

    pub fn push(&mut self, v: Rc<RefCell<Task>>, key: Option<u64>) {
        {
            let mut t = v.borrow_mut();
            assert!(t.ph_child.is_none());
            assert!(t.ph_next.is_none());
            t.data = Some(TaskWaitKind::None);
            t.ph_key = key.unwrap_or_else(core::ticks);
        }
        if let Some(h) = self.heap.take() {
            self.heap = Some(ph_meld(v, h));
        } else {
            self.heap = Some(v);
        }
    }

    pub fn pop(&mut self) -> Option<Rc<RefCell<Task>>> {
        let v = self.heap.take()?;
        assert!(v.borrow().ph_next.is_none());
        let child = v.borrow_mut().ph_child.take();
        self.heap = ph_pairing(child);
        v.borrow_mut().ph_child = None;
        Some(v)
    }

    pub fn remove(&mut self, v: &Rc<RefCell<Task>>) {
        if let Some(heap) = self.heap.take() {
            self.heap = Some(ph_delete(heap, v));
        }
    }
}

impl Task {
    pub fn new(coro: CoroHandle) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            coro,
            data: None,
            result: None,
            state: TaskState::Running,
            ph_key: 0,
            ph_child: None,
            ph_child_last: None,
            ph_next: None,
            ph_rightmost_parent: None,
            wait_queue: None,
            parent: None,
        }))
    }

    pub fn done(this: &Rc<RefCell<Self>>) -> bool {
        this.borrow().state.is_done()
    }

    pub fn cancel(this: &Rc<RefCell<Task>>) -> bool {
        if Task::done(this) {
            return false;
        }
        if Rc::ptr_eq(this, &core::cur_task().expect("cur_task")) {
            panic!("can't cancel self");
        }
        let mut target = this.clone();
        loop {
            let parent = target.borrow().parent.clone();
            if let Some(p) = parent {
                target = p;
            } else {
                break;
            }
        }
        if let Some(q) = target.borrow().wait_queue.clone() {
            q.borrow_mut().remove(&target);
            core::task_queue_push(target.clone(), None);
        } else if core::ticks_diff(target.borrow().ph_key, core::ticks()) > 0 {
            core::task_queue_remove(&target);
            core::task_queue_push(target.clone(), None);
        }
        target.borrow_mut().data = Some(TaskWaitKind::Cancelled);
        true
    }

    pub fn iter_start(this: &Rc<RefCell<Self>>) -> Result<(), &'static str> {
        match &this.borrow().state {
            TaskState::DoneAwaited | TaskState::DoneDetached => {
                this.borrow_mut().state = TaskState::DoneAwaited;
            }
            TaskState::Running => {
                this.borrow_mut().state =
                    TaskState::Waiters(Rc::new(RefCell::new(TaskQueue::new())));
            }
            TaskState::Waiters(_) => return Err("can't wait"),
        }
        Ok(())
    }

    pub fn iter_next(this: &Rc<RefCell<Self>>) -> Result<(), StopIteration> {
        if this.borrow().state.is_done() {
            Err(this.borrow().result.clone().unwrap_or(StopIteration(None)))
        } else if let TaskState::Waiters(q) = &this.borrow().state {
            let cur = core::cur_task().unwrap();
            q.borrow_mut().push(cur.clone(), None);
            cur.borrow_mut().parent = Some(this.clone());
            Ok(())
        } else {
            Ok(())
        }
    }
}

fn ph_meld(h1: Rc<RefCell<Task>>, h2: Rc<RefCell<Task>>) -> Rc<RefCell<Task>> {
    let lt = core::ticks_diff(h1.borrow().ph_key, h2.borrow().ph_key) < 0;
    if lt {
        let mut h1b = h1.borrow_mut();
        if h1b.ph_child.is_none() {
            h1b.ph_child = Some(h2.clone());
        } else {
            let last = h1b.ph_child_last.clone().unwrap();
            last.borrow_mut().ph_next = Some(h2.clone());
        }
        h1b.ph_child_last = Some(h2.clone());
        drop(h1b);
        h2.borrow_mut().ph_next = None;
        h2.borrow_mut().ph_rightmost_parent = Some(Rc::downgrade(&h1));
        h1
    } else {
        let child = h2.borrow_mut().ph_child.take();
        h1.borrow_mut().ph_next = child;
        h2.borrow_mut().ph_child = Some(h1.clone());
        if h1.borrow().ph_next.is_none() {
            h2.borrow_mut().ph_child_last = Some(h1.clone());
            h1.borrow_mut().ph_rightmost_parent = Some(Rc::downgrade(&h2));
        }
        h2
    }
}

fn ph_meld_opt(
    h1: Option<Rc<RefCell<Task>>>,
    h2: Option<Rc<RefCell<Task>>>,
) -> Option<Rc<RefCell<Task>>> {
    match (h1, h2) {
        (None, h) | (h, None) => h,
        (Some(a), Some(b)) => Some(ph_meld(a, b)),
    }
}

fn ph_pairing(mut child: Option<Rc<RefCell<Task>>>) -> Option<Rc<RefCell<Task>>> {
    let mut heap = None;
    while child.is_some() {
        let n1 = child.take().unwrap();
        child = n1.borrow_mut().ph_next.take();
        n1.borrow_mut().ph_next = None;
        let n1 = if child.is_some() {
            let n2 = child.take().unwrap();
            child = n2.borrow_mut().ph_next.take();
            n2.borrow_mut().ph_next = None;
            ph_meld(n1, n2)
        } else {
            n1
        };
        heap = ph_meld_opt(heap, Some(n1));
    }
    heap
}

fn ph_delete(heap: Rc<RefCell<Task>>, node: &Rc<RefCell<Task>>) -> Rc<RefCell<Task>> {
    if Rc::ptr_eq(&heap, node) {
        let child = heap.borrow_mut().ph_child.take();
        node.borrow_mut().ph_child = None;
        return ph_pairing(child).unwrap_or(heap);
    }
    let mut walk = node.clone();
    while walk.borrow().ph_next.is_some() {
        let next = walk.borrow().ph_next.clone().unwrap();
        walk = next;
    }
    let parent = walk
        .borrow()
        .ph_rightmost_parent
        .as_ref()
        .and_then(|w| w.upgrade())
        .expect("ph parent");

    let node_is_first = parent
        .borrow()
        .ph_child
        .as_ref()
        .map(|c| Rc::ptr_eq(c, node))
        .unwrap_or(false);
    let node_has_child = node.borrow().ph_child.is_some();

    if node_is_first && !node_has_child {
        parent.borrow_mut().ph_child = node.borrow_mut().ph_next.take();
        node.borrow_mut().ph_next = None;
        return heap;
    }

    let (child, next) = {
        let mut nb = node.borrow_mut();
        (nb.ph_child.take(), nb.ph_next.take())
    };

    let merged = ph_pairing(child);
    if node_is_first {
        parent.borrow_mut().ph_child = merged.clone();
    } else {
        let mut n = parent.borrow().ph_child.clone().unwrap();
        while !Rc::ptr_eq(n.borrow().ph_next.as_ref().unwrap(), node) {
            let next = n.borrow().ph_next.clone().unwrap();
            n = next;
        }
        n.borrow_mut().ph_next = merged.clone();
    }

    let mut out = merged.unwrap_or_else(|| node.clone());
    out.borrow_mut().ph_next = next;
    if out.borrow().ph_next.is_none() {
        out.borrow_mut().ph_rightmost_parent = Some(Rc::downgrade(&parent));
        parent.borrow_mut().ph_child_last = Some(out.clone());
    }
    heap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct NullCoro;
    impl Coroutine for NullCoro {
        fn send(&mut self, _: Option<()>) -> CoroStep {
            CoroStep::Return
        }
        fn throw(&mut self, _: CancelledError) -> CoroStep {
            CoroStep::Raise
        }
    }

    fn mk(key: u64) -> Rc<RefCell<Task>> {
        let t = Task::new(Rc::new(RefCell::new(NullCoro)));
        t.borrow_mut().ph_key = key;
        t
    }

    #[test]
    fn task_queue_orders_by_key() {
        let mut q = TaskQueue::new();
        q.push(mk(300), Some(300));
        q.push(mk(100), Some(100));
        q.push(mk(200), Some(200));
        assert_eq!(q.pop().unwrap().borrow().ph_key, 100);
        assert_eq!(q.pop().unwrap().borrow().ph_key, 200);
        assert_eq!(q.pop().unwrap().borrow().ph_key, 300);
    }
}
