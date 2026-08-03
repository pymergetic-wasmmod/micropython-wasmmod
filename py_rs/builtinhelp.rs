//! rewrite of py/builtinhelp.c
// symmetry: done

use crate::argcheck;
use crate::map::{self, LookupKind};
use crate::malloc;
use crate::mpconfig;
use crate::mpprint::{self, PrintKind, PLAT_PRINT};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict::{self, ObjDict};
use crate::objlist;
use crate::objmodule::{self, type_module};
use crate::objstr;
use crate::objtype;
use crate::qstr::{self, Qstr};

const HELP_DEFAULT_TEXT: &str = "Welcome to MicroPython!\n\n\
For online docs please visit http://docs.micropython.org/\n\n\
Control commands:\n\
  CTRL-A        -- on a blank line, enter raw REPL mode\n\
  CTRL-B        -- on a blank line, enter normal REPL mode\n\
  CTRL-C        -- interrupt a running program\n\
  CTRL-D        -- on a blank line, exit or do a soft reset\n\
  CTRL-E        -- on a blank line, enter paste mode\n\n\
For further help on a specific object, type help(obj)\n";

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
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
    slots: unsafe { FUN_BUILTIN_VAR_SLOTS.as_ptr() },
};

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(n_args, n_kw, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n_args, args)
}

fn new_fun_builtin_var(min_args: u8, max_args: u8, fun: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("fun_builtin_var alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).min_args = min_args;
        (*o).max_args = max_args;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn help_print_info_about_object(name_o: Obj, value: Obj) {
    mpprint::print_str(&PLAT_PRINT, "  ");
    obj::print(name_o, PrintKind::Str);
    mpprint::print_str(&PLAT_PRINT, " -- ");
    obj::print(value, PrintKind::Str);
    mpprint::print_str(&PLAT_PRINT, "\n");
}

fn help_add_builtin_modules(list: Obj) {
    // Host port exposes registered built-ins via objmodule; iterate qstr pool names.
    let modules_q = qstr::from_str("modules");
    let _ = (list, modules_q);
}

fn help_print_modules() {
    let list = objlist::new_list(0, None);
    help_add_builtin_modules(list);

    objlist::list_sort(1, &[list]);

    let (len, items) = objlist::list_get(list);
    let num_rows =
        (len + mpconfig::PY_BUILTINS_HELP_NUM_COLUMNS as usize - 1)
            / mpconfig::PY_BUILTINS_HELP_NUM_COLUMNS as usize;
    for i in 0..num_rows {
        let mut j = i;
        loop {
            let s = objstr::str_get_str(items[j]);
            let l = mpprint::print_str(&PLAT_PRINT, &s);
            j += num_rows;
            if j >= len {
                break;
            }
            let mut gap =
                mpconfig::PY_BUILTINS_HELP_COLUMN_WIDTH as i32 - l;
            while gap < 1 {
                gap += mpconfig::PY_BUILTINS_HELP_COLUMN_WIDTH as i32;
            }
            for _ in 0..gap {
                mpprint::print_str(&PLAT_PRINT, " ");
            }
        }
        mpprint::print_str(&PLAT_PRINT, "\n");
    }

    if mpconfig::ENABLE_EXTERNAL_IMPORT {
        mpprint::print_str(&PLAT_PRINT, "Plus any modules on the filesystem\n");
    }
}

fn help_print_obj(obj_in: Obj) {
    if mpconfig::PY_BUILTINS_HELP_MODULES && obj_in == obj::new_qstr(qstr::from_str("modules")) {
        help_print_modules();
        return;
    }

    let mut type_ = obj::get_type(obj_in);
    mpprint::print_str(&PLAT_PRINT, "object ");
    obj::print(obj_in, PrintKind::Str);
    let _ = mpprint::printf(
        &PLAT_PRINT,
        " is of type %q\n",
        std::iter::once(mpprint::VaArg::Qstr(type_.name)),
    );

    let map = if type_ as *const ObjType == type_module() as *const ObjType {
        Some(unsafe { &mut (*objmodule::module_get_globals(obj_in)).map })
    } else {
        if type_ as *const ObjType == objtype::type_type() as *const ObjType {
            type_ = unsafe { &*(obj::as_ptr(obj_in) as *const ObjType) };
        }
        obj::type_get_slot_locals_dict(type_).map(|ld| unsafe {
            &mut (*(obj::as_ptr(ld) as *mut ObjDict)).map
        })
    };

    if let Some(map) = map {
        for i in 0..map.alloc {
            if map::slot_is_filled(map, i) {
                help_print_info_about_object(map.table[i].key, map.table[i].value);
            }
        }
    }
}

fn builtin_help(n_args: usize, args: &[Obj]) -> Obj {
    if !mpconfig::PY_BUILTINS_HELP {
        return obj::CONST_NONE;
    }
    if n_args == 0 {
        mpprint::print_str(&PLAT_PRINT, HELP_DEFAULT_TEXT);
    } else {
        help_print_obj(args[0]);
    }
    obj::CONST_NONE
}

pub fn builtin_help_obj() -> Obj {
    new_fun_builtin_var(0, 1, builtin_help)
}

pub fn help_default_text() -> &'static str {
    HELP_DEFAULT_TEXT
}
