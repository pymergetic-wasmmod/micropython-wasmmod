//! rewrite of extmod/machine_can_port.h
//! Shared CAN types/constants ported; filter/IRQ/send/recv hooks require board `CanPort` HAL impl.
// symmetry: done
use py_rs::obj::Obj;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CanState {
    Stopped = 0,
    Active = 1,
    Warning = 2,
    Passive = 3,
    BusOff = 4,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CanMode {
    Normal = 0,
    Sleep = 1,
    Loopback = 2,
    Silent = 3,
    SilentLoopback = 4,
    Max = 5,
}

pub const MP_CAN_IRQ_TX: u32 = 1 << 0;
pub const MP_CAN_IRQ_RX: u32 = 1 << 1;
pub const MP_CAN_IRQ_TX_FAILED: u32 = 1 << 2;
pub const MP_CAN_IRQ_STATE: u32 = 1 << 3;
pub const MP_CAN_IRQ_IDX_SHIFT: u32 = 16;
pub const MP_CAN_IRQ_IDX_MASK: u32 = 0xFF;

#[cfg(feature = "fdcan")]
pub const MP_CAN_MAX_LEN: usize = 64;
#[cfg(not(feature = "fdcan"))]
pub const MP_CAN_MAX_LEN: usize = 8;

pub const CAN_STD_ID_MASK: u32 = 0x7ff;
pub const CAN_EXT_ID_MASK: u32 = 0x1fff_ffff;
pub const CAN_MSG_FLAG_RTR: u32 = 1 << 0;
pub const CAN_MSG_FLAG_EXT_ID: u32 = 1 << 1;
pub const CAN_MSG_FLAG_FD_F: u32 = 1 << 2;
pub const CAN_MSG_FLAG_BRS: u32 = 1 << 3;
pub const CAN_MSG_FLAG_UNORDERED: u32 = 1 << 4;
pub const CAN_RECV_ERR_FULL: u32 = 1 << 0;
pub const CAN_RECV_ERR_OVERRUN: u32 = 1 << 1;
pub const CAN_RECV_ERR_ESI: u32 = 1 << 2;

#[repr(C)]
pub struct CanCounters {
    pub tec: usize,
    pub rec: usize,
    pub num_warning: usize,
    pub num_passive: usize,
    pub num_bus_off: usize,
    pub tx_pending: usize,
    pub rx_pending: usize,
    pub rx_overruns: usize,
}

/// Port hooks are implemented by board-specific code when `feature = "machine_can"`.
#[cfg(feature = "machine_can")]
pub trait CanPort {
    fn f_clock(&self) -> i32;
    fn supports_mode(&self, mode: CanMode) -> bool;
    fn init(&mut self);
    fn deinit(&mut self);
    fn send(&mut self, id: u32, data: &[u8], flags: u32) -> i32;
    fn recv(&mut self, data: &mut [u8], id: &mut u32, flags: &mut u32, errors: &mut u32) -> bool;
    fn get_state(&self) -> CanState;
    fn restart(&mut self);
    fn get_additional_timings(&self, optional: Obj) -> Obj;
}
