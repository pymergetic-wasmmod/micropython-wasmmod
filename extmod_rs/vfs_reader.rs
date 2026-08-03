//! rewrite of extmod/vfs_reader.c
// symmetry: done

use py_rs::map::Map;
use py_rs::mpconfig;
use py_rs::obj::{self, Uint};
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use py_rs::reader::{self, Reader, READER_EOF, READER_IS_ROM};
use py_rs::stream::{
    self, STREAM_ERROR, STREAM_GET_BUFFER_SIZE, STREAM_OP_READ, STREAM_RW_ONCE, STREAM_RW_READ,
};

use crate::vfs;

const DEFAULT_BUFFER_SIZE: usize =
    2 * mpconfig::BYTES_PER_GC_BLOCK as usize - core::mem::size_of::<ReaderVfsHeader>();
const MIN_BUFFER_SIZE: usize =
    mpconfig::BYTES_PER_GC_BLOCK as usize - core::mem::size_of::<ReaderVfsHeader>();
const MAX_BUFFER_SIZE: usize = 255;

#[repr(C)]
struct ReaderVfsHeader {
    file: obj::Obj,
    bufpos: u8,
    buflen: u8,
    bufsize: u8,
}

struct ReaderVfs {
    header: ReaderVfsHeader,
    buf: Vec<u8>,
}

fn reader_vfs_readbyte(data: *mut ()) -> Uint {
    let reader = unsafe { &mut *(data as *mut ReaderVfs) };
    if reader.header.bufpos >= reader.header.buflen {
        if reader.header.buflen < reader.header.bufsize {
            return READER_EOF;
        }
        let mut errcode = 0;
        let n = stream::stream_rw(
            reader.header.file,
            &mut reader.buf,
            &mut errcode,
            STREAM_RW_READ | STREAM_RW_ONCE,
        );
        if errcode != 0 || n == 0 {
            return READER_EOF;
        }
        reader.header.buflen = n as u8;
        reader.header.bufpos = 0;
    }
    let b = reader.buf[reader.header.bufpos as usize];
    reader.header.bufpos += 1;
    Uint::from(b)
}

fn reader_vfs_close(data: *mut ()) {
    let reader = unsafe { Box::from_raw(data as *mut ReaderVfs) };
    stream::stream_close(reader.header.file);
}

fn choose_buffer_size(file: obj::Obj) -> usize {
    let stream_p = stream::get_stream_raise(file, STREAM_OP_READ);
    let mut errcode = 0;
    let bufsize = if let Some(ioctl) = stream_p.ioctl {
        ioctl(file, STREAM_GET_BUFFER_SIZE, 0, &mut errcode)
    } else {
        STREAM_ERROR
    };
    if errcode != 0 || bufsize == 0 || bufsize == STREAM_ERROR {
        DEFAULT_BUFFER_SIZE
    } else {
        bufsize.clamp(MIN_BUFFER_SIZE, MAX_BUFFER_SIZE)
    }
}

/// `mp_reader_new_file` — open path via VFS and attach a buffered reader.
pub fn reader_new_file(reader: &mut Reader, filename: Qstr) {
    if !mpconfig::READER_VFS {
        raise::raise(MpRaise::OSError(0));
    }
    let filename_obj = obj::new_qstr(filename);
    let mode = obj::new_qstr(qstr::from_str("rb"));
    let mut kw = Map::default();
    let file = vfs::open(2, &[filename_obj, mode], &mut kw);

    if mpconfig::VFS_ROM {
        let mut bufinfo = obj::BufferInfo::default();
        if obj::get_buffer(file, &mut bufinfo, obj::BUFFER_READ) {
            reader::reader_new_mem(reader, bufinfo.buf, bufinfo.len, READER_IS_ROM);
            return;
        }
    }

    let bufsize = choose_buffer_size(file);
    let mut rf = ReaderVfs {
        header: ReaderVfsHeader {
            file,
            bufpos: 0,
            buflen: 0,
            bufsize: bufsize as u8,
        },
        buf: vec![0; bufsize],
    };
    let mut errcode = 0;
    rf.header.buflen = stream::stream_rw(
        rf.header.file,
        &mut rf.buf,
        &mut errcode,
        STREAM_RW_READ | STREAM_RW_ONCE,
    ) as u8;
    if errcode != 0 {
        raise::raise(MpRaise::OSError(errcode));
    }
    rf.header.bufpos = 0;
    let raw = Box::into_raw(Box::new(rf));
    reader.data = raw as *mut ();
    reader.readbyte = reader_vfs_readbyte;
    reader.close = reader_vfs_close;
}

/// Register VFS reader hook with `py_rs::reader`.
pub fn init() {
    if mpconfig::READER_VFS {
        reader::set_reader_new_file_hook(reader_new_file);
    }
}

pub fn enabled() -> bool {
    mpconfig::READER_VFS
}
