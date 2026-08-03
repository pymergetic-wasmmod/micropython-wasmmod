//! rewrite of py/builtinimport.c
// symmetry: done

use std::sync::OnceLock;

use crate::bc::ModuleContext;
use crate::compile;
use crate::emitglue::{self, CompiledModule, ProtoFun};
use crate::lexer::Lexer;
use crate::map::{self, LookupKind};
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objmodule;
use crate::objstr;
use crate::parse::{self, ParseInputKind};
use crate::persistentcode;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::vstr::{self, Vstr};

const PATH_SEP_CHAR: u8 = b'/';
const FROZEN_PATH_PREFIX: &str = ".frozen/";

/// Import path stat result (`mp_import_stat_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImportStat {
    NoExist = 0,
    Dir = 1,
    File = 2,
}

type ImportStatHook = fn(&str) -> ImportStat;

static IMPORT_STAT_HOOK: OnceLock<ImportStatHook> = OnceLock::new();

/// Port hook: register a custom `mp_import_stat` implementation.
pub fn set_import_stat_hook(hook: ImportStatHook) {
    let _ = IMPORT_STAT_HOOK.set(hook);
}

/// Default port hook — no filesystem unless a host hook is registered.
pub fn import_stat(path: &str) -> ImportStat {
    if let Some(hook) = IMPORT_STAT_HOOK.get() {
        return hook(path);
    }
    let _ = path;
    ImportStat::NoExist
}

fn raise_import_error(name: Qstr) -> ! {
    if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_NORMAL {
        raise::raise_obj(objexcept::new_exception(objexcept::type_import_error()));
    }
    let msg = objstr::new_str(format!("no module named '{}'", qstr::str_from_qstr(name).unwrap_or_default()).as_bytes());
    raise::raise_obj(objexcept::new_exception_args(objexcept::type_import_error(), 1, &[msg]));
}

fn raise_import_error_msg(msg: &'static str) -> ! {
    raise::raise_obj(objexcept::new_exception_args(objexcept::type_import_error(), 1, &[objstr::new_str(msg.as_bytes())]));
}

fn vstr_cstr(path: &mut Vstr) -> String {
    let ptr = vstr::null_terminated_str(path);
    unsafe {
        std::ffi::CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

fn stat_path(path: &mut Vstr) -> ImportStat {
    let s = vstr_cstr(path);
    if mpconfig::MODULE_FROZEN && s.starts_with(FROZEN_PATH_PREFIX) {
        // Frozen modules not wired in this port yet.
        return ImportStat::NoExist;
    }
    import_stat(&s)
}

fn stat_file_py_or_mpy(path: &mut Vstr) -> ImportStat {
    let stat = stat_path(path);
    if stat == ImportStat::File {
        return stat;
    }
    if mpconfig::PERSISTENT_CODE_LOAD {
        vstr::ins_byte(path, path.len - 2, b'm');
        let stat = stat_path(path);
        if stat == ImportStat::File {
            return stat;
        }
    }
    ImportStat::NoExist
}

fn stat_module(path: &mut Vstr) -> ImportStat {
    let stat = stat_path(path);
    if stat == ImportStat::Dir {
        return stat;
    }
    vstr::add_str(path, ".py");
    stat_file_py_or_mpy(path)
}

fn default_sys_path_items() -> Vec<Obj> {
    mpconfig::PY_SYS_PATH_DEFAULT
        .split(':')
        .map(|entry| objstr::new_str(entry.as_bytes()))
        .collect()
}

fn sys_path_items() -> Vec<Obj> {
    if !(mpconfig::PY_SYS && mpconfig::PY_SYS_PATH) {
        return Vec::new();
    }
    let sys_name = obj::new_qstr(qstr::from_str("sys"));
    let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    let sys_mod = objdict::dict_get(loaded, sys_name);
    if sys_mod != obj::OBJ_NULL {
        let path_key = obj::new_qstr(qstr::from_str("path"));
        let path_obj = objdict::dict_get(
            obj::from_ptr(objmodule::module_get_globals(sys_mod) as *const ObjDict as *const ()),
            path_key,
        );
        if path_obj != obj::OBJ_NULL && obj::is_exact_type(path_obj, crate::objlist::type_list()) {
            let list = unsafe { &*(obj::as_ptr(path_obj) as *const crate::objlist::ObjList) };
            return unsafe { std::slice::from_raw_parts(list.items, list.len) }.to_vec();
        }
    }
    default_sys_path_items()
}

fn stat_top_level(mod_name: Qstr, dest: &mut Vstr) -> ImportStat {
    if mpconfig::PY_SYS && mpconfig::PY_SYS_PATH {
        for path_item in sys_path_items() {
            vstr::reset(dest);
            let (data, p_len) = objstr::str_get_data(path_item);
            if p_len > 0 {
                vstr::add_strn(dest, &data[..p_len]);
                vstr::add_char(dest, PATH_SEP_CHAR as u32);
            }
            if let Some(name) = qstr::str_data(mod_name) {
                vstr::add_strn(dest, &name);
            }
            let stat = stat_module(dest);
            if stat != ImportStat::NoExist {
                return stat;
            }
        }
        return ImportStat::NoExist;
    }
    if let Some(name) = qstr::str_data(mod_name) {
        vstr::add_strn(dest, &name);
    }
    stat_module(dest)
}

fn parse_compile_execute(lex: Lexer, globals: *mut ObjDict) {
    let source_name = lex.source_name;
    if mpconfig::MODULE___FILE__ {
        runtime::store_attr(
            obj::from_ptr(globals as *const ObjDict as *const ()),
            qstr::from_str("__file__"),
            obj::new_qstr(source_name),
        );
    }
    let mut tree = parse::parse(lex, ParseInputKind::FileInput);
    let ctx = malloc::new_obj::<ModuleContext>().expect("import module context");
    unsafe {
        (*ctx).module.globals = globals;
        (*ctx).constants = Default::default();
    }
    let mut cm = CompiledModule {
        context: ctx,
        rc: core::ptr::null(),
        has_native: false,
        n_qstr: 0,
        n_obj: 0,
        arch_flags: 0,
    };
    compile::compile_to_raw_code(&mut tree, source_name, false, &mut cm);
    let fun = emitglue::make_function_from_proto_fun(cm.rc as ProtoFun, ctx, None);
    runtime::call_function_0(fun);
}

fn do_execute_proto_fun(context: *const ModuleContext, proto_fun: ProtoFun, source_name: Qstr) {
    if mpconfig::MODULE___FILE__ {
        runtime::store_attr(
            obj::from_ptr(unsafe { (*context).module.globals } as *const ObjDict as *const ()),
            qstr::from_str("__file__"),
            obj::new_qstr(source_name),
        );
    }
    let mod_globals = unsafe { (*context).module.globals };
    let old_globals = runtime::globals_get();
    let old_locals = runtime::locals_get();
    runtime::globals_set(obj::from_ptr(mod_globals as *const ObjDict as *const ()));
    runtime::locals_set(obj::from_ptr(mod_globals as *const ObjDict as *const ()));
    crate::nlr::push_jump_callback(move || {
        runtime::globals_locals_set_from_nlr_jump_callback(old_globals, old_locals);
    });
    let module_fun = emitglue::make_function_from_proto_fun(proto_fun, context, None);
    runtime::call_function_0(module_fun);
    crate::nlr::pop_jump_callback(false);
}

fn do_load(module_obj: *mut ModuleContext, file: &mut Vstr) {
    let file_str = vstr_cstr(file);

    if mpconfig::ENABLE_COMPILER || (mpconfig::PERSISTENT_CODE_LOAD && mpconfig::HAS_FILE_READER) {
        let file_qstr = qstr::from_str(&file_str);

        if mpconfig::HAS_FILE_READER && mpconfig::PERSISTENT_CODE_LOAD {
            if file.len >= 3 && file_str.as_bytes()[file.len - 3] == b'm' {
                let mut cm = CompiledModule {
                    context: module_obj,
                    rc: core::ptr::null(),
                    has_native: false,
                    n_qstr: 0,
                    n_obj: 0,
                    arch_flags: 0,
                };
                persistentcode::raw_code_load_file(file_qstr, &mut cm);
                do_execute_proto_fun(module_obj, cm.rc as ProtoFun, file_qstr);
                return;
            }
        }

        if mpconfig::ENABLE_COMPILER {
            let lex = Lexer::new_from_file(file_qstr);
            parse_compile_execute(lex, unsafe { (*module_obj).module.globals });
            return;
        }
    }

    raise_import_error_msg("script compilation not supported");
}

fn evaluate_relative_import(level: i64, module_name: &mut String, globals: Obj) {
    let name_key = obj::new_qstr(qstr::from_str("__name__"));
    let mut current_module_name_obj = objdict::dict_get(globals, name_key);
    if current_module_name_obj == obj::OBJ_NULL {
        raise_import_error_msg("can't perform relative import");
    }

    if mpconfig::MODULE_OVERRIDE_MAIN_IMPORT && mpconfig::CPYTHON_COMPAT {
        if obj::is_qstr(current_module_name_obj)
            && obj::qstr_value(current_module_name_obj) == qstr::from_str("__main__")
        {
            current_module_name_obj =
                objdict::dict_get(globals, obj::new_qstr(qstr::from_str("__main__")));
        }
    }

    let path_key = obj::new_qstr(qstr::from_str("__path__"));
    let globals_map = unsafe { &mut (*objdict::dict_ptr(globals)).map };
    let is_pkg = map::lookup(globals_map, path_key, LookupKind::Lookup).is_some();

    let (data, current_module_name_len) = objstr::str_get_data(current_module_name_obj);
    let current_module_name = std::str::from_utf8(&data[..current_module_name_len]).unwrap_or("");
    let mut p = current_module_name_len;
    let mut level = level;
    if is_pkg {
        level -= 1;
    }
    while level > 0 && p > 0 {
        p -= 1;
        if current_module_name.as_bytes()[p] == b'.' {
            level -= 1;
        }
    }
    if p == 0 {
        raise_import_error_msg("can't perform relative import");
    }

    let prefix = &current_module_name[..p];
    if module_name.is_empty() {
        *module_name = prefix.to_string();
    } else {
        *module_name = format!("{prefix}.{module_name}");
    }
}

fn unregister_module_from_nlr_jump_callback(name: Qstr) {
    let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
    let loaded_map = unsafe { &mut (*objdict::dict_ptr(loaded)).map };
    map::lookup(
        loaded_map,
        obj::new_qstr(name),
        LookupKind::RemoveIfFound,
    );
}

/// Load a module at the specified absolute path.
pub fn process_import_at_level(
    full_mod_name: Qstr,
    level_mod_name: Qstr,
    outer_module_obj: Obj,
    override_main: bool,
) -> Obj {
    if mpconfig::PY_SYS && mpconfig::PY_SYS_PATH {
        if !sys_path_items().is_empty() {
            let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
            let loaded_map = unsafe { &mut (*objdict::dict_ptr(loaded)).map };
            if let Some(elem) = map::lookup(
                loaded_map,
                obj::new_qstr(full_mod_name),
                LookupKind::Lookup,
            ) {
                return elem.value;
            }
        }
    } else {
        let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
        let loaded_map = unsafe { &mut (*objdict::dict_ptr(loaded)).map };
        if let Some(elem) = map::lookup(
            loaded_map,
            obj::new_qstr(full_mod_name),
            LookupKind::Lookup,
        ) {
            return elem.value;
        }
    }

    let mut path_buf = vec![0u8; mpconfig::ALLOC_PATH_MAX];
    let mut path = Vstr {
        alloc: path_buf.len(),
        len: 0,
        buf: path_buf.as_mut_ptr(),
        fixed_buf: true,
    };
    vstr::reset(&mut path);

    let mut stat = ImportStat::NoExist;
    let mut module_obj = obj::OBJ_NULL;

    if outer_module_obj == obj::OBJ_NULL {
        module_obj = objmodule::module_get_builtin(level_mod_name, false);
        if module_obj != obj::OBJ_NULL {
            return module_obj;
        }
        stat = stat_top_level(level_mod_name, &mut path);
        if stat == ImportStat::NoExist {
            module_obj = objmodule::module_get_builtin(level_mod_name, true);
            if module_obj != obj::OBJ_NULL {
                return module_obj;
            }
        }
    } else {
        if mpconfig::MODULE_BUILTIN_SUBPACKAGES {
            let globals = objmodule::module_get_globals(outer_module_obj);
            let globals_map = unsafe { &(*globals).map };
            if globals_map.is_fixed {
                if let Some(elem) = map::lookup(
                    unsafe { &mut (*globals).map },
                    obj::new_qstr(level_mod_name),
                    LookupKind::Lookup,
                ) {
                    if obj::is_obj(elem.value)
                        && obj::is_exact_type(elem.value, objmodule::type_module())
                    {
                        return elem.value;
                    }
                }
            }
        }

        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        runtime::load_method_maybe(outer_module_obj, qstr::from_str("__path__"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            let parent_path = objstr::str_get_str(dest[0]);
            vstr::add_str(&mut path, &parent_path);
            vstr::add_char(&mut path, PATH_SEP_CHAR as u32);
            if let Some(name) = qstr::str_data(level_mod_name) {
                vstr::add_strn(&mut path, &name);
            }
            stat = stat_module(&mut path);
        }
    }

    if stat == ImportStat::NoExist {
        raise_import_error(full_mod_name);
    }

    module_obj = objmodule::new_module(full_mod_name);
    let unregister_name = full_mod_name;
    crate::nlr::push_jump_callback(move || {
        unregister_module_from_nlr_jump_callback(unregister_name);
    });

    if mpconfig::MODULE_OVERRIDE_MAIN_IMPORT
        && override_main
        && stat != ImportStat::Dir
    {
        let globals = objmodule::module_get_globals(module_obj);
        objdict::dict_store(
            obj::from_ptr(globals as *const ObjDict as *const ()),
            obj::new_qstr(qstr::from_str("__name__")),
            obj::new_qstr(qstr::from_str("__main__")),
        );
        if mpconfig::CPYTHON_COMPAT {
            let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
            objdict::dict_store(
                loaded,
                obj::new_qstr(qstr::from_str("__main__")),
                module_obj,
            );
            objdict::dict_store(
                obj::from_ptr(globals as *const ObjDict as *const ()),
                obj::new_qstr(qstr::from_str("__main__")),
                obj::new_qstr(full_mod_name),
            );
        }
    }

    let module_ctx = obj::as_ptr(module_obj) as *mut ModuleContext;

    if stat == ImportStat::Dir {
        runtime::store_attr(
            module_obj,
            qstr::from_str("__path__"),
            objstr::new_str(vstr_cstr(&mut path).as_bytes()),
        );
        let orig_path_len = path.len;
        vstr::add_str(&mut path, "/__init__.py");
        if stat_file_py_or_mpy(&mut path) == ImportStat::File {
            do_load(module_ctx, &mut path);
        }
        path.len = orig_path_len;
    } else {
        do_load(module_ctx, &mut path);
    }

    if outer_module_obj != obj::OBJ_NULL {
        runtime::store_attr(outer_module_obj, level_mod_name, module_obj);
    }

    crate::nlr::pop_jump_callback(false);
    module_obj
}

/// Default `__import__` implementation (`mp_builtin___import___default`).
pub fn builtin___import___default(n_args: usize, args: &[Obj]) -> Obj {
    if !mpconfig::ENABLE_EXTERNAL_IMPORT {
        if n_args >= 5 && obj::small_int_value(args[4]) != 0 {
            raise::raise(MpRaise::RuntimeError("relative import"));
        }
        let loaded = crate::mpstate::with_vm(|vm| vm.mp_loaded_modules_dict);
        let loaded_map = unsafe { &mut (*objdict::dict_ptr(loaded)).map };
        if let Some(elem) = map::lookup(loaded_map, args[0], LookupKind::Lookup) {
            return elem.value;
        }
        let module_name_qstr = objstr::str_get_qstr(args[0]);
        let module_obj = objmodule::module_get_builtin(module_name_qstr, false);
        if module_obj != obj::OBJ_NULL {
            return module_obj;
        }
        let module_obj = objmodule::module_get_builtin(module_name_qstr, true);
        if module_obj != obj::OBJ_NULL {
            return module_obj;
        }
        raise_import_error(module_name_qstr);
    }

    let module_name_obj = args[0];
    let mut fromtuple = obj::CONST_NONE;
    let mut level = 0i64;
    if n_args >= 4 {
        fromtuple = args[3];
        if n_args >= 5 {
            level = obj::small_int_value(args[4]) as i64;
            if level < 0 {
                raise::raise(MpRaise::ValueError("invalid relative import level"));
            }
        }
    }

    let (data, mut module_name_len) = objstr::str_get_data(module_name_obj);
    let mut module_name = String::from_utf8_lossy(&data[..module_name_len]).into_owned();

    if level != 0 {
        let mut globals = runtime::globals_get();
        if n_args >= 2 && args[1] != obj::CONST_NONE {
            globals = args[1];
            if !obj::is_exact_type(globals, objdict::type_dict()) {
                raise::raise(MpRaise::TypeError("globals must be a dict"));
            }
        }
        evaluate_relative_import(level, &mut module_name, globals);
        module_name_len = module_name.len();
    }

    if module_name.is_empty() {
        raise::raise(MpRaise::ValueError("empty module name"));
    }

    let mut top_module_obj = obj::OBJ_NULL;
    let mut outer_module_obj = obj::OBJ_NULL;
    let bytes = module_name.as_bytes();
    let mut current_component_start = 0usize;
    for i in 1..=module_name_len {
        if i == module_name_len || bytes[i - 1] == b'.' {
            let end = if i == module_name_len { i } else { i - 1 };
            let full_mod_name = qstr::from_strn(&bytes[..end]);
            let level_mod_name =
                qstr::from_strn(&bytes[current_component_start..end]);
            let override_main = mpconfig::MODULE_OVERRIDE_MAIN_IMPORT
                && i == module_name_len
                && fromtuple == obj::CONST_FALSE;
            let imported = process_import_at_level(
                full_mod_name,
                level_mod_name,
                outer_module_obj,
                override_main,
            );
            outer_module_obj = imported;
            if top_module_obj == obj::OBJ_NULL {
                top_module_obj = imported;
            }
            current_component_start = i;
        }
    }

    if fromtuple != obj::CONST_NONE {
        outer_module_obj
    } else {
        top_module_obj
    }
}

// --- builtin function object for `__import__` ---------------------------------

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: obj::ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut IMPORT_OBJ: Option<Obj> = None;
static mut IMPORT_FUN: ObjFunBuiltinVar = ObjFunBuiltinVar {
    base: obj::ObjBase {
        type_: core::ptr::null(),
    },
    min_args: 1,
    max_args: 5,
    fun: import_dispatch,
};

fn import_dispatch(n_args: usize, args: &[Obj]) -> Obj {
    if n_args < 1 || n_args > 5 {
        crate::argcheck::check_num(n_args, 0, 1, 5, false);
    }
    builtin___import___default(n_args, args)
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}

static mut IMPORT_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];
static mut IMPORT_TYPE: obj::ObjType = obj::ObjType {
    base: obj::ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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
    slots: core::ptr::null(),
};

fn init_import_obj() -> Obj {
    unsafe {
        IMPORT_TYPE.slots = IMPORT_SLOTS.as_ptr();
        IMPORT_TYPE.name = qstr::from_str("function");
        IMPORT_FUN.base.type_ = &IMPORT_TYPE as *const obj::ObjType;
        obj::from_ptr(&raw const IMPORT_FUN as *const ObjFunBuiltinVar as *const ())
    }
}

/// Built-in `__import__` object (`mp_builtin___import___obj`).
pub fn builtin___import___obj() -> Obj {
    unsafe {
        if IMPORT_OBJ.is_none() {
            IMPORT_OBJ = Some(init_import_obj());
        }
        IMPORT_OBJ.unwrap()
    }
}
