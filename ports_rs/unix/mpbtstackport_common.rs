//! rewrite of ports/unix/mpbtstackport_common.c
// symmetry: done

use py_rs::mphal;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BtStackState {
    Off,
    Starting,
    Active,
    Halting,
    Timeout,
}

static mut STATE: BtStackState = BtStackState::Off;

pub fn btstack_state() -> BtStackState {
    unsafe { STATE }
}

pub fn set_btstack_state(s: BtStackState) {
    unsafe {
        STATE = s;
    }
}

/// `mp_bluetooth_hci_poll`
pub fn hci_poll() -> bool {
    let state = btstack_state();
    if matches!(
        state,
        BtStackState::Starting | BtStackState::Active | BtStackState::Halting
    ) {
        let _state = super::mphalport::begin_atomic_section();
        super::mpbtstackport_h4::hci_poll_h4();
        super::mphalport::end_atomic_section(0);
        return true;
    }
    false
}

pub fn hci_active() -> bool {
    !matches!(btstack_state(), BtStackState::Off | BtStackState::Timeout)
}

pub fn hal_cpu_disable_irqs() {}
pub fn hal_cpu_enable_irqs() {}
pub fn hal_cpu_enable_irqs_and_sleep() {}
pub fn hal_time_ms() -> u32 {
    mphal::ticks_ms() as u32
}

/// `mp_bluetooth_btstack_port_init`
pub fn port_init() {
    super::mpbtstackport_h4::port_init_h4();
    super::mpbtstackport_usb::port_init_usb();
}
