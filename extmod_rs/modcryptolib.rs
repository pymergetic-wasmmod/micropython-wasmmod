//! rewrite of extmod/modcryptolib.c
// symmetry: done

use aes::{Aes128, Aes256};
use cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use py_rs::argcheck;
use py_rs::bc::ModuleContext;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::obj::{
    self, BufferInfo, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN,
    TYPE_FLAG_NONE,
};
use py_rs::objdict::{self, ObjDict};
use py_rs::objmodule;
use py_rs::objstr;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

const MODE_ECB: i32 = 1;
const MODE_CBC: i32 = 2;
const MODE_CTR: i32 = 6;

const KEYTYPE_NONE: u8 = 0;
const KEYTYPE_ENC: u8 = 1;
const KEYTYPE_DEC: u8 = 2;

type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltinVar {
    base: ObjBase,
    min_args: u8,
    max_args: u8,
    fun: BuiltinFnVar,
}

static mut FV: [*const (); 1] = [callv as *const ()];
static TV: ObjType = ObjType {
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
    slots: unsafe { FV.as_ptr() },
};

fn callv(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    let self_ = unsafe { &*(obj::as_ptr(s) as *const ObjFunBuiltinVar) };
    argcheck::check_num(
        n,
        k,
        self_.min_args as usize,
        self_.max_args as usize,
        false,
    );
    (self_.fun)(n, a)
}

fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("cryptolib fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

enum AesCipher {
    None,
    A128(Aes128),
    A256(Aes256),
}

#[repr(C)]
struct ObjAes {
    base: ObjBase,
    block_mode: u8,
    key_type: u8,
    key: [u8; 32],
    key_len: u8,
    iv: [u8; 16],
    ctr_offset: u8,
    encrypted_counter: [u8; 16],
    cipher: AesCipher,
}

fn is_ctr_mode(block_mode: u8) -> bool {
    mpconfig::PY_CRYPTOLIB_CTR && block_mode == MODE_CTR as u8
}

fn aes_ptr(o: Obj) -> *mut ObjAes {
    obj::as_ptr(o) as *mut ObjAes
}

fn get_buf_read(o: Obj) -> Vec<u8> {
    let mut info = BufferInfo::default();
    obj::get_buffer_raise(o, &mut info, obj::BUFFER_READ);
    unsafe { std::slice::from_raw_parts(info.buf as *const u8, info.len).to_vec() }
}

fn init_cipher(o: &mut ObjAes) {
    o.cipher = match o.key_len {
        16 => AesCipher::A128(Aes128::new(GenericArray::from_slice(&o.key[..16]))),
        32 => AesCipher::A256(Aes256::new(GenericArray::from_slice(&o.key[..32]))),
        _ => raise::raise(MpRaise::ValueError("key")),
    };
}

fn ensure_key(o: &mut ObjAes, encrypt: bool) {
    if o.key_type == KEYTYPE_NONE {
        init_cipher(o);
        o.key_type = if encrypt { KEYTYPE_ENC } else { KEYTYPE_DEC };
    } else if (encrypt && o.key_type == KEYTYPE_DEC) || (!encrypt && o.key_type == KEYTYPE_ENC) {
        raise::raise(MpRaise::ValueError("can't encrypt & decrypt"));
    }
}

fn encrypt_block(o: &ObjAes, block: &mut GenericArray<u8, cipher::consts::U16>) {
    match &o.cipher {
        AesCipher::A128(c) => c.encrypt_block(block),
        AesCipher::A256(c) => c.encrypt_block(block),
        AesCipher::None => raise::raise(MpRaise::RuntimeError("aes key not set")),
    }
}

fn decrypt_block(o: &ObjAes, block: &mut GenericArray<u8, cipher::consts::U16>) {
    match &o.cipher {
        AesCipher::A128(c) => c.decrypt_block(block),
        AesCipher::A256(c) => c.decrypt_block(block),
        AesCipher::None => raise::raise(MpRaise::RuntimeError("aes key not set")),
    }
}

fn process_ecb(o: &ObjAes, input: &[u8], output: &mut [u8], encrypt: bool) {
    for (chunk_in, chunk_out) in input.chunks(16).zip(output.chunks_mut(16)) {
        let mut block = GenericArray::clone_from_slice(chunk_in);
        if encrypt {
            encrypt_block(o, &mut block);
        } else {
            decrypt_block(o, &mut block);
        }
        chunk_out.copy_from_slice(block.as_slice());
    }
}

fn process_cbc(o: &mut ObjAes, input: &[u8], output: &mut [u8], encrypt: bool) {
    let mut iv = o.iv;
    if encrypt {
        for (chunk_in, chunk_out) in input.chunks(16).zip(output.chunks_mut(16)) {
            let mut block = GenericArray::from(*GenericArray::from_slice(chunk_in));
            for i in 0..16 {
                block[i] ^= iv[i];
            }
            encrypt_block(o, &mut block);
            iv = *block.as_ref();
            chunk_out.copy_from_slice(block.as_slice());
        }
    } else {
        for (chunk_in, chunk_out) in input.chunks(16).zip(output.chunks_mut(16)) {
            let mut block = GenericArray::clone_from_slice(chunk_in);
            let saved = *block.as_ref();
            decrypt_block(o, &mut block);
            for i in 0..16 {
                block[i] ^= iv[i];
            }
            iv = saved;
            chunk_out.copy_from_slice(block.as_slice());
        }
    }
    o.iv = iv;
}

fn process_ctr(o: &mut ObjAes, input: &[u8], output: &mut [u8]) {
    let mut n = o.ctr_offset as usize;
    for (in_b, out_b) in input.iter().zip(output.iter_mut()) {
        if n == 0 {
            let mut block = GenericArray::from(*GenericArray::from_slice(&o.iv));
            encrypt_block(o, &mut block);
            o.encrypted_counter = *block.as_ref();
            for i in (0..16).rev() {
                if o.iv[i] == 0xff {
                    o.iv[i] = 0;
                } else {
                    o.iv[i] += 1;
                    break;
                }
            }
        }
        *out_b = *in_b ^ o.encrypted_counter[n];
        n = (n + 1) & 0xf;
    }
    o.ctr_offset = n as u8;
}

fn aes_process(n_args: usize, args: &[Obj], encrypt: bool) -> Obj {
    let self_in = args[0];
    let o = unsafe { &mut *aes_ptr(self_in) };

    let in_buf = args[1];
    let out_buf = if n_args > 2 { args[2] } else { obj::OBJ_NULL };

    let in_data = get_buf_read(in_buf);
    if !is_ctr_mode(o.block_mode) && in_data.len() % 16 != 0 {
        raise::raise(MpRaise::ValueError("blksize % 16"));
    }

    ensure_key(o, encrypt);

    if out_buf != obj::OBJ_NULL {
        let mut out_info = BufferInfo::default();
        obj::get_buffer_raise(out_buf, &mut out_info, obj::BUFFER_WRITE);
        if out_info.len < in_data.len() {
            raise::raise(MpRaise::ValueError("output too small"));
        }
        let out_slice =
            unsafe { std::slice::from_raw_parts_mut(out_info.buf as *mut u8, in_data.len()) };
        match o.block_mode {
            m if m == MODE_ECB as u8 => process_ecb(o, &in_data, out_slice, encrypt),
            m if m == MODE_CBC as u8 => process_cbc(o, &in_data, out_slice, encrypt),
            m if is_ctr_mode(m) => process_ctr(o, &in_data, out_slice),
            _ => raise::raise(MpRaise::ValueError("mode")),
        }
        out_buf
    } else {
        let mut out_data = vec![0u8; in_data.len()];
        match o.block_mode {
            m if m == MODE_ECB as u8 => process_ecb(o, &in_data, &mut out_data, encrypt),
            m if m == MODE_CBC as u8 => process_cbc(o, &in_data, &mut out_data, encrypt),
            m if is_ctr_mode(m) => process_ctr(o, &in_data, &mut out_data),
            _ => raise::raise(MpRaise::ValueError("mode")),
        }
        objstr::new_bytes(&out_data)
    }
}

fn aes_encrypt(n: usize, args: &[Obj]) -> Obj {
    aes_process(n, args, true)
}

fn aes_decrypt(n: usize, args: &[Obj]) -> Obj {
    aes_process(n, args, false)
}

fn aes_make_new(type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 2, 3, false);

    let block_mode = obj::get_int(args[1]) as i32;
    match block_mode {
        MODE_ECB | MODE_CBC => {}
        MODE_CTR if mpconfig::PY_CRYPTOLIB_CTR => {}
        _ => raise::raise(MpRaise::ValueError("mode")),
    }

    let key = get_buf_read(args[0]);
    if key.len() != 16 && key.len() != 32 {
        raise::raise(MpRaise::ValueError("key"));
    }

    let mut iv = [0u8; 16];
    let has_iv = if n_args > 2 && args[2] != obj::CONST_NONE {
        let iv_buf = get_buf_read(args[2]);
        if iv_buf.len() != 16 {
            raise::raise(MpRaise::ValueError("IV"));
        }
        iv.copy_from_slice(&iv_buf);
        true
    } else {
        false
    };

    if (block_mode == MODE_CBC || is_ctr_mode(block_mode as u8)) && !has_iv {
        raise::raise(MpRaise::ValueError("IV"));
    }

    let o = malloc::new_obj::<ObjAes>().expect("aes");
    unsafe {
        (*o).base.type_ = type_in as *const ObjType;
        (*o).block_mode = block_mode as u8;
        (*o).key_type = KEYTYPE_NONE;
        (*o).key = [0u8; 32];
        std::ptr::copy_nonoverlapping(key.as_ptr(), (*o).key.as_mut_ptr(), key.len());
        (*o).key_len = key.len() as u8;
        (*o).iv = iv;
        (*o).ctr_offset = 0;
        (*o).encrypted_counter = [0u8; 16];
        (*o).cipher = AesCipher::None;
        obj::from_ptr(o as *const ObjAes as *const ())
    }
}

static mut AES_SLOTS: [*const (); 2] = [aes_make_new as *const (), core::ptr::null()];
static mut TYPE_AES: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
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
    slot_index_locals_dict: 2,
    slots: unsafe { AES_SLOTS.as_ptr() },
};

static AES_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_aes_type() -> &'static ObjType {
    AES_INIT.get_or_init(|| {
        let table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("encrypt")),
                value: mkv(2, 3, aes_encrypt),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("decrypt")),
                value: mkv(2, 3, aes_decrypt),
            },
        ];
        let ptr = obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict())
            as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            AES_SLOTS[1] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_AES.name = qstr::from_str("aes");
        }
    });
    unsafe { &TYPE_AES }
}

/// Register built-in `cryptolib` module (`MP_REGISTER_EXTENSIBLE_MODULE`).
pub fn init_module() -> Obj {
    if !mpconfig::PY_CRYPTOLIB {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("cryptolib")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("aes")),
            value: obj::from_ptr(init_aes_type() as *const ObjType as *const ()),
        },
    ];
    if mpconfig::PY_CRYPTOLIB_CONSTS {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("MODE_ECB")),
            value: obj::new_int(MODE_ECB as isize),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("MODE_CBC")),
            value: obj::new_int(MODE_CBC as isize),
        });
        if mpconfig::PY_CRYPTOLIB_CTR {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("MODE_CTR")),
                value: obj::new_int(MODE_CTR as isize),
            });
        }
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("cryptolib module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("cryptolib"), module);
    module
}
