//! rewrite of py/modsys.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::mpstate;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objexcept;
use crate::objlist;
use crate::objmodule;
use crate::objstr;
use crate::objtuple;
use crate::qstr;
use crate::raise::{self, MpRaise};

const PATHLIST_SEP: char = ':';

static mut SYS_EXECUTABLE: Obj = obj::OBJ_NULL;

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
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
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
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

fn call0(s: Obj, n: usize, k: usize, _a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 0, 0, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin0) };
    (self_.fun)()
}
fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    crate::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("sys fun0");
    unsafe {
        (*o).base.type_ = &T0 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("sys fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("sys funv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn sys_exit(n: usize, args: &[Obj]) -> Obj {
    if n == 0 {
        raise::raise_obj(objexcept::new_exception(objexcept::type_system_exit()));
    }
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_system_exit(),
        1,
        &[args[0]],
    ));
}

fn sys_print_exception(n: usize, args: &[Obj]) -> Obj {
    let _ = n;
    obj::print_exception(&crate::mpprint::PLAT_PRINT, args[0]);
    obj::CONST_NONE
}

fn sys_exc_info() -> Obj {
    let cur = mpstate::pending_exception();
    let items = if cur == obj::OBJ_NULL {
        vec![obj::CONST_NONE, obj::CONST_NONE, obj::CONST_NONE]
    } else {
        vec![
            obj::from_ptr(obj::get_type(cur) as *const ObjType as *const ()),
            cur,
            obj::CONST_NONE,
        ]
    };
    objtuple::new_tuple(3, Some(&items))
}

fn sys_atexit(obj: Obj) -> Obj {
    mpstate::with_vm(|vm| {
        let old = vm.sys_exitfunc;
        vm.sys_exitfunc = obj;
        old
    })
}

static mut SYS_MODULE: Obj = obj::OBJ_NULL;

/// True when `o` is the built-in `sys` module object.
pub fn is_sys_module(o: Obj) -> bool {
    unsafe { SYS_MODULE != obj::OBJ_NULL && o == SYS_MODULE }
}

/// `mp_module_sys_attr` — mutable `sys.ps1` / `sys.ps2` (and friends).
pub fn attr(attr: qstr::Qstr, dest: &mut [Obj; 2]) {
    if !mpconfig::PY_SYS_PS1_PS2 {
        return;
    }
    let keys = [
        qstr::from_str("ps1"),
        qstr::from_str("ps2"),
        0, // sentinel
    ];
    mpstate::with_vm(|vm| {
        let mut values = [vm.sys_ps1, vm.sys_ps2];
        objmodule::module_generic_attr(attr, dest, &keys, &mut values);
        vm.sys_ps1 = values[0];
        vm.sys_ps2 = values[1];
    });
}

/// Run `sys.atexit` callback if set (unix `main.c` teardown).
pub fn run_atexit() {
    if !mpconfig::PY_SYS_ATEXIT {
        return;
    }
    let f = mpstate::with_vm(|vm| vm.sys_exitfunc);
    if f != obj::CONST_NONE && f != obj::OBJ_NULL && obj::is_callable(f) {
        let _ = crate::runtime::call_function_0(f);
    }
}

fn append_sys_path_entry(path: Obj, entry: &str) {
    if entry.is_empty() {
        objlist::list_append(path, obj::new_qstr(qstr::from_str("")));
        return;
    }
    if entry.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            objlist::list_append(
                path,
                objstr::new_str(format!("{home}{}", &entry[1..]).as_bytes()),
            );
            return;
        }
    }
    objlist::list_append(path, objstr::new_str(entry.as_bytes()));
}

fn init_default_sys_path(path: Obj) {
    if !mpconfig::PY_SYS_PATH {
        return;
    }
    objlist::list_append(path, obj::new_qstr(qstr::from_str("")));
    let micropypath =
        std::env::var("MICROPYPATH").unwrap_or_else(|_| mpconfig::PY_SYS_PATH_DEFAULT.to_string());
    let mut rest = micropypath.as_str();
    if rest.starts_with(PATHLIST_SEP) {
        rest = &rest[1..];
    }
    while !rest.is_empty() {
        let (entry, tail) = match rest.find(PATHLIST_SEP) {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        append_sys_path_entry(path, entry);
        rest = tail;
    }
}

/// Host string entries from the live `sys.path` list.
pub fn sys_path_entries() -> Vec<String> {
    if !mpconfig::PY_SYS_PATH {
        return Vec::new();
    }
    mpstate::with_vm(|vm| {
        let path = vm.mp_sys_path;
        if path == obj::OBJ_NULL {
            return Vec::new();
        }
        let (_, items) = objlist::list_get(path);
        items
            .iter()
            .map(|item| {
                let (data, len) = objstr::str_get_data(*item);
                String::from_utf8_lossy(&data[..len]).into_owned()
            })
            .collect()
    })
}

/// Locate `module` or `module.__main__` on `sys.path` using the host filesystem.
pub fn locate_module_path(module: &str) -> Option<std::path::PathBuf> {
    let rel = module.replace('.', std::path::MAIN_SEPARATOR_STR);
    for entry in sys_path_entries() {
        let base = if entry.is_empty() {
            match std::env::current_dir() {
                Ok(cwd) => cwd,
                Err(_) => continue,
            }
        } else {
            std::path::PathBuf::from(entry)
        };
        let py = base.join(format!("{rel}.py"));
        if py.is_file() {
            return Some(py);
        }
        let pkg_main = base.join(&rel).join("__main__.py");
        if pkg_main.is_file() {
            return Some(pkg_main);
        }
    }
    None
}

/// Fill `sys.stdin` / `sys.stdout` / `sys.stderr` (placeholders in the fixed
/// sys dict; values may be mutated via `Lookup`).
pub fn set_sys_stdio(stdin: Obj, stdout: Obj, stderr: Obj) {
    if !(mpconfig::PY_SYS && mpconfig::PY_SYS_STDFILES) {
        return;
    }
    let sys = objmodule::module_get_builtin(qstr::from_str("sys"), false);
    if sys == obj::OBJ_NULL {
        return;
    }
    let globals = objmodule::module_get_globals(sys);
    unsafe {
        let map = &mut (*globals).map;
        for (name, val) in [
            ("stdin", stdin),
            ("stdout", stdout),
            ("stderr", stderr),
        ] {
            if let Some(elem) =
                map::lookup(map, obj::new_qstr(qstr::from_str(name)), map::LookupKind::Lookup)
            {
                elem.value = val;
            }
        }
    }
}

/// Update `sys.executable` (`MICROPY_PY_SYS_EXECUTABLE`).
pub fn set_sys_executable(path: &str) {
    if !mpconfig::PY_SYS_EXECUTABLE {
        return;
    }
    let bytes = path.as_bytes().to_vec().into_boxed_slice();
    let leaked = Box::leak(bytes);
    unsafe {
        if SYS_EXECUTABLE != obj::OBJ_NULL && obj::is_exact_type(SYS_EXECUTABLE, objstr::type_str())
        {
            let str = &mut *(obj::as_ptr(SYS_EXECUTABLE) as *mut objstr::ObjStr);
            objstr::str_set_data(str, leaked.as_ptr(), leaked.len());
        }
    }
}

/// Set or replace `sys.argv[0]`.
pub fn set_sys_argv0(arg: &str) {
    if !mpconfig::PY_SYS_ARGV {
        return;
    }
    mpstate::with_vm(|vm| {
        let argv = vm.mp_sys_argv;
        if argv == obj::OBJ_NULL {
            return;
        }
        let index = obj::new_small_int(0);
        let value = objstr::new_str(arg.as_bytes());
        if objlist::list_get(argv).1.is_empty() {
            objlist::list_append(argv, value);
        } else {
            objlist::list_store(argv, index, value);
        }
    });
}

/// Replace `sys.argv` with the given host argument strings.
pub fn set_sys_argv(args: &[&str]) {
    if !mpconfig::PY_SYS_ARGV {
        return;
    }
    mpstate::with_vm(|vm| {
        let argv = vm.mp_sys_argv;
        if argv == obj::OBJ_NULL {
            return;
        }
        objlist::list_set_len(argv, 0);
        for arg in args {
            objlist::list_append(argv, objstr::new_str(arg.as_bytes()));
        }
    });
}

/// Set `sys.path[0]` to the directory containing `script_path` (unix `main.c` behaviour).
pub fn set_script_sys_path(script_path: &str) {
    if !(mpconfig::PY_SYS && mpconfig::PY_SYS_PATH) {
        return;
    }
    let path = mpstate::with_vm(|vm| vm.mp_sys_path);
    if path == obj::OBJ_NULL {
        return;
    }
    let dir = std::path::Path::new(script_path)
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_string_lossy().into_owned()));
    if let Some(dir) = dir {
        objlist::list_store(path, obj::new_small_int(0), objstr::new_str(dir.as_bytes()));
    }
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_SYS {
        return obj::OBJ_NULL;
    }
    let argv = if mpconfig::PY_SYS_ARGV {
        objlist::new_list(0, None)
    } else {
        obj::OBJ_NULL
    };
    let path = if mpconfig::PY_SYS_PATH {
        let p = objlist::new_list(0, None);
        init_default_sys_path(p);
        p
    } else {
        obj::OBJ_NULL
    };
    mpstate::with_vm(|vm| {
        vm.mp_sys_argv = argv;
        vm.mp_sys_path = path;
    });
    let version = objstr::new_str(b"3.4.0; metalpython");
    let version_info = objtuple::new_tuple(
        3,
        Some(&[
            obj::new_small_int(3),
            obj::new_small_int(4),
            obj::new_small_int(0),
        ]),
    );
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("sys")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("version")),
            value: version,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("version_info")),
            value: version_info,
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("byteorder")),
            value: obj::new_qstr(qstr::from_str(if mpconfig::ENDIANNESS_LITTLE {
                "little"
            } else {
                "big"
            })),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("print_exception")),
            value: mkv(1, 2, sys_print_exception),
        },
    ];
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("platform")),
        value: objstr::new_str(mpconfig::PY_SYS_PLATFORM.as_bytes()),
    });
    // `sys.implementation` — C `mp_sys_implementation_obj` (attrtuple).
    if mpconfig::PY_ATTRTUPLE {
        let impl_ver = objtuple::new_tuple(
            4,
            Some(&[
                obj::new_small_int(mpconfig::VERSION_MAJOR as isize),
                obj::new_small_int(mpconfig::VERSION_MINOR as isize),
                obj::new_small_int(mpconfig::VERSION_MICRO as isize),
                if mpconfig::VERSION_PRERELEASE {
                    obj::new_qstr(qstr::from_str("preview"))
                } else {
                    obj::new_qstr(qstr::from_str(""))
                },
            ]),
        );
        let machine = format!("{} [host] version", mpconfig::PY_SYS_PLATFORM);
        let mut fields = vec![
            qstr::from_str("name"),
            qstr::from_str("version"),
            qstr::from_str("_machine"),
        ];
        let mut items = vec![
            obj::new_qstr(qstr::from_str(mpconfig::IMPLEMENTATION_NAME)),
            impl_ver,
            objstr::new_str(machine.as_bytes()),
        ];
        if mpconfig::PERSISTENT_CODE_LOAD {
            // `MPY_FILE_HEADER_INT` — see `py/persistentcode.h`.
            let feat = crate::persistentcode::mpy_file_feature_byte();
            let mpy = (crate::persistentcode::MPY_VERSION as isize) | ((feat as isize) << 8);
            fields.push(qstr::from_str("_mpy"));
            items.push(obj::new_small_int(mpy));
        }
        if let Some(build) = mpconfig::BOARD_BUILD_NAME {
            fields.push(qstr::from_str("_build"));
            items.push(objstr::new_str(build.as_bytes()));
        }
        if mpconfig::PY_THREAD {
            fields.push(qstr::from_str("_thread"));
            items.push(obj::new_qstr(qstr::from_str(if mpconfig::PY_THREAD_GIL {
                "GIL"
            } else {
                "unsafe"
            })));
        }
        if mpconfig::PREVIEW_VERSION_2 {
            fields.push(qstr::from_str("_v2"));
            items.push(obj::CONST_TRUE);
        }
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("implementation")),
            value: crate::objattrtuple::new_attrtuple(&fields, fields.len(), &items),
        });
    }
    if mpconfig::PY_SYS_MAXSIZE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("maxsize")),
            // Must be a full int: `isize::MAX` does not fit in a small-int.
            value: crate::objint::new_int_from_ll(isize::MAX as i64),
        });
    }
    if mpconfig::PY_SYS_EXIT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("exit")),
            value: mkv(0, 1, sys_exit),
        });
    }
    if mpconfig::PY_SYS_EXC_INFO {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("exc_info")),
            value: mk0(sys_exc_info),
        });
    }
    if mpconfig::PY_SYS_MODULES {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("modules")),
            value: mpstate::with_vm(|vm| vm.mp_loaded_modules_dict),
        });
    }
    if mpconfig::PY_SYS_ARGV {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("argv")),
            value: argv,
        });
    }
    if mpconfig::PY_SYS_PATH {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("path")),
            value: path,
        });
    }
    // Placeholders filled by `extmod` / port after VFS stdio is ready (C links
    // `mp_sys_std{in,out,err}_obj` into the ROM sys dict).
    if mpconfig::PY_SYS_STDFILES {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("stdin")),
            value: obj::CONST_NONE,
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("stdout")),
            value: obj::CONST_NONE,
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("stderr")),
            value: obj::CONST_NONE,
        });
    }
    let executable = if mpconfig::PY_SYS_EXECUTABLE {
        let o = objstr::new_str_copy(objstr::type_str(), Some(b""), 0);
        unsafe {
            SYS_EXECUTABLE = o;
        }
        o
    } else {
        obj::OBJ_NULL
    };
    if mpconfig::PY_SYS_EXECUTABLE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("executable")),
            value: executable,
        });
    }
    if mpconfig::PY_SYS_ATEXIT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("atexit")),
            value: mk1(sys_atexit),
        });
    }
    if mpconfig::PY_SYS_PS1_PS2 {
        // Intern names so `dir(sys)` probing finds them.
        let _ = qstr::from_str("ps1");
        let _ = qstr::from_str("ps2");
        mpstate::with_vm(|vm| {
            vm.sys_ps1 = obj::new_qstr(qstr::from_str(">>> "));
            vm.sys_ps2 = obj::new_qstr(qstr::from_str("... "));
            vm.sys_exitfunc = obj::CONST_NONE;
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("sys module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    unsafe {
        SYS_MODULE = module;
    }
    objmodule::register_builtin_module(qstr::from_str("sys"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;

    #[test]
    fn locate_module_in_cwd() {
        runtime::init();
        init_module();
        let dir = std::env::temp_dir().join("mp_loc_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.py"), b"pass\n").unwrap();
        std::env::set_current_dir(&dir).unwrap();
        assert!(locate_module_path("hello").is_some());
    }
}
