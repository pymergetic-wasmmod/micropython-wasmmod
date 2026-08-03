//! rewrite of ports/unix/mpbtstackport_usb.c
// symmetry: done

use std::thread;
use std::time::Duration;

const USB_POLL_INTERVAL_US: u64 = 1000;

/// `mp_bluetooth_btstack_port_init_usb`
pub fn port_init_usb() {
    if let Ok(path) = std::env::var("MICROPYBTUSB") {
        let usb_path = parse_usb_path(&path);
        let _ = usb_path;
        // hci_transport_usb_set_path(...); hci_init(hci_transport_usb_instance(), NULL);
    }
}

fn parse_usb_path(path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for part in path.split([':', '-']) {
        if let Ok(v) = u8::from_str_radix(part, 16) {
            out.push(v);
        }
        if out.len() >= 7 {
            break;
        }
    }
    out
}

pub fn port_deinit() {
    // pthread_join(btstack_thread_id, NULL);
}

/// `mp_bluetooth_btstack_port_start`
pub fn port_start() {
    thread::spawn(|| {
        // hci_power_control(HCI_POWER_ON);
        while super::mpbtstackport::hci_poll() {
            thread::sleep(Duration::from_micros(USB_POLL_INTERVAL_US));
        }
    });
}
