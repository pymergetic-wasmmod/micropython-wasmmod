//! rewrite of ports/unix/mpbthciport.c
// symmetry: done

use std::ffi::CString;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub static HCI_CMD_BUF: Mutex<[u8; 260]> = Mutex::new([0; 260]);

static UART_FD: Mutex<i32> = Mutex::new(-1);

const UART_POLL_INTERVAL_US: u64 = 1000;

/// `mp_bluetooth_hci_uart_init`
pub fn hci_uart_init(_port: u32, _baudrate: u32) -> i32 {
    let mut name = CString::new("/dev/ttyUSB0").unwrap();
    if let Ok(p) = std::env::var("MICROPYBTUART") {
        if let Ok(c) = CString::new(p) {
            name = c;
        }
    }
    let fd = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
        )
    };
    if fd == -1 {
        return -1;
    }
    if configure_uart(fd) != 0 {
        return -1;
    }
    *UART_FD.lock().unwrap() = fd;
    thread::spawn(hci_poll_thread);
    0
}

fn configure_uart(fd: i32) -> i32 {
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return -1;
        }
        libc::cfmakeraw(&mut t);
        t.c_cflag &= !libc::CSTOPB;
        t.c_cflag |= libc::CS8;
        t.c_cflag &= !libc::PARENB;
        t.c_cflag |= libc::CREAD | libc::CLOCAL;
        t.c_cc[libc::VMIN as usize] = 1;
        t.c_cc[libc::VTIME as usize] = 0;
        t.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
        t.c_cflag |= libc::CRTSCTS;
        libc::cfsetospeed(&mut t, libc::B1000000);
        libc::cfsetispeed(&mut t, libc::B1000000);
        if libc::tcsetattr(fd, libc::TCSANOW, &t) != 0 {
            return -1;
        }
    }
    0
}

fn hci_poll_thread() {
    while super::mpbtstackport::hci_poll() || super::mpnimbleport::hci_poll() {
        thread::sleep(Duration::from_micros(UART_POLL_INTERVAL_US));
    }
}

pub fn hci_uart_deinit() -> i32 {
    let fd = *UART_FD.lock().unwrap();
    if fd != -1 {
        unsafe {
            libc::close(fd);
        }
        *UART_FD.lock().unwrap() = -1;
    }
    0
}

pub fn hci_uart_set_baudrate(_baudrate: u32) -> i32 {
    0
}

pub fn hci_uart_readchar() -> i32 {
    let fd = *UART_FD.lock().unwrap();
    if fd == -1 {
        return -1;
    }
    let mut c = 0u8;
    let n = unsafe { libc::read(fd, &mut c as *mut _ as *mut _, 1) };
    if n == 1 {
        c as i32
    } else {
        -1
    }
}

pub fn hci_uart_write(buf: &[u8]) -> isize {
    let fd = *UART_FD.lock().unwrap();
    if fd == -1 {
        return 0;
    }
    unsafe { libc::write(fd, buf.as_ptr() as *const _, buf.len()) as isize }
}

pub fn hci_controller_init() -> i32 {
    0
}
pub fn hci_controller_deinit() -> i32 {
    0
}
pub fn hci_controller_sleep_maybe() -> i32 {
    0
}
pub fn hci_controller_woken() -> bool {
    true
}
pub fn hci_controller_wakeup() -> i32 {
    0
}
