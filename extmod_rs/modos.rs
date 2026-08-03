//! rewrite of extmod/modos.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::vfs;
use crate::vfs_fat;
use crate::vfs_lfs;
use crate::vfs_posix;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T0: ObjType = ObjType {
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
    slots: unsafe { F0.as_ptr() },
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
static T2: ObjType = ObjType {
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
    slots: unsafe { F2.as_ptr() },
};
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

fn call0(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}
fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("os fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("os fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("os fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("os fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn os_getenv(n: usize, args: &[Obj]) -> Obj {
    let key = objstr::str_get_str(args[0]);
    match std::env::var(key) {
        Ok(v) => objstr::new_str(v.as_bytes()),
        Err(_) => {
            if n > 1 {
                args[1]
            } else {
                obj::CONST_NONE
            }
        }
    }
}

fn os_putenv(key: Obj, value: Obj) -> Obj {
    let k = objstr::str_get_str(key);
    let v = objstr::str_get_str(value);
    std::env::set_var(k, v);
    obj::CONST_NONE
}

fn os_unsetenv(key: Obj) -> Obj {
    let k = objstr::str_get_str(key);
    std::env::remove_var(k);
    obj::CONST_NONE
}

fn os_system(cmd: Obj) -> Obj {
    let c = objstr::str_get_str(cmd);
    let status = std::process::Command::new("sh").arg("-c").arg(c).status();
    match status {
        Ok(s) => {
            if let Some(code) = s.code() {
                obj::new_small_int(code as isize)
            } else {
                obj::new_small_int(-1)
            }
        }
        Err(e) => raise::raise(MpRaise::OSError(e.raw_os_error().unwrap_or(0))),
    }
}

fn os_errno(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        return obj::new_small_int(errno() as isize);
    }
    let v = obj::get_int(args[0]) as i32;
    unsafe {
        *libc::__errno_location() = v;
    }
    obj::CONST_NONE
}

fn os_urandom(n_in: Obj) -> Obj {
    let n = obj::get_int(n_in);
    if n < 0 {
        raise::raise(MpRaise::ValueError(""));
    }
    let n = n as usize;
    let mut buf = vec![0u8; n];
    if n > 0 {
        use std::io::Read;
        let mut f = std::fs::File::open("/dev/urandom").unwrap_or_else(|e| {
            raise::raise(MpRaise::OSError(e.raw_os_error().unwrap_or(0)));
        });
        f.read_exact(&mut buf).unwrap_or_else(|e| {
            raise::raise(MpRaise::OSError(e.raw_os_error().unwrap_or(0)));
        });
    }
    objstr::new_bytes(&buf)
}

/// Register built-in `os` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_OS {
        return obj::OBJ_NULL;
    }
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("os")),
    }];
    if mpconfig::PY_OS_GETENV_PUTENV_UNSETENV {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("getenv")),
            value: mkv(1, 2, os_getenv),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("putenv")),
            value: mk2(os_putenv),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("unsetenv")),
            value: mk1(os_unsetenv),
        });
    }
    if mpconfig::PY_OS_SYSTEM {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("system")),
            value: mk1(os_system),
        });
    }
    if mpconfig::PY_OS_ERRNO {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("errno")),
            value: mkv(0, 1, os_errno),
        });
    }
    if mpconfig::PY_OS_URANDOM {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("urandom")),
            value: mk1(os_urandom),
        });
    }
    if mpconfig::PY_VFS {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("sep")),
            value: obj::new_qstr(qstr::from_str("/")),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("chdir")),
            value: mk1(vfs::chdir),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("getcwd")),
            value: mk0(vfs::getcwd),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("listdir")),
            value: mkv(0, 1, |n, a| vfs::listdir(n, a)),
        });
        if mpconfig::VFS_WRITABLE {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("mkdir")),
                value: mk1(vfs::mkdir),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("remove")),
                value: mk1(vfs::remove),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("rename")),
                value: mk2(vfs::rename),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("rmdir")),
                value: mk1(vfs::rmdir),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("unlink")),
                value: mk1(vfs::remove),
            });
        }
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("stat")),
            value: mk1(vfs::stat),
        });
        if mpconfig::PY_OS_STATVFS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("statvfs")),
                value: mk1(vfs::statvfs),
            });
        }
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("ilistdir")),
            value: mkv(0, 1, |n, a| vfs::ilistdir(n, a)),
        });
        if !mpconfig::PREVIEW_VERSION_2 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("mount")),
                value: mkv(0, 2, |n, a| {
                    let mut kw = py_rs::map::Map::default();
                    vfs::mount(n, a, &mut kw)
                }),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("umount")),
                value: mk1(vfs::umount),
            });
            if mpconfig::VFS_POSIX {
                table.push(MapElem {
                    key: obj::new_qstr(qstr::from_str("VfsPosix")),
                    value: obj::from_ptr(vfs_posix::type_vfs_posix() as *const ObjType as *const ()),
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
        }
    }
    if mpconfig::PY_OS_DUPTERM > 0 {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("dupterm")),
            value: crate::os_dupterm::dupterm_obj(),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("os module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("os"), module);
    module
}
