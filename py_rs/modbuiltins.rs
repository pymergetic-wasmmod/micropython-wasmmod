//! rewrite of py/modbuiltins.c + declarations from py/builtin.h
// symmetry: done

use crate::argcheck::{self, Arg, ArgFlag, ArgVal};
use crate::builtin;
use crate::builtinevex;
use crate::malloc;
use crate::map::{self, LookupKind, Map, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use crate::objarray;
use crate::objboundmeth;
use crate::objcell;
use crate::objcomplex;
use crate::objdict;
use crate::objenumerate;
use crate::objexcept;
use crate::objfilter;
use crate::objfloat;
use crate::objlist;
use crate::objmap;
use crate::objmodule;
use crate::objproperty;
use crate::objrange;
use crate::objreversed;
use crate::objsingleton;
use crate::objslice;
use crate::objstr;
use crate::objtemplate;
use crate::objtuple;
use crate::objtype;
use crate::objzip;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::unicode::{utf8_charlen, utf8_get_char};
use crate::vstr::{self, Vstr};

pub const PY_BUILTINS_HELP_TEXT: &str = concat!(
    "Welcome to MicroPython!\n\n",
    "For online docs please visit http://docs.micropython.org/\n\n",
    "Control commands:\n",
    "  CTRL-A        -- on a blank line, enter raw REPL mode\n",
    "  CTRL-B        -- on a blank line, enter normal REPL mode\n",
    "  CTRL-C        -- interrupt a running program\n",
    "  CTRL-D        -- on a blank line, exit or do a soft reset\n",
    "  CTRL-E        -- on a blank line, enter paste mode\n\n",
    "For further help on a specific object, type help(obj)\n"
);

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;
type BuiltinFnKw = fn(usize, &[Obj], &mut Map) -> Obj;

#[repr(C)]
struct ObjFunBuiltinFixed {
    base: ObjBase,
    fun: ObjFunBuiltinFixedFun,
}

#[repr(C)]
union ObjFunBuiltinFixedFun {
    f0: BuiltinFn0,
    f1: BuiltinFn1,
    f2: BuiltinFn2,
    f3: BuiltinFn3,
}

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    sig: u32,
    fun: ObjFunBuiltinVarFun,
}

#[repr(C)]
union ObjFunBuiltinVarFun {
    var: BuiltinFnVar,
    kw: BuiltinFnKw,
}

macro_rules! fun_builtin_type {
    ($name:ident, $slots:ident) => {
        static mut $slots: [*const (); 1] = [core::ptr::null()];
        static mut $name: ObjType = ObjType {
            base: ObjBase {
                type_: core::ptr::null(),
            },
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
        }
    });
}

fn new_fun_builtin_0(fun: BuiltinFn0) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_0 as *const ObjType;
        (*o).fun.f0 = fun;
        obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
    }
}

fn new_fun_builtin_1(fun: BuiltinFn1) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_1 as *const ObjType;
        (*o).fun.f1 = fun;
        obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
    }
}

fn new_fun_builtin_2(fun: BuiltinFn2) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_2 as *const ObjType;
        (*o).fun.f2 = fun;
        obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
    }
}

fn new_fun_builtin_3(fun: BuiltinFn3) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinFixed>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_3 as *const ObjType;
        (*o).fun.f3 = fun;
        obj::from_ptr(o as *const ObjFunBuiltinFixed as *const ())
    }
}

fn new_fun_builtin_var(min_args: u8, max_args: u8, fun: BuiltinFnVar) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).sig = argcheck::make_sig(min_args as usize, max_args as usize, false);
        (*o).fun.var = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn new_fun_builtin_kw(min_args: u8, fun: BuiltinFnKw) -> Obj {
    new_fun_builtin_kw_var(min_args, min_args, fun)
}

fn new_fun_builtin_kw_var(min_args: u8, max_args: u8, fun: BuiltinFnKw) -> Obj {
    init_types();
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("builtin alloc");
    unsafe {
        (*o).base.type_ = &TYPE_FUN_BUILTIN_VAR as *const ObjType;
        (*o).sig = argcheck::make_sig(min_args as usize, max_args as usize, true);
        (*o).fun.kw = fun;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn fun_builtin_0_call(self_in: Obj, n_args: usize, n_kw: usize, _args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 0, 0, false);
    unsafe { (self_.fun.f0)() }
}

fn fun_builtin_1_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    unsafe { (self_.fun.f1)(args[0]) }
}

fn fun_builtin_2_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 2, 2, false);
    unsafe { (self_.fun.f2)(args[0], args[1]) }
}

fn fun_builtin_3_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinFixed) };
    argcheck::check_num(n_args, n_kw, 3, 3, false);
    unsafe { (self_.fun.f3)(args[0], args[1], args[2]) }
}

fn fun_builtin_var_call(self_in: Obj, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(self_in) as *const ObjFunBuiltinVar) };
    argcheck::check_num_sig(n_args, n_kw, self_.sig);
    if self_.sig & 1 != 0 {
        let mut kw_args = Map::default();
        map::init(&mut kw_args, n_kw);
        for i in 0..n_kw {
            if let Some(slot) = map::lookup(
                &mut kw_args,
                args[n_args + i * 2],
                LookupKind::AddIfNotFound,
            ) {
                slot.value = args[n_args + i * 2 + 1];
            }
        }
        unsafe { (self_.fun.kw)(n_args, args, &mut kw_args) }
    } else {
        unsafe { (self_.fun.var)(n_args, args) }
    }
}

fn load_attr_default(base: Obj, attr: Qstr, defval: Obj) -> Obj {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    if defval == obj::OBJ_NULL {
        runtime::load_method(base, attr, &mut dest);
    } else {
        runtime::load_method_protected(base, attr, &mut dest, false);
    }
    if dest[0] == obj::OBJ_NULL {
        return defval;
    }
    if dest[1] == obj::OBJ_NULL {
        dest[0]
    } else {
        objboundmeth::new_bound_meth(dest[0], dest[1])
    }
}

fn builtin___build_class__(n_args: usize, args: &[Obj]) -> Obj {
    if n_args < 2 {
        raise::raise(MpRaise::TypeError("argument num/types mismatch"));
    }
    let old_locals = runtime::locals_get();
    let class_locals = objdict::new_dict(0);
    runtime::locals_set(class_locals);
    let cell = runtime::call_function_0(args[0]);
    runtime::locals_set(old_locals);
    let meta = if n_args == 2 {
        obj::from_ptr(objtype::type_type() as *const ObjType as *const ())
    } else {
        obj::from_ptr(obj::get_type(args[2]) as *const ObjType as *const ())
    };
    let bases = objtuple::new_tuple(n_args - 2, Some(&args[2..]));
    let new_class = runtime::call_function_n_kw(meta, 3, 0, &[args[1], bases, class_locals]);
    if cell != obj::CONST_NONE {
        objcell::cell_set(cell, new_class);
    }
    new_class
}

fn builtin_abs(o: Obj) -> Obj {
    runtime::unary_op_obj(UnaryOp::Abs, o)
}

fn empty_iter_buf() -> obj::ObjIterBuf {
    obj::ObjIterBuf {
        base: obj::ObjBase {
            type_: core::ptr::null(),
        },
        buf: [obj::OBJ_NULL; 3],
    }
}

fn builtin_all(o: Obj) -> Obj {
    let mut iter_buf = empty_iter_buf();
    let iterable = runtime::getiter(o, Some(&mut iter_buf));
    loop {
        let item = runtime::iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            return obj::CONST_TRUE;
        }
        if !obj::is_true(item) {
            return obj::CONST_FALSE;
        }
    }
}

fn builtin_any(o: Obj) -> Obj {
    let mut iter_buf = empty_iter_buf();
    let iterable = runtime::getiter(o, Some(&mut iter_buf));
    loop {
        let item = runtime::iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            return obj::CONST_FALSE;
        }
        if obj::is_true(item) {
            return obj::CONST_TRUE;
        }
    }
}

fn builtin_bin(o: Obj) -> Obj {
    let fmt = obj::new_qstr(qstr::from_str("{:#b}"));
    objstr::str_format(2, &[fmt, o], None)
}

fn builtin_callable(o: Obj) -> Obj {
    obj::new_bool(obj::is_callable(o))
}

fn builtin_chr(o: Obj) -> Obj {
    let c = obj::get_int(o) as u32;
    if mpconfig::PY_BUILTINS_STR_UNICODE {
        if c >= 0x110000 {
            raise::raise(MpRaise::ValueError("char not in range(0x110000)"));
        }
        let mut v = Vstr {
            alloc: 0,
            len: 0,
            buf: core::ptr::null_mut(),
            fixed_buf: false,
        };
        vstr::init(&mut v, 4);
        vstr::add_char(&mut v, c);
        return objstr::new_str_from_vstr(&mut v);
    } else {
        let ord = obj::get_int(o);
        if !(0..=0xff).contains(&ord) {
            raise::raise(MpRaise::ValueError("chr() arg not in range(256)"));
        }
        objstr::new_str_via_qstr(&[ord as u8])
    }
}

fn builtin_dir(n_args: usize, args: &[Obj]) -> Obj {
    let dir = objlist::new_list(0, None);
    if n_args == 0 {
        let dict = objdict::dict_ptr(runtime::locals_get());
        let map = unsafe { &(*dict).map };
        for i in 0..map.alloc {
            if map::slot_is_filled(map, i) {
                objlist::list_append(dir, map.table[i].key);
            }
        }
    } else {
        let nqstr = qstr::total();
        for i in (qstr::QSTR_EMPTY + 1)..=nqstr {
            let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
            runtime::load_method_protected(args[0], i, &mut dest, false);
            if dest[0] != obj::OBJ_NULL {
                if mpconfig::PY_ALL_SPECIAL_METHODS
                    && i == qstr::from_str("__dir__")
                    && dest[1] != obj::OBJ_NULL
                {
                    return runtime::call_method_n_kw(0, 0, &dest);
                }
                objlist::list_append(dir, obj::new_qstr(i));
            }
        }
    }
    dir
}

fn builtin_divmod(o1: Obj, o2: Obj) -> Obj {
    runtime::binary_op_obj(BinaryOp::Divmod, o1, o2)
}

fn builtin_hash(o: Obj) -> Obj {
    runtime::unary_op_obj(UnaryOp::Hash, o)
}

fn builtin_hex(o: Obj) -> Obj {
    // Prefer `str.format` so `#` prefix works; modulo `"%#x"` is incomplete.
    let fmt = obj::new_qstr(qstr::from_str("{:#x}"));
    objstr::str_format(2, &[fmt, o], None)
}

fn builtin_help(n_args: usize, args: &[Obj]) -> Obj {
    if n_args == 0 {
        mpprint::print_str(&mpprint::PLAT_PRINT, PY_BUILTINS_HELP_TEXT);
    } else {
        obj::print_helper(&mpprint::PLAT_PRINT, args[0], PrintKind::Str);
        mpprint::print_str(&mpprint::PLAT_PRINT, "\n");
    }
    obj::CONST_NONE
}

fn builtin_input(n_args: usize, args: &[Obj]) -> Obj {
    if n_args == 1 {
        obj::print_helper(&mpprint::PLAT_PRINT, args[0], PrintKind::Str);
    }
    let mut line = String::new();
    let _ = std::io::Write::write_all(&mut std::io::stdout(), b"");
    match std::io::stdin().read_line(&mut line) {
        Ok(0) => raise::raise_obj(objexcept::new_exception(objexcept::type_eof_error())),
        Err(_) => raise::raise_obj(objexcept::new_exception(
            objexcept::type_keyboard_interrupt(),
        )),
        Ok(_) => {}
    }
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    objstr::new_str(line.as_bytes())
}

fn builtin_isinstance(o: Obj, classinfo: Obj) -> Obj {
    objtype::isinstance(o, classinfo)
}

fn builtin_issubclass(o: Obj, classinfo: Obj) -> Obj {
    objtype::issubclass(o, classinfo)
}

fn builtin_iter(o: Obj) -> Obj {
    runtime::getiter(o, None)
}

fn min_max(n_args: usize, args: &[Obj], kwargs: &mut Map, op: BinaryOp) -> Obj {
    let key_fn = map::lookup(
        kwargs,
        obj::new_qstr(qstr::from_str("key")),
        LookupKind::Lookup,
    )
    .map(|e| e.value)
    .unwrap_or(obj::OBJ_NULL);
    if n_args == 1 {
        let mut iter_buf = empty_iter_buf();
        let iterable = runtime::getiter(args[0], Some(&mut iter_buf));
        let mut best_key = obj::OBJ_NULL;
        let mut best_obj = obj::OBJ_NULL;
        loop {
            let item = runtime::iternext(iterable);
            if item == obj::OBJ_STOP_ITERATION {
                break;
            }
            let key = if key_fn == obj::OBJ_NULL {
                item
            } else {
                runtime::call_function_1(key_fn, item)
            };
            if best_obj == obj::OBJ_NULL || obj::is_true(runtime::binary_op_obj(op, key, best_key))
            {
                best_key = key;
                best_obj = item;
            }
        }
        if best_obj == obj::OBJ_NULL {
            if let Some(elem) = map::lookup(
                kwargs,
                obj::new_qstr(qstr::from_str("default")),
                LookupKind::Lookup,
            ) {
                return elem.value;
            }
            raise::raise(MpRaise::ValueError("arg is an empty sequence"));
        }
        return best_obj;
    }
    let mut best_key = obj::OBJ_NULL;
    let mut best_obj = obj::OBJ_NULL;
    for &arg in &args[..n_args] {
        let key = if key_fn == obj::OBJ_NULL {
            arg
        } else {
            runtime::call_function_1(key_fn, arg)
        };
        if best_obj == obj::OBJ_NULL || obj::is_true(runtime::binary_op_obj(op, key, best_key)) {
            best_key = key;
            best_obj = arg;
        }
    }
    best_obj
}

fn builtin_max(n_args: usize, args: &[Obj], kwargs: &mut Map) -> Obj {
    min_max(n_args, args, kwargs, BinaryOp::More)
}

fn builtin_min(n_args: usize, args: &[Obj], kwargs: &mut Map) -> Obj {
    min_max(n_args, args, kwargs, BinaryOp::Less)
}

fn builtin_next(n_args: usize, args: &[Obj]) -> Obj {
    if n_args == 1 {
        let ret = runtime::iternext_allow_raise(args[0]);
        if ret == obj::OBJ_STOP_ITERATION {
            let arg = crate::mpstate::stop_iteration_arg();
            if arg == obj::OBJ_NULL {
                raise::raise_obj(objexcept::new_exception(objexcept::type_stop_iteration()));
            }
            raise::raise_obj(objexcept::new_exception_args(
                objexcept::type_stop_iteration(),
                1,
                &[arg],
            ));
        }
        ret
    } else {
        let ret = runtime::iternext(args[0]);
        if ret == obj::OBJ_STOP_ITERATION {
            args[1]
        } else {
            ret
        }
    }
}

fn builtin_oct(o: Obj) -> Obj {
    let fmt = obj::new_qstr(qstr::from_str("{:#o}"));
    objstr::str_format(2, &[fmt, o], None)
}

fn builtin_ord(o: Obj) -> Obj {
    let (data, len) = objstr::str_get_data(o);
    if mpconfig::PY_BUILTINS_STR_UNICODE && obj::is_str(o) {
        let char_len = utf8_charlen(&data[..len], len);
        if char_len == 1 {
            return obj::new_int(utf8_get_char(&data[..len]) as obj::Int);
        }
    } else if len == 1 {
        return obj::new_small_int(data[0] as obj::Int);
    }
    if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_NORMAL {
        raise::raise(MpRaise::TypeError("ord expects a character"));
    }
    let msg = objstr::new_str(
        format!("ord() expected a character, but string of length {len} found").as_bytes(),
    );
    raise::raise_obj(objexcept::new_exception_args(
        objexcept::type_type_error(),
        1,
        &[msg],
    ));
}

fn builtin_pow(n_args: usize, args: &[Obj]) -> Obj {
    if n_args == 2 || (mpconfig::PY_BUILTINS_POW3 && args.get(2).copied() == Some(obj::CONST_NONE))
    {
        return runtime::binary_op_obj(BinaryOp::Power, args[0], args[1]);
    }
    if !mpconfig::PY_BUILTINS_POW3 {
        raise::raise(MpRaise::RuntimeError("3-arg pow() not supported"));
    }
    let modded = runtime::binary_op_obj(BinaryOp::Power, args[0], args[1]);
    runtime::binary_op_obj(BinaryOp::Modulo, modded, args[2])
}

fn builtin_print(n_args: usize, pos_args: &[Obj], kw_args: &mut Map) -> Obj {
    let allowed = [
        Arg {
            qst: qstr::from_str("sep"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::new_qstr(qstr::from_str(" "))),
        },
        Arg {
            qst: qstr::from_str("end"),
            flags: ArgFlag::KwOnly as u16 | ArgFlag::Obj as u16,
            defval: ArgVal::Obj(obj::new_qstr(qstr::from_str("\n"))),
        },
    ];
    let mut vals = [ArgVal::default(), ArgVal::default()];
    argcheck::parse_all(0, &[], kw_args, allowed.len(), &allowed, &mut vals);
    let sep = match vals[0] {
        ArgVal::Obj(o) => o,
        _ => obj::OBJ_NULL,
    };
    let end = match vals[1] {
        ArgVal::Obj(o) => o,
        _ => obj::OBJ_NULL,
    };
    let (sep_data, sep_len) = objstr::str_get_data(sep);
    let (end_data, end_len) = objstr::str_get_data(end);
    for (i, &arg) in pos_args.iter().enumerate().take(n_args) {
        if i > 0 {
            mpprint::print_strn(&mpprint::PLAT_PRINT, &sep_data[..sep_len], 0, b' ', 0);
        }
        obj::print_helper(&mpprint::PLAT_PRINT, arg, PrintKind::Str);
    }
    mpprint::print_strn(&mpprint::PLAT_PRINT, &end_data[..end_len], 0, b' ', 0);
    obj::CONST_NONE
}

fn builtin___repl_print__(o: Obj) -> Obj {
    if o != obj::CONST_NONE {
        obj::print_helper(&mpprint::PLAT_PRINT, o, PrintKind::Repr);
        mpprint::print_str(&mpprint::PLAT_PRINT, "\n");
        if mpconfig::CAN_OVERRIDE_BUILTINS {
            runtime::store_attr(builtins_module_obj(), qstr::from_str("_"), o);
        }
    }
    obj::CONST_NONE
}

fn builtin_repr(o: Obj) -> Obj {
    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    let mut print = Print {
        data: &mut v as *mut Vstr as *mut (),
        print_strn: Some(vstr::vstr_add_strn_print),
    };
    obj::print_helper(&mut print, o, PrintKind::Repr);
    objstr::new_str_from_vstr(&mut v)
}

fn builtin_round(n_args: usize, args: &[Obj]) -> Obj {
    let o_in = args[0];
    if obj::is_int(o_in) {
        if n_args <= 1 {
            return o_in;
        }
        if !mpconfig::PY_BUILTINS_ROUND_INT {
            raise::raise(MpRaise::RuntimeError("round int not supported"));
        }
        let num_dig = obj::get_int(args[1]);
        if num_dig >= 0 {
            return o_in;
        }
        let mult = runtime::binary_op_obj(
            BinaryOp::Power,
            obj::new_small_int(10),
            obj::new_small_int(-num_dig),
        );
        let half_mult = runtime::binary_op_obj(BinaryOp::FloorDivide, mult, obj::new_small_int(2));
        let modulo = runtime::binary_op_obj(BinaryOp::Modulo, o_in, mult);
        let rounded = runtime::binary_op_obj(BinaryOp::Subtract, o_in, modulo);
        if obj::is_true(runtime::binary_op_obj(BinaryOp::More, half_mult, modulo)) {
            return rounded;
        }
        if obj::is_true(runtime::binary_op_obj(BinaryOp::More, modulo, half_mult)) {
            return runtime::binary_op_obj(BinaryOp::Add, rounded, mult);
        }
        let floor = runtime::binary_op_obj(BinaryOp::FloorDivide, o_in, mult);
        if obj::is_true(runtime::binary_op_obj(
            BinaryOp::And,
            floor,
            obj::new_small_int(1),
        )) {
            return runtime::binary_op_obj(BinaryOp::Add, rounded, mult);
        }
        return rounded;
    }
    if mpconfig::PY_BUILTINS_FLOAT {
        let val = objfloat::float_get(o_in);
        // C uses `nearbyint` (IEEE roundTiesToEven), not Rust's `.round()`.
        if n_args > 1 {
            let num_dig = obj::get_int(args[1]) as i32;
            let mult = 10f64.powi(num_dig);
            let rounded = nearbyint(val * mult) / mult;
            return objfloat::new_float(rounded);
        }
        let rounded = nearbyint(val);
        return obj::new_int(rounded as obj::Int);
    }
    obj::new_int(obj::get_int(o_in) as obj::Int)
}

/// IEEE 754 roundTiesToEven — matches MicroPython/C `nearbyint`.
fn nearbyint(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let abs = x.abs();
    let trunc = abs.trunc();
    let frac = abs - trunc;
    let rounded_abs = if frac < 0.5 {
        trunc
    } else if frac > 0.5 {
        trunc + 1.0
    } else if (trunc as i64) & 1 == 0 {
        // exactly halfway: keep even
        trunc
    } else {
        trunc + 1.0
    };
    if x.is_sign_negative() && rounded_abs != 0.0 {
        -rounded_abs
    } else {
        rounded_abs
    }
}

fn builtin_sum(n_args: usize, args: &[Obj]) -> Obj {
    let mut value = if n_args == 1 {
        obj::new_small_int(0)
    } else {
        args[1]
    };
    let mut iter_buf = empty_iter_buf();
    let iterable = runtime::getiter(args[0], Some(&mut iter_buf));
    loop {
        let item = runtime::iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        value = runtime::binary_op_obj(BinaryOp::Add, value, item);
    }
    value
}

fn builtin_sorted(n_args: usize, args: &[Obj], kwargs: &mut Map) -> Obj {
    if n_args > 1 {
        raise::raise(MpRaise::TypeError(
            "must use keyword argument for key function",
        ));
    }
    let self_ = objlist::list_make_new(objlist::type_list(), 1, 0, args);
    objlist::list_sort(1, &[self_], kwargs);
    self_
}

fn builtin_getattr(n_args: usize, args: &[Obj]) -> Obj {
    let defval = if n_args > 2 { args[2] } else { obj::OBJ_NULL };
    load_attr_default(args[0], objstr::str_get_qstr(args[1]), defval)
}

fn builtin_setattr(base: Obj, attr: Obj, value: Obj) -> Obj {
    runtime::store_attr(base, objstr::str_get_qstr(attr), value);
    obj::CONST_NONE
}

fn builtin_delattr(base: Obj, attr: Obj) -> Obj {
    builtin_setattr(base, attr, obj::OBJ_NULL)
}

fn builtin_hasattr(object: Obj, attr: Obj) -> Obj {
    let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
    runtime::load_method_protected(object, objstr::str_get_qstr(attr), &mut dest, false);
    obj::new_bool(dest[0] != obj::OBJ_NULL)
}

fn builtin_globals() -> Obj {
    runtime::globals_get()
}

fn builtin_locals() -> Obj {
    runtime::locals_get()
}

fn me(name: &str, value: Obj) -> MapElem {
    MapElem {
        key: obj::new_qstr(qstr::from_str(name)),
        value,
    }
}

fn type_elem(name: &str, ty: &ObjType) -> MapElem {
    me(name, obj::from_ptr(ty as *const ObjType as *const ()))
}

fn build_globals_table() -> Vec<MapElem> {
    let mut table = vec![
        me("__name__", obj::new_qstr(qstr::from_str("builtins"))),
        me(
            "__build_class__",
            new_fun_builtin_var(2, usize::MAX as u8, |n, a| builtin___build_class__(n, a)),
        ),
        me("__import__", crate::builtinimport::builtin___import___obj()),
        me("__repl_print__", new_fun_builtin_1(builtin___repl_print__)),
        type_elem("bool", obj::type_bool()),
        type_elem("bytes", objstr::type_bytes()),
        type_elem("dict", objdict::type_dict()),
        type_elem("map", objmap::type_map()),
        type_elem("int", obj::type_int()),
        type_elem("list", objlist::type_list()),
        type_elem("object", objtype::type_object()),
        type_elem("range", objrange::type_range()),
        type_elem("str", objstr::type_str()),
        type_elem("super", objtype::type_super()),
        type_elem("tuple", objtuple::type_tuple()),
        type_elem("type", objtype::type_type()),
        type_elem("zip", objzip::type_zip()),
        type_elem("classmethod", objtype::type_classmethod()),
        type_elem("staticmethod", objtype::type_staticmethod()),
        me("Ellipsis", objsingleton::const_ellipsis()),
        me("abs", new_fun_builtin_1(builtin_abs)),
        me("all", new_fun_builtin_1(builtin_all)),
        me("any", new_fun_builtin_1(builtin_any)),
        me("bin", new_fun_builtin_1(builtin_bin)),
        me("callable", new_fun_builtin_1(builtin_callable)),
        me("chr", new_fun_builtin_1(builtin_chr)),
        me("divmod", new_fun_builtin_2(builtin_divmod)),
        me(
            "getattr",
            new_fun_builtin_var(2, 3, |n, a| builtin_getattr(n, a)),
        ),
        me("setattr", new_fun_builtin_3(builtin_setattr)),
        me("globals", new_fun_builtin_0(builtin_globals)),
        me("hasattr", new_fun_builtin_2(builtin_hasattr)),
        me("hash", new_fun_builtin_1(builtin_hash)),
        me("hex", new_fun_builtin_1(builtin_hex)),
        me("id", new_fun_builtin_1(obj::id)),
        me("isinstance", new_fun_builtin_2(builtin_isinstance)),
        me("issubclass", new_fun_builtin_2(builtin_issubclass)),
        me("iter", new_fun_builtin_1(builtin_iter)),
        me("len", new_fun_builtin_1(obj::len)),
        me("locals", new_fun_builtin_0(builtin_locals)),
        me("next", new_fun_builtin_var(1, 2, |n, a| builtin_next(n, a))),
        me("oct", new_fun_builtin_1(builtin_oct)),
        me("ord", new_fun_builtin_1(builtin_ord)),
        me("pow", new_fun_builtin_var(2, 3, |n, a| builtin_pow(n, a))),
        me(
            "print",
            new_fun_builtin_kw_var(0, 0xff, |n, a, kw| {
                let mut kw = kw.clone();
                builtin_print(n, a, &mut kw)
            }),
        ),
        me("repr", new_fun_builtin_1(builtin_repr)),
        me(
            "round",
            new_fun_builtin_var(1, 2, |n, a| builtin_round(n, a)),
        ),
        me(
            "sorted",
            new_fun_builtin_kw(1, |n, a, kw| {
                let mut kw = kw.clone();
                builtin_sorted(n, a, &mut kw)
            }),
        ),
        me("sum", new_fun_builtin_var(1, 2, |n, a| builtin_sum(n, a))),
        type_elem("BaseException", objexcept::type_base_exception()),
        type_elem("ArithmeticError", objexcept::type_arithmetic_error()),
        type_elem("AssertionError", objexcept::type_assertion_error()),
        type_elem("AttributeError", objexcept::type_attribute_error()),
        type_elem("EOFError", objexcept::type_eof_error()),
        type_elem("Exception", objexcept::type_exception()),
        type_elem("GeneratorExit", objexcept::type_generator_exit()),
        type_elem("ImportError", objexcept::type_import_error()),
        type_elem("IndentationError", objexcept::type_indentation_error()),
        type_elem("IndexError", objexcept::type_index_error()),
        type_elem("KeyboardInterrupt", objexcept::type_keyboard_interrupt()),
        type_elem("KeyError", objexcept::type_key_error()),
        type_elem("LookupError", objexcept::type_lookup_error()),
        type_elem("MemoryError", objexcept::type_memory_error()),
        type_elem("NameError", objexcept::type_name_error()),
        type_elem(
            "NotImplementedError",
            objexcept::type_not_implemented_error(),
        ),
        type_elem("OSError", objexcept::type_os_error()),
        type_elem("OverflowError", objexcept::type_overflow_error()),
        type_elem("RuntimeError", objexcept::type_runtime_error()),
        type_elem("StopIteration", objexcept::type_stop_iteration()),
        type_elem("SyntaxError", objexcept::type_syntax_error()),
        type_elem("SystemExit", objexcept::type_system_exit()),
        type_elem("TypeError", objexcept::type_type_error()),
        type_elem("ValueError", objexcept::type_value_error()),
        type_elem("ZeroDivisionError", objexcept::type_zero_division_error()),
    ];

    if mpconfig::PY_BUILTINS_BYTEARRAY {
        table.push(type_elem("bytearray", objarray::type_bytearray()));
    }
    if mpconfig::PY_BUILTINS_COMPLEX {
        table.push(type_elem("complex", objcomplex::type_complex()));
    }
    if mpconfig::PY_BUILTINS_ENUMERATE {
        table.push(type_elem("enumerate", objenumerate::type_enumerate()));
    }
    if mpconfig::PY_BUILTINS_FILTER {
        table.push(type_elem("filter", objfilter::type_filter()));
    }
    if mpconfig::PY_BUILTINS_FLOAT {
        table.push(type_elem("float", objfloat::type_float()));
    }
    if mpconfig::PY_BUILTINS_SET {
        table.push(type_elem("set", crate::objset::type_set()));
        if mpconfig::PY_BUILTINS_FROZENSET {
            table.push(type_elem("frozenset", crate::objset::type_frozenset()));
        }
    }
    if mpconfig::PY_BUILTINS_MEMORYVIEW {
        table.push(type_elem("memoryview", objarray::type_memoryview()));
    }
    if mpconfig::PY_BUILTINS_PROPERTY {
        table.push(type_elem("property", objproperty::type_property()));
    }
    if mpconfig::PY_BUILTINS_REVERSED {
        table.push(type_elem("reversed", objreversed::type_reversed()));
    }
    if mpconfig::PY_BUILTINS_SLICE {
        table.push(type_elem("slice", objslice::type_slice()));
    }
    if mpconfig::PY_BUILTINS_NOTIMPLEMENTED {
        if let Some(o) = objsingleton::const_notimplemented() {
            table.push(me("NotImplemented", o));
        }
    }
    if mpconfig::PY_BUILTINS_DIR {
        table.push(me(
            "dir",
            new_fun_builtin_var(0, 1, |n, a| builtin_dir(n, a)),
        ));
    }
    if mpconfig::CPYTHON_COMPAT {
        table.push(me("delattr", new_fun_builtin_2(builtin_delattr)));
    }
    if mpconfig::PY_ASYNC_AWAIT {
        table.push(type_elem(
            "StopAsyncIteration",
            objexcept::type_stop_async_iteration(),
        ));
    }
    if mpconfig::PY_BUILTINS_STR_UNICODE {
        table.push(type_elem("UnicodeError", objexcept::type_unicode_error()));
    }
    if mpconfig::EMIT_NATIVE {
        table.push(type_elem(
            "ViperTypeError",
            objexcept::type_viper_type_error(),
        ));
    }
    if mpconfig::PY_TSTRINGS {
        table.push(me(
            "__template__",
            new_fun_builtin_var(1, usize::MAX as u8, |n, a| objtemplate::new_template(n, a)),
        ));
    }
    if mpconfig::PY_BUILTINS_COMPILE {
        table.push(me("compile", builtinevex::builtin_compile_obj()));
    }
    if mpconfig::PY_BUILTINS_EVAL_EXEC {
        table.push(me("eval", builtinevex::builtin_eval_obj()));
        table.push(me("exec", builtinevex::builtin_exec_obj()));
    }
    if mpconfig::PY_BUILTINS_EXECFILE {
        table.push(me("execfile", builtinevex::builtin_execfile_obj()));
    }
    if mpconfig::PY_IO {
        table.push(me(
            "open",
            new_fun_builtin_kw_var(0, 0xff, |n, a, kw| {
                let mut kw = kw.clone();
                builtin::builtin_open(n, a, Some(&mut kw))
            }),
        ));
    }
    if mpconfig::PY_BUILTINS_HELP {
        table.push(me(
            "help",
            new_fun_builtin_var(0, 1, |n, a| builtin_help(n, a)),
        ));
    }
    if mpconfig::PY_BUILTINS_INPUT {
        table.push(me(
            "input",
            new_fun_builtin_var(0, 1, |n, a| builtin_input(n, a)),
        ));
    }
    if mpconfig::PY_BUILTINS_MIN_MAX {
        table.push(me(
            "max",
            new_fun_builtin_kw_var(1, 0xff, |n, a, kw| {
                let mut kw = kw.clone();
                builtin_max(n, a, &mut kw)
            }),
        ));
        table.push(me(
            "min",
            new_fun_builtin_kw_var(1, 0xff, |n, a, kw| {
                let mut kw = kw.clone();
                builtin_min(n, a, &mut kw)
            }),
        ));
    }

    table
}

static mut BUILTINS_MODULE: Option<Obj> = None;

fn builtins_module_obj() -> Obj {
    unsafe {
        if BUILTINS_MODULE.is_none() {
            BUILTINS_MODULE = Some(init_builtins_module());
        }
        BUILTINS_MODULE.unwrap()
    }
}

/// Fallback target for `mp_load_build_class` (mirrors `&mp_builtin___build_class___obj`).
pub fn builtin___build_class___obj() -> Obj {
    let module = builtins_module_obj();
    let globals = objmodule::module_get_globals(module);
    objdict::dict_get(
        obj::from_ptr(globals as *const ()),
        obj::new_qstr(qstr::from_str("__build_class__")),
    )
}

/// Create and register the `builtins` module (`init_builtins_module`).
pub fn init_builtins_module() -> Obj {
    let ctx = malloc::new_obj::<crate::bc::ModuleContext>().expect("builtins module alloc");
    let dict = objdict::new_dict(1);
    let table = build_globals_table();
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const crate::bc::ModuleContext as *const ());
    objmodule::register_builtins_globals(dict);
    objmodule::register_builtin_module(qstr::from_str("builtins"), module);
    module
}
