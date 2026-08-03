//! rewrite of ports/unix/modmachine.c
// symmetry: done

use py_rs::obj::Obj;
use py_rs::raise::{self, MpRaise};
use std::sync::Mutex;

const PAGE_SIZE: usize = 4096;
const PAGE_MASK: usize = PAGE_SIZE - 1;

struct DevMemState {
    fd: i32,
    last_base: usize,
    map_page: usize,
}

static DEV_MEM: Mutex<Option<DevMemState>> = Mutex::new(None);

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

/// `mod_machine_mem_get_addr`
pub fn machine_mem_get_addr(addr_o: Obj, align: usize) -> usize {
    let addr = py_rs::obj::get_int_truncated(addr_o) as usize;
    if align > 0 && (addr & (align - 1)) != 0 {
        raise::raise(MpRaise::ValueError("address is not aligned"));
    }
    if !crate::mpconfigport::PLAT_DEV_MEM {
        return addr;
    }
    let mut guard = DEV_MEM.lock().unwrap();
    if guard.is_none() {
        let fd = unsafe { libc::open(c"/dev/mem".as_ptr(), libc::O_RDWR | libc::O_SYNC) };
        if fd == -1 {
            raise::raise(MpRaise::OSError(errno()));
        }
        *guard = Some(DevMemState {
            fd,
            last_base: usize::MAX,
            map_page: 0,
        });
    }
    let st = guard.as_mut().unwrap();
    let cur_base = addr & !PAGE_MASK;
    if cur_base != st.last_base {
        let map = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                PAGE_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                st.fd,
                cur_base as libc::off_t,
            )
        };
        st.map_page = map as usize;
        st.last_base = cur_base;
    }
    st.map_page + (addr & PAGE_MASK)
}

/// `mp_machine_idle`
pub fn machine_idle() {
    extmod_rs::machine_timer::host_service_poll();
    crate::mpconfigport::machine_idle();
}
