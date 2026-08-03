//! Wired `pm_mpy_machine_*` accessors.
// symmetry: done

use super::machine::machine_export;
use py_rs::pm::mpy::pm_mpy_obj_t;

/// `pm_mpy_machine_mem8` — return the `mem8` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_mem8() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("mem8"))
}

/// `pm_mpy_machine_mem16` — return the `mem16` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_mem16() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("mem16"))
}

/// `pm_mpy_machine_mem32` — return the `mem32` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_mem32() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("mem32"))
}

/// `pm_mpy_machine_mem_backup` — return the `mem_backup` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_mem_backup() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("mem_backup"))
}

/// `pm_mpy_machine_unique_id` — return the `unique_id` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_unique_id() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("unique_id"))
}

/// `pm_mpy_machine_soft_reset` — return the `soft_reset` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_soft_reset() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("soft_reset"))
}

/// `pm_mpy_machine_bootloader` — return the `bootloader` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_bootloader() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("bootloader"))
}

/// `pm_mpy_machine_reset` — return the `reset` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_reset() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("reset"))
}

/// `pm_mpy_machine_reset_cause` — return the `reset_cause` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_reset_cause() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("reset_cause"))
}

/// `pm_mpy_machine_idle` — return the `idle` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_idle() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("idle"))
}

/// `pm_mpy_machine_freq` — return the `freq` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_freq() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("freq"))
}

/// `pm_mpy_machine_lightsleep` — return the `lightsleep` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_lightsleep() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("lightsleep"))
}

/// `pm_mpy_machine_deepsleep` — return the `deepsleep` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_deepsleep() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("deepsleep"))
}

/// `pm_mpy_machine_disable_irq` — return the `disable_irq` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_disable_irq() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("disable_irq"))
}

/// `pm_mpy_machine_enable_irq` — return the `enable_irq` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_enable_irq() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("enable_irq"))
}

/// `pm_mpy_machine_bitstream` — return the `bitstream` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_bitstream() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("bitstream"))
}

/// `pm_mpy_machine_dht_readinto` — return the `dht_readinto` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_dht_readinto() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("dht_readinto"))
}

/// `pm_mpy_machine_time_pulse_us` — return the `time_pulse_us` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_time_pulse_us() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("time_pulse_us"))
}

/// `pm_mpy_machine_PinBase` — return the `PinBase` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_PinBase() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("PinBase"))
}

/// `pm_mpy_machine_Signal` — return the `Signal` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_Signal() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("Signal"))
}

/// `pm_mpy_machine_SoftI2C` — return the `SoftI2C` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_SoftI2C() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("SoftI2C"))
}

/// `pm_mpy_machine_SoftSPI` — return the `SoftSPI` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_SoftSPI() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("SoftSPI"))
}

/// `pm_mpy_machine_ADC` — return the `ADC` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_ADC() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("ADC"))
}

/// `pm_mpy_machine_ADCBlock` — return the `ADCBlock` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_ADCBlock() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("ADCBlock"))
}

/// `pm_mpy_machine_CAN` — return the `CAN` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_CAN() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("CAN"))
}

/// `pm_mpy_machine_DAC` — return the `DAC` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_DAC() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("DAC"))
}

/// `pm_mpy_machine_I2C` — return the `I2C` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_I2C() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("I2C"))
}

/// `pm_mpy_machine_I2CTarget` — return the `I2CTarget` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_I2CTarget() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("I2CTarget"))
}

/// `pm_mpy_machine_I2S` — return the `I2S` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_I2S() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("I2S"))
}

/// `pm_mpy_machine_PWM` — return the `PWM` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_PWM() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("PWM"))
}

/// `pm_mpy_machine_SDCard` — return the `SDCard` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_SDCard() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("SDCard"))
}

/// `pm_mpy_machine_SPI` — return the `SPI` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_SPI() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("SPI"))
}

/// `pm_mpy_machine_UART` — return the `UART` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_UART() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("UART"))
}

/// `pm_mpy_machine_USBDevice` — return the `USBDevice` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_USBDevice() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("USBDevice"))
}

/// `pm_mpy_machine_WDT` — return the `WDT` export from `machine`.
#[no_mangle]
pub unsafe extern "C" fn pm_mpy_machine_WDT() -> pm_mpy_obj_t {
    pm_mpy_obj_t::from_obj(machine_export("WDT"))
}
