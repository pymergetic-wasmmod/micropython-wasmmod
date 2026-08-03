//! rewrite of py/persistentcode.c + py/persistentcode.h
// symmetry: done
use core::mem::size_of;

use crate::bc::{self, ModuleContext};
use crate::bc0;
use crate::emitglue::{self, CompiledModule, ProtoFun, RawCode, RawCodeKind};
use crate::gc;
use crate::malloc;
use crate::mpconfig;
use crate::nativeglue;
use crate::mpprint::{self, Print};
use crate::obj::{self, Obj};
use crate::objstr;
use crate::objtuple;
use crate::parsenum;
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::reader::{self, Reader};
use crate::smallint;
use crate::vstr::{self, Vstr};

pub const MPY_VERSION: u8 = 6;
pub const MPY_SUB_VERSION: u8 = 3;

const QSTR_LAST_STATIC: Qstr = qstr::QSTR_LAST_STATIC;

const MPY_FEATURE_ENCODE_SUB_VERSION: fn(u8) -> u8 = |version| version;
const MPY_FEATURE_DECODE_SUB_VERSION: fn(u8) -> u8 = |feat| feat & 3;
const MPY_FEATURE_ENCODE_ARCH: fn(u8) -> u8 = |arch| arch << 2;
const MPY_FEATURE_DECODE_ARCH: fn(u8) -> u8 = |feat| (feat >> 2) & 0x2f;

pub const MPY_FEATURE_ARCH_FLAGS: u8 = 0x40;
const MPY_FEATURE_ARCH_FLAGS_TEST: fn(u8) -> bool = |feat| feat & MPY_FEATURE_ARCH_FLAGS != 0;

pub const MP_NATIVE_ARCH_NONE: u8 = 0;
pub const MP_NATIVE_ARCH_X86: u8 = 1;
pub const MP_NATIVE_ARCH_X64: u8 = 2;

const MP_PERSISTENT_OBJ_FUN_TABLE: u8 = 0;
const MP_PERSISTENT_OBJ_NONE: u8 = 1;
const MP_PERSISTENT_OBJ_FALSE: u8 = 2;
const MP_PERSISTENT_OBJ_TRUE: u8 = 3;
const MP_PERSISTENT_OBJ_ELLIPSIS: u8 = 4;
const MP_PERSISTENT_OBJ_STR: u8 = 5;
const MP_PERSISTENT_OBJ_BYTES: u8 = 6;
const MP_PERSISTENT_OBJ_INT: u8 = 7;
const MP_PERSISTENT_OBJ_FLOAT: u8 = 8;
const MP_PERSISTENT_OBJ_COMPLEX: u8 = 9;
const MP_PERSISTENT_OBJ_TUPLE: u8 = 10;

// Viper load-time scope flags (`MP_SCOPE_FLAG_VIPER*` in runtime0.h).
const SCOPE_FLAG_VIPERRELOC: u16 = 0x10;
const SCOPE_FLAG_VIPERRODATA: u16 = 0x20;
const SCOPE_FLAG_VIPERBSS: u16 = 0x40;

fn mp_align(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

struct RelocInfo<'a> {
    reader: &'a mut Reader,
    context: *mut ModuleContext,
    rodata: *mut u8,
    bss: *mut u8,
}

fn reloc_read_byte(reader: &mut Reader) -> Option<u8> {
    let b = (reader.readbyte)(reader.data);
    if b == reader::READER_EOF {
        None
    } else {
        Some(b as u8)
    }
}

#[allow(dead_code)]
fn track_root_pointer(_ptr: *mut u8) {
    if mpconfig::PERSISTENT_CODE_TRACK_BSS_RODATA || mpconfig::PERSISTENT_CODE_TRACK_FUN_DATA {
        // Root-pointer tracking is not wired to the host GC yet.
    }
}

const MPY_FEATURE_ARCH: u8 = if cfg!(target_arch = "x86_64") && mpconfig::PERSISTENT_CODE_LOAD_NATIVE {
    MP_NATIVE_ARCH_X64
} else if cfg!(target_arch = "x86") && mpconfig::PERSISTENT_CODE_LOAD_NATIVE {
    MP_NATIVE_ARCH_X86
} else {
    MP_NATIVE_ARCH_NONE
};

fn mpy_feature_arch_test(arch: u8) -> bool {
    arch == MPY_FEATURE_ARCH
}

fn read_byte(reader: &mut Reader) -> u8 {
    let b = (reader.readbyte)(reader.data);
    if b == reader::READER_EOF {
        raise::raise(MpRaise::ValueError("incompatible .mpy file"));
    }
    b as u8
}

fn read_bytes(reader: &mut Reader, buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = read_byte(reader);
    }
}

fn read_uint(reader: &mut Reader) -> usize {
    let mut unum = 0usize;
    loop {
        let b = read_byte(reader);
        unum = (unum << 7) | (b as usize & 0x7f);
        if b & 0x80 == 0 {
            break;
        }
    }
    unum
}

fn load_qstr(reader: &mut Reader) -> Qstr {
    let len = read_uint(reader);
    if len & 1 != 0 {
        return len >> 1;
    }
    let len = len >> 1;

    if mpconfig::VFS_ROM {
        if let Some(memmap) = reader::reader_try_read_rom(reader, len + 1) {
            let data = unsafe { core::slice::from_raw_parts(memmap, len) };
            let q = qstr::find_strn(data);
            if q != qstr::QSTR_NULL {
                return q;
            }
            return qstr::from_strn(data);
        }
    }

    let mut str = vec![0u8; len];
    read_bytes(reader, &mut str);
    let _ = read_byte(reader);
    qstr::from_strn(&str)
}

fn load_obj(reader: &mut Reader) -> Obj {
    let obj_type = read_byte(reader);
    if mpconfig::ENABLE_NATIVE_CODE && obj_type == MP_PERSISTENT_OBJ_FUN_TABLE {
        return Obj(nativeglue::fun_table_reloc_base());
    }
    match obj_type {
        MP_PERSISTENT_OBJ_NONE => obj::CONST_NONE,
        MP_PERSISTENT_OBJ_FALSE => obj::CONST_FALSE,
        MP_PERSISTENT_OBJ_TRUE => obj::CONST_TRUE,
        MP_PERSISTENT_OBJ_ELLIPSIS => crate::objsingleton::const_ellipsis(),
        _ => {
            let len = read_uint(reader);
            if len == 0 && obj_type == MP_PERSISTENT_OBJ_BYTES {
                let _ = read_byte(reader);
                return objstr::new_bytes(&[]);
            }
            if obj_type == MP_PERSISTENT_OBJ_TUPLE {
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    items.push(load_obj(reader));
                }
                return objtuple::new_tuple(len, Some(&items));
            }

            let memmap = if mpconfig::VFS_ROM {
                reader::reader_try_read_rom(reader, len)
            } else {
                None
            };

            if let Some(memmap) = memmap {
                if obj_type == MP_PERSISTENT_OBJ_STR || obj_type == MP_PERSISTENT_OBJ_BYTES {
                    let _ = read_byte(reader);
                    if obj_type == MP_PERSISTENT_OBJ_STR {
                        return objstr::new_str_from_rom(memmap, len);
                    }
                    return objstr::new_bytes_from_rom(memmap, len);
                }
            }

            let mut data = vec![0u8; len];
            read_bytes(reader, &mut data);
            match obj_type {
                MP_PERSISTENT_OBJ_STR | MP_PERSISTENT_OBJ_BYTES => {
                    let _ = read_byte(reader);
                    if obj_type == MP_PERSISTENT_OBJ_STR {
                        let mut v = Vstr {
                            alloc: data.len() + 1,
                            len: data.len(),
                            buf: data.as_mut_ptr(),
                            fixed_buf: true,
                        };
                        objstr::new_str_from_vstr(&mut v)
                    } else {
                        objstr::new_bytes(&data)
                    }
                }
                MP_PERSISTENT_OBJ_INT => parsenum::parse_num_integer(&data, 10, None),
                MP_PERSISTENT_OBJ_FLOAT => parsenum::parse_num_float(&data, false, None),
                MP_PERSISTENT_OBJ_COMPLEX => parsenum::parse_num_float(&data, true, None),
                _ => raise::raise(MpRaise::ValueError("incompatible .mpy file")),
            }
        }
    }
}

fn load_raw_code(reader: &mut Reader, context: *mut ModuleContext) -> *mut RawCode {
    let kind_len = read_uint(reader);
    let kind = (kind_len & 3) + RawCodeKind::Bytecode as usize;
    let has_children = kind_len & 4 != 0;
    let fun_data_len = kind_len >> 3;

    if !(mpconfig::EMIT_INLINE_ASM || mpconfig::ENABLE_NATIVE_CODE)
        && kind != RawCodeKind::Bytecode as usize
    {
        raise::raise(MpRaise::ValueError("incompatible .mpy file"));
    }

    let mut prelude_offset = 0u16;
    let mut scope_flags = 0u16;
    let mut asm_n_pos_args = 0u32;
    let mut asm_type_sig = 0u32;

    let fun_data = if kind == RawCodeKind::Bytecode as usize {
        let fun_data = malloc::new::<u8>(fun_data_len).expect("bytecode alloc");
        unsafe {
            read_bytes(reader, std::slice::from_raw_parts_mut(fun_data, fun_data_len));
        }
        fun_data
    } else if mpconfig::ENABLE_NATIVE_CODE || mpconfig::EMIT_INLINE_ASM {
        let fun_data = malloc::new::<u8>(fun_data_len).expect("native code alloc");
        unsafe {
            read_bytes(reader, std::slice::from_raw_parts_mut(fun_data, fun_data_len));
        }
        if kind == RawCodeKind::NativePy as usize {
            let po = read_uint(reader);
            prelude_offset = po.min(u16::MAX as usize) as u16;
            let mut ip = unsafe { fun_data.add(po) as *const u8 };
            let sig = bc::prelude_sig_decode_into(&mut ip);
            scope_flags = sig.scope_flags as u16;
        } else if kind == RawCodeKind::NativeViper as usize || kind == RawCodeKind::NativeAsm as usize {
            scope_flags = read_uint(reader) as u16;
            if kind == RawCodeKind::NativeAsm as usize && mpconfig::EMIT_INLINE_ASM {
                asm_n_pos_args = read_uint(reader) as u32;
                asm_type_sig = read_uint(reader) as u32;
            }
        }
        fun_data
    } else {
        raise::raise(MpRaise::ValueError("incompatible .mpy file"));
    };

    let mut rodata: *mut u8 = core::ptr::null_mut();
    let mut bss: *mut u8 = core::ptr::null_mut();
    if (mpconfig::ENABLE_NATIVE_CODE || mpconfig::EMIT_INLINE_ASM)
        && kind == RawCodeKind::NativeViper as usize
    {
        let mut rodata_size = 0usize;
        if scope_flags & SCOPE_FLAG_VIPERRODATA != 0 {
            rodata_size = read_uint(reader);
        }
        let mut bss_size = 0usize;
        if scope_flags & SCOPE_FLAG_VIPERBSS != 0 {
            bss_size = read_uint(reader);
        }
        if rodata_size + bss_size != 0 {
            bss_size = mp_align(bss_size, size_of::<usize>());
            let data = malloc::new::<u8>(bss_size + rodata_size).expect("bss/rodata alloc");
            unsafe {
                core::ptr::write_bytes(data, 0, bss_size + rodata_size);
                bss = data;
                rodata = data.add(bss_size);
                if scope_flags & SCOPE_FLAG_VIPERRODATA != 0 {
                    read_bytes(
                        reader,
                        std::slice::from_raw_parts_mut(rodata, rodata_size),
                    );
                }
            }
            if mpconfig::PERSISTENT_CODE_TRACK_BSS_RODATA {
                track_root_pointer(data);
            }
        }
    }

    let native_py_extra = if (mpconfig::ENABLE_NATIVE_CODE || mpconfig::EMIT_INLINE_ASM)
        && kind == RawCodeKind::NativePy as usize
    {
        1
    } else {
        0
    };

    let mut n_children = 0usize;
    let mut children: *mut *mut RawCode = core::ptr::null_mut();
    if has_children {
        n_children = read_uint(reader);
        let child_ptr =
            malloc::new::<*mut RawCode>(n_children + native_py_extra).expect("children alloc");
        children = child_ptr;
        unsafe {
            for i in 0..n_children {
                *child_ptr.add(i) = load_raw_code(reader, context);
            }
        }
    }

    let rc = emitglue::new_raw_code();
    if kind == RawCodeKind::Bytecode as usize {
        let mut ip = fun_data as *const u8;
        let sig = bc::prelude_sig_decode_into(&mut ip);
        emitglue::assign_bytecode_ex(
            rc,
            fun_data as *const u8,
            children,
            sig.scope_flags as u16,
            fun_data_len as u32,
            n_children as u16,
        );
    } else if mpconfig::ENABLE_NATIVE_CODE || mpconfig::EMIT_INLINE_ASM {
        if mpconfig::PERSISTENT_CODE_TRACK_FUN_DATA && scope_flags & SCOPE_FLAG_VIPERRELOC != 0 {
            track_root_pointer(fun_data);
        }
        if scope_flags & SCOPE_FLAG_VIPERRELOC != 0 {
            let mut ri = RelocInfo {
                reader,
                context,
                rodata,
                bss,
            };
            native_relocate(&mut ri, fun_data, fun_data as usize);
        }
        if kind == RawCodeKind::NativePy as usize {
            let prelude_ptr = unsafe { fun_data.add(prelude_offset as usize) };
            if n_children == 0 {
                children = prelude_ptr as *mut *mut RawCode;
            } else {
                unsafe {
                    *children.add(n_children) = prelude_ptr as *mut RawCode;
                }
            }
        }
        emitglue::assign_native(
            rc,
            unsafe { core::mem::transmute(kind as u8) },
            fun_data,
            fun_data_len as u32,
            children,
            n_children as u16,
            prelude_offset,
            scope_flags,
            asm_n_pos_args,
            asm_type_sig,
        );
    }
    rc
}

pub fn raw_code_load(reader: &mut Reader, cm: &mut CompiledModule) {
    if !mpconfig::PERSISTENT_CODE_LOAD {
        raise::raise(MpRaise::RuntimeError("persistent code load disabled"));
    }

    crate::nlr::push_jump_callback({
        let close = reader.close;
        let data = reader.data;
        move || close(data)
    });

    let mut header = [0u8; 4];
    read_bytes(reader, &mut header);
    let arch = MPY_FEATURE_DECODE_ARCH(header[2]);
    if header[0] != b'M'
        || header[1] != MPY_VERSION
        || (arch != MP_NATIVE_ARCH_NONE
            && MPY_FEATURE_DECODE_SUB_VERSION(header[2]) != MPY_SUB_VERSION)
        || header[3] > smallint::BITS as u8
    {
        raise::raise(MpRaise::ValueError("incompatible .mpy file"));
    }
    if arch != MP_NATIVE_ARCH_NONE && !mpy_feature_arch_test(arch) {
        if mpy_feature_arch_test(MP_NATIVE_ARCH_NONE) {
            raise::raise(MpRaise::ValueError("native code in .mpy unsupported"));
        }
        raise::raise(MpRaise::ValueError("incompatible .mpy arch"));
    }

    let mut arch_flags = 0usize;
    if MPY_FEATURE_ARCH_FLAGS_TEST(header[2]) {
        arch_flags = read_uint(reader);
    }

    let n_qstr = read_uint(reader);
    let n_obj = read_uint(reader);
    emitglue::module_context_alloc_tables(cm.context, n_qstr, n_obj);

    unsafe {
        let ctx = &mut *cm.context;
        for i in 0..n_qstr {
            ctx.qstr_table_mut()[i] = load_qstr(reader);
        }
        for i in 0..n_obj {
            ctx.obj_table_mut()[i] = load_obj(reader);
        }
        cm.rc = load_raw_code(reader, cm.context);
        cm.has_native = arch != MP_NATIVE_ARCH_NONE;
        cm.n_qstr = n_qstr;
        cm.n_obj = n_obj;
        cm.arch_flags = arch_flags;
    }

    crate::nlr::pop_jump_callback(true);
}

pub fn raw_code_load_mem(buf: &[u8], cm: &mut CompiledModule) {
    if !mpconfig::PERSISTENT_CODE_LOAD {
        return;
    }
    let mut reader = Reader {
        data: core::ptr::null_mut(),
        readbyte: reader::reader_mem_readbyte,
        close: reader::reader_mem_close,
    };
    reader::reader_new_mem(&mut reader, buf.as_ptr(), buf.len(), 0);
    raw_code_load(&mut reader, cm);
}

pub fn raw_code_load_file(filename: Qstr, cm: &mut CompiledModule) {
    if !(mpconfig::PERSISTENT_CODE_LOAD && mpconfig::HAS_FILE_READER) {
        return;
    }
    let mut reader = Reader {
        data: core::ptr::null_mut(),
        readbyte: reader::reader_mem_readbyte,
        close: reader::reader_mem_close,
    };
    reader::reader_new_file(&mut reader, filename);
    raw_code_load(&mut reader, cm);
}

fn print_bytes(print: &Print, data: &[u8]) {
    if let Some(f) = print.print_strn {
        f(print.data, data.as_ptr(), data.len());
    }
}

fn print_uint(print: &Print, mut n: usize) {
    let mut buf = [0u8; bc::ENCODE_UINT_MAX_BYTES];
    let mut p = buf.len();
    p -= 1;
    buf[p] = (n & 0x7f) as u8;
    n >>= 7;
    while n != 0 {
        p -= 1;
        buf[p] = 0x80 | ((n & 0x7f) as u8);
        n >>= 7;
    }
    print_bytes(print, &buf[p..]);
}

fn save_qstr(print: &Print, qst: Qstr) {
    if qst <= QSTR_LAST_STATIC {
        print_uint(print, qst << 1 | 1);
        return;
    }
    if let Some((data, len)) = qstr::qstr_data(qst) {
        print_uint(print, len << 1);
        print_bytes(print, &data);
        print_bytes(print, &[0]);
    }
}

fn save_obj(print: &Print, o: Obj) {
    if mpconfig::EMIT_MACHINE_CODE && o.0 == nativeglue::fun_table_reloc_base() {
        print_bytes(print, &[MP_PERSISTENT_OBJ_FUN_TABLE]);
    } else if obj::is_str_or_bytes(o) {
        let obj_type = if obj::is_str(o) {
            MP_PERSISTENT_OBJ_STR
        } else {
            MP_PERSISTENT_OBJ_BYTES
        };
        let (data, len) = objstr::str_get_data(o);
        print_bytes(print, &[obj_type]);
        print_uint(print, len);
        print_bytes(print, &data[..len]);
        print_bytes(print, &[0]);
    } else if o == obj::CONST_NONE {
        print_bytes(print, &[MP_PERSISTENT_OBJ_NONE]);
    } else if o == obj::CONST_FALSE {
        print_bytes(print, &[MP_PERSISTENT_OBJ_FALSE]);
    } else if o == obj::CONST_TRUE {
        print_bytes(print, &[MP_PERSISTENT_OBJ_TRUE]);
    } else if o == crate::objsingleton::const_ellipsis() {
        print_bytes(print, &[MP_PERSISTENT_OBJ_ELLIPSIS]);
    } else if obj::is_exact_type(o, objtuple::type_tuple()) {
        let (len, items) = objtuple::tuple_get(o);
        print_bytes(print, &[MP_PERSISTENT_OBJ_TUPLE]);
        print_uint(print, len);
        for item in items {
            save_obj(print, item);
        }
    } else {
        let obj_type = if obj::is_int(o) {
            MP_PERSISTENT_OBJ_INT
        } else if mpconfig::PY_BUILTINS_COMPLEX
            && obj::is_exact_type(o, crate::objcomplex::type_complex())
        {
            MP_PERSISTENT_OBJ_COMPLEX
        } else {
            MP_PERSISTENT_OBJ_FLOAT
        };
        let mut v = Vstr {
            alloc: 0,
            len: 0,
            buf: core::ptr::null_mut(),
            fixed_buf: false,
        };
        let mut pr = Print {
            data: &mut v as *mut Vstr as *mut (),
            print_strn: Some(vstr::vstr_add_strn_print),
        };
        obj::print_helper(&mut pr, o, mpprint::PrintKind::Repr);
        print_bytes(print, &[obj_type]);
        let bytes = unsafe { std::slice::from_raw_parts(v.buf, v.len) };
        print_uint(print, bytes.len());
        print_bytes(print, bytes);
        vstr::clear(&mut v);
    }
}

fn save_raw_code(print: &Print, rc: *const RawCode) {
    if !mpconfig::PERSISTENT_CODE_SAVE {
        return;
    }
    unsafe {
        let kind_bits = (*rc).kind as u8 as usize - RawCodeKind::Bytecode as usize;
        print_uint(
            print,
            ((*rc).fun_data_len as usize) << 3
                | (((*rc).n_children != 0) as usize) << 2
                | kind_bits,
        );
        print_bytes(
            print,
            std::slice::from_raw_parts((*rc).fun_data, (*rc).fun_data_len as usize),
        );
        if mpconfig::EMIT_MACHINE_CODE {
            if (*rc).kind == RawCodeKind::NativePy {
                print_uint(print, (*rc).prelude_offset as usize);
            } else if (*rc).kind == RawCodeKind::NativeViper || (*rc).kind == RawCodeKind::NativeAsm {
                print_uint(print, 0);
                if (*rc).kind == RawCodeKind::NativeAsm && mpconfig::EMIT_INLINE_ASM {
                    print_uint(print, (*rc).asm_n_pos_args as usize);
                    print_uint(print, (*rc).asm_type_sig as usize);
                }
            }
        }
        if (*rc).n_children != 0 {
            print_uint(print, (*rc).n_children as usize);
            for i in 0..(*rc).n_children as usize {
                save_raw_code(print, *(*rc).children.add(i));
            }
        }
    }
}

pub fn raw_code_save(cm: &CompiledModule, print: &Print) {
    if !mpconfig::PERSISTENT_CODE_SAVE {
        return;
    }
    let header = [
        b'M',
        MPY_VERSION,
        (if cm.arch_flags != 0 {
            MPY_FEATURE_ARCH_FLAGS
        } else {
            0
        }) | (if cm.has_native {
            MPY_FEATURE_ENCODE_SUB_VERSION(MPY_SUB_VERSION) | MPY_FEATURE_ENCODE_ARCH(MPY_FEATURE_ARCH)
        } else {
            0
        }),
        smallint::BITS as u8,
    ];
    print_bytes(print, &header);
    if cm.arch_flags != 0 {
        print_uint(print, cm.arch_flags);
    }
    print_uint(print, cm.n_qstr);
    print_uint(print, cm.n_obj);
    unsafe {
        let ctx = &*cm.context;
        for i in 0..cm.n_qstr {
            save_qstr(print, ctx.qstr_table()[i]);
        }
        for i in 0..cm.n_obj {
            save_obj(print, ctx.obj_table()[i]);
        }
        save_raw_code(print, cm.rc);
    }
}

struct BitVector {
    max_bit_set: usize,
    alloc: usize,
    bits: Vec<usize>,
}

impl BitVector {
    fn new() -> Self {
        Self {
            max_bit_set: 0,
            alloc: 1,
            bits: vec![0],
        }
    }

    fn is_set(&self, index: usize) -> bool {
        let bits_size = size_of::<usize>() * 8;
        index / bits_size < self.alloc
            && (self.bits[index / bits_size] & (1 << (index % bits_size))) != 0
    }

    fn set(&mut self, index: usize) {
        let bits_size = size_of::<usize>() * 8;
        self.max_bit_set = self.max_bit_set.max(index);
        if index / bits_size >= self.alloc {
            self.alloc *= 2;
            self.bits.resize(self.alloc, 0);
        }
        self.bits[index / bits_size] |= 1 << (index % bits_size);
    }
}

struct RawCodeSimplified {
    fun_data: *const u8,
    fun_data_len: usize,
    n_children: usize,
    children: Vec<RawCodeSimplified>,
}

fn proto_fun_to_raw_code_simplified(
    proto_fun: ProtoFun,
    qstr_table_used: &mut BitVector,
    obj_table_used: &mut BitVector,
    rcs: &mut RawCodeSimplified,
) {
    let (fun_data, children) = if emitglue::proto_fun_is_bytecode(proto_fun) {
        (proto_fun as *const u8, core::ptr::null_mut())
    } else {
        let rc = proto_fun as *const RawCode;
        unsafe {
            if (*rc).kind != RawCodeKind::Bytecode {
                raise::raise(MpRaise::ValueError("function must be bytecode"));
            }
            ((*rc).fun_data, (*rc).children)
        }
    };
    let fun_data_top = unsafe { fun_data.add(gc::gc_nbytes(fun_data as *const u8)) };
        let mut ip = fun_data as *const u8;
        let sig = bc::prelude_sig_decode_into(&mut ip);
    let (n_info, n_cell) = bc::prelude_size_decode(&mut ip);
    let simple_name = bc::decode_uint_value(ip);
    qstr_table_used.set(simple_name);
    let mut ip_names = ip;
    for _ in 0..sig.n_pos_args + sig.n_kwonly_args {
        let arg_name = bc::decode_uint(&mut ip_names);
        qstr_table_used.set(arg_name);
    }
    ip = unsafe { ip.add(n_info + n_cell) };
    let mut n_children = 0usize;
    while ip < fun_data_top {
        let op = unsafe { *ip };
        if op == bc0::BASE_RESERVED {
            break;
        }
        let format = bc0::format(op);
        let mut arg = 0usize;
        let mut cur = ip;
        if format == bc0::FORMAT_QSTR || format == bc0::FORMAT_VAR_UINT {
            arg = unsafe { *cur } as usize & 0x7f;
            while unsafe { *cur & 0x80 != 0 } {
                cur = unsafe { cur.add(1) };
                arg = (arg << 7) | (unsafe { *cur } as usize & 0x7f);
            }
            cur = unsafe { cur.add(1) };
        } else if format == bc0::FORMAT_OFFSET {
            if unsafe { *cur & 0x80 == 0 } {
                cur = unsafe { cur.add(1) };
            } else {
                cur = unsafe { cur.add(2) };
            }
        }
        if format == bc0::FORMAT_QSTR {
            qstr_table_used.set(arg);
        } else if op == bc0::LOAD_CONST_OBJ {
            obj_table_used.set(arg);
        } else if matches!(
            op,
            bc0::MAKE_FUNCTION
                | bc0::MAKE_FUNCTION_DEFARGS
                | bc0::MAKE_CLOSURE
                | bc0::MAKE_CLOSURE_DEFARGS
        ) {
            n_children = n_children.max(arg + 1);
        }
        if op & bc0::MASK_EXTRA_BYTE == 0 {
            cur = unsafe { cur.add(1) };
        }
        cur = unsafe { cur.add(1) };
        ip = cur;
    }
    rcs.fun_data = fun_data;
    rcs.fun_data_len = unsafe { ip.offset_from(fun_data) as usize };
    rcs.n_children = n_children;
    rcs.children = Vec::new();
    if n_children > 0 && !children.is_null() {
        for i in 0..n_children {
            let mut child = RawCodeSimplified {
                fun_data: core::ptr::null(),
                fun_data_len: 0,
                n_children: 0,
                children: Vec::new(),
            };
            unsafe {
                proto_fun_to_raw_code_simplified(
                    *children.add(i) as ProtoFun,
                    qstr_table_used,
                    obj_table_used,
                    &mut child,
                );
            }
            rcs.children.push(child);
        }
    }
}

fn save_raw_code_simplified(print: &Print, rcs: &RawCodeSimplified) {
    print_uint(
        print,
        (rcs.fun_data_len << 3) | ((rcs.n_children != 0) as usize) << 2,
    );
    print_bytes(
        print,
        unsafe { std::slice::from_raw_parts(rcs.fun_data, rcs.fun_data_len) },
    );
    if rcs.n_children != 0 {
        print_uint(print, rcs.n_children);
        for child in &rcs.children {
            save_raw_code_simplified(print, child);
        }
    }
}

pub fn raw_code_save_fun_to_bytes(consts: &bc::ModuleConstants, proto_fun: ProtoFun) -> Obj {
    if !mpconfig::PERSISTENT_CODE_SAVE_FUN {
        return obj::OBJ_NULL;
    }
    let mut qstr_table_used = BitVector::new();
    let mut obj_table_used = BitVector::new();
    if mpconfig::PY_BUILTINS_CODE >= mpconfig::PY_BUILTINS_CODE_FULL {
        qstr_table_used.set(0);
    }
    let mut rcs = RawCodeSimplified {
        fun_data: core::ptr::null(),
        fun_data_len: 0,
        n_children: 0,
        children: Vec::new(),
    };
    proto_fun_to_raw_code_simplified(proto_fun, &mut qstr_table_used, &mut obj_table_used, &mut rcs);

    let mut v = Vstr {
        alloc: 0,
        len: 0,
        buf: core::ptr::null_mut(),
        fixed_buf: false,
    };
    let print = Print {
        data: &mut v as *mut Vstr as *mut (),
        print_strn: Some(vstr::vstr_add_strn_print),
    };
    let header = [b'M', MPY_VERSION, 0, smallint::BITS as u8];
    print_bytes(&print, &header);
    print_uint(&print, qstr_table_used.max_bit_set + 1);
    print_uint(&print, obj_table_used.max_bit_set + 1);
    for i in 0..=qstr_table_used.max_bit_set {
        if qstr_table_used.is_set(i) {
            save_qstr(&print, unsafe { consts.qstr_at(i) });
        } else {
            save_qstr(&print, qstr::QSTR_EMPTY);
        }
    }
    for i in 0..=obj_table_used.max_bit_set {
        if obj_table_used.is_set(i) {
            save_obj(&print, unsafe { consts.obj_at(i) });
        } else {
            save_obj(&print, obj::CONST_NONE);
        }
    }
    save_raw_code_simplified(&print, &rcs);
    objstr::new_bytes_from_vstr(&mut v)
}

pub fn native_relocate(ri: &mut RelocInfo<'_>, text: *mut u8, reloc_text: usize) {
    let fun_table = nativeglue::fun_table_reloc_entries();
    let fun_table_base = nativeglue::fun_table_reloc_base();
    let mut addr_to_adjust: *mut usize = core::ptr::null_mut();

    loop {
        let op = match reloc_read_byte(ri.reader) {
            None => break,
            Some(0xff) => break,
            Some(op) => op,
        };

        if op & 1 != 0 {
            let addr = read_uint(ri.reader);
            addr_to_adjust = if addr & 1 == 0 {
                unsafe { (text as *mut usize).add(addr >> 1) }
            } else {
                unsafe { (ri.rodata as *mut usize).add(addr >> 1) }
            };
        }

        let mut op = op >> 1;
        let mut dest = 0usize;
        let mut n = 1usize;
        if op <= 5 {
            if op & 1 != 0 {
                n = read_uint(ri.reader);
            }
            op >>= 1;
            dest = if op == 0 {
                reloc_text
            } else if op == 1 {
                ri.rodata as usize
            } else {
                ri.bss as usize
            };
        } else if op == 6 {
            dest = unsafe { (*ri.context).constants.qstr_table as usize };
        } else if op == 7 {
            dest = unsafe { (*ri.context).constants.obj_table as usize };
        } else if op == 8 {
            dest = fun_table_base;
        } else {
            dest = unsafe { *fun_table.add(op as usize - 9) };
        }

        while n > 0 {
            n -= 1;
            unsafe {
                *addr_to_adjust = addr_to_adjust.read().wrapping_add(dest);
                addr_to_adjust = addr_to_adjust.add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitglue::{CompiledModule, RawCodeKind};

    fn make_reader(data: &[u8]) -> Reader {
        let mut reader = Reader {
            data: core::ptr::null_mut(),
            readbyte: reader::reader_mem_readbyte,
            close: reader::reader_mem_close,
        };
        reader::reader_new_mem(&mut reader, data.as_ptr(), data.len(), 0);
        reader
    }

    #[test]
    fn viper_bss_rodata_and_relocation_load() {
        let mpy_arch = if cfg!(target_arch = "x86_64") {
            MP_NATIVE_ARCH_X64
        } else if cfg!(target_arch = "x86") {
            MP_NATIVE_ARCH_X86
        } else {
            return;
        };

        let _ = crate::gc::init();
        let _ = crate::qstr::init();

        let small_int_bits = 30u8;
        let mut data = Vec::new();
        data.extend_from_slice(&[
            b'M',
            MPY_VERSION,
            MPY_FEATURE_ENCODE_SUB_VERSION(MPY_SUB_VERSION) | MPY_FEATURE_ENCODE_ARCH(mpy_arch),
            small_int_bits,
        ]);
        data.push(0x02); // n_qstr
        data.push(0x00); // n_obj
        data.extend_from_slice(b"\x0emod2.py\x00");
        data.extend_from_slice(b"\x0aouter\x00");
        data.push(0x2c); // bytecode, 5 bytes, has children
        data.extend_from_slice(b"\x00\x02\x01\x51\x63");
        data.push(0x01); // 1 child
        data.push(0x22); // viper, 4 bytes, no children
        data.extend_from_slice(&[0, 0, 0, 0]); // dummy machine code
        data.push(0x70); // VIPERBSS | VIPERRODATA | VIPERRELOC
        data.extend_from_slice(b"\x06\x04"); // rodata=6, bss=4
        data.extend_from_slice(b"rodata");
        data.extend_from_slice(b"\x03\x01\x00"); // dummy relocation

        let mut reader = make_reader(&data);
        let mut cm = CompiledModule {
            context: malloc::new_obj::<ModuleContext>().expect("context"),
            rc: core::ptr::null(),
            has_native: false,
            n_qstr: 0,
            n_obj: 0,
            arch_flags: 0,
        };

        let mut nlr_buf = crate::nlr::NlrBuf::default();
        crate::nlr::protect(&mut nlr_buf, || raw_code_load(&mut reader, &mut cm)).expect("load");

        unsafe {
            assert_eq!((*cm.rc).kind, RawCodeKind::Bytecode);
            let child = *(*cm.rc).children;
            assert_eq!((*child).kind, RawCodeKind::NativeViper);
            assert_eq!((*child).fun_data_len, 4);
        }
    }

    #[test]
    fn native_relocate_adjusts_rodata_pointer() {
        let mut rodata = vec![0usize; 1];
        let reloc_bytes = [0x01, 0x01, 0xff];
        let mut reader = make_reader(&reloc_bytes);

        let mut ctx = ModuleContext {
            module: bc::ObjModule {
                base: crate::obj::ObjBase {
                    type_: core::ptr::null(),
                },
                globals: core::ptr::null_mut(),
            },
            constants: bc::ModuleConstants::default(),
            n_qstr: 0,
            n_obj: 0,
        };
        emitglue::module_context_alloc_tables(&mut ctx, 1, 1);

        let mut ri = RelocInfo {
            reader: &mut reader,
            context: &mut ctx,
            rodata: rodata.as_mut_ptr() as *mut u8,
            bss: core::ptr::null_mut(),
        };
        let text = vec![0u8; 8];
        let text_base = text.as_ptr() as usize;
        native_relocate(&mut ri, text.as_ptr() as *mut u8, text_base);

        assert_eq!(rodata[0], text_base);
    }
}
