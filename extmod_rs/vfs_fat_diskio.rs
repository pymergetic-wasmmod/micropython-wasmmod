//! rewrite of extmod/vfs_fat_diskio.c
//! Host rewrite uses the Rust `fatfs` crate instead of upstream oofatfs (by design).
// symmetry: done

use std::io::{self, Read, Seek, SeekFrom, Write};

use fatfs::{Date, DateTime, Dir, FileSystem, FormatVolumeOptions, FsOptions, Time};
use py_rs::mperrno::{EEXIST, EIO, EISDIR, ENOENT, ENOTDIR};
use py_rs::obj::Obj;

use crate::vfs_blockdev::{
    self, VfsBlockdev, BLOCKDEV_FLAG_FREE_OBJ, BLOCKDEV_FLAG_NO_FILESYSTEM,
    BLOCKDEV_IOCTL_BLOCK_COUNT, BLOCKDEV_IOCTL_BLOCK_SIZE, BLOCKDEV_IOCTL_INIT,
};

/// Byte stream over `mp_vfs_blockdev_t` for the Rust `fatfs` crate.
pub struct FatBlockStream {
    bdev: *mut VfsBlockdev,
    block_count: usize,
    pos: u64,
}

impl FatBlockStream {
    pub(crate) fn new(bdev: *mut VfsBlockdev, block_count: usize) -> Self {
        Self {
            bdev,
            block_count,
            pos: 0,
        }
    }

    fn bdev(&mut self) -> &mut VfsBlockdev {
        unsafe { &mut *self.bdev }
    }

    fn capacity(&self) -> u64 {
        let block_size = unsafe { (*self.bdev).block_size };
        block_count_u64(self.block_count, block_size)
    }
}

fn block_count_u64(block_count: usize, block_size: usize) -> u64 {
    (block_count as u64).saturating_mul(block_size as u64)
}

impl Read for FatBlockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let cap = self.capacity();
        if self.pos >= cap {
            return Ok(0);
        }
        let block_size = self.bdev().block_size;
        if block_size == 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "zero block size"));
        }
        let mut done = 0usize;
        while done < buf.len() && self.pos < cap {
            let abs = self.pos as usize;
            let block = abs / block_size;
            let off = abs % block_size;
            let chunk = (block_size - off).min(buf.len() - done);
            let ret = vfs_blockdev::blockdev_read_ext(
                self.bdev(),
                block,
                off,
                chunk,
                buf[done..].as_mut_ptr(),
            );
            if ret != 0 {
                return Err(io::Error::new(io::ErrorKind::Other, "blockdev read failed"));
            }
            done += chunk;
            self.pos += chunk as u64;
        }
        Ok(done)
    }
}

impl Write for FatBlockStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let cap = self.capacity();
        let block_size = self.bdev().block_size;
        if block_size == 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "zero block size"));
        }
        let mut done = 0usize;
        while done < buf.len() {
            if self.pos >= cap {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "disk full"));
            }
            let abs = self.pos as usize;
            let block = abs / block_size;
            let off = abs % block_size;
            let chunk = (block_size - off).min(buf.len() - done);
            let ret = vfs_blockdev::blockdev_write_ext(
                self.bdev(),
                block,
                off,
                chunk,
                buf[done..].as_ptr(),
            );
            if ret != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "blockdev write failed",
                ));
            }
            done += chunk;
            self.pos += chunk as u64;
        }
        Ok(done)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for FatBlockStream {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let cap = self.capacity();
        let new_pos = match pos {
            SeekFrom::Start(off) => off,
            SeekFrom::Current(off) => {
                if off >= 0 {
                    self.pos.saturating_add(off as u64)
                } else {
                    self.pos.saturating_sub((-off) as u64)
                }
            }
            SeekFrom::End(off) => {
                if off >= 0 {
                    cap.saturating_add(off as u64)
                } else {
                    cap.saturating_sub((-off) as u64)
                }
            }
        };
        if new_pos > cap {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek past end"));
        }
        self.pos = new_pos;
        Ok(self.pos)
    }
}

/// Metadata returned by [`FatMount::stat_path`].
pub struct FatStat {
    pub is_dir: bool,
    pub size: u64,
    pub modified: DateTime,
}

/// One directory entry for ilistdir.
pub struct FatDirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Mounted FAT volume backed by a MicroPython block device.
pub struct FatMount {
    pub blockdev: VfsBlockdev,
    pub fs: Option<FileSystem<FatBlockStream>>,
    pub no_filesystem: bool,
    /// Current working directory relative to the volume root (empty = root).
    pub cwd: String,
}

pub fn normalize_vfs_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

fn join_path_components(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = if base.is_empty() {
        Vec::new()
    } else {
        base.split('/').filter(|s| !s.is_empty()).collect()
    };
    for comp in rel.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(comp),
        }
    }
    parts.join("/")
}

impl FatMount {
    pub fn create(bdev_obj: Obj, mkfs: bool) -> Result<Box<Self>, i32> {
        let mut blockdev = VfsBlockdev::default();
        blockdev.flags = BLOCKDEV_FLAG_FREE_OBJ;
        blockdev.block_size = 512;
        vfs_blockdev::blockdev_init(&mut blockdev, bdev_obj);
        init_blockdev_sizes(&mut blockdev)?;

        let block_count = match blockdev_count(&mut blockdev) {
            Ok(n) => n,
            Err(e) => return Err(e),
        };

        let mut mount = Box::new(FatMount {
            blockdev,
            fs: None,
            no_filesystem: false,
            cwd: String::new(),
        });
        let bdev_ptr = &mut mount.blockdev as *mut VfsBlockdev;
        let mut stream = FatBlockStream::new(bdev_ptr, block_count);

        if mkfs {
            fatfs::format_volume(&mut stream, FormatVolumeOptions::new())
                .map_err(|_| py_rs::mperrno::EIO)?;
            stream.pos = 0;
        }

        match FileSystem::new(stream, FsOptions::new()) {
            Ok(fs) => {
                mount.fs = Some(fs);
                mount.no_filesystem = false;
            }
            Err(_) => {
                mount.no_filesystem = true;
            }
        }
        Ok(mount)
    }

    pub fn fs(&self) -> Result<&FileSystem<FatBlockStream>, i32> {
        self.fs.as_ref().ok_or(py_rs::mperrno::ENODEV)
    }

    pub fn fs_mut(&mut self) -> Result<&mut FileSystem<FatBlockStream>, i32> {
        self.fs.as_mut().ok_or(py_rs::mperrno::ENODEV)
    }

    pub fn format_existing(&mut self) -> Result<(), i32> {
        let block_count = blockdev_count(&mut self.blockdev)?;
        let bdev_ptr = &mut self.blockdev as *mut VfsBlockdev;
        let mut stream = FatBlockStream::new(bdev_ptr, block_count);
        fatfs::format_volume(&mut stream, FormatVolumeOptions::new())
            .map_err(|_| py_rs::mperrno::EIO)?;
        stream.pos = 0;
        self.fs = Some(FileSystem::new(stream, FsOptions::new()).map_err(|_| py_rs::mperrno::EIO)?);
        self.no_filesystem = false;
        Ok(())
    }

    pub fn resolve_path(&self, path: &str) -> String {
        let path = normalize_vfs_path(path);
        if path.is_empty() {
            return self.cwd.clone();
        }
        join_path_components(&self.cwd, path)
    }

    pub fn chdir(&mut self, path: &str) -> Result<(), i32> {
        let path = normalize_vfs_path(path);
        let resolved = if path.is_empty() {
            String::new()
        } else {
            join_path_components(&self.cwd, path)
        };
        let fs = self.fs()?;
        if !resolved.is_empty() {
            fs.root_dir().open_dir(&resolved).map_err(map_io_err)?;
        }
        self.cwd = resolved;
        Ok(())
    }

    pub fn getcwd(&self) -> String {
        if self.cwd.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", self.cwd)
        }
    }

    fn open_dir_at(&self, path: &str) -> Result<Dir<'_, FatBlockStream>, i32> {
        let fs = self.fs()?;
        let resolved = self.resolve_path(path);
        if resolved.is_empty() {
            Ok(fs.root_dir())
        } else {
            fs.root_dir().open_dir(&resolved).map_err(map_io_err)
        }
    }

    fn find_entry(&self, path: &str) -> Result<FatStat, i32> {
        let resolved = self.resolve_path(path);
        if resolved.is_empty() {
            return Ok(root_stat());
        }
        let fs = self.fs()?;
        let root = fs.root_dir();
        if let Ok(mut file) = root.open_file(&resolved) {
            let size = file.seek(io::SeekFrom::End(0)).map_err(map_io_err)?;
            return Ok(FatStat {
                is_dir: false,
                size,
                modified: default_datetime(),
            });
        }
        if root.open_dir(&resolved).is_ok() {
            return Ok(FatStat {
                is_dir: true,
                size: 0,
                modified: default_datetime(),
            });
        }
        let (parent, name) = split_parent_name(&resolved);
        let parent_dir = if parent.is_empty() {
            root
        } else {
            root.open_dir(parent).map_err(map_io_err)?
        };
        for r in parent_dir.iter() {
            let e = r.map_err(map_io_err)?;
            if e.file_name() == name {
                return Ok(FatStat {
                    is_dir: e.is_dir(),
                    size: e.len(),
                    modified: e.modified(),
                });
            }
        }
        Err(ENOENT)
    }

    pub fn stat_path(&self, path: &str) -> Result<FatStat, i32> {
        let path = normalize_vfs_path(path);
        if path.is_empty() {
            return Ok(root_stat());
        }
        self.find_entry(path)
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<FatDirEntry>, i32> {
        let dir = self.open_dir_at(path)?;
        let mut out = Vec::new();
        for r in dir.iter() {
            let e = r.map_err(map_io_err)?;
            let name = e.file_name();
            if name == "." || name == ".." {
                continue;
            }
            out.push(FatDirEntry {
                name,
                is_dir: e.is_dir(),
                size: e.len(),
            });
        }
        Ok(out)
    }

    pub fn mkdir(&mut self, path: &str) -> Result<(), i32> {
        let resolved = self.resolve_path(path);
        let fs = self.fs_mut()?;
        let root = fs.root_dir();
        if !resolved.is_empty() && root.open_dir(&resolved).is_ok() {
            return Err(EEXIST);
        }
        root.create_dir(&resolved).map_err(map_io_err)?;
        Ok(())
    }

    pub fn remove_path(&mut self, path: &str, dir_only: bool) -> Result<(), i32> {
        let resolved = self.resolve_path(path);
        let stat = self.find_entry(path)?;
        if stat.is_dir {
            if !dir_only {
                return Err(EISDIR);
            }
        } else if dir_only {
            return Err(ENOTDIR);
        }
        let fs = self.fs_mut()?;
        fs.root_dir().remove(&resolved).map_err(map_io_err)?;
        Ok(())
    }

    pub fn rename_path(&mut self, old_path: &str, new_path: &str) -> Result<(), i32> {
        let src = self.resolve_path(old_path);
        let dst = self.resolve_path(new_path);
        let first_err = {
            let fs = self.fs_mut()?;
            let root = fs.root_dir();
            root.rename(&src, &root, &dst).err()
        };
        match first_err {
            None => Ok(()),
            Some(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                self.remove_path(new_path, false)?;
                let fs = self.fs_mut()?;
                let root = fs.root_dir();
                root.rename(&src, &root, &dst).map_err(map_io_err)
            }
            Some(e) => Err(map_io_err(e)),
        }
    }

    pub fn statvfs(&self) -> Result<[isize; 10], i32> {
        let fs = self.fs()?;
        let stats = fs.stats().map_err(|_| EIO)?;
        let bsize = stats.cluster_size() as isize;
        let blocks = stats.total_clusters() as isize;
        let bfree = stats.free_clusters() as isize;
        Ok([bsize, bsize, blocks, bfree, bfree, 0, 0, 0, 0, 255])
    }
}

fn root_stat() -> FatStat {
    FatStat {
        is_dir: true,
        size: 0,
        modified: default_datetime(),
    }
}

fn default_datetime() -> DateTime {
    DateTime {
        date: Date {
            year: 2000,
            month: 1,
            day: 1,
        },
        time: Time {
            hour: 0,
            min: 0,
            sec: 0,
            millis: 0,
        },
    }
}

fn split_parent_name(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((parent, name)) => (parent, name),
        None => ("", path),
    }
}

fn init_blockdev_sizes(bdev: &mut VfsBlockdev) -> Result<(), i32> {
    let _ = vfs_blockdev::blockdev_ioctl(bdev, BLOCKDEV_IOCTL_INIT, 0);
    let ret = vfs_blockdev::blockdev_ioctl(bdev, BLOCKDEV_IOCTL_BLOCK_SIZE, 0);
    if ret != py_rs::obj::CONST_NONE {
        bdev.block_size = py_rs::obj::get_int_truncated(ret) as usize;
    }
    if bdev.block_size == 0 {
        bdev.block_size = 512;
    }
    Ok(())
}

fn blockdev_count(bdev: &mut VfsBlockdev) -> Result<usize, i32> {
    let ret = vfs_blockdev::blockdev_ioctl(bdev, BLOCKDEV_IOCTL_BLOCK_COUNT, 0);
    if ret == py_rs::obj::CONST_NONE {
        Err(py_rs::mperrno::EINVAL)
    } else {
        Ok(py_rs::obj::get_int_truncated(ret) as usize)
    }
}

pub fn map_io_err(err: io::Error) -> i32 {
    use py_rs::mperrno::{EACCES, EEXIST, EINVAL, EIO, EISDIR, ENOENT, ENOSPC, ENOTDIR};
    let msg = err.to_string();
    if msg.contains("Is a directory") {
        return EISDIR;
    }
    if msg.contains("Not a directory") {
        return ENOTDIR;
    }
    if msg.contains("Directory not empty") {
        return EACCES;
    }
    match err.kind() {
        io::ErrorKind::NotFound => ENOENT,
        io::ErrorKind::AlreadyExists => EEXIST,
        io::ErrorKind::PermissionDenied => EACCES,
        io::ErrorKind::WriteZero => ENOSPC,
        io::ErrorKind::InvalidInput => EINVAL,
        _ => EIO,
    }
}

pub fn enabled() -> bool {
    py_rs::mpconfig::VFS_FAT && py_rs::mpconfig::PY_VFS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn fatfs_cursor_mkfs_open_read_write() {
        let mut cursor = Cursor::new(vec![0u8; 512 * 50]);
        fatfs::format_volume(&mut cursor, FormatVolumeOptions::new()).unwrap();
        cursor.set_position(0);
        let fs = FileSystem::new(cursor, FsOptions::new()).unwrap();
        let root = fs.root_dir();
        let mut file = root.create_file("hello.txt").unwrap();
        file.write_all(b"hello!").unwrap();
        file.flush().unwrap();
        drop(file);
        let mut file = root.open_file("hello.txt").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"hello!");
    }

    #[test]
    fn fatfs_mkfs_mkdir_write_ilistdir_stat_read() {
        let mut cursor = Cursor::new(vec![0u8; 512 * 50]);
        fatfs::format_volume(&mut cursor, FormatVolumeOptions::new()).unwrap();
        cursor.set_position(0);
        let fs = FileSystem::new(cursor, FsOptions::new()).unwrap();
        let root = fs.root_dir();

        root.create_dir("testdir").unwrap();
        let mut file = root.create_file("testdir/data.txt").unwrap();
        file.write_all(b"payload").unwrap();
        file.flush().unwrap();
        drop(file);

        let sub = root.open_dir("testdir").unwrap();
        let entries: Vec<_> = sub
            .iter()
            .filter_map(|e| e.ok())
            .map(|e| (e.file_name(), e.is_dir(), e.len()))
            .filter(|(n, _, _)| n != "." && n != "..")
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "data.txt");
        assert!(!entries[0].1);
        assert_eq!(entries[0].2, 7);

        let mut stat = root.open_file("testdir/data.txt").unwrap();
        assert_eq!(stat.seek(io::SeekFrom::End(0)).unwrap(), 7);
        drop(stat);

        let mut file = root.open_file("testdir/data.txt").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"payload");
    }

    #[test]
    fn join_path_components_resolves_dotdot() {
        assert_eq!(join_path_components("a/b", "c"), "a/b/c");
        assert_eq!(join_path_components("a/b", "../c"), "a/c");
        assert_eq!(join_path_components("", "x/y"), "x/y");
    }
}
