//! rewrite of extmod/modrandom.c
// symmetry: done

use py_rs::bc::ModuleContext;
use py_rs::map::{self, MapElem};
use py_rs::malloc;
use py_rs::mpconfig;
use py_rs::obj::{self, Obj, ObjBase, ObjType, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict;
use py_rs::objfloat::{self, MpFloat};
use py_rs::objmodule;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};
use std::sync::{LazyLock, Mutex};

type BuiltinFn0 = fn() -> Obj;
type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFnVar = fn(usize, &[Obj]) -> Obj;

#[repr(C)]
struct ObjFunBuiltin0 {
    base: ObjBase,
    fun: BuiltinFn0,
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

static mut F0: [*const (); 1] = [call0 as *const ()];
static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut FV: [*const (); 1] = [callv as *const ()];
static T0: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    slots: unsafe { F0.as_ptr() },
};
static T1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
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
    base: ObjBase { type_: core::ptr::null() },
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
    base: ObjBase { type_: core::ptr::null() },
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

fn call0(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 0, 0, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin0)).fun)() }
}
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
    py_rs::argcheck::check_num(n, k, self_.min_args as usize, self_.max_args as usize, false);
    (self_.fun)(n, a)
}
fn mk0(f: BuiltinFn0) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin0>().expect("random fn0");
    unsafe {
        (*o).base.type_ = &T0;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin0 as *const ())
    }
}
fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("random fn1");
    unsafe {
        (*o).base.type_ = &T1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("random fn2");
    unsafe {
        (*o).base.type_ = &T2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mkv(min: u8, max: u8, f: BuiltinFnVar) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltinVar>().expect("random fnv");
    unsafe {
        (*o).base.type_ = &TV;
        (*o).min_args = min;
        (*o).max_args = max;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltinVar as *const ())
    }
}

struct Yasmarang {
    pad: u32,
    n: u32,
    d: u32,
    dat: u8,
}

impl Yasmarang {
    fn new() -> Self {
        Self {
            pad: 0xeda4_baba,
            n: 69,
            d: 233,
            dat: 0,
        }
    }

    fn next(&mut self) -> u32 {
        self.pad = self
            .pad
            .wrapping_add(self.dat as u32)
            .wrapping_add(self.d.wrapping_mul(self.n));
        self.pad = (self.pad << 3) | (self.pad >> 29);
        self.n = self.pad | 2;
        self.d ^= (self.pad << 31).wrapping_add(self.pad >> 1);
        self.dat ^= (self.pad as u8) ^ (self.d >> 8) as u8 ^ 1;
        self.pad ^ (self.d << 5) ^ (self.pad >> 18) ^ ((self.dat as u32) << 1)
    }

    fn randbelow(&mut self, n: u32) -> u32 {
        let mut mask = 1u32;
        while (n & mask) < n {
            mask = (mask << 1) | 1;
        }
        loop {
            let r = self.next() & mask;
            if r < n {
                return r;
            }
        }
    }

    fn float01(&mut self) -> f64 {
        let bits = self.next();
        1.0 + (bits as f64) / (u32::MAX as f64 + 1.0) - 1.0
    }
}

static PRNG: LazyLock<Mutex<Yasmarang>> = LazyLock::new(|| Mutex::new(Yasmarang::new()));

fn getrandbits(n_in: Obj) -> Obj {
    let n = obj::get_int(n_in);
    if n > 32 || n < 0 {
        raise::raise(MpRaise::ValueError("bits must be 32 or less"));
    }
    if n == 0 {
        return obj::new_small_int(0);
    }
    let mask = !0u32 >> (32 - n as u32);
    let mut prng = PRNG.lock().unwrap();
    obj::new_int((prng.next() & mask) as isize)
}

fn seed(n: usize, args: &[Obj]) -> Obj {
    let seed = if n == 0 || args[0] == obj::CONST_NONE {
        raise::raise(MpRaise::ValueError("no default seed"));
    } else {
        obj::get_int_truncated(args[0]) as u32
    };
    let mut prng = PRNG.lock().unwrap();
    prng.pad = seed;
    prng.n = 69;
    prng.d = 233;
    prng.dat = 0;
    obj::CONST_NONE
}

fn randrange(n: usize, args: &[Obj]) -> Obj {
    let start = obj::get_int(args[0]);
    let mut prng = PRNG.lock().unwrap();
    if n == 1 {
        if start <= 0 {
            raise::raise(MpRaise::ValueError(""));
        }
        return obj::new_int(prng.randbelow(start as u32) as isize);
    }
    let stop = obj::get_int(args[1]);
    if n == 2 {
        if start >= stop {
            raise::raise(MpRaise::ValueError(""));
        }
        return obj::new_int(start + prng.randbelow((stop - start) as u32) as isize);
    }
    let step = obj::get_int(args[2]);
    let count = if step > 0 {
        (stop - start + step - 1) / step
    } else if step < 0 {
        (stop - start + step + 1) / step
    } else {
        raise::raise(MpRaise::ValueError(""));
    };
    if count <= 0 {
        raise::raise(MpRaise::ValueError(""));
    }
    obj::new_int(start + step * prng.randbelow(count as u32) as isize)
}

fn randint(a_in: Obj, b_in: Obj) -> Obj {
    let a = obj::get_int(a_in);
    let b = obj::get_int(b_in);
    if a > b {
        raise::raise(MpRaise::ValueError(""));
    }
    let mut prng = PRNG.lock().unwrap();
    obj::new_int(a + prng.randbelow((b - a + 1) as u32) as isize)
}

fn choice(seq: Obj) -> Obj {
    let len = obj::get_int(obj::len(seq));
    if len <= 0 {
        raise::raise(MpRaise::RuntimeError("empty sequence"));
    }
    let mut prng = PRNG.lock().unwrap();
    let idx = obj::new_int(prng.randbelow(len as u32) as isize);
    obj::subscr(seq, idx, obj::OBJ_SENTINEL)
}

fn random_float() -> Obj {
    let mut prng = PRNG.lock().unwrap();
    objfloat::new_float(prng.float01() as MpFloat)
}

fn uniform(a_in: Obj, b_in: Obj) -> Obj {
    let a = objfloat::get_float(a_in) as f64;
    let b = objfloat::get_float(b_in) as f64;
    let mut prng = PRNG.lock().unwrap();
    objfloat::new_float(a + (b - a) * prng.float01())
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_RANDOM {
        return obj::OBJ_NULL;
    }
    let mut table = vec![
        MapElem {
            key: obj::new_qstr(qstr::from_str("__name__")),
            value: obj::new_qstr(qstr::from_str("random")),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("getrandbits")),
            value: mk1(getrandbits),
        },
        MapElem {
            key: obj::new_qstr(qstr::from_str("seed")),
            value: mkv(0, 1, seed),
        },
    ];
    if mpconfig::PY_RANDOM_EXTRA_FUNCS {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("randrange")),
            value: mkv(1, 3, randrange),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("randint")),
            value: mk2(randint),
        });
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("choice")),
            value: mk1(choice),
        });
        if mpconfig::PY_BUILTINS_FLOAT {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("random")),
                value: mk0(random_float),
            });
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("uniform")),
                value: mk2(uniform),
            });
        }
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("random");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("random"), module);
    module
}
