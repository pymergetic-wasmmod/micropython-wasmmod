//! rewrite of ports/unix/mpnimbleport.c + ports/unix/mpnimbleport.h
// symmetry: done

pub const HW_BLE_UART_ID: u32 = 0;
pub const HW_BLE_UART_BAUDRATE: u32 = 1_000_000;

/// `mp_bluetooth_hci_poll` (NimBLE path).
pub fn hci_poll() -> bool {
    false
}

pub fn hci_active() -> bool { hci_poll() }

pub fn hci_uart_wfi() {
    #[cfg(feature = "bluetooth_nimble")]
    { /* mp_bluetooth_nimble_hci_uart_process(false) */ }
}
