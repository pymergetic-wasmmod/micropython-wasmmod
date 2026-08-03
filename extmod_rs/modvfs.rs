//! rewrite of extmod/modvfs.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, Map, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::qstr;

use crate::vfs;
use crate::vfs_fat;
use crate::vfs_lfs;
use crate::vfs_posix;
use crate::vfs_rom;

type BuiltinFnVar = fn(usize, &[Obj], &mut Map) -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}
#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}

static mut FV: [*const (); 1] = [callv as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slots: unsafe { FV.as_ptr() },
};
static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
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
    slots: unsafe { F1.as_ptr() },
};

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, true);
    let mut kw = Map::default();
    (self_.fun)(n, a, &mut kw)
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("vfs fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn vfs_mount(n: usize, args: &[Obj], kw: &mut Map) -> Obj {
    vfs::mount(n, args, kw)
}

fn vfs_umount(mnt: Obj) -> Obj {
    vfs::umount(mnt)
}

/// Register built-in `vfs` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_VFS {
        return obj::OBJ_NULL;
    }
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("vfs")),
    }];
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("mount")),
        value: mkv(0, 2, vfs_mount),
    });
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("umount")),
        value: mk1(vfs_umount),
    });
    if mpconfig::VFS_POSIX {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("VfsPosix")),
            value: obj::from_ptr(vfs_posix::type_vfs_posix() as *const ObjType as *const ()),
        });
    }
    if mpconfig::VFS_ROM {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("VfsRom")),
            value: obj::from_ptr(vfs_rom::type_vfs_rom() as *const ObjType as *const ()),
        });
    }
    if mpconfig::VFS_FAT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("VfsFat")),
            value: obj::from_ptr(vfs_fat::type_vfs_fat() as *const ObjType as *const ()),
        });
    }
    if mpconfig::VFS_LFS2 {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("VfsLfs2")),
            value: obj::from_ptr(vfs_lfs::type_vfs_lfs2() as *const ObjType as *const ()),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("vfs module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("vfs"), module);
    module
}
