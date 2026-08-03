//! rewrite of extmod/machine_mem.c
// symmetry: done

use std::sync::OnceLock;

use py_rs::mpconfig;
use py_rs::mpprint::{self, Print, PrintKind, VaArg};
use py_rs::obj::{self, Obj, ObjBase, ObjType, OBJ_SENTINEL};
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

type MemGetAddrFn = fn(Obj, usize) -> usize;

static MEM_GET_ADDR: OnceLock<MemGetAddrFn> = OnceLock::new();

fn default_mem_get_addr(addr_o: Obj, align: usize) -> usize {
    let addr = obj::get_int_truncated(addr_o) as usize;
    if align > 0 && (addr & (align - 1)) != 0 {
        raise::raise(MpRaise::ValueError("address is not aligned"));
    }
    addr
}

fn mem_get_read_addr(addr_o: Obj, align: usize) -> usize {
    MEM_GET_ADDR.get().copied().unwrap_or(default_mem_get_addr)(addr_o, align)
}

fn mem_get_write_addr(addr_o: Obj, align: usize) -> usize {
    mem_get_read_addr(addr_o, align)
}

/// Port hook for `/dev/mem` mapping etc. (`MICROPY_MACHINE_MEM_GET_READ_ADDR`).
pub fn set_mem_get_addr_hook(hook: MemGetAddrFn) {
    let _ = MEM_GET_ADDR.set(hook);
}

#[repr(C)]
pub struct MachineMemObj {
    pub base: ObjBase,
    pub elem_size: u32,
}

fn mem_ptr(o: Obj) -> *const MachineMemObj {
    obj::as_ptr(o) as *const MachineMemObj
}

fn mem_print(print: &Print, self_in: Obj, _kind: PrintKind) {
    let self_ = unsafe { &*mem_ptr(self_in) };
    mpprint::printf(
        print,
        "<{}-bit memory>",
        [VaArg::Int((8 * self_.elem_size) as i32)],
    );
}

fn mem_subscr(self_in: Obj, index: Obj, value: Obj) -> Obj {
    let self_ = unsafe { &*mem_ptr(self_in) };
    let align = self_.elem_size as usize;
    if value == obj::OBJ_NULL {
        return obj::OBJ_NULL;
    }
    if value == OBJ_SENTINEL {
        let addr = mem_get_read_addr(index, align);
        let val = unsafe {
            match self_.elem_size {
                1 => *(addr as *const u8) as u32,
                2 => *(addr as *const u16) as u32,
                _ => *(addr as *const u32),
            }
        };
        return obj::new_small_int(val as isize);
    }
    let addr = mem_get_write_addr(index, align);
    let val = obj::get_int_truncated(value) as u32;
    unsafe {
        match self_.elem_size {
            1 => *(addr as *mut u8) = val as u8,
            2 => *(addr as *mut u16) = val as u16,
            _ => *(addr as *mut u32) = val,
        }
    }
    obj::CONST_NONE
}

static mut MEM_SLOTS: [*const (); 2] = [mem_print as *const (), mem_subscr as *const ()];
static mut MEM_TYPE: ObjType = ObjType {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 2,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 3,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { MEM_SLOTS.as_ptr() },
};

static mut MEM8: MachineMemObj = MachineMemObj {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    elem_size: 1,
};
static mut MEM16: MachineMemObj = MachineMemObj {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    elem_size: 2,
};
static mut MEM32: MachineMemObj = MachineMemObj {
    base: ObjBase {
        type_: core::ptr::null(),
    },
    elem_size: 4,
};

static INIT: OnceLock<()> = OnceLock::new();

fn init_mem() {
    INIT.get_or_init(|| unsafe {
        MEM_TYPE.name = qstr::from_str("mem");
        MEM8.base.type_ = &MEM_TYPE;
        MEM16.base.type_ = &MEM_TYPE;
        MEM32.base.type_ = &MEM_TYPE;
    });
}

/// `machine_mem8_obj`
pub fn mem8_obj() -> Obj {
    if !mpconfig::PY_MACHINE_MEMX {
        return obj::OBJ_NULL;
    }
    init_mem();
    unsafe { obj::from_ptr(&raw const MEM8 as *const MachineMemObj as *const ()) }
}

/// `machine_mem16_obj`
pub fn mem16_obj() -> Obj {
    if !mpconfig::PY_MACHINE_MEMX {
        return obj::OBJ_NULL;
    }
    init_mem();
    unsafe { obj::from_ptr(&raw const MEM16 as *const MachineMemObj as *const ()) }
}

/// `machine_mem32_obj`
pub fn mem32_obj() -> Obj {
    if !mpconfig::PY_MACHINE_MEMX {
        return obj::OBJ_NULL;
    }
    init_mem();
    unsafe { obj::from_ptr(&raw const MEM32 as *const MachineMemObj as *const ()) }
}
