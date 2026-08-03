//! rewrite of py/reader.c + py/reader.h
// symmetry: done

use crate::malloc;
use crate::misc::Byte;
use crate::mpconfig;
use crate::obj::Uint;
use crate::qstr::Qstr;
use crate::raise::{self, MpRaise};

/// ROM marker for `mp_reader_new_mem` (`MP_READER_IS_ROM`).
pub const READER_IS_ROM: usize = usize::MAX;

/// End-of-stream marker (`MP_READER_EOF`).
pub const READER_EOF: Uint = Uint::MAX;

/// Lexer input source (`mp_reader_t`).
pub struct Reader {
    pub data: *mut (),
    pub readbyte: fn(*mut ()) -> Uint,
    pub close: fn(*mut ()),
}

/// Optional hook for VFS-backed `reader_new_file` (registered by `extmod_rs::vfs_reader`).
pub type ReaderNewFileFn = fn(&mut Reader, Qstr);

static mut READER_NEW_FILE_HOOK: Option<ReaderNewFileFn> = None;

/// Register VFS reader implementation (`MICROPY_READER_VFS`).
pub fn set_reader_new_file_hook(f: ReaderNewFileFn) {
    unsafe { READER_NEW_FILE_HOOK = Some(f) };
}

#[repr(C)]
struct ReaderMem {
    free_len: usize,
    beg: *const Byte,
    cur: *const Byte,
    end: *const Byte,
}

pub(crate) fn reader_mem_readbyte(data: *mut ()) -> Uint {
    let reader = unsafe { &mut *(data as *mut ReaderMem) };
    if reader.cur < reader.end {
        let b = unsafe { *reader.cur };
        reader.cur = unsafe { reader.cur.add(1) };
        Uint::from(b)
    } else {
        READER_EOF
    }
}

pub(crate) fn reader_mem_close(data: *mut ()) -> () {
    let reader = unsafe { Box::from_raw(data as *mut ReaderMem) };
    if reader.free_len > 0 && reader.free_len != READER_IS_ROM {
        unsafe {
            let len = reader.end as usize - reader.beg as usize;
            malloc::del(reader.beg as *mut Byte, len);
        }
    }
}

/// Create memory reader (`mp_reader_new_mem`).
pub fn reader_new_mem(reader: &mut Reader, buf: *const Byte, len: usize, free_len: usize) {
    let rm = Box::into_raw(Box::new(ReaderMem {
        free_len,
        beg: buf,
        cur: buf,
        end: unsafe { buf.add(len) },
    }));
    reader.data = rm as *mut ();
    reader.readbyte = reader_mem_readbyte;
    reader.close = reader_mem_close;
}

/// Try bulk read from ROM reader (`mp_reader_try_read_rom`).
pub fn reader_try_read_rom(reader: &Reader, len: usize) -> Option<*const u8> {
    if reader.readbyte as usize != reader_mem_readbyte as *const () as usize {
        return None;
    }
    let m = unsafe { &mut *(reader.data as *mut ReaderMem) };
    if m.free_len != READER_IS_ROM {
        return None;
    }
    let data = m.cur;
    m.cur = unsafe { m.cur.add(len) };
    Some(data as *const u8)
}

#[cfg(unix)]
mod posix {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::{FromRawFd, IntoRawFd, RawFd};

    struct ReaderPosix {
        close_fd: bool,
        file: File,
        len: usize,
        pos: usize,
        buf: [u8; 20],
    }

    fn reader_posix_readbyte(data: *mut ()) -> Uint {
        let reader = unsafe { &mut *(data as *mut ReaderPosix) };
        if reader.pos >= reader.len {
            if reader.len == 0 {
                return READER_EOF;
            }
            match reader.file.read(&mut reader.buf) {
                Ok(0) => {
                    reader.len = 0;
                    return READER_EOF;
                }
                Ok(n) => {
                    reader.len = n;
                    reader.pos = 0;
                }
                Err(_) => {
                    reader.len = 0;
                    return READER_EOF;
                }
            }
        }
        let b = reader.buf[reader.pos];
        reader.pos += 1;
        Uint::from(b)
    }

    fn reader_posix_close(data: *mut ()) {
        let _ = unsafe { Box::from_raw(data as *mut ReaderPosix) };
    }

    /// Create reader from open file descriptor (`mp_reader_new_file_from_fd`).
    pub fn reader_new_file_from_fd(reader: &mut Reader, fd: RawFd, close_fd: bool) {
        let mut file = unsafe { File::from_raw_fd(fd) };
        let mut buf = [0u8; 20];
        let n = match file.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                if close_fd {
                    let _ = file.into_raw_fd();
                }
                raise::raise(MpRaise::OSError(e.raw_os_error().unwrap_or(0)));
            }
        };
        let rp = Box::into_raw(Box::new(ReaderPosix {
            close_fd,
            file,
            len: n,
            pos: 0,
            buf,
        }));
        reader.data = rp as *mut ();
        reader.readbyte = reader_posix_readbyte;
        reader.close = reader_posix_close;
    }

    /// Open file by qstr path (`mp_reader_new_file`).
    pub fn reader_new_file(reader: &mut Reader, filename: Qstr) {
        let path = crate::qstr::str_data(filename)
            .and_then(|v| String::from_utf8(v).ok())
            .unwrap_or_default();
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => raise::raise(MpRaise::OSError(e.raw_os_error().unwrap_or(0))),
        };
        reader_new_file_from_fd(reader, file.into_raw_fd(), true);
    }
}

/// Create reader from open file descriptor (`mp_reader_new_file_from_fd`).
pub fn reader_new_file_from_fd(reader: &mut Reader, fd: i32, close_fd: bool) {
    if mpconfig::READER_POSIX {
        #[cfg(unix)]
        {
            posix::reader_new_file_from_fd(reader, fd, close_fd);
            return;
        }
    }
    let _ = (reader, fd, close_fd);
    raise::raise(MpRaise::OSError(0));
}

/// Open file by qstr path (`mp_reader_new_file`).
pub fn reader_new_file(reader: &mut Reader, filename: Qstr) {
    if mpconfig::READER_VFS {
        unsafe {
            if let Some(f) = READER_NEW_FILE_HOOK {
                f(reader, filename);
                return;
            }
        }
    }
    if mpconfig::READER_POSIX {
        #[cfg(unix)]
        {
            posix::reader_new_file(reader, filename);
            return;
        }
    }
    let _ = (reader, filename);
    raise::raise(MpRaise::OSError(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_reader_reads_bytes() {
        let data = b"abc";
        let mut reader = Reader {
            data: std::ptr::null_mut(),
            readbyte: reader_mem_readbyte,
            close: reader_mem_close,
        };
        reader_new_mem(&mut reader, data.as_ptr(), data.len(), READER_IS_ROM);
        assert_eq!((reader.readbyte)(reader.data), b'a' as Uint);
        assert_eq!((reader.readbyte)(reader.data), b'b' as Uint);
        (reader.close)(reader.data);
    }
}
