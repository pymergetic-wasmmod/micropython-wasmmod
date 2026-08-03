//! rewrite of py/modstruct.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::binary;
use crate::malloc;
use crate::map::{self, MapElem};
use crate::mpconfig;
use crate::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use crate::objdict;
use crate::objmodule;
use crate::objstr;
use crate::objtuple;
use crate::parsenum;
use crate::qstr;
use crate::raise::{self, MpRaise};
use crate::vstr::{self, Vstr};

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
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
    crate::argcheck::check_num(n, k, 1, 1, false);
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltin1) };
    (self_.fun)(a[0])
}
fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    crate::argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("struct fun1");
    unsafe {
        (*o).base.type_ = &T1 as *const ObjType;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("struct funv");
    unsafe {
        (*o).base.type_ = &TV as *const ObjType;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

fn get_fmt_type(fmt: &mut &[u8]) -> u8 {
    if fmt.is_empty() {
        return b'@';
    }
    let t = fmt[0];
    match t {
        b'!' => {
            *fmt = &fmt[1..];
            b'>'
        }
        b'@' | b'=' | b'<' | b'>' => {
            *fmt = &fmt[1..];
            t
        }
        _ => b'@',
    }
}

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

fn get_fmt_num(fmt: &mut &[u8]) -> usize {
    let start = *fmt;
    let mut len = 0usize;
    while len < start.len() && is_digit(start[len]) {
        len += 1;
    }
    let val = parsenum::parse_num_integer(&start[..len], 10, None);
    *fmt = &start[len..];
    obj::small_int_value(val) as usize
}

fn calc_size_items(fmt: &[u8]) -> (usize, usize) {
    let mut fmt = fmt;
    let fmt_type = get_fmt_type(&mut fmt);
    let mut total_cnt = 0usize;
    let mut size = 0usize;
    while !fmt.is_empty() {
        let mut cnt = 1usize;
        if is_digit(fmt[0]) {
            cnt = get_fmt_num(&mut fmt);
        }
        let ch = fmt[0];
        fmt = &fmt[1..];
        if ch == b'x' {
            size += cnt;
        } else if ch == b's' {
            total_cnt += 1;
            size += cnt;
        } else {
            total_cnt += cnt;
            let mut align = 0usize;
            let sz = binary::get_size(fmt_type, ch, Some(&mut align));
            for _ in 0..cnt {
                size = (size + align - 1) & !(align - 1);
                size += sz;
            }
        }
    }
    (total_cnt, size)
}

fn struct_calcsize(fmt_in: Obj) -> Obj {
    let fmt = objstr::str_get_str(fmt_in);
    let (_, size) = calc_size_items(fmt.as_bytes());
    obj::new_small_int(size as isize)
}

fn struct_unpack_from(n_args: usize, args: &[Obj]) -> Obj {
    let fmt = objstr::str_get_str(args[0]);
    let (num_items, total_sz) = calc_size_items(fmt.as_bytes());
    let mut fmt = fmt.as_bytes();
    let fmt_type = get_fmt_type(&mut fmt);
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_READ);
    let mut offset = 0isize;
    if n_args > 2 {
        offset = obj::get_int(args[2]) as isize;
        if offset < 0 {
            offset = bufinfo.len as isize + offset;
            if offset < 0 {
                raise::raise(MpRaise::ValueError("buffer too small"));
            }
        }
    }
    if (offset as usize) + total_sz > bufinfo.len {
        raise::raise(MpRaise::ValueError("buffer too small"));
    }
    let mut items = Vec::with_capacity(num_items);
    let base_slice = unsafe {
        std::slice::from_raw_parts(
            bufinfo.buf.add(offset as usize),
            bufinfo.len - offset as usize,
        )
    };
    let mut pos = 0usize;
    let mut i = 0usize;
    while i < num_items {
        let mut cnt = 1usize;
        if !fmt.is_empty() && is_digit(fmt[0]) {
            cnt = get_fmt_num(&mut fmt);
        }
        let ch = fmt[0];
        fmt = &fmt[1..];
        if ch == b'x' {
            pos += cnt;
        } else if ch == b's' {
            items.push(objstr::new_bytes(&base_slice[pos..pos + cnt]));
            pos += cnt;
            i += 1;
        } else {
            for _ in 0..cnt {
                if i >= num_items {
                    break;
                }
                items.push(binary::get_val(fmt_type, ch, base_slice, &mut pos));
                i += 1;
            }
        }
    }
    objtuple::new_tuple(items.len(), Some(&items))
}

fn struct_pack(n_args: usize, args: &[Obj]) -> Obj {
    let size = obj::small_int_value(struct_calcsize(args[0]));
    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    vstr::init_len(&mut v, size as usize);
    let p = vstr::str_ptr(&mut v);
    unsafe {
        std::ptr::write_bytes(p, 0, size as usize);
    }
    struct_pack_into_internal(args[0], p, n_args - 1, &args[1..]);
    objstr::new_bytes(unsafe { std::slice::from_raw_parts(vstr::str_ptr(&v), v.len) })
}

fn struct_pack_into_internal(fmt_in: Obj, p: *mut u8, n_args: usize, args: &[Obj]) {
    let fmt = objstr::str_get_str(fmt_in);
    let mut fmt = fmt.as_bytes();
    let fmt_type = get_fmt_type(&mut fmt);
    let len = {
        let (_, sz) = calc_size_items(objstr::str_get_str(fmt_in).as_bytes());
        sz
    };
    let mut base_slice = unsafe { std::slice::from_raw_parts_mut(p, len) };
    let mut pos = 0usize;
    let mut i = 0usize;
    while i < n_args {
        if fmt.is_empty() {
            break;
        }
        let mut cnt = 1usize;
        if is_digit(fmt[0]) {
            cnt = get_fmt_num(&mut fmt);
        }
        let ch = fmt[0];
        fmt = &fmt[1..];
        if ch == b'x' {
            for b in &mut base_slice[pos..pos + cnt] {
                *b = 0;
            }
            pos += cnt;
        } else if ch == b's' {
            let mut bufinfo = obj::BufferInfo::default();
            obj::get_buffer_raise(args[i], &mut bufinfo, obj::BUFFER_READ);
            let to_copy = cnt.min(bufinfo.len);
            base_slice[pos..pos + to_copy].copy_from_slice(&bufinfo.as_bytes()[..to_copy]);
            for b in &mut base_slice[pos + to_copy..pos + cnt] {
                *b = 0;
            }
            pos += cnt;
            i += 1;
        } else {
            for _ in 0..cnt {
                if i >= n_args {
                    break;
                }
                binary::set_val(fmt_type, ch, args[i], &mut base_slice, &mut pos);
                i += 1;
            }
        }
    }
}

fn struct_pack_into(n_args: usize, args: &[Obj]) -> Obj {
    let mut bufinfo = obj::BufferInfo::default();
    obj::get_buffer_raise(args[1], &mut bufinfo, obj::BUFFER_WRITE);
    let mut offset = obj::get_int(args[2]) as isize;
    if offset < 0 {
        offset = bufinfo.len as isize + offset;
        if offset < 0 {
            raise::raise(MpRaise::ValueError("buffer too small"));
        }
    }
    let sz = obj::small_int_value(struct_calcsize(args[0]));
    if offset as usize + sz as usize > bufinfo.len {
        raise::raise(MpRaise::ValueError("buffer too small"));
    }
    struct_pack_into_internal(
        args[0],
        unsafe { bufinfo.buf.add(offset as usize) },
        n_args - 3,
        &args[3..],
    );
    obj::CONST_NONE
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_STRUCT {
        return obj::OBJ_NULL;
    }
    let table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("struct")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("calcsize")),
            value: mk1(struct_calcsize),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("pack")),
            value: mkv(1, 255, struct_pack),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("pack_into")),
            value: mkv(3, 255, struct_pack_into),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("unpack")),
            value: mkv(2, 3, struct_unpack_from),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("unpack_from")),
            value: mkv(2, 3, struct_unpack_from),
        },
    ];
    let ctx = malloc::new_obj::<ModuleContext>().expect("struct module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("struct"), module);
    module
}
