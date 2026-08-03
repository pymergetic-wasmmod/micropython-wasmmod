//! rewrite of py/objfun.c + py/objfun.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::asmbase;
use crate::bc::{
    decode_code_state_size, decode_uint_value, prelude_sig_decode_into, prelude_size_decode,
    setup_code_state, CodeState, ModuleContext, ObjFunBc,
};
use crate::bc0;
use crate::cstack;
use crate::gc;
use crate::map::{self, LookupKind, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::mpstate;
use crate::obj::{
    self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
};
use crate::objdict::ObjDict;
use crate::objexcept;
use crate::objtuple::{self, ObjTuple};
use crate::qstr::{self, Qstr};
use crate::raise;
use crate::runtime;
use crate::runtime::VmReturnKind;
use crate::vm;

pub use crate::bc::{ExcStack, ModuleConstants, ObjModule};

/// `mp_fun_*_t` / `mp_obj_fun_builtin_fixed_t`.
#[repr(C)]
pub struct ObjFunBuiltinFixed {
    pub base: ObjBase,
    pub fun: ObjFunBuiltinFixedFun,
}

#[repr(C)]
pub union ObjFunBuiltinFixedFun {
    pub f0: BuiltinFn0,
    pub f1: BuiltinFn1,
    pub f2: BuiltinFn2,
    pub f3: BuiltinFn3,
}

pub type BuiltinFn0 = fn() -> Obj;
pub type BuiltinFn1 = fn(Obj) -> Obj;
pub type BuiltinFn2 = fn(Obj, Obj) -> Obj;
pub type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
pub type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
pub type BuiltinFnKw = fn(usize, &[Obj], &map::Map) -> Obj;

/// `mp_obj_fun_builtin_var_t`.
#[repr(C)]
pub struct ObjFunBuiltinVar {
    pub base: ObjBase,
    pub sig: u32,
    pub fun: ObjFunBuiltinVarFun,
}

#[repr(C)]
pub union ObjFunBuiltinVarFun {
    pub var: BuiltinFnVar,
    pub kw: BuiltinFnKw,
}

/// `mp_obj_fun_asm_t` (header parity; inline asm disabled via `EMIT_INLINE_ASM`).
#[repr(C)]
pub struct ObjFunAsm {
    pub base: ObjBase,
    pub n_args: usize,
    pub fun_data: *const (),
    pub type_sig: usize,
}

const VM_MAX_STATE_ON_STACK: usize = size_of::<usize>() * 11;

macro_rules! fun_builtin_type {
    ($name:ident, $slots:ident) => {
        static mut $slots: [*const (); 1] = [core::ptr::null()];
        static mut $name: ObjType = ObjType {
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
            slots: core::ptr::null(),
        };
    };
}

fun_builtin_type!(TYPE_FUN_BUILTIN_0, FUN_BUILTIN_0_SLOTS);
fun_builtin_type!(TYPE_FUN_BUILTIN_1, FUN_BUILTIN_1_SLOTS);
fun_builtin_type!(TYPE_FUN_BUILTIN_2, FUN_BUILTIN_2_SLOTS);
fun_builtin_type!(TYPE_FUN_BUILTIN_3, FUN_BUILTIN_3_SLOTS);
fun_builtin_type!(TYPE_FUN_BUILTIN_VAR, FUN_BUILTIN_VAR_SLOTS);

static mut FUN_BC_SLOTS: [*const (); 4] = [core::ptr::null(); 4];
static mut TYPE_FUN_BC: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
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

static mut FUN_NATIVE_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_FUN_NATIVE: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 0,
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

static mut FUN_VIPER_SLOTS: [*const (); 1] = [core::ptr::null()];
static mut TYPE_FUN_VIPER: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF,
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

static mut FUN_ASM_SLOTS: [*const (); 1] = [core::ptr::null()];
static mut TYPE_FUN_ASM: ObjType = ObjType {
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
    slots: core::ptr::null(),
};

static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_types() {
    INIT.get_or_init(|| {
        let name = qstr::from_str("function");
        unsafe {
            FUN_BUILTIN_0_SLOTS[0] = fun_builtin_0_call as *const ();
            FUN_BUILTIN_1_SLOTS[0] = fun_builtin_1_call as *const ();
            FUN_BUILTIN_2_SLOTS[0] = fun_builtin_2_call as *const ();
            FUN_BUILTIN_3_SLOTS[0] = fun_builtin_3_call as *const ();
            FUN_BUILTIN_VAR_SLOTS[0] = fun_builtin_var_call as *const ();
            TYPE_FUN_BUILTIN_0.slots = FUN_BUILTIN_0_SLOTS.as_ptr();
            TYPE_FUN_BUILTIN_1.slots = FUN_BUILTIN_1_SLOTS.as_ptr();
            TYPE_FUN_BUILTIN_2.slots = FUN_BUILTIN_2_SLOTS.as_ptr();
            TYPE_FUN_BUILTIN_3.slots = FUN_BUILTIN_3_SLOTS.as_ptr();
            TYPE_FUN_BUILTIN_VAR.slots = FUN_BUILTIN_VAR_SLOTS.as_ptr();
            TYPE_FUN_BUILTIN_0.name = name;
            TYPE_FUN_BUILTIN_1.name = name;
            TYPE_FUN_BUILTIN_2.name = name;
            TYPE_FUN_BUILTIN_3.name = name;
            TYPE_FUN_BUILTIN_VAR.name = name;

            TYPE_FUN_BC.slots = FUN_BC_SLOTS.as_ptr();
            TYPE_FUN_BC.name = name;

            let mut next_slot = 0usize;
            if mpconfig::PY_FUNCTION_ATTRS_CODE {
                FUN_BC_SLOTS[next_slot] = fun_bc_make_new as *const ();
                TYPE_FUN_BC.slot_index_make_new = (next_slot + 1) as u8;
                next_slot += 1;
            }
            if mpconfig::CPYTHON_COMPAT {
                FUN_BC_SLOTS[next_slot] = fun_bc_print as *const ();
                TYPE_FUN_BC.slot_index_print = (next_slot + 1) as u8;
                next_slot += 1;
            }
            if mpconfig::PY_FUNCTION_ATTRS {
                FUN_BC_SLOTS[next_slot] = fun_bc_attr as *const ();
                TYPE_FUN_BC.slot_index_attr = (next_slot + 1) as u8;
                next_slot += 1;
            }
            FUN_BC_SLOTS[next_slot] = fun_bc_call as *const ();
            TYPE_FUN_BC.slot_index_call = (next_slot + 1) as u8;

            if mpconfig::ENABLE_NATIVE_CODE {
                TYPE_FUN_NATIVE.name = name;
                TYPE_FUN_VIPER.name = name;
                let mut native_slot = 0usize;
                if mpconfig::CPYTHON_COMPAT {
                    FUN_NATIVE_SLOTS[native_slot] = fun_bc_print as *const ();
                    TYPE_FUN_NATIVE.slot_index_print = (native_slot + 1) as u8;
                    native_slot += 1;
                }
                if mpconfig::PY_FUNCTION_ATTRS {
                    FUN_NATIVE_SLOTS[native_slot] = fun_bc_attr as *const ();
                    TYPE_FUN_NATIVE.slot_index_attr = (native_slot + 1) as u8;
                    native_slot += 1;
                }
                FUN_NATIVE_SLOTS[native_slot] = fun_native_call as *const ();
                TYPE_FUN_NATIVE.slot_index_call = (native_slot + 1) as u8;
                TYPE_FUN_NATIVE.slots = FUN_NATIVE_SLOTS.as_ptr();

                FUN_VIPER_SLOTS[0] = fun_viper_call as *const ();
                TYPE_FUN_VIPER.slot_index_call = 1;
                TYPE_FUN_VIPER.slots = FUN_VIPER_SLOTS.as_ptr();

                TYPE_FUN_ASM.name = name;
                FUN_ASM_SLOTS[0] = fun_asm_call as *const ();
                TYPE_FUN_ASM.slot_index_call = 1;
                TYPE_FUN_ASM.slots = FUN_ASM_SLOTS.as_ptr();
            }
        }
    });
}

pub fn type_fun_builtin_0() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BUILTIN_0 }
}

pub fn type_fun_builtin_1() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BUILTIN_1 }
}

pub fn type_fun_builtin_2() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BUILTIN_2 }
}

pub fn type_fun_builtin_3() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BUILTIN_3 }
}

pub fn type_fun_builtin_var() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BUILTIN_VAR }
}

pub fn type_fun_bc() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_BC }
}

pub fn type_fun_native() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_NATIVE }
}

pub fn type_fun_viper() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_VIPER }
}

pub fn type_fun_asm() -> &'static ObjType {
    init_types();
    unsafe { &TYPE_FUN_ASM }
}

fn fun_builtin_0_call(self_in: Obj, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    debug_assert!(obj::is_exact_type(self_in, type_fun_builtin_0()));
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 0, 0, false);
    unsafe { (self_.fun.f0)() }
}

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(obj::is_exact_type(self_in, type_fun_builtin_1()));
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    unsafe { (self_.fun.f1)(args[0]) }
}

fn fun_builtin_2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(obj::is_exact_type(self_in, type_fun_builtin_2()));
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    unsafe { (self_.fun.f2)(args[0], args[1]) }
}

fn fun_builtin_3_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(obj::is_exact_type(self_in, type_fun_builtin_3()));
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 3, 3, false);
    unsafe { (self_.fun.f3)(args[0], args[1], args[2]) }
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(obj::is_exact_type(self_in, type_fun_builtin_var()));
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num_sig(n_args, n_kw, self_.sig);
    if self_.sig & 1 != 0 {
        let mut kw_args = map::Map::default();
        map::init(&mut kw_args, n_kw);
        for i in 0..n_kw {
            let key = args[n_args + i * 2];
            let val = args[n_args + i * 2 + 1];
            if let Some(slot) = map::lookup(&mut kw_args, key, LookupKind::AddIfNotFound) {
                slot.value = val;
            }
        }
        unsafe { (self_.fun.kw)(n_args, args, &kw_args) }
    } else {
        unsafe { (self_.fun.var)(n_args, args) }
    }
}

/// `mp_obj_fun_bc_get_name`
pub fn fun_bc_get_name(fun: &ObjFunBc) -> Qstr {
    let mut bc = fun.bytecode;
    if mpconfig::ENABLE_NATIVE_CODE {
        let ty = unsafe { &*fun.base.type_ };
        if core::ptr::eq(ty, type_fun_native()) {
            bc = fun_native_get_prelude_ptr(fun);
        }
    }
    let mut ip = bc;
    let _sig = prelude_sig_decode_into(&mut ip);
    let (_n_info, _n_cell) = prelude_size_decode(&mut ip);
    let mut name = decode_uint_value(ip) as Qstr;
    if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        let ctx = unsafe { &*fun.context };
        name = ctx.qstr_table()[name];
    }
    name
}

fn fun_bc_extra_args(fun: &ObjFunBc, n: usize) -> &mut [Obj] {
    unsafe {
        std::slice::from_raw_parts_mut((fun as *const ObjFunBc).add(1) as *mut Obj, n)
    }
}

fn init_code_state(
    code_state: &mut CodeState,
    fun: *mut ObjFunBc,
    n_state: usize,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) {
    code_state.fun_bc = fun;
    code_state.n_state = n_state as u16;
    setup_code_state(code_state, n_args, n_kw, args);
    code_state.old_globals = mpstate::globals_get().0 as *mut ObjDict;
}

fn fun_bc_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    cstack::check();

    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjFunBc) };
    let (n_state, state_size) = decode_code_state_size(self_.bytecode);
    let total = size_of::<CodeState>() + state_size;

    if state_size > VM_MAX_STATE_ON_STACK {
        let mut heap_ptr = gc::alloc(total, size_of::<CodeState>());
        if heap_ptr.is_none() {
            return fun_bc_call_with_buffer(
                self_,
                n_state,
                n_args,
                n_kw,
                args,
                &mut [0u8; VM_MAX_STATE_ON_STACK + size_of::<CodeState>()],
            );
        }
        let ptr = heap_ptr.take().unwrap();
        let result = fun_bc_call_with_state(self_, n_state, n_args, n_kw, args, ptr as *mut CodeState);
        gc::free(ptr);
        result
    } else {
        fun_bc_call_with_buffer(
            self_,
            n_state,
            n_args,
            n_kw,
            args,
            &mut [0u8; VM_MAX_STATE_ON_STACK + size_of::<CodeState>()],
        )
    }
}

fn fun_bc_call_with_buffer(
    self_: &mut ObjFunBc,
    n_state: usize,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
    buf: &mut [u8],
) -> Obj {
    assert!(buf.len() >= size_of::<CodeState>());
    let code_state = buf.as_mut_ptr() as *mut CodeState;
    fun_bc_call_with_state(self_, n_state, n_args, n_kw, args, code_state)
}

fn fun_bc_call_with_state(
    self_: &mut ObjFunBc,
    n_state: usize,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
    code_state: *mut CodeState,
) -> Obj {
    unsafe {
        init_code_state(&mut *code_state, self_ as *mut ObjFunBc, n_state, n_args, n_kw, args);
        let ctx = &*self_.context;
        mpstate::globals_set(obj::from_ptr(ctx.module.globals as *const ()));
        let vm_return_kind = vm::execute_bytecode(&mut *code_state, obj::OBJ_NULL);
        mpstate::globals_set(obj::from_ptr((*code_state).old_globals as *const ()));

        let result = if vm_return_kind == VmReturnKind::Normal {
            *(*code_state).sp
        } else {
            debug_assert!(vm_return_kind == VmReturnKind::Exception);
            *(*code_state).state_ptr()
        };

        if vm_return_kind == VmReturnKind::Normal {
            result
        } else {
            raise::raise_obj(result);
        }
    }
}

fn fun_bc_make_new(_type_: &ObjType, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    if mpconfig::PY_FUNCTION_ATTRS_CODE {
        argcheck::check_num(n_args, n_kw, 2, 2, false);
        raise::raise(raise::MpRaise::TypeError("code object required"));
    }
    raise::raise(raise::MpRaise::TypeError("code object required"));
}

fn fun_bc_print(print: &Print, o_in: Obj, _kind: PrintKind) {
    let o = unsafe { &*(obj::as_ptr(o_in) as *const ObjFunBc) };
    mpprint::print_str(print, "<function ");
    if let Some(name) = qstr::str_from_qstr(fun_bc_get_name(o)) {
        mpprint::print_str(print, &name);
        mpprint::print_str(print, " ");
    }
    mpprint::print_str(print, &format!("at {o_in:?}>"));
}

/// `mp_obj_fun_bc_attr`
pub fn fun_bc_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBc) };
    if attr == qstr::from_str("__name__") {
        dest[0] = obj::new_qstr(fun_bc_get_name(self_));
    }
    if attr == qstr::from_str("__globals__") {
        dest[0] = obj::from_ptr(unsafe { (*self_.context).module.globals as *const () });
    }
}

/// `mp_obj_new_fun_bc`
pub fn new_fun_bc(
    def_args: Option<&[Obj; 2]>,
    code: *const u8,
    context: *const ModuleContext,
    child_table: *const *const (),
) -> Obj {
    let mut n_def_args = 0usize;
    let mut n_extra_args = 0usize;
    let mut def_pos_args: Option<*const ObjTuple> = None;
    let mut def_kw_args = obj::OBJ_NULL;

    if let Some(def_args) = def_args {
        if def_args[0] != obj::OBJ_NULL {
            debug_assert!(obj::is_exact_type(def_args[0], objtuple::type_tuple()));
            def_pos_args = Some(obj::as_ptr(def_args[0]) as *const ObjTuple);
            n_def_args = unsafe { (*def_pos_args.unwrap()).len };
            n_extra_args = n_def_args;
        }
        if def_args[1] != obj::OBJ_NULL {
            debug_assert!(obj::is_dict_or_ordereddict(def_args[1]));
            def_kw_args = def_args[1];
            n_extra_args += 1;
        }
    }

    let o = obj::malloc_var::<ObjFunBc>(n_extra_args * size_of::<Obj>(), type_fun_bc()) as *mut ObjFunBc;
    unsafe {
        (*o).bytecode = code;
        (*o).context = context;
        (*o).child_table = child_table;
        if let Some(tuple) = def_pos_args {
            let (_len, items) = objtuple::tuple_get(obj::from_ptr(tuple as *const ()));
            fun_bc_extra_args(&*o, n_def_args)[..n_def_args].copy_from_slice(&items[..n_def_args]);
        }
        if def_kw_args != obj::OBJ_NULL {
            fun_bc_extra_args(&*o, n_extra_args)[n_def_args] = def_kw_args;
        }
        obj::from_ptr(o as *const ObjFunBc as *const ())
    }
}


fn raise_native_dispatch_unsupported() -> ! {
    raise::raise_obj(objexcept::new_exception(objexcept::type_not_implemented_error()));
}

fn dispatch_native_code(
    code: *const (),
    self_in: Obj,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) -> Obj {
    if !asmbase::machine_code_dispatch_supported() {
        raise_native_dispatch_unsupported();
    }
    let callable = mpconfig::make_pointer_callable(code);
    let args_ptr = args.as_ptr();
    type NativeCallFn = extern "C-unwind" fn(Obj, usize, usize, *const Obj) -> Obj;
    let mut nlr_buf = crate::nlr::NlrBuf::default();
    match crate::nlr::protect(&mut nlr_buf, || unsafe {
        (core::mem::transmute::<*const (), NativeCallFn>(callable))(
            self_in,
            n_args,
            n_kw,
            args_ptr,
        )
    }) {
        Ok(v) => v,
        Err(v) => crate::raise::raise_obj(Obj(v)),
    }
}

fn fun_native_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    cstack::check();
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBc) };
    dispatch_native_code(
        fun_native_get_function_start(self_),
        self_in,
        n_args,
        n_kw,
        args,
    )
}

fn fun_viper_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    cstack::check();
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBc) };
    dispatch_native_code(
        self_.bytecode as *const (),
        self_in,
        n_args,
        n_kw,
        args,
    )
}

fn fun_asm_call(_self_in: Obj, _n_args: usize, _n_kw: usize, _args: &[Obj]) -> Obj {
    raise_native_dispatch_unsupported();
}

/// `mp_obj_fun_native_get_prelude_ptr`
pub fn fun_native_get_prelude_ptr(fun_native: &ObjFunBc) -> *const u8 {
    let prelude_ptr_index =
        unsafe { fun_native.bytecode.cast::<usize>().read_unaligned() };
    if prelude_ptr_index == 0 {
        fun_native.child_table as *const u8
    } else {
        unsafe {
            fun_native
                .child_table
                .add(prelude_ptr_index)
                .cast::<*const u8>()
                .read_unaligned()
        }
    }
}

/// `mp_obj_fun_native_get_function_start`
pub fn fun_native_get_function_start(fun_native: &ObjFunBc) -> *const () {
    unsafe { fun_native.bytecode.add(size_of::<usize>()) as *const () }
}

/// `mp_obj_fun_native_get_generator_start`
pub fn fun_native_get_generator_start(fun_native: &ObjFunBc) -> *const () {
    let start_offset = unsafe { fun_native.bytecode.add(size_of::<usize>()).cast::<usize>().read_unaligned() };
    mpconfig::make_pointer_callable(unsafe { fun_native.bytecode.add(start_offset) as *const () })
}

/// `mp_obj_fun_native_get_generator_resume`
pub fn fun_native_get_generator_resume(fun_native: &ObjFunBc) -> *const () {
    mpconfig::make_pointer_callable(
        unsafe { fun_native.bytecode.add(2 * size_of::<usize>()) as *const () },
    )
}

/// `mp_obj_new_fun_native`
pub fn new_fun_native(
    def_args: Option<&[Obj; 2]>,
    fun_data: *const (),
    context: *const ModuleContext,
    child_table: *const *const (),
) -> Obj {
    let o = new_fun_bc(def_args, fun_data as *const u8, context, child_table);
    unsafe {
        (*(obj::as_ptr(o) as *mut ObjFunBc)).base.type_ = type_fun_native() as *const ObjType;
    }
    o
}

/// `mp_obj_new_fun_viper`
pub fn new_fun_viper(
    fun_data: *const (),
    context: *const ModuleContext,
    child_table: *const *const (),
) -> Obj {
    let o = obj::malloc_var::<ObjFunBc>(0, type_fun_viper()) as *mut ObjFunBc;
    unsafe {
        (*o).bytecode = fun_data as *const u8;
        (*o).context = context;
        (*o).child_table = child_table;
        obj::from_ptr(o as *const ObjFunBc as *const ())
    }
}

/// `mp_obj_new_fun_asm`
pub fn new_fun_asm(n_args: usize, fun_data: *const (), type_sig: usize) -> Obj {
    let o = obj::malloc_var::<ObjFunAsm>(0, type_fun_asm()) as *mut ObjFunAsm;
    unsafe {
        (*o).n_args = n_args;
        (*o).fun_data = fun_data;
        (*o).type_sig = type_sig;
        obj::from_ptr(o as *const ObjFunAsm as *const ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitglue;
    use crate::gc;
    use crate::mpstate;

    fn setup() {
        let _ = gc::init();
        runtime::init();
        init_types();
    }

    fn encode_uint(mut n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (n & 0x7f) as u8;
            n >>= 7;
            if n != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if n == 0 {
                break;
            }
        }
        out
    }

    fn minimal_bytecode(name_idx: usize) -> Vec<u8> {
        let mut bc = vec![0x08u8, 0x00u8];
        bc.extend(encode_uint(name_idx));
        bc.push(bc0::LOAD_CONST_NONE);
        bc.push(bc0::RETURN_VALUE);
        bc
    }

    fn test_context(name_qstr: Qstr) -> (*const ModuleContext, Obj) {
        let globals = crate::objdict::new_dict(0);
        mpstate::globals_set(globals);
        let globals_ptr = obj::as_ptr(globals) as *mut ObjDict;
        let ctx = Box::leak(Box::new(ModuleContext {
            module: ObjModule {
                base: ObjBase { type_: core::ptr::null() },
                globals: globals_ptr,
            },
            constants: ModuleConstants::default(),
            n_qstr: 0,
            n_obj: 0,
        }));
        emitglue::module_context_alloc_tables(ctx, 1, 0);
        ctx.qstr_table_mut()[0] = name_qstr;
        (ctx, globals)
    }

    fn test_fun(name: &str) -> Obj {
        let name_qstr = qstr::from_str(name);
        let (ctx, _) = test_context(name_qstr);
        let bc = Box::leak(minimal_bytecode(0).into_boxed_slice());
        new_fun_bc(None, bc.as_ptr(), ctx, core::ptr::null())
    }

    #[test]
    fn fun_bc_get_name_reads_prelude() {
        setup();
        let fun = test_fun("hello");
        let fun_ptr = obj::as_ptr(fun) as *const ObjFunBc;
        assert_eq!(fun_bc_get_name(unsafe { &*fun_ptr }), qstr::from_str("hello"));
    }

    #[test]
    fn fun_bc_attr_name_and_globals() {
        setup();
        let name_qstr = qstr::from_str("f");
        let (ctx, globals) = test_context(name_qstr);
        let bc = Box::leak(minimal_bytecode(0).into_boxed_slice());
        let fun = new_fun_bc(None, bc.as_ptr(), ctx, core::ptr::null());
        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        fun_bc_attr(fun, qstr::from_str("__name__"), &mut dest);
        assert_eq!(dest[0], obj::new_qstr(qstr::from_str("f")));
        dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        fun_bc_attr(fun, qstr::from_str("__globals__"), &mut dest);
        assert_eq!(dest[0], globals);
    }

    #[test]
    fn fun_bc_call_runs_vm_stub() {
        setup();
        let fun = test_fun("g");
        let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
        assert_eq!(result, obj::CONST_NONE);
    }

    #[test]
    fn new_fun_bc_default_args() {
        setup();
        let name_qstr = qstr::from_str("h");
        let (ctx, _) = test_context(name_qstr);
        let bc = Box::leak(minimal_bytecode(0).into_boxed_slice());
        let default = obj::new_small_int(42);
        let def_tuple = objtuple::new_tuple(1, Some(&[default]));
        let fun = new_fun_bc(Some(&[def_tuple, obj::OBJ_NULL]), bc.as_ptr(), ctx, core::ptr::null());
        let fun_ptr = unsafe { &*(obj::as_ptr(fun) as *const ObjFunBc) };
        assert_eq!(fun_bc_extra_args(fun_ptr, 1)[0], default);
    }

    #[test]
    fn fun_builtin_0_call() {
        setup();
        fn f0() -> Obj {
            obj::new_small_int(0)
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinFixed>(), type_fun_builtin_0()) as *mut ObjFunBuiltinFixed;
        unsafe {
            (*o).fun.f0 = f0;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ());
            assert_eq!(runtime::call_function_n_kw(fun, 0, 0, &[]), obj::new_small_int(0));
        }
    }

    #[test]
    fn fun_builtin_1_call() {
        setup();
        fn f1(x: Obj) -> Obj {
            x
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinFixed>(), type_fun_builtin_1()) as *mut ObjFunBuiltinFixed;
        unsafe {
            (*o).fun.f1 = f1;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ());
            let arg = obj::new_small_int(7);
            assert_eq!(runtime::call_function_n_kw(fun, 1, 0, &[arg]), arg);
        }
    }

    #[test]
    fn fun_builtin_2_call() {
        setup();
        fn f2(a: Obj, b: Obj) -> Obj {
            obj::new_small_int(obj::get_int(a) + obj::get_int(b))
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinFixed>(), type_fun_builtin_2()) as *mut ObjFunBuiltinFixed;
        unsafe {
            (*o).fun.f2 = f2;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ());
            let result = runtime::call_function_n_kw(fun, 2, 0, &[obj::new_small_int(3), obj::new_small_int(4)]);
            assert_eq!(obj::get_int(result), 7);
        }
    }

    #[test]
    fn fun_builtin_3_call() {
        setup();
        fn f3(a: Obj, b: Obj, c: Obj) -> Obj {
            obj::new_small_int(obj::get_int(a) + obj::get_int(b) + obj::get_int(c))
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinFixed>(), type_fun_builtin_3()) as *mut ObjFunBuiltinFixed;
        unsafe {
            (*o).fun.f3 = f3;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ());
            let result = runtime::call_function_n_kw(
                fun,
                3,
                0,
                &[obj::new_small_int(1), obj::new_small_int(2), obj::new_small_int(3)],
            );
            assert_eq!(obj::get_int(result), 6);
        }
    }

    #[test]
    fn fun_builtin_var_call() {
        setup();
        fn fvar(n: usize, args: &[Obj]) -> Obj {
            obj::new_small_int(n as isize + obj::get_int(args[0]))
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinVar>(), type_fun_builtin_var()) as *mut ObjFunBuiltinVar;
        unsafe {
            (*o).sig = argcheck::make_sig(1, 2, false);
            (*o).fun.var = fvar;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinVar as *const ());
            let r = runtime::call_function_n_kw(fun, 2, 0, &[obj::new_small_int(10), obj::new_small_int(5)]);
            assert_eq!(obj::get_int(r), 12);
        }
    }

    #[test]
    fn fun_builtin_var_kw_call() {
        setup();
        fn fkw(_n: usize, _args: &[Obj], kw: &map::Map) -> Obj {
            kw.table[0].value
        }
        let o = obj::malloc_helper(size_of::<ObjFunBuiltinVar>(), type_fun_builtin_var()) as *mut ObjFunBuiltinVar;
        unsafe {
            (*o).sig = argcheck::make_sig(0, 0xffff, true);
            (*o).fun.kw = fkw;
            let fun = obj::from_ptr(o as *const ObjFunBuiltinVar as *const ());
            let key = obj::new_qstr(qstr::from_str("x"));
            let val = obj::new_small_int(99);
            let r = runtime::call_function_n_kw(fun, 0, 1, &[key, val]);
            assert_eq!(obj::get_int(r), 99);
        }
    }

    #[test]
    fn type_fun_names_are_function() {
        setup();
        let name = qstr::from_str("function");
        assert_eq!(type_fun_bc().name, name);
        assert_eq!(type_fun_builtin_0().name, name);
    }

    #[test]
    fn native_fun_types_and_helpers() {
        setup();
        let name_qstr = qstr::from_str("native_fn");
        let (ctx, _) = test_context(name_qstr);
        let bc = Box::leak(minimal_bytecode(0).into_boxed_slice());
        let child_table = Box::leak(Box::new([bc.as_ptr() as *const ()]));
        let native_data = Box::leak(Box::new([0usize, bc.as_ptr() as usize]));
        let fun = new_fun_native(
            None,
            native_data.as_ptr() as *const (),
            ctx,
            child_table.as_ptr(),
        );
        assert!(obj::is_exact_type(fun, type_fun_native()));
        let fun_ptr = unsafe { &*(obj::as_ptr(fun) as *const ObjFunBc) };
        assert_eq!(
            fun_native_get_prelude_ptr(fun_ptr),
            child_table.as_ptr() as *const u8
        );
        if asmbase::machine_code_dispatch_supported() {
            let (code, _size) = emit_trivial_native_return_none();
            let fun = new_fun_native(
                None,
                code as *const (),
                ctx,
                child_table.as_ptr(),
            );
            let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
            assert_eq!(result, obj::CONST_NONE);
        } else {
            let mut nlr_buf = crate::nlr::NlrBuf::default();
            let err =
                crate::nlr::protect(&mut nlr_buf, || runtime::call_function_n_kw(fun, 0, 0, &[]));
            assert!(err.is_err(), "native call should raise when dispatch is gated");
            let exc = Obj(err.unwrap_err());
            assert!(objexcept::exception_match(
                exc,
                obj::from_ptr(objexcept::type_not_implemented_error() as *const ObjType as *const ()),
            ));
        }
    }

    #[test]
    fn module_context_native_offsets() {
        use crate::bc::{ModuleConstants, ModuleContext, ObjModule};
        assert_eq!(core::mem::offset_of!(ModuleContext, module), 0);
        assert_eq!(core::mem::offset_of!(ModuleContext, constants), 16);
        assert_eq!(core::mem::offset_of!(ModuleConstants, qstr_table), 0);
        assert_eq!(core::mem::offset_of!(ModuleConstants, obj_table), 8);
        assert_eq!(core::mem::offset_of!(ModuleContext, constants.obj_table), 24);
        assert_eq!(core::mem::size_of::<ModuleConstants>(), 16);
        assert_eq!(core::mem::size_of::<ObjModule>(), 16);
        assert_eq!(
            core::mem::offset_of!(ModuleContext, constants.obj_table) / core::mem::size_of::<usize>(),
            3,
        );
    }

    fn emit_native_return_small_int_impl(
        value: i64,
        emit_options: u16,
        scope_flags: u16,
        name: &str,
    ) -> Obj {
        use crate::bc::{ModuleConstants, ModuleContext, ObjModule};
        use crate::emit::{self, EmitCommon, PassKind};
        use crate::emitglue;
        use crate::emitnx64;
        use crate::malloc;
        use crate::nativeglue;
        use crate::objdict;
        use crate::parse::PARSE_NODE_NULL;
        use crate::scope::{self, ScopeKind};

        nativeglue::init_fun_table_extras();

        let name_qstr = qstr::from_str(name);
        let ctx = Box::leak(Box::new(ModuleContext {
            module: crate::bc::ObjModule {
                base: ObjBase { type_: core::ptr::null() },
                globals: objdict::dict_ptr(mpstate::globals_get()),
            },
            constants: ModuleConstants::default(),
            n_qstr: 0,
            n_obj: 0,
        }));
        emitglue::module_context_alloc_tables(ctx, 1, 1);
        ctx.qstr_table_mut()[0] = name_qstr;
        ctx.obj_table_mut()[0] = Obj(nativeglue::fun_table_reloc_base());

        let scope = malloc::new_obj::<scope::Scope>().expect("scope");
        unsafe {
            (*scope).kind = ScopeKind::Function;
            (*scope).pn = PARSE_NODE_NULL;
            (*scope).simple_name = name_qstr;
            (*scope).raw_code = emitglue::new_raw_code();
            (*scope).emit_options = emit_options;
            (*scope).scope_flags = scope_flags;
            (*scope).num_pos_args = 0;
            (*scope).num_kwonly_args = 0;
            (*scope).num_def_pos_args = 0;
            (*scope).num_locals = 0;
            (*scope).stack_size = 0;
            (*scope).exc_stack_size = 0;
            (*scope).id_info = Vec::new();
            (*scope).parent = None;
            (*scope).next = None;
        }

        let mut emit_common = EmitCommon {
            pass: PassKind::Scope,
            ct_cur_child: 0,
            children: core::ptr::null_mut(),
            qstr_map: map::Map::default(),
            const_obj_list: Vec::new(),
        };
        map::init(&mut emit_common.qstr_map, 1);
        let elem = map::lookup(
            &mut emit_common.qstr_map,
            obj::new_qstr(name_qstr),
            LookupKind::AddIfNotFound,
        )
        .expect("qstr");
        elem.value = obj::new_small_int(0);
        let _fun_table_off =
            emit::emit_common_use_const_obj(&mut emit_common, Obj(nativeglue::fun_table_reloc_base()));

        let mut compile_error = obj::OBJ_NULL;
        let mut next_label = 0usize;
        let emit = emitnx64::emit_native_x64_new(
            &mut emit_common as *mut _,
            &mut compile_error,
            &mut next_label,
            8,
        );

        for pass in [PassKind::StackSize, PassKind::CodeSize, PassKind::Emit] {
            emit_common.pass = pass;
            if pass == PassKind::CodeSize {
                emit_common.children = if emit_common.ct_cur_child == 0 {
                    core::ptr::null_mut()
                } else {
                    malloc::new(emit_common.ct_cur_child).unwrap()
                };
                emit_common.ct_cur_child = 0;
            }
            emitnx64::emit_native_x64_start_pass(emit, pass, scope);
            emitnx64::emit_native_x64_load_const_small_int(emit, value);
            emitnx64::emit_native_x64_return_value(emit);
            while !emitnx64::emit_native_x64_end_pass(emit) {}
        }

        assert_eq!(compile_error, obj::OBJ_NULL);

        let rc = unsafe { (*scope).raw_code as *const _ };
        let fun = emitglue::make_function_from_proto_fun(rc as *const _, ctx, None);
        let fun_ptr = unsafe { &*(obj::as_ptr(fun) as *const ObjFunBc) };
        assert!(core::ptr::eq(fun_ptr.context, ctx));
        let ctx_ref = unsafe { &*fun_ptr.context };
        assert_eq!(ctx_ref.n_obj, 1);
        assert_ne!(ctx_ref.obj_table()[0].0, 0);
        let result = runtime::call_function_n_kw(fun, 0, 0, &[]);
        emitnx64::emit_native_x64_free(emit);
        scope::free(scope);
        assert_eq!(obj::get_int(result), value as isize);
        fun
    }

    #[test]
    fn native_emit_return_const_int_e2e() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let fun = emit_native_return_small_int(42);
        assert!(obj::is_exact_type(fun, type_fun_native()));
    }

    #[test]
    fn viper_emit_return_const_int_e2e() {
        setup();
        if !asmbase::machine_code_dispatch_supported() {
            return;
        }
        let fun = emit_viper_return_small_int(42);
        assert!(obj::is_exact_type(fun, type_fun_viper()));
    }

    fn emit_viper_return_small_int(value: i64) -> Obj {
        use crate::emitnative::{self, EMIT_OPT_VIPER};
        use crate::nativeglue;

        emit_native_return_small_int_impl(
            value,
            EMIT_OPT_VIPER,
            (nativeglue::NATIVE_TYPE_INT as u16) << emitnative::MP_SCOPE_FLAG_VIPERRET_POS,
            "viper_const",
        )
    }

    fn emit_native_return_small_int(value: i64) -> Obj {
        use crate::emitglue::EMIT_OPT_NATIVE_PYTHON;

        emit_native_return_small_int_impl(value, EMIT_OPT_NATIVE_PYTHON, 0, "native_const")
    }

    fn emit_trivial_native_return_none() -> (*const u8, usize) {
        use crate::asmx64::{self, AsmX64, ASM_X64_REG_RAX};
        use crate::asmbase::{self, MpAsmBase, MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT};

        let mut asm = Box::new(AsmX64 {
            base: MpAsmBase {
                pass: 0,
                suppress: false,
                code_offset: 0,
                code_size: 0,
                code_base: core::ptr::null_mut(),
                max_num_labels: 0,
                label_offsets: core::ptr::null_mut(),
            },
            num_locals: 0,
        });
        asmbase::init(&mut asm.base, 4);

        for pass in [MP_ASM_PASS_COMPUTE, MP_ASM_PASS_EMIT] {
            asmbase::start_pass(&mut asm.base, pass as i32);
            asmbase::data(&mut asm.base, 8, 0);
            asmx64::entry(&mut asm, 0);
            asmx64::mov_i64_to_r64_optimised(&mut asm, obj::CONST_NONE.0 as i64, ASM_X64_REG_RAX);
            asmx64::exit(&mut asm);
        }

        let code = asm.base.get_code();
        let size = asm.base.get_code_size();
        Box::leak(asm);
        (code, size)
    }
}
