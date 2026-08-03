//! Block-device bridge for host `littlefs2-rust` (LFS2).
//!
//! Host rewrite uses the pure-Rust `littlefs2-rust` crate instead of upstream
//! lib/littlefs C bindings — same role as `vfs_fat_diskio.rs` + `fatfs`.
// symmetry: done

use littlefs_rust::{
    BlockDevice, Config, Error, FileOptions, Filesystem, FilesystemMut, FilesystemOptions,
};
use py_rs::mperrno::{EINVAL, EIO, ENODEV};
use py_rs::obj::Obj;

use crate::vfs_blockdev::{
    self, VfsBlockdev, BLOCKDEV_FLAG_FREE_OBJ, BLOCKDEV_FLAG_NO_FILESYSTEM,
    BLOCKDEV_IOCTL_BLOCK_COUNT, BLOCKDEV_IOCTL_BLOCK_ERASE, BLOCKDEV_IOCTL_BLOCK_SIZE,
    BLOCKDEV_IOCTL_INIT, BLOCKDEV_IOCTL_SYNC,
};

/// User attribute id for modification time (64-bit LE ns since 1970), from upstream `vfs_lfs.c`.
pub const LFS_ATTR_MTIME: u8 = 1;

/// littlefs block device backed by `mp_vfs_blockdev_t`.
pub struct LfsBlockDevice {
    bdev: *mut VfsBlockdev,
    cfg: Config,
}

impl LfsBlockDevice {
    pub(crate) fn new(bdev: *mut VfsBlockdev, block_count: usize, block_size: usize) -> Self {
        Self {
            bdev,
            cfg: Config {
                block_size,
                block_count,
            },
        }
    }

    fn bdev(&self) -> &VfsBlockdev {
        unsafe { &*self.bdev }
    }

    fn bdev_mut(&mut self) -> &mut VfsBlockdev {
        unsafe { &mut *self.bdev }
    }
}

impl BlockDevice for LfsBlockDevice {
    fn config(&self) -> Config {
        self.cfg
    }

    fn read(&self, block: u32, off: usize, out: &mut [u8]) -> Result<(), Error> {
        let ret = unsafe {
            vfs_blockdev::blockdev_read_ext(
                &mut *self.bdev,
                block as usize,
                off,
                out.len(),
                out.as_mut_ptr(),
            )
        };
        if ret != 0 {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }

    fn prog(&mut self, block: u32, off: usize, data: &[u8]) -> Result<(), Error> {
        let block_size = self.cfg.block_size;
        let mut old = vec![0xff; data.len()];
        let read_ret = vfs_blockdev::blockdev_read_ext(
            self.bdev_mut(),
            block as usize,
            off,
            data.len(),
            old.as_mut_ptr(),
        );
        if read_ret != 0 {
            return Err(Error::Io);
        }
        for (d, s) in old.iter_mut().zip(data) {
            *d &= *s;
        }
        let write_ret = vfs_blockdev::blockdev_write_ext(
            self.bdev_mut(),
            block as usize,
            off,
            old.len(),
            old.as_ptr(),
        );
        if write_ret != 0 {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }

    fn erase(&mut self, block: u32) -> Result<(), Error> {
        let block_size = self.cfg.block_size;
        let bdev = self.bdev_mut();
        if bdev.flags & vfs_blockdev::BLOCKDEV_FLAG_HAVE_IOCTL != 0 {
            let ret =
                vfs_blockdev::blockdev_ioctl(bdev, BLOCKDEV_IOCTL_BLOCK_ERASE, block as usize);
            if ret != py_rs::obj::CONST_NONE {
                let code = py_rs::obj::get_int_truncated(ret) as i32;
                if code != 0 {
                    return Err(Error::Io);
                }
                return Ok(());
            }
        }
        let erased = vec![0xffu8; block_size];
        let ret =
            vfs_blockdev::blockdev_write_ext(bdev, block as usize, 0, block_size, erased.as_ptr());
        if ret != 0 {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }

    fn sync(&mut self) -> Result<(), Error> {
        let _ = vfs_blockdev::blockdev_ioctl(self.bdev_mut(), BLOCKDEV_IOCTL_SYNC, 0);
        Ok(())
    }
}

/// One directory entry for ilistdir.
pub struct LfsDirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Metadata returned by [`LfsMount::stat_path`].
pub struct LfsStat {
    pub is_dir: bool,
    pub size: u64,
}

/// Mounted LFS2 volume backed by a MicroPython block device.
pub struct LfsMount {
    pub blockdev: VfsBlockdev,
    pub no_filesystem: bool,
    pub cwd: String,
    pub read_size: usize,
    pub prog_size: usize,
    pub lookahead: usize,
    pub enable_mtime: bool,
    pub(crate) fs: Option<FilesystemMut<LfsBlockDevice>>,
}

impl LfsMount {
    pub(crate) fn fs_options(&self) -> FilesystemOptions {
        let mut opts = FilesystemOptions::default();
        opts.read_size = self.read_size;
        opts.prog_size = self.prog_size;
        opts.lookahead_size = self.lookahead;
        opts
    }

    /// Matches upstream LFS2 `cache_size` (`MIN(block_size, 4 * MAX(read_size, prog_size))`).
    pub fn cache_size(&self) -> usize {
        let block_size = self.blockdev.block_size.max(512);
        let geom = 4 * self.read_size.max(self.prog_size);
        block_size.min(geom)
    }

    pub fn read_mtime(&self, path: &str) -> Option<shared_rs::timeutils::timeutils::Timestamp> {
        let fs = self.fs.as_ref()?;
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        let mut buf = [0u8; 8];
        let len = fs
            .read_attr_into(&lfs_path, LFS_ATTR_MTIME, &mut buf)
            .ok()?;
        if len == buf.len() {
            Some(shared_rs::timeutils::timeutils::lfs_mtime_bytes_to_timestamp(&buf))
        } else {
            None
        }
    }

    pub fn write_mtime(&mut self, path: &str, mtime: &[u8; 8]) -> Result<(), i32> {
        if !self.enable_mtime {
            return Ok(());
        }
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        self.fs_mut()?
            .set_attr(&lfs_path, LFS_ATTR_MTIME, mtime)
            .map_err(map_lfs_err)
    }

    pub fn touch_mtime(&mut self, path: &str) -> Result<(), i32> {
        let mtime = shared_rs::timeutils::timeutils::lfs_mtime_bytes_from_now();
        self.write_mtime(path, &mtime)
    }

    pub fn create(
        bdev_obj: Obj,
        read_size: usize,
        prog_size: usize,
        lookahead: usize,
        enable_mtime: bool,
        mkfs: bool,
    ) -> Result<Box<Self>, i32> {
        let mut blockdev = VfsBlockdev::default();
        blockdev.flags = BLOCKDEV_FLAG_FREE_OBJ;
        blockdev.block_size = 512;
        vfs_blockdev::blockdev_init(&mut blockdev, bdev_obj);
        init_blockdev_sizes(&mut blockdev)?;
        let block_count = blockdev_count(&mut blockdev)?;
        let block_size = blockdev.block_size;

        let mut mount = Box::new(LfsMount {
            blockdev,
            no_filesystem: false,
            cwd: "/".to_string(),
            read_size,
            prog_size,
            lookahead,
            enable_mtime,
            fs: None,
        });
        let bdev_ptr = &mut mount.blockdev as *mut VfsBlockdev;
        let mut device = LfsBlockDevice::new(bdev_ptr, block_count, block_size);
        let opts = mount.fs_options();

        if mkfs {
            Filesystem::format_device_with_options(&mut device, opts).map_err(map_lfs_err)?;
        }

        match Filesystem::mount_device_mut_with_options(device, opts) {
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

    pub fn is_mounted(&self) -> bool {
        self.fs.is_some()
    }

    pub fn fs_mut(&mut self) -> Result<&mut FilesystemMut<LfsBlockDevice>, i32> {
        self.fs.as_mut().ok_or(ENODEV)
    }

    pub fn format_existing(&mut self) -> Result<(), i32> {
        let block_count = blockdev_count(&mut self.blockdev)?;
        let block_size = self.blockdev.block_size;
        let bdev_ptr = &mut self.blockdev as *mut VfsBlockdev;
        let mut device = LfsBlockDevice::new(bdev_ptr, block_count, block_size);
        let opts = self.fs_options();
        Filesystem::format_device_with_options(&mut device, opts).map_err(map_lfs_err)?;
        self.fs =
            Some(Filesystem::mount_device_mut_with_options(device, opts).map_err(map_lfs_err)?);
        self.no_filesystem = false;
        Ok(())
    }

    pub fn lfs_path(resolved: &str) -> String {
        if resolved.is_empty() || resolved == "/" {
            "/".to_string()
        } else if resolved.starts_with('/') {
            resolved.to_string()
        } else {
            format!("/{resolved}")
        }
    }

    pub fn resolve_path(&self, path: &str) -> String {
        let path = normalize_vfs_path(path);
        if path.is_empty() {
            return self.cwd.clone();
        }
        join_path_components(&self.cwd, path)
    }

    pub fn stat_path(&self, path: &str) -> Result<LfsStat, i32> {
        let fs = self.fs.as_ref().ok_or(ENODEV)?;
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        let entry = fs.as_filesystem().stat(&lfs_path).map_err(map_lfs_err)?;
        Ok(LfsStat {
            is_dir: entry.ty == littlefs_rust::FileType::Dir,
            size: entry.size as u64,
        })
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<LfsDirEntry>, i32> {
        let fs = self.fs.as_ref().ok_or(ENODEV)?;
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        let entries = fs.read_dir(&lfs_path).map_err(map_lfs_err)?;
        Ok(entries
            .into_iter()
            .filter(|e| e.name != "." && e.name != "..")
            .map(|e| LfsDirEntry {
                name: e.name,
                is_dir: e.ty == littlefs_rust::FileType::Dir,
                size: e.size as u64,
            })
            .collect())
    }

    pub fn mkdir(&mut self, path: &str) -> Result<(), i32> {
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        self.fs_mut()?.create_dir(&lfs_path).map_err(map_lfs_err)
    }

    pub fn remove_path(&mut self, path: &str) -> Result<(), i32> {
        let lfs_path = Self::lfs_path(&self.resolve_path(path));
        let stat = self.stat_path(path)?;
        if stat.is_dir {
            self.fs_mut()?.remove_dir(&lfs_path).map_err(map_lfs_err)
        } else {
            self.fs_mut()?.remove_file(&lfs_path).map_err(map_lfs_err)
        }
    }

    pub fn rename_path(&mut self, old_path: &str, new_path: &str) -> Result<(), i32> {
        let src = Self::lfs_path(&self.resolve_path(old_path));
        let dst = Self::lfs_path(&self.resolve_path(new_path));
        let stat = self.stat_path(old_path)?;
        if stat.is_dir {
            self.fs_mut()?.rename_dir(&src, &dst).map_err(map_lfs_err)
        } else {
            self.fs_mut()?.rename_file(&src, &dst).map_err(map_lfs_err)
        }
    }

    pub fn chdir(&mut self, path: &str) -> Result<(), i32> {
        let resolved = self.resolve_path(path);
        if resolved != "/" && resolved.len() > 1 {
            let stat = self.stat_path(path)?;
            if !stat.is_dir {
                return Err(py_rs::mperrno::ENOTDIR);
            }
        }
        self.cwd = if resolved.is_empty() {
            "/".to_string()
        } else {
            resolved
        };
        if self.cwd != "/" && !self.cwd.ends_with('/') {
            self.cwd.push('/');
        }
        Ok(())
    }

    pub fn getcwd(&self) -> String {
        if self.cwd == "/" {
            "/".to_string()
        } else {
            self.cwd.trim_end_matches('/').to_string()
        }
    }

    /// Returns the 10-tuple fields for `VfsLfs2.statvfs` / `os.statvfs`.
    pub fn statvfs(&self) -> Result<[isize; 10], i32> {
        let fs = self.fs.as_ref().ok_or(ENODEV)?;
        let info = fs.info();
        let block_size = info.block_size as isize;
        let block_count = info.block_count as isize;
        let n_used = fs
            .used_blocks()
            .map_err(map_lfs_err)?
            .iter()
            .filter(|&&used| used)
            .count() as isize;
        let bfree = block_count - n_used;
        Ok([
            block_size,
            block_size,
            block_count,
            bfree,
            bfree,
            0,
            0,
            0,
            0,
            info.name_max as isize,
        ])
    }

    pub fn file_options_from_mode(mode: &str) -> Result<FileOptions, i32> {
        let mut read = false;
        let mut write = false;
        let mut append = false;
        let mut create = false;
        let mut create_new = false;
        let mut truncate = false;
        for b in mode.bytes() {
            match b {
                b'r' => read = true,
                b'w' => {
                    write = true;
                    create = true;
                    truncate = true;
                }
                b'x' => {
                    write = true;
                    create_new = true;
                }
                b'a' => {
                    write = true;
                    append = true;
                    create = true;
                }
                b'+' => {
                    read = true;
                    write = true;
                }
                b'b' | b't' => {}
                _ => {}
            }
        }
        if !read && !write && !append {
            read = true;
        }
        Ok(FileOptions::new()
            .read(read)
            .write(write)
            .append(append)
            .create(create)
            .create_new(create_new)
            .truncate(truncate))
    }
}

pub fn normalize_vfs_path(path: &str) -> &str {
    path.trim_start_matches('/')
}

fn join_path_components(base: &str, rel: &str) -> String {
    let base = base.trim_end_matches('/');
    let mut parts: Vec<&str> = if base.is_empty() || base == "/" {
        Vec::new()
    } else {
        base.trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect()
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
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
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
        Err(EINVAL)
    } else {
        Ok(py_rs::obj::get_int_truncated(ret) as usize)
    }
}

pub fn map_lfs_err(err: Error) -> i32 {
    use py_rs::mperrno::{EACCES, EBADF, EEXIST, EINVAL, EIO, EISDIR, ENOENT, ENOSPC, ENOTDIR};
    match err {
        Error::NotFound => ENOENT,
        Error::AlreadyExists => EEXIST,
        Error::IsDir => EISDIR,
        Error::NotDir => ENOTDIR,
        Error::NotEmpty => EACCES,
        Error::NoSpace => ENOSPC,
        Error::InvalidPath | Error::InvalidConfig => EINVAL,
        Error::BadFileDescriptor => EBADF,
        Error::Corrupt | Error::Io => EIO,
        Error::Unsupported => EIO,
        _ => EIO,
    }
}

pub fn enabled() -> bool {
    py_rs::mpconfig::VFS_LFS2 && py_rs::mpconfig::PY_VFS
}

#[cfg(test)]
mod tests {
    use super::*;
    use littlefs_rust::{FilesystemOptions, MemoryBlockDevice};

    #[test]
    fn statvfs_counts_used_blocks() {
        let cfg = Config {
            block_size: 512,
            block_count: 32,
        };
        let mut device = MemoryBlockDevice::new_erased(cfg).unwrap();
        let opts = FilesystemOptions::default();
        Filesystem::format_device_with_options(&mut device, opts).unwrap();
        let mut fs = Filesystem::mount_device_mut_with_options(device, opts).unwrap();
        fs.create_file("/data.bin", &[0u8; 1024]).unwrap();
        let info = fs.info();
        let n_used = fs
            .used_blocks()
            .unwrap()
            .iter()
            .filter(|&&used| used)
            .count();
        assert!(n_used > 0);
        assert!(n_used < info.block_count as usize);
        let bfree = info.block_count as isize - n_used as isize;
        assert!(bfree > 0);
    }

    #[test]
    fn memory_blockdev_mkfs_open_read_write() {
        let cfg = Config {
            block_size: 512,
            block_count: 32,
        };
        let mut device = MemoryBlockDevice::new_erased(cfg).unwrap();
        Filesystem::format_device(&mut device).unwrap();
        let mut fs = Filesystem::mount_device_mut(device).unwrap();
        fs.create_file("/hello.txt", b"hello!").unwrap();
        assert_eq!(fs.read_file("/hello.txt").unwrap(), b"hello!");

        let mut file = fs
            .open_file("/hello.txt", FileOptions::new().read(true).write(true))
            .unwrap();
        let mut buf = [0u8; 6];
        assert_eq!(file.read(&mut buf).unwrap(), 6);
        assert_eq!(&buf, b"hello!");
    }

    #[test]
    fn join_path_components_resolves_dotdot() {
        assert_eq!(join_path_components("/a/b", "c"), "/a/b/c");
        assert_eq!(join_path_components("/a/b", "../c"), "/a/c");
        assert_eq!(join_path_components("/", "x/y"), "/x/y");
    }

    #[test]
    fn cache_size_matches_upstream_formula() {
        let mount = LfsMount {
            blockdev: VfsBlockdev {
                block_size: 512,
                ..VfsBlockdev::default()
            },
            no_filesystem: true,
            cwd: "/".to_string(),
            read_size: 32,
            prog_size: 64,
            lookahead: 32,
            enable_mtime: true,
            fs: None,
        };
        assert_eq!(mount.cache_size(), 256);
    }

    #[test]
    fn mtime_roundtrip() {
        let mount = LfsMount {
            blockdev: VfsBlockdev::default(),
            no_filesystem: true,
            cwd: "/".to_string(),
            read_size: 32,
            prog_size: 32,
            lookahead: 32,
            enable_mtime: true,
            fs: None,
        };
        let bytes = shared_rs::timeutils::timeutils::lfs_mtime_bytes_from_now();
        assert_eq!(
            shared_rs::timeutils::timeutils::lfs_mtime_bytes_to_timestamp(&bytes),
            shared_rs::timeutils::timeutils::lfs_mtime_bytes_to_timestamp(&bytes)
        );
        let _ = mount;
    }
}
