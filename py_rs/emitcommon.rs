//! rewrite of py/emitcommon.c
// symmetry: done

use crate::emit::{self, EmitCommon};
use crate::emit::{Emit, EmitIdOps};
use crate::emitbc;
use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::objfloat;
use crate::objtuple;
use crate::qstr::Qstr;
use crate::scope::{self, IdInfoKind, Scope};

fn strictly_equal(a: Obj, b: Obj) -> bool {
    if a == b {
        return true;
    }
    let a_type = obj::get_type(a);
    let b_type = obj::get_type(b);
    if !core::ptr::eq(a_type, b_type) {
        return false;
    }
    if core::ptr::eq(a_type, objtuple::type_tuple()) {
        let (_, a_items) = objtuple::tuple_get(a);
        let (_, b_items) = objtuple::tuple_get(b);
        if a_items.len() != b_items.len() {
            return false;
        }
        for i in 0..a_items.len() {
            if !strictly_equal(a_items[i], b_items[i]) {
                return false;
            }
        }
        return true;
    }
    if !obj::equal(a, b) {
        return false;
    }
    if mpconfig::PY_BUILTINS_FLOAT && mpconfig::COMP_CONST_FLOAT {
        if core::ptr::eq(a_type, objfloat::type_float()) {
            let a_val = objfloat::get_float(a);
            if a_val == 0.0 {
                let b_val = objfloat::get_float(b);
                return a_val.signum() == b_val.signum();
            }
        }
    }
    true
}

/// `mp_emit_common_use_const_obj`
pub fn use_const_obj(emit: &mut EmitCommon, const_obj: Obj) -> usize {
    for (i, &item) in emit.const_obj_list.iter().enumerate() {
        if strictly_equal(item, const_obj) {
            return i;
        }
    }
    emit.const_obj_list.push(const_obj);
    emit.const_obj_list.len() - 1
}

/// `mp_emit_common_get_id_for_modification`
pub fn get_id_for_modification(scope: &mut Scope, qst: Qstr) {
    emit::emit_common_get_id_for_modification(scope, qst);
}

/// `mp_emit_common_id_op`
pub fn id_op(emit: *mut Emit, ops: EmitIdOps, scope: &Scope, qst: Qstr) {
    let id = scope::find(scope, qst).expect("identifier must exist");
    match id.kind {
        IdInfoKind::GlobalImplicit | IdInfoKind::GlobalImplicitAssigned => {
            let f = match ops {
                EmitIdOps::Load => emitbc::load_global,
                EmitIdOps::Store => emitbc::store_global,
                EmitIdOps::Delete => emitbc::delete_global,
            };
            f(emit, qst, emit::EMIT_IDOP_GLOBAL_NAME);
        }
        IdInfoKind::GlobalExplicit => {
            let f = match ops {
                EmitIdOps::Load => emitbc::load_global,
                EmitIdOps::Store => emitbc::store_global,
                EmitIdOps::Delete => emitbc::delete_global,
            };
            f(emit, qst, emit::EMIT_IDOP_GLOBAL_GLOBAL);
        }
        IdInfoKind::Local => {
            let f = match ops {
                EmitIdOps::Load => emitbc::load_local,
                EmitIdOps::Store => emitbc::store_local,
                EmitIdOps::Delete => emitbc::delete_local,
            };
            f(emit, qst, id.local_num as usize, emit::EMIT_IDOP_LOCAL_FAST);
        }
        IdInfoKind::Cell | IdInfoKind::Free => {
            let f = match ops {
                EmitIdOps::Load => emitbc::load_local,
                EmitIdOps::Store => emitbc::store_local,
                EmitIdOps::Delete => emitbc::delete_local,
            };
            f(
                emit,
                qst,
                id.local_num as usize,
                emit::EMIT_IDOP_LOCAL_DEREF,
            );
        }
        _ => unreachable!("unexpected id kind"),
    }
}
