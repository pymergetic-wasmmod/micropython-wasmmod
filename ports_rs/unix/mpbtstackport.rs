//! rewrite of ports/unix/mpbtstackport.h
// symmetry: done

pub const HW_BLE_UART_ID: u32 = 0;
pub const HW_BLE_UART_BAUDRATE: u32 = 1_000_000;

#[cfg(all(feature = "bluetooth_btstack_h4", feature = "bluetooth_btstack_usb"))]
compile_error!("only one btstack transport");

pub fn hci_poll() -> bool {
    super::mpbtstackport_common::hci_poll()
}

#[cfg(feature = "bluetooth_btstack_h4")]
pub fn hci_poll_h4() { super::mpbtstackport_h4::hci_poll_h4(); }

#[cfg(feature = "bluetooth_btstack_h4")]
pub fn port_init_h4() { super::mpbtstackport_h4::port_init_h4(); }

#[cfg(feature = "bluetooth_btstack_usb")]
pub fn port_init_usb() { super::mpbtstackport_usb::port_init_usb(); }
