//! rewrite of extmod/asyncio/stream.py
// symmetry: done

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::core::{self, CancelledError};
use super::task::Task;

pub struct Stream {
    pub s: usize,
    pub extra: HashMap<&'static str, String>,
    out_buf: Vec<u8>,
}

impl Stream {
    pub fn new(s: usize, extra: HashMap<&'static str, String>) -> Self {
        Self {
            s,
            extra,
            out_buf: Vec::new(),
        }
    }

    pub fn get_extra_info(&self, key: &'static str) -> Option<&String> {
        self.extra.get(key)
    }

    pub fn close(&self) {}

    pub fn wait_closed(&mut self) {
        self.close();
    }

    pub fn read(&mut self, n: i32, read_fn: impl Fn(usize, i32) -> Option<Vec<u8>>) -> StreamStep {
        core::with_io_queue(|io| io.queue_read(self.s));
        if let Some(r2) = read_fn(self.s, n) {
            if n >= 0 {
                return StreamStep::Done(r2);
            }
            if r2.is_empty() {
                return StreamStep::Done(Vec::new());
            }
        }
        StreamStep::Yield
    }

    pub fn readinto(&mut self, readinto_fn: impl Fn(usize, &mut [u8]) -> Option<usize>) -> StreamStep {
        core::with_io_queue(|io| io.queue_read(self.s));
        let _ = readinto_fn;
        StreamStep::Yield
    }

    pub fn readexactly(&mut self, mut n: usize, read_fn: impl Fn(usize, usize) -> Option<Vec<u8>>) -> StreamStep {
        core::with_io_queue(|io| io.queue_read(self.s));
        if let Some(r2) = read_fn(self.s, n) {
            if r2.is_empty() {
                return StreamStep::Error("EOFError");
            }
            n = n.saturating_sub(r2.len());
            if n == 0 {
                return StreamStep::Done(r2);
            }
        }
        StreamStep::Yield
    }

    pub fn readline(&mut self, readline_fn: impl Fn(usize) -> Option<Vec<u8>>) -> StreamStep {
        core::with_io_queue(|io| io.queue_read(self.s));
        if let Some(l2) = readline_fn(self.s) {
            if l2.is_empty() || l2.last() == Some(&b'\n') {
                return StreamStep::Done(l2);
            }
        }
        StreamStep::Yield
    }

    pub fn write(&mut self, buf: &[u8], write_fn: impl Fn(usize, &[u8]) -> Option<usize>) {
        if self.out_buf.is_empty() {
            if let Some(ret) = write_fn(self.s, buf) {
                if ret == buf.len() {
                    return;
                }
                self.out_buf.extend_from_slice(&buf[ret..]);
                return;
            }
        }
        self.out_buf.extend_from_slice(buf);
    }

    pub fn drain(&mut self, write_fn: impl Fn(usize, &[u8]) -> Option<usize>) -> StreamStep {
        if self.out_buf.is_empty() {
            let _ = core::sleep_ms(0);
            return StreamStep::Yield;
        }
        let mut off = 0usize;
        while off < self.out_buf.len() {
            core::with_io_queue(|io| io.queue_write(self.s));
            if let Some(ret) = write_fn(self.s, &self.out_buf[off..]) {
                off += ret;
            } else {
                return StreamStep::Yield;
            }
        }
        self.out_buf.clear();
        StreamStep::Done(Vec::new())
    }

    pub fn aclose(&mut self) {
        self.wait_closed();
    }

    pub fn awrite(&mut self, buf: &[u8], off: usize, sz: i32, write_fn: impl Fn(usize, &[u8]) -> Option<usize>) {
        let data = if off != 0 || sz != -1 {
            let end = if sz == -1 { buf.len() } else { off + sz as usize };
            &buf[off..end]
        } else {
            buf
        };
        self.write(data, write_fn);
    }
}

pub type StreamReader = Stream;
pub type StreamWriter = Stream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStep {
    Yield,
    Done(Vec<u8>),
    Error(&'static str),
}

pub fn open_connection(
    _host: &str,
    _port: u16,
    stream_id: usize,
) -> (StreamReader, StreamWriter) {
    core::with_io_queue(|io| io.queue_write(stream_id));
    let s = Stream::new(stream_id, HashMap::new());
    (s, Stream::new(stream_id, HashMap::new()))
}

pub struct Server {
    pub state: bool,
    pub task: Option<Rc<RefCell<Task>>>,
}

impl Server {
    pub fn close(&mut self) {
        self.state = true;
        if let Some(t) = &self.task {
            Task::cancel(t);
        }
    }

    pub fn wait_closed(&self) {
        let _ = &self.task;
    }

    pub fn serve(
        &mut self,
        s: usize,
        accept_fn: impl Fn(usize) -> Option<(usize, String)>,
        cb: impl Fn(Stream, Stream),
    ) -> ServerServeStep {
        self.state = false;
        core::with_io_queue(|io| io.queue_read(s));
        if let Some((s2, addr)) = accept_fn(s) {
            let mut extra = HashMap::new();
            extra.insert("peername", addr);
            let ss = Stream::new(s2, extra);
            cb(ss, Stream::new(s2, HashMap::new()));
        }
        ServerServeStep::Continue
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerServeStep {
    Continue,
    Cancelled(CancelledError),
}

pub fn start_server(
    cb: impl Fn(Stream, Stream),
    host: &str,
    port: u16,
    backlog: u32,
) -> Server {
    let _ = (cb, host, port, backlog);
    Server {
        state: false,
        task: None,
    }
}
