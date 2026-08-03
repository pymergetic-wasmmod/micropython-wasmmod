//! rewrite of extmod/vfs_posix.c + extmod/vfs_posix.h
// symmetry: done

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::builtinimport::ImportStat;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mperrno;
use py_rs::obj::{
    self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objpolyiter;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use py_rs::vstr::{self, Vstr};

use crate::vfs_posix_file;

pub const MP_S_IFDIR: i32 = 0x4000;
pub const MP_S_IFREG: i32 = 0x8000;

#[repr(C)]
pub struct ObjVfsPosix {
    pub base: ObjBase,
    pub root: *mut Vstr,
    pub root_len: usize,
    pub readonly: bool,
}

#[repr(C)]
pub struct VfsProto {
    pub import_stat: fn(*const ObjVfsPosix, &str) -> ImportStat,
}

fn vfs_ptr(o: Obj) -> *mut ObjVfsPosix {
    obj::as_ptr(o) as *mut ObjVfsPosix
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn root_mut(vfs: &mut ObjVfsPosix) -> &mut Vstr {
    unsafe { &mut *vfs.root }
}

fn vstr_path(v: &mut Vstr) -> String {
    let p = vstr::null_terminated_str(v);
    unsafe {
        CStr::from_ptr(p as *const c_char)
            .to_string_lossy()
            .into_owned()
    }
}

fn get_path_str(vfs: &mut ObjVfsPosix, path: Obj) -> String {
    let path_str = objstr::str_get_str(path);
    let root_len = vfs.root_len;
    if root_len == 0 || !path_str.starts_with('/') {
        return path_str;
    }
    let root = root_mut(vfs);
    root.len = root_len - 1;
    vstr::add_str(root, &path_str);
    vstr_path(root)
}

fn get_path_obj(vfs: &mut ObjVfsPosix, path: Obj) -> Obj {
    let path_str = objstr::str_get_str(path);
    let root_len = vfs.root_len;
    if root_len == 0 || !path_str.starts_with('/') {
        return path;
    }
    let root = root_mut(vfs);
    root.len = root_len - 1;
    vstr::add_str(root, &path_str);
    let slice = unsafe { std::slice::from_raw_parts(root.buf, root.len) };
    objstr::new_str(slice)
}

fn fun1_helper(self_in: Obj, path_in: Obj, f: fn(*const c_char) -> i32) -> Obj {
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    let cpath = CString::new(path).unwrap_or_default();
    if f(cpath.as_ptr()) != 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    obj::CONST_NONE
}

fn import_stat(self_: *const ObjVfsPosix, path: &str) -> ImportStat {
    let vfs = unsafe { &*self_ };
    let path = if vfs.root_len != 0 {
        let root = unsafe { &mut *vfs.root };
        root.len = vfs.root_len;
        vstr::add_str(root, path);
        vstr_path(root)
    } else {
        path.to_string()
    };
    let cpath = CString::new(path).unwrap_or_default();
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::stat(cpath.as_ptr(), &mut st) } == 0 {
        if (st.st_mode & libc::S_IFDIR as u32) != 0 {
            return ImportStat::Dir;
        }
        if (st.st_mode & libc::S_IFREG as u32) != 0 {
            return ImportStat::File;
        }
    }
    ImportStat::NoExist
}

fn make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    let o = malloc::new_obj::<ObjVfsPosix>().expect("VfsPosix");
    unsafe {
        (*o).base.type_ = type_vfs_posix();
        (*o).root = vstr::new(0);
        (*o).readonly = !mpconfig::VFS_POSIX_WRITABLE;
        if n_args == 1 {
            let root = objstr::str_get_str(args[0]);
            if !root.is_empty() && !root.starts_with('/') {
                let mut buf = vec![0u8; mpconfig::ALLOC_PATH_MAX + 1];
                let cwd = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut c_char, buf.len()) };
                if cwd.is_null() {
                    raise::raise(MpRaise::OSError(errno()));
                }
                let cwd = unsafe { CStr::from_ptr(cwd) }.to_string_lossy();
                vstr::add_str(&mut *(*o).root, &cwd);
                vstr::add_byte(&mut *(*o).root, b'/');
            }
            vstr::add_str(&mut *(*o).root, &root);
            vstr::add_byte(&mut *(*o).root, b'/');
        }
        (*o).root_len = (*(*o).root).len;
        obj::from_ptr(o as *const ObjVfsPosix as *const ())
    }
}

fn is_readonly(vfs: &ObjVfsPosix) -> bool {
    !mpconfig::VFS_POSIX_WRITABLE || vfs.readonly
}

fn require_writable(self_in: Obj) {
    if is_readonly(unsafe { &*vfs_ptr(self_in) }) {
        raise::raise(MpRaise::OSError(mperrno::EROFS));
    }
}

fn mount(self_in: Obj, readonly: Obj, mkfs: Obj) -> Obj {
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    if obj::is_true(readonly) {
        self_.readonly = true;
    }
    if obj::is_true(mkfs) {
        raise::raise(MpRaise::OSError(mperrno::EPERM));
    }
    obj::CONST_NONE
}

fn umount(_self_in: Obj) -> Obj {
    obj::CONST_NONE
}

fn open(self_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let mode = objstr::str_get_str(mode_in);
    if is_readonly(self_) && (mode.contains('w') || mode.contains('a') || mode.contains('+')) {
        raise::raise(MpRaise::OSError(mperrno::EROFS));
    }
    let path = if obj::is_small_int(path_in) {
        path_in
    } else {
        get_path_obj(unsafe { &mut *vfs_ptr(self_in) }, path_in)
    };
    vfs_posix_file::open(vfs_posix_file::type_textio(), path, mode_in)
}

fn chdir(self_in: Obj, path_in: Obj) -> Obj {
    fun1_helper(self_in, path_in, |p| unsafe { libc::chdir(p) })
}

fn getcwd(self_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let mut buf = vec![0u8; mpconfig::ALLOC_PATH_MAX + 1];
    let ret = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut c_char, buf.len()) };
    if ret.is_null() {
        raise::raise(MpRaise::OSError(errno()));
    }
    let mut s = unsafe { CStr::from_ptr(ret) }
        .to_string_lossy()
        .into_owned();
    if self_.root_len > 0 {
        s = s[self_.root_len - 1..].to_string();
    }
    objstr::new_str(s.as_bytes())
}

#[repr(C)]
struct IlistdirIter {
    base: ObjBase,
    iternext: py_rs::obj::IterNextFn,
    finaliser: py_rs::obj::IterNextFn,
    is_str: bool,
    dir: *mut libc::DIR,
}

fn ilistdir_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    if self_.dir.is_null() {
        return obj::OBJ_STOP_ITERATION;
    }
    loop {
        let dirent = unsafe { libc::readdir(self_.dir) };
        if dirent.is_null() {
            unsafe {
                libc::closedir(self_.dir);
            }
            self_.dir = core::ptr::null_mut();
            return obj::OBJ_STOP_ITERATION;
        }
        let name = unsafe { CStr::from_ptr((*dirent).d_name.as_ptr()) };
        let fn_str = name.to_string_lossy();
        if fn_str == "." || fn_str == ".." {
            continue;
        }
        let name_obj = if self_.is_str {
            objstr::new_str(fn_str.as_bytes())
        } else {
            objstr::new_bytes(fn_str.as_bytes())
        };
        #[cfg(target_os = "linux")]
        let mode = {
            let dt = unsafe { (*dirent).d_type };
            if dt == libc::DT_DIR as u8 {
                obj::new_small_int(MP_S_IFDIR as isize)
            } else if dt == libc::DT_REG as u8 {
                obj::new_small_int(MP_S_IFREG as isize)
            } else {
                obj::new_small_int(dt as isize)
            }
        };
        #[cfg(not(target_os = "linux"))]
        let mode = obj::new_small_int(0);
        let ino = obj::new_small_int(0);
        return objtuple::new_tuple(3, Some(&[name_obj, mode, ino]));
    }
}

fn ilistdir_it_del(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    if !self_.dir.is_null() {
        unsafe {
            libc::closedir(self_.dir);
        }
        self_.dir = core::ptr::null_mut();
    }
    obj::CONST_NONE
}

fn ilistdir(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let mut path = get_path_str(self_, path_in);
    if path.is_empty() {
        path = ".".into();
    }
    let cpath = CString::new(path).unwrap_or_default();
    let dir = unsafe { libc::opendir(cpath.as_ptr()) };
    if dir.is_null() {
        raise::raise(MpRaise::OSError(errno()));
    }
    let o = malloc::new_obj::<IlistdirIter>().expect("ilistdir iter");
    unsafe {
        (*o).base.type_ = objpolyiter::type_polymorph_iter_with_finaliser();
        (*o).iternext = ilistdir_iternext;
        (*o).finaliser = ilistdir_it_del;
        (*o).is_str = obj::is_str(path_in);
        (*o).dir = dir;
        obj::from_ptr(o as *const IlistdirIter as *const ())
    }
}

fn mkdir(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    let cpath = CString::new(path).unwrap_or_default();
    let ret = unsafe { libc::mkdir(cpath.as_ptr(), 0o777) };
    if ret != 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    obj::CONST_NONE
}

fn remove(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    fun1_helper(self_in, path_in, |p| unsafe { libc::unlink(p) })
}

fn rename(self_in: Obj, old_path_in: Obj, new_path_in: Obj) -> Obj {
    require_writable(self_in);
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let old_path = get_path_str(self_, old_path_in);
    let new_path = get_path_str(self_, new_path_in);
    let old_c = CString::new(old_path).unwrap_or_default();
    let new_c = CString::new(new_path).unwrap_or_default();
    if unsafe { libc::rename(old_c.as_ptr(), new_c.as_ptr()) } != 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    obj::CONST_NONE
}

fn rmdir(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    fun1_helper(self_in, path_in, |p| unsafe { libc::rmdir(p) })
}

fn stat(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    let cpath = CString::new(path).unwrap_or_default();
    let mut sb: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::stat(cpath.as_ptr(), &mut sb) } != 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(sb.st_mode as isize),
            obj::new_small_int(sb.st_ino as isize),
            obj::new_small_int(sb.st_dev as isize),
            obj::new_small_int(sb.st_nlink as isize),
            obj::new_small_int(sb.st_uid as isize),
            obj::new_small_int(sb.st_gid as isize),
            obj::new_small_int(sb.st_size as isize),
            obj::new_small_int(sb.st_atime as isize),
            obj::new_small_int(sb.st_mtime as isize),
            obj::new_small_int(sb.st_ctime as isize),
        ]),
    )
}

fn statvfs(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &mut *vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    let cpath = CString::new(path).unwrap_or_default();
    let mut sb: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(cpath.as_ptr(), &mut sb) } != 0 {
        raise::raise(MpRaise::OSError(errno()));
    }
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(sb.f_bsize as isize),
            obj::new_small_int(sb.f_frsize as isize),
            obj::new_small_int(sb.f_blocks as isize),
            obj::new_small_int(sb.f_bfree as isize),
            obj::new_small_int(sb.f_bavail as isize),
            obj::new_small_int(sb.f_files as isize),
            obj::new_small_int(sb.f_ffree as isize),
            obj::new_small_int(sb.f_favail as isize),
            obj::new_small_int(sb.f_flag as isize),
            obj::new_small_int(sb.f_namemax as isize),
        ]),
    )
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];
static TF1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static TF2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};
static TF3: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F3.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_posix fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("vfs_posix fn2");
    unsafe {
        (*o).base.type_ = &TF2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("vfs_posix fn3");
    unsafe {
        (*o).base.type_ = &TF3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}

static VFS_POSIX_PROTO: VfsProto = VfsProto { import_stat };

static mut VFS_POSIX_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_VFS_POSIX: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { VFS_POSIX_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    TYPE_INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("mount")),
                value: mk3(mount),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("umount")),
                value: mk1(umount),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("open")),
                value: mk3(open),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("chdir")),
                value: mk2(chdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("getcwd")),
                value: mk1(getcwd),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ilistdir")),
                value: mk2(ilistdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("mkdir")),
                value: mk2(mkdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("remove")),
                value: mk2(remove),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rename")),
                value: mk3(rename),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rmdir")),
                value: mk2(rmdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("stat")),
                value: mk2(stat),
            },
        ];
        if mpconfig::PY_OS_STATVFS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("statvfs")),
                value: mk2(statvfs),
            });
        }
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            VFS_POSIX_SLOTS[0] = make_new as MakeNewFn as *const ();
            VFS_POSIX_SLOTS[1] = &VFS_POSIX_PROTO as *const VfsProto as *const ();
            VFS_POSIX_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_VFS_POSIX.name = qstr::from_str("VfsPosix");
        }
    });
}

pub fn type_vfs_posix() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_VFS_POSIX }
}

pub fn make_new_vfs_posix() -> Obj {
    make_new(type_vfs_posix(), 0, 0, &[])
}

pub fn enabled() -> bool {
    mpconfig::VFS_POSIX && mpconfig::PY_VFS
}

pub fn import_stat_for(obj_in: Obj, path: &str) -> ImportStat {
    let ptr = obj::as_ptr(obj_in) as *const ObjVfsPosix;
    import_stat(ptr, path)
}
