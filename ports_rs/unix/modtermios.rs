//! rewrite of ports/unix/modtermios.c
// symmetry: done

use py_rs::obj::Obj;
use py_rs::objlist;
use py_rs::objstr;
use py_rs::raise::{self, MpRaise};

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn raise_if_err(ret: i32) {
    if ret == -1 {
        raise::raise(MpRaise::OSError(errno()));
    }
}

/// `termios.tcgetattr`
pub fn tcgetattr(fd: Obj) -> Obj {
    let fd = py_rs::obj::get_int(fd) as i32;
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    raise_if_err(unsafe { libc::tcgetattr(fd, &mut term) });
    let mut cc_items = Vec::with_capacity(libc::NCCS as usize);
    for i in 0..libc::NCCS as usize {
        let v = term.c_cc[i];
        cc_items.push(if i == libc::VMIN as usize || i == libc::VTIME as usize {
            py_rs::obj::new_small_int(v as isize)
        } else {
            objstr::new_str(&[v])
        });
    }
    let cc = objlist::new_list(libc::NCCS as usize, Some(&cc_items));
    let items = [
        py_rs::obj::new_small_int(term.c_iflag as isize),
        py_rs::obj::new_small_int(term.c_oflag as isize),
        py_rs::obj::new_small_int(term.c_cflag as isize),
        py_rs::obj::new_small_int(term.c_lflag as isize),
        py_rs::obj::new_small_int(unsafe { libc::cfgetispeed(&term) } as isize),
        py_rs::obj::new_small_int(unsafe { libc::cfgetospeed(&term) } as isize),
        cc,
    ];
    objlist::new_list(7, Some(&items))
}

/// `termios.tcsetattr`
pub fn tcsetattr(fd: Obj, when: Obj, attrs: Obj) -> Obj {
    let fd = py_rs::obj::get_int(fd) as i32;
    let mut when = py_rs::obj::get_int(when) as i32;
    if when == 0 {
        when = libc::TCSANOW;
    }
    let (len, attr_items) = objlist::list_get(attrs);
    if len < 7 {
        raise::raise(MpRaise::TypeError("tcsetattr attrs list too short"));
    }
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    term.c_iflag = py_rs::obj::get_int(attr_items[0]) as libc::tcflag_t;
    term.c_oflag = py_rs::obj::get_int(attr_items[1]) as libc::tcflag_t;
    term.c_cflag = py_rs::obj::get_int(attr_items[2]) as libc::tcflag_t;
    term.c_lflag = py_rs::obj::get_int(attr_items[3]) as libc::tcflag_t;
    let (_, cc_items) = objlist::list_get(attr_items[6]);
    for i in 0..libc::NCCS as usize {
        term.c_cc[i] = if i == libc::VMIN as usize || i == libc::VTIME as usize {
            py_rs::obj::get_int(cc_items[i]) as libc::cc_t
        } else {
            let s = objstr::str_get_str(cc_items[i]);
            s.as_bytes().first().copied().unwrap_or(0)
        };
    }
    raise_if_err(unsafe {
        libc::cfsetispeed(&mut term, py_rs::obj::get_int(attr_items[4]) as libc::speed_t)
    });
    raise_if_err(unsafe {
        libc::cfsetospeed(&mut term, py_rs::obj::get_int(attr_items[5]) as libc::speed_t)
    });
    raise_if_err(unsafe { libc::tcsetattr(fd, when, &term) });
    py_rs::obj::CONST_NONE
}

/// `termios.setraw`
pub fn setraw(fd: Obj) -> Obj {
    let fd = py_rs::obj::get_int(fd) as i32;
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    raise_if_err(unsafe { libc::tcgetattr(fd, &mut term) });
    term.c_iflag &= !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
    term.c_oflag = 0;
    term.c_cflag = (term.c_cflag & !(libc::CSIZE | libc::PARENB)) | libc::CS8;
    term.c_lflag = 0;
    term.c_cc[libc::VMIN as usize] = 1;
    term.c_cc[libc::VTIME as usize] = 0;
    raise_if_err(unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &term) });
    py_rs::obj::CONST_NONE
}

/// Module-level constants exported by unix termios.
pub fn module_constants() -> Vec<(&'static str, i32)> {
    vec![("TCSANOW", libc::TCSANOW), ("B9600", libc::B9600 as i32)]
}
