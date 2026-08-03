//! rewrite of py/objgenerator.c + py/objgenerator.h
// symmetry: done

use core::mem::size_of;
use core::ptr;

use crate::argcheck;
use crate::bc::{self, CodeState, CodeStateNative, ExcStack, ModuleContext, ObjFunBc};
use crate::cstack;
use crate::emitglue;
use crate::malloc;
use crate::map::{self, LookupKind, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::mpstate;
use crate::nlr;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_ITER_IS_ITERNEXT};
use crate::objdict::{self, ObjDict};
use crate::objexcept::{self, ObjException};
use crate::objfun;
use crate::objstr;
use crate::objtuple;
use crate::objtype;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime::VmReturnKind;
use crate::runtime0::CODE_STATE_EXC_SP_IDX_SENTINEL;
use crate::vm;

// --- GeneratorExit singleton --------------------------------------------------

static mut CONST_GENERATOR_EXIT: ObjException = ObjException {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    traceback_alloc_len: 0,
    traceback_data: core::ptr::null_mut(),
    args: core::ptr::null_mut(),
};

static GENERATOR_EXIT_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_generator_exit() {
    GENERATOR_EXIT_INIT.get_or_init(|| unsafe {
        CONST_GENERATOR_EXIT.base.type_ = objexcept::type_generator_exit() as *const ObjType;
        CONST_GENERATOR_EXIT.args =
            obj::as_ptr(objtuple::const_empty_tuple()) as *mut objtuple::ObjTuple;
    });
}

pub fn const_generator_exit() -> Obj {
    init_generator_exit();
    unsafe { obj::from_ptr(&CONST_GENERATOR_EXIT as *const ObjException as *const ()) }
}

// --- generator instance -------------------------------------------------------

#[repr(C)]
pub struct ObjGenInstance {
    pub base: ObjBase,
    /// `mp_const_none`: not running; `OBJ_NULL`: running; other: pending exception.
    pub pend_exc: Obj,
    pub code_state: CodeState,
}

#[repr(C)]
pub struct ObjGenInstanceNative {
    pub base: ObjBase,
    pub pend_exc: Obj,
    pub code_state: CodeStateNative,
}

fn gen_instance_native_ptr(o: Obj) -> *mut ObjGenInstanceNative {
    obj::as_ptr(o) as *mut ObjGenInstanceNative
}

fn gen_instance_ptr(o: Obj) -> *mut ObjGenInstance {
    obj::as_ptr(o) as *mut ObjGenInstance
}

fn decode_code_state_size(bytecode: *const u8) -> (usize, usize) {
    let mut ip = bytecode;
    let sig = bc::prelude_sig_decode_into(&mut ip);
    let state_size = sig.n_state * size_of::<Obj>() + sig.n_exc_stack * size_of::<ExcStack>();
    (sig.n_state, state_size)
}

fn is_native_gen_instance(o: Obj) -> bool {
    unsafe { (*gen_instance_ptr(o)).code_state.exc_sp_idx == CODE_STATE_EXC_SP_IDX_SENTINEL }
}

fn gen_fun_bc(o: Obj) -> *const ObjFunBc {
    if is_native_gen_instance(o) {
        let inst = unsafe { &*gen_instance_native_ptr(o) };
        obj::as_ptr(inst.code_state.fun_bc) as *const ObjFunBc
    } else {
        unsafe { (*gen_instance_ptr(o)).code_state.fun_bc }
    }
}

fn gen_instance_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let name = objfun::fun_bc_get_name(unsafe { &*gen_fun_bc(self_in) });
    let _ = mpprint::printf(
        print,
        "<generator object '%q'",
        std::iter::once(mpprint::VaArg::Qstr(name)),
    );
    mpprint::print_str(print, " at ");
    mpprint::print_str(print, &format!("{self_in:?}"));
    mpprint::print_str(print, ">");
}

/// `mp_obj_gen_resume`
pub fn gen_resume(
    self_in: Obj,
    send_value: Obj,
    throw_value: Obj,
    ret_val: &mut Obj,
) -> VmReturnKind {
    cstack::check();
    if !obj::is_exact_type(self_in, type_gen_instance()) {
        raise::raise(MpRaise::TypeError("expecting a generator"));
    }

    let native = is_native_gen_instance(self_in);
    let ip = if native {
        unsafe { (*gen_instance_native_ptr(self_in)).code_state.ip }
    } else {
        unsafe { (*gen_instance_ptr(self_in)).code_state.ip }
    };
    if ip.is_null() {
        *ret_val = obj::CONST_NONE;
        return VmReturnKind::Normal;
    }

    let pend_exc = if native {
        unsafe { (*gen_instance_native_ptr(self_in)).pend_exc }
    } else {
        unsafe { (*gen_instance_ptr(self_in)).pend_exc }
    };
    if pend_exc == obj::OBJ_NULL {
        raise::raise(MpRaise::ValueError("generator already executing"));
    }

    let mut throw_value = throw_value;
    if mpconfig::PY_GENERATOR_PEND_THROW && pend_exc != obj::CONST_NONE {
        throw_value = pend_exc;
    }

    if native {
        let self_ = unsafe { &mut *gen_instance_native_ptr(self_in) };
        let state_start = unsafe { self_.code_state.state_ptr().sub(1) };
        if self_.code_state.sp == state_start {
            if send_value != obj::CONST_NONE {
                raise::raise(MpRaise::TypeError(
                    "can't send non-None value to a just-started generator",
                ));
            }
        } else {
            unsafe {
                *self_.code_state.sp = send_value;
            }
        }
        self_.pend_exc = obj::OBJ_NULL;
        self_.code_state.old_globals = objdict::dict_ptr(mpstate::globals_get());
        let fun = unsafe { &*(obj::as_ptr(self_.code_state.fun_bc) as *const ObjFunBc) };
        mpstate::globals_set(obj::from_ptr(
            unsafe { (*fun.context).module.globals } as *const ObjDict as *const ()
        ));
        type NativeGenFn = unsafe extern "C-unwind" fn(*mut CodeStateNative, Obj) -> VmReturnKind;
        let mut nlr_buf = nlr::NlrBuf::default();
        let ret_kind = match nlr::protect(&mut nlr_buf, || unsafe {
            let resume_fn: NativeGenFn =
                core::mem::transmute(objfun::fun_native_get_generator_resume(fun));
            resume_fn(&mut self_.code_state, throw_value)
        }) {
            Ok(k) => k,
            Err(v) => {
                unsafe {
                    *self_.code_state.state_ptr() = Obj(v);
                }
                VmReturnKind::Exception
            }
        };
        mpstate::globals_set(obj::from_ptr(
            self_.code_state.old_globals as *const ObjDict as *const (),
        ));
        self_.pend_exc = obj::CONST_NONE;
        match ret_kind {
            VmReturnKind::Normal => {
                self_.code_state.ip = ptr::null();
                unsafe {
                    *ret_val = *self_.code_state.sp;
                }
            }
            VmReturnKind::Yield => unsafe {
                *ret_val = *self_.code_state.sp;
                if mpconfig::PY_GENERATOR_PEND_THROW {
                    *self_.code_state.sp = obj::CONST_NONE;
                }
            },
            VmReturnKind::Exception => {
                self_.code_state.ip = ptr::null();
                unsafe {
                    *ret_val = *self_.code_state.state_ptr();
                }
                if objtype::is_subclass_fast(
                    obj::from_ptr(obj::get_type(*ret_val) as *const ObjType as *const ()),
                    obj::from_ptr(objexcept::type_stop_iteration() as *const ObjType as *const ()),
                ) {
                    let msg = objstr::new_str(b"generator raised StopIteration");
                    *ret_val =
                        objexcept::new_exception_args(objexcept::type_runtime_error(), 1, &[msg]);
                }
            }
        }
        return ret_kind;
    }

    let self_ = unsafe { &mut *gen_instance_ptr(self_in) };
    let state_start = unsafe { self_.code_state.state_ptr().sub(1) };
    if self_.code_state.sp == state_start {
        if send_value != obj::CONST_NONE {
            raise::raise(MpRaise::TypeError(
                "can't send non-None value to a just-started generator",
            ));
        }
    } else {
        unsafe {
            *self_.code_state.sp = send_value;
        }
    }

    self_.pend_exc = obj::OBJ_NULL;

    self_.code_state.old_globals = objdict::dict_ptr(mpstate::globals_get());
    let fun = unsafe { &*self_.code_state.fun_bc };
    mpstate::globals_set(obj::from_ptr(
        unsafe { (*fun.context).module.globals } as *const ObjDict as *const (),
    ));

    let ret_kind = vm::execute_bytecode(&mut self_.code_state, throw_value);

    mpstate::globals_set(obj::from_ptr(
        self_.code_state.old_globals as *const ObjDict as *const (),
    ));

    self_.pend_exc = obj::CONST_NONE;

    match ret_kind {
        VmReturnKind::Normal => {
            self_.code_state.ip = ptr::null();
            unsafe {
                *ret_val = *self_.code_state.sp;
            }
        }
        VmReturnKind::Yield => unsafe {
            *ret_val = *self_.code_state.sp;
            if mpconfig::PY_GENERATOR_PEND_THROW {
                *self_.code_state.sp = obj::CONST_NONE;
            }
        },
        VmReturnKind::Exception => {
            self_.code_state.ip = ptr::null();
            unsafe {
                *ret_val = *self_.code_state.state_ptr();
            }
            if objtype::is_subclass_fast(
                obj::from_ptr(obj::get_type(*ret_val) as *const ObjType as *const ()),
                obj::from_ptr(objexcept::type_stop_iteration() as *const ObjType as *const ()),
            ) {
                let msg = objstr::new_str(b"generator raised StopIteration");
                *ret_val =
                    objexcept::new_exception_args(objexcept::type_runtime_error(), 1, &[msg]);
            }
        }
    }

    ret_kind
}

fn raise_stop_iteration(ret: Obj) -> ! {
    if ret == obj::OBJ_NULL {
        raise::raise_obj(objexcept::new_exception(objexcept::type_stop_iteration()));
    } else {
        raise::raise_obj(objexcept::new_exception_args(
            objexcept::type_stop_iteration(),
            1,
            &[ret],
        ));
    }
}

fn gen_resume_and_raise(
    self_in: Obj,
    send_value: Obj,
    throw_value: Obj,
    raise_stop_iter: bool,
) -> Obj {
    let mut ret = obj::OBJ_NULL;
    match gen_resume(self_in, send_value, throw_value, &mut ret) {
        VmReturnKind::Normal => {
            if ret == obj::CONST_NONE {
                ret = obj::OBJ_NULL;
            }
            if raise_stop_iter {
                raise_stop_iteration(ret);
            } else {
                return runtime::make_stop_iteration(ret);
            }
        }
        VmReturnKind::Yield => ret,
        VmReturnKind::Exception => raise::raise_obj(ret),
    }
}

fn gen_instance_iternext(self_in: Obj, _buf: *mut obj::ObjIterBuf) -> Obj {
    gen_resume_and_raise(self_in, obj::CONST_NONE, obj::OBJ_NULL, false)
}

fn gen_instance_send(self_in: Obj, send_value: Obj) -> Obj {
    gen_resume_and_raise(self_in, send_value, obj::OBJ_NULL, true)
}

fn gen_instance_throw(n_args: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, 0, 2, 4, false);
    let mut exc = args[1];
    if n_args > 2 && args[2] != obj::CONST_NONE {
        exc = args[2];
    }
    gen_resume_and_raise(args[0], obj::CONST_NONE, exc, true)
}

fn gen_instance_close(self_in: Obj) -> Obj {
    let mut ret = obj::OBJ_NULL;
    match gen_resume(self_in, obj::CONST_NONE, const_generator_exit(), &mut ret) {
        VmReturnKind::Yield => {
            raise::raise(MpRaise::RuntimeError("generator ignored GeneratorExit"));
        }
        VmReturnKind::Exception => {
            if objtype::is_subclass_fast(
                obj::from_ptr(obj::get_type(ret) as *const ObjType as *const ()),
                obj::from_ptr(objexcept::type_generator_exit() as *const ObjType as *const ()),
            ) {
                return obj::CONST_NONE;
            }
            raise::raise_obj(ret);
        }
        VmReturnKind::Normal => obj::CONST_NONE,
    }
}

fn gen_instance_pend_throw(self_in: Obj, exc_in: Obj) -> Obj {
    let self_ = unsafe { &mut *gen_instance_ptr(self_in) };
    if self_.pend_exc == obj::OBJ_NULL {
        raise::raise(MpRaise::ValueError("generator already executing"));
    }
    let prev = self_.pend_exc;
    self_.pend_exc = exc_in;
    prev
}

// --- builtin method wrappers --------------------------------------------------

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

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

static mut FUN_BUILTIN_1_SLOTS: [*const (); 1] = [fun_builtin_1_call as *const ()];
static mut FUN_BUILTIN_2_SLOTS: [*const (); 1] = [fun_builtin_2_call as *const ()];
static mut FUN_BUILTIN_VAR_SLOTS: [*const (); 1] = [fun_builtin_var_call as *const ()];

static TYPE_FUN_BUILTIN_1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FUN_BUILTIN_1_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FUN_BUILTIN_2_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF | obj::TYPE_FLAG_BUILTIN_FUN,
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

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin1) };
    (self_.fun)(args[0])
}

fn fun_builtin_2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltin2) };
    (self_.fun)(args[0], args[1])
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n_args,
        n_kw,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n_args, args)
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("fun_builtin_1 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("fun_builtin_2 alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_2 as *const ObjType;
        (*o).fun = fun;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
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

// --- generator wrapper --------------------------------------------------------

fn decode_native_code_state_size(fun: &ObjFunBc) -> (usize, usize) {
    let mut ip = objfun::fun_native_get_prelude_ptr(fun);
    let sig = bc::prelude_sig_decode_into(&mut ip);
    let state_size = sig.n_state * size_of::<Obj>();
    (sig.n_state, state_size)
}

fn gen_wrap_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_fun = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBc) };
    let (n_state, state_size) = decode_code_state_size(self_fun.bytecode);
    let o =
        obj::malloc_var::<ObjGenInstance>(state_size, type_gen_instance()) as *mut ObjGenInstance;
    unsafe {
        (*o).pend_exc = obj::CONST_NONE;
        (*o).code_state.fun_bc = obj::as_ptr(self_in) as *mut ObjFunBc;
        (*o).code_state.n_state = n_state as u16;
        bc::setup_code_state(&mut (*o).code_state, n_args, n_kw, args);
        obj::from_ptr(o as *const ObjGenInstance as *const ())
    }
}

fn native_gen_wrap_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_fun = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBc) };
    let (n_state, state_size) = decode_native_code_state_size(self_fun);
    let o = obj::malloc_var::<ObjGenInstanceNative>(state_size, type_gen_instance())
        as *mut ObjGenInstanceNative;
    unsafe {
        (*o).pend_exc = obj::CONST_NONE;
        (*o).code_state.fun_bc = self_in;
        (*o).code_state.n_state = n_state as u16;
        bc::setup_code_state_native(&mut (*o).code_state, n_args, n_kw, args);
        (*o).code_state.exc_sp_idx = CODE_STATE_EXC_SP_IDX_SENTINEL;
        (*o).code_state.ip = objfun::fun_native_get_generator_start(self_fun) as *const u8;
        obj::from_ptr(o as *const ObjGenInstanceNative as *const ())
    }
}

static mut GEN_WRAP_SLOTS: [*const (); 2] = [gen_wrap_call as *const (), core::ptr::null()];

static mut TYPE_GEN_WRAP: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: if mpconfig::PY_FUNCTION_ATTRS { 2 } else { 0 },
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { GEN_WRAP_SLOTS.as_ptr() },
};

static mut NATIVE_GEN_WRAP_SLOTS: [*const (); 2] =
    [native_gen_wrap_call as *const (), core::ptr::null()];

static mut TYPE_NATIVE_GEN_WRAP: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BINDS_SELF,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: if mpconfig::PY_FUNCTION_ATTRS { 2 } else { 0 },
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { NATIVE_GEN_WRAP_SLOTS.as_ptr() },
};

static mut GEN_INSTANCE_SLOTS: [*const (); 3] = [
    gen_instance_print as *const (),
    gen_instance_iternext as *const (),
    core::ptr::null(),
];

static mut TYPE_GEN_INSTANCE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_ITER_IS_ITERNEXT,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 1,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 2,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { GEN_INSTANCE_SLOTS.as_ptr() },
};

static GEN_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_generator_types() {
    GEN_INIT.get_or_init(|| {
        unsafe {
            TYPE_GEN_WRAP.name = qstr::from_str("generator");
            TYPE_NATIVE_GEN_WRAP.name = qstr::from_str("generator");
            TYPE_GEN_INSTANCE.name = qstr::from_str("generator");
            if mpconfig::PY_FUNCTION_ATTRS {
                GEN_WRAP_SLOTS[1] = objfun::fun_bc_attr as *const ();
                NATIVE_GEN_WRAP_SLOTS[1] = objfun::fun_bc_attr as *const ();
            }
            let mut table = vec![
                MapElem {
                    key: obj::new_qstr(qstr::from_str("close")),
                    value: new_fun_builtin_1(gen_instance_close),
                },
                MapElem {
                    key: obj::new_qstr(qstr::from_str("send")),
                    value: new_fun_builtin_2(gen_instance_send),
                },
                MapElem {
                    key: obj::new_qstr(qstr::from_str("throw")),
                    value: new_fun_builtin_var(2, 4, gen_instance_throw),
                },
            ];
            if mpconfig::PY_GENERATOR_PEND_THROW {
                table.push(MapElem {
                    key: obj::new_qstr(qstr::from_str("pend_throw")),
                    value: new_fun_builtin_2(gen_instance_pend_throw),
                });
            }
            let ptr =
                obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
            map::init_fixed_table(&mut (*ptr).map, table);
            GEN_INSTANCE_SLOTS[2] =
                obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
        init_generator_exit();
    });
}

pub fn type_gen_wrap() -> &'static ObjType {
    init_generator_types();
    unsafe { &TYPE_GEN_WRAP }
}

pub fn type_native_gen_wrap() -> &'static ObjType {
    init_generator_types();
    unsafe { &TYPE_NATIVE_GEN_WRAP }
}

pub fn type_gen_instance() -> &'static ObjType {
    init_generator_types();
    unsafe { &TYPE_GEN_INSTANCE }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::mpprint;
    use crate::objmodule;
    use crate::runtime;

    fn setup() {
        let _ = gc::init();
        runtime::init();
        init_generator_types();
    }

    fn make_test_fun_bc(name: Qstr) -> Obj {
        // Minimal prelude: n_state=1, n_exc_stack=0, name index 0, no cells.
        static mut BYTECODE: [u8; 5] = [
            0x00,
            0x00,
            0x00,
            crate::bc0::LOAD_CONST_NONE,
            crate::bc0::RETURN_VALUE,
        ];
        let ctx = malloc::new_obj::<ModuleContext>().expect("ctx");
        unsafe {
            (*ctx).module.base.type_ = objmodule::type_module() as *const ObjType;
            (*ctx).module.globals =
                objdict::dict_ptr(objdict::new_dict(mpconfig::MODULE_DICT_SIZE as usize));
            emitglue::module_context_alloc_tables(ctx, 1, 0);
            (*ctx).qstr_table_mut()[0] = name;
        }
        let fun = malloc::new_obj::<ObjFunBc>().expect("fun");
        unsafe {
            (*fun).base.type_ = type_gen_wrap() as *const ObjType;
            (*fun).context = ctx;
            (*fun).child_table = ptr::null();
            (*fun).bytecode = BYTECODE.as_ptr();
            obj::from_ptr(fun as *const ObjFunBc as *const ())
        }
    }

    #[test]
    fn gen_wrap_call_creates_instance() {
        setup();
        let name = qstr::from_str("g");
        let fun = make_test_fun_bc(name);
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        assert!(obj::is_exact_type(gen, type_gen_instance()));
        let inst = unsafe { &*gen_instance_ptr(gen) };
        assert_eq!(inst.pend_exc, obj::CONST_NONE);
        assert!(!inst.code_state.ip.is_null());
    }

    #[test]
    fn gen_resume_stopped_returns_normal() {
        setup();
        let fun = make_test_fun_bc(qstr::from_str("g"));
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        let inst = unsafe { &mut *gen_instance_ptr(gen) };
        inst.code_state.ip = ptr::null();
        let mut ret = obj::OBJ_NULL;
        let kind = gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, VmReturnKind::Normal);
        assert_eq!(ret, obj::CONST_NONE);
    }

    #[test]
    fn gen_resume_executes_and_completes() {
        setup();
        let fun = make_test_fun_bc(qstr::from_str("g"));
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        let kind = gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(kind, VmReturnKind::Normal);
        assert_eq!(ret, obj::CONST_NONE);
        let inst = unsafe { &*gen_instance_ptr(gen) };
        assert!(inst.code_state.ip.is_null());
    }

    #[test]
    fn gen_instance_close_on_finished() {
        setup();
        let fun = make_test_fun_bc(qstr::from_str("g"));
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        let mut ret = obj::OBJ_NULL;
        gen_resume(gen, obj::CONST_NONE, obj::OBJ_NULL, &mut ret);
        assert_eq!(gen_instance_close(gen), obj::CONST_NONE);
    }

    #[test]
    fn pend_throw_stores_exception() {
        setup();
        let fun = make_test_fun_bc(qstr::from_str("g"));
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        let exc = objexcept::new_exception(objexcept::type_value_error());
        let prev = gen_instance_pend_throw(gen, exc);
        assert_eq!(prev, obj::CONST_NONE);
        let inst = unsafe { &*gen_instance_ptr(gen) };
        assert_eq!(inst.pend_exc, exc);
    }

    #[test]
    fn const_generator_exit_is_exception() {
        setup();
        let ge = const_generator_exit();
        assert!(objexcept::is_native_exception_instance(ge));
        assert!(objexcept::is_exception_instance(ge));
    }

    #[test]
    fn gen_instance_print_shows_name() {
        setup();
        let name = qstr::from_str("mygen");
        let fun = make_test_fun_bc(name);
        let gen = gen_wrap_call(fun, 0, 0, &[]);
        let mut out = Vec::new();
        let mut print = Print {
            data: &mut out as *mut Vec<u8> as *mut (),
            print_strn: Some(collect_print),
        };
        gen_instance_print(&print, gen, PrintKind::Repr);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("generator object"));
        assert!(s.contains("mygen"));
    }

    extern "C" fn collect_print(data: *mut (), str: *const u8, len: usize) {
        let out = unsafe { &mut *(data as *mut Vec<u8>) };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(str, len) });
    }
}
