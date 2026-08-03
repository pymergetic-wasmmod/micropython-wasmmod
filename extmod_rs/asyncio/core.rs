//! rewrite of extmod/asyncio/core.py
// symmetry: done
//! Asyncio event loop core (`core.py` rewrite).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub use super::task::TaskQueue;
use super::task::{CoroHandle, CoroStep, Task, TaskState, TaskWaitKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelledError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopIteration(pub Option<i64>);

pub struct SingletonGenerator {
    state: Option<u64>,
}

impl SingletonGenerator {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn advance(&mut self, task: Rc<RefCell<Task>>) {
        if let Some(key) = self.state.take() {
            task_queue_push(task, Some(key));
        }
    }
}

pub fn sleep_ms(t: i64) -> SingletonGenerator {
    let mut sgen = SingletonGenerator::new();
    assert!(sgen.state.is_none());
    sgen.state = Some(ticks_add(ticks(), t.max(0) as u64));
    sgen
}

pub fn sleep(t: f64) -> SingletonGenerator {
    sleep_ms((t * 1000.0) as i64)
}

pub struct IoQueue {
    map: HashMap<usize, IoEntry>,
}

#[derive(Clone)]
struct IoEntry {
    read: Option<Rc<RefCell<Task>>>,
    write: Option<Rc<RefCell<Task>>>,
    stream_id: usize,
}

impl IoQueue {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn enqueue(&mut self, stream_id: usize, idx: usize, task: Rc<RefCell<Task>>) {
        use std::collections::hash_map::Entry;
        match self.map.entry(stream_id) {
            Entry::Vacant(e) => {
                let mut entry = IoEntry {
                    read: None,
                    write: None,
                    stream_id,
                };
                if idx == 0 {
                    entry.read = Some(task.clone());
                } else {
                    entry.write = Some(task.clone());
                }
                e.insert(entry);
            }
            Entry::Occupied(mut e) => {
                let sm = e.get_mut();
                assert!(if idx == 0 {
                    sm.read.is_none()
                } else {
                    sm.write.is_none()
                });
                assert!(if idx == 0 {
                    sm.write.is_some()
                } else {
                    sm.read.is_some()
                });
                if idx == 0 {
                    sm.read = Some(task.clone());
                } else {
                    sm.write = Some(task.clone());
                }
            }
        }
        task.borrow_mut().data = Some(TaskWaitKind::IoQueue);
    }

    fn dequeue(&mut self, stream_id: usize) {
        self.map.remove(&stream_id);
    }

    pub fn queue_read(&mut self, stream_id: usize) {
        self.enqueue(stream_id, 0, cur_task().expect("cur_task"));
    }

    pub fn queue_write(&mut self, stream_id: usize) {
        self.enqueue(stream_id, 1, cur_task().expect("cur_task"));
    }

    pub fn remove(&mut self, task: &Rc<RefCell<Task>>) {
        loop {
            let mut del_s = None;
            for (_, sm) in &self.map {
                if sm.read.as_ref().is_some_and(|t| Rc::ptr_eq(t, task))
                    || sm.write.as_ref().is_some_and(|t| Rc::ptr_eq(t, task))
                {
                    del_s = Some(sm.stream_id);
                    break;
                }
            }
            if let Some(s) = del_s {
                self.dequeue(s);
            } else {
                break;
            }
        }
    }

    pub fn wait_io_event(&mut self, _dt: i64) {
        let keys: Vec<usize> = self.map.keys().copied().collect();
        for s in keys {
            let Some(entry) = self.map.get_mut(&s) else {
                continue;
            };
            let ev = 0x001 | 0x004;
            if ev & !0x004 != 0 {
                if let Some(t) = entry.read.take() {
                    task_queue_push(t, None);
                }
            }
            if ev & !0x001 != 0 {
                if let Some(t) = entry.write.take() {
                    task_queue_push(t, None);
                }
            }
            if entry.read.is_none() && entry.write.is_none() {
                self.dequeue(s);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

thread_local! {
    static CUR: RefCell<Option<Rc<RefCell<Task>>>> = RefCell::new(None);
    static TASK_Q: RefCell<TaskQueue> = RefCell::new(TaskQueue::new());
    static IO_Q: RefCell<IoQueue> = RefCell::new(IoQueue::new());
    static STOP_TASK: RefCell<Option<Rc<RefCell<Task>>>> = RefCell::new(None);
    static LOOP_INIT: RefCell<bool> = RefCell::new(false);
}

static mut EXC_HANDLER: Option<fn(&Loop, &ExcContext)> = None;

#[derive(Debug)]
pub struct ExcContext {
    pub message: &'static str,
    pub exception: Option<CancelledError>,
    pub future: Option<Rc<RefCell<Task>>>,
}

pub fn ticks() -> u64 {
    py_rs::mphal::ticks_ms() as u64 & (py_rs::mpconfig::PY_TIME_TICKS_PERIOD - 1)
}

pub fn ticks_diff(t1: u64, t0: u64) -> i64 {
    let period = py_rs::mpconfig::PY_TIME_TICKS_PERIOD;
    let half = (period / 2) as i64;
    // wrapping_add: period is 2^62, so `delta + half` can exceed u64 mid-range in debug.
    (((t1.wrapping_sub(t0).wrapping_add(half as u64)) & (period - 1)) as i64) - half
}

pub fn ticks_add(t0: u64, t: u64) -> u64 {
    t0.wrapping_add(t) & (py_rs::mpconfig::PY_TIME_TICKS_PERIOD - 1)
}

fn ensure_loop() {
    LOOP_INIT.with(|init| {
        if !*init.borrow() {
            new_event_loop();
            *init.borrow_mut() = true;
        }
    });
}

pub fn cur_task() -> Option<Rc<RefCell<Task>>> {
    CUR.with(|c| c.borrow().clone())
}

pub fn set_cur_task(t: Option<Rc<RefCell<Task>>>) {
    CUR.with(|c| *c.borrow_mut() = t);
}

pub fn task_queue_push(t: Rc<RefCell<Task>>, key: Option<u64>) {
    TASK_Q.with(|q| q.borrow_mut().push(t, key));
}

pub fn task_queue_peek() -> Option<Rc<RefCell<Task>>> {
    TASK_Q.with(|q| q.borrow().peek())
}

pub fn task_queue_pop() -> Option<Rc<RefCell<Task>>> {
    TASK_Q.with(|q| q.borrow_mut().pop())
}

pub fn task_queue_remove(t: &Rc<RefCell<Task>>) {
    TASK_Q.with(|q| q.borrow_mut().remove(t));
}

pub fn with_io_queue<R>(f: impl FnOnce(&mut IoQueue) -> R) -> R {
    IO_Q.with(|q| f(&mut *q.borrow_mut()))
}

pub fn promote_to_task(aw: Rc<RefCell<Task>>) -> Rc<RefCell<Task>> {
    aw
}

pub fn create_task(coro: CoroHandle) -> Rc<RefCell<Task>> {
    ensure_loop();
    let t = Task::new(coro);
    task_queue_push(t.clone(), None);
    t
}

pub fn run_until_complete(main_task: Option<Rc<RefCell<Task>>>) -> Option<StopIteration> {
    ensure_loop();
    loop {
        let mut dt = 1i64;
        while dt > 0 {
            dt = -1;
            if let Some(t) = task_queue_peek() {
                dt = ticks_diff(t.borrow().ph_key, ticks()).max(0);
            } else if with_io_queue(|io| io.is_empty()) {
                set_cur_task(None);
                if main_task.as_ref().is_none_or(Task::done) {
                    return None;
                }
                dt = 3;
            }
            with_io_queue(|io| io.wait_io_event(dt));
        }

        let Some(t) = task_queue_pop() else { continue };
        set_cur_task(Some(t.clone()));

        let pending = t.borrow().data;
        let step = if pending != Some(TaskWaitKind::Cancelled) {
            t.borrow_mut().coro.borrow_mut().send(None)
        } else {
            t.borrow_mut().data = None;
            t.borrow_mut().coro.borrow_mut().throw(CancelledError)
        };

        match step {
            CoroStep::Yield => continue,
            CoroStep::Return => {
                finish_task(&t, main_task.as_ref(), StopIteration(None));
                if main_task.as_ref().is_some_and(|m| Rc::ptr_eq(m, &t)) {
                    return t.borrow().result.clone();
                }
            }
            CoroStep::Raise => finish_task(&t, main_task.as_ref(), StopIteration(None)),
        }
    }
}

fn finish_task(t: &Rc<RefCell<Task>>, main_task: Option<&Rc<RefCell<Task>>>, er: StopIteration) {
    assert!(t.borrow().data.is_none());
    let awaited = main_task.is_some_and(|m| Rc::ptr_eq(m, t));
    if awaited {
        set_cur_task(None);
    }
    let mut tb = t.borrow_mut();
    if matches!(tb.state, TaskState::Running) {
        tb.state = if awaited {
            TaskState::DoneAwaited
        } else {
            TaskState::DoneDetached
        };
        tb.result = Some(er);
    }
}

pub fn run(coro: CoroHandle) -> Option<StopIteration> {
    run_until_complete(Some(create_task(coro)))
}

pub struct Loop;

impl Loop {
    pub fn create_task(coro: CoroHandle) -> Rc<RefCell<Task>> {
        create_task(coro)
    }

    pub fn run_forever() {
        let stop = create_task(Rc::new(RefCell::new(StopperCoro)));
        STOP_TASK.with(|s| *s.borrow_mut() = Some(stop.clone()));
        run_until_complete(Some(stop));
    }

    pub fn run_until_complete(aw: Rc<RefCell<Task>>) -> Option<StopIteration> {
        run_until_complete(Some(promote_to_task(aw)))
    }

    pub fn stop() {
        STOP_TASK.with(|s| {
            if let Some(st) = s.borrow_mut().take() {
                task_queue_push(st, None);
            }
        });
    }

    pub fn close() {}

    pub fn set_exception_handler(handler: fn(&Loop, &ExcContext)) {
        unsafe { EXC_HANDLER = Some(handler) };
    }

    pub fn get_exception_handler() -> Option<fn(&Loop, &ExcContext)> {
        unsafe { EXC_HANDLER }
    }

    pub fn default_exception_handler(_loop: &Loop, ctx: &ExcContext) {
        eprintln!("{}", ctx.message);
        if ctx.future.is_some() {
            eprintln!("future: detached task");
        }
        if ctx.exception.is_some() {
            eprintln!("exception: task error");
        }
    }

    pub fn call_exception_handler(ctx: &ExcContext) {
        let handler = Self::get_exception_handler().unwrap_or(Self::default_exception_handler);
        handler(&Loop, ctx);
    }
}

impl Copy for Loop {}
impl Clone for Loop {
    fn clone(&self) -> Self {
        Loop
    }
}

pub fn get_event_loop(_runq_len: usize, _waitq_len: usize) -> Loop {
    ensure_loop();
    Loop
}

pub fn current_task() -> Rc<RefCell<Task>> {
    cur_task().expect("no running event loop")
}

pub fn new_event_loop() -> Loop {
    TASK_Q.with(|q| *q.borrow_mut() = TaskQueue::new());
    IO_Q.with(|q| *q.borrow_mut() = IoQueue::new());
    Loop
}

#[derive(Debug)]
struct StopperCoro;
impl super::task::Coroutine for StopperCoro {
    fn send(&mut self, _: Option<()>) -> CoroStep {
        CoroStep::Yield
    }
    fn throw(&mut self, _: CancelledError) -> CoroStep {
        CoroStep::Raise
    }
}
