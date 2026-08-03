//! rewrite of shared/memzip/memzip.c + shared/memzip/memzip.h
// symmetry: done

use core::mem;

pub const FILE_HEADER_SIGNATURE: u32 = 0x0403_4b50;

#[repr(C, packed)]
pub struct FileHdr {
    pub signature: u32,
    pub version: u16,
    pub flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename_len: u16,
    pub extra_len: u16,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemzipResult {
    Ok = 0,
    NoFile = 1,
    FileCompressed = 2,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileInfo {
    pub file_size: u32,
    pub last_mod_date: u16,
    pub last_mod_time: u16,
    pub is_dir: u8,
}

/// Port-provided embedded zip blob.
static mut MEMZIP_DATA: Option<&'static [u8]> = None;

pub fn set_memzip_data(data: &'static [u8]) {
    unsafe {
        MEMZIP_DATA = Some(data);
    }
}

fn memzip_data() -> Option<&'static [u8]> {
    unsafe { MEMZIP_DATA }
}

fn strip_leading_slash(filename: &str) -> &str {
    filename.strip_prefix('/').unwrap_or(filename)
}

fn find_file_header(filename: &str) -> Option<(usize, FileHdr)> {
    let data = memzip_data()?;
    let filename = strip_leading_slash(filename);
    let mut offset = 0usize;
    while offset + mem::size_of::<FileHdr>() <= data.len() {
        let hdr = unsafe { (data.as_ptr().add(offset) as *const FileHdr).read_unaligned() };
        if hdr.signature != FILE_HEADER_SIGNATURE {
            break;
        }
        let name_offset = offset + mem::size_of::<FileHdr>();
        let name_end = name_offset + hdr.filename_len as usize;
        if name_end > data.len() {
            break;
        }
        let file_name = std::str::from_utf8(&data[name_offset..name_end]).ok()?;
        if file_name == filename {
            return Some((offset, hdr));
        }
        let next = name_end + hdr.extra_len as usize + hdr.uncompressed_size as usize;
        offset = next;
    }
    None
}

pub fn is_dir(filename: &str) -> bool {
    if filename == "/" {
        return true;
    }
    let data = match memzip_data() {
        Some(d) => d,
        None => return false,
    };
    let filename = strip_leading_slash(filename);
    let filename_len = filename.len();
    let mut offset = 0usize;
    while offset + mem::size_of::<FileHdr>() <= data.len() {
        let hdr = unsafe { (data.as_ptr().add(offset) as *const FileHdr).read_unaligned() };
        if hdr.signature != FILE_HEADER_SIGNATURE {
            break;
        }
        let name_offset = offset + mem::size_of::<FileHdr>();
        let name_end = name_offset + hdr.filename_len as usize;
        if name_end > data.len() {
            break;
        }
        let file_name = match std::str::from_utf8(&data[name_offset..name_end]) {
            Ok(s) => s,
            Err(_) => break,
        };
        if filename_len < file_name.len()
            && file_name.starts_with(filename)
            && file_name.as_bytes().get(filename_len) == Some(&b'/')
        {
            return true;
        }
        offset = name_end + hdr.extra_len as usize + hdr.uncompressed_size as usize;
    }
    false
}

pub fn locate(filename: &str) -> Result<(&'static [u8], usize), MemzipResult> {
    let (_, hdr) = find_file_header(filename).ok_or(MemzipResult::NoFile)?;
    if hdr.compression_method != 0 {
        return Err(MemzipResult::FileCompressed);
    }
    let data = memzip_data().ok_or(MemzipResult::NoFile)?;
    let base = find_file_header(filename).unwrap().0;
    let start = base + mem::size_of::<FileHdr>() + hdr.filename_len as usize + hdr.extra_len as usize;
    let end = start + hdr.uncompressed_size as usize;
    if end > data.len() {
        return Err(MemzipResult::NoFile);
    }
    Ok((&data[start..end], hdr.uncompressed_size as usize))
}

pub fn stat(path: &str) -> Result<FileInfo, MemzipResult> {
    if let Some((_, hdr)) = find_file_header(path) {
        return Ok(FileInfo {
            file_size: hdr.uncompressed_size,
            last_mod_date: hdr.last_mod_date,
            last_mod_time: hdr.last_mod_time,
            is_dir: 0,
        });
    }
    if is_dir(path) {
        return Ok(FileInfo {
            file_size: 0,
            last_mod_date: 0,
            last_mod_time: 0,
            is_dir: 1,
        });
    }
    Err(MemzipResult::NoFile)
}
