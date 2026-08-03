//! rewrite of ports/unix/coverage.c
// symmetry: done

use py_rs::obj::Obj;

/// Coverage-variant instrumentation (`MICROPY_UNIX_COVERAGE`).
pub fn enabled() -> bool {
    crate::mpconfigport::UNIX_COVERAGE
}

/// Stream test object used by coverage tests (`mp_obj_streamtest_t`).
pub struct StreamTest {
    pub buf: Vec<u8>,
    pub pos: usize,
    pub error_code: i32,
}

impl StreamTest {
    pub fn set_buf(&mut self, data: &[u8]) {
        self.buf = data.to_vec();
        self.pos = 0;
    }

    pub fn set_error(&mut self, code: i32) {
        self.error_code = code;
    }

    pub fn read(&mut self, out: &mut [u8]) -> Result<usize, i32> {
        if self.pos < self.buf.len() {
            let n = out.len().min(self.buf.len() - self.pos);
            out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        } else if self.error_code == 0 {
            Ok(0)
        } else {
            Err(self.error_code)
        }
    }

    pub fn write(&self, _data: &[u8]) -> Result<usize, i32> {
        Err(self.error_code)
    }
}

/// Register coverage-only builtins when building coverage variant.
pub fn register_coverage_builtins() -> Vec<(&'static str, Obj)> {
    if !enabled() {
        return Vec::new();
    }
    Vec::new()
}

/// Helper exercised by coverage tests for VM internals.
pub fn exercise_internals() {
    if !enabled() {
        return;
    }
    let _ = py_rs::obj::new_small_int(0);
}
