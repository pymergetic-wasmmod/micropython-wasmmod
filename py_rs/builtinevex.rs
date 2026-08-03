//! rewrite of py/builtinevex.c
// symmetry: done

use crate::argcheck;
use crate::bc::ModuleContext;
use crate::compile;
use crate::emitglue::{self, CompiledModule, ProtoFun};
use crate::lexer::Lexer;
use crate::malloc;
use crate::mpconfig;
use crate::nlr;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objcode::{self, ObjCode};
use crate::objdict;
use crate::objmodule::type_module;
use crate::objstr;
use crate::parse::{self, ParseInputKind};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::reader::READER_IS_ROM;

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: obj::ObjBase,
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

fn code_execute(code: &ObjCode, globals: Obj, locals: Obj) -> Obj {
    let old_globals = runtime::globals_get();
    let old_locals = runtime::locals_get();
    runtime::globals_set(globals);
    runtime::locals_set(locals);

    nlr::push_jump_callback(move || {
        runtime::globals_locals_set_from_nlr_jump_callback(old_globals, old_locals);
    });

    let module_fun = if mpconfig::PY_BUILTINS_CODE >= mpconfig::PY_BUILTINS_CODE_BASIC {
        let ctx = malloc::new_obj::<ModuleContext>().expect("module context");
        unsafe {
            (*ctx).module.base = ObjBase {
                type_: type_module() as *const ObjType,
            };
            (*ctx).module.globals = objdict::dict_ptr(globals);
            (*ctx).constants = code.constants.clone();
        }
        emitglue::make_function_from_proto_fun(code.proto_fun, ctx, None)
    } else {
        obj::OBJ_NULL
    };

    let ret = runtime::call_function_0(module_fun);
    nlr::pop_jump_callback(true);
    ret
}

fn parse_mode(mode: Qstr) -> ParseInputKind {
    let single = qstr::from_str("single");
    let exec = qstr::from_str("exec");
    let eval = qstr::from_str("eval");
    if mode == single {
        ParseInputKind::SingleInput
    } else if mode == exec {
        ParseInputKind::FileInput
    } else if mode == eval {
        ParseInputKind::EvalInput
    } else {
        raise::raise(MpRaise::ValueError("bad compile mode"));
    }
}

fn builtin_compile(_n_args: usize, args: &[Obj]) -> Obj {
    if !mpconfig::PY_BUILTINS_COMPILE {
        return obj::OBJ_NULL;
    }

    let (str_data, str_len) = objstr::str_get_data(args[0]);
    let filename = objstr::str_get_qstr(args[1]);
    let mode = objstr::str_get_qstr(args[2]);
    let parse_input_kind = parse_mode(mode);

    let lex = Lexer::new_from_str_len(filename, &str_data[..str_len], READER_IS_ROM);

    if mpconfig::PY_BUILTINS_CODE >= mpconfig::PY_BUILTINS_CODE_BASIC {
        let mut parse_tree = parse::parse(lex, parse_input_kind);
        let ctx = malloc::new_obj::<ModuleContext>().expect("compile ctx");
        unsafe {
            (*ctx).module.globals = core::ptr::null_mut();
        }
        let mut cm = CompiledModule {
            context: ctx,
            rc: core::ptr::null(),
            has_native: false,
            n_qstr: 0,
            n_obj: 0,
            arch_flags: 0,
        };
        compile::compile_to_raw_code(
            &mut parse_tree,
            filename,
            parse_input_kind == ParseInputKind::SingleInput,
            &mut cm,
        );
        let constants = unsafe { (*ctx).constants.clone() };
        return objcode::new_code(constants, cm.rc as ProtoFun);
    }

    compile::parse_compile_execute(lex, parse_input_kind, None, None)
}

fn eval_exec_helper(n_args: usize, args: &[Obj], parse_input_kind: ParseInputKind) -> Obj {
    if !mpconfig::PY_BUILTINS_EVAL_EXEC {
        return obj::OBJ_NULL;
    }

    let mut globals = runtime::globals_get();
    let mut locals = runtime::locals_get();
    for i in 1..3.min(n_args) {
        if args[i] != obj::CONST_NONE {
            if !objdict::is_dict_or_ordereddict(args[i]) {
                raise::raise(MpRaise::TypeError("dict expected"));
            }
            locals = args[i];
            if i == 1 {
                globals = locals;
            }
        }
    }

    if mpconfig::PY_BUILTINS_COMPILE {
        if let Some(code) = objcode::as_code(args[0]) {
            return code_execute(code, globals, locals);
        }
    }

    let mut kind = parse_input_kind;
    let lex = if mpconfig::PY_BUILTINS_EXECFILE && parse_input_kind == ParseInputKind::SingleInput {
        kind = ParseInputKind::FileInput;
        Lexer::new_from_file(objstr::str_get_qstr(args[0]))
    } else {
        let mut bufinfo = obj::BufferInfo::default();
        obj::get_buffer_raise(args[0], &mut bufinfo, obj::BUFFER_READ);
        let slice = unsafe { std::slice::from_raw_parts(bufinfo.buf, bufinfo.len) };
        Lexer::new_from_str_len(qstr::from_str("<string>"), slice, READER_IS_ROM)
    };

    compile::parse_compile_execute(lex, kind, Some(globals), Some(locals))
}

fn builtin_eval(n_args: usize, args: &[Obj]) -> Obj {
    eval_exec_helper(n_args, args, ParseInputKind::EvalInput)
}

fn builtin_exec(n_args: usize, args: &[Obj]) -> Obj {
    eval_exec_helper(n_args, args, ParseInputKind::FileInput)
}

fn builtin_execfile(n_args: usize, args: &[Obj]) -> Obj {
    if !mpconfig::PY_BUILTINS_EXECFILE {
        return obj::OBJ_NULL;
    }
    eval_exec_helper(n_args, args, ParseInputKind::SingleInput)
}

pub fn builtin_compile_obj() -> Obj {
    new_fun_builtin_var(3, 6, builtin_compile)
}

pub fn builtin_eval_obj() -> Obj {
    new_fun_builtin_var(1, 3, builtin_eval)
}

pub fn builtin_exec_obj() -> Obj {
    new_fun_builtin_var(1, 3, builtin_exec)
}

pub fn builtin_execfile_obj() -> Obj {
    new_fun_builtin_var(1, 3, builtin_execfile)
}
