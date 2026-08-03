//! rewrite of extmod/moductypes.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::binary::{self, BYTEARRAY_TYPECODE};
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, OBJ_NULL, OBJ_SENTINEL, TYPE_FLAG_BUILTIN_FUN,
};
use py_rs::objarray;
use py_rs::objdict::{self, ObjDict};
use py_rs::objfloat;
use py_rs::objint;
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr::{self, Qstr};
use py_rs::raise::{self, MpRaise};
use py_rs::runtime0::UnaryOp;
use py_rs::smallint;

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

const LAYOUT_LITTLE_ENDIAN: u32 = 0;
const LAYOUT_BIG_ENDIAN: u32 = 1;
const LAYOUT_NATIVE: u32 = 2;

const VAL_TYPE_BITS: u32 = 4;
const BITF_LEN_BITS: u32 = 5;
const BITF_OFF_BITS: u32 = 5;
const OFFSET_BITS: u32 = 17;
const LEN_BITS: u32 = OFFSET_BITS + BITF_OFF_BITS;
const AGG_TYPE_BITS: u32 = 2;

const UINT8: u32 = 0;
const INT8: u32 = 1;
const UINT16: u32 = 2;
const INT16: u32 = 3;
const UINT32: u32 = 4;
const INT32: u32 = 5;
const UINT64: u32 = 6;
const INT64: u32 = 7;
const BFUINT8: u32 = 8;
const BFINT8: u32 = 9;
const BFUINT16: u32 = 10;
const BFINT16: u32 = 11;
const BFUINT32: u32 = 12;
const BFINT32: u32 = 13;
const FLOAT32: u32 = 14;
const FLOAT64: u32 = 15;

const STRUCT: u32 = 0;
const PTR: u32 = 1;
const ARRAY: u32 = 2;

const TYPE2CHAR: [u8; 16] = [
    b'B', b'b', b'H', b'h', b'I', b'i', b'Q', b'q', b'-', b'-', b'-', b'-', b'-', b'-', b'f', b'd',
];

const fn type2smallint(x: i32, nbits: u32) -> isize {
    let v = (x << (32 - nbits)) as i32;
    (v >> 1) as isize
}

fn get_type(x: obj::Int, nbits: u32) -> u32 {
    (((x as i32) >> (31 - nbits as i32)) as u32) & ((1 << nbits) - 1)
}

fn value_mask(nbits: u32) -> obj::Int {
    (!((0x80000000u32 as i32) >> nbits as i32)) as obj::Int
}

fn get_scalar_size(val_type: u32) -> usize {
    1 << (val_type >> 1)
}

fn syntax_error() -> ! {
    raise::raise(MpRaise::TypeError("syntax error in uctypes descriptor"));
}

#[repr(C)]
struct ObjUctypesStruct {
    base: ObjBase,
    desc: Obj,
    addr: *mut u8,
    flags: u32,
}

fn struct_ptr(o: Obj) -> *mut ObjUctypesStruct {
    obj::as_ptr(o) as *mut ObjUctypesStruct
}

fn is_scalar_array(tuple_desc: Obj) -> bool {
    let (len, _) = objtuple::tuple_get(tuple_desc);
    len == 2
}

fn is_scalar_array_of_bytes(tuple_desc: Obj) -> bool {
    let (_, items) = objtuple::tuple_get(tuple_desc);
    obj::is_small_int(items[1]) && get_type(obj::small_int_value(items[1]), VAL_TYPE_BITS) == UINT8
}

fn scalar_size(val_type: u32) -> usize {
    if val_type == FLOAT32 {
        4
    } else {
        get_scalar_size(val_type & 7)
    }
}

fn struct_agg_size(t: Obj, layout_type: u32, max_field_size: &mut usize) -> usize {
    let (len, items) = objtuple::tuple_get(t);
    if len < 2 {
        syntax_error();
    }
    let offset_ = obj::small_int_value(items[0]);
    let agg_type = get_type(offset_, AGG_TYPE_BITS);
    match agg_type {
        STRUCT => {
            if len != 2 {
                syntax_error();
            }
            struct_size(items[1], layout_type, max_field_size)
        }
        PTR => {
            if len != 2 {
                syntax_error();
            }
            let ptr_sz = core::mem::size_of::<usize>();
            if ptr_sz > *max_field_size {
                *max_field_size = ptr_sz;
            }
            ptr_sz
        }
        ARRAY => {
            let mut arr_sz = obj::small_int_value(items[1]);
            let val_type = get_type(arr_sz, VAL_TYPE_BITS);
            arr_sz &= value_mask(VAL_TYPE_BITS);
            let item_s = if len == 2 {
                let s = scalar_size(val_type);
                if s > *max_field_size {
                    *max_field_size = s;
                }
                s
            } else if len == 3 {
                struct_size(items[2], layout_type, max_field_size)
            } else {
                syntax_error();
            };
            (arr_sz as usize) * item_s
        }
        _ => syntax_error(),
    }
}

fn struct_size(desc_in: Obj, layout_type: u32, max_field_size: &mut usize) -> usize {
    if !objdict::is_dict_or_ordereddict(desc_in) {
        if obj::is_exact_type(desc_in, obj::type_tuple()) {
            return struct_agg_size(desc_in, layout_type, max_field_size);
        }
        if obj::is_small_int(desc_in) {
            raise::raise(MpRaise::TypeError("can't unambiguously get sizeof scalar"));
        }
        syntax_error();
    }
    let dict = unsafe { &*(objdict::dict_ptr(desc_in) as *const ObjDict) };
    let mut total_size = 0usize;
    for i in 0..dict.map.alloc {
        if !map::slot_is_filled(&dict.map, i) {
            continue;
        }
        let v = dict.map.table[i].value;
        if obj::is_small_int(v) {
            let mut offset = obj::small_int_value(v);
            let val_type = get_type(offset, VAL_TYPE_BITS);
            offset &= value_mask(VAL_TYPE_BITS);
            if (BFUINT8..=BFINT32).contains(&val_type) {
                offset &= (1 << OFFSET_BITS) - 1;
            }
            let s = scalar_size(val_type);
            if s > *max_field_size {
                *max_field_size = s;
            }
            let end = offset as usize + s;
            if end > total_size {
                total_size = end;
            }
        } else if obj::is_exact_type(v, obj::type_tuple()) {
            let (_, items) = objtuple::tuple_get(v);
            let mut offset = obj::small_int_value(items[0]);
            offset &= value_mask(AGG_TYPE_BITS);
            let s = struct_agg_size(v, layout_type, max_field_size);
            let end = offset as usize + s;
            if end > total_size {
                total_size = end;
            }
        } else {
            syntax_error();
        }
    }
    if layout_type == LAYOUT_NATIVE && *max_field_size > 0 {
        total_size = (total_size + *max_field_size - 1) & !(*max_field_size - 1);
    }
    total_size
}

fn get_unaligned(val_type: u32, p: &[u8], big_endian: u32) -> Obj {
    let struct_type = if big_endian != 0 { b'>' } else { b'<' };
    let mut ptr = 0usize;
    binary::get_val(struct_type, TYPE2CHAR[val_type as usize], p, &mut ptr)
}

fn set_unaligned(val_type: u32, p: &mut [u8], big_endian: u32, val: Obj) {
    let struct_type = if big_endian != 0 { b'>' } else { b'<' };
    let mut ptr = 0usize;
    binary::set_val(struct_type, TYPE2CHAR[val_type as usize], val, p, &mut ptr);
}

fn get_aligned_basic(val_type: u32, p: *const u8) -> u32 {
    unsafe {
        match val_type {
            UINT8 => *p as u32,
            UINT16 => *(p as *const u16) as u32,
            UINT32 => *(p as *const u32),
            _ => {
                debug_assert!(false);
                0
            }
        }
    }
}

fn set_aligned_basic(val_type: u32, p: *mut u8, v: u32) {
    unsafe {
        match val_type {
            UINT8 => *p = v as u8,
            UINT16 => *(p as *mut u16) = v as u16,
            UINT32 => *(p as *mut u32) = v,
            _ => debug_assert!(false),
        }
    }
}

fn get_aligned(val_type: u32, p: *const u8, index: isize) -> Obj {
    unsafe {
        match val_type {
            UINT8 => obj::new_small_int(*p.offset(index) as obj::Int),
            INT8 => obj::new_small_int(*p.offset(index) as i8 as obj::Int),
            UINT16 => obj::new_small_int(*(p.offset(index * 2) as *const u16) as obj::Int),
            INT16 => obj::new_small_int(*(p.offset(index * 2) as *const i16) as obj::Int),
            UINT32 => objint::new_int_from_uint(*(p.offset(index * 4) as *const u32) as obj::Uint),
            INT32 => objint::new_int(*(p.offset(index * 4) as *const i32) as obj::Int),
            UINT64 => objint::new_int_from_ull(*(p.offset(index * 8) as *const u64)),
            INT64 => objint::new_int_from_ll(*(p.offset(index * 8) as *const i64)),
            FLOAT32 if mpconfig::PY_BUILTINS_FLOAT => {
                objfloat::new_float_from_f(*(p.offset(index * 4) as *const f32))
            }
            FLOAT64 if mpconfig::PY_BUILTINS_FLOAT => {
                objfloat::new_float_from_d(*(p.offset(index * 8) as *const f64))
            }
            _ => {
                debug_assert!(false);
                OBJ_NULL
            }
        }
    }
}

fn set_aligned(val_type: u32, p: *mut u8, index: isize, val: Obj) {
    if mpconfig::PY_BUILTINS_FLOAT && (val_type == FLOAT32 || val_type == FLOAT64) {
        unsafe {
            if val_type == FLOAT32 {
                *(p.offset(index * 4) as *mut f32) = objfloat::get_float_to_f(val);
            } else {
                *(p.offset(index * 8) as *mut f64) = objfloat::get_float_to_d(val);
            }
        }
        return;
    }
    let v = obj::get_int_truncated(val);
    unsafe {
        match val_type {
            UINT8 => *p.offset(index) = v as u8,
            INT8 => *p.offset(index) = v as i8 as u8,
            UINT16 => *(p.offset(index * 2) as *mut u16) = v as u16,
            INT16 => *(p.offset(index * 2) as *mut i16) = v as i16,
            UINT32 => *(p.offset(index * 4) as *mut u32) = v as u32,
            INT32 => *(p.offset(index * 4) as *mut i32) = v as i32,
            INT64 | UINT64 => {
                if core::mem::size_of::<obj::Int>() == 8 {
                    *(p.offset(index * 8) as *mut u64) = v as u64;
                } else {
                    let slice = std::slice::from_raw_parts_mut(p.offset(index * 8), 8);
                    set_unaligned(val_type, slice, LAYOUT_BIG_ENDIAN, val);
                }
            }
            _ => debug_assert!(false),
        }
    }
}

fn struct_attr_op(self_in: Obj, attr: Qstr, set_val: Obj) -> Obj {
    let self_ = unsafe { &mut *struct_ptr(self_in) };
    if !objdict::is_dict_or_ordereddict(self_.desc) {
        raise::raise(MpRaise::TypeError("struct: no fields"));
    }
    let deref = objdict::dict_get(self_.desc, obj::new_qstr(attr));
    if deref == OBJ_NULL {
        raise::raise(MpRaise::RuntimeError("KeyError"));
    }
    if obj::is_small_int(deref) {
        let mut offset = obj::small_int_value(deref);
        let val_type = get_type(offset, VAL_TYPE_BITS);
        offset &= value_mask(VAL_TYPE_BITS);
        if val_type <= INT64 || val_type == FLOAT32 || val_type == FLOAT64 {
            let addr = unsafe { self_.addr.add(offset as usize) };
            if self_.flags == LAYOUT_NATIVE {
                if set_val == OBJ_NULL {
                    return get_aligned(val_type, addr, 0);
                }
                set_aligned(val_type, addr, 0, set_val);
                return set_val;
            }
            let slice = unsafe { std::slice::from_raw_parts_mut(addr, scalar_size(val_type)) };
            if set_val == OBJ_NULL {
                return get_unaligned(val_type, slice, self_.flags);
            }
            set_unaligned(val_type, slice, self_.flags, set_val);
            return set_val;
        }
        if (BFUINT8..=BFINT32).contains(&val_type) {
            let bit_offset = ((offset as u32) >> OFFSET_BITS) & 31;
            let bit_len = ((offset as u32) >> LEN_BITS) & 31;
            offset &= (1 << OFFSET_BITS) - 1;
            let addr = unsafe { self_.addr.add(offset as usize) };
            let mut val = if self_.flags == LAYOUT_NATIVE {
                get_aligned_basic(val_type & 6, addr)
            } else {
                binary::get_int(
                    get_scalar_size(val_type & 7),
                    (val_type & 1) != 0,
                    self_.flags != 0,
                    unsafe { std::slice::from_raw_parts(addr, get_scalar_size(val_type & 7)) },
                ) as u32
            };
            if set_val == OBJ_NULL {
                val >>= bit_offset;
                val &= (1 << bit_len) - 1;
                debug_assert_eq!(val_type & 1, 0);
                return objint::new_int(val as obj::Int);
            }
            let mut set_val_int = obj::get_int(set_val) as u32;
            let mask = (1u32 << bit_len) - 1;
            set_val_int &= mask;
            set_val_int <<= bit_offset;
            let mask = mask << bit_offset;
            val = (val & !mask) | set_val_int;
            if self_.flags == LAYOUT_NATIVE {
                set_aligned_basic(val_type & 6, addr, val);
            } else {
                let item_size = get_scalar_size(val_type & 7);
                let slice = unsafe { std::slice::from_raw_parts_mut(addr, item_size) };
                binary::set_int(
                    item_size,
                    slice,
                    item_size,
                    val as obj::Uint,
                    self_.flags == LAYOUT_BIG_ENDIAN,
                );
            }
            return set_val;
        }
        debug_assert!(false);
        return OBJ_NULL;
    }
    if !obj::is_exact_type(deref, obj::type_tuple()) {
        syntax_error();
    }
    if set_val != OBJ_NULL {
        syntax_error();
    }
    let (_, sub_items) = objtuple::tuple_get(deref);
    let mut offset = obj::small_int_value(sub_items[0]);
    let agg_type = get_type(offset, AGG_TYPE_BITS);
    offset &= value_mask(AGG_TYPE_BITS);
    match agg_type {
        STRUCT => {
            let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
            unsafe {
                (*o).base.type_ = init_struct_type() as *const ObjType;
                (*o).desc = sub_items[1];
                (*o).addr = self_.addr.add(offset as usize);
                (*o).flags = self_.flags;
                obj::from_ptr(o as *const ObjUctypesStruct as *const ())
            }
        }
        ARRAY => {
            if is_scalar_array(deref) && is_scalar_array_of_bytes(deref) {
                let mut dummy = 0usize;
                let sz = struct_agg_size(deref, self_.flags, &mut dummy);
                return objarray::new_bytearray_by_ref(sz, unsafe {
                    self_.addr.add(offset as usize)
                });
            }
            let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
            unsafe {
                (*o).base.type_ = init_struct_type() as *const ObjType;
                (*o).desc = deref;
                (*o).addr = self_.addr.add(offset as usize);
                (*o).flags = self_.flags;
                obj::from_ptr(o as *const ObjUctypesStruct as *const ())
            }
        }
        PTR => {
            let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
            unsafe {
                (*o).base.type_ = init_struct_type() as *const ObjType;
                (*o).desc = deref;
                (*o).addr = self_.addr.add(offset as usize);
                (*o).flags = self_.flags;
                obj::from_ptr(o as *const ObjUctypesStruct as *const ())
            }
        }
        _ => OBJ_NULL,
    }
}

fn struct_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    debug_assert!(smallint::BITS >= 31);
    py_rs::argcheck::check_num(n_args, n_kw, 2, 3, false);
    let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).addr = obj::get_int_truncated(args[0]) as usize as *mut u8;
        (*o).desc = args[1];
        (*o).flags = LAYOUT_NATIVE;
        if n_args == 3 {
            (*o).flags = obj::get_int(args[2]) as u32;
        }
        obj::from_ptr(o as *const ObjUctypesStruct as *const ())
    }
}

fn struct_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*struct_ptr(self_in) };
    let typen = if objdict::is_dict_or_ordereddict(self_.desc) {
        "STRUCT"
    } else if obj::is_exact_type(self_.desc, obj::type_tuple()) {
        let (_, items) = objtuple::tuple_get(self_.desc);
        match get_type(obj::small_int_value(items[0]), AGG_TYPE_BITS) {
            PTR => "PTR",
            ARRAY => "ARRAY",
            _ => "unk",
        }
    } else {
        "ERROR"
    };
    let _ = mpprint::printf(
        print,
        "<struct %s %p>",
        [VaArg::Str(typen), VaArg::USize(self_.addr as usize)].into_iter(),
    );
}

fn struct_attr(self_in: Obj, attr: Qstr, dest: &mut [Obj; 2]) {
    if dest[0] == OBJ_NULL {
        dest[0] = struct_attr_op(self_in, attr, OBJ_NULL);
    } else if struct_attr_op(self_in, attr, dest[1]) != OBJ_NULL {
        dest[0] = OBJ_NULL;
    }
}

fn struct_subscr(self_in: Obj, index_in: Obj, value: Obj) -> Obj {
    let self_ = unsafe { &mut *struct_ptr(self_in) };
    if value == OBJ_NULL {
        return OBJ_NULL;
    }
    if !obj::is_exact_type(self_.desc, obj::type_tuple()) {
        raise::raise(MpRaise::TypeError("struct: can't index"));
    }
    let (_, items) = objtuple::tuple_get(self_.desc);
    let offset = obj::small_int_value(items[0]);
    let agg_type = get_type(offset, AGG_TYPE_BITS);
    let index = obj::small_int_value(index_in);
    match agg_type {
        ARRAY => {
            let mut arr_sz = obj::small_int_value(items[1]);
            let val_type = get_type(arr_sz, VAL_TYPE_BITS);
            arr_sz &= value_mask(VAL_TYPE_BITS);
            if index >= arr_sz {
                raise::raise(MpRaise::RuntimeError("struct: index out of range"));
            }
            let idx = index as isize;
            if items.len() == 2 {
                if self_.flags == LAYOUT_NATIVE {
                    if value == OBJ_SENTINEL {
                        return get_aligned(val_type, self_.addr, idx);
                    }
                    set_aligned(val_type, self_.addr, idx, value);
                    return value;
                }
                let elem_sz = scalar_size(val_type);
                let p = unsafe { self_.addr.add((idx as usize) * elem_sz) };
                let slice = unsafe { std::slice::from_raw_parts_mut(p, elem_sz) };
                if value == OBJ_SENTINEL {
                    return get_unaligned(val_type, slice, self_.flags);
                }
                set_unaligned(val_type, slice, self_.flags, value);
                return value;
            }
            if value == OBJ_SENTINEL {
                let mut dummy = 0usize;
                let size = struct_size(items[2], self_.flags, &mut dummy);
                let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
                unsafe {
                    (*o).base.type_ = init_struct_type() as *const ObjType;
                    (*o).desc = items[2];
                    (*o).addr = self_.addr.add(size * index as usize);
                    (*o).flags = self_.flags;
                    return obj::from_ptr(o as *const ObjUctypesStruct as *const ());
                }
            }
            OBJ_NULL
        }
        PTR => {
            let p = unsafe { *(self_.addr as *const *mut u8) };
            if obj::is_small_int(items[1]) {
                let val_type = get_type(obj::small_int_value(items[1]), VAL_TYPE_BITS);
                get_aligned(val_type, p, index)
            } else {
                let mut dummy = 0usize;
                let size = struct_size(items[1], self_.flags, &mut dummy);
                let o = malloc::new_obj::<ObjUctypesStruct>().expect("uctypes struct");
                unsafe {
                    (*o).base.type_ = init_struct_type() as *const ObjType;
                    (*o).desc = items[1];
                    (*o).addr = p.add(size * index as usize);
                    (*o).flags = self_.flags;
                    obj::from_ptr(o as *const ObjUctypesStruct as *const ())
                }
            }
        }
        _ => {
            debug_assert!(false);
            OBJ_NULL
        }
    }
}

fn struct_unary_op(op: UnaryOp, self_in: Obj) -> Obj {
    let self_ = unsafe { &*struct_ptr(self_in) };
    match op {
        UnaryOp::IntMaybe => {
            if obj::is_exact_type(self_.desc, obj::type_tuple()) {
                let (_, items) = objtuple::tuple_get(self_.desc);
                let offset = obj::small_int_value(items[0]);
                if get_type(offset, AGG_TYPE_BITS) == PTR {
                    let p = unsafe { *(self_.addr as *const *mut u8) };
                    return objint::new_int_from_uint(p as usize as obj::Uint);
                }
            }
            OBJ_NULL
        }
        _ => OBJ_NULL,
    }
}

fn struct_get_buffer(self_in: Obj, bufinfo: &mut BufferInfo, _flags: u32) -> obj::Int {
    let self_ = unsafe { &*struct_ptr(self_in) };
    let mut max_field_size = 0usize;
    let size = struct_size(self_.desc, self_.flags, &mut max_field_size);
    bufinfo.buf = self_.addr;
    bufinfo.len = size;
    bufinfo.typecode = BYTEARRAY_TYPECODE as i32;
    0
}

fn struct_sizeof(n_args: usize, args: &[Obj]) -> Obj {
    let obj_in = args[0];
    let mut max_field_size = 0usize;
    if obj::is_exact_type(obj_in, objarray::type_bytearray()) {
        return obj::len(obj_in);
    }
    let mut layout_type = LAYOUT_NATIVE;
    let mut desc_in = obj_in;
    if obj::is_exact_type(obj_in, init_struct_type()) {
        if n_args != 1 {
            raise::raise(MpRaise::TypeError(""));
        }
        let s = unsafe { &*struct_ptr(obj_in) };
        desc_in = s.desc;
        layout_type = s.flags;
    } else if n_args == 2 {
        layout_type = obj::get_int(args[1]) as u32;
    }
    let size = struct_size(desc_in, layout_type, &mut max_field_size);
    obj::new_small_int(size as obj::Int)
}

fn struct_addressof(buf: Obj) -> Obj {
    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(buf, &mut bufinfo, obj::BUFFER_READ);
    objint::new_int_from_uint(bufinfo.buf as usize as obj::Uint)
}

fn struct_bytearray_at(ptr: Obj, size: Obj) -> Obj {
    objarray::new_bytearray_by_ref(
        obj::get_int_truncated(size) as usize,
        obj::get_int_truncated(ptr) as usize as *mut u8,
    )
}

fn struct_bytes_at(ptr: Obj, size: Obj) -> Obj {
    let n = obj::get_int_truncated(size) as usize;
    let p = obj::get_int_truncated(ptr) as usize as *const u8;
    let data = unsafe { std::slice::from_raw_parts(p, n) };
    objstr::new_bytes(data)
}

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

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T1: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { F1.as_ptr() },
};
static T2: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { F2.as_ptr() },
};
static TV: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_BUILTIN_FUN,
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
    slots: unsafe { FV.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    py_rs::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("uctypes fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("uctypes fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("uctypes fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

static mut STRUCT_SLOTS: [*const (); 7] = [
    struct_make_new as *const (),
    struct_print as *const (),
    struct_attr as *const (),
    struct_subscr as *const (),
    struct_unary_op as *const (),
    struct_get_buffer as *const (),
    core::ptr::null(),
];
static mut TYPE_STRUCT: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 5,
    slot_index_binary_op: 0,
    slot_index_attr: 3,
    slot_index_subscr: 4,
    slot_index_iter: 0,
    slot_index_buffer: 6,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { STRUCT_SLOTS.as_ptr() },
};

static STRUCT_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_struct_type() -> &'static ObjType {
    STRUCT_INIT.get_or_init(|| unsafe {
        TYPE_STRUCT.name = qstr::from_str("struct");
    });
    unsafe { &TYPE_STRUCT }
}

fn int_const(v: isize) -> Obj {
    obj::new_small_int(v as obj::Int)
}

fn build_globals_table() -> Vec<MapElem> {
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("uctypes")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("struct")),
            value: obj::from_ptr(init_struct_type() as *const ObjType as *const ()),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("sizeof")),
            value: mkv(1, 2, struct_sizeof),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("addressof")),
            value: mk1(struct_addressof),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("bytes_at")),
            value: mk2(struct_bytes_at),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("bytearray_at")),
            value: mk2(struct_bytearray_at),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("NATIVE")),
            value: int_const(LAYOUT_NATIVE as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("LITTLE_ENDIAN")),
            value: int_const(LAYOUT_LITTLE_ENDIAN as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BIG_ENDIAN")),
            value: int_const(LAYOUT_BIG_ENDIAN as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("VOID")),
            value: int_const(type2smallint(UINT8 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("UINT8")),
            value: int_const(type2smallint(UINT8 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("INT8")),
            value: int_const(type2smallint(INT8 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("UINT16")),
            value: int_const(type2smallint(UINT16 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("INT16")),
            value: int_const(type2smallint(INT16 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("UINT32")),
            value: int_const(type2smallint(UINT32 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("INT32")),
            value: int_const(type2smallint(INT32 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("UINT64")),
            value: int_const(type2smallint(UINT64 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("INT64")),
            value: int_const(type2smallint(INT64 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFUINT8")),
            value: int_const(type2smallint(BFUINT8 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFINT8")),
            value: int_const(type2smallint(BFINT8 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFUINT16")),
            value: int_const(type2smallint(BFUINT16 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFINT16")),
            value: int_const(type2smallint(BFINT16 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFUINT32")),
            value: int_const(type2smallint(BFUINT32 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BFINT32")),
            value: int_const(type2smallint(BFINT32 as i32, VAL_TYPE_BITS)),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BF_POS")),
            value: int_const(OFFSET_BITS as isize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("BF_LEN")),
            value: int_const(LEN_BITS as isize),
        },
    ];
    if mpconfig::PY_BUILTINS_FLOAT {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("FLOAT32")),
            value: int_const(type2smallint(FLOAT32 as i32, VAL_TYPE_BITS)),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("FLOAT64")),
            value: int_const(type2smallint(FLOAT64 as i32, VAL_TYPE_BITS)),
        });
    }
    if mpconfig::PY_UCTYPES_NATIVE_C_TYPES != 0 {
        if core::mem::size_of::<i16>() == 2 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("SHORT")),
                value: int_const(type2smallint(INT16 as i32, VAL_TYPE_BITS)),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("USHORT")),
                value: int_const(type2smallint(UINT16 as i32, VAL_TYPE_BITS)),
            });
        }
        if core::mem::size_of::<i32>() == 4 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("INT")),
                value: int_const(type2smallint(INT32 as i32, VAL_TYPE_BITS)),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("UINT")),
                value: int_const(type2smallint(UINT32 as i32, VAL_TYPE_BITS)),
            });
        }
        if core::mem::size_of::<i32>() == 4 && core::mem::size_of::<isize>() == 4 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("LONG")),
                value: int_const(type2smallint(INT32 as i32, VAL_TYPE_BITS)),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("ULONG")),
                value: int_const(type2smallint(UINT32 as i32, VAL_TYPE_BITS)),
            });
        } else if core::mem::size_of::<isize>() == 8 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("LONG")),
                value: int_const(type2smallint(INT64 as i32, VAL_TYPE_BITS)),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("ULONG")),
                value: int_const(type2smallint(UINT64 as i32, VAL_TYPE_BITS)),
            });
        }
        if core::mem::size_of::<i64>() == 8 {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("LONGLONG")),
                value: int_const(type2smallint(INT64 as i32, VAL_TYPE_BITS)),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("ULONGLONG")),
                value: int_const(type2smallint(UINT64 as i32, VAL_TYPE_BITS)),
            });
        }
    }
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("PTR")),
        value: int_const(type2smallint(PTR as i32, AGG_TYPE_BITS)),
    });
    table.push(MapElem {
        key: obj::new_qstr(qstr::from_str("ARRAY")),
        value: int_const(type2smallint(ARRAY as i32, AGG_TYPE_BITS)),
    });
    table
}

/// Register built-in `uctypes` module (`MP_REGISTER_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_UCTYPES {
        return OBJ_NULL;
    }
    init_struct_type();
    let table = build_globals_table();
    let ctx = malloc::new_obj::<ModuleContext>().expect("uctypes module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("uctypes"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use py_rs::gc;
    use py_rs::mpstate;
    use py_rs::objmodule;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    static TEST_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap();
        TEST_INIT.get_or_init(|| {
            let _ = gc::init();
            qstr::init();
            mpstate::init();
            let _ = init_module();
        });
        guard
    }

    fn load_field(s: Obj, name: &str) -> Obj {
        let mut dest = [OBJ_NULL, OBJ_NULL];
        struct_attr(s, qstr::from_str(name), &mut dest);
        dest[0]
    }

    fn store_field(s: Obj, name: &str, val: Obj) {
        let mut dest = [OBJ_SENTINEL, val];
        struct_attr(s, qstr::from_str(name), &mut dest);
    }

    fn global(name: &str) -> Obj {
        let m = objmodule::module_get_builtin(qstr::from_str("uctypes"), false);
        let globals = objmodule::module_get_globals(m);
        objdict::dict_get(
            obj::from_ptr(globals as *const ObjDict as *const ()),
            obj::new_qstr(qstr::from_str(name)),
        )
    }

    fn field(desc: &mut Vec<(Qstr, Obj)>, name: &str, ty: Obj, off: isize) -> Obj {
        desc.push((
            qstr::from_str(name),
            obj::new_small_int(obj::small_int_value(ty) | off),
        ));
        let elems: Vec<MapElem> = desc
            .iter()
            .map(|(k, v)| MapElem {
                key: obj::new_qstr(*k),
                value: *v,
            })
            .collect();
        let d = objdict::new_dict(elems.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(d)).map, elems);
        }
        d
    }

    #[test]
    fn type_constants_match_c_encoding() {
        if !mpconfig::PY_UCTYPES {
            return;
        }
        let _guard = setup();
        assert_eq!(obj::small_int_value(global("UINT8")), 0);
        assert_eq!(
            obj::small_int_value(global("INT8")),
            type2smallint(INT8 as i32, VAL_TYPE_BITS)
        );
        assert_eq!(
            obj::small_int_value(global("PTR")),
            type2smallint(PTR as i32, AGG_TYPE_BITS)
        );
        assert_eq!(
            obj::small_int_value(global("ARRAY")),
            type2smallint(ARRAY as i32, AGG_TYPE_BITS)
        );
        assert_eq!(obj::small_int_value(global("BF_POS")), OFFSET_BITS as isize);
    }

    #[test]
    fn sizeof_simple_layout() {
        if !mpconfig::PY_UCTYPES {
            return;
        }
        let _guard = setup();
        let u8 = global("UINT8");
        let mut desc = Vec::new();
        let d = field(&mut desc, "a", u8, 0);
        let sz = struct_sizeof(1, &[d]);
        assert_eq!(obj::small_int_value(sz), 1);
        desc.clear();
        let d2 = field(&mut desc, "w", global("UINT16"), 0);
        let sz2 = struct_sizeof(1, &[d2]);
        assert_eq!(obj::small_int_value(sz2), 2);
    }

    #[test]
    fn struct_scalar_native_le() {
        if !mpconfig::PY_UCTYPES {
            return;
        }
        let _guard = setup();
        let mut data = vec![0x12u8, 0x34, 0, 0, 0x78, 0x56, 0, 0];
        let u16 = global("UINT16");
        let u32 = global("UINT32");
        let le = global("LITTLE_ENDIAN");
        let mut desc = Vec::new();
        let d = field(&mut desc, "s0", u16, 0);
        desc.push((
            qstr::from_str("f32"),
            obj::new_small_int(obj::small_int_value(u32) | 4),
        ));
        let elems: Vec<MapElem> = desc
            .iter()
            .map(|(k, v)| MapElem {
                key: obj::new_qstr(*k),
                value: *v,
            })
            .collect();
        let dict = objdict::new_dict(elems.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, elems);
        }
        let addr = data.as_mut_ptr() as usize as obj::Int;
        let s = struct_make_new(init_struct_type(), 3, 0, &[objint::new_int(addr), dict, le]);
        let s0 = load_field(s, "s0");
        assert_eq!(obj::small_int_value(s0), 0x3412);
        let f32 = load_field(s, "f32");
        assert_eq!(obj::get_int(f32), 0x5678);
        store_field(s, "s0", obj::new_small_int(0xABCD));
        assert_eq!(data[0], 0xCD);
        assert_eq!(data[1], 0xAB);
    }

    #[test]
    fn array_uint8_bytearray_view() {
        if !mpconfig::PY_UCTYPES {
            return;
        }
        let _guard = setup();
        let mut data = vec![1u8, 2, 3, 4];
        let array = global("ARRAY");
        let u8 = global("UINT8");
        let le = global("LITTLE_ENDIAN");
        let fields = vec![(
            qstr::from_str("arr"),
            objtuple::new_tuple(
                2,
                Some(&[
                    obj::new_small_int(obj::small_int_value(array) | 0),
                    obj::new_small_int(obj::small_int_value(u8) | 4),
                ]),
            ),
        )];
        let elems: Vec<MapElem> = fields
            .iter()
            .map(|(k, v)| MapElem {
                key: obj::new_qstr(*k),
                value: *v,
            })
            .collect();
        let dict = objdict::new_dict(elems.len());
        unsafe {
            map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, elems);
        }
        let addr = data.as_mut_ptr() as usize as obj::Int;
        let s = struct_make_new(init_struct_type(), 3, 0, &[objint::new_int(addr), dict, le]);
        let arr = load_field(s, "arr");
        let mut info = BufferInfo::default();
        assert!(obj::get_buffer(arr, &mut info, obj::BUFFER_READ));
        let view = unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len) };
        assert_eq!(view, &[1, 2, 3, 4]);
    }

    #[test]
    fn addressof_and_bytes_at() {
        if !mpconfig::PY_UCTYPES {
            return;
        }
        let _guard = setup();
        let buf = objarray::new_bytearray(4, &[0xDE, 0xAD, 0xBE, 0xEF]);
        let addr = struct_addressof(buf);
        let b = struct_bytes_at(addr, obj::new_small_int(4));
        let (data, len) = objstr::str_get_data(b);
        assert_eq!(&data[..len], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
