//! rewrite of ports/qemu/modmachine.c
// symmetry: done

/// `mp_machine_idle` — no-op on qemu.
pub fn machine_idle() {}

/// `mp_machine_reset` — exit via semihosting / process exit.
pub fn machine_reset() {
    std::process::exit(0);
}

/// `mp_machine_reset_cause` — not implemented on qemu.
pub fn machine_reset_cause() -> i32 {
    0
}
