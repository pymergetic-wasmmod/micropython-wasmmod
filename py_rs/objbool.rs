//! rewrite of py/objbool.c
// symmetry: done

use crate::qstr;
use crate::argcheck;
use crate::mpconfig;
use crate::mpprint::{self, Print, PrintKind};
use crate::obj::{self, Obj, ObjBase, ObjType};
use crate::raise::{self, MpRaise};
use crate::runtime;
use crate::runtime0::{BinaryOp, UnaryOp};

#[repr(C)]
pub struct ObjBool {
    pub base: ObjBase,
    pub value: bool,
}

static TYPE_BOOL: ObjType = obj::empty_type(0);

#[inline]
fn bool_value(o: Obj) -> bool {
    if mpconfig::OBJ_IMMEDIATE_OBJS {
        obj::bool_value(o)
    } else {
        unsafe { (*(obj::as_ptr(o) as *const ObjBool)).value }
    }
}

/// `bool_print` from objbool.c
pub fn bool_print(print: &Print, self_in: Obj, kind: PrintKind) {
    let value = bool_value(self_in);
    if mpconfig::PY_JSON && kind == PrintKind::Json {
        mpprint::print_str(print, if value { "true" } else { "false" });
    } else {
        mpprint::print_str(print, if value { "True" } else { "False" });
    }
}

/// `bool_make_new` from objbool.c
pub fn bool_make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 0, 1, false);
    if n_args == 0 {
        obj::CONST_FALSE
    } else {
        obj::new_bool(obj::is_true(args[0]))
    }
}

/// `bool_unary_op` from objbool.c
pub fn bool_unary_op(op: UnaryOp, o_in: Obj) -> Obj {
    if op == UnaryOp::Len {
        return obj::OBJ_NULL;
    }
    let value = bool_value(o_in);
    match runtime::unary_op(op, i32::from(value) as obj::Int) {
        Ok(o) => o,
        Err(e) => raise::raise(MpRaise::TypeError(e.message())),
    }
}

/// `bool_binary_op` from objbool.c
pub fn bool_binary_op(op: BinaryOp, lhs_in: Obj, rhs_in: Obj) -> Obj {
    let value = bool_value(lhs_in);
    match runtime::binary_op(op, obj::new_small_int(i32::from(value) as obj::Int), rhs_in) {
        Ok(o) => o,
        Err(e) => raise::raise(MpRaise::TypeError(e.message())),
    }
}

pub fn type_bool() -> &'static ObjType {
    &TYPE_BOOL
}
