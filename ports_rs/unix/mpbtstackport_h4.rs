//! rewrite of ports/unix/mpbtstackport_h4.c
// symmetry: done

use super::mpbtstackport_common::{btstack_state, BtStackState};

/// `mp_bluetooth_hci_poll_h4`
pub fn hci_poll_h4() {
    if matches!(
        btstack_state(),
        BtStackState::Starting | BtStackState::Active
    ) {
        // mp_bluetooth_btstack_hci_uart_process();
    }
}

/// `mp_bluetooth_btstack_port_init_h4`
pub fn port_init_h4() {
    // hci_init(hci_transport_h4_instance_for_uart(...));
    let _cfg = HciTransportConfigUart {
        baudrate_init: 1_000_000,
        baudrate_main: 0,
        flowcontrol: 1,
        parity_off: true,
    };
}

/// UART transport config (`hci_transport_config_uart_t`).
#[derive(Clone, Copy, Debug)]
pub struct HciTransportConfigUart {
    pub baudrate_init: u32,
    pub baudrate_main: u32,
    pub flowcontrol: u8,
    pub parity_off: bool,
}

pub fn port_deinit() {
    // hci_power_control(HCI_POWER_OFF); hci_close();
}

pub fn port_start() {
    // hci_power_control(HCI_POWER_ON);
}
