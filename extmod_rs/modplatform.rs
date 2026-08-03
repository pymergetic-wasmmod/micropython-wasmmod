//! rewrite of extmod/modplatform.c + extmod/modplatform.h
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;

type BuiltinFnKw = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnKw,
}

static mut FV: [*const (); 1] = [callv as *const ()];
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

fn mkv(min: u8, max: u8, f: BuiltinFnKw) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("platform fn");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

const fn platform_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "riscv64") {
        "riscv64"
    } else if cfg!(target_arch = "riscv32") {
        "riscv"
    } else {
        ""
    }
}

const fn platform_libc_lib() -> &'static str {
    if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(unix) {
        "glibc"
    } else {
        ""
    }
}

const fn platform_version() -> &'static str {
    ""
}

fn platform_libc_ver() -> &'static str {
    if cfg!(target_env = "musl") {
        "1.2"
    } else if cfg!(all(unix, not(target_env = "musl"))) {
        unsafe {
            let v = libc::gnu_get_libc_version();
            if v.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr(v).to_str().unwrap_or("")
            }
        }
    } else {
        ""
    }
}

fn platform_info() -> String {
    format!(
        "{}-{}-{}-{}-with-{}{}",
        mpconfig::PY_SYS_PLATFORM,
        mpconfig::VERSION_STRING,
        platform_arch(),
        platform_version(),
        platform_libc_lib(),
        platform_libc_ver()
    )
}

fn platform_platform(_n: usize, _args: &[Obj]) -> Obj {
    objstr::new_str(platform_info().as_bytes())
}

fn platform_python_compiler(_n: usize, _args: &[Obj]) -> Obj {
    objstr::new_str(mpconfig::PLATFORM_COMPILER.as_bytes())
}

fn platform_libc_ver_fn(_n: usize, _args: &[Obj]) -> Obj {
    let lib = objstr::new_str(platform_libc_lib().as_bytes());
    let ver = objstr::new_str(platform_libc_ver().as_bytes());
    objtuple::new_tuple(2, Some(&[lib, ver]))
}

/// Register built-in `platform` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_PLATFORM {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("platform")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("platform")),
            value: mkv(0, 0, platform_platform),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("python_compiler")),
            value: mkv(0, 0, platform_python_compiler),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("libc_ver")),
            value: mkv(0, 0, platform_libc_ver_fn),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("platform module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("platform"), module);
    module
}
