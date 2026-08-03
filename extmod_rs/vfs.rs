//! rewrite of extmod/vfs.c + extmod/vfs.h
// symmetry: done

use py_rs::argcheck::{self, Arg, ArgFlag, ArgVal};
use py_rs::builtin::{self, ImportStat};
use py_rs::builtinimport;
use py_rs::map::Map;
use py_rs::mpconfig;
use py_rs::mperrno;
use py_rs::mpstate::{self, VfsCur, VfsMount};
use py_rs::nlr::{self, NlrBuf};
use py_rs::obj::{self, IterNextFn, Obj, ObjBase};
use py_rs::objlist;
use py_rs::objpolyiter;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use py_rs::runtime;

use crate::vfs_fat;
use crate::vfs_lfs;
use crate::vfs_posix;
use crate::vfs_posix_file;
use crate::vfs_rom;

pub const MP_S_IFDIR: i32 = 0x4000;
pub const MP_S_IFREG: i32 = 0x8000;

const PROXY_MAX_ARGS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LookupResult {
    None,
    Root,
    Mount(usize),
}

pub fn enabled() -> bool {
    mpconfig::PY_VFS
}

fn lookup_path(path: &str) -> (LookupResult, String) {
    let cur = mpstate::with_vm(|vm| vm.vfs_cur);
    if path.starts_with('/') || cur == VfsCur::Root {
        let mut is_abs = false;
        let mut p = path;
        if p.starts_with('/') {
            p = &p[1..];
            is_abs = true;
        }
        if p.is_empty() {
            return (LookupResult::Root, String::new());
        }
        let table = mpstate::with_vm(|vm| vm.vfs_mount_table.clone());
        for (i, vfs) in table.iter().enumerate() {
            let mnt = &vfs.mount_point;
            let len = mnt.len() - 1;
            if len == 0 {
                let out = if is_abs {
                    format!("/{p}")
                } else {
                    p.to_string()
                };
                return (LookupResult::Mount(i), out);
            }
            let prefix = &mnt[1..];
            if p.starts_with(prefix) {
                let rest = &p[len..];
                if rest.starts_with('/') {
                    return (LookupResult::Mount(i), rest.to_string());
                }
                if rest.is_empty() {
                    return (LookupResult::Mount(i), "/".into());
                }
            }
        }
        return (LookupResult::None, String::new());
    }
    match cur {
        VfsCur::Root => (LookupResult::Root, path.to_string()),
        VfsCur::Mount(i) => (LookupResult::Mount(i), path.to_string()),
    }
}

fn lookup_path_obj(path_in: Obj) -> (LookupResult, Obj) {
    let path = objstr::str_get_str(path_in);
    let (res, p_out) = lookup_path(&path);
    let path_out = if res != LookupResult::None && res != LookupResult::Root {
        objstr::new_str_of_type(obj::get_type(path_in), p_out.as_bytes())
    } else {
        obj::OBJ_NULL
    };
    (res, path_out)
}

fn mount_obj(idx: usize) -> Obj {
    mpstate::with_vm(|vm| vm.vfs_mount_table[idx].obj)
}

fn proxy_call(idx: LookupResult, meth: Qstr, args: &[Obj]) -> Obj {
    assert!(args.len() <= PROXY_MAX_ARGS);
    if idx == LookupResult::None {
        raise::raise(MpRaise::OSError(mperrno::ENODEV));
    }
    if idx == LookupResult::Root {
        raise::raise(MpRaise::OSError(mperrno::EPERM));
    }
    let LookupResult::Mount(i) = idx else {
        unreachable!()
    };
    let vfs_obj = mount_obj(i);
    let mut meth_dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method(vfs_obj, meth, &mut meth_dest);
    let mut call_args = Vec::with_capacity(2 + args.len());
    call_args.extend_from_slice(&meth_dest);
    call_args.extend_from_slice(args);
    runtime::call_method_n_kw(args.len(), 0, &call_args)
}

/// `mp_vfs_import_stat`
pub fn import_stat(path: &str) -> ImportStat {
    let (res, path_out) = lookup_path(path);
    if res == LookupResult::None || res == LookupResult::Root {
        return ImportStat::NoExist;
    }
    let LookupResult::Mount(i) = res else {
        return ImportStat::NoExist;
    };
    let vfs_obj = mount_obj(i);
    let type_ = obj::get_type(vfs_obj);
    if let Some(proto) = obj::type_get_protocol(type_) {
        if mpconfig::VFS_POSIX && core::ptr::eq(type_, vfs_posix::type_vfs_posix()) {
            return vfs_posix::import_stat_for(vfs_obj, &path_out);
        }
        if mpconfig::VFS_ROM && core::ptr::eq(type_, vfs_rom::type_vfs_rom()) {
            return vfs_rom::import_stat_for(vfs_obj, &path_out);
        }
        if mpconfig::VFS_FAT && core::ptr::eq(type_, vfs_fat::type_vfs_fat()) {
            return vfs_fat::import_stat_for(vfs_obj, &path_out);
        }
        if mpconfig::VFS_LFS2 && core::ptr::eq(type_, vfs_lfs::type_vfs_lfs2()) {
            return vfs_lfs::import_stat_for(vfs_obj, &path_out);
        }
        let proto = unsafe { &*(proto as *const vfs_posix::VfsProto) };
        let ptr = obj::as_ptr(vfs_obj) as *const vfs_posix::ObjVfsPosix;
        return (proto.import_stat)(ptr, &path_out);
    }
    let path_o = objstr::new_str(path_out.as_bytes());
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || proxy_call(res, qstr::from_str("stat"), &[path_o])) {
        Ok(stat) => {
            let (_, items) = objtuple::tuple_get(stat);
            let st_mode = obj::get_int(items[0]) as i32;
            if st_mode & MP_S_IFDIR != 0 {
                ImportStat::Dir
            } else {
                ImportStat::File
            }
        }
        Err(_) => ImportStat::NoExist,
    }
}

/// `mp_vfs_mount`
pub fn mount(n_pos: usize, pos_args: &[Obj], kw_args: &mut Map) -> Obj {
    if n_pos == 0 {
        let list = objlist::new_list(0, None);
        mpstate::with_vm(|vm| {
            for vfs in &vm.vfs_mount_table {
                let mp = objstr::new_str(vfs.mount_point.as_bytes());
                let tup = objtuple::new_tuple(2, Some(&[vfs.obj, mp]));
                objlist::list_append(list, tup);
            }
        });
        return list;
    }

    let allowed = [
        Arg {
            qst: 0,
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: 0,
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::OBJ_NULL),
        },
        Arg {
            qst: qstr::from_str("readonly"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_FALSE),
        },
        Arg {
            qst: qstr::from_str("mkfs"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_FALSE),
        },
    ];
    let mut vals = [ArgVal::default(); 4];
    argcheck::parse_all(n_pos, pos_args, kw_args, allowed.len(), &allowed, &mut vals);
    let fsobj = vals[0].as_obj();
    let mount_point = vals[1].as_obj();
    let readonly = vals[2].as_obj();
    let mkfs = vals[3].as_obj();

    let mnt_str = objstr::str_get_str(mount_point);
    let mut vfs_obj = fsobj;
    let mut dest = [obj::OBJ_NULL; 2];
    runtime::load_method_maybe(vfs_obj, qstr::from_str("mount"), &mut dest);
    if dest[0] == obj::OBJ_NULL {
        raise::raise(MpRaise::OSError(mperrno::ENODEV));
    }

    let (existing, _) = lookup_path_obj(mount_point);
    if existing != LookupResult::None && existing != LookupResult::Root {
        let new_len = mnt_str.len();
        let existing_len = mpstate::with_vm(|vm| {
            vm.vfs_mount_table[match existing {
                LookupResult::Mount(i) => i,
                _ => 0,
            }]
            .mount_point
            .len()
        });
        if !(new_len != 1 && existing_len == 1) {
            raise::raise(MpRaise::OSError(mperrno::EPERM));
        }
    }

    runtime::call_method_n_kw(2, 0, &[dest[0], dest[1], readonly, mkfs]);

    mpstate::with_vm(|vm| {
        let entry = VfsMount {
            mount_point: mnt_str.to_string(),
            obj: vfs_obj,
        };
        let mut insert_at = vm.vfs_mount_table.len();
        for (i, m) in vm.vfs_mount_table.iter().enumerate() {
            if m.mount_point.len() == 1 {
                insert_at = i;
                break;
            }
        }
        vm.vfs_mount_table.insert(insert_at, entry);
    });
    obj::CONST_NONE
}

/// `mp_vfs_umount`
pub fn umount(mnt_in: Obj) -> Obj {
    let (mnt_str, mnt_len) = if obj::is_str(mnt_in) {
        let s = objstr::str_get_str(mnt_in);
        (Some(s.to_string()), s.len())
    } else {
        (None, 0)
    };
    let mut removed: Option<(usize, Obj)> = None;
    mpstate::with_vm(|vm| {
        let mut i = 0;
        while i < vm.vfs_mount_table.len() {
            let hit = if let Some(ref mnt) = mnt_str {
                vm.vfs_mount_table[i].mount_point.len() == mnt_len
                    && vm.vfs_mount_table[i].mount_point == *mnt
            } else {
                vm.vfs_mount_table[i].obj == mnt_in
            };
            if hit {
                removed = Some((i, vm.vfs_mount_table[i].obj));
                break;
            }
            i += 1;
        }
    });
    let Some((idx, vfs_obj)) = removed else {
        raise::raise(MpRaise::OSError(mperrno::EINVAL));
    };
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method(vfs_obj, qstr::from_str("umount"), &mut dest);
    runtime::call_method_n_kw(0, 0, &dest);
    mpstate::with_vm(|vm| {
        vm.vfs_mount_table.remove(idx);
        if vm.vfs_cur == VfsCur::Mount(idx) {
            vm.vfs_cur = VfsCur::Root;
        } else if let VfsCur::Mount(cur) = vm.vfs_cur {
            if cur > idx {
                vm.vfs_cur = VfsCur::Mount(cur - 1);
            }
        }
    });
    obj::CONST_NONE
}

/// `mp_vfs_open`
pub fn open(n_pos: usize, pos_args: &[Obj], kw_args: &mut Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("file"),
            flags: ArgFlag::Required as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::CONST_NONE),
        },
        Arg {
            qst: qstr::from_str("mode"),
            flags: ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::new_qstr(qstr::from_str("r"))),
        },
    ];
    let mut vals = [ArgVal::default(); 2];
    argcheck::parse_all(n_pos, pos_args, kw_args, allowed.len(), &allowed, &mut vals);
    let file = vals[0].as_obj();
    let mode = vals[1].as_obj();

    if mpconfig::VFS_POSIX && obj::is_small_int(file) {
        return vfs_posix_file::open(vfs_posix_file::type_textio(), file, mode);
    }

    let (res, path_out) = lookup_path_obj(file);
    proxy_call(res, qstr::from_str("open"), &[path_out, mode])
}

pub fn chdir(path_in: Obj) -> Obj {
    let (res, path_out) = lookup_path_obj(path_in);
    if res == LookupResult::Root {
        mpstate::with_vm(|vm| {
            for (i, vfs) in vm.vfs_mount_table.iter().enumerate() {
                if vfs.mount_point.len() == 1 {
                    let root = obj::new_qstr(qstr::from_str("/"));
                    proxy_call(LookupResult::Mount(i), qstr::from_str("chdir"), &[root]);
                    break;
                }
            }
            vm.vfs_cur = VfsCur::Root;
        });
    } else {
        proxy_call(res, qstr::from_str("chdir"), &[path_out]);
        if let LookupResult::Mount(i) = res {
            mpstate::with_vm(|vm| vm.vfs_cur = VfsCur::Mount(i));
        }
    }
    obj::CONST_NONE
}

pub fn getcwd() -> Obj {
    let cur = mpstate::with_vm(|vm| vm.vfs_cur);
    if cur == VfsCur::Root {
        return obj::new_qstr(qstr::from_str("/"));
    }
    let VfsCur::Mount(i) = cur else {
        return obj::new_qstr(qstr::from_str("/"));
    };
    let cwd_o = proxy_call(LookupResult::Mount(i), qstr::from_str("getcwd"), &[]);
    let mnt = mpstate::with_vm(|vm| vm.vfs_mount_table[i].mount_point.clone());
    if mnt.len() == 1 {
        return cwd_o;
    }
    let cwd = objstr::str_get_str(cwd_o);
    if cwd == "/" {
        return objstr::new_str(mnt.as_bytes());
    }
    objstr::new_str(format!("{mnt}{cwd}").as_bytes())
}

#[repr(C)]
struct IlistdirIter {
    base: ObjBase,
    iternext: IterNextFn,
    cur_vfs: Option<usize>,
    sub_iter: Obj,
    is_str: bool,
    is_iter: bool,
}

fn ilistdir_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    if self_.is_iter {
        let next = runtime::iternext(self_.sub_iter);
        if next != obj::OBJ_STOP_ITERATION {
            return next;
        }
        self_.is_iter = false;
    }
    let Some(i) = self_.cur_vfs else {
        return obj::OBJ_STOP_ITERATION;
    };
    let table_len = mpstate::with_vm(|vm| vm.vfs_mount_table.len());
    if i >= table_len {
        self_.cur_vfs = None;
        return obj::OBJ_STOP_ITERATION;
    }
    let (mnt, len) = mpstate::with_vm(|vm| {
        (
            vm.vfs_mount_table[i].mount_point.clone(),
            vm.vfs_mount_table[i].mount_point.len(),
        )
    });
    self_.cur_vfs = Some(i + 1);
    if len == 1 {
        let root = obj::new_qstr(qstr::from_str("/"));
        self_.sub_iter = proxy_call(LookupResult::Mount(i), qstr::from_str("ilistdir"), &[root]);
        self_.is_iter = true;
        return runtime::iternext(self_.sub_iter);
    }
    let name = if self_.is_str {
        objstr::new_str(mnt[1..].as_bytes())
    } else {
        objstr::new_bytes(mnt[1..].as_bytes())
    };
    objtuple::new_tuple(
        3,
        Some(&[
            name,
            obj::new_small_int(MP_S_IFDIR as isize),
            obj::new_small_int(0),
        ]),
    )
}

pub fn ilistdir(n_args: usize, args: &[Obj]) -> Obj {
    let path_in = if n_args == 1 {
        args[0]
    } else {
        obj::new_qstr(qstr::from_str(""))
    };
    let (res, path_out) = lookup_path_obj(path_in);
    if res == LookupResult::Root {
        let o = py_rs::malloc::new_obj::<IlistdirIter>().expect("vfs ilistdir");
        unsafe {
            (*o).base.type_ = objpolyiter::type_polymorph_iter();
            (*o).iternext = ilistdir_iternext;
            (*o).cur_vfs = Some(0);
            (*o).sub_iter = obj::OBJ_NULL;
            (*o).is_str = obj::is_str(path_in);
            (*o).is_iter = false;
            return obj::from_ptr(o as *const IlistdirIter as *const ());
        }
    }
    proxy_call(res, qstr::from_str("ilistdir"), &[path_out])
}

pub fn listdir(n_args: usize, args: &[Obj]) -> Obj {
    let iter = ilistdir(n_args, args);
    let dir_list = objlist::new_list(0, None);
    loop {
        let next = runtime::iternext(iter);
        if next == obj::OBJ_STOP_ITERATION {
            break;
        }
        let (_, items) = objtuple::tuple_get(next);
        objlist::list_append(dir_list, items[0]);
    }
    dir_list
}

pub fn mkdir(path_in: Obj) -> Obj {
    let (res, path_out) = lookup_path_obj(path_in);
    if res == LookupResult::Root
        || (res != LookupResult::None && objstr::str_get_str(path_out) == "/")
    {
        raise::raise(MpRaise::OSError(mperrno::EEXIST));
    }
    proxy_call(res, qstr::from_str("mkdir"), &[path_out])
}

pub fn remove(path_in: Obj) -> Obj {
    let (res, path_out) = lookup_path_obj(path_in);
    proxy_call(res, qstr::from_str("remove"), &[path_out])
}

pub fn rename(old_path_in: Obj, new_path_in: Obj) -> Obj {
    let (old_res, old_out) = lookup_path_obj(old_path_in);
    let (new_res, new_out) = lookup_path_obj(new_path_in);
    if old_res != new_res {
        raise::raise(MpRaise::OSError(mperrno::EPERM));
    }
    proxy_call(old_res, qstr::from_str("rename"), &[old_out, new_out])
}

pub fn rmdir(path_in: Obj) -> Obj {
    let (res, path_out) = lookup_path_obj(path_in);
    proxy_call(res, qstr::from_str("rmdir"), &[path_out])
}

pub fn stat(path_in: Obj) -> Obj {
    let (res, path_out) = lookup_path_obj(path_in);
    if res == LookupResult::Root {
        return objtuple::new_tuple(
            10,
            Some(&[
                obj::new_small_int(MP_S_IFDIR as isize),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
                obj::new_small_int(0),
            ]),
        );
    }
    proxy_call(res, qstr::from_str("stat"), &[path_out])
}

pub fn statvfs(path_in: Obj) -> Obj {
    let (mut res, mut path_out) = lookup_path_obj(path_in);
    if res == LookupResult::Root {
        let root_idx = mpstate::with_vm(|vm| {
            vm.vfs_mount_table
                .iter()
                .position(|v| v.mount_point.len() == 1)
        });
        if root_idx.is_none() {
            return objtuple::new_tuple(
                10,
                Some(&[
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(0),
                    obj::new_small_int(mpconfig::ALLOC_PATH_MAX as isize),
                ]),
            );
        }
        res = LookupResult::Mount(root_idx.unwrap());
        path_out = obj::new_qstr(qstr::from_str("/"));
    }
    proxy_call(res, qstr::from_str("statvfs"), &[path_out])
}

trait ArgValExt {
    fn as_obj(self) -> Obj;
}

impl ArgValExt for ArgVal {
    fn as_obj(self) -> Obj {
        match self {
            ArgVal::Obj(o) => o,
            ArgVal::Bool(b) => obj::new_bool(b),
            ArgVal::Int(i) => obj::new_small_int(i),
        }
    }
}

/// Wire VFS into builtins and mount host POSIX root.
pub fn init_host() {
    if !enabled() {
        return;
    }
    builtin::set_builtin_open(|n, a, kw| {
        let mut map = kw.cloned().unwrap_or_default();
        open(n, a, &mut map)
    });
    builtinimport::set_import_stat_hook(import_stat);

    if mpconfig::VFS_POSIX {
        let vfs_obj = vfs_posix::make_new_vfs_posix();
        let mount_point = obj::new_qstr(qstr::from_str("/"));
        let mut empty = Map::default();
        mount(2, &[vfs_obj, mount_point], &mut empty);
        mpstate::with_vm(|vm| {
            if !vm.vfs_mount_table.is_empty() {
                vm.vfs_cur = VfsCur::Mount(vm.vfs_mount_table.len() - 1);
            }
        });
    }
    crate::vfs_reader::init();
}

pub fn mount_readonly(_readonly: bool) -> Obj {
    obj::CONST_NONE
}
