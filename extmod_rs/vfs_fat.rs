//! rewrite of extmod/vfs_fat.c + extmod/vfs_fat.h
// symmetry: done

use py_rs::argcheck;
use py_rs::builtinimport::ImportStat;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mperrno;
use py_rs::mpconfig;
use py_rs::obj::{self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objpolyiter;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use shared_rs::timeutils::timeutils;

use crate::vfs_blockdev::BLOCKDEV_FLAG_NO_FILESYSTEM;
use crate::vfs_fat_diskio::{self, normalize_vfs_path, FatDirEntry, FatMount};
use crate::vfs_fat_file;

pub const MP_S_IFDIR: i32 = 0x4000;
pub const MP_S_IFREG: i32 = 0x8000;

#[repr(C)]
pub struct ObjVfsFat {
    pub base: ObjBase,
    pub mount: *mut FatMount,
}

#[repr(C)]
pub struct VfsProto {
    pub import_stat: fn(*const ObjVfsFat, &str) -> ImportStat,
}

fn vfs_ptr(o: Obj) -> *mut ObjVfsFat {
    obj::as_ptr(o) as *mut ObjVfsFat
}

fn mount_mut(o: Obj) -> &'static mut FatMount {
    unsafe { &mut *(*vfs_ptr(o)).mount }
}

fn require_writable(o: Obj) {
    let mount = unsafe { &*(*vfs_ptr(o)).mount };
    if mount.blockdev.writeblocks[0] == obj::OBJ_NULL {
        raise::raise(MpRaise::OSError(mperrno::EROFS));
    }
}

fn import_stat(self_: *const ObjVfsFat, path: &str) -> ImportStat {
    let mount = unsafe { &*(*self_).mount };
    match mount.stat_path(path) {
        Ok(st) => {
            if st.is_dir {
                ImportStat::Dir
            } else {
                ImportStat::File
            }
        }
        Err(_) => ImportStat::NoExist,
    }
}

fn make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let o = malloc::new_obj::<ObjVfsFat>().expect("VfsFat");
    let mount = FatMount::create(args[0], false).unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    unsafe {
        (*o).base.type_ = type_vfs_fat();
        (*o).mount = Box::into_raw(mount);
        obj::from_ptr(o as *const ObjVfsFat as *const ())
    }
}

fn mkfs(bdev_in: Obj) -> Obj {
    let _mount = FatMount::create(bdev_in, true).unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn mount(self_in: Obj, readonly: Obj, mkfs_flag: Obj) -> Obj {
    let mount = mount_mut(self_in);
    if obj::is_true(readonly) {
        mount.blockdev.writeblocks[0] = obj::OBJ_NULL;
    }
    if mount.no_filesystem || (mount.blockdev.flags & BLOCKDEV_FLAG_NO_FILESYSTEM != 0) {
        if obj::is_true(mkfs_flag) {
            mount.format_existing().unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
        } else if mount.no_filesystem {
            raise::raise(MpRaise::OSError(mperrno::ENODEV));
        }
    }
    mount.blockdev.flags &= !BLOCKDEV_FLAG_NO_FILESYSTEM;
    obj::CONST_NONE
}

fn umount(_self_in: Obj) -> Obj {
    obj::CONST_NONE
}

fn open(self_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    vfs_fat_file::open(self_in, path_in, mode_in)
}

#[repr(C)]
struct IlistdirIter {
    base: ObjBase,
    iternext: py_rs::obj::IterNextFn,
    is_str: bool,
    entries: *mut Vec<FatDirEntry>,
    index: usize,
}

fn ilistdir_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    let entries = unsafe { &*self_.entries };
    if self_.index >= entries.len() {
        return obj::OBJ_STOP_ITERATION;
    }
    let e = &entries[self_.index];
    self_.index += 1;
    let name_obj = if self_.is_str {
        objstr::new_str(e.name.as_bytes())
    } else {
        objstr::new_bytes(e.name.as_bytes())
    };
    let mode = if e.is_dir {
        MP_S_IFDIR
    } else {
        MP_S_IFREG
    };
    objtuple::new_tuple(
        4,
        Some(&[
            name_obj,
            obj::new_small_int(mode as isize),
            obj::new_small_int(0),
            obj::new_small_int(e.size as isize),
        ]),
    )
}

fn ilistdir(self_in: Obj, n: usize, args: &[Obj]) -> Obj {
    let path_in = if n >= 1 {
        args[0]
    } else {
        objstr::new_str(b"")
    };
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    let entries = mount
        .list_dir(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let o = malloc::new_obj::<IlistdirIter>().expect("VfsFat ilistdir");
    unsafe {
        (*o).base.type_ = objpolyiter::type_polymorph_iter();
        (*o).iternext = ilistdir_iternext;
        (*o).is_str = obj::is_str(path_in);
        (*o).entries = Box::into_raw(Box::new(entries));
        (*o).index = 0;
        obj::from_ptr(o as *const IlistdirIter as *const ())
    }
}

fn mkdir(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .mkdir(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn remove(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .remove_path(&path, false)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn rmdir(self_in: Obj, path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .remove_path(&path, true)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn rename(self_in: Obj, old_path_in: Obj, new_path_in: Obj) -> Obj {
    require_writable(self_in);
    let mount = mount_mut(self_in);
    let old_path = objstr::str_get_str(old_path_in);
    let new_path = objstr::str_get_str(new_path_in);
    mount
        .rename_path(&old_path, &new_path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn chdir(self_in: Obj, path_in: Obj) -> Obj {
    let mount = mount_mut(self_in);
    let path = objstr::str_get_str(path_in);
    mount
        .chdir(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    obj::CONST_NONE
}

fn getcwd(self_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    objstr::new_str(mount.getcwd().as_bytes())
}

fn fat_datetime_to_timestamp(dt: &fatfs::DateTime) -> timeutils::Timestamp {
    timeutils::seconds_since_2000(
        dt.date.year as py_rs::obj::Uint,
        dt.date.month as py_rs::obj::Uint,
        dt.date.day as py_rs::obj::Uint,
        dt.time.hour as py_rs::obj::Uint,
        dt.time.min as py_rs::obj::Uint,
        dt.time.sec as py_rs::obj::Uint,
    )
}

fn stat(self_in: Obj, path_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    let path = objstr::str_get_str(path_in);
    let st = mount
        .stat_path(&path)
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let mode = if st.is_dir { MP_S_IFDIR } else { MP_S_IFREG };
    let ts = fat_datetime_to_timestamp(&st.modified);
    let ts_obj = timeutils::obj_from_timestamp(ts);
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(mode as isize),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(st.size as isize),
            ts_obj,
            ts_obj,
            ts_obj,
        ]),
    )
}

fn statvfs(self_in: Obj, _path_in: Obj) -> Obj {
    let mount = unsafe { &*(*vfs_ptr(self_in)).mount };
    let vals = mount
        .statvfs()
        .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
    let items: Vec<Obj> = vals.iter().map(|v| obj::new_small_int(*v)).collect();
    objtuple::new_tuple(10, Some(&items))
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
type BuiltinFnVarSelf = fn(Obj, usize, &[Obj]) -> Obj;

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
#[repr(C)]
struct ObjFunBuiltinVarSelf {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVarSelf,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];
static mut FVS: [*const (); 1] = [callvs as *const ()];
static TF1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    base: ObjBase { type_: core::ptr::null() },
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
    base: ObjBase { type_: core::ptr::null() },
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
static TFVS: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { FVS.as_ptr() },
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
fn callvs(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVarSelf) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    unsafe { (self_.fun)(a[0], n - 1, &a[1..]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_fat fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("vfs_fat fn2");
    unsafe {
        (*o).base.type_ = &TF2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("vfs_fat fn3");
    unsafe {
        (*o).base.type_ = &TF3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkvs(min: u8, max: u8, f: BuiltinFnVarSelf) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVarSelf>().expect("vfs_fat fnvs");
    unsafe {
        (*o).base.type_ = &TFVS;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVarSelf as *const ())
    }
}

static VFS_FAT_PROTO: VfsProto = VfsProto { import_stat };

static mut VFS_FAT_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_VFS_FAT: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slot_index_protocol: 1,
    slot_index_parent: 0,
    slot_index_locals_dict: 2,
    slots: unsafe { VFS_FAT_SLOTS.as_ptr() },
};

fn init_type() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("mkfs")),
                value: mk1(mkfs),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("open")),
                value: mk3(open),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("mount")),
                value: mk3(mount),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("umount")),
                value: mk1(umount),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ilistdir")),
                value: mkvs(1, 2, ilistdir),
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
                key: obj::new_qstr(qstr::from_str("rmdir")),
                value: mk2(rmdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rename")),
                value: mk3(rename),
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
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            VFS_FAT_SLOTS[0] = make_new as MakeNewFn as *const ();
            VFS_FAT_SLOTS[1] = &VFS_FAT_PROTO as *const VfsProto as *const ();
            VFS_FAT_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_VFS_FAT.name = qstr::from_str("VfsFat");
        }
    });
}

pub fn type_vfs_fat() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_VFS_FAT }
}

pub fn enabled() -> bool {
    vfs_fat_diskio::enabled()
}

pub fn import_stat_for(obj_in: Obj, path: &str) -> ImportStat {
    let ptr = obj::as_ptr(obj_in) as *const ObjVfsFat;
    import_stat(ptr, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_initializes_once() {
        assert!(enabled());
        assert!(!core::ptr::eq(type_vfs_fat(), core::ptr::null()));
    }

    #[test]
    fn normalize_path_strips_leading_slash() {
        assert_eq!(normalize_vfs_path("/foo/bar"), "foo/bar");
        assert_eq!(normalize_vfs_path(""), "");
    }
}
