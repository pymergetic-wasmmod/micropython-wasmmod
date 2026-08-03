//! rewrite of py/runtime.h + py/runtime.c
// symmetry: done
use crate::cstack;
use crate::gc;
use crate::map::{self, LookupKind, Map};
use crate::misc;
use crate::mpconfig;
use crate::mpstate;
use crate::mpthread;
use crate::nlr::{self, NlrBuf};
use crate::obj::{
    self, Int, Obj, ObjBase, ObjIterBuf, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_ITER_IS_CUSTOM, TYPE_FLAG_ITER_IS_ITERNEXT, TYPE_FLAG_ITER_IS_STREAM,
};
use crate::objboundmeth;
use crate::objcomplex;
use crate::objdict::{self, ObjDict};
use crate::objexcept;
use crate::objfloat;
use crate::objgenerator;
use crate::objint_mpz;
use crate::objlist;
use crate::objmodule;
use crate::objstr;
use crate::objtuple;
use crate::objtype;
use crate::pystack;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::smallint;

// --- public result type for the parse smoke path --------------------------------

#[derive(Debug)]
pub enum RuntimeError {
    TypeError(&'static str),
    ValueError(&'static str),
    ZeroDivision,
    Overflow(&'static str),
}

impl RuntimeError {
    pub fn message(self) -> &'static str {
        match self {
            RuntimeError::TypeError(m) => m,
            RuntimeError::ValueError(m) => m,
            RuntimeError::ZeroDivision => "divide by zero",
            RuntimeError::Overflow(m) => m,
        }
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

/// VM return kind (`mp_vm_return_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VmReturnKind {
    Normal = 0,
    Yield = 1,
    Exception = 2,
}

/// Pending-event behaviour (`mp_handle_pending_behaviour_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HandlePendingBehaviour {
    CallbacksAndClearExceptions = 0,
    CallbacksAndExceptions = 1,
    CallbacksOnly = 2,
}

/// Prepared call arguments (`mp_call_args_t`).
pub struct CallArgs {
    pub fun: Obj,
    pub n_args: usize,
    pub n_kw: usize,
    pub n_alloc: usize,
    pub args: Vec<Obj>,
}

// --- getitem iterator (objgetitemiter.c) ----------------------------------------

// --- init / deinit --------------------------------------------------------------

/// `mp_init` — initialise GC, qstr pools, VM state, and main module dict.
pub fn init() {
    mpstate::init();
    gc::init();
    qstr::init();

    mpstate::set_pending_exception(obj::OBJ_NULL);
    mpstate::with_vm(|vm| {
        vm.sched_state = mpstate::SCHED_IDLE;
        vm.sched_idx = 0;
        vm.sched_len = 0;
        vm.mp_optimise_value = 0;
        vm.default_emit_opt = 0;
        vm.mp_verbose_flag = 0;
        vm.mp_module_builtins_override_dict = None;

        if mpconfig::PY_THREAD && mpconfig::PY_THREAD_GIL {
            mpthread::mutex_init(&mut vm.gil_mutex);
        }

        vm.mp_loaded_modules_dict = objdict::new_dict(mpconfig::LOADED_MODULES_DICT_SIZE as usize);
        vm.dict_main = objdict::new_dict(1);
        objdict::dict_store(
            vm.dict_main,
            obj::new_qstr(qstr::from_str("__name__")),
            obj::new_qstr(qstr::from_str("__main__")),
        );
    });

    let main_mod = mpstate::with_vm(|vm| vm.dict_main);
    mpstate::locals_set(main_mod);
    mpstate::globals_set(main_mod);

    cstack::init_with_sp_here(64 * 1024);

    if mpconfig::ENABLE_GC {
        // Keep VM/thread Obj roots alive across collection (C `gc_collect_start`).
        gc::register_collect_hook(mpstate::mark_gc_roots);
    }

    if mpconfig::ENABLE_NATIVE_CODE {
        crate::nativeglue::init_fun_table_extras();
    }
}

/// `mp_deinit`.
pub fn deinit() {}

/// Banner line used by the unix stub port.
pub fn banner_line() -> String {
    format!(
        "{} {} (obj_repr=A, gc_heap={} KiB)",
        mpconfig::IMPLEMENTATION_NAME,
        mpconfig::VERSION_STRING,
        mpconfig::GC_HEAP_SIZE / 1024
    )
}

// --- globals / locals -----------------------------------------------------------

pub fn locals_get() -> Obj {
    mpstate::locals_get()
}

pub fn locals_set(dict: Obj) {
    mpstate::locals_set(dict);
}

pub fn globals_get() -> Obj {
    mpstate::globals_get()
}

pub fn globals_set(dict: Obj) {
    mpstate::globals_set(dict);
}

pub fn globals_locals_set_from_nlr_jump_callback(globals: Obj, locals: Obj) {
    globals_set(globals);
    locals_set(locals);
}

pub fn call_function_1_from_nlr_jump_callback(f: fn()) {
    f();
}

// --- name / global load & store -------------------------------------------------

fn dict_lookup(dict: Obj, attr: Qstr) -> Option<Obj> {
    if dict == obj::OBJ_NULL {
        return None;
    }
    let map = unsafe { &mut (*(obj::as_ptr(dict) as *mut ObjDict)).map };
    map::lookup(map, obj::new_qstr(attr), LookupKind::Lookup).map(|e| e.value)
}

/// `mp_load_name`.
pub fn load_name(qst: Qstr) -> Obj {
    if locals_get() != globals_get() {
        if let Some(v) = dict_lookup(locals_get(), qst) {
            return v;
        }
    }
    load_global(qst)
}

/// `mp_load_global`.
pub fn load_global(qst: Qstr) -> Obj {
    if let Some(v) = dict_lookup(globals_get(), qst) {
        return v;
    }
    if mpconfig::CAN_OVERRIDE_BUILTINS {
        if let Some(bo) = mpstate::with_vm(|vm| vm.mp_module_builtins_override_dict) {
            if let Some(v) = dict_lookup(bo, qst) {
                return v;
            }
        }
        if let Some(dict) = crate::objmodule::registered_builtins_globals() {
            if let Some(v) = dict_lookup(dict, qst) {
                return v;
            }
        }
    }
    if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_NORMAL {
        raise::raise(MpRaise::NameError("name not defined"));
    }
    raise::raise(MpRaise::NameError("name not defined"));
}

/// `mp_load_build_class`.
pub fn load_build_class() -> Obj {
    if mpconfig::CAN_OVERRIDE_BUILTINS {
        if let Some(bo) = mpstate::with_vm(|vm| vm.mp_module_builtins_override_dict) {
            if let Some(v) = dict_lookup(bo, qstr::from_str("__build_class__")) {
                return v;
            }
        }
        if let Some(dict) = crate::objmodule::registered_builtins_globals() {
            if let Some(v) = dict_lookup(dict, qstr::from_str("__build_class__")) {
                return v;
            }
        }
    }
    crate::modbuiltins::builtin___build_class___obj()
}

/// `mp_store_name`.
pub fn store_name(qst: Qstr, value: Obj) {
    objdict::dict_store(locals_get(), obj::new_qstr(qst), value);
}

/// `mp_delete_name`.
pub fn delete_name(qst: Qstr) {
    let _ = objdict::dict_delete(locals_get(), obj::new_qstr(qst));
}

/// `mp_store_global`.
pub fn store_global(qst: Qstr, value: Obj) {
    objdict::dict_store(globals_get(), obj::new_qstr(qst), value);
}

/// `mp_delete_global`.
pub fn delete_global(qst: Qstr) {
    let _ = objdict::dict_delete(globals_get(), obj::new_qstr(qst));
}

// --- unary / binary operators ---------------------------------------------------

fn type_has_iternext(type_: &ObjType) -> bool {
    (type_.flags
        & (TYPE_FLAG_ITER_IS_ITERNEXT | TYPE_FLAG_ITER_IS_CUSTOM | TYPE_FLAG_ITER_IS_STREAM))
        != 0
}

/// Smoke-path unary op on small ints (`Result` API for parser constant folding).
pub fn unary_op(op: UnaryOp, v: Int) -> Result<Obj> {
    let out = match op {
        UnaryOp::Positive => v,
        UnaryOp::Negative => v.checked_neg().ok_or(RuntimeError::Overflow("neg"))?,
        UnaryOp::Invert => !v,
        UnaryOp::Not => i32::from(v == 0) as Int,
        UnaryOp::Bool => i32::from(v != 0) as Int,
        UnaryOp::Abs => v.abs(),
        _ => return Err(RuntimeError::TypeError("unary_op: op not in smoke path")),
    };
    if !smallint::fits(out) {
        return Err(RuntimeError::Overflow("small int range"));
    }
    Ok(obj::new_small_int(out))
}

pub fn binary_op(op: BinaryOp, lhs: Obj, rhs: Obj) -> Result<Obj> {
    if obj::is_small_int(lhs) && obj::is_small_int(rhs) {
        return binary_op_small_int(op, obj::small_int_value(lhs), obj::small_int_value(rhs));
    }
    Err(RuntimeError::TypeError(
        "binary_op: only small ints in smoke path",
    ))
}

fn binary_op_small_int(op: BinaryOp, a: Int, b: Int) -> Result<Obj> {
    let v = match op {
        BinaryOp::Add | BinaryOp::InplaceAdd => {
            a.checked_add(b).ok_or(RuntimeError::Overflow("add"))?
        }
        BinaryOp::Subtract | BinaryOp::InplaceSubtract => {
            a.checked_sub(b).ok_or(RuntimeError::Overflow("sub"))?
        }
        BinaryOp::Multiply | BinaryOp::InplaceMultiply => {
            a.checked_mul(b).ok_or(RuntimeError::Overflow("mul"))?
        }
        BinaryOp::FloorDivide | BinaryOp::InplaceFloorDivide => {
            if b == 0 {
                return Err(RuntimeError::ZeroDivision);
            }
            smallint::floor_divide(a, b)
        }
        BinaryOp::TrueDivide | BinaryOp::InplaceTrueDivide => {
            if !mpconfig::PY_BUILTINS_FLOAT {
                return Err(RuntimeError::TypeError("binary_op: op not in smoke path"));
            }
            if b == 0 {
                return Err(RuntimeError::ZeroDivision);
            }
            return Ok(objfloat::new_float(a as f64 / b as f64));
        }
        BinaryOp::Modulo | BinaryOp::InplaceModulo => {
            if b == 0 {
                return Err(RuntimeError::ZeroDivision);
            }
            smallint::modulo(a, b)
        }
        BinaryOp::Divmod => {
            if b == 0 {
                return Err(RuntimeError::ZeroDivision);
            }
            let quo = smallint::floor_divide(a, b);
            let rem = smallint::modulo(a, b);
            return Ok(objtuple::new_tuple(
                2,
                Some(&[obj::new_small_int(quo), obj::new_small_int(rem)]),
            ));
        }
        BinaryOp::Or | BinaryOp::InplaceOr => a | b,
        BinaryOp::Xor | BinaryOp::InplaceXor => a ^ b,
        BinaryOp::And | BinaryOp::InplaceAnd => a & b,
        BinaryOp::Lshift | BinaryOp::InplaceLshift => {
            if b < 0 {
                return Err(RuntimeError::ValueError("negative shift count"));
            }
            let shift = b as u32;
            if shift >= u32::BITS - 1 {
                return Err(RuntimeError::Overflow("shift"));
            }
            a.checked_shl(shift)
                .ok_or(RuntimeError::Overflow("shift"))?
        }
        BinaryOp::Rshift | BinaryOp::InplaceRshift => {
            if b < 0 {
                return Err(RuntimeError::ValueError("negative shift count"));
            }
            a >> (b as u32).min(u32::BITS - 1)
        }
        BinaryOp::Power | BinaryOp::InplacePower => {
            if b < 0 {
                return Err(RuntimeError::TypeError("negative power without float"));
            }
            a.checked_pow(b as u32)
                .ok_or(RuntimeError::Overflow("pow"))?
        }
        BinaryOp::Equal => return Ok(obj::new_bool(a == b)),
        BinaryOp::NotEqual => return Ok(obj::new_bool(a != b)),
        BinaryOp::Less => return Ok(obj::new_bool(a < b)),
        BinaryOp::More => return Ok(obj::new_bool(a > b)),
        BinaryOp::LessEqual => return Ok(obj::new_bool(a <= b)),
        BinaryOp::MoreEqual => return Ok(obj::new_bool(a >= b)),
        _ => return Err(RuntimeError::TypeError("binary_op: op not in smoke path")),
    };
    if !smallint::fits(v) {
        return Err(RuntimeError::Overflow("small int range"));
    }
    Ok(obj::new_small_int(v))
}

/// `mp_unary_op`.
pub fn unary_op_obj(op: UnaryOp, arg: Obj) -> Obj {
    if op == UnaryOp::Not {
        return obj::new_bool(!obj::is_true(arg));
    }
    if obj::is_small_int(arg) {
        return unary_op_small_int_obj(op, obj::small_int_value(arg));
    }
    if op == UnaryOp::Hash && obj::is_str_or_bytes(arg) {
        if obj::is_qstr(arg) {
            if let Some(data) = qstr::qstr_str(obj::qstr_value(arg)) {
                let h = qstr::compute_hash(&data);
                return obj::new_small_int(h as Int);
            }
        }
        return obj::new_small_int(arg.0 as Int);
    }
    if let Some(slot) = obj::type_get_unary_op(obj::get_type(arg)) {
        let result = slot(op, arg);
        if result != obj::OBJ_NULL {
            return result;
        }
    } else if op == UnaryOp::Hash {
        return obj::new_small_int(arg.0 as Int);
    }
    if op == UnaryOp::Bool {
        return obj::CONST_TRUE;
    }
    if matches!(
        op,
        UnaryOp::IntMaybe | UnaryOp::FloatMaybe | UnaryOp::ComplexMaybe
    ) {
        return obj::OBJ_NULL;
    }
    raise::raise(MpRaise::TypeError("unsupported type for operator"));
}

fn unary_op_small_int_obj(op: UnaryOp, v: Int) -> Obj {
    match op {
        UnaryOp::Bool => return obj::new_bool(v != 0),
        UnaryOp::Hash | UnaryOp::Positive | UnaryOp::IntMaybe => return obj::new_small_int(v),
        UnaryOp::Negative => {
            if v == smallint::MIN {
                return objint_mpz::new_int_from_ll(-(v as i64));
            }
            return obj::new_small_int(-v);
        }
        UnaryOp::Abs => {
            if v >= 0 {
                return obj::new_small_int(v);
            }
            if v == smallint::MIN {
                return objint_mpz::new_int_from_ll(-(v as i64));
            }
            return obj::new_small_int(-v);
        }
        UnaryOp::Invert => return obj::new_small_int(!v),
        _ => {}
    }
    match unary_op(op, v) {
        Ok(o) => o,
        Err(e) => raise::raise(MpRaise::TypeError(e.message())),
    }
}

/// `mp_binary_op`.
pub fn binary_op_obj(op: BinaryOp, lhs: Obj, rhs: Obj) -> Obj {
    if op == BinaryOp::Is {
        return obj::new_bool(lhs == rhs);
    }
    if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
        return obj::equal_not_equal(op, lhs, rhs);
    }
    if op == BinaryOp::ExceptionMatch {
        if objexcept::is_exception_type(rhs) {
            return obj::new_bool(objexcept::exception_match(lhs, rhs));
        }
        if obj::is_exact_type(rhs, obj::type_tuple()) {
            let (len, items) = objtuple::tuple_get(rhs);
            for i in 0..len {
                let item = items[i];
                if !objexcept::is_exception_type(item) {
                    raise::raise(MpRaise::TypeError("unsupported type for operator"));
                }
                if objexcept::exception_match(lhs, item) {
                    return obj::CONST_TRUE;
                }
            }
            return obj::CONST_FALSE;
        }
        raise::raise(MpRaise::TypeError("unsupported type for operator"));
    }

    if obj::is_small_int(lhs) {
        let lhs_val = obj::small_int_value(lhs);
        if obj::is_small_int(rhs) {
            return binary_op_small_int_obj(op, lhs_val, obj::small_int_value(rhs));
        }
        // Bool acts as 0/1 (C `mp_obj_int_binary_op_extra_cases`).
        if obj::is_bool(rhs) {
            return binary_op_small_int_obj(op, lhs_val, i32::from(obj::bool_value(rhs)) as Int);
        }
        if mpconfig::PY_BUILTINS_FLOAT && objfloat::is_float(rhs) {
            let res = objfloat::float_binary_op_val(op, lhs_val as f64, rhs);
            if res != obj::OBJ_NULL {
                return res;
            }
        }
        if mpconfig::PY_BUILTINS_COMPLEX && obj::is_exact_type(rhs, objcomplex::type_complex()) {
            let res = objcomplex::complex_binary_op(op, lhs_val as f64, 0.0, rhs);
            if res != obj::OBJ_NULL {
                return res;
            }
        }
    }

    let mut op = op;
    let mut lhs = lhs;
    let mut rhs = rhs;
    if op == BinaryOp::In {
        op = BinaryOp::Contains;
        std::mem::swap(&mut lhs, &mut rhs);
    }

    if let Some(slot) = obj::type_get_binary_op(obj::get_type(lhs)) {
        let r = slot(op, lhs, rhs);
        if r != obj::OBJ_NULL {
            return r;
        }
    }

    let op_u8 = op as u8;
    if op_u8 >= BinaryOp::InplaceOr as u8 && op_u8 <= BinaryOp::InplacePower as u8 {
        let normal = unsafe {
            std::mem::transmute::<u8, BinaryOp>(
                op_u8 + (BinaryOp::Or as u8 - BinaryOp::InplaceOr as u8),
            )
        };
        if let Some(slot) = obj::type_get_binary_op(obj::get_type(lhs)) {
            let r = slot(normal, lhs, rhs);
            if r != obj::OBJ_NULL {
                return r;
            }
        }
    }

    if mpconfig::PY_REVERSE_SPECIAL_METHODS {
        if op_u8 >= BinaryOp::Or as u8 && op_u8 <= BinaryOp::Power as u8 {
            std::mem::swap(&mut lhs, &mut rhs);
            let reverse = unsafe {
                std::mem::transmute::<u8, BinaryOp>(
                    op_u8 + (BinaryOp::ReverseOr as u8 - BinaryOp::Or as u8),
                )
            };
            if let Some(slot) = obj::type_get_binary_op(obj::get_type(lhs)) {
                let r = slot(reverse, lhs, rhs);
                if r != obj::OBJ_NULL {
                    return r;
                }
            }
        }
    }

    if op == BinaryOp::Contains {
        let mut iter_buf = ObjIterBuf {
            base: ObjBase {
                type_: core::ptr::null(),
            },
            buf: [obj::OBJ_NULL; 3],
        };
        let iter = getiter(lhs, Some(&mut iter_buf));
        loop {
            let next = iternext(iter);
            if next == obj::OBJ_STOP_ITERATION {
                break;
            }
            if obj::equal(next, rhs) {
                return obj::CONST_TRUE;
            }
        }
        return obj::CONST_FALSE;
    }

    raise::raise(MpRaise::TypeError("unsupported type for operator"));
}

fn binary_op_small_int_obj(op: BinaryOp, a: Int, b: Int) -> Obj {
    match binary_op_small_int(op, a, b) {
        Ok(o) => o,
        Err(RuntimeError::ZeroDivision) => raise::raise(MpRaise::ZeroDivisionError),
        Err(RuntimeError::ValueError(m)) => raise::raise(MpRaise::ValueError(m)),
        Err(e) => raise::raise(MpRaise::TypeError(e.message())),
    }
}

// --- calls ----------------------------------------------------------------------

pub fn call_function_0(fun: Obj) -> Obj {
    call_function_n_kw(fun, 0, 0, &[])
}

pub fn call_function_1(fun: Obj, arg: Obj) -> Obj {
    call_function_n_kw(fun, 1, 0, &[arg])
}

pub fn call_function_2(fun: Obj, arg1: Obj, arg2: Obj) -> Obj {
    call_function_n_kw(fun, 2, 0, &[arg1, arg2])
}

/// `mp_call_function_n_kw`.
pub fn call_function_n_kw(fun: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    if let Some(call) = obj::type_get_call(obj::get_type(fun)) {
        return call(fun, n_args, n_kw, args);
    }
    raise::raise(MpRaise::TypeError("object not callable"));
}

/// `mp_call_method_n_kw`.
pub fn call_method_n_kw(n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let adjust = if args.get(1).copied().unwrap_or(obj::OBJ_NULL) == obj::OBJ_NULL {
        0
    } else {
        1
    };
    let fun = args[0];
    let rest = &args[2 - adjust..];
    call_function_n_kw(fun, n_args + adjust, n_kw, rest)
}

pub fn call_method_self_n_kw(
    fun: Obj,
    self_: Obj,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) -> Obj {
    let mut buf = Vec::with_capacity(2 + args.len() + 2 * n_kw);
    buf.push(fun);
    buf.push(self_);
    buf.extend_from_slice(args);
    call_method_n_kw(n_args, n_kw, &buf)
}

/// `mp_call_prepare_args_n_kw_var`.
pub fn call_prepare_args_n_kw_var(
    have_self: bool,
    n_args_n_kw: usize,
    args: &[Obj],
    out_args: &mut CallArgs,
) {
    let mut idx = 0usize;
    let fun = args[idx];
    idx += 1;
    let self_ = if have_self {
        let s = args[idx];
        idx += 1;
        s
    } else {
        obj::OBJ_NULL
    };
    let n_args = n_args_n_kw & 0xff;
    let n_kw = (n_args_n_kw >> 8) & 0xff;
    let star_args = if idx + n_args + 2 * n_kw < args.len() {
        obj::small_int_value(args[idx + n_args + 2 * n_kw]) as usize
    } else {
        0
    };

    let mut list_len = 0isize;
    if star_args != 0 {
        for i in 0..n_args {
            if (star_args >> i) & 1 != 0 {
                if let Some(len) = obj::len_maybe(args[idx + i]) {
                    list_len += obj::get_int(len) as isize - 1;
                }
            }
        }
    }

    let mut kw_dict_len = 0isize;
    for i in 0..n_kw {
        let key = args[idx + n_args + i * 2];
        let value = args[idx + n_args + i * 2 + 1];
        if key == obj::OBJ_NULL && value != obj::OBJ_NULL && obj::is_dict_or_ordereddict(value) {
            kw_dict_len += objdict::dict_len(value) as isize - 1;
        }
    }

    let pos_base = idx;
    let mut args2 = Vec::new();
    if self_ != obj::OBJ_NULL {
        args2.push(self_);
    }

    if star_args == 0 {
        args2.extend_from_slice(&args[pos_base..pos_base + n_args]);
    } else {
        for i in 0..n_args {
            let arg = args[pos_base + i];
            if (star_args >> i) & 1 != 0 {
                if obj::is_exact_type(arg, objtuple::type_tuple())
                    || obj::is_exact_type(arg, objlist::type_list())
                {
                    let (len, items) = if obj::is_exact_type(arg, objtuple::type_tuple()) {
                        objtuple::tuple_get(arg)
                    } else {
                        objlist::list_get(arg)
                    };
                    args2.extend_from_slice(&items[..len]);
                } else {
                    let mut iter_buf = ObjIterBuf {
                        base: ObjBase {
                            type_: core::ptr::null(),
                        },
                        buf: [obj::OBJ_NULL; 3],
                    };
                    let iterable = getiter(arg, Some(&mut iter_buf));
                    loop {
                        let item = iternext(iterable);
                        if item == obj::OBJ_STOP_ITERATION {
                            break;
                        }
                        args2.push(item);
                    }
                }
            } else {
                args2.push(arg);
            }
        }
    }
    let _ = list_len;

    let pos_args_len = args2.len();
    args2.reserve(2 * (n_kw + kw_dict_len.max(0) as usize));

    for i in 0..n_kw {
        let kw_key = args[pos_base + n_args + i * 2];
        let kw_value = args[pos_base + n_args + i * 2 + 1];
        if kw_key == obj::OBJ_NULL {
            if obj::is_dict_or_ordereddict(kw_value) {
                let map = unsafe { &(*objdict::dict_ptr(kw_value)).map };
                for j in 0..map.alloc {
                    if map::slot_is_filled(map, j) {
                        args2.push(map.table[j].key);
                        args2.push(map.table[j].value);
                    }
                }
            } else {
                let mut dest = [obj::OBJ_NULL; 3];
                load_method(
                    kw_value,
                    qstr::from_str("keys"),
                    &mut dest[..2].try_into().unwrap(),
                );
                let iterable = getiter(call_method_n_kw(0, 0, &dest), None);
                loop {
                    let key = iternext(iterable);
                    if key == obj::OBJ_STOP_ITERATION {
                        break;
                    }
                    load_method(
                        kw_value,
                        qstr::from_str("__getitem__"),
                        &mut dest[..2].try_into().unwrap(),
                    );
                    dest[2] = key;
                    let value = call_method_n_kw(1, 0, &dest);
                    args2.push(key);
                    args2.push(value);
                }
            }
        } else {
            args2.push(kw_key);
            args2.push(kw_value);
        }
    }

    out_args.fun = fun;
    out_args.n_args = pos_args_len;
    out_args.n_kw = (args2.len() - pos_args_len) / 2;
    out_args.n_alloc = args2.len();
    out_args.args = args2;
}

/// `mp_call_method_n_kw_var`.
pub fn call_method_n_kw_var(have_self: bool, n_args_n_kw: usize, args: &[Obj]) -> Obj {
    let mut out_args = CallArgs {
        fun: obj::OBJ_NULL,
        n_args: 0,
        n_kw: 0,
        n_alloc: 0,
        args: Vec::new(),
    };
    call_prepare_args_n_kw_var(have_self, n_args_n_kw, args, &mut out_args);
    call_function_n_kw(out_args.fun, out_args.n_args, out_args.n_kw, &out_args.args)
}

// --- member lookup / methods / attributes ---------------------------------------

/// `mp_convert_member_lookup`.
pub fn convert_member_lookup(self_: Obj, type_: &ObjType, member: Obj, dest: &mut [Obj; 2]) {
    if !obj::is_obj(member) {
        dest[0] = member;
        dest[1] = obj::OBJ_NULL;
        return;
    }
    let m_type = obj::get_type(member);
    if (m_type.flags & TYPE_FLAG_BINDS_SELF) != 0 {
        if (m_type.flags & TYPE_FLAG_BUILTIN_FUN) != 0 {
            if obj::is_instance_type(type_) {
                dest[0] = member;
            } else {
                dest[0] = member;
                dest[1] = self_;
            }
        } else {
            dest[0] = member;
            dest[1] = self_;
        }
    } else if core::ptr::eq(m_type, objtype::type_staticmethod()) {
        let scm = unsafe { &*(obj::as_ptr(member) as *const objtype::ObjStaticClassMethod) };
        dest[0] = scm.fun;
        dest[1] = obj::OBJ_NULL;
    } else if core::ptr::eq(m_type, objtype::type_classmethod()) {
        let mut ty = type_;
        if self_ != obj::OBJ_NULL {
            ty = obj::get_type(self_);
            if core::ptr::eq(ty, objtype::type_type()) {
                ty = unsafe { &*(obj::as_ptr(self_) as *const ObjType) };
            }
        }
        let scm = unsafe { &*(obj::as_ptr(member) as *const objtype::ObjStaticClassMethod) };
        dest[0] = scm.fun;
        dest[1] = obj::from_ptr(ty as *const ObjType as *const ());
    } else {
        dest[0] = member;
        dest[1] = obj::OBJ_NULL;
    }
}

/// `mp_load_method_maybe`.
pub fn load_method_maybe(obj_in: Obj, attr: Qstr, dest: &mut [Obj]) {
    assert!(dest.len() >= 2);
    dest[0] = obj::OBJ_NULL;
    dest[1] = obj::OBJ_NULL;
    if obj_in == obj::OBJ_NULL {
        return;
    }
    let t = obj::get_type(obj_in);

    if mpconfig::CPYTHON_COMPAT && attr == qstr::from_str("__class__") {
        dest[0] = obj::from_ptr(t as *const ObjType as *const ());
        return;
    }

    if attr == qstr::from_str("__next__") && type_has_iternext(t) {
        dest[0] = obj::OBJ_NULL;
        dest[1] = obj_in;
        return;
    }

    if let Some(attr_fn) = obj::type_get_attr(t) {
        let mut pair = [obj::OBJ_NULL; 2];
        attr_fn(obj_in, attr, &mut pair);
        // Match C `mp_load_method_maybe`: if type->attr ran (dest[1] != SENTINEL),
        // return immediately — do not convert_member_lookup (modules return bare funs).
        if pair[1] != obj::OBJ_SENTINEL {
            dest[0] = pair[0];
            dest[1] = pair[1];
            return;
        }
        // Clear the fail flag set by type->attr so it's like it never ran.
        // (pair[1] was SENTINEL; fall through to locals_dict.)
    }

    if let Some(locals) = obj::type_get_slot_locals_dict(t) {
        let map = unsafe { &mut (*(obj::as_ptr(locals) as *mut ObjDict)).map };
        if let Some(elem) = map::lookup(map, obj::new_qstr(attr), LookupKind::Lookup) {
            let mut pair = [obj::OBJ_NULL; 2];
            convert_member_lookup(obj_in, t, elem.value, &mut pair);
            dest[0] = pair[0];
            dest[1] = pair[1];
        }
    }
}

fn raise_no_attribute(obj_in: Obj, attr: Qstr) -> ! {
    if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_TERSE as u8 {
        raise::raise(MpRaise::AttributeError("no such attribute"));
    }
    let type_name = obj::get_type_str(obj_in);
    let attr_name = qstr::str_from_qstr(attr).unwrap_or_else(|| "?".into());
    let msg = format!("'{type_name}' object has no attribute '{attr_name}'");
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_attribute_error(),
        1,
        &[objstr::new_str(msg.as_bytes())],
    ));
}

/// `mp_load_method`.
pub fn load_method(obj_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    load_method_maybe(obj_in, attr, dest);
    if dest[0] == obj::OBJ_NULL {
        raise_no_attribute(obj_in, attr);
    }
}

/// `mp_load_method_protected`.
pub fn load_method_protected(obj_in: Obj, attr: Qstr, dest: &mut [Obj; 2], catch_all_exc: bool) {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || load_method_maybe(obj_in, attr, dest)) {
        Ok(()) => {}
        Err(_) if catch_all_exc => {
            dest[0] = obj::OBJ_NULL;
            dest[1] = obj::OBJ_NULL;
        }
        Err(v) => {
            // C always swallows AttributeError; other exceptions re-raise unless
            // `catch_all_exc` (needed so `dir()` survives module `__getattr__`).
            let is_attr_err = objexcept::exception_match(
                Obj(v),
                obj::from_ptr(objexcept::type_attribute_error() as *const ObjType as *const ()),
            );
            dest[0] = obj::OBJ_NULL;
            dest[1] = obj::OBJ_NULL;
            if !is_attr_err {
                raise::reraise(v);
            }
        }
    }
}

/// `mp_load_attr`.
pub fn load_attr(base: Obj, attr: Qstr) -> Obj {
    let mut dest = [obj::OBJ_NULL; 2];
    load_method(base, attr, &mut dest);
    if dest[1] == obj::OBJ_NULL {
        // load_method returned just a normal attribute
        dest[0]
    } else {
        // load_method returned a method, so build a bound method object
        // (must NOT be invoked here: `obj.method` without a call should
        // yield a bound-method value, not the result of calling it).
        objboundmeth::new_bound_meth(dest[0], dest[1])
    }
}

/// `mp_store_attr`.
pub fn store_attr(base: Obj, attr: Qstr, value: Obj) {
    let type_ = obj::get_type(base);
    if let Some(attr_fn) = obj::type_get_attr(type_) {
        let mut dest = [obj::OBJ_SENTINEL, value];
        attr_fn(base, attr, &mut dest);
        if dest[0] == obj::OBJ_NULL {
            return;
        }
    }
    raise_no_attribute(base, attr);
}

// --- iteration ------------------------------------------------------------------

/// `mp_getiter`.
pub fn getiter(o_in: Obj, iter_buf: Option<&mut ObjIterBuf>) -> Obj {
    assert!(o_in != obj::OBJ_NULL);
    let type_ = obj::get_type(o_in);
    if (type_.flags & TYPE_FLAG_ITER_IS_ITERNEXT) == TYPE_FLAG_ITER_IS_ITERNEXT
        || (type_.flags & TYPE_FLAG_ITER_IS_STREAM) == TYPE_FLAG_ITER_IS_STREAM
    {
        return o_in;
    }

    let buf_ptr = iter_buf.map(|b| b as *mut ObjIterBuf);

    if let Some(getiter_fn) = obj::type_get_iter(type_) {
        let iter = if let Some(ptr) = buf_ptr {
            getiter_fn(o_in, ptr)
        } else {
            let heap_buf = Box::new(ObjIterBuf {
                base: ObjBase {
                    type_: core::ptr::null(),
                },
                buf: [obj::OBJ_NULL; 3],
            });
            let ptr = &*heap_buf as *const ObjIterBuf as *mut ObjIterBuf;
            let iter = getiter_fn(o_in, ptr);
            if iter != obj::OBJ_NULL && iter.0 == ptr as usize {
                std::mem::forget(heap_buf);
            }
            iter
        };
        if iter != obj::OBJ_NULL {
            return iter;
        }
    }

    let mut dest = [obj::OBJ_NULL; 2];
    load_method_maybe(o_in, qstr::from_str("__getitem__"), &mut dest);
    if dest[0] != obj::OBJ_NULL {
        if let Some(ptr) = buf_ptr {
            return crate::objgetitemiter::new_getitem_iter(&dest, unsafe { &mut *ptr });
        }
        return crate::objgetitemiter::new_getitem_iter_heap(&dest);
    }

    raise::raise(MpRaise::TypeError("object is not iterable"));
}

fn type_get_iternext(type_: &ObjType) -> Option<obj::IterNextFn> {
    if (type_.flags & TYPE_FLAG_ITER_IS_STREAM) == TYPE_FLAG_ITER_IS_STREAM {
        Some(crate::stream::stream_unbuffered_iter)
    } else {
        obj::type_get_iternext_fn(type_)
    }
}

/// `mp_iternext_allow_raise`.
pub fn iternext_allow_raise(o_in: Obj) -> Obj {
    let type_ = obj::get_type(o_in);
    if type_has_iternext(type_) {
        mpstate::set_stop_iteration_arg(obj::OBJ_NULL);
        if let Some(iternext) = type_get_iternext(type_) {
            return iternext(o_in);
        }
    }
    let mut dest = [obj::OBJ_NULL; 2];
    load_method_maybe(o_in, qstr::from_str("__next__"), &mut dest);
    if dest[0] != obj::OBJ_NULL {
        return call_method_n_kw(0, 0, &dest);
    }
    raise::raise(MpRaise::TypeError("object is not an iterator"));
}

/// `mp_iternext`.
pub fn iternext(o_in: Obj) -> Obj {
    cstack::check();
    let type_ = obj::get_type(o_in);
    if type_has_iternext(type_) {
        mpstate::set_stop_iteration_arg(obj::OBJ_NULL);
        if let Some(iternext) = type_get_iternext(type_) {
            return iternext(o_in);
        }
    }
    let mut dest = [obj::OBJ_NULL; 2];
    load_method_maybe(o_in, qstr::from_str("__next__"), &mut dest);
    if dest[0] != obj::OBJ_NULL {
        let mut nlr_buf = NlrBuf::default();
        return match nlr::protect(&mut nlr_buf, || call_method_n_kw(0, 0, &dest)) {
            Ok(v) => v,
            Err(_) => make_stop_iteration(obj::OBJ_NULL),
        };
    }
    raise::raise(MpRaise::TypeError("object is not an iterator"));
}

/// `mp_make_stop_iteration`.
pub fn make_stop_iteration(o: Obj) -> Obj {
    mpstate::set_stop_iteration_arg(o);
    obj::OBJ_STOP_ITERATION
}

// --- unpack ---------------------------------------------------------------------

/// `mp_unpack_sequence`.
pub fn unpack_sequence(seq_in: Obj, num: usize, items: &mut [Obj]) {
    assert_eq!(items.len(), num);
    if obj::is_exact_type(seq_in, objtuple::type_tuple())
        || obj::is_exact_type(seq_in, objlist::type_list())
    {
        let (len, seq_items) = if obj::is_exact_type(seq_in, objtuple::type_tuple()) {
            objtuple::tuple_get(seq_in)
        } else {
            objlist::list_get(seq_in)
        };
        if len < num {
            raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
        }
        if len > num {
            raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
        }
        for i in 0..num {
            items[i] = seq_items[num - 1 - i];
        }
        return;
    }
    let mut iter_buf = ObjIterBuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf: [obj::OBJ_NULL; 3],
    };
    let iterable = getiter(seq_in, Some(&mut iter_buf));
    for seq_len in 0..num {
        let el = iternext(iterable);
        if el == obj::OBJ_STOP_ITERATION {
            raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
        }
        items[num - 1 - seq_len] = el;
    }
    if iternext(iterable) != obj::OBJ_STOP_ITERATION {
        raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
    }
}

/// `mp_unpack_ex`.
pub fn unpack_ex(seq_in: Obj, num_in: usize, items: &mut [Obj]) {
    let num_left = num_in & 0xff;
    let num_right = (num_in >> 8) & 0xff;
    if obj::is_exact_type(seq_in, objtuple::type_tuple())
        || obj::is_exact_type(seq_in, objlist::type_list())
    {
        let (seq_len, seq_items) = if obj::is_exact_type(seq_in, objtuple::type_tuple()) {
            objtuple::tuple_get(seq_in)
        } else {
            objlist::list_get(seq_in)
        };
        if seq_len < num_left + num_right {
            raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
        }
        for i in 0..num_right {
            items[i] = seq_items[seq_len - 1 - i];
        }
        items[num_right] =
            objlist::new_list(seq_len - num_left - num_right, Some(&seq_items[num_left..]));
        for i in 0..num_left {
            items[num_right + 1 + i] = seq_items[num_left - 1 - i];
        }
        return;
    }
    let iterable = getiter(seq_in, None);
    for seq_len in 0..num_left {
        let item = iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
        }
        items[num_left + num_right + 1 - 1 - seq_len] = item;
    }
    let mut rest = Vec::new();
    loop {
        let item = iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        rest.push(item);
    }
    if rest.len() < num_right {
        raise::raise(MpRaise::ValueError("wrong number of values to unpack"));
    }
    items[num_right] = objlist::new_list(
        rest.len() - num_right,
        Some(&rest[..rest.len() - num_right]),
    );
    for i in 0..num_right {
        items[num_right - 1 - i] = rest[rest.len() - num_right + i];
    }
}

// --- generators / exceptions / import -------------------------------------------

/// `mp_resume` — generator, iterator, and delegator protocol.
pub fn resume(self_in: Obj, send_value: Obj, throw_value: Obj, ret_val: &mut Obj) -> VmReturnKind {
    assert!((send_value != obj::OBJ_NULL) ^ (throw_value != obj::OBJ_NULL));
    let type_ = obj::get_type(self_in);

    if obj::is_exact_type(self_in, objgenerator::type_gen_instance()) {
        return objgenerator::gen_resume(self_in, send_value, throw_value, ret_val);
    }

    if type_has_iternext(type_) && send_value == obj::CONST_NONE {
        mpstate::set_stop_iteration_arg(obj::OBJ_NULL);
        if let Some(iternext) = type_get_iternext(type_) {
            let ret = iternext(self_in);
            *ret_val = ret;
            if ret != obj::OBJ_STOP_ITERATION {
                return VmReturnKind::Yield;
            }
            *ret_val = mpstate::stop_iteration_arg();
            if *ret_val == obj::OBJ_NULL {
                *ret_val = obj::CONST_NONE;
            }
            return VmReturnKind::Normal;
        }
    }

    let mut dest = [obj::OBJ_NULL; 3];

    if send_value == obj::CONST_NONE {
        load_method_maybe(self_in, qstr::from_str("__next__"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            *ret_val = call_method_n_kw(0, 0, &dest);
            return VmReturnKind::Yield;
        }
    }

    if send_value != obj::OBJ_NULL {
        load_method(
            self_in,
            qstr::from_str("send"),
            &mut dest[..2].try_into().unwrap(),
        );
        dest[2] = send_value;
        *ret_val = call_method_n_kw(1, 0, &dest);
        return VmReturnKind::Yield;
    }

    assert!(throw_value != obj::OBJ_NULL);
    if objexcept::exception_match(
        throw_value,
        obj::from_ptr(objexcept::type_generator_exit() as *const ObjType as *const ()),
    ) {
        load_method_maybe(self_in, qstr::from_str("close"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            *ret_val = call_method_n_kw(0, 0, &dest);
            return VmReturnKind::Normal;
        }
    } else {
        load_method_maybe(self_in, qstr::from_str("throw"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            dest[2] = throw_value;
            *ret_val = call_method_n_kw(1, 0, &dest);
            return VmReturnKind::Yield;
        }
    }

    if objexcept::exception_match(
        throw_value,
        obj::from_ptr(objexcept::type_stop_iteration() as *const ObjType as *const ()),
    ) {
        let msg = objstr::new_str(b"generator raised StopIteration");
        *ret_val = objexcept::new_exception_args(objexcept::type_runtime_error(), 1, &[msg]);
    } else {
        *ret_val = make_raise_obj(throw_value);
    }
    VmReturnKind::Exception
}

/// `mp_make_raise_obj`.
pub fn make_raise_obj(o: Obj) -> Obj {
    let mut o = o;
    if objexcept::is_exception_type(o) {
        o = call_function_n_kw(o, 0, 0, &[]);
    }
    if objexcept::is_exception_instance(o) {
        return o;
    }
    let msg = objstr::new_str(b"exceptions must derive from BaseException");
    objexcept::new_exception_args(objexcept::type_type_error(), 1, &[msg])
}

/// `mp_import_name`.
pub fn import_name(name: Qstr, fromlist: Obj, level: Obj) -> Obj {
    let args = [
        obj::new_qstr(name),
        mpstate::globals_get(),
        obj::CONST_NONE,
        fromlist,
        level,
    ];
    if mpconfig::CAN_OVERRIDE_BUILTINS {
        if let Some(bo) = mpstate::with_vm(|vm| vm.mp_module_builtins_override_dict) {
            if bo != obj::OBJ_NULL {
                let import_key = obj::new_qstr(qstr::from_str("__import__"));
                let import_fun = objdict::dict_get(bo, import_key);
                if import_fun != obj::OBJ_NULL {
                    return call_function_n_kw(import_fun, 5, 0, &args);
                }
            }
        }
    }
    crate::builtinimport::builtin___import___default(5, &args)
}

/// `mp_import_from`.
pub fn import_from(module: Obj, name: Qstr) -> Obj {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    load_method_maybe(module, name, &mut dest);
    if dest[1] != obj::OBJ_NULL {
        return objboundmeth::new_bound_meth(dest[0], dest[1]);
    }
    if dest[0] != obj::OBJ_NULL {
        return dest[0];
    }

    if mpconfig::ENABLE_EXTERNAL_IMPORT {
        load_method_maybe(module, qstr::from_str("__path__"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            load_method(module, qstr::from_str("__name__"), &mut dest);
            let (pkg_data, pkg_len) = objstr::get_str_data_len(dest[0]);
            let pkg_name = std::str::from_utf8(&pkg_data[..pkg_len]).unwrap_or("");
            let dot_name = format!(
                "{pkg_name}.{}",
                qstr::str_from_qstr(name).unwrap_or_default()
            );
            return import_name(
                qstr::from_str(&dot_name),
                obj::CONST_TRUE,
                obj::new_small_int(0),
            );
        }
    }

    let msg = objstr::new_str(
        format!(
            "can't import name {}",
            qstr::str_from_qstr(name).unwrap_or_default()
        )
        .as_bytes(),
    );
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_import_error(),
        1,
        &[msg],
    ));
}

/// `mp_import_all`.
pub fn import_all(module: Obj) {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];

    if mpconfig::MODULE_ALL {
        load_method_maybe(module, qstr::from_str("__all__"), &mut dest);
        if dest[0] != obj::OBJ_NULL {
            let (len, items) = objtuple::tuple_get(dest[0]);
            for item in items {
                let qname = objstr::str_get_qstr(item);
                load_method(module, qname, &mut dest);
                store_name(qname, dest[0]);
            }
            return;
        }
    }

    let globals = if mpconfig::CPYTHON_COMPAT {
        load_method(module, qstr::from_str("__dict__"), &mut dest);
        dest[0]
    } else {
        obj::from_ptr(objmodule::module_get_globals(module) as *const ObjDict as *const ())
    };

    let map = unsafe { &(*objdict::dict_ptr(globals)).map };
    for i in 0..map.alloc {
        if !crate::map::slot_is_filled(map, i) {
            continue;
        }
        let key = map.table[i].key;
        if !obj::is_qstr(key) {
            continue;
        }
        let (data, len) = objstr::get_str_data_len(key);
        if data.first() == Some(&b'_') {
            continue;
        }
        let qname = objstr::str_get_qstr(key);
        store_name(qname, map.table[i].value);
    }
}

// --- events / scheduler ---------------------------------------------------------

pub fn handle_pending(behavior: HandlePendingBehaviour) {
    crate::scheduler::handle_pending(behavior);
}

pub fn event_handle_nowait() {
    crate::scheduler::event_handle_nowait();
}

pub fn event_wait_indefinite() {
    crate::scheduler::event_wait_indefinite();
}

pub fn event_wait_ms(timeout_ms: usize) {
    crate::scheduler::event_wait_ms(timeout_ms);
}

pub fn sched_lock() {
    crate::scheduler::sched_lock();
}

pub fn sched_unlock() {
    crate::scheduler::sched_unlock();
}

pub fn sched_num_pending() -> u8 {
    crate::scheduler::sched_num_pending()
}

pub fn call_function_1_protected(fun: Obj, arg: Obj) {
    crate::scheduler::call_function_1_protected(fun, arg);
}

// --- allocation failure ---------------------------------------------------------

/// `m_malloc_fail`.
pub fn malloc_fail(_num_bytes: usize) -> ! {
    raise::raise(MpRaise::RuntimeError("memory allocation failed"));
}

// --- smoke REPL helpers -----------------------------------------------------------

pub fn obj_to_string(o: Obj) -> Result<String> {
    if obj::is_small_int(o) {
        return Ok(obj::small_int_value(o).to_string());
    }
    Err(RuntimeError::TypeError("obj_to_string: unsupported"))
}

pub fn eval_source(src: &str) -> Result<String> {
    match obj_to_string(eval_str(src)) {
        Ok(s) => Ok(s),
        Err(e) => Err(e),
    }
}

fn eval_str_impl(source: &str, kind: crate::parse::ParseInputKind) -> Obj {
    use crate::compile;
    use crate::lexer::Lexer;
    use crate::reader::READER_IS_ROM;
    let name = if kind == crate::parse::ParseInputKind::EvalInput {
        qstr::from_str("<string>")
    } else {
        qstr::from_str("<string>")
    };
    let lex = Lexer::new_from_str_len(name, source.trim().as_bytes(), READER_IS_ROM);
    compile::parse_compile_execute(lex, kind, None, None)
}

fn exec_str_impl(source: &str) -> Obj {
    use crate::compile;
    use crate::lexer::Lexer;
    use crate::parse::ParseInputKind;
    use crate::reader::READER_IS_ROM;
    let lex = Lexer::new_from_str_len(qstr::from_str("<string>"), source.as_bytes(), READER_IS_ROM);
    compile::parse_compile_execute(lex, ParseInputKind::FileInput, None, None)
}

fn execfile_impl(path: Qstr) -> Obj {
    use crate::compile;
    use crate::lexer::Lexer;
    use crate::parse::ParseInputKind;
    let lex = Lexer::new_from_file(path);
    compile::parse_compile_execute(lex, ParseInputKind::FileInput, None, None)
}

fn protect_eval<T>(f: impl FnOnce() -> T) -> T {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, f) {
        Ok(v) => v,
        Err(v) => raise::reraise(v),
    }
}

/// Parse, compile, and execute Python source as an expression.
pub fn eval_str(source: &str) -> Obj {
    protect_eval(|| eval_str_impl(source, crate::parse::ParseInputKind::EvalInput))
}

/// Parse, compile, and execute Python source as a module body.
pub fn exec_str(source: &str) -> Obj {
    protect_eval(|| exec_str_impl(source))
}

/// Parse, compile, and execute Python source from a file.
pub fn execfile(path: Qstr) -> Obj {
    protect_eval(|| execfile_impl(path))
}

pub fn eval_one_plus_two() -> Result<String> {
    eval_source("1+2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_arithmetic_smoke() {
        init();
        assert_eq!(eval_source("1+2*3").unwrap(), "7");
    }

    #[test]
    fn small_int_binary_ops() {
        init();
        let seven = binary_op_obj(BinaryOp::Add, obj::new_small_int(3), obj::new_small_int(4));
        assert_eq!(obj::small_int_value(seven), 7);
    }
}
