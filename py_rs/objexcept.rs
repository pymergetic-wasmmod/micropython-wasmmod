//! rewrite of py/objexcept.c + py/objexcept.h
// symmetry: done
use core::mem::size_of;

use crate::argcheck;
use crate::gc;
use crate::malloc;
use crate::moderrno;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::mpstate;
use crate::obj::{self, MakeNewFn, Obj, ObjBase, ObjType};
use crate::objstr;
use crate::objtuple::{self, ObjTuple};
use crate::objtype::{self, ObjInstance};
use crate::qstr::{self, Qstr};

/// Items per traceback entry (file, line, block).
const TRACEBACK_ENTRY_LEN: usize = 3;

/// Internal flag for printing exception subclasses (`PRINT_EXC_SUBCLASS`).
const PRINT_EXC_SUBCLASS: u8 = 0x80;

/// Native base init placeholder address (see `objtype.rs`).
const NATIVE_BASE_INIT_WRAPPER: Obj = Obj(0xdead_beef);

#[repr(C)]
pub struct ObjException {
    pub base: ObjBase,
    /// Low/high halves match C bitfields `traceback_len` / `traceback_alloc`.
    pub traceback_alloc_len: usize,
    pub traceback_data: *mut usize,
    pub args: *mut ObjTuple,
}

#[inline]
fn traceback_alloc(exc: &ObjException) -> usize {
    exc.traceback_alloc_len >> (8 * size_of::<usize>() / 2)
}

#[inline]
fn traceback_len(exc: &ObjException) -> usize {
    exc.traceback_alloc_len & ((1 << (8 * size_of::<usize>() / 2)) - 1)
}

#[inline]
fn set_traceback_alloc(exc: &mut ObjException, v: usize) {
    let half = 8 * size_of::<usize>() / 2;
    let mask = (1usize << half) - 1;
    exc.traceback_alloc_len = (v << half) | (exc.traceback_alloc_len & mask);
}

#[inline]
fn set_traceback_len(exc: &mut ObjException, v: usize) {
    let half = 8 * size_of::<usize>() / 2;
    let mask = (1usize << half) - 1;
    exc.traceback_alloc_len = (exc.traceback_alloc_len & !mask) | (v & mask);
}

// --- emergency exception buffer ------------------------------------------------

const EMG_BUF_TRACEBACK_SIZE: usize = 2 * TRACEBACK_ENTRY_LEN * size_of::<usize>();
const EMG_BUF_TUPLE_OFFSET: usize = EMG_BUF_TRACEBACK_SIZE;

fn emg_buf_tuple_size(n_args: usize) -> usize {
    size_of::<ObjTuple>() + n_args * size_of::<Obj>()
}

static mut EMERGENCY_EXCEPTION: ObjException = ObjException {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    traceback_alloc_len: 0,
    traceback_data: core::ptr::null_mut(),
    args: core::ptr::null_mut(),
};

static mut EMERGENCY_BUF: [u8; mpconfig::EMERGENCY_EXCEPTION_BUF_SIZE] =
    [0; mpconfig::EMERGENCY_EXCEPTION_BUF_SIZE];

// --- type slots ---------------------------------------------------------------

static mut EXCEPTION_SLOTS: [*const (); 3] = [
    exception_make_new as *const (),
    exception_print as *const (),
    exception_attr as *const (),
];

static TYPE_BASE_EXCEPTION: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_attr: 3,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { EXCEPTION_SLOTS.as_ptr() },
};

macro_rules! define_exception {
    ($acc:ident, $ty:ident, $slots:ident, $name:literal, $parent:expr) => {
        static mut $slots: [*const (); 4] = [
            exception_make_new as *const (),
            exception_print as *const (),
            exception_attr as *const (),
            $parent as *const ObjType as *const (),
        ];
        static $ty: ObjType = ObjType {
            base: ObjBase {
                type_: core::ptr::null(),
            },
            flags: obj::TYPE_FLAG_NONE,
            name: 0,
            slot_index_make_new: 1,
            slot_index_print: 2,
            slot_index_attr: 3,
            slot_index_call: 0,
            slot_index_unary_op: 0,
            slot_index_binary_op: 0,
            slot_index_subscr: 0,
            slot_index_iter: 0,
            slot_index_buffer: 0,
            slot_index_protocol: 0,
            slot_index_parent: 4,
            slot_index_locals_dict: 0,
            slots: unsafe { $slots.as_ptr() },
        };
        pub fn $acc() -> &'static ObjType {
            init_exception_types();
            &$ty
        }
    };
}

define_exception!(
    type_system_exit,
    TYPE_SYSTEM_EXIT,
    SLOTS_SYSTEM_EXIT,
    "SystemExit",
    &TYPE_BASE_EXCEPTION
);
define_exception!(
    type_keyboard_interrupt,
    TYPE_KEYBOARD_INTERRUPT,
    SLOTS_KEYBOARD_INTERRUPT,
    "KeyboardInterrupt",
    &TYPE_BASE_EXCEPTION
);
define_exception!(
    type_generator_exit,
    TYPE_GENERATOR_EXIT,
    SLOTS_GENERATOR_EXIT,
    "GeneratorExit",
    &TYPE_BASE_EXCEPTION
);
define_exception!(
    type_exception,
    TYPE_EXCEPTION,
    SLOTS_EXCEPTION,
    "Exception",
    &TYPE_BASE_EXCEPTION
);
define_exception!(
    type_stop_iteration,
    TYPE_STOP_ITERATION,
    SLOTS_STOP_ITERATION,
    "StopIteration",
    &TYPE_EXCEPTION
);
define_exception!(
    type_arithmetic_error,
    TYPE_ARITHMETIC_ERROR,
    SLOTS_ARITHMETIC_ERROR,
    "ArithmeticError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_overflow_error,
    TYPE_OVERFLOW_ERROR,
    SLOTS_OVERFLOW_ERROR,
    "OverflowError",
    &TYPE_ARITHMETIC_ERROR
);
define_exception!(
    type_zero_division_error,
    TYPE_ZERO_DIVISION_ERROR,
    SLOTS_ZERO_DIVISION_ERROR,
    "ZeroDivisionError",
    &TYPE_ARITHMETIC_ERROR
);
define_exception!(
    type_assertion_error,
    TYPE_ASSERTION_ERROR,
    SLOTS_ASSERTION_ERROR,
    "AssertionError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_attribute_error,
    TYPE_ATTRIBUTE_ERROR,
    SLOTS_ATTRIBUTE_ERROR,
    "AttributeError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_eof_error,
    TYPE_EOF_ERROR,
    SLOTS_EOF_ERROR,
    "EOFError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_import_error,
    TYPE_IMPORT_ERROR,
    SLOTS_IMPORT_ERROR,
    "ImportError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_lookup_error,
    TYPE_LOOKUP_ERROR,
    SLOTS_LOOKUP_ERROR,
    "LookupError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_index_error,
    TYPE_INDEX_ERROR,
    SLOTS_INDEX_ERROR,
    "IndexError",
    &TYPE_LOOKUP_ERROR
);
define_exception!(
    type_key_error,
    TYPE_KEY_ERROR,
    SLOTS_KEY_ERROR,
    "KeyError",
    &TYPE_LOOKUP_ERROR
);
define_exception!(
    type_memory_error,
    TYPE_MEMORY_ERROR,
    SLOTS_MEMORY_ERROR,
    "MemoryError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_name_error,
    TYPE_NAME_ERROR,
    SLOTS_NAME_ERROR,
    "NameError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_os_error,
    TYPE_OS_ERROR,
    SLOTS_OS_ERROR,
    "OSError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_runtime_error,
    TYPE_RUNTIME_ERROR,
    SLOTS_RUNTIME_ERROR,
    "RuntimeError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_not_implemented_error,
    TYPE_NOT_IMPLEMENTED_ERROR,
    SLOTS_NOT_IMPLEMENTED_ERROR,
    "NotImplementedError",
    &TYPE_RUNTIME_ERROR
);
define_exception!(
    type_syntax_error,
    TYPE_SYNTAX_ERROR,
    SLOTS_SYNTAX_ERROR,
    "SyntaxError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_indentation_error,
    TYPE_INDENTATION_ERROR,
    SLOTS_INDENTATION_ERROR,
    "IndentationError",
    &TYPE_SYNTAX_ERROR
);
define_exception!(
    type_type_error,
    TYPE_TYPE_ERROR,
    SLOTS_TYPE_ERROR,
    "TypeError",
    &TYPE_EXCEPTION
);
define_exception!(
    type_value_error,
    TYPE_VALUE_ERROR,
    SLOTS_VALUE_ERROR,
    "ValueError",
    &TYPE_EXCEPTION
);

define_exception!(
    type_stop_async_iteration,
    TYPE_STOP_ASYNC_ITERATION,
    SLOTS_STOP_ASYNC_ITERATION,
    "StopAsyncIteration",
    &TYPE_EXCEPTION
);

define_exception!(
    type_viper_type_error,
    TYPE_VIPER_TYPE_ERROR,
    SLOTS_VIPER_TYPE_ERROR,
    "ViperTypeError",
    &TYPE_TYPE_ERROR
);

define_exception!(
    type_unicode_error,
    TYPE_UNICODE_ERROR,
    SLOTS_UNICODE_ERROR,
    "UnicodeError",
    &TYPE_VALUE_ERROR
);

static EXCEPTION_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_exception_types() {
    EXCEPTION_INIT.get_or_init(|| {
        qstr::init();
        unsafe {
            EMERGENCY_EXCEPTION.base.type_ = &TYPE_BASE_EXCEPTION as *const ObjType;
            EMERGENCY_EXCEPTION.args =
                obj::as_ptr(objtuple::const_empty_tuple()) as *mut ObjTuple;
            mpstate::with_vm(|vm| {
                vm.mp_emergency_exception_obj =
                    obj::from_ptr(&raw const EMERGENCY_EXCEPTION as *const ObjException as *const ());
            });
        }
    });
}

/// `mp_type_BaseException`
pub fn type_base_exception() -> &'static ObjType {
    init_exception_types();
    &TYPE_BASE_EXCEPTION
}

// --- helpers ------------------------------------------------------------------

fn exception_ptr(o: Obj) -> *mut ObjException {
    obj::as_ptr(o) as *mut ObjException
}

fn get_native_exception(self_in: Obj) -> *mut ObjException {
    debug_assert!(is_exception_instance(self_in));
    if is_native_exception_instance(self_in) {
        return exception_ptr(self_in);
    }
    let inst = obj::as_ptr(self_in) as *const ObjInstance;
    unsafe {
        exception_ptr(objtype::subobj_get(inst, 0))
    }
}

fn emergency_traceback_ptr() -> *mut usize {
    if mpconfig::ENABLE_EMERGENCY_EXCEPTION_BUF {
        unsafe { EMERGENCY_BUF.as_mut_ptr() as *mut usize }
    } else {
        core::ptr::null_mut()
    }
}

fn emergency_tuple_ptr(n_args: usize) -> Option<*mut ObjTuple> {
    if !mpconfig::ENABLE_EMERGENCY_EXCEPTION_BUF {
        return None;
    }
    let needed = EMG_BUF_TUPLE_OFFSET + emg_buf_tuple_size(n_args);
    if needed <= mpconfig::EMERGENCY_EXCEPTION_BUF_SIZE {
        Some(unsafe {
            EMERGENCY_BUF
                .as_mut_ptr()
                .add(EMG_BUF_TUPLE_OFFSET) as *mut ObjTuple
        })
    } else {
        None
    }
}

/// `mp_init_emergency_exception_buf`
pub fn init_emergency_exception_buf() {
    init_exception_types();
}

// --- slot implementations -----------------------------------------------------

/// `mp_obj_exception_print`
pub fn exception_print(print: &Print, o_in: Obj, kind: PrintKind) {
    let o = unsafe { &*exception_ptr(o_in) };
    let raw = kind as u8;
    let k =
        unsafe { core::mem::transmute::<u8, PrintKind>(raw & !PRINT_EXC_SUBCLASS) };
    let is_subclass = raw & PRINT_EXC_SUBCLASS != 0;

    if !is_subclass && (k == PrintKind::Repr || k == PrintKind::Exc) {
        mpprint::print_str(print, &obj::get_type_str(o_in));
    }

    if k == PrintKind::Exc {
        mpprint::print_str(print, ": ");
    }

    if k == PrintKind::Str || k == PrintKind::Exc {
        let args = unsafe { &*o.args };
        if args.len == 0 {
            mpprint::print_str(print, "");
            return;
        }

        if mpconfig::PY_ERRNO
            && core::ptr::eq(o.base.type_, type_os_error() as *const ObjType)
            && args.len > 0
            && args.len < 3
            && obj::is_small_int(unsafe {
                *((o.args as *const u8).add(size_of::<ObjTuple>()) as *const Obj)
            })
        {
            let errno_val = unsafe {
                *((o.args as *const u8).add(size_of::<ObjTuple>()) as *const Obj)
            };
            let qst = moderrno::errno_to_str(errno_val);
            if qst != qstr::QSTR_NULL {
                mpprint::printf(
                    print,
                    &format!("[Errno {}] ", obj::small_int_value(errno_val)),
                    [],
                );
                if let Some(data) = qstr::str_data(qst) {
                    mpprint::print_str(
                        print,
                        core::str::from_utf8(&data).unwrap_or(""),
                    );
                }
                if args.len > 1 {
                    mpprint::print_str(print, ": ");
                    let item = unsafe {
                        *((o.args as *const u8)
                            .add(size_of::<ObjTuple>() + size_of::<Obj>())
                            as *const Obj)
                    };
                    obj::print_helper(print, item, PrintKind::Str);
                }
                return;
            }
        }

        if args.len == 1 {
            let item = unsafe {
                *((o.args as *const u8).add(size_of::<ObjTuple>()) as *const Obj)
            };
            obj::print_helper(print, item, PrintKind::Str);
            return;
        }
    }

    objtuple::tuple_print(print, obj::from_ptr(o.args as *const ObjTuple as *const ()), kind);
}

/// `mp_obj_exception_make_new`
pub fn exception_make_new(
    type_in: &'static ObjType,
    n_args: usize,
    n_kw: usize,
    args: &[Obj],
) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, usize::MAX, false);

    let o_exc = match malloc::new_obj::<ObjException>() {
        Some(ptr) => ptr,
        None => unsafe { &mut EMERGENCY_EXCEPTION },
    };

    unsafe {
        (*o_exc).base.type_ = type_in as *const ObjType;
        (*o_exc).traceback_data = core::ptr::null_mut();
        set_traceback_alloc(&mut *o_exc, 0);
        set_traceback_len(&mut *o_exc, 0);

        let o_tuple = if n_args == 0 {
            objtuple::const_empty_tuple()
        } else {
            let extra = n_args * size_of::<Obj>();
            let size = size_of::<ObjTuple>() + extra;
            if let Some(base) = gc::gc_alloc(size, 0) {
                let tuple_ptr = base as *mut ObjTuple;
                unsafe {
                    (*tuple_ptr).base.type_ = objtuple::type_tuple() as *const ObjType;
                    (*tuple_ptr).len = n_args;
                    core::ptr::copy_nonoverlapping(
                        args.as_ptr(),
                        (tuple_ptr as *mut u8).add(size_of::<ObjTuple>()) as *mut Obj,
                        n_args,
                    );
                }
                obj::from_ptr(tuple_ptr as *const ObjTuple as *const ())
            } else if let Some(tuple_ptr) = emergency_tuple_ptr(n_args) {
                unsafe {
                    (*tuple_ptr).base.type_ = objtuple::type_tuple() as *const ObjType;
                    (*tuple_ptr).len = n_args;
                    core::ptr::copy_nonoverlapping(
                        args.as_ptr(),
                        (tuple_ptr as *mut u8).add(size_of::<ObjTuple>()) as *mut Obj,
                        n_args,
                    );
                }
                obj::from_ptr(tuple_ptr as *const ObjTuple as *const ())
            } else {
                objtuple::const_empty_tuple()
            }
        };
        (*o_exc).args = obj::as_ptr(o_tuple) as *mut ObjTuple;
        obj::from_ptr(o_exc as *const ObjException as *const ())
    }
}

/// `mp_obj_exception_get_value`
pub fn exception_get_value(self_in: Obj) -> Obj {
    let self_ = unsafe { &*get_native_exception(self_in) };
    let args = unsafe { &*self_.args };
    if args.len == 0 {
        obj::CONST_NONE
    } else {
        unsafe {
            *((self_.args as *const u8).add(size_of::<ObjTuple>()) as *const Obj)
        }
    }
}

/// `mp_obj_exception_attr`
pub fn exception_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    let self_ = unsafe { &mut *exception_ptr(self_in) };
    if dest[0] != obj::OBJ_NULL {
        if attr == qstr::from_str("__traceback__") && dest[1] == obj::CONST_NONE {
            set_traceback_len(self_, 0);
            dest[0] = obj::OBJ_NULL;
        }
        return;
    }
    if attr == qstr::from_str("args") {
        dest[0] = obj::from_ptr(self_.args as *const ObjTuple as *const ());
    } else if attr == qstr::from_str("value") || attr == qstr::from_str("errno") {
        dest[0] = exception_get_value(self_in);
    }
}

// --- public API ---------------------------------------------------------------

fn type_from_obj(o: Obj) -> Option<&'static ObjType> {
    if !obj::is_obj(o) {
        return None;
    }
    Some(unsafe { &*(obj::as_ptr(o) as *const ObjType) })
}

fn is_exception_subclass(type_: &'static ObjType, classinfo: &'static ObjType) -> bool {
    let mut cur = type_;
    loop {
        if core::ptr::eq(cur, classinfo) {
            return true;
        }
        if !obj::type_has_slot(cur, cur.slot_index_parent) {
            return false;
        }
        let parent = match obj::type_get_slot_parent(cur) {
            Some(p) => type_from_obj(p).unwrap_or(obj::type_object()),
            None => return false,
        };
        cur = parent;
    }
}

/// `mp_obj_is_native_exception_instance`
pub fn is_native_exception_instance(self_in: Obj) -> bool {
    if !obj::is_obj(self_in) {
        return false;
    }
    let base = unsafe { &*(obj::as_ptr(self_in) as *const ObjBase) };
    if base.type_.is_null() {
        return false;
    }
    obj::type_get_make_new(unsafe { &*base.type_ }) == Some(exception_make_new as MakeNewFn)
}

/// `mp_obj_is_exception_type`
pub fn is_exception_type(self_in: Obj) -> bool {
    let Some(self_) = type_from_obj(self_in) else {
        return false;
    };
    if obj::type_get_make_new(self_) == Some(exception_make_new as MakeNewFn) {
        return true;
    }
    if obj::is_exact_type(self_in, obj::type_type()) {
        return is_exception_subclass(self_, type_base_exception());
    }
    false
}

/// `mp_obj_is_exception_instance`
pub fn is_exception_instance(self_in: Obj) -> bool {
    if is_native_exception_instance(self_in) {
        return true;
    }
    if !obj::is_obj(self_in) {
        return false;
    }
    let base = unsafe { &*(obj::as_ptr(self_in) as *const ObjBase) };
    if base.type_.is_null() {
        return false;
    }
    if !is_exception_type(obj::from_ptr(base.type_ as *const ObjType as *const ())) {
        return false;
    }
    let native = objtype::cast_to_native_base(
        self_in,
        obj::from_ptr(type_base_exception() as *const ObjType as *const ()),
    );
    native != obj::OBJ_NULL
        && native != NATIVE_BASE_INIT_WRAPPER
        && is_native_exception_instance(native)
}

/// `mp_obj_exception_match`
pub fn exception_match(exc: Obj, exc_type: Obj) -> bool {
    let Some(classinfo) = type_from_obj(exc_type) else {
        return false;
    };
    let check_type = if is_native_exception_instance(exc) {
        obj::get_type(exc)
    } else if let Some(t) = type_from_obj(exc) {
        if obj::type_get_make_new(t) == Some(exception_make_new as MakeNewFn) {
            t
        } else {
            return false;
        }
    } else if is_exception_instance(exc) {
        obj::get_type(exc)
    } else {
        return false;
    };
    is_exception_subclass(check_type, classinfo)
}

/// `mp_obj_new_exception`
pub fn new_exception(exc_type: &'static ObjType) -> Obj {
    debug_assert!(obj::type_get_make_new(exc_type) == Some(exception_make_new as MakeNewFn));
    exception_make_new(exc_type, 0, 0, &[])
}

/// `mp_obj_new_exception_args`
pub fn new_exception_args(exc_type: &'static ObjType, n_args: usize, args: &[Obj]) -> Obj {
    debug_assert!(obj::type_get_make_new(exc_type) == Some(exception_make_new as MakeNewFn));
    exception_make_new(exc_type, n_args, 0, args)
}

/// `mp_obj_exception_clear_traceback`
pub fn exception_clear_traceback(self_in: Obj) {
    let self_ = unsafe { &mut *get_native_exception(self_in) };
    self_.traceback_data = core::ptr::null_mut();
    set_traceback_len(self_, 0);
    set_traceback_alloc(self_, 0);
}

/// `mp_obj_exception_add_traceback`
pub fn exception_add_traceback(self_in: Obj, file: Qstr, line: usize, block: Qstr) {
    let self_ = unsafe { &mut *get_native_exception(self_in) };

    if self_.traceback_data.is_null() {
        match malloc::new::<usize>(TRACEBACK_ENTRY_LEN) {
            Some(tb) => {
                self_.traceback_data = tb;
                set_traceback_alloc(self_, TRACEBACK_ENTRY_LEN);
            }
            None => {
                if !mpconfig::ENABLE_EMERGENCY_EXCEPTION_BUF {
                    return;
                }
                self_.traceback_data = emergency_traceback_ptr();
                set_traceback_alloc(self_, 2 * TRACEBACK_ENTRY_LEN);
            }
        }
        set_traceback_len(self_, 0);
    } else if traceback_len(self_) + TRACEBACK_ENTRY_LEN > traceback_alloc(self_) {
        if mpconfig::ENABLE_EMERGENCY_EXCEPTION_BUF
            && self_.traceback_data == emergency_traceback_ptr()
        {
            return;
        }
        let old_bytes = traceback_alloc(self_) * size_of::<usize>();
        let new_alloc = traceback_alloc(self_) + TRACEBACK_ENTRY_LEN;
        let new_bytes = new_alloc * size_of::<usize>();
        match malloc::renew_maybe(
            self_.traceback_data,
            old_bytes,
            new_bytes,
            true,
        ) {
            Some(tb) => {
                self_.traceback_data = tb;
                set_traceback_alloc(self_, new_alloc);
            }
            None => return,
        }
    }

    let tb_data = unsafe {
        self_.traceback_data.add(traceback_len(self_))
    };
    set_traceback_len(self_, traceback_len(self_) + TRACEBACK_ENTRY_LEN);
    unsafe {
        *tb_data.add(0) = file;
        *tb_data.add(1) = line;
        *tb_data.add(2) = block;
    }
}

/// `mp_obj_exception_get_traceback`
pub fn exception_get_traceback(self_in: Obj, n: &mut usize, values: &mut *mut usize) {
    let self_ = unsafe { &*get_native_exception(self_in) };
    if self_.traceback_data.is_null() {
        *n = 0;
        *values = core::ptr::null_mut();
    } else {
        *n = traceback_len(self_);
        *values = self_.traceback_data;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::mpprint;

    fn setup() {
        let _ = gc::init();
        mpstate::init();
        init_exception_types();
    }

    fn print_to_string(o: Obj, kind: PrintKind) -> String {
        let mut out = Vec::new();
        let mut print = Print {
            data: &mut out as *mut Vec<u8> as *mut (),
            print_strn: Some(collect_print),
        };
        exception_print(&print, o, kind);
        String::from_utf8(out).unwrap()
    }

    extern "C" fn collect_print(data: *mut (), str: *const u8, len: usize) {
        let out = unsafe { &mut *(data as *mut Vec<u8>) };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(str, len) });
    }

    #[test]
    fn new_exception_empty() {
        setup();
        let exc = new_exception(type_value_error());
        assert!(is_native_exception_instance(exc));
        assert!(is_exception_instance(exc));
        assert_eq!(exception_get_value(exc), obj::CONST_NONE);
    }

    #[test]
    fn new_exception_with_args() {
        setup();
        let msg = objstr::new_str(b"bad value");
        let exc = new_exception_args(type_value_error(), 1, &[msg]);
        assert_eq!(exception_get_value(exc), msg);
        let (len, items) = objtuple::tuple_get(
            obj::from_ptr(
                unsafe { &*exception_ptr(exc) }.args as *const ObjTuple as *const (),
            ),
        );
        assert_eq!(len, 1);
        assert_eq!(items[0], msg);
    }

    #[test]
    fn exception_attr_args_and_value() {
        setup();
        let msg = objstr::new_str(b"oops");
        let exc = new_exception_args(type_runtime_error(), 1, &[msg]);
        let mut dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        exception_attr(exc, qstr::from_str("args"), &mut dest);
        let (len, _) = objtuple::tuple_get(dest[0]);
        assert_eq!(len, 1);
        dest = [obj::OBJ_NULL, obj::OBJ_NULL];
        exception_attr(exc, qstr::from_str("value"), &mut dest);
        assert_eq!(dest[0], msg);
    }

    #[test]
    fn is_exception_type_hierarchy() {
        setup();
        assert!(is_exception_type(obj::from_ptr(
            type_base_exception() as *const ObjType as *const ()
        )));
        assert!(is_exception_type(obj::from_ptr(
            type_value_error() as *const ObjType as *const ()
        )));
        assert!(!is_exception_type(obj::from_ptr(
            obj::type_int() as *const ObjType as *const ()
        )));
    }

    #[test]
    fn exception_match_subclasses() {
        setup();
        let exc = new_exception(type_key_error());
        assert!(exception_match(
            exc,
            obj::from_ptr(type_lookup_error() as *const ObjType as *const ())
        ));
        assert!(exception_match(
            exc,
            obj::from_ptr(type_exception() as *const ObjType as *const ())
        ));
        assert!(!exception_match(
            exc,
            obj::from_ptr(type_type_error() as *const ObjType as *const ())
        ));
        assert!(exception_match(
            obj::from_ptr(type_index_error() as *const ObjType as *const ()),
            obj::from_ptr(type_exception() as *const ObjType as *const ())
        ));
    }

    #[test]
    fn traceback_add_get_clear() {
        setup();
        let exc = new_exception(type_runtime_error());
        let file = qstr::from_str("main.py");
        let block = qstr::from_str("<module>");
        exception_add_traceback(exc, file, 10, block);
        exception_add_traceback(exc, file, 20, block);
        let mut n = 0usize;
        let mut values = core::ptr::null_mut();
        exception_get_traceback(exc, &mut n, &mut values);
        assert_eq!(n, 2 * TRACEBACK_ENTRY_LEN);
        unsafe {
            assert_eq!(*values.add(0), file);
            assert_eq!(*values.add(1), 10);
            assert_eq!(*values.add(2), block);
            assert_eq!(*values.add(3), file);
            assert_eq!(*values.add(4), 20);
        }
        let mut dest = [obj::OBJ_SENTINEL, obj::CONST_NONE];
        exception_attr(exc, qstr::from_str("__traceback__"), &mut dest);
        assert_eq!(dest[0], obj::OBJ_NULL);
        let self_ = unsafe { &*exception_ptr(exc) };
        assert_eq!(traceback_len(&*self_), 0);
        exception_clear_traceback(exc);
        exception_get_traceback(exc, &mut n, &mut values);
        assert_eq!(n, 0);
        assert!(values.is_null());
    }

    #[test]
    fn exception_print_str_and_exc() {
        setup();
        let msg = objstr::new_str(b"fail");
        let exc = new_exception_args(type_type_error(), 1, &[msg]);
        assert_eq!(print_to_string(exc, PrintKind::Str), "fail");
        let printed = print_to_string(exc, PrintKind::Exc);
        assert!(printed.ends_with("fail"));
    }

    #[test]
    fn make_new_via_type_slot() {
        setup();
        let msg = objstr::new_str(b"slot");
        let exc = exception_make_new(type_value_error(), 1, 0, &[msg]);
        assert!(is_native_exception_instance(exc));
        assert_eq!(exception_get_value(exc), msg);
    }
}
