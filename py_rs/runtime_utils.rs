//! rewrite of py/runtime_utils.c
// symmetry: done

use crate::misc;
use crate::mpprint;
use crate::nlr::{self, NlrBuf};
use crate::obj::{self, Int, Obj};
use crate::qstr;
use crate::runtime;

/// `mp_call_function_1_protected` — catch exceptions and print them.
pub fn call_function_1_protected(fun: Obj, arg: Obj) -> Obj {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::call_function_1(fun, arg)) {
        Ok(ret) => ret,
        Err(exc) => {
            let print = &mpprint::PLAT_PRINT;
            let _ = mpprint::print_str(&print, "Unhandled exception in protected call:\n");
            obj::print_helper(&print, Obj(exc), mpprint::PrintKind::Exc);
            obj::OBJ_NULL
        }
    }
}

/// `mp_call_function_2_protected`.
pub fn call_function_2_protected(fun: Obj, arg1: Obj, arg2: Obj) -> Obj {
    let mut nlr_buf = NlrBuf::default();
    match nlr::protect(&mut nlr_buf, || runtime::call_function_2(fun, arg1, arg2)) {
        Ok(ret) => ret,
        Err(exc) => {
            let print = &mpprint::PLAT_PRINT;
            let _ = mpprint::print_str(&print, "Unhandled exception in protected call:\n");
            obj::print_helper(&print, Obj(exc), mpprint::PrintKind::Exc);
            obj::OBJ_NULL
        }
    }
}

/// `mp_mul_ll_overflow`.
pub fn mul_ll_overflow(x: i64, y: i64, res: &mut i64) -> bool {
    misc::mp_mul_ll_overflow(x, y, res)
}

/// `mp_mul_mp_int_t_overflow`.
pub fn mul_mp_int_t_overflow(x: Int, y: Int, res: &mut Int) -> bool {
    misc::mp_mul_mp_int_t_overflow(x, y, res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    fn setup() {
        let _ = gc::init();
        runtime::init();
    }

    #[test]
    fn mul_overflow_detects_product() {
        let mut res = 0i64;
        assert!(mul_ll_overflow(i64::MAX, 2, &mut res));
        assert!(!mul_ll_overflow(3, 4, &mut res));
        assert_eq!(res, 12);
    }

    #[test]
    fn protected_call_returns_on_success() {
        setup();
        let module = crate::modbuiltins::init_builtins_module();
        let globals = crate::objmodule::module_get_globals(module);
        let len_key = obj::new_qstr(qstr::from_str("len"));
        let len_fn = crate::map::lookup(
            unsafe { &mut (*globals).map },
            len_key,
            crate::map::LookupKind::Lookup,
        )
        .expect("len builtin")
        .value;
        let ret = call_function_1_protected(len_fn, crate::objstr::new_str(b"3"));
        assert_eq!(obj::small_int_value(ret), 1);
    }
}
