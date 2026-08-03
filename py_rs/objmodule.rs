//! rewrite of py/objmodule.c + py/objmodule.h
// symmetry: done

use crate::bc::{ModuleContext, ObjModule};
use crate::malloc;
use crate::map::{self, LookupKind, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::mpstate;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_NONE};
use crate::objdict::{self, ObjDict};
use crate::objstr;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;

/// Optional builtins module globals for `MICROPY_CAN_OVERRIDE_BUILTINS` fixed-dict stores.
static BUILTINS_GLOBALS: std::sync::OnceLock<Obj> = std::sync::OnceLock::new();

/// Registered builtins module globals dict, if [`register_builtins_globals`] ran.
pub fn registered_builtins_globals() -> Option<Obj> {
    BUILTINS_GLOBALS.get().copied()
}

static mut MODULE_SLOTS: [*const (); 2] = [module_print as *const (), module_attr as *const ()];

static mut TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 2,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { MODULE_SLOTS.as_ptr() },
};

static MODULE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_module_type() {
    MODULE_INIT.get_or_init(|| unsafe {
        TYPE.name = qstr::from_str("module");
    });
}

pub fn type_module() -> &'static ObjType {
    init_module_type();
    unsafe { &TYPE }
}

fn module_ctx_ptr(o: Obj) -> *mut ModuleContext {
    obj::as_ptr(o) as *mut ModuleContext
}

/// `mp_obj_module_get_globals`
pub fn module_get_globals(module: Obj) -> *mut ObjDict {
    unsafe { (*module_ctx_ptr(module)).module.globals }
}

/// Register the builtins module globals dict for override stores (optional host hook).
pub fn register_builtins_globals(dict: Obj) {
    let _ = BUILTINS_GLOBALS.set(dict);
}

fn module_attr_try_delegation(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if mpconfig::PY_SYS
        && (mpconfig::PY_SYS_PS1_PS2 || mpconfig::PY_SYS_ATTR_DELEGATION)
        && crate::modsys::is_sys_module(self_in)
    {
        crate::modsys::attr(attr, dest);
    }
}

fn module_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*module_ctx_ptr(self_in) };
    let mut module_name = String::new();
    let name_key = obj::new_qstr(qstr::from_str("__name__"));
    if let Some(elem) = map::lookup(
        unsafe { &mut (*self_.module.globals).map },
        name_key,
        LookupKind::Lookup,
    ) {
        module_name = objstr::str_get_str(elem.value);
    }

    if mpconfig::MODULE_FILE {
        let file_key = obj::new_qstr(qstr::from_str("__file__"));
        if let Some(elem) = map::lookup(
            unsafe { &mut (*self_.module.globals).map },
            file_key,
            LookupKind::Lookup,
        ) {
            let file_name = objstr::str_get_str(elem.value);
            let _ = mpprint::printf(
                print,
                "<module '%s' from '%s'>",
                [
                    mpprint::VaArg::Str(&module_name),
                    mpprint::VaArg::Str(&file_name),
                ]
                .into_iter(),
            );
            return;
        }
    }

    let _ = mpprint::printf(
        print,
        "<module '%s'>",
        std::iter::once(mpprint::VaArg::Str(&module_name)),
    );
}

fn module_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ = unsafe { &*module_ctx_ptr(self_in) };
    if dest[0] == obj::OBJ_NULL {
        let key = obj::new_qstr(attr);
        if let Some(elem) = map::lookup(
            unsafe { &mut (*self_.module.globals).map },
            key,
            LookupKind::Lookup,
        ) {
            dest[0] = elem.value;
        } else if mpconfig::CPYTHON_COMPAT && attr == qstr::from_str("__dict__") {
            dest[0] = obj::from_ptr(self_.module.globals as *const ObjDict as *const ());
        } else if mpconfig::MODULE_GETATTR && attr != qstr::from_str("__getattr__") {
            let getattr_key = obj::new_qstr(qstr::from_str("__getattr__"));
            if let Some(elem) = map::lookup(
                unsafe { &mut (*self_.module.globals).map },
                getattr_key,
                LookupKind::Lookup,
            ) {
                dest[0] = runtime::call_function_1(elem.value, obj::new_qstr(attr));
            } else {
                module_attr_try_delegation(self_in, attr, dest);
            }
        } else {
            module_attr_try_delegation(self_in, attr, dest);
        }
    } else {
        let mut dict = self_.module.globals;
        if unsafe { (*dict).map.is_fixed } {
            if mpconfig::CAN_OVERRIDE_BUILTINS {
                if let Some(builtins) = BUILTINS_GLOBALS.get() {
                    if obj::from_ptr(dict as *const ObjDict as *const ()) == *builtins {
                        let override_dict = mpstate::with_vm(|vm| {
                            if vm.mp_module_builtins_override_dict.is_none() {
                                vm.mp_module_builtins_override_dict = Some(objdict::new_dict(1));
                            }
                            vm.mp_module_builtins_override_dict.unwrap()
                        });
                        dict = objdict::dict_ptr(override_dict);
                    } else {
                        module_attr_try_delegation(self_in, attr, dest);
                        return;
                    }
                } else {
                    module_attr_try_delegation(self_in, attr, dest);
                    return;
                }
            } else {
                module_attr_try_delegation(self_in, attr, dest);
                return;
            }
        }
        if dest[1] == obj::OBJ_NULL {
            objdict::dict_delete(
                obj::from_ptr(dict as *const ObjDict as *const ()),
                obj::new_qstr(attr),
            );
        } else {
            objdict::dict_store(
                obj::from_ptr(dict as *const ObjDict as *const ()),
                obj::new_qstr(attr),
                dest[1],
            );
        }
        dest[0] = obj::OBJ_NULL;
    }
}

/// `mp_obj_new_module`
pub fn new_module(module_name: Qstr) -> Obj {
    let loaded = mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    let key = obj::new_qstr(module_name);
    let loaded_map = unsafe { &mut (*objdict::dict_ptr(loaded)).map };
    if let Some(el) = map::lookup(loaded_map, key, LookupKind::AddIfNotFound) {
        if el.value != obj::OBJ_NULL {
            return el.value;
        }
    } else {
        raise::raise(MpRaise::RuntimeError("loaded modules map full"));
    }

    let ctx = malloc::new_obj::<ModuleContext>().expect("module alloc");
    unsafe {
        (*ctx).module.base.type_ = type_module() as *const ObjType;
        (*ctx).module.globals =
            objdict::dict_ptr(objdict::new_dict(mpconfig::MODULE_DICT_SIZE as usize));
        (*ctx).constants = Default::default();
        objdict::dict_store(
            obj::from_ptr((*ctx).module.globals as *const ObjDict as *const ()),
            obj::new_qstr(qstr::from_str("__name__")),
            obj::new_qstr(module_name),
        );
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    if let Some(el) = map::lookup(loaded_map, key, LookupKind::AddIfNotFound) {
        el.value = module;
    }
    module
}

/// Built-in module registry (empty until modules are registered by the host).
static BUILTIN_MODULES: std::sync::OnceLock<std::sync::Mutex<Vec<(Qstr, Obj)>>> =
    std::sync::OnceLock::new();

/// Register a built-in module object (host hook).
pub fn register_builtin_module(name: Qstr, module: Obj) {
    BUILTIN_MODULES
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push((name, module));
}

fn lookup_builtin_map(name: Qstr, extensible: bool) -> Option<Obj> {
    let _ = extensible;
    BUILTIN_MODULES.get().and_then(|table| {
        table
            .lock()
            .unwrap()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, m)| *m)
    })
}

fn maybe_call_module_init(m: Obj) -> Obj {
    if mpconfig::MODULE_BUILTIN_INIT && obj::is_obj(m) {
        let type_ptr = unsafe { (*(obj::as_ptr(m) as *const obj::ObjBase)).type_ };
        if !type_ptr.is_null() {
            let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
            runtime::load_method_maybe(m, qstr::from_str("__init__"), &mut dest);
            if dest[0] != obj::OBJ_NULL {
                runtime::call_method_n_kw(0, 0, &dest);
            }
        }
    }
    m
}

/// `mp_module_get_builtin`
pub fn module_get_builtin(module_name: Qstr, extensible: bool) -> Obj {
    if let Some(m) = lookup_builtin_map(module_name, extensible) {
        return maybe_call_module_init(m);
    }

    // `usys` always aliases `sys` (C `mp_module_get_builtin`).
    if mpconfig::PY_SYS && module_name == qstr::from_str("usys") {
        if let Some(m) = lookup_builtin_map(qstr::from_str("sys"), false) {
            return maybe_call_module_init(m);
        }
    }

    // `ufoo` forces the extensible built-in `foo` (legacy MicroPython alias).
    if !extensible {
        if let Some(name) = qstr::str_data(module_name) {
            if name.len() > 1 && name[0] == b'u' {
                let rest = qstr::from_strn(&name[1..]);
                if let Some(m) = lookup_builtin_map(rest, true) {
                    return maybe_call_module_init(m);
                }
            }
        }
    }

    obj::OBJ_NULL
}

/// `mp_module_generic_attr`
pub fn module_generic_attr(attr: Qstr, dest: &mut [Obj; 2], keys: &[Qstr], values: &mut [Obj]) {
    for i in 0..keys.len() {
        if keys[i] == 0 {
            break;
        }
        if attr == keys[i] {
            if dest[0] == obj::OBJ_NULL {
                dest[0] = values[i];
            } else {
                values[i] = dest[1];
                dest[0] = obj::OBJ_NULL;
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::mpconfig;
    use crate::mpprint;
    use crate::mpstate;

    fn setup() {
        let _ = gc::init();
        qstr::init();
        mpstate::init();
        mpstate::with_vm(|vm| {
            if vm.mp_loaded_modules_dict == obj::OBJ_NULL {
                vm.mp_loaded_modules_dict =
                    objdict::new_dict(mpconfig::LOADED_MODULES_DICT_SIZE as usize);
            }
        });
        init_module_type();
    }

    fn print_to_string(o: Obj) -> String {
        let mut out = Vec::new();
        let mut print = Print {
            data: &mut out as *mut Vec<u8> as *mut (),
            print_strn: Some(collect_print),
        };
        module_print(&print, o, PrintKind::Repr);
        String::from_utf8(out).unwrap()
    }

    extern "C" fn collect_print(data: *mut (), str: *const u8, len: usize) {
        let out = unsafe { &mut *(data as *mut Vec<u8>) };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(str, len) });
    }

    #[test]
    fn new_module_sets_name_and_dedupes() {
        setup();
        let name = qstr::from_str("mymod");
        let m1 = new_module(name);
        let m2 = new_module(name);
        assert_eq!(m1, m2);
        let globals = module_get_globals(m1);
        let stored = objdict::dict_get(
            obj::from_ptr(globals as *const ObjDict as *const ()),
            obj::new_qstr(qstr::from_str("__name__")),
        );
        assert_eq!(stored, obj::new_qstr(name));
    }

    #[test]
    fn module_attr_load_and_store() {
        setup();
        let m = new_module(qstr::from_str("t"));
        let key = obj::new_qstr(qstr::from_str("x"));
        let val = obj::new_small_int(99);
        let mut dest = [obj::OBJ_SENTINEL, val];
        module_attr(m, qstr::from_str("x"), &mut dest);
        assert_eq!(dest[0], obj::OBJ_NULL);

        dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        module_attr(m, qstr::from_str("x"), &mut dest);
        assert_eq!(dest[0], val);
    }

    #[test]
    fn module_print_shows_name() {
        setup();
        let m = new_module(qstr::from_str("hello"));
        let s = print_to_string(m);
        assert!(s.contains("hello"));
        assert!(s.contains("<module"));
    }

    #[test]
    fn module_generic_attr_roundtrip() {
        setup();
        let mut keys = [qstr::from_str("a"), 0];
        let mut values = [obj::new_small_int(1), obj::OBJ_NULL];
        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        module_generic_attr(qstr::from_str("a"), &mut dest, &keys, &mut values);
        assert_eq!(obj::small_int_value(dest[0]), 1);

        dest = [obj::OBJ_SENTINEL, obj::new_small_int(2)];
        module_generic_attr(qstr::from_str("a"), &mut dest, &keys, &mut values);
        assert_eq!(dest[0], obj::OBJ_NULL);
        assert_eq!(obj::small_int_value(values[0]), 2);
    }

    #[test]
    fn module_get_builtin_missing() {
        setup();
        assert_eq!(
            module_get_builtin(qstr::from_str("nosuch"), false),
            obj::OBJ_NULL
        );
    }

    #[test]
    fn module_get_builtin_registered() {
        setup();
        let name = qstr::from_str("built_in_test_mod");
        let m = new_module(name);
        register_builtin_module(name, m);
        assert_eq!(lookup_builtin_map(name, false), Some(m));
    }
}
