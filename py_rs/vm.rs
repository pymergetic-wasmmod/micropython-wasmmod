//! rewrite of py/vm.c (bytecode interpreter)
// symmetry: done

use core::mem::size_of;
use core::ptr;

use crate::bc::{
    self, decode_uint, exc_sp_idx_from_ptr, exc_sp_idx_to_ptr, tagptr_make, tagptr_ptr, tagptr_tag1,
    CodeState, ExcStack, ModuleContext, ObjFunBc,
};
use crate::bc0;
use crate::emitglue;
use crate::map::{self, LookupKind};
use crate::mpconfig;
use crate::nlr::{self, NlrBuf};
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::objcell;
use crate::objdict;
use crate::objexcept;
use crate::objgenerator;
use crate::objlist;
use crate::objset;
use crate::objslice::{self, ObjSlice};
use crate::objtuple;
use crate::objtype::{self, ObjInstance};
use crate::qstr::{self, Qstr};
use crate::raise::{self, MpRaise};
use crate::runtime::{self, HandlePendingBehaviour, VmReturnKind};
use crate::runtime0::{BinaryOp, UnaryOp};

enum InnerResult {
    Return(VmReturnKind),
    ContinueOuter,
}

fn decode_ulabel(ip: &mut *const u8) -> isize {
    unsafe {
        let b0 = ip.read();
        if b0 & 0x80 != 0 {
            let b1 = ip.add(1).read();
            let ulab = ((b0 & 0x7f) as isize) | ((b1 as isize) << 7);
            *ip = ip.add(2);
            ulab
        } else {
            *ip = ip.add(1);
            b0 as isize
        }
    }
}

fn decode_slabel(ip: &mut *const u8) -> isize {
    unsafe {
        let b0 = ip.read();
        if b0 & 0x80 != 0 {
            let b1 = ip.add(1).read();
            let slab = ((b0 & 0x7f) as isize) | ((b1 as isize) << 7) - 0x4000;
            *ip = ip.add(2);
            slab
        } else {
            *ip = ip.add(1);
            b0 as isize - 0x40
        }
    }
}

fn decode_qstr(ip: &mut *const u8, qstr_table: &[Qstr]) -> Qstr {
    let unum = decode_uint(ip);
    if mpconfig::EMIT_BYTECODE_USES_QSTR_TABLE {
        qstr_table[unum]
    } else {
        unum as Qstr
    }
}

fn decode_ptr(ip: &mut *const u8, child_table: *const *const ()) -> *const () {
    let unum = decode_uint(ip);
    unsafe { *child_table.add(unum) }
}

fn decode_obj(ip: &mut *const u8, obj_table: &[Obj]) -> Obj {
    let unum = decode_uint(ip);
    obj_table[unum]
}

fn push(sp: &mut *mut Obj, val: Obj) {
    unsafe {
        *sp = sp.add(1);
        **sp = val;
    }
}

fn pop(sp: &mut *mut Obj) -> Obj {
    unsafe {
        let val = **sp;
        *sp = sp.sub(1);
        val
    }
}

fn top(sp: *mut Obj) -> Obj {
    unsafe { *sp }
}

fn set_top(sp: *mut Obj, val: Obj) {
    unsafe { *sp = val }
}

fn load_check(obj_shared: Obj) -> Obj {
    if obj_shared == obj::OBJ_NULL {
        raise::raise_obj(local_name_error());
    }
    obj_shared
}

fn local_name_error() -> Obj {
    objexcept::new_exception_args(
        objexcept::type_name_error(),
        1,
        &[obj::new_qstr(crate::qstr::from_str(
            "local variable referenced before assignment",
        ))],
    )
}

fn push_exc_block(ip: &mut *const u8, exc_sp: &mut *mut ExcStack, sp: *mut Obj, with_or_finally: bool) {
    let ulab = decode_ulabel(ip);
    unsafe {
        *exc_sp = exc_sp.add(1);
        (**exc_sp).handler = ip.add(ulab as usize);
        (**exc_sp).val_sp = tagptr_make(sp, if with_or_finally { 2 } else { 0 });
        (**exc_sp).prev_exc = ptr::null_mut();
    }
}

fn pop_exc_block(exc_sp: &mut *mut ExcStack) {
    unsafe {
        *exc_sp = exc_sp.sub(1);
    }
}

fn cancel_active_finally(sp: &mut *mut Obj) {
    unsafe {
        if obj::is_small_int(top(*sp)) {
            *sp.sub(2) = top(*sp);
            *sp = sp.sub(2);
        } else {
            debug_assert!(top(*sp) == obj::CONST_NONE || objexcept::is_exception_instance(top(*sp)));
            *sp.sub(1) = top(*sp);
            *sp = sp.sub(1);
        }
    }
}

fn nlr_val_to_exc(val: usize) -> Obj {
    let candidate = Obj(val);
    if val != 0 && objexcept::is_exception_instance(candidate) {
        return candidate;
    }
    mp_raise_to_exception(raise::decode(val))
}

fn mp_raise_to_exception(err: MpRaise) -> Obj {
    match err {
        MpRaise::TypeError(m) | MpRaise::AttributeError(m) => {
            objexcept::new_exception_args(objexcept::type_type_error(), 1, &[obj::new_qstr(crate::qstr::from_str(m))])
        }
        MpRaise::ValueError(m) => {
            objexcept::new_exception_args(objexcept::type_value_error(), 1, &[obj::new_qstr(crate::qstr::from_str(m))])
        }
        MpRaise::RuntimeError(m) => objexcept::new_exception_args(
            objexcept::type_runtime_error(),
            1,
            &[obj::new_qstr(crate::qstr::from_str(m))],
        ),
        MpRaise::OverflowError(m) => objexcept::new_exception_args(
            objexcept::type_overflow_error(),
            1,
            &[obj::new_qstr(crate::qstr::from_str(m))],
        ),
        MpRaise::ZeroDivisionError => objexcept::new_exception(objexcept::type_zero_division_error()),
        MpRaise::OSError(code) => objexcept::new_exception_args(
            objexcept::type_os_error(),
            1,
            &[obj::new_small_int(code as isize)],
        ),
        MpRaise::SyntaxError(m) => {
            objexcept::new_exception_args(objexcept::type_syntax_error(), 1, &[obj::new_qstr(crate::qstr::from_str(m))])
        }
        MpRaise::RecursionDepth => objexcept::new_exception(objexcept::type_runtime_error()),
    }
}

fn load_attr_fast(top: Obj, qst: Qstr) -> Obj {
    if mpconfig::OPT_LOAD_ATTR_FAST_PATH && obj::is_instance_type(obj::get_type(top)) {
        let self_ = unsafe { &mut *(obj::as_ptr(top) as *mut ObjInstance) };
        if let Some(elem) = map::lookup(&mut self_.members, obj::new_qstr(qst), LookupKind::Lookup) {
            return elem.value;
        }
    }
    runtime::load_attr(top, qst)
}

#[allow(clippy::too_many_arguments)]
fn build_slice_stack_allocated(op: u8, sp: *mut Obj, step: Obj) -> *mut Obj {
    unsafe {
        let stop = *sp.sub(2);
        let start = *sp.sub(1);
        let slice = ObjSlice {
            base: ObjBase {
                type_: objslice::type_slice() as *const ObjType,
            },
            start,
            stop,
            step,
        };
        let slice_obj = obj::from_ptr(&slice as *const ObjSlice as *const ());
        if op == bc0::LOAD_SUBSCR {
            set_top(sp.sub(2), obj::subscr(top(sp.sub(2)), slice_obj, obj::OBJ_SENTINEL));
            sp.sub(2)
        } else {
            obj::subscr(top(sp.sub(2)), slice_obj, *sp.sub(3));
            sp.sub(4)
        }
    }
}

fn pending_exception_check(code_state: &mut CodeState, ip: *const u8) -> *const u8 {
    let _ = code_state;
    if mpconfig::ENABLE_SCHEDULER || mpconfig::KBD_EXCEPTION {
        runtime::handle_pending(HandlePendingBehaviour::CallbacksAndClearExceptions);
    }
    ip
}

fn vm_unwind_jump(
    ip: &mut *const u8,
    sp: &mut *mut Obj,
    exc_sp: &mut *mut ExcStack,
    exc_stack: *mut ExcStack,
) {
    let mut unum = pop(sp).0;
    while (unum & 0x7f) > 0 {
        unum -= 1;
        debug_assert!(*exc_sp >= exc_stack);
        if tagptr_tag1(unsafe { (**exc_sp).val_sp }) {
            unsafe {
                if (**exc_sp).handler >= *ip {
                    push(sp, obj::new_small_int(unum as isize));
                    *ip = (**exc_sp).handler;
                    return;
                }
                cancel_active_finally(sp);
            }
        }
        pop_exc_block(exc_sp);
    }
    unsafe {
        *ip = pop(sp).0 as *const u8;
    }
    if unum != 0 {
        *sp = unsafe { sp.sub(obj::ITER_BUF_NSLOTS) };
    }
}

fn run_unwind_return(
    ip: &mut *const u8,
    sp: &mut *mut Obj,
    exc_sp: &mut *mut ExcStack,
    exc_stack: *mut ExcStack,
) -> bool {
    while *exc_sp >= exc_stack {
        if tagptr_tag1(unsafe { (**exc_sp).val_sp }) {
            unsafe {
                if (**exc_sp).handler >= *ip {
                    let finally_sp = tagptr_ptr((**exc_sp).val_sp);
                    *finally_sp.add(1) = top(*sp);
                    *sp = finally_sp.add(1);
                    push(sp, obj::new_small_int(-1));
                    *ip = (**exc_sp).handler;
                    return true;
                }
                cancel_active_finally(sp);
            }
        }
        pop_exc_block(exc_sp);
    }
    false
}

fn handle_exception(
    code_state: &mut CodeState,
    exc_stack: *mut ExcStack,
    exc_val: usize,
    inject_exc: &mut Obj,
) -> InnerResult {
    let exc = nlr_val_to_exc(exc_val);
    let ip = code_state.ip;

    if objtype::is_subclass_fast(
        obj::from_ptr(unsafe { (*(obj::as_ptr(exc) as *const ObjBase)).type_ as *const ObjType as *const () }),
        obj::from_ptr(objexcept::type_stop_iteration() as *const ObjType as *const ()),
    ) {
        if unsafe { *ip } == bc0::FOR_ITER {
            let mut p = unsafe { ip.add(1) };
            let ulab = decode_ulabel(&mut p);
            code_state.ip = unsafe { p.add(ulab as usize) };
            code_state.sp = unsafe { code_state.sp.sub(obj::ITER_BUF_NSLOTS) };
            *inject_exc = obj::OBJ_NULL;
            return InnerResult::ContinueOuter;
        }
        if unsafe { *ip } == bc0::YIELD_FROM {
            set_top(code_state.sp, objexcept::exception_get_value(exc));
            code_state.ip = unsafe { ip.add(1) };
            *inject_exc = obj::OBJ_NULL;
            return InnerResult::ContinueOuter;
        }
    }

    let mut exc_sp = exc_sp_idx_to_ptr(exc_stack, code_state.exc_sp_idx);
    while exc_sp >= exc_stack && unsafe { (*exc_sp).handler <= ip } {
        pop_exc_block(&mut exc_sp);
    }

    if exc_sp >= exc_stack {
        code_state.ip = unsafe { (*exc_sp).handler };
        let mut sp = tagptr_ptr(unsafe { (*exc_sp).val_sp });
        unsafe {
            (*exc_sp).prev_exc = obj::as_ptr(exc) as *mut ObjBase;
        }
        push(&mut sp, exc);
        code_state.sp = sp;
        code_state.exc_sp_idx = exc_sp_idx_from_ptr(exc_stack, exc_sp);
        *inject_exc = obj::OBJ_NULL;
        InnerResult::ContinueOuter
    } else {
        unsafe {
            *code_state.state_ptr() = exc;
        }
        InnerResult::Return(VmReturnKind::Exception)
    }
}

fn dispatch_loop(
    code_state: &mut CodeState,
    fun_bc: &ObjFunBc,
    qstr_table: &[Qstr],
    obj_table: &[Obj],
    child_table: *const *const (),
    fastn: *mut Obj,
    exc_stack: *mut ExcStack,
    inject_exc: &mut Obj,
) -> InnerResult {
    let mut ip = code_state.ip;
    let mut sp = code_state.sp;
    let mut exc_sp = exc_sp_idx_to_ptr(exc_stack, code_state.exc_sp_idx);

    if *inject_exc != obj::OBJ_NULL && unsafe { *ip != bc0::YIELD_FROM } {
        let exc = *inject_exc;
        *inject_exc = obj::OBJ_NULL;
        raise::raise_obj(runtime::make_raise_obj(exc));
    }

    loop {
        code_state.ip = ip;
        let op = unsafe { *ip };
        ip = unsafe { ip.add(1) };

        match op {
            bc0::LOAD_CONST_FALSE => push(&mut sp, obj::CONST_FALSE),
            bc0::LOAD_CONST_NONE => push(&mut sp, obj::CONST_NONE),
            bc0::LOAD_CONST_TRUE => push(&mut sp, obj::CONST_TRUE),

            bc0::LOAD_CONST_SMALL_INT => {
                let mut num: isize = 0;
                unsafe {
                    if (*ip) & 0x40 != 0 {
                        num -= 1;
                    }
                    loop {
                        num = (num << 7) | ((*ip & 0x7f) as isize);
                        let cont = (*ip & 0x80) != 0;
                        ip = ip.add(1);
                        if !cont {
                            break;
                        }
                    }
                }
                push(&mut sp, obj::new_small_int(num));
            }

            bc0::LOAD_CONST_STRING => {
                let qst = decode_qstr(&mut ip, qstr_table);
                push(&mut sp, obj::new_qstr(qst));
            }

            bc0::LOAD_CONST_OBJ => {
                let val = decode_obj(&mut ip, obj_table);
                push(&mut sp, val);
            }

            bc0::LOAD_NULL => push(&mut sp, obj::OBJ_NULL),

            bc0::LOAD_FAST_N => {
                let unum = decode_uint(&mut ip);
                let val = load_check(unsafe { *fastn.sub(unum) });
                push(&mut sp, val);
            }

            bc0::LOAD_DEREF => {
                let unum = decode_uint(&mut ip);
                let val = load_check(objcell::cell_get(unsafe { *fastn.sub(unum) }));
                push(&mut sp, val);
            }

            bc0::LOAD_NAME => {
                let qst = decode_qstr(&mut ip, qstr_table);
                push(&mut sp, runtime::load_name(qst));
            }

            bc0::LOAD_GLOBAL => {
                let qst = decode_qstr(&mut ip, qstr_table);
                push(&mut sp, runtime::load_global(qst));
            }

            bc0::LOAD_ATTR => {
                let qst = decode_qstr(&mut ip, qstr_table);
                set_top(sp, load_attr_fast(top(sp), qst));
            }

            bc0::LOAD_METHOD => {
                let qst = decode_qstr(&mut ip, qstr_table);
                let mut dest = [top(sp), obj::OBJ_NULL];
                runtime::load_method(dest[0], qst, &mut dest);
                set_top(sp, dest[0]);
                push(&mut sp, dest[1]);
            }

            bc0::LOAD_SUPER_METHOD => {
                let qst = decode_qstr(&mut ip, qstr_table);
                sp = unsafe { sp.sub(1) };
                let mut dest = [obj::OBJ_NULL; 3];
                dest[1] = unsafe { *sp.sub(1) };
                dest[2] = unsafe { *sp };
                objtype::load_super_method(qst, &mut dest);
                set_top(sp, dest[0]);
                push(&mut sp, dest[1]);
            }

            bc0::LOAD_BUILD_CLASS => push(&mut sp, runtime::load_build_class()),

            bc0::LOAD_SUBSCR => {
                let index = pop(&mut sp);
                set_top(sp, obj::subscr(top(sp), index, obj::OBJ_SENTINEL));
            }

            bc0::STORE_FAST_N => {
                let unum = decode_uint(&mut ip);
                unsafe { *fastn.sub(unum) = pop(&mut sp) };
            }

            bc0::STORE_DEREF => {
                let unum = decode_uint(&mut ip);
                objcell::cell_set(unsafe { *fastn.sub(unum) }, pop(&mut sp));
            }

            bc0::STORE_NAME => {
                let qst = decode_qstr(&mut ip, qstr_table);
                runtime::store_name(qst, pop(&mut sp));
            }

            bc0::STORE_GLOBAL => {
                let qst = decode_qstr(&mut ip, qstr_table);
                runtime::store_global(qst, pop(&mut sp));
            }

            bc0::STORE_ATTR => {
                let qst = decode_qstr(&mut ip, qstr_table);
                let value = unsafe { *sp.sub(1) };
                runtime::store_attr(top(sp), qst, value);
                sp = unsafe { sp.sub(2) };
            }

            bc0::STORE_SUBSCR => {
                let value = unsafe { *sp.sub(2) };
                let index = unsafe { *sp.sub(1) };
                obj::subscr(top(sp), index, value);
                sp = unsafe { sp.sub(3) };
            }

            bc0::DELETE_FAST => {
                let unum = decode_uint(&mut ip);
                if unsafe { *fastn.sub(unum) } == obj::OBJ_NULL {
                    raise::raise_obj(local_name_error());
                }
                unsafe { *fastn.sub(unum) = obj::OBJ_NULL };
            }

            bc0::DELETE_DEREF => {
                let unum = decode_uint(&mut ip);
                if objcell::cell_get(unsafe { *fastn.sub(unum) }) == obj::OBJ_NULL {
                    raise::raise_obj(local_name_error());
                }
                objcell::cell_set(unsafe { *fastn.sub(unum) }, obj::OBJ_NULL);
            }

            bc0::DELETE_NAME => {
                let qst = decode_qstr(&mut ip, qstr_table);
                runtime::delete_name(qst);
            }

            bc0::DELETE_GLOBAL => {
                let qst = decode_qstr(&mut ip, qstr_table);
                runtime::delete_global(qst);
            }

            bc0::DUP_TOP => {
                let t = top(sp);
                push(&mut sp, t);
            }

            bc0::DUP_TOP_TWO => {
                unsafe {
                    sp = sp.add(2);
                    *sp = *sp.sub(2);
                    *sp.sub(1) = *sp.sub(3);
                }
            }

            bc0::POP_TOP => sp = unsafe { sp.sub(1) },

            bc0::ROT_TWO => unsafe {
                let t = *sp;
                *sp = *sp.sub(1);
                *sp.sub(1) = t;
            },

            bc0::ROT_THREE => unsafe {
                let t = *sp;
                *sp = *sp.sub(1);
                *sp.sub(1) = *sp.sub(2);
                *sp.sub(2) = t;
            },

            bc0::JUMP => {
                let slab = decode_slabel(&mut ip);
                ip = unsafe { ip.offset(slab) };
                ip = pending_exception_check(code_state, ip);
            }

            bc0::POP_JUMP_IF_TRUE => {
                let slab = decode_slabel(&mut ip);
                if obj::is_true(pop(&mut sp)) {
                    ip = unsafe { ip.offset(slab) };
                }
                ip = pending_exception_check(code_state, ip);
            }

            bc0::POP_JUMP_IF_FALSE => {
                let slab = decode_slabel(&mut ip);
                if !obj::is_true(pop(&mut sp)) {
                    ip = unsafe { ip.offset(slab) };
                }
                ip = pending_exception_check(code_state, ip);
            }

            bc0::JUMP_IF_TRUE_OR_POP => {
                let ulab = decode_ulabel(&mut ip);
                if obj::is_true(top(sp)) {
                    ip = unsafe { ip.offset(ulab) };
                } else {
                    sp = unsafe { sp.sub(1) };
                }
                ip = pending_exception_check(code_state, ip);
            }

            bc0::JUMP_IF_FALSE_OR_POP => {
                let ulab = decode_ulabel(&mut ip);
                if obj::is_true(top(sp)) {
                    sp = unsafe { sp.sub(1) };
                } else {
                    ip = unsafe { ip.offset(ulab) };
                }
                ip = pending_exception_check(code_state, ip);
            }

            bc0::SETUP_WITH => {
                let obj_ = top(sp);
                let mut m_exit = [obj::OBJ_NULL; 2];
                runtime::load_method(obj_, qstr::from_str("__exit__"), &mut m_exit);
                let mut m_enter = [obj::OBJ_NULL; 2];
                runtime::load_method(obj_, qstr::from_str("__enter__"), &mut m_enter);
                let call_args = [m_enter[0], m_enter[1], obj::OBJ_NULL];
                let ret = runtime::call_method_n_kw(0, 0, &call_args);
                push(&mut sp, m_exit[1]);
                push(&mut sp, m_exit[0]);
                push_exc_block(&mut ip, &mut exc_sp, sp, true);
                push(&mut sp, ret);
            }

            bc0::WITH_CLEANUP => {
                if top(sp) == obj::CONST_NONE {
                    unsafe {
                        *sp.sub(1) = obj::CONST_NONE;
                        *sp.sub(2) = obj::CONST_NONE;
                        sp = sp.sub(2);
                    }
                    let args = [unsafe { *sp.sub(2) }, unsafe { *sp.sub(1) }, unsafe { *sp }, obj::OBJ_NULL];
                    runtime::call_method_n_kw(3, 0, &args);
                    set_top(sp, obj::CONST_NONE);
                } else if obj::is_small_int(top(sp)) {
                    let data = unsafe { *sp.sub(1) };
                    let cause = top(sp);
                    unsafe {
                        *sp.sub(1) = obj::CONST_NONE;
                        *sp = obj::CONST_NONE;
                        *sp.add(1) = obj::CONST_NONE;
                    }
                    let args = [
                        unsafe { *sp.sub(3) },
                        unsafe { *sp.sub(2) },
                        obj::CONST_NONE,
                        obj::OBJ_NULL,
                    ];
                    runtime::call_method_n_kw(3, 0, &args);
                    unsafe {
                        *sp.sub(3) = data;
                        *sp.sub(2) = cause;
                        sp = sp.sub(2);
                    }
                } else {
                    debug_assert!(objexcept::is_exception_instance(top(sp)));
                    unsafe {
                        *sp.sub(1) = top(sp);
                        *sp = obj::from_ptr(obj::get_type(top(sp)) as *const ObjType as *const ());
                        *sp.add(1) = obj::CONST_NONE;
                        sp = sp.sub(2);
                    }
                    let ret_value = runtime::call_method_n_kw(3, 0, unsafe {
                        &[*sp, *sp.add(1), *sp.add(2), obj::OBJ_NULL]
                    });
                    if obj::is_true(ret_value) {
                        set_top(sp, obj::CONST_NONE);
                    } else {
                        unsafe { *sp = *sp.add(3) };
                    }
                }
            }

            bc0::UNWIND_JUMP => {
                let slab = decode_slabel(&mut ip);
                push(&mut sp, obj::from_ptr(unsafe { ip.offset(slab) } as *const ()));
                push(&mut sp, obj::new_small_int(unsafe { *ip } as isize));
                ip = unsafe { ip.add(1) };
                vm_unwind_jump(&mut ip, &mut sp, &mut exc_sp, exc_stack);
                ip = pending_exception_check(code_state, ip);
            }

            bc0::SETUP_EXCEPT => push_exc_block(&mut ip, &mut exc_sp, sp, false),
            bc0::SETUP_FINALLY => push_exc_block(&mut ip, &mut exc_sp, sp, true),

            bc0::END_FINALLY => {
                pop_exc_block(&mut exc_sp);
                if top(sp) == obj::CONST_NONE {
                    sp = unsafe { sp.sub(1) };
                } else if obj::is_small_int(top(sp)) {
                    let cause = obj::small_int_value(pop(&mut sp));
                    if cause < 0 {
                        if run_unwind_return(&mut ip, &mut sp, &mut exc_sp, exc_stack) {
                            continue;
                        }
                        code_state.sp = sp;
                        code_state.exc_sp_idx = exc_sp_idx_from_ptr(exc_stack, exc_sp);
                        return InnerResult::Return(VmReturnKind::Normal);
                    }
                    push(&mut sp, obj::new_small_int(cause));
                    vm_unwind_jump(&mut ip, &mut sp, &mut exc_sp, exc_stack);
                    ip = pending_exception_check(code_state, ip);
                } else {
                    raise::raise_obj(top(sp));
                }
            }

            bc0::GET_ITER => set_top(sp, runtime::getiter(top(sp), None)),

            bc0::GET_ITER_STACK => {
                let obj_ = top(sp);
                let iter_buf_ptr = sp;
                sp = unsafe { sp.add(obj::ITER_BUF_NSLOTS - 1) };
                let iter = runtime::getiter(obj_, Some(unsafe { &mut *(iter_buf_ptr as *mut obj::ObjIterBuf) }));
                if iter != obj::from_ptr(iter_buf_ptr as *const ()) {
                    unsafe {
                        *iter_buf_ptr.sub(obj::ITER_BUF_NSLOTS - 1).add(1) = obj::OBJ_NULL;
                        *iter_buf_ptr.sub(obj::ITER_BUF_NSLOTS - 1).add(2) = iter;
                    }
                }
            }

            bc0::FOR_ITER => {
                let ulab = decode_ulabel(&mut ip);
                code_state.sp = sp;
                let iter_obj = unsafe {
                    if *sp.sub(obj::ITER_BUF_NSLOTS - 1).add(1) == obj::OBJ_NULL {
                        *sp.sub(obj::ITER_BUF_NSLOTS - 1).add(2)
                    } else {
                        obj::from_ptr(sp.sub(obj::ITER_BUF_NSLOTS - 1).add(1) as *const ())
                    }
                };
                let value = runtime::iternext_allow_raise(iter_obj);
                if value == obj::OBJ_STOP_ITERATION {
                    sp = unsafe { sp.sub(obj::ITER_BUF_NSLOTS) };
                    ip = unsafe { ip.offset(ulab) };
                } else {
                    push(&mut sp, value);
                }
            }

            bc0::POP_EXCEPT_JUMP => {
                pop_exc_block(&mut exc_sp);
                let ulab = decode_ulabel(&mut ip);
                ip = unsafe { ip.offset(ulab) };
                ip = pending_exception_check(code_state, ip);
            }

            bc0::BUILD_TUPLE => {
                let unum = decode_uint(&mut ip);
                sp = unsafe { sp.sub(unum - 1) };
                let items = unsafe { std::slice::from_raw_parts(sp, unum) };
                set_top(sp, objtuple::new_tuple(unum, Some(items)));
            }

            bc0::BUILD_LIST => {
                let unum = decode_uint(&mut ip);
                sp = unsafe { sp.sub(unum - 1) };
                let items = unsafe { std::slice::from_raw_parts(sp, unum) };
                set_top(sp, objlist::new_list(unum, Some(items)));
            }

            bc0::BUILD_MAP => {
                let unum = decode_uint(&mut ip);
                push(&mut sp, objdict::new_dict(unum));
            }

            bc0::STORE_MAP => {
                let value = unsafe { *sp.sub(1) };
                let key = top(sp);
                objdict::dict_store(unsafe { *sp.sub(2) }, key, value);
                sp = unsafe { sp.sub(2) };
            }

            bc0::BUILD_SET => {
                if mpconfig::PY_BUILTINS_SET {
                    let unum = decode_uint(&mut ip);
                    sp = unsafe { sp.sub(unum - 1) };
                    let items = unsafe { std::slice::from_raw_parts(sp, unum) };
                    set_top(sp, objset::new_set(unum, Some(items)));
                }
            }

            bc0::BUILD_SLICE => {
                if mpconfig::PY_BUILTINS_SLICE {
                    let mut step = obj::CONST_NONE;
                    if unsafe { *ip == 3 } {
                        ip = unsafe { ip.add(1) };
                        step = pop(&mut sp);
                    }
                    if (unsafe { *ip == bc0::LOAD_SUBSCR || *ip == bc0::STORE_SUBSCR })
                        && (obj::get_type(unsafe { *sp.sub(2) }).flags
                            & obj::TYPE_FLAG_SUBSCR_ALLOWS_STACK_SLICE
                            != 0)
                    {
                        let op = unsafe { *ip };
                        ip = unsafe { ip.add(1) };
                        sp = build_slice_stack_allocated(op, unsafe { sp.sub(2) }, step);
                    } else {
                        let stop = pop(&mut sp);
                        set_top(sp, objslice::new_slice(top(sp), stop, step));
                    }
                }
            }

            bc0::STORE_COMP => {
                let unum = decode_uint(&mut ip);
                let kind = unum & 3;
                let obj_ = unsafe { *sp.sub(unum >> 2) };
                if kind == 0 {
                    objlist::list_append(obj_, pop(&mut sp));
                } else if !mpconfig::PY_BUILTINS_SET || kind == 1 {
                    let value = pop(&mut sp);
                    let key = top(sp);
                    objdict::dict_store(obj_, key, value);
                    sp = unsafe { sp.sub(1) };
                } else {
                    objset::set_store(obj_, pop(&mut sp));
                }
            }

            bc0::UNPACK_SEQUENCE => {
                let unum = decode_uint(&mut ip);
                let mut items = vec![obj::OBJ_NULL; unum];
                runtime::unpack_sequence(top(sp), unum, &mut items);
                pop(&mut sp);
                for item in items {
                    push(&mut sp, item);
                }
            }

            bc0::UNPACK_EX => {
                let unum = decode_uint(&mut ip);
                let nitems = (unum & 0xff) + ((unum >> 8) & 0xff) + 1;
                let mut items = vec![obj::OBJ_NULL; nitems];
                runtime::unpack_ex(top(sp), unum, &mut items);
                pop(&mut sp);
                for item in items {
                    push(&mut sp, item);
                }
            }

            bc0::MAKE_FUNCTION => {
                let ptr = decode_ptr(&mut ip, child_table);
                push(
                    &mut sp,
                    emitglue::make_function_from_proto_fun(ptr, fun_bc.context, None),
                );
            }

            bc0::MAKE_FUNCTION_DEFARGS => {
                let ptr = decode_ptr(&mut ip, child_table);
                sp = unsafe { sp.sub(1) };
                let def_args = [top(sp), unsafe { *sp.add(1) }];
                set_top(
                    sp,
                    emitglue::make_function_from_proto_fun(ptr, fun_bc.context, Some(&def_args)),
                );
            }

            bc0::MAKE_CLOSURE => {
                let ptr = decode_ptr(&mut ip, child_table);
                let n_closed = unsafe { *ip } as usize;
                ip = unsafe { ip.add(1) };
                sp = unsafe { sp.sub(n_closed - 1) };
                let closed = unsafe { std::slice::from_raw_parts(sp, n_closed) };
                set_top(
                    sp,
                    emitglue::make_closure_from_proto_fun(ptr, fun_bc.context, n_closed, closed),
                );
            }

            bc0::MAKE_CLOSURE_DEFARGS => {
                let ptr = decode_ptr(&mut ip, child_table);
                let n_closed = unsafe { *ip } as usize;
                ip = unsafe { ip.add(1) };
                sp = unsafe { sp.sub(2 + n_closed - 1) };
                let args = unsafe { std::slice::from_raw_parts(sp, 2 + n_closed) };
                set_top(
                    sp,
                    emitglue::make_closure_from_proto_fun(ptr, fun_bc.context, 0x100 | n_closed, args),
                );
            }

            bc0::CALL_FUNCTION => {
                let unum = decode_uint(&mut ip);
                let n_pos = unum & 0xff;
                let n_kw = (unum >> 8) & 0xff;
                sp = unsafe { sp.sub(n_pos + ((unum >> 7) & 0x1fe)) };
                let args = unsafe { std::slice::from_raw_parts(sp.add(1), n_pos + 2 * n_kw) };
                set_top(sp, runtime::call_function_n_kw(top(sp), n_pos, n_kw, args));
            }

            bc0::CALL_FUNCTION_VAR_KW => {
                let unum = decode_uint(&mut ip);
                sp = unsafe { sp.sub((unum & 0xff) + ((unum >> 7) & 0x1fe) + 1) };
                let args = unsafe {
                    std::slice::from_raw_parts(sp, (unum & 0xff) + 2 * ((unum >> 8) & 0xff) + 1)
                };
                set_top(sp, runtime::call_method_n_kw_var(false, unum, args));
            }

            bc0::CALL_METHOD => {
                let unum = decode_uint(&mut ip);
                let n_pos = unum & 0xff;
                let n_kw = (unum >> 8) & 0xff;
                sp = unsafe { sp.sub(n_pos + ((unum >> 7) & 0x1fe) + 1) };
                let args = unsafe { std::slice::from_raw_parts(sp, 2 + n_pos + 2 * n_kw) };
                set_top(sp, runtime::call_method_n_kw(n_pos, n_kw, args));
            }

            bc0::CALL_METHOD_VAR_KW => {
                let unum = decode_uint(&mut ip);
                sp = unsafe { sp.sub((unum & 0xff) + ((unum >> 7) & 0x1fe) + 2) };
                let args = unsafe {
                    std::slice::from_raw_parts(sp, 2 + (unum & 0xff) + 2 * ((unum >> 8) & 0xff) + 1)
                };
                set_top(sp, runtime::call_method_n_kw_var(true, unum, args));
            }

            bc0::RETURN_VALUE => {
                if run_unwind_return(&mut ip, &mut sp, &mut exc_sp, exc_stack) {
                    continue;
                }
                code_state.sp = sp;
                code_state.exc_sp_idx = exc_sp_idx_from_ptr(exc_stack, exc_sp);
                return InnerResult::Return(VmReturnKind::Normal);
            }

            bc0::RAISE_LAST => {
                let mut obj_ = obj::OBJ_NULL;
                let mut e = exc_sp;
                while e >= exc_stack {
                    if unsafe { (*e).prev_exc != ptr::null_mut() } {
                        obj_ = obj::from_ptr(unsafe { (*e).prev_exc as *const () });
                        break;
                    }
                    e = unsafe { e.sub(1) };
                }
                if obj_ == obj::OBJ_NULL {
                    obj_ = objexcept::new_exception_args(
                        objexcept::type_runtime_error(),
                        1,
                        &[obj::new_qstr(crate::qstr::from_str("no active exception to reraise"))],
                    );
                }
                raise::raise_obj(obj_);
            }

            bc0::RAISE_OBJ => raise::raise_obj(runtime::make_raise_obj(top(sp))),

            bc0::RAISE_FROM => {
                let from_value = pop(&mut sp);
                if from_value != obj::CONST_NONE {
                    // exception chaining not supported on host
                }
                raise::raise_obj(runtime::make_raise_obj(top(sp)));
            }

            bc0::YIELD_VALUE => {
                code_state.ip = ip;
                code_state.sp = sp;
                code_state.exc_sp_idx = exc_sp_idx_from_ptr(exc_stack, exc_sp);
                return InnerResult::Return(VmReturnKind::Yield);
            }

            bc0::YIELD_FROM => {
                let send_value = pop(&mut sp);
                let mut t_exc = obj::OBJ_NULL;
                let mut ret_value = obj::OBJ_NULL;
                code_state.sp = sp;
                let ret_kind = if *inject_exc != obj::OBJ_NULL {
                    t_exc = *inject_exc;
                    *inject_exc = obj::OBJ_NULL;
                    runtime::resume(top(sp), obj::OBJ_NULL, t_exc, &mut ret_value)
                } else {
                    runtime::resume(top(sp), send_value, obj::OBJ_NULL, &mut ret_value)
                };
                match ret_kind {
                    VmReturnKind::Yield => {
                        ip = unsafe { ip.sub(1) };
                        push(&mut sp, ret_value);
                        code_state.ip = ip;
                        code_state.sp = sp;
                        code_state.exc_sp_idx = exc_sp_idx_from_ptr(exc_stack, exc_sp);
                        return InnerResult::Return(VmReturnKind::Yield);
                    }
                    VmReturnKind::Normal => {
                        set_top(sp, ret_value);
                        if t_exc != obj::OBJ_NULL
                            && objexcept::exception_match(
                                t_exc,
                                obj::from_ptr(
                                    objexcept::type_generator_exit() as *const ObjType as *const (),
                                ),
                            )
                        {
                            raise::raise_obj(runtime::make_raise_obj(t_exc));
                        }
                    }
                    VmReturnKind::Exception => {
                        sp = unsafe { sp.sub(1) };
                        raise::raise_obj(ret_value);
                    }
                }
            }

            bc0::IMPORT_NAME => {
                let qst = decode_qstr(&mut ip, qstr_table);
                let fromlist = pop(&mut sp);
                set_top(sp, runtime::import_name(qst, fromlist, top(sp)));
            }

            bc0::IMPORT_FROM => {
                let qst = decode_qstr(&mut ip, qstr_table);
                let module = top(sp);
                push(&mut sp, runtime::import_from(module, qst));
            }

            bc0::IMPORT_STAR => runtime::import_all(pop(&mut sp)),

            _ => {
                if op < bc0::LOAD_CONST_SMALL_INT_MULTI + bc0::LOAD_CONST_SMALL_INT_MULTI_NUM {
                    push(
                        &mut sp,
                        obj::new_small_int(
                            (op as isize)
                                - bc0::LOAD_CONST_SMALL_INT_MULTI as isize
                                - bc0::LOAD_CONST_SMALL_INT_MULTI_EXCESS as isize,
                        ),
                    );
                } else if op < bc0::LOAD_FAST_MULTI + bc0::LOAD_FAST_MULTI_NUM {
                    let unum = bc0::LOAD_FAST_MULTI - op;
                    push(&mut sp, load_check(unsafe { *fastn.sub(unum as usize) }));
                } else if op < bc0::STORE_FAST_MULTI + bc0::STORE_FAST_MULTI_NUM {
                    let unum = bc0::STORE_FAST_MULTI - op;
                    unsafe { *fastn.sub(unum as usize) = pop(&mut sp) };
                } else if op < bc0::UNARY_OP_MULTI + bc0::UNARY_OP_MULTI_NUM {
                    let uop = unsafe { core::mem::transmute::<u8, UnaryOp>(op - bc0::UNARY_OP_MULTI) };
                    set_top(sp, runtime::unary_op_obj(uop, top(sp)));
                } else if op < bc0::BINARY_OP_MULTI + bc0::BINARY_OP_MULTI_NUM {
                    let bop = unsafe { core::mem::transmute::<u8, BinaryOp>(op - bc0::BINARY_OP_MULTI) };
                    let rhs = pop(&mut sp);
                    set_top(sp, runtime::binary_op_obj(bop, top(sp), rhs));
                } else {
                    unsafe {
                        *code_state.state_ptr() = objexcept::new_exception_args(
                            objexcept::type_not_implemented_error(),
                            1,
                            &[obj::new_qstr(crate::qstr::from_str("opcode"))],
                        );
                    }
                    return InnerResult::Return(VmReturnKind::Exception);
                }
            }
        }
    }
}

/// `mp_execute_bytecode`
pub fn execute_bytecode(code_state: &mut CodeState, mut inject_exc: Obj) -> VmReturnKind {
    let n_state = code_state.n_state as usize;
    let state_base = code_state.state_ptr();
    let fastn = unsafe { state_base.add(n_state - 1) };
    let exc_stack = unsafe { state_base.add(n_state) as *mut ExcStack };

    let fun_bc = unsafe { &*code_state.fun_bc };
    let ctx = unsafe { &*fun_bc.context };
    let qstr_table = ctx.qstr_table();
    let obj_table = ctx.obj_table();
    let child_table = fun_bc.child_table;

    loop {
        let mut nlr_buf = NlrBuf::default();
        match nlr::protect(&mut nlr_buf, || {
            dispatch_loop(
                code_state,
                fun_bc,
                qstr_table,
                obj_table,
                child_table,
                fastn,
                exc_stack,
                &mut inject_exc,
            )
        }) {
            Ok(InnerResult::Return(kind)) => return kind,
            Ok(InnerResult::ContinueOuter) => continue,
            Err(val) => match handle_exception(code_state, exc_stack, val, &mut inject_exc) {
                InnerResult::Return(kind) => return kind,
                InnerResult::ContinueOuter => continue,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::{ModuleConstants, ModuleContext, ObjModule};
    use crate::gc;
    use crate::mpstate;
    use crate::objdict::ObjDict;
    use crate::objfun;
    use crate::runtime;

    fn setup() {
        runtime::init();
    }

    fn encode_uint(mut n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (n & 0x7f) as u8;
            n >>= 7;
            if n != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if n == 0 {
                break;
            }
        }
        out
    }

    /// Bytecode prelude + body for `return 1+2`.
    fn one_plus_two_bytecode() -> Vec<u8> {
        let mut bc = vec![0x00u8]; // n_state=1 prelude sig
        bc.extend(encode_uint(0)); // block name index 0
        bc.push(0x81); // LOAD_CONST_SMALL_INT_MULTI 1
        bc.push(0x82); // LOAD_CONST_SMALL_INT_MULTI 2
        bc.push(0xd7 + BinaryOp::Add as u8); // BINARY_OP_MULTI Add
        bc.push(bc0::RETURN_VALUE);
        bc
    }

    fn run_bytecode(bc: &[u8]) -> Obj {
        let globals = crate::objdict::new_dict(0);
        mpstate::globals_set(globals);
        let globals_ptr = obj::as_ptr(globals) as *mut ObjDict;
        let ctx = Box::leak(Box::new(ModuleContext {
            module: ObjModule {
                base: ObjBase { type_: core::ptr::null() },
                globals: globals_ptr,
            },
            constants: ModuleConstants::default(),
            n_qstr: 0,
            n_obj: 0,
        }));
        crate::emitglue::module_context_alloc_tables(ctx, 1, 0);
        ctx.qstr_table_mut()[0] = crate::qstr::from_str("<module>");
        let bc = Box::leak(bc.to_vec().into_boxed_slice());
        let fun = objfun::new_fun_bc(None, bc.as_ptr(), ctx, core::ptr::null());
        runtime::call_function_n_kw(fun, 0, 0, &[])
    }

    #[test]
    fn execute_one_plus_two() {
        setup();
        let result = run_bytecode(&one_plus_two_bytecode());
        assert_eq!(result, obj::new_small_int(3));
    }

    #[test]
    fn execute_return_none() {
        setup();
        let mut bc = vec![0x00u8];
        bc.extend(encode_uint(0));
        bc.push(bc0::LOAD_CONST_NONE);
        bc.push(bc0::RETURN_VALUE);
        let result = run_bytecode(&bc);
        assert_eq!(result, obj::CONST_NONE);
    }
}
