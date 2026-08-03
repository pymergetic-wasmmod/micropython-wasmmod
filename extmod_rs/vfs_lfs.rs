//! rewrite of extmod/vfs_lfs.c + extmod/vfs_lfs.h (LFS2 only)
//! LFS1 (`MICROPY_VFS_LFS1`) is intentionally skipped on this host rewrite.
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::builtinimport::ImportStat;
use py_rs::malloc;
use py_rs::map::{self, Map, MapElem};
use py_rs::mpconfig;
use py_rs::mperrno;
use py_rs::obj::{self, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::vfs_blockdev::BLOCKDEV_FLAG_NO_FILESYSTEM;
use crate::vfs_lfs_diskio::{self, LfsMount};
use crate::vfs_lfsx;
use crate::vfs_lfsx_file;

const DEFAULT_READ_SIZE: usize = 32;
const DEFAULT_PROG_SIZE: usize = 32;
const DEFAULT_LOOKAHEAD: usize = 32;

struct LfsMakeParams {
    bdev: Obj,
    read_size: usize,
    prog_size: usize,
    lookahead: usize,
    enable_mtime: bool,
}

fn lfs_make_allowed() -> [Arg; 5] {
    [
        Arg {
            qst: qstr::from_str(""),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("readsize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(DEFAULT_READ_SIZE as isize),
        },
        Arg {
            qst: qstr::from_str("progsize"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(DEFAULT_PROG_SIZE as isize),
        },
        Arg {
            qst: qstr::from_str("lookahead"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Int as u16,
            defval: ArgVal::Int(DEFAULT_LOOKAHEAD as isize),
        },
        Arg {
            qst: qstr::from_str("mtime"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Bool as u16,
            defval: ArgVal::Bool(true),
        },
    ]
}

fn vals_to_lfs_make_params(vals: &[ArgVal; 5]) -> LfsMakeParams {
    LfsMakeParams {
        bdev: match vals[0] {
            ArgVal::Obj(o) => o,
            _ => obj::OBJ_NULL,
        },
        read_size: match vals[1] {
            ArgVal::Int(v) => v as usize,
            _ => DEFAULT_READ_SIZE,
        },
        prog_size: match vals[2] {
            ArgVal::Int(v) => v as usize,
            _ => DEFAULT_PROG_SIZE,
        },
        lookahead: match vals[3] {
            ArgVal::Int(v) => v as usize,
            _ => DEFAULT_LOOKAHEAD,
        },
        enable_mtime: match vals[4] {
            ArgVal::Bool(v) => v,
            _ => true,
        },
    }
}

fn parse_lfs_make_kw_array(n_args: usize, n_kw: usize, args: &[Obj]) -> LfsMakeParams {
    let allowed = lfs_make_allowed();
    let mut vals = [ArgVal::default(); 5];
    argcheck::parse_all_kw_array(n_args, n_kw, args, allowed.len(), &allowed, &mut vals);
    vals_to_lfs_make_params(&vals)
}

fn parse_lfs_make(n_pos: usize, pos: &[Obj], kws: &Map) -> LfsMakeParams {
    let allowed = lfs_make_allowed();
    let mut vals = [ArgVal::default(); 5];
    let mut kw_copy = kws.clone();
    argcheck::parse_all(n_pos, pos, &mut kw_copy, allowed.len(), &allowed, &mut vals);
    vals_to_lfs_make_params(&vals)
}

fn mount_from_params(params: LfsMakeParams, mkfs: bool) -> Box<LfsMount> {
    LfsMount::create(
        params.bdev,
        params.read_size,
        params.prog_size,
        params.lookahead,
        params.enable_mtime,
        mkfs,
    )
    .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)))
}

#[repr(C)]
pub struct ObjVfsLfs2 {
    pub base: ObjBase,
    pub mount: *mut LfsMount,
}

#[repr(C)]
pub struct VfsProto {
    pub import_stat: fn(*const ObjVfsLfs2, &str) -> ImportStat,
}

fn vfs_ptr(o: Obj) -> *mut ObjVfsLfs2 {
    obj::as_ptr(o) as *mut ObjVfsLfs2
}

fn mount_mut(o: Obj) -> &'static mut LfsMount {
    unsafe { &mut *(*vfs_ptr(o)).mount }
}

fn import_stat(self_: *const ObjVfsLfs2, path: &str) -> ImportStat {
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
    let params = parse_lfs_make_kw_array(n_args, n_kw, args);
    let o = malloc::new_obj::<ObjVfsLfs2>().expect("VfsLfs2");
    let mount = mount_from_params(params, false);
    unsafe {
        (*o).base.type_ = type_vfs_lfs2();
        (*o).mount = Box::into_raw(mount);
        obj::from_ptr(o as *const ObjVfsLfs2 as *const ())
    }
}

fn mkfs(n: usize, args: &[Obj], kw: &Map) -> Obj {
    let params = parse_lfs_make(n, args, kw);
    let _mount = mount_from_params(params, true);
    obj::CONST_NONE
}

fn mount(self_in: Obj, readonly: Obj, mkfs_flag: Obj) -> Obj {
    let mount = mount_mut(self_in);
    if obj::is_true(readonly) {
        mount.blockdev.writeblocks[0] = obj::OBJ_NULL;
    }
    if mount.no_filesystem || (mount.blockdev.flags & BLOCKDEV_FLAG_NO_FILESYSTEM != 0) {
        if obj::is_true(mkfs_flag) {
            mount
                .format_existing()
                .unwrap_or_else(|e| raise::raise(MpRaise::OSError(e)));
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
    vfs_lfsx_file::open(self_in, path_in, mode_in)
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
type BuiltinFnVarSelf = fn(Obj, usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltinKw {
    base: ObjBase,
    min_args: u8,
    fun: BuiltinFnKw,
}

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
static mut FKW: [*const (); 1] = [call_kw as *const ()];
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
static TFKW: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FKW.as_ptr() },
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

fn call_kw(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinKw) };
    if n < self_.min_args as usize {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let mut kw = Map::default();
    map::init(&mut kw, k);
    for i in 0..k {
        let key = a[n + i * 2];
        let val = a[n + i * 2 + 1];
        if let Some(slot) = map::lookup(&mut kw, key, map::LookupKind::AddIfNotFound) {
            slot.value = val;
        }
    }
    (self_.fun)(n, &a[..n], &kw)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_lfs fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("vfs_lfs fn2");
    unsafe {
        (*o).base.type_ = &TF2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("vfs_lfs fn3");
    unsafe {
        (*o).base.type_ = &TF3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}
fn mkvs(min: u8, max: u8, f: BuiltinFnVarSelf) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVarSelf>().expect("vfs_lfs fnvs");
    unsafe {
        (*o).base.type_ = &TFVS;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVarSelf as *const ())
    }
}

fn mk_kw(min: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinKw>().expect("vfs_lfs fnkw");
    unsafe {
        (*o).base.type_ = &TFKW;
        (*o).min_args = min;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinKw as *const ())
    }
}

static VFS_LFS2_PROTO: VfsProto = VfsProto { import_stat };

static mut VFS_LFS2_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_VFS_LFS2: ObjType = ObjType {
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
    slots: unsafe { VFS_LFS2_SLOTS.as_ptr() },
};

fn init_type() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("mkfs")),
                value: mk_kw(0, mkfs),
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
                value: mkvs(1, 2, vfs_lfsx::ilistdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("mkdir")),
                value: mk2(vfs_lfsx::mkdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("remove")),
                value: mk2(vfs_lfsx::remove),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rmdir")),
                value: mk2(vfs_lfsx::rmdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("rename")),
                value: mk3(vfs_lfsx::rename),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("chdir")),
                value: mk2(vfs_lfsx::chdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("getcwd")),
                value: mk1(vfs_lfsx::getcwd),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("stat")),
                value: mk2(vfs_lfsx::stat),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("statvfs")),
                value: mk2(vfs_lfsx::statvfs),
            },
        ];
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            VFS_LFS2_SLOTS[0] = make_new as MakeNewFn as *const ();
            VFS_LFS2_SLOTS[1] = &VFS_LFS2_PROTO as *const VfsProto as *const ();
            VFS_LFS2_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_VFS_LFS2.name = qstr::from_str("VfsLfs2");
        }
    });
}

pub fn type_vfs_lfs2() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_VFS_LFS2 }
}

pub fn enabled() -> bool {
    vfs_lfs_diskio::enabled()
}

pub fn import_stat_for(obj_in: Obj, path: &str) -> ImportStat {
    let ptr = obj::as_ptr(obj_in) as *const ObjVfsLfs2;
    import_stat(ptr, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_initializes_once() {
        assert!(enabled());
        assert!(!core::ptr::eq(type_vfs_lfs2(), core::ptr::null()));
    }

    #[test]
    fn lfs_make_defaults_match_c() {
        let allowed = lfs_make_allowed();
        let mut vals = [ArgVal::default(); 5];
        let pos = [obj::OBJ_NULL];
        let mut kw = Map::default();
        argcheck::parse_all(1, &pos, &mut kw, allowed.len(), &allowed, &mut vals);
        let params = vals_to_lfs_make_params(&vals);
        assert_eq!(params.read_size, DEFAULT_READ_SIZE);
        assert_eq!(params.prog_size, DEFAULT_PROG_SIZE);
        assert_eq!(params.lookahead, DEFAULT_LOOKAHEAD);
        assert!(params.enable_mtime);
    }

    #[test]
    fn lfs_make_parses_kw_only_geometry() {
        let args = [
            obj::OBJ_NULL,
            obj::new_qstr(qstr::from_str("readsize")),
            obj::new_small_int(64),
            obj::new_qstr(qstr::from_str("progsize")),
            obj::new_small_int(128),
            obj::new_qstr(qstr::from_str("lookahead")),
            obj::new_small_int(256),
            obj::new_qstr(qstr::from_str("mtime")),
            obj::CONST_FALSE,
        ];
        let params = parse_lfs_make_kw_array(1, 4, &args);
        assert_eq!(params.read_size, 64);
        assert_eq!(params.prog_size, 128);
        assert_eq!(params.lookahead, 256);
        assert!(!params.enable_mtime);
    }
}
