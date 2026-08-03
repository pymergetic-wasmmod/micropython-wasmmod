//! rewrite of py/moderrno.c
// symmetry: done

use crate::bc::ModuleContext;
use crate::map::{self, LookupKind, MapElem};
use crate::malloc;
use crate::mperrno;
use crate::mpconfig;
use crate::obj::{self, Obj, ObjType};
use crate::objdict::{self, ObjDict};
use crate::objmodule;
use crate::qstr::{self, Qstr};

macro_rules! errno_list {
    ($($name:ident),* $(,)?) => {
        &[$( (stringify!($name), stringify!($name)) ),*]
    };
}

const ERRNO_LIST: &[(&str, &str)] = errno_list! {
    EPERM, ENOENT, EIO, EBADF, EAGAIN, ENOMEM, EACCES, EEXIST, ENODEV, EISDIR, EINVAL,
    EOPNOTSUPP, EADDRINUSE, ECONNABORTED, ECONNRESET, ENOBUFS, ENOTCONN, ETIMEDOUT,
    ECONNREFUSED, EHOSTUNREACH, EALREADY, EINPROGRESS,
};

fn errno_value(name: &str) -> i32 {
    match name {
        "EPERM" => mperrno::EPERM,
        "ENOENT" => mperrno::ENOENT,
        "EIO" => mperrno::EIO,
        "EBADF" => mperrno::EBADF,
        "EAGAIN" => mperrno::EAGAIN,
        "ENOMEM" => mperrno::ENOMEM,
        "EACCES" => mperrno::EACCES,
        "EEXIST" => mperrno::EEXIST,
        "ENODEV" => mperrno::ENODEV,
        "EISDIR" => mperrno::EISDIR,
        "EINVAL" => mperrno::EINVAL,
        "EOPNOTSUPP" => mperrno::EOPNOTSUPP,
        "EADDRINUSE" => mperrno::EADDRINUSE,
        "ECONNABORTED" => mperrno::ECONNABORTED,
        "ECONNRESET" => mperrno::ECONNRESET,
        "ENOBUFS" => mperrno::ENOBUFS,
        "ENOTCONN" => mperrno::ENOTCONN,
        "ETIMEDOUT" => mperrno::ETIMEDOUT,
        "ECONNREFUSED" => mperrno::ECONNREFUSED,
        "EHOSTUNREACH" => mperrno::EHOSTUNREACH,
        "EALREADY" => mperrno::EALREADY,
        "EINPROGRESS" => mperrno::EINPROGRESS,
        _ => 0,
    }
}

static mut ERRORCODE_DICT: Option<Obj> = None;

fn errorcode_dict() -> Obj {
    unsafe {
        if ERRORCODE_DICT.is_none() {
            let mut table = Vec::new();
            for &(name, _) in ERRNO_LIST {
                table.push(MapElem {
                    key: obj::new_small_int(errno_value(name) as isize),
                    value: obj::new_qstr(qstr::from_str(name)),
                });
            }
            let ptr = malloc::new_obj::<ObjDict>().expect("errorcode dict");
            map::init_fixed_table(&mut (*ptr).map, table);
            (*ptr).map.all_keys_are_qstrs = false;
            (*ptr).base.type_ = objdict::type_dict() as *const ObjType;
            ERRORCODE_DICT = Some(obj::from_ptr(ptr as *const ObjDict as *const ()));
        }
        ERRORCODE_DICT.unwrap()
    }
}

/// `mp_errno_to_str`
pub fn errno_to_str(errno_val: Obj) -> Qstr {
    if !mpconfig::PY_ERRNO {
        return qstr::QSTR_NULL;
    }
    if mpconfig::PY_ERRNO_ERRORCODE {
        let dict = errorcode_dict();
        let map = unsafe { &mut (*objdict::dict_ptr(dict)).map };
        if let Some(elem) = map::lookup(map, errno_val, LookupKind::Lookup) {
            if obj::is_qstr(elem.value) {
                return obj::qstr_value(elem.value);
            }
        }
        qstr::QSTR_NULL
    } else {
        for &(name, _) in ERRNO_LIST {
            if errno_val == obj::new_int(errno_value(name) as isize) {
                return qstr::from_str(name);
            }
        }
        qstr::QSTR_NULL
    }
}

pub fn init_module() -> Obj {
    if !mpconfig::PY_ERRNO {
        return obj::OBJ_NULL;
    }
    let mut table = vec![MapElem {
        key: obj::new_qstr(qstr::from_str("__name__")),
        value: obj::new_qstr(qstr::from_str("errno")),
    }];
    if mpconfig::PY_ERRNO_ERRORCODE {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str("errorcode")),
            value: errorcode_dict(),
        });
    }
    for &(name, _) in ERRNO_LIST {
        table.push(MapElem {
            key: obj::new_qstr(qstr::from_str(name)),
            value: obj::new_small_int(errno_value(name) as isize),
        });
    }
    let ctx = malloc::new_obj::<ModuleContext>().expect("errno module");
    let dict = objdict::new_dict(table.len());
    unsafe {
        map::init_fixed_table(&mut (*objdict::dict_ptr(dict)).map, table);
        (*ctx).module.base.type_ = objmodule::type_module();
        (*ctx).module.globals = objdict::dict_ptr(dict);
        (*ctx).constants = Default::default();
    }
    let module = obj::from_ptr(ctx as *const ModuleContext as *const ());
    objmodule::register_builtin_module(qstr::from_str("errno"), module);
    module
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc;

    #[test]
    fn errno_to_str_lookup() {
        let _ = gc::init();
        crate::qstr::init();
        crate::runtime::init();
        init_module();
        let q = errno_to_str(obj::new_small_int(mperrno::ENOENT as isize));
        if q == qstr::QSTR_NULL {
            panic!("ENOENT not found in errno table");
        }
        assert_eq!(qstr::str_from_qstr(q).unwrap(), "ENOENT");
    }
}
