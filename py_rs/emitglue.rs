//! rewrite of py/emitglue.c + py/emitglue.h
// symmetry: done

use crate::bc::{self, ModuleContext, ObjFunBc};
use crate::bc0;
use crate::malloc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objclosure;
use crate::objfun;
use crate::objtuple;
use crate::qstr::Qstr;

pub const PROTO_FUN_INDICATOR_RAW_CODE_0: u8 = 0;
pub const PROTO_FUN_INDICATOR_RAW_CODE_1: u8 = 0;

pub const EMIT_OPT_NONE: u16 = 0;
pub const EMIT_OPT_BYTECODE: u16 = 1;
pub const EMIT_OPT_NATIVE_PYTHON: u16 = 2;

/// Raw code kind (`mp_raw_code_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RawCodeKind {
    Unused = 0,
    Reserved,
    Bytecode,
    NativePy,
    NativeViper,
    NativeAsm,
}

/// Compiled function metadata (`mp_raw_code_t`).
#[repr(C)]
pub struct RawCode {
    pub proto_fun_indicator: [u8; 2],
    pub kind: RawCodeKind,
    pub is_generator: bool,
    pub fun_data: *const u8,
    pub children: *mut *mut RawCode,
    pub fun_data_len: u32,
    pub n_children: u16,
    pub prelude_offset: u16,
    pub asm_n_pos_args: u8,
    pub asm_type_sig: u32,
}

/// Non-instantiated function pointer (`mp_proto_fun_t`).
pub type ProtoFun = *const ();

pub fn proto_fun_is_bytecode(proto_fun: ProtoFun) -> bool {
    if proto_fun.is_null() {
        return false;
    }
    let header = proto_fun as *const u8;
    unsafe {
        let header = u16::from(header.read()) | (u16::from(header.add(1).read()) << 8);
        header
            != (PROTO_FUN_INDICATOR_RAW_CODE_0 as u16
                | ((PROTO_FUN_INDICATOR_RAW_CODE_1 as u16) << 8))
    }
}

/// Outer compiled module (`mp_compiled_module_t`).
pub struct CompiledModule {
    pub context: *mut ModuleContext,
    pub rc: *const RawCode,
    pub has_native: bool,
    pub n_qstr: usize,
    pub n_obj: usize,
    pub arch_flags: usize,
}

/// `mp_emit_glue_new_raw_code`
pub fn new_raw_code() -> *mut RawCode {
    let rc = malloc::new_obj::<RawCode>().expect("raw code alloc");
    unsafe {
        (*rc).proto_fun_indicator = [0, 0];
        (*rc).kind = RawCodeKind::Reserved;
        (*rc).is_generator = false;
        (*rc).fun_data = core::ptr::null();
        (*rc).children = core::ptr::null_mut();
        (*rc).fun_data_len = 0;
        (*rc).n_children = 0;
        (*rc).prelude_offset = 0;
        (*rc).asm_n_pos_args = 0;
        (*rc).asm_type_sig = 0;
    }
    rc
}

/// `mp_emit_glue_assign_bytecode`
pub fn assign_bytecode(
    rc: *mut RawCode,
    code: *const u8,
    children: *mut *mut RawCode,
    scope_flags: u16,
) {
    assign_bytecode_ex(rc, code, children, scope_flags, 0, 0);
}

/// Assign bytecode with persistent-code metadata.
pub fn assign_bytecode_ex(
    rc: *mut RawCode,
    code: *const u8,
    children: *mut *mut RawCode,
    scope_flags: u16,
    fun_data_len: u32,
    n_children: u16,
) {
    unsafe {
        (*rc).kind = RawCodeKind::Bytecode;
        (*rc).is_generator = scope_flags & bc0::SCOPE_FLAG_GENERATOR as u16 != 0;
        (*rc).fun_data = code;
        (*rc).children = children;
        (*rc).fun_data_len = fun_data_len;
        (*rc).n_children = n_children;
        (*rc).prelude_offset = 0;
        (*rc).asm_n_pos_args = 0;
        (*rc).asm_type_sig = 0;
    }
}

/// `mp_emit_glue_assign_native`
pub fn assign_native(
    rc: *mut RawCode,
    kind: RawCodeKind,
    fun_data: *const u8,
    fun_data_len: u32,
    children: *mut *mut RawCode,
    n_children: u16,
    prelude_offset: u16,
    scope_flags: u16,
    asm_n_pos_args: u32,
    asm_type_sig: u32,
) {
    unsafe {
        (*rc).kind = kind;
        (*rc).is_generator = scope_flags & bc0::SCOPE_FLAG_GENERATOR as u16 != 0;
        (*rc).fun_data = fun_data;
        (*rc).children = children;
        (*rc).fun_data_len = fun_data_len;
        (*rc).n_children = n_children;
        (*rc).prelude_offset = prelude_offset;
        (*rc).asm_n_pos_args = asm_n_pos_args as u8;
        (*rc).asm_type_sig = asm_type_sig;
    }
}

/// `mp_make_function_from_proto_fun`
pub fn make_function_from_proto_fun(
    proto_fun: ProtoFun,
    context: *const ModuleContext,
    def_args: Option<&[Obj; 2]>,
) -> Obj {
    debug_assert!(!proto_fun.is_null());

    if mpconfig::MODULE_FROZEN_MPY || mpconfig::PY_BUILTINS_CODE >= mpconfig::PY_BUILTINS_CODE_BASIC
    {
        if proto_fun_is_bytecode(proto_fun) {
            let bc = proto_fun as *const u8;
            let fun = objfun::new_fun_bc(def_args, bc, context, core::ptr::null());
            let mut ip = bc;
            let sig = bc::prelude_sig_decode_into(&mut ip);
            if sig.scope_flags & bc0::SCOPE_FLAG_GENERATOR as usize != 0 {
                unsafe {
                    let base = obj::as_ptr(fun) as *mut obj::ObjBase;
                    (*base).type_ = crate::objgenerator::type_gen_wrap() as *const obj::ObjType;
                }
            }
            return fun;
        }
    }

    let rc = proto_fun as *const RawCode;
    unsafe {
        match (*rc).kind {
            RawCodeKind::NativePy if mpconfig::ENABLE_NATIVE_CODE => {
                let fun = objfun::new_fun_native(
                    def_args,
                    (*rc).fun_data as *const (),
                    context,
                    (*rc).children as *const *const (),
                );
                if (*rc).is_generator {
                    let base = obj::as_ptr(fun) as *mut obj::ObjBase;
                    (*base).type_ =
                        crate::objgenerator::type_native_gen_wrap() as *const obj::ObjType;
                }
                fun
            }
            RawCodeKind::NativeViper if mpconfig::ENABLE_NATIVE_CODE => objfun::new_fun_viper(
                (*rc).fun_data as *const (),
                context,
                (*rc).children as *const *const (),
            ),
            RawCodeKind::NativeAsm if mpconfig::EMIT_INLINE_ASM => objfun::new_fun_asm(
                (*rc).asm_n_pos_args as usize,
                (*rc).fun_data as *const (),
                (*rc).asm_type_sig as usize,
            ),
            _ => {
                debug_assert!((*rc).kind == RawCodeKind::Bytecode);
                let fun = objfun::new_fun_bc(
                    def_args,
                    (*rc).fun_data,
                    context,
                    (*rc).children as *const *const (),
                );
                if (*rc).is_generator {
                    let base = obj::as_ptr(fun) as *mut obj::ObjBase;
                    (*base).type_ = crate::objgenerator::type_gen_wrap() as *const obj::ObjType;
                }
                fun
            }
        }
    }
}

/// `mp_make_closure_from_proto_fun`
pub fn make_closure_from_proto_fun(
    proto_fun: ProtoFun,
    context: *const ModuleContext,
    n_closed_over: usize,
    args: &[Obj],
) -> Obj {
    let ffun = if n_closed_over & 0x100 != 0 {
        let def: [Obj; 2] = [
            args[0],
            if args.len() > 1 {
                args[1]
            } else {
                obj::OBJ_NULL
            },
        ];
        make_function_from_proto_fun(proto_fun, context, Some(&def))
    } else {
        make_function_from_proto_fun(proto_fun, context, None)
    };
    objclosure::new_closure(
        ffun,
        n_closed_over & 0xff,
        &args[(n_closed_over >> 7) & 2..],
    )
}

/// Allocate module context tables (`mp_module_context_alloc_tables`).
pub fn module_context_alloc_tables(context: *mut ModuleContext, n_qstr: usize, n_obj: usize) {
    if context.is_null() {
        return;
    }
    unsafe {
        if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
            let qstr = Box::leak(Box::new(vec![0 as Qstr; n_qstr]));
            let obj = Box::leak(Box::new(vec![obj::OBJ_NULL; n_obj]));
            (*context).constants.qstr_table = qstr.as_mut_ptr();
            (*context).constants.obj_table = obj.as_mut_ptr();
            (*context).n_qstr = n_qstr;
            (*context).n_obj = n_obj;
        } else {
            (*context).constants.qstr_table = core::ptr::null_mut();
            if n_obj == 0 {
                (*context).constants.obj_table = core::ptr::null_mut();
            } else {
                let obj = Box::leak(Box::new(vec![obj::OBJ_NULL; n_obj]));
                (*context).constants.obj_table = obj.as_mut_ptr();
            }
            (*context).n_qstr = 0;
            (*context).n_obj = n_obj;
        }
    }
}
