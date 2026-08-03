//! rewrite of py/objarray.c + py/objarray.h
// symmetry: done

use core::mem::size_of;

use crate::argcheck;
use crate::binary::{self, BYTEARRAY_TYPECODE};
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{
    self, BufferInfo, IterNextFn, Obj, ObjBase, ObjIterBuf, ObjType, OBJ_SENTINEL,
    TYPE_FLAG_EQ_CHECKS_OTHER_TYPE, TYPE_FLAG_SUBSCR_ALLOWS_STACK_SLICE,
};
use crate::objdict::{self, ObjDict};
use crate::objfloat;
use crate::objpolyiter;
use crate::objslice;
use crate::objstr::{self, find_subbytes};
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};
use crate::sequence;

/// Writable memoryview flag (`MP_OBJ_ARRAY_TYPECODE_FLAG_RW`).
pub const OBJ_ARRAY_TYPECODE_FLAG_RW: u8 = 0x80;

const TYPECODE_MASK: u8 = if mpconfig::PY_BUILTINS_MEMORYVIEW {
    0x7f
} else {
    0xff
};

const MEMVIEW_OFFSET_MAX: usize = (1usize << (8 * size_of::<usize>() - 8)) - 1;

#[repr(C)]
pub struct ObjArray {
    pub base: ObjBase,
    pub typecode: u8,
    /// Spare capacity (elements) for array/bytearray; offset (elements) for memoryview.
    pub free: usize,
    pub len: usize,
    pub items: *mut u8,
}

// --- minimal builtin method wrappers (MP_DEFINE_CONST_FUN_OBJ_*) ----------------

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
    slots: unsafe { FUN_BUILTIN_1_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_2: ObjType = ObjType {
    base: ObjBase {
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
    slots: unsafe { FUN_BUILTIN_2_SLOTS.as_ptr() },
};

static TYPE_FUN_BUILTIN_VAR: ObjType = ObjType {
    base: ObjBase {
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

fn me(name: &str, value: Obj) -> MapElem {
    MapElem {
        key: obj::new_qstr(qstr::from_str(name)),
        value,
    }
}

// --- array iterator -----------------------------------------------------------

#[repr(C)]
struct ObjArrayIter {
    base: ObjBase,
    iternext: IterNextFn,
    array: Obj,
    cur: usize,
}

fn array_it_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut ObjArrayIter) };
    let array = unsafe { &*(obj::as_ptr(self_.array) as *const ObjArray) };
    if self_.cur < array.len {
        let offset = if is_memoryview_obj(self_.array) {
            array.free
        } else {
            0
        };
        let idx = offset + self_.cur;
        self_.cur += 1;
        let tc = array.typecode & TYPECODE_MASK;
        let item_sz = binary::get_size(b'@', tc, None);
        let data = unsafe { std::slice::from_raw_parts(array.items, (idx + 1) * item_sz) };
        binary::get_val_array(tc, data, idx)
    } else {
        obj::OBJ_STOP_ITERATION
    }
}

// --- type objects -------------------------------------------------------------

static mut ARRAY_SLOTS: [*const (); 8] = [
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
];

static mut BYTEARRAY_SLOTS: [*const (); 8] = [
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
];

static mut TYPE_ARRAY: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_attr: 0,
    slot_index_subscr: 5,
    slot_index_iter: 6,
    slot_index_buffer: 7,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 8,
    slots: unsafe { ARRAY_SLOTS.as_ptr() },
};

static mut TYPE_BYTEARRAY: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_EQ_CHECKS_OTHER_TYPE | TYPE_FLAG_SUBSCR_ALLOWS_STACK_SLICE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 3,
    slot_index_binary_op: 4,
    slot_index_attr: 0,
    slot_index_subscr: 5,
    slot_index_iter: 6,
    slot_index_buffer: 7,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 8,
    slots: unsafe { BYTEARRAY_SLOTS.as_ptr() },
};

static mut MEMORYVIEW_SLOTS: [*const (); 8] = [
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
    core::ptr::null(),
];

static mut TYPE_MEMORYVIEW: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_EQ_CHECKS_OTHER_TYPE | TYPE_FLAG_SUBSCR_ALLOWS_STACK_SLICE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 2,
    slot_index_binary_op: 3,
    slot_index_attr: if mpconfig::PY_BUILTINS_MEMORYVIEW_ITEMSIZE {
        7
    } else {
        0
    },
    slot_index_subscr: 4,
    slot_index_iter: 5,
    slot_index_buffer: 6,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: if mpconfig::PY_BUILTINS_BYTES_HEX {
        8
    } else {
        0
    },
    slots: unsafe { MEMORYVIEW_SLOTS.as_ptr() },
};

static ARRAY_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static MEMORYVIEW_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_array_types() {
    if !(mpconfig::PY_ARRAY || mpconfig::PY_BUILTINS_BYTEARRAY) {
        return;
    }
    ARRAY_INIT.get_or_init(|| unsafe {
        if mpconfig::PY_ARRAY {
            (*(core::ptr::addr_of_mut!(TYPE_ARRAY) as *mut ObjType)).name = qstr::from_str("array");
        }
        if mpconfig::PY_BUILTINS_BYTEARRAY {
            (*(core::ptr::addr_of_mut!(TYPE_BYTEARRAY) as *mut ObjType)).name =
                qstr::from_str("bytearray");
        }
        if mpconfig::PY_ARRAY {
            ARRAY_SLOTS[0] = array_make_new as *const ();
            ARRAY_SLOTS[1] = array_print as *const ();
            ARRAY_SLOTS[2] = array_unary_op as *const ();
            ARRAY_SLOTS[3] = array_binary_op as *const ();
            ARRAY_SLOTS[4] = array_subscr as *const ();
            ARRAY_SLOTS[5] = array_iterator_new as *const ();
            ARRAY_SLOTS[6] = array_get_buffer as *const ();
        }
        if mpconfig::PY_BUILTINS_BYTEARRAY {
            BYTEARRAY_SLOTS[0] = bytearray_make_new as *const ();
            BYTEARRAY_SLOTS[1] = array_print as *const ();
            BYTEARRAY_SLOTS[2] = array_unary_op as *const ();
            BYTEARRAY_SLOTS[3] = array_binary_op as *const ();
            BYTEARRAY_SLOTS[4] = array_subscr as *const ();
            BYTEARRAY_SLOTS[5] = array_iterator_new as *const ();
            BYTEARRAY_SLOTS[6] = array_get_buffer as *const ();
        }

        let mut table = vec![
            me("append", new_fun_builtin_2(array_append)),
            me("extend", new_fun_builtin_2(array_extend)),
        ];
        if mpconfig::PY_BUILTINS_BYTES_HEX {
            table.push(me(
                "hex",
                new_fun_builtin_var(1, 2, crate::objstr::bytearray_hex_method),
            ));
            table.push(me(
                "fromhex",
                crate::objstr::bytes_fromhex_classmethod_obj(),
            ));
        }
        if mpconfig::CPYTHON_COMPAT {
            table.push(me(
                "decode",
                new_fun_builtin_var(1, 3, crate::objstr::bytearray_decode_method),
            ));
        }
        // Shared with bytes/str (C `mp_obj_bytearray_locals_dict`).
        table.extend(crate::objstr::str_bytes_shared_methods());
        let ptr = obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        map::init_fixed_table(&mut (*ptr).map, table);
        let dict_ptr = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        if mpconfig::PY_ARRAY {
            // array.array: append/extend only (slice of shared C table).
            let arr_table = vec![
                me("append", new_fun_builtin_2(array_append)),
                me("extend", new_fun_builtin_2(array_extend)),
            ];
            let aptr =
                obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
            map::init_fixed_table(&mut (*aptr).map, arr_table);
            ARRAY_SLOTS[7] = obj::from_ptr(aptr as *const ObjDict as *const ()).0 as *const ();
        }
        if mpconfig::PY_BUILTINS_BYTEARRAY {
            BYTEARRAY_SLOTS[7] = dict_ptr;
            // Pin locals: static type is not a GC root; Map.table is a Rust Vec.
            crate::gc::add_root(ptr as *mut u8);
            for elem in &(*ptr).map.table {
                if elem.key != obj::OBJ_NULL
                    && elem.key != obj::OBJ_SENTINEL
                    && obj::is_obj(elem.value)
                {
                    crate::gc::add_root(obj::to_ptr(elem.value) as *mut u8);
                }
            }
        }
    });
}

fn init_memoryview_type() {
    if !mpconfig::PY_BUILTINS_MEMORYVIEW {
        return;
    }
    MEMORYVIEW_INIT.get_or_init(|| unsafe {
        (*(core::ptr::addr_of_mut!(TYPE_MEMORYVIEW) as *mut ObjType)).name =
            qstr::from_str("memoryview");
        MEMORYVIEW_SLOTS[0] = memoryview_make_new as *const ();
        MEMORYVIEW_SLOTS[1] = array_unary_op as *const ();
        MEMORYVIEW_SLOTS[2] = array_binary_op as *const ();
        MEMORYVIEW_SLOTS[3] = array_subscr as *const ();
        MEMORYVIEW_SLOTS[4] = array_iterator_new as *const ();
        MEMORYVIEW_SLOTS[5] = array_get_buffer as *const ();
        if mpconfig::PY_BUILTINS_MEMORYVIEW_ITEMSIZE {
            MEMORYVIEW_SLOTS[6] = memoryview_attr as *const ();
        }
        if mpconfig::PY_BUILTINS_BYTES_HEX {
            let table = vec![me("hex", new_fun_builtin_var(1, 2, memoryview_hex_method))];
            let ptr =
                obj::malloc_helper(size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
            map::init_fixed_table(&mut (*ptr).map, table);
            MEMORYVIEW_SLOTS[7] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
        }
    });
}

pub fn type_array() -> &'static ObjType {
    init_array_types();
    unsafe { &*core::ptr::addr_of!(TYPE_ARRAY) }
}

pub fn type_bytearray() -> &'static ObjType {
    init_array_types();
    unsafe { &*core::ptr::addr_of!(TYPE_BYTEARRAY) }
}

pub fn type_memoryview() -> &'static ObjType {
    init_memoryview_type();
    unsafe { &*core::ptr::addr_of!(TYPE_MEMORYVIEW) }
}

// --- helpers ------------------------------------------------------------------

fn array_ptr(o: Obj) -> *mut ObjArray {
    obj::as_ptr(o) as *mut ObjArray
}

fn is_memoryview_obj(o: Obj) -> bool {
    mpconfig::PY_BUILTINS_MEMORYVIEW && obj::is_exact_type(o, type_memoryview())
}

fn is_array_like(o: Obj) -> bool {
    (mpconfig::PY_ARRAY && obj::is_exact_type(o, type_array()))
        || (mpconfig::PY_BUILTINS_BYTEARRAY && obj::is_exact_type(o, type_bytearray()))
        || is_memoryview_obj(o)
}

fn array_type_for(typecode: u8) -> &'static ObjType {
    if mpconfig::PY_BUILTINS_BYTEARRAY && mpconfig::PY_ARRAY {
        if typecode == BYTEARRAY_TYPECODE {
            type_bytearray()
        } else {
            type_array()
        }
    } else if mpconfig::PY_BUILTINS_BYTEARRAY {
        type_bytearray()
    } else {
        type_array()
    }
}

fn seq_clear(items: *mut u8, len: usize, alloc_len: usize, item_sz: usize) {
    unsafe {
        std::ptr::write_bytes(items.add(len * item_sz), 0, (alloc_len - len) * item_sz);
    }
}

fn seq_copy(dest: *mut u8, src: *const u8, byte_len: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src, dest, byte_len);
    }
}

fn seq_cat(dest: *mut u8, src1: *const u8, len1: usize, src2: *const u8, len2: usize) {
    unsafe {
        std::ptr::copy_nonoverlapping(src1, dest, len1);
        std::ptr::copy_nonoverlapping(src2, dest.add(len1), len2);
    }
}

fn seq_replace_no_grow(
    dest: *mut u8,
    dest_len: usize,
    beg: usize,
    end: usize,
    slice: *const u8,
    slice_len: usize,
    item_sz: usize,
) {
    unsafe {
        let dest = dest as *mut u8;
        core::ptr::copy(slice, dest.add(beg * item_sz), slice_len * item_sz);
        core::ptr::copy(
            dest.add(end * item_sz),
            dest.add((beg + slice_len) * item_sz),
            (dest_len - end) * item_sz,
        );
    }
}

fn seq_replace_grow_inplace(
    dest: *mut u8,
    dest_len: usize,
    beg: usize,
    end: usize,
    slice: *const u8,
    slice_len: usize,
    len_adj: isize,
    item_sz: usize,
) {
    unsafe {
        let dest = dest as *mut u8;
        core::ptr::copy(
            dest.add(end * item_sz),
            dest.add((beg + slice_len) * item_sz),
            ((dest_len as isize + len_adj - (beg as isize + slice_len as isize)) as usize)
                * item_sz,
        );
        core::ptr::copy(slice, dest.add(beg * item_sz), slice_len * item_sz);
    }
}

fn array_new(typecode: u8, n: usize) -> *mut ObjArray {
    let item_sz = binary::get_size(b'@', typecode, None);
    let o = malloc::new_obj::<ObjArray>().expect("objarray alloc");
    unsafe {
        (*o).base.type_ = array_type_for(typecode) as *const ObjType;
        (*o).typecode = typecode;
        (*o).free = 0;
        (*o).len = n;
        (*o).items = if n == 0 {
            core::ptr::null_mut()
        } else {
            malloc::new::<u8>(item_sz * n).expect("objarray items alloc")
        };
    }
    o
}

fn array_extend_impl(array: *mut ObjArray, arg: Obj, typecode: u8, len: usize) {
    let mut iter_buf = ObjIterBuf {
        base: ObjBase {
            type_: core::ptr::null(),
        },
        buf: [obj::OBJ_NULL; 3],
    };
    let iterable = runtime::getiter(arg, Some(&mut iter_buf));
    let item_sz = binary::get_size(b'@', typecode, None);
    let mut i = 0usize;
    loop {
        let item = runtime::iternext(iterable);
        if item == obj::OBJ_STOP_ITERATION {
            break;
        }
        if len == 0 {
            array_append(obj::from_ptr(array as *const ObjArray as *const ()), item);
        } else {
            let cap = len * item_sz;
            let items = unsafe { std::slice::from_raw_parts_mut((*array).items, cap) };
            binary::set_val_array(typecode, items, i, item);
            i += 1;
        }
    }
}

fn array_construct(typecode: u8, initializer: Obj) -> Obj {
    let mut buf = BufferInfo::default();
    let can_raw = (mpconfig::PY_BUILTINS_BYTEARRAY && typecode == BYTEARRAY_TYPECODE)
        || (mpconfig::PY_ARRAY
            && (obj::is_exact_type(initializer, objstr::type_bytes())
                || (mpconfig::PY_BUILTINS_BYTEARRAY
                    && obj::is_exact_type(initializer, type_bytearray()))));
    if can_raw && obj::get_buffer(initializer, &mut buf, obj::BUFFER_READ) {
        let sz = binary::get_size(b'@', typecode, None);
        let len = buf.len / sz;
        let o = array_new(typecode, len);
        unsafe {
            std::ptr::copy_nonoverlapping(buf.buf, (*o).items, len * sz);
        }
        return obj::from_ptr(o as *const ObjArray as *const ());
    }

    let len = if let Some(len_in) = obj::len_maybe(initializer) {
        obj::small_int_value(len_in) as usize
    } else {
        0
    };

    let array = array_new(typecode, len);
    array_extend_impl(array, initializer, typecode, len);
    obj::from_ptr(array as *const ObjArray as *const ())
}

fn typecode_for_comparison(typecode: u8, is_unsigned: &mut bool) -> u8 {
    let mut tc = typecode;
    if tc == BYTEARRAY_TYPECODE {
        tc = b'B';
    }
    if tc <= b'Z' {
        tc += 32;
        *is_unsigned = true;
    }
    tc
}

// --- type slots ---------------------------------------------------------------

fn array_print(print: &Print, o_in: Obj, kind: PrintKind) {
    let _ = kind;
    let o = unsafe { &*(array_ptr(o_in) as *const ObjArray) };
    if o.typecode == BYTEARRAY_TYPECODE {
        mpprint::print_str(print, "bytearray(b");
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        objstr::str_print_quoted(print, data, true);
    } else {
        let _ = mpprint::printf(print, "array('%c'", [mpprint::VaArg::Char(o.typecode)]);
        if o.len > 0 {
            mpprint::print_str(print, ", [");
            let item_sz = binary::get_size(b'@', o.typecode, None);
            let data = unsafe { std::slice::from_raw_parts(o.items, (o.len + o.free) * item_sz) };
            for i in 0..o.len {
                if i > 0 {
                    mpprint::print_str(print, ", ");
                }
                obj::print_helper(
                    print,
                    binary::get_val_array(o.typecode, data, i),
                    PrintKind::Repr,
                );
            }
            mpprint::print_str(print, "]");
        }
    }
    mpprint::print_str(print, ")");
}

fn array_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 2, false);
    let typecode = objstr::str_get_qstr(args[0]);
    let tc_bytes = qstr::str_from_qstr(typecode).unwrap_or_default();
    let tc = tc_bytes.as_bytes()[0];
    if n_args == 1 {
        obj::from_ptr(array_new(tc, 0) as *const ObjArray as *const ())
    } else {
        array_construct(tc, args[1])
    }
}

fn bytearray_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 3, false);
    if n_args == 0 {
        return obj::from_ptr(array_new(BYTEARRAY_TYPECODE, 0) as *const ObjArray as *const ());
    }
    if obj::is_int(args[0]) {
        let len = obj::get_uint(args[0]);
        let o = array_new(BYTEARRAY_TYPECODE, len);
        if !mpconfig::GC_CONSERVATIVE_CLEAR {
            unsafe {
                std::ptr::write_bytes((*o).items, 0, len);
            }
        }
        return obj::from_ptr(o as *const ObjArray as *const ());
    }
    if obj::is_str(args[0]) && n_args == 1 {
        if mpconfig::ERROR_REPORTING <= mpconfig::ERROR_REPORTING_NORMAL {
            raise::raise(MpRaise::TypeError("wrong number of arguments"));
        } else {
            raise::raise(MpRaise::TypeError("string argument without an encoding"));
        }
    }
    array_construct(BYTEARRAY_TYPECODE, args[0])
}

/// `mp_obj_memoryview_init`
pub fn memoryview_init(
    self_: *mut ObjArray,
    typecode: u8,
    offset: usize,
    len: usize,
    items: *mut u8,
) {
    unsafe {
        (*self_).base.type_ = type_memoryview() as *const ObjType;
        (*self_).typecode = typecode;
        (*self_).free = offset;
        (*self_).len = len;
        (*self_).items = items;
    }
}

/// `mp_obj_new_memoryview`
pub fn new_memoryview(typecode: u8, nitems: usize, items: *mut u8) -> Obj {
    let self_ = malloc::new_obj::<ObjArray>().expect("memoryview alloc");
    memoryview_init(self_, typecode, 0, nitems, items);
    obj::from_ptr(self_ as *const ObjArray as *const ())
}

fn memoryview_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut bufinfo, obj::BUFFER_READ);
    let tc = bufinfo.typecode as u8;
    let sz = binary::get_size(b'@', tc, None);
    let self_ = new_memoryview(tc, bufinfo.len / sz, bufinfo.buf);
    if is_memoryview_obj(args[0]) {
        let other = unsafe { &*(array_ptr(args[0]) as *const ObjArray) };
        let s = array_ptr(self_);
        unsafe {
            (*s).free = other.free;
            (*s).items = other.items;
        }
    }
    if obj::get_buffer(args[0], &mut bufinfo, obj::BUFFER_RW) {
        let s = array_ptr(self_);
        unsafe {
            (*s).typecode |= OBJ_ARRAY_TYPECODE_FLAG_RW;
        }
    }
    self_
}

fn memoryview_attr(self_in: Obj, attr: qstr::Qstr, dest: &mut [Obj; 2]) {
    if dest[0] != obj::OBJ_NULL {
        return;
    }
    if mpconfig::PY_BUILTINS_MEMORYVIEW_ITEMSIZE && attr == qstr::from_str("itemsize") {
        let self_ = unsafe { &*(array_ptr(self_in) as *const ObjArray) };
        dest[0] = obj::new_small_int(
            binary::get_size(b'@', self_.typecode & TYPECODE_MASK, None) as obj::Int
        );
    } else if mpconfig::PY_BUILTINS_BYTES_HEX {
        dest[1] = OBJ_SENTINEL;
    }
}

fn memoryview_hex(n_args: usize, args: &[Obj]) -> Obj {
    objstr::bytearray_hex_method(n_args, args)
}

fn memoryview_hex_method(n_args: usize, args: &[Obj]) -> Obj {
    memoryview_hex(n_args, args)
}

fn array_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    let o = unsafe { &*(array_ptr(o_in) as *const ObjArray) };
    match op {
        UnaryOp::Bool => obj::new_bool(o.len != 0),
        UnaryOp::Len => obj::new_small_int(o.len as obj::Int),
        _ => obj::OBJ_NULL,
    }
}

fn array_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let lhs = unsafe { &*(array_ptr(lhs_in) as *const ObjArray) };
    match op {
        BinaryOp::Add => {
            if is_memoryview_obj(lhs_in) {
                return obj::OBJ_NULL;
            }
            let mut lhs_bufinfo = BufferInfo::default();
            let mut rhs_bufinfo = BufferInfo::default();
            array_get_buffer(lhs_in, &mut lhs_bufinfo, obj::BUFFER_READ);
            obj::get_buffer_raise(rhs_in, &mut rhs_bufinfo, obj::BUFFER_READ);
            let sz = binary::get_size(b'@', lhs_bufinfo.typecode as u8, None);
            let rhs_len = rhs_bufinfo.len / sz;
            let res = array_new(lhs_bufinfo.typecode as u8, lhs.len + rhs_len);
            unsafe {
                seq_cat(
                    (*res).items,
                    lhs_bufinfo.buf,
                    lhs_bufinfo.len,
                    rhs_bufinfo.buf,
                    rhs_len * sz,
                );
            }
            obj::from_ptr(res as *const ObjArray as *const ())
        }
        BinaryOp::InplaceAdd => {
            if is_memoryview_obj(lhs_in) {
                return obj::OBJ_NULL;
            }
            array_extend(lhs_in, rhs_in);
            lhs_in
        }
        BinaryOp::Contains => {
            if mpconfig::PY_BUILTINS_BYTEARRAY {
                let mut lhs_bufinfo = BufferInfo::default();
                let mut rhs_bufinfo = BufferInfo::default();
                if obj::get_buffer(rhs_in, &mut rhs_bufinfo, obj::BUFFER_READ) {
                    if !obj::is_exact_type(lhs_in, type_bytearray()) {
                        return obj::CONST_FALSE;
                    }
                    array_get_buffer(lhs_in, &mut lhs_bufinfo, obj::BUFFER_READ);
                    let hay = lhs_bufinfo.as_bytes();
                    let needle = rhs_bufinfo.as_bytes();
                    return obj::new_bool(find_subbytes(hay, needle, 1).is_some());
                }
            }
            if obj::is_int(rhs_in) || objfloat::is_float(rhs_in) {
                raise::raise(MpRaise::RuntimeError("not implemented"));
            }
            obj::CONST_FALSE
        }
        BinaryOp::Equal
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::More
        | BinaryOp::MoreEqual => {
            let mut lhs_bufinfo = BufferInfo::default();
            let mut rhs_bufinfo = BufferInfo::default();
            array_get_buffer(lhs_in, &mut lhs_bufinfo, obj::BUFFER_READ);
            if !obj::get_buffer(rhs_in, &mut rhs_bufinfo, obj::BUFFER_READ) {
                return obj::CONST_FALSE;
            }
            let mut is_unsigned = false;
            let lhs_code = typecode_for_comparison(lhs_bufinfo.typecode as u8, &mut is_unsigned);
            let rhs_code = typecode_for_comparison(rhs_bufinfo.typecode as u8, &mut is_unsigned);
            if lhs_code == rhs_code
                && lhs_code != b'f'
                && lhs_code != b'd'
                && (op == BinaryOp::Equal || is_unsigned)
            {
                let d1 = lhs_bufinfo.as_bytes();
                let d2 = rhs_bufinfo.as_bytes();
                return obj::new_bool(sequence::cmp_bytes(op, d1, d2));
            }
            raise::raise(MpRaise::RuntimeError("not implemented"));
        }
        _ => obj::OBJ_NULL,
    }
}

/// `mp_obj_array_append`
pub fn array_append(self_in: Obj, arg: Obj) -> Obj {
    debug_assert!(
        (mpconfig::PY_BUILTINS_BYTEARRAY && obj::is_exact_type(self_in, type_bytearray()))
            || (mpconfig::PY_ARRAY && obj::is_exact_type(self_in, type_array()))
    );
    let self_ = unsafe { &mut *array_ptr(self_in) };
    if self_.free == 0 {
        let item_sz = binary::get_size(b'@', self_.typecode, None);
        let add_cnt = 8;
        self_.items = malloc::renew(
            self_.items,
            item_sz * self_.len,
            item_sz * (self_.len + add_cnt),
        )
        .expect("array grow");
        self_.free = add_cnt;
        seq_clear(self_.items, self_.len + 1, self_.len + self_.free, item_sz);
    }
    let cap = (self_.len + self_.free) * binary::get_size(b'@', self_.typecode, None);
    let items = unsafe { std::slice::from_raw_parts_mut(self_.items, cap) };
    binary::set_val_array(self_.typecode, items, self_.len, arg);
    self_.len += 1;
    self_.free -= 1;
    obj::CONST_NONE
}

/// `mp_obj_array_extend`
pub fn array_extend(self_in: Obj, arg_in: Obj) -> Obj {
    debug_assert!(
        (mpconfig::PY_BUILTINS_BYTEARRAY && obj::is_exact_type(self_in, type_bytearray()))
            || (mpconfig::PY_ARRAY && obj::is_exact_type(self_in, type_array()))
    );
    let self_ = unsafe { &mut *array_ptr(self_in) };
    let mut arg_bufinfo = BufferInfo::default();
    if !obj::get_buffer(arg_in, &mut arg_bufinfo, obj::BUFFER_READ) {
        array_extend_impl(self_, arg_in, 0, 0);
        return obj::CONST_NONE;
    }
    let sz = binary::get_size(b'@', self_.typecode, None);
    let len = arg_bufinfo.len / sz;
    if self_.free < len {
        self_.items = malloc::renew(
            self_.items,
            (self_.len + self_.free) * sz,
            (self_.len + len) * sz,
        )
        .expect("array extend grow");
        self_.free = 0;
        if self_in == arg_in {
            obj::get_buffer_raise(arg_in, &mut arg_bufinfo, obj::BUFFER_READ);
        }
    } else {
        self_.free -= len;
    }
    unsafe {
        seq_copy(self_.items.add(self_.len * sz), arg_bufinfo.buf, len * sz);
    }
    self_.len += len;
    obj::CONST_NONE
}

fn array_subscr(self_in: Obj, index_in: Obj, value: Obj) -> Obj {
    if value == obj::OBJ_NULL {
        return obj::OBJ_NULL;
    }
    let o = unsafe { &mut *array_ptr(self_in) };
    if mpconfig::PY_BUILTINS_SLICE && obj::is_exact_type(index_in, objslice::type_slice()) {
        let mut slice = objslice::BoundSlice {
            start: 0,
            stop: 0,
            step: 1,
        };
        if !sequence::get_fast_slice_indexes(o.len, index_in, &mut slice) {
            raise::raise(MpRaise::RuntimeError(
                "only slices with step=1 (aka None) are supported",
            ));
        }
        if value != OBJ_SENTINEL {
            if !mpconfig::PY_ARRAY_SLICE_ASSIGN {
                return obj::OBJ_NULL;
            }
            let item_sz = binary::get_size(b'@', o.typecode & TYPECODE_MASK, None);
            let (src_len, src_items, src_offs) = if is_array_like(value) {
                let src = unsafe { &*(array_ptr(value) as *const ObjArray) };
                if item_sz != binary::get_size(b'@', src.typecode & TYPECODE_MASK, None) {
                    raise::raise(MpRaise::ValueError("lhs and rhs should be compatible"));
                }
                let mut offs = 0usize;
                if is_memoryview_obj(value) {
                    offs = src.free * item_sz;
                }
                (src.len, src.items, offs)
            } else if obj::is_exact_type(value, objstr::type_bytes()) {
                if item_sz != 1 {
                    raise::raise(MpRaise::ValueError("lhs and rhs should be compatible"));
                }
                let mut bufinfo = BufferInfo::default();
                obj::get_buffer_raise(value, &mut bufinfo, obj::BUFFER_READ);
                (bufinfo.len, bufinfo.buf, 0)
            } else {
                raise::raise(MpRaise::RuntimeError("array/bytes required on right side"));
            };

            let len_adj = src_len as isize - (slice.stop - slice.start) as isize;
            let mut dest_items = o.items;
            if is_memoryview_obj(self_in) {
                if o.typecode & OBJ_ARRAY_TYPECODE_FLAG_RW == 0 {
                    return obj::OBJ_NULL;
                }
                if len_adj != 0 {
                    raise::raise(MpRaise::ValueError("lhs and rhs should be compatible"));
                }
                dest_items = unsafe { dest_items.add(o.free * item_sz) };
            }
            if len_adj > 0 {
                if len_adj as usize > o.free {
                    o.items = malloc::renew(
                        o.items,
                        (o.len + o.free) * item_sz,
                        (o.len + len_adj as usize) * item_sz,
                    )
                    .expect("array slice grow");
                    o.free = len_adj as usize;
                    if src_items == dest_items {
                        dest_items = o.items;
                    } else {
                        dest_items = o.items;
                    }
                }
                seq_replace_grow_inplace(
                    dest_items,
                    o.len,
                    slice.start as usize,
                    slice.stop as usize,
                    unsafe { src_items.add(src_offs) },
                    src_len,
                    len_adj,
                    item_sz,
                );
            } else {
                seq_replace_no_grow(
                    dest_items,
                    o.len,
                    slice.start as usize,
                    slice.stop as usize,
                    unsafe { src_items.add(src_offs) },
                    src_len,
                    item_sz,
                );
                seq_clear(
                    dest_items,
                    (o.len as isize + len_adj) as usize,
                    o.len,
                    item_sz,
                );
            }
            o.free -= len_adj as usize;
            o.len = (o.len as isize + len_adj) as usize;
            return obj::CONST_NONE;
        }

        let sz = binary::get_size(b'@', o.typecode & TYPECODE_MASK, None);
        debug_assert!(sz > 0);
        if is_memoryview_obj(self_in) {
            if slice.start as usize > MEMVIEW_OFFSET_MAX {
                raise::raise(MpRaise::OverflowError("memoryview offset too large"));
            }
            let res = malloc::new_obj::<ObjArray>().expect("memoryview slice");
            unsafe {
                (*res).base.type_ = o.base.type_;
                (*res).typecode = o.typecode;
                (*res).free = o.free + slice.start as usize;
                (*res).len = (slice.stop - slice.start) as usize;
                (*res).items = o.items;
            }
            return obj::from_ptr(res as *const ObjArray as *const ());
        }
        let res = array_new(o.typecode, (slice.stop - slice.start) as usize);
        unsafe {
            seq_copy(
                (*res).items,
                o.items.add(slice.start as usize * sz),
                (slice.stop - slice.start) as usize * sz,
            );
        }
        return obj::from_ptr(res as *const ObjArray as *const ());
    }

    let mut index = obj::get_index(unsafe { &*o.base.type_ }, o.len, index_in, false);
    if is_memoryview_obj(self_in) {
        index += o.free;
        if value != OBJ_SENTINEL && o.typecode & OBJ_ARRAY_TYPECODE_FLAG_RW == 0 {
            return obj::OBJ_NULL;
        }
    }
    let tc = o.typecode & TYPECODE_MASK;
    let cap = (o.len + o.free) * binary::get_size(b'@', tc, None);
    let items = unsafe { std::slice::from_raw_parts(o.items, cap) };
    if value == OBJ_SENTINEL {
        binary::get_val_array(tc, items, index)
    } else {
        binary::set_val_array(
            tc,
            unsafe { std::slice::from_raw_parts_mut(o.items, cap) },
            index,
            value,
        );
        obj::CONST_NONE
    }
}

pub fn array_get_buffer(o_in: Obj, bufinfo: &mut BufferInfo, flags: u32) -> obj::Int {
    let o = unsafe { &*(array_ptr(o_in) as *const ObjArray) };
    let sz = binary::get_size(b'@', o.typecode & TYPECODE_MASK, None);
    bufinfo.buf = o.items;
    bufinfo.len = o.len * sz;
    bufinfo.typecode = (o.typecode & TYPECODE_MASK) as i32;
    if is_memoryview_obj(o_in) {
        if o.typecode & OBJ_ARRAY_TYPECODE_FLAG_RW == 0 && flags & obj::BUFFER_WRITE != 0 {
            return 1;
        }
        bufinfo.buf = unsafe { bufinfo.buf.add(o.free * sz) };
    }
    0
}

fn array_iterator_new(array_in: Obj, iter_buf: *mut ObjIterBuf) -> Obj {
    debug_assert!(size_of::<ObjArrayIter>() <= size_of::<ObjIterBuf>());
    let o = unsafe { &mut *(iter_buf as *mut ObjArrayIter) };
    o.base.type_ = objpolyiter::type_polymorph_iter() as *const ObjType;
    o.iternext = array_it_iternext;
    o.array = array_in;
    o.cur = 0;
    obj::from_ptr(iter_buf as *const ObjArrayIter as *const ())
}

/// `mp_obj_new_bytearray`
pub fn new_bytearray(n: usize, items: &[u8]) -> Obj {
    let o = array_new(BYTEARRAY_TYPECODE, n);
    unsafe {
        std::ptr::copy_nonoverlapping(items.as_ptr(), (*o).items, n);
    }
    obj::from_ptr(o as *const ObjArray as *const ())
}

/// `mp_obj_new_bytearray_by_ref`
pub fn new_bytearray_by_ref(n: usize, items: *mut u8) -> Obj {
    let o = malloc::new_obj::<ObjArray>().expect("bytearray ref alloc");
    unsafe {
        (*o).base.type_ = type_bytearray() as *const ObjType;
        (*o).typecode = BYTEARRAY_TYPECODE;
        (*o).free = 0;
        (*o).len = n;
        (*o).items = items;
    }
    obj::from_ptr(o as *const ObjArray as *const ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;
    use crate::objint;
    use crate::objslice;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap();
        let _ = gc::init();
        qstr::init();
        guard
    }

    #[test]
    fn array_empty_new() {
        if !mpconfig::PY_ARRAY {
            return;
        }
        let _guard = setup();
        let a = array_make_new(type_array(), 1, 0, &[obj::new_qstr(qstr::from_str("b"))]);
        let o = unsafe { &*(array_ptr(a) as *const ObjArray) };
        assert_eq!(o.len, 0);
        assert_eq!(o.typecode, b'b');
    }

    #[test]
    fn array_from_bytes() {
        if !mpconfig::PY_ARRAY {
            return;
        }
        let _guard = setup();
        let args = [
            obj::new_qstr(qstr::from_str("b")),
            objstr::new_bytes(b"\x01\x02"),
        ];
        let a = array_make_new(type_array(), 2, 0, &args);
        let o = unsafe { &*(array_ptr(a) as *const ObjArray) };
        assert_eq!(o.len, 2);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, [1, 2]);
    }

    #[test]
    fn bytearray_empty() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = bytearray_make_new(type_bytearray(), 0, 0, &[]);
        let o = unsafe { &*(array_ptr(b) as *const ObjArray) };
        assert_eq!(o.len, 0);
        assert_eq!(o.typecode, BYTEARRAY_TYPECODE);
    }

    #[test]
    fn bytearray_from_int() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = bytearray_make_new(type_bytearray(), 1, 0, &[obj::new_small_int(4)]);
        let o = unsafe { &*(array_ptr(b) as *const ObjArray) };
        assert_eq!(o.len, 4);
    }

    #[test]
    fn bytearray_from_bytes() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let raw = objstr::new_bytes(b"abc");
        let b = bytearray_make_new(type_bytearray(), 1, 0, &[raw]);
        let o = unsafe { &*(array_ptr(b) as *const ObjArray) };
        assert_eq!(o.len, 3);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, b"abc");
    }

    #[test]
    fn append_and_index() {
        if !(mpconfig::PY_BUILTINS_BYTEARRAY || mpconfig::PY_ARRAY) {
            return;
        }
        let _guard = setup();
        let b = if mpconfig::PY_BUILTINS_BYTEARRAY {
            bytearray_make_new(type_bytearray(), 0, 0, &[])
        } else {
            array_make_new(type_array(), 1, 0, &[obj::new_qstr(qstr::from_str("B"))])
        };
        array_append(b, obj::new_small_int(10));
        array_append(b, obj::new_small_int(20));
        let v = array_subscr(b, obj::new_small_int(0), OBJ_SENTINEL);
        assert_eq!(obj::small_int_value(v), 10);
        let v = array_subscr(b, obj::new_small_int(1), OBJ_SENTINEL);
        assert_eq!(obj::small_int_value(v), 20);
    }

    #[test]
    fn extend_from_bytes() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = bytearray_make_new(type_bytearray(), 0, 0, &[]);
        array_extend(b, objstr::new_bytes(b"xy"));
        let o = unsafe { &*(array_ptr(b) as *const ObjArray) };
        assert_eq!(o.len, 2);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, b"xy");
    }

    #[test]
    fn binary_add() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let a = new_bytearray(2, b"ab");
        let c = array_binary_op(BinaryOp::Add, a, objstr::new_bytes(b"cd"));
        let o = unsafe { &*(array_ptr(c) as *const ObjArray) };
        assert_eq!(o.len, 4);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, b"abcd");
    }

    #[test]
    fn unary_len_bool() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(0, b"");
        assert!(!obj::is_true(array_unary_op(UnaryOp::Bool, b)));
        assert_eq!(obj::small_int_value(array_unary_op(UnaryOp::Len, b)), 0);
        let b2 = new_bytearray(3, b"abc");
        assert!(obj::is_true(array_unary_op(UnaryOp::Bool, b2)));
        assert_eq!(obj::small_int_value(array_unary_op(UnaryOp::Len, b2)), 3);
    }

    #[test]
    fn slice_get() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(4, b"abcd");
        let sl = objslice::new_slice(
            obj::new_small_int(1),
            obj::new_small_int(3),
            obj::CONST_NONE,
        );
        let sub = array_subscr(b, sl, OBJ_SENTINEL);
        let o = unsafe { &*(array_ptr(sub) as *const ObjArray) };
        assert_eq!(o.len, 2);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, b"bc");
    }

    #[test]
    fn buffer_read() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(2, b"\x01\x02");
        let mut buf = BufferInfo::default();
        assert_eq!(array_get_buffer(b, &mut buf, obj::BUFFER_READ), 0);
        assert_eq!(buf.len, 2);
        assert_eq!(buf.typecode, BYTEARRAY_TYPECODE as i32);
        assert_eq!(buf.as_bytes(), b"\x01\x02");
    }

    #[test]
    fn iterator_yields_elements() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(2, b"\x0a\x0b");
        let mut ibuf = ObjIterBuf {
            base: ObjBase {
                type_: core::ptr::null(),
            },
            buf: [obj::OBJ_NULL; 3],
        };
        let it = array_iterator_new(b, &mut ibuf);
        let v0 = array_it_iternext(it);
        assert_eq!(obj::small_int_value(v0), 10);
        let v1 = array_it_iternext(it);
        assert_eq!(obj::small_int_value(v1), 11);
        assert_eq!(array_it_iternext(it), obj::OBJ_STOP_ITERATION);
    }

    #[test]
    fn memoryview_from_bytearray() {
        if !(mpconfig::PY_BUILTINS_MEMORYVIEW && mpconfig::PY_BUILTINS_BYTEARRAY) {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(3, b"abc");
        let mv = memoryview_make_new(type_memoryview(), 1, 0, &[b]);
        let o = unsafe { &*(array_ptr(mv) as *const ObjArray) };
        assert_eq!(o.len, 3);
        let mut dest = [obj::OBJ_NULL; 2];
        memoryview_attr(mv, qstr::from_str("itemsize"), &mut dest);
        assert_eq!(obj::small_int_value(dest[0]), 1);
    }

    #[test]
    fn memoryview_slice() {
        if !(mpconfig::PY_BUILTINS_MEMORYVIEW && mpconfig::PY_BUILTINS_BYTEARRAY) {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(4, b"abcd");
        let mv = memoryview_make_new(type_memoryview(), 1, 0, &[b]);
        let sl = objslice::new_slice(
            obj::new_small_int(1),
            obj::new_small_int(3),
            obj::CONST_NONE,
        );
        let sub = array_subscr(mv, sl, OBJ_SENTINEL);
        let o = unsafe { &*(array_ptr(sub) as *const ObjArray) };
        assert_eq!(o.len, 2);
        assert_eq!(o.free, 1);
    }

    #[test]
    fn compare_equal_bytes() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(3, b"abc");
        let eq = array_binary_op(BinaryOp::Equal, b, objstr::new_bytes(b"abc"));
        assert!(obj::is_true(eq));
    }

    #[test]
    fn i32_array_roundtrip() {
        if !mpconfig::PY_ARRAY {
            return;
        }
        let _guard = setup();
        let a = array_make_new(type_array(), 1, 0, &[obj::new_qstr(qstr::from_str("i"))]);
        array_append(a, objint::new_int(0x01020304));
        let v = array_subscr(a, obj::new_small_int(0), OBJ_SENTINEL);
        assert_eq!(obj::get_int(v), 0x01020304);
    }

    #[test]
    fn new_bytearray_helper() {
        if !mpconfig::PY_BUILTINS_BYTEARRAY {
            return;
        }
        let _guard = setup();
        let b = new_bytearray(2, b"hi");
        let o = unsafe { &*(array_ptr(b) as *const ObjArray) };
        assert_eq!(o.len, 2);
        let data = unsafe { std::slice::from_raw_parts(o.items, o.len) };
        assert_eq!(data, b"hi");
    }

    #[test]
    fn new_memoryview_direct() {
        if !mpconfig::PY_BUILTINS_MEMORYVIEW {
            return;
        }
        let _guard = setup();
        let mut buf = [7u8, 8, 9];
        let mv = new_memoryview(b'B', 3, buf.as_mut_ptr());
        let o = unsafe { &*(array_ptr(mv) as *const ObjArray) };
        assert_eq!(o.len, 3);
        let v = array_subscr(mv, obj::new_small_int(1), OBJ_SENTINEL);
        assert_eq!(obj::small_int_value(v), 8);
    }
}
